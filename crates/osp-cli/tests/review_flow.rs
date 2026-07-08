//! CLI integration test'leri — `osp graph` + `osp review` uçtan uca akışlar.
//!
//! `assert_cmd` ile `osp` binary'sini çalıştırır; real subprocess (unit test değil).
//! Kabul kriterleri (plan):
//! - graph init (Candidate-only, deny_unknown_fields)
//! - review list (deterministic)
//! - accept/reject (--basis-digest, confirmation)
//! - stale basis (digest mismatch → fail + store unchanged)
//! - restart-safe (process restart → state+ledger+audit_seq korunuyor)
//! - corrupt snapshot fail-closed
//! - canonical bit-identik (one-shot vs interactive same snapshot)
//! - unrelated revision (digest aynı → başarı)

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

/// `osp graph init` helper — seed yaz, store path dön.
fn init_store(dir: &std::path::Path, seed_json: &str) -> std::path::PathBuf {
    let seed_path = dir.join("seed.json");
    std::fs::write(&seed_path, seed_json).unwrap();
    let store_path = dir.join("store.json");
    Command::cargo_bin("osp")
        .unwrap()
        .args([
            "graph",
            "init",
            "--seed",
            seed_path.to_str().unwrap(),
            "--store",
            store_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("Graph initialized"));
    store_path
}

const TWO_CANDIDATES_SEED: &str = r#"{
    "schema_version": 1,
    "nodes": [
        {"canonical": "CouplingMustNot", "kind": "RuleCandidate"},
        {"canonical": "Payment", "kind": "Concept"}
    ]
}"#;

/// `osp graph init` → 2 candidate node yüklenir.
#[test]
fn graph_init_loads_candidate_nodes() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "status", "--store", store.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("candidates: 2"));
}

/// `osp graph init` existing store → fail (overwrite gerekmez).
#[test]
fn graph_init_existing_store_fails_without_force() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    let seed_path = dir.path().join("seed.json");
    Command::cargo_bin("osp")
        .unwrap()
        .args([
            "graph",
            "init",
            "--seed",
            seed_path.to_str().unwrap(),
            "--store",
            store.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("already exists"));
}

/// Seed'de `decision_status` alanı → deny_unknown_fields reject.
#[test]
fn graph_init_rejects_seed_with_status_field() {
    let dir = tempdir().unwrap();
    let seed = r#"{ "schema_version": 1, "nodes": [{"canonical": "X", "kind": "RuleCandidate", "decision_status": "Accepted"}] }"#;
    let seed_path = dir.path().join("seed.json");
    std::fs::write(&seed_path, seed).unwrap();
    Command::cargo_bin("osp")
        .unwrap()
        .args([
            "graph",
            "init",
            "--seed",
            seed_path.to_str().unwrap(),
            "--store",
            dir.path().join("s.json").to_str().unwrap(),
        ])
        .assert()
        .failure();
}

/// `osp review list` → deterministic sıralı (NodeId ascending).
#[test]
fn review_list_shows_candidates() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    Command::cargo_bin("osp")
        .unwrap()
        .args(["review", "list", "--store", store.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Concept:Payment"))
        .stdout(contains("RuleCandidate:CouplingMustNot"));
}

/// `osp review accept` non-interactive (--yes --basis-digest) → Accepted + revision increment.
#[test]
fn review_accept_noninteractive_succeeds() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);

    // Digest al.
    let show_output = Command::cargo_bin("osp")
        .unwrap()
        .args([
            "review",
            "show",
            "RuleCandidate:CouplingMustNot",
            "--store",
            store.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: serde_json::Value = serde_json::from_slice(&show_output).unwrap();
    let digest_hex = show_json["basis_digest_hex"].as_str().unwrap();

    // Accept.
    Command::cargo_bin("osp")
        .unwrap()
        .args([
            "review",
            "accept",
            "RuleCandidate:CouplingMustNot",
            "--store",
            store.to_str().unwrap(),
            "--operator",
            "tester",
            "--reason",
            "approved threshold rule",
            "--yes",
            "--basis-digest",
            digest_hex,
        ])
        .assert()
        .success()
        .stdout(contains("Accepted"));

    // Revision 0 → 1.
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "status", "--store", store.to_str().unwrap()])
        .assert()
        .stdout(contains("Revision: 1"))
        .stdout(contains("candidates: 1"))
        .stdout(contains("accepted: 1"));
}

/// Stale basis: yanlış digest → fail + store unchanged (revision same).
#[test]
fn review_accept_stale_digest_rejected_store_unchanged() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);

    // Yanlış digest ile accept → fail.
    Command::cargo_bin("osp")
        .unwrap()
        .args([
            "review",
            "accept",
            "RuleCandidate:CouplingMustNot",
            "--store",
            store.to_str().unwrap(),
            "--operator",
            "tester",
            "--reason",
            "stale attempt",
            "--yes",
            "--basis-digest",
            "0000000000000000",
        ])
        .assert()
        .failure()
        .stderr(contains("stale basis"));

    // Store unchanged — revision hala 0.
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "status", "--store", store.to_str().unwrap()])
        .assert()
        .stdout(contains("Revision: 0"))
        .stdout(contains("candidates: 2"));
}

