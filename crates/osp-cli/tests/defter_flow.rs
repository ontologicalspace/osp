//! #172 integration — defter komutları (draft-task / suggest-targets / finalize-run).
//!
//! osp-cli bin-only crate olduğu için tüm testler gerçek `osp` binary'sini çağırır
//! (assert_cmd pattern). `HarnessFixture::new_with_use_edges()` ile gerçek use-kenar
//! grafı kurulur ve önerme ÖLÇEREK pinlenir (#173 deseni: 2 imports kenarı, main.rs
//! coupling 2/3 — vacuous pass yasak). Ardından tam ritüel uçtan uca koşar:
//! analyze → draft-task → attempt (--out) → after → finalize-run.

#![cfg(test)]

mod common;

use common::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;

fn osp_in(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    cmd.current_dir(cwd);
    cmd
}

fn read_json(path: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw).unwrap()
}

/// Bağımsız sha256 yeniden hesabı (finalize-run'ın kendi önermesinin dışından).
fn sha256_of(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(fs::read(path).unwrap());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("sha256:{hex}")
}

fn p_str(p: &Path) -> &str {
    p.to_str().expect("utf-8 path")
}

/// Ritüel adımı 1: baseline ölçümü + ÖNERME PİNİ (ölçerek).
///
/// Tur-1 P0-1: baseline `--require-clean-snapshot` ile üretilir — defter
/// komutları yalnız `clean_pre_post_equal` (revizyona bağlı) artifact kabul eder.
/// Dönen değer: (baseline_path, ölçülen main.rs coupling'i).
fn measured_baseline(fx: &HarnessFixture) -> (PathBuf, f64) {
    let work = fx.work_path();
    let baseline = work.join("baseline.json");
    let out = osp_in(work)
        .arg("analyze")
        .arg(fx.repo_path())
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&baseline)
        .arg("--require-clean-snapshot")
        .output()
        .expect("run osp analyze");
    assert!(
        out.status.success(),
        "analyze must succeed. stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let b = read_json(&baseline);
    assert_eq!(
        b["repository"]["binding"], "clean_pre_post_equal",
        "ritual baseline must be revision-bound"
    );
    let imports: Vec<_> = b["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .filter(|e| e["kind"] == "imports")
        .collect();
    assert_eq!(imports.len(), 2, "#173 premise: exactly 2 imports edges");
    let main_node = b["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|n| n["path"] == "main.rs")
        .expect("main.rs node");
    let c = main_node["coupling"]["value"].as_f64().expect("coupling");
    assert!(
        (c - 2.0 / 3.0).abs() < 1e-9,
        "measured premise 2/3, got {c}"
    );
    (baseline, c)
}

fn write_spec(work: &Path, from: &str, to: &str) -> PathBuf {
    let spec = work.join("spec.json");
    fs::write(
        &spec,
        format!(
            r#"{{"proposals": [{{"removed_edges": [{{"from": "{from}", "to": "{to}"}}], "reasoning": "test spec"}}]}}"#
        ),
    )
    .unwrap();
    spec
}

/// draft-task çağrısını kur (tekrarlayan argv'yi tek yerde tutar).
fn draft_task_cmd(
    fx: &HarnessFixture,
    work: &Path,
    baseline: &Path,
    target: &str,
    spec: Option<&Path>,
) -> Command {
    let mut cmd = osp_in(work);
    cmd.arg("draft-task")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--target")
        .arg(target)
        .arg("--task-id")
        .arg("1")
        .arg("--label")
        .arg("test label")
        .arg("--bar")
        .arg("0.55")
        .arg("--baseline")
        .arg(baseline);
    if let Some(spec) = spec {
        cmd.arg("--proposals-spec").arg(spec);
    }
    cmd
}

#[test]
fn draft_task_roundtrip_produces_validated_task_and_proposals() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _c) = measured_baseline(&fx);

    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = work.join("task.json");
    let props = work.join("proposals.json");
    let out = osp_in(&work)
        .arg("draft-task")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--target")
        .arg("main.rs")
        .arg("--task-id")
        .arg("1")
        .arg("--label")
        .arg("cut main.rs unused import")
        .arg("--bar")
        .arg("0.55")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--proposals-spec")
        .arg(&spec)
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(&props)
        .output()
        .expect("run osp draft-task");
    assert!(
        out.status.success(),
        "draft-task must succeed. stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("delegates verified against measured baseline"),
        "summary on stderr: {stderr}"
    );

    // Task v2: SHA transcription YOK — repository_head motorun ölçtüğü HEAD.
    let t = read_json(&task);
    assert_eq!(t["schema_version"], 2);
    assert_eq!(t["repository_head"], fx.head);
    assert_eq!(t["scope_bindings"][0]["path"], "main.rs");
    let pred = &t["task"]["target_predicate_set"]["predicates"][0]["predicate"];
    assert_eq!(pred["metric"], "Coupling");
    assert_eq!(pred["operator"], "Le");
    assert_eq!(pred["threshold"], 0.55);
    assert_eq!(pred["scope"], serde_json::json!({"Path": "main.rs"}));
    assert_eq!(pred["required_source"], "TreeSitter");
    assert_eq!(pred["tolerance"], 0.0);
    assert_eq!(
        t["task"]["allowed_operations"],
        serde_json::json!(["AddNode", "RemoveImport"])
    );
    assert_eq!(t["task"]["status"], "Pending");
    assert_eq!(t["task"]["policy"]["maneuver_limit"], 3);

    // Proposals v2: tam şema; affected_nodes uçlardan türetildi (sıralı-özgün).
    let p = read_json(&props);
    assert_eq!(p["schema_version"], 2);
    assert_eq!(p["repository_head"], fx.head);
    assert_eq!(
        p["proposals"][0]["removed_edges"][0],
        serde_json::json!({"from": "main.rs", "to": "a.rs", "kind": "Imports"})
    );
    assert_eq!(
        p["proposals"][0]["affected_nodes"],
        serde_json::json!(["a.rs", "main.rs"])
    );
}

