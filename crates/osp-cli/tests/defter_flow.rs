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

    // Adım 4b: realization-gate (karar 2: graph-completed finalization kapıyı
    // ZORUNLU tüketir). Declared evidence: yama gerçekten uygulandı + derlendi.
    let declared = run.join("declared-realization.json");
    fs::write(
        &declared,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":null}"#,
    )
    .unwrap();
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg("run")
        .arg("--state-dir")
        .arg(fx.work_path())
        .arg("--evidence")
        .arg(&declared)
        .output()
        .expect("run osp realization-gate");
    assert!(
        out.status.code() == Some(0) || out.status.code() == Some(3),
        "gate realized veya predicate-unsat olmalı (gerçek after ölçümüne göre); \
         stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let verdict: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run.join("realization-verdict.json")).unwrap())
            .unwrap();
    let gate_realized = verdict["verdict"]["RealizedCompleted"].is_object();

    // Adım 5: finalize-run (ritüel gibi OSP kökünden görece run-dir → ref'ler görece).
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg("run")
        .arg("--repository")
        .arg("testrepo")
        .arg("--state-dir")
        .arg(fx.work_path())
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
    // #178 tur-3/P2: anchor'lu satırda downgrade alanı YOKTUR (missing ≠ false
    // ≠ null-beyan; anahtar yalnız --allow-unanchored-legacy kullanımında eklenir).
    assert!(
        !row.as_object().unwrap().contains_key("unanchored_legacy"),
        "anchored row must not carry the downgrade key"
    );
    // #188: anchored satır finalize anındaki canonical kimliği taşır — digest
    // bağımsız yeniden hesapla eşit (read-once tampon ≡ attempt.json baytları),
    // ref state-dir'e göre ileri-slash, gösterdiği dosya bayt-özdeş.
    assert_eq!(row["attempt_digest"], sha256_of(&attempt));
    let canonical_ref = row["canonical_attempt_ref"].as_str().unwrap();
    let stored: Vec<String> = fs::read_dir(fx.work_path().join("attempts"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("task-1-") && n.ends_with(".json"))
        .collect();
    assert_eq!(stored.len(), 1, "fixture tek canonical attempt yazar");
    assert_eq!(canonical_ref, format!("attempts/{}", stored[0]));
    assert_eq!(
        sha256_of(&fx.work_path().join("attempts").join(&stored[0])),
        row["attempt_digest"],
        "canonical_attempt_ref'in gösterdiği dosya attempt.json ile bayt-özdeş"
    );
    // #198 (INV-T10 canonical consumer): graph-completed satır zeminini taşır —
    // kapı realized dediyse "realized"; değilse "graph" + verdict etiketi.
    if gate_realized {
        assert_eq!(row["completion_basis"], "realized");
    } else {
        assert_eq!(row["completion_basis"], "graph");
        assert!(row["realization_verdict"].is_string());
    }
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

/// #178: attempt artifact tükettiği task/proposals dosyalarının digest'lerini
/// taşır; finalize anında dosya değiştiyse fail-closed. Aynı task_id + aynı
/// repository_head altında "sonradan değiştirildi" artık ispatlanabilir.
#[test]
fn finalize_run_rejects_task_tampered_after_attempt() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let run = work.join("run-tamper");
    fs::create_dir_all(&run).unwrap();
    let baseline = run.join("baseline.json");

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

    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = run.join("task.json");
    let props = run.join("proposals.json");
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
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
    assert!(out.status.success(), "attempt must complete");

    // Producer tarafı: envelope digest alanlarını taşır ve attempt-ani baytlarıyla
    // eşleşir.
    let envelope = read_json(&attempt);
    let task_digest = envelope["run"]["task_digest"].as_str().unwrap_or_default();
    assert_eq!(
        task_digest,
        sha256_of(&task),
        "producer digest = attempt-ani bytes"
    );
    assert_eq!(
        envelope["run"]["proposals_digest"]
            .as_str()
            .unwrap_or_default(),
        sha256_of(&props)
    );

    // TAMPER: task.json'ı HEAD/id koruyarak değiştir (head fence'ler geçsin —
    // yalnız digest fence'i ateşlensin).
    let mut tampered = read_json(&task);
    tampered["task"]["label"] = serde_json::json!("tampered after the attempt");
    fs::write(&task, serde_json::to_string_pretty(&tampered).unwrap()).unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg("run-tamper")
        .arg("--repository")
        .arg("testrepo")
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp finalize-run");
    assert!(
        !out.status.success(),
        "tampered task.json must fail the digest fence"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("task digest fence"),
        "digest fence mesajı gelmeli (head fence değil): {stderr}"
    );
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
        .arg("--allow-unanchored-legacy")
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
        .arg("--allow-unanchored-legacy")
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
        .arg("--allow-unanchored-legacy")
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
        .arg("--allow-unanchored-legacy")
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
        .arg("--allow-unanchored-legacy")
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
        .arg("--allow-unanchored-legacy")
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
        .arg("--allow-unanchored-legacy")
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
/// #182: `--out-task` parent dizini yokken ölçüm/doğrulama ÇALIŞMADAN hızlı red
/// (run-17 sürtünmesi: pahalı adımdan sonra yazım hatası).
/// Not: tur-2 P2'nin "staged publish ikinci PREP hatasında yarım set bırakmaz"
/// garantisi artık CLI'dan erişilemez (yok-dizin preflight'te red) — kapsam,
/// `staged_publish_tests::staged_publish_no_partial_set_on_second_prep_failure`
/// unit testine taşındı (fonksiyon doğrudan çağrılır).
#[test]
fn draft_task_out_task_missing_parent_rejected_before_work() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = work.join("no-such-dir").join("task.json");

    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(&task)
        .arg("--out-proposals")
        .arg(work.join("proposals.json"))
        .output()
        .expect("run osp draft-task (missing --out-task parent)");
    assert!(
        !out.status.success(),
        "missing --out-task parent must reject"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("parent directory does not exist") && stderr.contains("--out-task"),
        "message: {stderr}"
    );
    assert!(!task.exists(), "rejected → no output file");
}

/// #178 review P0-2: digest alanı VARLIK BEYANIDIR — presence-aware fence matrisi:
/// (a) proposals_digest=NULL + run-dir'de proposals.json VAR → RED (attempt'in
///     tüketmediği artifact provenance'a sızamaz),
/// (b) task_digest=NULL + task_source=harness_task_file → RED (kanonik producer
///     bu şekli asla üretmez — task tüketildiyse hash'i vardır),
/// (c) proposals_digest=NULL + proposals.json YOK → GEÇER (#171 --llm real
///     dürüst-boşluk yolu; proposal_refs null ile ledger satırı üretilir).
#[test]
fn finalize_run_digest_presence_matrix() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    let canonical_evidence = serde_json::json!([{
        "trajectory_id": 1,
        "milestone_id": 1,
        "task_id": 1,
        "attempt_id": 1,
        "before": {"x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "after": {"x": 0.5, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "gate_decision": "PassedAll",
        "predicate_completion": "Completed",
        "mutation_decision": "AcceptAsCompleted",
        "token_cost": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
        "duration_ms": 1
    }]);

    let prepare = |name: &str| {
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
        run
    };
    let envelope_with = |task_digest: serde_json::Value, proposals_digest: serde_json::Value| {
        let mut env = attempt_envelope(&fx.head, 1);
        env["evidence"] = canonical_evidence.clone();
        env["run"]["task_digest"] = task_digest;
        env["run"]["proposals_digest"] = proposals_digest;
        env
    };
    // #178 tur-2 P0: el-yapımı yeni-şekil zarflar digest alanları taşıdığı için
    // canonical trust-anchor fence'i de geçmeli — zarfın AYNI baytları no-clobber
    // mağazaya da yazılır (gerçek attempt'ın persist adımının el ile taklidi) ve
    // finalize --state-dir alır.
    let write_with_canonical = |run: &Path, env: &serde_json::Value| {
        let bytes = env.to_string();
        fs::write(run.join("attempt.json"), &bytes).unwrap();
        let attempts = fx.work_path().join("attempts");
        fs::create_dir_all(&attempts).unwrap();
        fs::write(attempts.join("task-1-990001-1.json"), bytes).unwrap();
    };
    let finalize = |run: &Path| {
        osp_in(&work)
            .arg("finalize-run")
            .arg(run)
            .arg("--state-dir")
            .arg(fx.work_path())
            .output()
            .expect("run osp finalize-run")
    };

    // (a) null beyanı + var olan dosya → red.
    let run = prepare("run-null-props-present");
    write_with_canonical(
        &run,
        &envelope_with(
            serde_json::json!(sha256_of(&run.join("task.json"))),
            serde_json::Value::Null,
        ),
    );
    fs::write(run.join("proposals.json"), "{}").unwrap();
    let out = finalize(&run);
    assert!(!out.status.success(), "(a) null + present must reject");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("declares NO proposals consumed"),
        "(a) message: {stderr}"
    );

    // (b) harness_task_file + null task_digest → red (kanonik olmayan şekil).
    let run = prepare("run-null-task");
    write_with_canonical(
        &run,
        &envelope_with(serde_json::Value::Null, serde_json::Value::Null),
    );
    let out = finalize(&run);
    assert!(!out.status.success(), "(b) null task_digest must reject");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a canonical producer shape"),
        "(b) message: {stderr}"
    );

    // (c) null + yok → GEÇER (#171 dürüst-boşluk).
    let run = prepare("run-null-props-absent");
    write_with_canonical(
        &run,
        &envelope_with(
            serde_json::json!(sha256_of(&run.join("task.json"))),
            serde_json::Value::Null,
        ),
    );
    let out = finalize(&run);
    assert!(
        out.status.success(),
        "(c) null + absent must pass (#171 honest gap): {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let row: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(row["proposal_refs"], serde_json::Value::Null);
    assert_eq!(row["proposal_digest"], serde_json::Value::Null);
    // #188: anchored geçiş — canonical kimlik read-once baytlardan; ref
    // state-dir'e göre ileri-slash, hand-written mağaza adıyla birebir.
    assert_eq!(row["attempt_digest"], sha256_of(&run.join("attempt.json")));
    assert_eq!(
        row["canonical_attempt_ref"],
        "attempts/task-1-990001-1.json"
    );
}

