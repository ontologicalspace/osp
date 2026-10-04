//! #172 (K1): defter komutlarının ortak ölçüm kaynağı — baseline görünümü.
//!
//! `draft-task` / `suggest-targets` temsilci ve aday doğrulamasını YALNIZ ölçümden
//! yapar (#173 ilkesi: *ölçümden gelmeli, el beyanından değil*). İki giriş:
//!
//! - **`--baseline <file>`** — `osp analyze --out` artifact'ı (ölçüm kaydı).
//!   `repository.head` canlı `git rev-parse HEAD` ile exact-match fence'e girer:
//!   artifact ile task'ın bağlanacağı state aynı olmalıdır (drift → fail-closed).
//!   SHA hiç elle yazılmadığı için transcription hatası sınıfı ölür.
//! - **verilmezse** — canlı `analyze` koşar; ölçüm o an üretilir, HEAD snapshot'tan
//!   alınır ve analiz penceresinde HEAD'in hareket etmediği fence'lenir.
//!
//! Her iki yol da aynı `BaselineView`'ı üretir: path-keyed kenar kümesi + düğüm
//! listesi + coupling/import-out türevleri. Görünüm YORUM içermez — ölçülmüş
//! fact'ler ve onların tür dönüşümleri dışında hiçbir türetme yapılmaz.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use osp_analyzer::contract::AnalysisConfig;
use osp_analyzer::language::AdapterRegistry;
use osp_analyzer::pipeline::analyze_repo_with_config;
use osp_core::space::EdgeKind;

use crate::commands::analyze_provenance::{CliAnalyzeNode, CliEdge, CliEdgeKind};
use crate::commands::repo_snapshot::{GitCommitId, RepositorySnapshot};

/// Ölçülmüş baseline kenarı — path-keyed (artifact kenarları id-keyed gelir;
/// düğüm haritasıyla path'e çevrilir, buradan sonrası path dünyasında kalır).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BaselineEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// Ölçülmüş baseline görünümü — defter komutlarının tek ölçüm kaynağı.
#[derive(Debug)]
pub(crate) struct BaselineView {
    /// Full 40-hex SHA (artifact `repository.head` veya canlı snapshot).
    pub head: String,
    /// Ölçülen düğümler (artifact sırası: node_id ascending).
    pub nodes: Vec<CliAnalyzeNode>,
    /// Ölçülen kenar kümesi (path-keyed).
    pub edges: HashSet<BaselineEdge>,
    /// path → ölçülen Imports çıkış-derecesi (c = out/(1+out) formülünün payı).
    pub imports_out_by_path: BTreeMap<String, usize>,
}

impl BaselineView {
    /// Ölçülen düğüm path kümesi (temsilci doğrulamasının karşılaştırma kümesi).
    pub fn node_path_set(&self) -> BTreeSet<&str> {
        self.nodes.iter().map(|n| n.path.as_str()).collect()
    }

    /// Bir düğümün ölçülmüş çıkış-kenarlarını sıralı listeler (fail-closed hata
    /// mesajlarının debugging yardımı — elle py ayıklamanın yerine geçer).
    pub fn measured_out_edges(&self, from: &str) -> Vec<String> {
        let mut listed: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.from == from)
            .map(|e| format!("-> {} ({})", e.to, kind_name(e.kind)))
            .collect();
        listed.sort();
        listed
    }
}

/// Core `EdgeKind` → insan-okur adı (mesajlarda; wire serialization'a bağlı değil).
fn kind_name(kind: EdgeKind) -> &'static str {
    CliEdgeKind::from(kind).wire_name()
}

/// Baseline'ı artifact'tan yükle + canlı HEAD fence'i (K1).
///
/// Artifact `osp analyze --out` zarfı olmak zorundadır (`schema_version: 2`).
/// `repository.head != live_head` → fail-closed: temsilciler eski bir ölçüme
/// karşı doğrulanamaz, task ise başka bir state'e bağlanamaz.
pub(crate) fn load_baseline_artifact(path: &Path, live_head: &str) -> anyhow::Result<BaselineView> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read baseline artifact {}: {e}", path.display()))?;
    let envelope: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        anyhow::anyhow!("failed to parse baseline artifact {}: {e}", path.display())
    })?;
    let found = envelope
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    anyhow::ensure!(
        found == 2,
        "baseline artifact requires schema_version 2 (found {found}) — \
         regenerate with `osp analyze --out`"
    );
    let artifact_head = envelope["repository"]["head"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("baseline artifact is missing repository.head"))?;
    GitCommitId::try_from(artifact_head.to_string())
        .map_err(|e| anyhow::anyhow!("baseline artifact repository.head invalid: {e}"))?;
    anyhow::ensure!(
        artifact_head == live_head,
        "baseline artifact HEAD drift: artifact {artifact_head}, repo {live_head} — \
         the baseline was measured on a different state; re-analyze before drafting \
         (delegates must be validated against the state the task binds to)"
    );

    let nodes: Vec<CliAnalyzeNode> = serde_json::from_value(
        envelope
            .get("nodes")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("baseline artifact is missing nodes"))?,
    )
    .map_err(|e| anyhow::anyhow!("baseline artifact nodes do not parse: {e}"))?;
    let edges: Vec<CliEdge> = serde_json::from_value(
        envelope
            .get("edges")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("baseline artifact is missing edges"))?,
    )
    .map_err(|e| anyhow::anyhow!("baseline artifact edges do not parse: {e}"))?;
    build_view(nodes, edges, artifact_head.to_string())
}

