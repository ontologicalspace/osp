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
use osp_core::witness::Claim;
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
    /// task.predicate.scope == proposal.affected_nodes — parity expected.
    MatchingScope,
    /// proposal.affected_nodes ⊃ task.predicate.scope — known production-reachable divergence.
    WideAffectedScope,
    /// removed_edges[*].from outside task.predicate.scope — divergence.
    RemovedEdgeExternalSource,
    /// task.predicate.scope members all delta-introduced — V2 baseline Unavailable.
    DeltaIntroducedSubject,
    /// subject members report different MetricSource per axis — provenance divergence.
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
        // P2-0B.9'da eklenecek: wide_affected_scope_001, removed_edge_external_001,
        // delta_introduced_subject_001, mixed_per_axis_sources_001.
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
/// Sıralama: space, task, proposal (her biri serde_json ile canonical sorted-keys
/// serialize). Bu, builder drift'inin digest'e yansımasını sağlar.
pub fn serialize_case_bytes(case: &CharacterizationCase) -> Vec<u8> {
    // serde_json canonical (sorted keys) — builder drift deterministic yakalanır.
    let space = serde_json::to_vec(&case.space).expect("space serialize");
    let task = serde_json::to_vec(&case.task).expect("task serialize");
    let proposal = serde_json::to_vec(&case.proposal).expect("proposal serialize");
    let mut combined = Vec::new();
    combined.extend_from_slice(&space);
    combined.extend_from_slice(&task);
    combined.extend_from_slice(&proposal);
    combined
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case 001: matching-single-node (baseline parity)
// ═══════════════════════════════════════════════════════════════════════════════

/// Task targets Node(1) via `Node(1)` predicate scope; proposal declares
/// `affected_nodes=[1]`. Node 1 pre-exists in space.
///
/// Subject set identical V1/V2 → all metrics parity expected. Baseline parity case.
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
        description: "Task targets Node(1); proposal affected_nodes=[1]. Baseline parity."
            .to_string(),
        space,
        task,
        proposal,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Claim builder (structural delta proposal'dan)
// ═══════════════════════════════════════════════════════════════════════════════

/// Proposal'dan structural delta Claim üretir (her iki harness için ortak).
///
/// Bu, navigator'ın `build_claim_from_proposal` pattern'ini mirror eder ama
/// `computed_raw` parametresini caller verir (V1: `compute_raw_from_delta` sonucu,
/// V2-candidate probe: `RawPosition::default()`).
pub fn build_claim_from_proposal(
    proposal: &DeltaProposal,
    computed_raw: osp_core::coords::RawPosition,
    task_id: osp_core::trajectory::TaskId,
    agent: osp_core::witness::AgentId,
    claim_id: osp_core::witness::ClaimId,
) -> Claim {
    use osp_core::space::{Edge, Node};
    // NewNodeSpec → Node (connected_to ile edge'ler dahil).
    let mut delta_nodes: Vec<Node> = proposal
        .new_nodes
        .iter()
        .enumerate()
        .map(|(i, spec)| Node {
            id: 10_000 + i as u64,
            kind: spec.kind,
            mass: spec.initial_mass,
            ..Default::default()
        })
        .collect();
    let _ = &mut delta_nodes; // (matcher için; delta_nodes below'da kullanılıyor)
    let mut delta_edges: Vec<Edge> = proposal
        .new_edges
        .iter()
        .map(|spec| Edge {
            from: spec.from,
            to: spec.to,
            kind: spec.kind,
            is_type_only: false,
        })
        .collect();
    for (i, spec) in proposal.new_nodes.iter().enumerate() {
        let node_id = 10_000 + i as u64;
        for (target, kind) in &spec.connected_to {
            delta_edges.push(Edge {
                from: node_id,
                to: *target,
                kind: *kind,
                is_type_only: false,
            });
        }
    }
    Claim {
        id: claim_id,
        intent: osp_core::witness::Intent::new(agent, computed_raw),
        author: agent,
        computed_raw,
        delta_nodes,
        delta_edges,
        task_id: Some(task_id),
        removed_edges: proposal.removed_edges.clone(),
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
}

/// Measurement producer sonucu (V1: compute_raw_from_delta infallible;
/// V2-candidate: measure_task_delta fallible).
#[derive(Debug, Clone)]
pub enum MeasurementObservation {
    /// Measurement başarıyla üretildi.
    Produced {
        /// V2-candidate subject scope (task.predicate.scope üyeleri).
        /// V1 için `None` (V1 subject = affected_nodes, farklı kaynak).
        subject: Option<Vec<u64>>,
        /// After-state measured position (V1: compute_raw→provenanced_from_raw;
        /// V2-candidate: measurement.after()).
        measured_after: osp_core::trajectory::ProvenancedRawPosition,
        /// V2-candidate baseline kind (Available | Unavailable).
        /// V1 için `None` (V1 always has current_measured).
        baseline_kind: Option<BaselineKind>,
    },
    /// V2-candidate measurement producer hatası (V1 infallible → bu varyant V1'de yok).
    Failed { error: MeasurementFailureClass },
    /// V1 path measurement'ı tracking yapmaz (compute_raw_from_delta infallible).
    NotAttempted,
}

/// MeasurementBaseline::Available|Unavailable lossy projection (rust enum import etmek
/// yerine minimal representasyon — characterization raporlama için).
#[derive(Debug, Clone, PartialEq)]
pub enum BaselineKind {
    Available,
    UnavailableAllIntroduced,
    UnavailablePartialNew,
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
}

/// Pipeline (commit_task_claim) sonucu — stage-aware erken duruşlar dahil.
#[derive(Debug, Clone)]
pub enum PipelineObservation {
    /// commit_task_claim tam çalıştı (Evaluated/Held/Rejected dahil).
    CommitReached {
        q5: Q5Observation,
        predicate_completion: osp_core::trajectory::PredicateCompletion,
        mutation_decision: osp_core::trajectory::MutationDecision,
        apply_target: osp_core::trajectory::ApplyTarget,
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
#[derive(Debug, Clone)]
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
    Witness,
    Other,
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

/// Pipeline stage'ı EngineCommitError'dan çıkar (hangi aşamada durduğunu söyler).
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
            | EngineCommitError::MeasurementBindingVerification(_) => Self::Other,
            EngineCommitError::AuthorizationContextFailed(_) => Self::Other,
            EngineCommitError::NoPersistence | EngineCommitError::Persistence(_) => Self::Other,
            EngineCommitError::Internal(_) => Self::Other,
            EngineCommitError::InvalidWitnessEvidence(_) => Self::Witness,
        }
    }
}

/// Q5 exact theta bits (engine-unit'ten — integration public API'den alamaz).
#[derive(Debug, Clone)]
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

    // V1: compute_raw_from_delta (infallible).
    let delta_nodes: Vec<_> = case
        .proposal
        .new_nodes
        .iter()
        .enumerate()
        .map(|(i, spec)| osp_core::space::Node {
            id: 10_000 + i as u64,
            kind: spec.kind,
            mass: spec.initial_mass,
            ..Default::default()
        })
        .collect();
    let delta_edges: Vec<_> = case
        .proposal
        .new_edges
        .iter()
        .map(|spec| osp_core::space::Edge {
            from: spec.from,
            to: spec.to,
            kind: spec.kind,
            is_type_only: false,
        })
        .collect();
    let computed_raw = engine.compute_raw_from_delta(
        &delta_nodes,
        &delta_edges,
        &case.proposal.removed_edges,
        &affected,
    );

    // Claim + measured (V1 path).
    let claim = build_claim_from_proposal(&case.proposal, computed_raw, case.task.id, 100, 1)
        .expect("V1 claim build should succeed for characterization case");
    let measured = provenanced_from_raw(claim.computed_raw, osp_core::coords::MetricSource::Scip);
    let target = case
        .task
        .target_predicate_set
        .preferred_vector
        .unwrap_or_default();
    let loss_before = osp_core::trajectory::trajectory_loss(&measured, &target);

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

    let measurement = MeasurementObservation::Produced {
        subject: None, // V1 subject = affected_nodes (farklı kaynak), None = "not tracked"
        measured_after: measured,
        baseline_kind: None, // V1 always has current_measured
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
            return CharacterizationObservation {
                measurement: MeasurementObservation::Failed {
                    error: error.clone(),
                },
                pipeline: PipelineObservation::StoppedBeforeCommit {
                    stage: PipelineStage::Other, // measurement producer hatası
                    error: match error {
                        MeasurementFailureClass::Binding => PipelineFailureClass::Binding,
                        MeasurementFailureClass::Revision => PipelineFailureClass::Internal,
                        MeasurementFailureClass::SubjectScope => {
                            PipelineFailureClass::TaskValidation
                        }
                        MeasurementFailureClass::Coordinate => PipelineFailureClass::Internal,
                        MeasurementFailureClass::Digest => PipelineFailureClass::Internal,
                        MeasurementFailureClass::Other => PipelineFailureClass::Other,
                    },
                },
            };
        }
    };

    // Measurement başarılı — observation üret + commit için değerleri çıkar.
    let measured_for_commit = token.after().clone();
    let computed_raw_for_final_claim = token.after().to_raw();
    let baseline_kind = match token.before() {
        osp_core::measurement::MeasurementBaseline::Available(_) => Some(BaselineKind::Available),
        osp_core::measurement::MeasurementBaseline::Unavailable { reason } => {
            use osp_core::measurement::BaselineUnavailableReason;
            match reason {
                BaselineUnavailableReason::AllMembersIntroducedByDelta { .. } => {
                    Some(BaselineKind::UnavailableAllIntroduced)
                }
                BaselineUnavailableReason::PartialNewSubject { .. } => {
                    Some(BaselineKind::UnavailablePartialNew)
                }
            }
        }
    };
    let measurement = MeasurementObservation::Produced {
        subject: Some(
            case.task
                .target_predicate_set
                .predicates
                .iter()
                .flat_map(|wp| match &wp.predicate.scope {
                    osp_core::trajectory::PredicateScope::Node(n) => vec![*n],
                    osp_core::trajectory::PredicateScope::Subgraph(ns) => ns.clone(),
                    osp_core::trajectory::PredicateScope::Module(_) => vec![],
                })
                .collect(),
        ),
        measured_after: measured_for_commit.clone(),
        baseline_kind,
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
    }
}

