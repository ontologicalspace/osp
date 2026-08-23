//! **#96 MD-2 / #95-A — shared task-measurement boundary (neutral home).**
//!
//! Plan v4-FİNAL (Faz 8a, 2026-08-18): navigator da MCP de (ayrı crate) kullanacağı
//! claim-projection + Q4-structural truth burada yaşar — MCP navigator
//! implementation katmanına bağımlı OLMAZ (`legacy_compatibility_projection`'ın
//! `subject_authority.rs`'e taşınmasındaki aynı layering gerekçesi) ve Q4 logic'i
//! KOPYALANMAZ (tek truth source).
//!
//! İçerik:
//! - `build_claim_from_proposal` + `node_from_spec` + `ClaimBuildError`
//!   (navigator.rs'ten taşındı; navigator pub re-export korur — test compat).
//! - `validate_claim_structure` / `validate_raw_position_finite` — engine Q4'ünün
//!   iki bileşeni neutral pub fn olarak (motor zaten içsel ayırıyordu; Commit 4b).
//!   Ontoloji: **structural syntax = proposal/claim gerçeği** (measurement'tan
//!   önce); **raw finiteness = measurement gerçeği** (measurement'tan sonra).
//! - `StructurallyValidatedClaimDraft` — probe Claim + Q4 structural validation
//!   tek adımda; consuming `finalize(&NativeLegacySubjectMeasurement)` YALNIZ
//!   computed_raw/Intent enjekte eder (structural fields + claim_id aynı
//!   object'ten) → probe↔final structural TOCTOU type-level kapalı. #95-A
//!   `CheckedTaskMeasurement` boundary'sinin doğal temeli.

use crate::agent::{DeltaProposal, SyntaxViolation};
use crate::coords::RawPosition;
use crate::engine::EngineCommitError;
use crate::measurement::NativeLegacySubjectMeasurement;
use crate::space::{Edge, EdgeKind, Node, NodeId};
use crate::trajectory::{Task, TaskId};
use crate::witness::{AgentId, Claim, ClaimId, Intent};

/// Claim build hatası (navigator.rs'ten taşındı — davranış aynen).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimBuildError {
    /// DeltaProposal'da node/edge yok (empty proposal).
    EmptyProposal,
}

impl std::fmt::Display for ClaimBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaimBuildError::EmptyProposal => write!(f, "DeltaProposal has no nodes/edges"),
        }
    }
}

/// INV-T4 (boşluk #3) — DeltaProposal + computed_raw + task_id → Claim (task-bound).
/// navigator.rs'ten taşındı (bit-identical — davranış değişikliği YOK).
///
/// **#96 not:** computed_raw artık `NativeLegacySubjectMeasurement.raw()`'dan gelir
/// (session-bound native ölçüm); placeholder raw probe Claim için kullanılır.
pub fn build_claim_from_proposal(
    proposal: &DeltaProposal,
    computed_raw: RawPosition,
    task_id: TaskId,
    agent: AgentId,
    claim_id: ClaimId,
) -> Result<Claim, ClaimBuildError> {
    // G2c-2: empty check — removed_edges veya affected_nodes varsa proposal boş değil.
    // (sadece additive delta değil, subtractive delta da geçerli proposal).
    if proposal.new_nodes.is_empty()
        && proposal.new_edges.is_empty()
        && proposal.removed_edges.is_empty()
    {
        return Err(ClaimBuildError::EmptyProposal);
    }
    // NewNodeSpec → Node (resolve: connected_to ile yeni ID'ler ata).
    let delta_nodes: Vec<Node> = proposal
        .new_nodes
        .iter()
        .enumerate()
        .map(|(i, spec)| node_from_spec(spec, i))
        .collect();
    // NewEdgeSpec → Edge.
    let mut delta_edges: Vec<Edge> = proposal
        .new_edges
        .iter()
        .map(|spec| Edge {
            from: spec.from,
            to: spec.to,
            kind: spec.kind,
            is_type_only: false,
        })
        .collect();
    // connected_to edge'leri delta_edges'e ekle (NewNodeSpec.connected_to).
    for (i, spec) in proposal.new_nodes.iter().enumerate() {
        let node_id = delta_nodes[i].id;
        for (target, kind) in &spec.connected_to {
            delta_edges.push(Edge {
                from: node_id,
                to: *target,
                kind: *kind,
                is_type_only: false,
            });
        }
    }
    let intent = Intent::new(agent, computed_raw);
    Ok(Claim {
        id: claim_id,
        intent,
        author: agent,
        computed_raw,
        delta_nodes,
        delta_edges,
        task_id: Some(task_id),
        removed_edges: proposal.removed_edges.clone(), // G2c-2: subtractive delta
    })
}

/// NewNodeSpec → Node (navigator.rs'ten taşındı — ID atama `10_000 + index`
/// sözleşmesi aynen; #96 P1-tur-approve: agent node ID SEÇEMEZ).
pub fn node_from_spec(spec: &crate::agent::NewNodeSpec, index: usize) -> Node {
    Node {
        id: (10_000 + index as NodeId), // yeni node ID'leri (mevcut ID'lerle çakışmaması için)
        kind: spec.kind,
        mass: spec.initial_mass,
        ..Default::default()
    }
}

/// **#96 (PR #124 review tur-2 P1):** Effective legacy measure set — draft ile
/// producer'ın TEK truth'tan kullandığı hesap: `derive_v1_legacy_measurement_subject`
/// ordered union; boşsa delta node id'leri (legacy fallback). Draft'ın
/// `legacy_subject_binding` capture'ı ile token'ın audited subject'i bu hesapla
/// hizalı kalır (farklı hesap = sessiz binding drift).
pub fn effective_legacy_measure_set(proposal: &DeltaProposal, delta_nodes: &[Node]) -> Vec<NodeId> {
    let derived = crate::subject_authority::derive_v1_legacy_measurement_subject(proposal);
    if derived.is_empty() {
        delta_nodes.iter().map(|n| n.id).collect()
    } else {
        derived
    }
}

