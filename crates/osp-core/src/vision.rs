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
// VisionSource — vision vektörünün provenance'ı (epistemolojik dürüstlük)
// ═══════════════════════════════════════════════════════════════════════════════

/// Bir `VisionVector`'ün nereden geldiğini belirten provenance etiketi.
///
/// MetricValue'daki (source, confidence, coverage) provenance modelinin vision
/// için karşılığıdır (§3.2). UI "Vision: not loaded" ile "θ = 0.04 pass" çelişkisini
/// çözer: bir node'un θ değerinin hangi vision kaynağına dayandığı açık olmalıdır.
///
/// Güven hiyerarşisi (yüksek → düşük):
/// - `UserLoaded` — kullanıcı TOML `[raw]` ile elle deklare etti (en yüksek otorite)
/// - `RoleProfile` — kullanıcı TOML `[role_overrides.<Role>]` ile role-specific
/// - `GlobalDefault` — engine hardcoded default (analiz için, gate için değil)
/// - `BuiltinRole` — `VisionConfig::builtin_role_override` hardcoded sensible default
/// - `None` — vision yüklenmemiş; θ HESAPLANMAMALI (topology-only mod)
///
/// `confidence` alanı risk_score (§3.2, Faz 2 stub) için girdidir:
/// UserLoaded=1.0, RoleProfile=0.9, GlobalDefault=0.5, BuiltinRole=0.6, None=0.0.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum VisionSource {
    /// Vision yüklenmemiş. θ hesaplanmamalı (topology-only mod). "not loaded" UI etiketi.
    #[default]
    None,
    /// Engine hardcoded global default — analiz/görselleştirme içindir, gate için
    /// kullanıcı onayı gerekir. "global default" UI etiketi.
    GlobalDefault,
    /// `VisionConfig::builtin_role_override` — role-specific hardcoded sensible default.
    /// "role-default (builtin)" UI etiketi.
    BuiltinRole,
    /// Kullanıcı TOML `[role_overrides.<Role>]` — role-specific, user-declared.
    /// "role profile" UI etiketi.
    RoleProfile,
    /// Kullanıcı TOML `[raw]` — global, elle deklare (en yüksek otorite).
    /// "user loaded" UI etiketi.
    UserLoaded,
}

impl VisionSource {
    /// Provenance'a bağlı güven skoru ∈ [0, 1] (risk_score girdisi, Faz 2).
    pub fn confidence(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::GlobalDefault => 0.5,
            Self::BuiltinRole => 0.6,
            Self::RoleProfile => 0.9,
            Self::UserLoaded => 1.0,
        }
    }

    /// θ hesaplaması anlamlı mı? `None` → hayır (topology-only); diğerleri → evet.
    pub fn is_evaluable(self) -> bool {
        !matches!(self, Self::None)
    }

    /// UI etiketi (frontend için human-readable).
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "not loaded",
            Self::GlobalDefault => "global default",
            Self::BuiltinRole => "role-default (builtin)",
            Self::RoleProfile => "role profile",
            Self::UserLoaded => "user loaded",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VisionVector — elle-deklare (§5.1) + provenance
// ═══════════════════════════════════════════════════════════════════════════════

/// Projenin hedeflediği ideal konum. **Elle deklare edilir** (mimari kurallar +
/// DDD + NFR'lerden; LLM yalnızca önerir, asla otomatik uygulamaz).
///
/// `RawPosition` sarmalar — derived değil (inv #4). Faz 2 Space engine parse eder;
/// Faz 5 LLM önerileri yine insan-onayıyla eklenir.
///
/// `source` alanı provenance taşır (VisionSource) — "Vision: not loaded" iken
/// θ hesaplanması çelişkisini tip-seviyesinde çözer. Eski kod `VisionVector::new(raw)`
/// ile `VisionSource::GlobalDefault` varsayalım (backward-compat; gerçek kullanıcı
/// vision'ı `with_source` ile işaretlenmeli).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VisionVector {
    pub raw: RawPosition,
    #[serde(default = "default_global_source")]
    pub source: VisionSource,
}

fn default_global_source() -> VisionSource {
    VisionSource::GlobalDefault
}

impl VisionVector {
    /// Legacy constructor — source bilinmiyorsa `GlobalDefault` varsay.
    /// Yeni kod `with_source()` kullanmalı; vision yüklenmemişse `none()`.
    pub fn new(raw: RawPosition) -> Self {
        Self {
            raw,
            source: VisionSource::GlobalDefault,
        }
    }

    /// Belirli bir provenance ile kur.
    pub fn with_source(raw: RawPosition, source: VisionSource) -> Self {
        Self { raw, source }
    }