/// #178 review P0-2 (fail-open kapanışı): attempt proposals TÜKETTİYSE (zarf
/// digest taşıyorsa) dosyanın attempt sonrası SİLİNMESİ red üretir — Some(digest)
/// dosyanın zorunlu varlık kanıtıdır.
#[test]
fn finalize_run_rejects_proposals_deleted_after_attempt() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let run = work.join("run-props-deleted");
    fs::create_dir_all(&run).unwrap();
    let baseline = run.join("baseline.json");

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

    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = run.join("task.json");
    let props = run.join("proposals.json");
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
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
    assert!(out.status.success(), "attempt must complete");

    // Producer: zarf proposals digest'i TAŞIYOR (mock kol tüketti).
    let envelope = read_json(&attempt);
    assert!(
        envelope["run"]["proposals_digest"].is_string(),
        "mock attempt must carry proposals_digest"
    );

    // SİLME: attempt'in tüketildiği kanıtın üstünden dosyayı çek.
    fs::remove_file(&props).unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg("run-props-deleted")
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp finalize-run");
    assert!(!out.status.success(), "deleted proposals must fail closed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no longer carries proposals.json"),
        "message: {stderr}"
    );
}

/// #178 tur-2 P0 (trust anchor): TUTARLI tamper — task.json DEĞİŞTİRİLİR ve
/// attempt.json'un task_digest'i yeni hash'e GÜNCELLENİR (digest fence'lerin
/// hepsi geçer). Yakalayan şey canonical fence'tir: run-dir kopyası artık
/// makinenin no-clobber store'undaki artifact ile bayt-özdeş DEĞİLDİR.
#[test]
fn finalize_run_rejects_coherent_tamper_via_canonical_fence() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let run = work.join("run-coherent-tamper");
    fs::create_dir_all(&run).unwrap();
    let baseline = run.join("baseline.json");

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

    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = run.join("task.json");
    let props = run.join("proposals.json");
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
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
    assert!(out.status.success(), "attempt must complete");

    // TUTARLI saldırı: task.json'i değiştir + zarfdaki task_digest'i YENİ hash'e
    // güncelle (head/id/evidence aynen — tüm digest fence'ler geçecek şekilde).
    let mut tampered_task = read_json(&task);
    tampered_task["task"]["label"] = serde_json::json!("coherently forged");
    fs::write(&task, serde_json::to_string_pretty(&tampered_task).unwrap()).unwrap();
    let mut forged = read_json(&attempt);
    forged["run"]["task_digest"] = serde_json::json!(sha256_of(&task));
    fs::write(&attempt, serde_json::to_string_pretty(&forged).unwrap()).unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg("run-coherent-tamper")
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp finalize-run");
    assert!(
        !out.status.success(),
        "coherent tamper must fail the canonical fence (digest fences alone pass it)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not byte-match any canonical artifact"),
        "message: {stderr}"
    );
}

