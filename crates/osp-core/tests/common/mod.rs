//! Faz 8-P2 P2-0B characterization corpus — case builders + manifest loader.
//!
//! Bu modül, V1 (`compute_raw_from_delta` + `provenanced_from_raw`) ile V2-candidate
//! (`measure_task_delta`) semantic divergence'ını ölçen characterization test'leri için
//! ortak case builders ve fixture loading altyapısı sağlar.
//!
//! ## Frozen fixture modeli
//!
//! Plan (Tur 7) "neutral fixture" gerektirir: corpus değişikliği digest doğrulaması
//! tarafından TESPİT EDİLMELİ (explicit fixture+digest güncellemesi gerektirir).
//!
//! Bu modül **builder + manifest + digest** hibrit modelini kullanır:
//! - Case içeriği Rust builder fonksiyonlarıyla üretilir (invariant-faithful — smart
//!   constructor'lar `Intent::new` vb. üzerinden).
//! - `tests/data/faz8_p2_characterization/cases.json` manifest her case'in metadata'sını
//!   + builder çıktısının serialize-bytes BLAKE3 digest'ini pinler.
//! - Test, builder'ı çalıştırır → serialize eder → BLAKE3 hesaplar → manifest'teki
//!   digest ile karşılaştırır. Builder drift'i digest'i değiştirir → test fail.
//!
//! Bu model, planın LF-pin + sidecar + per-fixture digest gereksinimlerini korur
//! ve pure-JSON fixture'den daha invariant-faithful (serde default'ları yerine
//! smart constructor'lar).
//!
//! ## Path-traversal guard
//!
//! Loader, manifest'teki runtime path'leri `CARGO_MANIFEST_DIR` altında canonicalize
//! edip root containment kontrolü yapar — `../` kaçışı kapalı.

#![allow(
    dead_code,
    reason = "shared test helper — farklı test crate'ler farklı parçalarını kullanır"
)]

use osp_core::agent::DeltaProposal;
use osp_core::space::Space;
use osp_core::trajectory::Task;
use std::path::{Path, PathBuf};

// ═══════════════════════════════════════════════════════════════════════════════
// Case modeli
// ═══════════════════════════════════════════════════════════════════════════════

/// Bir characterization case'inin tüm girdilerini taşır.
///
/// `space` V1/V2 ayrı engine'lere kopyalanır (state izolasyonu — her engine aynı
/// başlangıç `SpaceDigest`'e sahip olmalı). `task` ve `claim` V1/V2 candidate
/// harness'lerinde aynı identity ile kullanılır (subject authority divergence
/// ölçümü için `affected_nodes` `DeltaProposal` içinde diff eder).
#[derive(Debug, Clone)]
pub struct CharacterizationCase {
    pub id: String,
    pub class: CaseClass,
    pub source: CaseSource,
    pub description: String,
    /// V1/V2 engine'lerin ortak başlangıç space'i (clone ile iki kopyaya ayrılır).
    pub space: Space,
    /// Task declaration (subject scope kaynağı — V2 authority).
    pub task: Task,
    /// Structural delta + LLM-declared affected_nodes (V1 measurement scope kaynağı).
    pub proposal: DeltaProposal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseClass {
    MatchingScope,
    /// proposal.affected_nodes ⊃ task.predicate.scope — known production-reachable divergence.
    WideAffectedScope,
    /// removed_edges[*].from outside task.predicate.scope — divergence.
    RemovedEdgeExternalSource,
    /// task.predicate.scope members all delta-introduced — V2 baseline Unavailable.
    DeltaIntroducedSubject,
    /// subject members report different MetricSource per axis — provenance divergence.
    ///
    /// **TODO (P2-0B kalan iş):** Henüz case builder üretemez (build_all_cases'te yok).
    /// Case 2 (wide-affected) zaten per-axis source divergence gösterdi ama dedicated
    /// required_source matrisi (Any/Exact(Scip)/Exact(Heuristic)/mixed/mismatch) eksik.
    MixedPerAxisSources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseSource {
    SyntheticAdversarial,
    NavigatorFixture,
    McpSubmitFixture,
    AnalyzerCorpus,
}

/// Manifest'ten deserialize edilen tek case kaydı.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestCase {
    pub id: String,
    pub class: CaseClass,
    pub source: CaseSource,
    #[serde(default)]
    pub description: String,
    /// Builder çıktısının (serialized form) BLAKE3 digest'i — lowercase 64-hex.
    /// "TBD_BY_BUILDER_RUN" placeholder initial commit'te; gerçek digest builder
    /// çalıştırıldıktan sonra `update_manifest_digests` helper ile yazılır.
    pub builder_digest_blake3: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(default)]
    pub description: String,
    pub cases: Vec<ManifestCase>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Fixture loading + path-traversal guard + digest doğrulama
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum FixtureLoadError {
    #[error("characterization root not found at {path}: {source}")]
    RootNotFound {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("fixture path escapes characterization root: {path}")]
    PathEscapesRoot { path: String },
    #[error("fixture read failed for {path}: {source}")]
    ReadFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("manifest parse failed: {0}")]
    ManifestParse(#[from] serde_json::Error),
    #[error("sidecar parse failed: {0}")]
    SidecarParse(String),
    #[error("digest mismatch for case {case_id}: expected {expected}, actual {actual}")]
    DigestMismatch {
        case_id: String,
        expected: String,
        actual: String,
    },
    #[error("sidecar digest mismatch: expected {expected}, actual {actual}")]
    SidecarDigestMismatch { expected: String, actual: String },
    #[error("case builder not found for id: {0}")]
    BuilderNotFound(String),
}

/// Characterization fixture kök dizini.
///
/// `crates/osp-core`'dan çağrıldığında `../../tests/data/faz8_p2_characterization`
/// olur. `crates/osp-mcp`'den çağrıldığında çağıran crate CARGO_MANIFEST_DIR'ini
/// vermelidir.
pub fn characterization_root(manifest_dir: &str) -> PathBuf {
    Path::new(manifest_dir).join("../../tests/data/faz8_p2_characterization")
}

/// Path-traversal-safe fixture loader.
///
/// `relative` path'i `root` altında canonicalize eder; candidate root dışına
/// çıkarsa `PathEscapesRoot` ile reddeder. Bu, manifest'teki dinamik path'lerin
/// (`../` kaçışı dahil) güvenli işlenmesini sağlar.
pub fn load_fixture_bytes(root: &Path, relative: &str) -> Result<Vec<u8>, FixtureLoadError> {
    let candidate = root.join(relative);
    let canonical_root = root
        .canonicalize()
        .map_err(|e| FixtureLoadError::RootNotFound {
            path: root.to_path_buf(),
            source: e,
        })?;
    let canonical_candidate =
        candidate
            .canonicalize()
            .map_err(|e| FixtureLoadError::ReadFailed {
                path: relative.to_string(),
                source: e,
            })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(FixtureLoadError::PathEscapesRoot {
            path: relative.to_string(),
        });
    }
    std::fs::read(&canonical_candidate).map_err(|e| FixtureLoadError::ReadFailed {
        path: relative.to_string(),
        source: e,
    })
}

/// Manifest JSON'u yükler + sidecar `cases.blake3` digest'ini doğrular.
///
/// Sidecar format: 64 lowercase hexadecimal karakter + opsiyonel tek LF.
/// Strict parser — büyük harf veya 64-dışı uzunluk reddeder.
pub fn load_manifest(root: &Path) -> Result<Manifest, FixtureLoadError> {
    let manifest_bytes = load_fixture_bytes(root, "cases.json")?;
    let sidecar_bytes = load_fixture_bytes(root, "cases.blake3")?;

    // Sidecar parse: strict 64 lowercase hex.
    let sidecar_text = std::str::from_utf8(&sidecar_bytes)
        .map_err(|e| FixtureLoadError::SidecarParse(format!("sidecar not UTF-8: {e}")))?;
    let expected_hex = sidecar_text.trim_ascii();
    validate_digest_hex(expected_hex)
        .map_err(|e| FixtureLoadError::SidecarParse(format!("invalid sidecar format: {e}")))?;

    // Sidecar digest == BLAKE3(manifest raw bytes)?
    let actual_hex = blake3_hex(&manifest_bytes);
    if expected_hex != actual_hex {
        return Err(FixtureLoadError::SidecarDigestMismatch {
            expected: expected_hex.to_string(),
            actual: actual_hex,
        });
    }

    serde_json::from_slice::<Manifest>(&manifest_bytes).map_err(FixtureLoadError::ManifestParse)
}

/// 64 lowercase hexadecimal kontrolü (sidecar + per-fixture digest için).
fn validate_digest_hex(s: &str) -> Result<(), String> {
    if s.len() != 64 {
        return Err(format!("expected 64 hex chars, got {}", s.len()));
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("must be lowercase hex (uppercase rejected)".to_string());
    }
    Ok(())
}

