//! INV-T9 #70 Commit 4b Faz 5 — Gate evaluation V2 (runtime proof child module).
//!
//! **Plan module map (issue #83):** `authorization/gate_v2.rs` (child — runtime proof)
//! `VerifiedGateEvaluationBundleV2`, evaluation functions, private proofs, error
//! taxonomy, `ProducedTrajectoryLossEvidence`. Parent `authorization.rs` canonical
//! persisted tipleri + restore validator'ları taşır.
//!
//! **`#[path]` child module:** authorization.rs → mod.rs taşıma YOK. Parent
//! `authorization.rs` `#[path = "authorization/gate_v2.rs"] mod gate_v2;` ile bu
//! dosyayı child module olarak declare eder. Private field access (parent → child)
//! child module olma sebebi — sibling DEĞIL.
//!
//! **Error taxonomy (plan P1-1):** explicit `map_err`, `#[from]` YOK. Mevcut Faz 4
//! error'lar `#[from]` kullanır; V2 error'lar bu pattern'i izlemez — typed mapping
//! restore path hatası lokalizasyonu için korunur.

use crate::measurement::{EngineMeasurementDigest, EngineMeasurementDigestError};

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 #70 Faz 5 Adım 13 (P1-1) — Gate evaluation V2 error taxonomy
//
// Plan (issue #83): explicit map_err, #[from] YOK. Typed variant mapping — restore
// path hatası lokalizasyonu (hangi digest mismatch, hangi loss production fail).
// ═══════════════════════════════════════════════════════════════════════════════

/// **INV-T9 #70 Faz 5 Adım 13 (P1-1):** Gate evaluation V2 error — restore path
/// (3 digest recheck + loss production) hataları. Explicit variant mapping,
/// `#[from]` YOK (plan negatif koşulu).
///
/// Her mismatch variant `proof` (stored digest) + `recomputed` (restore sırasında
/// hesaplanan) taşır — hatanın kaynağı lokalize edilir.
///
/// **Faz 5 Adım 19:** `LossProduction` varyantı `compute_completion_first_loss_and_decision`
/// helper'ı doğrudan canonical loss ürettiği için henüz constructed değil — producer-side
/// `ProducedTrajectoryLossEvidence` (Adım 17/restore) consumer bekleyen. Error taxonomy
/// complete tutulur; allow bu varyant için.
#[allow(
    dead_code,
    reason = "Faz 5 Item 15-17 consumers — LossProduction varyantı producer bekleyen"
)]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GateEvaluationV2Error {
    /// Measurement digest hesap hatası (structural canonicalization).
    #[error("measurement digest computation failed: {0}")]
    MeasurementDigest(EngineMeasurementDigestError),
    /// Measurement digest mismatch — proof ile recomputed farklı.
    #[error("measurement digest mismatch: proof={proof}, recomputed={recomputed}")]
    MeasurementDigestMismatch { proof: String, recomputed: String },
    /// Task-goal digest hesap hatası.
    #[error("task-goal digest computation failed: {0}")]
    TaskGoalDigest(EngineMeasurementDigestError),
    /// Task-goal digest mismatch.
    #[error("task-goal digest mismatch: proof={proof}, recomputed={recomputed}")]
    TaskGoalDigestMismatch { proof: String, recomputed: String },
    /// **INV-T9 #70 PR#84 review P0-1:** Task identity mismatch — binding captured task_id
    /// ile güncel task.id farklı. TOCTOU: iki farklı task gerçekliği birleştirilemez.
    #[error("task identity mismatch: binding captured task_id={proof}, current task.id={current}")]
    TaskIdentityMismatch { proof: u64, current: u64 },
    /// **INV-T9 #70 PR#84 review P0-1:** Current task projection, captured task-goal
    /// evidence ile parity sağlamıyor. Task değişmiş (predicate/policy/preferred_vector).
    #[error("current task-goal evidence mismatch: captured digest != current task projection")]
    CurrentTaskGoalEvidenceMismatch,
    /// Predicate gate policy digest hesap hatası.
    #[error("predicate gate policy digest computation failed: {0}")]
    PredicateGatePolicyDigest(EngineMeasurementDigestError),
    /// Predicate gate policy digest mismatch.
    #[error("predicate gate policy digest mismatch: proof={proof}, recomputed={recomputed}")]
    PredicateGatePolicyDigestMismatch { proof: String, recomputed: String },
    /// Loss production hatası — `ProducedTrajectoryLossEvidence` üretimi sırasında.
    #[error("trajectory loss production failed: {0}")]
    LossProduction(TrajectoryLossProductionError),
    /// **INV-T9 #70 PR#84 review P0-2:** Loss-before derivation failed — baseline
    /// unavailable veya preferred_vector None ile AcceptImprovement progress imkânsız.
    #[error("loss-before derivation failed: {0}")]
    LossBeforeDerivation(String),
    /// **INV-T9 #70 PR#84 review P0-2 (2. tur):** Full EngineMeasurementDigest mismatch —
    /// evaluator'a verilen measurement, binding'in capture ettiği artifact'tan farklı
    /// (before-state değişmiş). Cross-artifact TOCTOU: M1 binding, M2 evaluator.
    #[error("engine measurement digest mismatch: proof={proof}, recomputed={recomputed}")]
    EngineMeasurementDigestMismatch { proof: String, recomputed: String },
}

