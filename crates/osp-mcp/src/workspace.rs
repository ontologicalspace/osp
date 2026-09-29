//! Startup workspace (`docs/mcp-design.md` §4 — ilk sürüm).
//!
//! AI agent raw `repo_path` ALAMAZ (path traversal riski). MCP server startup'ta
//! `--workspace <path>` alır, analyze eder, [`Workspace`] state olarak saklar.
//! Tüm tool'lar bu tek workspace üzerinde çalışır — agent path geçemez.
//!
//! **Workspace security:**
//! - Path canonicalize → resolve symlink/`..`.
//! - Mevcut directory exists kontrolü.
//! - `to_str()` ile güvenli display path.
//!
//! **Analyze-once:** Server startup'ta analyze edilir, SpaceEngine in-memory saklanır.
//! Tool'lar her çağrıda re-analyze ETMEZ — performans + determinizm için.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use osp_analyzer::contract::{AnalysisConfig, RepoMetrics, SemanticCoverage};
use osp_analyzer::language::AdapterRegistry;
use osp_analyzer::pipeline::analyze_repo_with_config;
use osp_core::axes::{CohesionAxis, EntropyAxis, WitnessDepthAxis};
use osp_core::coords::{CoordinateSystem, MetricSource};
use osp_core::engine::{EngineConfig, SpaceEngine};
use osp_core::vision::VisionVector;

/// Workspace hatası — startup veya re-analyze sırasında.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("workspace path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("analyze failed: {0}")]
    Analyze(String),
    #[error("workspace not analyzed yet — call analyze() first")]
    NotAnalyzed,
}

/// **#133 (review P1):** Claim id üretimi tükendi — sayaç `u64::MAX`'a ulaştı.
/// Wrap edip id'leri yeniden üretmek benzersizlik sözleşmesini sessizce bozardı
/// (fail-open); allocation fail-closed durur (yenisi güvenle üretilemiyorsa
/// claim ÜRETİLMEZ).
#[derive(Debug, thiserror::Error)]
pub enum ClaimIdAllocationError {
    #[error("claim id space exhausted (u64::MAX reached — sentinel, never allocated) — refusing to wrap and reuse ids")]
    Exhausted,
}

/// Startup workspace — analyze edilmiş SpaceEngine + analyzer result.
///
/// **Concurrency:** MCP server tek bir `Arc<Mutex<Workspace>>` paylaşır. rmcp handler'lar
/// async (tokio), ama osp-core sync — `std::sync::Mutex` yeterli (konsol kapanmaz).
pub struct Workspace {
    /// Canonical path (symlink/`..` resolve edilmiş).
    pub path: PathBuf,
    /// Analyze edilmiş SpaceEngine (Q4-Q6 commit pipeline hazır).
    engine: SpaceEngine,
    /// Repo-level metrikler (abstractness, main_sequence_distance).
    pub repo_metrics: RepoMetrics,
    /// SCIP coverage (kalite — placeholder metric uyarısı için).
    pub semantic_coverage: SemanticCoverage,
    /// Node sayısı (space snapshot — agent'a "kaç node var" verir).
    pub node_count: usize,
    /// Edge sayısı.
    pub edge_count: usize,
    /// **#133:** Server-ömürlü monotonic claim id kaynağı (atanan id =
    /// depolanan değer; depolama `checked_add` ile bir sonrakine ilerler).
    /// Motorun in-flight `suspended_cold_starts` map'i claim_id ile
    /// anahtarlanır — benzersizlik yoksa eşzamanlı askılar birbirini ezer
    /// (Run D F1). Atomic: mutex bağımsız benzersizlik; Relaxed yeterli
    /// (yalnız tekillilik sözleşmesi). **Review P1:** tükenmede wrap YOK —
    /// `next_claim_id` Err döner (benzersizlik fail-closed).
    next_claim_id: AtomicU64,
}

