//! Şahitlik zinciri — Evidence → WitnessSet → Status → Result (§4).
//!
//! **Faz 1.5:** Workflow-agnostik witness modeli.
//! - **inv #1** author-witness rejection → `canonicalize_for(author)` author'ı çıkarır
//! - **inv #2** actor-eksenli dedup (#212) → her tanık için en güçlü gözlem kalır
//! - **inv #3** tri-state `WitnessStatus::{Witnessed, Unwitnessed, UnobservableLocally}`
//! - **inv #9** WitnessSet tabanlı `W(C, Ω)` → `evaluate(claim, &WitnessSet) -> WitnessResult`
//!
//! `CanonicalWitnessSet` (parse-don't-validate): actor-eksenli dedup + author-filter
//! yapısal garanti. `support()` / `approver_count()` yalnızca canonicalized set üzerinde
//! → "dedup'i unutma" imkânsız; küme actor-unique olduğundan `approver_count()`
//! yapısal olarak distinct non-author approver sayısıdır (#212).

use crate::coords::RawPosition;
use crate::space::{Edge, Node};

// ═══════════════════════════════════════════════════════════════════════════════
// Tanımlayıcılar
// ═══════════════════════════════════════════════════════════════════════════════

pub type AgentId = u64;
pub type ClaimId = u64;
pub type EvidenceId = u64;
/// Kanıt kaynağı: `"PR #42"`, `"<commit-sha>"`, `"trailer:Reviewed-by:alice"`.
pub type EvidenceSource = String;

// ═══════════════════════════════════════════════════════════════════════════════
// WitnessKind + ağırlıklar (§4.1 KARAR; kalibrasyon Faz 1.11)
// ═══════════════════════════════════════════════════════════════════════════════

/// Şahit türü. Ağırlık = güven seviyesi.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum WitnessKind {
    /// `git rev-list --merges` — kriptografik (imzalı merge). En güçlü.
    #[default]
    MergeCommit,
    /// `gh pr list --state merged` veya merge-imza metni — GitHub API + review-required.
    PRMerged,
    /// `Reviewed-by:` trailer'ı — imzalı ama daha zayıf.
    TrailerReviewed,
    /// `Co-authored-by:` trailer'ı — katkı, review değil. En zayıf.
    CoAuthored,
}

