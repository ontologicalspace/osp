//! **#95 MD-1 P2-1 (Faz 5 additive):** Subject-authority compatibility observation.
//!
//! MD-1 normatif kuralı (faz8-p2-migration-decisions.md): task-bound measurement
//! subject authority canonical task predicate scope'tur (`task.predicate.scope`).
//! Caller-declared `affected_nodes` measurement authority DEĞİL; yalnızca impact
//! hint veya compatibility observation olarak kullanılabilir.
//!
//! Bu modül MD-1 compatibility semantics'in tek evi:
//!
//! - [`derive_v1_legacy_measurement_subject`] — V1 (legacy) ölçüm subject türetimi
//!   (navigator.rs'ten taşındı; #92 karakterizasyonunun V1 tek truth'u).
//! - [`produce_legacy_subject_measurement`] — issue #95'in "ayrı explicit
//!   compatibility producer"ı: legacy subject + ölçümü TEK yapıda birlikte üretir.
//! - [`observe_subject_authority_drift`] — P2-1 additive shadow observation:
//!   V1 (authoritative, değişmez) ile V2 (canonical task scope, `measure_task_delta`)
//!   ölçüm lanes'ini karşılaştırır. **Caller davranışı değişmez** — gözlem motoru
//!   mutate etmez, authorization digest preimage'lerine girmez.
//!
//! ## Eligibility sözleşmesi (plan v6)
//!
//! Observation eligibility = legacy measurement üretilmiş VE attempt
//! comparison-surviving surface'e ulaşmış olması. Witness disposition eligibility'yi
//! etkilemez:
//!
//! ```text
//! Comparison-surviving:  Evaluated · Held · Rejected · retryable Q4/Q5/Q6 commit errors
//! Non-surviving:         TaskValidation · VisionContextInvalid · operational/SystemFailure
//! ```
//!
//! ## Draft → Finalized (plan v5 P1)
//!
//! Gözlem commit'ten ÖNCE üretilir (post-commit ölçüm yanlış baseline verirdi);
//! downstream yüzeyi ancak commit sonucu bilinince finalize edilebilir.
//! [`SubjectAuthorityDriftObservationDraft`] serde'sizdir — persist/emit edilen
//! [`SubjectAuthorityDriftObservation`] her zaman finalized'dır (tip seviyesinde
//! garanti). `finalize(self, ..)` consuming'dir: aynı draft iki farklı downstream
//! ile finalize edilemez.
//!
//! ## Semantic-neutrality (plan v4 P2-1)
//!
//! P2-1 neutralite garantisi mevcut `Axis::measure` deterministic/side-effect-free
//! TCB contract'ına dayanır; observation yeni bir authority semantics yaratmaz.
//! `measure_task_delta`'nın `BoundMeasurementSession` pre/post/final epoch verify'ı
//! bu sözleşmenin yapısal garantisi (mutable-axis drift → fail-closed typed error).

use crate::agent::DeltaProposal;
use crate::coords::{MeasuredRawPosition, MetricSource, RawPosition};
use crate::engine::SpaceEngine;
use crate::measurement::MeasurementError;
use crate::space::{Edge, Node, NodeId};
use crate::trajectory::{
    MutationDecision, PredicateCompletion, PredicateGate, PredicateGateInput, Task, TaskBoundClaim,
    TaskId,
};
use crate::witness::Claim;

// ═══════════════════════════════════════════════════════════════════════════════
// V1 legacy subject + explicit compatibility producer
// ═══════════════════════════════════════════════════════════════════════════════

/// V1 (legacy) measurement subject derivation — navigator'ın `compute_raw_from_delta`
/// ölçüm scope'u (G2c-2). MD-1 (#92 evidence) karakterizasyonunda tek truth olarak
/// paylaşılır: production caller'lar VE engine-unit theta testleri bu fonksiyonu çağırır.
///
/// **Sıra kontratı (issue #92 review P2-1):** ordered legacy union — `affected_nodes`
/// sırasını koru; daha önce bulunmayan `removed_edges.from` değerlerini proposal
/// sırasıyla append et. HashSet/sort KULLANMA — mass-weighted centroid aggregation
/// sırası `f64` rounding bitlerini etkileyebilir (exact `to_bits()` goldens var).
pub fn derive_v1_legacy_measurement_subject(proposal: &DeltaProposal) -> Vec<NodeId> {
    let mut affected: Vec<NodeId> = proposal.affected_nodes.clone();
    for er in &proposal.removed_edges {
        if !affected.contains(&er.from) {
            affected.push(er.from);
        }
    }
    affected
}

/// **#95 P2-1 explicit compatibility producer çıktısı** — legacy `affected_nodes`
/// ölçümünün subject + raw + measured üçlüsü TEK yapıda birlikte üretilir
/// (construction property; subject override DEĞİL — yalnızca legacy compatibility).
///
/// Field'lar private + tek üretici [`produce_legacy_subject_measurement`]:
/// literal ile sahte subject/raw/measured eşleşmesi derlenemez.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacySubjectMeasurement {
    subject_ids: Vec<NodeId>,
    raw: RawPosition,
    /// Uniform-Scip **compatibility projection** (navigator/MCP legacy davranışı,
    /// MD-2 konusu). Engine-native per-axis provenance DEĞİL.
    measured: MeasuredRawPosition,
}

impl LegacySubjectMeasurement {
    /// Ordered legacy union (`derive_v1_legacy_measurement_subject` çıktısı).
    pub fn subject_ids(&self) -> &[NodeId] {
        &self.subject_ids
    }

    /// V1 ölçüm raw değeri (`compute_raw_from_delta` çıktısı). Sözleşme:
    /// commit'e giden `claim.computed_raw` ile bit-exact eşit (navigator test pinler).
    pub fn raw(&self) -> RawPosition {
        self.raw
    }