    /// Vision yok (topology-only mod). raw = zero; θ hesaplanmamalı.
    pub fn none() -> Self {
        Self {
            raw: RawPosition::default(),
            source: VisionSource::None,
        }
    }

    /// Inner RawPosition'a erişim.
    pub fn raw(&self) -> &RawPosition {
        &self.raw
    }

    /// Provenance etiketi.
    pub fn source(&self) -> VisionSource {
        self.source
    }

    /// θ bu vision'a karşı hesaplanabilir mi? (None → false)
    pub fn is_evaluable(&self) -> bool {
        self.source.is_evaluable()
    }
}

impl Default for VisionVector {
    fn default() -> Self {
        Self::none()
    }
}

impl From<RawPosition> for VisionVector {
    fn from(raw: RawPosition) -> Self {
        Self::new(raw)
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
        // inv: source=None → θ hesaplanmamalı. Conservative: maksimum sapma döndür
        // (çift-katmanlı koruma — caller zaten is_evaluable() kontrol etmeli).
        if !vision.is_evaluable() {
            return 1.0;
        }
        cosine_normalized(raw, &vision.raw)
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
        // risk_score burada DEĞİL — compute_risk_score ayrı bir analiz-seviyesi
        // fonksiyondur. compute_derived commit path'inde (inv #5) kullanılır;
        // orada NodeWitness'e (git history) erişim yok. risk_score pipeline'ın
        // analiz katmanında compute_risk_score ile doldurulur.
        risk_score: 0.0,
        main_sequence_distance: d,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// compute_risk_score — composite risk (§3.2): vision θ × node witness
// ═══════════════════════════════════════════════════════════════════════════════

/// Composite risk için sabit kalibrasyon ağırlıkları (§3.2).
///
/// Toplamları 1.0:
/// - `W_VISION` (0.40) — mimari sapma θ (en güçlü sinyal)
/// - `W_VOLATILITY` (0.20) — commits_touching / C_MAX (çok dokunulan = hareketli)
/// - `W_RECENCY` (0.15) — yakın zamanda değişen (e-folding 90 gün)
/// - `W_SOLO` (0.15) — solo authorship (bus-factor)
/// - `W_SPECULATIVE` (0.10) — az dokunulan (yeni/olgunlaşmamış)
///
/// Bu sabitler Faz 1.11 kalibrasyonuna kadar donmuş; sonra `EngineConfig`'e
/// taşınabilir. Ağırlıkların **insan-okur** kalması kasıtlı — Inspector'da
/// risk scalar olarak DEĞİL, `RiskBreakdown` bileşenleriyle gösterilir
/// (gamification tuzağı, roadmap dersi #5).
pub mod risk_weights {
    pub const VISION: f64 = 0.40;
    pub const VOLATILITY: f64 = 0.20;
    pub const RECENCY: f64 = 0.15;
    pub const SOLO: f64 = 0.15;
    pub const SPECULATIVE: f64 = 0.10;
    /// Volatility normalizasyon eşiği — commits_touching >= C_MAX → 1.0.
    pub const C_MAX: f64 = 50.0;
    /// Recency e-folding sabiti (gün). 90 gün ≈ ~3 ay sprint-döngü.
    pub const RECENCY_TAU: f64 = 90.0;
}

/// Risk'in bileşen bazında dökümü — Inspector/scatter için insan-okur.
///
/// `compute_risk_score` hem toplamı hem dökümü döndürür. UI toplamı scalar
/// olarak göstermez (gamification); bunun yerine bu breakdown'u bar grafiği
/// olarak gösterir. Her bileşen ∈ [0,1].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RiskBreakdown {
    /// Mimari sapma θ ∈ [0,1] — vision'a uzaklık.
    pub vision: f64,
    /// `min(1, commits_touching / C_MAX)` — hareketlilik.
    pub volatility: f64,
    /// `exp(-days_ago / TAU)` — yakın değişiklik = yüksek.
    pub recency: f64,
    /// `ownership_concentration` — solo authorship riski.
    pub solo: f64,
    /// `commits_touching <= 2 ? 0.8 : 0.2` — yeni/az-dokunulmuş.
    pub speculative: f64,
}

impl RiskBreakdown {
    /// Ağırlıklı toplam → composite risk_score ∈ [0,1].
    pub fn total(self) -> f64 {
        use risk_weights as w;
        (w::VISION * self.vision
            + w::VOLATILITY * self.volatility
            + w::RECENCY * self.recency
            + w::SOLO * self.solo
            + w::SPECULATIVE * self.speculative)
            .clamp(0.0, 1.0)
    }
}

/// Composite risk hesabı (§3.2): vision θ × node witness × vision-confidence.
///
/// **Üç mod:**
/// - `witness=None` → "ölçülmemiş" epistemolojik durum. risk neutral 0.5
///   (placeholder cohesion'a paralel — "bilmiyoruz" sessiz varsayım değil).
///   `RiskBreakdown` tüm alanlar neutral (0.5).
/// - `witness=Some` + `vision_confidence=0.0` (VisionSource::None) → vision
///   ölçülmemiş. vision bileşeni 0, witness bileşenleri ölçülü. "topology-only"
///   risk: sadece historical stability.
/// - `witness=Some` + `vision_confidence>0` → tam composite.
///
/// `vision_confidence` `VisionSource::confidence()`'dan gelir (§3.2 girdi).
/// vision ölçülmemişse vision bileşeni risk'i şişirmemeli — confidence 0 →
/// vision katkısı 0.
pub fn compute_risk_score(
    theta: f64,
    witness: Option<&crate::space::NodeWitness>,
    vision_confidence: f64,
) -> (f64, RiskBreakdown) {
    let theta = theta.clamp(0.0, 1.0);
    let vision_confidence = vision_confidence.clamp(0.0, 1.0);

    // Vision bileşeni: θ × confidence. confidence 0 (ölçülmemiş vision) → 0.
    // Bu "Vision: not loaded" + "high risk" çelişkisini önler.
    let vision_component = theta * vision_confidence;

    match witness {
        None => {
            // Ölçülmemiş (git yok). Neutral — "bilmiyoruz".
            let neutral = RiskBreakdown {
                vision: vision_component,
                volatility: 0.5,
                recency: 0.5,
                solo: 0.5,
                speculative: 0.5,
            };
            (neutral.total(), neutral)
        }
        Some(w) => {
            use risk_weights as rw;
            let volatility = (w.commits_touching as f64 / rw::C_MAX).min(1.0);
            let recency = (-(w.last_modified_days_ago as f64) / rw::RECENCY_TAU).exp();
            let solo = w.ownership_concentration.clamp(0.0, 1.0);
            let speculative = if w.commits_touching <= 2 { 0.8 } else { 0.2 };
            let breakdown = RiskBreakdown {
                vision: vision_component,
                volatility,
                recency,
                solo,
                speculative,
            };
            (breakdown.total(), breakdown)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VisionVerdict — node analiz seviyesi karar semantiği (#5)
// ═══════════════════════════════════════════════════════════════════════════════

/// Bir node'un vision'a göre değerlendirme sonucu — analiz seviyesi (commit
/// pipeline'dan ayrı). Review'in #4 "advisory ayrı state" önerisinin formal hali.
///
/// **Önemli kapsam ayrımı:** Bu enum **commit pipeline Q5 gate'inin yerini almaz**.
/// Q5 (`check_claim_vision`) claim-based hard reject üretir (`EngineCommitError`).
/// `VisionVerdict` ise **repo-wide node analizi** içindir — her node'un mevcut
/// vision'a göre durumu (inspector, scatter coloring, breakdown). Aynı metrik
/// farklı rolde farklı anlama gelir:
///
/// - `Pass` — θ ≤ θ_bound, sapma kabul edilebilir (yeşil)
/// - `Warning` — θ_bound < θ ≤ θ_warn, boundary'ye yakın (sarı)
/// - `Advisory` — θ > θ_bound AMA node bir Support rolünde (test/fixture/migration).
///   Sapma "gerçek" ama bu rol bağlamında beklenen/normal. Hard fail DEĞİL.
/// - `Reject` — θ > θ_bound, Production/Core node (kırmızı)
/// - `Inconclusive` — θ hesaplanamadı (vision source=None, veya cohesion placeholder
///   iken θ sonucu düşük güven — "we don't know" epistemolojik dürüstlük)
///
/// Advisory, Warning'den farklıdır: Warning "dikkat, sorun olabilir" der; Advisory
/// ise "metrik sapıyor ama rol bağlamında normal olabilir" der. Bu ayrım OSP'nin
/// "aynı metrik farklı rolde farklı anlama gelir" iddiasının formal gövdesidir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VisionVerdict {
    /// θ ≤ θ_bound — kabul edilebilir sapma.
    Pass,
    /// θ_bound < θ ≤ θ_warn — boundary'ye yakın, dikkat.
    Warning,
    /// θ > θ_bound AMA Support rolünde (test/fixture/migration/generated/config/script).
    /// Sapma gerçek ama rol bağlamında beklenen. Hard fail değil, advisory.
    Advisory,
    /// θ > θ_warn — Production/Core/Adapter/Utility/Runtime node, sapma gerçek risk.
    Reject,
    /// θ hesaplanamadı (vision source=None) veya düşük güven (cohesion placeholder).
    /// "we don't know" — fail/pass kararı verilmez.
    Inconclusive,
}

impl VisionVerdict {
    /// UI etiketi (frontend için human-readable).
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warning => "warning",
            Self::Advisory => "advisory",
            Self::Reject => "reject",
            Self::Inconclusive => "inconclusive",
        }
    }

    /// UI rengi (frontend ile aynı değerler — tek kaynak).
    pub fn color_hex(self) -> &'static str {
        match self {
            Self::Pass => "#3fb950",         // yeşil
            Self::Warning => "#d29922",      // amber/sarı
            Self::Advisory => "#d29922",     // amber (warning ile aynı aile, ama farklı anlam)
            Self::Reject => "#f85149",       // kırmızı
            Self::Inconclusive => "#8b949e", // gri
        }
    }

    /// Bu verdict hard-reject mi? (Pass/Warning/Advisory/Inconclusive → false).
    /// Commit pipeline bu bilgiyi kullanmaz (kendi Q5 gate'i var); bu sadece
    /// analiz seviyesi "kaç ciddi sapma var?" sorusu için.
    pub fn is_hard_reject(self) -> bool {
        matches!(self, Self::Reject)
    }
}

/// Bir node'un vision sapma değerlendirmesi — analiz seviyesi `VisionVerdict` üretir.
///
/// **Parametreler:**
/// - `theta`: node'un vision'a sapması (CosineDeviation çıktısı)
/// - `theta_bound`: kabul eşiği (Q5 gate ile aynı, default 0.3)
/// - `theta_warn`: warning eşiği (theta_bound ile theta_bound arası warning).
///   Genelde `theta_bound`'a çok yakın, örn. bound − 0.05.
/// - `is_support_role`: node Support rolünde mi? (test/fixture/migration → true).
///   Advisory downgrade sadece Support için geçerli.
/// - `is_inconclusive`: cohesion placeholder veya vision source=None → true.
///
/// **Karar hiyerarşisi:**
/// 1. `is_inconclusive` → Inconclusive (en yüksek öncelik — "we don't know")
/// 2. theta ≤ theta_bound → Pass
/// 3. theta_bound < theta ≤ theta_warn → Warning
/// 4. theta > theta_warn AND is_support_role → Advisory (downgrade)
/// 5. theta > theta_warn AND !is_support_role → Reject
///
/// Bu, frontend'in soft-coded `advisoryTypes` array + `isAdvisory` mantığının
/// formal tip-seviyesi karşılığıdır. Frontend artık bu fonksiyonu çağırır (veya
/// aynı mantığı mirror'lar).
pub fn evaluate_node_vision(
    theta: f64,
    theta_bound: f64,
    theta_warn: f64,
    is_support_role: bool,
    is_inconclusive: bool,
) -> VisionVerdict {
    use VisionVerdict as V;
    // 1. En yüksek öncelik: ölçülmemişse karar verme.
    if is_inconclusive {
        return V::Inconclusive;
    }
    // 2-3. Pass / Warning boundary'leri.
    if theta <= theta_bound {
        return V::Pass;
    }
    if theta <= theta_warn {
        return V::Warning;
    }
    // 4-5. Üst sınır aşımı: Support → advisory downgrade, diğerleri → reject.
    if is_support_role {
        V::Advisory
    } else {
        V::Reject
    }
}

#[cfg(test)]
mod verdict_tests {
    use super::*;