#[test]
fn draft_task_fail_closed_unknown_target_and_unmeasured_edge() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let task = work.join("task.json");

    // (a) hedef ölçülmüş düğüm değil → red (el beyanı değil).
    let out = osp_in(&work)
        .arg("draft-task")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--target")
        .arg("nope.rs")
        .arg("--task-id")
        .arg("1")
        .arg("--label")
        .arg("x")
        .arg("--bar")
        .arg("0.55")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out-task")
        .arg(&task)
        .output()
        .expect("run osp draft-task (unknown target)");
    assert!(!out.status.success(), "unknown target must fail closed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not a measured baseline node"), "{stderr}");

    // (b) typo'lu temsilci kenar (run-14/16 sınıfı) → red + ölçülmüş kenar listesi.
    let spec = write_spec(&work, "main.rs", "ay.rs");
    let out = osp_in(&work)
        .arg("draft-task")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--target")
        .arg("main.rs")
        .arg("--task-id")
        .arg("1")
        .arg("--label")
        .arg("x")
        .arg("--bar")
        .arg("0.55")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--proposals-spec")
        .arg(&spec)
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(work.join("proposals.json"))
        .output()
        .expect("run osp draft-task (unmeasured edge)");
    assert!(!out.status.success(), "unmeasured edge must fail closed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("NOT measured"), "{stderr}");
    assert!(stderr.contains("-> a.rs (imports)"), "{stderr}");
    assert!(stderr.contains("-> b.rs (imports)"), "{stderr}");
}