/// Baseline'ı canlı analyze ile üret (K1, `--baseline` verilmediğinde).
///
/// HEAD authority: analiz SONRASI snapshot (`run_analyze` zarfıyla aynı tek kaynak).
/// Analiz penceresinde HEAD hareket ettiyse fail-closed — hareketli bir state'e
/// task bağlanamaz.
pub(crate) fn analyze_live(repo: &Path) -> anyhow::Result<BaselineView> {
    let snapshot_before = RepositorySnapshot::capture(repo).map_err(|e| anyhow::anyhow!(e))?;
    let registry = AdapterRegistry::default_all();
    let result = analyze_repo_with_config(repo, &registry, &AnalysisConfig::default())?;
    let snapshot_after = RepositorySnapshot::capture(repo).map_err(|e| anyhow::anyhow!(e))?;
    anyhow::ensure!(
        snapshot_before.head == snapshot_after.head,
        "repository HEAD moved during analysis ({} -> {}) — a task cannot bind \
         to a moving state; retry",
        snapshot_before.head,
        snapshot_after.head
    );

    // run_analyze ile aynı DTO üretimi (node_id ascending deterministik sıra).
    let mut nodes: Vec<CliAnalyzeNode> = Vec::with_capacity(result.space.nodes.len());
    let mut node_ids: Vec<u64> = result.space.nodes.keys().copied().collect();
    node_ids.sort_unstable();
    for node_id in &node_ids {
        let node = &result.space.nodes[node_id];
        let path = result
            .node_paths
            .get(node_id)
            .ok_or_else(|| anyhow::anyhow!("analysis produced a node without a path: {node_id}"))?;
        let metrics = result.module_metrics.get(node_id).ok_or_else(|| {
            anyhow::anyhow!("analysis produced a node without metrics: {node_id}")
        })?;
        nodes.push(CliAnalyzeNode::from_analysis(
            *node_id,
            path.clone(),
            node,
            metrics,
        ));
    }
    let edges: Vec<CliEdge> = result.space.edges.iter().map(CliEdge::from_edge).collect();
    build_view(nodes, edges, snapshot_after.head.as_str().to_string())
}

/// Düğüm/kenar listelerinden `BaselineView` türet — id→path dönüşümü + haritalar.
///
/// Kenar uçlarından biri düğüm kümesinde yoksa artifact bozuktur (fail-closed):
/// path dünyasında çözümlenemeyen kenar, temsilci doğrulamasında sessiz delik açar.
fn build_view(
    nodes: Vec<CliAnalyzeNode>,
    edges: Vec<CliEdge>,
    head: String,
) -> anyhow::Result<BaselineView> {
    let mut id_to_path: BTreeMap<u64, &str> = BTreeMap::new();
    for node in &nodes {
        anyhow::ensure!(
            id_to_path.insert(node.node_id, &node.path).is_none(),
            "baseline contains duplicate node_id {} — corrupt artifact",
            node.node_id
        );
    }
    let path_of = |id: u64| -> anyhow::Result<&str> {
        id_to_path.get(&id).copied().ok_or_else(|| {
            anyhow::anyhow!("baseline edge endpoint {id} is not in the node set — corrupt artifact")
        })
    };

    let mut edge_set: HashSet<BaselineEdge> = HashSet::new();
    let mut imports_out: BTreeMap<String, usize> = BTreeMap::new();
    for edge in &edges {
        let (from, to) = (path_of(edge.from)?, path_of(edge.to)?);
        let kind: EdgeKind = edge.kind.into();
        if kind == EdgeKind::Imports {
            *imports_out.entry(from.to_string()).or_insert(0) += 1;
        }
        edge_set.insert(BaselineEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        });
    }

    Ok(BaselineView {
        head,
        nodes,
        edges: edge_set,
        imports_out_by_path: imports_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// K1 fence sözleşmesi: artifact head ≠ canlı head → fail-closed (exact mesaj
    /// kırıntılarıyla — operatör yönlendirmesi ölçümün yeniden üretilmesine).
    #[test]
    fn artifact_head_drift_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("baseline.json");
        std::fs::write(
            &artifact,
            serde_json::json!({
                "schema_version": 2,
                "repository": {"head": "b".repeat(40)},
                "nodes": [], "edges": [], "semantic_coverage": {"files_with_scip": 0}
            })
            .to_string(),
        )
        .unwrap();
        let err = load_baseline_artifact(&artifact, &"a".repeat(40)).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("HEAD drift"), "{msg}");
        assert!(msg.contains(&"b".repeat(40)), "{msg}");
    }

    /// Yanlış şema sürümü → net red (eski v1 zarfı sessizce yorumlanamaz).
    #[test]
    fn artifact_wrong_schema_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("baseline.json");
        std::fs::write(
            &artifact,
            serde_json::json!({
                "schema_version": 1,
                "repository": {"head": "a".repeat(40)},
                "nodes": [], "edges": []
            })
            .to_string(),
        )
        .unwrap();
        let err = load_baseline_artifact(&artifact, &"a".repeat(40)).unwrap_err();
        assert!(format!("{err}").contains("schema_version 2"), "{err}");
    }

    /// Kenar ucu düğüm kümesi dışında → corrupt artifact red'i (path dünyasında
    /// çözümlenemeyen kenar temsilci doğrulamasında delik açar).
    #[test]
    fn edge_endpoint_outside_node_set_rejected() {
        let err = build_view(
            vec![],
            vec![CliEdge {
                from: 7,
                to: 8,
                kind: CliEdgeKind::Imports,
                is_type_only: false,
                type_ref: None,
            }],
            "a".repeat(40),
        )
        .unwrap_err();
        assert!(format!("{err}").contains("not in the node set"), "{err}");
    }
}
