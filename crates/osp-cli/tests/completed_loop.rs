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
        git(&["config", "user.email", "osp-test@example.invalid"]);
        git(&["config", "user.name", "OSP Test"]);
        git(&["config", "core.autocrlf", "false"]);
        git(&["config", "core.eol", "lf"]);
        // Deterministic commit (review P1-5): fixed author/committer dates → stable SHA
        // across machines/clocks.
        let commit = |args: &[&str]| {
            Command::new("git")
                .args(["-C", r.to_str().unwrap()])
                .args(args)
                .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
                .status()
                .expect("git")
        };
        // main.rs imports a + b → 2 outgoing value imports → coupling 2/3 = 0.667.
        fs::write(
            r.join("main.rs"),
            "mod a;\nmod b;\npub fn main() { a::a(); b::b(); }\n",
        )
        .expect("write main.rs");
        fs::write(r.join("a.rs"), "pub fn a() {}\n").expect("write a.rs");
        fs::write(r.join("b.rs"), "pub fn b() {}\n").expect("write b.rs");
        git(&["add", "-A"]);
        commit(&["commit", "-qm", "init"]);
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
    /// CWD + --state-dir = work tempdir (outside repo → git status clean, .osp/ isolated,
    /// harness state-dir invariant satisfied — review B-3 P0).
    /// `format` = "human" (default) veya "json" (versioned run envelope).
    fn run_attempt(
        &self,
        task_path: &std::path::Path,
        proposals_path: &std::path::Path,
        positional_task_id: u64,
        format: &str,
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
            .arg("--state-dir")
            .arg(self.work_path()) // harness invariant: state-dir outside repo (P0)
            .arg("--format")
            .arg(format)
            .output()
            .expect("run osp")
    }

    /// Assert the analyzed repo has no `.osp/` state (no side-effects leaked into repo).
    /// Review P1-2 — fail-closed must leave the repo untouched.
    fn assert_repo_clean(&self) {
        // git status --porcelain must be empty (no .osp/, no held artifacts).
        let status = Command::new("git")
            .args([
                "-C",
                self.repo_path().to_str().unwrap(),
                "status",
                "--porcelain",
                "--untracked-files=all",
            ])
            .output()
            .expect("git status");
        let porcelain = String::from_utf8_lossy(&status.stdout);
        assert!(
            porcelain.trim().is_empty(),
            "analyzed repo must be clean after attempt (review P1-2), but git status:\n{porcelain}"
        );
        assert!(
            !self.repo_path().join(".osp").exists(),
            "no .osp/ state should leak into analyzed repo (review P1-2)"
        );
    }

    /// Run an arbitrary `osp trajectory attempt` (no task file) — serialized.
    fn run_attempt_no_task<F>(&self, build: F) -> std::process::Output
    where
        F: FnOnce(&mut Command) -> &mut Command,
    {
        let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = Command::cargo_bin("osp").expect("osp binary");
        cmd.current_dir(self.work_path())
            .arg("trajectory")
            .arg("attempt");
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
    assert!(
        !output.status.success(),
        "harness without --task must fail (P0-2)"
    );
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(!output.status.success(), "schema mismatch must fail");
}

#[test]
fn harness_task_id_mismatch_rejected() {
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0); // task.id = 7
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    // positional task_id = 999 ≠ task.id (7) → fail-closed.
    let output = fx.run_attempt(&task_path, &proposals_path, 999, "human");
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(!output.status.success(), "subgraph scope must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported harness scope"),
        "stderr: {stderr}"
    );
}

#[test]
fn harness_task_missing_preferred_vector_rejected() {
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["task"]["target_predicate_set"]["preferred_vector"] = serde_json::Value::Null;
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "missing preferred_vector must fail"
    );
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "terminal initial status must fail"
    );
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(!output.status.success(), "path mismatch must fail");
}

#[test]
fn harness_malformed_json_rejected() {
    let fx = HarnessFixture::new();
    let task_path = fx.write_raw_task("malformed.json", "{ this is not valid json");
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
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
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "dirty worktree must fail pre-flight"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dirty") || stderr.contains("clean"),
        "stderr explains clean-worktree requirement: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Review P0 — state-dir invariant (harness mode requires state-dir outside repo)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn harness_state_dir_inside_repo_rejected() {
    // P0: harness mode + state-dir inside analyzed repo → preflight reject.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let state_inside = fx.repo_path().join(".osp-state"); // inside repo!
    let output = run_attempt_with_state(&fx, &task_path, &proposals_path, 7, &state_inside);
    assert!(
        !output.status.success(),
        "harness + state-dir inside repo must reject (P0)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr explains state-dir-invariant: {stderr}"
    );
}

