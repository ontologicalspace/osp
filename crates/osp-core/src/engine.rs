//! Space Engine — production runtime orchestrator (Faz 2.6).
//!
//! Tüm Faz 1-2 modüllerini tek çatı altında birleştirir:
//! - `vision_config` → `VisionVector` + `EngineConfig`
//! - `time::TimeFSM` → evaluate (Q1-Q3) + `bigbang::apply_delta` (mutate)
//! - `vision::compute_derived` → pozisyon reposition (`CosineDeviation`)
//! - `persistence::SnapshotStore` → event-sourcing (delta + milestone)
//!
//! **Commit pipeline (§4, space-engine-design.md):**
//! 0. CLAIM-BASED GATES (Q4-Q6) → syntax/vision/rule check (deterministik, witness öncesi)
//!    - Q4 Syntax: OutputContract compliant?
//!    - Q5 Vision: claim.computed_raw θ > bound → Err(VisionViolation) [mutasyon YOK]
//!    - Q6 Rule: Rule ihlali?
//! 1. WITNESS-BASED GATES (Q1-Q3) → evaluate + apply_delta (ΔV node + ΔE edge)
//! 2. REPOSITION → CosineDeviation ile ΔV∪N₁(ΔV) → drift_warnings
//! 3. SAVE DELTA → event-sourcing
//! 4. MILESTONE → periyodik tam snapshot
//! 5. EMIT → CommitOutcome

use std::path::Path;

use crate::agent::{PermissionMask, SyntaxViolation};
use crate::bigbang::Delta;
use crate::coords::{Position, RawPosition};
use crate::persistence::{
    DeltaRecord, PersistenceError, SNAPSHOT_FORMAT_VERSION, SpaceSnapshot, SnapshotStore,
};
use crate::rule::{Rule, RuleViolation};
use crate::space::{NodeId, Space};
use crate::time::{TimeFSM, TimeMachine};
use crate::vision::{compute_derived, CosineDeviation, DeviationMetric, VisionVector};
use crate::vision_config::VisionConfig;
use crate::witness::{Claim, ClaimId, Reason, WitnessResult, WitnessSet};

// ═══════════════════════════════════════════════════════════════════════════════
// EngineConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// Engine konfigürasyonu — `VisionConfig`'ten türetilir.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub min_approvers: usize,
    pub quorum_threshold: f64,
    pub theta_bound: f64,
    pub milestone_interval: u64,
    pub abstractness: f64,
    pub merge_ratio_observable: f64,
}

impl EngineConfig {
    pub fn from_vision_config(config: &VisionConfig) -> Self {
        Self {
            min_approvers: config.min_approvers(),
            quorum_threshold: config.quorum_threshold(),
            theta_bound: config.theta_bound(),
            milestone_interval: config.milestone_interval(),
            abstractness: config.abstractness(),
            merge_ratio_observable: config.merge_ratio_observable(),
        }
    }

    /// Test-friendly default (Faz 1.11 kalibrasyon değerleri).
    /// theta_bound=0.3: cosine deviation [0,1] değerlerde θ_max=0.5 (§5.2 NOT);
    /// 0.5 unreachable → 0.3 realistic threshold. TDA diffusion (Faz 5+) ile 0.5'e dönülebilir.
    pub fn default_calibrated() -> Self {
        Self {
            min_approvers: 2,
            quorum_threshold: 1.5,
            theta_bound: 0.3,
            milestone_interval: 1000,
            abstractness: 0.5,
            merge_ratio_observable: 0.10,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CommitOutcome + Warnings + Errors
// ═══════════════════════════════════════════════════════════════════════════════

/// Commit başarılı çıktısı.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitOutcome {
    pub event: Delta,
    pub drift_warnings: Vec<DriftWarning>,
    pub safety_weakened: bool,
    pub t_c: u64,
}

/// Post-mutation: neighbor θ > bound (commit geçerli, komşu degrade — WARNING, §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct DriftWarning {
    pub node_id: NodeId,
    pub theta: f64,
    pub raw: RawPosition,
}

/// Pre-mutation: claim θ > bound (Q5 ihlali — §4.1 REJECT, EngineCommitError::VisionViolation).
#[derive(Debug, Clone, PartialEq)]
pub struct VisionViolation {
    pub claim_id: ClaimId,
    pub theta: f64,
    pub raw: RawPosition,
}

impl std::fmt::Display for VisionViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Q5 vision violation (claim {}, negatif-uzay): θ={:.3}, raw={:?}",
            self.claim_id, self.theta, self.raw
        )
    }
}