/// Restart-safe: accept sonrası yeni process status → decision record + audit_seq korunuyor.
#[test]
fn review_state_survives_process_restart() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);

    // Accept (ilk process).
    let digest = {
        let out = Command::cargo_bin("osp")
            .unwrap()
            .args([
                "review",
                "show",
                "RuleCandidate:CouplingMustNot",
                "--store",
                store.to_str().unwrap(),
                "--format",
                "json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        v["basis_digest_hex"].as_str().unwrap().to_string()
    };
    Command::cargo_bin("osp")
        .unwrap()
        .args([
            "review",
            "accept",
            "RuleCandidate:CouplingMustNot",
            "--store",
            store.to_str().unwrap(),
            "--operator",
            "tester",
            "--reason",
            "ok",
            "--yes",
            "--basis-digest",
            &digest,
        ])
        .assert()
        .success();

    // Yeni process status → state korunuyor.
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "status", "--store", store.to_str().unwrap()])
        .assert()
        .stdout(contains("Revision: 1"))
        .stdout(contains("Decision records: 1"))
        .stdout(contains("Audit sequence: 1"));
}

/// `osp review accept` `--operator` olmadan → fail (generic "operator" default yok).
#[test]
fn review_accept_requires_operator_identity() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    Command::cargo_bin("osp")
        .unwrap()
        .env_remove("OSP_OPERATOR")
        .args([
            "review",
            "accept",
            "RuleCandidate:CouplingMustNot",
            "--store",
            store.to_str().unwrap(),
            "--reason",
            "no operator",
            "--yes",
            "--basis-digest",
            "0000000000000000",
        ])
        .assert()
        .failure()
        .stderr(contains("Operator identity is required"));
}

/// `osp graph validate` → valid store tüm invariant'lar geçer.
#[test]
fn graph_validate_passes_for_valid_store() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "validate", "--store", store.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Store valid"));
}

/// Corrupt snapshot → `osp graph validate` fail-closed.
#[test]
fn graph_validate_fails_for_corrupt_store() {
    let dir = tempdir().unwrap();
    let store = dir.path().join("store.json");
    // Geçersiz JSON yaz.
    std::fs::write(&store, "{ not valid json }").unwrap();
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "validate", "--store", store.to_str().unwrap()])
        .assert()
        .failure();
}

/// `osp graph status --format json` → structured output (CI/script).
#[test]
fn graph_status_json_output() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    let output = Command::cargo_bin("osp")
        .unwrap()
        .args([
            "graph",
            "status",
            "--store",
            store.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(v["candidates"], 2);
    assert_eq!(v["revision"], 0);
}

/// Argümansız `osp review` (sadece --store) → interactive session açar (Review P1.1).
/// Operator prompt'a cevap verip quit ile çıkar.
#[test]
fn argumanliz_osp_review_opens_interactive_session() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    // stdin: operator identity prompt → "tester", sonra quit.
    Command::cargo_bin("osp")
        .unwrap()
        .env_remove("OSP_OPERATOR")
        .arg("review")
        .arg("--store")
        .arg(store.to_str().unwrap())
        .write_stdin("tester\nquit\n")
        .assert()
        .success()
        .stdout(contains("Operator identity:"))
        .stdout(contains("OSP review session"));
}

/// Argümansız `osp review` OSP_OPERATOR set ise prompt sormaz.
#[test]
fn argumanliz_osp_review_uses_env_operator() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    Command::cargo_bin("osp")
        .unwrap()
        .env("OSP_OPERATOR", "env-op")
        .arg("review")
        .arg("--store")
        .arg(store.to_str().unwrap())
        .write_stdin("quit\n")
        .assert()
        .success()
        .stdout(contains("operator: env-op"))
        .stdout(contains("candidates awaiting review"));
}

/// Interactive informed-acceptance: accept → basis göster → confirm(y) → reason → Accepted.
/// Review P1.2: operator basis'i GÖRDÜKTEN sonra karar verir.
#[test]
fn interactive_accept_shows_basis_before_confirmation() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    // accept <id> → confirmation y → reason → quit.
    Command::cargo_bin("osp")
        .unwrap()
        .env("OSP_OPERATOR", "tester")
        .arg("review")
        .arg("--store")
        .arg(store.to_str().unwrap())
        .write_stdin("accept RuleCandidate:CouplingMustNot\ny\napproved rule\nquit\n")
        .assert()
        .success()
        .stdout(contains("this exact basis?"))
        .stdout(contains("Accepted"));
}

/// Interactive: confirmation 'n' → abort, mutation uygulanmaz.
#[test]
fn interactive_confirmation_n_aborts_mutation() {
    let dir = tempdir().unwrap();
    let store = init_store(dir.path(), TWO_CANDIDATES_SEED);
    Command::cargo_bin("osp")
        .unwrap()
        .env("OSP_OPERATOR", "tester")
        .arg("review")
        .arg("--store")
        .arg(store.to_str().unwrap())
        .write_stdin("accept RuleCandidate:CouplingMustNot\nn\nquit\n")
        .assert()
        .success()
        .stdout(contains("aborted by operator"));
    // Store unchanged — revision hala 0.
    Command::cargo_bin("osp")
        .unwrap()
        .args(["graph", "status", "--store", store.to_str().unwrap()])
        .assert()
        .stdout(contains("Revision: 0"));
}
