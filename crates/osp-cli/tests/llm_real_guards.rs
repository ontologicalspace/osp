//! #171 — llm-real deney-destek bayraklarının CLI uç testleri.
//!
//! Hepsi navigator/analyze ÇALIŞMADAN önce red eden guard'ları ve fence'leri
//! doğrular; bu yüzden `OPENAI_API_KEY` olmadan offline koşar (`env_remove`
//! ile anahtar bilinçli kaldırılır — makinede olsa bile test gerçek çağrı
//! yapmaz).

#![cfg(test)]

mod common;

use common::*;

/// Guard öncesi başarısız kombinasyonlar — network/anahtar GEREKMEZ.
/// (`--llm` DEĞERİ ÇAĞIRANDAN gelir — mock tabanıyla çakışma olmasın.)
fn attempt_with(fx: &HarnessFixture, extra: &[&str]) -> std::process::Output {
    fx.run_attempt_no_task(|cmd| {
        cmd.arg("7")
            .arg("--repo")
            .arg(fx.repo_path())
            .args(extra)
            .env_remove("OPENAI_API_KEY")
    })
}

/// Mock tabanlı kombinasyonlar (guard, mock tarafında red eder).
fn guarded_mock_attempt(fx: &HarnessFixture, extra: &[&str]) -> std::process::Output {
    attempt_with(
        fx,
        &[
            vec!["--llm", "mock", "--proposals", "/dev/null"],
            extra.to_vec(),
        ]
        .concat(),
    )
}

/// Repo DIŞINDA, parent'ı VAR bir kanıt dizini (guard'ın geçmesi için).
fn external_artifacts_dir(fx: &HarnessFixture) -> String {
    let dir = fx.work_path().join("llm-artifacts-dir");
    std::fs::create_dir_all(&dir).expect("mkdir external artifacts dir");
    dir.to_string_lossy().to_string()
}

#[test]
fn real_without_llm_artifacts_rejected_before_any_work() {
    let fx = HarnessFixture::new();
    let output = attempt_with(&fx, &["--llm", "real"]);
    assert!(
        !output.status.success(),
        "real without artifacts dir must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--llm real requires --llm-artifacts"),
        "stderr: {stderr}"
    );
}

#[test]
fn llm_artifacts_with_mock_rejected() {
    let fx = HarnessFixture::new();
    let external = external_artifacts_dir(&fx);
    let output = guarded_mock_attempt(&fx, &["--llm-artifacts", &external]);
    assert!(
        !output.status.success(),
        "artifacts dir with mock must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--llm-artifacts is only used with --llm real"),
        "stderr: {stderr}"
    );
}

#[test]
fn bar_elicitation_with_mock_rejected() {
    let fx = HarnessFixture::new();
    let output = guarded_mock_attempt(&fx, &["--bar-elicitation"]);
    assert!(
        !output.status.success(),
        "bar elicitation with mock must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--bar-elicitation requires --llm real"),
        "stderr: {stderr}"
    );
}

#[test]
fn llm_artifacts_inside_analyzed_repo_rejected() {
    let fx = HarnessFixture::new();
    let inside = fx.repo_path().join("llm-evidence");
    let output = attempt_with(
        &fx,
        &[
            "--llm",
            "real",
            "--llm-artifacts",
            &inside.to_string_lossy(),
        ],
    );
    assert!(
        !output.status.success(),
        "artifacts dir inside repo must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr: {stderr}"
    );
}