/// #178 tur-2 P0 (bypass #2): digest alanlarını zarfdan TAMAMEN SİLMEK —
/// "legacy missing" gibi görünür ve digest fence'leri atlatırdı. Canonical
/// fence: mağazada bu task'ın artifact'ı VAR → kopya bayt-özdeş olmalı → silinmiş
/// kopya RED.
#[test]
fn finalize_run_rejects_stripped_digest_fields() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let run = work.join("run-stripped");
    fs::create_dir_all(&run).unwrap();
    let baseline = run.join("baseline.json");

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

    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = run.join("task.json");
    let props = run.join("proposals.json");
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
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
    assert!(out.status.success(), "attempt must complete");

    // ALAN SİLME: digest alanlarını kaldır → "legacy" gibi görün.
    let mut stripped = read_json(&attempt);
    stripped["run"]
        .as_object_mut()
        .expect("run object")
        .remove("task_digest");
    stripped["run"]
        .as_object_mut()
        .expect("run object")
        .remove("proposals_digest");
    fs::write(&attempt, serde_json::to_string_pretty(&stripped).unwrap()).unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg("run-stripped")
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp finalize-run");
    assert!(
        !out.status.success(),
        "stripped digest fields must fail the canonical fence (legacy-look bypass)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("does not byte-match any canonical artifact"),
        "message: {stderr}"
    );
}

