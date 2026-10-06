//! #171 — yazar-ayrımı deneyi hesap-yardımcıları (pre-registration pin'leri).
//!
//! Tasarım notu: `docs/notes/yazar-ayrimi-deney-tasarimi-171.md` (v2, onaylı).
//! Bu modül VERİ YOKKEN pinlenir: hesap formülleri ve körleme türetimleri
//! deneyden ÖNCE koda girer, böylece sonradan yeniden yorumlanamaz. Raporlamaya
//! bağlanması (llm-author.jsonl üretimi) v1'de elle kalır (§7.4).
//!
//! Kapsam:
//! - **D4 imzalı estimand formülleri** (PE_H/PE_L/INT, BE_H/BE_L/BE) + INT == BE
//!   symmetry-check invariant'ı — 2×2 faktöriyelin iki bağımsız formülü aynı
//!   interaction contrast'ı verir; sayısal eşitsizlik yalnız hesaplama hatasıdır.
//! - **§4 blind-order seed türetimi** — `SHA256("llm-author-v1/blind-order" ‖
//!   task_digest ‖ baseline_digest)`; analist sunum sırasını SEÇEMEZ (cherry-pick
//!   yok). Sunum sırası seed'den deterministik türetilir.

#![allow(
    dead_code,
    reason = "#171 v1: hesap-yardımcıları raporlamaya bağlanana dek yalnız \
              operatör/test tarafından kullanılır (§7.4 — satır üretimi elle)"
)]

use sha2::{Digest, Sha256};

// ═══════════════════════════════════════════════════════════════════════════════
// D4 — 2×2 faktöriyel hücreleri ve imzalı contrast'lar
// ═══════════════════════════════════════════════════════════════════════════════

/// 2×2 faktöriyelin dört hücresi — Y(proposal-author, bar-author), her biri
/// hücrenin bar-geçme skoru (v1: binary 0/1; domainler üzerinden oranlanır).
///
/// Adlandırma tasarım notundaki ekseni izler: `y_<proposal-author>_<bar-author>`
/// (`h` = insan, `l` = LLM). Örn. `y_hl` = Y(insan-öneri, LLM-bar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FactorialCellScores {
    /// Y(insan-öneri, insan-bar) — kol A'nın kendi hücresi.
    pub y_hh: f64,
    /// Y(insan-öneri, LLM-bar).
    pub y_hl: f64,
    /// Y(LLM-öneri, insan-bar).
    pub y_lh: f64,
    /// Y(LLM-öneri, LLM-bar) — kol B'nin kendi hücresi.
    pub y_ll: f64,
}

/// İmzalı ve SABİT estimand'lar (D4; deney sonrası eklenmez, yönü değişmez):
///
/// ```text
/// PE_H = Y(LLM-öneri,  insan-bar) − Y(insan-öneri, insan-bar)
/// PE_L = Y(LLM-öneri,  LLM-bar)   − Y(insan-öneri, LLM-bar)
/// INT   = PE_L − PE_H   (pozitif ⇒ proposal-author etkisi LLM barında daha büyük)
///
/// BE_H = Y(insan-öneri, LLM-bar)  − Y(insan-öneri, insan-bar)
/// BE_L = Y(LLM-öneri,  LLM-bar)   − Y(LLM-öneri,  insan-bar)
/// BE    = BE_L − BE_H  (pozitif ⇒ bar-author etkisi LLM önerilerinde daha büyük)
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignedContrasts {
    pub pe_h: f64,
    pub pe_l: f64,
    /// INT = PE_L − PE_H (proposal-author perspektifinden interaction contrast).
    pub int_contrast: f64,
    pub be_h: f64,
    pub be_l: f64,
    /// BE = BE_L − BE_H (bar-author perspektifinden aynı contrast).
    pub be_contrast: f64,
}

/// D4 imzalı formülleri hesapla. Formüller koddan OKUNUR halde sabittir —
/// yeniden adlandırma/ters-çevirme DEĞİŞİKLİK olarak görülmeli ve tasarım
/// notunun yeni bir sürümünü gerektirmelidir.
pub fn signed_contrasts(c: &FactorialCellScores) -> SignedContrasts {
    let pe_h = c.y_lh - c.y_hh;
    let pe_l = c.y_ll - c.y_hl;
    let int_contrast = pe_l - pe_h;
    let be_h = c.y_hl - c.y_hh;
    let be_l = c.y_ll - c.y_lh;
    let be_contrast = be_l - be_h;
    SignedContrasts {
        pe_h,
        pe_l,
        int_contrast,
        be_h,
        be_l,
        be_contrast,
    }
}

