# OSP Space Engine — Tasarım Dokümanı (Faz 2)

> Bu doküman Faz 1'in osp-core tiplerini **üretim runtime'ına** dönüştürür: Space Engine
> (orchestrator) + persistence (snapshot/time-travel) + vision deklarasyon parse + commit pipeline.
>
> **Öncelik:** bu doküman > `OSP-formalism.md §6` (Big Bang) > yorumsal kararlar.
> **Girdiler:** `osp-core-design.md` (Faz 1 tipleri), `implementation-invariants.md` (10 inv),
> `calibration-results.md` (Faz 1.11 parametreler).
>
> **Sürüm:** 1.0-draft · Faz 2 implementasyon öncesi

---

## 1. Amaç & Sınırlar

| Faz 2 şunları kilitler | Şunları değil |
|---|---|
| `SpaceEngine` orchestrator (space + coords + vision + time) | LLM entegrasyonu (Faz 5) |
| Commit pipeline: evaluate → mutate → **reposition** → snapshot | SCIP semantic analysis (Faz 3) |
| Persistence: snapshot save/load + time-travel | Multi-repo sync (Faz 4) |
| Vision deklarasyon parse (TOML → `VisionVector`) | Malicious witness detection (Faz 4) |
| Vision violation reporting (`θ > θ_bound`) | Dashboard görselleştirme (Faz 6) |

**Faz 2 çıktısı:** `osp-core`'a `engine.rs` + `persistence.rs` + `vision_config.rs` modülleri eklenir.
Space Engine, Faz 5 God-mode CLI + Faz 6 dashboard tarafından sürücülenen canlı runtime.

---

## 2. Mimari

### 2.1 Modül DAG'ı (Faz 2 eklentileri kalın)