/// #178 tur-3 P0 kabul testi (review senaryosu birebir): yeni attempt üret →
/// run dizinini default probe'un bulamayacağı yere TAŞI → digest alanlarını SİL →
/// task label değiştir (head/id koru) → `--state-dir`'siz finalize → RED.
/// Otomatik legacy kabul kaldırıldı: anchor yok + legacy görünüm = fail-closed.
#[test]
fn finalize_run_rejects_relocated_run_with_stripped_fields() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let run = work.join("run-relocate");
    fs::create_dir_all(&run).unwrap();
    let baseline = run.join("baseline.json");

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

    let spec = write_spec(&work, "main.rs", "a.rs");
    let task = run.join("task.json");
    let props = run.join("proposals.json");
    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
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
    assert!(out.status.success(), "attempt must complete");

    // TAŞIMA: run dizinini probe'un (<run>/../../state) bulamayacağı bir yere kopyala.
    let outside = tempfile::tempdir().expect("relocated base");
    let relocated = outside.path().join("run-relocate");
    fs_extra_like_copy_dir(&run, &relocated);

    // ALAN SİLME + TUTARLI TAMPER: digest alanlarını kaldır + task label değiştir
    // (repository_head + task_id aynen — head/id fence'leri geçsin).
    let relocated_task = relocated.join("task.json");
    let mut tampered = read_json(&relocated_task);
    tampered["task"]["label"] = serde_json::json!("tampered in the relocated dir");
    fs::write(
        &relocated_task,
        serde_json::to_string_pretty(&tampered).unwrap(),
    )
    .unwrap();
    let mut stripped = read_json(&relocated.join("attempt.json"));
    stripped["run"]
        .as_object_mut()
        .expect("run object")
        .remove("task_digest");
    stripped["run"]
        .as_object_mut()
        .expect("run object")
        .remove("proposals_digest");
    fs::write(
        relocated.join("attempt.json"),
        serde_json::to_string_pretty(&stripped).unwrap(),
    )
    .unwrap();

    let out = osp_in(outside.path())
        .arg("finalize-run")
        .arg(&relocated)
        .output()
        .expect("run osp finalize-run (relocated, no --state-dir)");
    assert!(
        !out.status.success(),
        "relocated run + stripped fields + no anchor must fail closed (no silent legacy)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--allow-unanchored-legacy"),
        "RED açık downgrade'i işaret etmeli: {stderr}"
    );
}

