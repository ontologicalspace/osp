//! #171 §5 — llm-real producer-channel kanıt artifact'ları.
//!
//! `trajectory attempt --llm real` çağrısının DONMUŞ prompt'unu, ham yanıtını,
//! parse edilmiş öneriyi (ve D4 bar elicitation'ı) run dizinine yazar:
//!
//! - `llm-prompt.json` — ilk completion'ın (system, user) çifti + digest. Freeze
//!   disiplini (INV-E1): dosya YOKSA create-new ile donar; VARSA yeni prompt'un
//!   digest'i donmuş değerle AYNI olmalıdır (D6: yalnız aynı-prompt ağ retry'sına
//!   izin var — farklı prompt = yeni satır değil YASAK).
//! - `llm-raw-response.txt` — ham asistan metni (dönüştürme YOK).
//! - `llm-proposals.json` — parse edilmiş `DeltaProposal` + usage; parse ihlalinde
//!   `proposal: null` + `parse_error` (D6 dürüst boşluk — tamir döngüsü yok).
//! - `llm-bar.json` — D4 bar elicitation ikinci completion'ı (aynı kör view).
//!
//! Bu dosyalar #178 read-once zincirine GİRMEZ (tüketici değil üretici kanalı);
//! envelope'un `proposals_digest: null` bilinçli boşluğu aynen kalır. Kayıt
//! yalnızca İLK completion'da yapılır (first-freeze): navigator'ın feedback
//! retry'ları prompt'u değiştirir ve donmuş artifact'ı kirletirler — onların
//! kanıt izi attempt envelope'unun evidence dizisindedir.

use std::io::Write as _;
use std::path::Path;

use osp_core::agent::DeltaProposal;
use serde::{Deserialize, Serialize};

use crate::response::{strip_code_fence, RawCompletion, TokenUsage};
use crate::runtime::CompletionRequest;

/// `sha256:<64 hex>` — house digest formatı (finalize-run K3 ile aynı).
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Prompt-pair wire — digest tam olarak bu canonical serileştirme üzerindedir
/// (alan sırası sabit: system, user; JSON escaping sınır belirsizliğini kapatır).
#[derive(Serialize)]
struct PromptPair<'a> {
    system: &'a str,
    user: &'a str,
}

/// (system, user) çiftinin donmuş kimliği — `llm-prompt.json` / `llm-bar.json`
/// `prompt_digest` alanı ve freeze-doğrulaması bu değer üzerinden işler.
pub fn completion_prompt_digest(req: &CompletionRequest) -> String {
    let pair = PromptPair {
        system: &req.system,
        user: &req.user,
    };
    let canonical = serde_json::to_string(&pair).expect("string-only pair serialize");
    sha256_hex(canonical.as_bytes())
}

// ── llm-prompt.json ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PromptDoc<'a> {
    schema_version: u32,
    kind: &'a str,
    model: &'a str,
    endpoint: &'a str,
    system: &'a str,
    user: &'a str,
    prompt_digest: &'a str,
}

/// llm-prompt.json freeze çekirdeği: dosya yoksa no-clobber dondurur; varsa
/// donmuş digest'i `digest` ile karşılaştırır (aynı = D6 retry, farklı = red).
fn freeze_prompt(dir: &Path, doc: &PromptDoc<'_>, digest: &str) -> anyhow::Result<()> {
    let path = dir.join("llm-prompt.json");
    if path.exists() {
        let frozen = std::fs::read_to_string(&path)?;
        let existing: PromptDoc = serde_json::from_str(&frozen).map_err(|e| {
            anyhow::anyhow!(
                "llm-prompt.json exists but is unreadable ({path_display}): {e} — the \
                     frozen prompt record must stay parseable; resolve manually (do not delete \
                     blindly: it is the pre-registration anchor for this directory)",
                path_display = path.display()
            )
        })?;
        if existing.prompt_digest != digest {
            anyhow::bail!(
                "llm-prompt.json is already FROZEN for this directory with a different prompt \
                 (frozen {}, new {}). The pre-registered prompt cannot change mid-experiment \
                 (#171 INV-E1 / D6): a different prompt is a NEW experiment line, not a retry \
                 — use a fresh --llm-artifacts directory.",
                existing.prompt_digest,
                digest
            );
        }
        return Ok(());
    }
    let payload = serde_json::to_vec_pretty(doc)?;
    no_clobber_publish(&path, &payload)
}

// ── llm-proposals.json ─────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
struct ProposalsDoc<'a> {
    schema_version: u32,
    usage: TokenUsage,
    proposal: Option<&'a DeltaProposal>,
    parse_error: Option<&'a str>,
}

