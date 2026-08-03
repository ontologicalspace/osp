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
    /// **Faz 8-P2 #88 (MD-2 evidence contract):** Cohesion axis, Subgraph[1,2] scope. V2
    /// measured cohesion source = Mixed (aggregate [Scip, Placeholder]); V1 legacy
    /// projected = uniform Scip. 6-case required_source matrisi: None/Scip/TreeSitter/
    /// Heuristic/Placeholder (predicate evaluation) + Mixed (declaration validation reject).
    /// `Mixed` gerçek cohesion aggregation hattından doğar (elle enjekte edilmez).
    MixedPerAxisSources,
    /// Concrete V2 per-axis source required source ile eşleşince predicate completion
    /// `Completed`; legacy V1 projection (uniform Scip) eşleşmeyince `SourceInsufficient`.
    ///
    /// **Faz 8-P2 #88 (MD-2 pozitif-matching kontrolü):** Coupling axis, Node(1) scope.
    /// V1 legacy projected coupling = Scip; V2 measured = TreeSitter (engine topology).
    /// 5-case required_source matrisi (PR #85 inline matrix frozen corpus'a taşınır):
    /// None/Scip/TreeSitter/Placeholder/Heuristic. `required_source=TreeSitter → V2 Completed`
    /// dalı pozitif-matching kontrolü (MixedPerAxisSources'ta kayıp).
    DirectPerAxisAuthority,
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
    /// **Faz 8-P2 #88:** Measured-subject digest (production canonical tipler üzerinden).
    /// `DirectPerAxisAuthority`/`MixedPerAxisSources` family'leri için: aynı family içindeki
    /// case'ler aynı measured-subject digest'ine sahiptir (farklı declaration yorumları).
    /// Eski 5 case + `None` (backward-compatible; measured-subject digest yok).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_subject_digest_blake3: Option<String>,
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
    // MSRV note (issue #110): `str::trim_ascii` Rust 1.80+ stable. Workspace
    // MSRV=1.75 iddia ediyor ama toolchain pin 1.97 — çelişki #110'da takip ediliyor.
    #[allow(
        clippy::incompatible_msrv,
        reason = "toolchain pin 1.97.1; MSRV gap tracked by #110"
    )]
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
        // Faz 8-P2 #88 — DirectPerAxisAuthority family (5 case).
        direct_coupling_required_none_001(),
        direct_coupling_required_scip_001(),
        direct_coupling_required_tree_sitter_001(),
        direct_coupling_required_placeholder_001(),
        direct_coupling_required_heuristic_001(),
        // Faz 8-P2 #88 — MixedPerAxisSources family (6 case).
        mixed_cohesion_required_none_001(),
        mixed_cohesion_required_scip_001(),
        mixed_cohesion_required_tree_sitter_001(),
        mixed_cohesion_required_heuristic_001(),
        mixed_cohesion_required_placeholder_001(),
        mixed_cohesion_required_mixed_invalid_001(),
    ]
}

