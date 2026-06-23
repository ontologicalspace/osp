//! Vizyon vektörü + sapma metrikleri + derived hesap (§5).
//!
//! **Faz 1.7:**
//! - `VisionVector` — elle-deklare vizyon (§5.1, RawPosition)
//! - `DeviationMetric` trait — `theta(&RawPosition, ...)` (inv #4: full Position DEĞİL)
//! - `CosineDeviation` — naif 5-boyutlu kosinüs, commit path'inde (inv #5)
//! - `DiffusionDeviation { t }` — spektral stub (Faz 2 doldurur; analyze-only, inv #5)
//! - `compute_derived()` — Raw + Vizyon → `DerivedPosition` (u, θ, D ayrı — inv #10)

use crate::coords::{DerivedPosition, RawPosition};
use crate::space::Space;

// ═══════════════════════════════════════════════════════════════════════════════
// VisionVector — elle-deklare (§5.1)
// ═══════════════════════════════════════════════════════════════════════════════

/// Projenin hedeflediği ideal konum. **Elle deklare edilir** (mimari kurallar +
/// DDD + NFR'lerden; LLM yalnızca önerir, asla otomatik uygulamaz).
///
/// `RawPosition` sarmalar — derived değil (inv #4). Faz 2 Space Engine parse eder;
/// Faz 5 LLM önerileri yine insan-onayıyla eklenir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisionVector(pub RawPosition);

impl VisionVector {
    pub fn new(raw: RawPosition) -> Self {
        Self(raw)
    }

    /// Inner RawPosition'a erişim.
    pub fn raw(&self) -> &RawPosition {
        &self.0
    }
}