```
                    space (foundation)
                   /      |       \        \________
                  ↓       ↓        ↓                 ↓
              coords   witness   vision           bigbang
                 ↓                  ↑                ↑
               axes                 |                |
                 \__________________/                |
                                   ↘                 |
                                    time (FSM) ------/
                                   ↓
                              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
                              ┃   engine (Faz 2 — orchestrator)  ┃
                              ┃   ├── persistence (snapshot)     ┃
                              ┃   └── vision_config (TOML parse) ┃
                              ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

`engine` en üst katman — tüm Faz 1 modüllerini orkestre eder. `persistence` + `vision_config`
engine'in yardımcıları.

### 2.2 Modül Sorumlulukları

| Modül | Sorumluluk | Ana tipler |
|---|---|---|
| `engine` | SpaceEngine: space + coords + vision + time orchestration. Commit pipeline. | `SpaceEngine`, `EngineConfig`, `CommitOutcome`, `VisionViolation` |
| `persistence` | Snapshot save/load (bincode). Time-travel. | `SpaceSnapshot`, `SnapshotStore` |
| `vision_config` | TOML vision deklarasyon → `VisionVector` + policies | `VisionConfig`, `VisionPolicies`, `VisionThresholds` |

---

## 3. Persistence Kararı (Custom Binary, Graph DB değil)

> `roadmap.md` Faz 2: "Persistence Layer — KùzuDB / SurrealDB / SQLite+graph / custom binary
> karar". Bu bölüm kararı verir.

### 3.1 Seçim: Custom Binary (`serde` + `bincode`)

| Alternatif | Ret gerekçesi |
|---|---|
| **KùzuDB** (embedded graph DB) | Ağır bağımlılık; runtime graph ops zaten in-memory HashMap + Vec ile O(1)/O(deg) — DB'ye gerek yok Faz 2 ölçeğinde |
| **SurrealDB** | Multi-model ama OSP'nin ihtiyacı tek-model (graph + time snapshot); overkill |
| **SQLite + graph layer** | Relational emülasyon grafik için verbose; OSP whole-space load yapıyor (partial query değil) |
| **Custom binary (serde + bincode)** ✅ | Minimal dep, hızlı (de)serialize, whole-space snapshot, time-travel dosya-başına |

### 3.2 Strateji: Event Sourcing (Milestone Snapshot + Delta Replay)

> **Reviewer düzeltmesi (Faz 2 tasarım review):** Periyodik tam-snapshot disk'i patlatır
> (Django 34k commit / 100 = 340 snapshot × ~500KB ≈ 170MB; Linux-kernel ölçeğinde felaket).
> Çözüm: **event-sourcing** — tam snapshot yalnızca milestone'larda, her commit'in `Delta`'sı ayrı.

| Katman | Ne zaman | Boyut (3k node) |
|---|---|---|
| **Milestone snapshot** (tam Space) | `git tag`, manuel `save_milestone("v1.0")`, periyodik (her ~1000 commit) | ~500KB — nadir |
| **Delta record** (ΔV + ΔE + repositioned) | **Her commit** | ~5KB — sık |

**Disk tasarrufu:** 340 × 500KB = 170MB → 1 milestone + 340 delta = ~2.2MB (**%98 azalma**).

### 3.3 Formatlar (version field — reviewer #4)

```rust
pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;  // bincode sürüm yönetimi

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpaceSnapshot {
    pub version: u32,              // = SNAPSHOT_FORMAT_VERSION; uyumsuzluk → graceful error
    pub t_c: u64,
    pub timestamp_ms: u128,
    pub space: Space,
    pub engine_config: EngineConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeltaRecord {
    pub version: u32,
    pub t_c: u64,
    pub claim_id: ClaimId,
    pub delta: Delta,              // new_nodes + new_edges + repositioned (Faz 1.8)
    pub safety_weakened: bool,
}
```

**Sürüm yönetimi:** `version` uyumsuzluğu deserialize hatası yerine `Err(VersionMismatch)`
döner — geliştirme sırasında eski snapshot'lar açılmazsa temiz panic değil bildirim.

### 3.4 `SnapshotStore` (Milestone + Delta + Replay)

```rust
pub struct SnapshotStore {
    milestones_dir: PathBuf,
    deltas_dir: PathBuf,
}

pub struct RestoredState {
    pub space: Space,
    pub t_c: u64,
    pub replayed_deltas: usize,   // diagnostic
}

impl SnapshotStore {
    pub fn save_milestone(&self, name: &str, snapshot: SpaceSnapshot) -> Result<()>;
    pub fn save_delta(&self, record: DeltaRecord) -> Result<()>;

    /// Event-sourcing restore (reviewer #3): en yakın milestone ≤ request_t_c + delta replay.
    pub fn restore(&self, request_t_c: u64) -> Result<RestoredState>;

    pub fn list_milestones(&self) -> Result<Vec<(String, u64)>>;
    pub fn list_deltas(&self) -> Result<Vec<u64>>;
}
```

### 3.5 Restore = Milestone + Delta Replay

```
restore(request_t_c):
  1. milestones_dir'da t_c ≤ request_t_c olan EN BÜYÜK milestone'u bul
     (yoksa en eski → 0'dan replay, yoksa Err(NoMilestone))
  2. milestone'ı deserialize → Space @ milestone_t_c
  3. deltas_dir'den milestone_t_c < t_c ≤ request_t_c arası DeltaRecord'ları sırayla yükle
  4. her Delta'ı apply_delta(space, &delta) ile replay (node/edge ekle, repositioned işaretle)
  5. → RestoredState { space @ request_t_c, replayed_deltas: count }
```

`bigbang::apply_delta(space, &Delta) -> Vec<NodeId>` — **canonical mutasyon fonksiyonu**
(sadece node/edge ekle + repositioned işaretle, evaluate'siz). Hem `SpaceEngine::commit()`
live path hem event-sourcing replay için reusable. Eski `bigbang::commit()` (evaluate + mutasyon
birleşik) ayrıldı: evaluate → `witness::evaluate()`, mutasyon → `apply_delta()` (osp-core-design.md §3.4).

### 3.6 inv #8 Uyumu

`serde` + `bincode` + `toml` **network crate değil** (inv #8: osp-core `reqwest`/`gh` YOK).

---

## 4. Commit Pipeline (Faz 2'nin Kalbi)

`SpaceEngine::commit(claim, omega)` — full flow. Gate'ler iki kategoriye ayrıldı
(`agent-prompt-semantics.md §4` ile senkron):

- **Q4-Q6 (claim-based, deterministik):** Claim'in kendi içsel özellikleri. Witness'lardan
  **önce** koşar — sentaks/vision/rule hatası olan bir Claim şahitlere gösterilmez (inv #12).
- **Q1-Q3 (witness-based):** `WitnessSet Ω` üzerinden. Sadece Q4-Q6 geçen Claim'ler buraya gelir.

```
┌─────────────────────────────────────────────────────────────────┐
│ 0. CLAIM-BASED GATES (Q4-Q6 — MUTASYON ÖNCESİ, DETERMİNİSTİK)    │
│    Q4 Syntax:   ΔProposal OutputContract'a uyuyor mu?            │
│                 → değilse Err(SyntaxViolation) [inv #12]         │
│    Q5 Vision:   θ(claim.computed_raw, vision, CosineDeviation)   │
│                 > θ_bound → Err(VisionViolation)                 │
│    Q6 Rule:     ∀ Rule R: R(ΔS) = Violated?                      │
│                 → evet ise Err(RuleViolation)                    │
│    [HİÇBİR mutasyon YAPILMAZ — formalism §4.3, BFT Safety]       │
├─────────────────────────────────────────────────────────────────┤
│ 1. WITNESS-BASED GATES (Q1-Q3) + TIME ADVANCE                    │
│    time.advance(space, claim, omega) → WitnessResult             │
│    └─ evaluate (Q1 min_approvers, Q2 quorum, Q3 no-honest-reject)│
│    └─ Commit ise bigbang::apply_delta mutasyon                    │
│       └─ ΔV node + ΔE edge ekle                                 │
│       └─ repositioned = ΔV ∪ N₁(ΔV) işaretle (inv #6)          │
├─────────────────────────────────────────────────────────────────┤
│ 2. REPOSITION (inv #5: CosineDeviation) + DRIFT WARNING         │
│    for id in delta.repositioned:                                │
│      raw = coord_system.raw_position_of(node, space)             │
│      derived = compute_derived(...)                              │
│      node.position = Position { raw, derived }                   │
│      if derived.theta > theta_bound: drift_warnings.push(...)    │
│    (neighbor drift = WARNING; claim Q5 = REJECT — ayrım §4.1)   │
├─────────────────────────────────────────────────────────────────┤
│ 3. SAVE DELTA (event-sourcing, §3.2)                            │
│    snapshot_store.save_delta(DeltaRecord { t_c, delta, ... })    │
│    (milestone snapshot'lar ayrı — tag/manual/periyodik)         │
├─────────────────────────────────────────────────────────────────┤
│ 4. EMIT                                                          │
│    return CommitOutcome { event, drift_warnings, safety_weakened }│
└─────────────────────────────────────────────────────────────────┘
```

### 4.1 Safety Katmanları (reviewer #1)

İki seviye vision kontrolü — **REJECT vs WARN ayrımı** formalizm'e uyumlu:

| Seviye | Ne | Ne zaman | Sonuç |
|---|---|---|---|
| **Q5 Claim Safety** | `θ(claim.computed_raw, vision) > θ_bound` | Phase 0 (pre-mutation, claim-based) | **REJECT** — `Err(VisionViolation)`; mutasyon yok |
| **Neighbor drift** | `θ(repositioned_node, vision) > θ_bound` | Phase 2 (post-mutation) | **WARNING** — commit geçerli, komşu degrade |

**Neden ayrım?** Formalizm §4.3 Q5 `θ(P_raw(C), V_vision)` — **claim'in** pozisyonu.
Claim negatif-uzayda ise ana dala giremez (BFT-derived Safety, negatif-uzay sızıntısı yok).
Neighbor drift ise commit'in YAN etkisi — commit'in kendisi geçerli (Q5'i geçti), ama yeni
kenarlar komşuyu itti. Bu degrade sinyali, mutlak reject değil.

**Sonuç:** `CommitOutcome.violations` → `drift_warnings` olarak yeniden adlandırıldı (reject değil).
Q5 violation `EngineCommitError::VisionViolation` olarak pre-mutation döner. Q4 (syntax) ve Q6 (rule)
violations da aynı pre-mutation REJECT pattern'ini izler (aynı Phase 0).

### 4.2 İnvariant Uyumu

| İnvariant | Pipeline'da nerede |
|---|---|
| **Q4** (claim Syntax — inv #12) | Phase 0: `OutputContract` compliance check → `Err(SyntaxViolation)` pre-mutation |
| **Q5** (claim Vision Safety) | Phase 0: `θ(claim.computed_raw, vision) > bound → Err(VisionViolation)` pre-mutation |
| **Q6** (claim Rule Safety) | Phase 0: `Rule(ΔS) = Violated → Err(RuleViolation)` pre-mutation |
| **#13** (PermissionMask, agent-prompt-semantics.md §2.1) | Phase 0: nihai denetim noktası — `commit()` PermissionMask'i zorunlu kontrol eder |
| #4 (RawPosition/DerivedPosition) | `raw_position_of` → `compute_derived` (θ raw'dan, derived girdi değil) |
| #5 (lazy diffusion) | Reposition `CosineDeviation` kullanır; `DiffusionDeviation` çağrılmaz |
| #6 (incremental) | Reposition sadece `delta.repositioned` (ΔV ∪ N₁(ΔV)), tüm |V| değil |
| #7 (admin flag) | `safety_weakened` CommitOutcome'a propagate |
| #10 (D ayrı) | `compute_derived` `main_sequence_distance` ayrı field |

### 4.3 Borrow Checker Uyumu (Rust)

`reposition_nodes` iki-fazlı (collect → apply) — `self.space`'den hem read (raw_position_of)
hem write (node.position) yapılamaz (borrow conflict):

```rust
/// Phase 2: post-mutation neighbor drift tespiti (WARNING, reject değil — §4.1).
fn reposition_nodes(&mut self, ids: &[NodeId]) -> Vec<DriftWarning> {
    let mut drift_warnings = Vec::new();
    // Faz 1: hesapla (immutable borrow)
    let updates: Vec<(NodeId, Position)> = ids.iter()
        .filter_map(|&id| {
            let node = self.space.nodes.get(&id)?;
            let raw = self.coord_system.raw_position_of(node, &self.space);
            let derived = compute_derived(
                &raw, &self.vision, &self.space, &CosineDeviation,
                raw.z, self.config.abstractness,
            );
            if derived.theta > self.config.theta_bound {
                drift_warnings.push(DriftWarning { node_id: id, theta: derived.theta });
            }
            Some((id, Position { raw, derived }))
        })
        .collect();
    // Faz 2: uygula (mutable borrow)
    for (id, pos) in updates {
        if let Some(node) = self.space.nodes.get_mut(&id) {
            node.position = pos;
        }
    }
    drift_warnings
}

/// Phase 0: pre-mutation claim-based Safety check'leri (Q4-Q6, REJECT).
/// Her biri bağımsız; sırayla: Q4 Syntax → Q5 Vision → Q6 Rule.
/// İlk failure anında Err döner, kalanları koşmaz (short-circuit).
impl SpaceEngine {
    /// Q4: DeltaProposal OutputContract'a uyuyor mu? (inv #12)
    fn check_claim_syntax(&self, claim: &Claim) -> Result<(), EngineCommitError> {
        // ΔS şeması doğrulama: new_nodes/new_edges/modified_entities/position_hints tipleri geçerli,
        // bağlı NodeId'ler mevcut, EdgeKind'ler legal. agent-prompt-semantics.md §2.2.
        if !claim.delta_is_well_formed() {
            Err(EngineCommitError::SyntaxViolation(SyntaxViolation {
                claim_id: claim.id,
                detail: claim.syntax_error_detail(),
            }))
        } else { Ok(()) }
    }

    /// Q5: θ(claim.computed_raw, vision) ≤ θ_bound?
    fn check_claim_vision(&self, claim: &Claim) -> Result<(), EngineCommitError> {
        let theta = CosineDeviation.theta(&claim.computed_raw, &self.vision, &self.space);
        if theta > self.config.theta_bound {
            Err(EngineCommitError::VisionViolation(VisionViolation {
                claim_id: claim.id, theta, raw: claim.computed_raw,
            }))
        } else { Ok(()) }
    }

    /// Q6: ΔS herhangi bir Rule'u ihlal ediyor mu?
    fn check_claim_rules(&self, claim: &Claim) -> Result<(), EngineCommitError> {
        for rule in &self.rules {
            if let Some(violation) = rule.evaluate(&claim.delta, &self.space) {
                return Err(EngineCommitError::RuleViolation(violation));
            }
        }
        Ok(())
    }

    /// PermissionMask nihai denetimi (inv #13, agent-prompt-semantics.md §2.1 nokta 3).
    /// claim.author'ın yazma yetkisi olmayan düğümlere dokunması engellenir.
    fn check_permissions(&self, claim: &Claim, mask: &PermissionMask) -> Result<(), EngineCommitError> {
        // read_only_nodes'a yazma, forbidden_edge_kinds oluşturma,
        // max_position_deviation aşımı kontrolü...
        Ok(())
    }
}
```

---

## 5. Vision Deklarasyon (TOML → VisionVector)

§5.1 KARAR: vision elle deklare edilir. Faz 2 bunu parse eder.

### 5.1 Format (`osp-vision.toml`)

```toml
# Projenin vizyonu — elle deklare (inv: LLM önerir, uygulAMAZ)
[raw]
x = 0.4   # coupling: moderate (DDD bounded contexts)
y = 0.7   # cohesion: high (single-responsibility)
z = 0.5   # instability: balanced (Martin main sequence)
w = 0.5   # entropy: moderate (yaygın değişim)
v = 0.5   # witness-depth: moderate (review kültürü)

[policies]
min_approvers = 2
quorum_threshold = 1.5
merge_ratio_observable = 0.10   # tri-state heuristic (Faz 1.11 doğrulandı)

[thresholds]
theta_bound = 0.25             # vision violation threshold (formalism §5.2: 0.5 unreachable in [0,1])
milestone_interval = 1000     # tam snapshot sıklığı (event-sourcing — delta'lar ayrı)
# Not: 0.5 = teorik orthogonal limit (cosine normalized), runtime default DEĞİL.
# Faz 2 keşfi (OSP-formalism §5.2): tüm eksen değerleri [0,1] normalize → cos_sim ∈ [0,1]
# → θ_norm ∈ [0, 0.5]. 0.5 eşiği asla tetiklenmez. Production: 0.2-0.3 aralığı.

[diagnostics]
abstractness = 0.5             # placeholder (Faz 3 SCIP gerçek A)

# Custom axis registration (inv #15 — God Mode only, Agent çağıramaz)
# Her axis imzalı paket olarak yüklenir (formalism §2.2 marketplace).
[custom_axes]
# Örnek (commented out — gerçek projede God Mode `osp axis register` ile ekler):
# [[custom_axes.registered]]
# id = "security.audit"
# package = "osp-axis-security-audit@1.2.0"
# signature = "sha256:..."        # trust chain
# calibration = { theta_weight = 1.0, normalization_cap = 1.0 }
#
# [[custom_axes.registered]]
# id = "wcag.compliance"
# package = "osp-axis-wcag@0.3.1"
# signature = "sha256:..."
# calibration = { theta_weight = 0.8, normalization_cap = 1.0 }

[custom_axes.vision_targets]
# Custom axis'ler için vision hedefleri (V_vision_custom).
# security.audit = 0.85  # security ≥ 0.85 hedef
# wcag.compliance = 0.90 # WCAG ≥ 0.90 hedef
```

### 5.2 Parse

```rust
pub fn load_vision_config(path: &Path) -> Result<VisionConfig> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text).map_err(Into::into)
}

pub struct VisionConfig {
    pub raw: VisionRawConfig,
    pub policies: VisionPoliciesConfig,
    pub thresholds: VisionThresholdsConfig,
    pub diagnostics: VisionDiagnosticsConfig,
}
// → VisionVector(RawPosition { x, y, z, w, v }) + EngineConfig
```

---

## 6. Tam Tip Tanımları

### 6.1 `engine.rs`

```rust
/// Space Engine — production runtime orchestrator.
pub struct SpaceEngine {
    space: Space,
    coord_system: CoordinateSystem,
    vision: VisionVector,
    rules: Vec<Rule>,            // Q6 rule gate için (Hard Rules)
    time: TimeFSM,
    config: EngineConfig,
    t_c: u64,                    // commit count (time index)
    snapshot_store: SnapshotStore,
    // Faz 5: permission_mask burada DEĞİL — per-Agent/per-commit parametresi olarak
    // commit() imzasına eklenir (agent-prompt-semantics.md §2.1).
}

pub struct EngineConfig {
    pub min_approvers: usize,
    pub quorum_threshold: f64,
    pub theta_bound: f64,        // vision violation threshold (default 0.25; 0.5 unreachable — §5.1)
    pub milestone_interval: u64,
    pub abstractness: f64,       // placeholder (Faz 3 SCIP)
    pub merge_ratio_observable: f64,  // tri-state (Faz 1.11: 0.10)
    // Custom axis registration God Mode tarafından yapılır (inv #15).
    // CoordinateSystem.custom zaten registered axis'leri taşır; EngineConfig sadece
    // theta_bound gibi cross-axis parametreleri içerir. Custom axis vision target'ları
    // V_vision.custom içinde (§5.1 TOML [custom_axes.vision_targets]).
}

pub struct CommitOutcome {
    pub event: Delta,
    pub drift_warnings: Vec<DriftWarning>,   // post-mutation neighbor drift (§4.1 — WARNING)
    pub safety_weakened: bool,
    pub t_c: u64,
}

/// Post-mutation: neighbor θ > bound (commit geçerli, komşu degrade — §4.1 WARNING).
pub struct DriftWarning {
    pub node_id: NodeId,
    pub theta: f64,
    pub raw: RawPosition,
}

/// Pre-mutation: claim θ > bound (Q5 ihlali — §4.1 REJECT, EngineCommitError::VisionViolation).
pub struct VisionViolation {
    pub claim_id: ClaimId,
    pub theta: f64,
    pub raw: RawPosition,        // claim.computed_raw
}

/// Pre-mutation: ΔS OutputContract'a uymuyor (Q4 ihlali — inv #12 REJECT).
pub struct SyntaxViolation {
    pub claim_id: ClaimId,
    pub detail: String,          // şema hatası açıklaması (kalibrasyon geri bildirimi)
}

/// Pre-mutation: ΔS bir Rule'u ihlal ediyor (Q6 ihlali — REJECT).
pub struct RuleViolation {
    pub claim_id: ClaimId,
    pub rule_id: RuleId,
    pub detail: String,          // ihlal edilen kural + nasıl ihlal edildiği
}

/// Engine-level commit error (thiserror). Claim-based (Q4-Q6) + witness-based (Q1-Q3).
/// bigbang modülü mutation-only (apply_delta infallible) — kendi error'ı yok (osp-core-design.md §3.4).
/// Witness Reject/Hold `evaluate()` → `WitnessResult` üzerinden gelir, `Reason` wrap edilir.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EngineCommitError {
    #[error("witness gate: {0:?}")]
    Witness(crate::witness::Reason),   // Hold/Reject from Q1-Q3 (evaluate sonucu)
    #[error("Q4 syntax violation (claim malformed): {detail}")]
    SyntaxViolation { violation: SyntaxViolation },
    #[error("Q5 vision violation (claim negative-space): θ={theta:.3} > {bound:.3}")]
    VisionViolation { violation: VisionViolation, bound: f64 },
    #[error("Q6 rule violation (rule {rule_id:?}): {detail}")]
    RuleViolation { violation: RuleViolation },
    #[error("permission denied (inv #13): {0}")]
    PermissionDenied(String),
}

impl SpaceEngine {
    pub fn new(space: Space, coord_system: CoordinateSystem, vision: VisionVector, config: EngineConfig) -> Self;
    pub fn from_config(path: &Path) -> Result<Self>;  // TOML yükle → build

    /// Commit pipeline (§4). Q4-Q6 pre-check (claim-based) → Q1-Q3 (witness) → reposition → save delta.
    pub fn commit(&mut self, claim: &Claim, omega: &WitnessSet) -> Result<CommitOutcome, EngineCommitError>;

    /// θ_diffusion ile TAM reposition (analyze/dashboard — inv #5 lazy).
    pub fn full_reposition(&mut self) -> Vec<DriftWarning>;

    /// Time-travel (event-sourcing, §3.5): milestone + delta replay → request_t_c.
    pub fn restore(&mut self, request_t_c: u64) -> Result<usize>;  // returns replayed delta count

    /// Milestone snapshot (tag/manual/periyodik).
    pub fn save_milestone(&self, name: &str) -> Result<()>;

    /// Mevcut uzay (read-only).
    pub fn space(&self) -> &Space;
    pub fn t_c(&self) -> u64;
}
```

### 6.2 `persistence.rs`

> Tipler §3.3/§3.4'te tanımlı (`SpaceSnapshot`, `DeltaRecord`, `SnapshotStore`, `RestoredState`).
> `SNAPSHOT_FORMAT_VERSION = 1` (reviewer #4). Event-sourcing: `save_milestone` (nadir) +
> `save_delta` (her commit) + `restore` (milestone + replay).

### 6.3 `vision_config.rs`

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VisionConfig {
    pub raw: VisionRawConfig,
    pub policies: VisionPoliciesConfig,
    pub thresholds: VisionThresholdsConfig,
    pub diagnostics: VisionDiagnosticsConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VisionRawConfig { pub x: f64, pub y: f64, pub z: f64, pub w: f64, pub v: f64 }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VisionPoliciesConfig {
    pub min_approvers: usize,
    pub quorum_threshold: f64,
    pub merge_ratio_observable: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VisionThresholdsConfig {
    pub theta_bound: f64,
    pub milestone_interval: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VisionDiagnosticsConfig {
    pub abstractness: f64,
}

impl VisionConfig {
    pub fn load(path: &Path) -> Result<Self>;
    pub fn to_vision_vector(&self) -> VisionVector;
    pub fn to_engine_config(&self) -> EngineConfig;
}
```

---

## 7. Test Stratejisi

| Test türü | Ne |
|---|---|
| **Birim** | `engine::commit_*`, `persistence::save_load_roundtrip`, `vision_config::parse_*` |
| **Invariant** | inv #5 (`commit_uses_cosine_not_diffusion`), #6 (`reposition_only_delta`), #7 (flag propagate) |
| **Snapshot** | save → load → Space eşitmi; time-travel (t_c=5 → restore → commit count correct) |
| **Violation** | `θ > theta_bound` düğümü → `VisionViolation` emit |
| **TOML parse** | örnek `osp-vision.toml` → `VisionVector` doğru |

---

## 8. Faz 2 Uygulama Planı

| Adım | İçerik | Çıktı | Sıra |
|---|---|---|---|
| 2.1 | `serde` + `bincode` + `toml` dep'leri ekle (osp-core Cargo.toml) | bağımlılıklar | 1 |
| 2.2 | `Space` + `Node` + `Edge` + türevleri için `Serialize/Deserialize` derive | serde-ready tipler | 1 |
| 2.3 | `vision_config.rs` — TOML parse + `VisionConfig` → `VisionVector`/`EngineConfig` | `osp-core::vision_config` | 2 |
| 2.4 | `bigbang::apply_delta()` refactor — commit'in mutasyon fazı reusable (live + replay) | `osp-core::bigbang` | 3 |
| 2.5 | `persistence.rs` — `SpaceSnapshot` + `DeltaRecord` + `SnapshotStore` (event-sourcing: milestone + delta + restore/replay, §3) | `osp-core::persistence` | 3 |
| 2.6 | `engine.rs` — `SpaceEngine` + `commit()` pipeline (§4: **Q4-Q6 claim-based pre-check** → Q1-Q3 witness → reposition → save delta) + `check_claim_syntax`/`check_claim_vision`/`check_claim_rules` + `reposition_nodes` | `osp-core::engine` | 4 |
| 2.7 | `full_reposition()` (lazy diffusion — `DiffusionDeviation` stub) | analyze-only path | 5 |
| 2.8 | `restore()` event-sourcing (milestone + delta replay) + time-travel test'leri | time-travel | 5 |
| 2.9 | Q4-Q6 reject + drift warning test'leri + örnek `osp-vision.toml` + integration test | end-to-end | 6 |

**Kabul kriterleri:**
- `SpaceEngine::commit()` Q4-Q6 (claim-based) → Q1-Q3 (witness) → mutate → reposition → save delta akışı tam çalışır
- **Q4 violation** (syntax) → `Err(SyntaxViolation)`, **mutasyon yok** (inv #12)
- **Q5 violation** (claim θ > bound) → `Err(VisionViolation)`, **mutasyon yok** (reviewer #1 Safety)
- **Q6 violation** (rule ihlali) → `Err(RuleViolation)`, **mutasyon yok**
- Q4-Q6 witness'lardan **önce** koşar — syntax/vision/rule hatası olan claim `WitnessSet`'e gelmez
- **Drift warning** (neighbor θ > bound post-mutation) → `CommitOutcome.drift_warnings`, commit başarılı
- Event-sourcing: `save_delta` her commit, `save_milestone` nadir; `restore(t_c)` milestone + replay
- Snapshot sürüm uyumsuzluğu → graceful `Err(VersionMismatch)` (reviewer #4)
- inv #5 (cosine on commit path), #6 (incremental), #7 (flag) test'leri geçer

---

## 9. Açık Sorular

1. ~~Snapshot retention policy~~ → **ÇÖZÜLDÜ**: event-sourcing (§3.2) — milestone + delta.
   Prune: eski delta'lar milestone'lara kadar silinebilir (replay için yeterli).
2. **`abstractness` placeholder:** Faz 3 SCIP gelene kadar `EngineConfig.abstractness = 0.5`.
   D değerleri yarı-anlamlı. Proxy aday: `abstract_nodes / total_nodes` (AST'den).
3. **`full_reposition` maliyet:** Diffusion `O(n³)` — Faz 2'de cache mi, incremental mı?
   **Eğilim:** lazy + cache (analyze çağrısında bir kez, commit'lerde değil).
4. **Concurrent access:** SpaceEngine `&mut self` — single-threaded. Faz 5'te LLM agent'ları
   paralel commit isterse? **Eğilim:** actor model (channel-based serialize) — Faz 5 concern.
5. ~~**Q4 prospective vs claimed**~~ → **Q5 olarak yeniden adlandırıldı** (Q4-Q6 split).
   Mevcut: Q5 `claim.computed_raw` kontrol eder — bu, engine'in DeltaProposal'ın **yapısal**
   değişikliklerinden compute ettiği raw position (agent pozisyon declare ETMEZ, agent-prompt-semantics.md §2.2).
   **Kalan risk:** agent ΔS yapısında yalan söyleyebilir (gerçekte eklemeyeceği node'ları iddia).
   İleride: hypothetical-graph ile ΔS apply edildikten SONRAki ACTUAL raw position pre-mutation
   hesaplanabilir (tam deterministic validity). Faz 2.6+ değerlendirme.
6. **PermissionMask parametresi (Faz 5):** `commit()` imzasına `mask: &PermissionMask`
   eklenir (inv #13, agent-prompt-semantics.md §2.1). Faz 2'de no-op/full-access default.
7. **Rule engine (Faz 5):** `Rule` trait + `Rule::evaluate(ΔS, space) -> Option<RuleViolation>`.
   Hard Rules (statik, Q6 öncesi compute) vs Soft Rules (dinamik). agent-prompt-semantics.md §5.2.

---

## 10. Faz 2 → Faz 3 Köprüsü

Faz 2 Space Engine çalışır durumda. Faz 3 (SCIP Analyzer) şunları ekler:
- Gerçek `y` (LCOM4) + `A` (abstractness) → D anlamlı
- Rust/Go language support (tree-sitter-rust, tree-sitter-go)
- Scale test (100k+ node) — in-memory yeterli mi, KùzuDB gerekecek mi karar

**Space Engine bu geçişe hazır:** `coord_system` pluggable (Axis trait), `abstractness`
`EngineConfig`'ten okunuyor (SCIP gelince gerçek değer girer).

---

## 11. Karar Özeti

| Karar | Seçim | Gerekçe |
|---|---|---|
| Persistence | **serde + bincode** (custom binary) | Minimal dep, in-memory yeterli |
| Persistence stratejisi | **Event-sourcing** (milestone + delta replay) | %98 disk tasarrufu, time-travel (reviewer #2,3) |
| Graph DB (KùzuDB) | Faz 3'e erteli | Faz 2 ölçeği (binlerce node) in-memory rahat |
| **Claim-based gates (Q4-Q6)** | **Pre-mutation, witness öncesi REJECT** (`Err(Syntax/Vision/RuleViolation)`) | formalism §4.3: claim'in kendi özellikleri deterministik; witness'a gitmeden filtrelenir |
| **Witness-based gates (Q1-Q3)** | `WitnessSet` üzerinden Hold/Reject/Commit | Quorum/evidence-based |
| Neighbor drift | Post-mutation **WARNING** (`drift_warnings`) | Commit geçerli, komşu degrade — reject değil |
| PermissionMask denetimi | Üç nokta: slice → Agent kabuğu → `commit()` nihai (inv #13) | Defense in depth; Faz 5'te commit() imzasına eklenir |
| Snapshot version | `version: u32 = 1` (reviewer #4) | Sürüm uyumsuzluğu → graceful error |
| Vision format | **TOML** (`osp-vision.toml`) | Declarative, yorum-dostu, Rust ekosistem olgun |
| Module layout | `engine.rs` + `persistence.rs` + `vision_config.rs` (osp-core içinde) | roadmap: osp-core Faz 1-2 |
| Reposition metric (commit path) | `CosineDeviation` (inv #5) | Hızlı, diffusion lazy |
| `apply_delta` refactor | bigbang mutasyon fazı reusable (live + replay) | Event-sourcing replay için |

---

*Sonraki: Faz 2.1 implementasyonu (serde + bincode + toml dep'leri + Serialize/Deserialize derive).*