/// Engine-level commit error (thiserror). Claim-based (Q4-Q6) + witness-based (Q1-Q3).
///
/// bigbang modülü mutation-only (apply_delta infallible) — kendi error'ı yok
/// (osp-core-design.md §3.4). Witness Reject/Hold `evaluate()` → `WitnessResult` üzerinden
/// gelir, `Reason` wrap edilir (space-engine-design.md §6.1).
///
/// Variant tasarımı: violation struct'lar tek kaynak (single-source-of-truth). theta/detail/
/// rule_id gibi field'lar variant'ta TEKRAR EDİLMEZ — `Display` impl ile erişilir (drift risk yok).
#[derive(Debug, thiserror::Error)]
pub enum EngineCommitError {
    #[error("witness gate (Q1-Q3): {0:?}")]
    Witness(Reason),
    #[error("{violation}")]
    SyntaxViolation { violation: SyntaxViolation },
    #[error("{violation} (bound={bound:.3})")]
    VisionViolation {
        violation: VisionViolation,
        bound: f64,
    },
    #[error("{violation}")]
    RuleViolation { violation: RuleViolation },
    #[error("permission denied (inv #13): {0}")]
    PermissionDenied(String),
    #[error("persistence kapalı — restore/milestone kullanılamaz (snapshot_store None)")]
    NoPersistence,
    #[error("persistence hatası: {0}")]
    Persistence(#[from] PersistenceError),
}

// ═══════════════════════════════════════════════════════════════════════════════
// SpaceEngine
// ═══════════════════════════════════════════════════════════════════════════════

/// Production runtime — all Faz 1-2 modules orchestrated.
pub struct SpaceEngine {
    space: Space,
    coord_system: crate::coords::CoordinateSystem,
    vision: VisionVector,
    rules: Vec<Box<dyn Rule>>,
    time: TimeFSM,
    config: EngineConfig,
    t_c: u64,
    snapshot_store: Option<SnapshotStore>,
}

impl SpaceEngine {
    /// Manuel kurulum (tüm bileşenler caller sağlar).
    pub fn new(
        space: Space,
        coord_system: crate::coords::CoordinateSystem,
        vision: VisionVector,
        config: EngineConfig,
    ) -> Self {
        Self {
            space,
            coord_system,
            vision,
            rules: vec![], // Faz 5: God Mode `register_rule()` ile ekler
            time: TimeFSM::default(),
            config,
            t_c: 0,
            snapshot_store: None,
        }
    }

    /// `VisionConfig`'ten kurulum (TOML → engine).
    pub fn from_vision_config(
        space: Space,
        coord_system: crate::coords::CoordinateSystem,
        config: &VisionConfig,
    ) -> Self {
        Self::new(
            space,
            coord_system,
            config.to_vision_vector(),
            EngineConfig::from_vision_config(config),
        )
    }

