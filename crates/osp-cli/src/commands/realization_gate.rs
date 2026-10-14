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
//! **#203 (karar-0/b) — ikinci giriş sınıfı: witnessed resume.** Askı
//! (`awaiting_witnesses`) zarfı, kanonik pending-artifact + receipt zinciriyle
//! bağlanır (`bind_witnessed_resume` — bağlama sözleşmesi): zarfın
//! `result.pending` kimliğinden adreslenen askı, doğrulanmış receipt ve
//! attempt-anı tamamlanma-yetki kararlarıyla gate'e GİRER. `c_predicted` bu
//! yolda zarf evidence'ından DEĞİL, pending basis'inin ölçülmüş coupling'inden
//! gelir (askı zarfı evidence taşımaz). Verdict'e `completion_route` +
//! resume provenance'ı bağlanır; attempt zarfı DEĞİŞMEZ (üç-kayıt dürüstlüğü).
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
    // **P1 (tur-2) + #203 (karar-0/b): giriş yönlendirmesi** — iki giriş sınıfı.
    // Direct: graph-completed iddia (mevcut yol). WitnessedResume: askı zarfı +
    // kanonik pending/receipt zinciri (bağlama sözleşmesi, `bind_witnessed_resume`).
    // Diğer tüm şekiller RED — graph-completed OLMAYAN VE askı OLMAYAN gerçelerden
    // proof kurulamaz (NotAttempted verdict'i yazılmaz).
    let (proof, resumed_binding): (
        osp_core::realization::GateEntryProof,
        Option<ResumedBinding>,
    ) = if result_kind == "completed" && completion_basis == Some(CompletionBasis::Graph) {
        let direct = osp_core::realization::GraphCompletionShapeProof::from_facts(
            task_id,
            canonical_attempt_ref.clone(),
            &result_kind,
            completion_basis,
        )
        .expect("entry routing already matched the from_facts predicate");
        (osp_core::realization::GateEntryProof::Direct(direct), None)
    } else if result_kind == "awaiting_witnesses" && completion_basis.is_none() {
        let binding =
            bind_witnessed_resume(&envelope, task_id, &canonical_attempt_ref, &state_dir)?;
        let entry = osp_core::realization::GateEntryProof::WitnessedResume(binding.proof.clone());
        (entry, Some(binding))
    } else {
        anyhow::bail!(
            "realization-gate context: the attempt is neither a graph-completed claim nor \
             a witnessed-resume suspension (result.kind={result_kind:?}, \
             completion_basis={completion_basis:?}) — the gate runs only on these two \
             entry shapes"
        );
    };

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
    // P0-2 (tur-3 — **self-sufficient realization**) + #204 (tur-4 P0 — **ortak
    // ölçüm-hazırlama yolu**): gate worktree'ini açma + submodule hazırlığını
    // `detached_analysis::prepare_detached_worktree` YAPAR (attempt/resume ile
    // AYNI yol): pinned gitlink'lerde recursive init + status tam-doğrulama.
    // Submodule'li bir repada gate'in kendi analizi submodule'siz kalsaydı
    // SpaceDigest attempt'in ölçümünden ayrışır, #203 m.5b digest-parity geçerli
    // witnessed-resume'u yanlış RED'lerdi. Cleanup: RAII guard (her çıkış yolunda
    // worktree kalkar; erken `remove` + Drop güvenlik ağı).
    let base_head = envelope_run_head(&envelope);
    let patch_bytes = std::fs::read(&patch_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", patch_path.display()))?;
    let patch_digest = crate::commands::finalize_run::sha256_bytes(&patch_bytes);

    let mut worktree_guard =
        crate::commands::detached_analysis::prepare_detached_worktree(&args.repo, &base_head)?;
    let worktree_dir = worktree_guard.worktree_dir.clone();
    let worktree_path = worktree_guard.path().to_path_buf();

    // Cleanup guard — worktree her durumda kaldırılır. Parity fence'i DE bu
    // guard'ın İÇİNDE (closure'un başında) koşar: before-analyze hatası veya
    // digest uyuşmazlığı erken dönüşle worktree'yi ORTADA BIRAKMAZ (PR #207
    // review P1-1).
    let result = (|| -> anyhow::Result<(f64, String, String)> {
        // **#203 (bağlama sözleşmesi madde-5b — digest-parity):** witnessed-resume
        // girişinde gate'in KENDİ before-analyze'ı. Attempt akışıyla BİREBİR aynı
        // inşaat (analyze_repo_with_config + AnalysisConfig::default + aynı adapter
        // registry — mod.rs attempt akışının aynısı) → `SpaceDigest::compute`.
        // Sözleşme: aynı ağaç → aynı kanonik digest; askının `base.content_digest`'i
        // ancak o zaman BU repo durumunda ölçülmüş sayılır (yanlış-askı bağlanması
        // ve base değişimi burada ölür). Parite pinned testle sabitlenmiştir.
        if let Some(binding) = &resumed_binding {
            let registry = osp_analyzer::language::AdapterRegistry::default_all();
            let config = osp_analyzer::contract::AnalysisConfig::default();
            let before = osp_analyzer::pipeline::analyze_repo_with_config(
                &worktree_path,
                &registry,
                &config,
            )
            .map_err(|e| {
                anyhow::anyhow!("gate before-analysis failed on base state {base_head}: {e}")
            })?;
            let before_digest = osp_core::authorization::SpaceDigest::compute(&before.space)
                .map_err(|e| anyhow::anyhow!("gate before-analysis digest failed: {e}"))?;
            enforce_base_content_parity(&binding.base_content_digest, &before_digest, &base_head)?;
        }

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
    // Cleanup worktree + temp dir regardless of result (#204: paylaşılan
    // guard'ın erken bırakması — Drop güvenlik ağı olarak arkada kalır).
    worktree_guard.remove();
    let (c_observed, candidate_head, own_after_digest) = result?;
    // **#203 c_predicted yönlendirmesi:** direct → zarf evidence[]'ının sim-öngörüsü
    // (mevcut sözleşme). Witnessed-resume → pending basis'inin ölçülmüş coupling'i
    // (askı zarfı evidence TAŞIMAZ — run-26 gerçeği: `evidence: []`; öngörü değeri
    // basis'te, digest'ine bağlı olarak yaşar; provenance çiti bind aşamasında kuruldu).
    let c_predicted = match &resumed_binding {
        Some(binding) => Some(binding.c_predicted),
        None => envelope
            .pointer("/evidence")
            .and_then(|e| e.as_array())
            .and_then(|a| a.last())
            .and_then(|last| last.pointer("/after/x"))
            .and_then(|x| x.as_f64()),
    };
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
            // **#203:** giriş rotası + (witnessed) resume provenance'ı. Missing
            // `completion_route` yalnız eski direct verdictlerde tolere edilir;
            // finalize witnessed-resume verdictlerinde alanları ZORUNLU doğrular.
            "completion_route": if resumed_binding.is_some() {
                serde_json::json!("witnessed_resume")
            } else {
                serde_json::json!("attempt_direct")
            },
            "resume_pending_ref": resumed_binding
                .as_ref()
                .map(|b| serde_json::json!(b.pending_ref)),
            "resume_receipt_digest": resumed_binding
                .as_ref()
                .map(|b| serde_json::json!(b.receipt_digest)),
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
    // **#203 (PR review P1-2):** truth-surface tutarlılığı — terminal raporu
    // route-aware projeksiyondan basılır (finalize'ın ledger projeksiyonuyla TEK
    // kaynak). `RealizationVerdict::completion_basis()` direct giriş semantiği
    // taşır (graph-completed iddia girişte kurulmuştu); witnessed-resume'da
    // başarısız verdict HİÇ graph-completed iddia kurmaz → rapor basis=None
    // der, Some(Graph) DEMEZ.
    let ledger_label = match label {
        "RealizedCompleted" => "realized",
        "DeclaredRealizationBuildInvalid" => "declared_realization_build_invalid",
        "PredicateUnsatisfiedAfterReanalysis" => "predicate_unsatisfied_after_reanalysis",
        _ => "not_attempted",
    };
    let (route_basis, _) = crate::commands::finalize_run::ledger_completion_projection(
        resumed_binding.is_some(),
        ledger_label,
    );
    println!(
        "verdict = {label} (basis={route_basis:?}, build_errors={:?}, E_c={e_c:?})",
        verdict.build_error_count(),
    );
    std::process::exit(code);
}

/// **#203 (karar-0/b):** witnessed-resume bağlamı — pending kaydından, parity
/// ve c_predicted için gereken kanal. `bind_witnessed_resume`'ın çıktısı.
struct ResumedBinding {
    proof: osp_core::realization::ResumedCompletionShapeProof,
    /// Askının base uzay içeriği (madde-5b parity fence'inin karşılığı).
    base_content_digest: osp_core::authorization::SpaceDigest,
    /// Sim-öngörüsü: basis'in ölçülmüş coupling değeri (provenance-doğrulamalı).
    c_predicted: f64,
    /// Kanonik pending referansı (state-dir'e göre, ileri-slash).
    pending_ref: String,
    /// Bağlanan receipt'in sha256 digest'i (read-once baytlardan).
    receipt_digest: String,
}

/// **#203 m.5b digest-parity fence — #210 yan-karar:** RED mesajı İKİ digest'i
/// de taşır (beklenen [pending] vs ölçülen [gate]) — asimetri iki değer yan yana
/// göründüğü an okunur (#210 kök-nedene gidiş ~1 saat sürdü; tek-değerli mesaj
/// teşhisi geciktiriyordu).
fn enforce_base_content_parity(
    expected: &osp_core::authorization::SpaceDigest,
    measured: &osp_core::authorization::SpaceDigest,
    base_head: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        expected == measured,
        "space-content parity fence: the gate's own before-analysis of base_head does \
         not reproduce the pending authorization's base content digest — expected \
         [pending] {} but measured [gate] {} at base_head {base_head}; the \
         authorization was measured on a different base than this repo state \
         (binding refused)",
        expected.to_hex(),
        measured.to_hex()
    );
    Ok(())
}

/// **#203 bağlama sözleşmesi — attempt→pending tam kimlik bağı.** Yalnız
/// attempt-anında üretilmiş kanonik bilgiyle kurulur (tur-1 P0-1 kapanışı):
///
/// 1. Zarfın `result.pending` kimlik üçlüsü (anchor'lu zarfta taşınır; missing ≡
///    pre-#203 şekli → re-bind YOK, "re-run the attempt").
/// 2. Kanonik adres tek-kaynak helper'dan türetilir (V1; V2 kardeş varsa açık RED).
/// 3. Pending baytları read-once; strict parse + digest recompute + kimlik-üçlü
///    eşleşmesi AYNI tampondan (consumer-side TOCTOU yok).
/// 4. Receipt: sibling read-once + `verify_against` + `resulting_sequence ==
///    base.sequence + 1`. YOKLUK (deterministik NotAttempted exit 4, verdict
///    yazılmaz) ile BÜTÜNLÜK HATASI (integrity RED) bilinçle AYRI (tur-2 P1-3).
/// 5. Uzay bağlığı: `base.view_id == PersistedSpaceViewId(state-dir)` (load-only;
///    çapraz-uzay ikamesi reddi). (5b — before-analyze parity — worktree
///    kurulduktan sonra ana akışta kurulur.)
///
/// Ayrıca **tamamlanma-uygunluk çiti** (tur-2 P0): `ResumeApplied ⇏
/// CompletionAuthorized` — pending'in attempt-anı yetki kararları
/// (`predicate_completion == Completed ∧ mutation_decision == AcceptAsCompleted
/// ∧ intended_apply_target == Lane(Mainline)`) doğrulanır; ilerleme-yetkili
/// askı tamamlanma kanıtına terfi edemez. Resume motorunun kararları DEĞİŞMEZ.
#[allow(clippy::too_many_lines)]
fn bind_witnessed_resume(
    envelope: &serde_json::Value,
    task_id: u64,
    canonical_attempt_ref: &str,
    state_dir: &Path,
) -> anyhow::Result<ResumedBinding> {
    use osp_core::authorization::{
        parse_pending_authorization, pending_authorization_artifact_filename_v1,
        pending_authorization_artifact_filename_v2_prefix,
        pending_authorization_receipt_filename_v1, PersistedSpaceViewId, ResumeReceipt,
        SpaceViewId,
    };
    use osp_core::realization::{ResumeEligibilityFacts, ResumedCompletionShapeProof};
    use osp_core::trajectory::{ApplyTarget, CommitLane, MutationDecision, PredicateCompletion};

    // Madde-1: askı kimliği anchor'lu zarftan. Missing ≡ pre-#203 şekli.
    let pending_identity = envelope.pointer("/result/pending").ok_or_else(|| {
        anyhow::anyhow!(
            "witnessed-resume binding: this awaiting_witnesses envelope predates #203 and \
             carries no result.pending identity — it cannot be bound to its pending \
             authorization (re-run the attempt; retroactive identity injection is \
             refused by design)"
        )
    })?;
    let claim_id = pending_identity
        .pointer("/claim_id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("result.pending.claim_id is missing or not a u64"))?;
    let attempt_num = pending_identity
        .pointer("/attempt_num")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("result.pending.attempt_num is missing or not a u64"))?;
    let evidence_digest_hex = pending_identity
        .pointer("/evidence_digest")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("result.pending.evidence_digest is missing or not a string")
        })?;
    anyhow::ensure!(
        evidence_digest_hex.len() == 64
            && evidence_digest_hex.bytes().all(|b| b.is_ascii_hexdigit()),
        "result.pending.evidence_digest must be 64 hex chars, found {evidence_digest_hex:?}"
    );

    // Madde-2: kanonik adres — tek-kaynak helper (drift imkânsız).
    let pending_dir = state_dir.join(".osp").join("pending-authorizations");
    let pending_filename = pending_authorization_artifact_filename_v1(
        task_id,
        claim_id,
        attempt_num,
        evidence_digest_hex,
    );
    let pending_path = pending_dir.join(&pending_filename);
    if !pending_path.exists() {
        // Tur-2 P1-2: V1-only kapsam sabitlemesi — V2 kardeş varsa yanlış bir
        // "bu state-dir'de askı yok" yorumu VERİLMEZ, açık kapsam RED'i verilir.
        let v2_prefix =
            pending_authorization_artifact_filename_v2_prefix(task_id, claim_id, attempt_num);
        let v2_sibling = std::fs::read_dir(&pending_dir)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    name.starts_with(&v2_prefix) && name.ends_with(".json")
                })
            })
            .unwrap_or(false);
        anyhow::ensure!(
            !v2_sibling,
            "witnessed-resume binding: a V2 pending artifact exists for this identity \
             ({v2_prefix}…) — the #203 resumed gate binds V1 pending authorizations only \
             (fail-closed, not a silent 'not applied')"
        );
        anyhow::bail!(
            "witnessed-resume binding: canonical pending artifact not found at the address \
             the envelope names ({}) — this state dir does not carry the suspension the \
             attempt produced (cross-artifact substitution or wrong state dir)",
            pending_path.display()
        );
    }

    // Madde-3: read-once + strict verify + kimlik-üçlü eşleşmesi (aynı tampon).
    let pending_bytes = std::fs::read(&pending_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", pending_path.display()))?;
    let pending = parse_pending_authorization(&pending_bytes).map_err(|e| {
        anyhow::anyhow!(
            "pending artifact failed strict verification ({}): {e}",
            pending_path.display()
        )
    })?;
    let record = pending.record();
    anyhow::ensure!(
        record.task_id == task_id
            && record.claim_id == claim_id
            && record.attempt_num.get() == attempt_num
            && record.evidence_digest.to_hex() == evidence_digest_hex,
        "pending identity fence: the canonical artifact at the derived address carries a \
         different identity than the envelope claims — cross-artifact substitution refused"
    );

    // Tamamlanma-uygunluk çiti (tur-2 P0): yetki kararları pending kaydından.
    let eligibility = ResumeEligibilityFacts {
        predicate_completion: record.predicate_completion,
        mutation_decision: record.mutation_decision,
        intended_apply_target: record.intended_apply_target,
    };
    anyhow::ensure!(
        record.predicate_completion == PredicateCompletion::Completed
            && record.mutation_decision == MutationDecision::AcceptAsCompleted
            && record.intended_apply_target == ApplyTarget::Lane(CommitLane::Mainline),
        "completion-eligibility fence: the suspended authorization was not \
         completion-directed (predicate_completion={:?}, mutation_decision={:?}, \
         intended_apply_target={:?}) — a progress/checkpoint authorization cannot be \
         promoted to a completion claim (ResumeApplied does not imply \
         CompletionAuthorized); verdict refused",
        record.predicate_completion,
        record.mutation_decision,
        record.intended_apply_target
    );

    // Madde-4: receipt — yokluk ile bütünlük hatası AYRI (tur-2 P1-3).
    let receipt_filename = pending_authorization_receipt_filename_v1(
        task_id,
        claim_id,
        attempt_num,
        evidence_digest_hex,
    );
    let receipt_path = pending_dir.join(&receipt_filename);
    if !receipt_path.exists() {
        // Dürüst boşluk: askı geçerli ama HENÜZ uygulanmamış. Deterministik
        // NotAttempted (exit 4) — verdict YAZILMAZ (mevcut disiplin).
        println!(
            "NotAttempted: the suspension is valid but not yet applied — no resume \
             receipt at {} (run `osp trajectory resume` first)",
            receipt_path.display()
        );
        std::process::exit(4);
    }
    let receipt_bytes = std::fs::read(&receipt_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", receipt_path.display()))?;
    let receipt = ResumeReceipt::parse_strict_bytes(&receipt_bytes).map_err(|e| {
        anyhow::anyhow!(
            "resume receipt at {} is unreadable/corrupt — durable proof-chain \
             inconsistency (NOT a 'still awaiting' state; operator intervention \
             required): {e}",
            receipt_path.display()
        )
    })?;
    receipt.verify_against(record).map_err(|e| {
        anyhow::anyhow!(
            "resume receipt integrity RED ({}): {e} — durable proof-chain inconsistency \
             (NOT a 'still awaiting' state)",
            receipt_path.display()
        )
    })?;
    // Tur-1 P0-2 eki: apply tam bir adım ilerletir (verify_against'ta yok).
    anyhow::ensure!(
        receipt.resulting_sequence == record.base_space_view_revision.sequence + 1,
        "resume receipt sequence fence: resulting_sequence {} does not equal base \
         sequence {} + 1 — the apply chain is inconsistent with the recorded base",
        receipt.resulting_sequence,
        record.base_space_view_revision.sequence
    );

    // Madde-5a: view_id — çapraz-uzay ikamesi reddi (load-only; resume disiplini).
    let persisted = PersistedSpaceViewId::load_existing(state_dir).map_err(|e| {
        anyhow::anyhow!(
            "cannot load the persisted space identity for {}: {e} — cross-space binding \
             cannot be verified",
            state_dir.display()
        )
    })?;
    anyhow::ensure!(
        record.base_space_view_revision.view_id == SpaceViewId::Persisted(persisted),
        "space-identity fence: the pending authorization was issued under a different \
         persisted space identity than this state dir carries — cross-space substitution \
         refused (identical content digest would not prove the same persisted space)"
    );

    // Proof + kalıcı kanallar (5b parity ana akışta, worktree'de).
    let pending_ref = format!(".osp/pending-authorizations/{pending_filename}");
    let receipt_digest = crate::commands::finalize_run::sha256_bytes(&receipt_bytes);
    let proof = ResumedCompletionShapeProof::from_resumed_facts(
        task_id,
        canonical_attempt_ref.to_string(),
        pending_ref.clone(),
        receipt_digest.clone(),
        "awaiting_witnesses",
        None,
        eligibility,
    )
    .ok_or_else(|| {
        anyhow::anyhow!("internal: eligibility fence passed but from_resumed_facts refused")
    })?;

    // c_predicted + provenance çiti (tur-1 P1): değer basis'in ölçülmüş
    // coupling'i; kaynak aynı task predicate eksenine ve pipeline'ın ölçüm
    // otoritesine (TreeSitter) bağlı olmalı — sim-öngörü uydurulamaz.
    let basis = pending.authorization_basis();
    let predicates = &basis.predicate_content.predicates;
    anyhow::ensure!(
        predicates.len() == 1
            && osp_core::trajectory::PredicateAxis::from(predicates[0].axis)
                == osp_core::trajectory::PredicateAxis::Coupling,
        "c_predicted provenance fence: the pending basis does not carry exactly one \
         Coupling predicate ({}) — the sim prediction cannot be sourced for this task",
        predicates.len()
    );
    use osp_core::coords::MetricSource;
    anyhow::ensure!(
        MetricSource::from(basis.measured_result.coupling.source) == MetricSource::TreeSitter,
        "c_predicted provenance fence: the pending basis coupling measurement is not \
         TreeSitter-sourced — the attempt pipeline authority contract is violated"
    );
    let c_predicted = basis.measured_result.coupling.value;
    anyhow::ensure!(
        c_predicted.is_finite(),
        "c_predicted from the pending basis is not finite — corrupt basis"
    );

    Ok(ResumedBinding {
        proof,
        base_content_digest: record.base_space_view_revision.content_digest.clone(),
        c_predicted,
        pending_ref,
        receipt_digest,
    })
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

    /// #210 yan-karar: m.5b parity RED mesajı İKİ digest'i de taşır —
    /// beklenen [pending] ve ölçülen [gate] yan yana görünür.
    #[test]
    fn parity_fence_red_message_carries_both_digests() {
        let digest_of = |space: &osp_core::space::Space| {
            osp_core::authorization::SpaceDigest::compute(space).expect("digest")
        };
        let empty = osp_core::space::Space::default();
        let mut one_node = osp_core::space::Space::default();
        one_node.nodes.insert(
            42,
            osp_core::space::Node {
                id: 42,
                ..Default::default()
            },
        );
        let d_empty = digest_of(&empty);
        let d_one = digest_of(&one_node);
        assert_ne!(d_empty.to_hex(), d_one.to_hex(), "ayrışan uzaylar");

        // Eşitse sessiz geçiş.
        enforce_base_content_parity(&d_empty, &d_empty, "abc123").expect("parity holds");

        // Ayrışmada RED: mesaj iki digest'i VE base_head'i adıyla taşır.
        let err = enforce_base_content_parity(&d_empty, &d_one, "abc123")
            .expect_err("mismatched digests must refuse");
        let msg = format!("{err:#}");
        assert!(msg.contains("[pending]"), "{msg}");
        assert!(
            msg.contains(&d_empty.to_hex()),
            "expected digest visible: {msg}"
        );
        assert!(msg.contains("[gate]"), "{msg}");
        assert!(
            msg.contains(&d_one.to_hex()),
            "measured digest visible: {msg}"
        );
        assert!(msg.contains("abc123"), "base_head anchored: {msg}");
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