/// #190 review P1 (content identity ≠ artifact identity): aynı task için İKİ
/// byte-özdeş canonical artifact, satırın adlandırabileceği artifact'ı
/// belirsizleştirir (retry senaryosu: A publish + --out FAIL; B publish +
/// --out OK; envelope baytları özdeş — digest içerik kanıtlar, HANGİ invocation
/// ürettiğini değil). finalize kesinleştirilmiş bir seçim yapamaz → ambiguity
/// RED'i (deterministik sıralı seçim truthful olmazdı).
#[test]
fn finalize_run_rejects_ambiguous_byte_identical_canonical_artifacts() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    let run = work.join("run-ambiguous");
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
    // Yeni-şekil zarf (task_digest beyanlı) + kanonik evidence; AYNI baytlar iki
    // canonical dosyaya yazılır (iki invocation'ın persist adımının taklidi).
    let mut env = attempt_envelope(&fx.head, 1);
    env["run"]["task_digest"] = serde_json::json!(sha256_of(&run.join("task.json")));
    env["run"]["proposals_digest"] = serde_json::Value::Null;
    env["evidence"] = serde_json::json!([{
        "trajectory_id": 1, "milestone_id": 1, "task_id": 1, "attempt_id": 1,
        "before": {"x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "after": {"x": 0.5, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "gate_decision": "PassedAll", "predicate_completion": "Completed",
        "mutation_decision": "AcceptAsCompleted",
        "token_cost": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
        "duration_ms": 1
    }]);
    let bytes = env.to_string();
    fs::write(run.join("attempt.json"), &bytes).unwrap();
    let attempts = fx.work_path().join("attempts");
    fs::create_dir_all(&attempts).unwrap();
    fs::write(attempts.join("task-1-990001-1.json"), &bytes).unwrap();
    fs::write(attempts.join("task-1-990002-1.json"), &bytes).unwrap();

    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .expect("run osp finalize-run");
    assert!(
        !out.status.success(),
        "two byte-identical canonical artifacts must fail closed — no arbitrary selection"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ambiguous"),
        "ambiguity error expected: {stderr}"
    );
    assert!(
        stderr.contains("the digest proves content, not which invocation"),
        "content-vs-artifact rationale in message: {stderr}"
    );
}