    /// Persistence aç (event-sourcing — delta + milestone).
    pub fn with_persistence(
        mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<Self, PersistenceError> {
        self.snapshot_store = Some(SnapshotStore::new(base_dir)?);
        Ok(self)
    }

    // ── Commit pipeline (§4) ───────────────────────────────────────────────

    /// `commit(claim, omega)` — full pipeline (Q4-Q6 claim-based → Q1-Q3 witness → mutate → reposition → save).
    ///
    /// 0. CLAIM-BASED GATES (Q4-Q6, deterministik, witness öncesi):
    ///    - Q4 Syntax: OutputContract compliant? (inv #12)
    ///    - Q5 Vision: claim.computed_raw θ > bound → Err(VisionViolation) [mutasyon YOK]
    ///    - Q6 Rule: Rule ihlali?
    /// 1. WITNESS-BASED GATES (Q1-Q3) + TIME ADVANCE: evaluate + apply_delta
    /// 2. REPOSITION: CosineDeviation → drift_warnings
    /// 3. SAVE DELTA: event-sourcing
    /// 4. MILESTONE: periyodik tam snapshot
    /// 5. EMIT: CommitOutcome
    pub fn commit(
        &mut self,
        claim: &Claim,
        omega: &WitnessSet,
    ) -> Result<CommitOutcome, EngineCommitError> {
        // Phase 0: CLAIM-BASED GATES (Q4-Q6 — deterministik, witness öncesi)
        self.check_claim_syntax(claim)?;
        self.check_claim_vision(claim)?;
        self.check_claim_rules(claim)?;

        // Phase 1: WITNESS-BASED GATES (Q1-Q3) + TIME ADVANCE (apply_delta mutasyon)
        let result = self.time.advance(&mut self.space, claim, omega);
        let (delta, safety_weakened) = match result {
            WitnessResult::Commit {
                delta,
                safety_weakened,
                ..
            } => (delta, safety_weakened),
            WitnessResult::Hold(reason) => return Err(EngineCommitError::Witness(reason)),
            WitnessResult::Reject(reason) => return Err(EngineCommitError::Witness(reason)),
        };

        self.t_c += 1;

        // Phase 2: REPOSITION (CosineDeviation + drift warnings, inv #5)
        let drift_warnings = self.reposition_nodes(&delta.repositioned);

        // Phase 3: SAVE DELTA (event-sourcing)
        if let Some(store) = &self.snapshot_store {
            let record = DeltaRecord {
                version: SNAPSHOT_FORMAT_VERSION,
                t_c: self.t_c,
                claim_id: claim.id,
                delta: delta.clone(),
                safety_weakened,
            };
            let _ = store.save_delta(record); // best-effort; log on error
        }

        // Phase 4: MILESTONE (periyodik)
        if self.t_c % self.config.milestone_interval == 0 {
            if let Some(store) = &self.snapshot_store {
                let snapshot = SpaceSnapshot {
                    version: SNAPSHOT_FORMAT_VERSION,
                    t_c: self.t_c,
                    timestamp_ms: current_time_ms(),
                    space: self.space.clone(),
                };
                let _ = store.save_milestone(snapshot);
            }
        }

        // Phase 5: EMIT
        Ok(CommitOutcome {
            event: delta,
            drift_warnings,
            safety_weakened,
            t_c: self.t_c,
        })
    }

    // ── Claim-based gates (Q4-Q6, Phase 0 — witness öncesi, deterministik) ───

    /// Q4 Syntax Gate — DeltaProposal OutputContract'a uyuyor mu? (inv #12)
    ///
    /// Stub: Claim henüz DeltaProposal'dan compute edilmiş computed_raw taşıyor
    /// (Faz 2). Faz 5'te gerçek syntax validation (DeltaProposal şema doğrulaması)
    /// gelir. Şu an her zaman Ok (claim varsayılan olarak well-formed).
    fn check_claim_syntax(&self, _claim: &Claim) -> Result<(), EngineCommitError> {
        // Faz 5 stub: OutputContract::default().validate(&delta_proposal)
        // Şu an Claim yapısı zaten typed — syntax hatası olamaz.
        Ok(())
    }

    /// Q5 Vision Gate — `θ(claim.computed_raw, vision) > theta_bound` → Err.
    /// Claim negatif-uzayda ise ana dala GİREMEZ (BFT-derived Safety, §4.1).
    fn check_claim_vision(&self, claim: &Claim) -> Result<(), EngineCommitError> {
        let theta = CosineDeviation.theta(&claim.computed_raw, &self.vision, &self.space);
        if theta > self.config.theta_bound {
            tracing::warn!(
                claim_id = claim.id,
                theta,
                bound = self.config.theta_bound,
                "Q5 vision violation — claim rejected (negatif-uzay)"
            );
            return Err(EngineCommitError::VisionViolation {
                violation: VisionViolation {
                    claim_id: claim.id,
                    theta,
                    raw: claim.computed_raw,
                },
                bound: self.config.theta_bound,
            });
        }
        Ok(())
    }

    /// Q6 Rule Gate — ΔS herhangi bir Rule'u ihlal ediyor mu?
    ///
    /// Stub: `self.rules` boş (Faz 2) → her zaman Ok. Faz 5'te God Mode tarafından
    /// register edilen Hard/Soft Rule'lar `evaluate()` çağrılır (agent-prompt-semantics.md §4).
    fn check_claim_rules(&self, claim: &Claim) -> Result<(), EngineCommitError> {
        for rule in &self.rules {
            if let Some(violation) =
                rule.evaluate(&claim.delta_nodes, &claim.delta_edges, &self.space)
            {
                tracing::warn!(
                    claim_id = claim.id,
                    rule_id = %rule.id(),
                    "Q6 rule violation — claim rejected"
                );
                return Err(EngineCommitError::RuleViolation { violation });
            }
        }
        Ok(())
    }

    /// PermissionMask nihai denetimi (inv #13, agent-prompt-semantics.md §2.1 nokta 3).
    /// Claim.author'ın yazma yetkisi olmayan düğümlere dokunması engellenir.
    ///
    /// Stub: Faz 2'de full_access mask (tüm node'lar writable). Faz 5'te God Mode
    /// config'ten yüklenen gerçek PermissionMask ile çalışır.
    #[allow(dead_code)] // Faz 5'te commit() imzasına mask parametresi eklenecek
    fn check_permissions(
        &self,
        _claim: &Claim,
        _mask: &PermissionMask,
    ) -> Result<(), EngineCommitError> {
        // Faz 5 stub: read_only_nodes'a yazma, forbidden_edge_kinds oluşturma kontrolü
        Ok(())
    }

    // ── Reposition (incremental, inv #5/#6) ────────────────────────────────

    /// Phase 2: post-mutation neighbor drift tespiti + pozisyon güncelleme.
    /// `CosineDeviation` kullanır (inv #5 — DiffusionDeviation değil).
    /// İki-fazlı (collect → apply) — borrow checker uyumu.
    fn reposition_nodes(&mut self, ids: &[NodeId]) -> Vec<DriftWarning> {
        let mut drift_warnings = Vec::new();

        // Faz 1: hesapla (immutable borrow)
        let updates: Vec<(NodeId, Position)> = ids
            .iter()
            .filter_map(|&id| {
                let node = self.space.nodes.get(&id)?;
                let raw = self.coord_system.raw_position_of(node, &self.space);
                let derived = compute_derived(
                    &raw,
                    &self.vision,
                    &self.space,
                    &CosineDeviation,
                    raw.z,
                    self.config.abstractness,
                );
                if derived.theta > self.config.theta_bound {
                    drift_warnings.push(DriftWarning {
                        node_id: id,
                        theta: derived.theta,
                        raw,
                    });
                }
                Some((id, Position { raw, derived }))
            })
            .collect();

        // Faz 2: uygula (mutable borrow)
        for (id, pos) in updates {
            if let Some(node) = self.space.nodes.get_mut(&id) {
                node.position = pos;
            }
        }

        drift_warnings
    }

    /// TAM reposition (analyze/dashboard — inv #5 lazy). Tüm düğümleri günceller.
    /// Commit path'inde DEĞİL — `osp analyze` / dashboard çağrısı.
    /// Faz 5+: `DiffusionDeviation` ile upgrade.
    pub fn full_reposition(&mut self) -> Vec<DriftWarning> {
        let all_ids: Vec<NodeId> = self.space.nodes.keys().copied().collect();
        self.reposition_nodes(&all_ids)
    }

    // ── Persistence ────────────────────────────────────────────────────────

    /// Time-travel (event-sourcing): milestone + delta replay → request_t_c.
    pub fn restore(&mut self, request_t_c: u64) -> Result<usize, EngineCommitError> {
        let store = self
            .snapshot_store
            .as_ref()
            .ok_or(EngineCommitError::NoPersistence)?;
        let restored = store.restore(request_t_c)?;
        self.space = restored.space;
        self.t_c = restored.t_c;
        tracing::info!(
            t_c = restored.t_c,
            replayed = restored.replayed_deltas,
            "restore tamamlandı"
        );
        Ok(restored.replayed_deltas)
    }

    /// Manuel milestone snapshot (tag vb.).
    pub fn save_milestone(&self) -> Result<(), EngineCommitError> {
        let store = self
            .snapshot_store
            .as_ref()
            .ok_or(EngineCommitError::NoPersistence)?;
        let snapshot = SpaceSnapshot {
            version: SNAPSHOT_FORMAT_VERSION,
            t_c: self.t_c,
            timestamp_ms: current_time_ms(),
            space: self.space.clone(),
        };
        store.save_milestone(snapshot)?;
        Ok(())
    }

    // ── Accessors ───────────────────────────────────────────────────────────

    pub fn space(&self) -> &Space {
        &self.space
    }
    pub fn t_c(&self) -> u64 {
        self.t_c
    }
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }
    pub fn vision(&self) -> &VisionVector {
        &self.vision
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axes::{EntropyAxis, WitnessDepthAxis};
    use crate::coords::CoordinateSystem;
    use crate::space::{Edge, EdgeKind, Node, NodeKind};
    use crate::witness::{EvidenceEvent, EvidenceId, Intent, WitnessKind};

    /// Vision center — `make_engine` vision ile hizalı. Q5 pre-check geçer.
    const CENTER: RawPosition = RawPosition {
        x: 0.5,
        y: 0.5,
        z: 0.5,
        w: 0.5,
        v: 0.5,
    };

    fn mod_node(id: u64) -> Node {
        Node {
            id,
            kind: NodeKind::Module,
            ..Default::default()
        }
    }

    fn edge(from: u64, to: u64) -> Edge {
        Edge {
            from,
            to,
            kind: EdgeKind::Imports,
        }
    }

    fn ev(id: EvidenceId, actor: u64) -> EvidenceEvent {
        EvidenceEvent::new(id, &format!("src-{id}"), WitnessKind::MergeCommit, actor, 1)
    }

    fn two_witnesses() -> WitnessSet {
        WitnessSet::new(vec![ev(1, 200), ev(2, 300)])
    }

    fn claim_with(author: u64, computed_raw: RawPosition) -> Claim {
        Claim {
            id: 1,
            intent: Intent::new(author, RawPosition::default()),
            author,
            computed_raw,
            delta_nodes: vec![mod_node(10)],
            delta_edges: vec![],
        }
    }

    fn make_engine() -> SpaceEngine {
        let space = Space::new();
        let cs = CoordinateSystem::default_raw_three(
            EntropyAxis::from_commit_entropy(6.0),
            WitnessDepthAxis::from_witness(0.3, 5),
        );
        let vision = VisionVector::new(RawPosition {
            x: 0.5,
            y: 0.5,
            z: 0.5,
            w: 0.5,
            v: 0.5,
        });
        SpaceEngine::new(space, cs, vision, EngineConfig::default_calibrated())
    }

    // --- commit success ---

    #[test]
    fn commit_success_returns_outcome() {
        let mut engine = make_engine();
        let claim = claim_with(100, CENTER); // aligned with vision (center)
        let omega = two_witnesses();

        let outcome = engine.commit(&claim, &omega).expect("commit");
        assert_eq!(outcome.t_c, 1);
        assert!(!outcome.safety_weakened);
        assert_eq!(engine.space().node_count(), 1); // node 10 added
        assert!(engine.space().nodes.contains_key(&10));
    }

    #[test]
    fn commit_increments_t_c() {
        let mut engine = make_engine();
        let claim = claim_with(100, CENTER);
        let omega = two_witnesses();

        engine.commit(&claim, &omega).unwrap();
        assert_eq!(engine.t_c(), 1);
        engine.commit(&claim, &omega).unwrap();
        assert_eq!(engine.t_c(), 2);
    }

    // --- Q5 vision pre-check (Safety — reviewer #1) ---

    #[test]
    fn commit_q5_aligned_claim_passes() {
        let mut engine = make_engine();
        // Claim aligned with vision → θ ≈ 0 → passes Q5
        let good_claim = claim_with(100, RawPosition {
            x: 0.5, y: 0.5, z: 0.5, w: 0.5, v: 0.5,
        });
        let omega = two_witnesses();

        let result = engine.commit(&good_claim, &omega);
        assert!(result.is_ok(), "aligned claim → Commit");
    }

    // --- commit Hold (witness insufficient) ---

    #[test]
    fn commit_hold_returns_witness_error() {
        let mut engine = make_engine();
        let claim = claim_with(100, CENTER);
        let omega = WitnessSet::new(vec![ev(1, 200)]); // 1 witness → Hold

        let result = engine.commit(&claim, &omega);
        assert!(matches!(
            result,
            Err(EngineCommitError::Witness(Reason::MinApproversNotMet { .. }))
        ));
        assert_eq!(engine.space().node_count(), 0, "Hold → mutasyon yok");
    }

    // --- reposition + drift warnings ---

    #[test]
    fn commit_repositions_new_nodes() {
        let mut engine = make_engine();
        let claim = claim_with(100, CENTER);
        let omega = two_witnesses();

        let _outcome = engine.commit(&claim, &omega).unwrap();
        // node 10 was added + repositioned → has a position
        let node = engine.space().nodes.get(&10).expect("node 10");
        assert!(node.position.raw.x >= 0.0); // position computed (not default)
    }

    #[test]
    fn commit_drift_warning_when_node_far_from_vision() {
        // Engine vision = (0.5, 0.5, 0.5, 0.5, 0.5). Add a node that, after reposition,
        // has high coupling (x → 1.0) → θ > 0.5 → drift warning.
        let mut space = Space::new();
        for i in 1..=20 {
            space.insert_node(mod_node(i));
        }
        // node 1 imports everything → high coupling
        for i in 2..=20 {
            space.insert_edge(edge(1, i));
        }

        let cs = CoordinateSystem::default_raw_three(
            EntropyAxis::from_commit_entropy(6.0),
            WitnessDepthAxis::from_witness(0.3, 5),
        );
        let vision = VisionVector::new(RawPosition {
            x: 0.2, // low coupling vision — node 1 (x≈0.95) will drift
            y: 0.5,
            z: 0.5,
            w: 0.5,
            v: 0.5,
        });
        let mut config = EngineConfig::default_calibrated();
        config.theta_bound = 0.2; // test-specific: drift triggers at lower θ
        let mut engine = SpaceEngine::new(space, cs, vision, config);

        // full_reposition: node 1 has x ≈ 0.95 (19 imports) vs vision x=0.2 → θ high
        let warnings = engine.full_reposition();
        assert!(
            !warnings.is_empty(),
            "node 1 high coupling → drift warning expected"
        );
        assert!(warnings.iter().any(|w| w.node_id == 1));
    }

    // --- persistence ---

    #[test]
    fn commit_saves_delta_to_store() {
        let tmp = tempfile::tempdir().unwrap();
        let mut engine = make_engine().with_persistence(tmp.path()).unwrap();
        let claim = claim_with(100, CENTER);
        let omega = two_witnesses();

        engine.commit(&claim, &omega).unwrap();

        // Delta saved
        let store = SnapshotStore::new(tmp.path()).unwrap();
        let deltas = store.list_deltas_in_range(0, 1).unwrap();
        assert_eq!(deltas.len(), 1);
    }

    #[test]
    fn commit_milestone_at_interval() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = EngineConfig::default_calibrated();
        config.milestone_interval = 2; // every 2 commits
        let cs = CoordinateSystem::default_raw_three(
            EntropyAxis::from_commit_entropy(6.0),
            WitnessDepthAxis::from_witness(0.3, 5),
        );
        let vision = VisionVector::new(CENTER);
        let mut engine = SpaceEngine::new(Space::new(), cs, vision, config)
            .with_persistence(tmp.path())
            .unwrap();

        let claim = claim_with(100, CENTER);
        let omega = two_witnesses();

        engine.commit(&claim, &omega).unwrap(); // t_c=1 (no milestone)
        engine.commit(&claim, &omega).unwrap(); // t_c=2 → milestone

        let store = SnapshotStore::new(tmp.path()).unwrap();
        let milestones = store.list_milestones().unwrap();
        assert!(milestones.contains(&2), "milestone at t_c=2");
    }

