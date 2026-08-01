//! Faz 8 test-project B-2 — `osp analyze` provenance envelope integration.
//!
//! V1 honest metadata + per-axis provenance + repository HEAD binding + key-set integrity.
//! Review fixes: pre/post fence (P0-1), metadata separation (P1-1), enum snake_case DTOs
//! (P1-2), key-set exact equality (P1-3), --format/--out matrix + ValueEnum (P1-4).
//! Deterministic temp git fixture → `osp analyze` → assert envelope structure + honesty.

#![cfg(test)]

use std::fs;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

/// Build a tiny deterministic git fixture: two Rust files, one clean commit.
fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path();
    Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(repo)
        .status()
        .expect("git init");
    Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "config", "user.email", "t@t.com"])
        .status()
        .expect("git config email");
    Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "config", "user.name", "t"])
        .status()
        .expect("git config name");
    fs::write(repo.join("a.rs"), "pub fn a() {}\n").expect("write a.rs");
    fs::write(repo.join("main.rs"), "mod a;\npub fn main() { a::a(); }\n").expect("write main.rs");
    Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "add", "-A"])
        .status()
        .expect("git add");
    Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "commit", "-qm", "init"])
        .status()
        .expect("git commit");
    dir
}

/// `osp analyze --format json` produces a parseable provenance envelope on stdout only.
#[test]
fn analyze_json_emits_provenance_envelope() {
    let dir = fixture_repo();
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .output()
        .expect("run osp analyze");

    assert!(
        output.status.success(),
        "osp analyze failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout.clone()).expect("utf8 stdout");
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("stdout not JSON: {e}\n{stdout}"));

    // Review P1-1 — analysis provenance is axis-specific (not "native" — MD-2 term).
    assert_eq!(envelope["schema_version"], 1, "schema_version");
    assert_eq!(
        envelope["analysis"]["metric_representation"], "axis_provenanced_v1",
        "analysis metric representation"
    );
    assert_eq!(
        envelope["analysis"]["provenance_model"], "analyzer_axis_specific",
        "provenance model distinct from MD-2 native (review P1-1)"
    );
    assert_eq!(
        envelope["analysis"]["axis_specific_provenance"], true,
        "analyze produces axis-specific provenance"
    );
    // No top-level navigator authority field (that's B-3 run envelope's concern).
    assert!(
        envelope.get("measurement_authority").is_none(),
        "navigator authority (legacy_projected) belongs to run envelope, not analyze"
    );

    // repository HEAD binding — full 40-char SHA from snapshot_after (review P0/P1-2).
    let head = envelope["repository"]["head"]
        .as_str()
        .expect("head is string");
    assert_eq!(head.len(), 40, "head is full 40-char SHA");
    assert!(
        head.bytes().all(|b| b.is_ascii_hexdigit()),
        "head is hex SHA"
    );
    // Default generic mode → observed_worktree_unbound (dirty OK, review P0).
    assert_eq!(
        envelope["repository"]["binding"], "observed_worktree_unbound",
        "generic analyze = observed worktree (review P0)"
    );

    // Nodes: NodeId ascending, each carries path + snake_case kind + per-axis provenance.
    let nodes = envelope["nodes"]
        .as_array()
        .expect("nodes is array");
    assert!(!nodes.is_empty(), "at least one node");
    let mut prev_id: i64 = -1;
    for n in nodes {
        let id = n["node_id"].as_i64().expect("node_id is int");
        assert!(id > prev_id, "nodes ascending by node_id");
        prev_id = id;
        assert!(n["path"].as_str().is_some(), "node has path");
        // snake_case enum DTOs (review P1-2).
        let kind = n["kind"].as_str().expect("node kind is snake_case string");
        assert!(
            !kind.chars().any(|c| c.is_ascii_uppercase()),
            "kind {kind} is snake_case (no uppercase)"
        );
        // Per-axis CliAxisMeasurement: full provenance (value+source+confidence+coverage).
        for axis in ["coupling", "cohesion", "instability"] {
            assert!(n[axis]["value"].as_f64().is_some(), "axis {axis} value");
            assert!(n[axis]["source"].as_str().is_some(), "axis {axis} source");
            assert!(
                n[axis]["confidence"].as_f64().is_some(),
                "axis {axis} confidence (review: full provenance)"
            );
            assert!(
                n[axis]["coverage"].as_f64().is_some(),
                "axis {axis} coverage (review: full provenance)"
            );
        }
    }

    // Honesty without SCIP: cohesion source is "placeholder" (not fabricated "scip").
    let has_placeholder = nodes
        .iter()
        .any(|n| n["cohesion"]["source"].as_str() == Some("placeholder"));
    assert!(
        has_placeholder,
        "cohesion provenance honestly reports placeholder when no SCIP"
    );

    // stderr carries diagnostics (partial SCIP coverage), stdout is JSON-only.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("partial SCIP coverage"),
        "stderr carries partial coverage diagnostics"
    );
}

/// `osp analyze` without `--format` (human default) still emits the JSON envelope to stdout
/// (backward-compat: existing consumers parse stdout JSON).
#[test]
fn analyze_human_default_still_emits_envelope() {
    let dir = fixture_repo();
    Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 1"))
        .stdout(predicate::str::contains("\"axis_provenanced_v1\""));
}

