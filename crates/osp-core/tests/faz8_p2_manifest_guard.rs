//! Faz 8-P2 P2-0B fixture infrastructure guard — manifest loading, sidecar digest
//! doğrulaması, per-fixture (case) digest doğrulaması, path-traversal guard.
//!
//! Bu test, characterization fixture ağacının frozen olduğunu pins:
//! - sidecar `cases.blake3` == BLAKE3(cases.json raw bytes)
//! - her case builder_digest_blake3 == BLAKE3(builder serialize çıktısı)
//! - path-traversal (`../` kaçışı) reddedilir
//!
//! Bu test fail olursa, fixture/builder drift olmuştur — explicit fixture+digest
//! güncellemesi gerekir (bootstrap helper'ı ile).

mod common;

use common::{
    blake3_hex, build_all_cases, characterization_root, load_fixture_bytes, load_manifest,
};

#[test]
fn manifest_loads_and_sidecar_digest_matches() {
    let root = characterization_root(env!("CARGO_MANIFEST_DIR"));
    let manifest = load_manifest(&root).expect("manifest yüklenmeli + sidecar digest eşit olmalı");
    assert_eq!(manifest.schema_version, 1, "schema_version 1 olmalı");
    assert!(
        manifest
            .cases
            .iter()
            .any(|c| c.id == "matching-single-node-001"),
        "baseline case manifest'te olmalı"
    );
}

#[test]
fn every_case_builder_digest_matches_manifest() {
    let root = characterization_root(env!("CARGO_MANIFEST_DIR"));
    let manifest = load_manifest(&root).expect("manifest yüklenmeli");
    let cases = build_all_cases();

    // **Review P2:** manifest ID uniqueness — duplicate ID'ler characterization'da
    // karışıklığa yol açar.
    let mut seen_ids = std::collections::HashSet::new();
    for manifest_case in &manifest.cases {
        assert!(
            seen_ids.insert(&manifest_case.id),
            "duplicate case ID in manifest: {}",
            manifest_case.id
        );
    }

    // Manifest'teki her case için karşılık gelen builder olmalı + digest eşit olmalı.
    for manifest_case in &manifest.cases {
        let built = cases
            .iter()
            .find(|c| c.id == manifest_case.id)
            .unwrap_or_else(|| panic!("case builder bulunamadı: {}", manifest_case.id));
        let actual_digest = blake3_hex(&common::serialize_case_bytes(built));
        assert_eq!(
            actual_digest, manifest_case.builder_digest_blake3,
            "case {} builder digest mismatch — builder drift olmuş; \
             bootstrap helper'ı ile manifest'i güncelle",
            manifest_case.id
        );
        // **Review P2 fix:** metadata eşitliği — builder class/source/description'ı
        // değiştirirse digest zaten değişir (metadata artık serialize_case_bytes'ta),
        // ama explicit equality ek olarak pinler (class/source enum, description string).
        assert_eq!(
            format!("{:?}", built.class),
            format!("{:?}", manifest_case.class),
            "case {} class mismatch (builder vs manifest)",
            manifest_case.id
        );
        assert_eq!(
            format!("{:?}", built.source),
            format!("{:?}", manifest_case.source),
            "case {} source mismatch (builder vs manifest)",
            manifest_case.id
        );
        // **Review tur 3 P1-2 fix:** description equality — builder/manifest senkron.
        assert_eq!(
            built.description, manifest_case.description,
            "case {} description mismatch (builder vs manifest) — manifest stale; \
             builder güncellendikten sonra manifest description'ı da güncelle",
            manifest_case.id
        );

        // **Faz 8-P2 #88 P0 fix (review):** measured-subject digest manifest doğrulaması.
        // DirectPerAxisAuthority/MixedPerAxisSources family'leri için manifest değeri
        // builder'dan yeniden hesaplanan digest ile karşılaştırılır. Diğer family'ler
        // (matching_scope/wide_affected_scope/removed_edge_external_source/
        // delta_introduced_subject) measured-subject digest taşımaz (None).
        match built.class {
            common::CaseClass::DirectPerAxisAuthority | common::CaseClass::MixedPerAxisSources => {
                let actual = common::compute_measured_subject_digest(built).unwrap_or_else(|e| {
                    panic!(
                        "case {} measured-subject digest hesaplanamadı: {e:?}",
                        manifest_case.id
                    )
                });
                let actual_hex = hex::encode(actual);
                assert_eq!(
                    manifest_case.measured_subject_digest_blake3.as_deref(),
                    Some(actual_hex.as_str()),
                    "case {} measured-subject digest mismatch — manifest değeri builder \
                     digest ile eşit olmalı (double-hash hatası düzeltildi); bootstrap \
                     helper'ı ile manifest'i güncelle",
                    manifest_case.id
                );
            }
            _ => {
                assert!(
                    manifest_case.measured_subject_digest_blake3.is_none(),
                    "case {} eski family measured-subject digest taşımamalı (None), got {:?}",
                    manifest_case.id,
                    manifest_case.measured_subject_digest_blake3
                );
            }
        }
    }

    // Ters yönde: builder'da olup manifest'te olmayan case (unpinned) uyar.
    for built in &cases {
        let in_manifest = manifest.cases.iter().any(|c| c.id == built.id);
        assert!(
            in_manifest,
            "case {} builder'da var ama manifest'te yok — manifest'e ekle (bootstrap helper)",
            built.id
        );
    }
}

