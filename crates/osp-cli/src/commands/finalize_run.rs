//! #172: `osp finalize-run` — run dizininden ledger satırı taslağı.
//!
//! Run ritüelinin son elle-adımı olan ledger satırı doldurmasını motora taşır:
//! `dogfood/runs/<run>/` altındaki artifact'lardan makine-alanları dolu bir
//! `live-ledger-v1` satırı üretir (ref'ler + sha256 digest'ler + head bağları +
//! `osp_revision`/`osp_version`). Elle SHA transcription, stdout'tan ayıklama ve
//! elle digest sınıfları ölür.
//!
//! **K4 — yorum sözleşmesi:** motor yalnız İSPATLANABİLİR alanları doldurur.
//! `decision` / `patch_outcome` / `decision_utility` / `counterfactual` /
//! `human_override` / `friction` / `notes` / `commands` null kalır — insan
//! doldurur ve satırı ledger'a APPEND eder. Komut dosyaya yazmaz; ürettiği
//! taslaktır (ritüelin epistemik içeriği bilinçli olarak dışarıda).
//!
//! **K2:** `attempt.json` yalnız #166 run envelope (`--out` kopyası) olabilir;
//! stdout'tan elle ayıklanmış legacy listeler fail-closed reddedilir. Üç-head
//! çapraz-fence: task == baseline == attempt aynı ölçüm state'ine bağlıdır.

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
    /// Yazılacak çıktı dosyası (default: stdout).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

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

    let (baseline_head, files_with_scip) = read_baseline(&baseline_path)?;
    let task_head = read_task_head(&task_path)?;
    anyhow::ensure!(
        task_head == baseline_head,
        "head fence: task.json binds {task_head} but baseline.json measured {baseline_head} — \
         the task was drafted on a different state"
    );

    // K2: yalnız #166 run envelope.
    let attempt = read_json(&attempt_path)?;
    anyhow::ensure!(
        attempt.is_object(),
        "attempt.json must be the #166 run envelope (`--out` copy of the canonical \
         attempt artifact), found {} — hand-assembled legacy artifacts (stdout \
         scraping) are exactly the friction this command absorbs; re-run the \
         attempt with --out",
        shape_name(&attempt)
    );
    let attempt_schema = attempt.get("schema_version").and_then(|v| v.as_u64());
    anyhow::ensure!(
        attempt_schema == Some(1),
        "attempt.json schema_version must be 1 (#166 run envelope), found {attempt_schema:?}"
    );
    let attempt_head = attempt
        .get("run")
        .and_then(|r| r.get("repository_head"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("attempt.json envelope is missing run.repository_head"))?
        .to_string();
    anyhow::ensure!(
        attempt_head == baseline_head,
        "head fence: attempt.json ran on {attempt_head} but baseline.json measured \
         {baseline_head} — cross-state ledger row refused"
    );

    // Opsiyonel artifact'lar — varlıklarına göre alanlar dolar (yoksa null, dürüst boşluk).
    let proposals_path = args.run_dir.join("proposals.json");
    let patch_path = args.run_dir.join("applied.patch");
    let after_path = args.run_dir.join("after.json");
    let neighborhood_path = args.run_dir.join("neighborhood.json");

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
    let after_head = if after_path.is_file() {
        Some(read_baseline(&after_path)?.0)
    } else {
        None
    };

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

    // build_verification.verified_state yalnız İKİ kanıtla dolabilir: after ölçümü
    // (hangi head'e promote edildi) + patch digest (hangi yama). Komut/outcome insan.
    let build_verification = match (&after_head, &patch_digest) {
        (Some(head), Some(digest)) => serde_json::json!({
            "command": null,
            "verified_state": {"repository_head": head, "patch_digest": digest},
            "outcome": null
        }),
        _ => serde_json::Value::Null,
    };

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
        "commands": {
            "baseline_analysis": null,
            "attempt": null,
            "after_analysis": null
        },
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
        "build_verification": build_verification,
        "decision_utility": null,
        "counterfactual": null,
        "human_override": null,
        "friction": [],
        "notes": null
    });
    if neighborhood_path.is_file() {
        row["neighborhood_ref"] = serde_json::json!(make_ref(&args.run_dir, "neighborhood.json"));
    }

    let json = serde_json::to_string_pretty(&row)?;
    match &args.out {
        Some(path) => {
            std::fs::write(path, &json)?;
            println!("✓ ledger draft written to {}", path.display());
        }
        None => println!("{json}"),
    }
    eprintln!(
        "human fields left null by design (K4): decision, patch_outcome, \
         decision_utility, counterfactual, human_override, friction, notes, \
         commands, scip_index_digest — fill and APPEND to the ledger"
    );
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

/// Task zarfının repository_head'i (v1 ve v2 zarfının ikisinde de üst-düzey alan).
fn read_task_head(path: &Path) -> anyhow::Result<String> {
    let value = read_json(path)?;
    let head = value
        .get("repository_head")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} is missing repository_head", path.display()))?
        .to_string();
    Ok(head)
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
            format!(r#"{{"schema_version": 2, "repository": {{"head": "{}"}}, "semantic_coverage": {{"files_with_scip": 3}}}}"#, "a".repeat(40)),
        )
        .unwrap();
        assert_eq!(read_baseline(&ok).unwrap(), ("a".repeat(40), 3));
    }
}
