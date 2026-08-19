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
use crate::trajectory::TaskId;
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
}

/// **#96 (plan v4-FİNAL):** Probe Claim + Q4 STRUCTURAL validation — tek adımda.
///
/// Type-level TOCTOU kapanışı: `finalize` YALNIZ `computed_raw`/`Intent` enjekte
/// eder; structural fields + `claim_id` AYNI object'ten gelir. İki bağımsız
/// `build_claim_from_proposal` çağrısı (probe + final) arasında derleyici
/// garantisi olmaz — bu tip o boşluğu kapatır.
///
/// Sıra (plan v4 Bölüm 1): `try_new` (probe + structural) → engine derives
/// legacy subject → native measurement → `finalize(&token)` → Q4 final-raw
/// finite → `commit_task_claim` (structural + final-raw defensively repeat).
pub struct StructurallyValidatedClaimDraft {
    claim: Claim,
}

impl StructurallyValidatedClaimDraft {
    /// Probe Claim oluştur (placeholder `computed_raw` — ölçüm henüz YOK, raw
    /// finite-check BU aşamada yapılmaz) + Q4 structural validation.
    ///
    /// `claim_id` tek inkrement sözleşmesi: probe ve final Claim AYNI id'yi
    /// taşır (finalize yeni claim üretmez, mevcut object'i tüketir).
    #[allow(
        clippy::result_large_err,
        reason = "EngineCommitError carries MeasurementBindingVerificationError (intentional inline); see measurement.rs layout decision"
    )]
    pub fn try_new(
        proposal: &DeltaProposal,
        placeholder_raw: RawPosition,
        task_id: TaskId,
        agent: AgentId,
        claim_id: ClaimId,
    ) -> Result<Self, ClaimDraftError> {
        let claim = build_claim_from_proposal(proposal, placeholder_raw, task_id, agent, claim_id)
            .map_err(ClaimDraftError::Build)?;
        validate_claim_structure(&claim).map_err(ClaimDraftError::Syntax)?;
        Ok(Self { claim })
    }

    /// Doğrulanmış probe Claim (yalnız okuma — ölçüm/commit bu accessor üzerinden).
    pub fn claim(&self) -> &Claim {
        &self.claim
    }

    /// Final Claim — YALNIZ `computed_raw` + `Intent` enjekte edilir (token'ın
    /// `raw()` değeri); structural fields + `claim_id` aynı object'ten. Consuming:
    /// draft bir kez finalize edilir (çift finalize derleme hatası).
    pub fn finalize(self, measurement: &NativeLegacySubjectMeasurement) -> Claim {
        let raw = measurement.raw();
        let mut claim = self.claim;
        claim.computed_raw = raw;
        claim.intent = Intent::new(claim.author, raw);
        claim
    }
}

impl std::fmt::Debug for StructurallyValidatedClaimDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructurallyValidatedClaimDraft")
            .field("claim", &self.claim)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Not: kapsamlı test envanteri (yarış, construction contract, cross-pin) W8'de;
    // burada yalnız taşınan fonksiyonların bit-identical pin'leri.

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
        let err = StructurallyValidatedClaimDraft::try_new(
            &proposal,
            RawPosition::default(),
            1,
            1,
            1,
        )
        .unwrap_err();
        assert!(matches!(err, ClaimDraftError::Syntax(_)));
    }
}