/// **#96 (plan v4 P1-tur2):** Q4 STRUCTURAL validation — engine
/// `check_claim_structure`'ının neutral pub hâli (logic bit-identical taşındı;
/// engine metodu buna delege eder). `claim.computed_raw`'a DOKUNMAZ — raw
/// finiteness measurement gerçeğidir (`validate_raw_position_finite`).
///
/// Structural syntax = proposal/claim gerçeği → fallible measurement'tan ÖNCE
/// çalışır (Q4-vs-measurement precedence: structural-Q4-invalid proposal +
/// measurement-failing task → sonuç daima SyntaxViolation).
#[allow(
    clippy::result_large_err,
    reason = "EngineCommitError carries MeasurementBindingVerificationError (intentional inline); see measurement.rs layout decision"
)]
pub fn validate_claim_structure(claim: &Claim) -> Result<(), EngineCommitError> {
    // 1. Node validation
    for node in &claim.delta_nodes {
        if !node.mass.is_finite() || node.mass < 0.0 {
            return Err(EngineCommitError::SyntaxViolation {
                violation: SyntaxViolation {
                    claim_id: claim.id,
                    detail: format!(
                        "node {} has invalid mass: {} (must be finite, non-negative)",
                        node.id, node.mass
                    ),
                },
            });
        }
    }

    // 2. Duplicate node IDs within delta
    let mut seen_ids: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    for node in &claim.delta_nodes {
        if !seen_ids.insert(node.id) {
            return Err(EngineCommitError::SyntaxViolation {
                violation: SyntaxViolation {
                    claim_id: claim.id,
                    detail: format!("duplicate node id {} in delta_nodes", node.id),
                },
            });
        }
    }

    // 3. Edge validation
    for edge in &claim.delta_edges {
        // Imports self-loop: module cannot import itself (semantic rule)
        if edge.kind == EdgeKind::Imports && edge.from == edge.to {
            return Err(EngineCommitError::SyntaxViolation {
                violation: SyntaxViolation {
                    claim_id: claim.id,
                    detail: format!("self-import edge: node {} imports itself", edge.from),
                },
            });
        }
    }

    Ok(())
}

/// **#96 (plan v4 P1-tur2):** RawPosition finite-check — engine
/// `check_raw_position_finite`'ın neutral pub hâli (bit-identical). Raw
/// finiteness = measurement gerçeği → native measurement'TAN SONRA, final Claim
/// üzerinde çağrılır (`computed_raw = token.raw()`).
#[allow(
    clippy::result_large_err,
    reason = "EngineCommitError carries MeasurementBindingVerificationError (intentional inline); see measurement.rs layout decision"
)]
pub fn validate_raw_position_finite(
    claim_id: ClaimId,
    source_label: &str,
    raw: &RawPosition,
) -> Result<(), EngineCommitError> {
    let axes = [
        ("x", raw.x),
        ("y", raw.y),
        ("z", raw.z),
        ("w", raw.w),
        ("v", raw.v),
    ];
    for (name, val) in &axes {
        if !val.is_finite() {
            return Err(EngineCommitError::SyntaxViolation {
                violation: SyntaxViolation {
                    claim_id,
                    detail: format!("{}.{} is not finite: {}", source_label, name, val),
                },
            });
        }
    }
    Ok(())
}

/// **#96 draft error:** claim build hatası (empty/malformed proposal) ile
/// structural Q4 hatasının ayrımı — navigator/MCP evidence + calibration
/// akışları build hatasını, Q4 hatası Q4-precedence akışını kullanır.
/// (`EngineCommitError` Clone DEĞİL — draft error da Clone değil; consuming.)
#[derive(Debug)]
pub enum ClaimDraftError {
    Build(ClaimBuildError),
    /// `EngineCommitError::SyntaxViolation` — engine Q4 truth yüzeyi aynen.
    Syntax(EngineCommitError),
    /// **#95-A (MD-1):** `canonical_task_subject_scope(task)` başarısız
    /// (Module scope / heterojen / empty — TerminalTaskDeclaration family).
    /// Navigator/MCP shared disposition tablosuyla mapler (terminal — retry
    /// YOK). İsim bilinçli dar (reviewer tur-3 P2): her `MeasurementError`
    /// task-declaration DEĞİLDİR — yalnız subject-scope türetimi.
    TaskSubjectScope(crate::measurement::MeasurementError),
}

/// **#96 (plan v4-FİNAL + PR #124 review tur-2 P1) → #95-A (MD-1 subject
/// cutover):** Probe Claim + Q4 STRUCTURAL validation + **canonical task
/// subject scope binding capture** — tek adımda.
///
/// Type-level TOCTOU kapanışı: `finalize` YALNIZ `computed_raw`/`Intent` enjekte
/// eder; structural fields + `claim_id` AYNI object'ten gelir.
///
/// **Subject binding (#95-A sonrası semantik):** draft'ın capture ettiği
/// `LegacySubjectBindingDigest` **canonical task predicate scope**'un
/// (=`canonical_task_subject_scope(task)`) digest'idir; `finalize(&token)`
/// token'ın taşıdığı ile karşılaştırır — draft×token subject identity binding
/// (`LegacySubjectBindingMismatch`). *(Fiziksel "legacy" adı #95-B'ye kadar
/// kalır — pre-#95-A'da proposal-affected-union binding'anı taşırdı.)*
/// `affected_nodes` subject authority DEĞİLDİR (advisory impact hint — MD-1).
///
/// Sıra: `try_new` (probe + Q4 structural + scope capture) → engine native
/// measurement (task scope subject) → `finalize(&token)` → Q4 final-raw finite
/// → `commit_task_claim` (MD-1 subject fence + 5-fence defensively repeat).
pub struct StructurallyValidatedClaimDraft {
    claim: Claim,
    /// **#95-A:** canonical task scope binding digest'i (field adı legacy —
    /// fiziksel yeniden adlandırma #95-B).
    legacy_subject_binding: crate::measurement::LegacySubjectBindingDigest,
}

