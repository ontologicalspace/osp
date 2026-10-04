//! #172: `osp draft-task` — task v2 + proposals v2 üretimi, temsilci doğrulamasıyla.
//!
//! Run 14-16 sürtünme sınıflarını motora taşır: her run için elle yazılan
//! task.json/proposals.json üretim gen-script'leri (SHA transcription + tırnak
//! hataları) yerine, komut HEAD'i `git rev-parse`'ten alır ve İNSANIN verdiği
//! şekil niyetini (`--proposals-spec`) tam v2 şemaya çevirir.
//!
//! **Epistemik sınır (issue #172 Sınır bölümü):** bu komut YORUM ÜRETMEZ — bar
//! değeri, şekil seçimi, tercih insanda kalır. Ürettiği şey defter tutma +
//! doğrulamadır: temsilci yolları baseline'ın ÖLÇÜLMÜŞ kenar listesine karşı
//! kendisi doğrulanır (K1; #173 *test green ≢ claim tested* ilkesinin defter
//! yüzeyine uygulanması). Uyuşmazlıkta fail-closed; hatada söz konusu düğümün
//! ölçülmüş çıkış-kenarları listelenir (elle py ayıklamanın yerine geçer).

use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Args;
use osp_core::coords::{MetricSource, RawPosition};
use osp_core::space::{EdgeKind, NodeKind};
use osp_core::trajectory::{
    ColdStartPolicy, ComparisonOp, MetricPredicate, OpKind, PredicateAxis, PredicateFailurePolicy,
    PredicateMode, PredicateScope, PredicateSet, RuleRef, Task, TaskPolicy, TaskStatus,
    WeightedPredicate,
};

use crate::commands::baseline::{analyze_live, load_baseline_artifact, BaselineView};
use crate::commands::path_keyed_proposals::{
    CliPathKeyedEdgeRef, CliPathKeyedEdgeSpec, CliPathKeyedEntityChange, CliPathKeyedNewNodeSpec,
    CliPathKeyedProposal, CliPathKeyedProposalsFileV2,
};
use crate::commands::repo_snapshot::RepositorySnapshot;

/// Ritüel-standard preferred vector (dogfood task dosyalarının değişmezi).
const PREFERRED_VECTOR: RawPosition = RawPosition {
    x: 0.55,
    y: 0.6,
    z: 0.5,
    w: 0.5,
    v: 0.3,
};

/// `osp draft-task` — task v2 (+ opsionel proposals v2) üret.
#[derive(Args, Debug)]
pub struct DraftTaskArgs {
    /// Analiz edilecek repo path'i.
    #[arg(long)]
    pub repo: PathBuf,
    /// Hedef düğümün repo-göreli path'i (ileri-slash; ölçülmüş baseline düğümü olmak zorunda).
    #[arg(long)]
    pub target: String,
    /// Task ID (attempt positional'ı ile birebir aynı olmalı).
    #[arg(long)]
    pub task_id: u64,
    /// İnsan niyeti — task label'ı (yorum motorda üretilmez, taşınır).
    #[arg(long)]
    pub label: String,
    /// τ bar'ı — el tahminlerinden dondurulur (predicate threshold: coupling ≤ bar).
    #[arg(long)]
    pub bar: f64,
    /// Baseline ölçüm artifact'ı (`osp analyze --out`). Verilirse HEAD exact-match
    /// fence'e girer (drift → fail-closed); verilmezse canlı analyze koşar.
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    #[arg(long, default_value_t = 1)]
    pub milestone_id: u64,
    /// İnsan şekil tanımı (yüz kümeleri + removed/moved kenar niyetleri) → proposals v2.
    /// Verilirse --out-proposals zorunlu.
    #[arg(long, requires = "out_proposals")]
    pub proposals_spec: Option<PathBuf>,
    /// Task constraint'leri (insan prozu; aynen taşınır).
    #[arg(long = "constraint")]
    pub constraints: Vec<String>,
    /// İzinli operasyonlar (core OpKind wire adları). Default: AddNode, RemoveImport.
    #[arg(long = "operation", default_values_t = vec!["AddNode".to_string(), "RemoveImport".to_string()])]
    pub operations: Vec<String>,
    #[arg(long, default_value_t = 0.005)]
    pub min_improvement_delta: f64,
    #[arg(long, default_value_t = 0.15)]
    pub max_axis_regression: f64,
    #[arg(long, default_value_t = 3)]
    pub maneuver_limit: u32,
    /// Yazılacak task v2 dosyası.
    #[arg(long)]
    pub out_task: PathBuf,
    /// Yazılacak proposals v2 dosyası (--proposals-spec ile zorunlu).
    #[arg(long)]
    pub out_proposals: Option<PathBuf>,
}