#[test]
fn draft_task_baseline_head_drift_fails_closed() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    // HEAD'i ilerlet (artifact eskir).
    fs::write(fx.repo_path().join("b.rs"), "pub struct B; // second\n").unwrap();
    let commit = Command::new("git")
        .args(["-C", p_str(fx.repo_path()), "commit", "-aqm", "drift"])
        .output()
        .expect("git commit");
    assert!(commit.status.success());

    let out = osp_in(&work)
        .arg("draft-task")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--target")
        .arg("main.rs")
        .arg("--task-id")
        .arg("1")
        .arg("--label")
        .arg("x")
        .arg("--bar")
        .arg("0.55")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out-task")
        .arg(work.join("task.json"))
        .output()
        .expect("run osp draft-task (drift)");
    assert!(!out.status.success(), "baseline drift must fail closed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("HEAD drift"), "{stderr}");
}

#[test]
fn suggest_targets_measured_ranking_and_exclude_past() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    let out = osp_in(&work)
        .arg("suggest-targets")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--baseline")
        .arg(&baseline)
        .arg("--format")
        .arg("json")
        .output()
        .expect("run osp suggest-targets");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).expect("json stdout");
    assert_eq!(v["repository_head"], fx.head);
    let candidates = v["candidates"].as_array().expect("candidates");
    let first = &candidates[0];
    assert_eq!(first["path"], "main.rs", "2/3 outranks 0");
    let c = first["coupling"].as_f64().unwrap();
    assert!((c - 2.0 / 3.0).abs() < 1e-9, "measured ranking: {c}");
    assert_eq!(first["imports_out"], 2);
    assert_eq!(first["rank"], 1);

    // --exclude-past: v2 task ref'i ledger'dan okunur, hedef tablodan düşer (K6).
    let past_task = work.join("past-task.json");
    fs::write(
        &past_task,
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {}
        })
        .to_string(),
    )
    .unwrap();
    let ledger = work.join("ledger.jsonl");
    fs::write(
        &ledger,
        format!(
            "{{\"task_ref\": {}}}\n",
            serde_json::json!(p_str(&past_task))
        ),
    )
    .unwrap();
    let out = osp_in(&work)
        .arg("suggest-targets")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--baseline")
        .arg(&baseline)
        .arg("--exclude-past")
        .arg(&ledger)
        .arg("--format")
        .arg("json")
        .output()
        .expect("run osp suggest-targets (exclude-past)");
    assert!(out.status.success());
    let v: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    let paths: Vec<&str> = v["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["path"].as_str())
        .collect();
    assert!(
        !paths.contains(&"main.rs"),
        "past target excluded: {paths:?}"
    );
    assert_eq!(v["past_targets_excluded"], serde_json::json!(["main.rs"]));
}

