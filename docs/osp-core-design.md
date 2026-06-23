# OSP-Core — Tasarım Dokümanı

> Bu doküman `OSP-formalism.md`'i (matematik + API imzaları) **somut Rust mimarisine**
> çevirir. Faz 1.4-1.11 implementasyonu sırasında "hangi tip nereye, hangi trait nasıl"
> kararlarını önceden kilitler — rework önler.
>
> **Öncelik sırası:** bu doküman > `OSP-formalism.md §8` API taslağı > yorumsal kararlar.
> Çelişki olursa bu doküman bağlayıcıdır; formalizm güncellenir.
>
> **Sürüm:** 1.0 · Faz 1.4-1.11 reçetesi · `implementation-invariants.md` (10 inv) ile senkron

---

## 1. Amaç & Sınırlar

| Bu doküman şunları kilitler | Şunları değil |
|---|---|
| Modül sınırları + bağımlılık DAG'ı | Implementasyon detayları (algoritma içleri) |
| Tam tip tanımları (field + derive + doc) | Test fixture'lerin konkret içeriği |
| Hata stratejisi (thiserror vs anyhow) | Performance tuning (Faz 2) |
| Dosya layout'u | CI/CD pipeline |
| osp-spike ↔ osp-core köprü stratejisi | LLM entegrasyonu (Faz 5) |
| Faz 1.4-1.11 adımları (dosya/test/kabul) | Görselleştirme (Faz 6) |

---

## 2. Modül Mimarisi

### 2.1 Bağımlılık DAG'ı (döngüsüz)

```
                    space (foundation — generic graph)
                   /      |       \        \________
                  ↓       ↓        ↓                 ↓
              coords   witness   vision           bigbang
                 ↓                  ↑                ↑
               axes                 |                |
                 \__________________/                |
                                   ↘                 |
                                    time (FSM) ------/
                                   (orchestrates witness + bigbang)
```

**Kural:** Yukarıdaki ok "depends on" (import eder). Döngü yok. `space` foundation — hiçbir
osp-core modülüne bağımlı değil. `time` en üst katman (orchestrator).

### 2.2 Modül Sorumlulukları

| Modül | Sorumluluk | Ana tipler |
|---|---|---|
| `space` | Generic kavramsal graf: Node/Edge/Space + zaman katmanı. Witness-bilgili değil. | `Node`, `Edge`, `Space`, `NodeKind`, `EdgeKind`, `TimeLayer`, `GravityVector` |
| `coords` | Koordinat altyapısı: Raw/Derived ayrımı + pluggable axes. | `RawPosition`, `DerivedPosition`, `Position`, `Axis`, `CoordinateSystem` |
| `axes` | Somut eksen gerçeklemeleri (raw). Derived metric DEĞİL. | `CouplingAxis`, `EntropyAxis`, `WitnessDepthAxis`, (+Faz 1.9: `CohesionAxis`, `InstabilityAxis`) |
| `witness` | Şahitlik zinciri: Evidence → WitnessSet → Status → Result. Commit protokolü. | `EvidenceEvent`, `WitnessKind`, `WitnessSet`, `WitnessStatus`, `WitnessResult`, `evaluate()`, `Claim`, `Intent` |
| `vision` | Vizyon vektörü + sapma metrikleri. Derived hesap. | `VisionVector`, `DeviationMetric`, `CosineDeviation`, `DiffusionDeviation`, `compute_derived()` |
| `bigbang` | Uzay genişlemesi: mutation (`apply_delta`) + incremental recompute. | `apply_delta()`, `Delta`, `Event` |
| `time` | Zaman FSM: advance() orchestrates witness + bigbang. | `TimeMachine` trait, `TimeFSM` |

### 2.3 Katmanlı Olmayan (Semantic) Tipler

`Claim`, `Intent`, `AgentId` — `NodeKind::{Claim, Intent, Agent}` ile graf'a bağlı AMA semantik
payload `witness` modülünde. **Karar:** bu tipler `witness` modülünde; `space` yalnız `NodeId`
referansları tutar (generic). Böylece `space` witness-bilgili kalmaz, `witness` graph'a `NodeId`
üzerinden bağlanır.

---

## 3. Tam Tip Tanımları

> Faz 1.1-1.3'te gerçeklenen tipler (`Node`, `Edge`, `Space`, `Axis`, `CoordinateSystem`,
> `CouplingAxis`, `EntropyAxis`, `WitnessDepthAxis`) burada tekrar tanımlanmaz — kaynak kod
> bkz. Faz 1.4'te değişen kısımlar aşağıda.

### 3.1 `coords.rs` — RawPosition/DerivedPosition (Faz 1.4, inv #4)