/// BLAKE3 digest → lowercase 64-hex string.
pub fn blake3_hex(bytes: &[u8]) -> String {
    let hash = blake3::hash(bytes);
    hex::encode(hash.as_bytes())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case builders
// ═══════════════════════════════════════════════════════════════════════════════

/// Tüm characterization case'lerini builder olarak üretir.
///
/// Her case invariant-faithful smart constructor'lar kullanır (`Intent::new`,
/// `build_claim_from_proposal` pattern). Case içeriği serialize edilip BLAKE3
/// digest'ı manifest'te pinlenir — builder drift'i tespit edilir.
///
/// Yeni case ekleme: (1) builder fonksiyonu yaz, (2) bu fonksiyona ekle, (3)
/// `update_manifest_digests` helper'ı çalıştır → manifest güncellenir.
pub fn build_all_cases() -> Vec<CharacterizationCase> {
    vec![
        matching_single_node_001(),
        wide_affected_scope_001(),
        removed_edge_external_source_001(),
        delta_introduced_subject_001(),
        delta_introduced_subject_policy_001(),
    ]
}

/// Builder → manifest digest güncelleme helper'ı (test amaçlı).
///
/// Tüm case'leri üretir, her birinin serialize-bytes BLAKE3 digest'ini hesaplar,
/// manifest'teki `builder_digest_blake3` alanını günceller. Initial commit'te
/// "TBD_BY_BUILDER_RUN" placeholder'ları doldurmak için kullanılır.
pub fn compute_case_digests() -> Vec<(String, String)> {
    build_all_cases()
        .into_iter()
        .map(|case| {
            let bytes = serialize_case_bytes(&case);
            (case.id, blake3_hex(&bytes))
        })
        .collect()
}

/// Bir case'in deterministik serialize-bytes'ı (digest için).
///
/// **Kritik:** `Space.nodes: HashMap<NodeId, Node>` iteration order'ı Rust random
/// seed'ine bağlıdır (her process'de farklı). Bu yüzden düz `serde_json::to_vec`
/// nondeterministik digest üretir. Çözüm: önce `serde_json::Value`'ya parse edip
/// `to_string_pretty` + sort_keys yerine, `Serializer` ile canonical sorted output
/// almak. En temiz yol: her üçü için de `serde_json::to_value` → recursiv olarak
/// objeleri BTreeMap karşılığına çevir → serialize.
///
/// Pratik implementation: `serde_json::to_string_pretty` zaten default feature'suz
/// serde_json'de map'leri sort eder (BTreeMap-backed `Map` type). HashMap direkt
/// serialize entry order kullanır — bu yüzden önce `Value`'ya, sonra tekrar serialize.
pub fn serialize_case_bytes(case: &CharacterizationCase) -> Vec<u8> {
    // serde_json::Value (BTreeMap-backed default) → deterministic key order.
    // HashMap entry order Rust process-random seed'e bağlı; Value'ya round-trip
    // canonical sorted order verir.
    let space_val = serde_json::to_value(&case.space).expect("space → Value");
    let task_val = serde_json::to_value(&case.task).expect("task → Value");
    let proposal_val = serde_json::to_value(&case.proposal).expect("proposal → Value");
    let space = canonical_json_bytes(&space_val);
    let task = canonical_json_bytes(&task_val);
    let proposal = canonical_json_bytes(&proposal_val);
    let mut combined = Vec::new();
    // **Review P2 fix:** metadata (id/class/source/description) da digest'e dahil —
    // builder bu alanları değiştirirse manifest metadata ile drift olmadan digest
    // değişmeli. Domain-separator ile alanları ayır (collision yok).
    combined.extend_from_slice(b"faz8_p2_case_id\x00");
    combined.extend_from_slice(case.id.as_bytes());
    combined.extend_from_slice(b"\x00faz8_p2_case_class\x00");
    combined.extend_from_slice(format!("{:?}", case.class).as_bytes());
    combined.extend_from_slice(b"\x00faz8_p2_case_source\x00");
    combined.extend_from_slice(format!("{:?}", case.source).as_bytes());
    combined.extend_from_slice(b"\x00faz8_p2_case_description\x00");
    combined.extend_from_slice(case.description.as_bytes());
    combined.extend_from_slice(b"\x00faz8_p2_payload\x00");
    combined.extend_from_slice(&space);
    combined.extend_from_slice(&task);
    combined.extend_from_slice(&proposal);
    combined
}

/// JSON Value'yu canonical (sorted-key, deterministic) bytes'a çevir.
///
/// serde_json default `preserve_order` feature KAPALI olduğu için `Map` type
/// BTreeMap-backed'dir → key'ler sort edilir. Bu, HashMap iteration order
/// nondeterminism'ini kapatır.
fn canonical_json_bytes(value: &serde_json::Value) -> Vec<u8> {
    // Value zaten Map (BTreeMap-backed) → serialize sorted.
    // pretty=false (compact), deterministic.
    serde_json::to_vec(value).expect("Value → canonical bytes")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case 001: matching-single-node (subject/value parity, source divergence)
// ═══════════════════════════════════════════════════════════════════════════════

/// Task targets Node(1) via `Node(1)` predicate scope; proposal declares
/// `affected_nodes=[1]`. Node 1 pre-exists in space.
///
/// Subject set + axis value bits parity (V1/V2 aynı centroid). Source divergence INV-T4
/// (V1 uniform Scip override vs V2 engine axis defaults — Migration 2).
fn matching_single_node_001() -> CharacterizationCase {
    use osp_core::agent::DeltaProposal;
    use osp_core::space::{Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: tek node (id=1, Module).
    let mut space = Space::new();
    space.insert_node(Node {
        id: 1,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    // Task: Node(1) predicate scope, id=42.
    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Le,
        threshold: 0.5,
        scope: PredicateScope::Node(1),
        required_source: None,
        tolerance: 0.0,
    };
    let ps = PredicateSet {
        mode: PredicateMode::All,
        predicates: vec![WeightedPredicate {
            predicate,
            weight: None,
        }],
        preferred_vector: None,
    };
    let task = Task {
        id: 42,
        milestone_id: 0,
        label: "matching-single-node-001".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    // Proposal: affected_nodes=[1] (matches task scope). Structural delta olarak
    // node 1'e gelen yeni bir edge ekle (build_claim_from_proposal empty-proposal
    // check'ini geçmek için; subject node 1 space'te kaldığı için V1 affected
    // centroid ve V2 subject scope centroid aynı node üzerinden → parity).
    use osp_core::agent::NewEdgeSpec;
    use osp_core::space::EdgeKind;
    let proposal = DeltaProposal {
        new_edges: vec![NewEdgeSpec {
            from: 1,
            to: 2,
            kind: EdgeKind::Imports,
        }],
        affected_nodes: vec![1],
        ..Default::default()
    };

    CharacterizationCase {
        id: "matching-single-node-001".to_string(),
        class: CaseClass::MatchingScope,
        source: CaseSource::SyntheticAdversarial,
        description: "Task targets Node(1); proposal affected_nodes=[1]. Subject/value \
            parity but source divergence (INV-T4: V1 Scip override vs V2 TreeSitter axis)."
            .to_string(),
        space,
        task,
        proposal,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case 002: wide-affected-scope (KNOWN DIVERGENCE — production-reachable)
// ═══════════════════════════════════════════════════════════════════════════════

/// **Bilinen production-reachable divergence.** Task targets Node(1) via `Node(1)`
/// predicate scope; proposal declares `affected_nodes=[1, 2, 3]` (LLM geniş scope
/// bildirir — komşu coupling kaymasıyla).
///
/// - V1 subject = `{1, 2, 3}` (affected_nodes) → affected centroid 3 node üzerinden.
/// - V2 subject = `{1}` (task.predicate.scope) → subject centroid tek node üzerinden.
///
/// Bu fark **ontolojiktir** (subject authority: LLM-affected vs task-derived) ve
/// **production-reachable**. P2-1'in açılması için ontolojik karar gerekir.
fn wide_affected_scope_001() -> CharacterizationCase {
    use osp_core::agent::NewEdgeSpec;
    use osp_core::space::{EdgeKind, Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: node 1, 2, 3 mevcut (hepsi Module).
    let mut space = Space::new();
    for id in 1..=3u64 {
        space.insert_node(Node {
            id,
            kind: NodeKind::Module,
            mass: 1.0,
            ..Default::default()
        });
    }

    // Task: Node(1) predicate scope (dar — sadece node 1).
    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Le,
        threshold: 0.5,
        scope: PredicateScope::Node(1),
        required_source: None,
        tolerance: 0.0,
    };
    let ps = PredicateSet {
        mode: PredicateMode::All,
        predicates: vec![WeightedPredicate {
            predicate,
            weight: None,
        }],
        preferred_vector: None,
    };
    let task = Task {
        id: 42,
        milestone_id: 0,
        label: "wide-affected-scope-001".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    // Proposal: affected_nodes=[1,2,3] (geniş — LLM affected bildirir), structural
    // delta olarak 1→2 edge (empty-proposal check için).
    let proposal = DeltaProposal {
        new_edges: vec![NewEdgeSpec {
            from: 1,
            to: 2,
            kind: EdgeKind::Imports,
        }],
        affected_nodes: vec![1, 2, 3],
        ..Default::default()
    };

    CharacterizationCase {
        id: "wide-affected-scope-001".to_string(),
        class: CaseClass::WideAffectedScope,
        source: CaseSource::SyntheticAdversarial,
        description: "Task Node(1) scope; affected_nodes=[1,2,3]. KNOWN divergence: \
            V1 measures centroid over {1,2,3}, V2 over {1}."
            .to_string(),
        space,
        task,
        proposal,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case 003: removed-edge-external-source (divergence)
// ═══════════════════════════════════════════════════════════════════════════════

/// `removed_edges[*].from` task scope DIŞINDA bir node. V1 affected set'e bu node'u
/// dahil eder (navigator.rs affected_nodes + removed_edges.from); V2 subject scope
/// (task-derived) dahil ETMEZ. Divergence: V1 centroid etkilenir, V2 etkilenmez.
fn removed_edge_external_source_001() -> CharacterizationCase {
    use osp_core::agent::{EdgeRef, NewEdgeSpec};
    use osp_core::space::{EdgeKind, Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: node 1 (subject), node 9 (external — removed-edge source).
    let mut space = Space::new();
    space.insert_node(Node {
        id: 1,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });
    space.insert_node(Node {
        id: 9,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Le,
        threshold: 0.5,
        scope: PredicateScope::Node(1),
        required_source: None,
        tolerance: 0.0,
    };
    let ps = PredicateSet {
        mode: PredicateMode::All,
        predicates: vec![WeightedPredicate {
            predicate,
            weight: None,
        }],
        preferred_vector: None,
    };
    let task = Task {
        id: 42,
        milestone_id: 0,
        label: "removed-edge-external-001".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    // Proposal: affected_nodes=[1] (matching), ama removed_edges[0].from=9 (external).
    // V1 affected = {1, 9} (9 removed_edges.from'dan eklenir); V2 subject = {1}.
    let proposal = DeltaProposal {
        new_edges: vec![NewEdgeSpec {
            from: 1,
            to: 9,
            kind: EdgeKind::Imports,
        }],
        removed_edges: vec![EdgeRef {
            from: 9,
            to: 1,
            kind: EdgeKind::Imports,
        }],
        affected_nodes: vec![1],
        ..Default::default()
    };

    CharacterizationCase {
        id: "removed-edge-external-source-001".to_string(),
        class: CaseClass::RemovedEdgeExternalSource,
        source: CaseSource::SyntheticAdversarial,
        description: "Task Node(1); affected=[1] + removed_edge from=9 (external). \
            V1 folds 9 into affected set; V2 subject scope {1} excludes it."
            .to_string(),
        space,
        task,
        proposal,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case 004: delta-introduced-subject (V2 baseline Unavailable)
// ═══════════════════════════════════════════════════════════════════════════════

/// Task predicate scope node'ları tamamen delta ile introduced (space'te YOK).
/// - V2: `MeasurementBaseline::Unavailable { AllMembersIntroducedByDelta }` (typed).
/// - V1: `DefaultFallback` — subject base'de yok → `compute_raw_from_delta` empty
///   positions → `RawPosition::default()` (sıfır koordinat).
///
/// **Review tur 7-9:** Bu case **baseline epistemic availability divergence** kanıtlar
/// (V1 yokluk → sıfır, V2 typed UnavailableAllIntroduced). **Policy/decision divergence
/// KANITLAMAZ** — Case 4 `Completed` → `AcceptAsCompleted` (completion-first core
/// improved'a bakmaz). Policy etkisi SADECE `NotCompleted` + improvement-sensitive policy
/// dalında observable (P2-0B.8 fixture).
///
/// Bu case V2'nin "baseline yoksa progress kanıtlanamaz" semantiğini V1'in
/// "her zaman current_measured var" semantiğinden ayırır.
///
/// **Review P0-1 fix:** Task scope `Node(10000)` — `build_claim_from_proposal`/
/// `node_from_spec` (navigator.rs:312-318) `NewNodeSpec`'ten `id: 10_000 + index`
/// üretir. Önceki kod task scope `Node(1)` kullanıyordu ama delta node `10000`
/// oluyordu → subject node 1 ne base'de ne delta'da → SubjectScope hatası
/// "baseline unavailable" DEĞİL, kimlik uyuşmazlığı sonucuydu. Şimdi task scope
/// delta-produced id ile match ediyor → gerçek AllMembersIntroducedByDelta yolu.
fn delta_introduced_subject_001() -> CharacterizationCase {
    use osp_core::agent::NewNodeSpec;
    use osp_core::space::{Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: node 10 mevcut (delta-introduced değil, subject node 10 değil).
    // Subject node = 10000, delta ile introduced (node_from_spec 10_000 + 0 = 10000).
    let mut space = Space::new();
    space.insert_node(Node {
        id: 10,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Le,
        threshold: 0.5,
        // node_from_spec ilk NewNodeSpec için id=10000 üretir → subject ile match.
        scope: PredicateScope::Node(10_000),
        required_source: None,
        tolerance: 0.0,
    };
    let ps = PredicateSet {
        mode: PredicateMode::All,
        predicates: vec![WeightedPredicate {
            predicate,
            weight: None,
        }],
        preferred_vector: None,
    };
    let task = Task {
        id: 42,
        milestone_id: 0,
        label: "delta-introduced-subject-001".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    // Proposal: delta node 10000'i introduced; affected_nodes=[10000].
    let proposal = DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: NodeKind::Module,
            initial_mass: 1.0,
            connected_to: vec![],
        }],
        affected_nodes: vec![10_000],
        ..Default::default()
    };

    CharacterizationCase {
        id: "delta-introduced-subject-001".to_string(),
        class: CaseClass::DeltaIntroducedSubject,
        source: CaseSource::SyntheticAdversarial,
        description: "Subject node 10000 delta-introduced (node_from_spec id). \
            V2 baseline UnavailableAllIntroduced (typed); V1 DefaultFallback (empty \
            positions → RawPosition::default). Availability divergence (Completed → \
            AcceptAsCompleted parity; policy etkisi bu case'te observable DEĞİL — sibling \
            delta-introduced-subject-policy-001 divergence'ı gösterir)."
            .to_string(),
        space,
        task,
        proposal,
    }
}

/// `delta-introduced-subject-policy-001`: P2-0B.8 policy fixture — NotCompleted +
/// AcceptImprovement dalında **V1 legacy DefaultFallback baseline projection** ile
/// **V2 candidate fail-closed projection** arasında migration-relevant policy/decision
/// divergence üreten case.
///
/// `delta_introduced_subject_001`'den (Case 4) **dört değişiklikle** türetilir. Korunan
/// şey yapısal topoloji DEĞİL (edge geometrisi değişti — aşağıda), epistemik koşuldur:
/// subject node `10000` base space'te yok, delta ile tanıtılır → V1 `DefaultFallback`,
/// V2 `UnavailableAllIntroduced` baseline yoluna girer. Case 4'ün `Completed →
/// AcceptAsCompleted` (completion-first core, improved'a bakmaz) dalını **NotCompleted +
/// improvement-sensitive** dalına taşıyan dört değişiklik:
///
/// 1. **Predicate:** `Coupling >= 0.7` (measured coupling 0.5 → false → NotCompleted).
///    Case 4 `Coupling <= 0.5` idi → measured 0.0 true → Completed.
/// 2. **preferred_vector:** `(0.8, 0.5, 0.5, 0, 0)` — coupling/cohesion/instability
///    target'ı. Case 4 `None` idi → target=zero → loss_before=loss_after → improved imkansız.
/// 3. **TaskPolicy:** `AcceptImprovement` + `allow_progress_checkpoint: true` (g2c idiom).
///    Case 4 `StrictReject` + default idi → NotCompleted reject, improved'a bakmaz.
/// 4. **Edge geometrisi:** reciprocal Imports edges `10000→10` (Ce=1, `connected_to`) +
///    `10→10000` (Ca=1, `new_edges`). Case 4 `connected_to: vec![]` (edge yok) → measured
///    coupling 0.0. Reciprocal edge measured coupling 0.5 üretir ve instability 0.5
///    (Ce=1, Ca=1) `max_instability=0.85` hard-cap altında tutar (tek outgoing Ce=1/Ca=0 →
///    instability 1.0 → hard-cap ihlali → improved=false).
///
/// **Reciprocal Imports edge gerekliliği:** Sadece outgoing edge Ce=1/Ca=0 →
/// instability=1.0 → `max_instability=0.85` hard-cap ihlali → improved=false. Reciprocal
/// ile Ce=1, Ca=1 → instability=0.5 (hard-cap altında).
///
/// **Beklenen karar ayrışması:**
/// - V1: DefaultFallback zero baseline + target (0.8,0.5,0.5) → loss_before ≈ 1.068;
///   measured after (0.5,0.5,0.5) → loss_after = 0.3 → improved=true (loss drop 0.768 >
///   0.02, hard-cap'ler geçer) → `AcceptAsProgress` → `Lane(TrajectoryCheckpoint)` (INV-T8).
/// - V2: `project_v1_loss_before_compatibility_v2` Unavailable dalı → loss_before=loss_after
///   → improved=false → `Reject` → `NotApplied` (INV-T8), witness değerlendirilmez → `Evaluated`.
///
/// Bu fixture production-reachable structural topology üzerinde gelecekteki migration'ın
/// policy/decision divergence'ını karakterize eder; iki production implementation'ı
/// karşılaştırmaz.
fn delta_introduced_subject_policy_001() -> CharacterizationCase {
    use osp_core::agent::{NewEdgeSpec, NewNodeSpec};
    use osp_core::coords::RawPosition;
    use osp_core::space::{EdgeKind, Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateFailurePolicy, PredicateMode,
        PredicateScope, PredicateSet, TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: node 10 mevcut (Module — delta-introduced değil, subject node 10 değil).
    let mut space = Space::new();
    space.insert_node(Node {
        id: 10,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    // Predicate: Coupling >= 0.7 — measured coupling 0.5 → false → NotCompleted.
    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Ge,
        threshold: 0.7,
        // node_from_spec ilk NewNodeSpec için id=10000 üretir → subject ile match.
        scope: PredicateScope::Node(10_000),
        required_source: None,
        tolerance: 0.0,
    };
    let ps = PredicateSet {
        mode: PredicateMode::All,
        predicates: vec![WeightedPredicate {
            predicate,
            weight: None,
        }],
        // Target vector: coupling 0.8, cohesion 0.5, instability 0.5 (3 eksen trajectory_loss).
        preferred_vector: Some(RawPosition {
            x: 0.8,
            y: 0.5,
            z: 0.5,
            w: 0.0,
            v: 0.0,
        }),
    };
    let task = Task {
        id: 43, // Case 4 task id 42 — çakışma yok.
        milestone_id: 0,
        label: "delta-introduced-subject-policy-001".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy {
            predicate_failure_policy: PredicateFailurePolicy::AcceptImprovement,
            allow_progress_checkpoint: true, // ZORUNLU — improved değerlendirilmesi için.
            min_improvement_delta: 0.02,     // default.
            ..Default::default()
        },
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    // Reciprocal Imports edges → Ce=1 (outgoing 10000→10) + Ca=1 (incoming 10→10000)
    // → instability = Ce/(Ce+Ca) = 0.5 (hard-cap 0.85 altında).
    let proposal = DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: NodeKind::Module,
            initial_mass: 1.0,
            // 10000 → 10 (outgoing Imports) → Ce=1.
            connected_to: vec![(10, EdgeKind::Imports)],
        }],
        new_edges: vec![NewEdgeSpec {
            // 10 → 10000 (incoming Imports) → Ca=1.
            from: 10,
            to: 10_000,
            kind: EdgeKind::Imports,
        }],
        affected_nodes: vec![10_000],
        ..Default::default()
    };

    CharacterizationCase {
        id: "delta-introduced-subject-policy-001".to_string(),
        class: CaseClass::DeltaIntroducedSubject,
        source: CaseSource::SyntheticAdversarial,
        description: "Production-reachable structural topology üzerinde V1 legacy \
            DefaultFallback projection ile V2 candidate fail-closed projection arasında \
            migration-relevant policy/decision divergence. NotCompleted + \
            AcceptImprovement: V1 → loss_before > loss_after → improved → \
            AcceptAsProgress (TrajectoryCheckpoint, Held); V2 → loss_before=loss_after \
            → improved=false → Reject (NotApplied, Evaluated). Migration 3 (b) hipotezi \
            kanıtlandı."
            .to_string(),
        space,
        task,
        proposal,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Stage-aware observation model (P2-0B.4, plan Tur 5 P1-2)
// ═══════════════════════════════════════════════════════════════════════════════

/// Bir V1 veya V2-candidate evaluation'ın tam gözlemi.
///
/// Measurement ve pipeline gözlemleri ayrılır (plan Tur 5 P1-2): erken duruşlar
/// (SyntaxViolation/TaskValidation/MeasurementError) ve tam commit sonuçları aynı
/// modelde temsil edilebilir. Integration test uydurma değer ÜRETMEZ —
/// `q5_theta_bits` yalnız `Q5Observation::Rejected` içinde (failure observable),
/// successful theta ayrı engine-unit (`Q5ThetaCharacterization`).
#[derive(Debug, Clone)]
pub struct CharacterizationObservation {
    pub measurement: MeasurementObservation,
    pub pipeline: PipelineObservation,
    /// `commit_task_claim`'e gerçekten geçirilen decision-input scalar'ları
    /// (PR #91 review P1). Early-return/StoppedBeforeCommit'te `None` (commit'e
    /// ulaşılmadı). V2 candidate fail-closed projection (`loss_before == loss_after`)
    /// exact frozen evidence olarak pinlenir — sadece decision sonucu DEĞİL, projection
    /// zincirinin tamamı.
    pub decision_input: Option<DecisionInputObservation>,
}

/// `commit_task_claim`'e geçirilen gerçek decision-input scalar'ları.
///
/// **PR #91 review P1:** V2 candidate fail-closed projection'ın (`loss_before ==
/// loss_after`) frozen evidence olması için, helper'ın `commit_task_claim`'e geçirilen
/// `loss_before` değeri observation'a taşınır. Aksi halde projection helper gelecekte
/// değişse (`Unavailable → 0.0` veya `→ loss_after - 0.01`) test decision sonucu (`Reject`)
/// aynı kaldığı için yeşil kalırdı — "equality fail-closed projection" iddiası çürür.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecisionInputObservation {
    /// `TaskCommitInput.loss_before` — `commit_task_claim`'e geçirilen değer.
    pub loss_before_bits: u64,
    /// `trajectory_loss(measured_after, target)` — `assess_improvement_v1`'in
    /// karşılaştırdığı loss_after.
    pub loss_after_bits: u64,
}

/// Measurement producer sonucu (V1: compute_raw_from_delta infallible;
/// V2-candidate: measure_task_delta fallible).
///
/// **Review P0-2 fix:** Artık subject/value-bits/sources her iki path için de taşır
/// (V1 subject = affected_nodes, V2 subject = task scope). Exact parity assertion'ları
/// için gerekli zenginleştirme.
#[derive(Debug, Clone)]
pub enum MeasurementObservation {
    /// Measurement başarıyla üretildi.
    Produced {
        /// Subject node set (V1: affected_nodes ∪ removed_edges.from; V2: task.predicate.scope).
        /// Exact parity assertion'ları için gerçek set (approximation DEĞİL).
        subject: Vec<u64>,
        /// After-state measured position (V1: compute_raw→provenanced_from_raw;
        /// V2-candidate: measurement.after()).
        measured_after: osp_core::trajectory::ProvenancedRawPosition,
        /// 5-axis value bits — exact `to_bits()` parity için (review P0-2).
        values_bits: [u64; 5],
        /// 5-axis sources — exact source parity için (review P0-2, INV-T4).
        sources: [osp_core::coords::MetricSource; 5],
        /// **Review tur 7 P0:** Anlamlı baseline observation — V1 LegacyComputed
        /// (affected centroid veya DefaultFallback) / V2 Available/Unavailable.
        /// Önceki `baseline_kind: Option<BaselineKind>` V1 için her zaman None
        /// üretiyordu ve "baseline yok" anlamına gelmiyordu; gerçekte V1 yokluğu
        /// sıfır koordinata çeviriyordu (LegacyDefaultFallback).
        baseline: BaselineObservation,
    },
    /// V2-candidate measurement producer hatası (V1 infallible → bu varyant V1'de yok).
    Failed { error: MeasurementFailureClass },
    /// V1 path measurement'ı tracking yapmaz (compute_raw_from_delta infallible).
    ///
    /// **Not:** V1 harness `Produced { subject, baseline: LegacyComputed{...} }`
    /// üretir — bu varyant hiçbir builder tarafından üretilmez. Future-use: V1'in
    /// "measurement tracking yok" durumunu explicit temsil etmek için ayrılabilir.
    /// Şu an sadece matcher'larda referans edilir.
    NotAttempted,
}

/// MeasurementBaseline::Available|Unavailable lossy projection (rust enum import etmek
/// yerine minimal representasyon — characterization raporlama için).
#[derive(Debug, Clone, PartialEq)]
pub enum BaselineKind {
    Available,
    UnavailableAllIntroduced,
    /// **Not:** Hiçbir V2-candidate case builder tarafından üretilmedi (PartialNewSubject
    /// baseline gerektiren case henüz yok). Matcher'da referans edilir — future-use
    /// (P2-0B kalan: mixed_per_axis_sources veya subgraph-introduced case eklendiğinde).
    UnavailablePartialNew,
}

/// **Review tur 7 P0:** Anlamlı baseline observation — representation/availability/
/// value-source ayrımı. V1 `compute_raw_from_delta(&[], ...)` empty positions durumunda
/// `RawPosition::default()` döndürür (engine.rs:2329-2330) — delta-introduced subject
/// için V1 "current_measured her zaman var" DEĞİL, **legacy default fallback** üretir.
/// Bu, "typed/untyped representation" değil, **epistemik availability yorumu** farkı:
/// V1 yokluğu sıfır koordinaya çeviriyor, V2 typed `UnavailableAllIntroduced` koruyor.
#[derive(Debug, Clone, PartialEq)]
pub enum BaselineObservation {
    /// V1 legacy baseline — `compute_raw_from_delta` pre-delta affected centroid.
    /// `LegacyDefaultFallback`: subject node'ları base space'te yok → empty positions
    /// → `RawPosition::default()` (sıfır). OSP "bilinmeyeni ölçülmüş gibi sunmama"
    /// çizgisine aykırı — explicit pinlenmeli.
    LegacyComputed {
        values_bits: [u64; 5],
        sources: [osp_core::coords::MetricSource; 5],
        /// `trajectory_loss(baseline, target)` bit'leri.
        loss_bits: u64,
        /// Empty positions → RawPosition::default() mı, yoksa gerçek centroid mi?
        derivation: LegacyBaselineDerivation,
    },
    /// V2 Available baseline — `MeasurementBaseline::Available(before)`; before centroid
    /// value/source/loss ile birlikte.
    Available {
        values_bits: [u64; 5],
        sources: [osp_core::coords::MetricSource; 5],
        loss_bits: u64,
    },
    /// V2 Unavailable baseline — typed; geçmiş baseline yok.
    Unavailable(BaselineKind),
}

/// V1 legacy baseline türetme yolu (review tur 7).
#[derive(Debug, Clone, PartialEq)]
pub enum LegacyBaselineDerivation {
    /// Subject node'ları base space'te mevcut → gerçek pre-delta centroid.
    AffectedCentroid,
    /// Subject node'ları base space'te YOK → empty positions → `RawPosition::default()`.
    /// Case 4 (delta-introduced subject) bu yola girer.
    DefaultFallback,
}

/// 5-axis value bits (coupling/cohesion/instability/entropy/witness_depth sırasıyla).
/// Exact parity assertion'ları için `to_bits()` — NaN/signed-zero/rounding farkları
/// dahil tüm bit-level farkları yakalar (review P0-2).
pub fn axis_value_bits(pos: &osp_core::trajectory::ProvenancedRawPosition) -> [u64; 5] {
    [
        pos.coupling.value.to_bits(),
        pos.cohesion.value.to_bits(),
        pos.instability.value.to_bits(),
        pos.entropy.value.to_bits(),
        pos.witness_depth.value.to_bits(),
    ]
}

/// 5-axis sources (coupling/cohesion/instability/entropy/witness_depth sırasıyla).
/// Exact source parity — INV-T4 required_source predicate'leri için kritik (review P0-2).
pub fn axis_sources(
    pos: &osp_core::trajectory::ProvenancedRawPosition,
) -> [osp_core::coords::MetricSource; 5] {
    [
        pos.coupling.source,
        pos.cohesion.source,
        pos.instability.source,
        pos.entropy.source,
        pos.witness_depth.source,
    ]
}

/// MeasurementError varyantlarının characterization sınıflandırması
/// (tam enum yerine karar-odaklı kategoriler).
#[derive(Debug, Clone, PartialEq)]
pub enum MeasurementFailureClass {
    Binding,
    Revision,
    SubjectScope,
    Coordinate,
    Digest,
    Other,
}

impl MeasurementFailureClass {
    pub fn from_measurement_error(err: &osp_core::measurement::MeasurementError) -> Self {
        use osp_core::measurement::MeasurementError;
        match err {
            MeasurementError::ClaimNotTaskBound { .. }
            | MeasurementError::TaskBindingMismatch { .. } => Self::Binding,
            MeasurementError::RevisionComputationFailed { .. }
            | MeasurementError::RevisionMismatch { .. } => Self::Revision,
            MeasurementError::HeterogeneousPredicateScopes { .. }
            | MeasurementError::EmptySubjectScope
            | MeasurementError::SubjectScopeResolutionFailed(_)
            | MeasurementError::SubjectMemberUnresolvable { .. }
            | MeasurementError::SubjectMemberMissingAfterDelta { .. }
            | MeasurementError::SubjectScopeHintMismatch { .. } => Self::SubjectScope,
            MeasurementError::CoordinateMeasurement(_)
            | MeasurementError::InvalidSubjectMass { .. }
            | MeasurementError::InvalidTotalSubjectMass { .. } => Self::Coordinate,
            MeasurementError::MeasurementContext(_)
            | MeasurementError::MeasurementContextDrift { .. }
            | MeasurementError::MeasurementContextDigestMismatch
            | MeasurementError::Digest(_) => Self::Digest,
        }
    }

    /// Measurement failure → (PipelineStage, PipelineFailureClass) mapping.
    ///
    /// **Review P1-3 fix:** V2-candidate early-return'de `stage: Other` yerine gerçek
    /// stage. Measurement producer hatası commit pipeline'ının measurement-binding
    /// aşamasında durur — ama spesifik MeasurementError variant'a göre alt-stage
    /// (Binding/TaskValidation/Internal) verir.
    pub fn pipeline_stage_and_class(&self) -> (PipelineStage, PipelineFailureClass) {
        match self {
            Self::Binding => (PipelineStage::TaskBinding, PipelineFailureClass::Binding),
            Self::Revision => (
                PipelineStage::MeasurementBinding,
                PipelineFailureClass::Internal,
            ),
            Self::SubjectScope => (
                PipelineStage::TaskValidation,
                PipelineFailureClass::TaskValidation,
            ),
            Self::Coordinate => (
                PipelineStage::MeasurementBinding,
                PipelineFailureClass::Internal,
            ),
            Self::Digest => (
                PipelineStage::MeasurementBinding,
                PipelineFailureClass::Internal,
            ),
            Self::Other => (
                PipelineStage::MeasurementBinding,
                PipelineFailureClass::Other,
            ),
        }
    }
}

/// Pipeline (commit_task_claim) sonucu — stage-aware erken duruşlar dahil.
///
/// **Review tur 6/7:** `PartialEq` derive + Held/Rejected outcome authoritative okuma.
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineObservation {
    /// commit_task_claim tam çalıştı (Evaluated/Held/Rejected dahil).
    ///
    /// **Review tur 6 P0-1:** Held/Rejected `AuthorizationContext` taşır (engine.rs:1133);
    /// `authorization.outcome` gerçek `AttemptOutcome` (predicate_completion +
    /// mutation_decision) + `authorization.apply_target` verir. Önceki "Held fabrication"
    /// yanlıştı — Held/Rejected'da outcome `None` DEĞİL, authoritative engine çıktısı.
    /// `Option<>` sadece alignment/coverage eksikliği için (bazı pipeline stage'larda
    /// outcome henüz computed değil); Held/Rejected için her zaman `Some(...)`.
    CommitReached {
        q5: Q5Observation,
        /// Evaluated: TaskCommitResult.outcome; Held/Rejected: authorization.outcome.
        predicate_completion: Option<osp_core::trajectory::PredicateCompletion>,
        /// Evaluated: TaskCommitResult.outcome; Held/Rejected: authorization.outcome.
        mutation_decision: Option<osp_core::trajectory::MutationDecision>,
        /// Evaluated: TaskCommitResult.apply_target; Held/Rejected: authorization.apply_target.
        apply_target: Option<osp_core::trajectory::ApplyTarget>,
        witness_reachability: WitnessReachability,
    },
    /// commit_task_claim Q5/commit'e ulaşmadan error verdi (early stop).
    StoppedBeforeCommit {
        stage: PipelineStage,
        error: PipelineFailureClass,
    },
}

/// Q5 vision gözlemi — successful theta public API'den AÇILMAZ (TaskCommitResult'ta yok),
/// yalnız failure'da `VisionViolation.theta` observable. Successful theta engine-unit
/// (`Q5ThetaCharacterization`) karakterize eder.
#[derive(Debug, Clone, PartialEq)]
pub enum Q5Observation {
    /// Q5 çalışmadı (commit stopped before Q5).
    NotReached,
    /// Q5 passed (theta <= bound). Exact theta bits bilinmez — engine-unit gerek.
    Passed,
    /// Q5 rejected (theta > bound). theta observable via VisionViolation.
    Rejected { theta_bits: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineStage {
    Q4Syntax,
    TaskBinding,
    TaskValidation,
    Vision,
    PredicateGate,
    MeasurementBinding,
    Authorization,
    Witness,
    Persistence,
    Internal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineFailureClass {
    Syntax,
    Binding,
    TaskValidation,
    Vision,
    Rule,
    Measurement,
    Authorization,
    Persistence,
    Internal,
    Other,
}

impl PipelineFailureClass {
    pub fn from_engine_commit_error(err: &osp_core::engine::EngineCommitError) -> Self {
        use osp_core::engine::EngineCommitError;
        match err {
            EngineCommitError::SyntaxViolation { .. } => Self::Syntax,
            EngineCommitError::PermissionDenied(_) => Self::Binding,
            EngineCommitError::TaskValidation(_) => Self::TaskValidation,
            EngineCommitError::VisionViolation { .. }
            | EngineCommitError::VisionContextInvalid(_) => Self::Vision,
            EngineCommitError::RuleViolation { .. } => Self::Rule,
            EngineCommitError::MeasurementBindingMismatch(_)
            | EngineCommitError::MeasurementBindingFailed(_)
            | EngineCommitError::MeasurementBindingVerification(_) => Self::Measurement,
            EngineCommitError::AuthorizationContextFailed(_) => Self::Authorization,
            EngineCommitError::NoPersistence | EngineCommitError::Persistence(_) => {
                Self::Persistence
            }
            EngineCommitError::Internal(_) => Self::Internal,
            EngineCommitError::InvalidWitnessEvidence(_) => Self::Other,
        }
    }
}

/// Witness disposition classification (Held/Rejected/Evaluated).
#[derive(Debug, Clone, PartialEq)]
pub enum WitnessReachability {
    Evaluated,
    Held,
    Rejected,
    NotReached,
}

impl WitnessReachability {
    pub fn from_commit_result(result: &osp_core::engine::EngineCommitResult) -> Self {
        use osp_core::engine::EngineCommitResult;
        match result {
            EngineCommitResult::Evaluated { .. } => Self::Evaluated,
            EngineCommitResult::Held { .. } => Self::Held,
            EngineCommitResult::Rejected { .. } => Self::Rejected,
        }
    }
}

/// Mutation decision observation tri-state (review tur 5/6/7).
///
/// `NotReached` (gate çalışmadı) ile `ReachedButUnsurfaced` (gate çalıştı ama sonuç
/// observation'a taşınmıyor) farklı mimari problemlerdir.
///
/// **Review tur 6 P0-1:** Mevcut production'da `EngineCommitResult`'ın tüm varyantları
/// (Evaluated/Held/Rejected) `AuthorizationContext` üzerinden gerçek outcome taşır.
/// Yani `ReachedButUnsurfaced` şu an **production-reachable değil** — Held/Rejected
/// `authorization.outcome` observable olduğu için `Observed(...)` üretilir. Varyant
/// gelecekte farklı bir API şekli (outcome taşımayan sonuç) için tutulur; Held/Rejected'a
/// bağlanmadan generic "şu an production-reachable değil" olarak açıklanır.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationDecisionObservation {
    /// PredicateGate'e ulaşıldı → gerçek MutationDecision observable (Evaluated'ın
    /// TaskCommitResult.outcome veya Held/Rejected'ın authorization.outcome).
    Observed(osp_core::trajectory::MutationDecision),
    /// PredicateGate'e ulaşıldı ama outcome observation'a taşınmıyor. **Şu an
    /// production-reachable değil** — tüm EngineCommitResult varyantları outcome taşır.
    /// Future API shapes için tutulur.
    ReachedButUnsurfaced,
    /// PredicateGate'e ulaşılmadı (Q5 Vision veya erken stage'de durdu).
    NotReached,
}

impl MutationDecisionObservation {
    pub fn from_observations(pipeline: &PipelineObservation) -> Self {
        match pipeline {
            PipelineObservation::CommitReached {
                mutation_decision: Some(md),
                ..
            } => Self::Observed(md.clone()),
            PipelineObservation::CommitReached {
                mutation_decision: None,
                ..
            } => Self::ReachedButUnsurfaced,
            PipelineObservation::StoppedBeforeCommit { .. } => Self::NotReached,
        }
    }
}

/// Pipeline stage'ı EngineCommitError'dan çıkar (hangi aşamada durduğunu söyler).
///
/// **Review P1-2 fix:** `PipelineFailureClass::from_engine_commit_error` ile tutarlı
/// stage↔class correspondence. Önceki kod measurement binding → Measurement class
/// ama Other stage yapıyordu (self-contradictory StoppedBeforeCommit). Artık her
/// EngineCommitError variant tek bir (stage, class) çiftine map eder.
impl PipelineStage {
    pub fn from_engine_commit_error(err: &osp_core::engine::EngineCommitError) -> Self {
        use osp_core::engine::EngineCommitError;
        match err {
            EngineCommitError::SyntaxViolation { .. } => Self::Q4Syntax,
            EngineCommitError::PermissionDenied(_) => Self::TaskBinding,
            EngineCommitError::TaskValidation(_) => Self::TaskValidation,
            EngineCommitError::VisionViolation { .. }
            | EngineCommitError::VisionContextInvalid(_) => Self::Vision,
            EngineCommitError::RuleViolation { .. } => Self::PredicateGate,
            EngineCommitError::MeasurementBindingMismatch(_)
            | EngineCommitError::MeasurementBindingFailed(_)
            | EngineCommitError::MeasurementBindingVerification(_) => Self::MeasurementBinding,
            EngineCommitError::AuthorizationContextFailed(_) => Self::Authorization,
            EngineCommitError::NoPersistence | EngineCommitError::Persistence(_) => {
                Self::Persistence
            }
            EngineCommitError::Internal(_) => Self::Internal,
            EngineCommitError::InvalidWitnessEvidence(_) => Self::Witness,
        }
    }
}

/// Q5 exact theta bits (engine-unit'ten — integration public API'den alamaz).
#[derive(Debug, Clone)]
/// Q5 exact theta bits (engine-unit'ten — integration public API'den alamaz).
///
/// **TODO (P2-0B.8 kalan iş):** Henüz hiçbir engine-unit test bu struct'ı üretmiyor.
/// P2-0B.8 (Q5 exact theta engine-unit characterization) tamamlanana kadar dead.
/// Case 2/3 Q5 Vision'da durduğu için PredicateGate decision-drift ölçülemedi;
/// non-default `computed_raw` ile Q5'i geçen case'ler gerekiyor.
pub struct Q5ThetaCharacterization {
    pub case_id: String,
    pub v1_theta_bits: u64,
    pub v2_candidate_theta_bits: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// V1 vs V2-candidate harness (P2-0B.5)
// ═══════════════════════════════════════════════════════════════════════════════

/// Standart measurement engine (engine.rs `make_measurement_engine` mirror).
pub fn make_characterization_engine() -> osp_core::engine::SpaceEngine {
    engine_from_space(osp_core::space::Space::new())
}

/// Engine'i verilen space ile kur (V1/V2 ayrı engine — state izolasyonu).
///
/// `space_mut` `#[cfg(test)]` crate-internal olduğu için integration test erişemez;
/// space'i constructor'da veriyoruz (V1 ve V2 candidate ayrı engine, aynı başlangıç
/// space clone ile).
pub fn engine_from_space(space: osp_core::space::Space) -> osp_core::engine::SpaceEngine {
    use osp_core::axes::{CohesionAxis, EntropyAxis, WitnessDepthAxis};
    use osp_core::coords::CoordinateSystem;
    use osp_core::engine::{EngineConfig, SpaceEngine};
    use osp_core::vision::VisionVector;
    let cs = CoordinateSystem::default_raw_five(
        osp_core::coords::MetricSource::TreeSitter,
        CohesionAxis::try_with_observed_source(osp_core::coords::MetricSource::Scip).unwrap(),
        EntropyAxis::from_commit_entropy(6.5),
        WitnessDepthAxis::from_witness(0.5, 3),
    )
    .unwrap();
    SpaceEngine::new(
        space,
        cs,
        VisionVector::new(osp_core::coords::RawPosition::default()),
        EngineConfig::default_calibrated(),
    )
}

/// Engine'i case'in space'i ile kur (V1/V2 ayrı engine — state izolasyonu).
pub fn engine_with_case_space(case: &CharacterizationCase) -> osp_core::engine::SpaceEngine {
    engine_from_space(case.space.clone())
}

/// V1 evaluation: compute_raw_from_delta → build Claim → current_measured/loss_before
/// → commit_task_claim. compute_raw_from_delta infallible → measurement NotAttempted
/// (V1 measurement tracking yapmaz; measured = provenanced_from_raw(computed_raw)).
///
/// **İsimlendirme:** "V1 production path" — bu mevcut navigator/MCP yolu.
///
/// **Review P0-2 fix:** `loss_before` navigator.rs:618/879'daki gibi `current_measured`
/// (pre-delta engine space centroid) üzerinden hesaplanır — `measured` (post-delta after)
/// üzerinden DEĞİL. Önceki kod `trajectory_loss(&measured, &target)` ile loss_before =
/// loss_after yapıyordu; bu sistematik improved=false üretirdi ve V1/V2 baseline
/// karşılaştırmasını bozardı.
pub fn evaluate_v1_case(
    engine: &mut osp_core::engine::SpaceEngine,
    case: &CharacterizationCase,
) -> CharacterizationObservation {
    use osp_core::navigator::{build_claim_from_proposal, provenanced_from_raw};
    use osp_core::trajectory::{InMemoryTaskRegistry, TaskResolver};
    use osp_core::witness::WitnessSet;

    // affected = proposal.affected_nodes ∪ removed_edges.from (navigator.rs:810-815 mirror).
    let mut affected: Vec<u64> = case.proposal.affected_nodes.clone();
    for er in &case.proposal.removed_edges {
        if !affected.contains(&er.from) {
            affected.push(er.from);
        }
    }

    // **Review P1-6 fix:** Önce probe claim üret (placeholder computed_raw), sonra
    // claim'in structural delta'sını compute_raw_from_delta için kullan. Önceki kod
    // node/edge mapping'i 3. kez inline duplicate ediyordu (navigator + local helper
    // + V1 harness). Artık tek mapping kaynağı: build_claim_from_proposal.
    let probe_claim = build_claim_from_proposal(
        &case.proposal,
        osp_core::coords::RawPosition::default(),
        case.task.id,
        100,
        1,
    )
    .expect("V1 probe claim build should succeed for characterization case");

    // V1: compute_raw_from_delta (infallible) — post-delta hypothetical centroid.
    // claim'in structural delta'sını kullan (mapping duplicate YOK).
    let computed_raw = engine.compute_raw_from_delta(
        &probe_claim.delta_nodes,
        &probe_claim.delta_edges,
        &probe_claim.removed_edges,
        &affected,
    );

    // **P0-2 fix:** loss_before için current_measured (pre-delta) — navigator.rs:618
    // `trajectory_loss(&self.current_measured, &self.target_vector)` mirror. Pre-delta
    // engine space üzerinden affected centroid hesapla (delta uygulamadan).
    let current_measured_raw = engine.compute_raw_from_delta(&[], &[], &[], &affected);
    let current_measured =
        provenanced_from_raw(current_measured_raw, osp_core::coords::MetricSource::Scip);

    // **Review tur 7 P0:** V1 baseline derivation — affected node'ları base space'te
    // var mı? Yoksa compute_raw_from_delta empty positions → RawPosition::default()
    // (engine.rs:2329-2330). DefaultFallback OSP "bilinmeyeni ölçülmüş gibi sunmama"
    // çizgisine aykırı — explicit pinlenmeli.
    let baseline_derivation = {
        let any_present = affected
            .iter()
            .any(|id| engine.space().nodes.contains_key(id));
        if any_present {
            LegacyBaselineDerivation::AffectedCentroid
        } else {
            LegacyBaselineDerivation::DefaultFallback
        }
    };
    let target = case
        .task
        .target_predicate_set
        .preferred_vector
        .unwrap_or_default();
    let baseline_loss_bits =
        osp_core::trajectory::trajectory_loss(&current_measured, &target).to_bits();
    let baseline_observation = BaselineObservation::LegacyComputed {
        values_bits: axis_value_bits(&current_measured),
        sources: axis_sources(&current_measured),
        loss_bits: baseline_loss_bits,
        derivation: baseline_derivation,
    };

    // Final claim: computed_raw = compute_raw_from_delta sonucu (V1 production path).
    let claim = build_claim_from_proposal(&case.proposal, computed_raw, case.task.id, 100, 1)
        .expect("V1 final claim build should succeed for characterization case");
    let measured = provenanced_from_raw(claim.computed_raw, osp_core::coords::MetricSource::Scip);
    // **P0-2 fix:** loss_before current_measured (pre-delta) üzerinden — measured DEĞİL.
    let loss_before = osp_core::trajectory::trajectory_loss(&current_measured, &target);

    // Registry + commit.
    let mut registry = InMemoryTaskRegistry::new();
    registry.insert(case.task.clone());
    let omega = WitnessSet::new(vec![]);

    let result = engine.commit_task_claim(osp_core::engine::TaskCommitInput {
        claim: &claim,
        omega: &omega,
        task_resolver: &registry as &dyn TaskResolver,
        target,
        loss_before,
        measured: measured.clone(),
    });

    // **PR #91 review P1:** commit_task_claim'e geçirilen gerçek decision-input scalar'ları.
    // V1 loss_before = trajectory_loss(current_measured, target) (yukarıda baseline loss_bits
    // ile aynı kaynak). loss_after = trajectory_loss(measured_after, target).
    let loss_after = osp_core::trajectory::trajectory_loss(&measured, &target);
    let decision_input = Some(DecisionInputObservation {
        loss_before_bits: loss_before.to_bits(),
        loss_after_bits: loss_after.to_bits(),
    });

    let measurement = MeasurementObservation::Produced {
        // V1 subject = affected_nodes ∪ removed_edges.from (navigator.rs:810-815).
        subject: affected.clone(),
        measured_after: measured.clone(),
        // **Review P0-2:** 5-axis value bits + sources — exact parity için.
        // V1 provenanced_from_raw(..., Scip) tüm axis'lere uniform Scip verir.
        values_bits: axis_value_bits(&measured),
        sources: axis_sources(&measured),
        // **Review tur 7 P0:** V1 baseline observation — LegacyComputed (AffectedCentroid
        // veya DefaultFallback). Önceki `baseline_kind: None` V1 "current_measured her
        // zaman var" diyordu ama delta-introduced subject için yanlıştı.
        baseline: baseline_observation,
    };

    let pipeline = match result {
        Ok(commit_result) => finalize_pipeline_observation_commit_reached(&commit_result),
        Err(e) => PipelineObservation::StoppedBeforeCommit {
            stage: PipelineStage::from_engine_commit_error(&e),
            error: PipelineFailureClass::from_engine_commit_error(&e),
        },
    };

    CharacterizationObservation {
        measurement,
        pipeline,
        decision_input,
    }
}

/// V2-candidate evaluation: probe Claim → measure_task_delta → final Claim
/// (measurement.after().to_raw()) → V1 compatibility projection → commit_task_claim.
///
/// **İsimlendirme:** "V2 **candidate projection**" — production V2 consumer henüz yok
/// (commit_task_claim hala V1). Bu harness, P2-1 implementation'ının üreteceği
/// gözlemlenebilir davranışı characterize eder.
pub fn evaluate_v2_candidate_case(
    engine: &mut osp_core::engine::SpaceEngine,
    case: &CharacterizationCase,
) -> CharacterizationObservation {
    use osp_core::navigator::build_claim_from_proposal;
    use osp_core::trajectory::{InMemoryTaskRegistry, TaskResolver};
    use osp_core::witness::WitnessSet;

    // Probe Claim: placeholder computed_raw (RawPosition::default()).
    let probe_claim = build_claim_from_proposal(
        &case.proposal,
        osp_core::coords::RawPosition::default(),
        case.task.id,
        100,
        1,
    )
    .expect("probe claim build should succeed for characterization case");
    let probe_bound = osp_core::trajectory::TaskBoundClaim {
        claim: &probe_claim,
        task: &case.task,
    };

    let measurement_result = {
        let revision = engine
            .current_space_view_revision()
            .expect("revision computation should not fail in test engine");
        engine.measure_task_delta(&probe_bound, &revision, None)
    };

    // Measurement başarısız olursa early return (commit'e ulaşılamaz).
    let token = match measurement_result {
        Ok(t) => t,
        Err(ref e) => {
            let error = MeasurementFailureClass::from_measurement_error(e);
            // **Review P1-3 fix:** gerçek (stage, class) — önceden stage: Other hardcode.
            let (stage, class) = error.pipeline_stage_and_class();
            return CharacterizationObservation {
                measurement: MeasurementObservation::Failed { error },
                pipeline: PipelineObservation::StoppedBeforeCommit {
                    stage,
                    error: class,
                },
                decision_input: None,
            };
        }
    };

    // Measurement başarılı — observation üret + commit için değerleri çıkar.
    let measured_for_commit = token.after().clone();
    let computed_raw_for_final_claim = token.after().to_raw();
    let target = case
        .task
        .target_predicate_set
        .preferred_vector
        .unwrap_or_default();
    // **Review tur 7 P0:** V2 baseline observation — Available (before centroid
    // value/source/loss) veya Unavailable (typed reason).
    let baseline_observation = match token.before() {
        osp_core::measurement::MeasurementBaseline::Available(before) => {
            let before_measured = before.clone();
            let loss_bits =
                osp_core::trajectory::trajectory_loss(&before_measured, &target).to_bits();
            BaselineObservation::Available {
                values_bits: axis_value_bits(&before_measured),
                sources: axis_sources(&before_measured),
                loss_bits,
            }
        }
        osp_core::measurement::MeasurementBaseline::Unavailable { reason } => {
            use osp_core::measurement::BaselineUnavailableReason;
            let kind = match reason {
                BaselineUnavailableReason::AllMembersIntroducedByDelta { .. } => {
                    BaselineKind::UnavailableAllIntroduced
                }
                BaselineUnavailableReason::PartialNewSubject { .. } => {
                    BaselineKind::UnavailablePartialNew
                }
            };
            BaselineObservation::Unavailable(kind)
        }
    };
    let measurement = MeasurementObservation::Produced {
        // **Review P1-7 note + P0-2 fix:** `subject` artık Vec<u64> (Option değil).
        // Harness-side approximation olduğu korunur (doc yukarıda), ama exact parity
        // assertion'ları için gerçek task scope set taşınır. Sınırlamalar: heterogeneous
        // scope'lar engine'de HeterogeneousPredicateScopes hatası verirken burada
        // sessizce flatten edilir; Module scope vec![] döner ama engine SubjectScopeResolutionFailed
        // üretir. Bu case'lerde measurement Failed olur, Produced'a hiç gelinmez.
        subject: case
            .task
            .target_predicate_set
            .predicates
            .iter()
            .flat_map(|wp| match &wp.predicate.scope {
                osp_core::trajectory::PredicateScope::Node(n) => vec![*n],
                osp_core::trajectory::PredicateScope::Subgraph(ns) => ns.clone(),
                osp_core::trajectory::PredicateScope::Module(_) => vec![],
            })
            .collect(),
        measured_after: measured_for_commit.clone(),
        // **Review P0-2:** 5-axis value bits + sources — exact parity için.
        values_bits: axis_value_bits(&measured_for_commit),
        sources: axis_sources(&measured_for_commit),
        baseline: baseline_observation,
    };

    // Final Claim: computed_raw = measurement.after().to_raw().
    let final_claim = build_claim_from_proposal(
        &case.proposal,
        computed_raw_for_final_claim,
        case.task.id,
        100,
        1,
    )
    .expect("final claim build should succeed for characterization case");

    // V1 compatibility projection: loss_before measurement token'ından türe.
    let target = case
        .task
        .target_predicate_set
        .preferred_vector
        .unwrap_or_default();
    let loss_before = project_v1_loss_before_compatibility_v2(&token, &target);

    let mut registry = InMemoryTaskRegistry::new();
    registry.insert(case.task.clone());
    let omega = WitnessSet::new(vec![]);

    let result = engine.commit_task_claim(osp_core::engine::TaskCommitInput {
        claim: &final_claim,
        omega: &omega,
        task_resolver: &registry as &dyn TaskResolver,
        target,
        loss_before,
        measured: measured_for_commit,
    });

    // **PR #91 review P1:** commit_task_claim'e geçirilen gerçek decision-input scalar'ları.
    // V2 candidate fail-closed projection: loss_before = project_v1_loss_before_compatibility_v2
    // (Unavailable dalı → loss_after). loss_after = trajectory_loss(token.after(), target).
    // token move edilmedi (clone ile extracted), loss_after helper projection ile aynı after.
    let loss_after = osp_core::trajectory::trajectory_loss(token.after(), &target);
    let decision_input = Some(DecisionInputObservation {
        loss_before_bits: loss_before.to_bits(),
        loss_after_bits: loss_after.to_bits(),
    });

    let pipeline = match result {
        Ok(commit_result) => finalize_pipeline_observation_commit_reached(&commit_result),
        Err(e) => PipelineObservation::StoppedBeforeCommit {
            stage: PipelineStage::from_engine_commit_error(&e),
            error: PipelineFailureClass::from_engine_commit_error(&e),
        },
    };

    CharacterizationObservation {
        measurement,
        pipeline,
        decision_input,
    }
}

/// commit_task_claim başarılı sonucundan PipelineObservation::CommitReached üret.
///
/// **Review tur 6 P0-1 (Held fabrication çürütüldü):** Held/Rejected `AuthorizationContext`
/// taşır (engine.rs:1133) — `authorization.outcome` gerçek `AttemptOutcome` (predicate_completion
/// + mutation_decision) + `authorization.apply_target` verir. Tur 1 "Held fabrication"
/// düzeltmesi yanlıştı; Held/Rejected'da outcome observation'a taşınır (authoritative
/// engine çıktısı, fabrication DEĞİL).
fn finalize_pipeline_observation_commit_reached(
    result: &osp_core::engine::EngineCommitResult,
) -> PipelineObservation {
    use osp_core::engine::EngineCommitResult;
    match result {
        EngineCommitResult::Evaluated { result, .. } => {
            // Evaluated → witness Satisfied; gerçek outcome TaskCommitResult'ta surfaced.
            PipelineObservation::CommitReached {
                q5: Q5Observation::Passed, // Q5 passed (commit reached past vision gate)
                predicate_completion: Some(result.outcome.predicate_completion.clone()),
                mutation_decision: Some(result.outcome.mutation_decision.clone()),
                apply_target: Some(result.apply_target.clone()),
                witness_reachability: WitnessReachability::Evaluated,
            }
        }
        EngineCommitResult::Held { authorization, .. } => PipelineObservation::CommitReached {
            q5: Q5Observation::Passed,
            // **Review tur 6 P0-1 fix:** Held `AuthorizationContext` taşır (engine.rs:1133);
            // authorization.outcome AttemptOutcome (predicate_completion + mutation_decision)
            // + authorization.apply_target içerir. Bu authoritative engine çıktısıdır —
            // fabrication DEĞİL. Önceki tur "Held fabrication" düzeltmesi yanlıştı; gerçek
            // outcome observation'a taşınmalı (tur 1 fabrication iddiası çürütüldü).
            predicate_completion: Some(authorization.outcome.predicate_completion.clone()),
            mutation_decision: Some(authorization.outcome.mutation_decision.clone()),
            apply_target: Some(authorization.apply_target.clone()),
            witness_reachability: WitnessReachability::Held,
        },
        EngineCommitResult::Rejected { authorization, .. } => PipelineObservation::CommitReached {
            q5: Q5Observation::Passed,
            // Rejected aynı şekilde authorization taşır (engine.rs:1140).
            predicate_completion: Some(authorization.outcome.predicate_completion.clone()),
            mutation_decision: Some(authorization.outcome.mutation_decision.clone()),
            apply_target: Some(authorization.apply_target.clone()),
            witness_reachability: WitnessReachability::Rejected,
        },
    }
}

/// V1 compatibility projection: loss_before measurement'dan türe.
///
/// Available baseline → gerçek trajectory_loss(before, target).
/// Unavailable baseline → loss_before = loss_after (fail-closed projection).
///
/// **Önemli (review tur 5 P0-1):** `improved=false → Reject` zinciri **koşulsuz
/// DEĞİLDİR**. Production decision core completion-first çalışır:
/// - `PredicateSetResult::Completed` → `MutationDecision::AcceptAsCompleted`
///   (improved'a bakılmaz; loss_before=loss_after etkisiz).
/// - `PredicateSetResult::NotCompleted` + `AcceptImprovement` policy → improved
///   değerlendirilir; loss_before=loss_after → improved=false → progress yok →
///   Reject (BU dalda policy etkisi observable).
///
/// Yani Unavailable projection sadece **NotCompleted + improvement-sensitive policy**
/// durumunda decision'ı etkiler. Case 4 `Coupling ≤ 0.5` + measured coupling 0.0 →
/// `Completed` → projection'ın decision etkisi YOK. Bu yüzden:
/// - **Representation divergence kanıtlandı** (V1 baseline typed değil, V2 UnavailableAllIntroduced).
/// - **Policy/decision divergence kanıtlanmadı** — NotCompleted fixture gerek (P2-0B.8).
///
/// Unavailable scalar tarihsel loss_before DEĞİL; V1 scalar contract'ına geçici
/// adaptasyon. Faz 8a V2 typed loss evidence cutover'ında kaldırılacak.
pub fn project_v1_loss_before_compatibility_v2(
    measurement: &osp_core::measurement::EngineMeasurement,
    target: &osp_core::coords::RawPosition,
) -> f64 {
    use osp_core::measurement::MeasurementBaseline;
    use osp_core::trajectory::trajectory_loss;
    match measurement.before() {
        MeasurementBaseline::Available(before) => trajectory_loss(before, target),
        MeasurementBaseline::Unavailable { .. } => {
            // loss_before = loss_after. Decision etkisi SADECE NotCompleted +
            // improvement-sensitive policy dalında (Completed dalında etkisiz —
            // completion-first decision core improved'a bakmaz).
            trajectory_loss(measurement.after(), target)
        }
    }
}

// trim_ascii nightly'de std'de; stable için lokal implementation.
trait TrimAscii {
    fn trim_ascii(&self) -> &str;
}
impl TrimAscii for str {
    fn trim_ascii(&self) -> &str {
        let bytes = self.as_bytes();
        let start = bytes
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(bytes.len());
        let end = bytes
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map(|p| p + 1)
            .unwrap_or(0);
        &self[start..end]
    }
}