/// `osp analyze --out <path>` writes the provenance envelope to file (backward-compat).
#[test]
fn analyze_out_flag_writes_envelope_file() {
    let dir = fixture_repo();
    let out_path = dir.path().join("snapshot.json");
    Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--out")
        .arg(&out_path)
        .assert()
        .success();

    let written = fs::read_to_string(&out_path).expect("read snapshot.json");
    let envelope: serde_json::Value = serde_json::from_str(&written).expect("written file is JSON");
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(
        envelope["analysis"]["provenance_model"], "analyzer_axis_specific"
    );
}

/// `--format json --out file` → stdout empty, JSON to file, confirmation on stderr
/// (review P1-4 — matrix: no double-write to stdout).
#[test]
fn analyze_json_with_out_writes_file_stdout_empty() {
    let dir = fixture_repo();
    let out_path = dir.path().join("snapshot.json");
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&out_path)
        .output()
        .expect("run osp analyze");
    assert!(
        output.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // stdout empty (no JSON document, no confirmation text).
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "json+out: stdout must be empty, got: {stdout:?}"
    );
    // confirmation + diagnostics on stderr.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("written to"), "confirmation on stderr");
    // file has the JSON envelope.
    let written = fs::read_to_string(&out_path).expect("read snapshot.json");
    let _: serde_json::Value = serde_json::from_str(&written).expect("file is JSON");
}

/// `--format invalid` → clap parse error (ValueEnum, review P1-4) — no silent human fallback.
#[test]
fn analyze_invalid_format_is_clap_error() {
    let dir = fixture_repo();
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("yaml")
        .output()
        .expect("run osp analyze");
    // clap parse errors exit with code 2.
    assert!(
        !output.status.success(),
        "invalid format must fail, not silently fall back"
    );
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "clap parse error exits with code 2");
}

/// Generic analyze (default) accepts dirty worktree — observes current content
/// (review P0 — two contracts: ObserveWorktree vs RequireCleanHeadBinding).
#[test]
fn analyze_generic_accepts_dirty_worktree() {
    let dir = fixture_repo();
    // Introduce an untracked change → dirty worktree.
    fs::write(dir.path().join("a.rs"), "pub fn a() -> u32 { 1 }\n").expect("modify a.rs");
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .output()
        .expect("run osp analyze");
    assert!(
        output.status.success(),
        "generic analyze must accept dirty worktree (review P0): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout JSON envelope");
    assert_eq!(
        envelope["repository"]["binding"], "observed_worktree_unbound",
        "generic dirty = observed (unbound)"
    );
}

/// `--require-clean-snapshot` + dirty worktree → rejected before output (review P0).
#[test]
fn analyze_require_clean_rejects_dirty_worktree() {
    let dir = fixture_repo();
    fs::write(dir.path().join("a.rs"), "pub fn a() -> u32 { 1 }\n").expect("modify a.rs");
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .arg("--require-clean-snapshot")
        .output()
        .expect("run osp analyze");
    assert!(
        !output.status.success(),
        "require-clean-snapshot + dirty must reject"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty(), "rejected → no stdout JSON");
}

/// `--require-clean-snapshot` + clean worktree → clean_pre_post_equal binding (review P0).
#[test]
fn analyze_require_clean_clean_worktree_binds() {
    let dir = fixture_repo();
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .arg("--require-clean-snapshot")
        .output()
        .expect("run osp analyze");
    assert!(
        output.status.success(),
        "require-clean + clean worktree must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout))
            .expect("stdout JSON");
    assert_eq!(
        envelope["repository"]["binding"], "clean_pre_post_equal",
        "require-clean + clean = bound"
    );
}

/// `--require-clean-snapshot --out <inside-repo>` → rejected (review P1-4 — would dirty).
#[test]
fn analyze_require_clean_rejects_out_inside_repo() {
    let dir = fixture_repo();
    let out_path = dir.path().join("snapshot.json"); // inside analyzed repo
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .arg("--require-clean-snapshot")
        .arg("--out")
        .arg(&out_path)
        .output()
        .expect("run osp analyze");
    assert!(
        !output.status.success(),
        "out inside repo + require-clean must reject"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr explains out-inside-repo rejection"
    );
}

/// repository.head is full 40-char SHA from snapshot authority (review P1-2),
/// NOT the analyzer's short repo_head.
#[test]
fn analyze_head_is_full_sha_from_snapshot_authority() {
    let dir = fixture_repo();
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .arg("analyze")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .output()
        .expect("run osp analyze");
    let envelope: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("JSON");
    let head = envelope["repository"]["head"].as_str().expect("head");
    assert_eq!(head.len(), 40, "full SHA, not analyzer short repo_head");
    assert!(head.bytes().all(|b| b.is_ascii_hexdigit()));
    // Cross-check: matches `git rev-parse HEAD` (full).
    let git_head = Command::new("git")
        .args(["-C", dir.path().to_str().unwrap(), "rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse");
    let git_head_str = String::from_utf8_lossy(&git_head.stdout).trim().to_string();
    assert_eq!(head, git_head_str, "head == git rev-parse HEAD");
}