impl StructurallyValidatedClaimDraft {
    /// Probe Claim oluştur (placeholder `computed_raw` — ölçüm henüz YOK, raw
    /// finite-check BU aşamada yapılmaz) + Q4 structural validation + **task
    /// subject scope binding capture**.
    ///
    /// `claim_id` tek inkrement sözleşmesi: probe ve final Claim AYNI id'yi
    /// taşır (finalize yeni claim üretmez, mevcut object'i tüketir).
    ///
    /// Sıra kontratı: Q4 structural ÖNCE (structural-invalid + scope-invalid
    /// çiftinde Q4 kazanır — epistemik precedence korunur), sonra scope.
    #[allow(
        clippy::result_large_err,
        reason = "EngineCommitError carries MeasurementBindingVerificationError (intentional inline); see measurement.rs layout decision"
    )]
    pub fn try_new(
        proposal: &DeltaProposal,
        placeholder_raw: RawPosition,
        task: &Task,
        agent: AgentId,
        claim_id: ClaimId,
    ) -> Result<Self, ClaimDraftError> {
        let claim = build_claim_from_proposal(proposal, placeholder_raw, task.id, agent, claim_id)
            .map_err(ClaimDraftError::Build)?;
        validate_claim_structure(&claim).map_err(ClaimDraftError::Syntax)?;
        // **#95-A (MD-1):** subject authority = canonical task predicate scope.
        // Tek truth free fn — engine authority lane + commit fence aynı
        // fonksiyondan türetir; draft buradan private capture eder.
        let subject_scope = crate::measurement::canonical_task_subject_scope(task)
            .map_err(ClaimDraftError::TaskSubjectScope)?;
        let legacy_subject_binding =
            crate::measurement::LegacySubjectBindingDigest::compute(subject_scope.member_ids());
        Ok(Self {
            claim,
            legacy_subject_binding,
        })
    }

    /// Doğrulanmış probe Claim (yalnız okuma — ölçüm/commit bu accessor üzerinden).
    pub fn claim(&self) -> &Claim {
        &self.claim
    }

    /// **tur-2 P1:** Capture edilen legacy subject binding digest'i (readonly).
    pub fn legacy_subject_binding(&self) -> &crate::measurement::LegacySubjectBindingDigest {
        &self.legacy_subject_binding
    }

    /// Final Claim — YALNIZ `computed_raw` + `Intent` enjekte edilir (token'ın
    /// `raw()` değeri); structural fields + `claim_id` aynı object'ten. Consuming:
    /// draft bir kez finalize edilir (çift finalize derleme hatası).
    ///
    /// **tur-2 P1 subject-binding kontrolü:** token'ın `legacy_subject_binding`
    /// digest'i draft'ın capture ettiğiyle eşit olMALIDIR — eşit değilse
    /// `LegacySubjectBindingMismatch` (aynı structural delta + farklı affected_nodes
    /// artifact mix'i; MD-1 canonical authority ile karışmaz — "Binding" adı bilinçli).
    #[allow(
        clippy::result_large_err,
        reason = "binding error inline (see measurement.rs layout decision)"
    )]
    pub fn finalize(
        self,
        measurement: &NativeLegacySubjectMeasurement,
    ) -> Result<FinalizedNativeTaskClaim, crate::measurement::NativeLegacyMeasurementBindingError>
    {
        if self.legacy_subject_binding != *measurement.legacy_subject_binding() {
            return Err(
                crate::measurement::NativeLegacyMeasurementBindingError::LegacySubjectBindingMismatch {
                    expected: self.legacy_subject_binding,
                    presented: *measurement.legacy_subject_binding(),
                },
            );
        }
        let raw = measurement.raw();
        let mut claim = self.claim;
        claim.computed_raw = raw;
        claim.intent = Intent::new(claim.author, raw);
        Ok(FinalizedNativeTaskClaim::new(claim, measurement.clone()))
    }
}

/// **P0-tur3 (PR review):** Sealed carrier — finalize'ın proof'unu commit boundary'ye
/// kadar taşır. `TaskCommitInput::new` YALNIZ bu tipi kabul eder; ayrı `&Claim +
/// &token` kombinasyonu **type-level unrepresentable** (artifact mix bypass imkânsız).
///
/// Construction: `StructurallyValidatedClaimDraft::finalize(&token)` — tek üretici
/// (constructor private; crate içinden de elle kurulamaz). Private fields:
/// external construction kapalı; `claim()`/`measurement()` accessors read-only.
pub struct FinalizedNativeTaskClaim {
    claim: Claim,
    measurement: NativeLegacySubjectMeasurement,
}

impl FinalizedNativeTaskClaim {
    /// Private — yalnız `finalize` (aynı modül) çağırır; "sealed" iddiası crate
    /// içinde de geçerli (review P2: `pub(crate)` herhangi bir modülün carrier'ı
    /// elle kurmasına izin veriyordu).
    fn new(claim: Claim, measurement: NativeLegacySubjectMeasurement) -> Self {
        Self { claim, measurement }
    }

    /// **Crate-internal test helper (P0-tur4):** Specific measured değeri ile
    /// sealed carrier üretir — YALNIZ `#[cfg(test)]` unit test'lerde (engine.rs,
    /// navigator.rs). External crate'ler (integration tests) erişemez (`pub(crate)`).
    /// Authority tipi forge edilemez dışarıdan.
    ///
    /// **#95-A:** subject parametresi `CanonicalSubjectScope` (vec yerine —
    /// token içsel temsili ile hizalı; duplicate/noncanonical test girdisi
    /// construction'da reddedilir).
    #[cfg(test)]
    pub(crate) fn new_test_with_measured(
        claim: Claim,
        measured: crate::coords::MeasuredRawPosition,
        subject_scope: crate::measurement::CanonicalSubjectScope,
        delta_digest: crate::measurement::MeasurementDeltaDigest,
        base_revision: crate::authorization::SpaceViewRevision,
        measurement_input_digest: crate::authorization::MeasurementInputDigest,
    ) -> Self {
        let measurement = crate::measurement::NativeLegacySubjectMeasurement::new(
            measured,
            subject_scope,
            delta_digest,
            base_revision,
            measurement_input_digest,
            crate::coords::CoreAxisEpochStamp::from_u64s([0; 5]),
        );
        Self { claim, measurement }
    }