pub fn run_draft_task(args: DraftTaskArgs) -> anyhow::Result<()> {
    let snapshot = RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    let live_head = snapshot.head.as_str().to_string();
    let view = match &args.baseline {
        Some(path) => load_baseline_artifact(path, &live_head)?,
        None => analyze_live(&args.repo)?,
    };

    // Hedef ölçülmüş bir baseline düğümü olmalı (el beyanı değil).
    let node_set = view.node_path_set();
    anyhow::ensure!(
        node_set.contains(args.target.as_str()),
        "target {:?} is not a measured baseline node ({} nodes at HEAD {}) — \
         run `osp suggest-targets` for the measured candidate list",
        args.target,
        node_set.len(),
        view.head
    );

    // Spec'i ÖNCE çevir + doğrula: geçersiz spec'te task dosyası da yazılmamalı.
    let proposals = match &args.proposals_spec {
        Some(spec_path) => {
            let raw = std::fs::read_to_string(spec_path).map_err(|e| {
                anyhow::anyhow!("failed to read proposals spec {}: {e}", spec_path.display())
            })?;
            let spec: ProposalsSpec = serde_json::from_str(&raw).map_err(|e| {
                anyhow::anyhow!(
                    "failed to parse proposals spec {}: {e}",
                    spec_path.display()
                )
            })?;
            let (file, stats) = translate_spec(spec, &view)?;
            eprintln!(
                "delegates verified against measured baseline at HEAD {}: {} path refs, \
                 {} removed-edges (all measured), {} new-node links",
                view.head, stats.path_refs, stats.removed_edges, stats.new_node_links
            );
            Some(file)
        }
        None => None,
    };

    let task_value = build_task_value(&args)?;
    let envelope = serde_json::json!({
        "schema_version": 2,
        "repository_head": view.head,
        "scope_bindings": [{"path": args.target}],
        "task": task_value,
    });
    std::fs::write(&args.out_task, serde_json::to_string_pretty(&envelope)?)?;
    println!(
        "✓ task v2 written to {} (repository_head {}, target {})",
        args.out_task.display(),
        view.head,
        args.target
    );

    if let Some(file) = proposals {
        let out = args
            .out_proposals
            .as_ref()
            .expect("clap `requires` guarantees out_proposals with proposals_spec");
        std::fs::write(out, serde_json::to_string_pretty(&file)?)?;
        println!(
            "✓ proposals v2 written to {} (repository_head {}, {} proposals)",
            out.display(),
            view.head,
            file.proposals.len()
        );
    }
    Ok(())
}