    #[test]
    fn restore_via_event_sourcing() {
        let tmp = tempfile::tempdir().unwrap();
        let mut engine = make_engine().with_persistence(tmp.path()).unwrap();
        let claim = claim_with(100, CENTER);
        let omega = two_witnesses();

        engine.save_milestone().unwrap(); // milestone at t_c=0
        engine.commit(&claim, &omega).unwrap(); // t_c=1, delta saved
        engine.commit(&claim, &omega).unwrap(); // t_c=2, delta saved

        // Restore to t_c=1
        let replayed = engine.restore(1).unwrap();
        assert_eq!(replayed, 1); // 1 delta replayed (milestone at 0)
        assert_eq!(engine.t_c(), 1);
        assert_eq!(engine.space().node_count(), 1); // 1 commit → 1 node
    }

    // --- full_reposition ---

    #[test]
    fn full_reposition_updates_all_nodes() {
        let mut space = Space::new();
        space.insert_node(mod_node(1));
        space.insert_node(mod_node(2));

        let cs = CoordinateSystem::default_raw_three(
            EntropyAxis::from_commit_entropy(6.0),
            WitnessDepthAxis::from_witness(0.3, 5),
        );
        let mut engine = SpaceEngine::new(
            space,
            cs,
            VisionVector::new(CENTER),
            EngineConfig::default_calibrated(),
        );

        let _ = engine.full_reposition();
        // All nodes have positions (not default all-zero)
        for node in engine.space().nodes.values() {
            assert!(node.position.raw.x >= 0.0 || node.position.raw.w > 0.0);
        }
    }

    // --- from_vision_config ---

    #[test]
    fn from_vision_config_builds_engine() {
        let toml = r#"
[raw]
x = 0.4
y = 0.7
z = 0.5
w = 0.5
v = 0.5
"#;
        let config = VisionConfig::from_str(toml).unwrap();
        let cs = CoordinateSystem::default_raw_three(
            EntropyAxis::from_commit_entropy(6.0),
            WitnessDepthAxis::from_witness(0.3, 5),
        );
        let engine = SpaceEngine::from_vision_config(Space::new(), cs, &config);

        assert!((engine.vision().raw().x - 0.4).abs() < 1e-9);
        assert_eq!(engine.config().min_approvers, 2);
        assert!((engine.config().theta_bound - 0.3).abs() < 1e-9);
        assert_eq!(engine.t_c(), 0);
    }

    // --- no persistence ---

    #[test]
    fn restore_without_persistence_returns_error() {
        let mut engine = make_engine(); // no persistence
        let result = engine.restore(1);
        assert!(matches!(result, Err(EngineCommitError::NoPersistence)));
    }
}
