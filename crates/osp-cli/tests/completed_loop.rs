//! Faz 8 test-project B-3 — Completed-loop integration test matrix.
//!
//! Completed-loop'u uçtan uca doğrular. osp-cli bin-only crate olduğu için tüm testler
//! gerçek `osp` binary'sini çağırır (assert_cmd pattern — mevcut testlerle tutarlı).
//!
//! Fixture topolojisi (#173): varsayılan `HarnessFixture::new()` mod-bildirimlidir ve
//! güncel Rust adaptöründe 0 kenar üretir — yalnız wiring/rejection testleri için.
//! Kenar/coupling önermesi taşıyan testler `new_with_use_edges()` kullanır: main.rs →
//! a.rs + b.rs (2 çıkan kenar, coupling 2/3 = 0.667); RemoveImport → 1/2 = 0.5 ≤ 0.55
//! → Completed. Evidence semantiği (#173 repro): `before` G1 bootstrap tohumudur
//! (x=0.7, MetricSource::Placeholder — ÖLÇÜM DEĞİL, commands/mod.rs run_navigator);
//! `after` motordan ölçülür ve fixture'a duyarlıdır. Bu yüzden before pinleri tohumu,
//! after pinleri gerçek ölçümü sabitler.
//!
//! **Isolation**: `osp trajectory attempt` `FilesystemPendingAuthorizationStore::new(".")`
//! CWD .osp/'ye yazar. Test CWD'si analyzed repo DIŞINDA ayrı bir tempdir'dir (repo
//! git status'ünü kirletmemesi için). `OSP_ATTEMPT_LOCK` .osp/ paralel contamination'ı
//! önler (yine de defensive — her test ayrı CWD kullanır).

#![cfg(test)]

mod common;

use common::*;
use std::fs;
use std::process::Command;

