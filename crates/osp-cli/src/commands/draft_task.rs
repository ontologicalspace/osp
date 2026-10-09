//! #172: `osp draft-task` — task v2 + proposals v3 üretimi, temsilci doğrulamasıyla.
//!
//! Run 14-16 sürtünme sınıflarını motora taşır: her run için elle yazılan
//! task.json/proposals.json üretim gen-script'leri (SHA transcription + tırnak
//! hataları) yerine, komut HEAD'i `git rev-parse`'ten alır ve İNSANIN verdiği
//! şekil niyetini (`--proposals-spec`) tam v3 şemaya çevirir (#199: path'li
//! new_nodes + yeni düğümlere açılan new_edges uçları).
//!
//! **Epistemik sınır (issue #172 Sınır bölümü):** bu komut YORUM ÜRETMEZ — bar
//! değeri, şekil seçimi, tercih insanda kalır. Ürettiği şey defter tutma +
//! doğrulamadır: temsilci yolları baseline'ın ÖLÇÜLMÜŞ kenar listesine karşı
//! kendisi doğrulanır (K1; #173 *test green ≢ claim tested* ilkesinin defter
//! yüzeyine uygulanması). Uyuşmazlıkta fail-closed; hatada söz konusu düğümün
//! ölçülmüş çıkış-kenarları listelenir (elle py ayıklamanın yerine geçer).
//!
//! **Tur-1 P1-3 (op-matrix):** üretilen proposal'ın yapısal yüzeyinden op
//! gereksinimleri türetilir (`new_nodes→AddNode`, `new_edges→AddEdge`,
//! `removed Imports→RemoveImport`, `removed diğer→RemoveEdge`,
//! `modified_entities→ModifyEntity`) ve task'ın izinli operasyonlarına karşı
//! denetlenir — attempt'in op-matrix'inde reddedilecek çiftler generation-time
//! ölür. Analyzer-owned observational kind'ler (`TypeImports`/`SameNsType`)
//! proposal mutasyonunda yasaktır (#167 sonrası motor sözleşmesi).

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
    CliPathKeyedEdgeRef, CliPathKeyedEdgeSpec, CliPathKeyedEntityChange, CliPathKeyedNewNodeSpecV3,
    CliPathKeyedProposalV3, CliPathKeyedProposalsFileV3,
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
    /// Baseline ölçüm artifact'ı — yalnız revizyona bağlı (`osp analyze
    /// --require-clean-snapshot --out`); HEAD + #155 fence'leri uygulanır.
    /// Verilmezse aynı fence'lerle canlı analyze koşar.
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    #[arg(long, default_value_t = 1)]
    pub milestone_id: u64,
    /// İnsan şekil tanımı (yüz kümeleri + removed/moved kenar niyetleri) → proposals v3.
    /// Kabul edilen alanlar (deny_unknown_fields): new_nodes, new_edges, removed_edges,
    /// affected_nodes, modified_entities, reasoning — proposals ÇIKTI zarfının
    /// schema_version/repository_head/position_hints alanları GİRDİDE YOKTUR (#183).
    /// #199: new_nodes[].path (opsiyonel) yeni düğüme proposal-yerel kimlik verir;
    /// new_edges uçları bu path'lere işaret edebilir (mevcut→yeni / yeni→yeni).
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
    /// Yazılacak proposals v3 dosyası (--proposals-spec ile zorunlu).
    #[arg(long)]
    pub out_proposals: Option<PathBuf>,
}