#[test]
fn finalize_run_full_ritual_emits_machine_complete_ledger_row() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let run = work.join("run");
    fs::create_dir_all(&run).unwrap();
    let baseline = run.join("baseline.json");

    // Ritüel adımı 1: baseline (ölçüm pini measured_baseline'te değil burada da
    // gerekiyor — attempt fence'leri aynı önermeye yaslanır). P0-1: revizyona bağlı.
    let out = osp_in(&work)
        .arg("analyze")
        .arg(fx.repo_path())
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&baseline)
        .arg("--require-clean-snapshot")
        .output()
        .unwrap();
    assert!(out.status.success());

    // Adım 2: draft-task (task + proposals aynı HEAD'e bağlı).
    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = run.join("task.json");
    let props = run.join("proposals.json");
    let out = osp_in(&work)
        .arg("draft-task")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--target")
        .arg("main.rs")
        .arg("--task-id")
        .arg("1")
        .arg("--label")
        .arg("cut main.rs unused import")
        .arg("--bar")
        .arg("0.55")
        .arg("--baseline")
        .arg(&baseline)
        .arg("--proposals-spec")
        .arg(&spec)
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(&props)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Adım 3: attempt — kanonik artifact'ın --out kopyası run dizinine (#166).
    let attempt = run.join("attempt.json");
    let out = fx.run_attempt_no_task(|cmd| {
        cmd.arg("1")
            .arg("--repo")
            .arg(fx.repo_path())
            .arg("--execution-mode")
            .arg("harness")
            .arg("--witness")
            .arg("harness-auto-approve")
            .arg("--llm")
            .arg("mock")
            .arg("--proposals")
            .arg(&props)
            .arg("--task")
            .arg(&task)
            .arg("--state-dir")
            .arg(fx.work_path())
            .arg("--out")
            .arg(&attempt)
            .arg("--format")
            .arg("json")
    });
    assert!(
        out.status.success(),
        "attempt must complete. stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(attempt.is_file(), "--out copy exists in run dir");

    // Adım 4: promote benzeri HEAD ilerlemesi + after ölçümü + patch.
    fs::write(fx.repo_path().join("b.rs"), "pub struct B; // after\n").unwrap();
    let commit = Command::new("git")
        .args(["-C", p_str(fx.repo_path()), "commit", "-aqm", "after"])
        .output()
        .unwrap();
    assert!(commit.status.success());
    let after = run.join("after.json");
    let out = osp_in(&work)
        .arg("analyze")
        .arg(fx.repo_path())
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&after)
        .arg("--require-clean-snapshot")
        .output()
        .unwrap();
    assert!(out.status.success());
    let patch = run.join("applied.patch");
    fs::write(&patch, b"diff --git a/main.rs b/main.rs\n-test\n").unwrap();

    // Adım 5: finalize-run (ritüel gibi OSP kökünden görece run-dir → ref'ler görece).
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg("run")
        .arg("--repository")
        .arg("testrepo")
        .output()
        .expect("run osp finalize-run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let row: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();

    let baseline_head = read_json(&baseline)["repository"]["head"].clone();
    let after_head = read_json(&after)["repository"]["head"].clone();
    assert_eq!(row["schema_version"], "live-ledger-v1");
    assert_eq!(row["run_id"], "run");
    assert_eq!(row["contract_version"], "v1.1");
    assert_eq!(row["repository"], "testrepo");
    assert_eq!(row["repository_head"], baseline_head);
    assert_ne!(baseline_head, after_head, "fixture gerçekten promote etti");

    // K7: build-time gömülü osp_revision (tam 40-hex).
    let rev = row["osp_revision"].as_str().unwrap();
    assert_eq!(rev.len(), 40, "osp_revision: {rev}");
    assert!(rev
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));

    // K3: digest'ler bağımsız yeniden hesapla eşleşir (tam 64-hex).
    assert_eq!(row["task_digest"], sha256_of(&task));
    assert_eq!(row["proposal_digest"], sha256_of(&props));
    assert_eq!(row["patch_digest"], sha256_of(&patch));
    for field in ["task_digest", "proposal_digest", "patch_digest"] {
        let d = row[field].as_str().unwrap();
        assert!(
            d.starts_with("sha256:") && d.len() == 7 + 64,
            "{field}: {d}"
        );
    }

    assert_eq!(row["task_ref"], "run/task.json");
    assert_eq!(row["attempt_ref"], "run/attempt.json");
    assert_eq!(row["after_ref"], "run/after.json");
    assert_eq!(row["analysis_profile"], "tier1");
    // Tur-1 P1-1: Exists(after)+Exists(patch) ≠ Patch(S0)=S_after —
    // build_verification motorca DOLDURULMAZ, bütünüyle null.
    assert_eq!(row["build_verification"], serde_json::Value::Null);

    // K4 (tur-1 P1-4): yorum alanları null — null = "henüz değerlendirilmedi";
    // `[]` başka bir iddia olurdu ("değerlendirildi, friction yok").
    for field in [
        "decision",
        "patch_outcome",
        "decision_utility",
        "counterfactual",
        "human_override",
        "notes",
        "friction",
        "commands",
    ] {
        assert_eq!(
            row[field],
            serde_json::Value::Null,
            "{field} must stay null"
        );
    }
}