/// **INV-T9 #70 Faz 5 Adım 13 (P1-1):** Trajectory loss production error —
/// `ProducedTrajectoryLossEvidence` üretimi sırasında. Explicit variant mapping.
#[allow(
    dead_code,
    reason = "Faz 5 Item 17 evaluate_task_gate_v2 consumer — Measurement* varyantları producer bekleyen"
)]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TrajectoryLossProductionError {
    /// Measurement digest hesap hatası (loss production measurement binding).
    #[error("measurement digest computation failed: {0}")]
    MeasurementDigest(EngineMeasurementDigestError),
    /// Measurement binding mismatch — proof ile recomputed farklı.
    #[error("measurement binding mismatch: proof={proof}, recomputed={recomputed}")]
    MeasurementBindingMismatch { proof: String, recomputed: String },
    /// loss_before non-finite (NaN/±Infinity).
    #[error("non-finite loss_before: {value}")]
    NonFiniteLossBefore { value: f64 },
    /// loss_after non-finite.
    #[error("non-finite loss_after: {value}")]
    NonFiniteLossAfter { value: f64 },
}

// ═══════════════════════════════════════════════════════════════════════════════
// ProducedTrajectoryLossEvidence (Item 13) — opaque owned producer-side loss evidence
// ═══════════════════════════════════════════════════════════════════════════════

/// **INV-T9 #70 Faz 5 Adım 13 (P1-1):** ProducedTrajectoryLossEvidence — opaque owned
/// producer-side loss evidence. `is_finite` Before/After invariant. Infallible
/// `into_canonical()` → `CanonicalTrajectoryLossEvidence`.
///
/// **Plan negatif koşulları:**
/// - Producer hatası `NotRequired`'a dönüştürülmez — typed `TrajectoryLossProductionError`.
/// - `as_owned()` eklenmez (Faz 7).
/// - Private — yalnız gate_v2 evaluator (Item 17) üretir.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProducedTrajectoryLossEvidence {
    pub(crate) loss_before: f64,
    pub(crate) loss_after: f64,
    /// Engine measurement digest — loss production sırasında bağlanan artifact.
    pub(crate) engine_measurement_digest: EngineMeasurementDigest,
}