    #[test]
    fn verdict_pass_when_theta_within_bound() {
        let v = evaluate_node_vision(0.10, 0.30, 0.40, false, false);
        assert_eq!(v, VisionVerdict::Pass);
    }

    #[test]
    fn verdict_warning_between_bound_and_warn() {
        // theta=0.35, bound=0.30, warn=0.40 → warning bandı içinde
        let v = evaluate_node_vision(0.35, 0.30, 0.40, false, false);
        assert_eq!(v, VisionVerdict::Warning);
    }

    #[test]
    fn verdict_reject_for_production_above_warn() {
        // Production node, θ > warn → reject (hard)
        let v = evaluate_node_vision(0.50, 0.30, 0.40, false, false);
        assert_eq!(v, VisionVerdict::Reject);
        assert!(v.is_hard_reject());
    }

    #[test]
    fn verdict_advisory_for_support_above_warn() {
        // Support (test) node, θ > warn → advisory (downgrade, hard değil)
        let v = evaluate_node_vision(0.50, 0.30, 0.40, true, false);
        assert_eq!(v, VisionVerdict::Advisory);
        assert!(!v.is_hard_reject(), "advisory is NOT a hard reject");
    }

    #[test]
    fn verdict_inconclusive_overrides_everything() {
        // Cohesion placeholder iken θ ne olursa olsun → inconclusive.
        // "we don't know" en yüksek öncelik.
        let v_pass = evaluate_node_vision(0.10, 0.30, 0.40, false, true);
        let v_high = evaluate_node_vision(0.90, 0.30, 0.40, true, true);
        assert_eq!(v_pass, VisionVerdict::Inconclusive);
        assert_eq!(v_high, VisionVerdict::Inconclusive);
    }