#[test]
fn finalize_run_rejects_legacy_hand_assembled_attempt() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-legacy");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    // Run-16 tarzı el yapımı liste (stdout'tan ayıklanmış) — K2 ile reddedilir.
    fs::write(run.join("attempt.json"), "[]").unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .expect("run osp finalize-run (legacy)");
    assert!(
        !out.status.success(),
        "legacy attempt artifact must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("#166 run envelope"), "{stderr}");
}

#[test]
fn finalize_run_head_fence_rejects_cross_state_row() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-fence");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            // Başka bir state'e bağlanmış task (transcription sınıfının motor-dışı
            // kalıntısı) — üç-head fence yakalar.
            "repository_head": "f".repeat(40),
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        serde_json::json!({
            "schema_version": 1,
            "run": {"task_id": 1, "execution_mode": "harness", "witness_mode": "harness_auto_approve",
                    "task_source": "harness_task_file", "repository_head": fx.head},
            "execution_measurement": {}, "result": {}, "evidence": []
        })
        .to_string(),
    )
    .unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .expect("run osp finalize-run (fence)");
    assert!(!out.status.success(), "cross-state row must fail closed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("head fence"), "{stderr}");
}

/// Tur-1 P0-1: generic analyze'ın `observed_worktree_unbound` artifact'ı
/// revizyona bağlı değildir — draft-task kabul etmez.
#[test]
fn draft_task_rejects_unbound_baseline() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let baseline = work.join("baseline-unbound.json");
    let out = osp_in(&work)
        .arg("analyze")
        .arg(fx.repo_path())
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&baseline)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        read_json(&baseline)["repository"]["binding"],
        "observed_worktree_unbound"
    );

    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", None)
        .arg("--out-task")
        .arg(work.join("task.json"))
        .output()
        .expect("run osp draft-task (unbound)");
    assert!(!out.status.success(), "unbound baseline must fail closed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not revision-bound"), "{stderr}");
    assert!(stderr.contains("--require-clean-snapshot"), "{stderr}");
}

/// Tur-1 P0-1: artifact ölçümünden SONRA analyzed-path dirty'leşti —
/// draft-time #155 fence'i ölçülen içeriğin artık HEAD içeriği olmadığını yakalar.
#[test]
fn draft_task_rejects_dirty_analyzed_path_at_draft_time() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    fs::write(
        fx.repo_path().join("main.rs"),
        "mod a;\nmod b;\nuse crate::a::A;\nuse crate::b::B;\npub fn main() { let _ = (A, B); } // local edit\n",
    )
    .unwrap();

    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", None)
        .arg("--out-task")
        .arg(work.join("task.json"))
        .output()
        .expect("run osp draft-task (dirty analyzed path)");
    assert!(
        !out.status.success(),
        "dirty analyzed path must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("modified or untracked"), "{stderr}");
    assert!(stderr.contains("draft-time"), "{stderr}");
}

