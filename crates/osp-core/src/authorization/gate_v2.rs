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
#[allow(dead_code, reason = "Faz 5 Item 15-17 consumers")]
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
    /// Predicate gate policy digest hesap hatası.
    #[error("predicate gate policy digest computation failed: {0}")]
    PredicateGatePolicyDigest(EngineMeasurementDigestError),
    /// Predicate gate policy digest mismatch.
    #[error("predicate gate policy digest mismatch: proof={proof}, recomputed={recomputed}")]
    PredicateGatePolicyDigestMismatch { proof: String, recomputed: String },
    /// Loss production hatası — `ProducedTrajectoryLossEvidence` üretimi sırasında.
    #[error("trajectory loss production failed: {0}")]
    LossProduction(TrajectoryLossProductionError),
}

/// **INV-T9 #70 Faz 5 Adım 13 (P1-1):** Trajectory loss production error —
/// `ProducedTrajectoryLossEvidence` üretimi sırasında. Explicit variant mapping.
#[allow(dead_code, reason = "Faz 5 Item 17 evaluate_task_gate_v2 consumer")]
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
    #[allow(dead_code, reason = "Faz 5 Item 17 evaluate_task_gate_v2 consumer")]
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
    #[allow(dead_code, reason = "Faz 5 Item 17 evaluate_task_gate_v2 consumer")]
    pub(crate) fn is_finite(&self) -> bool {
        self.loss_before.is_finite() && self.loss_after.is_finite()
    }

    /// loss_before accessor (decision core girdisi).
    #[allow(dead_code, reason = "Faz 5 Item 17 evaluate_task_gate_v2 consumer")]
    pub(crate) fn loss_before(&self) -> f64 {
        self.loss_before
    }

    /// loss_after accessor (decision core girdisi).
    #[allow(dead_code, reason = "Faz 5 Item 17 evaluate_task_gate_v2 consumer")]
    pub(crate) fn loss_after(&self) -> f64 {
        self.loss_after
    }
}
