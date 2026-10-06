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