impl WitnessKind {
    /// §4.1 başlangıç ağırlıkları (kalibrasyon: Faz 1.11, 15-20 repo korpusu).
    pub fn default_weight(&self) -> f64 {
        match self {
            Self::MergeCommit => 1.0,
            Self::PRMerged => 0.8,
            Self::TrailerReviewed => 0.7,
            Self::CoAuthored => 0.4,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EvidenceEvent (gözlem — inv #2'nin girdi birimi)
// ═══════════════════════════════════════════════════════════════════════════════

/// Tek gözlemlenen kanıt. Pozitif evidence (approval). **Gözlem birimi, tanık
/// kimliği DEĞİL (#212):** bir actor'ün N gözlemi N bağımsız onaycı yaratmaz;
/// `canonicalize_for` actor başına en güçlü gözlemi kanonik kümeye taşır (inv #2).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceEvent {
    pub id: EvidenceId,
    pub source: EvidenceSource,
    pub witness_kind: WitnessKind,
    pub actor: AgentId,
    pub claim: ClaimId,
    pub weight: f64,
}

impl EvidenceEvent {
    pub fn new(
        id: EvidenceId,
        source: impl Into<EvidenceSource>,
        kind: WitnessKind,
        actor: AgentId,
        claim: ClaimId,
    ) -> Self {
        Self {
            id,
            source: source.into(),
            weight: kind.default_weight(),
            witness_kind: kind,
            actor,
            claim,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WitnessSet (raw — ham kanıtlar) + canonicalize_for
// ═══════════════════════════════════════════════════════════════════════════════

/// W(C, Ω)'nın Ω'sı. Ham kanıt kümesi.
///
/// **DİKKAT:** `support()` / approver sayısı doğrudan HESAPLANMAZ — önce
/// `canonicalize_for(author)` ile `CanonicalWitnessSet`'e çevir (inv #1 + #2 yapısal).
#[derive(Debug, Clone, Default)]
pub struct WitnessSet {
    pub events: Vec<EvidenceEvent>,
    pub min_approvers: usize,
    pub quorum_threshold: f64,
}

impl WitnessSet {
    /// Default: `min_approvers=2`, `quorum_threshold=1.5` (§4.2).
    pub fn new(events: Vec<EvidenceEvent>) -> Self {
        Self {
            events,
            min_approvers: 2,
            quorum_threshold: 1.5,
        }
    }

    /// Builder: quorum parametrelerini override et (kalibrasyon, Faz 1.11).
    pub fn with_quorum(mut self, min_approvers: usize, quorum_threshold: f64) -> Self {
        self.min_approvers = min_approvers;
        self.quorum_threshold = quorum_threshold;
        self
    }

    /// **inv #1 + #2 — tek giriş noktası.** Author'ı çıkar + actor-eksenli dedup
    /// (her tanık için en güçlü gözlem kalır) → `CanonicalWitnessSet`.
    ///
    /// **#217 fail-closed sayısal geçerlilik:** ham sayısal alanlar TÜM olaylarda
    /// doğrulanır (author ve dedup DAHİL — bir olayın Q1/Q2'de kullanılmaması ham
    /// kümenin geçerli olduğu anlamına gelmez); hesaplanan `support`'un sonluluğu
    /// YALNIZ author-filtreli + actor-dedup sonrası kanonik kümede kontrol edilir.
    ///
    /// **#212 seçim sözleşmesi (girdi sırasından bağımsız):** aynı actor'e ait
    /// olaylar arasında hayatta kalan, 5-anahtarlı total sırayla belirlenir —
    /// weight DESC (`total_cmp`) → id ASC → source ASC → claim ASC →
    /// witness_kind canonical-rank ASC. Bu beşi eşit iki ayrı olay olamaz
    /// (actor sabitken fark yaratan tek kalan alan weight'tir) → seçim her
    /// zaman tam deterministik. NaN/∞ determinizmi `total_cmp` ile çözülür;
    /// sayısal **geçerlilik** ayrı sözleşmedir (#217 — bu sınırda RED).
    pub fn canonicalize_for(
        &self,
        author: AgentId,
    ) -> Result<CanonicalWitnessSet, WitnessNumericsError> {
        use std::collections::HashMap;
        self.validate_numerics()?;
        let mut deduped: HashMap<AgentId, EvidenceEvent> = HashMap::new();
        for e in &self.events {
            if e.actor == author {
                continue; // inv #1 — author witness rejection
            }
            match deduped.get(&e.actor) {
                Some(existing) if actor_axis_prefers(existing, e) => {} // inv #2 — strongest per actor stays
                _ => {
                    deduped.insert(e.actor, e.clone());
                }
            }
        }
        let mut events: Vec<EvidenceEvent> = deduped.into_values().collect();
        // (id, actor): farklı actor'lerde aynı event-id HashMap iterasyon sırasını
        // sızdıramaz; tek başına id bu garantiyi vermez (#212 tur-1 P1-1).
        events.sort_by_key(|e| (e.id, e.actor));
        // #217: hesaplanan support'un temsil edilebilirliği — ham girdiler sonlu
        // olsa bile toplam taşabilir (f64::MAX + f64::MAX). Yalnız KANONİK küme
        // üzerinde: author'un olayı inv #1 ile dışlanmış olabilir (tur-2 P2-1).
        let support: f64 = events.iter().map(|e| e.weight).sum();
        if !support.is_finite() {
            return Err(WitnessNumericsError::NonFiniteSupport { support });
        }
        Ok(CanonicalWitnessSet {
            events,
            min_approvers: self.min_approvers,
            quorum_threshold: self.quorum_threshold,
        })
    }

    /// **#217 — tek kanonik sayısal geçerlilik sözleşmesi.** Ham alanları doğrular:
    /// quorum parametreleri (`quorum_threshold` sonlu ve ≥ 0) + TÜM olayların
    /// `weight`'i (sonlu ve ≥ 0 — negatif ağırlık tanık-susturma/quorum-düşürme
    /// yüzeyi). `canonicalize_for` ve motorun erken kontrolü (commit_task_claim)
    /// AYNI bu sözleşmeyi çağırır (ikinci implementasyon yok). Hesaplanan
    /// support'un sonluluğu burada DEĞİL — kanonikleştirme sonunda
    /// (author-filtre bağımlı). `min_approvers` serbest (usize; 0 = bilinçli
    /// HarnessAutoApprove tanıksız-rotası — sayısal geçerlilik ≠ yetkilendirme
    /// politikası; tanıksız-modun production izni witness-policy katmanının
    /// sorumluluğudur).
    pub fn validate_numerics(&self) -> Result<(), WitnessNumericsError> {
        if !self.quorum_threshold.is_finite() {
            return Err(WitnessNumericsError::NonFiniteQuorumThreshold {
                threshold: self.quorum_threshold,
            });
        }
        if self.quorum_threshold < 0.0 {
            return Err(WitnessNumericsError::NegativeQuorumThreshold {
                threshold: self.quorum_threshold,
            });
        }
        for e in &self.events {
            if !e.weight.is_finite() {
                return Err(WitnessNumericsError::NonFiniteWeight {
                    event_id: e.id,
                    weight: e.weight,
                });
            }
            if e.weight < 0.0 {
                return Err(WitnessNumericsError::NegativeWeight {
                    event_id: e.id,
                    weight: e.weight,
                });
            }
        }
        Ok(())
    }
}

/// **#212:** `WitnessKind` canonical rank — enum deklarasyon sırası
/// (MergeCommit < PRMerged < TrailerReviewed < CoAuthored). Seçim
/// sözleşmesinin son tie-break anahtarı.
fn witness_kind_rank(kind: WitnessKind) -> u8 {
    match kind {
        WitnessKind::MergeCommit => 0,
        WitnessKind::PRMerged => 1,
        WitnessKind::TrailerReviewed => 2,
        WitnessKind::CoAuthored => 3,
    }
}

/// **#212:** actor-eksenli seçim karşılaştırması — `incumbent` hayatta kalır mı?
/// Sıra: weight DESC → id ASC → source ASC → claim ASC → kind-rank ASC.
/// ASC anahtarları `challenger.cmp(incumbent)` ile yazılır (küçük olan kazanır);
/// weight DESC `incumbent.total_cmp(challenger)` (büyük olan kazanır). Girdi
/// sırasından bağımsız: anahtarlar actor sabitken olayın tüm kalan alanlarını
/// kapsar.
fn actor_axis_prefers(incumbent: &EvidenceEvent, challenger: &EvidenceEvent) -> bool {
    incumbent
        .weight
        .total_cmp(&challenger.weight)
        .then_with(|| challenger.id.cmp(&incumbent.id))
        .then_with(|| challenger.source.cmp(&incumbent.source))
        .then_with(|| challenger.claim.cmp(&incumbent.claim))
        .then_with(|| {
            witness_kind_rank(challenger.witness_kind)
                .cmp(&witness_kind_rank(incumbent.witness_kind))
        })
        != std::cmp::Ordering::Less
}

/// **#217:** sayısal geçerlilik hatası — operational fault (INV-T9
/// `InvalidWitnessEvidence` sınıfı; motor `EngineCommitError::InvalidWitnessEvidence`
/// / resume `ResumeHeldError::InvalidEvidence` üzerinden exit 20). NaN support ile
/// `NaN < θ == false` üzerinden Q2 bypass'ını kapatır.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WitnessNumericsError {
    /// Olay ağırlığı sonlu değil (NaN/±∞).
    #[error("non-finite evidence weight: event {event_id} weight {weight:?} — weights must be finite and non-negative")]
    NonFiniteWeight { event_id: EvidenceId, weight: f64 },
    /// Negatif ağırlık — pozitif-kanıt modelinde temsil yok (tanık susturma yüzeyi).
    #[error("negative evidence weight: event {event_id} weight {weight:?} — weights must be non-negative")]
    NegativeWeight { event_id: EvidenceId, weight: f64 },
    /// Quorum eşiği sonlu değil.
    #[error(
        "non-finite quorum threshold: {threshold:?} — threshold must be finite and non-negative"
    )]
    NonFiniteQuorumThreshold { threshold: f64 },
    /// Negatif quorum eşiği.
    #[error("negative quorum threshold: {threshold:?} — threshold must be non-negative")]
    NegativeQuorumThreshold { threshold: f64 },
    /// Ham girdiler sonlu olsa bile kanonik kümenin toplam support'u taşdı (+∞).
    #[error("non-finite computed support: {support:?} — canonical set support overflow (sum of individually finite weights)")]
    NonFiniteSupport { support: f64 },
}

// ═══════════════════════════════════════════════════════════════════════════════
// CanonicalWitnessSet (dedup'lı + author-filtered — yapısal garanti)
// ═══════════════════════════════════════════════════════════════════════════════

/// Dedup'lı + author-filtered witness set. Sadece `WitnessSet::canonicalize_for` ile
/// oluşur → inv #1 (author excluded) ve inv #2 (actor-eksenli dedup) **yapısal
/// garantili** (runtime-check değil, type-level). #212: küme actor-uniquedır —
/// her actor en fazla bir olay taşır → `approver_count() == events.len()` yapısal
/// olarak distinct non-author approver sayısıdır.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalWitnessSet {
    events: Vec<EvidenceEvent>,
    min_approvers: usize,
    quorum_threshold: f64,
}

impl CanonicalWitnessSet {
    /// Q1: distinct non-author approver sayısı (#212'den beri yapısal garanti:
    /// küme actor-unique, yani `events.len()` == distinct actor).
    pub fn approver_count(&self) -> usize {
        self.events.len()
    }

    /// Q2: Σ weight — her benzersiz actor'ün korunan (en güçlü) olayı üzerinden
    /// (#212: aynı actor'ün zayıf gözlemleri desteğe eklenmez).
    pub fn support(&self) -> f64 {
        self.events.iter().map(|e| e.weight).sum()
    }

    pub fn min_approvers(&self) -> usize {
        self.min_approvers
    }
    pub fn quorum_threshold(&self) -> f64 {
        self.quorum_threshold
    }
    pub fn events(&self) -> &[EvidenceEvent] {
        &self.events
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WitnessStatus (inv #3 tri-state)
// ═══════════════════════════════════════════════════════════════════════════════

/// Epistemolojik üçlü durum. "Görünmüyor" ≠ "yok" (inv #3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessStatus {
    /// Lokalde dedup'lı `support ≥ θ_quorum` — nesnel uzaya kabul için yeterli.
    Witnessed,
    /// Lokalde observable ama quorum yok — foam sinyali.
    Unwitnessed,
    /// Squash/rebase + trailersız — lokalde karar verilemez; provider API opsiyonel.
    UnobservableLocally,
}

// ═══════════════════════════════════════════════════════════════════════════════
// WitnessResult + Reason (inv #7, #9)
// ═══════════════════════════════════════════════════════════════════════════════

/// W(C, Ω) sonucu. Sadece witness-based Q1-Q3 karar.
///
/// **Deprecated:** INV-T9 conformance fix'i ile `WitnessDisposition`'a migrate edildi.
/// `WitnessResult` artık `WitnessDisposition`'ın type alias'ıdır. Yeni kod
/// `WitnessDisposition` kullanmalıdır; eski `WitnessResult::Commit/Hold/Reject`
/// varyantları artık yoktur — `Satisfied`/`Held`/`Rejected` kullanın.
///
/// Claim-based Q4-Q6 gate'ler `SpaceEngine::commit()` Phase 0'da evaluate()'den ÖNCE koşar
/// (space-engine-design.md §4, osp-core-design.md §3.2). Bu yüzden evaluate() yalnızca
/// Q1-Q3'ü kontrol eder — Claim'in Q4-Q6'yı geçtiği varsayılır.
#[deprecated(note = "use WitnessDisposition (INV-T9 conformance fix)")]
pub type WitnessResult = WitnessDisposition;

/// Sadece witness-based gate failure'ları (Q1-Q3). Claim-based Q4-Q6 failure'ları
/// `EngineCommitError`'da (engine.rs §6.1) — burada tekrar tanımlanmaz
/// (single-source-of-truth, duplication drift risk — osp-core-design.md §3.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// Q2: `support < θ_quorum`.
    QuorumInsufficient { support: f64, threshold: f64 },
    /// Q1: `distinct_non_author_approvers < min_approvers`.
    MinApproversNotMet { distinct: usize, required: usize },
    /// Q3: honest witness reddetti (Faz 1.7+ explicit-reject sinyalleri).
    HonestReject { witness: AgentId },
    /// inv #3 tri-state — lokalde gözlemlenemeyen (squash/rebase + trailersız).
    UnobservableLocally { hint: String },
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 — WitnessDisposition + WitnessHoldReason + NonEmptyWitnessRejections
// ═══════════════════════════════════════════════════════════════════════════════
//
// Paper 2 model–implementation conformance fix: "insufficient witness quorum" bir
// agent-correctable failure DEĞİLDIR. Expected authorization bekleme durumudur ve
// generic Reject retry loop'una girmemelidir (INV-T9).
//
// Bu tipler Commit 1'de tanımlanır; `WitnessResult` → `WitnessDisposition` migration
// Commit 2'de yapılır (deprecated alias). Engine `EngineCommitResult` ayrımı Commit 2.

/// Witness değerlendirmesinin quorum anlık görüntüsü.
///
/// Hem satisfied hem hold/rejected durumlarında aynı yapı — navigator ve pending
/// authorization record için tam kanıt taşır.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitnessQuorumSnapshot {
    /// Distinct non-author approver sayısı (Q1).
    pub approvers: usize,
    /// Gerekli minimum approver sayısı (production: 2).
    pub required_approvers: usize,
    /// Ağırlıklı support (Q2).
    pub support: f64,
    /// Quorum threshold (production: 1.5).
    pub required_support: f64,
}

/// Hold nedenleri — expected authorization bekleme (INVALID evidence DEĞİL).
///
/// `EvidenceNotLocallyObservable`, Paper 1 inv #3 tri-state'inden gelir: evidence
/// lokalde gözlemlenemiyor (squash/rebase + trailersız). Bu bozuk evidence değil,
/// erişilemeyen evidence'dır. `InvalidWitnessEvidence` yalnızca gerçekten bozuk/
/// yetkisiz evidence içindir (malformed, author-self, duplicate).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WitnessHoldReason {
    /// Q1: `distinct_non_author_approvers < min_approvers`.
    MinApproversNotMet { distinct: usize, required: usize },
    /// Q2: `support < θ_quorum`.
    QuorumInsufficient { support: f64, threshold: f64 },
    /// inv #3 tri-state — lokalde gözlemlenemeyen evidence (invalid DEĞİL).
    EvidenceNotLocallyObservable { hint: String },
}

impl WitnessHoldReason {
    /// Kısa stabil string — CLI JSON `"reason"` alanı ve digest için.
    pub fn as_reason_str(&self) -> &'static str {
        match self {
            Self::MinApproversNotMet { .. } => "min_approvers_not_met",
            Self::QuorumInsufficient { .. } => "quorum_insufficient",
            Self::EvidenceNotLocallyObservable { .. } => "evidence_not_locally_observable",
        }
    }
}

/// Tek witness'ın explicit reddi (Q3 honest-reject). Hold DEĞİL — gerçek ret.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WitnessRejection {
    /// Reddeden witness'ın agent ID'si.
    pub witness: AgentId,
    /// Red gerekçesi (opsiyonel, Faz 1.7+ explicit-reject sinyalleriyle gelir).
    pub rationale: Option<String>,
}

