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

#[test]
fn path_traversal_outside_root_rejected() {
    let root = characterization_root(env!("CARGO_MANIFEST_DIR"));
    // `../` kaçışı — cases.json parent'ına çıkma denemesi.
    let result = load_fixture_bytes(&root, "../../Cargo.toml");
    assert!(
        matches!(
            result,
            Err(common::FixtureLoadError::PathEscapesRoot { .. })
                | Err(common::FixtureLoadError::ReadFailed { .. })
        ),
        "root dışına kaçış reddedilmeli; got {result:?}"
    );
}

#[test]
fn existing_fixture_loads_within_root() {
    let root = characterization_root(env!("CARGO_MANIFEST_DIR"));
    // Meşru fixture load.
    let bytes = load_fixture_bytes(&root, "cases.json").expect("cases.json yüklenmeli");
    assert!(!bytes.is_empty());
}