    #[test]
    fn verdict_boundary_theta_equals_bound_is_pass() {
        // θ == θ_bound → pass (≤ bound).
        let v = evaluate_node_vision(0.30, 0.30, 0.40, false, false);
        assert_eq!(v, VisionVerdict::Pass);
    }

    #[test]
    fn verdict_boundary_theta_equals_warn_is_warning() {
        // θ == θ_warn → warning (≤ warn).
        let v = evaluate_node_vision(0.40, 0.30, 0.40, false, false);
        assert_eq!(v, VisionVerdict::Warning);
    }

    #[test]
    fn verdict_labels_and_colors_distinct() {
        use VisionVerdict as V;
        let cases = [V::Pass, V::Warning, V::Advisory, V::Reject, V::Inconclusive];
        // etiketler benzersiz
        let labels: Vec<&str> = cases.iter().map(|v| v.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        for w in sorted.windows(2) {
            assert_ne!(w[0], w[1]);
        }
        // reject hard, diğerleri değil
        assert!(V::Reject.is_hard_reject());
        assert!(!V::Pass.is_hard_reject());
        assert!(!V::Advisory.is_hard_reject());
        assert!(!V::Inconclusive.is_hard_reject());
    }

    #[test]
    fn verdict_advisory_vs_warning_semantic_distinction() {
        // Aynı θ=0.50: warning değil (≤ warn değil), advisory (support) veya reject.
        // Bu üç durum aynı θ için rol'e göre farklı verdict → OSP'nin "aynı metrik
        // farklı rolde farklı anlam" iddiasının formal kanıtı.
        let support = evaluate_node_vision(0.50, 0.30, 0.40, true, false);
        let production = evaluate_node_vision(0.50, 0.30, 0.40, false, false);
        assert_eq!(support, VisionVerdict::Advisory);
        assert_eq!(production, VisionVerdict::Reject);
        assert_ne!(
            support, production,
            "same θ, different role → different verdict"
        );
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
        let vision = VisionVector::new(v);
        let theta = CosineDeviation.theta(&v, &vision, &Space::new());
        assert!(theta.abs() < 1e-9, "aligned θ = {}", theta);
    }

    #[test]
    fn cosine_opposite_is_near_one() {
        // raw = -vision → cos_sim = -1 → θ = 1
        let v = raw(1.0, 1.0, 1.0, 1.0, 1.0);
        let opposite = raw(-1.0, -1.0, -1.0, -1.0, -1.0);
        let vision = VisionVector::new(v);
        let theta = CosineDeviation.theta(&opposite, &vision, &Space::new());
        assert!((theta - 1.0).abs() < 1e-9, "opposite θ = {}", theta);
    }

    #[test]
    fn cosine_orthogonal_is_half() {
        // cos_sim = 0 → θ = 0.5 (negatif-uzay eşiği)
        let v = raw(1.0, 0.0, 0.0, 0.0, 0.0);
        let ortho = raw(0.0, 1.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(v);
        let theta = CosineDeviation.theta(&ortho, &vision, &Space::new());
        assert!((theta - 0.5).abs() < 1e-9, "orthogonal θ = {}", theta);
    }

    #[test]
    fn cosine_zero_vector_returns_max_deviation() {
        // norm=0 → tanımsız → conservative maksimum (1.0)
        let zero = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(raw(0.5, 0.5, 0.5, 0.5, 0.5));
        let theta = CosineDeviation.theta(&zero, &vision, &Space::new());
        assert!((theta - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_symmetric_under_scale() {
        // kosinüs ölçek-bağımsız: 2x raw aynı θ
        let v = raw(0.5, 0.5, 0.5, 0.5, 0.5);
        let scaled = raw(1.0, 1.0, 1.0, 1.0, 1.0);
        let vision = VisionVector::new(v);
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
        let v = VisionVector::new(r);
        let theta = check(&m, &r, &v, &Space::new());
        assert!(theta.is_finite());
    }

    // --- compute_derived (inv #10: D ayrı) ---

    #[test]
    fn compute_derived_u_is_one_minus_theta() {
        let r = raw(1.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(raw(1.0, 0.0, 0.0, 0.0, 0.0)); // aligned → θ=0 → u=1
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.5, 0.5);
        assert!((d.theta).abs() < 1e-9);
        assert!((d.u - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_d_zero_on_main_sequence() {
        // A + I = 1 → D = 0 (Martin ideal çizgi)
        let r = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        // I=0.3, A=0.7 → A+I=1.0 → D=0
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.3, 0.7);
        assert!(
            d.main_sequence_distance.abs() < 1e-9,
            "D = {}",
            d.main_sequence_distance
        );
    }

    #[test]
    fn compute_derived_d_max_at_zone_of_pain() {
        // Zone of Pain: A=0, I=0 (concrete + rigid) → D = |0+0-1| = 1
        let r = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.0, 0.0);
        assert!((d.main_sequence_distance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_d_max_at_zone_of_uselessness() {
        // Zone of Uselessness: A=1, I=1 (abstract + unstable) → D = |1+1-1| = 1
        let r = raw(0.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 1.0, 1.0);
        assert!((d.main_sequence_distance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_d_separate_from_z() {
        // inv #10 — D ayrı field; z (instability) raw'da. compute_derived ikisini ayrı tutar.
        let r = raw(0.0, 0.0, 0.4, 0.0, 0.0); // z = I = 0.4 (raw'da)
        let vision = VisionVector::new(raw(1.0, 1.0, 1.0, 1.0, 1.0));
        // instability param = 0.4 (raw.z ile aynı), abstractness = 0.3
        let d = compute_derived(&r, &vision, &Space::new(), &CosineDeviation, 0.4, 0.3);
        // D = |0.3 + 0.4 - 1| = 0.3
        assert!(
            (d.main_sequence_distance - 0.3).abs() < 1e-9,
            "D = {}",
            d.main_sequence_distance
        );
        // z (raw) değişmedi — compute_derived raw'yı mutate etmez
        assert!((r.z - 0.4).abs() < 1e-9);
    }

    #[test]
    fn compute_derived_orthogonal_negative_space() {
        // θ = 0.5 (orthogonal) → negatif-uzay eşiği; u = 0.5
        let r = raw(1.0, 0.0, 0.0, 0.0, 0.0);
        let vision = VisionVector::new(raw(0.0, 1.0, 0.0, 0.0, 0.0));
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
        let _ = d.theta(
            &raw(0.5, 0.5, 0.5, 0.5, 0.5),
            &VisionVector::new(raw(0.5, 0.5, 0.5, 0.5, 0.5)),
            &Space::new(),
        );
    }

    // ── VisionSource provenance ──────────────────────────────────────────────
    // #2 görevi: "Vision: not loaded" iken θ hesaplanması çelişkisini çöz.
    // None → θ hesaplanmamalı (topology-only); diğerleri → evaluable.

    #[test]
    fn vision_source_confidence_hierarchy() {
        // Güven sıralaması: None < GlobalDefault < BuiltinRole < RoleProfile < UserLoaded
        use super::VisionSource as S;
        assert!((S::None.confidence() - 0.0).abs() < 1e-9);
        assert!((S::GlobalDefault.confidence() - 0.5).abs() < 1e-9);
        assert!((S::BuiltinRole.confidence() - 0.6).abs() < 1e-9);
        assert!((S::RoleProfile.confidence() - 0.9).abs() < 1e-9);
        assert!((S::UserLoaded.confidence() - 1.0).abs() < 1e-9);
        assert!(S::None.confidence() < S::GlobalDefault.confidence());
        assert!(S::GlobalDefault.confidence() < S::BuiltinRole.confidence());
        assert!(S::BuiltinRole.confidence() < S::RoleProfile.confidence());
        assert!(S::RoleProfile.confidence() < S::UserLoaded.confidence());
    }

    #[test]
    fn vision_source_evaluable_when_not_none() {
        use super::VisionSource as S;
        assert!(!S::None.is_evaluable(), "None must NOT be evaluable");
        assert!(S::GlobalDefault.is_evaluable());
        assert!(S::BuiltinRole.is_evaluable());
        assert!(S::RoleProfile.is_evaluable());
        assert!(S::UserLoaded.is_evaluable());
    }

    #[test]
    fn vision_vector_none_is_topology_only() {
        // VisionVector::none() → source=None → is_evaluable() false.
        let v = VisionVector::none();
        assert_eq!(v.source(), VisionSource::None);
        assert!(!v.is_evaluable());
        // none() raw'ı default (zero) — ama bu hesaba katılmamalı (topology-only).
        assert_eq!(v.raw(), &RawPosition::default());
    }

    #[test]
    fn cosine_returns_max_deviation_for_none_vision() {
        // Kritik: source=None vision ile θ hesaplanmamalı. Conservative davranış:
        // maksimum sapma (1.0) döndür — bu ya caller'ı is_evaluable() kontrol etmeye
        // zorlar ya da güvenli-taraf hatası üretir (topology-only modda hiçbir claim
        // "pass" olarak yanlış işaretlenmez).
        let none_vision = VisionVector::none();
        let aligned_raw = raw(0.5, 0.5, 0.5, 0.5, 0.5);
        let theta = CosineDeviation.theta(&aligned_raw, &none_vision, &Space::new());
        assert!(
            (theta - 1.0).abs() < 1e-9,
            "None vision must return max deviation, got θ = {}",
            theta
        );
    }

    #[test]
    fn vision_vector_with_source_preserves_provenance() {
        // with_source provenance'ı korur; new() GlobalDefault varsayar (legacy).
        let user_vision =
            VisionVector::with_source(raw(0.3, 0.7, 0.5, 0.5, 0.5), VisionSource::UserLoaded);
        assert_eq!(user_vision.source(), VisionSource::UserLoaded);
        assert!(user_vision.is_evaluable());

        let legacy = VisionVector::new(raw(0.3, 0.7, 0.5, 0.5, 0.5));
        assert_eq!(
            legacy.source(),
            VisionSource::GlobalDefault,
            "new() backwards-compat: GlobalDefault varsay"
        );
    }

    #[test]
    fn vision_source_labels_are_distinct() {
        // UI etiketleri benzersiz ve human-readable olmalı.
        use super::VisionSource as S;
        let labels = [
            S::None.label(),
            S::GlobalDefault.label(),
            S::BuiltinRole.label(),
            S::RoleProfile.label(),
            S::UserLoaded.label(),
        ];
        // tüm etiketler benzersiz
        let mut sorted = labels;
        sorted.sort();
        for w in sorted.windows(2) {
            assert_ne!(w[0], w[1], "duplicate label: {}", w[0]);
        }
    }

    #[test]
    fn vision_vector_default_is_none() {
        // Default → None (topology-only). Engine kurulurken vision verilmezse
        // güvenli taraf: değerlendirme yapılmaz, "not loaded" etiketi gösterilir.
        let v = VisionVector::default();
        assert_eq!(v.source(), VisionSource::None);
        assert!(!v.is_evaluable());
    }

    // --- compute_risk_score (§3.2 composite risk) ---

    use crate::space::NodeWitness;

    fn witness(commits: usize, authors: usize, days_ago: u32, ownership: f64) -> NodeWitness {
        NodeWitness {
            commits_touching: commits,
            distinct_authors: authors,
            last_modified_days_ago: days_ago,
            churn: commits as u64 * 10,
            ownership_concentration: ownership,
        }
    }

    #[test]
    fn risk_score_none_witness_is_neutral() {
        // Ölçülmemiş (git yok) → neutral. "bilmiyoruz" sessiz varsayım değil.
        let (score, bd) = compute_risk_score(0.8, None, 1.0);
        // Neutral breakdown: vision=θ (confidence 1.0), diğerleri 0.5.
        assert!((bd.vision - 0.8).abs() < 1e-9);
        assert!((bd.volatility - 0.5).abs() < 1e-9);
        assert!((bd.recency - 0.5).abs() < 1e-9);
        assert!((bd.solo - 0.5).abs() < 1e-9);
        assert!((bd.speculative - 0.5).abs() < 1e-9);
        // total = 0.40*0.8 + 0.20*0.5 + 0.15*0.5 + 0.15*0.5 + 0.10*0.5
        //       = 0.32 + 0.10 + 0.075 + 0.075 + 0.05 = 0.62
        assert!(
            (score - 0.62).abs() < 1e-9,
            "neutral total = 0.62, got {score}"
        );
    }

    #[test]
    fn risk_score_zero_vision_confidence_zeros_vision_component() {
        // VisionSource::None → confidence 0 → vision katkısı 0.
        // "Vision: not loaded" + "high risk" çelişkisi çözülür.
        let w = witness(20, 3, 5, 0.4);
        let (score, bd) = compute_risk_score(0.9, Some(&w), 0.0);
        assert!(bd.vision.abs() < 1e-9, "confidence 0 → vision 0");
        assert!(score < 0.5, "vision yok → risk düşük-orta, got {score}");
    }

    #[test]
    fn risk_score_high_theta_high_witness_risk() {
        // Yüksek mimari sapma + volatil + yeni + solo → yüksek risk.
        // commits=60 > 2 → speculative = 0.2 (olgun ama hareketli, yeni DEĞİL).
        let w = witness(60, 1, 1, 1.0); // C_MAX aşımı, solo, dün
        let (score, bd) = compute_risk_score(0.9, Some(&w), 1.0);
        assert!(bd.vision > 0.8, "vision = θ×conf = 0.9");
        assert!(
            (bd.volatility - 1.0).abs() < 1e-9,
            "60 commits > C_MAX → 1.0"
        );
        assert!(bd.recency > 0.9, "1 day ago → ~0.99");
        assert!((bd.solo - 1.0).abs() < 1e-9, "solo ownership");
        assert!(
            (bd.speculative - 0.2).abs() < 1e-9,
            "commits>2 → 0.2 (olgun)"
        );
        // Yüksek vision + volatility + recency + solo → yüksek risk, speculative düşük olsa da.
        assert!(score > 0.7, "composite high risk, got {score}");
    }

    #[test]
    fn risk_score_battle_tested_low_risk() {
        // Çok dokunulmuş, çok yazar, eski, shared → low risk (battle-tested).
        let w = witness(100, 8, 365, 0.2); // çok commit, 8 yazar, 1 yıl önce, shared
        let (score, bd) = compute_risk_score(0.1, Some(&w), 1.0); // düşük sapma
        assert!((bd.volatility - 1.0).abs() < 1e-9, "100 commits → volatil");
        assert!(bd.recency < 0.05, "365 days → ~0.018, eski = stabil");
        assert!((bd.solo - 0.2).abs() < 1e-9, "shared ownership");
        assert!((bd.speculative - 0.2).abs() < 1e-9, "olgun");
        // volatil AMA eski + shared + düşük vision → net düşük-orta
        assert!(score < 0.45, "battle-tested → düşük risk, got {score}");
    }

    #[test]
    fn risk_score_monotonic_in_theta() {
        // Daha yüksek θ → daha yüksek risk (diğerleri sabit, confidence > 0).
        let w = witness(10, 2, 30, 0.5);
        let (s_low, _) = compute_risk_score(0.1, Some(&w), 1.0);
        let (s_high, _) = compute_risk_score(0.9, Some(&w), 1.0);
        assert!(
            s_high > s_low,
            "monotonic: θ=0.9 ({s_high}) > θ=0.1 ({s_low})"
        );
    }

    #[test]
    fn risk_score_speculative_flag_for_new_files() {
        // commits <= 2 → speculative 0.8 (yeni/az-dokunulmuş).
        let new_file = witness(1, 1, 0, 1.0);
        let (_, bd_new) = compute_risk_score(0.0, Some(&new_file), 1.0);
        assert!((bd_new.speculative - 0.8).abs() < 1e-9);

        let established = witness(50, 1, 0, 1.0);
        let (_, bd_est) = compute_risk_score(0.0, Some(&established), 1.0);
        assert!((bd_est.speculative - 0.2).abs() < 1e-9);
    }

    #[test]
    fn risk_score_clamps_to_unit_interval() {
        // Volatil ekstrem witness (C_MAX aşımı) → volatility 1.0, total clamp [0,1].
        let w_extreme = witness(10_000, 1, 0, 1.0);
        let (score, _) = compute_risk_score(1.0, Some(&w_extreme), 1.0);
        assert!(score >= 0.0 && score <= 1.0, "clamped [0,1], got {score}");

        // Out-of-range θ/confidence girdileri clamp edilir → total yine [0,1].
        let (score2, _) = compute_risk_score(5.0, None, 2.0);
        assert!(score2 >= 0.0 && score2 <= 1.0, "input clamp works");
    }

    #[test]
    fn risk_weights_sum_to_one() {
        // Ağırlıklar 1.0 toplam — composite anlamlı (her bileşen eşit ağırlık değil).
        let total = risk_weights::VISION
            + risk_weights::VOLATILITY
            + risk_weights::RECENCY
            + risk_weights::SOLO
            + risk_weights::SPECULATIVE;
        assert!((total - 1.0).abs() < 1e-9, "weights sum = 1.0, got {total}");
    }
}
