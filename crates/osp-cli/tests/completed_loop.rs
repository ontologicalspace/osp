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

    /// Write a path-keyed proposals v2 envelope (one RemoveImport from_path→to_path).
    /// #161/B5: NodeId alanları repo-relative path; R1 P1-1 sonrası envelope
    /// `repository_head` taşır — proposal'ın üretildiği state, re-bind'ten önce
    /// exact-match fence'e girer.
    fn write_proposals_v2(&self, head: &str, from_path: &str, to_path: &str) -> std::path::PathBuf {
        let proposals = serde_json::json!({
            "schema_version": 2,
            "repository_head": head,
            "proposals": [{
                "removed_edges": [{"from": from_path, "to": to_path, "kind": "Imports"}],
                "affected_nodes": [from_path],
                "reasoning": "remove import to reduce coupling below threshold"
            }]
        });
        let path = self.work_path().join("proposals.v2.json");
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
        // Submodule fixture: alfabetik keşif sırası a.rs, b.rs, clients/fe/src/main.ts,
        // main.rs → main.rs node 3'e kayar (#156 R2 P1-2: exact kontrat gerçek
        // node kimliğine bağlanır).
        3 => "main.rs",
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
                        // Live Contract v1 authority profile (#151 Faz 0): coupling
                        // attempt pipeline'da TreeSitter ölçülür — declare edilmesi zorunlu
                        // (eski null, #96 MD-2 regolden bağlamındaydı; preflight artık
                        // coupling'de declaration'ı şart koşuyor).
                        "required_source": "TreeSitter",
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

/// #161/B5: path-keyed harness task envelope (v2). Anchor path doğrudan dosya
/// yoludur; node-id, attempt anındaki taze baseline'a karşı CLI'da çözümlenir.
fn task_envelope_v2(head: &str, anchor_path: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 2,
        "repository_head": head,
        "scope_bindings": [{"path": anchor_path}],
        "task": {
            "id": 7,
            "milestone_id": 1,
            "label": "completed-loop fixture (path-keyed v2)",
            "target_predicate_set": {
                "mode": "All",
                "predicates": [{
                    "predicate": {
                        "metric": "Coupling",
                        "operator": "Le",
                        "threshold": 0.55,
                        "scope": {"Path": anchor_path},
                        "required_source": "TreeSitter",
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
    // #161: schema_version 2 artık GEÇERLİ (path-keyed) — bilinmeyen sürüm reddi
    // 3 üzerinden pinlenir.
    let fx = HarnessFixture::new();
    let mut env = task_envelope(&fx.head, 0);
    env["schema_version"] = serde_json::json!(3);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(!output.status.success(), "unknown schema_version must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported schema_version"),
        "stderr: {stderr}"
    );
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
    // P0-3 (#155 analyzed-scope güncellemesi): ÖLÇÜLEN dosya modified → attempt
    // yine reddedilir; mesaj artık analyzed-scope fence'inden gelir ("analyzed
    // path is modified or untracked" + commit/stash önerisi). İlgisiz dosyaların
    // kirli olması artık run'ı bloklamaz (analyzed_scope_fence_allows_* testleri).
    let fx = HarnessFixture::new();
    fs::write(fx.repo_path().join("main.rs"), "pub fn main() {}\n").expect("dirty main.rs");
    let env = task_envelope(&fx.head, 0);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "modified analyzed file must fail pre-flight"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("analyzed path is modified or untracked")
            && stderr.contains("main.rs")
            && stderr.contains("commit/stash"),
        "stderr explains the analyzed-scope fence with a remedy: {stderr}"
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

    // **#96 iki-eksen + #95-A MD-1 subject cutover** (eski pin'ler tarihsel:
    // #96 öncesi authority=legacy_projected_v1; #95-A öncesi
    // subject_authority=affected_nodes — reason: `subject-cutover (#95-A)`;
    // deprecated `authority` alias yalnız provenance mirror).
    assert_eq!(
        envelope["execution_measurement"]["subject_authority"], "task_scope",
        "subject_authority: #95-A canonical task-scope cutover"
    );
    assert_eq!(
        envelope["execution_measurement"]["provenance_authority"], "engine_native_per_axis",
        "provenance_authority: #96 native per-axis cutover"
    );
    assert_eq!(
        envelope["execution_measurement"]["provenance_native"], true,
        "provenance_native=true (#96)"
    );
    // #100 (S5): deprecated `authority` alias kaldırıldı (tarihsel değer:
    // engine_native_per_axis — yalnız provenance mirror'ıydı).
    assert!(
        envelope["execution_measurement"].get("authority").is_none(),
        "#100: deprecated authority alias kaldirildi"
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

// ═══════════════════════════════════════════════════════════════════════════════
// #155 — analyzed-scope clean fence (Faz 1 ilk run friction: gerçek monorepo)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn analyzed_scope_fence_allows_untracked_outside_analysis() {
    // Faz 1 vaka: analiz kapsamı DIŞINDA aktif iş (untracked not/submodule işi),
    // ölçülen dosyalar HEAD-tracked ve değişmemiş → attempt ÇALIŞMALI
    // (eski global clean-worktree fence bu durumda reddediyordu).
    let fx = HarnessFixture::new();
    std::fs::write(fx.repo_path().join("notes-local.md"), "active dev work\n")
        .expect("write out-of-scope untracked file");
    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        output.status.success(),
        "out-of-scope untracked file must not block the attempt. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Task completed"), "got: {stdout}");
}

#[test]
fn analyzed_scope_fence_rejects_modified_analyzed_path() {
    // Fence'in epistemik çekirdeği korunur: ÖLÇÜLEN dosya modified ise
    // spesifik, eyleme dönüştürülebilir red (path + commit/stash önerisi).
    let fx = HarnessFixture::new();
    let a_path = fx.repo_path().join("a.rs");
    let original = std::fs::read_to_string(&a_path).expect("read a.rs");
    std::fs::write(&a_path, format!("{original}// local edit\n")).expect("modify analyzed file");
    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "modified analyzed file must fail the fence"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("analyzed path is modified or untracked"),
        "expected analyzed-scope fence message, got: {stderr}"
    );
    assert!(
        stderr.contains("a.rs") && stderr.contains("commit/stash"),
        "expected actionable path + remedy, got: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// #155 P1-1 — gerçek git submodule: hiyerarşik tracked set + prefix dirty fence
// ═══════════════════════════════════════════════════════════════════════════════

/// Parent repo (main.rs + a.rs + b.rs) + İÇİNDE gerçek `git submodule` (clients/fe,
/// içinde src/main.ts). Analyzer .ts analiz eder → nested dosya analyzed setinde.
struct SubmoduleFixture {
    parent: HarnessFixture,
    sub_nested: std::path::PathBuf,
}

impl SubmoduleFixture {
    fn new() -> Self {
        let mut parent = HarnessFixture::new();
        // 1) Submodule olacak repo: ayrı tempdir'de git init + ts dosyası + commit.
        let sub_repo = tempfile::tempdir().expect("sub tempdir");
        let s = sub_repo.path();
        let git = |args: &[&str], cwd: &std::path::Path| {
            let st = Command::new("git")
                .args(["-C", cwd.to_str().unwrap()])
                .args(args)
                .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
                .status()
                .expect("git");
            assert!(st.success(), "git {args:?} failed");
        };
        Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(s)
            .status()
            .expect("git init sub");
        git(&["config", "user.email", "osp-test@example.invalid"], s);
        git(&["config", "user.name", "OSP Test"], s);
        git(&["config", "core.autocrlf", "false"], s);
        std::fs::create_dir_all(s.join("src")).expect("mkdir src");
        std::fs::write(s.join("src/main.ts"), "export const x = 1;\n").expect("write ts");
        git(&["add", "-A"], s);
        git(&["commit", "-qm", "sub init"], s);

        // 2) Parent'a gerçek submodule olarak ekle (file:// URL) + parent commit.
        // Git 2.38+ file-protocol kısıtı: clone SÜB-PROSES'i repo config'ini görmez —
        // GIT_CONFIG_* env'i (süb-proseslere iner) ile aç.
        let st = Command::new("git")
            .args(["-C", parent.repo_path().to_str().unwrap()])
            .args(["submodule", "add"])
            // file:/// + slash'lı mutlak path (Windows "C:/..." ve Linux "/..." ikisinde de geçerli).
            .arg(format!(
                "file:///{}",
                s.to_str().unwrap().replace('\\', "/")
            ))
            .arg("clients/fe")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
            .env("GIT_CONFIG_VALUE_0", "always")
            .status()
            .expect("git submodule add");
        assert!(st.success(), "git submodule add failed");
        git(&["add", "-A"], parent.repo_path());
        git(&["commit", "-qm", "add submodule"], parent.repo_path());
        // Submodule ekleme commit'i parent HEAD'i değiştirdi — task binding
        // yeni HEAD'e bağlanmalı (fixture alanını güncelle).
        let new_head = String::from_utf8(
            Command::new("git")
                .args(["-C", parent.repo_path().to_str().unwrap()])
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git rev-parse")
                .stdout,
        )
        .expect("utf8 head");
        let sub_nested = parent.repo_path().join("clients/fe/src/main.ts");
        parent.head = new_head.trim().to_string();
        Self { parent, sub_nested }
    }
}

#[test]
fn real_submodule_clean_nested_analyzed_paths_pass() {
    // Clean + initialized submodule: gitlink SHA == sub HEAD → nested tree
    // `<sub>/…` önekiyle tracked sete girer → analyzed nested ts P1-2 fence'ini
    // GEÇMELİ (parent ls-tree'de görünmediği için eskiden hep red ediyordu).
    let fx = SubmoduleFixture::new();
    assert!(
        fx.sub_nested.exists(),
        "initialized submodule worktree expected"
    );
    // HEAD task'e bağlanır: parent HEAD submodule eklenmesiyle DEĞİŞTİ —
    // fixture kendi fx.parent.head'ini taşır.
    let env = task_envelope(&fx.parent.head, 3);
    let task_path = fx.parent.write_task(&env);
    let proposals_path = fx.parent.write_proposals(3, 1);
    let output = fx
        .parent
        .run_attempt(&task_path, &proposals_path, 7, "human");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Exact kontrat (#156 R2 P1-2): clean initialized submodule → attempt
    // BAŞARILI olmalı ve Task completed üretmeli. Zayıf `success || !contains`
    // assertion'ı yanlış-pozitife açıktı — scope-binding/node-id/ başka bir
    // preflight hatası da geçerdi.
    assert!(
        output.status.success(),
        "clean initialized submodule must complete the attempt. stderr={stderr}"
    );
    assert!(stdout.contains("Task completed"), "got: {stdout}");
}

#[test]
fn real_submodule_ignore_config_cannot_silence_the_fence() {
    // #156 R2 P1-1 (epistemik bypass): `submodule.<name>.ignore = dirty` config'i
    // submodule worktree değişikliklerini `git status`tan gizler. Fence
    // `--ignore-submodules=none` ile bunu override etmezse: HEAD == gitlink →
    // nested tracked sette, analyzer değiştirilmiş içeriği okur, dirty kümesi
    // boş → ölçüm commit'siz içerikten üretilebilir. Bu test config'in fence'i
    // susturamadığını pinler.
    let fx = SubmoduleFixture::new();
    std::fs::write(&fx.sub_nested, "export const x = 3; // local edit\n")
        .expect("modify nested ts");
    let st = Command::new("git")
        .args(["-C", fx.parent.repo_path().to_str().unwrap()])
        .args(["config", "submodule.clients/fe.ignore", "dirty"])
        .status()
        .expect("git config submodule ignore");
    assert!(st.success(), "set submodule ignore=dirty");
    let env = task_envelope(&fx.parent.head, 3);
    let task_path = fx.parent.write_task(&env);
    let proposals_path = fx.parent.write_proposals(3, 1);
    let output = fx
        .parent
        .run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "submodule ignore config must NOT silence the dirty-submodule fence"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dirty submodule/directory") && stderr.contains("clients/fe"),
        "expected dirty-submodule fence despite ignore=dirty config, got: {stderr}"
    );
}

#[test]
fn real_submodule_dirty_nested_analyzed_paths_reject() {
    // Submodule İÇİNDEKI dosya modify → parent porcelain ` m clients/fe` girdisi
    // üretir → analyzed nested path dirty-önek fence'inde RED (tam path + öneri).
    let fx = SubmoduleFixture::new();
    std::fs::write(&fx.sub_nested, "export const x = 2; // local edit\n")
        .expect("modify nested ts");
    let env = task_envelope(&fx.parent.head, 3);
    let task_path = fx.parent.write_task(&env);
    let proposals_path = fx.parent.write_proposals(3, 1);
    let output = fx
        .parent
        .run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "dirty submodule content in analysis scope must reject the attempt"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dirty submodule/directory")
            && stderr.contains("clients/fe")
            && stderr.contains("commit/stash"),
        "expected actionable dirty-submodule fence message, got: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// #161 (B5) — path-keyed v2: task + proposals uçtan uca
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn v2_path_keyed_completed_loop_exact_pin() {
    // v1 exact-pin testinin path-keyed karşılığı: dosyalar id AVI gerektirmeden
    // main.rs/b.rs yollarıyla bağlanır; Completed + coupling düşüşü aynı ölçülür.
    // main.rs 2 outgoing imports → coupling 2/3 ≈ 0.667; RemoveImport main→b
    // → 1/2 = 0.5 ≤ 0.55 → Completed.
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope_v2(&fx.head, "main.rs"));
    let proposals_path = fx.write_proposals_v2(&fx.head, "main.rs", "b.rs");
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "json");

    assert!(
        output.status.success(),
        "path-keyed v2 Completed-loop must exit 0. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let envelope: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not JSON envelope: {e}\n{stdout}"));

    assert_eq!(envelope["run"]["task_source"], "harness_task_file");
    assert_eq!(envelope["result"]["kind"], "completed", "v2 result kind");
    assert_eq!(
        envelope["result"]["attempts"].as_u64(),
        Some(1),
        "single-attempt Completed (path-keyed)"
    );

    let evidence = envelope["evidence"].as_array().expect("evidence array");
    assert_eq!(evidence.len(), 1);
    let entry = &evidence[0];
    assert_eq!(entry["gate_decision"], "PassedAll");
    assert_eq!(entry["predicate_completion"], "Completed");
    assert_eq!(entry["mutation_decision"], "AcceptAsCompleted");

    let before_coupling = entry["before"]["x"].as_f64().expect("before coupling");
    let after_coupling = entry["after"]["x"].as_f64().expect("after coupling");
    assert!(
        before_coupling > 0.55,
        "before > threshold: {before_coupling}"
    );
    assert!(
        after_coupling <= 0.55,
        "after ≤ threshold: {after_coupling}"
    );
    assert!(after_coupling < before_coupling, "coupling must decrease");

    fx.assert_repo_clean();
}

#[test]
fn v2_task_unknown_path_fails_closed_at_cli() {
    let fx = HarnessFixture::new();
    let env = task_envelope_v2(&fx.head, "src/nonexistent.rs");
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals_v2(&fx.head, "main.rs", "b.rs");
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "unknown path must fail pre-flight"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not present in analysis node_paths")
            && stderr.contains("src/nonexistent.rs"),
        "stderr names the unresolvable path: {stderr}"
    );
    fx.assert_repo_clean();
}

#[test]
fn v2_proposals_unknown_path_fails_closed_at_cli() {
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope_v2(&fx.head, "main.rs"));
    let proposals_path = fx.write_proposals_v2(&fx.head, "main.rs", "src/stale.rs");
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(!output.status.success(), "unknown proposal path must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("removed_edges.to") && stderr.contains("src/stale.rs"),
        "stderr names the field + path: {stderr}"
    );
}

#[test]
fn v1_task_with_v2_proposals_mix_works() {
    // Dispatch'ler bağımsız: v1 id-keyed task + v2 path-keyed proposals karışımı
    // geçerli (run 7-8 deseni: task el yazımı id'lerle, proposals taze).
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals_v2(&fx.head, "main.rs", "b.rs");
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "json");
    assert!(
        output.status.success(),
        "mixed v1-task + v2-proposals must run. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let envelope: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not JSON envelope: {e}\n{stdout}"));
    assert_eq!(envelope["result"]["kind"], "completed");
}