/// Symmetry-check invariant'ı toleransı: formüller f64 üzerinde farklı işlem
/// sırasıyla koşar; cebirsel özdeşlik ancak yuvarlama ölçeğinde bozulabilir.
/// Bu tolerans hesap HATASINI (yanlış formül) yakatacak kadar geniş, dürüst
/// f64 accrual'ını yanlış işaretlemeyecek kadar dardır.
pub const SYMMETRY_EPSILON: f64 = 1e-12;

/// **INT == BE symmetry-check invariant'ı** (D4 cebirsel not, merge onayı):
///
/// ```text
/// INT = [Y(L,L) − Y(H,L)] − [Y(L,H) − Y(H,H)]
/// BE  = [Y(L,L) − Y(L,H)] − [Y(H,L) − Y(H,H)]   ⟹   INT == BE
/// ```
///
/// İki formül iki bağımsız etkileşim sonucu gibi raporlanamaz; sayısal
/// eşitsizlik yalnız hesaplama hatasını gösterebilir.
pub fn int_be_symmetry_ok(c: &SignedContrasts) -> bool {
    (c.int_contrast - c.be_contrast).abs() <= SYMMETRY_EPSILON
}

// ═══════════════════════════════════════════════════════════════════════════════
// §4 — blind-order seed + sunum sırası
// ═══════════════════════════════════════════════════════════════════════════════