impl ProducedTrajectoryLossEvidence {
    /// **Producer constructor:** loss_before/after finite invariant. Non-finite →
    /// `TrajectoryLossProductionError` (NotRequired'a dönüştürülmez).
    ///
    /// **Not (Faz 5 Adım 19):** Şimdilik helper `compute_completion_first_loss_and_decision`
    /// doğrudan `CanonicalTrajectoryLossEvidence` üretiyor; bu constructor producer-side
    /// evidence için hazır (Adım 17 restore validator / Faz 7 as_owned). Consumer bekleyen.
    #[allow(
        dead_code,
        reason = "Faz 5 Item 17/restore producer — helper canonical üretiyor şimdilik"
    )]
    pub(crate) fn new(
        loss_before: f64,
        loss_after: f64,
        engine_measurement_digest: EngineMeasurementDigest,
    ) -> Result<Self, TrajectoryLossProductionError> {
        if !loss_before.is_finite() {
            return Err(TrajectoryLossProductionError::NonFiniteLossBefore { value: loss_before });
        }
        if !loss_after.is_finite() {
            return Err(TrajectoryLossProductionError::NonFiniteLossAfter { value: loss_after });
        }
        Ok(Self {
            loss_before,
            loss_after,
            engine_measurement_digest,
        })
    }

    /// **Invariant check:** Before/After finite. Defensive — constructor zaten kontrol eder.
    #[allow(
        dead_code,
        reason = "Faz 5 Item 17/restore producer — consumer bekleyen"
    )]
    pub(crate) fn is_finite(&self) -> bool {
        self.loss_before.is_finite() && self.loss_after.is_finite()
    }

    /// loss_before accessor (decision core girdisi).
    #[allow(
        dead_code,
        reason = "Faz 5 Item 17/restore producer — consumer bekleyen"
    )]
    pub(crate) fn loss_before(&self) -> f64 {
        self.loss_before
    }

    /// loss_after accessor (decision core girdisi).
    #[allow(
        dead_code,
        reason = "Faz 5 Item 17/restore producer — consumer bekleyen"
    )]
    pub(crate) fn loss_after(&self) -> f64 {
        self.loss_after
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 #70 Faz 5 Adım 18 — VerifiedGateEvaluationBundleV2 (opaque proof bundle)
//
// Plan (issue #83 v8): Evaluator'ın ürettiği tüm kanıtları tek opaque bundle'da toplar.
// `build_authorization_context_v2` bundle'ı BÜTÜN consume eder — ayrı binding + gate_eval
// kabul etmez (review P0-2: bağımsız eşleştirme kapalı). `from_gate_passed` private ctor —
// sadece `evaluate_task_gate_v2` çağırır (field private, tampering kapalı).
//
// **Proof semantics:** Bundle kanıt üretmez; kanıt `evaluate_task_gate_v2`'nin
// 3 digest recheck + tek predicate evaluation + completion-first loss + decision core
// zinciridir. Bundle bu zincirin çıktısını opaque sarmalar.
//
// **Adım 16 review P0-1:** `CanonicalPredicateEvaluationBasisV2` (evaluation sonucu:
// result + policy + semantics version) burada yaşar — measurement binding epoch'unda
// DEĞİL. "predicate exactly once" invariantı korunur (tek evaluate_completion çağrısı).
// ═══════════════════════════════════════════════════════════════════════════════

/// **INV-T9 #70 Faz 5 Adım 18:** VerifiedGateEvaluationBundleV2 — opaque proof bundle.
/// Evaluator çıktısının tamamı: identity + digests + readable evidence + evaluation
/// sonucu + loss evidence + gate decision proof.
///
/// Field private; Serialize/Deserialize/Clone YOK (opaque proof). Production build'de
/// constructor YOK — `from_gate_passed` private, sadece `evaluate_task_gate_v2` çağırır.
///
/// **Consumer:** `build_authorization_context_v2` → `into_parts()` → AuthorizationBasisV2.
#[derive(Debug)]
pub(crate) struct VerifiedGateEvaluationBundleV2 {
    // Identity (proof'tan gelir — identity injection kapalı).
    task_id: crate::trajectory::TaskId,
    claim_id: crate::witness::ClaimId,
    // Commitment digests (TOCTOU recheck için proof + recompute parity).
    task_claim_digest: crate::measurement::TaskClaimDigest,
    measurement_digest: crate::measurement::MeasurementDigest,
    task_goal_digest: crate::measurement::TaskGoalDigest,
    engine_measurement_digest: crate::measurement::EngineMeasurementDigest,
    predicate_gate_policy_digest: crate::measurement::PredicateGatePolicyDigestV2,
    // Readable canonical evidence (basis field'ları için).
    task_goal_evidence: crate::authorization::CanonicalTaskGoalEvidenceV2,
    measured_after: crate::authorization::ProvenancedMeasuredResult,
    // Evaluation sonucu (review P0-1: bundle-scoped, binding DEĞIL).
    predicate_basis: crate::authorization::CanonicalPredicateEvaluationBasisV2,
    loss_evidence: crate::authorization::CanonicalTrajectoryLossEvidence,
    // Inner binding (subject/impact/delta/revision/context/request_digest).
    binding_inner: crate::engine::VerifiedMeasurementBinding,
    // Preferred vector snapshot (loss context + basis field).
    preferred_vector_snapshot: Option<crate::coords::RawPosition>,
    // Gate decision proof.
    gate_evaluation: crate::authorization::VerifiedGateEvaluationV2,
}

impl VerifiedGateEvaluationBundleV2 {
    /// **Private constructor (Adım 18):** Sadece `evaluate_task_gate_v2` çağırır.
    /// Tüm field'lar evaluator çıktısı — external construction kapalı (tampering-proof).
    #[allow(
        dead_code,
        reason = "Faz 5 evaluate_task_gate_v2 consumer (Faz 8 wiring)"
    )]
    fn from_gate_passed(
        task_id: crate::trajectory::TaskId,
        claim_id: crate::witness::ClaimId,
        task_claim_digest: crate::measurement::TaskClaimDigest,
        measurement_digest: crate::measurement::MeasurementDigest,
        task_goal_digest: crate::measurement::TaskGoalDigest,
        engine_measurement_digest: crate::measurement::EngineMeasurementDigest,
        predicate_gate_policy_digest: crate::measurement::PredicateGatePolicyDigestV2,
        task_goal_evidence: crate::authorization::CanonicalTaskGoalEvidenceV2,
        measured_after: crate::authorization::ProvenancedMeasuredResult,
        predicate_basis: crate::authorization::CanonicalPredicateEvaluationBasisV2,
        loss_evidence: crate::authorization::CanonicalTrajectoryLossEvidence,
        binding_inner: crate::engine::VerifiedMeasurementBinding,
        preferred_vector_snapshot: Option<crate::coords::RawPosition>,
        gate_evaluation: crate::authorization::VerifiedGateEvaluationV2,
    ) -> Self {
        Self {
            task_id,
            claim_id,
            task_claim_digest,
            measurement_digest,
            task_goal_digest,
            engine_measurement_digest,
            predicate_gate_policy_digest,
            task_goal_evidence,
            measured_after,
            predicate_basis,
            loss_evidence,
            binding_inner,
            preferred_vector_snapshot,
            gate_evaluation,
        }
    }

    /// **pub(crate) consumer (Adım 20):** Bundle'ı bütün olarak açar.
    /// `build_authorization_context_v2` bu proof'ları consume eder → AuthorizationBasisV2.
    /// Move-only — bundle iki defa kullanılamaz (Clone YOK).
    #[allow(
        dead_code,
        reason = "Faz 5 build_authorization_context_v2 consumer (Faz 8 wiring)"
    )]
    pub(crate) fn into_parts(self) -> VerifiedGateEvaluationBundlePartsV2 {
        VerifiedGateEvaluationBundlePartsV2 {
            task_id: self.task_id,
            claim_id: self.claim_id,
            task_claim_digest: self.task_claim_digest,
            measurement_digest: self.measurement_digest,
            task_goal_digest: self.task_goal_digest,
            engine_measurement_digest: self.engine_measurement_digest,
            predicate_gate_policy_digest: self.predicate_gate_policy_digest,
            task_goal_evidence: self.task_goal_evidence,
            measured_after: self.measured_after,
            predicate_basis: self.predicate_basis,
            loss_evidence: self.loss_evidence,
            binding_inner: self.binding_inner,
            preferred_vector_snapshot: self.preferred_vector_snapshot,
            gate_evaluation: self.gate_evaluation,
        }
    }
}