impl From<RawPosition> for VisionVector {
    fn from(raw: RawPosition) -> Self {
        Self(raw)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DeviationMetric trait (inv #4 — compile-time dairesellik koruması)
// ═══════════════════════════════════════════════════════════════════════════════

/// Sapma ölçümü. `theta` **yalnızca `&RawPosition`** alır — `&Position` (full) DEĞİL.
///
/// Bu imza **inv #4**'ü yapısal garanti eder: `u = 1 − θ` ve `θ = deviation(P, V_vision)`
/// daireselliğini tip-seviyesinde önler. `compute_theta(full_P)` yazmak derleme hatası.
///
/// Dönüş: `θ ∈ [0, 1]` normalize sapma. 0 = mükemmel hizalı, 1 = zıt (negatif uzay).
/// Negatif-uzay eşiği (formalism §5.2: `θ ≥ π/2`) → normalize'de `θ ≥ 0.5`.
pub trait DeviationMetric: Send + Sync {
    fn theta(&self, raw: &RawPosition, vision: &VisionVector, space: &Space) -> f64;
}

// ═══════════════════════════════════════════════════════════════════════════════
// CosineDeviation — naif baseline (inv #5: commit path'inde kullanılır)
// ═══════════════════════════════════════════════════════════════════════════════

/// Naif 5-boyutlu kosinüs sapması. Hızlı (`O(5)`), commit akışında güvenli (inv #5).
///
/// `θ = (1 − cos_sim) / 2` mapping: `cos_sim ∈ [-1, 1]` → `θ ∈ [0, 1]`.
/// - `cos_sim = 1` (aynı yön) → `θ = 0` (mükemmel hizalı)
/// - `cos_sim = 0` (ortogonal) → `θ = 0.5` (negatif-uzay eşiği)
/// - `cos_sim = -1` (zıt) → `θ = 1` (maksimum sapma)
#[derive(Debug, Clone, Copy, Default)]
pub struct CosineDeviation;

impl DeviationMetric for CosineDeviation {
    fn theta(&self, raw: &RawPosition, vision: &VisionVector, _space: &Space) -> f64 {
        cosine_normalized(raw, &vision.0)
    }
}

/// 5-boyutlu kosinüs benzerliği → normalize sapma [0,1].
fn cosine_normalized(p: &RawPosition, v: &RawPosition) -> f64 {
    let dot = p.x * v.x + p.y * v.y + p.z * v.z + p.w * v.w + p.v * v.v;
    let norm_p = (p.x * p.x + p.y * p.y + p.z * p.z + p.w * p.w + p.v * p.v).sqrt();
    let norm_v = (v.x * v.x + v.y * v.y + v.z * v.z + v.w * v.w + v.v * v.v).sqrt();
    if norm_p < 1e-9 || norm_v < 1e-9 {
        return 1.0; // sıfır vektör → maksimum sapma (tanımsız, conservative)
    }
    let cos_sim = (dot / (norm_p * norm_v)).clamp(-1.0, 1.0);
    ((1.0 - cos_sim) / 2.0).clamp(0.0, 1.0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// DiffusionDeviation — spektral stub (Faz 2, inv #5: analyze-only)
// ═══════════════════════════════════════════════════════════════════════════════

/// Diffusion distance — spektral Laplacian tabanlı (§5.3).
///
/// **Faz 1.7: stub.** Gerçek hesap (`K_t = e^{−tL}`, `θ = ||f_P − f_V||₂`) Faz 2'de.
/// Şimdilik `todo!()` panik — kimse commit path'inde çağırmamalı (inv #5).
/// `commit()` her zaman `CosineDeviation` kullanır; `DiffusionDeviation` yalnızca
/// `osp analyze` / dashboard context (Faz 2).
#[derive(Debug, Clone, Copy)]
pub struct DiffusionDeviation {
    /// Diffusion zaman-parametresi (§5.3). `t* = 1/λ₂` aday (Faz 1.11 kalibrasyon).
    pub t: f64,
}

impl DeviationMetric for DiffusionDeviation {
    fn theta(&self, _raw: &RawPosition, _vision: &VisionVector, _space: &Space) -> f64 {
        // Faz 2: graph Laplacian L = D − A; K_t = e^{−tL}; θ = ||K_t·δ_P − K_t·δ_V||₂
        todo!("DiffusionDeviation::theta — Faz 2 implementasyonu (spektral Laplacian)")
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// compute_derived — Raw + Vizyon → DerivedPosition (inv #10: D ayrı)
// ═══════════════════════════════════════════════════════════════════════════════

/// `RawPosition` + `VisionVector` → `DerivedPosition` hesapla.
///
/// - `theta` = metric.theta(raw, vision, space) ∈ [0,1]
/// - `u` = `(1 − theta).clamp(0, 1)` (vision alignment)
/// - `main_sequence_distance` = `|A + I − 1|` (Martin, inv #10 — z'ye gömülü DEĞİL)
/// - `risk_score` = 0.0 (Faz 2 composite risk)
///
/// `instability` (I) ve `abstractness` (A) ayrı parametreler: I `raw.z`'den okunabilir
/// (Faz 1.9 InstabilityAxis) AMA A bir raw eksen DEĞİL — D için ayrı gerekir. Explicit
/// parametreler compute_derived'i Faz 1.4-1.8 (raw.z=0) için de çalıştırır.
pub fn compute_derived(
    raw: &RawPosition,
    vision: &VisionVector,
    space: &Space,
    metric: &dyn DeviationMetric,
    instability: f64,
    abstractness: f64,
) -> DerivedPosition {
    let theta = metric.theta(raw, vision, space);
    let u = (1.0 - theta).clamp(0.0, 1.0);
    let d = (abstractness + instability - 1.0).abs().clamp(0.0, 1.0);
    DerivedPosition {
        u,
        theta,
        risk_score: 0.0, // Faz 2
        main_sequence_distance: d,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(x: f64, y: f64, z: f64, w: f64, v: f64) -> RawPosition {
        RawPosition { x, y, z, w, v }
    }

    // --- VisionVector ---

    #[test]
    fn vision_vector_wraps_raw_position() {
        let vv = VisionVector::new(raw(0.5, 0.5, 0.5, 0.5, 0.5));
        assert_eq!(vv.raw(), &raw(0.5, 0.5, 0.5, 0.5, 0.5));
    }

    #[test]
    fn vision_vector_from_raw() {
        let vv: VisionVector = raw(0.1, 0.2, 0.3, 0.4, 0.5).into();
        assert!((vv.raw().x - 0.1).abs() < 1e-9);
    }

    // --- CosineDeviation ---

    #[test]
    fn cosine_aligned_is_zero() {
        // raw == vision → cos_sim = 1 → θ = 0
        let v = raw(0.5, 0.5, 0.5, 0.5, 0.5);
        let vision = VisionVector(v);
        let theta = CosineDeviation.theta(&v, &vision, &Space::new());
        assert!(theta.abs() < 1e-9, "aligned θ = {}", theta);
    }

    #[test]
    fn cosine_opposite_is_near_one() {
        // raw = -vision → cos_sim = -1 → θ = 1
        let v = raw(1.0, 1.0, 1.0, 1.0, 1.0);
        let opposite = raw(-1.0, -1.0, -1.0, -1.0, -1.0);
        let vision = VisionVector(v);
        let theta = CosineDeviation.theta(&opposite, &vision, &Space::new());
        assert!((theta - 1.0).abs() < 1e-9, "opposite θ = {}", theta);
    }

    #[test]
    fn cosine_orthogonal_is_half() {
        // cos_sim = 0 → θ = 0.5 (negatif-uzay eşiği)
        let v = raw(1.0, 0.0, 0.0, 0.0, 0.0);
        let ortho = raw(0.0, 1.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(v);
        let theta = CosineDeviation.theta(&ortho, &vision, &Space::new());
        assert!((theta - 0.5).abs() < 1e-9, "orthogonal θ = {}", theta);
    }

    #[test]
    fn cosine_zero_vector_returns_max_deviation() {
        // norm=0 → tanımsız → conservative maksimum (1.0)
        let zero = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(raw(0.5, 0.5, 0.5, 0.5, 0.5));
        let theta = CosineDeviation.theta(&zero, &vision, &Space::new());
        assert!((theta - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_symmetric_under_scale() {
        // kosinüs ölçek-bağımsız: 2x raw aynı θ
        let v = raw(0.5, 0.5, 0.5, 0.5, 0.5);
        let scaled = raw(1.0, 1.0, 1.0, 1.0, 1.0);
        let vision = VisionVector(v);
        let t1 = CosineDeviation.theta(&v, &vision, &Space::new());
        let t2 = CosineDeviation.theta(&scaled, &vision, &Space::new());
        assert!((t1 - t2).abs() < 1e-9);
    }

    // --- inv #4: DeviationMetric imzası RawPosition alır (compile-time garanti) ---

    #[test]
    fn deviation_metric_trait_compiles_with_raw_position() {
        // Bu test derlendiği sürece inv #4 geçerli: theta &RawPosition alır.
        // theta(&Position) denendiğinde derleme hatası (tip uyumsuzluğu).
        fn check<M: DeviationMetric>(m: &M, r: &RawPosition, v: &VisionVector, s: &Space) -> f64 {
            m.theta(r, v, s)
        }
        let m = CosineDeviation;
        let r = raw(0.5, 0.5, 0.5, 0.5, 0.5);
        let v = VisionVector(r);
        let theta = check(&m, &r, &v, &Space::new());
        assert!(theta.is_finite());
    }

    // --- compute_derived (inv #10: D ayrı) ---

    #[test]
    fn compute_derived_u_is_one_minus_theta() {
        let r = raw(1.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(raw(1.0, 0.0, 0.0, 0.0, 0.0)); // aligned → θ=0 → u=1
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.5, 0.5);
        assert!((d.theta).abs() < 1e-9);
        assert!((d.u - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_d_zero_on_main_sequence() {
        // A + I = 1 → D = 0 (Martin ideal çizgi)
        let r = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        // I=0.3, A=0.7 → A+I=1.0 → D=0
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.3, 0.7);
        assert!(d.main_sequence_distance.abs() < 1e-9, "D = {}", d.main_sequence_distance);
    }

    #[test]
    fn compute_derived_d_max_at_zone_of_pain() {
        // Zone of Pain: A=0, I=0 (concrete + rigid) → D = |0+0-1| = 1
        let r = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.0, 0.0);
        assert!((d.main_sequence_distance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_d_max_at_zone_of_uselessness() {
        // Zone of Uselessness: A=1, I=1 (abstract + unstable) → D = |1+1-1| = 1
        let r = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 1.0, 1.0);
        assert!((d.main_sequence_distance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_d_separate_from_z() {
        // inv #10 — D ayrı field; z (instability) raw'da. compute_derived ikisini ayrı tutar.
        let r = raw(0.0, 0.0, 0.4, 0.0, 0.0); // z = I = 0.4 (raw'da)
        let vision = VisionVector(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        // instability param = 0.4 (raw.z ile aynı), abstractness = 0.3
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.4, 0.3);
        // D = |0.3 + 0.4 - 1| = 0.3
        assert!((d.main_sequence_distance - 0.3).abs() < 1e-9, "D = {}", d.main_sequence_distance);
        // z (raw) değişmedi — compute_derived raw'yı mutate etmez
        assert!((r.z - 0.4).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_orthogonal_negative_space() {
        // θ = 0.5 (orthogonal) → negatif-uzay eşiği; u = 0.5
        let r = raw(1.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector(raw(0.0, 1.0, 0.0, 0.0, 0.0));
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.5, 0.5);
        assert!((d.theta - 0.5).abs() < 1e-9);
        assert!((d.u - 0.5).abs() < 1e-9);
    }

    // --- DiffusionDeviation stub (Faz 2) ---

    #[test]
    #[should_panic(expected = "DiffusionDeviation::theta")]
    fn diffusion_deviation_is_faz2_stub() {
        // inv #5 — DiffusionDeviation commit path'inde değil; Faz 2 doldurana kadar todo!.
        let d = DiffusionDeviation { t: 1.0 };
        let _ = d.theta(&raw(0.5, 0.5, 0.5, 0.5, 0.5), &VisionVector(raw(0.5, 0.5, 0.5, 0.5, 0.5)), &Space::new());
    }
}