/// commit_task_claim başarılı sonucundan PipelineObservation::CommitReached üret.
fn finalize_pipeline_observation_commit_reached(
    result: &osp_core::engine::EngineCommitResult,
) -> PipelineObservation {
    use osp_core::engine::EngineCommitResult;
    match result {
        EngineCommitResult::Evaluated { result, .. } => {
            // Evaluated → witness reached (Satisfied).
            PipelineObservation::CommitReached {
                q5: Q5Observation::Passed, // Q5 passed (commit reached past vision gate)
                predicate_completion: result.outcome.predicate_completion.clone(),
                mutation_decision: result.outcome.mutation_decision.clone(),
                apply_target: result.apply_target.clone(),
                witness_reachability: WitnessReachability::Evaluated,
            }
        }
        EngineCommitResult::Held { .. } => PipelineObservation::CommitReached {
            q5: Q5Observation::Passed,
            predicate_completion: osp_core::trajectory::PredicateCompletion::NotCompleted,
            mutation_decision: osp_core::trajectory::MutationDecision::AcceptAsCompleted,
            apply_target: osp_core::trajectory::ApplyTarget::NotApplied,
            witness_reachability: WitnessReachability::Held,
        },
        EngineCommitResult::Rejected { .. } => PipelineObservation::CommitReached {
            q5: Q5Observation::Passed,
            predicate_completion: osp_core::trajectory::PredicateCompletion::NotCompleted,
            mutation_decision: osp_core::trajectory::MutationDecision::AcceptAsCompleted,
            apply_target: osp_core::trajectory::ApplyTarget::NotApplied,
            witness_reachability: WitnessReachability::Rejected,
        },
    }
}

/// V1 compatibility projection: loss_before measurement'dan türe.
///
/// Available baseline → gerçek trajectory_loss(before, target).
/// Unavailable baseline → fail-closed: loss_before = loss_after → improved=false → Reject.
///
/// **Önemli:** Unavailable durumundaki scalar tarihsel loss_before DEĞİL; V1 scalar
/// contract'ına geçici adaptasyon (improved=false zorla). Faz 8a V2 typed loss evidence
/// cutover'ında kaldırılacak.
pub fn project_v1_loss_before_compatibility_v2(
    measurement: &osp_core::measurement::EngineMeasurement,
    target: &osp_core::coords::RawPosition,
) -> f64 {
    use osp_core::measurement::MeasurementBaseline;
    use osp_core::trajectory::trajectory_loss;
    match measurement.before() {
        MeasurementBaseline::Available(before) => trajectory_loss(before, target),
        MeasurementBaseline::Unavailable { .. } => {
            // Fail-closed: loss_before = loss_after → improved=false → Reject.
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