/// #198 INV-T10: `osp realization-gate` uçtan uca — D5a'nın üç hücresi +
/// anchor zorunluluğu + no-clobber. Motor türetimleri: c_observed after.json'dan,
/// predicate_after_reanalysis = observed ≤ threshold.
#[test]
fn realization_gate_end_to_end_matrix() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);

    let prepare = |name: &str, anchored: bool, threshold: f64, observed: f64| {
        let run = work.join(name);
        fs::create_dir_all(&run).unwrap();
        fs::copy(&baseline, run.join("baseline.json")).unwrap();
        fs::write(
            run.join("task.json"),
            serde_json::json!({
                "schema_version": 2,
                "repository_head": fx.head,
                "scope_bindings": [{"path": "main.rs"}],
                "task": {"id": 1, "target_predicate_set": {"mode": "All", "predicates": [
                    {"predicate": {"metric": "Coupling", "operator": "Le", "threshold": threshold,
                                   "tolerance": 0.0, "scope": {"Path": "main.rs"}, "required_source": "TreeSitter"},
                     "weight": null}]}}
            })
            .to_string(),
        )
        .unwrap();
        // Reanalysis gözlemi: after.json (analyze zarfı) — scope node coupling.
        fs::write(
            run.join("after.json"),
            serde_json::json!({
                "schema_version": 2,
                "repository": {"head": fx.head},
                "nodes": [{"path": "main.rs", "coupling": {"value": observed}}]
            })
            .to_string(),
        )
        .unwrap();
        // Graph-completed zarf (kind=completed + completion_basis=graph +
        // run.task_digest — gate yalnız digest-taşıyan yeni-şekil zarflarda koşar)
        // ve kanonik mağaza kopyası (bayt-özdeş).
        let task_digest = sha256_of(&run.join("task.json"));
        let mut env = attempt_envelope(&fx.head, 1);
        env["completion_basis"] = serde_json::json!("graph");
        env["run"]["task_digest"] = serde_json::json!(task_digest);
        env["evidence"] = serde_json::json!([{
            "trajectory_id": 1, "milestone_id": 1, "task_id": 1, "attempt_id": 1,
            "before": {"x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
            "after": {"x": 0.8, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
            "gate_decision": "PassedAll", "predicate_completion": "Completed",
            "mutation_decision": "AcceptAsCompleted",
            "token_cost": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
            "duration_ms": 1
        }]);
        let bytes = env.to_string();
        fs::write(run.join("attempt.json"), &bytes).unwrap();
        if anchored {
            let attempts = fx.work_path().join("attempts");
            fs::create_dir_all(&attempts).unwrap();
            fs::write(attempts.join("task-1-990001-1.json"), bytes).unwrap();
        }
        run
    };
    let evidence_file = |run: &Path, body: &str| {
        let p = run.join("declared-realization.json");
        fs::write(&p, body).unwrap();
        p
    };

    // (a) D5a A-hücresi: build-geçer + gözlem predicate altı → RealizedCompleted (exit 0).
    let run = prepare("run-gate-ok", true, 0.9, 0.875);
    let ev = evidence_file(
        &run,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":{"passed":271,"failed":0,"skipped":0}}"#,
    );
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg(&run)
        .arg("--state-dir")
        .arg(fx.work_path())
        .arg("--evidence")
        .arg(&ev)
        .output()
        .expect("run osp realization-gate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let verdict: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run.join("realization-verdict.json")).unwrap(),
    )
    .unwrap();
    assert!(
        verdict["verdict"]["RealizedCompleted"].is_object(),
        "{verdict}"
    );
    assert_eq!(verdict["gate_context"]["c_observed"], 0.875);
    // Sim-öngörü (evidence son after.x=0.8) → E_c = 0.875 − 0.8 imzalı residual.
    let e_c = verdict["e_c"].as_f64().expect("e_c derive edilmeli");
    assert!((e_c - 0.075).abs() < 1e-9, "E_c={e_c}");

    // (b) D5a B-hücresi: build-kırık → DeclaredRealizationBuildInvalid (exit 2),
    // TAM kanıt taşınır (error_count + c_observed).
    let run = prepare("run-gate-bad", true, 0.9, 0.0);
    let ev = evidence_file(
        &run,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"failed","error_count":122},"tests":null}"#,
    );
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg(&run)
        .arg("--state-dir")
        .arg(fx.work_path())
        .arg("--evidence")
        .arg(&ev)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let verdict: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run.join("realization-verdict.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        verdict["verdict"]["DeclaredRealizationBuildInvalid"]["evidence"]["raw"]["build"]["Failed"]
            ["error_count"],
        122
    );

    // (c) Anchor yok → gate RED (legacy downgrade kapı bağlamı değildir).
    let run = prepare("run-gate-unanchored", false, 0.9, 0.5);
    let ev = evidence_file(
        &run,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":null}"#,
    );
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg(&run)
        .arg("--evidence")
        .arg(&ev)
        .output()
        .unwrap();
    assert!(!out.status.success());
    // Anchor yok → #178 fence'i RED (digest'siz zarf legacy-şekli mesajıyla;
    // gate'in kendi UnanchoredLegacy RED'i ancak flag'li indirimde devreye girer).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("allow-unanchored-legacy") || stderr.contains("anchored attempt envelope"),
        "anchor gerekçesi mesajda yok: {stderr}"
    );

    // (d) Predicate gözlemde düşer → PredicateUnsatisfiedAfterReanalysis (exit 3).
    let run = prepare("run-gate-unsat", true, 0.85, 0.9167);
    let ev = evidence_file(
        &run,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":null}"#,
    );
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg(&run)
        .arg("--state-dir")
        .arg(fx.work_path())
        .arg("--evidence")
        .arg(&ev)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    let verdict: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run.join("realization-verdict.json")).unwrap(),
    )
    .unwrap();
    assert!(verdict["verdict"]["PredicateUnsatisfiedAfterReanalysis"].is_object());

    // (e) No-clobber: aynı run-dir'de ikinci değerlendirme RED.
    let ev = evidence_file(
        &run,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":null}"#,
    );
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg(&run)
        .arg("--state-dir")
        .arg(fx.work_path())
        .arg("--evidence")
        .arg(&ev)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("written once"));

    // (f) P0 artifact-identity: attempt koştuktan SONRA task.json değiştirilirse
    // (ör. threshold kolaylaştırma) digest fence RED — substitution reddi.
    let run = prepare("run-gate-tamper", true, 0.9, 0.5);
    let ev = evidence_file(
        &run,
        r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":null}"#,
    );
    let task_file = run.join("task.json");
    let task_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&task_file).unwrap()).unwrap();
    let mut easier = task_json.clone();
    easier["task"]["target_predicate_set"]["predicates"][0]["predicate"]["threshold"] =
        serde_json::json!(0.99);
    fs::write(&task_file, serde_json::to_string(&easier).unwrap()).unwrap();
    let out = osp_in(&work)
        .arg("realization-gate")
        .arg(&run)
        .arg("--state-dir")
        .arg(fx.work_path())
        .arg("--evidence")
        .arg(&ev)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "kolaylaştırılmış threshold RED olmalı"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("substitution refused") || stderr.contains("changed after the attempt"),
        "P0 fence gerekçesi: {stderr}"
    );

    // (g) Karar 2 (zorunlu tüketim): graph-completed run KAPISIZ finalize EDİLEMEZ.
    let run = prepare("run-gate-noverdict", true, 0.9, 0.5);
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .arg("--repository")
        .arg("testrepo")
        .arg("--state-dir")
        .arg(fx.work_path())
        .output()
        .unwrap();
    assert!(!out.status.success(), "verdict'siz finalize RED olmalı");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("realization gate fence"),
        "zorunlu-tüketim mesajı"
    );
}

