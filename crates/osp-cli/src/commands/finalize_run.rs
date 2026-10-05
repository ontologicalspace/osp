//! #172: `osp finalize-run` — run dizininden ledger satırı taslağı.
//!
//! Run ritüelinin son elle-adımı olan ledger satırı doldurmasını motora taşır:
//! `dogfood/runs/<run>/` altındaki artifact'lardan makine-alanları dolu bir
//! `live-ledger-v1` satırı üretir (ref'ler + sha256 digest'ler + head bağları +
//! `osp_revision`/`osp_version`). Elle SHA transcription, stdout'tan ayıklama ve
//! elle digest sınıfları ölür.
//!
//! **K4 — yorum sözleşmesi (tur-1 P1-4 düzeltmesiyle):** motor yalnız
//! İSPATLANABİLİR alanları doldurur ve doldurulmamış alan **null**'dır (null =
//! "henüz insan tarafından değerlendirilmedi"; `[]` = "değerlendirildi, friction
//! yok" — başka bir epistemik durum). `decision` / `patch_outcome` /
//! `decision_utility` / `counterfactual` / `human_override` / `friction` /
//! `notes` / `commands` / `scip_index_digest` null kalır — insan doldurur ve
//! satırı ledger'a APPEND eder. `build_verification` bütünüyle null'dır
//! (tur-1 P1-1): `Exists(after) ∧ Exists(patch)`'den `Patch(S₀)=S_after`
//! ÇIKMAZ — verified_state ancak gerçek yama-uygulama ilişkisi doğrulanırsa
//! doldurulabilir (takibi issue'da); motor yalnız `after_ref` + `patch_digest`
//! artifact alanlarını taşır.
//!
//! **K2 (tur-1 P0-2 ile sıkılaştırıldı) — çapraz-artifact kimlik:** `attempt.json`
//! yalnız #166 run envelope olabilir ve SIKI parse edilir (zorunlu alanlar:
//! `run.task_id` / `execution_mode` / `witness_mode` / `task_source` /
//! `repository_head` + `execution_measurement` / `result` / `evidence`).
//! Assert zinciri: `task.id == attempt.run.task_id`, `task.head == baseline.head
//! == attempt.head`, `attempt.run.task_source == harness_task_file`,
//! `proposals.repository_head == baseline.head` (v1 çıplak array state'e
//! bağlanamaz → reddedilir). Elle tutulan defterin yanlış artifact eşleşme
//! sınıfı burada ölür.
//!
//! **P0-3 — `--out` girdi koruması:** çıktı, tüketilen artifact'lardan herhangi
//! birinin alias'ı olamaz (digest'i hesaplanan dosyayı overwrite etmek ref↔digest
//! tutarsızlığı üretir); yazım unique-temp + rename atomic publish ile yapılır.

use std::path::{Path, PathBuf};

use clap::Args;
use sha2::{Digest, Sha256};

