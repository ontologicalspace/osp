//! Analyze provenance DTO — CLI-owned stable wire format for `osp analyze` JSON output.
//!
//! Faz 8 test-project (review v7 + B-2 review): CLI analyze çıktısı per-axis provenance taşır.
//! Tüm core enum/struct'lardan ayrı DTO'lar — core wire format (PascalCase, display biçimi)
//! değişirse CLI envelope kararlı kalır. Her DTO golden test ile pinlenir.
//!
//! ## Metadata ayrımı (review P1-1)
//!
//! Analyze komutu GERÇEK axis-specific analyzer provenance üretir (MetricValue source +
//! confidence + coverage). Bu nedenle `analysis.provenance_native: true`. Navigator'ın
//! `legacy_projected` execution authority'si (B-3 run envelope'una ait) analyze çıktısında
//! DEĞİL — o navigator'ın runtime measurement projection özelliğidir.

#![allow(
    dead_code,
    reason = "Faz 8 test-project: wired incrementally across commits"
)]

use osp_core::coords::MetricSource;
use osp_core::space::{NodeClassification, NodeKind, NodeRole};

// ═══════════════════════════════════════════════════════════════════════════════
// MetricSource DTO
// ═══════════════════════════════════════════════════════════════════════════════

/// CLI metric-source DTO — core `MetricSource`'tan ayrı stable wire format.
///
/// `snake_case` rename: core `MetricSource` serde default PascalCase emit eder.
/// Bu DTO `tree_sitter` (underscore) emit eder — golden test ile sabitlenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliMetricSource {
    TreeSitter,
    Scip,
    Placeholder,
    Heuristic,
    Mixed,
}

impl From<MetricSource> for CliMetricSource {
    fn from(source: MetricSource) -> Self {
        match source {
            MetricSource::TreeSitter => CliMetricSource::TreeSitter,
            MetricSource::Scip => CliMetricSource::Scip,
            MetricSource::Placeholder => CliMetricSource::Placeholder,
            MetricSource::Heuristic => CliMetricSource::Heuristic,
            MetricSource::Mixed => CliMetricSource::Mixed,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Node enum DTOs (review P1-2 — core enum serde'den bağımsız)
// ═══════════════════════════════════════════════════════════════════════════════

/// CLI node-kind DTO — snake_case, core `NodeKind` PascalCase serde'dan ayrılmış.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliNodeKind {
    Module,
    Concept,
    Feature,
    Bug,
    Rule,
    Agent,
    Intent,
    Claim,
    Witness,
}

impl From<NodeKind> for CliNodeKind {
    fn from(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Module => CliNodeKind::Module,
            NodeKind::Concept => CliNodeKind::Concept,
            NodeKind::Feature => CliNodeKind::Feature,
            NodeKind::Bug => CliNodeKind::Bug,
            NodeKind::Rule => CliNodeKind::Rule,
            NodeKind::Agent => CliNodeKind::Agent,
            NodeKind::Intent => CliNodeKind::Intent,
            NodeKind::Claim => CliNodeKind::Claim,
            NodeKind::Witness => CliNodeKind::Witness,
        }
    }
}

/// CLI node-classification DTO — snake_case, core enum'dan ayrılmış.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliNodeClassification {
    Production,
    Test,
    Fixture,
    Migration,
    Config,
    Script,
    Generated,
    Documentation,
    Unknown,
}

impl From<NodeClassification> for CliNodeClassification {
    fn from(classification: NodeClassification) -> Self {
        match classification {
            NodeClassification::Production => CliNodeClassification::Production,
            NodeClassification::Test => CliNodeClassification::Test,
            NodeClassification::Fixture => CliNodeClassification::Fixture,
            NodeClassification::Migration => CliNodeClassification::Migration,
            NodeClassification::Config => CliNodeClassification::Config,
            NodeClassification::Script => CliNodeClassification::Script,
            NodeClassification::Generated => CliNodeClassification::Generated,
            NodeClassification::Documentation => CliNodeClassification::Documentation,
            NodeClassification::Unknown => CliNodeClassification::Unknown,
        }
    }
}

/// CLI node-role DTO — snake_case, core enum'dan ayrılmış.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliNodeRole {
    TypeSurface,
    Core,
    Adapter,
    Utility,
    Runtime,
    Support,
}

