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
        .output()
        .expect("run osp analyze");
    assert!(
        out.status.success(),
        "analyze must succeed. stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let b = read_json(&baseline);
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
    // gerekiyor — attempt fence'leri aynı önermeye yaslanır).
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
    assert_eq!(
        row["build_verification"]["verified_state"]["repository_head"],
        after_head
    );
    assert_eq!(
        row["build_verification"]["verified_state"]["patch_digest"],
        sha256_of(&patch)
    );
    assert_eq!(
        row["build_verification"]["command"],
        serde_json::Value::Null
    );

    // K4: yorum alanları null — motor doldurmaz.
    for field in [
        "decision",
        "patch_outcome",
        "decision_utility",
        "counterfactual",
        "human_override",
        "notes",
    ] {
        assert_eq!(
            row[field],
            serde_json::Value::Null,
            "{field} must stay null"
        );
    }
    assert_eq!(row["friction"], serde_json::json!([]));
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
            "task": {}
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
            "task": {}
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