use assert_cmd::prelude::*;

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
    // #166 akış ayrımı: progress artık stderr'de; "navigator'a ulaştı" sinyali
    // stderr progress'inden veya başarılı exit'ten gelir.
    assert!(
        stderr.contains("Evidence entries")
            || stderr.contains("Task completed")
            || stderr.contains("Maneuver limit")
            || stderr.contains("Awaiting witnesses")
            || stderr.contains("Canonical attempt artifact")
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
    // Must reach navigator (not fail on state-dir invariant). #166: progress stderr'de.
    assert!(
        stderr.contains("Evidence entries")
            || stderr.contains("Task completed")
            || stderr.contains("Maneuver limit")
            || stderr.contains("Awaiting witnesses")
            || stderr.contains("Canonical attempt artifact")
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
    // #166 akış ayrımı: progress stderr'de.
    assert!(
        stderr.contains("Evidence entries")
            || stderr.contains("Task completed")
            || stderr.contains("Maneuver limit")
            || stderr.contains("Awaiting witnesses")
            || stderr.contains("Canonical attempt artifact")
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
    // #166 akış ayrımı: progress stderr'de.
    assert!(
        stderr.contains("Evidence entries")
            || stderr.contains("Awaiting witnesses")
            || stderr.contains("Maneuver limit")
            || stderr.contains("Task completed")
            || stderr.contains("Canonical attempt artifact"),
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
    // #166 akış ayrımı: progress stderr'de.
    assert!(
        stderr.contains("Evidence entries")
            || stderr.contains("Awaiting witnesses")
            || stderr.contains("Maneuver limit")
            || stderr.contains("Task completed")
            || stderr.contains("Canonical attempt artifact"),
        "harness+production-witness+task must reach navigator. stdout={stdout}\nstderr={stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Review P0 — Completed-loop exact pin via versioned JSON envelope
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn completed_loop_exact_pin_via_json_envelope() {
    // Reviewer'ın istediği exact V1 run envelope + state-transition assertion.
    // use-edges fixture (#173): main.rs (Node 2) GERÇEK 2 çıkan use-kenarı →
    // ölçülen coupling 2/3 = 0.667; RemoveImport 2→1 → ölçülen 1/2 = 0.5 ≤ 0.55
    // → Completed. Evidence `before` fixture'tan DEĞİL, G1 bootstrap tohumundan
    // gelir (x=0.7 Placeholder — repro #173); `after` ölçülür (0.5 tam pin).
    let fx = HarnessFixture::new_with_use_edges();
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

    // before/after coupling (#173 semantiği + review tur-1 P1-2): üç kanal ayrışır —
    // (i) evidence `before` = G1 bootstrap tohumu (Placeholder; graf ölçümü DEĞİL),
    // (ii) fixture'ın ölçülen baseline'ı (analyze kanalı; gerçek 2/3),
    // (iii) evidence `after` = manevra sonrası motor ölçümü (1/2).
    // Geçiş iddiası yalnız (ii)↔(iii) arasında kurulur (ölçüm↔ölçüm); tohumla
    // ölçüm karşılaştırılmaz.
    let before_coupling = entry["before"]["x"].as_f64().expect("before coupling (x)");
    let after_coupling = entry["after"]["x"].as_f64().expect("after coupling (x)");
    assert!(
        (before_coupling - 0.7).abs() < 1e-9,
        "before.x == bootstrap seed 0.7 (Placeholder, not a graph measurement): {before_coupling}"
    );
    let baseline_coupling = fx.measured_main_coupling();
    assert!(
        (baseline_coupling - 2.0 / 3.0).abs() < 1e-9,
        "fixture baseline (analyze) must be 2/3: {baseline_coupling}"
    );
    assert!(
        (after_coupling - 0.5).abs() < 1e-9,
        "after.x == measured 1/2 (RemoveImport 2→1 on use-edges fixture): {after_coupling}"
    );
    assert!(
        after_coupling < baseline_coupling,
        "real graph transition 2/3 → 1/2 (measured ↔ measured): after={after_coupling}, baseline={baseline_coupling}"
    );
    assert!(
        after_coupling <= 0.55,
        "after coupling ≤ threshold (0.55): after={after_coupling}"
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
    // use-edges fixture (#173 review tur-1 P1-1): başarı tanığı gerçek
    // RemoveImport geçişinden gelir — 0-kenarlı fixture'ta Completed,
    // fence değil coupling vakuitiesi sayesinde yeşil düşerdi.
    let fx = HarnessFixture::new_with_use_edges();
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
    // #166 akış ayrımı: progress stderr'de.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Task completed"), "got: {stderr}");
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
    /// #204: submodule KAYNAK repo'su canlı tutulur — detached worktree'ler
    /// .gitmodules URL'sinden yeniden klonlar (tempdir düşerse kaynak gider).
    _sub_source: tempfile::TempDir,
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
        Self {
            parent,
            sub_nested,
            _sub_source: sub_repo,
        }
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
    // #210 N1: ölçüm kanonik (detached @ HEAD, submodule init'li) — file://
    // submodule init izni CAĞIRANDAN (run_attempt_file_submodules).
    let output = fx
        .parent
        .run_attempt_file_submodules(&task_path, &proposals_path, 7, "human");
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Exact kontrat (#156 R2 P1-2): clean initialized submodule → attempt
    // BAŞARILI olmalı ve Task completed üretmeli. Zayıf `success || !contains`
    // assertion'ı yanlış-pozitife açıktı — scope-binding/node-id/ başka bir
    // preflight hatası da geçerdi.
    assert!(
        output.status.success(),
        "clean initialized submodule must complete the attempt. stderr={stderr}"
    );
    // #166 akış ayrımı: progress stderr'de.
    assert!(stderr.contains("Task completed"), "got: {stderr}");
}

/// #210 (N1) kapsam-fence regresyonu: çağırıcının worktree'sinde submodule
/// İNİT'SİZ (deinit — gitlink durur, status TEMİZ) → kanonik ölçum submodule
/// içeriğini YİNE görür (detached worktree pinned gitlink'lerde init'li) ve
/// analyzed ⊆ kanonik tracked fence'i GEÇER. N1 öncesi: canlı ölçüm içeriği
/// görmezdi (ölçüm kapsamı worktree init tesadüfüne bağlıydı); N1'siz bu
/// deinit durumu "analyzed path not tracked" yanlış-RED'i üretirdi.
#[test]
fn strict_attempt_completes_with_uninitialized_submodule() {
    let fx = SubmoduleFixture::new();
    let st = Command::new("git")
        .args(["-C", fx.parent.repo_path().to_str().unwrap()])
        .args(["submodule", "deinit", "--force", "clients/fe"])
        .status()
        .expect("git submodule deinit");
    assert!(st.success(), "submodule deinit failed");
    assert!(
        !fx.sub_nested.exists(),
        "caller worktree submodule content is gone (uninitialized)"
    );
    let status = Command::new("git")
        .args([
            "-C",
            fx.parent.repo_path().to_str().unwrap(),
            "status",
            "--porcelain",
            "--untracked-files=all",
        ])
        .output()
        .expect("git status");
    assert!(
        String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "deinit leaves the caller worktree CLEAN (gitlink unchanged)"
    );

    let env = task_envelope(&fx.parent.head, 3);
    let task_path = fx.parent.write_task(&env);
    let proposals_path = fx.parent.write_proposals(3, 1);
    let output = fx
        .parent
        .run_attempt_file_submodules(&task_path, &proposals_path, 7, "human");
    assert!(
        output.status.success(),
        "uninitialized caller submodule must NOT change the canonical measurement \
         scope (fence uses the canonical tracked set). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Task completed"),
        "got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
        .run_attempt_file_submodules(&task_path, &proposals_path, 7, "human");
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
        .run_attempt_file_submodules(&task_path, &proposals_path, 7, "human");
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
    // use-edges fixture (#173): main.rs GERÇEK 2 çıkan use-kenarı → ölçülen
    // 2/3 = 0.667; RemoveImport main→b → ölçülen 1/2 = 0.5 ≤ 0.55 → Completed.
    // `before` bootstrap tohumu 0.7'dir (Placeholder — v1 exact-pin testindeki
    // semantik); `after` ölçülür.
    let fx = HarnessFixture::new_with_use_edges();
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
        (before_coupling - 0.7).abs() < 1e-9,
        "before.x == bootstrap seed 0.7 (Placeholder): {before_coupling}"
    );
    let baseline_coupling = fx.measured_main_coupling();
    assert!(
        (baseline_coupling - 2.0 / 3.0).abs() < 1e-9,
        "fixture baseline (analyze) must be 2/3: {baseline_coupling}"
    );
    assert!(
        (after_coupling - 0.5).abs() < 1e-9,
        "after.x == measured 1/2 (use-edges fixture): {after_coupling}"
    );
    // Geçiş iddiası ölçüm↔ölçüm kurulur (review tur-1 P1-2); tohum (0.7)
    // yalnız telemetry olarak yukarıda pinlendi.
    assert!(
        after_coupling < baseline_coupling,
        "real graph transition 2/3 → 1/2: after={after_coupling}, baseline={baseline_coupling}"
    );
    assert!(
        after_coupling <= 0.55,
        "after ≤ threshold: {after_coupling}"
    );

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
    // use-edges fixture (#173 review tur-1 P1-1): main→b kenarı GERÇEK vardır —
    // Completed, path-keyed proposal'ın gerçekten bind edilip uygulandığının
    // kanıtıdır; 0-kenarlı fixture'ta bu yine vakuite olurdu.
    let fx = HarnessFixture::new_with_use_edges();
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
    // #152: predicate satisfied (ölçülen after 0.5 ≤ 0.55 — use-edges fixture #173;
    // `before` bootstrap tohumu 0.7'dir, graf ölçümü değil) + production witness
    // (quorum 2/1.5, boş witness seti) → Held. Persisted identity load-or-create
    // edildiği için D3 (Ephemeral + CrossProcess → exit 70) GEÇİLİR:
    // AwaitingWitnesses (exit 10 — expected domain outcome) + gerçek pending
    // artifact + space-identity, ikisi de state-dir altında (repo temiz kalır).
    let fx = HarnessFixture::new_with_use_edges();
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

// ═══════════════════════════════════════════════════════════════════════════════
// #164 — Resume flow: askılı (exit 10) görevin artifact'tan sürdürülmesi
// ═══════════════════════════════════════════════════════════════════════════════

/// Askı üret + tek pending artifact'ının path'ini ve claim_id'sini döndür.
fn suspend_and_locate_artifact() -> (HarnessFixture, std::path::PathBuf, u64) {
    let fx = HarnessFixture::new();
    let task_path = fx.write_task(&task_envelope(&fx.head, 2));
    let proposals_path = fx.write_proposals(2, 1);
    let output = run_production_attempt(&fx, &task_path, &proposals_path);
    assert_eq!(
        output.status.code(),
        Some(10),
        "suspend precondition (exit 10). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let pending_dir = fx.work_path().join(".osp").join("pending-authorizations");
    let mut artifacts: Vec<std::path::PathBuf> = pending_dir
        .read_dir()
        .expect("pending-authorizations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().map(|e| e == "json").unwrap_or(false)
                && !p
                    .file_name()
                    .map(|n| n.to_string_lossy().contains("receipt"))
                    .unwrap_or(false)
        })
        .collect();
    artifacts.sort();
    assert_eq!(artifacts.len(), 1, "exactly one pending artifact");
    let artifact = artifacts.pop().unwrap();

    let raw = fs::read_to_string(&artifact).expect("artifact read");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("artifact JSON");
    let claim_id = v["record"]["claim_id"].as_u64().expect("claim_id") as u64;
    (fx, artifact, claim_id)
}

/// Witness kanıt dosyası yaz — claim'e bağlı N olay (actor'ler author'dan farklı).
fn write_witness_evidence(
    fx: &HarnessFixture,
    claim_id: u64,
    actors: &[u64],
) -> std::path::PathBuf {
    let events: Vec<serde_json::Value> = actors
        .iter()
        .enumerate()
        .map(|(i, actor)| {
            serde_json::json!({
                "id": (i + 1) as u64,
                "source": format!("test-evidence-{i}"),
                "witness_kind": "MergeCommit",
                "actor": actor,
                "claim": claim_id,
            })
        })
        .collect();
    let path = fx.work_path().join(format!("witnesses-{claim_id}.json"));
    fs::write(&path, serde_json::to_string_pretty(&events).unwrap()).expect("write evidence");
    path
}

fn run_resume(
    fx: &HarnessFixture,
    artifact: &std::path::Path,
    evidence: &std::path::Path,
    format: &str,
) -> std::process::Output {
    run_resume_opt(fx, artifact, evidence, format, true)
}

/// `with_trust = false` → `--evidence-trust` beyanı verilmez (R2 P0 testi).
fn run_resume_opt(
    fx: &HarnessFixture,
    artifact: &std::path::Path,
    evidence: &std::path::Path,
    format: &str,
    with_trust: bool,
) -> std::process::Output {
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    cmd.current_dir(fx.work_path())
        .arg("trajectory")
        .arg("resume")
        .arg(artifact)
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--witness-evidence")
        .arg(evidence)
        .arg("--state-dir")
        .arg(fx.work_path())
        .args(["--format", format]);
    if with_trust {
        cmd.args(["--evidence-trust", "operator-asserted"]);
    }
    cmd.output().expect("run osp resume")
}

#[test]
fn production_resume_applies_with_two_witnesses() {
    // #164: attempt exit 10 → 2 witness kanıtıyla resume → quorum (2/1.5,
    // MergeCommit 1.0+1.0=2.0 ≥ 1.5) → kayıtlı delta uygulanır → exit 0 +
    // kalıcı receipt. Repo dokunulmaz (engine-space apply process-inner).
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let evidence = write_witness_evidence(&fx, claim_id, &[200, 300]);

    let output = run_resume(&fx, &artifact, &evidence, "json");
    assert_eq!(
        output.status.code(),
        Some(0),
        "applied resume exit 0. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect("resume JSON envelope");
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["result"]["kind"], "applied");
    assert_eq!(v["result"]["witness"]["approvers"], 2);
    assert_eq!(v["evidence_trust"], "operator_asserted");

    // Kalıcı receipt — artifact kardeşi. R2 P1: kanıt bağlama alanları
    // (trust beyanı + kanıt digest'i + actor listesi) envelope ile aynı kaynak.
    let receipt_path = artifact.with_extension("json.receipt.json");
    let receipt_raw =
        fs::read_to_string(&receipt_path).expect("resume receipt persisted next to artifact");
    let receipt: serde_json::Value = serde_json::from_str(&receipt_raw).unwrap();
    assert_eq!(receipt["schema"], "osp.resume-receipt.v1");
    assert_eq!(receipt["claim_id"].as_u64(), Some(claim_id));
    assert_eq!(receipt["resulting_sequence"].as_u64(), Some(1));
    assert_eq!(receipt["evidence_trust"], "operator_asserted");
    assert_eq!(receipt["witness_actors"], serde_json::json!([200, 300]));
    assert_eq!(
        receipt["witness_evidence_digest_hex"], v["result"]["witness_evidence_digest"],
        "receipt binds the same evaluated-evidence digest the envelope reported"
    );
    assert_eq!(
        receipt["witness_evidence_digest_hex"]
            .as_str()
            .unwrap()
            .len(),
        64
    );

    fx.assert_repo_clean();
}

#[test]
fn production_resume_requires_explicit_evidence_trust_declaration() {
    // R2 P0: raw JSON doğrudan production authorization sağlayamaz — güven
    // sınırı beyanı (--evidence-trust) ZORUNLU; beyansız çağrı clap red
    // (exit 2), hiçbir iş yapılmadan.
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let evidence = write_witness_evidence(&fx, claim_id, &[200, 300]);

    let output = run_resume_opt(&fx, &artifact, &evidence, "human", false);
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing --evidence-trust → clap usage error. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--evidence-trust"),
        "stderr names the required trust declaration"
    );
    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt without the trust declaration"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_resume_insufficient_witnesses_still_held() {
    // #164: tek witness → quorum yetersiz → exit 10 (artifact DEĞİŞMEZ, receipt YOK).
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let evidence = write_witness_evidence(&fx, claim_id, &[200]);

    let output = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        output.status.code(),
        Some(10),
        "insufficient quorum → still awaiting. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Still awaiting witnesses"),
        "human output explains hold: {stdout}"
    );
    assert!(
        stdout.contains("does NOT accumulate"),
        "R2 P2: message states non-accumulation semantics: {stdout}"
    );
    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt on still-held"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_resume_twice_refused_by_receipt() {
    // #164 applied-idempotency: başarılı resume sonrası AYNI artifact'ın ikinci
    // resume'u receipt fence'ine takılır → exit 15 (çift-yetkilendirme kapalı).
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let evidence = write_witness_evidence(&fx, claim_id, &[200, 300]);

    let first = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(first.status.code(), Some(0), "first resume applies");

    let second = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        second.status.code(),
        Some(15),
        "second resume refused by receipt fence. stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("already applied"),
        "stderr explains the refusal"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_resume_foreign_claim_evidence_exit_20() {
    // #164 claim-binding fence: kanıt olayı başka claim'e → exit 20 (operational
    // fault — yabancı kanıtla bu askı yetkilendirilemez).
    let (fx, artifact, _claim_id) = suspend_and_locate_artifact();
    let evidence = write_witness_evidence(&fx, 9999, &[200, 300]);

    let output = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        output.status.code(),
        Some(20),
        "foreign-claim evidence → witness evaluation error. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("witnesses claim"),
        "stderr names the binding mismatch"
    );
    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt on rejected evidence"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_resume_defective_evidence_exit_20() {
    // #164 R1 P1-2 — INV-T9 sözleşmesi: author-self / duplicate kanıt operational
    // fault (exit 20); sessiz dışlama/dedup YOK. Author = navigator agent id 1.
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();

    // (a) author-self: actor 1 (navigator agent) + geçerli witness.
    let evidence = write_witness_evidence(&fx, claim_id, &[1, 200]);
    let output = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        output.status.code(),
        Some(20),
        "author-self evidence → exit 20. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("claim author"),
        "stderr explains author-self rejection"
    );

    // (b) duplicate actor (aynı actor, farklı kaynak) — #212'den beri çekirdek
    // actor-eksenli seçer; wire yüzeyi yine de dosya kusuru olarak reddeder.
    let dup_path = fx.work_path().join("witnesses-dup.json");
    let dup_events = serde_json::json!([
        { "id": 1, "source": "PR#1", "witness_kind": "MergeCommit", "actor": 200, "claim": claim_id },
        { "id": 2, "source": "commit-abc", "witness_kind": "MergeCommit", "actor": 200, "claim": claim_id },
    ]);
    fs::write(
        &dup_path,
        serde_json::to_string_pretty(&dup_events).unwrap(),
    )
    .expect("write dup evidence");
    let output = run_resume(&fx, &artifact, &dup_path, "human");
    assert_eq!(
        output.status.code(),
        Some(20),
        "duplicate-actor evidence → exit 20. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("duplicate actor"),
        "stderr explains duplicate-actor rejection"
    );

    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt on defective evidence"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_resume_via_copied_artifact_still_blocked_after_apply() {
    // #164 R1 P0-1 — receipt EVIDENCE IDENTITY'den adreslenir (caller path'inden
    // DEĞİL): geçerli artifact'ı copy.json'a kopyala → oradan resume ET (uygular,
    // receipt canonical adrese yazılır) → ORİJİNAL path'ten ikinci resume yine
    // "already applied" ile reddedilir (çift-yetkilendirme kapalı).
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let copied = fx.work_path().join("copy.json");
    fs::copy(&artifact, &copied).expect("copy artifact");

    let evidence = write_witness_evidence(&fx, claim_id, &[200, 300]);

    // Kopyadan ilk resume — uygular (exit 0); receipt kopyanın yanına DEĞİL
    // canonical identity adresine yazılır.
    let first = run_resume(&fx, &copied, &evidence, "human");
    assert_eq!(
        first.status.code(),
        Some(0),
        "resume from copied artifact path applies. stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        !copied.with_extension("json.receipt.json").exists(),
        "receipt must NOT be written next to the copied path"
    );
    assert!(
        artifact.with_extension("json.receipt.json").exists(),
        "receipt written at the canonical identity-keyed address"
    );

    // Orijinal path'ten ikinci resume → receipt bulunur → exit 15.
    let second = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        second.status.code(),
        Some(15),
        "resume via original path after copy-apply must be refused. stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("already applied"),
        "stderr explains the refusal"
    );
    fx.assert_repo_clean();
}

#[test]
fn production_resume_forged_receipt_fails_closed_exit_70() {
    // #164 R1 P1-1 — parse-edilebilir-ama-sahte receipt "already applied"
    // durumuna DÖNÜŞEMEZ: strict wire + identity doğrulaması → exit 70.
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();

    // Sahte receipt: tip şekli doğru (yeni kanıt alanları dahil), schema +
    // claim yabancı — red verify katmanından gelmeli (wire'dan değil).
    let receipt_path = artifact.with_extension("json.receipt.json");
    let forged = serde_json::json!({
        "schema": "osp.resume-receipt.v9",
        "task_id": 1,
        "claim_id": claim_id + 1000,
        "attempt_num": 1,
        "evidence_digest_hex": format!("{:064x}", 0),
        "applied_at": 1,
        "resulting_sequence": 1,
        "evidence_trust": "operator_asserted",
        "witness_evidence_digest_hex": "ab".repeat(32),
        "witness_actors": [200, 300],
    });
    fs::write(
        &receipt_path,
        serde_json::to_string_pretty(&forged).unwrap(),
    )
    .expect("write forged receipt");

    let evidence = write_witness_evidence(&fx, claim_id, &[200, 300]);
    let output = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        output.status.code(),
        Some(70),
        "forged receipt must fail closed (not already-applied, not applied). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid"),
        "stderr names the receipt integrity failure"
    );
    assert!(
        !output.status.success(),
        "forged receipt never yields a successful apply"
    );
    fx.assert_repo_clean();
}