#[test]
fn v2_proposal_head_mismatch_rejected_at_cli() {
    // R1 P1-1: proposals v2 envelope kendi üretim state'ini taşır; HEAD=A'da
    // üretilmiş proposal HEAD=B'ye sessizce re-bind edilemez — fence re-bind'ten
    // önce, path'ler geçerli olsa bile reddeder.
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope_v2(&fx.head, "main.rs"));
    let proposals_path = fx.write_proposals_v2(&"f".repeat(40), "main.rs", "b.rs");
    let output = fx.run_attempt(&task_path, &proposals_path, 7, "human");
    assert!(
        !output.status.success(),
        "proposal HEAD mismatch must fail pre-flight"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("repository HEAD mismatch"),
        "stderr explains the proposal provenance fence: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// #152 — persisted space identity: production witness-hold artık D3'te değil
// ═══════════════════════════════════════════════════════════════════════════════

fn run_production_attempt(
    fx: &HarnessFixture,
    task_path: &std::path::Path,
    proposals_path: &std::path::Path,
) -> std::process::Output {
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::cargo_bin("osp")
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
        .arg(proposals_path)
        .arg("--task")
        .arg(task_path)
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp")
}

#[test]
fn production_witness_awaits_with_persisted_identity() {
    // #152: predicate satisfied (coupling 0.667 → 0.5 ≤ 0.55) + production witness
    // (quorum 2/1.5, boş witness seti) → Held. Persisted identity load-or-create
    // edildiği için D3 (Ephemeral + CrossProcess → exit 70) GEÇİLİR:
    // AwaitingWitnesses (exit 10 — expected domain outcome) + gerçek pending
    // artifact + space-identity, ikisi de state-dir altında (repo temiz kalır).
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals(2, 1);

    let output = run_production_attempt(&fx, &task_path, &proposals_path);
    assert_eq!(
        output.status.code(),
        Some(10),
        "AwaitingWitnesses exit 10 (not D3 SystemFailure 70). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let osp_dir = fx.work_path().join(".osp");
    assert!(
        osp_dir.join("space-identity").exists(),
        "persisted space identity created under state-dir"
    );
    let pending_dir = osp_dir.join("pending-authorizations");
    let artifact_count = pending_dir
        .read_dir()
        .expect("pending-authorizations dir")
        .count();
    assert!(
        artifact_count >= 1,
        "pending-authorization artifact persisted"
    );

    // NOT (kapsam gözlemi): AYNI task'ın tekrar suspend edilmeye çalışması
    // BasisConflict → exit 40 üretir (envelope zaman damgalı olduğundan
    // idempotent-success yoluna düşmez). Bu, #152 kapsamındaki resume akışının
    // girdisidir (issue'ya notlandı) — identity kararlılığı birim testte pinli
    // (load_or_create reload aynı id).

    fx.assert_repo_clean();
}

#[test]
fn corrupted_space_identity_fails_exit_70() {
    // #152: bozuk identity dosyası otomatik yeniden ÜRETİLMEZ — SystemFailure
    // bucket (exit 70), dosya olduğu gibi korunur (operator müdahalesi gerekir).
    let fx = HarnessFixture::new();
    let osp_dir = fx.work_path().join(".osp");
    fs::create_dir_all(&osp_dir).expect("osp dir");
    let corrupted = b"{ this is not valid json";
    fs::write(osp_dir.join("space-identity"), corrupted).expect("corrupt identity");

    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals(2, 1);
    let output = run_production_attempt(&fx, &task_path, &proposals_path);

    assert_eq!(output.status.code(), Some(70), "SystemFailure exit 70");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("space identity"),
        "stderr names the identity failure: {stderr}"
    );
    assert_eq!(
        fs::read(osp_dir.join("space-identity")).unwrap(),
        corrupted,
        "corrupted identity preserved (no silent regeneration)"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_state_dir_inside_repo_rejected() {
    // #152 R1 P1-2: identity artık her attempt'te <state-dir>/.osp/space-identity
    // yazıyor; production default CWD repo kökü olduğunda dosya repoya düşerdi
    // (.osp gitignore'da YOK). Fence her iki mode'da: state-dir (default CWD
    // dahil) analyzed repo içinde → red + açık --state-dir iste. Hiçbir yazma
    // gerçekleşmeden reddedilir → repo temiz kalır.
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals(2, 1);
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.repo_path()) // CWD = analyzed repo kökü → default state-dir repo içinde
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
        .arg("--task")
        .arg(&task_path)
        .output()
        .expect("run osp");
    assert!(
        !output.status.success(),
        "state-dir inside repo must fail in every execution mode"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr explains the fence: {stderr}"
    );
    assert!(
        !fx.repo_path().join(".osp").exists(),
        "no identity or artifacts written into the repo"
    );
    fx.assert_repo_clean();
}

#[test]
fn state_dir_relative_missing_deep_path_rejected() {
    // #152 R2 P1: relative + henüz VAR OLMAYAN path (--state-dir deep/missing,
    // repo kökünden) eskiden parent canonicalize başarısız olduğunda raw
    // relative kalıyordu → absolute repo ile karşılaştırma her zaman false →
    // fence BYPASS → .osp/space-identity repoya yazılırdı. Artık mutlaklaştır +
    // var olan atadan canonicalize → red, hiçbir yazma olmadan.
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals(2, 1);
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.repo_path()) // relative state-dir repo köküne çözümlenir
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--execution-mode")
        .arg("harness")
        .arg("--witness")
        .arg("harness-auto-approve")
        .arg("--llm")
        .arg("mock")
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg("deep/missing/nested") // deep/ repoda YOK — eski kod bypass ediyordu
        .output()
        .expect("run osp");
    assert!(
        !output.status.success(),
        "relative missing state-dir inside repo must be rejected (R2 P1)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr: {stderr}"
    );
    assert!(
        !fx.repo_path().join(".osp").exists(),
        "no identity written into the repo (bypass closed)"
    );
    fx.assert_repo_clean();
}

#[test]
fn state_dir_dotdot_path_resolved_before_fence() {
    // ".." bileşenli state-dir lexically normalize edilir: sub/.. → repo kökü →
    // RED. (Karşılaştırma ham prefix değil, normalize edilmiş mutlak yol.)
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals(2, 1);
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.repo_path())
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--execution-mode")
        .arg("harness")
        .arg("--witness")
        .arg("harness-auto-approve")
        .arg("--llm")
        .arg("mock")
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg("a/../b/..") // lexical → repo kökü
        .output()
        .expect("run osp");
    assert!(
        !output.status.success(),
        "dotdot-resolved-inside-repo state-dir must be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr: {stderr}"
    );
    fx.assert_repo_clean();
}
