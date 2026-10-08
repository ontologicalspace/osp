//! #196 uygulama-2 — `osp realization-gate`: INV-T10 RealizationGate'ın CLI
//! ritüeli (karar 2: gate konumu = finalize/başvuru anı).
//!
//! #171 D5a'nın bulgusunun protokolleşmesi: **graph predicate success ⇏
//! build-valid declared realization** (B: E_c = 0 iken build çöktü). Gate,
//! attempt'in graph-katmanı tamamlamasını source gerçekliğine bağlar:
//!
//! ```text
//! attempt.json (canonical-anchor doğrulamalı — #178/#188 trust kökü)
//!   + task.json predicate (Coupling/Le threshold + scope.Path)
//!   + after.json (reanalysis: gözlemlenen coupling — motor türetimi)
//!   + --evidence (insan-beyanlı: patch_created/parse/build/tests)
//!         → evaluate_gate → realization-verdict.json (run dizinine, no-clobber)
//! ```
//!
//! **D5a şemasının motorlaşması:** `c_observed` (after.json), `c_predicted`
//! (zarf evidence'ının sim öngörüsü) ve `predicate_after_reanalysis` (observed
//! ≤ threshold + tolerance) MAKİNE türetilir — insan yalnız patch/parse/build/
//! test gerçelerini beyan eder. E_c imzalı residual kanıtta yaşar (karar 1 v1:
//! sayısal D_G eşiği bilinçle YOK — eşik verisi birikene dek uydurulmaz).
//!
//! Exit kodları betiklenebilir: 0 = RealizedCompleted, 2 = BuildInvalid,
//! 3 = PredicateUnsatisfied, 4 = NotAttempted.

use std::path::{Path, PathBuf};

use clap::Args;
use osp_core::realization::{
    evaluate_gate, BuildOutcome, CompletionBasis, GraphCompletionFacts, RealizationEvidence,
    RealizationVerdict, TestOutcome,
};

/// `osp realization-gate` — INV-T10 kapısı.
#[derive(Args, Debug)]
pub struct RealizationGateArgs {
    /// Run dizini (`dogfood/runs/<run>/`): attempt.json + task.json + after.json
    /// zorunlu.
    pub run_dir: PathBuf,
    /// #178/#188 trust anchor: kanonik zarf mağazası. Verilmezse ritüel düzeni
    /// denenir (`<run_dir>/../../state`).
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
    /// İnsan-beyanlı realization gerçeleri (JSON, deny_unknown_fields):
    /// `{ "patch_created": bool, "parse": bool, "build": {"Succeeded": ...} |
    /// {"Failed": {"error_count": n}}, "tests": {"passed":n,"failed":n,"skipped":n} | null }`
    /// — makine-türetilen alanlar (c_observed/c_predicted/predicate_after_reanalysis)
    /// verilmez; gate üretir.
    #[arg(long)]
    pub evidence: PathBuf,
}

/// İnsan-beyanlı kanıt girdisi (D5a şemasının insan tarafı).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredRealizationInput {
    patch_created: bool,
    parse: bool,
    build: CliBuild,
    tests: Option<TestOutcome>,
}

/// `build` alanının düz wire okuması: `{"outcome":"succeeded"}` veya
/// `{"outcome":"failed","error_count":n}`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CliBuild {
    outcome: String,
    error_count: Option<u64>,
}

impl TryFrom<CliBuild> for BuildOutcome {
    type Error = String;

    fn try_from(value: CliBuild) -> Result<Self, Self::Error> {
        match value.outcome.as_str() {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed {
                error_count: value
                    .error_count
                    .ok_or_else(|| "failed build requires error_count".to_string())?,
            }),
            other => Err(format!(
                "unknown build outcome {other:?} (succeeded | failed)"
            )),
        }
    }
}