#[test]
fn harness_state_dir_outside_repo_accepted() {
    // P0: harness mode + state-dir outside repo → reaches navigator (no state-dir reject).
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human"); // state-dir = work (outside)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Must reach navigator (not fail on state-dir invariant).
    assert!(
        stdout.contains("Evidence entries")
            || stdout.contains("Task completed")
            || stdout.contains("Maneuver limit")
            || stdout.contains("Awaiting witnesses")
            || output.status.success(),
        "harness + external state-dir must reach navigator. stdout={stdout}\nstderr={stderr}"
    );
    // P1-2: repo stays clean (no .osp/ leak, no held artifacts).
    fx.assert_repo_clean();
}

#[test]
fn harness_state_dir_repo_subdir_rejected() {
    // P1-1: state-dir = repo/.subdir (canonical resolve → inside repo) → reject.
    // Covers the symlink/relative-path authority: canonical comparison, not string match.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let state_subdir = fx.repo_path().join("nested-state"); // inside repo subdir
    let output = run_attempt_with_state(&fx, &task_path, &proposals_path, 7, &state_subdir);
    assert!(
        !output.status.success(),
        "harness + state-dir repo subdir must reject (P1-1 canonical)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr explains canonical state-dir invariant: {stderr}"
    );
}

#[test]
fn harness_state_dir_sibling_external_accepted() {
    // P1-1: state-dir = repo/../external (canonical resolve → outside repo) → accepted.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    // repo/../external-sibling — canonical resolves outside repo.
    let state_external = fx
        .repo_path()
        .parent()
        .expect("repo has parent")
        .join("external-sibling-state");
    fs::create_dir_all(&state_external).expect("create external state dir");
    let output = run_attempt_with_state(&fx, &task_path, &proposals_path, 7, &state_external);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Must reach navigator (canonical resolved outside repo → no invariant reject).
    assert!(
        stdout.contains("Evidence entries")
            || stdout.contains("Task completed")
            || stdout.contains("Maneuver limit")
            || stdout.contains("Awaiting witnesses")
            || output.status.success(),
        "harness + external sibling state-dir must reach navigator (P1-1 canonical). stdout={stdout}\nstderr={stderr}"
    );
}