pub fn run_draft_task(args: DraftTaskArgs) -> anyhow::Result<()> {
    let snapshot = RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    let view = match &args.baseline {
        Some(path) => load_baseline_artifact(path, &snapshot)?,
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

    // Operasyonları ÖNCE parse et (typo burada ölsün; op-matrix denetimi için gerekli).
    let operations: Vec<OpKind> = args
        .operations
        .iter()
        .map(|name| parse_op(name))
        .collect::<Result<_, _>>()?;

    // Spec'i ÖNCE çevir + doğrula: geçersiz spec'te task dosyası da yazılmamalı.
    // Tur-1 P1-3: üretilen proposal'ın op-gereksinimleri task'ın izinli
    // operasyonlarına karşı denetlenir — attempt'te reddedilecek bir çifti
    // generation-time yakalamak bu komutun varlık sebebi.
    let proposals = match &args.proposals_spec {
        Some(spec_path) => {
            let raw = std::fs::read_to_string(spec_path).map_err(|e| {
                anyhow::anyhow!("failed to read proposals spec {}: {e}", spec_path.display())
            })?;
            let spec: ProposalsSpec = serde_json::from_str(&raw).map_err(|e| {
                anyhow::anyhow!(
                    "failed to parse proposals spec {}: {}",
                    spec_path.display(),
                    annotate_output_only_field(&e)
                )
            })?;
            let (file, stats, required_ops) = translate_spec(spec, &view)?;
            let missing: Vec<&str> = required_ops
                .iter()
                .filter(|op| !operations.contains(op))
                .map(|op| op_wire_name(*op))
                .collect();
            anyhow::ensure!(
                missing.is_empty(),
                "proposals spec requires operation(s) {} that the task does not allow \
                 ({}) — add them via --operation or narrow the spec; a proposal the \
                 attempt would reject in the op-matrix must not be generated",
                missing.join(", "),
                args.operations.join(", ")
            );
            eprintln!(
                "delegates verified against measured baseline at HEAD {}: {} path refs, \
                 {} removed-edges (all measured), {} new-node links; op-matrix ok \
                 (required: {})",
                view.head,
                stats.path_refs,
                stats.removed_edges,
                stats.new_node_links,
                required_ops
                    .iter()
                    .map(|op| op_wire_name(*op))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Some(file)
        }
        None => None,
    };

    // Tur-1 P2 + tur-2 P2: her iki payload ÖNCE serialize edilir; iki-dosya
    // seti STAGED publish ile yazılır — TÜM temp'ler hazırlanıp sync olana kadar
    // hiçbir hedef görünmez (ikinci hazırlık hatası ilkinin publish'ini önler).
    // Tam iki-dosya filesystem transaction'ı değil; rename aşaması dosya-başına
    // atomik (atomic_write_replace_staged dokümanunda dürüstçe yazılı).
    preflight_output_aliases(&args)?;
    let task_payload = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 2,
        "repository_head": view.head,
        "scope_bindings": [{"path": args.target}],
        "task": build_task_value(&args, &operations)?,
    }))?;

    match proposals {
        Some(file) => {
            let out = args
                .out_proposals
                .as_ref()
                .expect("clap `requires` guarantees out_proposals with proposals_spec");
            let proposals_payload = serde_json::to_string_pretty(&file)?;
            crate::commands::atomic_write_replace_staged(&[
                (&args.out_task, task_payload.as_bytes()),
                (out, proposals_payload.as_bytes()),
            ])?;
            println!(
                "✓ task v2 written to {} (repository_head {}, target {})",
                args.out_task.display(),
                view.head,
                args.target
            );
            println!(
                "✓ proposals v3 written to {} (repository_head {}, {} proposals)",
                out.display(),
                view.head,
                file.proposals.len()
            );
        }
        None => {
            crate::commands::atomic_write_replace(&args.out_task, task_payload.as_bytes())?;
            println!(
                "✓ task v2 written to {} (repository_head {}, target {})",
                args.out_task.display(),
                view.head,
                args.target
            );
        }
    }
    Ok(())
}

