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
    evaluate_gate, BuildOutcome, CompletionBasis, RealizationEvidence, RealizationVerdict,
    TestOutcome,
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
    // **P0-1 (tur-2): verdict KANONİK mağazaya da publish edilir** — run-dir
    // kopyası caller-owned'dır; finalize kanonik nüshayı arayıp bayt-eşleştirir
    // (#178/#188 deseni: elle yazılmış verdict'in canonical provenance'ı yoktur).
    let state_dir: PathBuf = match args.state_dir.as_deref() {
        Some(p) => p.to_path_buf(),
        None => args
            .run_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("state"))
            .filter(|p| p.is_dir())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot resolve the canonical state dir (anchor succeeded; probe broken?)"
                )
            })?,
    };
    // **P1 (tur-2): GraphCompletedProof** — graph-completed OLMAYAN gerçelerden
    // proof kurulamaz; gate bağlamı RED ile belli olur (NotAttempted verdict'i yazılmaz).
    let proof = osp_core::realization::GraphCompletedProof::from_facts(
        task_id,
        canonical_attempt_ref.clone(),
        &result_kind,
        completion_basis,
    )
    .ok_or_else(|| {
        anyhow::anyhow!(
            "realization-gate context: the attempt is not a graph-completed claim \
             (result.kind={result_kind:?}, completion_basis={completion_basis:?}) — the \
             gate only runs on graph-completed claims"
        )
    })?;

    // 3) **Artifact identity (P0 fence ailesi — #198 review):** task/after/evidence
    //    read-once tamponlardan okunur ve verdict artifact'ı tükettiği her şeyin
    //    digest'ini taşır. Çapraz-fence'ler: task.id ↔ run.task_id,
    //    run.task_digest ↔ hash(task.json), task.repository_head ↔ zarf head'i.
    //    after.json'in patch'li-state bağı v1'de digest kaydıyla (post-hoc tamper
    //    reddi) kurulur — daha güçlü cross-fence ayrı çalışmadır.
    let task_bytes = std::fs::read(&task_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", task_path.display()))?;
    let task_digest = crate::commands::finalize_run::sha256_bytes(&task_bytes);
    let (task_head, task_id_from_task) = read_task_identity(&task_bytes, &task_path)?;
    anyhow::ensure!(
        task_id_from_task == task_id,
        "task-id fence: task.json carries id {task_id_from_task} but the attempt ran \
         task_id {task_id} — cross-artifact substitution refused"
    );
    anyhow::ensure!(
        task_head == envelope_run_head(&envelope),
        "head fence: task.json binds {} but the attempt ran on {} — cross-artifact \
         substitution refused",
        task_head,
        envelope_run_head(&envelope)
    );
    // P0: gate yalnız digest TAŞIYAN (post-#178 yeni-şekil) zarflarda koşar —
    // task baytlarına bağlanamayan zarf, substitution'a açıktır (anchor
    // felsefesiyle aynı: yeni-şekil + anchor'suz = RED).
    let claimed_task_digest = envelope
        .pointer("/run/task_digest")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "realization-gate requires a digest-carrying envelope (run.task_digest) — \
                 pre-#178 shapes cannot be byte-bound to their task.json; re-run the attempt"
            )
        })?
        .to_string();
    anyhow::ensure!(
        claimed_task_digest == task_digest,
        "task digest fence: the attempt bound task.json as {claimed_task_digest} but the \
         run dir now hashes {task_digest} — the task file changed after the attempt \
         (cross-artifact substitution refused)"
    );
    let (threshold, tolerance, scope_path) = read_task_predicate(&task_bytes, &task_path)?;
    let after_bytes = std::fs::read(&after_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", after_path.display()))?;
    let after_digest = crate::commands::finalize_run::sha256_bytes(&after_bytes);
    // P0-2 (tur-2 — state identity, zincir-güvenlik kesiti): after.json yalnız
    // snapshot-BAĞLI temiz analiz olabilir (binding=clean_pre_post_equal ∧ clean)
    // — "herhangi bir schema-v2" RED; ölçülen commit'in head'i kayda geçer.
    // after'ın base⊕patch state'inin analizi olduğu v1'de operatör beyanıdır
    // (daha güçlü exact-state bağı ayrı iş; artifact identity ≠ state identity).
    let after_head = require_snapshot_bound_after(&after_bytes, &after_path)?;
    let c_observed = read_observed_coupling(&after_bytes, &after_path, &scope_path)?;
    // P0-2: gerçekleştirilen yamanın kimliği — applied.patch zorunlu + digest
    // binding'e girer (finalize kendi hesabıyla eşleştirir).
    let patch_path = args.run_dir.join("applied.patch");
    anyhow::ensure!(
        patch_path.is_file(),
        "realization-gate requires {} — the gate binds the realized delta's identity \
         (artifact identity ≠ state identity; the patch digest is the chain-of-custody link)",
        patch_path.display()
    );
    let patch_bytes = std::fs::read(&patch_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", patch_path.display()))?;
    let patch_digest = crate::commands::finalize_run::sha256_bytes(&patch_bytes);
    let c_predicted = envelope
        .pointer("/evidence")
        .and_then(|e| e.as_array())
        .and_then(|a| a.last())
        .and_then(|last| last.pointer("/after/x"))
        .and_then(|x| x.as_f64());
    let predicate_after_reanalysis = Some(c_observed <= threshold + tolerance + 1e-12);

    // 4) İnsan-beyanlı gerçeler + kanıt (read-once + digest).
    let evidence_bytes = std::fs::read(&args.evidence)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", args.evidence.display()))?;
    let evidence_input_digest = crate::commands::finalize_run::sha256_bytes(&evidence_bytes);
    let declared: DeclaredRealizationInput =
        serde_json::from_slice(&evidence_bytes).map_err(|e| {
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
    let e_c = evidence.e_c();
    let verdict = evaluate_gate(proof, evidence);

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
        // P0 artifact-identity bağı: verdict'in tükettiği her şeyin read-once
        // digest'i — finalize bu değerleri KENDİ read-once tamponlarıyla
        // eşleştirir (post-hoc substitution reddi). P0-2 kesiti: patch digest +
        // after head = zincir-güvenlik (exact-state bağı v1 sınırı: operatör beyanı).
        "binding": {
            "attempt_digest": crate::commands::finalize_run::sha256_bytes(&attempt_bytes),
            "task_digest": task_digest,
            "after_digest": after_digest,
            "after_head": after_head,
            "patch_digest": patch_digest,
            "evidence_input_digest": evidence_input_digest,
        },
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
    // No-clobber yayın (P1/P2 — TOCTOU'ya kapalı): unique temp + hard_link
    // atomik create — hedef VARSA link RED verir (iki eşzamanlı süreçten yalnız
    // ilki yazar; #171 claim deseni). atomic_write_replace (rename) YOK.
    // **P0-1 (tur-2): ÖNCE kanonik mağazaya** (`<state-dir>/realizations/`,
    // no-clobber `-N` soneğiyle) — run-dir kopyası caller-owned'dur; finalize
    // kanonik nüshayı binding.attempt_digest ile bulur ve bayt-eşleştirir
    // (elle yazılmış verdict'in canonical provenance'ı YOKTUR).
    let canonical_ref =
        publish_canonical_verdict(&state_dir, task_id, payload.to_string().as_bytes())?;
    no_clobber_publish(&verdict_path, payload.to_string().as_bytes())?;

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
    println!("✓ canonical verdict published: {canonical_ref}");
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

/// after.json (read-once tampon) → **snapshot-bound** temiz analiz zorunluluğu
/// (P0-2 zincir-güvenlik kesiti): `repository.binding == clean_pre_post_equal ∧
/// repository.clean == true` — herhangi bir schema-v2 RED. Dönen head,
/// binding'e kaydedilir (ölçülen commit'in kimliği).
fn require_snapshot_bound_after(after_bytes: &[u8], after_path: &Path) -> anyhow::Result<String> {
    let value: serde_json::Value = serde_json::from_slice(after_bytes)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", after_path.display()))?;
    let binding = value
        .pointer("/repository/binding")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let clean = value.pointer("/repository/clean").and_then(|v| v.as_bool());
    anyhow::ensure!(
        binding == "clean_pre_post_equal" && clean == Some(true),
        "{} must be a snapshot-bound clean analysis (repository.binding=\
         clean_pre_post_equal ∧ repository.clean=true; found binding={binding:?}, \
         clean={clean:?}) — a generic/unbound analysis does not establish which \
         state was measured",
        after_path.display()
    );
    value
        .pointer("/repository/head")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("{} is missing repository.head", after_path.display()))
}

/// **P0-1 (tur-2): kanonik verdict publish** — `<state-dir>/realizations/
/// realization-<task>-<millis>-<pid>[-N].json`, no-clobber hard-link atomik
/// create (`-N` soneği çakışmada; attempts mağazası deseni). Dönen ref
/// canonical provenance'dır — finalize binding.attempt_digest ile BULUR, bu
/// nedenle payload kendi adını taşımak zorunda değildir.
fn publish_canonical_verdict(
    state_dir: &Path,
    task_id: u64,
    payload: &[u8],
) -> anyhow::Result<String> {
    use std::io::Write as _;
    let realizations_dir = state_dir.join("realizations");
    std::fs::create_dir_all(&realizations_dir)
        .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", realizations_dir.display()))?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    let tmp = realizations_dir.join(format!("verdict.tmp.{pid}.{millis}"));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(payload)?;
        file.sync_all()?;
    }
    for suffix in 0..=64u32 {
        let name = match suffix {
            0 => format!("realization-{task_id}-{millis}-{pid}.json"),
            n => format!("realization-{task_id}-{millis}-{pid}-{n}.json"),
        };
        let candidate = realizations_dir.join(&name);
        match std::fs::hard_link(&tmp, &candidate) {
            Ok(()) => {
                let _ = std::fs::File::open(&realizations_dir).and_then(|d| d.sync_all());
                let _ = std::fs::remove_file(&tmp);
                return Ok(format!("realizations/{name}"));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                anyhow::bail!("canonical verdict publish failed: {e}");
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    anyhow::bail!("canonical verdict collision budget exhausted for task {task_id}")
}

/// task.json (read-once tampon) → (repository_head, task.id) — P0 fence girdileri.
fn read_task_identity(task_bytes: &[u8], task_path: &Path) -> anyhow::Result<(String, u64)> {
    let value: serde_json::Value = serde_json::from_slice(task_bytes)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", task_path.display()))?;
    let head = value
        .get("repository_head")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} is missing repository_head", task_path.display()))?
        .to_string();
    let id = value
        .pointer("/task/id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("{} is missing task.id", task_path.display()))?;
    Ok((head, id))
}

/// Zarfın run.repository_head'i (fence karşılaştırması için).
fn envelope_run_head(envelope: &serde_json::Value) -> String {
    envelope
        .pointer("/run/repository_head")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// task.json (read-once tampon) → (threshold, tolerance, scope.Path) —
/// v1: Coupling/Le, **mode=All VE tam 1 predicate** (fail-closed: çoklu
/// predicate'li task sessizce değerlendirilemez — RED).
fn read_task_predicate(task_bytes: &[u8], task_path: &Path) -> anyhow::Result<(f64, f64, String)> {
    let value: serde_json::Value = serde_json::from_slice(task_bytes)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", task_path.display()))?;
    let mode = value
        .pointer("/task/target_predicate_set/mode")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let count = value
        .pointer("/task/target_predicate_set/predicates")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    anyhow::ensure!(
        mode == "All" && count == 1,
        "realization-gate v1 evaluates exactly one predicate with mode=All (found \
         mode={mode:?}, {count} predicates) — multi-predicate tasks are refused \
         fail-closed, not silently partially evaluated"
    );
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

/// after.json (read-once tampon) → scope node'unun ölçülmüş coupling'i.
fn read_observed_coupling(
    after_bytes: &[u8],
    after_path: &Path,
    scope_path: &str,
) -> anyhow::Result<f64> {
    let value: serde_json::Value = serde_json::from_slice(after_bytes)
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

/// **No-clobber publish (P1/P2 — TOCTOU'ya kapalı):** içerik shared
/// `stage_temp` ile unique temp'e yazılır, hedefe **hard_link** ile atomik
/// create edilir — hedef VARSA link RED verir (iki eşzamanlı süreçten yalnız
/// ilki kazanır; #171 claim deseni).
fn no_clobber_publish(target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let temp = crate::commands::stage_temp(target, bytes)?;
    match std::fs::hard_link(&temp, target) {
        Ok(()) => {
            let _ = std::fs::remove_file(&temp);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&temp);
            anyhow::bail!(
                "{} already exists — a realization verdict is written once \
                 (hard-link atomic create; concurrent writers: only the first wins)",
                target.display()
            )
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            anyhow::bail!("verdict publish failed: {e}")
        }
    }
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
            r#"{"task":{"target_predicate_set":{"mode":"All","predicates":[{"predicate":{"metric":"Coupling","operator":"Le","threshold":0.875,"tolerance":0.0,"scope":{"Path":"a.rs"}}}]}}}"#,
        )
        .unwrap();
        assert_eq!(
            read_task_predicate(&std::fs::read(&ok).unwrap(), &ok).unwrap(),
            (0.875, 0.0, "a.rs".to_string())
        );

        let bad = dir.path().join("bad.json");
        std::fs::write(
            &bad,
            r#"{"task":{"target_predicate_set":{"mode":"All","predicates":[{"predicate":{"metric":"Instability","operator":"Le","threshold":0.5,"scope":{"Path":"a.rs"}}}]}}}"#,
        )
        .unwrap();
        assert!(read_task_predicate(&std::fs::read(&bad).unwrap(), &bad).is_err());

        // P1: çoklu predicate sessizce kısmi değerlendirilemez — RED.
        let multi = dir.path().join("multi.json");
        std::fs::write(
            &multi,
            r#"{"task":{"target_predicate_set":{"mode":"All","predicates":[{"predicate":{"metric":"Coupling","operator":"Le","threshold":0.5,"scope":{"Path":"a.rs"}}},{"predicate":{"metric":"Coupling","operator":"Le","threshold":0.4,"scope":{"Path":"b.rs"}}}]}}}"#,
        )
        .unwrap();
        let err = read_task_predicate(&std::fs::read(&multi).unwrap(), &multi)
            .expect_err("multi-predicate RED olmalı");
        assert!(err.to_string().contains("fail-closed"), "{err}");
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
        let bytes = std::fs::read(&p).unwrap();
        assert!((read_observed_coupling(&bytes, &p, "a.rs").unwrap() - 0.9167).abs() < 1e-12);
        assert!(read_observed_coupling(&bytes, &p, "yok.rs").is_err());
    }

    /// P1/P2: no-clobber publish gerçek yarış-korumalı — hedef VARSA hard_link RED.
    #[test]
    fn no_clobber_publish_refuses_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("verdict.json");
        no_clobber_publish(&target, b"first").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        let err = no_clobber_publish(&target, b"second").expect_err("ikinci yazım RED olmalı");
        assert!(err.to_string().contains("written once"), "{err}");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"first",
            "ilk yazım bozulmadı"
        );
    }
}