/// **Faz 8-P2 #88 (MD-2 evidence contract):** DirectPerAxisAuthority + MixedPerAxisSources
/// family guard'ları.
///
/// Her family için:
/// - cardinality (5 direct, 6 mixed)
/// - measured-subject digest presence (her case'de `Some`)
/// - exact-one predicate (digest V1 invariant)
/// - family-içi measured-subject digest uniqueness (aynı subject, farklı declaration)
/// - family-içi full-case digest uniqueness (her declaration ayrı)
/// - exact ID set (BTreeSet — order bağımsız)
/// - exact selector pin (axis + scope)
/// - family-arası measured-subject digest inequality (farklı fiziksel subject)
///
/// Bu guard, `compute_measured_subject_digest`'in production canonical tipleri (SpaceDigest,
/// PredicateAxisTag, CanonicalPredicateScope, CanonicalSubjectScope, CanonicalStructuralDelta)
/// üzerinden tek authority disipliniyle çalıştığını pinler.
#[test]
fn md2_evidence_contract_families_measured_subject_digest_invariants() {
    use common::{build_all_cases, compute_measured_subject_digest, CaseClass};
    use std::collections::BTreeSet;

    let cases = build_all_cases();

    // --- DirectPerAxisAuthority family (5 case) ---
    let direct: Vec<_> = cases
        .iter()
        .filter(|c| c.class == CaseClass::DirectPerAxisAuthority)
        .collect();
    assert_eq!(
        direct.len(),
        5,
        "DirectPerAxisAuthority exact 5 case içermeli"
    );

    // measured-subject digest presence + exact-one predicate.
    for case in &direct {
        let digest = compute_measured_subject_digest(case).unwrap_or_else(|e| {
            panic!(
                "DirectPerAxisAuthority {} measured-subject digest başarısız: {e:?}",
                case.id
            )
        });
        assert!(
            !digest.is_empty(),
            "DirectPerAxisAuthority {} measured-subject digest boş",
            case.id
        );
        assert_eq!(
            case.task.target_predicate_set.predicates.len(),
            1,
            "DirectPerAxisAuthority {} exact-one predicate içermeli",
            case.id
        );
    }

    // family-içi measured-subject digest uniqueness (5 case aynı subject).
    let direct_subject_digests: BTreeSet<String> = direct
        .iter()
        .map(|c| {
            let d = compute_measured_subject_digest(c).unwrap();
            hex::encode(&d)
        })
        .collect();
    assert_eq!(
        direct_subject_digests.len(),
        1,
        "DirectPerAxisAuthority family aynı measured-subject digest'ine sahip olmalı (farklı declaration, aynı subject)"
    );

    // family-içi full-case digest uniqueness (5 farklı declaration → 5 farklı full-case).
    let direct_full_digests: BTreeSet<String> = direct
        .iter()
        .map(|c| common::blake3_hex(&common::serialize_case_bytes(c)))
        .collect();
    assert_eq!(
        direct_full_digests.len(),
        5,
        "DirectPerAxisAuthority her declaration ayrı full-case digest üretmeli"
    );

    // exact ID set (BTreeSet — order bağımsız).
    let direct_ids: BTreeSet<&str> = direct.iter().map(|c| c.id.as_str()).collect();
    let direct_expected: BTreeSet<&str> = [
        "direct-coupling-required-none-001",
        "direct-coupling-required-scip-001",
        "direct-coupling-required-tree-sitter-001",
        "direct-coupling-required-placeholder-001",
        "direct-coupling-required-heuristic-001",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        direct_ids, direct_expected,
        "DirectPerAxisAuthority exact ID set"
    );

    // exact selector pin: Coupling axis, Node(1) scope.
    for case in &direct {
        let pred = &case.task.target_predicate_set.predicates[0].predicate;
        assert_eq!(
            pred.metric,
            common_metric_axis_coupling(),
            "DirectPerAxisAuthority {} Coupling axis olmalı",
            case.id
        );
        assert_eq!(
            pred.scope,
            common_scope_node_1(),
            "DirectPerAxisAuthority {} Node(1) scope olmalı",
            case.id
        );
    }

    // --- MixedPerAxisSources family (6 case) ---
    let mixed: Vec<_> = cases
        .iter()
        .filter(|c| c.class == CaseClass::MixedPerAxisSources)
        .collect();
    assert_eq!(mixed.len(), 6, "MixedPerAxisSources exact 6 case içermeli");

    for case in &mixed {
        let digest = compute_measured_subject_digest(case).unwrap_or_else(|e| {
            panic!(
                "MixedPerAxisSources {} measured-subject digest başarısız: {e:?}",
                case.id
            )
        });
        assert!(
            !digest.is_empty(),
            "MixedPerAxisSources {} measured-subject digest boş",
            case.id
        );
        assert_eq!(
            case.task.target_predicate_set.predicates.len(),
            1,
            "MixedPerAxisSources {} exact-one predicate içermeli",
            case.id
        );
    }

    let mixed_subject_digests: BTreeSet<String> = mixed
        .iter()
        .map(|c| {
            let d = compute_measured_subject_digest(c).unwrap();
            hex::encode(&d)
        })
        .collect();
    assert_eq!(
        mixed_subject_digests.len(),
        1,
        "MixedPerAxisSources family aynı measured-subject digest'ine sahip olmalı"
    );

    let mixed_full_digests: BTreeSet<String> = mixed
        .iter()
        .map(|c| common::blake3_hex(&common::serialize_case_bytes(c)))
        .collect();
    assert_eq!(
        mixed_full_digests.len(),
        6,
        "MixedPerAxisSources her declaration ayrı full-case digest üretmeli"
    );

    let mixed_ids: BTreeSet<&str> = mixed.iter().map(|c| c.id.as_str()).collect();
    let mixed_expected: BTreeSet<&str> = [
        "mixed-cohesion-required-none-001",
        "mixed-cohesion-required-scip-001",
        "mixed-cohesion-required-tree-sitter-001",
        "mixed-cohesion-required-heuristic-001",
        "mixed-cohesion-required-placeholder-001",
        "mixed-cohesion-required-mixed-invalid-001",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        mixed_ids, mixed_expected,
        "MixedPerAxisSources exact ID set"
    );

    // exact selector pin: Cohesion axis, Subgraph([1,2]) scope.
    for case in &mixed {
        let pred = &case.task.target_predicate_set.predicates[0].predicate;
        assert_eq!(
            pred.metric,
            common_metric_axis_cohesion(),
            "MixedPerAxisSources {} Cohesion axis olmalı",
            case.id
        );
        assert_eq!(
            pred.scope,
            common_scope_subgraph_1_2(),
            "MixedPerAxisSources {} Subgraph([1,2]) scope olmalı",
            case.id
        );
    }

    // --- family-arası measured-subject digest inequality (farklı fiziksel subject) ---
    let direct_subject = direct_subject_digests.iter().next().unwrap();
    let mixed_subject = mixed_subject_digests.iter().next().unwrap();
    assert_ne!(
        direct_subject, mixed_subject,
        "Direct ve Mixed family farklı measurement subject'ler — aynı digest olamaz (Coupling/Node(1) vs Cohesion/Subgraph([1,2]))"
    );
}