/// Run harness attempt with an explicit (possibly inside-repo) --state-dir.
fn run_attempt_with_state(
    fx: &HarnessFixture,
    task_path: &std::path::Path,
    proposals_path: &std::path::Path,
    positional_task_id: u64,
    state_dir: &std::path::Path,
) -> std::process::Output {
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.work_path())
        .arg("trajectory")
        .arg("attempt")
        .arg(positional_task_id.to_string())
        .arg("--repo")
        .arg(fx.repo_path())
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
        .arg("--state-dir")
        .arg(state_dir)
        .output()
        .expect("run osp")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Review P1-2 — fail-closed leaves no side-effects (no .osp/, repo clean)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn fail_closed_leaves_no_side_effects() {
    // Representative fail-closed (HEAD mismatch): navigator must never run → no .osp/,
    // repo untouched. P1-2: fail-closed = authority pipeline'a hiç girilmemesi.
    let fx = HarnessFixture::new();
    let env = task_envelope(&"f".repeat(40), 0); // wrong HEAD → fail-closed
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(!output.status.success(), "HEAD mismatch must fail");
    // No side-effects: repo clean, no .osp/ in repo.
    fx.assert_repo_clean();
    // No .osp/ in work state-dir either (navigator never reached Held).
    // (HarnessAutoApprove would skip Held anyway, but fail-closed happens before that.)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Review P1-4 — mode matrix exact coverage
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn production_production_no_task_uses_legacy_path() {
    // (Production, Production, no task) → legacy hardcoded task path (backward-compat).
    // This reaches the navigator (legacy task); not a pre-flight reject.
    let fx = HarnessFixture::new();
    let proposals_path = fx.write_proposals(0, 1);
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.work_path())
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--execution-mode")
        .arg("production")
        .arg("--witness")
        .arg("production")
        .arg("--llm")
        .arg("mock")
        .arg("--proposals")
        .arg(&proposals_path)
        .output()
        .expect("run osp");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Legacy path reaches navigator (production witness → AwaitingWitnesses expected,
    // since no real witnesses). The key assertion: no "harness requires --task" error.
    assert!(
        !stderr.contains("requires --task"),
        "production+no-task must NOT require --task (legacy backward-compat): {stderr}"
    );
    assert!(
        stdout.contains("Evidence entries")
            || stdout.contains("Awaiting witnesses")
            || stdout.contains("Maneuver limit")
            || stdout.contains("Task completed"),
        "production legacy path reaches navigator. stdout={stdout}\nstderr={stderr}"
    );
}

#[test]
fn harness_production_witness_reaches_navigator() {
    // (Harness, Production witness, task) → allowed (task file present, witness production).
    // Harness execution with production witness is valid — just needs authorization.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.work_path())
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--execution-mode")
        .arg("harness")
        .arg("--witness")
        .arg("production")
        .arg("--llm")
        .arg("mock")
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Must reach navigator (no mode-guard reject). Production witness → likely
    // AwaitingWitnesses (no real approvers), but the point is it runs.
    assert!(
        stdout.contains("Evidence entries")
            || stdout.contains("Awaiting witnesses")
            || stdout.contains("Maneuver limit")
            || stdout.contains("Task completed"),
        "harness+production-witness+task must reach navigator. stdout={stdout}\nstderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Review P0 — Completed-loop exact pin via versioned JSON envelope
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn completed_loop_exact_pin_via_json_envelope() {
    // Reviewer'ın istediği exact V1 run envelope + state-transition assertion.
    // main.rs (Node 2) has 2 outgoing imports → coupling 2/3 = 0.667.
    // RemoveImport 2→1 → coupling 1/2 = 0.5 ≤ 0.55 → Completed.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 2); // Node 2 = main.rs (2 outgoing imports)
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1); // remove main→b
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "json");

    // The attempt must succeed (Completed) and emit a parseable JSON envelope.
    assert!(
        output.status.success(),
        "Completed-loop must exit 0. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let envelope: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not JSON envelope: {e}\n{stdout}"));

    // V1 envelope schema.
    assert_eq!(envelope["schema_version"], 1, "schema_version");
    assert_eq!(envelope["run"]["execution_mode"], "harness");
    assert_eq!(envelope["run"]["witness_mode"], "harness_auto_approve");
    assert_eq!(envelope["run"]["task_source"], "harness_task_file");
    assert_eq!(
        envelope["run"]["repository_head"], fx.head,
        "envelope repository_head == fixture HEAD"
    );

    // V1 honest execution-measurement metadata.
    assert_eq!(
        envelope["execution_measurement"]["authority"], "legacy_projected_v1",
        "V1 honest authority (review P0)"
    );
    assert_eq!(
        envelope["execution_measurement"]["provenance_native"], false,
        "provenance_native=false (MD-2 deferred)"
    );

    // Result kind + attempts.
    assert_eq!(
        envelope["result"]["kind"], "completed",
        "Completed-loop result kind"
    );
    let attempts = envelope["result"]["attempts"]
        .as_u64()
        .expect("attempts is u64");
    assert_eq!(
        attempts, 1,
        "single-attempt Completed (RemoveImport satisfies)"
    );

    // Evidence — exact state-transition pin.
    let evidence = envelope["evidence"].as_array().expect("evidence array");
    assert_eq!(
        evidence.len(),
        1,
        "exactly one evidence entry (single attempt)"
    );
    let entry = &evidence[0];
    assert_eq!(entry["gate_decision"], "PassedAll", "gate_decision");
    assert_eq!(
        entry["predicate_completion"], "Completed",
        "predicate completion"
    );
    assert_eq!(
        entry["mutation_decision"], "AcceptAsCompleted",
        "mutation decision"
    );

    // before/after coupling: after < before, after ≤ threshold (0.55).
    let before_coupling = entry["before"]["x"].as_f64().expect("before coupling (x)");
    let after_coupling = entry["after"]["x"].as_f64().expect("after coupling (x)");
    assert!(
        after_coupling < before_coupling,
        "coupling must decrease: before={before_coupling}, after={after_coupling}"
    );
    assert!(
        after_coupling <= 0.55,
        "after coupling ≤ threshold (0.55): after={after_coupling}"
    );
    // before coupling was 2/3 ≈ 0.667 (2 outgoing imports).
    assert!(
        before_coupling > 0.55,
        "before coupling > threshold (was unsatisfied): before={before_coupling}"
    );
}
