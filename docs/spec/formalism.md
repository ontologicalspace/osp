# OSP — Matematiksel Formalizm (Faz 1)

> Bu doküman OSP'nin (Ontological Space Protocol) matematiksel iskeletidir.
> İki amaca hizmet eder: (1) makalenin *Formalism* bölümünün taslağı,
> (2) `osp-core` Rust crate'inin uygulama reçetesi.
>
> **Girdiler:** `SoftwarePhysics.txt` (vizyon), `docs/literature-scan.md` (öncüller),
> `docs/spike-results.md` (ampirik Faz 0 verisi).
>
> **Sürüm:** 1.0-draft · Yazım: 2026-06-17 · Kilitlenecek: `osp-core` implementasyonu öncesi

---

## 0. Notasyon Sözlüğü

| Sembol | Anlam |
|---|---|
| `S` | Kavramsal uzay (space) |
| `V` | Düğüm kümesi (nodes), `|V| = n` |
| `E` | Kenar kümesi (edges), `E ⊆ V × V × K_E` |
| `G` | Kütleçekim fonksiyonu (rule-ağırlıkları) |
| `P` | Konum vektörü: `P_raw ∈ ℝ^(5+N)` (5 core + N custom) + `P_derived = (u, θ, risk_score, D)` — §2 |
| `V_vision` | Projenin vizyon vektörü |
| `θ` | Sapma açısı (P ile V_vision arası) |
| `W` | Şahitlik operatörü |
| `I` | Niyet (intent — agent'a verilen görev; `t_f` katmanında yaşar, bkz. `agent-prompt-semantics.md §0`) |
| `C` | İddia (claim — agent'ın ürettiği iş; `t_m`'de Belief, `t_c`'de Knowledge) |
| `t_m, t_c, t_f` | Miş'li / şimdiki / gelecek zaman katmanları (ontolojik harita: `agent-prompt-semantics.md §0`) |

---

## 1. Ontolojik Primitifler

### 1.1 Düğüm (Node)

Bir kavramsal uzay düğümü:

```
n ∈ V : (id, kind, mass, position)
```

- **`id`**: kararlı tanımlayıcı (içerik-adresli hash önerisi —内容-addressed)
- **`kind`** ∈ `{Module, Concept, Feature, Bug, Rule, Agent, Intent, Claim, Witness}` (meta-ontology)
- **`mass ∈ ℝ⁺`**: düğümün kütlesi. Faz 0 spike'ında `mass = LOC`. Faz 1'de:
  `mass = α·LOC + β·AST_node_count + γ·import_in_degree` (ağırlıklar kalibre edilecek)
- **`position`**: koordinat — 5 core raw + N custom raw + derived (§2). Boyut proje-bazlı
  değişebilir (custom axis sayısı N, God Mode tarafından register edilir — §2.2, inv #15).

### 1.2 Düğüm Türleri (Meta-Ontology)

| Tür | Açıklama | OSP'deki rolü |
|---|---|---|
| `Module` | Kaynak dosya / paket | Faz 0 spike seviyesi (dosya-bazlı) |
| `Concept` | Domain kavramı (DDD aggregate root gibi) | Faz 1'de AST + isim-analizi ile |
| `Feature` | Kullanıcı-görünür yetenek | Big Bang tetikleyicisi |
| `Bug` | Hata/negatif-uzay işareti | θ > θ_eşik'nin somut karşılığı |
| `Rule` | Mimari/domain kuralı (kültrelçekim kaynağı) | `G`'yi besler |
| `Agent` | AI-agent (LLLM sürücüsü) | `I` üretir, `C` üretir |
| `Intent` | Agent'a verilen görev | **`t_f` katmanında yaşar** (potansiyel gradyan — agent-prompt-semantics.md §0 ontolojik harita) |
| `Claim` | Agent'ın ürettiği iş (PR) | `t_m`'den `t_c`'ye aday (`t_m`'de Belief, `t_c`'de Knowledge) |
| `Witness` | Onay/red veren kimlik (agent veya insan) | `W` operatörüne girdi |

**KARAR:** Üst-ontoloji (`Feature`/`Bug`/`Rule`/`Branch`/`Issue`/`PR`/`Review`) OSP'nin
yazılım-süreç meta-modelidir — bu, `docs/literature-scan.md §5`'teki ontoloji-bileşenleri
formalizmine (Individuals/Classes/Relations/Restrictions) uyar.

### 1.3 Kenar (Edge)

```
e = (from, to, kind) ∈ E
kind ∈ {Imports, Calls, DependsOn, PartOf, DerivesFrom, Witnesses, Approves, Violates}
```

**KARAR:** Kenarlar **tipleşmiş**. Bu, klasik yazılım grafından (yalnız `DependsOn`)
farklı olarak OSP'de epistemolojik ilişkileri (`Witnesses`, `Approves`) de modellememizi
sağlar — `literature-scan.md §6` BFT-köprüsünün gerektirdiği yapısal zenginlik.

### 1.4 Uzay (Space)

```
S = (V, E, G, t_state)
```

- `V, E`: yukarıdaki
- `G: V → ℝᵏ`: kütleçekim — her düğüme `Rule`'lardan gelen kısıt ağırlıkları atar.
  Örn: bir `Rule = "tüm Feature'lar Test düğümü olmadan var olamaz"` ise, bu kuralın
  ihlali `G(testless_feature)` değerini düşürür → negatif-uzay sinyali.
- `t_state ∈ {t_m, t_c, t_f}`: uzayın zaman katmanı (§3)

**Faz 0 spike'ındaki karşılığı:** `DepGraph { nodes, edges }` → `S`'nin `(V,E)` kısmı.
`G` ve zaman katmanı Faz 1'de geliyor.

---

## 2. Koordinat Sistemi (5 Core + N Custom + Derived)

OSP'nin koordinat sistemi üç katmandan oluşur: **5 core raw axis** (standart yazılım fiziği),
**N custom raw axis** (domain-bazlı, God Mode tarafından register edilen eklentiler — §2.2),
ve **derived koordinatlar** (raw'dan hesaplanan).

**KARAR:** 5 core eksen Faz 0 verisiyle kesinlendi (`spike-results.md §6`). Custom eksenler
`Axis` trait'i ile genişletilebilir (§2.2 "Custom Axis Extensibility"). Tüm raw eksen
değerleri [0,1] normalize.

### 2.1 Core Raw Eksenler (sabit 5)

| Sembol | Eksen | Tanım | Faz 0 göstergesi | Faz 0 değeri (click vs worms) |
|---|---|---|---|---|
| `x` | Kuplaj | `|E_imports| / |V|` | edges/nodes | click 0.97 / worms 0.65 |
| `y` | Kohezyon | LCOM4 proxy (Faz 1: tree-sitter pseudo-type; Faz 3: SCIP gerçek tip) | (Faz 1.3'te) | — |
| `z` | **İstikrarsızlık (Instability)** | **Martin Instability `I = Ce/(Ca+Ce)`** (Ce=fan-out, Ca=fan-in) | (Faz 1.3'te) | — |
| `w` | İstikrar/Entropi | Shannon `H = -Σ p_i log2 p_i` (commit→dosya) | commit_entropy | click 6.78 / worms 6.60 |
| `v` | Şahitlik derinliği | `w_ratio · ln(1+distinct_witnesses)` | witness_depth | click 2.16 / worms 0.00 |

> **z-ekseni naming düzeltmesi:** Önceki sürümlerde `z` label'ı yanlış "Soyutlama"
> (Abstractness) olarak görünüyordu — ama formül Martin **Instability** `I`. Soyutlama ayrı
> bir metrik: `A = Na/Nc` (diagnostic input, raw axis değil — §2.3).

**Konum vektörü (core + custom + derived):**
```
P_core     = (x, y, z, w, v) ∈ ℝ⁵           — 5 sabit core raw eksen
P_custom   = (c₁, c₂, ..., cₙ) ∈ ℝᴺ         — N custom raw eksen (§2.2, proje-bazlı)
P_raw      = (P_core, P_custom) ∈ ℝ^(5+N)   — tüm raw eksenler
P_derived  = (u, θ, risk_score, D)          — raw'dan türetilmiş (dairesellik önlemi, §3.1)
P_full     = (P_raw, P_derived)
```

**Faz 0 spike-tablosu → koordinat örnekleri (y=0, z=0, N=0 varsayımıyla):**

| repo | x | w | v | u |
|---|---|---|---|---|
| click | 0.97 | 6.78 | 2.16 | 0.98 |
| worms-supabase | 0.65 | 6.60 | 0.00 | 0.41 |

**Gözlem:** click'in `v=2.16` ve `u=0.98` değerleri onu `worms`'tan core raw uzayda net
biçimde ayırır. Bu, OSP'nin geometrik ayrım tezinin ampirik kanıtıdır. Custom axis eklendiğinde
ayrım güçlenir (domain-bazlı sinyaller eklenir).

### 2.2 Custom Axis Extensibility (Dimension Marketplace)

OSP bir **software physics engine**'dir — farklı domain'ler farklı fizik kuralları gerektirir.
5 core axis standart yazılım fiziğini (coupling/cohesion/instability/entropy/witnessing) kapsar;
custom axis'ler domain-bazlı fizik kurallarını (security, accessibility, performance, compliance)
ekler.

**Mekanizma:** `Axis` trait'i her custom axis için bir reallemedir. `CoordinateSystem` core +
custom axis koleksiyonunu taşır:

```rust
pub trait Axis: Send + Sync {
    fn id(&self) -> AxisId;                    // "security.audit", "wcag.compliance", vb.
    fn name(&self) -> &'static str;
    fn compute(&self, node: &Node, space: &Space) -> MetricValue;  // [0,1] + provenance
    fn calibration(&self) -> AxisCalibration;  // θ_bound weight, normalization sabitleri
}

pub struct CoordinateSystem {
    core: [Box<dyn Axis>; 5],                  // x, y, z, w, v — sabit
    custom: Vec<Box<dyn Axis>>,                // God Mode tarafından register edilen
}
```

**Core vs Custom value asymmetry (kasıtlı):** Core 5 axis `CoreRawPosition`'da plain
`f64` olarak saklanır (deterministik, implicit full-confidence — coupling graph yapısından,
entropy git log'tan). Custom axis'ler `CustomRawPosition.values: HashMap<AxisId, MetricValue>`
olarak saklanır (provenance gerekli — security scanner partial coverage, heuristic confidence).
Bu ayrım hesaplama hızını (core hot path, f64) ve epistemolojik dürüstlüğü (custom, MetricValue)
dengeye kor. `Axis::compute()` her iki durumda `MetricValue` döner; core axis'ler çağrı sonrası
`.value` extract edilip `CoreRawPosition`'a `f64` olarak yazılır.
```

**Custom axis örnekleri (dimension marketplace):**

| Domain | Custom Axis | Compute Kaynağı |
|---|---|---|
| Security | `security.audit` — vulnerability density | SCIP + CVE database |
| Accessibility | `wcag.compliance` — WCAG kural adherence | tree-sitter + ARIA analyzer |
| Performance | `perf.budget` — latency budget adherence | profiler output |
| Compliance | `compliance.hipaa` — HIPAA rule adherence | rule engine |
| Team dynamics | `team.bus_factor` — knowledge concentration | git log + blame |
| Testing | `test.mutation_score` — mutation testing | mutation runner |

**Güvenlik modeli (inv #15):** Custom axis'leri **sadece God Mode register eder**. Agent/LLM
yeni axis tanımlayamaz veya mevcut axis'in compute fonksiyonunu değiştiremez. Aksi halde LLM
"fake_security" axis inject eder (her zaman 1.0 döner) ve Q5 vision gate'i deler. Bu, inv #13
(PermissionMask God Mode) ile aynı güven modeli.

**Marketplace vizyonu (Faz 6+):** Custom axis'ler imzalı paketler olarak (crate/WASM) dağıtılır.
Registry/index (npm/crates.io analojisi) ile discovery; her axis metadata taşır (name, version,
author, calibration params, source type). Community physics rules biriktikçe değer artar
(network effect) — OSP'nin defensible moat'ı.

### 2.3 Abstractness (diagnostic, raw axis değil)

`A = Na/Nc` (Abstractness) raw eksen **değildir** — `D = |A + I − 1|` derived metric'inin
diagnostic input'udur:

- `z = I = Ce/(Ca+Ce)` (saf Instability, raw axis) ∈ [0,1] — Martin'in "pain of change" metriği
- `A = Na/Nc` (Abstractness) ∈ [0,1] — **diagnostic input**, `EngineConfig.abstractness`'ten
- `D = |A + I − 1|` (Distance from Main Sequence) ∈ [0,1] — **ayrı derived metric**
  (`P_derived.main_sequence_distance`, inv #10; "architectural balance"; Zone of Pain/Uselessness dedektörü)

**D eski `z = I × (1−D)` formülünden AYRILDI** (reviewer notu: bilgi kaybı + naming muğlaklığı).
Saf `z = I` bilgiyi geri-kazanılabilir tutar; `D` bağımsız sinyal olarak `θ` hesabına
bileşen olur (`θ_eff = θ_diffusion × (1 + α·D)` — α kalibrasyon) VE raporlarda "main-sequence
sapması" olarak ayrı gösterilir.

**Gerekçe:** `I×(1−D)` çarpımından `I` ve `D`'yi ayrı kurtaramazsın. Bir projenin "high-I,
high-D" (zone of uselessness: abstract + unstable) mü yoksa "low-I, high-D" (zone of pain:
concrete + rigid) mi olduğunu ayırt etmek için ikisi de lazım. Saf ayrım bu diagnoses'i mümkün kılar.

**KARAR (y-ekseni — type inference stratejisi):** Faz 1'de **tree-sitter pseudo-type
heuristik** (class/inheritance/decorator çıkarımı, sıfır yeni bağımlılık). Faz 3'te **SCIP**
(rust-analyzer/pyright/tsc/gopls çıktısı) ile gerçek semantik tip bilgisi gelir — LCOM4 ve
Abstractness gerçek değerlerle hesaplanır.

### 2.4 Raw vs Derived Koordinatlar (Dairesellik Önleme)

`u = 1 − θ` ama `θ = deviation(P, V_vision)`. Eğer `u ∈ P` ise, `θ` `u`'yu kullanır →
**dairesel**. Çözüm: `u` derived (türetilmiş) koordinattır, raw değil.

```
P_raw      = (P_core, P_custom) ∈ ℝ^(5+N)    — tüm raw eksenler (§2.1, §2.2)
θ          = deviation(P_raw, V_vision_raw)   — raw'dan hesaplanır (§5)
P_derived  = (u = 1 − θ_norm, θ, risk_score, D)  — raw'dan türetilmiş
P_full     = (P_raw, P_derived)
```

**Kod ayrımı (tip-güvenliği — Faz 1.4 osp-core):**
```rust
struct CoreRawPosition { x: f64, y: f64, z: f64, w: f64, v: f64 }  // sabit 5
struct CustomRawPosition { values: HashMap<AxisId, MetricValue> }  // N custom (değişken, provenance ile)
struct RawPosition { core: CoreRawPosition, custom: CustomRawPosition }
struct DerivedPosition { u: f64, theta: f64, risk_score: f64, main_sequence_distance: f64 }
struct Position { raw: RawPosition, derived: DerivedPosition }
```

`θ` hesabı SADECE `P_raw`'ı okur; `P_derived` asla `θ` girdisi olamaz. Bu, daireselliği
**yapısal garanti** eder — biri yanlışlıkla `compute_theta(full_P)` yazsa derleyici engeller.
Ayrı bir `D` (Martin main-sequence distance) de derived metric olarak `P_derived`'a eklenir.

---

## 3. Zaman Modeli

### 3.1 Üç Zaman Katmanı

OSP'de zaman, kronolojik bir sayaç değil **epistemolojik durum**'dur (`literature-scan.md §5`,
`SoftwarePhysics.txt`'in "miş'li zaman" teorisinden).

| Katman | Sembol | İçerik | OSP karşılığı |
|---|---|---|---|
| **Miş'li (öznel)** | `t_m` | Agent'ın izole lokal uzayı — iddialar, taslaklar | feature branch + açık PR |
| **Şimdiki (nesnel)** | `t_c` | Onaylanmış, kütleçekimli gerçek uzay | main branch / production |
| **Gelecek (potansiyel)** | `t_f` | Henüz-doğmamış niyetler | issue'lar, roadmap |

**Ontolojik kategori haritası (her katmana bir birincil kategori — `agent-prompt-semantics.md §0` ile senkron):**

| Katman | Birincil Kategori | Epistemik Statü |
|---|---|---|
| `t_f` | **Intent** | Potansiyel — gradyan, hedef |
| `t_m` | **Belief** (Claim Candidate) | Aday — henüz şahitlenmemiş |
| `t_c` | **Knowledge** (Commit) | Gerçekleşmiş — şahitlenmiş |

**Bilgi akışı:** `Intent (t_f) → projeksiyon → Agent → Belief (t_m) → [W(C,Ω)] → Knowledge (t_c)`

Intent uzayı doğrudan mutate etmez; `t_f`'den `t_m`'ye gradyan `OspPrompt` projeksiyonu
üzerinden yayılır (`agent-prompt-semantics.md §2-3`). Bu, "miş'li zaman" felsefesiyle uyumlu:
gelecek şimdikiyi çeker ama deterministik neden olmaz.

### 3.2 Durum Geçişi (State Transition)

```
t_c ← t_c + 1   iff   W(C, Ω) = Commit   (§4.3)
```

Yalnızca **çift-şahitli onay** zamanı ilerletir. Aksi halde `t_c` sabit, iddia `t_m`'de bekler.

**Faz 0 spike'ındaki karşılığı:** `witnessed_ratio` = `t_m → t_c` geçişlerinin tüm
commit'lere oranı. Squash-workflow kör noktası (`spike-results.md §2`) bu geçişin
`merge-commit` varlığına indirgendiğinde ortaya çıktı — Faz 1 redesign bunu çözüyor (§4).

---

## 4. Şahitlik Operatörü `W` (Workflow-Agnostik)

> Faz 0'nın en önemli öğrenimi: `merge-commit`-tabanlı şahitlik modern workflow'larda kör.
> Bu bölüm `W`'yi **workflow-agnostik** olarak yeniden tanımlar — `spike-results.md §4`'ün
> uygulaması.

### 4.1 Şahit Türleri ve Ağırlıkları

Bir `Claim C`'yi onaylayan her şahit `W_i`'nin bir türü ve ağırlığı vardır:

| Tür | Ağırlık | Kaynak | Güven |
|---|---|---|---|
| `MergeCommit` | **1.0** | `git rev-list --merges <default>` | kriptografik (imzalı merge) |
| `PR-merged` | **0.8** | `gh pr list --state merged --base <default>` | GitHub API + review-required |
| `Trailer-Reviewed` | **0.7** | commit-message `Reviewed-by:` trailer'ı | imzalı ama daha zayıf |
| `Co-authored` | **0.4** | `Co-authored-by:` trailer'ı | katkı, review değil |

**KARAR:** Bu ağırlıklar başlangıç değerleri. Kalibrasyon **Faz 1.5'te** 15-20 repo
korpusu ile yapılacak: **Python + Rust + TypeScript + Go** (tree-sitter destekli hepsi),
maturity çeşitliliği (olgun / orta / küçük-temiz / köpük) ve **workflow çeşitliliği**
(merge-commit / squash / rebase karışık — kör-nokta doğrulaması için kritik). Faz 0'nın
5 reposu bu korpusun çekirdeğini oluşturur.

### 4.2 Şahitlik Skoru

Bir `Claim C` için toplam şahitlik desteği:

```
support(C) = Σ_{W_i ∈ Approvers(C)} weight(W_i)
```

**Quorum** (yeter-imza) sağlanır: `support(C) ≥ θ_quorum`

**KARAR:** `θ_quorum = 1.5` (başlangıç). Bu, en az iki güçlü şahit (örn. 1 MergeCommit + 1
PR-merged = 1.8 ≥ 1.5) VEYA üç zayıf şahit (3 × Trailer = 2.1 ≥ 1.5) gerektirir. Tek başına
bir merge-commit yetmez (1.0 < 1.5) → self-merge'i önler.

### 4.3 Operatör Tanımı

```
W : (Claim, WitnessSet) → WitnessResult
W(C, Ω) → {Commit(δS), Reject, Hold}
```

`Ω` = bağımsız non-author witness kümesi. Üç sonuç:
- **`Commit(δS)`** — quorum + vision-bound sağlandı → uzay genişler (§6), `t_c ← t_c+1`.
- **`Reject`** — honest-reject veya vision ihlali → iddia reddedilir, `t_m`'de kalır.
- **`Hold`** — quorum yetersiz → beklemeye alınır (§7.3 Lemma 2b; pratik mekanizmalarla çözülür).

```
W(C, Ω) = Commit(δS)   iff
    (Q4) OutputContract compliant — ΔS şeması geçerli            (claim-based, deterministic; agent-prompt-semantics.md §2.2, inv #12)
    (Q5) θ(P_raw(C), V_vision_raw) ≤ θ_bound                     (vision bound, §5; raw pozisyon; varsayılan θ_bound=0.25)
    (Q6) ∀ Rule R: R(ΔS) ≠ Violated                              (claim-based rule gate; agent-prompt-semantics.md §4)
    (Q1) |distinct_non_author_approvers(Ω)| ≥ min_approvers      (default 2)
    (Q2) support(Ω) = Σ weight(W_i) ≥ θ_quorum                   (§4.2)
    (Q3) ∀ honest W ∈ Ω: W.verdict ≠ Reject                      (no blocking honest-reject)

W(C, Ω) = Reject   iff  (Q3), (Q4), (Q5) veya (Q6) ihlal
W(C, Ω) = Hold     iff  (Q1) veya (Q2) yetersiz, (Q3)(Q4)(Q5)(Q6) sağlanır
```

**Gate sırası (önemli):** Q4-Q6 **claim-based ve deterministik**; Q1-Q3 **witness-based**.
Pipeline Q4-Q6'yı **witness'lardan önce** koşar — sentaks hatası (`Q4 fail`) veya vision
ihlali (`Q5 fail`) olan bir Claim şahitlere gösterilmez (kaynak israfı önlenir, inv #12).
Yalnızca Q4-Q6 geçen Claim'ler `WitnessSet Ω` üzerinden Q1-Q3 değerlendirmesine girer
(`space-engine-design.md §4`, `agent-prompt-semantics.md §4`).

> **Operatör isimlendirme notu:** `W(C, Ω)` geleneksel olarak "witness operator" olarak
> adlandırılır, ama Q4-Q6 `Ω`'yı kullanmaz (claim-in içsel özellikleridir). Kapsam açısından
> `W` bir **commit operator**'dür: claim-validation (Q4-Q6, Ω'suz) + witness-validation
> (Q1-Q3, Ω ile). İmplementasyonda bu iki faz `SpaceEngine::commit()` içinde ayrıdır
> (`check_claim_*` metotları → `evaluate()`). İsimlendirme `W` korunur çünkü literatürde
> BFT reduction (`§7`) bu sembolle kuruldu; `Ω` parametresi "witness mevcudiyeti"ni
> ifade eder (Q4-Q6 onu okumasa da Commit sonucu yine de Ω gerektirir — Q1-Q3 için).

> **Eski `Q4` → `Q5` yeniden adlandırması:** Bu dokümanın önceki sürümündeki tek-phase `Q4`
> (vision θ) artık `Q5`'tir. `Q4` Syntax Gate, `Q6` Rule Gate olarak eklenmiştir.
> `agent-prompt-semantics.md §4.1` hallucination sınıflandırması her gate'i karşılık gelen
> hata türüne map eder.

`min_approvers=2` + `θ_quorum=1.5` birlikte: en az 2 bağımsız non-author approver VE toplam
support ≥ 1.5. Tek MergeCommit (1.0) `min_approvers=2`'yi karşılamaz → `Hold` (**self-merge
prevention** yapısal). Bu imza `WitnessSet` tabanlıdır — 2 güçlü, 3 zayıf, maintainer+CI-bot
gibi kombinasyonları doğal destekler.

### 4.4 Workflow-Agnostik Hesap

#### 4.4.1 EvidenceEvent Modeli (dedup zorunlu)

Aynı olayın (ör. bir PR'ın merge-commit ile kapatılması) hem `MergeCommit` hem `PRMerged`
olarak sayılması **double-counting**'e yol açar (Lemma 2(b)'deki hatanın yapısal versiyonu).
Çözüm: önce `EvidenceEvent` modeli, sonra dedup.

```rust
struct EvidenceEvent {
    id: EvidenceId,
    source: EvidenceSource,      // "PR #42", commit SHA, trailer key
    witness_kind: WitnessKind,   // MergeCommit | PRMerged | TrailerReviewed | CoAuthored
    actor: ActorId,
    claim: ClaimId,
    weight: f64,
}
```

**Dedup kuralı:** aynı `(source, actor, claim)` üçlüsü için **en güçlü evidence** sayılır.
Örn: PR #42'nin merge-commit'i (MergeCommit, 1.0) + aynı PR'ın PRMerged (0.8) kaydı →
yalnızca MergeCommit (1.0) sayılır; PRMerged droplanır.

#### 4.4.2 Workflow-Agnostik Skor (dedup sonrası)

```
support(C) = Σ_{e ∈ dedup_events(C)} e.weight        (her event bir kez)
```

Repo-level `WitnessProfile` (Faz 1.4'te `osp-core::witness`):
```
witnessed_support = Σ (dedup'lı weights)
total_commits     = direct_to_default + merges + prs_merged
w_ratio_v2        = min(witnessed_support / (total_commits × θ_quorum), 1.0)
```

#### 4.4.3 Lokal God-mode (sağlayıcı-bağımsız)

**KARAR:** OSP'nin God-mode felsefesi (`SoftwarePhysics.txt §181`) **internet-bağımsızlığını**
gerektirir. ∴ GitHub API (`gh pr list`) yalnızca opsiyonel zenginleştirme; **asıl şahitlik
kaynağı tamamen lokal**:
- **`git2-rs` crate** ile merge-commit'lerin parent yapısı + commit-message trailer'ları parse
- `git log --pretty=format:"%h %an %s %b %(trailers)"` → `Reviewed-by:`, `Co-authored-by:` ayrıştırma
- Merge-commit imza metni (`Merge pull request #N`, `Merge branch ...`) → `PRMerged` / `MergeCommit` çıkarımı
- **Hiçbir network çağrısı yok** — OSP God-mode offline çalışır. GitLab/Bitbucket/on-prem
  git sunucularında otomatik çalışır.

Lokal gözlem sınırları için tri-state (§4.5) ayrımı şarttır — "görünmüyor" ≠ "yok".

### 4.5 Üçlü Witness Durumu (Epistemolojik Dürüstlük)

Lokal God-mode her review olayını göremez — özellikle squash/rebase workflow'unda review
metadata commit mesajına yazılmadıysa (Faz 0 kör-noktası, `spike-results.md §2`).
**"Görünmüyor" ≠ "yok".** ∴ OSP witness durumunu üçlü sınıflar:

| Durum | Koşul | Anlamı |
|---|---|---|
| **`Witnessed`** | Lokalde dedup'lı `support ≥ θ_quorum` | Nesnel uzaya kabul için yeterli şahitlik var |
| **`Unwitnessed`** | Lokalde observable ama `support < θ_quorum` | Şahitlik gerçekten eksik (foam sinyali) |
| **`Unobservable-locally`** | Squash/rebase + trailer yok; lokal kanıt yok | Karar verilemez; provider API opsiyonel zenginleştirme |

Bu ayrım OSP'nin "miş'li zaman" epistemolojisiyle uyumlu: iddia, şahitleri *gözlemlenemiyorsa*
yalan (unwitnessed) değil, **kanıt-bekleyen** (unobservable)'dır. Faz 0'da fastapi/django
"foam" sanılırdı; tri-state ile doğru etiket **`Unobservable-locally`** olurdu (squash
workflow + trailersız).

**Implementasyon:** `WitnessProfile.witness_status: WitnessStatus` enum (Faz 1.4). Raporlar
`Unwitnessed` ile `Unobservable`'ı net ayırt eder; kullanıcıya "GitHub API bağla → confidence
artır" önerisi sunar. Bu, OSP'yi lokal sınırlarını kabul eden **dürüst** bir araç yapar.

---

## 5. Vizyon Vektörü ve Sapma `θ`

### 5.1 Vizyon Vektörü `V_vision`

Projenin hedeflediği ideal konum `ℝ^(5+N)` core+custom raw uzayında:

```
V_vision = (x_v, y_v, z_v, w_v, v_v, c₁_v, c₂_v, ..., cₙ_v)
```

Core 5 eksen her projede vardır; custom N eksen God Mode tarafından register edilen
axis'lere göre değişir (§2.2). `u_v` derived'dır (raw değil), her zaman 1.0.

**KARAR (elle-deklare, zorunlu):** `V_vision` **elle deklare edilir** — üç katman:

1. **Mimari kurallar** (DDD bounded contexts, layered architecture, hexagonal) → `x_v, y_v, z_v`.
   Örn: "kuplaj ≤ 0.8, kohezyon ≥ 0.7, instability ≤ 0.5" → `x_v=0.4, y_v=0.8, z_v=0.5`.
2. **Domain/şahitlik politikaları** (review-required, branch-protection, min-witness-count,
   custom domain axis hedefleri: `security.audit_v ≥ 0.8`, `wcag.compliance_v ≥ 0.9`) →
   `v_v` + `cᵢ_v` eşikleri.
3. **Non-functional requirements** (performans bütçeleri, güvenlik çıtaları, entropi
   penceresi) → `w_v` eşiği. `u_v = 1.0` her zaman (vizyon kendisiyle hizalı).

**LLM (Faz 5) sadece ÖNERİ yapar, asla otomatik-uygulamaz.** Bu, OSP'nin "God-mode =
insan-kontrolü" felsefesinin temelidir: vizyon özneldir, AI'ın kehaneti değil. Faz 2'de
Space Engine bu deklarasyonu parse eder + ihlal bildirimi üretir; Faz 5'te LLM önerileri
yine insan-onayıyla `V_vision`'a eklenir.

### 5.2 Sapma Açısı (Naif)

```
cos θ = (V_vision · P_agent) / (||V_vision|| ||P_agent||)
θ ∈ [0, π]
```

- `θ < π/3`: hizalı (şimdiki zamana kabul için uygun)
- `π/3 ≤ θ < π/2`: sapma (kalibrasyon uyarısı)
- `θ ≥ π/2`: **negatif uzay** (anti-matter — red)

Faz 0 proxy'si (`spike-results.md §5.1`): `θ_proxy = hub_ratio × (1 - tanh(witness_depth))`.

> **⚠️ NOT (Faz 2 keşfi — Cosine Deviation Limit):**
>
> `CosineDeviation` implementasyonu `θ_norm = (1 − cos_sim) / 2` mapping kullanır
> (`cos_sim ∈ [−1, 1]` → `θ_norm ∈ [0, 1]`). AMA tüm eksen değerleri **[0, 1]
> normalize** olduğu için `P_raw` ve `V_vision`'ın tüm bileşenleri non-negative →
> dot product her zaman ≥ 0 → **`cos_sim ∈ [0, 1]`** → **`θ_norm ∈ [0, 0.5]`**.
>
> Yani iki non-zero all-positive vektör **asla ortogonal'den öte** sapamaz.
> Formalizm'in `θ ≥ π/2` (negatif-uzay) eşiği → normalize'de `θ_norm = 0.5` =
> **orthogonal limit — non-zero vektörler için unreachable**. Tek istisna:
> zero-vector (tüm eksenler 0) → `θ_norm = 1.0` (conservative max).
>
> **Sonuç:**
> - `theta_bound = 0.5` (teori-naif default, π/2 normalize) ile **drift warning imkânsız** (θ her zaman < 0.5).
> - Production runtime default: `theta_bound = 0.25` (space-engine-design.md §5.1 TOML ile senkron).
> - Üretim'de `theta_bound` **0.2–0.3** aralığında kalibre edilmeli (custom axis varlığında weight ayarı).
> - Bu limit **Faz 5+ TDA Diffusion Distance** ile aşılır (§5.3): diffusion graph
>   topology üzerinden çalışır, raw vektör bileşenlerinden değil → arbitrary
>   distance üretebilir. Cosine baseline (Faz 1-2) pratik ama limitli.
> - Keşif kaynağı: Faz 2.7 integration test (`scenario_3_drift_warning`) — empirik.

### 5.3 Diffusion Distance Reformülasyonu (TDA + Spektral Graf Teorisi)

Naif kosinüs ekseni korelasyonlarını ve grafin topolojisini yakalayamaz. Faz 1'de
**Diffusion Distance** — OSP'nin "şahitlik/vizyon yayılımı" felsefesiyle mükemmel uyumlu
(bir iddia veya şahitlik kavramsal uzayda nasıl yayılıyor? random-walk yanıtı).

**Graph Laplacian:**
```
A : n × n   komşuluk matrisi (E'den türetilmiş, ağırlıklı)
D : n × n   derece diyagonal, D_ii = Σ_j A_ij
L = D − A   (combinatorial Laplacian)
```

**Diffusion operatörü** (ısı-yayılımı / Markov-chain):
```
K_t = e^{−tL}   (t ≥ 0: zaman-parametresi, multi-scale)
f_P : V → ℝ^n ,  f_P(x) = (K_t · δ_x)   (P düğümünden t sürede yayılan ısı)
```

**Diffusion Distance:**
```
θ_diffusion(P_agent, V_vision) = || f_{P_agent} − f_{V_vision} ||₂
```

**Neden Diffusion Distance (naif kosinüs / bottleneck yerine)?**
1. **Semantik uyum:** OSP'de "vizyon yayılımı" + "şahitlik yayılımı" aynı matematikle —
   diffusion random-walk'ı bir konseptin uzayda nasıl ilerlediğini modeller.
2. **Multi-scale:** `t` parametresi yerel (küçük t) → global (büyük t) yapıyı yakalar.
   Heat Kernel Signature (HKS) ile zenginleştirilebilir (Faz 5+).
3. **Stability:** spektral perturbation teorisi — küçük graf değişikliği → küçük `θ_diffusion`
   değişikliği (Laplacian eigenvalue perturbation bound'ları). Cohen-Steiner stability
   (`literature-scan.md §3`) ile uyumlu.

**Hesap maliyeti:** `K_t = e^{−tL}` full matris eksponansiyeli O(n³). Pratikte:
- İlk `k` eigenvalue/eigenvector ile truncate (spektral yaklaşım): O(k·n)
- Sparse Laplacian + Krylov-yöntemleri (Faz 2 optimizasyonu)

**KARAR (lazy evaluation — uygulama kritik):** `θ_diffusion` HER commit'te hesaplanmaz.
Sadece: (a) `osp analyze` çağrıldığında, (b) PR açıldığında, (c) dashboard yenilenirken.
Commit akışı sırasında naif `CosineDeviation` (O(n)) kullanılır; tam diffusion hesabı lazy/periodik.
Bu, O(n³) maliyeti üretim-dışı zamanlara taşır — God-mode'un real-time olmasını sağlar.

**AÇIK SORU-3:** `t` (diffusion time) parametresi için optimal değer — kalibrasyon
korpusundan (§4.1 Faz 1.5) türetilecek. Aday: `t* = 1/λ₂` (Fiedler value'un tersi —
grafın bağlantı yapısının karakteristik zaman-skala'sı).

---

## 6. Big Bang (Uzay Genişlemesi)

Onaylanmış bir `Claim C` `t_c`'ye girdiğinde, uzay genişler:

```
commit(C, Ω):                          # Ω: WitnessSet; çağrıdan önce evaluate(C,Ω)=Commit
  0. assert evaluate(C, Ω) = Commit    # §4.3 Q1-Q4 sağlanmış (canonicalize_for içte)
  1. V ← V ∪ ΔV(C)              # yeni düğümler eklenir
  2. E ← E ∪ ΔE(C)              # yeni kenarlar eklenir
  3. G ← G' : ∀n ∈ ΔV, G'(n) = applicable_rules(n)   # kütleçekim yeniden hesaplanır
  4. ∀n ∈ ΔV ∪ N₁(ΔV): recompute position(n)   # SADECE yeni düğümler + 1-hop komşular (incremental)
     # Tam recompute lazy: osp analyze / PR-open anında (§5.3 lazy eval)
  5. t_c ← t_c + 1
  6. emit Event(t_c, C, δS, safety_weakened?)
```

**Genişlemenin fiziksel yorumu** (`SoftwarePhysics.txt §107`): yeni bir `Feature` düğümü
uzaya girdiğinde, gerçek dünyadaki izdüşümleri (yeni kurallar, validation'lar, db ilişkileri)
anında var olur → `G` güncellenir.

**KARAR (incremental position update):** Adım 4'te SADECE `ΔV` ve 1-hop komşuları
(`N₁(ΔV)`) yeniden konumlanır — `O(|ΔV| · ⟨degree⟩)` maliyet. Tam diffusion recompute
**lazy**: yalnızca `osp analyze` / PR-open anında (§5.3). Bu, her commit'in `O(|V|)` değil
`O(|ΔV|)` maliyetle işlenmesini sağlar; Faz 2'de k-hop threshold ile tune edilebilir.

---

## 7. BFT Reduction (Makalenin Merkezi Teoremi)

> `literature-scan.md §6`'da kurulan köprünün formal ispat taslağı.

### 7.1 Teorem

**Teorem (OSP Witness Commit ≡ Safety-Refinement of Authenticated BFT):**
OSP'nin `W(C, Ω)` witness-commit kuralı (§4.3), `f = 1` Byzantine hatasına karşı
**Dolev-Strong authenticated Byzantine agreement**'ın (`n > f+1`) bir **safety-refinement**'ıdır.
(Tam equivalence değil — liveness ayrı, §7.3 Lemma 2b.)

### 7.2 Reduction Mapping (OSP ↔ Dolev-Strong)

OSP commit protokolü `Π_OSP` ile Dolev-Strong `Π_DS` (authenticated, synchronous,
`literature-scan.md §6`) arası simülasyon `φ`:

| OSP | BFT (Dolev-Strong) |
|---|---|
| Agent `A` (claim author) | Designated sender `s` |
| Claim `C` | Input value `v` |
| Witness `W_i` (GitHub account, GPG-signed) | Replica `i` (signed identity) |
| `Approve(W_i)` verdict | Signed echo by process `i` |
| `support(C) ≥ θ_quorum` (§4.3 Q2) | Quorum of `f+1` distinct signatures |
| `commit(C)` → `t_c+1` | `Decide(v)` |
| `θ(P_C, V_vision) ≤ θ_bound` (vision bound, §5; **Q5 — deterministic validity predicate**) | Validity predicate |
| `OutputContract` compliance (**Q4**) + Rule compliance (**Q6**) | Syntactic/semantic well-formedness (pre-conditions) |
| main branch (replicated log) | replicated state machine log |

### 7.3 İspat (Pen-and-Paper, Makale İçin)

**Lemma 1 (Lower Bound — Gereklilik).**
Tek witness (`A + W₁`, `n = 2`, **author weight = 0**) ile `f = 1` Byzantine tolere **edilemez**.
*İspat.* OSP'nin **self-merge prevention aksiyomu** (§4.3 Q1: `distinct_non_author_approvers`)
nedeniyle author `A`'nın kendi iddiasına weight'i **0**'dır. `W₁` Byzantine olsun:

- *Liveness ihlali (false-negative, **asıl vektör**):* `W₁` doğru `C`'yi haksız `Reject` eder.
  Author kendini onaylayamadığı için `support(C) = 0` (reject) veya `≤ w(W₁) ≤ 1.0` (approve)
  — her durumda `< θ_quorum = 1.5` → `Hold`. Liveness kaybı.

- *Safety vektörü (author weight > 0 olsaydı):* Eğer OSP author weight'ini 0 yapmasaydı,
  `W₁` kötü `C`'yi approve + author self-approve → `support ≥ 2.0 ≥ 1.5` → `commit(C)` →
  safety ihlali. **OSP bu vektörü `author weight = 0` aksiyomuyla kapatır** — ama bu kez tek
  witness ile liveness imkânsızlaşır (yukarıdaki vektör).

∴ `n = 2` ile safety VE liveness **aynı anda** sağlanamaz; `n ≥ 3` zorunlu.

*(Yapısal not: §4.3 koşul 2 iki **bağımsız non-author** witness gerektirir → `n ≥ 3` zaten
operatör tanımında doğrudan. Bu aksiyom dekoratif değil, Lemma 1'in alt sınırının taşıyıcısı.)* □

*(Düzeltme notu: önceki versiyon author weight > 0 varsayımıyla false-positive vektörünü
yanlış tanımlıyordu — self-merge prevention ile çelişki. Asıl failure liveness'tır.)*

**Lemma 2a (Safety — Yeterlilik).**
İki witness (`A + W₁ + W₂`, `n = 3`) ile `f = 1` Byzantine'a karşı **Safety garanti edilir**:
kötü niyetli bir `Claim` **asla** `t_c`'ye geçemez.
*İspat.* Kötü `C` iki yolla engellenir (iki-katmanlı safety, §4.3 Q4-Q6 + Q1-Q3):

1. **Deterministik gate (Q4-Q6):** `C` negatif-uzayda (`θ > θ_bound`, Q5 ihlal) veya şema
   hatası (Q4) veya rule ihlali (Q6) içeriyorsa, motor **witness'lardan önce** reddeder.
   Bu, BFT validity predicate'ın deterministik uygulamasıdır — hiçbir witness (Byzantine
   dahil) bunu geçemez.

2. **Witness quorum (Q1-Q3):** `C` Q4-Q6'yı geçerse (geçerli şema, `θ ≤ θ_bound`,
   rule-compliant), hala iki bağımsız non-author approver (Q1) + quorum (Q2) + honest-reject
   yok (Q3) gerekir. WLOG `W₁` Byzantine, `W₂` honest olsun. Eğer `C` gizlice kötü niyetli
   ise, `W₂` (honest) `Reject` verdict verir → en fazla bir approver (`W₁`) kalır →
   `support(C) ≤ weight(W₁) ≤ 1.0 < θ_quorum = 1.5` → `commit(C) = ⊥`.

Her iki katman da kötü `C`'yi engeller; negatif-uzay ana dalda yer alamaz. □

*(Düzeltme notu: önceki versiyondaki "MergeCommit + PR-merged = 1.8" argümanı **çifte-sayımdı**
— aynı PR olayını iki kez sayıyordu. Safety için buna gerek yok; tek honest-reject yeter.)*

**Lemma 2b (Liveness — Koşullu, strict-synchronous değil).**
İki witness (`n = 3`) ile `f = 1` Byzantine'a karşı **strict-synchronous liveness garanti EDİLEMEZ**;
ancak pratik OSP dağıtımında standart BFT mekanizmalarıyla çözülür.
*İspat (imkânsızlık).* `W₁` Byzantine-sessiz (silent) olsun, `W₂` honest ve doğru `C`'yi approve
etsin. Quorum için iki bağımsız approver gerekir; yalnız `W₂` var → `support = w(W₂) ≤ 1.0 < 1.5` →
`Hold` durumu. Liveness kaybı. FLP (Corollary 2) bu durumun asenkron-deterministik çözülemez olduğunu
garantiler. □

*Pratik çözüm:* `Hold` durumları üç mekanizmadan biriyle çözülür (sıra ile denenir):
1. **3. witness çağrısı** — `n = 4 > f + 1 = 2` → Lemma 1'in alt sınırı rahat aşılır, liveness restore.
2. **Timeout + retry** — partial-synchrony varsayımı (Paxos/Raft tarzı), eventual liveness.
3. **Admin override** — `W₂` maintainer-role ise tek başına `θ_quorum`'u karşıyorsa accept;
   **ama Safety'i zayıflatır, önerilmez** (yalnızca acil-rollback senaryoları için).

**Remark (Liveness şeffaflık notu).** Lemma 2b, OSP'nin strict-synchronous BFT anlamında
`n = 3` ile tam liveness sağlamadığını açıkça kabul eder. **Ana teorik katkı — Byzantine
witness'lara karşı optimal Safety (Lemma 2a) — tamamen intact.** Liveness, standart
dağıtık-consensus pratikleri (timeout/retry, 3. quorum) ile yönetilir; bu, OSP'yi Paxos/Raft/PBFT
ile aynı kategoride konumlandırır. Dağıtık sistemlerde **Safety > Liveness** önceliği
literatürde köklüdür (Lamport, FLP) — OSP bu geleneği izler.

**Teorem (Main Safety-Refinement).** `Π_OSP`, `f = 1` için `Π_DS`'nin **safety-refinement**'idir.
*İspat.* Lemmas 1, 2a + simulation mapping §7.2:
- φ, OSP state'lerini DS state'lerine birebir örten.
- Her OSP `commit(C)`, φ altında bir DS `Decide(v)`'ye map olur.
- Lemma 1: OSP quorum kuralı **minimum** — daha az witness `f = 1` Safety'i garanti edemez.
- Lemma 2a: OSP quorum kuralı **Safety için yeter** — `n = 3` ile kötü commit kesin engellenir.
- Lemma 2b: Liveness pratik mekanizmalarla (3. witness / timeout / partial-sync) çözülür —
  strict-sync altında garanti değildir, OSP'yi standart BFT çözümleriyle aynı kategoride tutar.
- ∴ OSP'nin "author + 2 bağımsız witness" kuralı, `f = 1` Byzantine'a karşı **optimal Safety**
  sağlar; liveness pratik BFT araçlarıyla elde edilir. □

**Corollary (θ_quorum tutarlılığı).** §4.2'deki `θ_quorum = 1.5` Theorem ile tutarlıdır:
- İki güçlü witness (2 × MergeCommit = 2.0) veya (MergeCommit + PR-merged = 1.8) ≥ 1.5 ✓
- Tek witness (1.0) < 1.5 → **self-merge prevention** (safety) ✓
- Üç zayıf (3 × Trailer = 2.1) ≥ 1.5 → dark-horse zayıf-şahit çoğunluğuna izin verir,
  AMA her biri authenticated olmalı (Corollary 1).

### 7.4 Corollary'ler

**Corollary 1 (Sybil Resistance — Faz 4).** Kimlik-doğrulamasız ortamda herkes sahte witness
yaratabilir → DS authenticated modeli ihlal → Lemma 1'in lower bound'u çöker. ∴ OSP Faz 4'te:
(a) **org-bound identity** (GitHub team membership, GPG-signed commits), veya
(b) **stake-weighted witnesses** (PoS/PoA analojisi — `weight(W) = f(reputation, stake)`).

**Corollary 2 (FLP Relaxation — Faz 5).** Asenkron agent'ler (LLM agent'ları) için
deterministik liveness **FLP** (Fischer-Lynch-Paterson 1985) ile imkânsız. OSP çözümleri:
(a) **Partial synchrony** (Paxos/Raft tarzı — eventual-synchrony varsayımı), veya
(b) **Randomized quorum** (Ben-Or 1983 — yüksek-olasılıkla liveness).

**Corollary 3 (Safety-Liveness Trade-off).** `θ_quorum` arttıkça safety artar, liveness azalır.
Ağırlık kalibrasyonu (§4.1, Faz 1.5) bu trade-off'u projenin risk-profil'ine göre ayarlar.

**Corollary 4 (Lean Formalization — Faz 7).** Yukarıdaki kalemtürü ispat, makale için
yeterli. Tam mekanik doğrulama (Lean 4 / Coq) Faz 7 **stretch-goal** — encoding planı:
`Π_OSP` ve `Π_DS` state-machine'lerini Lean 4 `Inductive` tipleriyle, Lemmas 1–2'yi
`theorem ... by ...` taktikleriyle. Tahmini süre: 2–4 hafta odaklı çalışma.

---

## 8. `osp-core` Rust API Taslağı

> Bu bölüm `osp-core` crate'inin (Faz 1-2 implementasyonu) tip/imza reçetesi.

```rust
// crates/osp-core/src/lib.rs (Faz 1-2)

pub mod axes;
pub mod bigbang;
pub mod coords;
pub mod space;
pub mod time;
pub mod vision;
pub mod witness;

// ═══ Ontolojik primitifler (§1) — Faz 1.1'de gerçeklendi ═══
pub struct Node { pub id: NodeId, pub kind: NodeKind, pub mass: f64, pub position: Position }
pub enum NodeKind { Module, Concept, Feature, Bug, Rule, Agent, Intent, Claim, Witness }
pub struct Edge { pub from: NodeId, pub to: NodeId, pub kind: EdgeKind }
pub enum EdgeKind { Imports, Calls, DependsOn, PartOf, DerivesFrom, Witnesses, Approves, Violates }

pub struct Space {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: Vec<Edge>,
    pub gravity: HashMap<NodeId, GravityVector>,   // Faz 2: Rule'lerden computed
    pub time_layer: TimeLayer,
}
pub enum TimeLayer { Misli, Simdiki, Gelecek }   // t_m, t_c, t_f

// ═══ Koordinatlar (§2) — Core + Custom Raw vs Derived (inv #4, dairesellik önleme) ═══
pub struct CoreRawPosition { pub x: f64, pub y: f64, pub z: f64, pub w: f64, pub v: f64 }  // sabit 5
pub struct CustomRawPosition { pub values: HashMap<AxisId, MetricValue> }  // N custom (§2.2, MetricValue — provenance gerek)
pub struct RawPosition {
    pub core: CoreRawPosition,
    pub custom: CustomRawPosition,
}
pub struct DerivedPosition {
    pub u: f64,                        // vision alignment = 1 − θ_norm
    pub theta: f64,                    // sapma açısı (raw'dan hesaplanır)
    pub risk_score: f64,
    pub main_sequence_distance: f64,   // D = |A + I − 1| (inv #10)
}
pub struct Position { pub raw: RawPosition, pub derived: DerivedPosition }

pub trait Axis: Send + Sync {
    fn id(&self) -> AxisId;
    fn name(&self) -> &'static str;
    fn compute(&self, node: &Node, space: &Space) -> MetricValue;   // [0,1] + provenance
}
pub struct CoordinateSystem {
    pub core: [Box<dyn Axis>; 5],      // x, y, z, w, v — sabit
    pub custom: Vec<Box<dyn Axis>>,    // God Mode register (§2.2, inv #15)
}

// ═══ Şahitlik zinciri (§4) — Evidence → WitnessSet → Status → Result ═══
//          EvidenceEvent   : gözlemlenen kanıt (dedup birimi, inv #2)
//          WitnessSet (Ω)  : claim için toplanmış dedup'lı kanıtlar (inv #9)
//          WitnessStatus   : repo/claim seviyesinde epistemolojik durum (inv #3)
//          WitnessResult   : Commit / Reject / Hold kararı

pub struct EvidenceEvent {
    pub id: EvidenceId,
    pub source: EvidenceSource,      // "PR #42", commit SHA, trailer key
    pub witness_kind: WitnessKind,
    pub actor: ActorId,
    pub claim: ClaimId,
    pub weight: f64,
}
pub enum WitnessKind { MergeCommit, PRMerged, TrailerReviewed, CoAuthored }

pub struct WitnessSet {              // W(C, Ω)'nın Ω'sı
    pub events: Vec<EvidenceEvent>,  // dedup'lı (inv #2)
    pub min_approvers: usize,        // default 2 (inv #1 — author dışarıda)
    pub quorum_threshold: f64,       // θ_quorum, default 1.5
}

pub enum WitnessStatus {             // inv #3 — tri-state epistemolojik
    Witnessed,
    Unwitnessed,
    UnobservableLocally,             // squash/rebase + trailersız
}

pub enum WitnessResult {
    Commit {
        delta: Delta,
        safety_weakened: bool,       // inv #7 — admin override flag
        override_reason: Option<String>,
    },
    Reject(Reason),                  // honest-reject veya vision ihlali (Q3/Q4)
    Hold(Reason),                    // quorum yetersiz (Q1/Q2)
}

pub fn evaluate(claim: &Claim, omega: &WitnessSet) -> WitnessResult;   // W(C, Ω)

// ═══ Zaman (§3) — FSM ═══
pub trait TimeMachine {
    fn advance(&mut self, claim: &Claim, omega: &WitnessSet) -> WitnessResult;
}

// ═══ Vizyon ve sapma (§5) — θ SADECE RawPosition'ı okur (inv #4) ═══
pub struct VisionVector(pub RawPosition);   // elle-deklare (§5.1)
pub trait DeviationMetric {
    fn theta(&self, raw: &RawPosition, vision: &VisionVector, space: &Space) -> f64;
    //                                               ^^^ full Position DEĞİL — compile-time koruma
}
pub struct CosineDeviation;                 // naif (commit path'inde — inv #5)
pub struct DiffusionDeviation { t: f64 }    // spektral (yalnızca analyze — inv #5)

// ═══ Big Bang (§6) — incremental (inv #6), mutation-only ═══
// Sorumluluk ayrımı (space-engine-design.md §3.5, osp-core-design.md §3.4):
//   witness::evaluate(claim, omega) → Q1-Q3 karar
//   bigbang::apply_delta(space, delta) → sadece mutasyon
//   SpaceEngine::commit(claim, omega) → full orchestration (Q4-Q6 + Q1-Q3 + apply_delta + reposition)
pub fn apply_delta(space: &mut Space, delta: &Delta) -> Vec<NodeId>;  // mutasyon, infallible
// position recompute: sadece ΔV ∪ N₁(ΔV); tam recompute lazy (engine::full_reposition)
```

---

## 9. Faz 1 Uygulama Planı (`osp-core`) — Yeniden Sıralı

> Reviewer notu (arkadaş incelemesi): şahitlik + pozisyon modeli EKSENLERDEN ÖNCE sağlamlaştırılmalı.
> Aşağıdaki sıra witness+position refactor'u öne alır; y/z eksenleri sonraya iter.

| Adım | İçerik | Çıktı | Durum |
|---|---|---|---|
| 1.1 | `Node/Edge/Space/NodeKind/EdgeKind/TimeLayer` tipleri | `osp-core::space` | ✅ |
| 1.2 | `Axis` trait + `CoordinateSystem` pluggable altyapı | `osp-core::coords` | ✅ |
| 1.3 | Raw eksenler (x Coupling, w Entropy, v WitnessDepth) Faz 0'dan | `osp-core::axes` | ✅ |
| 1.4 | **RawPosition/DerivedPosition refactor** + `VisionAlignmentAxis` → derived (inv #4) | `osp-core::coords` | ⬜ |
| 1.5 | **`EvidenceEvent` + dedup + `WitnessSet` + `W(C,Ω)` + tri-state `WitnessStatus`** (inv #1,2,3,9) | `osp-core::witness` | ⬜ |
| 1.6 | `Intent/Claim` tipleri + osp-spike'a `w_ratio_v2` + tri-state bağla | bağlantı | ⬜ |
| 1.7 | `VisionVector` (elle-deklare) + `CosineDeviation` (naif θ) + `D` derived metric (inv #10) | `osp-core::vision` | ⬜ |
| 1.8 | `TimeLayer` FSM + `commit()` (Big Bang, incremental — inv #5,6) | `osp-core::time`, `osp-core::bigbang` | ⬜ |
| 1.9 | `y` (LCOM4 tree-sitter pseudo-type) + `z` (saf Martin Instability `I`) raw eksenleri | `osp-core::axes` | ⬜ |
| 1.10 | Re-spike: `w_ratio_v2` + tri-state ile 5-repo (fastapi/django/date-fns doğru sınıfla) | doğrulama | ⬜ |
| 1.11 | **Kalibrasyon**: 15-20 repo korpusu (Py/Rust/TS/Go) — weight + `θ_quorum` + `t*` tuning | kalibrasyon | ⬜ |

**Tüm invariant'lar:** `docs/implementation-invariants.md` (15 adet — her adım ilgili
invariant'a atıfla uygulanır).

**Kabul kriteri (1.10):** fastapi/django/date-fns `w_ratio_v2 > 0.3` VEYA doğru biçimde
`Unobservable-locally` etiketli (squash kör-noktası çözüldüğünün kanıtı, üçlü durum dahil).

---

## 10. Makale İçin Açık Sorular

**Çözülenler (Faz 1.0 tasarım kararları ile):**
1. ~~`y, z` eksenleri~~ → **ÇÖZÜLDÜ**: y = LCOM4 (Faz 1 tree-sitter, Faz 3 SCIP), z = Martin Instability `I` (§2 KARAR).
2. ~~`f_P` tanımı~~ → **ÇÖZÜLDÜ**: Diffusion Distance (§5.3).
3. ~~Kalibrasyon verisi~~ → **ÇÖZÜLDÜ**: Faz 1.5'te 15-20 repo, çok-dilli (§4.1, §9 adım 1.11).
4. ~~BFT ispatı~~ → **ÇÖZÜLDÜ**: Kalemtürü reduction proof §7.3'te. Lean Faz 7 stretch.
5. ~~Vizyon vektörü~~ → **ÇÖZÜLDÜ**: Elle-deklare, 3 katman (§5.1). LLM sadece öneri.

**Kalan açık sorular:**
6. **`t*` diffusion parametresi** (§5.3) → **future-work**: kalibrasyon korpusunda ampirik
   tuning (Faz 1.5). Aday `t* = 1/λ₂`. Lazy-evaluation (§5.3 KARAR) sayesinde üretim-isı yok —
   sadece analyze/PR-open anında hesap.
7. ~~Distance-from-Main-Sequence `D` entegrasyonu~~ → **ÇÖZÜLDÜ (revize)**:
   `z = I` saf Martin Instability olarak kalır. `D = |A + I − 1|` **ayrı derived metric**
   (`P_derived.main_sequence_distance`, inv #10). θ'ya opsiyonel ceza bileşeni:
   `θ_eff = θ × (1 + α·D)`. (Önceki `z = I × (1−D)` kararı bilgi kaybı nedeniye terk edildi.)
8. **LCOM4 pseudo-type kalitesi** — tree-sitter heuristiği SCIP ground-truth ile uyumu:
   - **Beklenen doğruluk:** %60-75 (Faz 1-2 heuristic), hedef >%80 (Faz 3 SCIP).
   - **Neden tolere edilebilir:** çok-eksenli sistemde (5 core + N custom) `y` tek başına karar vermez — entropi
     (`w`) + şahitlik (`v`) hata marjını emer (multi-axis redundance).
   - **SCIP ground-truth metodolojisi (Faz 3):** her sınıf için method-field erişim grafı
     kur → bağlı bileşen sayısı = LCOM4. Dil sunucuları: pyright (Python), tsc (TS),
     rust-analyzer (Rust), gopls (Go).
   - **Karşılaştırma metrikleri:** Pearson/Spearman korelasyonu (hedef `r > 0.8`), binary
     sınıflandırma (LCOM4 = 1 vs ≥ 2, hedef accuracy `> 0.75`), dil-bazında ayrıştırma.
   - **Fallback (accuracy < %60):** `y` yerine kompozit `y_effective = w × (1 − LCOM4_norm)`
     (entropi-ağırlıklı kohezyon proxy'si).
9. ~~`gh pr list` alternatifi~~ → **ÇÖZÜLDÜ**: `git2-rs` + trailer parsing, lokal God-mode
   (§4.4 KARAR). GitHub API opsiyonel zenginleştirme only.
10. **Liveness'in pratik doğrulanması** (Lemma 2b) — 3. witness / timeout mekanizmalarının
    gerçek OSS projelerinde tetiklenme sıklığı (Faz 1.5 korpusunda ölçülecek).

---

## 11. Referanslar (literatür taraması çapraz)

- `docs/literature-scan.md §3` — TDA (Edelsbrunner, Cohen-Steiner stability)
- `docs/literature-scan.md §6` — BFT (Dolev-Strong, Lamport, FLP)
- `docs/literature-scan.md §5` — Ontoloji bileşenleri (Gómez-Pérez)
- `docs/literature-scan.md §2` — Yazılım metrikleri (McCabe, Halstead, Tempero-Ralph)
- **Robert C. Martin, *Clean Architecture* (2017)** — Instability `I`, Abstractness `A`,
  Distance from Main Sequence `D = |A+I−1|` (z-ekseni ve θ-bileşeni, §2/§5.2)
- **R. R. Coifman & S. Lafon, "Diffusion Maps" (2006)** — Diffusion Distance temeli (§5.3)
- **U. von Luxburg, "A Tutorial on Spectral Clustering" (2007)** — graph Laplacian pratikleri
- `docs/spike-results.md §5-6` — Ampirik Faz 0 verisi

---

*Sonraki doküman: `docs/osp-core-design.md` (Faz 1 implementasyon detayı) — sonra kod.*