// ── llm-bar.json (D4) ──────────────────────────────────────────────────────────

/// Elicitasyon öngörüsü: tek bir aday değişikliğin etiketi + öngörülen
/// değişim-sonrası coupling. Sıra (vec index) = öngörülen etki sırası (τ).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictedEffect {
    pub label: String,
    pub predicted_coupling: f64,
}

/// D4 bar elicitation'ın parse edilmiş yanıtı: kabul barı (öngörülen
/// değişim-sonrası coupling) + önerilerin öngörülen-etki sıralaması (τ).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElicitedBar {
    pub bar_value: f64,
    pub predicted: Vec<PredictedEffect>,
}

/// Elicitation yanıtını parse et (code-fence toleransı proposal yoluyla aynı).
pub fn parse_elicited_bar(content: &str) -> Result<ElicitedBar, serde_json::Error> {
    let json = strip_code_fence(content.trim());
    serde_json::from_str::<ElicitedBar>(&json)
}

#[derive(Serialize, Debug)]
struct BarDoc<'a> {
    schema_version: u32,
    kind: &'static str,
    model: &'a str,
    endpoint: &'a str,
    system: &'a str,
    user: &'a str,
    prompt_digest: &'a str,
    usage: Option<TokenUsage>,
    raw_response: Option<&'a str>,
    elicited: Option<&'a ElicitedBar>,
    parse_error: Option<&'a str>,
    transport_error: Option<&'a str>,
}

// ── üst-düzey kayıt API'leri ───────────────────────────────────────────────────

/// İlk llm-real proposal completion'ını kaydet (first-freeze; §5).
///
/// `parse_error = Some` ⇒ `proposal: null` dürüst boşluğu (D6). Disk hatası
/// çağıranı hatayla bırakır: kanıt persist'i protokolün parçasıdır, sessiz
/// unpersisted gerçek çağrı olmaz.
#[allow(clippy::too_many_arguments)]
pub fn record_proposal_completion(
    dir: &Path,
    model: &str,
    endpoint: &str,
    req: &CompletionRequest,
    raw: &RawCompletion,
    proposal: Option<&DeltaProposal>,
    parse_error: Option<&str>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let digest = completion_prompt_digest(req);
    freeze_prompt(
        dir,
        &PromptDoc {
            schema_version: 1,
            kind: "llm-proposal-prompt",
            model,
            endpoint,
            system: &req.system,
            user: &req.user,
            prompt_digest: &digest,
        },
        &digest,
    )?;
    atomic_replace(&dir.join("llm-raw-response.txt"), raw.content.as_bytes())?;
    let doc = ProposalsDoc {
        schema_version: 1,
        usage: raw.usage,
        proposal,
        parse_error,
    };
    atomic_replace(
        &dir.join("llm-proposals.json"),
        &serde_json::to_vec_pretty(&doc)?,
    )?;
    Ok(())
}

/// D4 bar elicitation kaydı — transport hatası ve parse ihlali de DÜRÜST boşluk
/// olarak yazılır (elicit_bar asla sessiz atlamaz; disk hatası hariç fail yok).
#[allow(clippy::too_many_arguments)]
pub fn record_bar_elicitation(
    dir: &Path,
    model: &str,
    endpoint: &str,
    req: &CompletionRequest,
    raw: Option<&RawCompletion>,
    elicited: Option<&ElicitedBar>,
    parse_error: Option<&str>,
    transport_error: Option<&str>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let digest = completion_prompt_digest(req);
    let doc = BarDoc {
        schema_version: 1,
        kind: "llm-bar-elicitation",
        model,
        endpoint,
        system: &req.system,
        user: &req.user,
        prompt_digest: &digest,
        usage: raw.map(|r| r.usage),
        raw_response: raw.map(|r| r.content.as_str()),
        elicited,
        parse_error,
        transport_error,
    };
    atomic_replace(&dir.join("llm-bar.json"), &serde_json::to_vec_pretty(&doc)?)
}

// ── atomik yazım çekirdekleri ──────────────────────────────────────────────────

/// Unique same-dir temp (`create_new`) → write+sync → rename-replace.
/// Düz `fs::write`'ın truncate penceresini kapatır (osp-cli stage_temp emsali).
fn atomic_replace(out: &Path, payload: &[u8]) -> anyhow::Result<()> {
    let tmp = stage_temp(out, payload)?;
    if let Err(e) = std::fs::rename(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        anyhow::bail!("rename failed for {}: {e}", out.display());
    }
    Ok(())
}