/// Helper: `PredicateAxis::Coupling` — guard test'inde exact selector assertion için.
fn common_metric_axis_coupling() -> osp_core::trajectory::PredicateAxis {
    osp_core::trajectory::PredicateAxis::Coupling
}

/// Helper: `PredicateAxis::Cohesion`.
fn common_metric_axis_cohesion() -> osp_core::trajectory::PredicateAxis {
    osp_core::trajectory::PredicateAxis::Cohesion
}

/// Helper: `PredicateScope::Node(1)`.
fn common_scope_node_1() -> osp_core::trajectory::PredicateScope {
    osp_core::trajectory::PredicateScope::Node(1)
}

/// Helper: `PredicateScope::Subgraph(vec![1, 2])`.
fn common_scope_subgraph_1_2() -> osp_core::trajectory::PredicateScope {
    osp_core::trajectory::PredicateScope::Subgraph(vec![1, 2])
}

#[test]
fn path_traversal_outside_root_rejected() {
    let root = characterization_root(env!("CARGO_MANIFEST_DIR"));
    // **Review P1-2 fix:** `../../../Cargo.toml` repo root'taki MEVCUT Cargo.toml'a
    // ulaşır → containment guard gerçekten çalışır (PathEscapesRoot). Önceki
    // `../../Cargo.toml` repo/tests/Cargo.toml'a gidiyordu (yok → ReadFailed), yani
    // containment kontrolü hiç çalışmadan test geçiyordu.
    let result = load_fixture_bytes(&root, "../../../Cargo.toml");
    assert!(
        matches!(
            result,
            Err(common::FixtureLoadError::PathEscapesRoot { .. })
        ),
        "root dışına kaçış PathEscapesRoot ile reddedilmeli (ReadFailed DEĞIL — \
         containment guard gerçekten çalışmalı); got {result:?}"
    );
}

#[test]
fn existing_fixture_loads_within_root() {
    let root = characterization_root(env!("CARGO_MANIFEST_DIR"));
    // Meşru fixture load.
    let bytes = load_fixture_bytes(&root, "cases.json").expect("cases.json yüklenmeli");
    assert!(!bytes.is_empty());
}