/// Tur-1 P1-3: spec'in op-gereksinimleri task'ın izinli operasyonlarında yoksa
/// generation-time red; analyzer-owned kind'ler her yerde red.
#[test]
fn draft_task_op_matrix_and_analyzer_owned_kinds_fail_closed() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let task = work.join("task.json");
    let props = work.join("proposals.json");

    // (a) new_edges → AddEdge gerekir; default ops (AddNode, RemoveImport) yetersiz.
    let spec = work.join("spec-addedge.json");
    fs::write(
        &spec,
        r#"{"proposals": [{"new_edges": [{"from": "main.rs", "to": "a.rs"}],
                          "reasoning": "needs AddEdge"}]}"#,
    )
    .unwrap();
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(&props)
        .output()
        .expect("run osp draft-task (op matrix)");
    assert!(
        !out.status.success(),
        "op-matrix violation must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("AddEdge"), "{stderr}");
    assert!(stderr.contains("--operation"), "{stderr}");

    // (b) analyzer-owned kind — proposal mutasyonunda yasak.
    let spec = work.join("spec-owned.json");
    fs::write(
        &spec,
        r#"{"proposals": [{"removed_edges": [{"from": "main.rs", "to": "a.rs", "kind": "TypeImports"}],
                          "reasoning": "observational kind"}]}"#,
    )
    .unwrap();
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(&props)
        .output()
        .expect("run osp draft-task (analyzer-owned kind)");
    assert!(
        !out.status.success(),
        "analyzer-owned kind must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("analyzer-owned"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // (c) aynı spec --operation AddEdge ile geçer (çözüm yolunun işlediğini göster).
    let spec = work.join("spec-addedge.json");
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--operation")
        .arg("AddNode")
        .arg("--operation")
        .arg("AddEdge")
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(&props)
        .output()
        .expect("run osp draft-task (op matrix satisfied)");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Tur-1 P2: çıktılar birbirinin/kendi girdilerinin alias'ı olamaz.
#[test]
fn draft_task_output_alias_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let spec = write_spec(&work, "main.rs", "a.rs");
    let same = work.join("same.json");

    // (a) out-task == out-proposals.
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(&same)
        .arg("--out-proposals")
        .arg(&same)
        .output()
        .expect("run osp draft-task (alias outputs)");
    assert!(!out.status.success(), "output alias must fail closed");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("same file"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // (b) out-task == girdi baseline.
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(&baseline)
        .arg("--out-proposals")
        .arg(work.join("proposals.json"))
        .output()
        .expect("run osp draft-task (output overwrites input)");
    assert!(!out.status.success(), "output-over-input must fail closed");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--baseline input"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Tam-şekil #166 attempt envelope'u (negatif testler için).
fn attempt_envelope(head: &str, task_id: u64) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "run": {
            "task_id": task_id,
            "execution_mode": "harness",
            "witness_mode": "harness_auto_approve",
            "task_source": "harness_task_file",
            "repository_head": head
        },
        "execution_measurement": {
            "subject_authority": "task_scope",
            "provenance_authority": "engine_native_per_axis",
            "provenance_native": true
        },
        "result": {"kind": "completed", "attempts": 1},
        "evidence": []
    })
}

/// Tur-1 P0-2: yalnız head taşıyan minimal sahte obje SIKI #166 şeklinden geçemez.
#[test]
fn finalize_run_minimal_attempt_object_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-minimal");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        serde_json::json!({
            "schema_version": 1,
            "run": {"repository_head": fx.head}
        })
        .to_string(),
    )
    .unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "minimal attempt shape must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not match the #166 run envelope shape"),
        "{stderr}"
    );
}

/// Tur-1 P0-2: task.id ≠ attempt.run.task_id — yanlış artifact eşleşmesi tek
/// ledger satırında birleştirilemez.
#[test]
fn finalize_run_task_id_mismatch_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-idmismatch");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 17}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        attempt_envelope(&fx.head, 42).to_string(),
    )
    .unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(!out.status.success(), "task-id mismatch must fail closed");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("task-id fence"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Tur-1 P0-2: proposals yalnız v2 zarfı + doğru head ile bağlanabilir;
/// v1 çıplak array state'e bağlanamaz.
#[test]
fn finalize_run_proposals_state_fence() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    // (a) farklı HEAD'ten kopyalanmış proposals v2.
    let run = work.join("run-props-head");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        attempt_envelope(&fx.head, 1).to_string(),
    )
    .unwrap();
    fs::write(
        run.join("proposals.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": "e".repeat(40),
            "proposals": []
        })
        .to_string(),
    )
    .unwrap();
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "cross-state proposals must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("head fence: proposals.json"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // (b) v1 çıplak array — state'e bağlanamaz.
    let run = work.join("run-props-v1");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        attempt_envelope(&fx.head, 1).to_string(),
    )
    .unwrap();
    fs::write(run.join("proposals.json"), "[]").unwrap();
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "v1 bare-array proposals must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("bare JSON array"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Tur-1 P0-3: --out tüketilen artifact'ı overwrite edemez (ref↔digest ayrışır).
#[test]
fn finalize_run_out_alias_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-outalias");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        attempt_envelope(&fx.head, 1).to_string(),
    )
    .unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .arg("--out")
        .arg(run.join("task.json"))
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "--out aliasing a consumed artifact must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("overwrite the consumed run artifact"),
        "{stderr}"
    );
    // task.json overwrite EDİLMEDİ (digest girdisi korunur).
    assert_eq!(
        read_json(&run.join("task.json"))["task"]["id"],
        1,
        "consumed artifact must be untouched after the rejected run"
    );
}