/// #178 tur-3 P0 kabul testi (ikinci yarısı): gerçek historical legacy zarf +
/// canonical store yok + AÇIK `--allow-unanchored-legacy` → GEÇER; ledger satırı
/// `unanchored_legacy: true` taşır (epistemik zayıflama kayıtlı).
#[test]
fn finalize_run_unanchored_legacy_with_explicit_flag_passes() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    let run = work.join("run-historical-legacy");
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
    // Legacy-şekil zarf (digest alanları YOK — pre-#178 producer) + kanonik evidence.
    let mut env = attempt_envelope(&fx.head, 1);
    env["evidence"] = serde_json::json!([{
        "trajectory_id": 1, "milestone_id": 1, "task_id": 1, "attempt_id": 1,
        "before": {"x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "after": {"x": 0.5, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "gate_decision": "PassedAll", "predicate_completion": "Completed",
        "mutation_decision": "AcceptAsCompleted",
        "token_cost": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
        "duration_ms": 1
    }]);
    fs::write(run.join("attempt.json"), env.to_string()).unwrap();

    // Store yok + flag YOK → RED (fail-closed default).
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "unanchored legacy without the explicit flag must fail closed"
    );

    // Flag İLE → GEÇER + downgrade ledger'da görünür.
    let out = osp_in(&work)
        .arg("finalize-run")
        .arg(&run)
        .arg("--allow-unanchored-legacy")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "explicit --allow-unanchored-legacy must pass: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let row: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(row["unanchored_legacy"], serde_json::json!(true));
    // #188: legacy downgrade satırında canonical kimlik alanları YOKTUR
    // (missing ≡ downgrade tutarlılığı — unanchored_legacy zaten kayıtlı).
    for key in ["attempt_digest", "canonical_attempt_ref"] {
        assert!(
            !row.as_object().unwrap().contains_key(key),
            "legacy row must not carry {key}"
        );
    }
}