/// Non-empty witness rejection listesi — smart constructor guaranteed.
///
/// Boş rejection listesi anlamsızdır (en az bir witness reddetmiş olmalı).
/// `from_single` ve `from_vec` (non-empty kontrolü ile) dışında oluşturulamaz.
/// Serde deserialize'da boş Vec → hata (smart constructor invariant).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NonEmptyWitnessRejections(
    #[serde(deserialize_with = "deserialize_non_empty_rejections")] Vec<WitnessRejection>,
);

impl NonEmptyWitnessRejections {
    /// Tek rejection'dan oluştur.
    pub fn from_single(rejection: WitnessRejection) -> Self {
        Self(vec![rejection])
    }

    /// Vec'den oluştur — boşsa panic (smart constructor invariant).
    pub fn from_vec(rejections: Vec<WitnessRejection>) -> Self {
        assert!(
            !rejections.is_empty(),
            "NonEmptyWitnessRejections::from_vec — boş vec geçersiz (invariant)"
        );
        Self(rejections)
    }

    /// Vec'den oluştur — boşsa None (fallible variant, parsing/validation için).
    pub fn try_from_vec(rejections: Vec<WitnessRejection>) -> Option<Self> {
        if rejections.is_empty() {
            None
        } else {
            Some(Self(rejections))
        }
    }

    /// İç slice'a erişim.
    pub fn as_slice(&self) -> &[WitnessRejection] {
        &self.0
    }

    /// İç Vec'e erişim (consume).
    pub fn into_inner(self) -> Vec<WitnessRejection> {
        self.0
    }

    /// Rejection sayısı.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Her zaman false (non-empty invariant).
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// Serde deserialize helper — boş Vec'i reddeder (NonEmpty invariant).
fn deserialize_non_empty_rejections<'de, D>(
    deserializer: D,
) -> Result<Vec<WitnessRejection>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let v = Vec::<WitnessRejection>::deserialize(deserializer)?;
    if v.is_empty() {
        use serde::de::Error;
        return Err(D::Error::custom(
            "NonEmptyWitnessRejections cannot be empty (invariant)",
        ));
    }
    Ok(v)
}

/// Canonical witness değerlendirme sonucu — TimeFSM::advance çıktısı.
///
/// **INV-T9:** `Held` expected authorization bekleme durumudur, HATA DEĞİL.
/// `Rejected` explicit witness reddidir, HATA DEĞİL. Operational fault'lar
/// (`InvalidWitnessEvidence`, permission, persistence, internal) `EngineCommitError`'da
/// kalır — bu enum yalnızca domain outcome'ları taşır.
///
/// Migration: `WitnessResult` deprecated alias olarak korunur (Commit 2).
#[derive(Debug, Clone, PartialEq)]
pub enum WitnessDisposition {
    /// Quorum + vision-bound sağlandı → uzay genişler (§6).
    Satisfied {
        delta: crate::bigbang::Delta,
        /// inv #7 — admin override kullanıldıysa true (asla sessiz).
        safety_weakened: bool,
        override_reason: Option<String>,
        snapshot: WitnessQuorumSnapshot,
    },
    /// Q1/Q2/EvidenceNotLocallyObservable yetersiz — INV-T9 suspended authorization.
    Held {
        reason: WitnessHoldReason,
        snapshot: WitnessQuorumSnapshot,
    },
    /// Q3: honest-reject (explicit witness rejection). NonEmpty invariant.
    Rejected {
        reasons: NonEmptyWitnessRejections,
        snapshot: WitnessQuorumSnapshot,
    },
}

// ═══════════════════════════════════════════════════════════════════════════════
// Intent + Claim (witness-domain; space'e NodeId ile bağlı)
// ═══════════════════════════════════════════════════════════════════════════════

/// Agent'a verilen görev. **`t_f` (Gelecek) katmanında yaşar** (potansiyel gradyan —
/// agent-prompt-semantics.md §0 ontolojik harita, OSP-formalism.md §1.2 + §3.1).
///
/// Intent uzayı mutasyona uğratmaz; `t_f`'den `t_m`'ye gradyan `OspPrompt` projeksiyonu
/// üzerinden yayılır (Faz 5).
///
/// **Invariant (yapısal):** `time_layer` private + `#[serde(skip)]` → her zaman `Gelecek`.
/// Struct literal bypass imkânsız (field private); `Intent::new()` tek constructor.
/// Serde deserialize'da da `default_time_layer_gelecek()` ile Gelecek set edilir.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Intent {
    pub agent: AgentId,
    /// Raw hedef konum — derived değil (inv #4).
    pub target_raw: RawPosition,
    #[serde(skip, default = "default_time_layer_gelecek")]
    time_layer: crate::space::TimeLayer,
}

fn default_time_layer_gelecek() -> crate::space::TimeLayer {
    crate::space::TimeLayer::Gelecek
}

impl Intent {
    /// Intent oluştur — time_layer her zaman Gelecek (constructor invariant).
    pub fn new(agent: AgentId, target_raw: RawPosition) -> Self {
        Self {
            agent,
            target_raw,
            time_layer: crate::space::TimeLayer::Gelecek,
        }
    }

    /// Zaman katmanı — her zaman `TimeLayer::Gelecek` (yapısal invariant).
    pub fn time_layer(&self) -> crate::space::TimeLayer {
        self.time_layer
    }