```rust
/// 5 sabit core raw eksen. θ hesabının GİRDİSİ — dairesellik yok (inv #4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CoreRawPosition {
    pub x: f64,   // coupling
    pub y: f64,   // cohesion (Faz 1.9)
    pub z: f64,   // Martin Instability I (saf, inv #10)
    pub w: f64,   // entropy
    pub v: f64,   // witness-depth
}

/// N custom raw eksen (§2.2, değişken boyut). God Mode tarafından register edilen
/// AxisId → değer map. θ hesabına core ile birlikte girdi olur.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CustomRawPosition {
    pub values: HashMap<AxisId, MetricValue>,  // AxisId ("security.audit") → MetricValue
}

/// Tam raw: core + custom. θ hesabının GİRDİSİ (inv #4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawPosition {
    pub core: CoreRawPosition,
    pub custom: CustomRawPosition,
}

/// Raw + θ'dan türetilmiş. θ hesabına GİRDİ DEĞİL — çıktı (inv #4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DerivedPosition {
    pub u: f64,                       // vision alignment = 1 − θ_norm
    pub theta: f64,                   // sapma açısı (raw'dan hesaplanır)
    pub risk_score: f64,              // ileride: composite risk (Faz 2)
    pub main_sequence_distance: f64,  // D = |A + I − 1| (inv #10, ayrı metric)
}

/// Tam konum: raw (core+custom) + derived.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Position {
    pub raw: RawPosition,
    pub derived: DerivedPosition,
}

impl CoreRawPosition {
    /// CoordinateSystem core çıktısından (canonical x,y,z,w,v sırası) kur.
    pub fn from_canonical_slice(values: &[f64]) -> Self {
        Self {
            x: values.get(0).copied().unwrap_or(0.0),
            y: values.get(1).copied().unwrap_or(0.0),
            z: values.get(2).copied().unwrap_or(0.0),
            w: values.get(3).copied().unwrap_or(0.0),
            v: values.get(4).copied().unwrap_or(0.0),
        }
    }
}
```