/// **Adım 18:** Bundle decomposition — consumer'a geçirilen açık struct.
/// `into_parts()` çıktısı; field'lar pub(crate) (builder doğrudan erişir).
#[doc(hidden)]
#[allow(
    dead_code,
    reason = "Faz 5 bundle decomposition — preferred_vector_snapshot inspector/restore için"
)]
pub(crate) struct VerifiedGateEvaluationBundlePartsV2 {
    pub task_id: crate::trajectory::TaskId,
    pub claim_id: crate::witness::ClaimId,
    pub task_claim_digest: crate::measurement::TaskClaimDigest,
    pub measurement_digest: crate::measurement::MeasurementDigest,
    pub task_goal_digest: crate::measurement::TaskGoalDigest,
    pub engine_measurement_digest: crate::measurement::EngineMeasurementDigest,
    pub predicate_gate_policy_digest: crate::measurement::PredicateGatePolicyDigestV2,
    pub task_goal_evidence: crate::authorization::CanonicalTaskGoalEvidenceV2,
    pub measured_after: crate::authorization::ProvenancedMeasuredResult,
    pub predicate_basis: crate::authorization::CanonicalPredicateEvaluationBasisV2,
    pub loss_evidence: crate::authorization::CanonicalTrajectoryLossEvidence,
    pub binding_inner: crate::engine::VerifiedMeasurementBinding,
    pub preferred_vector_snapshot: Option<crate::coords::RawPosition>,
    pub gate_evaluation: crate::authorization::VerifiedGateEvaluationV2,
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 #70 Faz 5 Adım 19 — evaluate_task_gate_v2 (tek evaluator, TOCTOU closure)
//
// Plan (issue #83 v8): Tek giriş noktası — 3 digest recheck (measurement/task-goal/policy)
// + predicate EXACTLY ONCE + completion-first loss matrisi + decision core. Çıktı:
// VerifiedGateEvaluationBundleV2 (Adım 18). Builder (Adım 20) bundle'ı bütün consume eder.
//
// **Predicate exactly once (review P0-1):** `evaluate_completion` bu fonksiyonda BİR KEZ
// çağrılır. Restore path ayrı evaluator KULLANMAZ — reverse projection + decision core
// (Adım 17). İki paralel evaluator YOK.
//
// **Completion-first loss matrisi (review P0-2):** loss preferred_vector'den DEĞİL,
// predicate sonucundan önce belirlenir. Completed/SourceInsufficient/StrictReject/
// OperatorApproval → NotRequired(reason) (loss hesap YOK). Sadece AcceptImprovement +
// NotCompleted + Available preferred_vector → ProducedTrajectoryLossEvidence.
//
// **3 digest recheck (TOCTOU closure):** verify epoch'ta capture edilen digest'ler,
// evaluator epoch'unda güncel measurement/task'tan recomputed digest ile parity kanıtı.
// Mismatch → fail-closed reject (GateEvaluationV2Error::*Mismatch).
// ═══════════════════════════════════════════════════════════════════════════════

/// **INV-T9 #70 Faz 5 Adım 19:** Tek gate evaluator — V2 semantic closure.
///
/// 3 digest recheck (TOCTOU) + predicate exactly once + completion-first loss +
/// decision core → `VerifiedGateEvaluationBundleV2`. Builder bundle'ı bütün consume eder.
///
/// **Inputs:** `binding` (verify epoch proof), `measurement` (engine-measured before+after),
/// `task` (trusted task snapshot).
///
/// **PR#84 review P0-2:** `loss_before` parametresi kaldırıldı — caller-controlled scalar
/// yerine `measurement.before()` + binding `preferred_vector_snapshot`'tan derive.
/// `MeasurementBaseline::Available` + `Some(target)` → `trajectory_loss(before, target)`.
/// `Unavailable` veya `None` → progress imkânsız (typed unavailable/reject).
#[allow(
    dead_code,
    reason = "Faz 5 build_authorization_context_v2 consumer (Faz 8 wiring)"
)]
pub(crate) fn evaluate_task_gate_v2(
    binding: crate::engine::VerifiedTaskMeasurementBinding,
    measurement: &crate::measurement::EngineMeasurement,
    task: &crate::trajectory::Task,
) -> Result<VerifiedGateEvaluationBundleV2, GateEvaluationV2Error> {
    use crate::authorization::{
        CanonicalPredicateEvaluationBasisV2, EffectiveImproPolicyBasisV2,
        ProvenancedMeasuredResult, VerifiedGateEvaluationV2, GATE_EVALUATION_SEMANTICS_V1,
    };
    use crate::canonical_tags::{PredicateFailurePolicyTag, PredicateSetResultTag};
    use crate::trajectory::EffectiveImprovementPolicy;

    // Binding decomposition — verify epoch proof'ları.
    let (
        task_id,
        claim_id,
        task_claim_digest,
        measurement_digest,
        binding_inner,
        (
            task_goal_digest,
            task_goal_evidence,
            engine_measurement_digest,
            preferred_vector_snapshot,
            predicate_gate_policy_digest,
        ),
    ) = binding.into_parts();

    // ── 3 digest recheck (TOCTOU closure) ──────────────────────────────────────
    // Evaluator epoch'ta güncel measurement/task'tan recomputed digest, verify epoch'ta
    // capture edilen proof digest ile parity kanıtı. Mismatch = fail-closed.

    // **PR#84 review P0-1 (task-goal TOCTOU closure):** Önce task identity + current task
    // projection parity. Captured evidence (Task A) ile güncel task (Task B) birleştirilemez.
    // `task.id == captured task_id` + current task'tan projection recompute + captured digest
    // ile parity. Bu olmadan predicate güncel task'tan, kimlik/evidence başka task'tan gelebilir.
    if task.id != task_id {
        return Err(GateEvaluationV2Error::TaskIdentityMismatch {
            proof: task_id,
            current: task.id,
        });
    }
    let current_task_goal_evidence =
        crate::authorization::CanonicalTaskGoalEvidenceV2::try_from(task).map_err(|e| {
            GateEvaluationV2Error::TaskGoalDigest(EngineMeasurementDigestError::from(
                crate::measurement::MeasurementDigestError::StructuralCanonicalization {
                    detail: format!("current task-goal projection: {e}"),
                },
            ))
        })?;
    let current_task_goal_digest =
        crate::measurement::TaskGoalDigest::compute_from_canonical(&current_task_goal_evidence)
            .map_err(GateEvaluationV2Error::TaskGoalDigest)?;
    // Current projection captured digest ile parity — task değişmişse reject.
    if current_task_goal_digest.as_bytes() != task_goal_digest.as_bytes() {
        return Err(GateEvaluationV2Error::CurrentTaskGoalEvidenceMismatch);
    }

    // (0) **PR#84 review P0-2 (2. tur):** Full EngineMeasurementDigest parity — before+after+
    // context+request bütününü bağlar. Bu olmadan evaluator'a M2 (farklı before, aynı after)
    // verilebilir; loss-before M2'den, builder M1'den gelir (cross-artifact TOCTOU).
    // loss-before derivation'dan ÖNCE — before-state doğrulaması.
    let recomputed_engine_measurement = measurement.compute_digest().map_err(|e| {
        GateEvaluationV2Error::MeasurementDigest(EngineMeasurementDigestError::from(
            crate::measurement::MeasurementDigestError::StructuralCanonicalization {
                detail: format!("engine measurement full digest: {e}"),
            },
        ))
    })?;
    if recomputed_engine_measurement.as_bytes() != engine_measurement_digest.as_bytes() {
        return Err(GateEvaluationV2Error::EngineMeasurementDigestMismatch {
            proof: engine_measurement_digest.to_hex(),
            recomputed: recomputed_engine_measurement.to_hex(),
        });
    }

    // (1) Measurement digest — measured_after'dan recompute (defense-in-depth: after-only).
    let measured_after = ProvenancedMeasuredResult::try_from(measurement.after()).map_err(|e| {
        GateEvaluationV2Error::MeasurementDigest(EngineMeasurementDigestError::from(
            crate::measurement::MeasurementDigestError::StructuralCanonicalization {
                detail: format!("measured_after projection: {e}"),
            },
        ))
    })?;
    let recomputed_measurement = crate::measurement::MeasurementDigest::compute_from_canonical(
        &measured_after,
    )
    .map_err(|e| GateEvaluationV2Error::MeasurementDigest(EngineMeasurementDigestError::from(e)))?;
    if recomputed_measurement.as_bytes() != measurement_digest.as_bytes() {
        return Err(GateEvaluationV2Error::MeasurementDigestMismatch {
            proof: measurement_digest.to_hex(),
            recomputed: recomputed_measurement.to_hex(),
        });
    }

    // (2) Task-goal digest — captured evidence'dan recompute (current projection yukarıda
    // parity ile kanıtlandı, bu ek defense-in-depth: captured evidence kendi digest'iyle
    // tutarlı — binding içi tutarlılık).
    let recomputed_task_goal =
        crate::measurement::TaskGoalDigest::compute_from_canonical(&task_goal_evidence)
            .map_err(GateEvaluationV2Error::TaskGoalDigest)?;
    if recomputed_task_goal.as_bytes() != task_goal_digest.as_bytes() {
        return Err(GateEvaluationV2Error::TaskGoalDigestMismatch {
            proof: task_goal_digest.to_hex(),
            recomputed: recomputed_task_goal.to_hex(),
        });
    }

    // ── Predicate evaluate EXACTLY ONCE ─────────────────────────────────────────
    // Tek evaluate_completion çağrısı — "predicate exactly once" invariantı (review P0-1).
    // Bu evaluator epoch'taki tek predicate evaluation; restore path ayrı evaluator
    // KULLANMAZ (reverse projection + decision core, Adım 17).
    let completion = task
        .target_predicate_set
        .evaluate_completion(measurement.after());

    // ── CanonicalPredicateEvaluationBasisV2 (evaluation sonucu — bundle-scoped) ──
    // Review P0-1: result + policy + semantics version burada üretilir, binding'de DEĞIL.
    let improvement_policy = EffectiveImprovementPolicy::current_semantics();
    let result_tag = PredicateSetResultTag::try_from(&completion).map_err(|e| {
        GateEvaluationV2Error::MeasurementDigest(EngineMeasurementDigestError::from(
            crate::measurement::MeasurementDigestError::StructuralCanonicalization {
                detail: format!("PredicateSetResultTag: {e}"),
            },
        ))
    })?;
    let failure_policy_tag = PredicateFailurePolicyTag::try_from(
        &task.policy.predicate_failure_policy,
    )
    .map_err(|e| {
        GateEvaluationV2Error::MeasurementDigest(EngineMeasurementDigestError::from(
            crate::measurement::MeasurementDigestError::StructuralCanonicalization {
                detail: format!("PredicateFailurePolicyTag: {e}"),
            },
        ))
    })?;
    let predicate_basis = CanonicalPredicateEvaluationBasisV2 {
        gate_evaluation_semantics_version: GATE_EVALUATION_SEMANTICS_V1,
        result: result_tag,
        failure_policy: failure_policy_tag,
        min_improvement_delta: task.policy.min_improvement_delta,
        allow_progress_checkpoint: task.policy.allow_progress_checkpoint,
        effective_improvement: EffectiveImproPolicyBasisV2::try_from(improvement_policy).map_err(
            |e| {
                GateEvaluationV2Error::MeasurementDigest(EngineMeasurementDigestError::from(
                    crate::measurement::MeasurementDigestError::StructuralCanonicalization {
                        detail: format!("EffectiveImproPolicyBasisV2: {e}"),
                    },
                ))
            },
        )?,
    };

    // (3) Predicate gate policy digest — oluşturulan predicate_basis üzerinden recompute
    // (TOCTOU: task snapshot). compute_from_canonical basis.result KULLANMAZ (policy
    // digest evaluation sonucundan bağımsız — sadece failure_policy + min_delta + ...).
    let recomputed_policy =
        crate::measurement::PredicateGatePolicyDigestV2::compute_from_canonical(
            task_id,
            &task_goal_digest,
            &predicate_basis,
        )
        .map_err(GateEvaluationV2Error::PredicateGatePolicyDigest)?;
    if recomputed_policy.as_bytes() != predicate_gate_policy_digest.as_bytes() {
        return Err(GateEvaluationV2Error::PredicateGatePolicyDigestMismatch {
            proof: predicate_gate_policy_digest.to_hex(),
            recomputed: recomputed_policy.to_hex(),
        });
    }

    // ── Completion-first loss matrisi (review P0-2) ─────────────────────────────
    // Loss preferred_vector'den DEĞİL, predicate sonucundan önce belirlenir.
    // **PR#84 review P0-2:** loss_before measurement.before()'tan derive (caller scalar DEĞİL).
    let (loss_evidence, _improved, mutation_decision) = compute_completion_first_loss_and_decision(
        completion,
        &task.policy,
        &improvement_policy,
        measurement,
        preferred_vector_snapshot,
    )?;

    // ── Decision core → MutationDecision (review P0-1: pure, loss-free) ──────────
    // Yukarıdaki helper improved + completion'dan decision üretti; burada sadece
    // gate evaluation proof sarmalanır.
    let gate_evaluation = VerifiedGateEvaluationV2::from_gate_passed(mutation_decision);

    Ok(VerifiedGateEvaluationBundleV2::from_gate_passed(
        task_id,
        claim_id,
        task_claim_digest,
        measurement_digest,
        task_goal_digest,
        engine_measurement_digest,
        predicate_gate_policy_digest,
        task_goal_evidence,
        measured_after,
        predicate_basis,
        loss_evidence,
        binding_inner,
        preferred_vector_snapshot,
        gate_evaluation,
    ))
}