/// `osp finalize-run` — run dizininden ledger satırı taslağı.
#[derive(Args, Debug)]
pub struct FinalizeRunArgs {
    /// Run dizini (`dogfood/runs/<run_id>/`). task.json + baseline.json +
    /// attempt.json zorunlu; proposals.json / applied.patch / after.json /
    /// neighborhood.json opsiyonel (varlıklarına göre alanlar dolar).
    pub run_dir: PathBuf,
    /// run_id (default: run dizininin baz adı).
    #[arg(long)]
    pub run_id: Option<String>,
    /// Mantıksal repo etiketi (ledger `repository` alanı; "nexus" gibi). İnsan etiketi —
    /// verilmezse null kalır.
    #[arg(long)]
    pub repository: Option<String>,
    /// Yazılacak çıktı dosyası (default: stdout). Tüketilen run artifact'larının
    /// kendisi olamaz (alias fence — P0-3).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// #166 run envelope'unun finalize-run tarafındaki SIKI okuma şekli (P0-2;
/// tur-2 presence-şekil + tur-3 canonical-semantik): alan VARLIĞI yetmez —
/// iç şekiller TİPLİ, karar alanları CORE TİPLERİNDEN doğrulanır. `evidence[]`
/// doğrudan `osp_core::trajectory::TrajectoryEvidence` olarak deserialize
/// edilir (tur-3 P0): `GateDecision`/`PredicateCompletion`/`MutationDecision`
/// KAPALI enum'lardır ve üyelik serde tarafından TEK truth source'tan
/// doğrulanır — "Vibes"/"Maybe"/"ShipIt" gibi canonical producer'ın
/// üretemeyeceği değerler burada düşer. `deny_unknown_fields` bilinçli YOK:
/// envelope evrimi (additive alanlar) missing≡null felsefesiyle uyumlu kalsın.
#[derive(Debug, serde::Deserialize)]
struct AttemptEnvelopeRead {
    schema_version: u32,
    run: AttemptRunRead,
    execution_measurement: ExecutionMeasurementRead,
    result: AttemptResultRead,
    evidence: Vec<osp_core::trajectory::TrajectoryEvidence>,
}

#[derive(Debug, serde::Deserialize)]
struct AttemptRunRead {
    task_id: u64,
    execution_mode: String,
    witness_mode: String,
    task_source: String,
    repository_head: String,
}

/// #96 MD-2 iki-eksen authority vocabulary — üyelik doğrulaması (exact değer
/// pinlemek değil; bilinen kanonik sözlük dışı değer red).
#[derive(Debug, serde::Deserialize)]
struct ExecutionMeasurementRead {
    subject_authority: String,
    provenance_authority: String,
    /// Şekil-doğrulama amaçlı (bool tipi serde'de zorlanır).
    #[allow(dead_code)]
    provenance_native: bool,
}

/// `CliRunResult` mirror — `kind` üyeliği + `attempts` sayısı.
#[derive(Debug, serde::Deserialize)]
struct AttemptResultRead {
    kind: String,
    #[allow(dead_code)]
    attempts: u64,
}

/// `CliRunResultKind` snake_case wire kümesi (`run_envelope.rs` ile aynı üyeler).
const RESULT_KINDS: &[&str] = &[
    "completed",
    "awaiting_witnesses",
    "exceeded_maneuver_limit",
    "requires_revision",
    "requires_operator_approval",
    "awaiting_cold_start_approval",
    "task_not_found",
    "witness_evaluation_error",
    "pending_authorization_persistence_failure",
    "system_failure",
    "llm_error",
];

/// #96 MD-2 authority vocabulary (tarihsel değerler dahil — üyelik seti).
const SUBJECT_AUTHORITIES: &[&str] = &["task_scope", "affected_nodes"];
const PROVENANCE_AUTHORITIES: &[&str] = &["engine_native_per_axis", "legacy_projected_v1"];

pub fn run_finalize_run(args: FinalizeRunArgs) -> anyhow::Result<()> {
    anyhow::ensure!(
        args.run_dir.is_dir(),
        "run dir {} is not a directory",
        args.run_dir.display()
    );

    // Zorunlu artifact'lar — yoksa isimleriyle red (fail-closed).
    let task_path = args.run_dir.join("task.json");
    let baseline_path = args.run_dir.join("baseline.json");
    let attempt_path = args.run_dir.join("attempt.json");
    for path in [&task_path, &baseline_path, &attempt_path] {
        anyhow::ensure!(
            path.is_file(),
            "required run artifact missing: {} — finalize-run consumes the run dir \
             as the ritual wrote it",
            path.display()
        );
    }

    // P0-3: --out, tüketilen artifact'lardan birinin alias'ı olamaz.
    if let Some(out) = &args.out {
        reject_out_aliasing_consumed_artifacts(out, &args.run_dir)?;
    }

    let (baseline_head, files_with_scip) = read_baseline(&baseline_path)?;

    // K2: task zarfı (v1/v2) — head + id çıkar; başkasının task'ı geçemez.
    let (task_head, task_id) = read_task(&task_path)?;
    anyhow::ensure!(
        task_head == baseline_head,
        "head fence: task.json binds {task_head} but baseline.json measured {baseline_head} — \
         the task was drafted on a different state"
    );

    // K2: attempt — yalnız #166 run envelope, SIKI şekil.
    let attempt_raw = read_json(&attempt_path)?;
    anyhow::ensure!(
        attempt_raw.is_object(),
        "attempt.json must be the #166 run envelope (`--out` copy of the canonical \
         attempt artifact), found {} — hand-assembled legacy artifacts (stdout \
         scraping) are exactly the friction this command absorbs; re-run the \
         attempt with --out",
        shape_name(&attempt_raw)
    );
    let attempt: AttemptEnvelopeRead = serde_json::from_value(attempt_raw).map_err(|e| {
        anyhow::anyhow!(
            "attempt.json does not match the #166 run envelope shape (missing or \
             mistyped required fields in run/execution_measurement/result/evidence[]): {e}"
        )
    })?;
    anyhow::ensure!(
        attempt.schema_version == 1,
        "attempt.json schema_version must be 1 (#166 run envelope), found {}",
        attempt.schema_version
    );
    anyhow::ensure!(
        RESULT_KINDS.contains(&attempt.result.kind.as_str()),
        "attempt.json result.kind {:?} is not a canonical CliRunResultKind wire value",
        attempt.result.kind
    );
    anyhow::ensure!(
        SUBJECT_AUTHORITIES.contains(&attempt.execution_measurement.subject_authority.as_str()),
        "attempt.json execution_measurement.subject_authority {:?} is not in the \
         #96 MD-2 vocabulary",
        attempt.execution_measurement.subject_authority
    );
    anyhow::ensure!(
        PROVENANCE_AUTHORITIES
            .contains(&attempt.execution_measurement.provenance_authority.as_str()),
        "attempt.json execution_measurement.provenance_authority {:?} is not in the \
         #96 MD-2 vocabulary",
        attempt.execution_measurement.provenance_authority
    );
    anyhow::ensure!(
        attempt.run.execution_mode == "harness" || attempt.run.execution_mode == "production",
        "attempt.json run.execution_mode has an unknown value {:?}",
        attempt.run.execution_mode
    );
    anyhow::ensure!(
        attempt.run.witness_mode == "harness_auto_approve"
            || attempt.run.witness_mode == "production",
        "attempt.json run.witness_mode has an unknown value {:?}",
        attempt.run.witness_mode
    );
    // Tur-3 P0: producer'ın kendi guard'ı mirror'lanır (validate_execution_
    // witness_combination) — üyelikler ayrı ayrı geçse bile production +
    // harness_auto_approve kombinasyonu canonical producer tarafından üretilmez.
    anyhow::ensure!(
        !(attempt.run.witness_mode == "harness_auto_approve"
            && attempt.run.execution_mode != "harness"),
        "attempt.json run mode combination is not canonical: witness \
         harness_auto_approve requires execution harness (Paper 2 scoped \
         relaxation — the producer guard rejects this combination at emit time)"
    );
    anyhow::ensure!(
        attempt.run.task_source == "harness_task_file",
        "attempt.json run.task_source = {:?} — finalize-run consumes the \
         task-file ritual; a legacy_hardcoded attempt has no task.json to bind",
        attempt.run.task_source
    );
    anyhow::ensure!(
        attempt.run.task_id == task_id,
        "task-id fence: task.json carries id {task_id} but attempt.json ran \
         task_id {} — mismatched artifacts cannot share one ledger row",
        attempt.run.task_id
    );
    // Tur-3 P0: run envelope'un KENDİ evidence'ı run task'ına bağlanır —
    // task.json↔run bağını geçen bir artifact, içine yabancı task'ın
    // evidence'ını taşıyabilir.
    for evidence in &attempt.evidence {
        anyhow::ensure!(
            evidence.task_id == attempt.run.task_id,
            "evidence fence: evidence entry carries task_id {} but the run is \
             task_id {} — foreign evidence cannot share this envelope",
            evidence.task_id,
            attempt.run.task_id
        );
    }
    anyhow::ensure!(
        attempt.run.repository_head == baseline_head,
        "head fence: attempt.json ran on {} but baseline.json measured \
         {baseline_head} — cross-state ledger row refused",
        attempt.run.repository_head
    );

    // Opsiyonel artifact'lar — varlıklarına göre alanlar dolar (yoksa null, dürüst boşluk).
    let proposals_path = args.run_dir.join("proposals.json");
    let patch_path = args.run_dir.join("applied.patch");
    let after_path = args.run_dir.join("after.json");
    let neighborhood_path = args.run_dir.join("neighborhood.json");

    // K2 (tur-2 P0): proposals varsa v2 zarfı TİPLİ parse ile doğrulanır —
    // `CliPathKeyedProposalsFileV2` zaten `Deserialize + deny_unknown_fields`
    // taşıyor; `{"schema_version":2,"repository_head":…}` gibi presence-only
    // sahte zarflar (proposals alanı eksik / unknown alan) serde'de düşer.
    // v1 çıplak array state'e bağlanamaz (repository_head fence'i v2'ye özgü).
    if proposals_path.is_file() {
        let proposals_raw = read_json(&proposals_path)?;
        anyhow::ensure!(
            proposals_raw.is_object(),
            "proposals.json is a bare JSON array (v1, node-id keyed) — it cannot be \
             state-bound; finalize-run requires the v2 envelope whose \
             repository_head fence ties it to the baseline state"
        );
        let proposals: crate::commands::path_keyed_proposals::CliPathKeyedProposalsFileV2 =
            serde_json::from_value(proposals_raw).map_err(|e| {
                anyhow::anyhow!("proposals.json does not match the v2 envelope shape: {e}")
            })?;
        anyhow::ensure!(
            proposals.schema_version == 2,
            "proposals.json envelope requires schema_version 2 (found {})",
            proposals.schema_version
        );
        anyhow::ensure!(
            proposals.repository_head == baseline_head,
            "head fence: proposals.json was produced on {} but baseline.json \
             measured {baseline_head} — mismatched artifacts cannot share one \
             ledger row",
            proposals.repository_head
        );
    }

    let task_digest = sha256_file(&task_path)?;
    let proposal_digest = if proposals_path.is_file() {
        Some(sha256_file(&proposals_path)?)
    } else {
        None
    };
    let patch_digest = if patch_path.is_file() {
        Some(sha256_file(&patch_path)?)
    } else {
        None
    };
    // P1-1: after yalnızca şekil-validasyonuyla taşınır (after_ref) — head'inden
    // "verified" türetmesi YOK.
    if after_path.is_file() {
        read_baseline(&after_path)?;
    }

    let run_id = args
        .run_id
        .clone()
        .or_else(|| {
            args.run_dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .ok_or_else(|| anyhow::anyhow!("cannot derive run_id from {}", args.run_dir.display()))?;

    let mut row = serde_json::json!({
        "schema_version": "live-ledger-v1",
        "run_id": run_id,
        "contract_version": "v1.1",
        "repository": args.repository,
        "repository_head": baseline_head,
        // K7: build-time gömülü OSP checkout HEAD'i (build.rs) — runtime cwd bağımsız.
        "osp_revision": env!("OSP_GIT_REVISION"),
        "osp_version": env!("CARGO_PKG_VERSION"),
        "analysis_profile": if files_with_scip > 0 { "tier2-scip" } else { "tier1" },
        "commands": null,
        "scip_index_digest": null,
        "task_ref": make_ref(&args.run_dir, "task.json"),
        "task_digest": task_digest,
        "baseline_ref": make_ref(&args.run_dir, "baseline.json"),
        "proposal_refs": if proposals_path.is_file() {
            serde_json::json!([make_ref(&args.run_dir, "proposals.json")])
        } else {
            serde_json::Value::Null
        },
        "proposal_digest": proposal_digest,
        "attempt_ref": make_ref(&args.run_dir, "attempt.json"),
        "decision": null,
        "patch_outcome": null,
        "patch_ref": if patch_path.is_file() {
            serde_json::json!(make_ref(&args.run_dir, "applied.patch"))
        } else {
            serde_json::Value::Null
        },
        "patch_digest": patch_digest,
        "after_ref": if after_path.is_file() {
            serde_json::json!(make_ref(&args.run_dir, "after.json"))
        } else {
            serde_json::Value::Null
        },
        // P1-1: verified_state iddiası kaldırıldı — Exists(after)+Exists(patch)
        // ≠ Patch(S0)=S_after; insan (veya gelecekte gerçek yama-doğrulama) doldurur.
        "build_verification": null,
        "decision_utility": null,
        "counterfactual": null,
        "human_override": null,
        "friction": null,
        "notes": null
    });
    if neighborhood_path.is_file() {
        row["neighborhood_ref"] = serde_json::json!(make_ref(&args.run_dir, "neighborhood.json"));
    }

    let json = serde_json::to_string_pretty(&row)?;
    match &args.out {
        Some(path) => {
            crate::commands::atomic_write_replace(path, json.as_bytes())?;
            println!("✓ ledger draft written to {}", path.display());
        }
        None => println!("{json}"),
    }
    eprintln!(
        "human fields left null by design (K4): decision, patch_outcome, \
         decision_utility, counterfactual, human_override, friction, notes, \
         commands, scip_index_digest, build_verification — fill and APPEND to \
         the ledger"
    );
    Ok(())
}

/// P0-3: `--out` tüketilen herhangi bir run artifact'ının alias'ıysa red.
///
/// Digest'ler okunan baytlar üzerinden hesaplanır; çıktının aynı dosyayı
/// overwrite etmesi `ref → başka baytlar / digest → eski baytlar` üretir
/// (#174'in kapattığı arka kapı sınıfı).
fn reject_out_aliasing_consumed_artifacts(out: &Path, run_dir: &Path) -> anyhow::Result<()> {
    let consumed = [
        "task.json",
        "baseline.json",
        "attempt.json",
        "proposals.json",
        "applied.patch",
        "after.json",
        "neighborhood.json",
    ];
    let out_canon = crate::commands::canon_path(out);
    for name in consumed {
        let candidate = run_dir.join(name);
        if candidate.is_file() {
            let candidate_canon = crate::commands::canon_path(&candidate);
            anyhow::ensure!(
                candidate_canon != out_canon,
                "--out {} would overwrite the consumed run artifact {name} — \
                 its digest was computed over the bytes you are about to replace; \
                 write the ledger draft to a new file (e.g. <run>/ledger-draft.json)",
                out.display()
            );
        }
    }
    Ok(())
}

/// Baseline/after zarfından (head, scip kapsamı) oku — canlı-fence YOK
/// (finalize-run ritüelin sonunda koşar; repo çoktan promote edilmiş olabilir).
fn read_baseline(path: &Path) -> anyhow::Result<(String, u64)> {
    let value = read_json(path)?;
    let found = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    anyhow::ensure!(
        found == 2,
        "{} requires schema_version 2 (osp analyze envelope), found {found}",
        path.display()
    );
    let head = value
        .get("repository")
        .and_then(|r| r.get("head"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} is missing repository.head", path.display()))?
        .to_string();
    crate::commands::repo_snapshot::GitCommitId::try_from(head.clone())
        .map_err(|e| anyhow::anyhow!("{} repository.head invalid: {e}", path.display()))?;
    let files_with_scip = value
        .get("semantic_coverage")
        .and_then(|c| c.get("files_with_scip"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Ok((head, files_with_scip))
}

/// Task zarfı → (repository_head, task.id). v1 ve v2 zarfının ikisinde de
/// `repository_head` üst-düzey, `task.id` gövdede taşınır (P0-2).
fn read_task(path: &Path) -> anyhow::Result<(String, u64)> {
    let value = read_json(path)?;
    let version = value.get("schema_version").and_then(|v| v.as_u64());
    anyhow::ensure!(
        version == Some(1) || version == Some(2),
        "{} has unsupported task schema_version {version:?} (expected 1 or 2)",
        path.display()
    );
    let head = value
        .get("repository_head")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} is missing repository_head", path.display()))?
        .to_string();
    let task_id = value
        .get("task")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} is missing task.id — the cross-artifact id fence needs it",
                path.display()
            )
        })?;
    Ok((head, task_id))
}

fn read_json(path: &Path) -> anyhow::Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))
}

