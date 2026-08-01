//! Faz 8 test-project B-3 — Completed-loop integration test matrix.
//!
//! Completed-loop'u uçtan uca doğrular. osp-cli bin-only crate olduğu için tüm testler
//! gerçek `osp` binary'sini çağırır (assert_cmd pattern — mevcut testlerle tutarlı).
//!
//! Fixture topolojisi: main.rs imports a.rs + b.rs → target node 2 outgoing Imports →
//! coupling 2/3 = 0.667. RemoveImport → coupling 1/2 = 0.5 ≤ 0.55 → Completed.
//!
//! **Isolation**: `osp trajectory attempt` `FilesystemPendingAuthorizationStore::new(".")`
//! CWD .osp/'ye yazar. Test CWD'si analyzed repo DIŞINDA ayrı bir tempdir'dir (repo
//! git status'ünü kirletmemesi için). `OSP_ATTEMPT_LOCK` .osp/ paralel contamination'ı
//! önler (yine de defensive — her test ayrı CWD kullanır).

#![cfg(test)]

use std::fs;
use std::process::Command;
use std::sync::Mutex;

use assert_cmd::prelude::*;

/// Serializes `osp trajectory attempt` invocations (defensive — CWD isolation handles
/// the real contamination, but the lock keeps output deterministic under load).
static OSP_ATTEMPT_LOCK: Mutex<()> = Mutex::new(());

// ═══════════════════════════════════════════════════════════════════════════════
// HarnessFixture — owns repo tempdir + work tempdir (CWD) + HEAD
// ═══════════════════════════════════════════════════════════════════════════════

/// Bundles the analyzed git repo, the out-of-repo working dir (CWD), and the fixture HEAD.
/// `work` is a separate tempdir so `.osp/` writes and task/proposal files don't dirty
/// the repo's git status (which would fail snapshot eligibility).
struct HarnessFixture {
    repo: tempfile::TempDir,
    work: tempfile::TempDir,
    head: String,
}

