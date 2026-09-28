//! **#96 MD-2 — Provenance authority drift observation (plan v4-FİNAL W5).**
//!
//! MD-2 normatif kuralı (faz8-p2-migration-decisions.md): predicate source authority
//! engine-native per-axis provenance'dır; V1 uniform-Scip projection karar ÜRETMEZ
//! (yalnız reference telemetry — fiziksel kaldırma #100).
//!
//! Bu modül, #96 cutover sonrası **kalıcı MD-2 telemetrisi**: otorite (native) ile
//! kaldırılmak üzere olan referans (uniform-Scip izdüşümü) arasında, **AYNI ölçüm
//! event'i** üzerinde yalnız PROVENANCE eksenindeki farkı gözlemler:
//!
//! ```text
//! NativeLegacySubjectMeasurement (otorite — commit'in tükettiği token)
//!   ├─ native lane    : measured values + engine-native per-axis sources  → AUTHORITATIVE
//!   └─ reference lane : AYNI value bits + uniform-Scip izdüşümü            → REFERENCE ONLY
//!                        (counterfactual PredicateGate — asla "production
//!                         observed" olarak serialize EDILMEZ)
//! ```
//!
//! **Ayrım MD-1 observer ile (tarihsel):** eski `subject_authority.rs` subject
//! eksenini izlerdi (legacy affected_nodes ↔ task scope; iki lane de native) —
//! modül #95-B'de silindi; MD-2 gözlemi kalıcı kurumsal yüzeydir. Bu modül
//! provenance eksenini izler (native ↔ uniform-Scip; iki lane aynı subject/value
//! bits). Dogfood Run A senaryosunun (subject+θ parity + downstream diverge)
//! kurumsal kalıcı yüzeyi.
//!
//! **Eligibility (plan v4 P1-tur3 — precedence correction):**
//! - structural-Q4 reject → observation YOK (measurement henüz gerçekleşmedi)
//! - measurement failure → karşılaştırma YOK
//! - measurement OK → draft (pre-commit)
//! - Q5 violated → finalize `NotReached{Q5Violated}`
//! - Predicate çalıştı + Q6 fail → `ReachedButUnavailable{Q6RuleViolationAfterPredicateGate}`
//! - Evaluated/Held/Rejected → `Observed{...}` (üretim outcome'u — authoritative)
//! - `Q4SyntaxRejection` arm'ı YOK (structural Q4 draft aşamasında yakalanır).
//!
//! Wire: additive sidecar'lar (`TrajectoryEvidence` / `PendingAuthorization` /
//! `RevisionRequired` / MCP response) — digest preimage'lerine GİRMEZ; durable
//! wire'larda identity-bound (task/claim parent). #95-B subject_authority.rs
//! silinirken paylaşılan lane tipleri bu modüle taşındı (tek hamle — E4).

use crate::coords::{MeasuredRawPosition, MetricSource, RawPosition};
use crate::engine::SpaceEngine;
use crate::measurement::NativeLegacySubjectMeasurement;
use crate::trajectory::{
    MutationDecision, PredicateCompletion, PredicateGate, PredicateGateInput, Task, TaskBoundClaim,
};
use crate::witness::Claim;

// ═══════════════════════════════════════════════════════════════════════════════
// Paylaşılan Q5-observation makineleri — #95-B (E4): subject_authority.rs
// modülünden taşındı (serde wire attrs VERBATİM; JSON değişmez).
// ═══════════════════════════════════════════════════════════════════════════════

/// Ölçüm değer yüzeyi — 5-axis bit-exact değerler + per-axis provenance.
///
/// `bits` sırası: `(x, y, z, w, v)` = (coupling, cohesion, instability, entropy,
/// witness_depth) `to_bits()`. `sources` aynı sırada. V1 sources = uniform-Scip
/// **compatibility projection** (native evidence DEĞİL — MD-2 ayrı migration
/// authority'dir); V2 sources = engine-native per-axis.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawMeasurementObservation {
    pub bits: [u64; 5],
    pub sources: [MetricSource; 5],
}