    /// Finalized Claim (computed_raw = measurement.raw(); structural aynı object'ten).
    pub fn claim(&self) -> &Claim {
        &self.claim
    }

    /// Authority token (finalize'den gelen — subject-bound).
    pub fn measurement(&self) -> &NativeLegacySubjectMeasurement {
        &self.measurement
    }
}

impl std::fmt::Debug for FinalizedNativeTaskClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinalizedNativeTaskClaim")
            .field("claim", &self.claim)
            .field("measurement", &self.measurement)
            .finish()
    }
}

impl std::fmt::Debug for StructurallyValidatedClaimDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructurallyValidatedClaimDraft")
            .field("claim", &self.claim)
            .finish()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// #96 — MeasurementFailureDisposition (plan v5 tablosu LITERAL; navigator+MCP ortak)
//
// Measurement PRODUCTION failure'ları (17 `MeasurementError` varyantı — exact,
// wildcard YOK). Token-BINDING failure'ları ayrıdır (funnel:
// `EngineCommitError::MeasurementBindingVerification` → SystemFailure).
// `RetryAgentProposal`/`RegenerateMeasurement` bugün üreticisiz — vocabulary +
// unlock notları tabloda (repo prejededi: `DecisionDriftClass`).
// ═══════════════════════════════════════════════════════════════════════════════

/// Pre-commit native measurement failure disposition — maneuver-budget/retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementFailureDisposition {
    /// Task declaration kendisi geçersiz (agent düzeltemez). Budget YOK, LLM YOK.
    TerminalTaskDeclaration,
    /// Claim/task identity wiring hatası. Budget YOK, LLM YOK.
    TerminalIdentityViolation,
    /// Stale token — atomik baseline-refresh kontratı tasarlanmadan AÇILMAZ
    /// (unlock: refresh revision + baseline + loss_before-consistent context +
    /// same proposal + bounded retry). Bugün üreticisiz.
    RegenerateMeasurement,
    /// Agent-shape kaynaklı — budget EVET, LLM retry EVET.
    /// Unlock koşulları: node-removal op / explicit ID allocation /
    /// typed `StructuralCanonicalizationOrigin`. Bugün üreticisiz.
    RetryAgentProposal,
    /// Operational fault / TCB instability / space veri bozukluğu. Budget YOK, LLM YOK.
    SystemFailure,
}

/// **#96 MD-2 P1-1 (PR review tur 5):** navigator + MCP ORTAK typed agent-surface
/// sınıfı — native failure'ların agent/LLM yüzeyinde ne olduğu tek yerden tanımlı.
/// MCP'nin her native failure'ı `RejectedBySyntax` JSON'una flatten etmesi
/// (navigator `SystemFailure` dönerken) bu mapper ile kapanır — gözlenmeyen
/// gate kararının gözlenmiş gibi sunulması (fabrication) YASAK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFailureSurface {
    /// Gerçek structural Q4 — agent'ın kendi delta'sının şekli (draft aşaması;
    /// measurement gerçekleşmedi, sidecar yok). Wire: `RejectedBySyntax`.
    SyntaxRejection,
    /// Agent-correctable — budget EVET, retry EVET (commit-aşaması retryable
    /// Syntax/Vision/Rule ailesi veya dormant measurement disposition'ları).
    /// Wire: retry yüzeyi (gerçek gate kararı ile).
    RetryAgentProposal,
    /// Native authority/binding/TCB/operational fault — agent hatası DEĞİL.
    /// Terminal: budget YOK, LLM retry YOK (navigator `SystemFailure` mirror).
    /// Wire: system failure JSON.
    SystemFailure,
    /// Task resolver'da binding yok (standalone/task not found) — terminal;
    /// navigator `TaskNotFound` mirror. Agent başka task seçer.
    TaskNotFound,
    /// Witness evidence operational fault (malformed/author-self/duplicate) —
    /// terminal; navigator `WitnessEvaluationError` mirror.
    WitnessEvaluationError,
}

impl MeasurementFailureDisposition {
    /// Navigator'ın production arm'ı ile birebir (navigator.rs native-measurement
    /// match): `RetryAgentProposal` → retry; diğerlerinin TAMAMI terminal
    /// SystemFailure (budget yok, LLM retry yok).
    pub fn agent_surface(self) -> NativeFailureSurface {
        match self {
            Self::RetryAgentProposal => NativeFailureSurface::RetryAgentProposal,
            Self::TerminalTaskDeclaration
            | Self::TerminalIdentityViolation
            | Self::RegenerateMeasurement
            | Self::SystemFailure => NativeFailureSurface::SystemFailure,
        }
    }
}