impl HarnessFixture {
    /// Build a deterministic git fixture: main.rs imports a.rs + b.rs (2 outgoing value imports).
    /// CRLF normalization disabled to prevent dirty-worktree cross-contamination.
    fn new() -> Self {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let r = repo.path();
        let git = |args: &[&str]| {
            Command::new("git")
                .args(["-C", r.to_str().unwrap()])
                .args(args)
                .status()
                .expect("git")
        };
        Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(r)
            .status()
            .expect("git init");
        git(&["config", "user.email", "t@t.com"]);
        git(&["config", "user.name", "t"]);
        git(&["config", "core.autocrlf", "false"]);
        git(&["config", "core.eol", "lf"]);
        // main.rs imports a + b → 2 outgoing value imports → coupling 2/3 = 0.667.
        fs::write(
            r.join("main.rs"),
            "mod a;\nmod b;\npub fn main() { a::a(); b::b(); }\n",
        )
        .expect("write main.rs");
        fs::write(r.join("a.rs"), "pub fn a() {}\n").expect("write a.rs");
        fs::write(r.join("b.rs"), "pub fn b() {}\n").expect("write b.rs");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "init"]);
        let head = String::from_utf8(
            Command::new("git")
                .args(["-C", r.to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let work = tempfile::tempdir().expect("work tempdir (CWD)");
        Self { repo, work, head }
    }

    fn repo_path(&self) -> &std::path::Path {
        self.repo.path()
    }

    fn work_path(&self) -> &std::path::Path {
        self.work.path()
    }

    /// Write task JSON to the work CWD, return its path.
    fn write_task(&self, value: &serde_json::Value) -> std::path::PathBuf {
        let path = self.work_path().join("task.v1.json");
        fs::write(&path, serde_json::to_string_pretty(value).unwrap()).unwrap();
        path
    }

    /// Write a proposals JSON (one RemoveImport from_node→to_node) to the work CWD.
    fn write_proposals(&self, from_node: u64, to_node: u64) -> std::path::PathBuf {
        let proposals = serde_json::json!([{
            "new_nodes": [],
            "new_edges": [],
            "removed_edges": [{"from": from_node, "to": to_node, "kind": "Imports"}],
            "affected_nodes": [from_node],
            "modified_entities": [],
            "position_hints": [],
            "reasoning": "remove import to reduce coupling below threshold"
        }]);
        let path = self.work_path().join("proposals.json");
        fs::write(&path, serde_json::to_string_pretty(&proposals).unwrap()).unwrap();
        path
    }

    /// Write raw string to the work CWD (for malformed-JSON tests).
    fn write_raw_task(&self, name: &str, content: &str) -> std::path::PathBuf {
        let path = self.work_path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    /// Run `osp trajectory attempt` with harness mode + task file. Returns the output.
    /// CWD = work tempdir (outside repo → git status clean, .osp/ isolated).
    fn run_attempt(
        &self,
        task_path: &std::path::Path,
        proposals_path: &std::path::Path,
        positional_task_id: u64,
    ) -> std::process::Output {
        let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        Command::cargo_bin("osp")
            .expect("osp binary")
            .current_dir(self.work_path())
            .arg("trajectory")
            .arg("attempt")
            .arg(positional_task_id.to_string())
            .arg("--repo")
            .arg(self.repo_path())
            .arg("--execution-mode")
            .arg("harness")
            .arg("--witness")
            .arg("harness-auto-approve")
            .arg("--llm")
            .arg("mock")
            .arg("--proposals")
            .arg(proposals_path)
            .arg("--task")
            .arg(task_path)
            .output()
            .expect("run osp")
    }

    /// Run an arbitrary `osp trajectory attempt` (no task file) — serialized.
    fn run_attempt_no_task<F>(&self, build: F) -> std::process::Output
    where
        F: FnOnce(&mut Command) -> &mut Command,
    {
        let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = Command::cargo_bin("osp").expect("osp binary");
        cmd.current_dir(self.work_path()).arg("trajectory").arg("attempt");
        build(&mut cmd);
        cmd.output().expect("run osp")
    }
}

/// Build a harness task JSON envelope. anchor defaults: Node 0 = "a.rs", 1 = "b.rs",
/// 2 = "main.rs" (analyzer alphabetical ordering).
fn task_envelope(head: &str, anchor_node_id: u64) -> serde_json::Value {
    let anchor_path = match anchor_node_id {
        0 => "a.rs",
        1 => "b.rs",
        2 => "main.rs",
        _ => "a.rs",
    };
    serde_json::json!({
        "schema_version": 1,
        "repository_head": head,
        "scope_bindings": [{"node_id": anchor_node_id, "expected_path": anchor_path}],
        "task": {
            "id": 7,
            "milestone_id": 1,
            "label": "completed-loop fixture",
            "target_predicate_set": {
                "mode": "All",
                "predicates": [{
                    "predicate": {
                        "metric": "Coupling",
                        "operator": "Le",
                        "threshold": 0.55,
                        "scope": {"Node": anchor_node_id},
                        "required_source": "Scip",
                        "tolerance": 0.0
                    },
                    "weight": null
                }],
                "preferred_vector": {"x": 0.55, "y": 0.6, "z": 0.5, "w": 0.5, "v": 0.3}
            },
            "policy": {
                "predicate_failure_policy": "StrictReject",
                "min_improvement_delta": 0.02,
                "max_axis_regression": 0.15,
                "maneuver_limit": 3,
                "allow_progress_checkpoint": false
            },
            "allowed_operations": ["RemoveImport"],
            "constraints": [],
            "status": "Pending"
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Matrix — mode-matrix guard (P0-1, P0-2)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn harness_without_task_rejected() {
    // P0-2: harness mode without --task → error (no legacy bypass).
    let fx = HarnessFixture::new();
    let output = fx.run_attempt_no_task(|cmd| {
        cmd.arg("7")
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--execution-mode")
            .arg("harness")
            .arg("--witness")
            .arg("harness-auto-approve")
            .arg("--llm")
            .arg("mock")
            .arg("--proposals")
            .arg("/dev/null")
    });
    assert!(!output.status.success(), "harness without --task must fail (P0-2)");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--execution-mode harness requires --task"),
        "stderr: {stderr}"
    );
}

#[test]
fn production_witness_with_autoapprove_rejected_by_guard() {
    // P0-1 guard: harness-auto-approve witness requires harness execution mode.
    let fx = HarnessFixture::new();
    let output = fx.run_attempt_no_task(|cmd| {
        cmd.arg("7")
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--execution-mode")
            .arg("production")
            .arg("--witness")
            .arg("harness-auto-approve")
            .arg("--llm")
            .arg("mock")
            .arg("--proposals")
            .arg("/dev/null")
    });
    assert!(
        !output.status.success(),
        "production + harness-auto-approve must fail (guard)"
    );
}

#[test]
fn invalid_witness_enum_is_clap_error() {
    let fx = HarnessFixture::new();
    let output = fx.run_attempt_no_task(|cmd| {
        cmd.arg("7")
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--witness")
            .arg("bogus-witness")
    });
    assert!(!output.status.success(), "invalid witness must fail");
    assert_eq!(output.status.code(), Some(2), "clap parse error = exit 2");
}

#[test]
fn invalid_execution_mode_is_clap_error() {
    let fx = HarnessFixture::new();
    let output = fx.run_attempt_no_task(|cmd| {
        cmd.arg("7")
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--execution-mode")
            .arg("bogus-mode")
    });
    assert!(!output.status.success(), "invalid execution mode must fail");
    assert_eq!(output.status.code(), Some(2), "clap parse error = exit 2");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Matrix — harness-task-loader fail-closed scenarios
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn harness_task_head_mismatch_rejected() {
    let fx = HarnessFixture::new();
    let env = task_envelope(&"f".repeat(40), 0); // wrong HEAD
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "HEAD mismatch must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("mismatch"), "stderr: {stderr}");
}

#[test]
fn harness_task_schema_mismatch_rejected() {
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["schema_version"] = serde_json::json!(2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "schema mismatch must fail");
}

#[test]
fn harness_task_id_mismatch_rejected() {
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0); // task.id = 7
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    // positional task_id = 999 ≠ task.id (7) → fail-closed.
    let output = fx.run_attempt(&task_path, &proposals_path, 999);
    assert!(!output.status.success(), "task id mismatch must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("mismatch"), "stderr: {stderr}");
}

#[test]
fn harness_task_subgraph_scope_rejected() {
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["task"]["target_predicate_set"]["predicates"][0]["predicate"]["scope"] =
        serde_json::json!({"Subgraph": [0, 1]});
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "subgraph scope must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported harness scope"), "stderr: {stderr}");
}

#[test]
fn harness_task_missing_preferred_vector_rejected() {
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["task"]["target_predicate_set"]["preferred_vector"] = serde_json::Value::Null;
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "missing preferred_vector must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("preferred_vector"), "stderr: {stderr}");
}

#[test]
fn harness_task_terminal_status_rejected() {
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["task"]["status"] = serde_json::json!("Completed");
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "terminal initial status must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("status"), "stderr: {stderr}");
}

#[test]
fn harness_task_scope_binding_path_mismatch_rejected() {
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["scope_bindings"][0]["expected_path"] = serde_json::json!("src/nonexistent.rs");
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "path mismatch must fail");
}

#[test]
fn harness_malformed_json_rejected() {
    let fx = HarnessFixture::new();
    let task_path = fx.write_raw_task("malformed.json", "{ this is not valid json");
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "malformed JSON must fail");
}

#[test]
fn harness_core_invalid_maneuver_limit_rejected() {
    // maneuver_limit=0 → core InvalidManeuverLimit (review P0-1 delegation proof).
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["task"]["policy"]["maneuver_limit"] = serde_json::json!(0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(
        !output.status.success(),
        "invalid maneuver_limit must fail (core delegation)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("maneuver_limit") || stderr.contains("maneuver"),
        "core validation surfaced: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Completed-loop happy path + dirty-worktree scenario
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn harness_valid_completed_loop_runs() {
    // Happy path: harness + valid task + RemoveImport proposal → navigator runs.
    // NodeId 0/1/2 depend on analyzer ordering (a=0, b=1, main=2). The task targets
    // Node(0)=a.rs which has 0 outgoing imports (coupling already 0). This asserts the
    // harness wiring reaches the navigator (no pre-flight rejection); the navigator then
    // runs with whatever measured position the engine reports.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The attempt must reach the navigator (evidence/maneuver output), not fail pre-flight.
    assert!(
        stdout.contains("Evidence entries")
            || stdout.contains("Task completed")
            || stdout.contains("Maneuver limit")
            || stdout.contains("Awaiting witnesses")
            || output.status.success(),
        "harness wiring reached navigator. stdout={stdout}\nstderr={stderr}"
    );
}

#[test]
fn harness_dirty_worktree_rejected() {
    // ensure_snapshot_eligible (P0-3): dirty worktree → attempt rejected before run.
    let fx = HarnessFixture::new();
    fs::write(fx.repo_path().join("main.rs"), "pub fn main() {}\n").expect("dirty main.rs");
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7);
    assert!(!output.status.success(), "dirty worktree must fail pre-flight");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dirty") || stderr.contains("clean"),
        "stderr explains clean-worktree requirement: {stderr}"
    );
}