/// No-clobber publish (freeze): temp + sync + `hard_link` (`fs::rename` hedefi
/// REPLACE eder — freeze'e uygun değil; osp-cli canonical-store emsali).
fn no_clobber_publish(out: &Path, payload: &[u8]) -> anyhow::Result<()> {
    let tmp = stage_temp(out, payload)?;
    match std::fs::hard_link(&tmp, out) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Yarış penceresi: başka process aynı anda dondurdu — çağıranın
            // verify dalı bunu digest karşılaştırmasıyla çözer; burada kalan
            // temp temizlenir ve mevcut dosya güvenilebilir kabul EDİLEMEZ,
            // bu yüzden hata verip doğrulama dalına bırak.
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!(
                "llm-prompt.json appeared concurrently while freezing {}; re-run to verify \
                 the frozen digest matches",
                out.display()
            );
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("no-clobber publish failed for {}: {e}", out.display());
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

fn stage_temp(out: &Path, payload: &[u8]) -> anyhow::Result<std::path::PathBuf> {
    let dir = out
        .parent()
        .ok_or_else(|| anyhow::anyhow!("artifact path {} has no parent", out.display()))?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    for suffix in 0..=64u32 {
        let name = match suffix {
            0 => format!(".llm-artifacts-tmp-{pid}-{millis}"),
            n => format!(".llm-artifacts-tmp-{pid}-{millis}-{n}"),
        };
        let candidate = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(payload).and_then(|_| file.sync_all()) {
                    let _ = std::fs::remove_file(&candidate);
                    anyhow::bail!("temp write failed for {}: {e}", out.display());
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => anyhow::bail!("temp open failed for {}: {e}", out.display()),
        }
    }
    anyhow::bail!("temp collision budget exhausted for {}", out.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(system: &str, user: &str) -> CompletionRequest {
        CompletionRequest {
            system: system.to_string(),
            user: user.to_string(),
        }
    }

    fn raw(content: &str) -> RawCompletion {
        RawCompletion {
            usage: TokenUsage {
                prompt_tokens: 11,
                completion_tokens: 7,
                total_tokens: 18,
            },
            content: content.to_string(),
        }
    }

    fn valid_proposal_json() -> String {
        r#"{"new_nodes":[],"new_edges":[],"removed_edges":[{"from":0,"to":1,"kind":"Imports"}],"affected_nodes":[0],"modified_entities":[],"position_hints":[],"reasoning":"r"}"#.to_string()
    }

    fn parsed_proposal() -> DeltaProposal {
        serde_json::from_str(&valid_proposal_json()).unwrap()
    }

    // ── digest ────────────────────────────────────────────────────────────────

    #[test]
    fn prompt_digest_known_answer_vector() {
        // serde_json::to_string(PromptPair) === {"system":"s","user":"u"}
        // (alan sırası sabit) — sha256 KAT py ile bağımsız hesaplandı.
        let d = completion_prompt_digest(&req("s", "u"));
        assert_eq!(
            d,
            "sha256:f47adadc6fb3ce18459e9a4cf1453a8f924d48a7bd3eef53e8f7ec31a137ebbf"
        );
    }

    #[test]
    fn prompt_digest_deterministic_and_input_sensitive() {
        let a = completion_prompt_digest(&req("s", "u"));
        assert_eq!(a, completion_prompt_digest(&req("s", "u")));
        assert_ne!(a, completion_prompt_digest(&req("s", "v")));
        assert_ne!(a, completion_prompt_digest(&req("t", "u")));
        // Concatenation sınırı: JSON escaping sınırı netleştirir ("ab"+"c" ≠ "a"+"bc").
        assert_ne!(
            completion_prompt_digest(&req("ab", "c")),
            completion_prompt_digest(&req("a", "bc"))
        );
    }

    // ── record_proposal_completion ────────────────────────────────────────────

    #[test]
    fn record_first_completion_freezes_three_files() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        let response = raw(&valid_proposal_json());
        record_proposal_completion(
            dir.path(),
            "gpt-4o-mini",
            "https://api.openai.com/v1/chat/completions",
            &request,
            &response,
            Some(&parsed_proposal()),
            None,
        )
        .unwrap();

        let prompt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("llm-prompt.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(prompt["kind"], "llm-proposal-prompt");
        assert_eq!(prompt["model"], "gpt-4o-mini");
        assert_eq!(prompt["system"], "sys");
        assert_eq!(prompt["user"], "user");
        assert_eq!(
            prompt["prompt_digest"],
            completion_prompt_digest(&request).as_str()
        );

        let raw_text = std::fs::read_to_string(dir.path().join("llm-raw-response.txt")).unwrap();
        assert_eq!(raw_text, valid_proposal_json());

        let proposals: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("llm-proposals.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(proposals["usage"]["total_tokens"], 18);
        assert_eq!(proposals["parse_error"], serde_json::Value::Null);
        assert_eq!(proposals["proposal"]["reasoning"], "r");
    }

    #[test]
    fn record_parse_failure_is_honest_null() {
        let dir = tempfile::tempdir().unwrap();
        record_proposal_completion(
            dir.path(),
            "m",
            "e",
            &req("sys", "user"),
            &raw("not json at all"),
            None,
            Some("expected value at line 1 column 1"),
        )
        .unwrap();
        let proposals: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("llm-proposals.json")).unwrap(),
        )
        .unwrap();
        assert!(
            proposals["proposal"].is_null(),
            "D6: tamir yok, null dürüst"
        );
        assert_eq!(
            proposals["parse_error"],
            "expected value at line 1 column 1"
        );
        // Ham yanıt yine de durur (hitalet sebebi kayda geçer).
        assert_eq!(
            std::fs::read_to_string(dir.path().join("llm-raw-response.txt")).unwrap(),
            "not json at all"
        );
    }

    #[test]
    fn record_same_prompt_retry_overwrites_response() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        record_proposal_completion(
            dir.path(),
            "m",
            "e",
            &request,
            &raw("first"),
            None,
            Some("x"),
        )
        .unwrap();
        // D6: aynı prompt'la retry — prompt donmuş, yanıt güncellenir.
        record_proposal_completion(dir.path(), "m", "e", &request, &raw("second"), None, None)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("llm-raw-response.txt")).unwrap(),
            "second"
        );
    }

    #[test]
    fn record_different_prompt_refused_by_freeze() {
        let dir = tempfile::tempdir().unwrap();
        record_proposal_completion(
            dir.path(),
            "m",
            "e",
            &req("sys", "user"),
            &raw("first"),
            None,
            None,
        )
        .unwrap();
        let err = record_proposal_completion(
            dir.path(),
            "m",
            "e",
            &req("sys", "DIFFERENT"),
            &raw("second"),
            None,
            None,
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("already FROZEN") && msg.contains("INV-E1"),
            "educational freeze error, got: {msg}"
        );
    }

    #[test]
    fn record_corrupt_frozen_prompt_surfaces_read_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("llm-prompt.json"), "not json").unwrap();
        let err = record_proposal_completion(
            dir.path(),
            "m",
            "e",
            &req("sys", "user"),
            &raw("r"),
            None,
            None,
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("unreadable"),
            "corrupt freeze anchor is a loud error"
        );
    }

    // ── bar elicitation ───────────────────────────────────────────────────────

    #[test]
    fn parse_elicited_bar_plain_and_fenced() {
        let plain = r#"{"bar_value":0.55,"predicted":[{"label":"remove b import","predicted_coupling":0.5}]}"#;
        let bar = parse_elicited_bar(plain).unwrap();
        assert_eq!(bar.bar_value, 0.55);
        assert_eq!(bar.predicted.len(), 1);
        assert_eq!(bar.predicted[0].label, "remove b import");

        let fenced = format!("```json\n{plain}\n```");
        assert_eq!(parse_elicited_bar(&fenced).unwrap(), bar);
        assert!(parse_elicited_bar("nope").is_err());
    }

    #[test]
    fn record_bar_happy_path_and_transport_honest_null() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys-bar", "user-view");
        let response = raw(r#"{"bar_value":0.6,"predicted":[]}"#);
        let elicited = parse_elicited_bar(&response.content).unwrap();
        record_bar_elicitation(
            dir.path(),
            "m",
            "e",
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
        )
        .unwrap();
        let bar: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("llm-bar.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(bar["kind"], "llm-bar-elicitation");
        assert_eq!(bar["elicited"]["bar_value"], 0.6);
        assert_eq!(
            bar["prompt_digest"],
            completion_prompt_digest(&request).as_str()
        );

        // Transport hatası — D6 dürüst boşluk: dosya YİNE yazılır.
        record_bar_elicitation(
            dir.path(),
            "m",
            "e",
            &req("sys-bar2", "user-view"),
            None,
            None,
            None,
            Some("connection reset"),
        )
        .unwrap();
        let bar2: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("llm-bar.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(bar2["transport_error"], "connection reset");
        assert!(bar2["elicited"].is_null());
        assert!(bar2["usage"].is_null());
    }
}