fn shape_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Array(_) => "a JSON array (legacy hand-assembled attempt list)",
        serde_json::Value::Object(_) => "a JSON object",
        _ => "a non-object JSON value",
    }
}

/// K3: sha256 over ham dosya baytları — `sha256:<64 hex>` (tam uzunluk; mevcut
/// 16-hex ledger değerleri el-dönemi annotasyondur, missing≡null uyumu bozulmaz).
fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("failed to read {} for digest: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

/// Ledger ref'i: run-dir KULLANILDIĞI GİBİ (ritüelde OSP kökünden göreli) +
/// dosya adı, ileri-slash normalize (platform-bağımsız deterministik satır).
fn make_ref(run_dir: &Path, file: &str) -> String {
    let mut dir = run_dir.to_string_lossy().replace('\\', "/");
    if !dir.ends_with('/') {
        dir.push('/');
    }
    format!("{dir}{file}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_file_pins_known_vector() {
        // sha256("abc") — bilinen standart vektör; tam 64-hex biçim K3.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn make_ref_normalizes_separators_without_double_slash() {
        assert_eq!(
            make_ref(Path::new("dogfood/runs/r1"), "task.json"),
            "dogfood/runs/r1/task.json"
        );
        assert_eq!(
            make_ref(Path::new("dogfood\\runs\\r1\\"), "task.json"),
            "dogfood/runs/r1/task.json"
        );
        assert_eq!(
            make_ref(Path::new("dogfood/runs/r1/"), "task.json"),
            "dogfood/runs/r1/task.json"
        );
    }

    #[test]
    fn baseline_reader_rejects_v1_and_missing_head() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = dir.path().join("v1.json");
        std::fs::write(&v1, r#"{"schema_version": 1, "repository": {"head": "x"}}"#).unwrap();
        assert!(read_baseline(&v1).is_err());

        let nohead = dir.path().join("nohead.json");
        std::fs::write(&nohead, r#"{"schema_version": 2, "repository": {}}"#).unwrap();
        assert!(read_baseline(&nohead).is_err());

        let ok = dir.path().join("ok.json");
        std::fs::write(
            &ok,
            format!(
                r#"{{"schema_version": 2, "repository": {{"head": "{}"}}, "semantic_coverage": {{"files_with_scip": 3}}}}"#,
                "a".repeat(40)
            ),
        )
        .unwrap();
        assert_eq!(read_baseline(&ok).unwrap(), ("a".repeat(40), 3));
    }

    /// P0-2: minimal sahte envelope (yalnız head taşıyan) SIKI şekilden geçemez.
    #[test]
    fn minimal_attempt_object_rejected_by_strict_shape() {
        let minimal = serde_json::json!({
            "schema_version": 1,
            "run": {"repository_head": "a".repeat(40)}
        });
        let err = serde_json::from_value::<AttemptEnvelopeRead>(minimal).unwrap_err();
        assert!(err.to_string().contains("task_id"), "{err}");
    }

    /// P0-2: read_task head + id çıkarır; eksik id red.
    #[test]
    fn task_reader_requires_head_and_id() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("t2.json");
        std::fs::write(
            &ok,
            serde_json::json!({
                "schema_version": 2,
                "repository_head": "a".repeat(40),
                "scope_bindings": [{"path": "m.rs"}],
                "task": {"id": 16}
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(read_task(&ok).unwrap(), ("a".repeat(40), 16));

        let no_id = dir.path().join("t_no_id.json");
        std::fs::write(
            &no_id,
            serde_json::json!({
                "schema_version": 2,
                "repository_head": "a".repeat(40),
                "task": {}
            })
            .to_string(),
        )
        .unwrap();
        assert!(read_task(&no_id).is_err());
    }

    /// P0-3: --out alias fence — tüketilen artifact üstüne yazım reddi.
    #[test]
    fn out_aliasing_consumed_artifact_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path().join("run");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("task.json"), "{}").unwrap();
        let err = reject_out_aliasing_consumed_artifacts(&run.join("task.json"), &run).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("overwrite the consumed run artifact"), "{msg}");
        assert!(msg.contains("ledger-draft.json"), "{msg}");
        // Yeni dosya adı serbest.
        reject_out_aliasing_consumed_artifacts(&run.join("ledger-draft.json"), &run).unwrap();
    }
}
