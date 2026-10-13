//! #204 E2E — "kirli yollar ⊆ beyan edilen evren" opt-in fence'i.
//!
//! Kabul kriterleri (issue #204) + review turlarının zorunlu regresyonları:
//! - dirty ⊆ declared durumunda stash'siz draft+attempt koşar (run-26 replikasyonu),
//! - beyan dışı tek kirli yol RED + yol listesi,
//! - default davranış değişmez (strict zarfta fence anahtarı YOK),
//! - intent-to-add (`git add -N`) kirli yol aynı kurala tabi,
//! - ölçüm SABİT HEAD'den (döngüsellik pini: kirli içerik hipotetik kararını
//!   etkileyemez),
//! - resume politikası originating zarftan devralınır (kirli worktree'de resume),
//! - kanonik originating zarfı olmayan askı resume'lanamaz (migration).

#[path = "common/mod.rs"]
mod common;

use assert_cmd::cargo::CommandCargoExt;
use common::{task_envelope_v2, HarnessFixture, OSP_ATTEMPT_LOCK};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git_in(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_DATE", "2002-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2002-01-01T00:00:00Z")
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn porcelain(repo: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("git status");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn worktree_count(repo: &Path) -> usize {
    git_in(repo, &["worktree", "list"]).lines().count()
}

/// `trajectory attempt` komutini hazır getir (test ek bayraklarla tamamlar).
fn attempt_cmd(fx: &HarnessFixture) -> Command {
    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Command::cargo_bin("osp").expect("osp binary");
    cmd.current_dir(fx.work_path())
        .arg("trajectory")
        .arg("attempt")
        .arg("7")
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--llm")
        .arg("mock")
        .arg("--state-dir")
        .arg(fx.work_path());
    cmd
}

/// use-edges fixture'ına ÜÇÜNCÜ tracked modül (c.rs) commit et → yeni HEAD.
/// Döngüsellik pini için: kirli main.rs `use crate::c::C;` ekleyince CANLI
/// ölçüm 3/4 olurdu (baz 2/3) — eşik 0.55 iki dünyayı AYIRT eder.
fn commit_third_module(fx: &HarnessFixture) -> String {
    std::fs::write(fx.repo_path().join("c.rs"), "pub struct C;\n").expect("write c.rs");
    git_in(fx.repo_path(), &["add", "c.rs"]);
    git_in(fx.repo_path(), &["commit", "-qm", "add c"]);
    git_in(fx.repo_path(), &["rev-parse", "HEAD"])
}

/// Run-26 yenileme senaryosunun worktree durumu: ölçülen main.rs modify +
/// yeni untracked dosya + intent-to-add dosya. Dönen porcelain, koşu-sonrası
/// karşılaştırma için taban alınır (kullanıcı ağacı değişmeden kalmalı).
fn dirty_renewal_state(fx: &HarnessFixture) -> String {
    let main = std::fs::read_to_string(fx.repo_path().join("main.rs")).unwrap();
    let dirty_main = main
        .replace("mod a;\nmod b;\n", "mod a;\nmod b;\nmod c;\n")
        .replace("use crate::a::A;\n", "use crate::a::A;\nuse crate::c::C;\n");
    std::fs::write(fx.repo_path().join("main.rs"), dirty_main).unwrap();
    std::fs::create_dir_all(fx.repo_path().join("src")).unwrap();
    std::fs::write(fx.repo_path().join("src/new.rs"), "pub struct New;\n").unwrap();
    std::fs::create_dir_all(fx.repo_path().join("gen")).unwrap();
    std::fs::write(fx.repo_path().join("gen/g.rs"), "pub struct G;\n").unwrap();
    git_in(fx.repo_path(), &["add", "-N", "gen/g.rs"]);
    porcelain(fx.repo_path())
}

/// Beyan edilen evreni taşıyan v3 proposals zarfı (yenileme senaryosunun kendisi).
fn write_v3_declared_proposals(fx: &HarnessFixture, head: &str) -> PathBuf {
    let proposals = serde_json::json!({
        "schema_version": 3,
        "repository_head": head,
        "proposals": [{
            "new_nodes": [
                {"kind": "Module", "initial_mass": 5.0, "path": "src/new.rs"},
                {"kind": "Module", "initial_mass": 1.0, "path": "gen/g.rs"}
            ],
            "new_edges": [],
            "removed_edges": [{"from": "main.rs", "to": "a.rs", "kind": "Imports"}],
            "affected_nodes": ["main.rs"],
            "modified_entities": [{"path": "main.rs"}],
            "reasoning": "honest renewal: declare the code that exists"
        }]
    });
    let path = fx.work_path().join("proposals.v3.json");
    std::fs::write(&path, serde_json::to_string_pretty(&proposals).unwrap()).unwrap();
    path
}

/// AddNode + RemoveImport + ModifyEntity izinli task (op-matrix: new_nodes →
/// AddNode, removed Imports → RemoveImport, modified_entities → ModifyEntity).
fn renewal_task(fx: &HarnessFixture, head: &str) -> PathBuf {
    let mut env = task_envelope_v2(head, "main.rs");
    env["task"]["allowed_operations"] =
        serde_json::json!(["AddNode", "RemoveImport", "ModifyEntity"]);
    let path = fx.work_path().join("task.v2.json");
    std::fs::write(&path, serde_json::to_string_pretty(&env).unwrap()).unwrap();
    path
}

fn read_json(path: &Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(path).expect("read json");
    serde_json::from_str(&raw).expect("parse json")
}

/// **Kabul başlığı — run-26 replikasyonu (tek komut dizisinde, stash'siz).**
/// Döngüsellik pini dahil: kirli main.rs canlı ölçümde 3/4=0.75 üretirdi
/// (eşik 0.55 altına RemoveImport ile DÜŞMEZDİ → RED); SABİT HEAD'den ölçüm
/// 2/3 → 1/2=0.5 → Completed. Başarı = ölçümün bazdan geldiğinin kanıtı.
#[test]
fn run26_renewal_replicated_stashfree_with_base_measurement() {
    let fx = HarnessFixture::new_with_use_edges();
    let head = commit_third_module(&fx);
    let task_path = renewal_task(&fx, &head);
    let proposals_path = write_v3_declared_proposals(&fx, &head);
    let attempt_out = fx.work_path().join("attempt.json");

    let before_dirty = dirty_renewal_state(&fx);

    // Non-vacuous pin: kirli ağaçta CANLI (generic) analiz main.rs 3/4 ölçer —
    // bu dünya eşiği GEÇEMEZDİ (RemoveImport sonrası 2/3 = 0.667 > 0.55).
    let live_coupling = fx.measured_main_coupling();
    assert!(
        (live_coupling - 0.75).abs() < 1e-9,
        "dirty live measurement must differ from base (got {live_coupling})"
    );

    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "harness"])
        .args(["--witness", "harness-auto-approve"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--out")
        .arg(&attempt_out)
        .arg("--allow-declared-dirty")
        .output()
        .expect("run osp attempt");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stderr.contains("Task completed"),
        "declared-dirty attempt must complete from the pinned-HEAD base. \
         exit={:?}\nstderr={stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("declared-dirty mode"),
        "visible exclusion line: {stderr}"
    );

    // Kullanıcı çalışma ağacı DEĞİŞMEDİ (kirli-set birebir; worktree kaydı temiz).
    assert_eq!(
        porcelain(fx.repo_path()),
        before_dirty,
        "the user's dirty worktree must be untouched by the detached measurement"
    );
    assert_eq!(
        worktree_count(fx.repo_path()),
        1,
        "detached analysis worktree must be removed after the run"
    );

    // Zarf: fence bloğu (Karar-C) — declared/observed kümeleri + provenance.
    let envelope = read_json(&attempt_out);
    let fence = &envelope["result"]["fence"];
    assert_eq!(fence["mode"], "declared_dirty", "{fence}");
    assert_eq!(fence["result"], "passed", "{fence}");
    let declared: Vec<&str> = fence["declared_paths"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for expected in ["main.rs", "src/new.rs", "gen/g.rs"] {
        assert!(
            declared.contains(&expected),
            "declared universe must contain {expected}: {fence}"
        );
    }
    let observed: Vec<&str> = fence["observed_dirty_paths"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        observed.contains(&"main.rs")
            && observed.contains(&"src/new.rs")
            && observed.contains(&"gen/g.rs"),
        "observed dirty set must record intent-to-add + untracked too: {fence}"
    );
    assert_eq!(
        fence["measurement"]["source_kind"], "detached_head",
        "{fence}"
    );
    assert_eq!(
        fence["measurement"]["head"].as_str(),
        Some(head.as_str()),
        "measurement head must be the pinned run HEAD"
    );
    assert!(
        !fence["measurement"]["space_digest"]
            .as_str()
            .unwrap_or("")
            .is_empty(),
        "space digest must be carried"
    );
    assert_eq!(
        envelope["run"]["repository_head"].as_str(),
        Some(head.as_str())
    );
}

/// Beyan dışı TEK kirli yol → RED + yol listeli + remedya (fail-closed).
#[test]
fn declared_dirty_attempt_rejects_undeclared_path() {
    let fx = HarnessFixture::new_with_use_edges();
    let head = commit_third_module(&fx);
    let task_path = renewal_task(&fx, &head);
    let proposals_path = write_v3_declared_proposals(&fx, &head);
    dirty_renewal_state(&fx);
    // Beyan-dışı scratch (analiz kapsamı DIŞI — global fence'te yine RED).
    std::fs::write(fx.repo_path().join("scratch-notes.md"), "todo\n").unwrap();

    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "harness"])
        .args(["--witness", "harness-auto-approve"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--allow-declared-dirty")
        .output()
        .expect("run osp attempt");
    assert!(
        !output.status.success(),
        "undeclared dirty path must fail the fence"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("undeclared dirty path") && stderr.contains("scratch-notes.md"),
        "offending path must be listed: {stderr}"
    );
    assert!(
        stderr.contains("stash") || stderr.contains("declare"),
        "remedy must be actionable: {stderr}"
    );
    // Fence RED'i worktree AÇMADAN koşar (hızlı red) — kayıt yok.
    assert_eq!(
        worktree_count(fx.repo_path()),
        1,
        "no worktree on early RED"
    );
}

/// `--allow-declared-dirty` --proposals'sız clap red'i; `--llm real` ile
/// koşu-öncesi guard red'i (beyan evreni koşu-öncesi bilinmeli).
#[test]
fn declared_dirty_flag_requires_proposals_and_rejects_llm_real() {
    let fx = HarnessFixture::new_with_use_edges();
    let head = commit_third_module(&fx);
    let task_path = renewal_task(&fx, &head);
    let proposals_path = write_v3_declared_proposals(&fx, &head);

    // (a) Flag + task ama proposals YOK → clap `requires` red'i.
    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "harness"])
        .args(["--witness", "harness-auto-approve"])
        .arg("--task")
        .arg(&task_path)
        .arg("--allow-declared-dirty")
        .output()
        .expect("run osp attempt");
    assert!(
        !output.status.success(),
        "flag without --proposals must be a clap error"
    );

    // (b) Flag + --llm real (+ artifacts) → koşu-öncesi kombinatorik guard.
    let artifacts = fx.work_path().join("llm-artifacts");
    std::fs::create_dir_all(&artifacts).unwrap();
    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "production"])
        .args(["--witness", "production"])
        .arg("--llm")
        .arg("real")
        .arg("--llm-artifacts")
        .arg(&artifacts)
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--allow-declared-dirty")
        .output()
        .expect("run osp attempt");
    assert!(
        !output.status.success(),
        "--llm real cannot pre-declare the universe"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("mock proposals path") || stderr.contains("cannot"),
        "guard message must explain the mock-path requirement: {stderr}"
    );
}