#[test]
fn real_with_valid_artifacts_dir_passes_guards_then_fails_at_missing_key() {
    // Guard'lar GEÇİLİR (dizin dışarıda, parent var) → bir sonraki fence API
    // anahtarıdır: guard sırası (flag → path → key) bu testle pinlenir. Gerçek
    // ağ çağrısı YAPILMAZ (anahtar env_remove ile kesin yok).
    let fx = HarnessFixture::new();
    let external = external_artifacts_dir(&fx);
    let output = attempt_with(&fx, &["--llm", "real", "--llm-artifacts", &external]);
    assert!(
        !output.status.success(),
        "without OPENAI_API_KEY the run must fail (no network)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LLM runtime") && stderr.contains("OPENAI_API_KEY"),
        "key fence mesajı: {stderr}"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// #171 review P0 — D6 strict retry semantiğinin UÇTAN UCA doğrulanması.
// Loopback sahte OpenAI endpoint'i (offline — yalnız 127.0.0.1; dış ağ YOK):
// yanıt gövdeleri ve bağlantı-düşürme adımları testte script'lenir; istek
// gövdeleri bayt-bayt kaydedilir (byte-özdeşlik iddiası wire düzeyinde kanıt).
// ═════════════════════════════════════════════════════════════════════════════

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

enum EndpointStep {
    /// 200 + verilen gövde (OpenAI chat-completion envelope'u).
    Respond(String),
    /// Verilen status kodu + gövde (ör. 401 — non-retryable sınıfı).
    RespondStatus(u16, String),
    /// İsteği OKU (kayda geçer) ama bağlantıyı yanıtsız düşür (transport hatası).
    DropConnection,
}

/// Tek-atımlık loopback endpoint: her bağlantıda script'teki sıradaki adım.
/// `watch` verilirse İLK istek geldiğinde o path'in diskte VAR olup olmadığını
/// rapor eder (P0 "freeze önce kanıtı" wire pin'i).
struct FakeEndpoint {
    port: u16,
    requests: mpsc::Receiver<Vec<u8>>,
    watch_first_request: Option<mpsc::Receiver<bool>>,
}

impl FakeEndpoint {
    fn start(steps: Vec<EndpointStep>) -> Self {
        Self::start_inner(steps, None)
    }

    fn start_watching(steps: Vec<EndpointStep>, watch: std::path::PathBuf) -> Self {
        Self::start_inner(steps, Some(watch))
    }

    fn start_inner(steps: Vec<EndpointStep>, watch: Option<std::path::PathBuf>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let (wtx, wrx) = mpsc::channel();
        let watch_channel = watch.is_some().then_some(wrx);
        thread::spawn(move || {
            for (i, step) in steps.into_iter().enumerate() {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                if i == 0 {
                    if let Some(path) = &watch {
                        // İstek SIRASINDA dosya var mı? (P0: çağrı-öncesi freeze)
                        let _ = wtx.send(path.exists());
                    }
                }
                // Bir HTTP isteği oku: başlıklar + Content-Length gövdesi.
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(header_end) = find_subslice(&buf, b"\r\n\r\n") {
                        let content_length = buf[header_end..]
                            .windows(16)
                            .find(|w| w[..16].eq_ignore_ascii_case(b"content-length:"))
                            .map(|w| {
                                let line_end = w.iter().position(|b| *b == b'\r').unwrap();
                                String::from_utf8_lossy(&w[16..line_end])
                                    .trim()
                                    .parse::<usize>()
                                    .unwrap_or(0)
                            })
                            .unwrap_or(0);
                        if buf.len() >= header_end + 4 + content_length {
                            break;
                        }
                    }
                }
                let _ = tx.send(buf.clone());
                match step {
                    EndpointStep::Respond(body) => {
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    EndpointStep::RespondStatus(code, body) => {
                        let reason = match code {
                            401 => "Unauthorized",
                            429 => "Too Many Requests",
                            _ => "Status",
                        };
                        let response = format!(
                            "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    EndpointStep::DropConnection => {
                        // Yanıt YOK — istemci transport hatası görür (D6 retry tetiklenir).
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    }
                }
            }
        });
        Self {
            port,
            requests: rx,
            watch_first_request: watch_channel,
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// OpenAI envelope'u — content alanı serbest (geçerli proposal VEYA çöp).
fn openai_body(content: &str) -> String {
    format!(
        r#"{{"choices":[{{"message":{{"content":{}}}}}],"usage":{{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}}}"#,
        serde_json::to_string(content).unwrap()
    )
}

fn attempt_real_against(fx: &HarnessFixture, port: u16) -> (std::process::Output, String) {
    let artifacts = external_artifacts_dir(fx);
    let output = attempt_real_dir(fx, port, &artifacts);
    (output, artifacts)
}

/// Aynı artifacts dizinine İKİNCİ bir süreç koşusu (cross-process state machine).
fn attempt_real_dir(fx: &HarnessFixture, port: u16, artifacts: &str) -> std::process::Output {
    fx.run_attempt_no_task(|cmd| {
        cmd.arg("7")
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--llm")
            .arg("real")
            .arg("--llm-artifacts")
            .arg(artifacts)
            .env("OPENAI_API_KEY", "sk-loopback-test")
            .env(
                "OSP_LLM_ENDPOINT",
                format!("http://127.0.0.1:{port}/v1/chat/completions"),
            )
    })
}

fn read_artifact_json(artifacts: &str, name: &str) -> serde_json::Value {
    let path = std::path::Path::new(artifacts).join(name);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn d6_invalid_first_response_exactly_one_call_and_honest_null() {
    // Review P0 test #1: invalid ilk yanıt → TAM OLARAK 1 LLM çağrısı →
    // proposal null → değiştirilmiş-prompt tamir completion'ı YOK.
    let fx = HarnessFixture::new();
    let endpoint = FakeEndpoint::start(vec![EndpointStep::Respond(openai_body("not json at all"))]);

    let (output, artifacts) = attempt_real_against(&fx, endpoint.port);
    assert_eq!(
        output.status.code(),
        Some(90),
        "parse ihlali D6 strict modda terminal (LlmError → exit 90); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let requests: Vec<Vec<u8>> = endpoint.requests.try_iter().collect();
    assert_eq!(
        requests.len(),
        1,
        "tam olarak bir çağrı — tamir döngüsü yok"
    );

    let proposals = read_artifact_json(&artifacts, "llm-proposals.json");
    assert!(proposals["proposal"].is_null(), "honest null (D6)");
    assert!(proposals["parse_error"].is_string());
    assert_eq!(proposals["network_retry_count"], 0);

    // Donmuş anchor + INV-E1 provenance bloğu.
    let prompt = read_artifact_json(&artifacts, "llm-prompt.json");
    assert!(prompt["prompt_digest"].is_string());
    assert!(
        prompt["completion_identity_digest"].is_string(),
        "treatment kimliği anchor'da"
    );
    assert!(prompt["inv_e1"]["baseline_digest"].is_string());
    assert_eq!(
        prompt["inv_e1"]["osp_revision"],
        env!("OSP_GIT_REVISION"),
        "osp_revision build-time gömülü değerle"
    );
    assert_eq!(
        prompt["inv_e1"]["files_read"].as_array().map(Vec::len),
        Some(0),
        "procedural blindness: prompt üreticisi dosya OKUMADI"
    );
}

#[test]
fn d6_network_fail_retries_once_byte_identical_and_persists_retry() {
    // Review P0 test #2: ilk bağlantı düşer → AYNI prompt'la tam bir retry →
    // iki çağrı, gövdeler BAYT-ÖZDEŞ; retry metadata kayıtta.
    let fx = HarnessFixture::new();
    let endpoint = FakeEndpoint::start(vec![
        EndpointStep::DropConnection,
        EndpointStep::Respond(openai_body("still not json")),
    ]);

    let (output, artifacts) = attempt_real_against(&fx, endpoint.port);
    // Retry taşımada başarılı ama yanıt parse edilmedi → yine terminal (90);
    // önemli olan çağrı sayısı + byte-özdeşlik + retry kaydı.
    assert_eq!(
        output.status.code(),
        Some(90),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let requests: Vec<Vec<u8>> = endpoint.requests.try_iter().collect();
    assert_eq!(requests.len(), 2, "tam bir retry — üçüncü deneme yok");
    assert_eq!(
        requests[0], requests[1],
        "iki denemenin HTTP gövdeleri bayt-özdeş (system+user+model+sampling)"
    );

    let proposals = read_artifact_json(&artifacts, "llm-proposals.json");
    assert_eq!(
        proposals["network_retry_count"], 1,
        "retry metadata persisted (D6: retry kayda geçer)"
    );
    assert!(proposals["parse_error"].is_string());
    assert!(proposals["proposal"].is_null());
    assert_eq!(proposals["usage"]["total_tokens"], 5);
}

#[test]
fn d6_non_retryable_status_single_call_and_sealed() {
    // Review R2 P1: 401 kalıcı hatadır — retry YOK (tam 1 çağrı), kayıt
    // retry_count=0 ile MÜHÜRLÜ.
    let fx = HarnessFixture::new();
    let endpoint = FakeEndpoint::start(vec![EndpointStep::RespondStatus(
        401,
        r#"{"error":{"message":"bad key"}}"#.to_string(),
    )]);

    let (output, artifacts) = attempt_real_against(&fx, endpoint.port);
    assert_eq!(
        output.status.code(),
        Some(90),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let requests: Vec<Vec<u8>> = endpoint.requests.try_iter().collect();
    assert_eq!(requests.len(), 1, "non-retryable: retry yapılmadı");

    let proposals = read_artifact_json(&artifacts, "llm-proposals.json");
    assert_eq!(proposals["network_retry_count"], 0);
    let err_text = proposals["transport_error"].as_str().unwrap_or_default();
    assert!(
        err_text.contains("401"),
        "hata kaydı 401'i taşır: {err_text}"
    );
    assert!(proposals["proposal"].is_null());
}

#[test]
fn d6_sealed_line_blocks_second_process_before_any_http_call() {
    // Review R2 P1: retry bütçesi EXPERIMENT-LINE bütçesidir — process
    // restart ile sıfırlanamaz. Süreç 1: iki retryable deneme de düşer →
    // transport-exhausted kaydı MÜHÜRLÜ. Süreç 2 (aynı dizin): çağrı-öncesi
    // preflight RED — İKİNCİ endpoint HİÇ istek GÖRMEZ (wire düzeyi kanıtı).
    let fx = HarnessFixture::new();
    let artifacts = external_artifacts_dir(&fx);

    let exhausted = FakeEndpoint::start(vec![
        EndpointStep::DropConnection,
        EndpointStep::DropConnection,
    ]);
    let output1 = attempt_real_dir(&fx, exhausted.port, &artifacts);
    assert_eq!(
        output1.status.code(),
        Some(90),
        "süreç 1: transport tükenmesi terminal; stderr: {}",
        String::from_utf8_lossy(&output1.stderr)
    );
    let requests1: Vec<Vec<u8>> = exhausted.requests.try_iter().collect();
    assert_eq!(requests1.len(), 2, "süreç 1: ilk çağrı + bir retry");
    let proposals = read_artifact_json(&artifacts, "llm-proposals.json");
    assert_eq!(proposals["network_retry_count"], 1);
    assert!(proposals["transport_error"].is_string());

    // Süreç 2: same directory — preflight, HTTP çağrısından ÖNCE reddeder.
    let would_succeed = FakeEndpoint::start(vec![EndpointStep::Respond(openai_body(
        "irrelevant — sealed line must not call",
    ))]);
    let output2 = attempt_real_dir(&fx, would_succeed.port, &artifacts);
    assert_eq!(
        output2.status.code(),
        Some(90),
        "süreç 2: sealed satır terminal; stderr: {}",
        String::from_utf8_lossy(&output2.stderr)
    );
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(
        stderr2.contains("SEALED"),
        "preflight sealed mesajı: {stderr2}"
    );
    let requests2: Vec<Vec<u8>> = would_succeed.requests.try_iter().collect();
    assert_eq!(
        requests2.len(),
        0,
        "wire kanıtı: sealed dizinde hiç HTTP isteği yapılmadı"
    );
    // Kayıt süreç 1'in durumuyla değişmedi (üçüncü çağrı bütçe ihlali yok).
    let proposals_after = read_artifact_json(&artifacts, "llm-proposals.json");
    assert_eq!(
        proposals_after["network_retry_count"], 1,
        "bütçe sıfırlanmadı — kayıt değişmedi"
    );
}

#[test]
fn pre_call_frozen_anchor_exists_when_first_request_arrives() {
    // Review R3 P0 wire pin'i: llm-prompt.json + line claim, İLK HTTP isteği
    // geldiği AN diskte durmak zorunda — "freeze önce kanıtı" sıralaması
    // (anchor çağrı-sonrası evidence DEĞİL, çağrı-öncesi anchor'dur).
    let fx = HarnessFixture::new();
    let artifacts = external_artifacts_dir(&fx);
    let watch = std::path::Path::new(&artifacts).join("llm-prompt.json");
    let endpoint =
        FakeEndpoint::start_watching(vec![EndpointStep::Respond(openai_body("not json"))], watch);

    let output = attempt_real_dir(&fx, endpoint.port, &artifacts);
    assert_eq!(
        output.status.code(),
        Some(90),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let anchor_existed_at_request = endpoint
        .watch_first_request
        .expect("watch channel")
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("server reported watch result");
    assert!(
        anchor_existed_at_request,
        "llm-prompt.json İSTEK ANINDA diskte olmalı (pre-call freeze; P0)"
    );

    // Claim dosyası da isteğe kadar yazılmış olmalı (single-writer sahiplenme).
    let requests: Vec<Vec<u8>> = endpoint.requests.try_iter().collect();
    assert_eq!(requests.len(), 1);
    let claim = read_artifact_json(&artifacts, "llm-line-claim.json");
    assert_eq!(claim["kind"], "proposal-line-claim");
}
