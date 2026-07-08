//! Review application service — tek domain motoru (query + mutation).
//!
//! Query'ler (list/show) read-only — `repository.read()`, `--operator` gerekmez.
//! Mutation'lar (accept/reject) `repository.mutate()` — `expected_basis_digest` precondition
//! (operator'ın gördüğü basis ile karar anındaki aynı — Review 1#1).
//!
//! # Basis-freshness (Review 1#1 + Review 3 son#2)
//! `mutate()` closure'ı içinde (lock altında — yeni TOCTOU penceresi yok):
//! ```text
//! current_digest = node_digest(store'daki node)
//! if current_digest != expected_basis_digest → ReviewError::StaleBasis
//! basis = PresentedBasis::compile(store, id)   // gerçek basis, digest precondition sonrası
//! session.accept/reject(basis, reason)
//! ```
//! Session'ın kendi StaleBasis'i tautolojik (zararsız) — tek mesaja map (Review 4).

use osp_core::anchoring::review::{
    node_digest, NodeDigest, OperatorId, OperatorReviewSession, PresentedBasis,
};
use osp_core::anchoring::store::InMemoryAnchorStore;
use osp_core::anchoring::types::ConceptNodeId;
use osp_core::anchoring::NonEmptyExplanation;

use crate::application::repository::ReviewStoreRepository;
use crate::errors::{PersistedReviewOutput, ReviewError, ReviewMutation};

// ═══════════════════════════════════════════════════════════════════════════════
// Command tipleri — query/mutation ayrımı (Review 3 son#1)
// ═══════════════════════════════════════════════════════════════════════════════

/// Read-only review komutları — `--operator` gerekmez.
#[derive(Debug, Clone)]
pub enum ReviewQuery {
    /// Candidate lane'i listele (candidate_query).
    List,
    /// Tek node detayı (id ile — Candidate/Accepted/SupersededAccepted görüntüler).
    Show(ConceptNodeId),
}

/// Mutation review komutları — `--operator` zorunlu, `expected_basis_digest` precondition.
#[derive(Debug, Clone)]
pub enum ReviewMutationCommand {
    Accept {
        id: ConceptNodeId,
        /// Operator'ın gördüğü basis digest — karar anındaki ile aynı olmalı (Review 1#1).
        expected_basis_digest: NodeDigest,
        reason: String,
    },
    Reject {
        id: ConceptNodeId,
        expected_basis_digest: NodeDigest,
        reason: String,
    },
}