impl Workspace {
    /// Yeni workspace kur + analyze et (startup'ta çağrılır).
    ///
    /// **Akış:**
    /// 1. Path canonicalize + exists kontrolü
    /// 2. AdapterRegistry::default_all() — 5 dil
    /// 3. analyze_repo_with_config → space + metrics + coverage
    /// 4. CoordinateSystem + Vision + EngineConfig → SpaceEngine::with_default_rules
    pub fn analyze(path: &Path, scip_index: Option<&Path>) -> Result<Self, WorkspaceError> {
        // 1. Path validation (security).
        let canonical = path
            .canonicalize()
            .map_err(|_| WorkspaceError::PathNotFound(path.to_path_buf()))?;
        if !canonical.is_dir() {
            return Err(WorkspaceError::NotADirectory(canonical));
        }

        // 2. Analyze.
        let registry = AdapterRegistry::default_all();
        let config = AnalysisConfig {
            scip_index: scip_index.map(|p| p.to_path_buf()),
            ..Default::default()
        };
        let result = analyze_repo_with_config(&canonical, &registry, &config)
            .map_err(|e| WorkspaceError::Analyze(e.to_string()))?;
        let node_count = result.space.nodes.len();
        let edge_count = result.space.edges.len();
        let repo_metrics = result.repo_metrics.clone();
        let semantic_coverage = result.semantic_coverage.clone();

        // 3. Engine kur (D2 calibrated — osp-cli ve osp-desktop ile aynı).
        // **INV-T9 Adım 3:** default_raw_five artık validated Result döner.
        // **INV-T9 #70:** production topology_source = TreeSitter, observed cohesion = Scip.
        let cohesion = CohesionAxis::try_with_observed_source(MetricSource::Scip)
            .map_err(|e| WorkspaceError::Analyze(format!("cohesion axis source: {e}")))?;
        let cs = CoordinateSystem::default_raw_five(
            MetricSource::TreeSitter,
            cohesion,
            EntropyAxis::from_commit_entropy(6.0),
            WitnessDepthAxis::from_witness(0.3, 5),
        )
        .map_err(|e| WorkspaceError::Analyze(format!("axis registration failed: {e}")))?;
        // Default vision — Aşama C'de operator override edebilir.
        let vision = VisionVector::new(osp_core::coords::RawPosition {
            x: 0.4,
            y: 0.6,
            z: 0.5,
            w: 0.5,
            v: 0.5,
        });
        let engine = SpaceEngine::with_default_rules(
            result.space,
            cs,
            vision,
            EngineConfig::default_calibrated(),
        )
        .map_err(|e| WorkspaceError::Analyze(format!("engine rule registration failed: {e}")))?;

        Ok(Self {
            path: canonical,
            engine,
            repo_metrics,
            semantic_coverage,
            node_count,
            edge_count,
            // Sayaç 1'den başlar; üretilen id = sayaçtaki mevcut değer,
            // depolama `checked_add` ile bir sonrakine ilerler → ilk submit
            // claim_id 1 alır (#133 öncesi tek-submit kanıtlarıyla wire-uyumlu).
            // u64::MAX asla id olarak VERİLMEZ (tükenme sentinel'i).
            next_claim_id: AtomicU64::new(1),
        })
    }

    /// **#133:** Benzersiz claim id üret — döndürülen id, bu workspace'ta daha
    /// önce döndürülen her id'den kesinlikle büyüktür (server ömrü boyunca
    /// tekillik). Sayaç `u64::MAX`'a ulaştığında closure `None` döner ve
    /// üretim **fail-closed** durur (review P1): wrap'li `fetch_add` bilinçli
    /// reddedildi — `u64::MAX` sentinel'dir, son atanabilir id MAX−1.
    pub fn next_claim_id(&self) -> Result<u64, ClaimIdAllocationError> {
        self.next_claim_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| ClaimIdAllocationError::Exhausted)
    }

    /// Engine'e mutable reference al (commit_task_claim için — osp-core sync).
    pub fn engine_mut(&mut self) -> &mut SpaceEngine {
        &mut self.engine
    }

    /// Workspace snapshot — agent'a "ne var" özeti (node/edge count + coverage).
    pub fn snapshot_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "workspace_path": self.path.to_string_lossy(),
            "node_count": self.node_count,
            "edge_count": self.edge_count,
            "repo_metrics": {
                "abstractness": self.repo_metrics.abstractness.value,
                "main_sequence_distance": self.repo_metrics.main_sequence_distance.value,
            },
            "semantic_coverage": {
                "files_total": self.semantic_coverage.files_total,
                "files_with_scip": self.semantic_coverage.files_with_scip,
                "coverage_ratio": self.semantic_coverage.coverage_ratio,
            },
        })
    }
}

