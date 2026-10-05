//! #172 (K1, tur-1 P0-1 ile sıkılaştırıldı): defter komutlarının ortak ölçüm
//! kaynağı — **revizyona bağlı** baseline görünümü.
//!
//! `draft-task` / `suggest-targets` temsilci ve aday doğrulamasını YALNIZ ölçümden
//! yapar (#173 ilkesi: *ölçümden gelmeli, el beyanından değil*). İki giriş:
//!
//! - **`--baseline <file>`** — `osp analyze --require-clean-snapshot --out`
//!   artifact'ı. Üç katmanlı fence (tur-1 P0-1):
//!   1. `repository.binding == clean_pre_post_equal` — generic analyze bilinçli
//!      olarak `observed_worktree_unbound` üretir ve içeriği HEAD'e BAĞLAMAYABİLİR
//!      (dirty analyzed-path ölçümü HEAD yerine worktree içeriğinden gelir);
//!      revizyona bağlı artifact yalnız clean-bound sözleşmeden çıkar.
//!   2. `repository.head ==` canlı `git rev-parse HEAD` (drift → fail-closed).
//!   3. Ölçülen düğüm path'leri ŞİMDİ HEAD-tracked + clean (#155 analyzed-scope
//!      fence'leri — artifact üretiminden bu yana ölçülen dosyalar değişmedi).
//! - **verilmezse** — canlı `analyze` koşar; aynı fence ailesi uygulanır:
//!   analiz penceresinde HEAD **ve tracked-set** değişmedi, ölçülen path'ler
//!   HEAD-tracked ve clean.
//!
//! Böylece `MeasuredState == HEADState == Task'ın bağlandığı state` olması motorda
//! ispatlanabilir kalır (#155/#160 state-authority çizgisi). SHA hiç elle
//! yazılmadığı için transcription hatası sınıfı da ölür.
//!
//! Her iki yol da aynı `BaselineView`'ı üretir: path-keyed kenar kümesi + düğüm
//! listesi + coupling/import-out türevleri. Görünüm YORUM içermez — ölçülmüş
//! fact'ler ve onların tür dönüşümleri dışında hiçbir türetme yapılmaz.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use osp_analyzer::contract::AnalysisConfig;
use osp_analyzer::language::AdapterRegistry;
use osp_analyzer::pipeline::analyze_repo_with_config;
use osp_core::space::EdgeKind;

use crate::commands::analyze_provenance::{CliAnalyzeNode, CliEdge, CliEdgeKind};
use crate::commands::repo_snapshot::{
    validate_analyzed_paths_clean, validate_analyzed_paths_tracked, GitCommitId, RepositorySnapshot,
};

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

/// Baseline'ı artifact'tan yükle — revizyona bağlılık fence'leriyle (K1, tur-1 P0-1).
///
/// Artifact `osp analyze --require-clean-snapshot --out` zarfı olmak zorundadır
/// (`schema_version: 2` + `binding: clean_pre_post_equal`): generic analyze'ın
/// `observed_worktree_unbound` çıktısı ölçülen içeriğin HEAD içeriği OLDUĞUNU
/// iddia edemez. Ardından head-drift fence'i ve #155 analyzed-scope fence'leri
/// (ölçülen path'ler bugün HEAD-tracked + clean) uygulanır.
pub(crate) fn load_baseline_artifact(
    path: &Path,
    snapshot: &RepositorySnapshot,
) -> anyhow::Result<BaselineView> {
    let live_head = snapshot.head.as_str();
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
    // P0-1 katman 1: yalnız clean-bound artifact revizyona bağlıdır.
    let binding = envelope["repository"]["binding"].as_str().unwrap_or("");
    anyhow::ensure!(
        binding == "clean_pre_post_equal",
        "baseline artifact is not revision-bound: repository.binding = {binding:?} — \
         only `clean_pre_post_equal` (osp analyze --require-clean-snapshot) guarantees \
         the measured content IS the HEAD content; an observed_worktree_unbound \
         baseline may have measured dirty/uncommitted files under this HEAD"
    );
    let clean = envelope["repository"]["clean"].as_bool().unwrap_or(false);
    anyhow::ensure!(
        clean,
        "baseline artifact reports a dirty repository (repository.clean = false) — \
         regenerate with `osp analyze --require-clean-snapshot`"
    );
    // P0-1 katman 2: head drift.
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

    // P0-1 katman 3: ölçülen path'ler bugün HEAD-tracked + clean (#155 fence'leri —
    // artifact üretiminden bu yana ölçülen dosyalar değişmedi).
    let node_paths: HashMap<u64, String> =
        nodes.iter().map(|n| (n.node_id, n.path.clone())).collect();
    validate_analyzed_paths_tracked(&node_paths, snapshot).map_err(|e| anyhow::anyhow!(e))?;
    validate_analyzed_paths_clean(&node_paths, snapshot, "draft-time baseline fence")
        .map_err(|e| anyhow::anyhow!(e))?;

    build_view(nodes, edges, artifact_head.to_string())
}