// ═══════════════════════════════════════════════════════════════════════════════
// #205 — `osp witness-evidence --template`: biçim keşfedilebilirliği.
// Protokol kanıtın nasıl İFADE edileceğini öğretir; kanıt üretmez (karar-5).
// ═══════════════════════════════════════════════════════════════════════════════

/// #205: `osp witness-evidence --template [--artifact …]` subprocess.
fn run_witness_evidence(
    cwd: &std::path::Path,
    artifact: Option<&std::path::Path>,
) -> std::process::Output {
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    cmd.current_dir(cwd)
        .arg("witness-evidence")
        .arg("--template");
    if let Some(path) = artifact {
        cmd.arg("--artifact").arg(path);
    }
    cmd.output().expect("run osp witness-evidence")
}

/// #205 testleri için CWD olarak kullanılacak boş temp dizini (komut yazmaz;
/// fixture'sız vakalar için yeterli izolasyon).
fn template_cwd(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("osp205-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("template test cwd");
    dir
}

#[test]
fn witness_evidence_template_without_artifact_uses_claim_zero() {
    // Karar-3: artifact'siz şablon claim 0 placeholder'ı basar + uyarı;
    // stdout saf JSON (parse edilen tek yüzey), rehberlik stderr'de.
    let cwd = template_cwd("t6");
    let output = run_witness_evidence(&cwd, None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "template print succeeds. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("stdout is pure parseable JSON (redirect-ready)");
    let arr = v.as_array().expect("root is an array");
    assert_eq!(arr.len(), 1, "template carries a single event");
    assert_eq!(arr[0]["claim"].as_u64(), Some(0), "no artifact → claim 0");
    let mut keys: Vec<&str> = arr[0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["actor", "claim", "id", "source", "witness_kind"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("claim 0"),
        "stderr warns about the placeholder: {stderr}"
    );
    assert!(
        stderr.contains("1.0"),
        "stderr shows the weight table: {stderr}"
    );
    assert!(
        stderr.contains("does NOT satisfy quorum"),
        "stderr states the non-authorizing nature: {stderr}"
    );
    let _ = fs::remove_dir_all(&cwd);
}

#[test]
fn witness_evidence_requires_explicit_template_flag() {
    // Tur-3 notu A / tur-1 P2-2: `--template` ZORUNLU — bare komut ve
    // yalnız-`--artifact` çağrısı clap usage hatası (exit 2), çıktı yok.
    let cwd = template_cwd("t8");
    let dummy_artifact = cwd.join("dummy.json");
    fs::write(&dummy_artifact, "{}").expect("dummy artifact");

    let bare = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(&cwd)
        .arg("witness-evidence")
        .output()
        .expect("bare invocation");
    assert_eq!(
        bare.status.code(),
        Some(2),
        "bare `osp witness-evidence` → usage error. stderr={}",
        String::from_utf8_lossy(&bare.stderr)
    );
    assert!(
        bare.stdout.is_empty(),
        "no template without the explicit flag"
    );

    let artifact_only = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(&cwd)
        .arg("witness-evidence")
        .arg("--artifact")
        .arg(&dummy_artifact)
        .output()
        .expect("artifact-only invocation");
    assert_eq!(
        artifact_only.status.code(),
        Some(2),
        "`--artifact` alone is not enough. stderr={}",
        String::from_utf8_lossy(&artifact_only.stderr)
    );
    assert!(artifact_only.stdout.is_empty());
    let _ = fs::remove_dir_all(&cwd);
}

#[test]
fn witness_evidence_corrupt_artifact_fails_closed_exit_70() {
    // Karar-3: artifact yüklenemez → SYSTEM_FAILURE (resume'un artifact-yükleme
    // moduyla tutarlı); stdout boş kalır (saf-JSON sözleşmesi bozulmaz).
    let cwd = template_cwd("t9");
    let garbage = cwd.join("garbage-artifact.json");
    fs::write(&garbage, "{ not a pending authorization envelope").expect("garbage artifact");

    let output = run_witness_evidence(&cwd, Some(&garbage));
    assert_eq!(
        output.status.code(),
        Some(70),
        "corrupt artifact fails closed. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "no template JSON on system failure"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("System failure"),
        "stderr names the failure mode"
    );
    let _ = fs::remove_dir_all(&cwd);
}

#[test]
fn witness_evidence_template_with_artifact_binds_claim_and_changes_nothing() {
    // Karar-3 + tur-1 P2-5 (read-only): claim GERÇEK artifact'tan bağlanır;
    // artifact bayt-eşitliği korunur, receipt YOK, pending dizin dosya kümesi
    // değişmez — komut kalıcı hiçbir etki üretmez.
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let pending_dir = artifact.parent().expect("pending dir");
    let dir_snapshot = |d: &std::path::Path| -> std::collections::BTreeSet<String> {
        fs::read_dir(d)
            .expect("pending dir readable")
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect()
    };
    let before_bytes = fs::read(&artifact).expect("artifact bytes before");
    let before_set = dir_snapshot(pending_dir);

    let output = run_witness_evidence(fx.work_path(), Some(&artifact));
    assert_eq!(
        output.status.code(),
        Some(0),
        "template with a valid artifact succeeds. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect("pure JSON stdout");
    assert_eq!(
        v[0]["claim"].as_u64(),
        Some(claim_id),
        "claim bound to the artifact's real claim id"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("claim {claim_id}")),
        "stderr shows the suspension identity: {stderr}"
    );
    assert!(
        stderr.contains("PLACEHOLDER"),
        "stderr marks actor values as placeholders: {stderr}"
    );

    assert_eq!(
        fs::read(&artifact).expect("artifact bytes after"),
        before_bytes,
        "artifact is byte-identical (read-only)"
    );
    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt is created"
    );
    assert_eq!(
        dir_snapshot(pending_dir),
        before_set,
        "pending-authorizations file set is unchanged"
    );
    fx.assert_repo_clean();
}

#[test]
fn witness_evidence_template_alone_cannot_authorize_resume() {
    // Karar-5 integration kilidi: şablon TEK olay taşır. Fixture invariant'ı:
    // askının author'ı navigator agent 1'dir, placeholder actor 2 ≠ 1 → RED
    // DEĞİL Held: production-default Q1 (1 < 2 distinct approver) → exit 10,
    // receipt YOK. (Actor == author olsaydı sonuç exit 20 AuthorSelf olurdu —
    // Held DEĞİL; o ayrım production_resume_defective_evidence_exit_20'de
    // kanıtlı. Kayıt author taşımadığından şablon çakışmayı önleyemez.)
    let (fx, artifact, _claim_id) = suspend_and_locate_artifact();
    let output = run_witness_evidence(fx.work_path(), Some(&artifact));
    assert_eq!(output.status.code(), Some(0), "template prints");

    let template_path = fx.work_path().join("template-witnesses.json");
    fs::write(
        &template_path,
        String::from_utf8_lossy(&output.stdout).as_bytes(),
    )
    .expect("redirect template to a file");

    let resume = run_resume(&fx, &artifact, &template_path, "human");
    assert_eq!(
        resume.status.code(),
        Some(10),
        "template alone → still awaiting witnesses (not applied). stderr={}",
        String::from_utf8_lossy(&resume.stderr)
    );
    assert!(
        String::from_utf8_lossy(&resume.stdout).contains("Still awaiting witnesses"),
        "human output explains the hold"
    );
    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt on template-only evidence"
    );
    fx.assert_repo_clean();
}

#[test]
fn resume_applies_with_worked_example_witness_pair() {
    // Yardım/rehberdeki worked-example GERÇEK kanıtla maddeleştirildiğinde:
    // MergeCommit + TrailerReviewed (1.7 ≥ 1.5), iki bağımsız author-dışı
    // actor → uygular. Kanıtı fixture üretir — şablonun kendisi değil.
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();
    let events = serde_json::json!([
        { "id": 1, "source": "merge-commit-sha", "witness_kind": "MergeCommit",
          "actor": 200, "claim": claim_id },
        { "id": 2, "source": "trailer:Reviewed-by:300", "witness_kind": "TrailerReviewed",
          "actor": 300, "claim": claim_id },
    ]);
    let evidence = fx.work_path().join("witnesses-worked-example.json");
    fs::write(&evidence, serde_json::to_string_pretty(&events).unwrap())
        .expect("write worked-example evidence");

    let output = run_resume(&fx, &artifact, &evidence, "json");
    assert_eq!(
        output.status.code(),
        Some(0),
        "worked-example pair applies. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
        .expect("resume JSON envelope");
    assert_eq!(v["result"]["kind"], "applied");
    assert_eq!(v["result"]["witness"]["approvers"], 2);
    assert!(
        artifact.with_extension("json.receipt.json").exists(),
        "receipt persisted on apply"
    );
    fx.assert_repo_clean();
}

#[test]
fn resume_help_lists_witness_weight_table() {
    // Karar-2 wiring: tablo runtime üretilir VE gerçek binary'nin
    // `trajectory resume --help` çıktısında görünür (derleme başarısı yetmez).
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .args(["trajectory", "resume", "--help"])
        .output()
        .expect("render resume help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for fragment in [
        "MergeCommit",
        "PRMerged",
        "TrailerReviewed",
        "CoAuthored",
        "1.0",
        "0.8",
        "0.7",
        "0.4",
        "1.5",
        "production default",
        "witness-evidence --template",
    ] {
        assert!(
            text.contains(fragment),
            "resume --help must contain {fragment}; got:\n{text}"
        );
    }
}

#[test]
fn production_resume_stale_after_repo_change_exit_15() {
    // #164 staleness fence: askıdan sonra analyzed kapsam İÇERİĞİ değişirse
    // content digest değişir → remeasure gerekir → exit 15. Drift COMMIT'lenir
    // (kirli worktree ayrı bir fence — analyzed-scope clean; burada digest farkı
    // izolenir). Artifact'a/repo'ya dokunulmaz.
    let (fx, artifact, claim_id) = suspend_and_locate_artifact();

    // Analyzed kapsamda içerik değişikliği (b.rs'ye satır ekle) + commit → temiz
    // worktree ama farklı içerik → farklı space content digest.
    let b_rs = fx.repo_path().join("b.rs");
    let content = fs::read_to_string(&b_rs).expect("b.rs read");
    fs::write(&b_rs, format!("{content}// drift after suspension\n")).expect("b.rs drift");
    let r = fx.repo_path();
    Command::new("git")
        .arg("-C")
        .arg(r.to_str().unwrap())
        .args(["add", "--"])
        .arg("b.rs")
        .status()
        .expect("git add");
    Command::new("git")
        .arg("-C")
        .arg(r.to_str().unwrap())
        .args(["-c", "user.email=osp-test@example.invalid"])
        .args(["-c", "user.name=OSP Test"])
        .args(["commit", "-q", "-m", "drift after suspension"])
        .status()
        .expect("git commit");

    let evidence = write_witness_evidence(&fx, claim_id, &[200, 300]);
    let output = run_resume(&fx, &artifact, &evidence, "human");
    assert_eq!(
        output.status.code(),
        Some(15),
        "stale base → resume refused (remeasure). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("stale base revision"),
        "stderr explains remeasure requirement"
    );
    assert!(
        !artifact.with_extension("json.receipt.json").exists(),
        "no receipt on stale refusal"
    );
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

// ═══════════════════════════════════════════════════════════════════════════════
// #166 — canonical attempt artifact + stream separation
// ═══════════════════════════════════════════════════════════════════════════════

/// Run harness attempt with an additional `--out` flag (canonical artifact copy).
fn run_attempt_with_out(
    fx: &HarnessFixture,
    task_path: &std::path::Path,
    proposals_path: &std::path::Path,
    out_path: &std::path::Path,
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
        .arg(fx.work_path())
        .arg("--out")
        .arg(out_path)
        .output()
        .expect("run osp")
}

#[test]
fn attempt_human_mode_streams_separate_and_artifacts_persist() {
    // #166: human modunda stdout YALNIZ makine-okunur evidence dizisi; progress
    // (Task completed / Evidence entries / artifact path) stderr'de. Kanonik
    // artifact state-dir altına + --out kopyası olarak şemalı yazılır.
    // use-edges fixture: main.rs (node 2, GERÇEK coupling 2/3=0.667) +
    // RemoveImport(2→1) → 1/2=0.5 ≤ 0.55 → Completed, evidence dolu.
    let fx = HarnessFixture::new_with_use_edges();
    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let out_path = fx.work_path().join("attempt-out.json");
    let output = run_attempt_with_out(&fx, &task_path, &proposals_path, &out_path);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Akış ayrımı: stdout'ta progress YOK, yalnız JSON dizisi.
    assert!(
        !stdout.contains("Task completed") && !stdout.contains("Evidence entries"),
        "human stdout must be evidence-only (stream separation). stdout={stdout}"
    );
    let stdout_json: serde_json::Value =
        serde_json::from_str(stdout.trim_start_matches('\u{feff}').trim())
            .unwrap_or_else(|e| panic!("stdout must parse as JSON. err={e} stdout={stdout}"));
    let stdout_arr = stdout_json
        .as_array()
        .unwrap_or_else(|| panic!("stdout must be a JSON array. stdout={stdout}"));
    assert!(!stdout_arr.is_empty(), "completed run must emit evidence");
    // Progress stderr'de + artifact path bildirimi.
    assert!(
        stderr.contains("Task completed"),
        "progress moved to stderr. stderr={stderr}"
    );
    assert!(
        stderr.contains("Canonical attempt artifact"),
        "canonical artifact path announced on stderr. stderr={stderr}"
    );

    // --out kopyası: şemalı envelope (run 10 sözleşmesi — schema_version + metadata).
    let out_text = fs::read_to_string(&out_path).expect("--out artifact written");
    let out_json: serde_json::Value = serde_json::from_str(&out_text).expect("--out parses");
    assert_eq!(out_json["schema_version"], 1, "envelope schema_version=1");
    assert_eq!(out_json["result"]["kind"], "completed");
    assert_eq!(out_json["run"]["repository_head"], fx.head);
    assert_eq!(
        out_json["run"]["execution_mode"], "harness",
        "run metadata pins execution mode"
    );
    let out_evidence = out_json["evidence"].as_array().expect("evidence array");
    assert_eq!(
        out_evidence.len(),
        stdout_arr.len(),
        "--out envelope evidence matches stdout evidence"
    );

    // State-dir kalıcı kaydı: attempts/task-7-*.json (aynı şema).
    let attempts_dir = fx.work_path().join("attempts");
    let mut persisted = Vec::new();
    for entry in fs::read_dir(&attempts_dir).expect("attempts dir exists") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("task-7-") && name.ends_with(".json") {
            persisted.push(entry.path());
        }
    }
    assert_eq!(persisted.len(), 1, "exactly one persisted attempt artifact");
    let persisted_text = fs::read_to_string(&persisted[0]).expect("persisted artifact read");
    let persisted_json: serde_json::Value =
        serde_json::from_str(&persisted_text).expect("persisted parses");
    assert_eq!(persisted_json["schema_version"], 1);
    assert_eq!(persisted_json["result"]["kind"], "completed");

    fx.assert_repo_clean();
}

#[test]
fn attempt_artifact_persists_on_unsatisfied_predicate() {
    // #166: kanıt kalıcılığı exit-code'a bağlı DEĞİL — predicate tatmin
    // olamazsa bile kanonik artifact yazılır. Kanıt hattı her attempt için
    // eksiksiz. Fixture: main.rs (node 2) GERÇEK coupling 2/3=0.667 (use-edges);
    // RemoveImport(2→1) sonrası 1/2=0.5 — bar 0.1'e indirildiği için DAİMA red
    // (tatmin imkânsız).
    let fx = HarnessFixture::new_with_use_edges();
    let mut env = task_envelope(&fx.head, 2);
    env["task"]["target_predicate_set"]["predicates"][0]["predicate"]["threshold"] =
        serde_json::json!(0.1);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let out_path = fx.work_path().join("attempt-out.json");
    let output = run_attempt_with_out(&fx, &task_path, &proposals_path, &out_path);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Tatmin imkânsız → sıfır-dışı çıkış yollarından biri (maneuver-limit /
    // mock-exhausted); hangisi olduğundan bağımsız artifact yazılmış olmalı.
    assert!(
        !output.status.success(),
        "unsatisfiable predicate must yield non-zero exit. stderr={stderr}"
    );
    assert!(
        stderr.contains("Canonical attempt artifact"),
        "artifact persisted regardless of decision. stderr={stderr}"
    );
    let out_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&out_path).expect("--out written"))
            .expect("parses");
    assert_eq!(out_json["schema_version"], 1);
    assert_ne!(
        out_json["result"]["kind"], "completed",
        "predicate must remain unsatisfied in this fixture"
    );
    fx.assert_repo_clean();
}

// ═══════════════════════════════════════════════════════════════════════════════
// #166 review — P2-3 fixture-graph sanity + P1-3 zero-evidence stdout contract
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn use_edge_fixture_graph_premise_holds() {
    // #173 dersi (review P2-3): fixture'ın graf önermesi analiz çıktısından
    // assert edilir — adapter ileride değişirse (ör. `mod` kenar üretmeye
    // başlarsa) vacuous-test sınıfı kırmızıya döner, sessizce geri gelmez.
    let fx = HarnessFixture::new_with_use_edges();
    let output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.work_path())
        .arg("analyze")
        .arg(fx.repo_path())
        .arg("--format")
        .arg("json")
        .output()
        .expect("run osp analyze");
    assert!(
        output.status.success(),
        "analyze must succeed. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let space: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("analyze json parses: {e}. stdout={stdout}"));
    let nodes = space["nodes"].as_array().expect("nodes");
    let edges = space["edges"].as_array().expect("edges");
    let import_edges: Vec<&serde_json::Value> = edges
        .iter()
        .filter(|e| e["kind"].as_str() == Some("imports"))
        .collect();
    assert_eq!(
        import_edges.len(),
        2,
        "use-edges fixture must have exactly 2 import edges (got {import_edges:?})"
    );
    let main_node = nodes
        .iter()
        .find(|n| n["path"].as_str().map(|p| p.ends_with("main.rs")) == Some(true))
        .expect("main.rs node");
    let c_main = main_node["coupling"]["value"]
        .as_f64()
        .expect("coupling value");
    assert!(
        (c_main - 2.0 / 3.0).abs() < 1e-9,
        "main.rs coupling must be 2/3 (got {c_main})"
    );
}

#[test]
fn attempt_zero_evidence_human_stdout_is_empty_json_array() {
    // #166 P1-3: zero-evidence navigator sonucunda (legacy path + production
    // witness → AwaitingWitnesses, evidence 0) human stdout HER ZAMAN geçerli
    // JSON dizisidir: `[]`. Parser istisnası sınıfı yoktur. Aynı akış P1-1'i
    // pinler: non-Completed sonuçta da fence koşar + canonical artifact yazılır.
    let fx = HarnessFixture::new();
    let proposals_path = fx.write_proposals(0, 1);
    let output = fx.run_attempt_no_task(|cmd| {
        cmd.arg("99") // legacy hardcoded task id'si ile eşleşmez → TaskNotFound
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--llm")
            .arg("mock")
            .arg("--proposals")
            .arg(&proposals_path)
            .arg("--state-dir")
            .arg(fx.work_path())
    });
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON. err={e} stdout={stdout}"));
    assert!(
        parsed.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "zero-evidence stdout must be []. got: {stdout}"
    );
    // Legacy yol istenen task id'yi benimser; production witness'te
    // AwaitingWitnesses (exit 10, min_approvers_not_met) — evidence 0.
    // Zero-evidence outcome'u budur; P1-1 sayesinde fence + canonical artifact
    // BU sonuç için de çalışır (non-Completed yolu).
    assert!(
        stderr.contains("Awaiting witnesses") && stderr.contains("Evidence entries: 0"),
        "zero-evidence outcome progress on stderr. stderr={stderr}"
    );
    assert!(
        stderr.contains("Canonical attempt artifact"),
        "canonical artifact persisted for non-Completed navigator result. stderr={stderr}"
    );
    assert!(
        !output.status.success(),
        "non-Completed result must exit non-zero"
    );
    fx.assert_repo_clean();
}

// ═══════════════════════════════════════════════════════════════════════════════
// #166 review tur-2 — P0/P1-1 --out hedef fence'i + P2-1 drift negatifi
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attempt_out_inside_repo_rejected_preflight() {
    // P1-1: --out analyzed repo içinde → navigator ÇALIŞMADAN reddedilir
    // (final fence'i geçtikten sonra analyzed source'u değiştiremezdi).
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let out_inside = fx.repo_path().join("attempt-out.json");
    let output = run_attempt_with_out(&fx, &task_path, &proposals_path, &out_inside);
    assert!(
        !output.status.success(),
        "--out inside analyzed repo must be rejected preflight"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("inside the analyzed repository"),
        "stderr explains the --out repo fence: {stderr}"
    );
    fx.assert_repo_clean();
}

#[test]
fn attempt_out_targeting_canonical_store_rejected_preflight() {
    // P0: --out state-dir'in .osp/ (space identity) veya attempts/ (no-clobber
    // canonical evidence) alanına yazamaz — canonical store'un immutability'si
    // caller-controlled rename ile kırılamaz. Preflight red; state-dir kökü serbest.
    let fx = HarnessFixture::new();
    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    // #182: parent-dizin preflight'i önce koştuğu için canonical-store fence'inin
    // KENDİSİNİ görmek istiyoruz — reserved parent'ları önceden yarat (gerçek
    // attempt akışında canonical store bu dizinleri zaten yaratır).
    std::fs::create_dir_all(fx.work_path().join("attempts")).expect("mkdir attempts");
    std::fs::create_dir_all(fx.work_path().join(".osp")).expect("mkdir .osp");
    for reserved in ["attempts/task-7-999.json", ".osp/space-identity"] {
        let output = run_attempt_with_out(
            &fx,
            &task_path,
            &proposals_path,
            &fx.work_path().join(reserved),
        );
        assert!(
            !output.status.success(),
            "--out under {reserved} must be rejected preflight"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("canonical state store"),
            "stderr explains the canonical-store fence ({reserved}): {stderr}"
        );
    }
    fx.assert_repo_clean();
}

#[test]
fn attempt_drift_after_navigator_leaves_no_canonical_artifact() {
    // P2-1 (regression, review tur-1 P1-1'in negatifi): navigator SONUÇ ÜRETTİKTE
    // SONRA analyzed repo drift ederse final snapshot fence başarısız olur ve
    // diskte canonical artifact YOKTUR (--out da yazılmaz). "Persistent evidence
    // ≠ canonical validated evidence" kontratının doğrudan testi.
    //
    // Controlled drift: state-dir'e space-identity ilk kez ERKEN yazılır (engine
    // kurulumundan önce) — bu deterministik tetik; izleyici thread onu görünce
    // tracked+analyzed a.rs'yi kirletir. Fence attempt'in EN SONUNDA koşar.
    let fx = HarnessFixture::new_with_use_edges();
    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let out_path = fx.work_path().join("attempt-out.json");
    let identity_marker = fx.work_path().join(".osp").join("space-identity");
    let repo_a = fx.repo_path().join("a.rs");
    let watcher = std::thread::spawn(move || {
        for _ in 0..25_000 {
            if identity_marker.exists() {
                // Drift: analyzed-scope kirliliği — post-fence bunu yakalar.
                let _ = std::fs::write(&repo_a, "pub struct A;\n// drift\n");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    let output = run_attempt_with_out(&fx, &task_path, &proposals_path, &out_path);
    watcher.join().expect("watcher thread");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "post-attempt drift must fail the attempt. stderr={stderr}"
    );
    assert!(
        stderr.contains("modified or untracked (after attempt)"),
        "post-fence fence message. stderr={stderr}"
    );
    assert!(
        !stderr.contains("Canonical attempt artifact"),
        "NO canonical artifact announcement on fence failure. stderr={stderr}"
    );

    // Disk gerçeği: attempts/ altında artifact yok; --out yok.
    let attempts_dir = fx.work_path().join("attempts");
    let artifacts: Vec<_> = match std::fs::read_dir(&attempts_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("task-"))
            .collect(),
        Err(_) => Vec::new(),
    };
    assert!(
        artifacts.is_empty(),
        "fence-failed attempt must leave NO canonical artifacts: {artifacts:?}"
    );
    assert!(
        !out_path.exists(),
        "--out copy must not be written on fence failure"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// #204 (tur-4 P0) — submodule'li declared-dirty zinciri: attempt → resume → gate
// ═══════════════════════════════════════════════════════════════════════════════

/// file:// submodule init'i (git 2.38+ protokol kısıtı) için ortam —
/// CAĞIRANDAN gelir; araç kendi güvenlik varsayınını sessizce gevşETMEZ
/// (init başarısızlığı bilinçli, fail-closed bir RED'dir).
fn file_protocol_env(cmd: &mut Command) -> &mut Command {
    cmd.env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
        .env("GIT_CONFIG_VALUE_0", "always")
}

fn read_json_value(path: &std::path::Path) -> serde_json::Value {
    let raw = fs::read_to_string(path).expect("read json");
    serde_json::from_str(&raw).expect("parse json")
}

/// **Tur-4 P0 merge-blocker regresyonu:** submodule içeren repoda opt-in
/// attempt (kirli-beyanlı) → resume (AYNI kirli worktree) → realization-gate.
/// Gate'in #203 m.5b digest-parity fence'i, before-analyze'ını KENDİ
/// worktree'inde (paylaşılan hazırlıkla submodule init'li) koşar: gate'in
/// ölçümü attempt'in detached ölçümüyle eşdeğer DEĞİLSE bu zincir RED'lerdi.
/// `RealizedCompleted` = submodule-aware ölçümün uçtan-uca eşdeğerliğinin
/// canlı kanıtı. Attempt ayrıca nested kapsamın korunduğunu dolaylı kanıtlar
/// (recursive init olmadan detached analiz nested node'u görmezdi; buradaki
/// scope main.rs olsa da digest-parity tüm uzayı bağlar).
#[test]
fn submodule_declared_dirty_chain_attempt_resume_gate() {
    let fx = SubmoduleFixture::new();
    let parent = &fx.parent;
    let work = parent.work_path().to_path_buf();

    // Kirli yenileme durumu: ölçülen main.rs modify + yeni untracked dosya.
    let main = fs::read_to_string(parent.repo_path().join("main.rs")).unwrap();
    fs::write(
        parent.repo_path().join("main.rs"),
        format!("{main}// renewal edit\n"),
    )
    .unwrap();
    fs::create_dir_all(parent.repo_path().join("src2")).unwrap();
    fs::write(parent.repo_path().join("src2/new.rs"), "pub struct New;\n").unwrap();

    // Task: main.rs, path-kapsamlı v2 (gate predicate-scope sözleşmesi),
    // AddNode+ModifyEntity (op-matrix).
    let mut task_env = task_envelope_v2(&parent.head, "main.rs");
    task_env["task"]["allowed_operations"] = serde_json::json!(["AddNode", "ModifyEntity"]);
    let task_path = parent.write_task(&task_env);

    // v3 proposals: new_node src2/new.rs + modified main.rs (beyan evreni).
    let proposals = serde_json::json!({
        "schema_version": 3,
        "repository_head": parent.head,
        "proposals": [{
            "new_nodes": [{"kind": "Module", "initial_mass": 5.0, "path": "src2/new.rs"}],
            "new_edges": [],
            "removed_edges": [],
            "affected_nodes": ["main.rs"],
            "modified_entities": [{"path": "main.rs"}],
            "reasoning": "submodule repo honest renewal"
        }]
    });
    let proposals_path = work.join("proposals-204-sub.json");
    fs::write(
        &proposals_path,
        serde_json::to_string_pretty(&proposals).unwrap(),
    )
    .unwrap();

    // 1) Opt-in askı (production witness, kirli-beyanlı ağaç) → exit 10.
    let run_dir = work.join("run-204-submodule");
    fs::create_dir_all(&run_dir).unwrap();
    let attempt_path = run_dir.join("attempt.json");
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(parent.repo_path())
        .args(["--execution-mode", "production"])
        .args(["--witness", "production"])
        .args(["--llm", "mock"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg(&work)
        .arg("--out")
        .arg(&attempt_path)
        .arg("--allow-declared-dirty")
        .output()
        .expect("run osp attempt");
    assert_eq!(
        output.status.code(),
        Some(10),
        "declared-dirty suspension on the submodule fixture. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let attempt = read_json_value(&attempt_path);
    assert_eq!(
        attempt["result"]["fence"]["mode"], "declared_dirty",
        "{attempt}"
    );
    assert_eq!(
        attempt["run"]["repository_head"].as_str(),
        Some(parent.head.as_str()),
        "{attempt}"
    );
    let claim_id = attempt["result"]["pending"]["claim_id"].as_u64().unwrap();

    // 2) Resume — AYNI kirli worktree üzerinde (politika originating zarftan).
    let pending_dir = work.join(".osp").join("pending-authorizations");
    let mut artifacts: Vec<std::path::PathBuf> = fs::read_dir(&pending_dir)
        .expect("pending-authorizations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().map(|e| e == "json").unwrap_or(false)
                && !p
                    .file_name()
                    .map(|n| n.to_string_lossy().contains("receipt"))
                    .unwrap_or(false)
        })
        .collect();
    artifacts.sort();
    assert_eq!(artifacts.len(), 1, "tam bir askı: {artifacts:?}");
    let artifact = artifacts.pop().unwrap();

    let events: Vec<serde_json::Value> = [200u64, 300u64]
        .iter()
        .enumerate()
        .map(|(i, actor)| {
            serde_json::json!({
                "id": (i + 1) as u64,
                "source": format!("sub-chain-{i}"),
                "witness_kind": "MergeCommit",
                "actor": actor,
                "claim": claim_id,
            })
        })
        .collect();
    let evidence = work.join("witnesses-204-sub.json");
    fs::write(&evidence, serde_json::to_string_pretty(&events).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("trajectory")
        .arg("resume")
        .arg(&artifact)
        .arg("--repo")
        .arg(parent.repo_path())
        .arg("--witness-evidence")
        .arg(&evidence)
        .arg("--state-dir")
        .arg(&work)
        .args(["--format", "json"])
        .args(["--evidence-trust", "operator-asserted"])
        .output()
        .expect("run osp resume");
    assert_eq!(
        output.status.code(),
        Some(0),
        "declared-dirty resume on the SAME dirty submodule worktree. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // 3) Gate — paylaşılan hazırlık (submodule init + pinned gitlink doğrulama)
    //    olmadan before-analyze digest'i pending basisinden AYRILIRDI → RED.
    fs::copy(&task_path, run_dir.join("task.json")).unwrap();
    fs::write(
        run_dir.join("applied.patch"),
        b"diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1,3 +1,4 @@\n+// gate realization candidate\n mod a;\n mod b;\n pub fn main() { a::a(); b::b(); }\n",
    )
    .unwrap();
    let gate_evidence = run_dir.join("declared-realization.json");
    fs::write(
        &gate_evidence,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":{"passed":1,"failed":0,"skipped":0}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("realization-gate")
        .arg("--repo")
        .arg(parent.repo_path())
        .arg(&run_dir)
        .arg("--state-dir")
        .arg(&work)
        .arg("--evidence")
        .arg(&gate_evidence)
        .output()
        .expect("run osp realization-gate");
    let gate_stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "gate must realize on the submodule fixture (digest parity holds). stderr={gate_stderr}"
    );
    let verdict = read_json_value(&run_dir.join("realization-verdict.json"));
    assert!(
        verdict["verdict"]["RealizedCompleted"].is_object(),
        "{verdict}"
    );
    assert_eq!(
        verdict["binding"]["completion_route"], "witnessed_resume",
        "{verdict}"
    );
}

/// **#210 (N1) — run-27 yanlış-RED'inin tersyüz edilmiş kanıtı:** çağırıcı
/// worktree'sinde submodule İNİT'SİZKEN (run-27'in nexus-core base'i gibi;
/// gitlink duruyor, ağaç TEMİZ) STRICT attempt → resume → realization-gate
/// zinciri. N1 öncesi: strict attempt canlı ölçerdi (submodule'siz kapsam →
/// pending tabanı `32e8…` sınıfı), gate kanonik ölçerdi (`a3b5…` sınıfı) →
/// #203 m.5b digest-parite YANLIŞ RED. N1 sonrası: her iki uç da kanonik
/// ağacı ölçer → parite tutar → `RealizedCompleted`.
#[test]
fn uninitialized_submodule_strict_chain_attempt_resume_gate() {
    let fx = SubmoduleFixture::new();
    let parent = &fx.parent;
    let work = parent.work_path().to_path_buf();

    // Çağırıcı worktree: submodule init'siz (deinit) — içerik yok, status temiz.
    let st = Command::new("git")
        .args(["-C", parent.repo_path().to_str().unwrap()])
        .args(["submodule", "deinit", "--force", "clients/fe"])
        .status()
        .expect("git submodule deinit");
    assert!(st.success(), "submodule deinit failed");
    assert!(
        !fx.sub_nested.exists(),
        "uninitialized caller worktree (the run-27 condition)"
    );

    // Task: main.rs path-kapsamlı v2 (gate predicate-scope sözleşmesi),
    // AddNode+ModifyEntity (op-matrix — #204 zinciriyle aynı şekil).
    let mut task_env = task_envelope_v2(&parent.head, "main.rs");
    task_env["task"]["allowed_operations"] = serde_json::json!(["AddNode", "ModifyEntity"]);
    let task_path = parent.write_task(&task_env);

    // v3 proposals — new_node HİPOTETİK (diskte dosya YOK; strict ağaç temiz
    // kalır; aday içeriği yalnız applied.patch'te gerçekleşir).
    let proposals = serde_json::json!({
        "schema_version": 3,
        "repository_head": parent.head,
        "proposals": [{
            "new_nodes": [{"kind": "Module", "initial_mass": 5.0, "path": "src2/new.rs"}],
            "new_edges": [],
            "removed_edges": [],
            "affected_nodes": ["main.rs"],
            "modified_entities": [{"path": "main.rs"}],
            "reasoning": "strict canonical measurement chain"
        }]
    });
    let proposals_path = work.join("proposals-210-strict.json");
    fs::write(
        &proposals_path,
        serde_json::to_string_pretty(&proposals).unwrap(),
    )
    .unwrap();

    // 1) STRICT askı (production witness, temiz ağaç, submodule init'siz) →
    //    ölçüm kanonik (detached @ HEAD, submodule init'li) → exit 10.
    let run_dir = work.join("run-210-strict");
    fs::create_dir_all(&run_dir).unwrap();
    let attempt_path = run_dir.join("attempt.json");
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(parent.repo_path())
        .args(["--execution-mode", "production"])
        .args(["--witness", "production"])
        .args(["--llm", "mock"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg(&work)
        .arg("--out")
        .arg(&attempt_path)
        .output()
        .expect("run osp attempt");
    assert_eq!(
        output.status.code(),
        Some(10),
        "strict suspension must suspend (not a scope/parity failure) on the \
         uninitialized-submodule fixture. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let attempt = read_json_value(&attempt_path);
    let claim_id = attempt["result"]["pending"]["claim_id"]
        .as_u64()
        .expect("pending claim id");

    // 2) Resume — strict kol da kanonik ölçer (N1) → basis paritesi tutar;
    //    2 bağımsız tanık → Applied.
    let pending_dir = work.join(".osp").join("pending-authorizations");
    let mut artifacts: Vec<std::path::PathBuf> = fs::read_dir(&pending_dir)
        .expect("pending-authorizations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().map(|e| e == "json").unwrap_or(false)
                && !p
                    .file_name()
                    .map(|n| n.to_string_lossy().contains("receipt"))
                    .unwrap_or(false)
        })
        .collect();
    artifacts.sort();
    assert_eq!(artifacts.len(), 1, "tam bir askı: {artifacts:?}");
    let artifact = artifacts.pop().unwrap();

    let events: Vec<serde_json::Value> = [200u64, 300u64]
        .iter()
        .enumerate()
        .map(|(i, actor)| {
            serde_json::json!({
                "id": (i + 1) as u64,
                "source": format!("strict-chain-{i}"),
                "witness_kind": "MergeCommit",
                "actor": actor,
                "claim": claim_id,
            })
        })
        .collect();
    let evidence = work.join("witnesses-210-strict.json");
    fs::write(&evidence, serde_json::to_string_pretty(&events).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("trajectory")
        .arg("resume")
        .arg(&artifact)
        .arg("--repo")
        .arg(parent.repo_path())
        .arg("--witness-evidence")
        .arg(&evidence)
        .arg("--state-dir")
        .arg(&work)
        .args(["--format", "json"])
        .args(["--evidence-trust", "operator-asserted"])
        .output()
        .expect("run osp resume");
    assert_eq!(
        output.status.code(),
        Some(0),
        "strict resume must apply (canonical basis parity holds). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // 3) Gate — before-analyze kanonik; pending tabanı da kanonik → m.5b
    //    digest-parity TUTAR (N1 öncesi burası yanlış RED'liyordu).
    fs::copy(&task_path, run_dir.join("task.json")).unwrap();
    fs::write(
        run_dir.join("applied.patch"),
        b"diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1,3 +1,4 @@\n+// gate realization candidate\n mod a;\n mod b;\n pub fn main() { a::a(); b::b(); }\n",
    )
    .unwrap();
    let gate_evidence = run_dir.join("declared-realization.json");
    fs::write(
        &gate_evidence,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":{"passed":1,"failed":0,"skipped":0}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("realization-gate")
        .arg("--repo")
        .arg(parent.repo_path())
        .arg(&run_dir)
        .arg("--state-dir")
        .arg(&work)
        .arg("--evidence")
        .arg(&gate_evidence)
        .output()
        .expect("run osp realization-gate");
    let gate_stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "gate must realize with digest parity DESPITE the uninitialized caller \
         submodule (the #210 wrong-RED, inverted). stderr={gate_stderr}"
    );
    let verdict = read_json_value(&run_dir.join("realization-verdict.json"));
    assert!(
        verdict["verdict"]["RealizedCompleted"].is_object(),
        "{verdict}"
    );
    assert_eq!(
        verdict["binding"]["completion_route"], "witnessed_resume",
        "{verdict}"
    );
    parent.assert_repo_clean();
}

/// #215 review P1 (süreç-düzeyi): post-checkout hook'u kanonik checkout'u
/// kirletirse attempt FAIL-CLOSED döner — kalıcı zarf/attempt artifact
/// oluşmaz (ölçüm, bağlandığı commit'in içeriği olmadan üretilmemiş sayılır).
#[test]
fn post_checkout_hook_attempt_fails_closed_without_artifact() {
    let fx = HarnessFixture::new();
    // Hook'U fixture kurulduktan SONRA yaz (fixture'ın kendi commit'leri
    // etkilenmesin); hook yalnız `git worktree add` checkout'unda koşar.
    let hooks = fx.repo_path().join(".git").join("hooks");
    fs::create_dir_all(&hooks).expect("hooks dir");
    let hook = hooks.join("post-checkout");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf '// injected by hook\\n' >> main.rs\n",
    )
    .expect("write hook");
    // Linux'ta git, çalıştırma biti OLMAYAN hook'u atlar (Windows'ta atlamaz) —
    // yerel/CI ayrışmasını kapatmak için POSIX tarafında +x zorlanır.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
            .expect("chmod +x hook");
    }

    let env = task_envelope(&fx.head, 2);
    let task_path = fx.write_task(&env);
    let proposals_path = fx.write_proposals(2, 1);
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    cmd.current_dir(fx.work_path())
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(fx.repo_path())
        .args(["--execution-mode", "harness"])
        .args(["--witness", "harness-auto-approve"])
        .args(["--llm", "mock"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg(fx.work_path());
    let output = cmd.output().expect("run osp attempt");
    assert!(
        !output.status.success(),
        "dirty canonical checkout must fail the attempt"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("canonical checkout is dirty") && stderr.contains("post-checkout hook"),
        "RED names the cleanliness failure and its likely cause: {stderr}"
    );
    assert!(
        !fx.work_path().join("attempts").exists(),
        "no canonical attempt artifact on a refused measurement"
    );
    assert!(
        !fx.work_path().join(".osp").exists(),
        "no .osp/ state on a refused measurement"
    );
    fx.assert_repo_clean();
}

/// #215 review P2 (yaşam-döngüsü matrisi): kanonik ölçümün iddiası caller'ın
/// init durumundan BAĞIMSIZLIKTIR — attempt ile resume arasında submodule init
/// durumu değişse bile resume aynı kanonik digest'i ölçer (pending tabanıyla
/// parite → Applied). Gate kendi worktree'ini açar (caller'ı hiç görmez);
/// matris hücreleri resume düzeyinde kanıtlar. Mevcut
/// `uninitialized_submodule_strict_chain_*` testi (init'siz,init'siz) hücresini
/// kapatır; bu ikisi kalan hücreler.
fn strict_resume_applies_across_submodule_state_change(
    init_at_attempt: bool,
    init_at_resume: bool,
) {
    assert_ne!(
        init_at_attempt, init_at_resume,
        "matris köşegeni başka testte"
    );
    let fx = SubmoduleFixture::new();
    let parent = &fx.parent;
    let work = parent.work_path().to_path_buf();

    let set_submodule_state = |initialized: bool| {
        let mut cmd = Command::new("git");
        cmd.args(["-C", parent.repo_path().to_str().unwrap()]);
        if initialized {
            cmd.args(["submodule", "update", "--init", "clients/fe"]);
            file_protocol_env(&mut cmd);
        } else {
            cmd.args(["submodule", "deinit", "--force", "clients/fe"]);
        }
        let st = cmd.status().expect("toggle submodule state");
        assert!(st.success(), "submodule toggle failed");
        assert_eq!(
            fx.sub_nested.exists(),
            initialized,
            "caller worktree submodule state matches the requested phase"
        );
    };

    set_submodule_state(init_at_attempt);
    let mut task_env = task_envelope_v2(&parent.head, "main.rs");
    task_env["task"]["allowed_operations"] = serde_json::json!(["AddNode", "ModifyEntity"]);
    let task_path = parent.write_task(&task_env);
    let proposals = serde_json::json!({
        "schema_version": 3,
        "repository_head": parent.head,
        "proposals": [{
            "new_nodes": [{"kind": "Module", "initial_mass": 5.0, "path": "src2/new.rs"}],
            "new_edges": [],
            "removed_edges": [],
            "affected_nodes": ["main.rs"],
            "modified_entities": [{"path": "main.rs"}],
            "reasoning": "lifecycle matrix cell"
        }]
    });
    let proposals_path = work.join("proposals-215-matrix.json");
    fs::write(
        &proposals_path,
        serde_json::to_string_pretty(&proposals).unwrap(),
    )
    .unwrap();

    // 1) Strict askı (production witness) — init durumu init_at_attempt.
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(parent.repo_path())
        .args(["--execution-mode", "production"])
        .args(["--witness", "production"])
        .args(["--llm", "mock"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--state-dir")
        .arg(&work)
        .output()
        .expect("run osp attempt");
    assert_eq!(
        output.status.code(),
        Some(10),
        "strict suspension (phase A init={init_at_attempt}). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // 2) Askının claim'idini --out zarfından al (pending kimliği).
    let pending_dir = work.join(".osp").join("pending-authorizations");
    let mut artifacts: Vec<std::path::PathBuf> = fs::read_dir(&pending_dir)
        .expect("pending-authorizations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().map(|e| e == "json").unwrap_or(false)
                && !p
                    .file_name()
                    .map(|n| n.to_string_lossy().contains("receipt"))
                    .unwrap_or(false)
        })
        .collect();
    artifacts.sort();
    assert_eq!(artifacts.len(), 1, "tam bir askı: {artifacts:?}");
    let artifact = artifacts.pop().unwrap();
    let pending = read_json_value(&artifact);
    let claim_id = pending["record"]["claim_id"].as_u64().expect("claim id");

    // 3) Aşamalar ARASINDA init durumunu ÇEVİR — kanonik ölçüm bunu görmemeli.
    set_submodule_state(init_at_resume);

    // 4) Resume — kanonik digest değişmedi → basis paritesi → Applied.
    let events: Vec<serde_json::Value> = [200u64, 300u64]
        .iter()
        .enumerate()
        .map(|(i, actor)| {
            serde_json::json!({
                "id": (i + 1) as u64,
                "source": format!("matrix-{i}"),
                "witness_kind": "MergeCommit",
                "actor": actor,
                "claim": claim_id,
            })
        })
        .collect();
    let evidence = work.join("witnesses-215-matrix.json");
    fs::write(&evidence, serde_json::to_string_pretty(&events).unwrap()).unwrap();
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    file_protocol_env(&mut cmd);
    let output = cmd
        .current_dir(&work)
        .arg("trajectory")
        .arg("resume")
        .arg(&artifact)
        .arg("--repo")
        .arg(parent.repo_path())
        .arg("--witness-evidence")
        .arg(&evidence)
        .arg("--state-dir")
        .arg(&work)
        .args(["--format", "json"])
        .args(["--evidence-trust", "operator-asserted"])
        .output()
        .expect("run osp resume");
    assert_eq!(
        output.status.code(),
        Some(0),
        "resume must apply despite the init-state flip between phases \
         (canonical digest is caller-init independent). stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    parent.assert_repo_clean();
}

#[test]
fn resume_applies_after_deinit_between_attempt_and_resume() {
    strict_resume_applies_across_submodule_state_change(true, false);
}

#[test]
fn resume_applies_after_init_between_attempt_and_resume() {
    strict_resume_applies_across_submodule_state_change(false, true);
}