/// Default (bayraksız) koşunun zarfında fence anahtari YOK — bayt-sözleşme
/// (run_envelope golden'ının E2E karşılığı; strict davranış değişmez).
#[test]
fn strict_default_run_envelope_has_no_fence_key() {
    let fx = HarnessFixture::new_with_use_edges();
    let task_path = renewal_task(&fx, &fx.head);
    let proposals_path = fx.write_proposals_v2(&fx.head, "main.rs", "a.rs");
    let attempt_out = fx.work_path().join("attempt-strict.json");
    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "harness"])
        .args(["--witness", "harness-auto-approve"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--out")
        .arg(&attempt_out)
        .output()
        .expect("run osp attempt");
    assert!(
        output.status.success(),
        "strict happy path must stay green. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope = read_json(&attempt_out);
    assert!(
        envelope["result"].get("fence").is_none(),
        "default runs must not carry a fence record: {}",
        envelope["result"]
    );
}

/// Kirli-beyanlı askı kur (production witness) → (fixture, askı artifact'ı,
/// claim_id). Askıyı üreten koşu `--out` kullanmaz; kanonik zarf state-dir'dedir.
fn declared_dirty_suspension() -> (HarnessFixture, PathBuf, u64) {
    let fx = HarnessFixture::new_with_use_edges();
    let head = commit_third_module(&fx);
    let task_path = renewal_task(&fx, &head);
    let proposals_path = write_v3_declared_proposals(&fx, &head);
    dirty_renewal_state(&fx);

    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "production"])
        .args(["--witness", "production"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .arg("--allow-declared-dirty")
        .output()
        .expect("run osp attempt");
    assert_eq!(
        output.status.code(),
        Some(10),
        "declared-dirty suspension exit 10. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let pending_dir = fx.work_path().join(".osp").join("pending-authorizations");
    let mut artifacts: Vec<PathBuf> = std::fs::read_dir(&pending_dir)
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
    let claim_id = read_json(&artifact)["record"]["claim_id"]
        .as_u64()
        .expect("claim_id") as u64;
    (fx, artifact, claim_id)
}

/// 2× MergeCommit kanıtıyla resume koş (operator-asserted).
fn run_resume_with_two_witnesses(
    fx: &HarnessFixture,
    artifact: &Path,
    claim_id: u64,
) -> std::process::Output {
    let events: Vec<serde_json::Value> = [200u64, 300u64]
        .iter()
        .enumerate()
        .map(|(i, actor)| {
            serde_json::json!({
                "id": (i + 1) as u64,
                "source": format!("resume-evidence-{i}"),
                "witness_kind": "MergeCommit",
                "actor": actor,
                "claim": claim_id,
            })
        })
        .collect();
    let evidence = fx.work_path().join(format!("witnesses-{claim_id}.json"));
    std::fs::write(&evidence, serde_json::to_string_pretty(&events).unwrap()).unwrap();

    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.work_path())
        .arg("trajectory")
        .arg("resume")
        .arg(artifact)
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--witness-evidence")
        .arg(&evidence)
        .arg("--state-dir")
        .arg(fx.work_path())
        .args(["--format", "json"])
        .args(["--evidence-trust", "operator-asserted"])
        .output()
        .expect("run osp resume")
}

/// Kirli-beyanlı askı → tanık → AYNI kirli worktree üzerinde resume Applied
/// (tur-2 P0: stash ritüeli resume'a taşınmaz; politika originating zarftan
/// devralınır — resume'a bayrak YOK).
#[test]
fn declared_dirty_witnessed_resume_applies_on_dirty_worktree() {
    let (fx, artifact, claim_id) = declared_dirty_suspension();
    let resume_output = run_resume_with_two_witnesses(&fx, &artifact, claim_id);
    let stderr = String::from_utf8_lossy(&resume_output.stderr);
    assert_eq!(
        resume_output.status.code(),
        Some(0),
        "declared-dirty resume must apply on the SAME dirty worktree. stderr={stderr}"
    );
    assert!(
        stderr.contains("declared-dirty resume"),
        "inheritance line must be visible: {stderr}"
    );
}

/// **Re-review P1-2:** originating fence kaydının ölçüm digest'i bozulmuşsa
/// (biçimsel olarak geçerli 64-hex kalsa bile) resume RED — kayıt provenans
/// iddiasını DOĞRULATMAK zorundadır:
/// `Digest(ResumedBaseline) == Digest(OriginatingMeasurement)`.
#[test]
fn resume_rejects_tampered_originating_measurement_digest() {
    let (fx, artifact, claim_id) = declared_dirty_suspension();

    // Kanonik zarfı bul ve fence ölçüm digest'ini BAŞKA bir geçerli 64-hex'le
    // değiştir (shape + typed fence kontrolleri hâlâ geçer — yalnız özünde
    // yanlış olan provenans iddiası kalır).
    let attempts_dir = fx.work_path().join("attempts");
    let mut envelopes: Vec<PathBuf> = std::fs::read_dir(&attempts_dir)
        .expect("attempts dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("task-7-"))
                .unwrap_or(false)
        })
        .collect();
    envelopes.sort();
    assert_eq!(envelopes.len(), 1, "tam bir kanonik zarf: {envelopes:?}");
    let envelope_path = envelopes.pop().unwrap();
    let mut envelope = read_json(&envelope_path);
    assert_eq!(
        envelope["result"]["fence"]["mode"], "declared_dirty",
        "{envelope}"
    );
    envelope["result"]["fence"]["measurement"]["space_digest"] = serde_json::json!("ff".repeat(32));
    std::fs::write(
        &envelope_path,
        serde_json::to_string_pretty(&envelope).unwrap(),
    )
    .unwrap();

    let resume_output = run_resume_with_two_witnesses(&fx, &artifact, claim_id);
    assert_eq!(
        resume_output.status.code(),
        Some(15),
        "tampered originating measurement digest must refuse the resume"
    );
    let stderr = String::from_utf8_lossy(&resume_output.stderr);
    assert!(
        stderr.contains("does not describe this space"),
        "digest-parity refusal must be explicit: {stderr}"
    );
}

/// Kanonik originating zarfı olmayan askı resume'lanamaz (bilinçli migration).
/// Disk-son-durumu simülasyonu: fence-RED'li koşunun bıraktığı durum = pending
/// var, kanonik zarf yok (persist asla yazılmadı) — deterministik eşdeğer.
#[test]
fn resume_without_canonical_origin_is_rejected() {
    let fx = HarnessFixture::new_with_use_edges();
    let task_path = fx.write_task(&task_envelope_v2(&fx.head, "main.rs"));
    let proposals_path = fx.write_proposals_v2(&fx.head, "main.rs", "a.rs");

    // Strict askı (temiz ağaç) — kanonik zarf YAZILIR (fence'ler geçti).
    let output = attempt_cmd(&fx)
        .args(["--execution-mode", "production"])
        .args(["--witness", "production"])
        .arg("--proposals")
        .arg(&proposals_path)
        .arg("--task")
        .arg(&task_path)
        .output()
        .expect("run osp attempt");
    assert_eq!(output.status.code(), Some(10), "strict suspension");

    // Fence-RED'li koşunun disk-son-durumu: kanonik zarfı sil.
    let attempts_dir = fx.work_path().join("attempts");
    assert!(attempts_dir.is_dir(), "canonical attempts store exists");
    for entry in std::fs::read_dir(&attempts_dir).unwrap().flatten() {
        std::fs::remove_file(entry.path()).unwrap();
    }

    let pending_dir = fx.work_path().join(".osp").join("pending-authorizations");
    let mut artifacts: Vec<PathBuf> = std::fs::read_dir(&pending_dir)
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
    let artifact = artifacts.pop().expect("pending artifact");
    let claim_id = read_json(&artifact)["record"]["claim_id"].as_u64().unwrap() as u64;

    let events = serde_json::json!([{
        "id": 1u64, "source": "s", "witness_kind": "MergeCommit",
        "actor": 42u64, "claim": claim_id
    }]);
    let evidence = fx.work_path().join("witnesses.json");
    std::fs::write(&evidence, serde_json::to_string_pretty(&events).unwrap()).unwrap();

    let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let resume_output = Command::cargo_bin("osp")
        .expect("osp binary")
        .current_dir(fx.work_path())
        .arg("trajectory")
        .arg("resume")
        .arg(&artifact)
        .arg("--repo")
        .arg(fx.repo_path())
        .arg("--witness-evidence")
        .arg(&evidence)
        .arg("--state-dir")
        .arg(fx.work_path())
        .args(["--format", "json"])
        .args(["--evidence-trust", "operator-asserted"])
        .output()
        .expect("run osp resume");
    assert_eq!(
        resume_output.status.code(),
        Some(15),
        "resume without a canonical originating envelope must refuse (exit 15)"
    );
    let stderr = String::from_utf8_lossy(&resume_output.stderr);
    assert!(
        stderr.contains("re-run the attempt"),
        "migration refusal must direct to re-run: {stderr}"
    );
}