/// Tur-2 P0: alan VARLIĞI ≠ canonical ŞEKİL — `execution_measurement: null`,
/// `result: null`, `evidence: [null]` taşıyan presence-only sahte envelope
/// tipli iç-shape doğrulamasından geçemez.
#[test]
fn finalize_run_presence_only_null_shapes_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-nullshape");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        serde_json::json!({
            "schema_version": 1,
            "run": {
                "task_id": 1,
                "execution_mode": "harness",
                "witness_mode": "harness_auto_approve",
                "task_source": "harness_task_file",
                "repository_head": fx.head
            },
            "execution_measurement": null,
            "result": null,
            "evidence": [null]
        })
        .to_string(),
    )
    .unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "presence-only null shapes must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not match the #166 run envelope shape"),
        "{stderr}"
    );
}

/// Tur-2 P0: result.kind kanonik wire kümesi dışındaysa red.
#[test]
fn finalize_run_unknown_result_kind_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-badkind");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    let mut attempt = attempt_envelope(&fx.head, 1);
    attempt["result"]["kind"] = serde_json::json!("vibes_ok");
    fs::write(run.join("attempt.json"), attempt.to_string()).unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "unknown result.kind must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("canonical CliRunResultKind"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Tur-2 P0: proposals zarfı da TİPLİ doğrulanır — `{"schema_version":2,
/// "repository_head": <doğru>}` presence-only minimal obje (proposals alanı
/// yok) reddedilir.
#[test]
fn finalize_run_proposals_minimal_envelope_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-props-minimal");
    fs::create_dir_all(&run).unwrap();
    fs::copy(&baseline, run.join("baseline.json")).unwrap();
    fs::write(
        run.join("task.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head,
            "scope_bindings": [{"path": "main.rs"}],
            "task": {"id": 1}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run.join("attempt.json"),
        attempt_envelope(&fx.head, 1).to_string(),
    )
    .unwrap();
    fs::write(
        run.join("proposals.json"),
        serde_json::json!({
            "schema_version": 2,
            "repository_head": fx.head
        })
        .to_string(),
    )
    .unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "minimal proposals envelope must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("v2 envelope shape"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Tur-2 P1 (reviewer'ın regression matrisi): baseline Some, proposals_spec
/// None, out_proposals None, out_task == baseline → red + baseline baytları
/// DEĞİŞMEDİ (task-only yolda alias fence eskiden hiç koşmuyordu).
#[test]
fn draft_task_task_only_out_task_aliasing_baseline_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let bytes_before = fs::read(&baseline).unwrap();

    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", None)
        .arg("--out-task")
        .arg(&baseline)
        .output()
        .expect("run osp draft-task (task-only alias)");
    assert!(
        !out.status.success(),
        "task-only out-task == baseline must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--baseline input"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read(&baseline).unwrap(),
        bytes_before,
        "baseline bytes must be unchanged after the rejected run"
    );
}

/// Tur-2 P2: staged publish — ikinci çıktının HAZIRLIK hatasında (var olmayan
/// parent dizin) ilkinin hiçbir baytı görünmez olur; yarım artifact seti kalmaz.
#[test]
fn draft_task_staged_publish_no_partial_set_on_second_prep_failure() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = work.join("task.json");
    let missing_dir_props = work.join("no-such-dir").join("proposals.json");

    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(&missing_dir_props)
        .output()
        .expect("run osp draft-task (staged prep failure)");
    assert!(
        !out.status.success(),
        "second prep failure must fail the command"
    );
    assert!(
        !task.exists(),
        "task.json must NOT be visible when the proposals prep failed — \
         staged publish leaves no partial artifact set"
    );
    assert!(
        !missing_dir_props.exists(),
        "nothing may be created under the missing dir"
    );
}