    /// Aşama C — Task'tan Intent türet. `target_raw` = `InternalTaskPlan.milestone_target_vector`
    /// (preferred_vector). **INV-T1:** internal-only — agent'a serialize edilmez.
    /// Predicate validation için değil, internal navigation/distance hesabı için.
    ///
    /// Agent hedef koordinatı görmez; sadece `AgentTaskView` (predicate + current_measurement)
    /// alır. Bu Intent sadece engine/claim/witness içindir.
    pub fn from_task(agent: AgentId, plan: &crate::trajectory::InternalTaskPlan) -> Self {
        Self::new(agent, plan.milestone_target_vector)
    }
}

/// Agent'ın ürettiği iş (PR). `t_m`'de Belief → `t_c`'de Knowledge (witness sonrası).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub intent: Intent,
    /// inv #1 — evaluate'de approvers'tan çıkarılır.
    pub author: AgentId,
    /// Engine tarafından DeltaProposal'dan **compute edilmiş** raw position.
    /// LLM tarafından declare EDİLMEZ (inv #4, agent-prompt-semantics.md §2.2) —
    /// coord_system/analyzer'ın ΔS'i uygulayıp node'ları yeniden ölçmesinin sonucu.
    pub computed_raw: RawPosition,
    pub delta_nodes: Vec<Node>,
    pub delta_edges: Vec<Edge>,
    /// INV-T5 — Trajectory binding. `None` = standalone claim (Paper 1 static flow,
    /// legacy, baseline analiz). `Some(id)` = trajectory-bound (Q5.b Predicate Gate
    /// bu claim'i değerlendirir). `#[serde(default)]` ile backward-compat — eski
    /// snapshot'lar (task_id yok) None ile deserialize olur.
    ///
    /// **Q5.b kuralı:** çıplak Claim ile çalışmaz; `bind_task_claim()` ile
    /// `TaskBoundClaim`'e dönüştürülmeli (INV-T5 — static Claim taskless olabilir,
    /// ama Q5.b sadece TaskBoundClaim kabul eder).
    #[serde(default)]
    pub task_id: Option<crate::trajectory::TaskId>,
    /// **G2c-2 (arkadaş review 7 #5):** Kaldırılacak kenarlar. DeltaProposal → Claim →
    /// Delta zinciri — `evaluate` bunu `Delta.removed_edges`'e geçirir, `apply_delta`
    /// uygular. Coupling/instability düşürme = import kaldırma.
    #[serde(default)]
    pub removed_edges: Vec<crate::agent::EdgeRef>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// evaluate — W(C, Ω) pure karar fonksiyonu (inv #9)
// ═══════════════════════════════════════════════════════════════════════════════

/// W(C, Ω) — pure karar (mutasyon yok). Sadece witness-based Q1-Q3.
/// Claim-based Q4-Q6 (`SpaceEngine::commit()` Phase 0) zaten geçti varsayılır.
///
/// Faz 1.5: Q1 (min_approvers) + Q2 (quorum) uygular. Q3 (honest-reject) Faz 1.7+'te
/// explicit sinyallerle gelir; o zamana kadar Q1+Q2 geçer → Commit.
///
/// **INV-T9:** `Held` expected authorization bekleme durumudur (agent-correctable
/// failure DEĞİL). Engine bunu `EngineCommitResult::Held`'e map'ler, navigator
/// `AwaitingWitnesses` ile suspend eder — retry loop'a girmez, budget tüketmez.
///
/// **#217:** sayısal geçerlilik ihlali operational fault'tur — domain outcome
/// (Held/Rejected) DEĞİL; `Err(WitnessNumericsError)` olarak döner, motor
/// `InvalidWitnessEvidence` sınıfına map'ler. `Err(e) ⇒ S_after == S_before`
/// (mutasyon yalnız `Ok(Satisfied)` dalında).
pub fn evaluate(
    claim: &Claim,
    omega: &WitnessSet,
) -> Result<WitnessDisposition, WitnessNumericsError> {
    let canon = omega.canonicalize_for(claim.author)?;

    // Snapshot — her disposition'da tam witness kanıtı taşır.
    let snapshot = WitnessQuorumSnapshot {
        approvers: canon.approver_count(),
        required_approvers: canon.min_approvers(),
        support: canon.support(),
        required_support: canon.quorum_threshold(),
    };

    // Q1: distinct non-author approvers
    if canon.approver_count() < canon.min_approvers() {
        return Ok(WitnessDisposition::Held {
            reason: WitnessHoldReason::MinApproversNotMet {
                distinct: canon.approver_count(),
                required: canon.min_approvers(),
            },
            snapshot,
        });
    }

    // Q2: support >= quorum
    if canon.support() < canon.quorum_threshold() {
        return Ok(WitnessDisposition::Held {
            reason: WitnessHoldReason::QuorumInsufficient {
                support: canon.support(),
                threshold: canon.quorum_threshold(),
            },
            snapshot,
        });
    }

    // Q3 (honest-reject): Faz 1.7+ — sinyal yok, skip.
    // (Q4-Q6 claim-based — engine seviyesinde, evaluate()'den önce kontrol edildi)

    // Commit — prospective delta (gerçek mutasyon Faz 1.8 commit() içinde).
    // **#97 MD-3 S3:** paylaşılan üretici — `approve_cold_start` apply'ı aynı
    // preimage'ı kullanır (iki truth source drift riski kapalı).
    let delta = crate::bigbang::prospective_delta_from_claim(claim);

    Ok(WitnessDisposition::Satisfied {
        delta,
        safety_weakened: false,
        override_reason: None,
        snapshot,
    })
}

/// Repo/claim seviyesinde tri-state sınıflama (inv #3).
///
/// Faz 1.10 re-spike'ta `osp-spike` `WitnessProfile`'ından beslenir. `locally_observable=false`
/// (`squash/rebase + trailersız`) → `UnobservableLocally` (`Unwitnessed` ile karıştırma).
pub fn classify_status(
    support: f64,
    quorum_threshold: f64,
    locally_observable: bool,
) -> WitnessStatus {
    if !locally_observable {
        WitnessStatus::UnobservableLocally
    } else if support >= quorum_threshold {
        WitnessStatus::Witnessed
    } else {
        WitnessStatus::Unwitnessed
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// #164 (review R1 P1-2) — Dış (operatör) kanıt yüzeyi doğrulayıcısı
//
// INV-T9 sözleşmesi (`EngineCommitError::InvalidWitnessEvidence` + exit 20 =
// "malformed/author-self/duplicate" + NativeFailureSurface dokümantasyonu) kusurlu
// kanıtı operational fault sayar; bu sınıf bugüne dek hiçbir production kodu
// tarafından üretilmiyordu. Resume wire'ı bu sözleşmenin İLK production
// tüketicisidir: operatör kanıt dosyası temiz değilse BAŞTAN reddedilir.
//
// İki katmanın ayrımı (bilinçli — #212 ile netleştirildi): farklı güven
// sınırları, farklı anahtarlar, ÇELİŞMEYEN politikalar:
//
// | Katman               | Anahtar                  | Davranış              |
// |----------------------|--------------------------|-----------------------|
// | Core canonicalization| `actor`                  | en güçlü olay kalır   |
// | Wire exact-duplicate | `(source, actor, claim)` | `DuplicateKey` RED    |
// | Wire actor tekliği   | `actor`                  | `DuplicateActor` RED  |
//
// Çekirdek TOPLANMIŞ kanıt kümelerinden hesaplama için tekil actor temsili
// üretir (inv #1 author-dışlama + inv #2 actor-eksenli seçim) — örn. `gh pr
// list` çıktısında author'un kendi commit'i legitimately görünebilir ve
// sayılmaz; aynı tanığın iki gözlemi tek temsile iner. Operatör wire'ı ise
// bir askıyı çözmek için bilinçli listelenmiş kanıttır: author-self veya
// duplicate giriş DOSYA KUSURUDUR; sessiz düzeltme/dedup, quorum eksikliğini
// beklenmedik bir StillHeld olarak maskeler. Yüzey kusuru RED der; çekirdek
// tekil temsil üretir.
// ═══════════════════════════════════════════════════════════════════════════════

/// **#164 (R1 P1-2):** Dış kanıt yüzeyi doğrulama hatası — operational fault.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExternalEvidenceError {
    /// inv #1 — author kendi claim'ine şahitlik edemez; wire'da kusur olarak reddedilir.
    #[error("author-self evidence: event {event_id} actor {actor} is the claim author — the author cannot witness their own claim")]
    AuthorSelf {
        event_id: EvidenceId,
        actor: AgentId,
    },
    /// Wire-surface exact-duplicate politikası: `(source, actor, claim)` üçlüsü
    /// dosyada iki kez göründü. #212'den beri core INV #2 ile ÖZDEŞ DEĞİL —
    /// core'un seçim anahtarı `actor`; bu anahtar wire'ın kendi sıkıdır.
    #[error(
        "duplicate evidence: events {first_id} and {second_id} share dedup key \
         (source={evidence_source:?}, actor={actor}, claim={claim})"
    )]
    DuplicateKey {
        first_id: EvidenceId,
        second_id: EvidenceId,
        evidence_source: String,
        actor: AgentId,
        claim: ClaimId,
    },
    /// Aynı actor iki ayrı olayla geldi — Q1 "distinct non-author approvers"
    /// semantiği wire yüzeyinde zorlanır (tek actor iki kaynakla çift-onaycı
    /// olamaz). #212'den beri çekirdek de actor-eksenli seçer (tek olay kalır);
    /// wire yine de RED der — bilinçli listelenmiş kanıtta bu bir dosya kusurudur,
    /// sessiz collapse quorum eksikliğini StillHeld olarak maskeler.
    #[error(
        "duplicate actor: actor {actor} appears in events {first_id} and {second_id} — \
         each witness contributes exactly one event"
    )]
    DuplicateActor {
        actor: AgentId,
        first_id: EvidenceId,
        second_id: EvidenceId,
    },
    /// Olay kimliği çakışması (wire kusuru).
    #[error("duplicate event id: {event_id}")]
    DuplicateEventId { event_id: EvidenceId },
}