/// Core `Task` kur → doğrula → v2 wire (predicate scope `{"Path": target}`).
///
/// Scope placeholder'ı `Node(0)` ile kurulur, core validation o aşamada koşar,
/// sonra scope v2 wire'ına rewrite edilir (attempt yükleyicisi `rebind_task_v2`
/// ile ters yönü yapar — üretim/tüketim simetrik).
fn build_task_value(args: &DraftTaskArgs) -> anyhow::Result<serde_json::Value> {
    let mut operations = Vec::with_capacity(args.operations.len());
    for name in &args.operations {
        operations.push(parse_op(name)?);
    }
    let task = Task {
        id: args.task_id,
        milestone_id: args.milestone_id,
        label: args.label.clone(),
        target_predicate_set: PredicateSet {
            mode: PredicateMode::All,
            predicates: vec![WeightedPredicate {
                predicate: MetricPredicate {
                    metric: PredicateAxis::Coupling,
                    operator: ComparisonOp::Le,
                    threshold: args.bar,
                    scope: PredicateScope::Node(0),
                    required_source: Some(MetricSource::TreeSitter),
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: Some(PREFERRED_VECTOR),
        },
        policy: TaskPolicy {
            predicate_failure_policy: PredicateFailurePolicy::StrictReject,
            min_improvement_delta: args.min_improvement_delta,
            max_axis_regression: args.max_axis_regression,
            maneuver_limit: args.maneuver_limit,
            allow_progress_checkpoint: false,
            cold_start_policy: ColdStartPolicy::Disallow,
        },
        allowed_operations: operations,
        constraints: args
            .constraints
            .iter()
            .map(|c| RuleRef(c.clone()))
            .collect(),
        status: TaskStatus::Pending,
    };
    task.validate()
        .map_err(|e| anyhow::anyhow!("drafted task fails core validation: {e}"))?;

    let mut value = serde_json::to_value(&task)?;
    let target = args.target.clone();
    let predicates = value
        .get_mut("target_predicate_set")
        .and_then(|v| v.get_mut("predicates"))
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("task serialization lost predicate list"))?;
    for weighted in predicates {
        weighted["predicate"]["scope"] = serde_json::json!({"Path": target});
    }
    Ok(value)
}

/// Core OpKind wire adı ("AddNode", "RemoveImport", ...) — typo fail-closed.
fn parse_op(name: &str) -> anyhow::Result<OpKind> {
    serde_json::from_value(serde_json::json!(name)).map_err(|_| {
        anyhow::anyhow!(
            "unknown operation {name:?} — expected core OpKind wire names \
             (AddNode, RemoveImport, ExtractModule, ...)"
        )
    })
}

/// Doğrulanan temsilci sayıları (stdout özeti — yorum değil, sayım).
#[derive(Debug, Default)]
struct DelegateStats {
    path_refs: usize,
    removed_edges: usize,
    new_node_links: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Proposals-spec: minimal insan yüzeyi (K5)
// ─────────────────────────────────────────────────────────────────────────────

/// İnsan şekil tanımı — `{"proposals": [...]}`.
///
/// Bilinçli sadeleştirme (K5): `repository_head` YOK (motor doldurur), kenar
/// `kind` default `Imports`, `connected_to` çıplak path veya `[path, kind]`.
/// `deny_unknown_fields` — typo parse'te yakalanır (tırnak hatası sınıfı motor
/// assert'ine değil parse'a taşınır).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalsSpec {
    proposals: Vec<ProposalSpec>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalSpec {
    #[serde(default)]
    new_nodes: Vec<NewNodeHuman>,
    #[serde(default)]
    new_edges: Vec<EdgeHuman>,
    #[serde(default)]
    removed_edges: Vec<EdgeHuman>,
    /// Verilmezse türetilir: removed/new kenar uçlarının sıralı-özgün birleşimi.
    #[serde(default)]
    affected_nodes: Option<Vec<String>>,
    #[serde(default)]
    modified_entities: Vec<String>,
    reasoning: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NewNodeHuman {
    kind: NodeKind,
    initial_mass: f64,
    #[serde(default)]
    connected_to: Vec<PathOrKinded>,
}

/// Çıplak path (`"a.cs"`) veya kind'lı çift (`["a.cs", "Imports"]`).
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum PathOrKinded {
    Plain(String),
    Kinded(String, EdgeKind),
}

impl PathOrKinded {
    fn into_path_kind(self) -> (String, EdgeKind) {
        match self {
            Self::Plain(path) => (path, EdgeKind::Imports),
            Self::Kinded(path, kind) => (path, kind),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeHuman {
    from: String,
    to: String,
    #[serde(default)]
    kind: Option<EdgeKind>,
}

impl EdgeHuman {
    fn into_edge_ref(self) -> CliPathKeyedEdgeRef {
        CliPathKeyedEdgeRef {
            from: self.from,
            to: self.to,
            kind: self.kind.unwrap_or(EdgeKind::Imports),
        }
    }
}

/// Spec → proposals v2 zarfı + temsilci doğrulaması (ölçüme karşı, fail-closed).
fn translate_spec(
    spec: ProposalsSpec,
    view: &BaselineView,
) -> anyhow::Result<(CliPathKeyedProposalsFileV2, DelegateStats)> {
    let node_set = view.node_path_set();
    let mut stats = DelegateStats::default();
    let mut out = Vec::with_capacity(spec.proposals.len());

    let ensure_node = |path: &str, context: &str| -> anyhow::Result<()> {
        anyhow::ensure!(
            node_set.contains(path),
            "{context} references unmeasured path {path:?} — delegate paths must be \
             measured baseline nodes ({} nodes at HEAD {})",
            node_set.len(),
            view.head
        );
        Ok(())
    };

    for proposal in spec.proposals {
        let mut new_nodes = Vec::with_capacity(proposal.new_nodes.len());
        for node in proposal.new_nodes {
            let mut connected = Vec::with_capacity(node.connected_to.len());
            for entry in node.connected_to {
                let (path, kind) = entry.into_path_kind();
                ensure_node(&path, "new_nodes.connected_to")?;
                stats.path_refs += 1;
                stats.new_node_links += 1;
                connected.push((path, kind));
            }
            new_nodes.push(CliPathKeyedNewNodeSpec {
                kind: node.kind,
                initial_mass: node.initial_mass,
                connected_to: connected,
            });
        }

        let mut new_edges = Vec::with_capacity(proposal.new_edges.len());
        for edge in proposal.new_edges {
            let edge_ref = edge.into_edge_ref();
            ensure_node(&edge_ref.from, "new_edges.from")?;
            ensure_node(&edge_ref.to, "new_edges.to")?;
            stats.path_refs += 2;
            new_edges.push(CliPathKeyedEdgeSpec {
                from: edge_ref.from,
                to: edge_ref.to,
                kind: edge_ref.kind,
            });
        }

        let mut removed_edges = Vec::with_capacity(proposal.removed_edges.len());
        for edge in proposal.removed_edges {
            let edge_ref = edge.into_edge_ref();
            // Sıra (run-14/16 typo sınıfının debugging yardımı): `from` bilinen bir
            // ölçülmüş düğümdense, kenar-varlık kontrolü ÖNCE gelir ve hatada o
            // düğümün ölçülmüş çıkış-kenarlarını listeler — `to` ucundaki typo
            // "unmeasured path" değil, ölçülmüş listeyle döner.
            ensure_node(&edge_ref.from, "removed_edges.from")?;
            stats.path_refs += 1;
            let measured = view
                .edges
                .contains(&crate::commands::baseline::BaselineEdge {
                    from: edge_ref.from.clone(),
                    to: edge_ref.to.clone(),
                    kind: edge_ref.kind,
                });
            anyhow::ensure!(
                measured,
                "removed edge is NOT measured in the baseline: {} -> {} ({})\n\
                 measured out-edges of {}: {}",
                edge_ref.from,
                edge_ref.to,
                crate::commands::analyze_provenance::CliEdgeKind::from(edge_ref.kind).wire_name(),
                edge_ref.from,
                view.measured_out_edges(&edge_ref.from).join(", ")
            );
            stats.removed_edges += 1;
            removed_edges.push(edge_ref);
        }

        let affected_nodes = match proposal.affected_nodes {
            Some(explicit) => {
                for path in &explicit {
                    ensure_node(path, "affected_nodes")?;
                    stats.path_refs += 1;
                }
                explicit
            }
            None => {
                let mut union: BTreeSet<String> = BTreeSet::new();
                for e in &removed_edges {
                    union.insert(e.from.clone());
                    union.insert(e.to.clone());
                }
                for e in &new_edges {
                    union.insert(e.from.clone());
                    union.insert(e.to.clone());
                }
                union.into_iter().collect()
            }
        };

        let mut modified_entities = Vec::with_capacity(proposal.modified_entities.len());
        for path in proposal.modified_entities {
            ensure_node(&path, "modified_entities")?;
            stats.path_refs += 1;
            modified_entities.push(CliPathKeyedEntityChange { path });
        }

        out.push(CliPathKeyedProposal {
            new_nodes,
            new_edges,
            removed_edges,
            affected_nodes,
            modified_entities,
            position_hints: Vec::new(),
            reasoning: proposal.reasoning,
        });
    }

    Ok((
        CliPathKeyedProposalsFileV2 {
            schema_version: 2,
            repository_head: view.head.clone(),
            proposals: out,
        },
        stats,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn view(edges: &[(&str, &str)]) -> BaselineView {
        // Gerçek DTO düğümleri — node_path_set() nodes'tan türetilir; boş nodes
        // kümesi temsilci doğrulamasını hep düşürürdü (vacuous-red tuzağı).
        let node = |id: u64, path: &str| {
            serde_json::from_value::<crate::commands::analyze_provenance::CliAnalyzeNode>(
                serde_json::json!({
                    "node_id": id, "path": path, "kind": "module",
                    "classification": "production", "role": "runtime", "mass": 1.0,
                    "coupling": {"value": 0.0, "source": "tree_sitter", "confidence": 0.75, "coverage": 1.0},
                    "cohesion": {"value": 0.5, "source": "placeholder", "confidence": 0.0, "coverage": 0.0},
                    "instability": {"value": 0.5, "source": "tree_sitter", "confidence": 0.75, "coverage": 1.0}
                }),
            )
            .expect("test node DTO")
        };
        let mut paths: Vec<String> = Vec::new();
        let mut edge_set = HashSet::new();
        for (from, to) in edges {
            for p in [*from, *to] {
                if !paths.iter().any(|x| x == p) {
                    paths.push(p.to_string());
                }
            }
            edge_set.insert(crate::commands::baseline::BaselineEdge {
                from: from.to_string(),
                to: to.to_string(),
                kind: EdgeKind::Imports,
            });
        }
        BaselineView {
            head: "a".repeat(40),
            nodes: paths
                .iter()
                .enumerate()
                .map(|(i, p)| node(i as u64, p))
                .collect(),
            edges: edge_set,
            imports_out_by_path: Default::default(),
        }
    }

    #[test]
    fn spec_removed_edge_must_be_measured() {
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"removed_edges": [{"from": "main.rs", "to": "a.rs"}],
                              "reasoning": "drop unused import"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs"), ("main.rs", "b.rs")]);
        let (file, stats) = translate_spec(spec, &view).unwrap();
        assert_eq!(file.proposals.len(), 1);
        assert_eq!(file.proposals[0].removed_edges.len(), 1);
        assert_eq!(stats.removed_edges, 1);
        // affected_nodes türetildi: uçların sıralı-özgün birleşimi.
        assert_eq!(file.proposals[0].affected_nodes, vec!["a.rs", "main.rs"]);
    }

    #[test]
    fn spec_unmeasured_removed_edge_fails_closed_with_measured_list() {
        // main.rs -> b.rs ölçülmüş; spec c.rs hedefini istiyor → red + ölçülen liste.
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"removed_edges": [{"from": "main.rs", "to": "c.rs"}],
                              "reasoning": "typo class"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs"), ("main.rs", "b.rs")]);
        let err = translate_spec(spec, &view).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("NOT measured"), "{msg}");
        assert!(msg.contains("-> a.rs (imports)"), "{msg}");
        assert!(msg.contains("-> b.rs (imports)"), "{msg}");
    }

    #[test]
    fn spec_unmeasured_connected_to_fails_closed() {
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"new_nodes": [{"kind": "Module", "initial_mass": 5.0,
                              "connected_to": ["nope.rs"]}], "reasoning": "x"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs")]);
        let err = translate_spec(spec, &view).unwrap_err();
        assert!(format!("{err}").contains("unmeasured path"), "{err}");
    }

    #[test]
    fn spec_kinded_connected_to_and_default_imports() {
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"new_nodes": [{"kind": "Module", "initial_mass": 5.0,
                              "connected_to": ["a.rs", ["b.rs", "Imports"]]}],
                              "reasoning": "x"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs"), ("main.rs", "b.rs")]);
        let (file, stats) = translate_spec(spec, &view).unwrap();
        let connected = &file.proposals[0].new_nodes[0].connected_to;
        assert_eq!(connected[0], ("a.rs".to_string(), EdgeKind::Imports));
        assert_eq!(connected[1], ("b.rs".to_string(), EdgeKind::Imports));
        assert_eq!(stats.new_node_links, 2);
    }

    #[test]
    fn spec_typo_field_rejected_at_parse() {
        let err = serde_json::from_str::<ProposalsSpec>(
            r#"{"proposals": [{"removed_edge": [], "reasoning": "typo"}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("removed_edge"), "{err}");
    }

    #[test]
    fn op_wire_names_parse_and_typo_fails() {
        assert!(matches!(parse_op("AddNode").unwrap(), OpKind::AddNode));
        assert!(matches!(
            parse_op("RemoveImport").unwrap(),
            OpKind::RemoveImport
        ));
        assert!(parse_op("add-node").is_err());
    }
}