/// Shared workspace handle — `Arc<Mutex<Workspace>>`. MCP server handler bunu tutar,
/// her tool call'da lock'lar. rmcp async, osp-core sync → std::sync::Mutex yeterli.
pub type SharedWorkspace = Arc<Mutex<Workspace>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_fixture() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("main.py"),
            "from utils import helper\n\nclass App:\n    pass\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("utils.py"), "class Helper:\n    pass\n").unwrap();
        dir
    }

    /// **#133 review P1:** sayaç `u64::MAX`'ta wrap ETMEZ — `u64::MAX`
    /// sentinel'dir, asla id olarak ATANMAZ; son atanabilir id MAX−1,
    /// sonrası exhaustion `Err` (benzersizlik sözleşmesi fail-closed;
    /// wrap'li `fetch_add` bilinçli reddedildi).
    #[test]
    fn next_claim_id_exhaustion_fails_closed_no_wrap() {
        let dir = tiny_fixture();
        let ws = Workspace::analyze(dir.path(), None).expect("workspace analyze");
        assert_eq!(ws.next_claim_id().unwrap(), 1, "ilk id 1 — wire-uyumlu");
        assert_eq!(ws.next_claim_id().unwrap(), 2, "monotonic");

        ws.next_claim_id.store(u64::MAX - 1, Ordering::Relaxed);
        assert_eq!(
            ws.next_claim_id().unwrap(),
            u64::MAX - 1,
            "MAX−1 son atanabilir id"
        );
        let err = ws
            .next_claim_id()
            .expect_err("wrap YOK — exhaustion fail-closed");
        assert!(matches!(err, ClaimIdAllocationError::Exhausted), "{err:?}");
        // Kalıcı: tükenme sonrası her istek aynı şekilde reddedilir.
        assert!(ws.next_claim_id().is_err());
    }

    /// **#133 review P1 (yüzey pin'i):** id uzayı tükenince submit ölçüme/
    /// commit'e ULAŞMAZ — terminal `system_failure` envelope (mutasyon
    /// imkânsız; allocation draft `try_new`'den ÖNCE reddedilir).
    #[test]
    fn submit_delta_attempt_claim_id_exhaustion_returns_system_failure() {
        let dir = tiny_fixture();
        let mut ws = Workspace::analyze(dir.path(), None).expect("workspace analyze");
        // Allocation boş-proposal kontrolünden SONRA, draft/ölçümden ÖNCE —
        // task içeriği bu dalda okunmaz; yalnızca tip uyumu için minimal kurulum.
        let task = osp_core::trajectory::Task {
            id: 1,
            milestone_id: 1,
            label: "claim-id exhaustion surface pin".into(),
            target_predicate_set: osp_core::trajectory::PredicateSet {
                mode: osp_core::trajectory::PredicateMode::All,
                predicates: vec![],
                preferred_vector: None,
            },
            policy: osp_core::trajectory::TaskPolicy::default(),
            allowed_operations: vec![],
            constraints: vec![],
            status: osp_core::trajectory::TaskStatus::Pending,
        };
        let proposal = osp_core::agent::DeltaProposal {
            new_nodes: vec![osp_core::agent::NewNodeSpec {
                kind: osp_core::space::NodeKind::Module,
                initial_mass: 1.0,
                connected_to: vec![],
            }],
            ..Default::default()
        };
        ws.next_claim_id.store(u64::MAX, Ordering::Relaxed);

        let outcome = ws
            .submit_delta_attempt(&proposal, &task)
            .expect("attempt json kanalı");
        assert_eq!(
            outcome["system_failure"]["class"], "ClaimIdExhausted",
            "{outcome}"
        );
        assert_eq!(outcome["system_failure"]["retryable"], false);
        assert_eq!(outcome["apply_target"], "NotApplied");
        assert!(outcome["loss_after"].is_null());
    }
}