fn raw_observation(raw: RawPosition, sources: [MetricSource; 5]) -> RawMeasurementObservation {
    RawMeasurementObservation {
        bits: [
            raw.x.to_bits(),
            raw.y.to_bits(),
            raw.z.to_bits(),
            raw.w.to_bits(),
            raw.v.to_bits(),
        ],
        sources,
    }
}

fn measured_sources(measured: &MeasuredRawPosition) -> [MetricSource; 5] {
    [
        measured.coupling.source,
        measured.cohesion.source,
        measured.instability.source,
        measured.entropy.source,
        measured.witness_depth.source,
    ]
}

/// Lane Q5 yüzeyi — illegal state'ler tip seviyesinde imkânsız (plan v3 P1-4):
/// `Passed + theta=None` gibi kombinasyonlar temsil edilemez.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneQ5Observation {
    Evaluated {
        theta_bits: u64,
        theta_bound_bits: u64,
        verdict: EvaluatedQ5Verdict,
    },
    NotEvaluated {
        reason: Q5ObservationFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatedQ5Verdict {
    Passed,
    Violated,
}

/// Q5 gözlem yüzeyine ulaşılamama sebebi — güncel `VisionContextError`'un
/// **8 varyantının tamamı** (authorization.rs), telemetry wire'ına runtime payload
/// taşımadan (unit varyantlar; runtime error representation wire contract'a bağlanmaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q5ObservationFailure {
    VisionUnavailable,
    VisionAuthorityInsufficient,
    SubjectSourceMismatch,
    NonFiniteVisionAxis,
    NonFiniteThetaBound,
    ThetaBoundOutOfRange,
    UnsupportedSemanticsVersion,
    CanonicalRoleConversionFailed,
}