/// Tur-3 P0: canonical producer domain'i — üç adversarial case tek matriste:
/// (a) kapalı enum dışı evidence kararları, (b) yabancı evidence task_id,
/// (c) producer guard'ının reddettiği mode kombinasyonu (production +
/// harness_auto_approve).
#[test]
fn finalize_run_adversarial_canonical_semantics_matrix() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    let canonical_evidence = |task_id: u64| {
        serde_json::json!({
            "trajectory_id": 1,
            "milestone_id": 1,
            "task_id": task_id,
            "attempt_id": 1,
            "before": {"x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
            "after": {"x": 0.5, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
            "gate_decision": "PassedAll",
            "predicate_completion": "Completed",
            "mutation_decision": "AcceptAsCompleted",
            "token_cost": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
            "duration_ms": 1
        })
    };

    let prepare_run = |name: &str, mutate: &dyn Fn(&mut serde_json::Value)| {
        let run = work.join(name);
        fs::create_dir_all(&run).unwrap();
        fs::copy(&baseline, run.join("baseline.json")).unwrap();
        fs::write(
            run.join("task.json"),
            serde_json::json!({
                "schema_version": 2,
                "repository_head": fx.head,
                "scope_bindings": [{"path": "main.rs"}],
                "task": {"id": 1}
            })
            .to_string(),
        )
        .unwrap();
        let mut attempt = attempt_envelope(&fx.head, 1);
        attempt["evidence"] = serde_json::json!([canonical_evidence(1)]);
        mutate(&mut attempt);
        fs::write(run.join("attempt.json"), attempt.to_string()).unwrap();
        let out = osp_in(&work)
            .arg("finalize-run")
            .arg(&run)
            .output()
            .unwrap();
        (out, run)
    };

    // (a) kapalı enum dışı kararlar — core TrajectoryEvidence serde'sinde düşer.
    let (out, _) = prepare_run("run-adv-enum", &|attempt: &mut serde_json::Value| {
        let evidence = &mut attempt["evidence"][0];
        evidence["gate_decision"] = serde_json::json!("Vibes");
        evidence["predicate_completion"] = serde_json::json!("Maybe");
        evidence["mutation_decision"] = serde_json::json!("ShipIt");
    });
    assert!(
        !out.status.success(),
        "non-canonical evidence enums must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not match the #166 run envelope shape"),
        "{stderr}"
    );

    // (b) yabancı evidence task_id — run task 1, evidence task 999.
    let (out, _) = prepare_run("run-adv-evidence-id", &|attempt: &mut serde_json::Value| {
        attempt["evidence"][0]["task_id"] = serde_json::json!(999);
    });
    assert!(
        !out.status.success(),
        "foreign evidence task_id must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("evidence fence"), "{stderr}");

    // (c) production + harness_auto_approove — producer guard'ının reddettiği
    // kombinasyon; üyelikler ayrı ayrı geçse de canonical DEĞİL.
    let (out, _) = prepare_run("run-adv-mode-combo", &|attempt: &mut serde_json::Value| {
        attempt["run"]["execution_mode"] = serde_json::json!("production");
        attempt["run"]["witness_mode"] = serde_json::json!("harness_auto_approve");
    });
    assert!(
        !out.status.success(),
        "producer-invalid mode combination must fail closed"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not canonical"), "{stderr}");

    // Kontrol: mutasyonsuz canonical envelope aynı helper'dan GEÇER.
    let (out, run_ok) = prepare_run("run-adv-clean", &|_attempt: &mut serde_json::Value| {});
    assert!(
        out.status.success(),
        "canonical envelope must be accepted. stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let row: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(row["schema_version"], "live-ledger-v1");
    assert!(run_ok.join("attempt.json").is_file());
}