pub fn run_realization_gate(args: RealizationGateArgs) -> anyhow::Result<()> {
    let attempt_path = args.run_dir.join("attempt.json");
    let task_path = args.run_dir.join("task.json");
    let after_path = args.run_dir.join("after.json");
    for path in [&attempt_path, &task_path, &after_path, &args.evidence] {
        anyhow::ensure!(
            path.is_file(),
            "realization-gate requires {}: {} — the gate consumes the ritual run dir as written",
            path.file_name().unwrap_or_default().to_string_lossy(),
            path.display()
        );
    }

    // 1) Zarf — read-once baytlar; graph-completion gerçeleri.
    let attempt_bytes = std::fs::read(&attempt_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", attempt_path.display()))?;
    let envelope: serde_json::Value = serde_json::from_slice(&attempt_bytes)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", attempt_path.display()))?;
    let task_id = envelope
        .pointer("/run/task_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("attempt.json is missing run.task_id"))?;
    let result_kind = envelope
        .pointer("/result/kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("attempt.json is missing result.kind"))?
        .to_string();
    let completion_basis = match envelope.get("completion_basis") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(
            serde_json::from_value::<CompletionBasis>(serde_json::Value::String(s.clone()))
                .map_err(|e| anyhow::anyhow!("attempt.json completion_basis invalid: {e}"))?,
        ),
        Some(other) => {
            anyhow::bail!("attempt.json completion_basis must be a string or null, found {other}")
        }
    };
    anyhow::ensure!(
        completion_basis != Some(CompletionBasis::Realized),
        "attempt.json claims completion_basis=realized — unrepresentable at attempt time (INV-T10); only the gate can establish it"
    );

    // 2) Trust anchor: zarf KANONİK mağazayla bayt-özdeş doğrulanır (legacy
    //    downgrade YOK — gate yalnız anchored, graph-completed iddialar üzerinde koşar).
    let envelope_has_digest_fields = envelope
        .pointer("/run/task_digest")
        .map(|v| !v.is_null())
        .unwrap_or(false)
        || envelope
            .pointer("/run/proposals_digest")
            .map(|v| !v.is_null())
            .unwrap_or(false);
    let anchor = crate::commands::finalize_run::verify_attempt_against_canonical_store(
        &args.run_dir,
        &attempt_bytes,
        task_id,
        args.state_dir.as_deref(),
        envelope_has_digest_fields,
        false,
    )?;
    let canonical_attempt_ref = match &anchor {
        crate::commands::finalize_run::AttemptAnchor::Canonical { canonical_ref } => {
            canonical_ref.clone()
        }
        crate::commands::finalize_run::AttemptAnchor::UnanchoredLegacy => {
            anyhow::bail!(
                "realization-gate requires an anchored attempt envelope (canonical store \
                 byte-match) — legacy downgrade is not a gate context"
            );
        }
    };

    // 3) Task predicate (v1: Coupling/Le tek-predicate) + after.json gözlemi —
    //    makine türetimleri.
    let (threshold, tolerance, scope_path) = read_task_predicate(&task_path)?;
    let c_observed = read_observed_coupling(&after_path, &scope_path)?;
    let c_predicted = envelope
        .pointer("/evidence")
        .and_then(|e| e.as_array())
        .and_then(|a| a.last())
        .and_then(|last| last.pointer("/after/x"))
        .and_then(|x| x.as_f64());
    let predicate_after_reanalysis = Some(c_observed <= threshold + tolerance + 1e-12);

    // 4) İnsan-beyanlı gerçeler + kanıt.
    let declared: DeclaredRealizationInput = serde_json::from_reader(
        std::fs::File::open(&args.evidence)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", args.evidence.display()))?,
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "{} does not match the declared-realization input shape: {e}",
            args.evidence.display()
        )
    })?;
    let build = BuildOutcome::try_from(declared.build)
        .map_err(|e| anyhow::anyhow!("{}: {e}", args.evidence.display()))?;
    let evidence = RealizationEvidence {
        patch_created: declared.patch_created,
        parse: declared.parse,
        build,
        tests: declared.tests,
        c_observed: Some(c_observed),
        c_predicted,
        predicate_after_reanalysis,
    };

    // 5) Kapı.
    let facts = GraphCompletionFacts {
        task_id,
        canonical_attempt_ref: canonical_attempt_ref.clone(),
        result_kind,
        completion_basis,
    };
    let e_c = evidence.e_c();
    let verdict = evaluate_gate(facts, evidence);

    // 6) Kanıt artifact'ı — no-clobber (tek yazım; yeniden değerlendirme = yeni kanıt dosyası değil RED).
    let verdict_path = args.run_dir.join("realization-verdict.json");
    anyhow::ensure!(
        !verdict_path.exists(),
        "{} already exists — a realization verdict is written once; re-evaluation requires a fresh run dir",
        verdict_path.display()
    );
    let payload = serde_json::json!({
        "schema_version": 1,
        "kind": "realization-verdict-v1",
        "task_id": task_id,
        "task_ref": make_ref(&args.run_dir, "task.json"),
        "canonical_attempt_ref": canonical_attempt_ref,
        "verdict": serde_json::to_value(&verdict)?,
        "e_c": e_c,
        "gate_context": {
            "threshold": threshold,
            "tolerance": tolerance,
            "scope_path": scope_path,
            "c_observed": c_observed,
            "c_predicted": c_predicted,
        },
    });
    crate::commands::atomic_write_replace(&verdict_path, payload.to_string().as_bytes())?;

    // 7) İnsan yüzü + exit kodu.
    let (label, code) = match &verdict {
        RealizationVerdict::RealizedCompleted { .. } => ("RealizedCompleted", 0),
        RealizationVerdict::DeclaredRealizationBuildInvalid { .. } => {
            ("DeclaredRealizationBuildInvalid", 2)
        }
        RealizationVerdict::PredicateUnsatisfiedAfterReanalysis { .. } => {
            ("PredicateUnsatisfiedAfterReanalysis", 3)
        }
        RealizationVerdict::NotAttempted => ("NotAttempted", 4),
    };
    println!(
        "✓ realization verdict written to {}",
        verdict_path.display()
    );
    println!(
        "verdict = {label} (basis={:?}, build_errors={:?}, E_c={e_c:?})",
        verdict.completion_basis(),
        verdict.build_error_count(),
    );
    std::process::exit(code);
}