/// Baseline'ı canlı analyze ile üret (K1, `--baseline` verilmediğinde; tur-1 P0-1).
///
/// HEAD authority: analiz SONRASI snapshot (`run_analyze` zarfıyla aynı tek kaynak).
/// Fence ailesi: analiz penceresinde HEAD **ve tracked-set** değişmedi; ölçülen
/// path'ler HEAD-tracked ve clean (#155 invariant'ları — hareketli veya
/// commit'siz içeriğe task bağlanamaz).
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
    anyhow::ensure!(
        snapshot_before.tracked_paths == snapshot_after.tracked_paths,
        "repository tracked-path set changed during analysis — a task cannot bind \
         to a state whose file set is moving; retry"
    );
    validate_analyzed_paths_tracked(&result.node_paths, &snapshot_after)
        .map_err(|e| anyhow::anyhow!(e))?;
    validate_analyzed_paths_clean(&result.node_paths, &snapshot_after, "after live analysis")
        .map_err(|e| anyhow::anyhow!(e))?;

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

    fn snapshot_at_head(head: &str) -> RepositorySnapshot {
        RepositorySnapshot {
            head: GitCommitId::try_from(head.to_string()).unwrap(),
            tracked_paths: BTreeSet::new(),
            dirty_paths: BTreeSet::new(),
        }
    }

    fn write_artifact(dir: &Path, repository: serde_json::Value) -> std::path::PathBuf {
        let artifact = dir.join("baseline.json");
        std::fs::write(
            &artifact,
            serde_json::json!({
                "schema_version": 2,
                "repository": repository,
                "nodes": [], "edges": [], "semantic_coverage": {"files_with_scip": 0}
            })
            .to_string(),
        )
        .unwrap();
        artifact
    }

    /// P0-1 katman 1: generic analyze'ın `observed_worktree_unbound` artifact'ı
    /// revizyona bağlı DEĞİL — ölçülmüş içerik HEAD içeriği olmak zorunda değil.
    #[test]
    fn unbound_binding_artifact_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = write_artifact(
            dir.path(),
            serde_json::json!({"head": "a".repeat(40), "clean": false, "binding": "observed_worktree_unbound"}),
        );
        let err =
            load_baseline_artifact(&artifact, &snapshot_at_head(&"a".repeat(40))).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("not revision-bound"), "{msg}");
        assert!(msg.contains("--require-clean-snapshot"), "{msg}");
    }

    /// dirty-worktree'den clean-bound görünümüyle yazılmış sahte artifact → red.
    #[test]
    fn clean_false_artifact_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = write_artifact(
            dir.path(),
            serde_json::json!({"head": "a".repeat(40), "clean": false, "binding": "clean_pre_post_equal"}),
        );
        let err =
            load_baseline_artifact(&artifact, &snapshot_at_head(&"a".repeat(40))).unwrap_err();
        assert!(format!("{err}").contains("dirty repository"), "{err}");
    }

    /// K1 fence sözleşmesi: artifact head ≠ canlı head → fail-closed (exact mesaj
    /// kırıntılarıyla — operatör yönlendirmesi ölçümün yeniden üretilmesine).
    #[test]
    fn artifact_head_drift_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = write_artifact(
            dir.path(),
            serde_json::json!({"head": "b".repeat(40), "clean": true, "binding": "clean_pre_post_equal"}),
        );
        let err =
            load_baseline_artifact(&artifact, &snapshot_at_head(&"a".repeat(40))).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("HEAD drift"), "{msg}");
        assert!(msg.contains(&"b".repeat(40)), "{msg}");
    }

    /// P0-1 katman 3: artifact ölçümünden SONRA analyzed-path dirty'leşti →
    /// draft-time #155 fence'i red (ölçülen içerik artık HEAD içeriği değil).
    #[test]
    fn dirty_analyzed_path_at_draft_time_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = write_artifact(
            dir.path(),
            serde_json::json!({"head": "a".repeat(40), "clean": true, "binding": "clean_pre_post_equal"}),
        );
        // Artifact'ı node'lu yaz (fence'e girmesi için).
        let raw = std::fs::read_to_string(&artifact).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        v["nodes"] = serde_json::json!([{
            "node_id": 0, "path": "src/main.cs", "kind": "module",
            "classification": "production", "role": "runtime", "mass": 1.0,
            "coupling": {"value": 0.0, "source": "tree_sitter", "confidence": 0.75, "coverage": 1.0},
            "cohesion": {"value": 0.5, "source": "placeholder", "confidence": 0.0, "coverage": 0.0},
            "instability": {"value": 0.5, "source": "tree_sitter", "confidence": 0.75, "coverage": 1.0}
        }]);
        std::fs::write(&artifact, v.to_string()).unwrap();

        let mut snapshot = snapshot_at_head(&"a".repeat(40));
        snapshot.tracked_paths.insert("src/main.cs".to_string());
        snapshot.dirty_paths.insert("src/main.cs".to_string());
        let err = load_baseline_artifact(&artifact, &snapshot).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("modified or untracked"), "{msg}");
        assert!(msg.contains("draft-time"), "{msg}");
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
        let err =
            load_baseline_artifact(&artifact, &snapshot_at_head(&"a".repeat(40))).unwrap_err();
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