/// **Completion-first loss + decision helper (review P0-2):** Predicate sonucu + policy'ye
/// göre loss evidence + improved flag + mutation decision üretir. Matris (issue #83):
///
/// - Completed → NotRequired(PredicateCompleted), AcceptAsCompleted
/// - SourceInsufficient → NotRequired(SourceInsufficient), Reject
/// - NotCompleted + StrictReject → NotRequired(StrictRejectPolicy), Reject
/// - NotCompleted + OperatorApproval → NotRequired(OperatorApprovalPolicy), RequireOperatorApproval
/// - NotCompleted + AcceptImprovement + NoPreferredVector → Unavailable(NoPreferredVector)
/// - NotCompleted + AcceptImprovement + Available → ProducedTrajectoryLossEvidence → improved
///
/// **Loss-before hardcoded YOK:** `loss_before` caller'dan (engine measurement baseline context).
#[allow(
    dead_code,
    reason = "Faz 5 evaluate_task_gate_v2 consumer (Faz 8 wiring)"
)]
/// **PR#84 review P0-2:** `loss_before` parametresi kaldırıldı — caller-controlled
/// scalar yerine `measurement.before()` + `preferred_vector_snapshot`'tan derive.
/// `MeasurementBaseline::Available` + `Some(target)` → trajectory_loss(before, target).
/// `Unavailable` veya `None` → progress imkânsız (typed unavailable/reject).
fn compute_completion_first_loss_and_decision(
    completion: crate::trajectory::PredicateSetResult,
    policy: &crate::trajectory::TaskPolicy,
    improvement_policy: &crate::trajectory::EffectiveImprovementPolicy,
    measurement: &crate::measurement::EngineMeasurement,
    preferred_vector_snapshot: Option<crate::coords::RawPosition>,
) -> Result<
    (
        crate::authorization::CanonicalTrajectoryLossEvidence,
        bool,
        crate::trajectory::MutationDecision,
    ),
    GateEvaluationV2Error,
