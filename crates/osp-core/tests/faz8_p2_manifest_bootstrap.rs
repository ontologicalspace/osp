//! Faz 8-P2 P2-0B manifest bootstrap — builder digest'lerini hesaplayıp yazdırır.
//!
//! Bu test, initial commit sonrası manifest'teki "TBD_BY_BUILDER_RUN" placeholder'larını
//! gerçek BLAKE3 digest'leriyle güncellemek için **bir kerelik** çalıştırılır.
//!
//! Kullanım:
//! ```sh
//! cargo test -p osp-core --test faz8_p2_manifest_bootstrap -- --nocapture
//! ```
//! Çıktıdaki JSON'u `tests/data/faz8_p2_characterization/cases.json` içine kopyala,
//! sonra `cases.blake3` sidecar'ı güncellemek için `faz8_p2_sidecar_bootstrap`'ı çalıştır.
//!
//! **Normal CI'da bu test no-op'tur** (sadece stdout'a yazdırır, assertion yok).

mod common;

#[test]
fn print_case_digests_for_manifest_update() {
    let digests = common::compute_case_digests();
    println!("=== Faz 8-P2 case digests (cases.json builder_digest_blake3 alanı için) ===");
    println!("[");
    for (id, digest) in &digests {
        println!("  {{ \"id\": \"{id}\", \"builder_digest_blake3\": \"{digest}\" }},");
    }
    println!("]");
    println!("=== Toplam case: {} ===", digests.len());
}

/// **Faz 8-P2 #88:** Measured-subject digest'leri (production canonical tipler üzerinden).
/// `DirectPerAxisAuthority`/`MixedPerAxisSources` family'leri için `cases.json`'daki
/// `measured_subject_digest_blake3` alanını günceller. Eski 5 case digest üretmez (None).
#[test]
fn print_case_measured_subject_digests_for_manifest_update() {
    let digests = common::compute_case_measured_subject_digests();
    println!("=== Faz 8-P2 measured-subject digests (cases.json measured_subject_digest_blake3 alanı için) ===");
    println!("[");
    for (id, digest) in &digests {
        match digest {
            Ok(hex) => {
                println!("  {{ \"id\": \"{id}\", \"measured_subject_digest_blake3\": \"{hex}\" }},")
            }
            Err(e) => println!("  // {id}: measured-subject digest üretilemedi — {e:?}"),
        }
    }
    println!("]");
    println!("=== Toplam case: {} ===", digests.len());
}

#[test]
fn print_sidecar_digest_for_manifest_update() {
    let root = common::characterization_root(env!("CARGO_MANIFEST_DIR"));
    let manifest_bytes = common::load_fixture_bytes(&root, "cases.json")
        .expect("cases.json yüklenmeli (P2-0B.1 ile oluşturuldu)");
    let sidecar_hex = common::blake3_hex(&manifest_bytes);
    println!("=== Faz 8-P2 sidecar digest (cases.blake3 içeriği) ===");
    println!("cases.blake3 dosyasına şu içerikleri yaz (sonunda tek LF):");
    println!("{sidecar_hex}");
}