/// Basit özyinelemeli dizin kopyası (std-only; test yardımcısı).
fn fs_extra_like_copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("mkdir dst");
    for entry in fs::read_dir(src).expect("read src").filter_map(|e| e.ok()) {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            fs_extra_like_copy_dir(&from, &to);
        } else {
            fs::copy(&from, &to).expect("copy file");
        }
    }
}

/// #183: proposals v2 ÇIKTISINDAN kopyalanan output-only alan (position_hints)
/// girdi spec'ine girerse unknown-field reddi tek adımda yönlendirir
/// (run-17 sürtünmesi 3: çıktı doğal şablon alınıyor).
#[test]
fn draft_task_spec_output_only_field_message_guides_to_drop() {
    let fx = HarnessFixture::new_with_use_edges();
    let work = fx.work_path().to_path_buf();
    let (baseline, _) = measured_baseline(&fx);
    // Aksi takdirde geçerli bir spec; yalnız çıktı-only `position_hints` fazla.
    let spec = work.join("spec-output-only.json");
    fs::write(
        &spec,
        r#"{"proposals": [{"removed_edges": [{"from": "main.rs", "to": "a.rs"}],
            "position_hints": [], "reasoning": "output copied as template"}]}"#,
    )
    .expect("write spec");

    let out = draft_task_cmd(&fx, &work, &baseline, "main.rs", Some(&spec))
        .arg("--out-task")
        .arg(work.join("task.json"))
        .arg("--out-proposals")
        .arg(work.join("proposals.json"))
        .output()
        .expect("run osp draft-task (output-only field)");
    assert!(
        !out.status.success(),
        "output-only field in spec must reject (deny_unknown_fields unchanged)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown field `position_hints`"),
        "serde red mesajı korunmalı: {stderr}"
    );
    assert!(
        stderr.contains("output-only field") && stderr.contains("drop it from the input spec"),
        "yönlendirme eklenmeli: {stderr}"
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
            // El-yapımı legacy-şekil zarflar (digest alansız) + store yok → semantik
            // fence'leri hedefleyen bu matris AÇIK downgrade ile koşar (#178 tur-3).
            .arg("--allow-unanchored-legacy")
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