/// Tur-1 P2 + tur-2 P1: çıktılar birbirinin ve KENDİ girdilerinin alias'ı
/// olamaz. Girdi-karşılaştırması `out_proposals` VARLIĞINA bağlı değildir —
/// task-only çağrıda (`--out-task == --baseline`) da aynı fence koşar; aksi
/// halde baseline artifact'ı türetilen task JSON'uyla overwrite edilebilirdi.
fn preflight_output_aliases(args: &DraftTaskArgs) -> anyhow::Result<()> {
    // #182: parent-dizin preflight — ölçüm/doğrulama çalışmadan önce hızlı red
    // (staged publish zaten yazım anındaki hatayı kapsıyor; buradaki amaç hiç
    // başlamamak).
    crate::commands::preflight_out_parent(&args.out_task, "--out-task")?;
    if let Some(out_proposals) = &args.out_proposals {
        crate::commands::preflight_out_parent(out_proposals, "--out-proposals")?;
    }

    let out_task = crate::commands::canon_path(&args.out_task);
    let out_proposals = args
        .out_proposals
        .as_ref()
        .map(|p| crate::commands::canon_path(p));
    if let Some(out_proposals) = &out_proposals {
        anyhow::ensure!(
            out_task != *out_proposals,
            "--out-task and --out-proposals resolve to the same file ({}) — \
             the second write would overwrite the first",
            args.out_task.display()
        );
    }
    for (flag, input) in [
        ("baseline", &args.baseline),
        ("proposals-spec", &args.proposals_spec),
    ] {
        if let Some(input) = input {
            let input = crate::commands::canon_path(input);
            anyhow::ensure!(
                input != out_task,
                "output must not overwrite its own --{flag} input ({}) — \
                 the measurement it was validated against would be replaced by \
                 its derived artifact",
                input.display()
            );
            if let Some(out_proposals) = &out_proposals {
                anyhow::ensure!(
                    input != *out_proposals,
                    "output must not overwrite its own --{flag} input ({}) — \
                     digests are computed over the consumed bytes",
                    input.display()
                );
            }
        }
    }
    Ok(())
}

/// Core `Task` kur → doğrula → v2 wire (predicate scope `{"Path": target}`).
///
/// Scope placeholder'ı `Node(0)` ile kurulur, core validation o aşamada koşar,
/// sonra scope v2 wire'ına rewrite edilir (attempt yükleyicisi `rebind_task_v2`
/// ile ters yönü yapar — üretim/tüketim simetrik).
fn build_task_value(
    args: &DraftTaskArgs,
    operations: &[OpKind],
) -> anyhow::Result<serde_json::Value> {
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
        allowed_operations: operations.to_vec(),
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

/// OpKind → wire adı (op-matrix hata mesajları için; serde tek kaynak).
fn op_wire_name(op: OpKind) -> &'static str {
    match op {
        OpKind::AddImport => "AddImport",
        OpKind::RemoveImport => "RemoveImport",
        OpKind::AddAbstraction => "AddAbstraction",
        OpKind::ExtractModule => "ExtractModule",
        OpKind::AddNode => "AddNode",
        OpKind::RemoveNode => "RemoveNode",
        OpKind::AddEdge => "AddEdge",
        OpKind::RemoveEdge => "RemoveEdge",
        OpKind::ModifyEntity => "ModifyEntity",
    }
}

/// Tur-1 P1-3: analyzer-owned observational kind'ler (#167 `TypeImports` /
/// `SameNsType`) proposal mutation'ında yasaktır — analiz üretir, mutasyon değil.
fn ensure_mutation_kind(kind: EdgeKind, context: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !matches!(kind, EdgeKind::TypeImports | EdgeKind::SameNsType),
        "{context} uses analyzer-owned observational kind {} — proposals cannot \
         create or remove analyzer-measured edges (mutation kinds only)",
        crate::commands::analyze_provenance::CliEdgeKind::from(kind).wire_name()
    );
    Ok(())
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
/// #183: proposals v2 ÇIKTI dosyasında bulunan ama girdi spec'inde bulunmayan
/// alanlar. Çıktıyı şablon olarak kopyalayan kullanıcı (run-17 sürtünmesi 3)
/// unknown-field reddini tek adımda anlaşılır kılmak ister — `deny_unknown_fields`
/// mizacı bozulmaz, yalnızca mesaj zenginleşir.
const PROPOSALS_OUTPUT_ONLY_FIELDS: &[&str] =
    &["schema_version", "repository_head", "position_hints"];

/// Serde unknown-field hatası bir ÇIKTI-only alansa, mesaja "drop it from the
/// input spec" yönlendirmesi eklenir (geri kalan hatalar aynen taşınır).
fn annotate_output_only_field(err: &serde_json::Error) -> String {
    let msg = err.to_string();
    for field in PROPOSALS_OUTPUT_ONLY_FIELDS {
        if msg.contains(&format!("unknown field `{field}`")) {
            return format!(
                "{msg} — `{field}` is an output-only field of the proposals v2 file; \
                 drop it from the input spec"
            );
        }
    }
    msg
}

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
    /// #199: opsiyonel path — yeni düğüme proposal-yerel kimlik verir;
    /// `new_edges` uçları bu path'e işaret edebilir (mevcut→yeni / yeni→yeni
    /// kenar temsili). Path'siz yeni düğümler v2 davranışındadır (yalnız
    /// `connected_to` ile mevcut düğümlere bağlanır, adreslenemez).
    #[serde(default)]
    path: Option<String>,
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

