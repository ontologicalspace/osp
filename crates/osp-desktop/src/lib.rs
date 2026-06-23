//! OSP Desktop — backend command handlers.
//!
//! Bu fonksiyonlar hem HTTP server (mevcut) hem de Tauri IPC (gelecek)
//! tarafından çağrılır. Logic ayrımı: bu modül sadece iş mantığı içerir,
//! sunum (HTTP/Tauri) `main.rs`'te.

use std::path::{Path, PathBuf};

use osp_analyzer::contract::AnalysisConfig;
use osp_analyzer::language::AdapterRegistry;
use osp_analyzer::pipeline::analyze_repo_with_config;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// JSON types (frontend ile paylaşılır)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceJson {
    pub nodes: Vec<NodeJson>,
    pub edges: Vec<EdgeJson>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeJson {
    pub id: u64,
    pub kind: String,
    pub mass: f64,
    pub coupling: Option<f64>,
    pub cohesion: Option<f64>,
    pub instability: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeJson {
    pub from: u64,
    pub to: u64,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResultJson {
    pub space: SpaceJson,
    pub repo_metrics: RepoMetricsJson,
    pub semantic_coverage: SemanticCoverageJson,
    pub module_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoMetricsJson {
    pub abstractness: f64,
    pub main_sequence_distance: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticCoverageJson {
    pub files_total: usize,
    pub files_with_scip: usize,
    pub coverage_ratio: f64,
    pub classes_total: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Command: analyze_repo
// ═══════════════════════════════════════════════════════════════════════════════

/// Bir reponun tam analizini yap → JSON (frontend için).
///
/// `scip_path` verilirse gerçek LCOM4 cohesion; yoksa placeholder.
pub fn cmd_analyze_repo(
    repo_path: &str,
    scip_path: Option<&str>,
) -> Result<AnalysisResultJson, String> {
    let config = AnalysisConfig {
        scip_index: scip_path.map(PathBuf::from),
        ..Default::default()
    };
    let registry = AdapterRegistry::default_all();
    let result = analyze_repo_with_config(Path::new(repo_path), &registry, &config)
        .map_err(|e| e.to_string())?;

    // Convert Space → JSON
    let nodes: Vec<NodeJson> = result
        .space
        .nodes
        .values()
        .map(|n| {
            let metrics = result.module_metrics.get(&n.id);
            NodeJson {
                id: n.id,
                kind: format!("{:?}", n.kind),
                mass: n.mass,
                coupling: metrics.map(|m| m.coupling.value),
                cohesion: n.cohesion.or_else(|| metrics.map(|m| m.cohesion.value)),
                instability: metrics.map(|m| m.instability.value),
            }
        })
        .collect();

    let edges: Vec<EdgeJson> = result
        .space
        .edges
        .iter()
        .map(|e| EdgeJson {
            from: e.from,
            to: e.to,
            kind: format!("{:?}", e.kind),
        })
        .collect();

    let node_count = nodes.len();

    Ok(AnalysisResultJson {
        space: SpaceJson { nodes, edges },
        repo_metrics: RepoMetricsJson {
            abstractness: result.repo_metrics.abstractness.value,
            main_sequence_distance: result.repo_metrics.main_sequence_distance.value,
        },
        semantic_coverage: SemanticCoverageJson {
            files_total: result.semantic_coverage.files_total,
            files_with_scip: result.semantic_coverage.files_with_scip,
            coverage_ratio: result.semantic_coverage.coverage_ratio,
            classes_total: result.semantic_coverage.classes_total,
        },
        module_count: node_count,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Command: health check
// ═══════════════════════════════════════════════════════════════════════════════

pub fn cmd_health() -> &'static str {
    "OSP Desktop v0.1 — ready"
}