/// **Faz 8-P2 #88:** Class'a göre case filtreleme — behavioral test'ler frozen corpus'tan
/// yükler. `DirectPerAxisAuthority`/`MixedPerAxisSources` family'leri için matrix test'leri
/// bu helper üzerinden ilgili subset'i çeker.
pub fn load_cases_by_class(class: CaseClass) -> Vec<CharacterizationCase> {
    build_all_cases()
        .into_iter()
        .filter(|c| c.class == class)
        .collect()
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

/// **Faz 8-P2 #88:** Measured-subject digest helper'ları için case ID + digest çifti.
///
/// `compute_case_digests`'ten ayrı çünkü measured-subject digest fallible (exact-one
/// predicate guard, Module scope unsupported). Bootstrap bu helper'ı çağırıp
/// `measured_subject_digest_blake3` alanını günceller.
///
/// **P0 fix (review):** `compute_measured_subject_digest` zaten raw BLAKE3 32-byte hash
/// döndürür (`H(canonical_subject)`). Önceki kod `blake3_hex` çağırıyordu — bu tekrar
/// hash'leyip `H(H(canonical_subject))` üretiyordu. Doğru davranış: raw bytes'ı hex-encode.
pub fn compute_case_measured_subject_digests() -> Vec<(String, Result<String, SubjectDigestError>)>
{
    // **P2-1 fix (review):** Yalnız DirectPerAxisAuthority + MixedPerAxisSources family'leri
    // measured-subject digest taşır. Eski family'ler (matching_scope/wide_affected_scope/
    // removed_edge_external_source/delta_introduced_subject) guard tarafından None zorunlu —
    // bootstrap eski case digest yazdırırsa manifest'e kopyalandığında guard reddeder.
    build_all_cases()
        .into_iter()
        .filter(|case| {
            matches!(
                case.class,
                CaseClass::DirectPerAxisAuthority | CaseClass::MixedPerAxisSources
            )
        })
        .map(|case| {
            let digest = compute_measured_subject_digest(&case);
            (case.id, digest.map(hex::encode))
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Measured-subject digest V1 — production canonical tipler üzerinden tek authority
// ═══════════════════════════════════════════════════════════════════════════════
//
// Faz 8-P2 #88 (MD-2 evidence contract). `MEASUREMENT_SUBJECT:V1` digest sözleşmesi:
// aynı family içindeki case'ler (farklı required_source declaration) aynı measured-subject
// digest'ine sahiptir — ontolojik olarak aynı fiziksel subject + aynı measurement selector.
//
// **Tek canonicalization authority:** Tüm parçalar production tiplerine bağlanır:
// - SpaceDigest::compute (authorization.rs:1305) — canonical node/edge content
// - PredicateAxisTag (canonical_tags.rs:167) — stable numeric axis tag
// - CanonicalPredicateScope (authorization.rs:608) — scope_tag + identity_bytes
// - CanonicalSubjectScope (measurement.rs:113) — sort + duplicate reject + empty reject
// - CanonicalStructuralDelta::try_new (authorization.rs:207) — sort + duplicate/conflict reject
//
// Test katmanı ikinci bir canonicalizer YAZMAZ. `affected_nodes` structural delta içine
// karıştırılmaz — ayrı `legacy-selector` segmentinde (V1 measurement selector).

/// Measured-subject digest hesaplama hataları (V1 evidence schema).
///
/// Bu hatalar **digest evidence schema** kararıdır — production proposal ingress
/// davranışını temsil ETMEZ. Production canonicalization hataları (`CanonicalizationError`,
/// `MeasurementDigestError`) buraya map edilir.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SubjectDigestError {
    /// Exact-one predicate invariant: characterization case tek predicate taşımalı.
    /// Digest V1 yalnızca exact-one characterization case'lerini kabul eder (zero/multi
    /// fail-closed). Pozitif corpus assertion'ına güvenilmez — negatif testlerle kanıtlanır.
    #[error("expected exactly one predicate, got {actual}")]
    ExpectedExactlyOnePredicate { actual: usize },
    /// `PredicateScope::Module` V1 evidence schema'da desteklenmez (characterization
    /// case'leri Node/Subgraph kullanır). Out-of-scope olmak fail-closed test'ten ayrı.
    #[error("unsupported scope for measured-subject digest V1: {scope:?}")]
    UnsupportedScope {
        scope: osp_core::trajectory::PredicateScope,
    },
    /// Boş Subgraph scope — production `CanonicalSubgraphScope::try_new` empty'yi reject
    /// etmez; V1 evidence schema kendi dar kararıyla reddeder.
    #[error("empty subgraph scope")]
    EmptySubgraphScope,
    /// Production canonicalization hatası (duplicate node id, duplicate edge identity,
    /// cross-list conflict, non-finite field) — production semantiğiyle reddedilir.
    #[error("production canonicalization failed: {0}")]
    ProductionCanonicalization(String),
}

/// Measured-subject digest için segment append — u64 big-endian length prefix.
///
/// Frozen digest platformdan bağımsız: byte width (u64) + endianness (big-endian)
/// sözleşmenin parçası. `usize → u64` checked (digest segment length'u makul sınırlarda).
fn append_segment(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u64::try_from(bytes.len()).expect("digest segment length fits u64");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Bir case'in measured-subject digest bytes'ını üretir (production canonical tiplerle).
///
/// **Ontolojik parçalar** (hepsi production tipleri):
/// 1. `space-content:v1` — `SpaceDigest::compute(space)` (position HARİÇ — author content)
/// 2. `axis-encoding:v1` — `PredicateAxisTag` stable numeric tag
/// 3. `predicate-scope:v1` — `CanonicalPredicateScope` (V2 task-authoritative subject)
/// 4. `legacy-selector:v1` — `CanonicalSubjectScope` (V1 affected_nodes measurement selector)
/// 5. `structural-delta:v1` — `CanonicalStructuralDelta` (uygulanacak fiziksel değişim)
///
/// Digest'ten hariç: required_source, comparison_operator, threshold, tolerance, case_id,
/// description (declaration/policy interpretation — full-case digest'te ayrışır).
pub fn serialize_measured_subject_bytes(
    case: &CharacterizationCase,
) -> Result<Vec<u8>, SubjectDigestError> {
    use osp_core::authorization::{
        CanonicalEdge, CanonicalEdgeIdentity, CanonicalEdgeKind, CanonicalNodeClassification,
        CanonicalNodeKind, CanonicalNodeRole, CanonicalPredicateScope, CanonicalStructuralDelta,
        CanonicalSubgraphScope, SpaceDigest,
    };
    use osp_core::canonical_tags::PredicateAxisTag;
    use osp_core::measurement::CanonicalSubjectScope;

    // Exact-one predicate guard — V1 digest yalnızca exact-one characterization case'leri.
    let predicates = &case.task.target_predicate_set.predicates;
    let [wp] = predicates.as_slice() else {
        return Err(SubjectDigestError::ExpectedExactlyOnePredicate {
            actual: predicates.len(),
        });
    };
    let predicate = &wp.predicate;

    let mut bytes = Vec::new();
    append_segment(&mut bytes, b"OSP:F8P2:MEASUREMENT_SUBJECT:V1");

    // 1. Space content digest (production SpaceDigest — canonical node/edge, position HARİÇ).
    let space_digest = SpaceDigest::compute(&case.space)
        .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
    append_segment(&mut bytes, b"space-content:v1");
    append_segment(&mut bytes, space_digest.as_bytes());

    // 2. Axis encoding (production PredicateAxisTag — stable numeric, Debug/serde DEĞİL).
    let axis_tag = PredicateAxisTag::try_from(&predicate.metric)
        .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
    append_segment(&mut bytes, b"axis-encoding:v1");
    append_segment(&mut bytes, &[axis_tag.as_u8()]);

    // 3. Predicate scope (production CanonicalPredicateScope — V2 task-authoritative subject).
    //    Node(1) ≠ Subgraph([1]): scope_tag ayrımı (Node=0, Subgraph=2).
    let canonical_scope = match &predicate.scope {
        osp_core::trajectory::PredicateScope::Node(id) => CanonicalPredicateScope::Node(*id),
        osp_core::trajectory::PredicateScope::Subgraph(ids) => {
            if ids.is_empty() {
                return Err(SubjectDigestError::EmptySubgraphScope);
            }
            let canonical = CanonicalSubgraphScope::try_new(ids.clone())
                .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
            CanonicalPredicateScope::Subgraph(canonical)
        }
        osp_core::trajectory::PredicateScope::Module(_) => {
            return Err(SubjectDigestError::UnsupportedScope {
                scope: predicate.scope.clone(),
            });
        }
    };
    append_segment(&mut bytes, b"predicate-scope:v1");
    append_segment(&mut bytes, &[canonical_scope.scope_tag()]);
    append_segment(&mut bytes, &canonical_scope.identity_bytes());

    // 4. Legacy measurement selector — V1 effective ölçüm kümesi (P1-1 fix review).
    //    V1 characterization yolu (evaluate_v1_case, mod.rs:1289-1294) affected_nodes ∪
    //    removed_edges.from kümesini ölçer. Önceki kod yalnız raw affected_nodes kullanıyordu —
    //    removed_edges.from içeren case'lerde gerçek V1 subject'i temsil etmiyordu.
    //    Production-effective: affected_nodes ∪ removed_edges.from.
    let mut effective_selector: Vec<u64> = case.proposal.affected_nodes.clone();
    for er in &case.proposal.removed_edges {
        if !effective_selector.contains(&er.from) {
            effective_selector.push(er.from);
        }
    }
    let canonical_selector = CanonicalSubjectScope::try_new(effective_selector)
        .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
    append_segment(&mut bytes, b"legacy-selector:v1");
    let selector_bytes: Vec<u8> = canonical_selector
        .member_ids()
        .iter()
        .flat_map(|id| id.to_le_bytes())
        .collect();
    append_segment(&mut bytes, &selector_bytes);

    // 5. Structural delta — production-effective claim üzerinden (P1-1 fix review).
    //    Önceki kod raw DeltaProposal field'larını manuel çeviriyordu — NewNodeSpec.connected_to
    //    edge'leri düşüyordu (build_claim_from_proposal onları delta_edges'e ekler). Düzeltme:
    //    DeltaProposal → build_claim_from_proposal → gerçek claim.delta_nodes/delta_edges/
    //    removed_edges → CanonicalNode/Edge/Identity → try_new. Bu, production claim
    //    transformation'ının tamamını (connected_to dahil) digest'e taşır.
    use osp_core::navigator::build_claim_from_proposal;
    let probe_claim = build_claim_from_proposal(
        &case.proposal,
        osp_core::coords::RawPosition::default(),
        case.task.id,
        100,
        1,
    )
    .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
    let new_nodes: Vec<osp_core::authorization::CanonicalNode> = probe_claim
        .delta_nodes
        .iter()
        .map(|node| {
            Ok(osp_core::authorization::CanonicalNode {
                id: node.id,
                kind: CanonicalNodeKind::try_from(&node.kind)
                    .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?,
                mass: node.mass,
                cohesion: node.cohesion,
                classification: CanonicalNodeClassification::try_from(&node.classification)
                    .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?,
                role: CanonicalNodeRole::try_from(&node.role)
                    .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?,
            })
        })
        .collect::<Result<Vec<_>, SubjectDigestError>>()?;
    let new_edges: Vec<CanonicalEdge> = probe_claim
        .delta_edges
        .iter()
        .map(|edge| {
            Ok(CanonicalEdge {
                from: edge.from,
                to: edge.to,
                kind: CanonicalEdgeKind::try_from(&edge.kind)
                    .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?,
                is_type_only: edge.is_type_only,
            })
        })
        .collect::<Result<Vec<_>, SubjectDigestError>>()?;
    let removed_edges: Vec<CanonicalEdgeIdentity> = probe_claim
        .removed_edges
        .iter()
        .map(|er| {
            Ok(CanonicalEdgeIdentity::new(
                er.from,
                er.to,
                CanonicalEdgeKind::try_from(&er.kind)
                    .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?,
            ))
        })
        .collect::<Result<Vec<_>, SubjectDigestError>>()?;
    let structural = CanonicalStructuralDelta::try_new(new_nodes, new_edges, removed_edges)
        .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
    // **P1-1 fix (review):** Structural delta segment'i production encoder kullanır.
    // Önceki kod serde_json::to_vec kullanıyordu — production structural digest'inden farklı.
    // MeasurementDeltaDigest::compute_from_canonical production canonical encoding (defensive
    // validate + canonical node/edge/identity encode). Tek structural-delta authority.
    let structural_digest =
        osp_core::measurement::MeasurementDeltaDigest::compute_from_canonical(&structural)
            .map_err(|e| SubjectDigestError::ProductionCanonicalization(e.to_string()))?;
    append_segment(&mut bytes, b"structural-delta:v1");
    append_segment(&mut bytes, structural_digest.as_bytes());

    Ok(bytes)
}

/// Measured-subject digest BLAKE3 hex (64 lowercase hex chars).
///
/// Fallible — exact-one predicate guard / Module scope / production canonicalization
/// hatalarını yayar. Bootstrap bu fonksiyonu çağırıp manifest `measured_subject_digest_blake3`
/// alanını günceller.
pub fn compute_measured_subject_digest(
    case: &CharacterizationCase,
) -> Result<Vec<u8>, SubjectDigestError> {
    let bytes = serialize_measured_subject_bytes(case)?;
    Ok(blake3_raw(&bytes).to_vec())
}

/// BLAKE3 raw 32-byte hash (hex değil). `blake3_hex` hex string döner; bu raw bytes.
fn blake3_raw(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
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
// Faz 8-P2 #88 — DirectPerAxisAuthority family (MD-2 pozitif-matching kontrolü)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Coupling axis, Node(1) scope. V1 legacy projected coupling source = Scip (uniform
// provenanced_from_raw override); V2 measured coupling source = TreeSitter (engine
// topology_source). 5-case required_source matrisi (PR #85 inline matrix frozen corpus'a
// taşınır — assertion-equivalence map commit message'da).
//
// **P1-1 affected_nodes:[1]:** compute_raw_from_delta `affected_nodes`'u ölçüm kümesi
// olarak kullanır (engine.rs:2291, mass-weighted centroid). affected_nodes:[1] → V1
// measurement selector = Node(1) = V2 subject scope → V1/V2 numeric equality (coupling
// 0.5). Node(2) pre-state'te (fiziksel endpoint bütünlüğü) ama ölçüme dahil değil.
//
// **P2-2 threshold Le 0.5:** eski inline fixture exact. Coupling after = 0.5 (binary
// exact temsil) → floating-point fragility yok. Imports 1→2 → Node(1) out-degree=1 →
// coupling = 1/(1+1) = 0.5.

/// DirectPerAxisAuthority canonical subject — Coupling axis, Node(1) scope.
///
/// Node(1) ve Node(2) her ikisi pre-state'te (Imports 1→2 edge endpoint bütünlüğü).
/// `affected_nodes:[1]` — V1 legacy measurement selector (Node(1) = V2 subject scope).
fn direct_coupling_subject_base() -> CharacterizationCase {
    use osp_core::agent::DeltaProposal;
    use osp_core::space::{Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: iki node (edge endpoint bütünlüğü).
    let mut space = Space::new();
    space.insert_node(Node {
        id: 1,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });
    space.insert_node(Node {
        id: 2,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    // Predicate: Coupling axis, Node(1) scope (V2 task-authoritative subject).
    // required_source caller tarafından set edilir (case builder'lar).
    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Le,
        threshold: 0.5, // exact eski fixture; coupling after = 0.5 (binary exact).
        scope: PredicateScope::Node(1),
        required_source: None, // override by caller
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
        label: "direct-coupling-base".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    let proposal = DeltaProposal {
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 1,
            to: 2,
            kind: osp_core::space::EdgeKind::Imports,
        }],
        affected_nodes: vec![1], // V1 measurement selector = Node(1) = V2 subject scope.
        ..Default::default()
    };

    CharacterizationCase {
        id: "direct-coupling-base".to_string(), // overridden by case builders
        class: CaseClass::DirectPerAxisAuthority,
        source: CaseSource::SyntheticAdversarial,
        description: String::new(), // overridden
        space,
        task,
        proposal,
    }
}

/// `direct_coupling_subject_base`'i alıp required_source + case identity set eder.
fn direct_coupling_case(
    required_source: Option<osp_core::coords::MetricSource>,
    id: &str,
    description: &str,
) -> CharacterizationCase {
    let mut case = direct_coupling_subject_base();
    case.id = id.to_string();
    case.description = description.to_string();
    case.task.label = id.to_string();
    case.task.target_predicate_set.predicates[0]
        .predicate
        .required_source = required_source;
    case
}

fn direct_coupling_required_none_001() -> CharacterizationCase {
    direct_coupling_case(
        None,
        "direct-coupling-required-none-001",
        "DirectPerAxisAuthority: required_source=None. V1 legacy projected coupling = Scip \
         (Completed — source constraint yok); V2 measured coupling = TreeSitter (Completed). \
         Numeric parity: coupling after = 0.5 (Imports 1→2, Node(1) out-degree=1). Provenance \
         divergence yok (None → source authority constraint yok).",
    )
}

fn direct_coupling_required_scip_001() -> CharacterizationCase {
    direct_coupling_case(
        Some(osp_core::coords::MetricSource::Scip),
        "direct-coupling-required-scip-001",
        "DirectPerAxisAuthority: required_source=Some(Scip). V1 legacy projected = Scip → \
         Completed (match); V2 measured = TreeSitter → SourceInsufficient (mismatch). \
         MD-2 divergence: V1 eşleşir, V2 eşleşmez.",
    )
}

fn direct_coupling_required_tree_sitter_001() -> CharacterizationCase {
    direct_coupling_case(
        Some(osp_core::coords::MetricSource::TreeSitter),
        "direct-coupling-required-tree-sitter-001",
        "DirectPerAxisAuthority: required_source=Some(TreeSitter). V1 legacy projected = Scip → \
         SourceInsufficient (mismatch); V2 measured = TreeSitter → Completed (match). \
         Pozitif-matching kontrolü: concrete V2 source required source ile eşleşince \
         predicate completion Completed. MixedPerAxisSources'ta bu dal kaybolur (Mixed hiçbir \
         concrete'a eşit değil).",
    )
}

fn direct_coupling_required_placeholder_001() -> CharacterizationCase {
    direct_coupling_case(
        Some(osp_core::coords::MetricSource::Placeholder),
        "direct-coupling-required-placeholder-001",
        "DirectPerAxisAuthority: required_source=Some(Placeholder). V1 legacy = Scip → \
         SourceInsufficient; V2 measured = TreeSitter → SourceInsufficient. Declaration-valid \
         source token under current validator; unmet by both V1 and V2. Normative authority \
         status is out of MD-2 scope.",
    )
}

fn direct_coupling_required_heuristic_001() -> CharacterizationCase {
    direct_coupling_case(
        Some(osp_core::coords::MetricSource::Heuristic),
        "direct-coupling-required-heuristic-001",
        "DirectPerAxisAuthority: required_source=Some(Heuristic). V1 legacy = Scip → \
         SourceInsufficient; V2 measured = TreeSitter → SourceInsufficient.",
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Faz 8-P2 #88 — MixedPerAxisSources family (MD-2 fail-closed + declaration-validation)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Cohesion axis, Subgraph[1,2] scope. Node(1) cohesion=Some(0.6) → V2 source Scip;
// Node(2) cohesion=None → V2 source Placeholder (effective fallback). Aggregate → Mixed
// (coords.rs aggregate_source). V1 legacy projected = uniform Scip.
//
// **Mixed gerçek aggregation hattından doğar** (elle enjekte edilmez): measure_task_delta
// Subgraph[1,2] centroid → aggregate_source([Scip, Placeholder]) = Mixed.
//
// **DependsOn delta:** coupling/instability sadece Imports okur (axes.rs:64, 258-259);
// cohesion edge-bağımsız (axes.rs:412-425). DependsOn 5 axis nötr → Q5/trajectory loss/
// regression metric değişiminden etkilenmez (metric isolation).
//
// **Predicate Ge 0.50:** measured cohesion = (0.6+0.5)/2 = 0.55 (eşit mass centroid).
// 0.05 marjın — floating-point aggregation fragility'sine karşı dayanıklı.

/// MixedPerAxisSources canonical subject — Cohesion axis, Subgraph[1,2] scope.
fn mixed_cohesion_subject_base() -> CharacterizationCase {
    use osp_core::agent::DeltaProposal;
    use osp_core::space::{Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Space: Node(1) cohesion=Some(0.6), Node(2) cohesion=None.
    let mut space = Space::new();
    space.insert_node(Node {
        id: 1,
        kind: NodeKind::Module,
        mass: 1.0,
        cohesion: Some(0.6), // → V2 cohesion source = Scip (observed_source)
        ..Default::default()
    });
    space.insert_node(Node {
        id: 2,
        kind: NodeKind::Module,
        mass: 1.0,
        cohesion: None, // → V2 cohesion source = Placeholder (effective fallback 0.5)
        ..Default::default()
    });

    // Predicate: Cohesion axis, Subgraph[1,2] scope.
    let predicate = MetricPredicate {
        metric: PredicateAxis::Cohesion,
        operator: ComparisonOp::Ge,
        threshold: 0.50, // measured 0.55 (0.05 marjın).
        scope: PredicateScope::Subgraph(vec![1, 2]),
        required_source: None, // override by caller
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
        id: 43, // direct family'den farklı task id.
        milestone_id: 0,
        label: "mixed-cohesion-base".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    // Delta: DependsOn (5 axis nötr — metric isolation).
    let proposal = DeltaProposal {
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 1,
            to: 2,
            kind: osp_core::space::EdgeKind::DependsOn,
        }],
        affected_nodes: vec![1, 2],
        ..Default::default()
    };

    CharacterizationCase {
        id: "mixed-cohesion-base".to_string(), // overridden
        class: CaseClass::MixedPerAxisSources,
        source: CaseSource::SyntheticAdversarial,
        description: String::new(), // overridden
        space,
        task,
        proposal,
    }
}

/// `mixed_cohesion_subject_base`'i alıp required_source + case identity set eder.
fn mixed_cohesion_case(
    required_source: Option<osp_core::coords::MetricSource>,
    id: &str,
    description: &str,
) -> CharacterizationCase {
    let mut case = mixed_cohesion_subject_base();
    case.id = id.to_string();
    case.description = description.to_string();
    case.task.label = id.to_string();
    case.task.target_predicate_set.predicates[0]
        .predicate
        .required_source = required_source;
    case
}

fn mixed_cohesion_required_none_001() -> CharacterizationCase {
    mixed_cohesion_case(
        None,
        "mixed-cohesion-required-none-001",
        "MixedPerAxisSources: required_source=None. V1 legacy projected = Scip (Completed); \
         V2 measured aggregate = Mixed (Completed). Mixed numeric değerlendirmeye katılır \
         (source authority constraint yok). Cohesion after = 0.55.",
    )
}

fn mixed_cohesion_required_scip_001() -> CharacterizationCase {
    mixed_cohesion_case(
        Some(osp_core::coords::MetricSource::Scip),
        "mixed-cohesion-required-scip-001",
        "MixedPerAxisSources: required_source=Some(Scip). V1 legacy projected = Scip (Completed — \
         match); V2 measured = Mixed → SourceInsufficient (fail-closed — Mixed concrete authority \
         şartını karşılamaz). MD-2 divergence: V1 eşleşir, V2 eşleşmez.",
    )
}

fn mixed_cohesion_required_tree_sitter_001() -> CharacterizationCase {
    mixed_cohesion_case(
        Some(osp_core::coords::MetricSource::TreeSitter),
        "mixed-cohesion-required-tree-sitter-001",
        "MixedPerAxisSources: required_source=Some(TreeSitter). V1 legacy = Scip → \
         SourceInsufficient; V2 measured = Mixed → SourceInsufficient (fail-closed). \
         V1/V2 agreement (ikisi de karşılamaz).",
    )
}

fn mixed_cohesion_required_heuristic_001() -> CharacterizationCase {
    mixed_cohesion_case(
        Some(osp_core::coords::MetricSource::Heuristic),
        "mixed-cohesion-required-heuristic-001",
        "MixedPerAxisSources: required_source=Some(Heuristic). V1 legacy = Scip → \
         SourceInsufficient; V2 measured = Mixed → SourceInsufficient.",
    )
}

fn mixed_cohesion_required_placeholder_001() -> CharacterizationCase {
    mixed_cohesion_case(
        Some(osp_core::coords::MetricSource::Placeholder),
        "mixed-cohesion-required-placeholder-001",
        "MixedPerAxisSources: required_source=Some(Placeholder). V1 legacy = Scip → \
         SourceInsufficient; V2 measured = Mixed → SourceInsufficient. Declaration-valid \
         source token under current validator; unmet by both V1 and V2. Normative authority \
         status is out of MD-2 scope.",
    )
}

fn mixed_cohesion_required_mixed_invalid_001() -> CharacterizationCase {
    mixed_cohesion_case(
        Some(osp_core::coords::MetricSource::Mixed),
        "mixed-cohesion-required-mixed-invalid-001",
        "MixedPerAxisSources: required_source=Some(Mixed) → declaration validation reject. \
         Mixed epistemik bir talep değildir — yalnız heterojen aggregation çıktısıdır. \
         commit_task_claim validate_for_commit (engine.rs:1416) → InvalidRequiredMetricSource \
         (trajectory.rs:822-828) → EngineCommitError::TaskValidation. Measurement'dan ÖNCE \
         terminal reject. Standalone probe Mixed üretebildi (cohesion aggregation), ama commit \
         pipeline measurement'a ulaşmadan durdu (probe vs pipeline ayrımı).",
    )
}

/// Bir V1 veya V2-candidate evaluation'ın tam gözlemi.
///
/// Measurement ve pipeline gözlemleri ayrılır (plan Tur 5 P1-2): erken duruşlar
/// (SyntaxViolation/TaskValidation/MeasurementError) ve tam commit sonuçları aynı
/// modelde temsil edilebilir. Integration test uydurma değer ÜRETMEZ —
/// `q5_theta_bits` yalnız `Q5Observation::Rejected` içinde (failure observable),
/// successful theta ayrı engine-unit (engine.rs #[cfg(test)] Q5ThetaObservation).
#[derive(Debug, Clone)]
pub struct CharacterizationObservation {
    pub measurement: MeasurementObservation,
    pub pipeline: PipelineObservation,
    /// `commit_task_claim`'e gerçekten geçirilen decision-input scalar'ları
    /// (PR #91 review P1). `Some` ⟺ PredicateGate'e ulaşıldı (`commit_task_claim` Ok /
    /// `EngineCommitResult` üretildi) — yalnızca `EngineCommitResult` ile decision yolu
    /// tamamlandığında scalar'lar meaningful. Measurement early-return (commit çağrılmadan)
    /// VE `commit_task_claim` Err (engine çağrıldı ama PredicateGate'e ulaşmadı, örn Q5
    /// Vision ihlali) durumlarında `None`. V2 candidate fail-closed projection
    /// (`loss_before == loss_after`) exact frozen evidence olarak pinlenir — sadece
    /// decision sonucu DEĞİL, projection zincirinin tamamı.
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
// Layout: Produced varyantı ~205 byte (Vec + ProvenancedRawPosition + arrays + baseline).
// Test fixture — production carrier enum DEĞİL (Result'ta taşınmaz, lokal değişken).
// PR #112 ontolojik kararından farklı: orada carrier enum'lar her Result'ta taşınıyordu
// (gerçek layout maliyeti). Burada test-only allow makul.
#[allow(
    clippy::large_enum_variant,
    reason = "test fixture enum — local variable, not carried in Result; production carrier boxing policy (PR #112) does not apply"
)]
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
/// (engine.rs #[cfg(test)] Q5ThetaObservation, PR #87-B) karakterize eder.
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
            } => Self::Observed(*md),
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

// Successful Q5 theta is intentionally not reconstructed by the integration
// characterization harness. It is observed at the authoritative engine-unit
// boundary (engine.rs #[cfg(test)] — PR #87-B Q5ThetaObservation); integration
// observations expose theta only on typed Q5 rejection
// (EngineCommitError::VisionViolation).

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
    //
    // **PR #91 review P2 (non-blocking):** `decision_input` yalnızca predicate decision yoluna
    // ulaşıldığında (commit_task_claim Ok) Some — Err (StoppedBeforeCommit) olsa bile engine
    // çağrıldıysa scalar'lar gönderildi, ama decision yolu tamamlanmadı. Doc contract: Some ⟺
    // PredicateGate'e ulaşıldı (EngineCommitResult üretildi).
    let loss_after = osp_core::trajectory::trajectory_loss(&measured, &target);
    let decision_input = result.as_ref().ok().map(|_| DecisionInputObservation {
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
    //
    // **PR #91 review P2 (non-blocking):** `decision_input` yalnızca predicate decision yoluna
    // ulaşıldığında (commit_task_claim Ok) Some — Err (StoppedBeforeCommit) scalar'lar gönderildi
    // ama decision yolu tamamlanmadı. Doc contract: Some ⟺ PredicateGate'e ulaşıldı.
    let loss_after = osp_core::trajectory::trajectory_loss(token.after(), &target);
    let decision_input = result.as_ref().ok().map(|_| DecisionInputObservation {
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
///   düzeltmesi yanlıştı; Held/Rejected'da outcome observation'a taşınır (authoritative
///   engine çıktısı, fabrication DEĞİL).
fn finalize_pipeline_observation_commit_reached(
    result: &osp_core::engine::EngineCommitResult,
) -> PipelineObservation {
    use osp_core::engine::EngineCommitResult;
    match result {
        EngineCommitResult::Evaluated { result, .. } => {
            // Evaluated → witness Satisfied; gerçek outcome TaskCommitResult'ta surfaced.
            PipelineObservation::CommitReached {
                q5: Q5Observation::Passed, // Q5 passed (commit reached past vision gate)
                predicate_completion: Some(result.outcome.predicate_completion),
                mutation_decision: Some(result.outcome.mutation_decision),
                apply_target: Some(result.apply_target),
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
            predicate_completion: Some(authorization.outcome.predicate_completion),
            mutation_decision: Some(authorization.outcome.mutation_decision),
            apply_target: Some(authorization.apply_target),
            witness_reachability: WitnessReachability::Held,
        },
        EngineCommitResult::Rejected { authorization, .. } => PipelineObservation::CommitReached {
            q5: Q5Observation::Passed,
            // Rejected aynı şekilde authorization taşır (engine.rs:1140).
            predicate_completion: Some(authorization.outcome.predicate_completion),
            mutation_decision: Some(authorization.outcome.mutation_decision),
            apply_target: Some(authorization.apply_target),
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