    /// V1 measured — uniform-Scip compatibility projection
    /// (`legacy_compatibility_projection(raw)`).
    pub fn measured(&self) -> &MeasuredRawPosition {
        &self.measured
    }
}

/// **#95 MD-1 P2-1 (EK review tur-2 P2-2):** Uniform-Scip **compatibility
/// projection** — navigator'ın legacy `provenanced_from_raw(raw, Scip)` çağrısının
/// birebir aynı üretimi, MD-1 compatibility semantics'in parçası olarak BU modülde
/// yaşar (`subject_authority → navigator` katman bağımlılığı yok). Bit-identical:
/// aynı `AxisMeasurement { value, source: Scip }` construction.
fn legacy_compatibility_projection(raw: RawPosition) -> MeasuredRawPosition {
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

/// **#95 P2-1:** Legacy `affected_nodes` ölçen ayrı explicit compatibility producer.
/// Navigator ve MCP ölçüm noktalarının tek çağrısı (bit-identical refactor hedefi:
/// aynı iki production çağrısı — derive + `compute_raw_from_delta` — bundle edilir).
pub fn produce_legacy_subject_measurement(
    engine: &SpaceEngine,
    delta_nodes: &[Node],
    delta_edges: &[Edge],
    proposal: &DeltaProposal,
) -> LegacySubjectMeasurement {
    let subject_ids = derive_v1_legacy_measurement_subject(proposal);
    let raw = engine.compute_raw_from_delta(
        delta_nodes,
        delta_edges,
        &proposal.removed_edges,
        &subject_ids,
    );
    let measured = legacy_compatibility_projection(raw);
    LegacySubjectMeasurement {
        subject_ids,
        raw,
        measured,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MeasurementSubjectDigest
// ═══════════════════════════════════════════════════════════════════════════════

/// Ölçümde kullanılan subject id listesinin domain-separated blake3 digest'i.
///
/// **Encoding (adlandırma `osp.measurement-request.v1` mirror):** domain separator +
/// count (u64 LE) + id'ler sırayla (u64 LE). V1 lane **ordered** listeyi
/// (`[1,2,3] ≠ [2,1]`), V2 lane **canonical sorted** `member_ids()`'i hash eder —
/// mass-weighted centroid aggregation sırasının `f64` rounding bitlerini etkilediği
/// gerçeği (#92) digest semantiğinde yaşar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementSubjectDigest([u8; 32]);

impl MeasurementSubjectDigest {
    const DOMAIN_SEPARATOR: &'static [u8] = b"osp.measurement-subject.v1\0";

    pub fn compute(ids: &[NodeId]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(Self::DOMAIN_SEPARATOR);
        hasher.update(&(ids.len() as u64).to_le_bytes());
        for id in ids {
            hasher.update(&id.to_le_bytes());
        }
        Self(hasher.finalize().into())
    }

    pub fn to_hex(&self) -> String {
        const HEX_CHARS: &[u8] = b"0123456789abcdef";
        let mut s = String::with_capacity(64);
        for b in &self.0 {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0xf) as usize] as char);
        }
        s
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Observation model — lane tipleri
// ═══════════════════════════════════════════════════════════════════════════════

/// Ölçüm subject yüzeyi — id listesi + digest (hex).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MeasurementSubjectObservation {
    pub ids: Vec<NodeId>,
    pub digest: String,
}

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

fn map_q5_observation_failure(
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

// ═══════════════════════════════════════════════════════════════════════════════
// V1 downstream — üç durumlu reachability
// ═══════════════════════════════════════════════════════════════════════════════

/// V1 lane downstream yüzeyi — #120 `PipelineReachability` disiplininin V1 tarafı.
///
/// **Q6 `RuleViolation` uyarısı (plan v4 P1-2):** production sırası
/// Q5 → PredicateGate → Q6'dır; RuleViolation anında PredicateGate gerçekten çalışmış
/// olabilir (ör. Completed/AcceptAsCompleted) ama gerçek outcome tuple'ı
/// `EngineCommitError`'da taşınmaz. Navigator'ın sentetik `NotCompleted/Reject`
/// evidence değerleri bu nedenle **authoritative diye taşınmaz** —
/// `ReachedButUnavailable` sınıflandırılır.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V1DownstreamObservation {
    /// Gerçek outcome tuple'ı: Evaluated `task_result.outcome` /
    /// Held+Rejected `authorization.outcome`.
    Observed(AuthoritativeDownstreamObservation),
    NotReached {
        reason: V1DownstreamNotReachedReason,
    },
    ReachedButUnavailable {
        reason: V1DownstreamUnavailableReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V1DownstreamNotReachedReason {
    /// Q5 vision violation — PredicateGate hiç çalışmadı.
    Q5Violated,
    /// Engine-side Q4 syntax rejection. Navigator proposal'u pre-validate ettiği
    /// için pratikte unreachable — explicit match (yeni engine hatası sessiz
    /// sınıflandırılmaz).
    Q4SyntaxRejection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V1DownstreamUnavailableReason {
    /// Q6 rule violation — PredicateGate çalıştı, gerçek outcome error'da yok.
    Q6RuleViolationAfterPredicateGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoritativeDownstreamObservation {
    pub predicate_completion: PredicateCompletion,
    pub mutation_decision: MutationDecision,
}

impl AuthoritativeDownstreamObservation {
    /// Production `AttemptOutcome`'dan — tek mapping noktası.
    pub fn from_outcome(outcome: &crate::trajectory::AttemptOutcome) -> Self {
        Self {
            predicate_completion: outcome.predicate_completion,
            mutation_decision: outcome.mutation_decision,
        }
    }
}

/// **#95 MD-1 P2-1:** Commit hatasından V1 downstream reachability sınıflandırması
/// (navigator + MCP ortak sözleşmesi — path→finalize eşlemesinin tek truth'u).
///
/// Yalnız comparison-surviving retryable hatalar `Some` döner; non-surviving
/// varyantlar `None` — caller observation emit ETMEZ (eligibility contract).
/// Bu `None` sessiz sınıflandırma değildir: surviving/non-surviving sınırının ta
/// kendisidir. **EK review P2-2:** `EngineCommitError`'ın güncel 14 varyantının
/// TAMAMI explicit match — wildcard YOK; yeni retryable varyant eklenirse derleme
/// hatası zorlanır (sessiz taxonomy drift imkânsız).
pub fn v1_downstream_from_engine_commit_error(
    err: &crate::engine::EngineCommitError,
) -> Option<V1DownstreamObservation> {
    use crate::engine::EngineCommitError;
    match err {
        // Comparison-surviving (retryable) — Q4/Q5/Q6.
        EngineCommitError::SyntaxViolation { .. } => Some(V1DownstreamObservation::NotReached {
            reason: V1DownstreamNotReachedReason::Q4SyntaxRejection,
        }),
        EngineCommitError::VisionViolation { .. } => Some(V1DownstreamObservation::NotReached {
            reason: V1DownstreamNotReachedReason::Q5Violated,
        }),
        EngineCommitError::RuleViolation { .. } => {
            Some(V1DownstreamObservation::ReachedButUnavailable {
                reason: V1DownstreamUnavailableReason::Q6RuleViolationAfterPredicateGate,
            })
        }
        // Non-surviving — eligibility contract: emit yok (explicit; yeni varyant
        // derleme hatası üretir).
        EngineCommitError::InvalidWitnessEvidence(_)
        | EngineCommitError::PermissionDenied(_)
        | EngineCommitError::NoPersistence
        | EngineCommitError::Persistence(_)
        | EngineCommitError::Internal(_)
        | EngineCommitError::AuthorizationContextFailed(_)
        | EngineCommitError::VisionContextInvalid(_)
        | EngineCommitError::TaskValidation(_)
        | EngineCommitError::MeasurementBindingMismatch(_)
        | EngineCommitError::MeasurementBindingFailed(_)
        | EngineCommitError::MeasurementBindingVerification(_) => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// V2 lane — shadow measurement + downstream
// ═══════════════════════════════════════════════════════════════════════════════

/// V2 (canonical task scope) lane ölçüm sonucu. `MeasurementFailed` fail-closed
/// typed sınıflandırmadır — sessiz skip YOK; shadow lane failure'ı authoritative
/// lane'i etkilemez (transparency testi pinler).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V2LaneOutcome {
    Measured(V2LaneObservation),
    MeasurementFailed(V2MeasurementFailure),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct V2LaneObservation {
    /// Canonical (sorted) task scope member id'leri.
    pub subject: MeasurementSubjectObservation,
    pub raw: RawMeasurementObservation,
    pub q5: LaneQ5Observation,
    /// Yalnız `q5 == Evaluated{Passed}` iken `Some` — production
    /// `PredicateGate.evaluate` çağrısından (engine.rs commit pipeline'ı ile birebir
    /// aynı input). Q5 Violated/NotEvaluated iken counterfactual üretilmez.
    pub downstream: Option<ShadowDownstreamObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShadowDownstreamObservation {
    pub predicate_completion: PredicateCompletion,
    pub mutation_decision: MutationDecision,
}

/// `measure_task_delta` hatalarının kapalı telemetry sınıflandırması — güncel
/// `MeasurementError`'ın **17 varyantının tamamı** (measurement.rs:2124), wildcard
/// YOK: yeni varyant derleme hatası zorlar. `SubjectScopeHintMismatch` observer
/// hint=None geçtiği için pratikte unreachable — yine de explicit match.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum V2MeasurementFailure {
    CoordinateMeasurement,
    MeasurementContext,
    RevisionComputationFailed,
    MeasurementContextDrift,
    Digest,
    MeasurementContextDigestMismatch,
    ClaimNotTaskBound {
        claim_id: u64,
    },
    TaskBindingMismatch {
        claim_task_id: u64,
        bound_task_id: u64,
    },
    RevisionMismatch,
    HeterogeneousPredicateScopes,
    EmptySubjectScope,
    SubjectScopeResolutionFailed {
        module: String,
    },
    SubjectMemberUnresolvable {
        missing: Vec<NodeId>,
    },
    SubjectMemberMissingAfterDelta {
        node_id: NodeId,
    },
    SubjectScopeHintMismatch,
    InvalidSubjectMass {
        node_id: NodeId,
    },
    InvalidTotalSubjectMass,
}

/// **#96:** pub(crate) — engine'in md1_shadow lane'i telemetry sınıflandırması için
/// kullanır (authority lane'i etkilenmez).
pub(crate) fn map_v2_measurement_failure(err: &MeasurementError) -> V2MeasurementFailure {
    use crate::measurement::{MeasurementError as E, SubjectScopeResolutionError};
    match err {
        E::CoordinateMeasurement(_) => V2MeasurementFailure::CoordinateMeasurement,
        E::MeasurementContext(_) => V2MeasurementFailure::MeasurementContext,
        E::RevisionComputationFailed { .. } => V2MeasurementFailure::RevisionComputationFailed,
        E::MeasurementContextDrift { .. } => V2MeasurementFailure::MeasurementContextDrift,
        E::Digest(_) => V2MeasurementFailure::Digest,
        E::MeasurementContextDigestMismatch => {
            V2MeasurementFailure::MeasurementContextDigestMismatch
        }
        E::ClaimNotTaskBound { claim_id } => V2MeasurementFailure::ClaimNotTaskBound {
            claim_id: *claim_id,
        },
        E::TaskBindingMismatch {
            claim_task_id,
            bound_task_id,
        } => V2MeasurementFailure::TaskBindingMismatch {
            claim_task_id: *claim_task_id,
            bound_task_id: *bound_task_id,
        },
        E::RevisionMismatch { .. } => V2MeasurementFailure::RevisionMismatch,
        E::HeterogeneousPredicateScopes { .. } => {
            V2MeasurementFailure::HeterogeneousPredicateScopes
        }
        E::EmptySubjectScope => V2MeasurementFailure::EmptySubjectScope,
        E::SubjectScopeResolutionFailed(
            SubjectScopeResolutionError::ModuleResolutionUnavailable { module },
        ) => V2MeasurementFailure::SubjectScopeResolutionFailed {
            module: module.clone(),
        },
        E::SubjectMemberUnresolvable { missing } => {
            V2MeasurementFailure::SubjectMemberUnresolvable {
                missing: missing.clone(),
            }
        }
        E::SubjectMemberMissingAfterDelta { node_id } => {
            V2MeasurementFailure::SubjectMemberMissingAfterDelta { node_id: *node_id }
        }
        E::SubjectScopeHintMismatch { .. } => V2MeasurementFailure::SubjectScopeHintMismatch,
        E::InvalidSubjectMass { node_id, .. } => {
            V2MeasurementFailure::InvalidSubjectMass { node_id: *node_id }
        }
        E::InvalidTotalSubjectMass { .. } => V2MeasurementFailure::InvalidTotalSubjectMass,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Draft → Finalized
// ═══════════════════════════════════════════════════════════════════════════════

/// Construction-time gövde — **serde derives YOK**: persist/emit edilemez, yalnızca
/// finalize edilebilir.
///
/// **Construction boundary (EK review tur-2 P1):** alanlar **private** + read-only
/// accessor'lar. Dış caller Draft'ı ne clone edebilir (`Clone` yok), ne literal
/// construct edebilir (private alanlar), ne de component clone'layıp twin Draft
/// üretebilir — yalnızca observer'ın ürettiği capability'yi consuming `finalize`
/// ile tek kez tüketebilir. "Aynı draft iki farklı downstream ile finalize
/// edilemez" invariant'ı böylece tip seviyesinde gerçek.
#[derive(Debug)]
pub struct SubjectAuthorityDriftObservationDraft {
    task_id: TaskId,
    claim_id: crate::witness::ClaimId,
    v1: V1LaneObservationDraft,
    v2: V2LaneOutcome,
}

/// Draft'in V1 lane gövdesi — read-only erişim `SubjectAuthorityDriftObservationDraft::
/// v1()` accessor'ü üzerinden. Final observation üretme capability'si yalnız
/// top-level Draft'ın consuming `finalize`'ındadır (bu tip tek başına finalize
/// edilemez; Clone'u zararsız).
#[derive(Debug, Clone)]
pub struct V1LaneObservationDraft {
    pub subject: MeasurementSubjectObservation,
    pub raw: RawMeasurementObservation,
    pub q5: LaneQ5Observation,
}

impl SubjectAuthorityDriftObservationDraft {
    /// Correlation accessor — parent task kimliği.
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    /// Correlation accessor — parent claim kimliği.
    pub fn claim_id(&self) -> crate::witness::ClaimId {
        self.claim_id
    }

    /// Read-only V1 lane gövdesi (draft characterization testleri buradan okur).
    pub fn v1(&self) -> &V1LaneObservationDraft {
        &self.v1
    }

    /// Read-only V2 lane sonucu.
    pub fn v2(&self) -> &V2LaneOutcome {
        &self.v2
    }

    /// Consuming finalize — aynı draft iki farklı downstream ile finalize edilemez
    /// (ownership taşınır). Production observer path'inde finalized serde tipi
    /// yalnız bu consuming transition üzerinden üretilir. (Final DTO'nun kendisi
    /// public-field + deserialize edilebilir bir **untrusted telemetry** yüzeyidir —
    /// literal kurulum bilinçli olarak mümkün; Draft instance uniqueness ile
    /// final DTO forgeability ayrı iddialardır. EK review tur-3 P2.)
    pub fn finalize(self, downstream: V1DownstreamObservation) -> SubjectAuthorityDriftObservation {
        SubjectAuthorityDriftObservation {
            task_id: self.task_id,
            claim_id: self.claim_id,
            v1: V1LaneObservation {
                subject: self.v1.subject,
                raw: self.v1.raw,
                q5: self.v1.q5,
                downstream,
            },
            v2: self.v2,
        }
    }
}

/// **Final tip** — yalnızca bu tip persist/emit edilir (serde kanalları:
/// `TrajectoryEvidence`, `PendingAuthorization`/`RevisionRequired` wire sidecar'ları,
/// MCP response JSON). Her zaman finalized'dır.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubjectAuthorityDriftObservation {
    pub task_id: TaskId,
    pub claim_id: crate::witness::ClaimId,
    pub v1: V1LaneObservation,
    pub v2: V2LaneOutcome,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct V1LaneObservation {
    /// Ordered legacy union (V1 compatibility subject).
    pub subject: MeasurementSubjectObservation,
    /// V1 raw + uniform-Scip compatibility projection sources.
    pub raw: RawMeasurementObservation,
    pub q5: LaneQ5Observation,
    pub downstream: V1DownstreamObservation,
}

// ═══════════════════════════════════════════════════════════════════════════════
// observe_subject_authority_drift
// ═══════════════════════════════════════════════════════════════════════════════

/// **#95 MD-1 P2-1:** V1/V2 measurement subject-authority drift shadow observation.
///
/// Commit'ten ÖNCE çağrılmalıdır (V2 `measure_task_delta` pre-mutation baseline
/// üzerinden ölçer; post-commit çağrı yanlış baseline verirdi). Engine'i mutate
/// etmez — `measure_task_delta` hypothetical-clone + `BoundMeasurementSession`
/// (pre/post/final epoch verify) üzerinden non-mutating ölçer.
///
/// **Sözleşme:** `legacy.raw() == claim.computed_raw` bit-exact olmalıdır —
/// caller (navigator/MCP) claim'i `legacy.raw()` ile kurar; navigator test'i
/// production wiring'de bu eşitliği pinler.
///
/// **Q5 "same context" construction property (plan v3 P1-4):**
/// `EffectiveVisionGateContext` bir kez capture edilir; V1 ve V2 raw'ları aynı
/// captured context altında değerlendirilir — "same context" test beklentisi
/// değil, construction property'dir. Verdict, production karşılaştırmasının
/// aynısıdır (`theta > theta_bound`; #92 `observe_q5_theta`, captured-context
/// recompute ↔ production violation bit-parity'sini kanıtladı).
///
/// **V2 downstream:** Q5 `Evaluated{Passed}` ise production `PredicateGate.evaluate`
/// ile (engine.rs commit pipeline'ı ile birebir aynı input — shadow pipeline
/// replikasyonu DEĞİL) hesaplanır; aksi halde üretilmez (counterfactual yasak).
/// Bit-exact karşılaştırma için observer'a commit'e giden **aynı** `loss_before` +
/// `target` verilmelidir.
pub fn observe_subject_authority_drift(
    engine: &SpaceEngine,
    claim: &Claim,
    task: &Task,
    native: &crate::engine::NativeAttemptMeasurement,
    loss_before: f64,
    target: &RawPosition,
) -> SubjectAuthorityDriftObservationDraft {
    // **#96 MD-2 re-anchor (plan v4 P1-tur4):** her iki lane AYNI measurement
    // bundle'ından (tek BoundMeasurementSession) — V1 lane authority token'ının
    // native değerleri, V2 lane md1_shadow material'i. Observer'ın kendi
    // `measure_task_delta` çağrısı KALDIRILDI (ikinci bağımsız session YOK —
    // "yalnız subject farkı" iddiası SAME context/session altında geçerli).
    // Provenance iki lane'de de native: gözlem yalnız SUBJECT farkını taşır
    // (dogfood Run A confound'u giderildi).

    // Tek captured vision context — iki lane'in ortak değerlendirme zemini.
    // Context kurulamazsa (ör. GlobalDefault → VisionAuthorityInsufficient,
    // frozen corpus 001 yüzeyi) iki lane'in Q5'sü de aynı NotEvaluated ile
    // sınıflandırılır (aynı claim → aynı context sonucu).
    let shared_context = engine
        .effective_vision_gate_context(claim)
        .map_err(map_q5_observation_failure);

    let authority = native.authority();
    let (v1_q5, shared_ctx) = match &shared_context {
        Ok(ctx) => (evaluate_lane_q5(engine, authority.raw(), ctx), Some(ctx)),
        Err(reason) => (LaneQ5Observation::NotEvaluated { reason: *reason }, None),
    };

    let v1 = V1LaneObservationDraft {
        subject: MeasurementSubjectObservation {
            ids: authority.legacy_subject_ids().to_vec(),
            digest: MeasurementSubjectDigest::compute(authority.legacy_subject_ids()).to_hex(),
        },
        raw: raw_observation(authority.raw(), measured_sources(authority.measured())),
        q5: v1_q5,
    };

    // V2 lane: md1_shadow material (task scope, native) — AYNI session'dan
    // (`measure_attempt_native_with_md1_shadow`). Subject, ölçümün canonical
    // task-scope snapshot'ından ("measured subject"); failure'lar fail-closed
    // telemetry sınıflandırması (authority lane'i etkilenmez).
    let v2 = match native.md1_shadow() {
        Ok(material) => {
            let v2_raw = material.measured().to_raw();
            let v2_q5 = match shared_ctx {
                Some(ctx) => evaluate_lane_q5(engine, v2_raw, ctx),
                None => v1.q5.clone(), // context kurulamadıysa iki lane aynı NotEvaluated
            };
            let downstream = match &v2_q5 {
                LaneQ5Observation::Evaluated {
                    verdict: EvaluatedQ5Verdict::Passed,
                    ..
                } => {
                    let gate_out = PredicateGate.evaluate(PredicateGateInput {
                        bound: TaskBoundClaim { claim, task },
                        measured: material.measured(),
                        loss_before,
                        target,
                    });
                    Some(ShadowDownstreamObservation {
                        predicate_completion: gate_out.outcome.predicate_completion,
                        mutation_decision: gate_out.outcome.mutation_decision,
                    })
                }
                _ => None,
            };
            V2LaneOutcome::Measured(V2LaneObservation {
                subject: MeasurementSubjectObservation {
                    ids: material.subject_ids().to_vec(),
                    digest: MeasurementSubjectDigest::compute(material.subject_ids()).to_hex(),
                },
                raw: raw_observation(v2_raw, measured_sources(material.measured())),
                q5: v2_q5,
                downstream,
            })
        }
        Err(failure) => V2LaneOutcome::MeasurementFailed(failure.clone()),
    };

    SubjectAuthorityDriftObservationDraft {
        task_id: task.id,
        claim_id: claim.id,
        v1,
        v2,
    }
}

/// Tek captured context altında bir raw'ın Q5 yüzeyi. Verdict üretimi production
/// karşılaştırmasının aynısı: `CosineDeviation.theta(raw, effective_vision, space)`
/// + `theta > theta_bound` (engine.rs `check_vision_raw_with_context` gövdesi).
fn evaluate_lane_q5(
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
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// #92 dual exact contract pinning — production helper yanı.
    ///
    /// `derive_v1_legacy_measurement_subject` frozen corpus Case 2/3 proposal
    /// şekilleriyle exact pinlenir. Integration mirror (`tests/common`) aynı
    /// fixture ID'leri için aynı setleri bağımsız pinler (cross-crate assertion
    /// kurulamaz — dual pinning). (navigator.rs'ten taşındı — PR #120 konumları.)
    #[test]
    fn v1_legacy_subject_derivation_pinned_to_frozen_corpus_shapes() {
        use crate::agent::{EdgeRef, NewEdgeSpec};
        use crate::space::EdgeKind;

        // Case 2 shape (wide-affected-scope-001): affected_nodes=[1,2,3],
        // removed_edges boş → subject [1,2,3] (affected sırası korunur).
        let wide = DeltaProposal {
            new_nodes: vec![],
            new_edges: vec![NewEdgeSpec {
                from: 1,
                to: 2,
                kind: EdgeKind::Imports,
            }],
            removed_edges: vec![],
            affected_nodes: vec![1, 2, 3],
            modified_entities: vec![],
            position_hints: vec![],
            reasoning: "case-2 shape".to_string(),
        };
        assert_eq!(
            derive_v1_legacy_measurement_subject(&wide),
            vec![1, 2, 3],
            "Case 2: legacy subject = affected_nodes as-is (ordered)"
        );

        // Case 3 shape (removed-edge-external-source-001): affected_nodes=[1],
        // removed_edges.from=9 (external) → subject [1,9] (unseen from append).
        let removed_external = DeltaProposal {
            new_nodes: vec![],
            new_edges: vec![NewEdgeSpec {
                from: 1,
                to: 9,
                kind: EdgeKind::Imports,
            }],
            removed_edges: vec![EdgeRef {
                from: 9,
                to: 1,
                kind: EdgeKind::Imports,
            }],
            affected_nodes: vec![1],
            modified_entities: vec![],
            position_hints: vec![],
            reasoning: "case-3 shape".to_string(),
        };
        assert_eq!(
            derive_v1_legacy_measurement_subject(&removed_external),
            vec![1, 9],
            "Case 3: legacy subject = affected + unseen removed_edges.from (proposal order)"
        );

        // Sıra kontratı (P2-1): duplicate from → tekrar append edilmez;
        // affected sırası değişmez.
        let dup_from = DeltaProposal {
            new_nodes: vec![],
            new_edges: vec![],
            removed_edges: vec![
                EdgeRef {
                    from: 9,
                    to: 1,
                    kind: EdgeKind::Imports,
                },
                EdgeRef {
                    from: 3,
                    to: 2,
                    kind: EdgeKind::Imports,
                },
                EdgeRef {
                    from: 9,
                    to: 2,
                    kind: EdgeKind::Imports,
                },
            ],
            affected_nodes: vec![3, 1],
            modified_entities: vec![],
            position_hints: vec![],
            reasoning: "order contract".to_string(),
        };
        assert_eq!(
            derive_v1_legacy_measurement_subject(&dup_from),
            vec![3, 1, 9],
            "ordered legacy union: affected order preserved; unseen from appended once, in proposal order"
        );
    }

    #[test]
    fn measurement_subject_digest_is_deterministic_and_order_sensitive() {
        // Determinism.
        assert_eq!(
            MeasurementSubjectDigest::compute(&[1, 2, 3]),
            MeasurementSubjectDigest::compute(&[1, 2, 3])
        );

        // Order sensitivity — V1 ordered semantiği: aggregation sırası rounding
        // bitlerini etkiler, digest exact listeyi parmak izler.
        assert_ne!(
            MeasurementSubjectDigest::compute(&[1, 2, 3]),
            MeasurementSubjectDigest::compute(&[3, 2, 1])
        );

        // Boş liste geçerli (V1 affected boş + removed_edges yok durumunda ölçüm
        // delta_nodes fallback'ine gider — digest yine de tanımlı).
        let empty = MeasurementSubjectDigest::compute(&[]);
        assert_eq!(empty.to_hex().len(), 64);

        // Hex sabit uzunluk + lowercase.
        let hex = MeasurementSubjectDigest::compute(&[7]).to_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

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

    #[test]
    fn v2_measurement_failure_maps_all_measurement_error_variants() {
        use crate::measurement::{CanonicalSubjectScope, MeasurementDigestError};

        // Exhaustiveness compiler contract'ı: map_v2_measurement_failure wildcard'sız
        // exhaustive match — yeni MeasurementError varyantı derleme hatası üretir.
        // Bu test representative değerlerle mapping doğruluğunu pinler.
        // (MeasurementContextDrift + RevisionMismatch outer payload'ları private
        // digest newtype'larıdır — test-side construction yok; kolları yukarıdaki
        // compiler contract kapsar. Toplam varyant sayısı 17.)
        let single_scope = CanonicalSubjectScope::try_new(vec![1]).expect("single-node scope");
        let cases: Vec<(MeasurementError, V2MeasurementFailure)> =
            vec![
            (
                MeasurementError::CoordinateMeasurement(
                    crate::coords::CoordinateMeasurementError::EmptySourceSet,
                ),
                V2MeasurementFailure::CoordinateMeasurement,
            ),
            (
                MeasurementError::MeasurementContext(
                    crate::authorization::CanonicalizationError::DuplicateNodeId(9),
                ),
                V2MeasurementFailure::MeasurementContext,
            ),
            (
                MeasurementError::RevisionComputationFailed {
                    detail: "digest".to_string(),
                },
                V2MeasurementFailure::RevisionComputationFailed,
            ),
            (
                MeasurementError::Digest(MeasurementDigestError::NonFiniteRejected),
                V2MeasurementFailure::Digest,
            ),
            (
                MeasurementError::MeasurementContextDigestMismatch,
                V2MeasurementFailure::MeasurementContextDigestMismatch,
            ),
            (
                MeasurementError::ClaimNotTaskBound { claim_id: 3 },
                V2MeasurementFailure::ClaimNotTaskBound { claim_id: 3 },
            ),
            (
                MeasurementError::TaskBindingMismatch {
                    claim_task_id: 1,
                    bound_task_id: 2,
                },
                V2MeasurementFailure::TaskBindingMismatch {
                    claim_task_id: 1,
                    bound_task_id: 2,
                },
            ),
            (
                MeasurementError::HeterogeneousPredicateScopes {
                    scopes: vec![single_scope.clone(), single_scope],
                },
                V2MeasurementFailure::HeterogeneousPredicateScopes,
            ),
            (
                MeasurementError::EmptySubjectScope,
                V2MeasurementFailure::EmptySubjectScope,
            ),
            (
                MeasurementError::SubjectScopeResolutionFailed(
                    crate::measurement::SubjectScopeResolutionError::ModuleResolutionUnavailable {
                        module: "core".to_string(),
                    },
                ),
                V2MeasurementFailure::SubjectScopeResolutionFailed {
                    module: "core".to_string(),
                },
            ),
            (
                MeasurementError::SubjectMemberUnresolvable { missing: vec![4, 5] },
                V2MeasurementFailure::SubjectMemberUnresolvable { missing: vec![4, 5] },
            ),
            (
                MeasurementError::SubjectMemberMissingAfterDelta { node_id: 6 },
                V2MeasurementFailure::SubjectMemberMissingAfterDelta { node_id: 6 },
            ),
            (
                MeasurementError::SubjectScopeHintMismatch {
                    hint_members: vec![1],
                    derived_members: vec![1, 2],
                },
                V2MeasurementFailure::SubjectScopeHintMismatch,
            ),
            (
                MeasurementError::InvalidSubjectMass {
                    node_id: 7,
                    mass: -1.0,
                },
                V2MeasurementFailure::InvalidSubjectMass { node_id: 7 },
            ),
            (
                MeasurementError::InvalidTotalSubjectMass { total_mass: 0.0 },
                V2MeasurementFailure::InvalidTotalSubjectMass,
            ),
        ];
        assert_eq!(
            cases.len(),
            15,
            "15 constructible varyant + compiler-contract kapsamındaki MeasurementContextDrift/RevisionMismatch = 17"
        );
        for (err, expected) in &cases {
            assert_eq!(&map_v2_measurement_failure(err), expected);
        }
    }

    /// **EK review P2-2/P2-3:** surviving/non-surviving sınıflandırmasının mapping
    /// pin'i — 3 surviving varyant exact; non-surviving representative'ları None.
    /// Exhaustiveness compiler contract'ı (wildcard yok — 14 varyant explicit).
    #[test]
    fn v1_downstream_from_engine_commit_error_classifies_surviving_set() {
        use crate::engine::EngineCommitError;

        // Surviving (retryable) — path→finalize eşlemesi.
        let syntax = EngineCommitError::SyntaxViolation {
            violation: crate::agent::SyntaxViolation {
                claim_id: 1,
                detail: "self-import".to_string(),
            },
        };
        assert_eq!(
            v1_downstream_from_engine_commit_error(&syntax),
            Some(V1DownstreamObservation::NotReached {
                reason: V1DownstreamNotReachedReason::Q4SyntaxRejection,
            })
        );

        let vision = EngineCommitError::VisionViolation {
            violation: crate::engine::VisionViolation {
                claim_id: 1,
                theta: 0.9,
                raw: RawPosition::default(),
            },
            bound: 0.3,
        };
        assert_eq!(
            v1_downstream_from_engine_commit_error(&vision),
            Some(V1DownstreamObservation::NotReached {
                reason: V1DownstreamNotReachedReason::Q5Violated,
            })
        );

        let rule = EngineCommitError::RuleViolation {
            violation: crate::rule::RuleViolation {
                rule_id: "test".to_string(),
                detail: "sentinel".to_string(),
                severity: crate::rule::RuleSeverity::Hard,
            },
        };
        assert_eq!(
            v1_downstream_from_engine_commit_error(&rule),
            Some(V1DownstreamObservation::ReachedButUnavailable {
                reason: V1DownstreamUnavailableReason::Q6RuleViolationAfterPredicateGate,
            })
        );

        // Non-surviving representative'lar — emit yok (eligibility contract).
        assert_eq!(
            v1_downstream_from_engine_commit_error(&EngineCommitError::Internal(
                "system failure".to_string()
            )),
            None
        );
        assert_eq!(
            v1_downstream_from_engine_commit_error(&EngineCommitError::NoPersistence),
            None
        );
        assert_eq!(
            v1_downstream_from_engine_commit_error(&EngineCommitError::PermissionDenied(
                "task not found".to_string()
            )),
            None
        );
    }

    #[test]
    fn finalized_observation_serde_roundtrip_and_draft_has_no_serde() {
        // Final tip serde roundtrip — lane değerleri representative.
        let finalized = SubjectAuthorityDriftObservationDraft {
            task_id: 1,
            claim_id: 2,
            v1: V1LaneObservationDraft {
                subject: MeasurementSubjectObservation {
                    ids: vec![1, 2, 3],
                    digest: MeasurementSubjectDigest::compute(&[1, 2, 3]).to_hex(),
                },
                raw: RawMeasurementObservation {
                    bits: [1, 2, 3, 4, 5],
                    sources: [MetricSource::Scip; 5],
                },
                q5: LaneQ5Observation::Evaluated {
                    theta_bits: 42,
                    theta_bound_bits: 7,
                    verdict: EvaluatedQ5Verdict::Passed,
                },
            },
            v2: V2LaneOutcome::Measured(V2LaneObservation {
                subject: MeasurementSubjectObservation {
                    ids: vec![1],
                    digest: MeasurementSubjectDigest::compute(&[1]).to_hex(),
                },
                raw: RawMeasurementObservation {
                    bits: [9, 8, 7, 6, 5],
                    sources: [
                        MetricSource::TreeSitter,
                        MetricSource::Placeholder,
                        MetricSource::TreeSitter,
                        MetricSource::Heuristic,
                        MetricSource::Heuristic,
                    ],
                },
                q5: LaneQ5Observation::Evaluated {
                    theta_bits: 40,
                    theta_bound_bits: 7,
                    verdict: EvaluatedQ5Verdict::Passed,
                },
                downstream: Some(ShadowDownstreamObservation {
                    predicate_completion: PredicateCompletion::Completed,
                    mutation_decision: MutationDecision::AcceptAsCompleted,
                }),
            }),
        }
        .finalize(V1DownstreamObservation::Observed(
            AuthoritativeDownstreamObservation {
                predicate_completion: PredicateCompletion::Completed,
                mutation_decision: MutationDecision::AcceptAsCompleted,
            },
        ));

        let json = serde_json::to_string(&finalized).expect("finalized observation serializes");
        let parsed: SubjectAuthorityDriftObservation =
            serde_json::from_str(&json).expect("finalized observation roundtrips");
        assert_eq!(parsed, finalized);
    }

    /// **#95 MD-1 P2-1 (plan v6 §5-3):** Q5 reachability — V2 lane `Violated` ise
    /// downstream üretilmez (counterfactual yasak). Fixture: UserLoaded adversarial
    /// vision (authority yeterli → context evaluable) + ölçümü vision'dan uzak subject.
    #[test]
    fn md1_q5_violated_v2_downstream_not_reached() {
        use crate::agent::NewEdgeSpec;
        use crate::space::{EdgeKind, NodeKind};

        // Production built-in axis set (engine.rs make_measurement_engine mirror).
        let cs = crate::coords::CoordinateSystem::default_raw_five(
            crate::coords::MetricSource::TreeSitter,
            crate::axes::CohesionAxis::try_with_observed_source(crate::coords::MetricSource::Scip)
                .unwrap(),
            crate::axes::EntropyAxis::from_commit_entropy(6.5),
            crate::axes::WitnessDepthAxis::from_witness(0.5, 3),
        )
        .unwrap();
        // UserLoaded + adversarial yön — Global subject source inherit eder
        // (UserLoaded authority yeterli; GlobalDefault reject edilmez).
        let vision = crate::vision::VisionVector::with_source(
            RawPosition {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
                v: 0.0,
            },
            crate::vision::VisionSource::UserLoaded,
        );
        let mut space = crate::space::Space::new();
        space.insert_node(crate::space::Node {
            id: 1,
            kind: NodeKind::Module,
            mass: 1.0,
            ..Default::default()
        });
        space.insert_node(crate::space::Node {
            id: 2,
            kind: NodeKind::Module,
            mass: 1.0,
            ..Default::default()
        });
        space.insert_edge(crate::space::Edge {
            from: 1,
            to: 2,
            kind: EdgeKind::Imports,
            is_type_only: false,
        });
        let engine = SpaceEngine::new(space, cs, vision, {
            let mut config = crate::engine::EngineConfig::default_calibrated();
            // Tight bound — adversarial vision altında θ ≈ 0.26 net Violated marjı verir.
            config.theta_bound = 0.1;
            config
        });

        let proposal = DeltaProposal {
            new_nodes: vec![], // delta node YOK → Global subject → UserLoaded inherit
            new_edges: vec![NewEdgeSpec {
                from: 1,
                to: 2,
                kind: EdgeKind::Imports,
            }],
            removed_edges: vec![],
            affected_nodes: vec![1],
            modified_entities: vec![],
            position_hints: vec![],
            reasoning: "q5 violated fixture".to_string(),
        };
        let task = Task {
            id: 1,
            milestone_id: 1,
            label: "Q5 violated fixture".into(),
            target_predicate_set: crate::trajectory::PredicateSet {
                mode: crate::trajectory::PredicateMode::All,
                predicates: vec![crate::trajectory::WeightedPredicate {
                    predicate: crate::trajectory::MetricPredicate {
                        metric: crate::trajectory::PredicateAxis::Coupling,
                        operator: crate::trajectory::ComparisonOp::Le,
                        threshold: 10.0,
                        scope: crate::trajectory::PredicateScope::Node(1),
                        required_source: None,
                        tolerance: 0.0,
                    },
                    weight: None,
                }],
                preferred_vector: None,
            },
            policy: crate::trajectory::TaskPolicy::default(),
            allowed_operations: vec![],
            constraints: vec![],
            status: crate::trajectory::TaskStatus::Pending,
        };

        let draft_claim = crate::task_measurement::StructurallyValidatedClaimDraft::try_new(
            &proposal,
            RawPosition::default(),
            task.id,
            100,
            1,
        )
        .expect("draft (probe + structural Q4)");
        let native = engine
            .measure_attempt_native_with_md1_shadow(&draft_claim, &proposal, &task)
            .expect("native measurement");
        let claim = draft_claim
            .finalize(native.authority())
            .expect("subject binding: draft ve token ayni proposal");
        let target = RawPosition::default();

        let draft = observe_subject_authority_drift(&engine, &claim, &task, &native, 0.0, &target);

        // Her iki lane de Evaluated olmalı (UserLoaded authority) ve Violated
        // (ölçüm vision'dan uzak) — aynı captured context altında.
        assert!(
            matches!(
                &draft.v1.q5,
                LaneQ5Observation::Evaluated {
                    verdict: EvaluatedQ5Verdict::Violated,
                    ..
                }
            ),
            "V1 lane must be Evaluated Violated: {:?}",
            draft.v1.q5
        );
        match &draft.v2 {
            V2LaneOutcome::Measured(v2) => {
                assert!(
                    matches!(
                        &v2.q5,
                        LaneQ5Observation::Evaluated {
                            verdict: EvaluatedQ5Verdict::Violated,
                            ..
                        }
                    ),
                    "V2 lane must be Evaluated Violated: {:?}",
                    v2.q5
                );
                assert!(
                    v2.downstream.is_none(),
                    "Q5 Violated → counterfactual downstream üretilmez (PipelineReachability)"
                );
            }
            V2LaneOutcome::MeasurementFailed(f) => panic!("fixture V2 must measure: {f:?}"),
        }
    }
}