> {
    use crate::authorization::{
        CanonicalLossNotRequiredReason, CanonicalRawPosition, CanonicalTrajectoryLossEvidence,
        CanonicalTrajectoryLossUnavailableReason,
    };
    use crate::measurement::MeasurementBaseline;
    use crate::trajectory::{assess_improvement_v1, evaluate_decision_core, PredicateSetResult};

    match completion {
        PredicateSetResult::Completed | PredicateSetResult::SourceInsufficient => {
            // Completion-first: loss hesap YOK. Decision core short-circuit.
            let reason = match completion {
                PredicateSetResult::Completed => CanonicalLossNotRequiredReason::PredicateCompleted,
                PredicateSetResult::SourceInsufficient => {
                    CanonicalLossNotRequiredReason::SourceInsufficient
                }
                _ => unreachable!(),
            };
            let (_assessment, decision) = evaluate_decision_core(
                completion,
                false, // improved irrelevant — completion short-circuits
                policy.predicate_failure_policy,
                policy.allow_progress_checkpoint,
            );
            Ok((
                CanonicalTrajectoryLossEvidence::NotRequired { reason },
                false,
                decision,
            ))
        }
        PredicateSetResult::NotCompleted => match policy.predicate_failure_policy {
            crate::trajectory::PredicateFailurePolicy::StrictReject => {
                let (_assessment, decision) = evaluate_decision_core(
                    completion,
                    false,
                    policy.predicate_failure_policy,
                    policy.allow_progress_checkpoint,
                );
                Ok((
                    CanonicalTrajectoryLossEvidence::NotRequired {
                        reason: CanonicalLossNotRequiredReason::StrictRejectPolicy,
                    },
                    false,
                    decision,
                ))
            }
            crate::trajectory::PredicateFailurePolicy::OperatorApproval => {
                let (_assessment, decision) = evaluate_decision_core(
                    completion,
                    false,
                    policy.predicate_failure_policy,
                    policy.allow_progress_checkpoint,
                );
                Ok((
                    CanonicalTrajectoryLossEvidence::NotRequired {
                        reason: CanonicalLossNotRequiredReason::OperatorApprovalPolicy,
                    },
                    false,
                    decision,
                ))
            }
            crate::trajectory::PredicateFailurePolicy::AcceptImprovement => {
                // Loss calculation needed — preferred_vector + baseline required.
                match preferred_vector_snapshot {
                    None => {
                        // No preferred vector → loss meaningless → Reject.
                        let (_assessment, decision) = evaluate_decision_core(
                            completion,
                            false,
                            policy.predicate_failure_policy,
                            policy.allow_progress_checkpoint,
                        );
                        Ok((
                            CanonicalTrajectoryLossEvidence::Unavailable {
                                reason: CanonicalTrajectoryLossUnavailableReason::NoPreferredVector,
                            },
                            false,
                            decision,
                        ))
                    }
                    Some(target) => {
                        // **PR#84 review P0-2:** loss_before measurement.before()'tan derive.
                        // **PR#84 review P1 (2. tur):** baseline unavailable → target var, baseline
                        // yok. Ontolojik olarak: Available{target, loss_after} (target gerçek) +
                        // improved=false + Reject (baseline olmadan progress kanıtlanamaz).
                        // NoPreferredVector YANLIŞ — preferred vector var, eksik olan baseline.
                        let loss_after =
                            crate::trajectory::trajectory_loss(measurement.after(), &target);
                        if !loss_after.is_finite() {
                            return Err(GateEvaluationV2Error::LossBeforeDerivation(format!(
                                "non-finite loss_after: {loss_after}"
                            )));
                        }
                        match measurement.before() {
                            MeasurementBaseline::Available(before) => {
                                let loss_before =
                                    crate::trajectory::trajectory_loss(before, &target);
                                if !loss_before.is_finite() {
                                    return Err(GateEvaluationV2Error::LossBeforeDerivation(
                                        format!("non-finite loss_before: {loss_before}"),
                                    ));
                                }
                                let improved = assess_improvement_v1(
                                    loss_before,
                                    loss_after,
                                    measurement.after(),
                                    policy,
                                    improvement_policy,
                                );
                                let (_assessment, decision) = evaluate_decision_core(
                                    completion,
                                    improved,
                                    policy.predicate_failure_policy,
                                    policy.allow_progress_checkpoint,
                                );
                                let loss_evidence = CanonicalTrajectoryLossEvidence::Available {
                                    target: CanonicalRawPosition::from(target),
                                    loss_after,
                                };
                                Ok((loss_evidence, improved, decision))
                            }
                            MeasurementBaseline::Unavailable { .. } => {
                                // Baseline yok → improvement kanıtlanamaz → improved=false, Reject.
                                // Loss evidence Available (target gerçek, loss_after hesaplanabilir);
                                // baseline ayrı CanonicalTrajectoryEvidenceBaseline::Unavailable.
                                let (_assessment, decision) = evaluate_decision_core(
                                    completion,
                                    false,
                                    policy.predicate_failure_policy,
                                    policy.allow_progress_checkpoint,
                                );
                                let loss_evidence = CanonicalTrajectoryLossEvidence::Available {
                                    target: CanonicalRawPosition::from(target),
                                    loss_after,
                                };
                                Ok((loss_evidence, false, decision))
                            }
                        }
                    }
                }
            }
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 #70 Faz 5 Adım 20 — build_authorization_context_v2 (bundle consume, gate_v2)
//
// Plan (issue #83 v8): engine.rs'ten taşındı (self. kullanımı 0 → free fn). Artık tek
// bundle consume eder — ayrı binding + gate_evaluation kabul ETMEZ (review P0-2:
// bağımsız eşleştirme kapalı). Bundle, evaluator'ın ürettiği tüm kanıtları taşır.
//
// **Review P0-2 completion-first:** loss artık bundle'dan gelir (evaluator'ın
// completion-first matrisiyle üretilmiş). Builder preferred_vector'den loss üretmez.
// ═══════════════════════════════════════════════════════════════════════════════

/// **INV-T9 #70 Faz 5 Adım 20:** Standalone V2 authorization context builder.
///
/// Bundle (evaluator çıktısı) + witness requirement + presented measurement artifact →
/// `AuthorizationContextV2`. Re-derivation YOK — bundle consume + baseline projection +
/// checked constructor zinciri.
///
/// **Ontolojik zincir:** verify_measurement_binding → evaluate_task_gate_v2 (bundle)
/// → build_authorization_context_v2(bundle, witness, measurement) → AuthorizationContextV2.
///
/// **Production wiring Faz 8.** Engine state kullanmaz (free fn, self. = 0).
#[allow(
    dead_code,
    reason = "Faz 8 production wiring / Commit 2 standalone test"
)]
pub(crate) fn build_authorization_context_v2(
    bundle: VerifiedGateEvaluationBundleV2,
    witness_requirement: crate::authorization::CanonicalWitnessRequirementV2,
    measurement: &crate::measurement::EngineMeasurement,
) -> Result<
    crate::authorization::AuthorizationContextV2,
    crate::authorization::AuthorizationContextV2BuildError,
> {
    use crate::authorization::{
        AuthorizationBasisV2, AuthorizationContextV2, CanonicalBaselineUnavailableReason,
        CanonicalTrajectoryEvidenceBaseline, ProvenancedMeasuredResult,
    };
    use crate::measurement::{
        MeasurementBaseline, MeasurementContextDigest, MeasurementDeltaDigest,
    };

    // Bundle decomposition — evaluator'ın ürettiği tüm kanıtlar.
    let parts = bundle.into_parts();

    // 1. Presented artifact == consumed proof? (tüm evidence projection'dan ÖNCE)
    let recomputed = measurement.compute_digest()?;
    if recomputed.as_bytes() != parts.engine_measurement_digest.as_bytes() {
        return Err(
            crate::authorization::AuthorizationContextV2BuildError::EngineMeasurementBindingMismatch {
                proof: parts.engine_measurement_digest.to_hex(),
                recomputed: recomputed.to_hex(),
            },
        );
    }

    // 2. Baseline canonical evidence (MeasurementBaseline → CanonicalTrajectoryEvidenceBaseline).
    let trajectory_baseline = match measurement.before() {
        MeasurementBaseline::Available(before) => CanonicalTrajectoryEvidenceBaseline::Available {
            before: ProvenancedMeasuredResult::try_from(before)?,
        },
        MeasurementBaseline::Unavailable { reason } => {
            CanonicalTrajectoryEvidenceBaseline::Unavailable {
                reason: CanonicalBaselineUnavailableReason::try_from_reason(
                    reason,
                    parts.binding_inner.subject(),
                )?,
            }
        }
    };
    let measurement_baseline_digest = trajectory_baseline.compute_measurement_baseline_digest()?;

    // 3. Request + subordinate commitments.
    let measurement_request = measurement.request().canonical_evidence();
    let measurement_request_digest = parts.binding_inner.request_digest().clone();
    let canonical_delta_digest =
        MeasurementDeltaDigest::compute_from_canonical(parts.binding_inner.canonical_delta())?;
    let measurement_context_digest =
        MeasurementContextDigest::compute(parts.binding_inner.current_context())?;

    // 4. Checked basis (validate_semantics — 17-field + 4-field commitment parity).
    // Review P0-2: loss + measured_after + task_goal_evidence + predicate_basis +
    // predicate_gate_policy_digest bundle'dan gelir (evaluator çıktısı, completion-first).
    let basis = AuthorizationBasisV2::new(
        parts.task_id,
        parts.claim_id,
        parts.task_claim_digest,
        parts.task_goal_digest,
        parts.measurement_digest,
        parts.engine_measurement_digest,
        trajectory_baseline,
        measurement_baseline_digest,
        parts.loss_evidence,
        measurement_request,
        measurement_request_digest,
        measurement_context_digest,
        canonical_delta_digest,
        parts.measured_after,
        parts.task_goal_evidence,
        parts.predicate_basis,
        parts.predicate_gate_policy_digest,
    )?;

    // 5. Proof-gated context + witness/apply-target consistency.
    AuthorizationContextV2::new(basis, parts.gate_evaluation, witness_requirement)
}