pub(crate) fn map_q5_observation_failure(
    err: crate::authorization::VisionContextError,
) -> Q5ObservationFailure {
    use crate::authorization::VisionContextError;
    match err {
        VisionContextError::VisionUnavailable => Q5ObservationFailure::VisionUnavailable,
        VisionContextError::VisionAuthorityInsufficient { .. } => {
            Q5ObservationFailure::VisionAuthorityInsufficient
        }
        VisionContextError::SubjectSourceMismatch { .. } => {
            Q5ObservationFailure::SubjectSourceMismatch
        }
        VisionContextError::NonFiniteVisionAxis { .. } => Q5ObservationFailure::NonFiniteVisionAxis,
        VisionContextError::NonFiniteThetaBound(_) => Q5ObservationFailure::NonFiniteThetaBound,
        VisionContextError::ThetaBoundOutOfRange(_) => Q5ObservationFailure::ThetaBoundOutOfRange,
        VisionContextError::UnsupportedSemanticsVersion { .. } => {
            Q5ObservationFailure::UnsupportedSemanticsVersion
        }
        VisionContextError::CanonicalRoleConversionFailed(_) => {
            Q5ObservationFailure::CanonicalRoleConversionFailed
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShadowDownstreamObservation {
    pub predicate_completion: PredicateCompletion,
    pub mutation_decision: MutationDecision,
}

/// Tek captured context altında bir raw'ın Q5 yüzeyi. Verdict üretimi production
/// karşılaştırmasının aynısı: `CosineDeviation.theta(raw, effective_vision, space)`
/// + `theta > theta_bound` (engine.rs `check_vision_raw_with_context` gövdesi).
pub(crate) fn evaluate_lane_q5(
    engine: &SpaceEngine,
    raw: RawPosition,
    ctx: &crate::authorization::EffectiveVisionGateContext,
) -> LaneQ5Observation {
    use crate::vision::{CosineDeviation, DeviationMetric};
    let theta = CosineDeviation.theta(&raw, &ctx.selection.effective_vision, engine.space());
    let verdict = if theta > ctx.theta_bound {
        EvaluatedQ5Verdict::Violated
    } else {
        EvaluatedQ5Verdict::Passed
    };
    LaneQ5Observation::Evaluated {
        theta_bits: theta.to_bits(),
        theta_bound_bits: ctx.theta_bound.to_bits(),
        verdict,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Reference projection (uniform-Scip — reference-only)
// ═══════════════════════════════════════════════════════════════════════════════

/// **#96 MD-2:** Uniform-Scip **reference projection** — aynı ölçüm event'inin
/// value bit'lerinin legacy V1 provenance temsilindeki izdüşümü. Karar ÜRETMEZ
/// (mutation authority DEĞİL); yalnız MD-2 reference telemetry. Eski MD-1
/// modülünün `legacy_compatibility_projection`'ının (#95-B'de modülle birlikte
/// silindi) reference-only mirasçısı burasıdır (fiziksel kaldırma #100).
/// Bit-identical construction: `AxisMeasurement { value, Scip }`.
pub fn uniform_scip_reference_projection(raw: RawPosition) -> MeasuredRawPosition {
    let axis = |value: f64| crate::coords::AxisMeasurement {
        value,
        source: MetricSource::Scip,
    };
    MeasuredRawPosition {
        coupling: axis(raw.x),
        cohesion: axis(raw.y),
        instability: axis(raw.z),
        entropy: axis(raw.w),
        witness_depth: axis(raw.v),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Observation model
// ═══════════════════════════════════════════════════════════════════════════════

/// Native (otorite) lane — commit'in tükettiği engine-native per-axis ölçüm.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NativeLaneObservation {
    /// Token'ın measured value bits + engine-native sources.
    pub raw: RawMeasurementObservation,
    /// Q5 — token raw üzerinde (claim.computed_raw ile bit-exact aynı değer).
    pub q5: LaneQ5Observation,
}

/// Reference (uniform-Scip) lane — AYNI value bits, legacy provenance temsili.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReferenceLaneObservation {
    /// AYNI value bits + [Scip; 5] (construction property — value parity test pinler).
    pub raw: RawMeasurementObservation,
    /// Q5 — aynı raw bits → aynı θ/verdict (yine de explicit taşınır).
    pub q5: LaneQ5Observation,
    /// Yalnız `q5 == Evaluated{Passed}` iken `Some` — counterfactual production
    /// `PredicateGate.evaluate` (reference measured ile). **Asla "production
    /// observed" olarak yorumlanmaz — serialization'da lane kimliği belirgin.**
    pub downstream: Option<ShadowDownstreamObservation>,
}

/// Finalize sonrası downstream — üç-durum (`Q4SyntaxRejection` arm'ı YOK —
/// structural Q4 draft aşamasında yakalanır, observation hiç üretilmez).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceDownstreamObservation {
    /// Üretim outcome'u (authoritative — native lane'in gerçek sonucu).
    Observed {
        predicate_completion: PredicateCompletion,
        mutation_decision: crate::trajectory::MutationDecision,
    },
    NotReached {
        reason: ProvenanceNotReachedReason,
    },
    ReachedButUnavailable {
        reason: ProvenanceUnavailableReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceNotReachedReason {
    Q5Violated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceUnavailableReason {
    Q6RuleViolationAfterPredicateGate,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Draft (pre-commit) → Finalized (consuming)
// ═══════════════════════════════════════════════════════════════════════════════

/// Pre-commit draft — construction boundary (serde YOK, Clone YOK; consuming
/// `finalize` tek üretici). Yalnız measurement BAŞARILI olduğunda üretilir
/// (structural-Q4 reject / measurement failure → draft yok — eligibility).
pub struct ProvenanceAuthorityDriftDraft {
    task_id: crate::trajectory::TaskId,
    claim_id: u64,
    native: NativeLaneObservation,
    reference: ReferenceLaneObservation,
}

impl ProvenanceAuthorityDriftDraft {
    pub fn task_id(&self) -> crate::trajectory::TaskId {
        self.task_id
    }
    pub fn claim_id(&self) -> u64 {
        self.claim_id
    }
    pub fn native(&self) -> &NativeLaneObservation {
        &self.native
    }
    pub fn reference(&self) -> &ReferenceLaneObservation {
        &self.reference
    }

    /// Consuming finalize — yalnız comparison-surviving yollarda çağrılır
    /// (Evaluated/Held/Rejected/retryable Q5/Q6). `Observed` üretim outcome'undan.
    pub fn finalize(
        self,
        downstream: ProvenanceDownstreamObservation,
    ) -> ProvenanceAuthorityDriftObservation {
        ProvenanceAuthorityDriftObservation {
            task_id: self.task_id,
            claim_id: self.claim_id,
            native: self.native,
            reference: self.reference,
            downstream,
        }
    }
}

/// Final observation — daima serialize edilir (serde'li, untrusted/outbound
/// telemetry; task identity kontrolü consumer'a bırakılmış).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceAuthorityDriftObservation {
    pub task_id: crate::trajectory::TaskId,
    pub claim_id: u64,
    pub native: NativeLaneObservation,
    pub reference: ReferenceLaneObservation,
    pub downstream: ProvenanceDownstreamObservation,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Observer
// ═══════════════════════════════════════════════════════════════════════════════

/// **#96 MD-2:** Aynı ölçüm event'i üzerinde native↔reference provenance drift
/// gözlemi. Commit'ten ÖNCE çağrılır (finalize outcome ile). Motoru mutasyona
/// uğratmaz (hypothetical ölçüm yok — token zaten üretilmiş; yalnız Q5 değerlendirme
/// + counterfactual gate, ikisi de salt-okunur).
///
/// Sözleşmeler:
/// - **SAME value bits:** iki lane'in raw bit'leri construction olarak eşittir
///   (reference, token raw'ının izdüşümü) — test pinler.
/// - **Q5 paylaşımı:** tek captured `EffectiveVisionGateContext` ile native lane
///   değerlendirilir; reference aynı raw bits → aynı Q5 (clone; explicit taşınır).
/// - **Counterfactual yasağı:** reference downstream yalnız Q5 Passed iken ve
///   `PredicateGate.evaluate` production çağrısıyla üretilir; serialize edilen
///   tipte lane kimliği belirgindir ("asla production observed değil").
pub fn observe_provenance_authority_drift(
    engine: &SpaceEngine,
    claim: &Claim,
    task: &Task,
    token: &NativeLegacySubjectMeasurement,
    loss_before: f64,
    target: &RawPosition,
) -> ProvenanceAuthorityDriftDraft {
    // Tek captured vision context (MD-1 observer ile aynı disiplin).
    let ctx_result = engine
        .effective_vision_gate_context(claim)
        .map_err(map_q5_observation_failure);
    let raw = token.raw();
    let q5 = match &ctx_result {
        Ok(c) => evaluate_lane_q5(engine, raw, c),
        Err(reason) => LaneQ5Observation::NotEvaluated { reason: *reason },
    };

    let native = NativeLaneObservation {
        raw: raw_observation(raw, measured_sources(token.measured())),
        q5: q5.clone(),
    };

    // Reference lane: AYNI value bits + uniform-Scip izdüşümü.
    let reference_measured = uniform_scip_reference_projection(raw);
    let reference = ReferenceLaneObservation {
        raw: raw_observation(raw, measured_sources(&reference_measured)),
        q5: q5.clone(),
        downstream: match &q5 {
            LaneQ5Observation::Evaluated {
                verdict: EvaluatedQ5Verdict::Passed,
                ..
            } => {
                let gate_out = PredicateGate.evaluate(PredicateGateInput {
                    bound: TaskBoundClaim { claim, task },
                    measured: &reference_measured,
                    loss_before,
                    target,
                });
                Some(ShadowDownstreamObservation {
                    predicate_completion: gate_out.outcome.predicate_completion,
                    mutation_decision: gate_out.outcome.mutation_decision,
                })
            }
            _ => None,
        },
    };

    ProvenanceAuthorityDriftDraft {
        task_id: task.id,
        claim_id: claim.id,
        native,
        reference,
    }
}

/// Navigator + MCP ortak path→finalize eşlemesi (MD-1
/// `v1_downstream_from_engine_commit_error` aynası; **`Q4SyntaxRejection` arm'ı
/// YOK** — structural Q4 draft aşamasında yakalanır, observation hiç üretilmez;
/// engine'in defensive tekrarı SyntaxViolation üretirse emission YOK kalır).
pub fn provenance_downstream_from_engine_commit_error(
    err: &crate::engine::EngineCommitError,
) -> Option<ProvenanceDownstreamObservation> {
    use crate::engine::EngineCommitError;
    match err {
        EngineCommitError::VisionViolation { .. } => {
            Some(ProvenanceDownstreamObservation::NotReached {
                reason: ProvenanceNotReachedReason::Q5Violated,
            })
        }
        EngineCommitError::RuleViolation { .. } => {
            Some(ProvenanceDownstreamObservation::ReachedButUnavailable {
                reason: ProvenanceUnavailableReason::Q6RuleViolationAfterPredicateGate,
            })
        }
        // SyntaxViolation dahil diğer tüm varyantlar → emission YOK (non-surviving).
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_scip_reference_projection_preserves_value_bits() {
        let raw = RawPosition {
            x: 0.75,
            y: 0.6,
            z: 0.75,
            w: 0.46153846153846156,
            v: 0.3496052731635571,
        };
        let m = uniform_scip_reference_projection(raw);
        assert_eq!(m.to_raw().x.to_bits(), raw.x.to_bits());
        assert_eq!(m.to_raw().w.to_bits(), raw.w.to_bits());
        assert_eq!(m.coupling.source, MetricSource::Scip);
        assert_eq!(m.witness_depth.source, MetricSource::Scip);
    }

    /// **#95-B (E4):** subject_authority.rs'den taşındı — `map_q5_observation_failure`
    /// bu modülde yaşamaya devam ettiği için mapping-pin kapsamı da taşındı.
    #[test]
    fn q5_observation_failure_maps_all_vision_context_error_variants() {
        use crate::authorization::{CanonicalVisionSubject, VisionContextError};
        use crate::vision::VisionSource;

        // 8 varyantın tamamı — exhaustiveness compiler contract'ı; bu test
        // representative değerlerle mapping doğruluğunu pinler.
        let cases: Vec<(VisionContextError, Q5ObservationFailure)> = vec![
            (
                VisionContextError::VisionUnavailable,
                Q5ObservationFailure::VisionUnavailable,
            ),
            (
                VisionContextError::VisionAuthorityInsufficient {
                    vision_source: VisionSource::GlobalDefault,
                },
                Q5ObservationFailure::VisionAuthorityInsufficient,
            ),
            (
                VisionContextError::SubjectSourceMismatch {
                    subject: CanonicalVisionSubject::Global,
                    vision_source: VisionSource::RoleProfile,
                },
                Q5ObservationFailure::SubjectSourceMismatch,
            ),
            (
                VisionContextError::NonFiniteVisionAxis { axis: "x" },
                Q5ObservationFailure::NonFiniteVisionAxis,
            ),
            (
                VisionContextError::NonFiniteThetaBound(f64::NAN),
                Q5ObservationFailure::NonFiniteThetaBound,
            ),
            (
                VisionContextError::ThetaBoundOutOfRange(9.0),
                Q5ObservationFailure::ThetaBoundOutOfRange,
            ),
            (
                VisionContextError::UnsupportedSemanticsVersion {
                    field: "role_inference_semver",
                    found: 2,
                    supported: 1,
                },
                Q5ObservationFailure::UnsupportedSemanticsVersion,
            ),
            (
                VisionContextError::CanonicalRoleConversionFailed("role".to_string()),
                Q5ObservationFailure::CanonicalRoleConversionFailed,
            ),
        ];
        assert_eq!(
            cases.len(),
            8,
            "VisionContextError 8 varyantlı — plan v5 P1"
        );
        for (err, expected) in cases {
            assert_eq!(map_q5_observation_failure(err), expected);
        }
    }
}
