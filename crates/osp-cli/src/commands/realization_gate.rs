//! #196 uygulama-2 — `osp realization-gate`: INV-T10 RealizationGate'ın CLI
//! ritüeli (karar 2: gate konumu = finalize/başvuru anı).
//!
//! #171 D5a'nın bulgusunun protokolleşmesi: **graph predicate success ⇏
//! build-valid declared realization** (B: E_c = 0 iken build çöktü). Gate,
//! attempt'in graph-katmanı tamamlamasını source gerçekliğine bağlar —
//! **kendi realization'ını kendisi YAPAR** (tur-3 P0: exact-state):
//!
//! ```text
//! --repo <path> → git worktree add --detach <tmp> <base_head>
//!              → git apply applied.patch (mutlak yol)
//!              → git commit (deterministik tarih) → candidate_head
//!              → osp analyze --require-clean-snapshot (KENDİ analizi)
//!              → c_observed (kendi ölçümü; run-dir after.json'a güven YOK)
//!   + attempt.json (canonical-anchor doğrulamalı)
//!   + task.json predicate (Coupling/Le; mode=All ∧ tam 1 predicate)
//!   + applied.patch (mutlak-yol, gate worktree'de uygulanır)
//!   + --evidence (insan-beyanlı: patch_created/parse/build/tests)
//!         → evaluate_gate → realization-verdict.json
//!           (kanonik mağazaya + run dizinine no-clobber)
//! ```
//!
//! **D5a şemasının motorlaşması:** `c_observed` (gate'in kendi analyze'ından),
//! `c_predicted` (zarf evidence'ının sim öngörüsü) ve
//! `predicate_after_reanalysis` (observed ≤ threshold + tolerance) MAKİNE
//! türetilir — insan yalnız patch/parse/build/test gerçelerini beyan eder.
//! E_c imzalı residual kanıtta yaşar (karar 1 v1: sayısal D_G eşiği bilinçle
//! YOK). Binding: `{base_head, patch_digest, candidate_head, own_after_digest}`
//! — After = Apply(Base, Patch) **yapı tarafından kanıtlanır**.
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
    /// Run dizini (`dogfood/runs/<run>/`): attempt.json + task.json + applied.patch
    /// zorunlu. after.json GATE tarafından üretilir (kendi realization'ı).
    pub run_dir: PathBuf,
    /// **P0 (tur-3): analyzed repo** — gate kendi worktree'ini açar
    /// (base_head'de detach), applied.patch'i uygular, commit eder (candidate
    /// state) ve KENDİ analyze'ını koşar. Run-dir after.json'a güven YOK —
    /// After = Apply(Base, Patch) yapı tarafından kanıtlanır.
    #[arg(long)]
    pub repo: PathBuf,
    /// #178/#188 trust anchor: kanonik zarf mağazası. Verilmezse ritüel düzeni
    /// denenir (`<run_dir>/../../state`).
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
    /// İnsan-beyanlı realization gerçeleri (JSON, deny_unknown_fields):
    /// `{ "patch_created": bool, "parse": bool, "build": {"outcome": "succeeded"} |
    /// {"outcome": "failed", "error_count": n}, "tests": {...} | null }`
    /// — makine-türetilen alanlar verilmez; gate üretir.
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
    let patch_path = args.run_dir.join("applied.patch");
    for path in [&attempt_path, &task_path, &patch_path, &args.evidence] {
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
    // **P1 (tur-2): GraphCompletionShapeProof** — graph-completed OLMAYAN gerçelerden
    // proof kurulamaz; gate bağlamı RED ile belli olur (NotAttempted verdict'i yazılmaz).
    let proof = osp_core::realization::GraphCompletionShapeProof::from_facts(
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

    // P0-2 (tur-3 — **self-sufficient realization**): gate KENDİ worktree'ini
    // açar (base_head'de detach), applied.patch'i uygular, commit eder
    // (candidate state = exact base⊕patch) ve KENDİ analyze'ını koşar.
    // Run-dir after.json'a güven TAMAMEN YOK — After = Apply(Base, Patch)
    // YAPI TARAFINDAN kanıtlanır (review tur-3 P0: "artifact identity ≠
    // state identity" kapanışı).
    let base_head = envelope_run_head(&envelope);
    let patch_bytes = std::fs::read(&patch_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", patch_path.display()))?;
    let patch_digest = crate::commands::finalize_run::sha256_bytes(&patch_bytes);

    let worktree_dir = std::env::temp_dir().join(format!(
        "osp-gate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&worktree_dir)
        .map_err(|e| anyhow::anyhow!("cannot create gate worktree tempdir: {e}"))?;
    let worktree_path = worktree_dir.join("candidate");

    // (a) Worktree at base_head.
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&args.repo)
        .arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(&worktree_path)
        .arg(&base_head)
        .output()
        .map_err(|e| anyhow::anyhow!("git worktree add failed to spawn: {e}"))?;
    anyhow::ensure!(
        out.status.success(),
        "gate worktree creation failed at {base_head}: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Cleanup guard — worktree her durumda kaldırılır.
    let result = (|| -> anyhow::Result<(f64, String, String)> {
        // (b) Patch'i uygula (MUTLAK yol — git worktree -C ile göreli yolu
        // worktree içinde arar). Canonicalize YOK (Windows UNC \\?\
        // öneki git'i kırar) — absolute join yeterli.
        let patch_abs = if patch_path.is_absolute() {
            patch_path.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .join(&patch_path)
        };
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&worktree_path)
            .arg("apply")
            .arg(&patch_abs)
            .output()
            .map_err(|e| anyhow::anyhow!("git apply failed to spawn: {e}"))?;
        anyhow::ensure!(
            out.status.success(),
            "gate realization: applied.patch does not apply to base state {base_head}: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // (c) Commit → candidate state kimliği.
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&worktree_path)
            .arg("add")
            .arg("-A")
            .output()
            .map_err(|e| anyhow::anyhow!("git add failed: {e}"))?;
        anyhow::ensure!(out.status.success(), "git add failed in gate worktree");
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&worktree_path)
            .arg("commit")
            .arg("-qm")
            .arg("gate realization candidate (base ⊕ applied.patch)")
            // P1 (tur-4): DETERMINİSTİK commit — tarih de sabitlenir. Aynı
            // base+patch her koşuda AYNI candidate_head üretir (retry idempotent
            // canonical verdict için önkoşul; tarih farklı olsa SHA değişir →
            // farklı own_after_digest → conflict). Tarih = base_head'dan türetme
            // değil sabit epoch — amaç yalnızca determinizmdir.
            .env("GIT_AUTHOR_NAME", "osp-realization-gate")
            .env("GIT_AUTHOR_EMAIL", "gate@osp.local")
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
            .env("GIT_COMMITTER_NAME", "osp-realization-gate")
            .env("GIT_COMMITTER_EMAIL", "gate@osp.local")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
            .output()
            .map_err(|e| anyhow::anyhow!("git commit failed to spawn: {e}"))?;
        anyhow::ensure!(
            out.status.success(),
            "gate realization: commit failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&worktree_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .map_err(|e| anyhow::anyhow!("git rev-parse failed: {e}"))?;
        let candidate_head = String::from_utf8_lossy(&out.stdout).trim().to_string();

        // (d) KENDİ analyze'ını koş — after.json = gate'in ölçümü (run-dir değil).
        let exe = std::env::current_exe()
            .map_err(|e| anyhow::anyhow!("cannot resolve osp binary: {e}"))?;
        let own_after = worktree_dir.join(".osp-gate-after.json");
        let out = std::process::Command::new(exe)
            .arg("analyze")
            .arg(&worktree_path)
            .arg("--format")
            .arg("json")
            .arg("--out")
            .arg(&own_after)
            .arg("--require-clean-snapshot")
            .output()
            .map_err(|e| anyhow::anyhow!("osp analyze failed to spawn: {e}"))?;
        anyhow::ensure!(
            out.status.success(),
            "gate realization: own analysis failed on candidate state {candidate_head}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let own_after_bytes = std::fs::read(&own_after)
            .map_err(|e| anyhow::anyhow!("cannot read gate's own after.json: {e}"))?;
        let own_after_digest = crate::commands::finalize_run::sha256_bytes(&own_after_bytes);
        let c_observed = read_observed_coupling(&own_after_bytes, &own_after, &scope_path)?;
        Ok((c_observed, candidate_head, own_after_digest))
    })();
    // Cleanup worktree + temp dir regardless of result.
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&args.repo)
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(&worktree_path)
        .output();
    let _ = std::fs::remove_dir_all(&worktree_dir);
    let (c_observed, candidate_head, own_after_digest) = result?;
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
        // P0-3 (tur-3 — exact-state binding): base⊕patch = candidate_head,
        // gate'ın KENDİ analyze'ı = own_after_digest. Run-dir after.json
        // güven zincirine GİRMEZ — After = Apply(Base, Patch) yapı tarafından
        // kanıtlanır.
        "binding": {
            "attempt_digest": crate::commands::finalize_run::sha256_bytes(&attempt_bytes),
            "task_digest": task_digest,
            "base_head": base_head,
            "patch_digest": patch_digest,
            "candidate_head": candidate_head,
            "own_after_digest": own_after_digest,
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
    let attempt_digest_str = crate::commands::finalize_run::sha256_bytes(&attempt_bytes);
    let canonical_ref = publish_canonical_verdict(
        &state_dir,
        &attempt_digest_str,
        payload.to_string().as_bytes(),
    )?;
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

/// **P0-1 + P1-cardinality (tur-3):** canonical verdict **attempt-digest-temelli**
/// tek-isimli no-clobber: `realization-<attempt_digest[:16]>.json`. Aynı attempt
/// için birden fazla canonical verdict ÜRETİLEMEZ — bayt-özdeş retry idempotent
/// SUCCESS (aynı dosya zaten duruyor), farklı içerik → RED (attempt identity
/// çelişkisi). Timestamp/pid adlandırması KALDIRILDI (iki gate koşusu iki dosya
/// → ambiguity RED üretiyordu — retry sözleşmesiyle çelişki).
fn publish_canonical_verdict(
    state_dir: &Path,
    attempt_digest: &str,
    payload: &[u8],
) -> anyhow::Result<String> {
    let realizations_dir = state_dir.join("realizations");
    std::fs::create_dir_all(&realizations_dir)
        .map_err(|e| anyhow::anyhow!("cannot create {}: {e}", realizations_dir.display()))?;
    // Attempt-digest'ten tek-isimli canonical kimlik (16 hex karakter yeterli).
    let short = attempt_digest
        .strip_prefix("sha256:")
        .unwrap_or(attempt_digest)
        .chars()
        .take(16)
        .collect::<String>();
    let name = format!("realization-{short}.json");
    let candidate = realizations_dir.join(&name);
    if candidate.exists() {
        let existing = std::fs::read(&candidate).map_err(|e| {
            anyhow::anyhow!("cannot read existing verdict {}: {e}", candidate.display())
        })?;
        if existing == payload {
            // Bayt-özdeş retry — idempotent success (attempt identity aynı,
            // verdict aynı; ikinci koşu zaten aynı sonuca vardı).
            return Ok(format!("realizations/{name}"));
        }
        anyhow::bail!(
            "canonical verdict conflict: {} already exists with DIFFERENT content for \
             the same attempt identity ({short}…) — one attempt, one verdict; a \
             different verdict for the same attempt is a protocol violation",
            candidate.display()
        );
    }
    // No-clobber publish: write temp + hard_link atomik create.
    // P1 (tur-4): temp adı BENZERSİZ (pid+millis) — eşzamanlı aynı-attempt
    // süreçler aynı temp'i truncate edemez; canonical hedef adı sabit kalır.
    let tmp = realizations_dir.join(format!("verdict.tmp.{short}.{}", std::process::id()));
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    std::fs::write(&tmp, payload).map_err(|e| anyhow::anyhow!("verdict temp write failed: {e}"))?;
    match std::fs::hard_link(&tmp, &candidate) {
        Ok(()) => {
            let _ = std::fs::File::open(&realizations_dir).and_then(|d| d.sync_all());
            let _ = std::fs::remove_file(&tmp);
            Ok(format!("realizations/{name}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Yarış: başka süreç aynı anda yayınladı — bayt-eşleşmesi kontrol et.
            let _ = std::fs::remove_file(&tmp);
            let existing = std::fs::read(&candidate).unwrap_or_default();
            if existing == payload {
                Ok(format!("realizations/{name}"))
            } else {
                anyhow::bail!(
                    "canonical verdict race: concurrent writer published DIFFERENT \
                     content for attempt {short}…"
                )
            }
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("canonical verdict publish failed: {e}")
        }
    }
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