/// **W8-c (review tur-7 P1):** Agent yüzeyinin LLM wire ailesi — MCP JSON
/// şekli bu sınıfla ayrışır (navigator+MCP ortak contract). Bir arm'ın wire
/// anlamının SESSİZCE değişmesi compiler exhaustive-match ile yakalanamaz;
/// bu tablo + `native_failure_surface_wire_shape_table` test'i yakalar.
///
/// **Rol notu (review tur-8 tasarım notu):** MCP wire producer bu tipi henüz
/// TÜKETMİYOR — tip contract/table pin'dir (MCP arm'ları surface'a göre
/// dispatch eder; stage'e özgü class string'leri osp-core'a taşınmadığı için
/// consume etmek yanlış yönde bağlanma üretirdi). Gerçek consume ya da
/// `pub(crate)`'ye daraltma kararı bilinçli olarak #100/W-lane sürecine
/// bırakılmıştır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFailureWireShape {
    /// Gerçek structural Q4 (draft aşaması) — `attempt_outcome{gate_decision:
    /// "RejectedBySyntax"}`; measurement gerçekleşmedi (sidecar yok).
    SyntaxRejectionOutcome,
    /// Agent-correctable retry — `attempt_outcome` + GERÇEK gate kararı
    /// (`RejectedByVision`/`RejectedByRule`; hardcode `RejectedBySyntax`
    /// DEĞİL — eski ontology bug'ının en hassas kolu).
    RetryableRealGateOutcome,
    /// `system_failure` JSON (class: `NativeMeasurementFailed`/
    /// `NativeBindingFailed`/`EngineCommitFailed` — stage'e göre);
    /// `attempt_outcome` ÜRETİLMEZ (fabrication YOK).
    SystemFailureJson,
    /// `error: "task_not_found"` JSON — terminal, agent başka task seçer.
    TaskNotFoundJson,
    /// `system_failure{class: "WitnessEvidenceInvalid"}` JSON — terminal.
    WitnessEvidenceSystemFailureJson,
}

impl NativeFailureSurface {
    /// Surface → wire ailesi (5/5). Yeni surface varyanti bu match'i derleme
    /// hatasına zorlar; anlam ataması table-test ile pinli.
    pub fn wire_shape(self) -> NativeFailureWireShape {
        match self {
            Self::SyntaxRejection => NativeFailureWireShape::SyntaxRejectionOutcome,
            Self::RetryAgentProposal => NativeFailureWireShape::RetryableRealGateOutcome,
            Self::SystemFailure => NativeFailureWireShape::SystemFailureJson,
            Self::TaskNotFound => NativeFailureWireShape::TaskNotFoundJson,
            Self::WitnessEvaluationError => {
                NativeFailureWireShape::WitnessEvidenceSystemFailureJson
            }
        }
    }
}

/// `EngineCommitError` → agent yüzeyi — navigator'ın production commit-error
/// arm'ları ile birebir sınıflandırma (navigator.rs exhaustive match'in
/// sınıflandırma yüzüi; side-effect'ler navigator'da kalır). MCP bu mapper'ı
/// wire sınıfı için kullanır — iki tüketici tek tabloya pinlenir.
pub fn commit_error_agent_surface(err: &crate::engine::EngineCommitError) -> NativeFailureSurface {
    use crate::engine::EngineCommitError;
    match err {
        // Retryable (agent-correctable) — budget tüketir, evidence+feedback, continue.
        EngineCommitError::SyntaxViolation { .. }
        | EngineCommitError::VisionViolation { .. }
        | EngineCommitError::RuleViolation { .. } => NativeFailureSurface::RetryAgentProposal,
        // Terminal — witness evidence operational fault.
        EngineCommitError::InvalidWitnessEvidence(_) => {
            NativeFailureSurface::WitnessEvaluationError
        }
        // Terminal — task binding yok (task not found / standalone claim).
        EngineCommitError::PermissionDenied(_) => NativeFailureSurface::TaskNotFound,
        // Terminal system failure — operational / TCB / native-authority family
        // (persistence, internal, fail-closed context, task-validation, measurement
        // binding mismatch/derivation/verification — NativeAuthority kontratı:
        // SystemFailure | budget yok | LLM retry yok).
        EngineCommitError::NoPersistence
        | EngineCommitError::Persistence(_)
        | EngineCommitError::Internal(_)
        | EngineCommitError::AuthorizationContextFailed(_)
        | EngineCommitError::VisionContextInvalid(_)
        | EngineCommitError::TaskValidation(_)
        | EngineCommitError::MeasurementBindingMismatch(_)
        | EngineCommitError::MeasurementBindingFailed(_)
        | EngineCommitError::MeasurementBindingVerification(_) => {
            NativeFailureSurface::SystemFailure
        }
    }
}