/// Spec → proposals v3 zarfı + temsilci doğrulaması (ölçüme karşı, fail-closed)
/// + op-gereksinimleri (tur-1 P1-3):
///
/// - `new_nodes` → `AddNode`
/// - `new_edges` → `AddEdge`
/// - `removed` Imports → `RemoveImport`; removed diğer kind → `RemoveEdge`
/// - `modified_entities` → `ModifyEntity`
///
/// Gereksinimler task'ın izinli operasyonlarına karşı denetlenir.
///
/// #199: `new_nodes[].path` beyanları proposal-yerel adres alanı kurar —
/// `new_edges` uçları ölçülmüş baseline düğümlerine VEYA bu path'lere işaret
/// edebilir (mevcut→yeni / yeni→yeni). Yeni-düğüm path'i baseline'ı
/// gölgeleyemez ve tekrar edemez; `connected_to` baseline-only kalır
/// (geri-uyumlu kısaltma). `affected_nodes` türetmesi yalnız ölçülmüş
/// uçlardan birleşiktir (danışma alanı ölçülmüş gerçekler içindir).
fn translate_spec(
    spec: ProposalsSpec,
    view: &BaselineView,
) -> anyhow::Result<(CliPathKeyedProposalsFileV3, DelegateStats, Vec<OpKind>)> {
    let node_set = view.node_path_set();
    let mut stats = DelegateStats::default();
    let mut required_ops: Vec<OpKind> = Vec::new();
    let mut require = |op: OpKind| {
        if !required_ops.contains(&op) {
            required_ops.push(op);
        }
    };
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
        // #199 geçiş 1: beyan edilen yeni-düğüm path'lerini topla + doğrula.
        let mut declared_new: BTreeSet<String> = BTreeSet::new();
        for node in &proposal.new_nodes {
            if let Some(path) = &node.path {
                anyhow::ensure!(
                    !node_set.contains(path.as_str()),
                    "new_nodes path {path:?} is already a measured baseline node — \
                     a new node must not shadow an existing path; reference it \
                     directly instead"
                );
                anyhow::ensure!(
                    declared_new.insert(path.clone()),
                    "duplicate new_nodes path {path:?} — each declared new node \
                     must carry a unique path"
                );
            }
        }

        // #199 kenar-uç doğrulaması: ölçülmüş baseline düğümü VEYA aynı
        // proposal'da path beyan edilen yeni düğüm.
        let ensure_endpoint = |path: &str, context: &str| -> anyhow::Result<()> {
            anyhow::ensure!(
                node_set.contains(path) || declared_new.contains(path),
                "{context} references unmeasured path {path:?} that no new_nodes[].path \
                 declares either — endpoints resolve against measured baseline nodes \
                 or new nodes declared with a path in the same proposal ({} nodes at \
                 HEAD {})",
                node_set.len(),
                view.head
            );
            Ok(())
        };

        // #199 geçiş 2: new_nodes → v3 spec (connected_to baseline-only).
        let mut new_nodes = Vec::with_capacity(proposal.new_nodes.len());
        for node in proposal.new_nodes {
            let mut connected = Vec::with_capacity(node.connected_to.len());
            for entry in node.connected_to {
                let (path, kind) = entry.into_path_kind();
                ensure_mutation_kind(kind, "new_nodes.connected_to")?;
                anyhow::ensure!(
                    !declared_new.contains(&path),
                    "new_nodes.connected_to targets new-node path {path:?} — connected_to \
                     resolves baseline nodes only (back-compat shorthand); express \
                     new-node edges via new_edges"
                );
                ensure_node(&path, "new_nodes.connected_to")?;
                stats.path_refs += 1;
                stats.new_node_links += 1;
                connected.push((path, kind));
            }
            new_nodes.push(CliPathKeyedNewNodeSpecV3 {
                kind: node.kind,
                initial_mass: node.initial_mass,
                path: node.path,
                connected_to: connected,
            });
        }
        if !new_nodes.is_empty() {
            require(OpKind::AddNode);
        }

        let mut new_edges = Vec::with_capacity(proposal.new_edges.len());
        for edge in proposal.new_edges {
            let edge_ref = edge.into_edge_ref();
            ensure_mutation_kind(edge_ref.kind, "new_edges")?;
            ensure_endpoint(&edge_ref.from, "new_edges.from")?;
            ensure_endpoint(&edge_ref.to, "new_edges.to")?;
            stats.path_refs += 2;
            new_edges.push(CliPathKeyedEdgeSpec {
                from: edge_ref.from,
                to: edge_ref.to,
                kind: edge_ref.kind,
            });
        }
        if !new_edges.is_empty() {
            require(OpKind::AddEdge);
        }

        let mut removed_edges = Vec::with_capacity(proposal.removed_edges.len());
        for edge in proposal.removed_edges {
            let edge_ref = edge.into_edge_ref();
            ensure_mutation_kind(edge_ref.kind, "removed_edges")?;
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
            require(if edge_ref.kind == EdgeKind::Imports {
                OpKind::RemoveImport
            } else {
                OpKind::RemoveEdge
            });
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
                    // #199: türetme yalnız ölçülmüş uçlardan — yeni-düğüm path'i
                    // danışma alanına girmez (affected_nodes re-bind'i baseline-only).
                    if node_set.contains(e.from.as_str()) {
                        union.insert(e.from.clone());
                    }
                    if node_set.contains(e.to.as_str()) {
                        union.insert(e.to.clone());
                    }
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
        if !modified_entities.is_empty() {
            require(OpKind::ModifyEntity);
        }

        out.push(CliPathKeyedProposalV3 {
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
        CliPathKeyedProposalsFileV3 {
            schema_version: 3,
            repository_head: view.head.clone(),
            proposals: out,
        },
        stats,
        required_ops,
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
        let (file, stats, required_ops) = translate_spec(spec, &view).unwrap();
        assert_eq!(file.proposals.len(), 1);
        assert_eq!(file.proposals[0].removed_edges.len(), 1);
        assert_eq!(stats.removed_edges, 1);
        // affected_nodes türetildi: uçların sıralı-özgün birleşimi.
        assert_eq!(file.proposals[0].affected_nodes, vec!["a.rs", "main.rs"]);
        // Tur-1 P1-3: removed Imports → RemoveImport gereksinimi.
        assert!(
            required_ops.contains(&OpKind::RemoveImport),
            "{required_ops:?}"
        );
    }

    #[test]
    fn spec_op_requirements_derived_per_structural_surface() {
        // new_nodes + new_edges + modified_entities → AddNode + AddEdge + ModifyEntity.
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 5.0, "connected_to": ["a.rs"]}],
                "new_edges": [{"from": "main.rs", "to": "a.rs"}],
                "modified_entities": ["b.rs"],
                "reasoning": "x"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs"), ("main.rs", "b.rs")]);
        let (_, _, required_ops) = translate_spec(spec, &view).unwrap();
        assert!(required_ops.contains(&OpKind::AddNode), "{required_ops:?}");
        assert!(required_ops.contains(&OpKind::AddEdge), "{required_ops:?}");
        assert!(
            required_ops.contains(&OpKind::ModifyEntity),
            "{required_ops:?}"
        );
        assert!(
            !required_ops.contains(&OpKind::RemoveImport),
            "{required_ops:?}"
        );
    }

    #[test]
    fn spec_analyzer_owned_kind_rejected_everywhere() {
        // Tur-1 P1-3: TypeImports/SameNsType analyzer-owned — mutasyon yüzeyinde yasak.
        for raw in [
            r#"{"proposals": [{"new_edges": [{"from": "main.rs", "to": "a.rs", "kind": "TypeImports"}], "reasoning": "x"}]}"#,
            r#"{"proposals": [{"removed_edges": [{"from": "main.rs", "to": "a.rs", "kind": "SameNsType"}], "reasoning": "x"}]}"#,
            r#"{"proposals": [{"new_nodes": [{"kind": "Module", "initial_mass": 5.0, "connected_to": [["a.rs", "TypeImports"]]}], "reasoning": "x"}]}"#,
        ] {
            let spec: ProposalsSpec = serde_json::from_str(raw).unwrap();
            let view = view(&[("main.rs", "a.rs")]);
            let err = translate_spec(spec, &view).unwrap_err();
            let msg = format!("{err}");
            assert!(msg.contains("analyzer-owned"), "{msg}");
        }
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
        let (file, stats, _) = translate_spec(spec, &view).unwrap();
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

    // ── #199: path'li yeni düğümler + yeni düğümlere açılan kenar uçları ────────

    #[test]
    fn spec_v3_new_node_path_enables_new_edges_and_derives_measured_only_affected() {
        // #199: path beyan eden yeni düğüme new_edges ile kenar (mevcut→yeni —
        // run-18'de temsil edilemeyen desen). affected türetmesi yalnız ölçülmüş
        // uçtan birleşir (newmod.rs danışma alanına girmez).
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 5.0, "path": "newmod.rs"}],
                "new_edges": [{"from": "main.rs", "to": "newmod.rs"}],
                "reasoning": "type-move: existing gains import on new node"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs"), ("main.rs", "b.rs")]);
        let (file, _, required_ops) = translate_spec(spec, &view).unwrap();
        assert_eq!(file.schema_version, 3);
        assert_eq!(
            file.proposals[0].new_nodes[0].path.as_deref(),
            Some("newmod.rs")
        );
        assert_eq!(file.proposals[0].new_edges[0].to, "newmod.rs");
        assert_eq!(file.proposals[0].affected_nodes, vec!["main.rs"]);
        assert!(required_ops.contains(&OpKind::AddNode), "{required_ops:?}");
        assert!(required_ops.contains(&OpKind::AddEdge), "{required_ops:?}");
    }

    #[test]
    fn spec_new_node_path_shadowing_and_duplicate_rejected() {
        // Baseline'ı gölgeleyen path → red (yeni düğüm ölçülmüş yolu bastıramaz).
        let shadow: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"new_nodes": [{"kind": "Module", "initial_mass": 1.0,
                              "path": "a.rs"}], "reasoning": "x"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs")]);
        let err = translate_spec(shadow, &view).unwrap_err();
        assert!(format!("{err}").contains("shadow"), "{err}");

        // Aynı path iki yeni düğümde → red (adres alanı injective olmalı).
        let dup: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"new_nodes": [
                {"kind": "Module", "initial_mass": 1.0, "path": "n.rs"},
                {"kind": "Module", "initial_mass": 1.0, "path": "n.rs"}],
                "reasoning": "x"}]}"#,
        )
        .unwrap();
        let err = translate_spec(dup, &view).unwrap_err();
        assert!(format!("{err}").contains("unique path"), "{err}");
    }

    #[test]
    fn spec_connected_to_new_node_rejected_with_new_edges_guidance() {
        // connected_to geri-uyumlu kısaltma: baseline-only; yeni düğüme kenar
        // new_edges üzerinden ifade edilir.
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"new_nodes": [
                {"kind": "Module", "initial_mass": 1.0, "path": "n1.rs"},
                {"kind": "Module", "initial_mass": 1.0, "path": "n2.rs",
                 "connected_to": ["n1.rs"]}],
                "reasoning": "x"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs")]);
        let err = translate_spec(spec, &view).unwrap_err();
        assert!(format!("{err}").contains("new_edges"), "{err}");
    }

    #[test]
    fn spec_edge_endpoint_undeclared_anywhere_fails_closed() {
        // Ne baseline'ta ne de beyan edilen yeni-düğüm path'lerinde → red.
        let spec: ProposalsSpec = serde_json::from_str(
            r#"{"proposals": [{"new_nodes": [{"kind": "Module", "initial_mass": 1.0,
                              "path": "n.rs"}],
                "new_edges": [{"from": "main.rs", "to": "ghost.rs"}],
                "reasoning": "x"}]}"#,
        )
        .unwrap();
        let view = view(&[("main.rs", "a.rs")]);
        let err = translate_spec(spec, &view).unwrap_err();
        assert!(
            format!("{err}").contains("no new_nodes[].path declares"),
            "{err}"
        );
    }
}