/// §4 (a): `SHA256("llm-author-v1/blind-order" ‖ task_digest ‖ baseline_digest)`.
///
/// Değiştirilemez girdilerden (task + baseline digest) deterministik türetilir —
/// analist sırayı seçemez, cherry-pick edilebilir order yok. Girdi digest'leri
/// fixed-width (`sha256:<64 hex>`) olduğundan concatenation sınırı belirlidir
/// (tasarım notu v1 notu; şema değişirse length-prefix eklenir).
pub fn blind_order_seed(task_digest: &str, baseline_digest: &str) -> String {
    let domain = "llm-author-v1/blind-order";
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(task_digest.as_bytes());
    hasher.update(baseline_digest.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// §4 (a): seed'den deterministik sunum sırası — `n` öneri için permütasyon.
///
/// Türetim: her indeks `i` için `key_i = SHA256(seed ‖ ":" ‖ i)` ilk 8 baytı
/// (big-endian u64); sıra = key'lere göre artan (eşitlikte indeks). Seed
/// değişmedikçe sıra değişmez; aynı seed farklı n'lerde tutarlı önek sırası
/// vermez (bağımsız key'ler) — yalnızca tam sıra pinlenir.
pub fn blind_presentation_order(n: usize, seed: &str) -> Vec<usize> {
    let mut indexed: Vec<(usize, [u8; 8])> = (0..n)
        .map(|i| {
            let mut hasher = Sha256::new();
            hasher.update(seed.as_bytes());
            hasher.update(b":");
            hasher.update(i.to_string().as_bytes());
            let digest = hasher.finalize();
            let mut key = [0u8; 8];
            key.copy_from_slice(&digest[..8]);
            (i, key)
        })
        .collect();
    indexed.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    indexed.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── D4: imzalı formüller + INT == BE ──────────────────────────────────────

    #[test]
    fn signed_contrasts_hand_computed_example() {
        // El hesabı (tasarım notu formülleriyle):
        //   PE_H = 0.4 − 0.5 = −0.1   PE_L = 0.9 − 0.7 = +0.2   INT = +0.3
        //   BE_H = 0.7 − 0.5 = +0.2   BE_L = 0.9 − 0.4 = +0.5   BE  = +0.3
        let c = signed_contrasts(&FactorialCellScores {
            y_hh: 0.5,
            y_hl: 0.7,
            y_lh: 0.4,
            y_ll: 0.9,
        });
        assert!((c.pe_h - (-0.1)).abs() < 1e-12, "PE_H = {}", c.pe_h);
        assert!((c.pe_l - 0.2).abs() < 1e-12, "PE_L = {}", c.pe_l);
        assert!(
            (c.int_contrast - 0.3).abs() < 1e-12,
            "INT = {}",
            c.int_contrast
        );
        assert!((c.be_h - 0.2).abs() < 1e-12, "BE_H = {}", c.be_h);
        assert!((c.be_l - 0.5).abs() < 1e-12, "BE_L = {}", c.be_l);
        assert!(
            (c.be_contrast - 0.3).abs() < 1e-12,
            "BE = {}",
            c.be_contrast
        );
    }

    #[test]
    fn int_be_symmetry_holds_across_cell_grid() {
        // Cebirsel özdeşlik (D4): iki bağımsız formül her hücre değerinde aynı
        // contrast'ı verir — grid üzerinde invariant.
        let mut checked = 0;
        for &y_hh in &[0.0, 0.25, 0.5, 1.0, 0.3333333333333333] {
            for &y_hl in &[0.0, 0.5, 1.0, 0.6666666666666666] {
                for &y_lh in &[0.1, 0.4, 0.9] {
                    for &y_ll in &[0.2, 0.6, 1.0, 0.125] {
                        let c = signed_contrasts(&FactorialCellScores {
                            y_hh,
                            y_hl,
                            y_lh,
                            y_ll,
                        });
                        assert!(
                            int_be_symmetry_ok(&c),
                            "INT({}) == BE({}) ihlali: {:?}",
                            c.int_contrast,
                            c.be_contrast,
                            (y_hh, y_hl, y_lh, y_ll)
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 100, "grid gerçekten dolaşıldı: {checked}");
    }

    #[test]
    fn symmetry_check_catches_computation_error() {
        // Invariant'ın İŞE YARAMASI: bir formül bozulursa (hesap hatası)
        // symmetry-check kırmızı olmalı. Yanlış contrast enjekte edilerek
        // dedektörün çalıştığı pinlenir.
        let mut c = signed_contrasts(&FactorialCellScores {
            y_hh: 0.5,
            y_hl: 0.7,
            y_lh: 0.4,
            y_ll: 0.9,
        });
        assert!(int_be_symmetry_ok(&c));
        c.int_contrast += 0.01; // hesap hatası simülasyonu
        assert!(!int_be_symmetry_ok(&c), "bozulan formül yakalanmalı");
    }

    #[test]
    fn contrast_directions_are_interpretable() {
        // INT > 0 ⇒ proposal-author etkisi LLM barında daha büyük (D4 imza).
        // Simetrik olmayan hücrelerle yön pinlenir (sonradan yeniden yorumlanamaz).
        let c = signed_contrasts(&FactorialCellScores {
            y_hh: 0.0,
            y_hl: 0.0,
            y_lh: 0.0,
            y_ll: 1.0,
        });
        assert!(c.int_contrast > 0.0, "INT pozitif: {:?}", c);
        assert!(c.be_contrast > 0.0, "BE pozitif (aynı contrast): {:?}", c);
    }

    // ── §4: blind-order seed + sunum sırası ───────────────────────────────────

    const TASK_DIGEST: &str =
        "sha256:1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a";
    const BASELINE_DIGEST: &str =
        "sha256:2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b";

    #[test]
    fn blind_order_seed_known_answer_vector() {
        // KAT: sha256("llm-author-v1/blind-order" + TASK + BASELINE) — py ile
        // bağımsız hesaplandı; formül değişirse bu test kırılır (bilinçli pin).
        let seed = blind_order_seed(TASK_DIGEST, BASELINE_DIGEST);
        assert_eq!(
            seed,
            "sha256:c9177cf8456bad04989c8b2497944e7eebaafed053ccbd7ce76ef19d6b021501"
        );
    }

    #[test]
    fn blind_order_seed_is_input_sensitive_and_deterministic() {
        let seed = blind_order_seed(TASK_DIGEST, BASELINE_DIGEST);
        assert_eq!(seed, blind_order_seed(TASK_DIGEST, BASELINE_DIGEST));
        // Hem task hem baseline digest'i değiştirilemez girdidir — ikisi de
        // sırayı taşır; swap dahi farklı seed üretir (concatenation sıralı).
        assert_ne!(seed, blind_order_seed(BASELINE_DIGEST, TASK_DIGEST));
        let task2 = "sha256:1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1b";
        assert_ne!(seed, blind_order_seed(task2, BASELINE_DIGEST));
    }

    #[test]
    fn presentation_order_is_valid_permutation_and_deterministic() {
        let seed = blind_order_seed(TASK_DIGEST, BASELINE_DIGEST);
        for n in [0usize, 1, 2, 3, 8, 17] {
            let order = blind_presentation_order(n, &seed);
            assert_eq!(order.len(), n);
            let mut sorted = order.clone();
            sorted.sort_unstable();
            assert_eq!(
                sorted,
                (0..n).collect::<Vec<_>>(),
                "n={n}: her indeks tam bir kez (permütasyon)"
            );
            assert_eq!(
                order,
                blind_presentation_order(n, &seed),
                "n={n}: aynı seed aynı sıra"
            );
        }
    }

    #[test]
    fn presentation_order_is_seed_sensitive() {
        // Farklı değiştirilemez girdiler farklı sıra üretir — analist seed'i
        // beğenmesine göre sıra ÇEVRİLEMEZ (seed girdilerden gelir, seçilmez).
        let seed1 = blind_order_seed(TASK_DIGEST, BASELINE_DIGEST);
        let task2 = "sha256:1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1b";
        let seed2 = blind_order_seed(task2, BASELINE_DIGEST);
        assert_ne!(seed1, seed2);
        assert_ne!(
            blind_presentation_order(8, &seed1),
            blind_presentation_order(8, &seed2),
            "farklı seed farklı sunum sırası"
        );
    }
}