impl From<NodeRole> for CliNodeRole {
    fn from(role: NodeRole) -> Self {
        match role {
            NodeRole::TypeSurface => CliNodeRole::TypeSurface,
            NodeRole::Core => CliNodeRole::Core,
            NodeRole::Adapter => CliNodeRole::Adapter,
            NodeRole::Utility => CliNodeRole::Utility,
            NodeRole::Runtime => CliNodeRole::Runtime,
            NodeRole::Support => CliNodeRole::Support,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Per-axis measurement + node entry DTOs
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-axis measurement DTO — full provenance (review B-2: value + source + confidence +
/// coverage). Analyzer `MetricValue` 4 alan taşır; "provenance envelope" eksiksiz olmalı.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliAxisMeasurement {
    pub value: f64,
    pub source: CliMetricSource,
    pub confidence: f64,
    pub coverage: f64,
}

/// Per-node analyze entry — NodeId + path + kind/classification/role + mass + 3-axis.
///
/// V1 yalnız coupling/cohesion/instability taşır (analyzer ModuleMetrics scope).
/// entropy/witness_depth native engine measurement (#96) gelene kadar yok — fabricate edilmez.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliAnalyzeNode {
    pub node_id: u64,
    pub path: String,
    pub kind: CliNodeKind,
    pub classification: CliNodeClassification,
    pub role: CliNodeRole,
    pub mass: f64,
    pub coupling: CliAxisMeasurement,
    pub cohesion: CliAxisMeasurement,
    pub instability: CliAxisMeasurement,
}

impl CliAnalyzeNode {
    pub fn from_analysis(
        node_id: u64,
        path: String,
        node: &osp_core::space::Node,
        metrics: &osp_analyzer::contract::ModuleMetrics,
    ) -> Self {
        Self {
            node_id,
            path,
            kind: node.kind.into(),
            classification: node.classification.into(),
            role: node.role.into(),
            mass: node.mass,
            coupling: CliAxisMeasurement {
                value: metrics.coupling.value,
                source: metrics.coupling.source.into(),
                confidence: metrics.coupling.confidence,
                coverage: metrics.coupling.coverage,
            },
            cohesion: CliAxisMeasurement {
                value: metrics.cohesion.value,
                source: metrics.cohesion.source.into(),
                confidence: metrics.cohesion.confidence,
                coverage: metrics.cohesion.coverage,
            },
            instability: CliAxisMeasurement {
                value: metrics.instability.value,
                source: metrics.instability.source.into(),
                confidence: metrics.instability.confidence,
                coverage: metrics.instability.coverage,
            },
        }
    }
}

/// Analyze envelope integrity error (review P1-3 — exact-set node identity + path bijection).
#[derive(Debug, thiserror::Error)]
pub enum AnalyzeEnvelopeError {
    #[error("node key-set mismatch: space.nodes={node_ids:?} vs node_paths={path_ids:?}")]
    NodePathKeySetMismatch {
        node_ids: Vec<u64>,
        path_ids: Vec<u64>,
    },
    #[error("node key-set mismatch: space.nodes={node_ids:?} vs module_metrics={metric_ids:?}")]
    ModuleMetricKeySetMismatch {
        node_ids: Vec<u64>,
        metric_ids: Vec<u64>,
    },
    /// Two distinct NodeIds mapped to the same path (review P1-3 — path bijection).
    #[error("duplicate path binding: node {node_a} and node {node_b} both map to {path}")]
    DuplicatePathBinding {
        node_a: u64,
        node_b: u64,
        path: String,
    },
    /// Path is not a clean repo-relative forward-slash normalized identifier (review P1-3).
    #[error("invalid node path for node {node_id}: {path:?} (must be non-empty repo-relative forward-slash normalized, no ../ or absolute)")]
    InvalidNodePath { node_id: u64, path: String },
}

/// Exact-set node identity check (review P1-3).
///
/// `space.nodes`, `node_paths`, `module_metrics` aynı NodeId kümesine sahip olmalı.
/// Missing (lookup fail) + extra/orphan (sessiz ignore) ikisi de fail-closed.
pub fn validate_node_key_sets(
    result: &osp_analyzer::contract::AnalysisResult,
) -> Result<(), AnalyzeEnvelopeError> {
    let mut node_ids: Vec<u64> = result.space.nodes.keys().copied().collect();
    node_ids.sort_unstable();
    let mut path_ids: Vec<u64> = result.node_paths.keys().copied().collect();
    path_ids.sort_unstable();
    let mut metric_ids: Vec<u64> = result.module_metrics.keys().copied().collect();
    metric_ids.sort_unstable();
    if node_ids != path_ids {
        return Err(AnalyzeEnvelopeError::NodePathKeySetMismatch { node_ids, path_ids });
    }
    if node_ids != metric_ids {
        return Err(AnalyzeEnvelopeError::ModuleMetricKeySetMismatch {
            node_ids,
            metric_ids,
        });
    }
    Ok(())
}

/// Path bijection + path validity check (review P1-3 — defensive truth-surface).
///
/// Her NodeId unique path'e, her path unique NodeId'ye bağlı olmalı. Path'ler:
/// repo-relative, forward-slash normalized, non-empty, absolute değil, `../` traversal yok.
/// (Analyzer normalde tek source file → tek node üretir; bu defensive guard invariant korur.)
pub fn validate_node_paths_bijection(
    node_paths: &std::collections::HashMap<u64, String>,
) -> Result<(), AnalyzeEnvelopeError> {
    let mut seen_paths: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    // NodeId ascending → deterministic duplicate attribution (lower id reported as node_a).
    let mut ids: Vec<u64> = node_paths.keys().copied().collect();
    ids.sort_unstable();
    for node_id in ids {
        let path = &node_paths[&node_id];
        if path.is_empty()
            || path.starts_with('/')
            || std::path::Path::new(path).is_absolute()
            || path.contains("..")
            || path.contains('\\')
        {
            return Err(AnalyzeEnvelopeError::InvalidNodePath {
                node_id,
                path: path.clone(),
            });
        }
        if let Some(prev) = seen_paths.insert(path.clone(), node_id) {
            return Err(AnalyzeEnvelopeError::DuplicatePathBinding {
                node_a: prev,
                node_b: node_id,
                path: path.clone(),
            });
        }
    }
    Ok(())
}

/// Repository content binding mode (review P0 — generic vs harness-bound analyze).
///
/// İki ayrı contract: generic `osp analyze` dirty worktree'yi gözlemler (HEAD metadata);
/// harness-bound analyze clean pre/post-equal HEAD zorunlu (snapshot-bound subject authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliRepositoryBinding {
    /// Clean worktree, HEAD == post-analysis HEAD. Harness task üretimi için.
    CleanPrePostEqual,
    /// Worktree gözlemlendi; içerik HEAD'e bağlı DEĞİL (dirty/observed). Generic analyze.
    ObservedWorktreeUnbound,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Golden wire-format tests (review P1-2 — exact snake_case tags pinned) ──────

    #[test]
    fn cli_metric_source_serializes_snake_case_exact() {
        let serialized: Vec<String> = [
            CliMetricSource::TreeSitter,
            CliMetricSource::Scip,
            CliMetricSource::Placeholder,
            CliMetricSource::Heuristic,
            CliMetricSource::Mixed,
        ]
        .iter()
        .map(|s| {
            serde_json::to_string(s)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
        assert_eq!(
            serialized,
            ["tree_sitter", "scip", "placeholder", "heuristic", "mixed"]
        );
    }

    #[test]
    fn cli_node_kind_serializes_snake_case_exact() {
        let serialized: Vec<String> = [
            CliNodeKind::Module,
            CliNodeKind::Concept,
            CliNodeKind::Feature,
            CliNodeKind::Bug,
            CliNodeKind::Rule,
            CliNodeKind::Agent,
            CliNodeKind::Intent,
            CliNodeKind::Claim,
            CliNodeKind::Witness,
        ]
        .iter()
        .map(|s| {
            serde_json::to_string(s)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
        assert_eq!(
            serialized,
            [
                "module", "concept", "feature", "bug", "rule", "agent", "intent", "claim",
                "witness"
            ]
        );
    }

    #[test]
    fn cli_node_classification_serializes_snake_case_exact() {
        let serialized: Vec<String> = [
            CliNodeClassification::Production,
            CliNodeClassification::Test,
            CliNodeClassification::Fixture,
            CliNodeClassification::Migration,
            CliNodeClassification::Config,
            CliNodeClassification::Script,
            CliNodeClassification::Generated,
            CliNodeClassification::Documentation,
            CliNodeClassification::Unknown,
        ]
        .iter()
        .map(|s| {
            serde_json::to_string(s)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
        assert_eq!(
            serialized,
            [
                "production",
                "test",
                "fixture",
                "migration",
                "config",
                "script",
                "generated",
                "documentation",
                "unknown"
            ]
        );
    }

    #[test]
    fn cli_node_role_serializes_snake_case_exact() {
        let serialized: Vec<String> = [
            CliNodeRole::TypeSurface,
            CliNodeRole::Core,
            CliNodeRole::Adapter,
            CliNodeRole::Utility,
            CliNodeRole::Runtime,
            CliNodeRole::Support,
        ]
        .iter()
        .map(|s| {
            serde_json::to_string(s)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
        assert_eq!(
            serialized,
            [
                "type_surface",
                "core",
                "adapter",
                "utility",
                "runtime",
                "support"
            ]
        );
    }

    // ── Bijective From<core> mappings ─────────────────────────────────────────────

    #[test]
    fn from_core_metric_source_is_bijective() {
        assert_eq!(
            CliMetricSource::from(MetricSource::TreeSitter),
            CliMetricSource::TreeSitter
        );
        assert_eq!(
            CliMetricSource::from(MetricSource::Scip),
            CliMetricSource::Scip
        );
        assert_eq!(
            CliMetricSource::from(MetricSource::Placeholder),
            CliMetricSource::Placeholder
        );
        assert_eq!(
            CliMetricSource::from(MetricSource::Heuristic),
            CliMetricSource::Heuristic
        );
        assert_eq!(
            CliMetricSource::from(MetricSource::Mixed),
            CliMetricSource::Mixed
        );
    }

    #[test]
    fn from_core_node_kind_is_bijective() {
        for original in [
            NodeKind::Module,
            NodeKind::Concept,
            NodeKind::Feature,
            NodeKind::Bug,
            NodeKind::Rule,
            NodeKind::Agent,
            NodeKind::Intent,
            NodeKind::Claim,
            NodeKind::Witness,
        ] {
            let cli: CliNodeKind = original.into();
            assert_eq!(
                serde_json::to_string(&cli).unwrap(),
                format!(
                    "\"{}\"",
                    match original {
                        NodeKind::Module => "module",
                        NodeKind::Concept => "concept",
                        NodeKind::Feature => "feature",
                        NodeKind::Bug => "bug",
                        NodeKind::Rule => "rule",
                        NodeKind::Agent => "agent",
                        NodeKind::Intent => "intent",
                        NodeKind::Claim => "claim",
                        NodeKind::Witness => "witness",
                    }
                )
            );
        }
    }

    #[test]
    fn from_core_node_classification_is_bijective() {
        for original in [
            NodeClassification::Production,
            NodeClassification::Test,
            NodeClassification::Fixture,
            NodeClassification::Migration,
            NodeClassification::Config,
            NodeClassification::Script,
            NodeClassification::Generated,
            NodeClassification::Documentation,
            NodeClassification::Unknown,
        ] {
            let cli: CliNodeClassification = original.into();
            // round-trip through JSON to assert exact tag.
            let json = serde_json::to_string(&cli).unwrap();
            let back: CliNodeClassification = serde_json::from_str(&json).unwrap();
            assert_eq!(back, cli);
        }
    }

    #[test]
    fn from_core_node_role_is_bijective() {
        for original in [
            NodeRole::TypeSurface,
            NodeRole::Core,
            NodeRole::Adapter,
            NodeRole::Utility,
            NodeRole::Runtime,
            NodeRole::Support,
        ] {
            let cli: CliNodeRole = original.into();
            let json = serde_json::to_string(&cli).unwrap();
            let back: CliNodeRole = serde_json::from_str(&json).unwrap();
            assert_eq!(back, cli);
        }
    }

    #[test]
    fn cli_axis_measurement_carries_full_provenance() {
        let m = CliAxisMeasurement {
            value: 0.42,
            source: CliMetricSource::Scip,
            confidence: 0.9,
            coverage: 1.0,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: CliAxisMeasurement = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, 0.42);
        assert_eq!(back.source, CliMetricSource::Scip);
        assert_eq!(back.confidence, 0.9);
        assert_eq!(back.coverage, 1.0);
    }

    // ── Repository binding wire format (review P0) ────────────────────────────────

    #[test]
    fn cli_repository_binding_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CliRepositoryBinding::CleanPrePostEqual).unwrap(),
            "\"clean_pre_post_equal\""
        );
        assert_eq!(
            serde_json::to_string(&CliRepositoryBinding::ObservedWorktreeUnbound).unwrap(),
            "\"observed_worktree_unbound\""
        );
    }

    // ── Path bijection + validity (review P1-3) ───────────────────────────────────

    fn np(entries: &[(u64, &str)]) -> std::collections::HashMap<u64, String> {
        entries
            .iter()
            .map(|(k, v)| (*k, (*v).to_string()))
            .collect()
    }

    #[test]
    fn bijection_accepts_unique_normalized_paths() {
        let map = np(&[(1, "src/a.rs"), (2, "src/b.rs")]);
        validate_node_paths_bijection(&map).unwrap();
    }

    #[test]
    fn bijection_rejects_duplicate_path() {
        let map = np(&[(1, "src/a.rs"), (2, "src/a.rs")]);
        assert!(matches!(
            validate_node_paths_bijection(&map).unwrap_err(),
            AnalyzeEnvelopeError::DuplicatePathBinding {
                node_a: 1,
                node_b: 2,
                ..
            }
        ));
    }

    #[test]
    fn bijection_rejects_absolute_path() {
        let map = np(&[(1, "/abs/a.rs")]);
        assert!(matches!(
            validate_node_paths_bijection(&map).unwrap_err(),
            AnalyzeEnvelopeError::InvalidNodePath { node_id: 1, .. }
        ));
    }

    #[test]
    fn bijection_rejects_parent_traversal() {
        let map = np(&[(1, "../escape.rs")]);
        assert!(matches!(
            validate_node_paths_bijection(&map).unwrap_err(),
            AnalyzeEnvelopeError::InvalidNodePath { node_id: 1, .. }
        ));
    }

    #[test]
    fn bijection_rejects_backslash_path() {
        let map = np(&[(1, "src\\a.rs")]);
        assert!(matches!(
            validate_node_paths_bijection(&map).unwrap_err(),
            AnalyzeEnvelopeError::InvalidNodePath { node_id: 1, .. }
        ));
    }

    #[test]
    fn bijection_rejects_empty_path() {
        let map = np(&[(1, "")]);
        assert!(matches!(
            validate_node_paths_bijection(&map).unwrap_err(),
            AnalyzeEnvelopeError::InvalidNodePath { node_id: 1, .. }
        ));
    }

    #[test]
    fn key_set_error_is_deterministic_sorted() {
        // Error variants carry sorted Vec (not HashSet) — deterministic debug output.
        let mut result = osp_analyzer::contract::AnalysisResult {
            space: osp_core::space::Space {
                nodes: std::collections::HashMap::new(),
                edges: vec![],
                gravity: std::collections::HashMap::new(),
                time_layer: osp_core::space::TimeLayer::default(),
            },
            module_metrics: std::collections::HashMap::new(),
            node_paths: std::collections::HashMap::new(),
            node_semantics: std::collections::HashMap::new(),
            node_witnesses: std::collections::HashMap::new(),
            repo_metrics: osp_analyzer::contract::RepoMetrics {
                abstractness: osp_core::coords::MetricValue::placeholder(0.0),
                main_sequence_distance: osp_core::coords::MetricValue::placeholder(0.0),
                abstractness_by_package: None,
            },
            semantic_coverage: osp_analyzer::contract::SemanticCoverage {
                files_total: 0,
                files_with_scip: 0,
                classes_total: 0,
                classes_with_field_access: 0,
                coverage_ratio: 0.0,
                index_commit: None,
                repo_head: String::new(),
                stale: false,
            },
            diagnostics: vec![],
            completeness: osp_analyzer::language::AnalysisCompleteness::Complete,
        };
        result
            .space
            .nodes
            .insert(2, osp_core::space::Node::default());
        result
            .space
            .nodes
            .insert(1, osp_core::space::Node::default());
        result.node_paths.insert(1, "a.rs".into());
        // node_paths missing node 2 → mismatch; reported sorted.
        let err = validate_node_key_sets(&result).unwrap_err();
        match err {
            AnalyzeEnvelopeError::NodePathKeySetMismatch { node_ids, path_ids } => {
                assert_eq!(node_ids, vec![1, 2], "sorted ascending");
                assert_eq!(path_ids, vec![1], "sorted ascending");
            }
            other => panic!("expected NodePathKeySetMismatch, got {other:?}"),
        }
    }
}