**CoordinateSystem değişikliği (Faz 1.4 + custom axis §2.2):**
- `default_four()` → **kaldır** (VisionAlignmentAxis artık Axis değil)
- `CoordinateSystem { core: [Box<dyn Axis>; 5], custom: Vec<Box<dyn Axis>> }` — core sabit,
  custom God Mode tarafından register (inv #15, formalism §2.2)
- `default_core_five(coupling, cohesion, instability, entropy, witness_depth)` → Faz 1.9 preset.
  Faz 1.4'te `default_core_three(coupling, entropy, witness_depth)` (y, z henüz yok).
- `register_custom_axis(axis: Box<dyn Axis>)` — God Mode API, Agent çağıramaz (inv #15)
- `raw_position_of(node, space) -> RawPosition` → `(core, custom)` döner (slice değil struct).

### 3.2 `witness.rs` — Şahitlik Zinciri (Faz 1.5, inv #1,2,3,9)

```rust
/// Stabil tanımlayıcılar (içerik-adresli Faz 2'de).
pub type AgentId = u64;
pub type ClaimId = u64;
pub type EvidenceId = u64;
pub type EvidenceSource = String;   // "PR #42", "<sha>", "trailer:Reviewed-by:alice"

/// Şahit türü + ağırlık (§4.1 KARAR; kalibrasyon Faz 1.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WitnessKind {
    MergeCommit,       // weight 1.0
    PRMerged,          // weight 0.8
    TrailerReviewed,   // weight 0.7
    CoAuthored,        // weight 0.4
}

impl WitnessKind {
    pub fn default_weight(&self) -> f64 {
        match self {
            Self::MergeCommit => 1.0,
            Self::PRMerged => 0.8,
            Self::TrailerReviewed => 0.7,
            Self::CoAuthored => 0.4,
        }
    }
}

/// Tek gözlemlenen kanıt. Dedup birimi: (source, actor, claim) key (inv #2).
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceEvent {
    pub id: EvidenceId,
    pub source: EvidenceSource,
    pub witness_kind: WitnessKind,
    pub actor: AgentId,
    pub claim: ClaimId,
    pub weight: f64,   // = witness_kind.default_weight() (override edilebilir, Faz 1.11)
}

/// W(C, Ω)'nın Ω'sı. Claim için toplanmış ham kanıtlar (inv #9).
/// DİKKAT: `support()` / `approvers` doğrudan ÇAĞIRILMAZ — önce `canonicalize_for`
/// ile `CanonicalWitnessSet`'e çevir (inv #1 + #2 yapısal garanti).
#[derive(Debug, Clone, Default)]
pub struct WitnessSet {
    pub events: Vec<EvidenceEvent>,
    pub min_approvers: usize,       // default 2
    pub quorum_threshold: f64,      // θ_quorum, default 1.5
}

impl WitnessSet {
    pub fn new(events: Vec<EvidenceEvent>) -> Self {
        Self { events, min_approvers: 2, quorum_threshold: 1.5 }
    }

    /// **inv #1 + #2 — tek giriş noktası.** Author'ı çıkar + (source,actor,claim)
    /// dedup + approve filtrele → `CanonicalWitnessSet`. Q1/Q2 hesapları bunun
    /// çıktısı üzerinden yapılır. "dedup'i unutma" hatası imkânsız (parse-don't-validate).
    pub fn canonicalize_for(&self, author: AgentId) -> CanonicalWitnessSet { /* ... */ }
}

/// Dedup'lı + author-filtered + approve-only witness set.
/// Sadece `WitnessSet::canonicalize_for` ile oluşur — inv #1 (author excluded)
/// ve inv #2 (dedup) **yapısal garantili** (türlenmiş, runtime-check değil).
#[derive(Debug, Clone)]
pub struct CanonicalWitnessSet {
    events: Vec<EvidenceEvent>,  // dedup'lı, author dışı, approve verdict'li
    min_approvers: usize,
    quorum_threshold: f64,
}

impl CanonicalWitnessSet {
    /// Q1: distinct non-author approver sayısı.
    pub fn approver_count(&self) -> usize { self.events.len() }

    /// Q2: Σ weight (zaten dedup'lı — ekstra çağrı gerekmez).
    pub fn support(&self) -> f64 { self.events.iter().map(|e| e.weight).sum() }

    pub fn min_approvers(&self) -> usize { self.min_approvers }
    pub fn quorum_threshold(&self) -> f64 { self.quorum_threshold }
    pub fn events(&self) -> &[EvidenceEvent] { &self.events }
}

/// Tri-state epistemolojik durum (inv #3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessStatus {
    Witnessed,
    Unwitnessed,
    UnobservableLocally,   // squash/rebase + trailersız
}

/// W(C, Ω) sonucu (inv #9 + #7 admin override flag).
#[derive(Debug, Clone, PartialEq)]
pub enum WitnessResult {
    Commit {
        delta: crate::bigbang::Delta,
        safety_weakened: bool,        // inv #7 — admin override
        override_reason: Option<String>,
    },
    Reject(Reason),                   // Q3 (honest-reject) — witness-based
    Hold(Reason),                     // Q1 (min_approvers) veya Q2 (θ_quorum) — witness-based
}

// NOT: Claim-based gate'ler Q4 (syntax), Q5 (vision), Q6 (rule) `SpaceEngine::commit()`
// pipeline'ında evaluate()'den ÖNCE koşar (space-engine-design.md §4). Bu yüzden
// `evaluate()` yalnızca witness-based Q1-Q3'ü kontrol eder — Claim'in Q4-Q6'yı geçtiği
// varsayılır. Q4-Q6 failure'ları `EngineCommitError` olarak engine seviyesinde döner
// (space-engine-design.md §6.1) — `Reason` enum'ında DEĞİL.

/// Sadece witness-based gate failure'ları (Q1-Q3). Claim-based Q4-Q6 failure'ları
/// `EngineCommitError`'da (space-engine-design.md §6.1) — burada tekrar tanımlanmaz
/// (single-source-of-truth, duplication drift risk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    QuorumInsufficient { support: f64, threshold: f64 },       // Q2
    MinApproversNotMet { distinct: usize, required: usize },   // Q1
    HonestReject { witness: AgentId },                         // Q3
    UnobservableLocally { hint: String },   // squash/rebase — provider API önerisi
}

/// Claim ve Intent (witness-domain; space'e NodeId ile bağlı).
#[derive(Debug, Clone)]
pub struct Intent {
    pub agent: AgentId,
    pub target_raw: RawPosition,   // raw — derived değil (inv #4)
    // Time layer: Intent her zaman t_f (Gelecek) — agent-prompt-semantics.md §0 ontolojik harita.
    // Implementasyonda ya `time_layer: TimeLayer::Gelecek` alanı (invariant'ı tip-seviyesinde
    // zorlamak için) ya da constructor'da sabitlenir. Intent t_m/t_c'de yaşayamaz.
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub id: ClaimId,
    pub intent: Intent,
    pub author: AgentId,           // inv #1 — evaluate'de approvers'tan çıkarılır
    /// Engine tarafından DeltaProposal'dan **compute edilmiş** raw position.
    /// LLM tarafından declare EDİLMEZ (inv #4, agent-prompt-semantics.md §2.2).
    /// `computed_raw` = coord_system/analyzer'ın ΔS'i uygulayıp node'ları yeniden
    /// ölçmesinin sonucu — agent'ın iddia ettiği pozisyon değil, actual measured pozisyon.
    pub computed_raw: RawPosition,
    pub delta_nodes: Vec<crate::space::Node>,
    pub delta_edges: Vec<crate::space::Edge>,
}

/// W(C, Ω) — pure karar fonksiyonu (mutasyon yok). Sadece witness-based Q1-Q3.
/// Claim-based Q4-Q6 (`SpaceEngine::commit()` Phase 0) zaten geçti varsayılır.
pub fn evaluate(claim: &Claim, omega: &WitnessSet) -> WitnessResult {
    let canon = omega.canonicalize_for(claim.author);  // inv #1 + #2 yapısal
    // Q1: canon.approver_count() >= canon.min_approvers()
    // Q2: canon.support() >= canon.quorum_threshold()
    // Q3: ∀ honest W ∈ canon: W.verdict ≠ Reject
    // (Q4-Q6 claim-based — engine'da evaluate() öncesi kontrol edildi)
}
```

### 3.3 `vision.rs` — Sapma + Derived (Faz 1.7, inv #4,5,10)

```rust
/// Elle-deklare vizyon (§5.1). Raw — derived değil.
#[derive(Debug, Clone, Copy)]
pub struct VisionVector(pub RawPosition);

/// θ trait — SADECE RawPosition alır (inv #4 compile-time koruma).
pub trait DeviationMetric: Send + Sync {
    fn theta(&self, raw: &RawPosition, vision: &VisionVector, space: &crate::space::Space) -> f64;
}

pub struct CosineDeviation;                  // naif — commit path'inde (inv #5)
pub struct DiffusionDeviation { pub t: f64 } // spektral — yalnızca analyze (inv #5)

/// RawPosition + VisionVector → DerivedPosition hesapla.
pub fn compute_derived(
    raw: &RawPosition,
    vision: &VisionVector,
    space: &crate::space::Space,
    metric: &dyn DeviationMetric,
    instability: f64,   // I (zaten raw'da ama Abstractness A ayrı gerekli — D için)
    abstractness: f64,  // A
) -> DerivedPosition {
    // θ = metric.theta(raw, vision, space)
    // u = (1 − θ_norm).clamp(0,1)
    // D = |A + I − 1| (inv #10)
    // risk_score = ... (Faz 2)
}
```

### 3.4 `bigbang.rs` — Mutation (`apply_delta`) (Faz 1.8, inv #5,6,7)

> **Sorumluluk ayrımı (Faz 2 sync — `space-engine-design.md §3.5` ile):**
> - `witness::evaluate(claim, omega) -> WitnessResult` → sadece Q1-Q3 karar (Commit/Reject/Hold)
> - `bigbang::apply_delta(space, delta) -> Vec<NodeId>` → **sadece mutasyon** (node/edge ekle, repositioned işaretle)
> - `SpaceEngine::commit(claim, omega) -> CommitOutcome` → **full orchestration**: Q4-Q6 claim-based gate → Q1-Q3 witness (`evaluate`) → `apply_delta` mutasyon → reposition → save delta
>
> `bigbang::commit()` artık **yok** — evaluate + mutasyon ayrıldı. Production path'inde tek
> orchestration noktası `SpaceEngine::commit()`. `apply_delta` hem live commit hem event-sourcing
> replay için reusable (space-engine-design.md §3.5).

```rust
/// Uzay genişlemesi sonucu (mutasyon çıktısı).
#[derive(Debug, Clone)]
pub struct Delta {
    pub new_nodes: Vec<crate::space::NodeId>,
    pub new_edges: Vec<crate::space::Edge>,
    pub repositioned: Vec<crate::space::NodeId>,   // ΔV ∪ N₁(ΔV) — inv #6
}

#[derive(Debug, Clone)]
pub struct Event {
    pub time_layer: crate::space::TimeLayer,
    pub claim_id: crate::witness::ClaimId,
    pub delta: Delta,
    pub safety_weakened: bool,   // inv #7
}

/// apply_delta: sadece mutasyon (evaluate'siz). Live commit + event-sourcing replay
/// için reusable. Reposition `SpaceEngine::commit()`'in sorumluluğunda (inv #5: CosineDeviation).
pub fn apply_delta(
    space: &mut crate::space::Space,
    delta: &Delta,
) -> Vec<crate::space::NodeId> { /* node/edge ekle, repositioned işaretle, return affected */ }
```

> **Not:** `CommitError` (Hold/Reject wrapping) artık `SpaceEngine::commit()` → `EngineCommitError`
> içinde (space-engine-design.md §6.1). `bigbang` modülü mutasyon-only olduğu için kendi error
> enum'ı yoktur — `apply_delta` infallible'dır (geçerli delta her zaman uygulanır).

### 3.5 `time.rs` — FSM (Faz 1.8)

```rust
pub trait TimeMachine {
    fn advance(
        &mut self,
        space: &mut crate::space::Space,
        claim: &crate::witness::Claim,
        omega: &crate::witness::WitnessSet,
    ) -> crate::witness::WitnessResult;
}

pub struct TimeFSM;   // statelessVarsayılan — Faz 2'de snapshot/persist ekler

impl TimeMachine for TimeFSM {
    fn advance(&mut self, space, claim, omega) -> WitnessResult {
        // 1. evaluate(claim, omega) → Q1-Q3 karar (Commit/Reject/Hold)
        // 2. Commit ise bigbang::apply_delta(space, &delta) çağır, Event emit
        //    (mutasyon only — Q4-Q6 claim-based gate ve reposition SpaceEngine::commit()'te)
        // 3. WitnessResult döndür
    }
}
```

---

## 4. Hata Stratejisi

| Modül | Strateji | Gerekçe |
|---|---|---|
| `space`, `coords`, `axes` | Pure fonksiyonlar **infallible**; nadir hatalar typed enum | Veri tipleri + hesap — `anyhow` YOK |
| `witness::evaluate` | **Infallible** (`WitnessResult` dönüyor, `Result` değil) | Karar logic — hata değil sonuç (Commit/Reject/Hold) |
| `bigbang::apply_delta` | **Infallible** (`Vec<NodeId>` döner — mutasyon, her zaman başarılı) | Geçerli delta her zaman uygulanır; error yok |
| `vision::DiffusionDeviation::theta` | **`thiserror::DeviationError`** (Faz 1.7+) | LAPACK/spektral hesap başarısız olabilir |
| `engine::commit` | **`thiserror::EngineCommitError`** enum (Q4-Q6 + witness) | Claim-based + witness-based gate failure'ları caller match etmeli |
| `time::advance` | **Infallible** (`WitnessResult`) | evaluate + apply_delta'nın kompozisyonu |

**Kural (çelişkisiz):** `osp-core` library seviyesinde **typed errors only** (`thiserror`).
`anyhow` **kullanılmaz** — caller match edemez. Pure fonksiyonlar infallible; hata gereken
yerde typed enum zorunlu. `anyhow` yalnızca `osp-spike` CLI/bin seviyesinde (binary boundary).

---

## 5. Test Stratejisi

### 5.1 Üç Katmanlı Test

| Katman | Nerde | Ne |
|---|---|---|
| **Birim** | `src/*.rs` içinde `#[cfg(test)] mod tests` | Fonksiyon/tip bazlı, izole |
| **Invariant** | `tests/invariants.rs` (integration) | 15 invariant'ın her biri → 1 test |
| **Compile-fail** | `tests/ui/*.rs` + `trybuild` | inv #4: `theta(full_Position)` derleme hatası |

### 5.2 İnvariant Test Haritası (`tests/invariants.rs`)

| Test | İnvariant | Ne yapar |
|---|---|---|
| `inv01_author_excluded` | #1 | `WitnessSet::canonicalize_for(author)` sonrası `CanonicalWitnessSet` author içermiyor |
| `inv02_dedup_strongest` | #2 | aynı (source,actor,claim) iki event → `canonicalize_for` en yüksek weight'ı tutuyor |
| `inv03_unobservable_tristate` | #3 | squash+trailersız → `UnobservableLocally`, `Unwitnessed` değil |
| `inv04_theta_rejects_full_position` | #4 | `trybuild`: `theta(&Position, ...)` compile error |
| `inv05_commit_uses_cosine` | #5 | `commit()` sonrası diffusion çağrı sayacı = 0 |
| `inv06_commit_incremental` | #6 | `commit()` sonrası sadece ΔV ∪ N₁(ΔV) reposition |
| `inv07_admin_override_flagged` | #7 | admin override → `safety_weakened: true` |
| `inv08_no_network_deps` | #8 | `osp-core/Cargo.toml` `reqwest`/`gh` içermiyor |
| `inv09_witnessset_quorum` | #9 | 3 zayıf → Commit; 1 güçlü → Hold |
| `inv10_z_pure_I_D_separate` | #10 | `InstabilityAxis` pure I; `D` ayrı `DerivedPosition` field |
| `inv11_llm_runtime_stateless` | #11 | Faz 5: `osp-llm-runtime` struct'ı `&mut self` yok, config dışında field yok |
| `inv11_agent_shell_preserves_state` | #11 | Faz 5: LLM restart sonrası Agent kabuğu aynı Intent'i takip eder |
| `inv12_q4_syntax_reject_before_witness` | #12 | hatalı-formatlı Claim → `WitnessSet` çağrılmadan `Err(SyntaxViolation)` |
| `inv13_permission_mask_immutable` | #13 | Agent kabuğu `mask.writable_axes`'e yeni axis ekleyemez (immutable ref) |
| `inv13_commit_rejects_unauthorized` | #13 | PermissionMask dışı node'a yazma → `Err(PermissionDenied)` nihai gate |
| `inv14_prompt_typed_not_text` | #14 | Faz 5: `OspPrompt` serialize çıktısı şemaya uyuyor, serbest metin alanı yok |
| `inv14_llm_output_matches_contract` | #14 | Faz 5: şema-dışı LLM çıktısı → Q4 `SyntaxViolation` |

### 5.3 Property Tests (opsiyonel, Faz 1.11)

`proptest` ile:
- Tüm eksen `compute()` çıktıları ∈ [0, 1]
- `WitnessSet::support()` ≤ Σ max-weights
- `commit()` idempotent değil AMA rollback'able (Faz 2)

---

## 6. Dosya Layout

```
crates/osp-core/
├── Cargo.toml
├── tests/
│   ├── invariants.rs                          # 15 invariant integration test
│   └── ui/
│       ├── theta_rejects_full_position.rs     # trybuild compile-fail (inv #4)
│       └── theta_rejects_full_position.stderr # beklenen hata
└── src/
    ├── lib.rs            # Faz 1.1 (modül bildirimleri + re-exports)
    ├── space.rs          # Faz 1.1 (exists) — generic graf
    ├── coords.rs         # Faz 1.2 (exists) + Faz 1.4 (RawPosition/DerivedPosition)
    ├── axes.rs           # Faz 1.3 (exists) + Faz 1.9 (CohesionAxis, InstabilityAxis)
    ├── witness.rs        # Faz 1.5 (yeni) — Evidence → Result zinciri
    ├── vision.rs         # Faz 1.7 (yeni) — VisionVector + Deviation + compute_derived
    ├── bigbang.rs        # Faz 1.8 (yeni) — apply_delta + Delta + Event (mutation-only, infallible)
    └── time.rs           # Faz 1.8 (yeni) — TimeMachine FSM
```

### 6.1 `Cargo.toml` (Faz 1.4 son hedef)

```toml
[package]
name = "osp-core"
version.workspace = true
# ...

[dependencies]
serde = { workspace = true, optional = true }
thiserror = "1"           # Faz 1.8 (EngineCommitError engine seviyesinde, DeviationError vision'da)
# Faz 1.5+: git2 = "0.18" (lokal God-mode witness — inv #8: osp-core DEĞİL, osp-analyzer)

[dev-dependencies]
trybuild = "1"            # inv #4 compile-fail test
proptest = "1"            # property tests (opsiyonel)

[features]
default = []
serde = ["dep:serde"]     # Faz 2 persistence
```

**inv #8 doğrulaması:** CI'da `cargo tree -p osp-core | grep -E 'reqwest|gh'` boş dönmeli.

---

## 7. osp-spike ↔ osp-core Köprüsü (Faz 1.6 + 1.10)

### 7.1 Faz 1.6: osp-spike, osp-core'a bağımlı olur

```toml
# crates/osp-spike/Cargo.toml (Faz 1.6)
[dependencies]
osp-core = { path = "../osp-core" }
# ... mevcut tree-sitter, git, serde
```

### 7.2 Köprü Fonksiyonları (Faz 1.6, `osp-spike/src/bridge.rs`)

osp-spike'ın Faz 0 tipleri → osp-core tipleri:

```rust
/// osp-spike DepGraph → osp-core Space (generic graf).
pub fn spike_graph_to_space(g: &DepGraph) -> osp_core::space::Space { /* ... */ }

/// osp-spike WitnessProfile → osp-core WitnessSet (EvidenceEvent'lere ayrışır).
/// inv #3 tri-state: squash+trailersız → UnobservableLocally sinyali.
pub fn spike_witness_to_set(
    w: &WitnessProfile,
) -> (osp_core::witness::WitnessSet, osp_core::witness::WitnessStatus) { /* ... */ }
```

### 7.3 Faz 1.10: Re-spike

osp-spike `main.rs`'i `--v2` flag'i ekler: osp-core `evaluate` + tri-state kullanır. Eski
Faz 0 metrikler `--legacy` ile korunur (karşılaştırma için). Çıktı: her repo için
`WitnessStatus` etiketi (fastapi/django → `UnobservableLocally` veya `Witnessed` — squash
kör-noktası çözümünün kanıtı).

---

## 8. Faz 1.4-1.11 Somut Reçete

### Faz 1.4 — RawPosition/DerivedPosition refactor (inv #4)

| Dosya | Değişiklik |
|---|---|
| `coords.rs` | `RawPosition`, `DerivedPosition`, `Position` tipleri ekle; `default_four` → kaldır; `default_raw_three` preset; `raw_position_of` metodu |
| `axes.rs` | `VisionAlignmentAxis`'ı CoordinateSystem preset'ten çıkar (derived'a taşınacak — Faz 1.7'de `vision.rs`); `default_four` test'lerini güncelle |
| `space.rs` | `Node.position: Position` (Vec<f64> → Position struct) — backward-incompatible AMA Faz 1.x |

**Test'ler:** `raw_position_has_five_fields`, `derived_position_has_four_fields`, `default_raw_three_excludes_vision_alignment`
**Kabul:** `cargo test -p osp-core` yeşil; `VisionAlignmentAxis` preset'te değil.

### Faz 1.5 — EvidenceEvent + WitnessSet + W(C,Ω) + tri-state (inv #1,2,3,9)

| Dosya | Değişiklik |
|---|---|
| `witness.rs` | Yeni modül: §3.2'deki tüm tipler + `evaluate()` |
| `lib.rs` | `pub mod witness;` |
| `tests/invariants.rs` | `inv01`, `inv02`, `inv03`, `inv09` |

**Test'ler:** `evaluate_returns_commit_for_quorum`, `evaluate_returns_hold_for_single_strong` (self-merge), `evaluate_returns_reject_for_honest_reject`, `dedup_keeps_strongest_drops_weaker`, `squash_no_trailer_yields_unobservable`
**Kabul:** Tüm quorum kombinasyonları (2 güçlü, 3 zayıf, maintainer+CI) doğru sınıflanır.

### Faz 1.6 — Intent/Claim + osp-spike köprüsü

| Dosya | Değişiklik |
|---|---|
| `witness.rs` | `Claim`, `Intent` tipleri (§3.2) finalize |
| `osp-spike/Cargo.toml` | `osp-core` dep ekle |
| `osp-spike/src/bridge.rs` | Yeni: `spike_graph_to_space`, `spike_witness_to_set` |

**Test'ler:** `bridge_graph_preserves_node_count`, `bridge_witness_correct_tristate`
**Kabul:** osp-spike, osp-core tipleri üretebilir.

### Faz 1.7 — VisionVector + CosineDeviation + D derived (inv #4,10)

| Dosya | Değişiklik |
|---|---|
| `vision.rs` | Yeni modül: §3.3'teki tipler; `compute_derived`; `VisionAlignmentAxis` mantığı `compute_derived`'a taşınır |
| `lib.rs` | `pub mod vision;` |

**Test'ler:** `theta_uses_raw_only`, `u_is_one_minus_theta`, `D_separate_from_z` (inv #10), `compute_derived_full`
**Kabul:** `compute_derived(RawPosition, VisionVector) → DerivedPosition` çalışır; D ayrı.

### Faz 1.8 — TimeMachine FSM + commit() incremental (inv #5,6,7)

| Dosya | Değişiklik |
|---|---|
| `bigbang.rs` | Yeni: `Delta`, `Event`, `apply_delta()` (mutation-only, incremental recompute — infallible) |
| `time.rs` | Yeni: `TimeMachine` trait, `TimeFSM` |
| `lib.rs` | `pub mod bigbang; pub mod time;` |
| `Cargo.toml` | `thiserror = "1"` |
| `tests/invariants.rs` | `inv05`, `inv06`, `inv07` |

**Test'ler:** `commit_returns_event_on_commit_result`, `commit_returns_hold_error`, `commit_repositions_only_delta_neighbors` (inv #6), `admin_override_sets_safety_weakened` (inv #7), `commit_path_no_diffusion` (inv #5)
**Kabul:** commit() mutasyon + Event üretir; incremental; admin override flag'li.

### Faz 1.9 — y (LCOM4) + z (Martin I) raw eksenleri (inv #10)

| Dosya | Değişiklik |
|---|---|
| `axes.rs` | `CohesionAxis` (LCOM4 tree-sitter pseudo-type, §10 Q8 metodoloji), `InstabilityAxis` (saf Martin I) |
| `coords.rs` | `default_raw_five` preset (x,y,z,w,v) |

**Test'ler:** `instability_axis_returns_pure_I` (inv #10), `cohesion_zero_for_isolated_class`, `default_raw_five_includes_all`
**Kabul:** 5 raw eksen preset hazır; LCOM4 heuristic SCIP-ground-truth ile kıyaslanabilir (Faz 3).

### Faz 1.10 — Re-spike (w_ratio_v2 + tri-state)

| Dosya | Değişiklik |
|---|---|
| `osp-spike/src/main.rs` | `--v2` flag; osp-core `evaluate` + `WitnessStatus` kullan |
| `docs/spike-results-v2.md` | Yeni: 5 repo tri-state etiketli |

**Kabul:** fastapi/django/date-fns `Witnessed` veya doğru biçimde `UnobservableLocally` (squash kör-noktası çözüldü).

### Faz 1.11 — Kalibrasyon korpusu

| Dosya | Değişiklik |
|---|---|
| `docs/calibration-corpus.md` | 15-20 repo (Py/Rust/TS/Go), maturity/workflow çeşitliliği |
| `scripts/calibrate.sh` | korpusu koş, weight/θ_quorum/t* tune |

**Kabul:** weight'ler ampirik olarak tune edilmiş; θ_quorum=1.5 doğrulanmış veya revize.

---

## 9. Açık Tasarım Soruları (Implementasyon Sırasında)

1. **Faz 1.4 — `Node.position` breaking change:** `osp-spike` Faz 0 çıktıları `Vec<f64>`
   bekliyor. Ya (a) bridge adapter'i güncelle, ya (b) osp-spike'ı aynı anda refactor et.
   **Eğilim:** (a) — osp-spike Faz 0 artifact'i, donmuş.
2. **Faz 1.5 — `EvidenceEvent.id` üretimi:** sequential (`AtomicU64`) vs hash. **Eğilim:**
   hash (içerik-adresli, Faz 2 dedup kolaylığı).
3. **Faz 1.7 — `compute_derived` imzası:** A (abstractness) parametre olarak dışarıdan
   gelmeli (LCOM4/Abstractness tree-sitter'dan). `InstabilityAxis` zaten z (I) veriyor; A
   ayrı hesap. **Eğilim:** `compute_derived` `abstractness: f64` parametresi alır (Faz 1.9'da
   `AbstractnessAxis` eklenince).
4. **Faz 1.8 — `commit` atomiklik:** space mutation + Event emit ayrılabilir mi (rollback)?
   **Eğilim:** Faz 1.8'de tek-pass atomik; Faz 2 snapshot/rollback ekler.
5. **Faz 1.10 — `--legacy` karşılaştırma:** Faz 0 metrikleri korumaya değer mi? **Eğilim:**
   evet — `docs/spike-results.md` A/B karşılaştırması makale için güçlü.

---

## 10. Karar Özeti

| Karar | Seçim | İnvariant |
|---|---|---|
| Tip ayrımı (Raw vs Derived) | Farklı struct'lar, `theta(&RawPosition)` | #4 |
| Author witness rejection | `WitnessSet::non_author_approvers` | #1 |
| Evidence dedup | `(source, actor, claim)` HashMap, max-weight | #2 |
| Tri-state witness | `WitnessStatus` enum | #3 |
| W operatörü | `evaluate(claim, &WitnessSet) -> WitnessResult` (yalnız Q1-Q3) | #9 |
| Lazy diffusion | `commit` CosineDeviation kullanır | #5 |
| Incremental Big Bang | `ΔV ∪ N₁(ΔV)` recompute; k=2 ayrı (projeksiyon) | #6 |
| Admin override flag | `WitnessResult::Commit { safety_weakened }` | #7 |
| osp-core network-free | `osp-core/Cargo.toml` `reqwest`/`gh` YOK | #8 |
| z = I saf, D ayrı | `InstabilityAxis` pure I; `DerivedPosition.main_sequence_distance` | #10 |
| Intent zaman katmanı | `t_f` (Gelecek) — ontolojik harita (agent-prompt-semantics.md §0) | — |
| Q4-Q6 claim-based gates | Syntax/Vision/Rule — `SpaceEngine::commit()` Phase 0, witness öncesi | #12 |
| PermissionMask denetimi | Üç nokta: slice → Agent kabuğu → `commit()` nihai | #13 |
| LLM durumsuz | LLM runtime stateless; durum Agent kabuğunda (Faz 5) | #11 |
| Prompt tiplenmiş paket | `OspPrompt` struct, doğal dil değil (Faz 5) | #14 |
| Hata stratejisi | Library: `thiserror`; CLI: `anyhow` | — |
| Test katmanı | Birim + invariant + trybuild | all |

---

*Sonraki: Faz 1.4 implementasyonu (RawPosition/DerivedPosition refactor). Bu doküman reçetedir.*