/// task.json → (threshold, tolerance, scope.Path) — v1: Coupling/Le tek predicate.
fn read_task_predicate(task_path: &Path) -> anyhow::Result<(f64, f64, String)> {
    let value: serde_json::Value = serde_json::from_reader(std::fs::File::open(task_path)?)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", task_path.display()))?;
    let predicate = value
        .pointer("/task/target_predicate_set/predicates/0/predicate")
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} is missing task.target_predicate_set.predicates[0]",
                task_path.display()
            )
        })?;
    let metric = predicate
        .get("metric")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let operator = predicate
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    anyhow::ensure!(
        metric == "Coupling" && operator == "Le",
        "realization-gate v1 supports a single Coupling/Le predicate (found {metric}/{operator})"
    );
    let threshold = predicate
        .get("threshold")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| anyhow::anyhow!("{} predicate is missing threshold", task_path.display()))?;
    let tolerance = predicate
        .get("tolerance")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let scope_path = predicate
        .pointer("/scope/Path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} predicate scope must be Path-scoped",
                task_path.display()
            )
        })?
        .to_string();
    Ok((threshold, tolerance, scope_path))
}

/// after.json → scope node'unun ölçülmüş coupling'i.
fn read_observed_coupling(after_path: &Path, scope_path: &str) -> anyhow::Result<f64> {
    let value: serde_json::Value = serde_json::from_reader(std::fs::File::open(after_path)?)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", after_path.display()))?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    anyhow::ensure!(
        version == 2,
        "{} requires schema_version 2 (osp analyze envelope), found {version}",
        after_path.display()
    );
    value
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or_else(|| anyhow::anyhow!("{} is missing nodes[]", after_path.display()))?
        .iter()
        .find(|n| n.get("path").and_then(|p| p.as_str()) == Some(scope_path))
        .and_then(|n| n.pointer("/coupling/value"))
        .and_then(|c| c.as_f64())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} has no measured coupling for scope path {scope_path}",
                after_path.display()
            )
        })
}

/// Ledger ref'i üretimi (finalize_run ile aynı normalizasyon).
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
    fn declared_input_parses_build_shapes() {
        let ok: DeclaredRealizationInput =
            serde_json::from_str(r#"{"patch_created":true,"parse":true,"build":{"outcome":"failed","error_count":122},"tests":null}"#)
                .unwrap();
        assert!(matches!(
            BuildOutcome::try_from(ok.build).unwrap(),
            BuildOutcome::Failed { error_count: 122 }
        ));
        let ok2: DeclaredRealizationInput =
            serde_json::from_str(r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"tests":{"passed":271,"failed":0,"skipped":0}}"#)
                .unwrap();
        assert!(matches!(
            BuildOutcome::try_from(ok2.build).unwrap(),
            BuildOutcome::Succeeded
        ));
        // Bilinmeyen outcome / eksik error_count / bilinmeyen alan → red.
        let bad: DeclaredRealizationInput = serde_json::from_str(
            r#"{"patch_created":true,"parse":true,"build":{"outcome":"failed"},"tests":null}"#,
        )
        .unwrap();
        assert!(BuildOutcome::try_from(bad.build).is_err());
        assert!(serde_json::from_str::<DeclaredRealizationInput>(
            r#"{"patch_created":true,"parse":true,"build":{"outcome":"succeeded"},"c_observed":0.5}"#
        )
        .is_err());
    }

    #[test]
    fn task_predicate_reader_requires_coupling_le_path_scope() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("t.json");
        std::fs::write(
            &ok,
            r#"{"task":{"target_predicate_set":{"predicates":[{"predicate":{"metric":"Coupling","operator":"Le","threshold":0.875,"tolerance":0.0,"scope":{"Path":"a.rs"}}}]}}}"#,
        )
        .unwrap();
        assert_eq!(
            read_task_predicate(&ok).unwrap(),
            (0.875, 0.0, "a.rs".to_string())
        );

        let bad = dir.path().join("bad.json");
        std::fs::write(
            &bad,
            r#"{"task":{"target_predicate_set":{"predicates":[{"predicate":{"metric":"Instability","operator":"Le","threshold":0.5,"scope":{"Path":"a.rs"}}}]}}}"#,
        )
        .unwrap();
        assert!(read_task_predicate(&bad).is_err());
    }

    #[test]
    fn observed_coupling_reader_finds_scope_node() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("after.json");
        std::fs::write(
            &p,
            r#"{"schema_version":2,"nodes":[{"path":"x.rs","coupling":{"value":0.1}},{"path":"a.rs","coupling":{"value":0.9167}}]}"#,
        )
        .unwrap();
        assert!((read_observed_coupling(&p, "a.rs").unwrap() - 0.9167).abs() < 1e-12);
        assert!(read_observed_coupling(&p, "yok.rs").is_err());
    }
}