/// Read-only query çıktısı (revision dahil — operator'a güncel revision göstermek için).
#[derive(Debug, Clone)]
pub enum ReviewReadOutput {
    List {
        items: Vec<ReviewListItem>,
        revision: u64,
    },
    Show {
        node: Option<ReviewNodeDetails>,
        revision: u64,
    },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewListItem {
    pub id: String,
    pub canonical: String,
    pub kind: String,
    pub decision_status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewNodeDetails {
    pub id: String,
    pub canonical: String,
    pub kind: String,
    pub decision_status: String,
    /// Basis digest (operator'ın --basis-digest için). Sadece Candidate node'larda.
    pub basis_digest: Option<u64>,
    pub basis_digest_hex: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// ReviewApplicationService
// ═══════════════════════════════════════════════════════════════════════════════

/// Review application service — query + mutation. Repository üzerinden persistent
/// transaction; subcommand ve interactive adapter aynı service'i kullanır.
pub struct ReviewApplicationService<R: ReviewStoreRepository> {
    repo: R,
}

impl<R: ReviewStoreRepository> ReviewApplicationService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Read-only query (list/show). `--operator` gerekmez.
    pub fn execute_query(&self, query: ReviewQuery) -> Result<ReviewReadOutput, ReviewError> {
        let persisted = self.repo.read()?;
        let store = InMemoryAnchorStore::restore_snapshot(persisted.snapshot.clone())
            .map_err(|e| ReviewError::Store(e.to_string()))?;
        let revision = persisted.revision;
        match query {
            ReviewQuery::List => {
                let candidates = store
                    .candidate_query()
                    .map_err(|e| ReviewError::Store(e.to_string()))?;
                let items: Vec<ReviewListItem> = candidates
                    .into_iter()
                    .map(|n| ReviewListItem {
                        id: n.id.0.clone(),
                        canonical: n.canonical.clone(),
                        kind: format!("{:?}", n.node_kind),
                        decision_status: format!("{:?}", n.decision_status),
                    })
                    .collect();
                Ok(ReviewReadOutput::List { items, revision })
            }
            ReviewQuery::Show(id) => {
                // Candidate lane'de ara, yoksa tüm node'larda.
                let candidates = store
                    .candidate_query()
                    .map_err(|e| ReviewError::Store(e.to_string()))?;
                let node = candidates
                    .into_iter()
                    .find(|n| &n.id == &id)
                    .or_else(|| store.graph().nodes_iter().find(|n| &n.id == &id).cloned());
                let details = node.map(|n| {
                    let digest =
                        if n.decision_status == osp_core::anchoring::DecisionStatus::Candidate {
                            Some(node_digest(&n))
                        } else {
                            None
                        };
                    ReviewNodeDetails {
                        id: n.id.0.clone(),
                        canonical: n.canonical.clone(),
                        kind: format!("{:?}", n.node_kind),
                        decision_status: format!("{:?}", n.decision_status),
                        basis_digest: digest.map(|d| d.get()),
                        basis_digest_hex: digest.map(|d| format!("{:016x}", d.get())),
                    }
                });
                Ok(ReviewReadOutput::Show {
                    node: details,
                    revision,
                })
            }
        }
    }

    /// Mutation (accept/reject). `expected_basis_digest` precondition + session accept/reject.
    pub fn execute_mutation(
        &self,
        command: ReviewMutationCommand,
        operator: OperatorId,
    ) -> Result<PersistedReviewOutput, ReviewError> {
        self.repo
            .mutate(|store| match command {
                ReviewMutationCommand::Accept {
                    id,
                    expected_basis_digest,
                    reason,
                } => {
                    let record = apply_review(
                        store,
                        &id,
                        expected_basis_digest,
                        reason,
                        operator.clone(),
                        true, // accept
                    )?;
                    Ok(ReviewMutation {
                        status: "accepted".into(),
                        node_id: record.candidate_id.0.clone(),
                        decision_sequence: record.seq,
                    })
                }
                ReviewMutationCommand::Reject {
                    id,
                    expected_basis_digest,
                    reason,
                } => {
                    let record = apply_review(
                        store,
                        &id,
                        expected_basis_digest,
                        reason,
                        operator.clone(),
                        false, // reject
                    )?;
                    Ok(ReviewMutation {
                        status: "rejected".into(),
                        node_id: record.candidate_id.0.clone(),
                        decision_sequence: record.seq,
                    })
                }
            })
            .map(|(mutation, revision)| PersistedReviewOutput { mutation, revision })
    }
}

/// Basis-freshness precondition + session accept/reject (lock altında — yeni TOCTOU yok).
///
/// `expected_basis_digest` operator'ın gördüğü basis; current ile karşılaştırılır.
/// Eşleşirse gerçek `PresentedBasis::compile` + session. Session'ın StaleBasis'i
/// tautolojik (lock altında digest zaten kontrol edildi) ama zararsız.
fn apply_review(
    store: &mut InMemoryAnchorStore,
    id: &ConceptNodeId,
    expected_basis_digest: NodeDigest,
    reason: String,
    operator: OperatorId,
    accept: bool,
) -> Result<osp_core::anchoring::review::DecisionRecord, ReviewError> {
    // Node'u bul (candidate_query içinde — Candidate-only v1 scope).
    let candidates = store
        .candidate_query()
        .map_err(|e| ReviewError::Store(e.to_string()))?;
    let node = candidates
        .into_iter()
        .find(|n| &n.id == id)
        .ok_or_else(|| ReviewError::NotFound(id.0.clone()))?;

    // Basis-freshness precondition: operator'ın gördüğü digest ile current aynı mı?
    let current_digest = node_digest(&node);
    if current_digest != expected_basis_digest {
        return Err(ReviewError::StaleBasis);
    }

    // Gerçek PresentedBasis (digest precondition sonrası — kapıyı zayıflatmaz).
    let basis =
        PresentedBasis::compile(store, id).map_err(|e| ReviewError::Store(e.to_string()))?;
    let reason = NonEmptyExplanation::new(reason).map_err(|e| ReviewError::Store(e.to_string()))?;

    let mut session = OperatorReviewSession::open_for_operator(operator);
    if accept {
        session
            .accept(store, id, basis, reason)
            .map_err(|e| map_review_error(e))
    } else {
        session
            .reject(store, id, basis, reason)
            .map_err(|e| map_review_error(e))
    }
}

/// osp-core `ReviewError` → CLI `ReviewError` map (tautolojik StaleBasis dahil tek mesaja).
fn map_review_error(e: osp_core::anchoring::review::ReviewError) -> ReviewError {
    use osp_core::anchoring::review::ReviewError as CoreErr;
    match e {
        CoreErr::NotFound(id) => ReviewError::NotFound(id.0),
        CoreErr::StaleBasis { .. } => ReviewError::StaleBasis,
        CoreErr::NotPromotable { current } => ReviewError::NotPromotable(format!("{current:?}")),
        other => ReviewError::Store(other.to_string()),
    }
}