/// **#164 (R1 P1-2):** Operatör kanıt dosyası yüzey doğrulaması.
///
/// Sıra: duplicate-id → author-self → duplicate-key → duplicate-actor.
/// Claim-binding (event.claim == artifact claim) bu fonksiyonun DEĞİL resume
/// fence'inin işidir (`ResumeHeldError::ClaimBindingMismatch`).
pub fn validate_external_evidence(
    events: &[EvidenceEvent],
    author: AgentId,
) -> Result<(), ExternalEvidenceError> {
    use std::collections::{HashMap, HashSet};
    let mut seen_ids: HashSet<EvidenceId> = HashSet::new();
    let mut seen_keys: HashMap<(String, AgentId, ClaimId), EvidenceId> = HashMap::new();
    let mut seen_actors: HashMap<AgentId, EvidenceId> = HashMap::new();
    for e in events {
        if !seen_ids.insert(e.id) {
            return Err(ExternalEvidenceError::DuplicateEventId { event_id: e.id });
        }
        if e.actor == author {
            return Err(ExternalEvidenceError::AuthorSelf {
                event_id: e.id,
                actor: e.actor,
            });
        }
        let key = (e.source.clone(), e.actor, e.claim);
        if let Some(&first_id) = seen_keys.get(&key) {
            return Err(ExternalEvidenceError::DuplicateKey {
                first_id,
                second_id: e.id,
                evidence_source: e.source.clone(),
                actor: e.actor,
                claim: e.claim,
            });
        }
        seen_keys.insert(key, e.id);
        if let Some(&first_id) = seen_actors.get(&e.actor) {
            return Err(ExternalEvidenceError::DuplicateActor {
                actor: e.actor,
                first_id,
                second_id: e.id,
            });
        }
        seen_actors.insert(e.actor, e.id);
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::RawPosition;

    fn claim_with_author(author: AgentId) -> Claim {
        Claim {
            id: 1,
            intent: Intent::new(author, RawPosition::default()),
            author,
            computed_raw: RawPosition::default(),
            delta_nodes: vec![],
            delta_edges: vec![],
            task_id: None,         // standalone (Paper 1 static flow, INV-T5)
            removed_edges: vec![], // G2c-2
        }
    }

    fn ev(id: EvidenceId, source: &str, kind: WitnessKind, actor: AgentId) -> EvidenceEvent {
        EvidenceEvent::new(id, source, kind, actor, 1)
    }

    /// #212: claim + weight açık — seçim sözleşmesi testleri eş-ağırlık kurmak için
    /// weight'i elle eşitler (default ağırlıklar türlere göre farklıdır).
    fn ev_w(
        id: EvidenceId,
        source: &str,
        kind: WitnessKind,
        actor: AgentId,
        claim: ClaimId,
        weight: f64,
    ) -> EvidenceEvent {
        let mut e = EvidenceEvent::new(id, source, kind, actor, claim);
        e.weight = weight;
        e
    }

    // --- inv #1: author exclusion ---

    #[test]
    fn canonicalize_excludes_author_events() {
        let omega = WitnessSet::new(vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "PR#2", WitnessKind::MergeCommit, 100), // author!
            ev(3, "PR#3", WitnessKind::MergeCommit, 300),
        ]);
        let canon = omega.canonicalize_for(100).unwrap();
        // author (100) çıkarıldı → 2 approver (200, 300)
        assert_eq!(canon.approver_count(), 2);
        assert!(!canon.events().iter().any(|e| e.actor == 100));
    }

    #[test]
    fn evaluate_author_self_approval_ignored() {
        // author 100 iki self-event → 0 distinct non-author → Hold
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 100),
            ev(2, "PR#2", WitnessKind::MergeCommit, 100),
        ]);
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(
            result,
            WitnessDisposition::Held {
                reason: WitnessHoldReason::MinApproversNotMet { distinct: 0, .. },
                ..
            }
        ));
    }

    #[test]
    fn single_actor_multi_source_cannot_satisfy_quorum() {
        // #212 asıl ihlali: aynı actor iki MergeCommit kaynağıyla eskiden 2 approver
        // sayılıp min_approvers=2 eşiği TEK kişiyle geçilebiliyordu. Actor-eksenli
        // seçimle 1 tanık → Held; snapshot OLAY TOPLAMI değil en-güçlü-oyu taşır.
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev(1, "PR#42", WitnessKind::MergeCommit, 200),
            ev(2, "commit-abc", WitnessKind::MergeCommit, 200),
        ]);
        match evaluate(&claim, &omega).unwrap() {
            WitnessDisposition::Held { reason, snapshot } => {
                assert!(matches!(
                    reason,
                    WitnessHoldReason::MinApproversNotMet {
                        distinct: 1,
                        required: 2
                    }
                ));
                assert_eq!(
                    snapshot,
                    WitnessQuorumSnapshot {
                        approvers: 1,
                        required_approvers: 2,
                        support: 1.0, // en güçlü olay — olaylar toplamı (2.0) DEĞİL
                        required_support: 1.5,
                    }
                );
            }
            other => panic!("Held bekleniyordu: {other:?}"),
        }
    }

    // --- inv #2: dedup ---

    #[test]
    fn dedup_keeps_strongest_per_key() {
        // Aynı (source, actor, claim) — MergeCommit (1.0) vs CoAuthored (0.4) → 1.0 kalmalı
        let omega = WitnessSet::new(vec![
            ev(1, "PR#42", WitnessKind::MergeCommit, 200),
            ev(2, "PR#42", WitnessKind::CoAuthored, 200), // same source+actor+claim
        ]);
        let canon = omega.canonicalize_for(999).unwrap(); // author yok
        assert_eq!(canon.approver_count(), 1, "aynı key → 1 event");
        assert!(
            (canon.support() - 1.0).abs() < 1e-9,
            "strongest (1.0) kalmalı"
        );
    }

    #[test]
    fn dedup_same_actor_different_source_collapses_to_strongest() {
        // #212 bilinçli değişim: öncülü `dedup_different_source_kept` üçlü-dedup
        // çağında farklı source'un İKİ olay bıraktığını pinliyordu. Actor-eksenli
        // sözleşmede aynı actor tek temsile iner — en güçlü gözlem kalır.
        let omega = WitnessSet::new(vec![
            ev(1, "PR#42", WitnessKind::PRMerged, 200),
            ev(
                2,
                "trailer: Reviewed-by:200",
                WitnessKind::TrailerReviewed,
                200,
            ),
        ]);
        let canon = omega.canonicalize_for(999).unwrap();
        assert_eq!(canon.approver_count(), 1, "aynı actor → 1 olay");
        assert!(
            (canon.support() - 0.8).abs() < 1e-9,
            "en güçlü (PRMerged 0.8) kalmalı"
        );
    }

    #[test]
    fn support_counts_strongest_per_actor() {
        // #212: Q2 actor-tutarlı — aynı actor'ün zayıf gözlemi desteğe EkLENMEZ.
        let omega = WitnessSet::new(vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(
                2,
                "trailer: Co-authored-by:200",
                WitnessKind::CoAuthored,
                200,
            ),
        ]);
        let canon = omega.canonicalize_for(999).unwrap();
        assert_eq!(canon.approver_count(), 1);
        assert!(
            (canon.support() - 1.0).abs() < 1e-9,
            "1.0 + 0.4 = 1.4 DEĞİL — en güçlü 1.0"
        );
    }

    #[test]
    fn actor_axis_dedup_is_input_order_independent() {
        // #212 tur-1 P1-1: 5-anahtarlı seçim sözleşmesinin HER dalı — girdi sırası
        // sonucu etkileyemez ve doğru dal kazandırır. Eş ağırlıklar ev_w ile elle
        // kurulur (default ağırlıklar türlere göre farklı olduğundan kind satırında
        // canonical-rank'ın kendisi — weight değil — karar verir).
        let rows: Vec<(&str, EvidenceEvent, EvidenceEvent, EvidenceEvent)> = vec![
            (
                "weight_desc",
                ev_w(1, "PR#1", WitnessKind::CoAuthored, 200, 1, 0.4),
                ev_w(2, "PR#2", WitnessKind::MergeCommit, 200, 1, 1.0),
                ev_w(2, "PR#2", WitnessKind::MergeCommit, 200, 1, 1.0),
            ),
            (
                "id_asc",
                ev_w(5, "PR#5", WitnessKind::MergeCommit, 200, 1, 1.0),
                ev_w(3, "PR#3", WitnessKind::MergeCommit, 200, 1, 1.0),
                ev_w(3, "PR#3", WitnessKind::MergeCommit, 200, 1, 1.0),
            ),
            (
                "source_asc",
                ev_w(7, "trailer:B", WitnessKind::MergeCommit, 200, 1, 1.0),
                ev_w(7, "PR#A", WitnessKind::MergeCommit, 200, 1, 1.0),
                ev_w(7, "PR#A", WitnessKind::MergeCommit, 200, 1, 1.0),
            ),
            (
                "claim_asc",
                ev_w(7, "PR#9", WitnessKind::MergeCommit, 200, 2, 1.0),
                ev_w(7, "PR#9", WitnessKind::MergeCommit, 200, 1, 1.0),
                ev_w(7, "PR#9", WitnessKind::MergeCommit, 200, 1, 1.0),
            ),
            (
                "kind_rank_asc",
                ev_w(7, "PR#9", WitnessKind::CoAuthored, 200, 1, 0.7),
                ev_w(7, "PR#9", WitnessKind::TrailerReviewed, 200, 1, 0.7),
                ev_w(7, "PR#9", WitnessKind::TrailerReviewed, 200, 1, 0.7),
            ),
        ];
        for (name, a, b, winner) in rows {
            let fwd = WitnessSet::new(vec![a.clone(), b.clone()])
                .canonicalize_for(999)
                .unwrap();
            let rev = WitnessSet::new(vec![b, a]).canonicalize_for(999).unwrap();
            assert_eq!(fwd, rev, "{name}: girdi sırası sonucu değiştirdi");
            assert_eq!(fwd.events(), &[winner], "{name}: yanlış olay hayatta kaldı");
        }
    }

    #[test]
    fn actor_axis_equal_weight_equal_id_is_deterministic() {
        // #212 tur-1 P1-1'in dar pini: eş ağırlık + eş id + farklı source →
        // source ASC kazanır; her iki girdi sırası aynı kümeyi verir.
        let a = ev_w(7, "trailer:Z", WitnessKind::MergeCommit, 200, 1, 1.0);
        let b = ev_w(7, "PR#A", WitnessKind::MergeCommit, 200, 1, 1.0);
        let expected = b.clone();
        let fwd = WitnessSet::new(vec![a.clone(), b.clone()])
            .canonicalize_for(999)
            .unwrap();
        let rev = WitnessSet::new(vec![b, a]).canonicalize_for(999).unwrap();
        assert_eq!(fwd, rev);
        assert_eq!(
            fwd.events(),
            &[expected],
            "source ASC kazanmalı (PR#A < trailer:Z)"
        );
    }

    #[test]
    fn actor_axis_keeps_all_distinct_actors() {
        // #212: eksen actor — FARKLI actor'ler birbirini asla elemez
        // (aynı source kullanmaları bile eleme yaratmaz).
        let omega = WitnessSet::new(vec![
            ev(1, "PR#42", WitnessKind::MergeCommit, 200),
            ev(2, "PR#42", WitnessKind::MergeCommit, 300), // aynı source, farklı actor
            ev(
                3,
                "trailer: Reviewed-by:400",
                WitnessKind::TrailerReviewed,
                400,
            ),
        ]);
        let canon = omega.canonicalize_for(999).unwrap();
        assert_eq!(canon.approver_count(), 3);
        assert!((canon.support() - (1.0 + 1.0 + 0.7)).abs() < 1e-9);
    }

    #[test]
    fn canonical_output_order_stable_with_same_id_across_actors() {
        // #212 review P2-2: farklı actor'lerde AYNI event-id (wire'da
        // DuplicateEventId ile reddedilir; programatik core girişinde mümkün) —
        // çıktı sıralaması (id, actor) ile girdi sırasından bağımsız sabittir:
        // id eşitse actor'a göre, 200 < 300 → actor 200'nin olayı önce.
        let a = ev(7, "PR#1", WitnessKind::MergeCommit, 200);
        let b = ev(7, "PR#2", WitnessKind::PRMerged, 300);
        let fwd = WitnessSet::new(vec![a.clone(), b.clone()])
            .canonicalize_for(999)
            .unwrap();
        let rev = WitnessSet::new(vec![b, a]).canonicalize_for(999).unwrap();
        assert_eq!(fwd, rev, "girdi sırası çıktıyı değiştirdi");
        let actors: Vec<AgentId> = fwd.events().iter().map(|e| e.actor).collect();
        assert_eq!(
            actors,
            vec![200, 300],
            "(id, actor) sıralaması: id eşit → actor ASC"
        );
    }

    #[test]
    fn dedup_different_actor_kept() {
        // Aynı PR, farklı approver → ikisi de kalır (iki kişi)
        let omega = WitnessSet::new(vec![
            ev(1, "PR#42", WitnessKind::MergeCommit, 200),
            ev(2, "PR#42", WitnessKind::MergeCommit, 300),
        ]);
        let canon = omega.canonicalize_for(999).unwrap();
        assert_eq!(canon.approver_count(), 2);
    }

    // --- inv #9: evaluate quorum combinations ---

    #[test]
    fn evaluate_two_strong_witnesses_commit() {
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "PR#2", WitnessKind::MergeCommit, 300),
        ]);
        // support = 2.0 ≥ 1.5, 2 distinct ≥ 2 → Satisfied
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(
            result,
            WitnessDisposition::Satisfied {
                safety_weakened: false,
                ..
            }
        ));
    }

    #[test]
    fn evaluate_three_weak_witnesses_commit() {
        // 3 × TrailerReviewed = 2.1 ≥ 1.5, 3 distinct ≥ 2 → Satisfied
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev(1, "t1", WitnessKind::TrailerReviewed, 200),
            ev(2, "t2", WitnessKind::TrailerReviewed, 300),
            ev(3, "t3", WitnessKind::TrailerReviewed, 400),
        ]);
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(result, WitnessDisposition::Satisfied { .. }));
    }

    #[test]
    fn evaluate_single_strong_witness_holds_on_min_approvers() {
        // 1 MergeCommit: support=1.0 ama distinct=1 < 2 → Held (Q1 önce)
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![ev(1, "PR#1", WitnessKind::MergeCommit, 200)]);
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(
            result,
            WitnessDisposition::Held {
                reason: WitnessHoldReason::MinApproversNotMet {
                    distinct: 1,
                    required: 2
                },
                ..
            }
        ));
    }

    #[test]
    fn evaluate_quorum_insufficient_hold() {
        // 2 CoAuthored = 0.8 < 1.5 → Held (Q2). distinct=2 ≥ 2 (Q1 geçer).
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev(1, "c1", WitnessKind::CoAuthored, 200),
            ev(2, "c2", WitnessKind::CoAuthored, 300),
        ]);
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(
            result,
            WitnessDisposition::Held {
                reason: WitnessHoldReason::QuorumInsufficient { support, threshold },
                ..
            }
            if (support - 0.8).abs() < 1e-9 && (threshold - 1.5).abs() < 1e-9
        ));
    }

    #[test]
    fn evaluate_two_actors_second_evidence_not_double_counted() {
        // #212 review P2-1: Q1 GEÇER (2 distinct actor) ama aynı actor'ün ikinci
        // kanıtı Q2'de sayılmaz → support 1.2 < 1.5 → Held(QuorumInsufficient).
        // Eski davranış: 3 olay, support 1.9 → Satisfied (Q2 çift-sayımdı).
        // Q1 ve Q2'nin actor-unique sözleşmesini evaluate() ucunda birlikte doğrular.
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev(1, "PR#1", WitnessKind::PRMerged, 200),
            ev(
                2,
                "trailer: Reviewed-by:200",
                WitnessKind::TrailerReviewed,
                200,
            ),
            ev(3, "commit-x", WitnessKind::CoAuthored, 300),
        ]);
        match evaluate(&claim, &omega).unwrap() {
            WitnessDisposition::Held { reason, snapshot } => {
                assert!(
                    matches!(
                        reason,
                        WitnessHoldReason::QuorumInsufficient { support, threshold }
                            if (support - 1.2).abs() < 1e-9 && (threshold - 1.5).abs() < 1e-9
                    ),
                    "got {reason:?}"
                );
                assert_eq!(snapshot.approvers, 2, "Q1 geçti: 2 distinct actor");
                assert_eq!(snapshot.required_approvers, 2);
                assert!(
                    (snapshot.support - 1.2).abs() < 1e-9,
                    "0.8 (en güçlü, actor 200) + 0.4 — 1.9 DEĞİL"
                );
                assert_eq!(snapshot.required_support, 1.5);
            }
            other => panic!("Held bekleniyordu: {other:?}"),
        }
    }

    #[test]
    fn evaluate_empty_set_holds() {
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![]);
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(result, WitnessDisposition::Held { .. }));
    }

    // --- custom quorum (kalibrasyon) ---

    #[test]
    fn evaluate_custom_quorum() {
        // min_approvers=1, quorum=1.0 → tek MergeCommit Satisfied eder
        let claim = claim_with_author(100);
        let omega =
            WitnessSet::new(vec![ev(1, "PR#1", WitnessKind::MergeCommit, 200)]).with_quorum(1, 1.0);
        let result = evaluate(&claim, &omega).unwrap();
        assert!(matches!(result, WitnessDisposition::Satisfied { .. }));
    }

    // --- commit delta prospective ---

    #[test]
    fn evaluate_commit_carries_prospective_delta() {
        let mut claim = claim_with_author(100);
        claim.delta_nodes = vec![Node {
            id: 42,
            ..Default::default()
        }];
        let omega = WitnessSet::new(vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "PR#2", WitnessKind::MergeCommit, 300),
        ]);
        if let WitnessDisposition::Satisfied { delta, .. } = evaluate(&claim, &omega).unwrap() {
            assert_eq!(delta.new_nodes.len(), 1);
            assert_eq!(delta.new_nodes[0].id, 42);
        } else {
            panic!("Satisfied bekleniyordu");
        }
    }

    // --- inv #3: tri-state classify ---

    #[test]
    fn classify_witnessed_when_observable_and_quorum_met() {
        assert_eq!(classify_status(2.0, 1.5, true), WitnessStatus::Witnessed);
    }

    #[test]
    fn classify_unwitnessed_when_observable_but_quorum_low() {
        assert_eq!(classify_status(0.5, 1.5, true), WitnessStatus::Unwitnessed);
    }

    #[test]
    fn classify_unobservable_when_not_observable_even_if_support_high() {
        // inv #3 kritik: "görünmüyor" ≠ "yok" — yüksek support bile UnobservableLocally
        assert_eq!(
            classify_status(10.0, 1.5, false),
            WitnessStatus::UnobservableLocally
        );
    }

    // --- WitnessKind ağırlıkları ---

    #[test]
    fn witness_kind_weights_ordered() {
        assert!(WitnessKind::MergeCommit.default_weight() > WitnessKind::PRMerged.default_weight());
        assert!(
            WitnessKind::PRMerged.default_weight() > WitnessKind::TrailerReviewed.default_weight()
        );
        assert!(
            WitnessKind::TrailerReviewed.default_weight()
                > WitnessKind::CoAuthored.default_weight()
        );
    }

    // --- Intent invariant (yapısal — time_layer her zaman Gelecek) ---

    #[test]
    fn intent_new_sets_time_layer_gelecek() {
        use crate::space::TimeLayer;
        let intent = Intent::new(100, RawPosition::default());
        assert_eq!(intent.time_layer(), TimeLayer::Gelecek);
    }

    #[test]
    fn intent_serde_roundtrip_preserves_gelecek() {
        // serde #[serde(skip)] ile time_layer serialize edilmez, deserialize'da default (Gelecek)
        use crate::space::TimeLayer;
        let intent = Intent::new(
            42,
            RawPosition {
                x: 0.5,
                ..Default::default()
            },
        );
        let json = serde_json::to_string(&intent).expect("serialize");
        // time_layer skip edildi → JSON'da yok
        assert!(!json.contains("time_layer"));
        let restored: Intent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            restored.time_layer(),
            TimeLayer::Gelecek,
            "deserialize her zaman Gelecek"
        );
        assert_eq!(restored.agent, 42);
        assert!((restored.target_raw.x - 0.5).abs() < 1e-9);
    }

    #[test]
    fn intent_serdeskip_ignores_time_layer_in_input() {
        // Kötü niyetli serialize edilmiş veri time_layer=Simdiki içerse bile → Gelecek olur
        use crate::space::TimeLayer;
        let malicious_json = r#"{"agent":1,"target_raw":{"x":0.0,"y":0.0,"z":0.0,"w":0.0,"v":0.0},"time_layer":"Simdiki"}"#;
        let restored: Intent = serde_json::from_str(malicious_json).expect("deserialize");
        assert_eq!(
            restored.time_layer(),
            TimeLayer::Gelecek,
            "serde #[skip] input'taki time_layer'ı yok sayar → invariant korundu"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // #164 (R1 P1-2) — dış kanıt yüzeyi doğrulayıcısı
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn validate_external_evidence_accepts_clean_set() {
        let events = vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "PR#2", WitnessKind::PRMerged, 300),
        ];
        assert!(validate_external_evidence(&events, 100).is_ok());
    }

    #[test]
    fn validate_external_evidence_rejects_author_self() {
        let events = vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "PR#2", WitnessKind::MergeCommit, 100), // author!
        ];
        let err = validate_external_evidence(&events, 100).unwrap_err();
        assert!(
            matches!(err, ExternalEvidenceError::AuthorSelf { actor: 100, .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn validate_external_evidence_rejects_duplicate_key() {
        // Aynı (source, actor, claim) — farklı id.
        let events = vec![
            ev(1, "PR#42", WitnessKind::MergeCommit, 200),
            ev(2, "PR#42", WitnessKind::CoAuthored, 200),
        ];
        let err = validate_external_evidence(&events, 999).unwrap_err();
        assert!(
            matches!(
                err,
                ExternalEvidenceError::DuplicateKey {
                    first_id: 1,
                    second_id: 2,
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn validate_external_evidence_rejects_duplicate_actor() {
        // Aynı actor, FARKLI source — çekirdek bunu tekil temsile indirger (#212);
        // wire yüzeyi yine de dosya kusuru olarak RED der.
        let events = vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "commit-abc", WitnessKind::MergeCommit, 200),
        ];
        let err = validate_external_evidence(&events, 999).unwrap_err();
        assert!(
            matches!(
                err,
                ExternalEvidenceError::DuplicateActor {
                    actor: 200,
                    first_id: 1,
                    second_id: 2
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn wire_and_core_same_shape_different_policy() {
        // #212 iki-katman pini: AYNI girdi şekli (aynı actor, iki kaynak) wire'da
        // dosya kusuru (DuplicateActor RED), çekirdekte tekil actor temsili (1 olay).
        // Kabul kriteri 4: wire sözleşmesi actor-eksenli çekirdekLE zayıflamaz.
        let events = vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(2, "commit-abc", WitnessKind::MergeCommit, 200),
        ];
        let err = validate_external_evidence(&events, 999).unwrap_err();
        assert!(
            matches!(
                err,
                ExternalEvidenceError::DuplicateActor {
                    actor: 200,
                    first_id: 1,
                    second_id: 2
                }
            ),
            "got {err:?}"
        );
        let canon = WitnessSet::new(events).canonicalize_for(999).unwrap();
        assert_eq!(canon.approver_count(), 1, "çekirdek tekil temsil üretir");
    }

    #[test]
    fn validate_external_evidence_rejects_duplicate_event_id() {
        let events = vec![
            ev(1, "PR#1", WitnessKind::MergeCommit, 200),
            ev(1, "PR#2", WitnessKind::MergeCommit, 300),
        ];
        let err = validate_external_evidence(&events, 999).unwrap_err();
        assert!(
            matches!(err, ExternalEvidenceError::DuplicateEventId { event_id: 1 }),
            "got {err:?}"
        );
    }

    #[test]
    fn validate_external_evidence_empty_set_is_clean() {
        // Boş kanıt dosyası kusur DEĞİL — StillHeld domain outcome'ıdır.
        assert!(validate_external_evidence(&[], 100).is_ok());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // #217 — fail-closed sayısal geçerlilik (plan-v0 K-1/K-2/K-3 probe'ları;
    // hepsi varyant-assert'li — "is_err değil, DOĞRU ret kimliği")
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn nan_weight_is_invalid_evidence() {
        // K-3 CE1 (#217 asıl ihlali): Q1 GEÇEN küme (2 distinct actor) + NaN
        // ağırlık → eskiden support=NaN → `NaN < 1.5` false → Satisfied (bypass);
        // şimdi operational fault.
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![
            ev_w(1, "PR#1", WitnessKind::MergeCommit, 200, 1, f64::NAN),
            ev_w(2, "PR#2", WitnessKind::CoAuthored, 300, 1, 0.4),
        ]);
        let err = evaluate(&claim, &omega).unwrap_err();
        assert!(
            matches!(
                err,
                WitnessNumericsError::NonFiniteWeight { event_id: 1, .. }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn infinite_weight_rejected() {
        let omega = WitnessSet::new(vec![ev_w(
            1,
            "PR#1",
            WitnessKind::MergeCommit,
            200,
            1,
            f64::INFINITY,
        )]);
        let err = omega.canonicalize_for(999).unwrap_err();
        assert!(
            matches!(
                err,
                WitnessNumericsError::NonFiniteWeight { event_id: 1, .. }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn negative_weight_rejected() {
        // Negatif ağırlık = tanık-susturma/quorum-düşürme yüzeyi — pozitif-kanıt
        // modelinde temsil yok.
        let omega = WitnessSet::new(vec![ev_w(
            1,
            "PR#1",
            WitnessKind::MergeCommit,
            200,
            1,
            -1.0,
        )]);
        let err = omega.canonicalize_for(999).unwrap_err();
        assert!(
            matches!(
                err,
                WitnessNumericsError::NegativeWeight { event_id: 1, weight } if weight == -1.0
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn nan_quorum_threshold_rejected() {
        let omega = WitnessSet::new(vec![]).with_quorum(2, f64::NAN);
        let err = omega.canonicalize_for(999).unwrap_err();
        assert!(
            matches!(err, WitnessNumericsError::NonFiniteQuorumThreshold { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn positive_infinite_threshold_rejected() {
        let omega = WitnessSet::new(vec![]).with_quorum(2, f64::INFINITY);
        let err = omega.canonicalize_for(999).unwrap_err();
        assert!(
            matches!(err, WitnessNumericsError::NonFiniteQuorumThreshold { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn negative_threshold_rejected() {
        let omega = WitnessSet::new(vec![]).with_quorum(2, -0.1);
        let err = omega.canonicalize_for(999).unwrap_err();
        assert!(
            matches!(
                err,
                WitnessNumericsError::NegativeQuorumThreshold { threshold } if threshold == -0.1
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn nan_author_weight_rejected() {
        // K-1 CE1: doğrulama author-filtresinden ÖNCE — author'un bozuk olayı da
        // ham küme kusurudur (Q1/Q2'de kullanılmaması geçerli olduğu anlamına gelmez).
        let omega = WitnessSet::new(vec![
            ev_w(1, "PR#1", WitnessKind::MergeCommit, 100, 1, f64::NAN), // author!
            ev_w(2, "PR#2", WitnessKind::MergeCommit, 200, 1, 1.0),
        ]);
        let err = omega.canonicalize_for(100).unwrap_err();
        assert!(
            matches!(
                err,
                WitnessNumericsError::NonFiniteWeight { event_id: 1, .. }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn invalid_weaker_duplicate_rejected() {
        // K-1 CE2: doğrulama dedup'tan ÖNCE — actor-eksenli seçimin ELEYECEĞİ
        // zayıf kopyanın geçersiz ağırlığı bile RED üretir.
        // **P1-1 (kod-review tur-1):** girdi -∞ — NaN DEĞİL. total_cmp'de pozitif
        // NaN sonlu değerlerden BÜYÜK sıralanır (dedup'un KORUYACAĞI olay olurdu);
        // -∞ ise 1.0 karşısında kesin zayıf → dedup'ın eleyeceği olay garanti.
        // İki giriş sırası da doğrulanır.
        let invalid = ev_w(
            2,
            "PR#1",
            WitnessKind::CoAuthored,
            200,
            1,
            f64::NEG_INFINITY,
        );
        let strong = ev_w(1, "PR#1", WitnessKind::MergeCommit, 200, 1, 1.0);
        for events in [vec![strong.clone(), invalid.clone()], vec![invalid, strong]] {
            let omega = WitnessSet::new(events);
            let err = omega.canonicalize_for(999).unwrap_err();
            assert!(
                matches!(
                    err,
                    WitnessNumericsError::NonFiniteWeight { event_id: 2, .. }
                ),
                "got {err:?}"
            );
        }
    }

    #[test]
    fn finite_weights_support_overflow_rejected() {
        // K-3 CE2: ham girdiler sonlu; KANONİK kümenin toplamı taşar → RED.
        let omega = WitnessSet::new(vec![
            ev_w(1, "a", WitnessKind::MergeCommit, 200, 1, f64::MAX),
            ev_w(2, "b", WitnessKind::MergeCommit, 300, 1, f64::MAX),
        ]);
        let err = omega.canonicalize_for(999).unwrap_err();
        assert!(
            matches!(err, WitnessNumericsError::NonFiniteSupport { .. }),
            "got {err:?}"
        );
        // Ters yön (tur-2 P2-1): author'un f64::MAX'i inv #1 ile dışlanır →
        // kalan kanonik küme SONLU → geçerli küme yanlış reddedilMEZ.
        let omega_author = WitnessSet::new(vec![
            ev_w(1, "a", WitnessKind::MergeCommit, 100, 1, f64::MAX), // author
            ev_w(2, "b", WitnessKind::MergeCommit, 200, 1, f64::MAX),
        ]);
        let canon = omega_author
            .canonicalize_for(100)
            .expect("author dışlanınca kalan support sonlu — geçerli");
        assert_eq!(canon.approver_count(), 1);
    }

    #[test]
    fn harness_zero_quorum_remains_valid() {
        // K-2 bilinçli-geçerli pini: (0, 0.0) = HarnessAutoApprove tanıksız-rotası —
        // sayısal geçerlilik bunu HARİÇ TUTMAZ (sayısal ≠ yetkilendirme politikası).
        let claim = claim_with_author(100);
        let omega = WitnessSet::new(vec![]).with_quorum(0, 0.0);
        let result = evaluate(&claim, &omega).expect("(0, 0.0) geçerli konfigürasyon");
        assert!(matches!(result, WitnessDisposition::Satisfied { .. }));
    }

    #[test]
    fn zero_weight_is_valid() {
        // Sıfır ağırlık = nötr tanık — zararsız, geçerli (yalnız negatif/sonsuza RED).
        let omega = WitnessSet::new(vec![
            ev_w(1, "PR#1", WitnessKind::MergeCommit, 200, 1, 0.0),
            ev_w(2, "PR#2", WitnessKind::MergeCommit, 300, 1, 1.0),
        ]);
        let canon = omega.canonicalize_for(999).unwrap();
        assert_eq!(canon.approver_count(), 2);
        assert!((canon.support() - 1.0).abs() < 1e-9);
    }
}