/// 17 varyantın exact eşlemesi (plan v5/#96 v4-FİNAL tablosu; wildcard YOK —
/// yeni varyant derleme hatası zorlar).
pub fn measurement_failure_disposition(
    err: &crate::measurement::MeasurementError,
) -> MeasurementFailureDisposition {
    use crate::measurement::MeasurementError;
    match err {
        // TerminalIdentityViolation — claim/task binding wiring.
        MeasurementError::ClaimNotTaskBound { .. }
        | MeasurementError::TaskBindingMismatch { .. } => {
            MeasurementFailureDisposition::TerminalIdentityViolation
        }
        // TerminalTaskDeclaration — task yazarı hatası; agent ID SEÇEMEZ
        // (production builder `10_000 + index` atar — explicit ID allocation
        // gelirse yeniden değerlendirme).
        MeasurementError::HeterogeneousPredicateScopes { .. }
        | MeasurementError::EmptySubjectScope
        | MeasurementError::SubjectScopeResolutionFailed(..)
        | MeasurementError::SubjectMemberUnresolvable { .. } => {
            MeasurementFailureDisposition::TerminalTaskDeclaration
        }
        // SystemFailure — grammar'da node silme YOK (`removed_nodes` yok);
        // agent düzeltemez. Node-removal op gelirse normatif yeniden değerlendirme.
        MeasurementError::SubjectMemberMissingAfterDelta { .. } => {
            MeasurementFailureDisposition::SystemFailure
        }
        // SystemFailure — atomik baseline-refresh kontratı olmadan regenerate,
        // eski current_measured/loss_before ↔ yeni-space measurement karışımı üretir.
        MeasurementError::RevisionMismatch { .. } => MeasurementFailureDisposition::SystemFailure,
        // SystemFailure — interior-mutability threat / measurement TCB instability.
        MeasurementError::MeasurementContextDrift { .. } => {
            MeasurementFailureDisposition::SystemFailure
        }
        // SystemFailure — caller kod hatası (production None geçirir).
        MeasurementError::SubjectScopeHintMismatch { .. } => {
            MeasurementFailureDisposition::SystemFailure
        }
        // SystemFailure — defensive invariant / construction / computation faults.
        MeasurementError::MeasurementContextDigestMismatch
        | MeasurementError::MeasurementContext(..)
        | MeasurementError::RevisionComputationFailed { .. } => {
            MeasurementFailureDisposition::SystemFailure
        }
        // SystemFailure (fail-closed) — `Digest` tüm arm'lar: StructuralCanonicalization
        // hem agent-delta hem task-scope kaynaklı üretilebiliyor; `detail: String`
        // ontolojik sebebi ayıramaz, string parsing YASAK. Typed
        // `StructuralCanonicalizationOrigin` sonrası agent-shape arm'ı
        // RetryAgentProposal'a AÇILABİLİR (unlock).
        MeasurementError::Digest(_) => MeasurementFailureDisposition::SystemFailure,
        // SystemFailure — axis machinery / space veri bozukluğu (operator).
        MeasurementError::CoordinateMeasurement(..)
        | MeasurementError::InvalidSubjectMass { .. }
        | MeasurementError::InvalidTotalSubjectMass { .. } => {
            MeasurementFailureDisposition::SystemFailure
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Not: kapsamlı test envanteri (yarış, construction contract, cross-pin) W8'de;
    // burada yalnız taşınan fonksiyonların bit-identical pin'leri.

    /// **P1-1 (review tur 5):** disposition → agent yüzeyi tablosu — navigator
    /// arm'ı ile birebir. Yeni disposition eklendiğinde bu test derleme hatası
    /// verir (wildcard YOK).
    #[test]
    fn measurement_failure_disposition_agent_surface_table() {
        use MeasurementFailureDisposition as D;
        use NativeFailureSurface as S;
        assert_eq!(D::TerminalTaskDeclaration.agent_surface(), S::SystemFailure);
        assert_eq!(
            D::TerminalIdentityViolation.agent_surface(),
            S::SystemFailure
        );
        assert_eq!(D::RegenerateMeasurement.agent_surface(), S::SystemFailure);
        assert_eq!(D::SystemFailure.agent_surface(), S::SystemFailure);
        assert_eq!(D::RetryAgentProposal.agent_surface(), S::RetryAgentProposal);
    }

    /// **P1-1:** commit-error → agent yüzeyi tablosu — navigator'ın production
    /// commit-error arm'ları ile birebir (retryable Syntax/Vision/Rule; task
    /// binding; witness evidence; kalanı system failure).
    #[test]
    fn commit_error_agent_surface_table() {
        use crate::engine::EngineCommitError;
        use NativeFailureSurface as S;

        let retryable = [
            EngineCommitError::SyntaxViolation {
                violation: crate::agent::SyntaxViolation {
                    claim_id: 1,
                    detail: "test".into(),
                },
            },
            EngineCommitError::RuleViolation {
                violation: crate::rule::RuleViolation {
                    rule_id: "test".into(),
                    detail: "test".into(),
                    severity: crate::rule::RuleSeverity::Hard,
                },
            },
        ];
        for e in &retryable {
            assert_eq!(
                commit_error_agent_surface(e),
                S::RetryAgentProposal,
                "retryable family: {e:?}"
            );
        }
        assert_eq!(
            commit_error_agent_surface(&EngineCommitError::PermissionDenied(
                "task not found".into()
            )),
            S::TaskNotFound
        );
        assert_eq!(
            commit_error_agent_surface(&EngineCommitError::InvalidWitnessEvidence(
                "malformed".into()
            )),
            S::WitnessEvaluationError
        );
        assert_eq!(
            commit_error_agent_surface(&EngineCommitError::NoPersistence),
            S::SystemFailure
        );
        assert_eq!(
            commit_error_agent_surface(&EngineCommitError::Internal("x".into())),
            S::SystemFailure
        );
    }

    /// **W8-a (#96 MD-2 review tur-5/6 — P1 kapanışı):** 17-varyant disposition
    /// tablosunun TAM envanter pin'i. Mapping fn zaten compiler-exhaustive; bu
    /// test MEVCUT atamaların (plan v5/#96 v4-FİNAL frozen tablo) sessizce
    /// değişmediğini pinler — yanlış satır taşırması regression olarak yakalanır.
    ///
    /// **Dormant producer envanteri:** `RetryAgentProposal` ve
    /// `RegenerateMeasurement` disposition'larına BUGÜN hiçbir MeasurementError
    /// üreticisi YOK (bilinçli — unlock koşulları plan v5 tablo notlarında:
    /// node-removal op / explicit ID allocation / typed structural origin /
    /// atomik baseline-refresh kontratı). Bu dormant durum testin bir parçası
    /// olarak pinlenir: tablo SADECE Terminal*/SystemFailure üretir.
    #[test]
    fn measurement_failure_disposition_full_inventory() {
        use crate::measurement::MeasurementError as E;
        use MeasurementFailureDisposition as D;

        let revision = || crate::authorization::SpaceViewRevision {
            view_id: crate::authorization::SpaceViewId::Ephemeral(0),
            sequence: 0,
            content_digest: crate::authorization::SpaceDigest::compute(
                &crate::space::Space::default(),
            )
            .unwrap(),
        };
        let input_digest = {
            let cs = crate::coords::CoordinateSystem::default_raw_five(
                crate::coords::MetricSource::Placeholder,
                crate::axes::CohesionAxis::new(),
                crate::axes::EntropyAxis::from_commit_entropy(0.0),
                crate::axes::WitnessDepthAxis::from_witness(0.0, 0),
            )
            .unwrap();
            let ctx = crate::authorization::MeasurementInputContext::try_from(&cs).unwrap();
            crate::authorization::MeasurementInputDigest::compute(&ctx).unwrap()
        };

        // (variant, expected disposition) — 17/17.
        let table: Vec<(E, D)> =
            vec![
            (
                E::CoordinateMeasurement(crate::coords::CoordinateMeasurementError::EmptySourceSet),
                D::SystemFailure,
            ),
            (
                E::MeasurementContext(
                    crate::authorization::CanonicalizationError::DuplicateNodeId(1),
                ),
                D::SystemFailure,
            ),
            (
                E::RevisionComputationFailed { detail: "x".into() },
                D::SystemFailure,
            ),
            (
                E::MeasurementContextDrift {
                    before: input_digest.clone(),
                    after: input_digest.clone(),
                },
                D::SystemFailure,
            ),
            (
                E::Digest(crate::measurement::MeasurementDigestError::NonFiniteRejected),
                D::SystemFailure,
            ),
            (E::MeasurementContextDigestMismatch, D::SystemFailure),
            (E::ClaimNotTaskBound { claim_id: 1 }, D::TerminalIdentityViolation),
            (
                E::TaskBindingMismatch {
                    claim_task_id: 1,
                    bound_task_id: 2,
                },
                D::TerminalIdentityViolation,
            ),
            (
                E::RevisionMismatch {
                    expected: revision(),
                    current: revision(),
                },
                D::SystemFailure,
            ),
            (
                E::HeterogeneousPredicateScopes { scopes: vec![] },
                D::TerminalTaskDeclaration,
            ),
            (E::EmptySubjectScope, D::TerminalTaskDeclaration),
            (
                E::SubjectScopeResolutionFailed(
                    crate::measurement::SubjectScopeResolutionError::ModuleResolutionUnavailable {
                        module: "core".into(),
                    },
                ),
                D::TerminalTaskDeclaration,
            ),
            (
                E::SubjectMemberUnresolvable { missing: vec![7] },
                D::TerminalTaskDeclaration,
            ),
            (
                E::SubjectMemberMissingAfterDelta { node_id: 7 },
                D::SystemFailure,
            ),
            (
                E::SubjectScopeHintMismatch {
                    hint_members: vec![1],
                    derived_members: vec![2],
                },
                D::SystemFailure,
            ),
            (
                E::InvalidSubjectMass {
                    node_id: 7,
                    mass: -1.0,
                },
                D::SystemFailure,
            ),
            (
                E::InvalidTotalSubjectMass { total_mass: 0.0 },
                D::SystemFailure,
            ),
        ];
        assert_eq!(table.len(), 17, "17 varyantın TAMAMI envanterde");
        for (err, expected) in &table {
            assert_eq!(
                &measurement_failure_disposition(err),
                expected,
                "disposition table drift: {err:?} → {expected:?} bekleniyordu"
            );
        }
        // Dormant pin: envanterde RetryAgentProposal/RegenerateMeasurement YOK —
        // bu disposition'lar bugün üreticisiz (yukarıdaki not).
        assert!(
            !table
                .iter()
                .any(|(_, d)| matches!(d, D::RetryAgentProposal | D::RegenerateMeasurement)),
            "dormant disposition'lara üretici eklendiyse bu pin + plan v5 unlock notları güncellenmeli"
        );
    }

    /// **W8-c (review tur-7 P1):** surface → wire ailesi tablosu 5/5 — bir
    /// arm'ın JSON anlamının sessizce değişmesi bu pinle yakalanır (compiler
    /// yalnız YENİ variant eklenmesini yakalar, mevcut atamanın drift'ini değil).
    /// Reachability notu: `TaskNotFound`/`WitnessEvaluationError` MCP
    /// `submit_delta_attempt` yüzeyinden bugün unreachable (tmp_reg her zaman
    /// task'ı içerir; witness evidence inject edilemez) — wire anlamları BU
    /// tabloda pinli, e2e karşılıkları dormant.
    #[test]
    fn native_failure_surface_wire_shape_table() {
        use NativeFailureSurface as S;
        use NativeFailureWireShape as W;
        assert_eq!(S::SyntaxRejection.wire_shape(), W::SyntaxRejectionOutcome);
        assert_eq!(
            S::RetryAgentProposal.wire_shape(),
            W::RetryableRealGateOutcome
        );
        assert_eq!(S::SystemFailure.wire_shape(), W::SystemFailureJson);
        assert_eq!(S::TaskNotFound.wire_shape(), W::TaskNotFoundJson);
        assert_eq!(
            S::WitnessEvaluationError.wire_shape(),
            W::WitnessEvidenceSystemFailureJson
        );
    }

    /// **W8-c:** retryable ailenin GERÇEK gate kararı — eski ontology bug'ının
    /// en hassas kolu (hardcode `RejectedBySyntax` fabrication'ı). VisionViolation
    /// MCP yüzeyinden e2e tetiklenemiyor (vision vector fixture'ı MCP'de
    /// konfigüre edilemez); mapping burada pinlenir, RuleViolation karşılığı
    /// MCP e2e'de (`w8_retryable_rule_violation_emits_real_gate_decision`).
    #[test]
    fn retryable_errors_map_to_real_gate_decisions() {
        use crate::engine::EngineCommitError;
        // VisionViolation → RejectedByVision (RejectedBySyntax DEĞİL).
        let vision_err = EngineCommitError::VisionViolation {
            violation: crate::engine::VisionViolation {
                claim_id: 1,
                theta: 0.9,
                raw: crate::coords::RawPosition::default(),
            },
            bound: 0.1,
        };
        assert_eq!(
            crate::navigator::gate_decision_from_engine_error(&vision_err),
            crate::trajectory::GateDecision::RejectedByVision
        );
        // RuleViolation → RejectedByRule.
        let rule_err = EngineCommitError::RuleViolation {
            violation: crate::rule::RuleViolation {
                rule_id: "test.pin".into(),
                detail: "pin".into(),
                severity: crate::rule::RuleSeverity::Hard,
            },
        };
        assert_eq!(
            crate::navigator::gate_decision_from_engine_error(&rule_err),
            crate::trajectory::GateDecision::RejectedByRule
        );
        // İkisi de retryable surface (wire: RetryableRealGateOutcome).
        assert_eq!(
            commit_error_agent_surface(&vision_err),
            NativeFailureSurface::RetryAgentProposal
        );
        assert_eq!(
            commit_error_agent_surface(&rule_err),
            NativeFailureSurface::RetryAgentProposal
        );
    }

    #[test]
    fn build_claim_from_proposal_empty_proposal_rejected() {
        let proposal = DeltaProposal {
            affected_nodes: vec![1],
            ..Default::default()
        };
        let result = build_claim_from_proposal(&proposal, RawPosition::default(), 1, 1, 1);
        assert_eq!(result.unwrap_err(), ClaimBuildError::EmptyProposal);
    }

    #[test]
    fn draft_try_new_rejects_structurally_invalid_self_import() {
        use crate::space::EdgeKind;
        let proposal = DeltaProposal {
            new_edges: vec![crate::agent::NewEdgeSpec {
                from: 1,
                to: 1, // self-import → structural Q4
                kind: EdgeKind::Imports,
            }],
            ..Default::default()
        };
        // Q4 precedence: geçerli task scope'lu task ile bile structural Q4
        // hatası ÖNCE yüzeye çıkar (scope türetiminden önce).
        let task = test_task_with_node_scope(1);
        let err = StructurallyValidatedClaimDraft::try_new(
            &proposal,
            RawPosition::default(),
            &task,
            1,
            1,
        )
        .unwrap_err();
        assert!(matches!(err, ClaimDraftError::Syntax(_)));
    }

    /// **#95-A:** scope türetimi hataları — draft aşamasında `TaskSubjectScope`
    /// (Q4 BAŞARILI olduktan sonra). Üç aile: Module / heterojen / empty.
    #[test]
    fn draft_try_new_rejects_underivable_task_subject_scope() {
        // (a) Module scope → SubjectScopeResolutionFailed.
        let module_task = test_task_with_module_scope("core");
        let err = StructurallyValidatedClaimDraft::try_new(
            &test_valid_proposal(),
            RawPosition::default(),
            &module_task,
            1,
            1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ClaimDraftError::TaskSubjectScope(
                crate::measurement::MeasurementError::SubjectScopeResolutionFailed(_)
            )
        ));

        // (b) Heterojen scope'lar → HeterogeneousPredicateScopes.
        let mut hetero = test_task_with_node_scope(1);
        hetero
            .target_predicate_set
            .predicates
            .push(crate::trajectory::WeightedPredicate {
                predicate: crate::trajectory::MetricPredicate {
                    metric: crate::trajectory::PredicateAxis::Coupling,
                    operator: crate::trajectory::ComparisonOp::Le,
                    threshold: 1.0,
                    scope: crate::trajectory::PredicateScope::Node(2),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            });
        let err = StructurallyValidatedClaimDraft::try_new(
            &test_valid_proposal(),
            RawPosition::default(),
            &hetero,
            1,
            1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ClaimDraftError::TaskSubjectScope(
                crate::measurement::MeasurementError::HeterogeneousPredicateScopes { .. }
            )
        ));

        // (c) Boş predicate set → EmptySubjectScope.
        let mut empty = test_task_with_node_scope(1);
        empty.target_predicate_set.predicates.clear();
        let err = StructurallyValidatedClaimDraft::try_new(
            &test_valid_proposal(),
            RawPosition::default(),
            &empty,
            1,
            1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ClaimDraftError::TaskSubjectScope(
                crate::measurement::MeasurementError::EmptySubjectScope
            )
        ));
    }

    /// **#95-A Q4-precedence:** structural-invalid + scope-failing çiftinde Q4
    /// KAZANIR (scope hatası gözlemlenmez).
    #[test]
    fn draft_try_new_q4_syntax_precedes_scope_failure() {
        use crate::space::EdgeKind;
        let proposal = DeltaProposal {
            new_edges: vec![crate::agent::NewEdgeSpec {
                from: 1,
                to: 1, // structural Q4 ihlali
                kind: EdgeKind::Imports,
            }],
            ..Default::default()
        };
        let module_task = test_task_with_module_scope("core"); // scope da fail ederdi
        let err = StructurallyValidatedClaimDraft::try_new(
            &proposal,
            RawPosition::default(),
            &module_task,
            1,
            1,
        )
        .unwrap_err();
        assert!(
            matches!(err, ClaimDraftError::Syntax(_)),
            "Q4 önce — scope hatası değil: {err:?}"
        );
    }

    fn test_valid_proposal() -> DeltaProposal {
        DeltaProposal {
            new_nodes: vec![crate::agent::NewNodeSpec {
                kind: crate::space::NodeKind::Module,
                initial_mass: 1.0,
                connected_to: vec![],
            }],
            affected_nodes: vec![1],
            ..Default::default()
        }
    }

    fn test_task_with_node_scope(node: u64) -> Task {
        Task {
            id: 1,
            milestone_id: 1,
            label: "node-scope test task".into(),
            target_predicate_set: crate::trajectory::PredicateSet {
                mode: crate::trajectory::PredicateMode::All,
                predicates: vec![crate::trajectory::WeightedPredicate {
                    predicate: crate::trajectory::MetricPredicate {
                        metric: crate::trajectory::PredicateAxis::Coupling,
                        operator: crate::trajectory::ComparisonOp::Le,
                        threshold: 10.0,
                        scope: crate::trajectory::PredicateScope::Node(node),
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
        }
    }

    fn test_task_with_module_scope(name: &str) -> Task {
        let mut task = test_task_with_node_scope(1);
        task.target_predicate_set.predicates[0].predicate.scope =
            crate::trajectory::PredicateScope::Module(name.to_string());
        task
    }
}
