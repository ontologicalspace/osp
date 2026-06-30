# OSP Agent Trajectory Roadmap — Mimari Navigasyon Protokolü

> **Durum:** Taslak (v0.1) — review + rafinasyon sonrası implementasyon
> **Tarih:** 2026-06-30
> **İlişki:** Paper 1 (Statik Uzay) tamamlandı → bu doküman Paper 2 (Dinamik/Agent)'nin omurgası
> **3D viewer durumu:** DURDURULDU (görsel geliştirme, agent işleri öncelik)

---

## 1. Motivasyon — Neden Bu Katman?

Mevcut OSP **statik bir uzay** tanımlar: SCIP/tree-sitter ile metrik çıkarımı → 5-axis
coordinates → vision evaluation → tri-state witness. Bu "anlık fotoğraf" almakta mükemmel.

**Ama geliştirme sürecindeki kritik halka eksik:**

```
[İNSAN] task yazar → [LLM agent] kod yazar → [OSP] "uygun mu?" kontrol
   ↑ burası tanımsız                ↑ OSP'nin mevcut gücü
```

OSP **kısıtları** (constraints: coupling ≤ 0.30) biliyor ama **niyeti** (intent: bu
task neden var, hangi role ait) bilmiyor. Agent'a sadece "kurala uy" deniyor,
"şu yöne git" denmiyor.

**Bu doküman:** OSP'yi reaktif bir kapıdan (gate) **proaktif bir mimari navigasyon
protokolüne** taşır. Temel fikir (beyin fırtınası yorumlarından):

> **Task, doğal dilde bir iş değildir; gelecekte engine tarafından ölçülecek mimari
> durum üzerinde doğrulanabilir bir şarttır (predicate).**
> **Agent, kod üreten bir işçi değil; mimari yörüngeyi takip eden bir navigatördür.**

**Önemli ayrım (review 1 düzeltmesi):** Task bir *hareket vektörü* (movement vector)
değildir. Hareket vektörü `Δ⃗T = S_target - S_current` operatör/planner seviyesinde
hesaplanır ve agent'a **verilmez**. Agent sadece predicate'i görür (§3 hibrit model,
INV-T1). Bu ayrım korunmazsa "agent pozisyon belirler" yanılgısı doğar — ki bu OSP'nin
en temel invariant'ını (engine ölçer) ihlal eder.

---

## 2. Tez Cümlesi

> **A task is not a claimed coordinate and not a structural delta.
> A task is a verifiable measurement predicate over future engine-measured coordinates.**

Bu cümle Paper 2'nin tezidir. Mevcut paper'ın *"the engine computes the raw position"*
(§6.1) tanımını bir adım öteye taşır: sadece pozisyonun değil, **task'ın kendisinin**
epistemolojik statüsünü tanımlar.

---

## 3. Hibrit Model: Predicate (güven) + Coordinate (güç)

**Saf koordinat modeli** matematiksel olarak güçlü (Yorum 1/3'ün vektör matematiği)
ama **inv ihlali** riski taşır: agent hedef koordinatı görürse "AI söylediği için
doğru" tarafına kayar.

**Saf predicate modeli** epistemolojik olarak güvenli (inv korunur) ama **navigasyon
gücü** eksik — "0.55'in altında" tek nokta değil bir bölge tanımlar.

**Çözüm (onaylanan hibrit):** İki katmanı ayrı ontolojik seviyelere koy:

```
Matematiksel güç    → Milestone katmanı (koordinat vektörleri, Trajectory)
Epistemolojik güven → Task katmanı (measurement predicate, agent'a verilir)
```

**Kritik:** `Milestone.target_vector` ile `Task.target_predicate` arasındaki dönüşüm
**operator/planner seviyesindedir**. Agent predicate'i görür, koordinatı GÖRMEZ.

---

## 4. Yeni Ontolojik Tipler

### 4.1 Trajectory (Yörünge)

```rust
/// Vision'dan türetilmiş, sıralı Milestone'lar dizisi. Bir projenin "nereye
/// gideceği" planı. Operator (insan mimar / planner) tanımlar — agent DEĞİL.
pub struct Trajectory {
    pub id: TrajectoryId,
    pub vision: VisionVector,              // hedef mimari (ℝ³ + source)
    pub milestones: Vec<Milestone>,        // sıralı ara hedefler (waypoints)
    pub created_at: Instant,
    pub status: TrajectoryStatus,          // Planned / Active / Completed / Superseded
}
```

### 4.2 Milestone (Ara Hedef)

```rust
/// Trajectory üzerinde bir waypoint. Uzayda ulaşılması gereken ara koordinat.
/// Operator/planner tarafından tanımlanır; koordinat agent'a verilmez,
/// predicate'e dönüştürülür.
pub struct Milestone {
    pub id: MilestoneId,
    pub label: String,                     // "Repository layer ayrımı"
    pub target_region: TargetRegion,       // kabul bölgesi (tek nokta DEĞİL — review 1, F1)
    pub scope: MilestoneScope,             // node / module / subgraph
    pub tasks: Vec<TaskId>,                // bu milestone'a bağlı tasklar
    pub status: MilestoneStatus,           // Pending / InProgress / Achieved / Failed
}

/// Milestone tek nokta değil, KABUL BÖLGESİ tanımlar (F1 çözümü, review 1).
/// Gerçek mimaride "tam 0.55 coupling, tam 0.70 cohesion" nokta hedefleri kırılgan.
/// Region = predicate bölgesi; preferred_vector = navigasyon için ideal merkez.
pub struct TargetRegion {
    pub predicates: Vec<MetricPredicate>,  // bölgeyi tanımlayan şartlar (AND)
    pub preferred_vector: Option<RawPosition>, // ideal merkez (navigasyon, debug)
}
```

**Neden region, tek nokta değil?** (F1 formalizasyon sorunu — çözüldü)
- Multi-axis predicate (`coupling ≤ 0.55 AND cohesion ≥ 0.70`) tek nokta değil bölge.
- `preferred_vector` navigasyon/distance hesabı için ideal merkez, ama **sert kriter
  değildir** — region içinde herhangi bir nokta milestone'uAchieved yapar.
- Tek nokta hedefler (eski `target_vector: RawPosition`) kırılgandı; region esnek ama
  yine de ölçülebilir (her predicate engine-measured).


### 4.3 Task (Ölçülebilir Niyet) — HİBRİT MODELİN ÖZÜ

```rust
/// Bir Milestone'a ulaşmak için uzayda yapılması gereken ölçülebilir hareketin
/// PREDICATE karşılığı. Agent'a bu verilir — koordinat hedefi DEĞİL.
///
/// ÖRNEK: "StoreRepository coupling measured ≤ 0.55 olmalı"
///   metric: Coupling, operator: Le, threshold: 0.55, scope: node(StoreRepository)
pub struct Task {
    pub id: TaskId,
    pub milestone_id: MilestoneId,
    pub label: String,                     // insan-okur: "Reduce StoreRepository coupling"
    pub target_predicate: MetricPredicate, // ölçüm şartı (epistemolojik güven)
    pub allowed_operations: Vec<OpKind>,   // agent'a izin verilen structural ops
    pub constraints: Vec<Rule>,            // ek kısıtlar (Q6 Rule gate)
    pub status: TaskStatus,                // Pending / Assigned / InProgress / Done / Blocked
}
```

### 4.4 MetricPredicate (Ölçüm Şartı) — mevcut MetricValue ile uyumlu

```rust
/// Engine-measured koordinat üzerinde doğrulanabilir şart.
/// MetricValue provenance'ı korur (measured/scip/placeholder/heuristic).
pub struct MetricPredicate {
    pub metric: PredicateAxis,             // hangi eksen (coupling/cohesion/instability/...)
    pub operator: ComparisonOp,            // Lt | Le | Gt | Ge | Eq | Ne
    pub threshold: f64,                    // ölçülecek eşik değeri
    pub scope: PredicateScope,             // Node(id) | Module(name) | Subgraph(ids)
    pub required_source: Option<MetricSource>, // measured/scip zorunluysa (provenance)
    pub tolerance: f64,                    // ε — "≤ 0.55 ± 0.02"
}

pub enum PredicateAxis {
    Coupling, Cohesion, Instability, Entropy, WitnessDepth,
    RiskScore, MainSequenceDistance,
    Custom(String),                        // security.audit, wcag.compliance vb.
}

pub enum ComparisonOp { Lt, Le, Gt, Ge, Eq, Ne }

pub enum PredicateScope {
    Node(NodeId),
    Module(String),
    Subgraph(Vec<NodeId>),
}
```

### 4.5 View Ayrımı — Agent vs Internal (review 1, INV-T1 için kritik)

**En kritik teknik nokta:** Hibrit modelin güvenliği, `Milestone.target_vector`'ün
**agent'a sızmamasına** bağlı. Mevcut `Intent { target_raw: RawPosition }` bu sızıntıya
açık. Çözüm: iki ayrı view.

```rust
/// Agent'a serialize edilen görünümdür. KOORDİNAT HEDİFİ İÇERMEZ (INV-T1).
/// Agent bu view'ı alır, DeltaProposal üretir. Sadece predicate + mevcut ölçüm +
/// izinli operasyonlar + kısıtlar.
pub struct AgentTaskView {
    pub task_id: TaskId,
    pub label: String,                     // "Reduce StoreRepository coupling"
    pub current_measurement: RawPosition,  // engine-measured mevcut durum
    pub target_predicate: MetricPredicate, // ölçüm şartı (koordinat DEĞİL)
    pub allowed_operations: Vec<OpKind>,   // agent'ın araç kutusu
    pub constraints: Vec<Rule>,            // ek kısıtlar
}

/// Engine/planner/debug içindir. Koordinat hedefini taşır ama agent'a
/// serialize edilmez. `Intent::from_task` bu view'ı kullanır.
pub struct InternalTaskPlan {
    pub task_id: TaskId,
    pub milestone_target_vector: RawPosition,  // koordinat hedefi (operator seviyesi)
    pub task_predicate: MetricPredicate,       // predicate (agent'a verilir)
    pub tolerance: f64,
}

/// Agent'ın yapabileceği structural operasyonlar (review 2 önerisi — Task.allowed_operations
/// için). Planner, Task'a "coupling düşürmek için sadece import'ları soyutla, yeni modül
/// yaratma" diyebilir.
pub enum OpKind {
    AddImport, RemoveImport,
    AddAbstraction,      // interface/trait ekle (dependency inversion)
    ExtractModule,       // mevcut kodu yeni modüle taşı
    AddNode, RemoveNode,
    AddEdge, RemoveEdge,
    ModifyEntity,        // kind/mass/metadata (RawPosition hariç)
}
```

**Serileştirme kuralı:** `AgentTaskView` JSON'a çevrilirken `target_predicate`'in
**nominal koordinat karşılığı** asla eklenmez. Engine, `InternalTaskPlan`'dan
`AgentTaskView` üretirken koordinatı düşürür. Bu `docs/invariant-spec.md`'de formal
garanti edilir (INV-T1).

### 4.6 TaskAttempt + PredicateGateResult (review 1 — progress ayrımı)

Bir Task birden fazla deneme gerektirebilir (örn. coupling 0.82 → hedef 0.55):
attempt 1 reddedilebilir ama **yön doğru** (0.71), attempt 2 ilerler (0.63), attempt 3
başarır (0.53). Bu ayrım olmadan sistem çok katı olur — her reddi "başarısızlık" sayar.

**Kritik ayrım (INV-T6, aşağıda):** Predicate failure ≠ negative progress. Bir
DeltaProposal predicate'i sağlamayabilir ama milestone'a **yaklaşmış** olabilir.

```rust
/// Bir Task için tek bir deneme. Agent'ın bir DeltaProposal'ı → Claim → gate akışı.
pub struct TaskAttempt {
    pub id: TaskAttemptId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub claim_id: Option<ClaimId>,         // gate geçerse witness'a gider
    pub measured_before: RawPosition,
    pub measured_after: RawPosition,
    pub gate_results: Vec<GateResult>,     // Q4/Q5/Q5.b/Q6 sonuçları
    pub predicate_result: PredicateGateResult,
    pub status: AttemptStatus,             // Rejected / Committed / Witnessed
}

/// Q5.b Predicate Gate çıktısı — sadece done/not-done değil, progress farkı.
/// review 1'in "Satisfied / Improved / Regressed" üçlüsü.
pub enum PredicateGateResult {
    /// Predicate sağlandı — task kapanabilir.
    Satisfied,
    /// Sağlanmadı ama milestone'a yaklaşıldı (distance azaldı).
    UnsatisfiedButImproved { previous_distance: f64, new_distance: f64 },
    /// Sağlanmadı VE milestone'dan uzaklaşıldı (axis oscillation, review 3).
    UnsatisfiedAndRegressed { previous_distance: f64, new_distance: f64 },
}

pub enum AttemptStatus { Rejected, Committed, Witnessed }
```

**Evidence Ledger** (`TrajectoryEvidence`) her attempt'in token cost + duration'ını
kaydeder → RQ6 (token), RQ7 (task success), RQ8 (correction değeri) için ham veri.
Detay §8 Aşama B2.



---

## 5. Uyum Haritası — Mevcut OSP Tipleriyle

Bu bölüm yeni tiplerin mevcut ontolojiye **nasıl oturduğunu** gösterir.
Invariant'lar bozulmadan entegrasyon için kritik.

### 5.1 Ontolojik hiyerarşi (mevcut + yeni)

```
Trajectory (YENİ)
  └─ VisionVector (MEVCUT — vision.rs)         → hedef mimari
       └─ Milestone (YENİ)                       → ara koordinat hedefi
            └─ Task (YENİ)                       → measurement predicate
                 └─ Intent (MEVCUT — witness.rs)  → t_f, agent'ın hedefi
                      └─ Claim (MEVCUT)           → t_m, agent'ın işi
                           └─ DeltaProposal (MEVCUT — agent.rs) → structural change
                                └─ Engine computes P_raw (MEVCUT)
                                     └─ Q5 Vision Gate (MEVCUT)
                                     └─ Q5.b Predicate Gate (YENİ) ← Task kontrolü
```

### 5.2 Mevcut tiplerin yeniden kullanımı

| Mevcut tip | Yeni katmanda rolü | Değişiklik |
|---|---|---|
| `VisionVector` | Trajectory'nin hedefi | Yok (mevcut) |
| `RawPosition` | Milestone.target_vector | Yok (mevcut) |
| `MetricValue` + `MetricSource` | MetricPredicate'in provenance'ı | Yok (mevcut) |
| `Intent` (witness.rs) | Task → Intent dönüşümü | **target_raw → predicate'den türetilir** |
| `Claim` (witness.rs) | Task → Claim akışı | **task_id alanı eklenir** |
| `DeltaProposal` (agent.rs) | Agent'ın ürettiği structural change | Yok (mevcut) |
| `Rule` (rule.rs) | Task.constraints | Yok (mevcut) |
| `OutputContract` (agent.rs) | Task.allowed_operations kaynaklı | **allowed_ops alanı eklenir** |

### 5.3 Kritik entegrasyon noktaları

**N1: Intent.target_raw'in durumu — ÇÖZÜLDÜ (review 1, §4.5)**
Mevcut `Intent { agent, target_raw: RawPosition }` doğrudan koordinat alıyor — bu
agent'a sızma riski. Çözüm: **AgentTaskView / InternalTaskPlan ayrımı** (§4.5).
- `AgentTaskView` (agent'a serialize) — **koordinat İÇERMEZ**, sadece predicate.
- `InternalTaskPlan` (engine/planner) — koordinat taşır, agent'a verilmez.
- `Intent::from_task` predicate'in nominal koordinatını `InternalTaskPlan`'dan alır,
  ama bu sadece engine içindir. Agent serialization'ında `target_raw` çıkarılır.
**Garanti:** `docs/invariant-spec.md` INV-T1'de formal — AgentTaskView serde'de
koordinat alanı yok (serde derive ile yapısal zorunluluk).

**N2: Q5 Vision Gate genişletme**
Mevcut Q5: `θ(claim.computed_raw, vision) > bound → reject`.
Yeni Q5.b: `task.target_predicate(claim.computed_raw) == false → reject`.
İki gate ardışık: önce θ (mimari sapma), sonra predicate (task hedefi).

**N3: Task → Claim zinciri**
Mevcut akış: `DeltaProposal → Claim`. Yeni: `Task → Intent(from task) → Claim(task_id)`.
Claim artık **hangi task'a hizmet ettiğini** bilir → trajectory progress tracking.

---

## 6. Invariant'lar (Korunacak Kurallar)

Yeni katman OSP'nin 15 mevcut invariant'ına zarar vermemeli. Ek invariant'lar:

- **INV-T1 (Predicate epistemolojisi):** Task, agent'a **predicate olarak** verilir.
  Koordinat hedefi (`Milestone.target_vector`) agent'a gösterilmez. Sadece operator
  ve planner (trusted) görür.

- **INV-T2 (Operator tanımlar hedef):** `Trajectory` ve `Milestone` **trusted operator**
  tarafından tanımlanır (inv #13, #15 ile uyumlu). Agent hedef belirlemez, sadece
  oraya giden structural change (DeltaProposal) üretir.

- **INV-T3 (Engine ölçer — korunmuş):** Task predicate'i **engine-measured** değer
  üzerinde değerlendirilir (`claim.computed_raw`). Agent ölçmez, engine ölçer (inv #4).

- **INV-T4 (Predicate provenance):** MetricPredicate `required_source` alanı ile
  "measured" zorunlu kılabilir. Placeholder/heuristic kaynaklı ölçümlerle task
  kapatılamaz (epistemolojik bütünlük).

- **INV-T5 (Task ≠ Claim):** Task bir **şart** (predicate), Claim bir **iş** (structural
  delta). Bir task birden fazla claim gerektirebilir; bir claim bir task'a hizmet eder.

- **INV-T6 (Failure ≠ regression — review 1):** Predicate failure negative progress
  *gerektirmez*. Bir DeltaProposal predicate'i sağlamasa bile milestone'a yaklaşmış
  olabilir (`PredicateGateResult::UnsatisfiedButImproved`). OSP, completion ile
  directional improvement'ı ayırır — uzun refactoring task'larında sürekli reject
  yerine progress sinyali. Aksi halde sistem çok katı olur.

- **INV-T7 (Maneuver limit — review 3):** Bir Task için ardışık N reddedilen attempt'tan
  sonra sistem kendini kilitler ve **Trajectory Deviation Alert** ile operatöre (God Mode)
  kontrol devreder. Sonsuz context-loop ve token patlaması önlenir. N operator tarafından
  yapılandırılır (default: 5).

---

## 7. Adaptif Mimari Kontrol Döngüsü

Yorum 2'nin "Planla → Uygula → Kontrol Et → Düzelt" döngüsünün OSP karşılığı:

```
1. PLANLA
   Operator → Trajectory (vision + milestones)
   Milestone.target_vector → Task.target_predicate (planner dönüşümü, agent görmüyor)

2. UYGULA
   Agent ← Task(predicate + mevcut ölçüm + allowed_ops + constraints)
   Agent → DeltaProposal (structural change)
   Agent kabuğu → Claim (task_id, intent from task, delta_nodes/edges)

3. KONTROL ET
   Engine Q4 Syntax → Q5 Vision (θ) → Q5.b Predicate → Q6 Rule
   [Tüm gate'ler geçerse] Witness Q1-Q3

4. MUTATE + ÖLÇ
   apply_delta → re-analyze → yeni P_raw (engine ölçer)

5. DÜZELT (Trajectory Correction)
   Yeni P_raw vs Milestone.target_vector → progress %
   Task.target_predicate(P_raw) == true? → Task Done
   Milestone'ın tüm taskları Done? → Milestone Achieved
   Tüm milestone'lar Achieved? → Trajectory Completed
   Sapma varsa → Trajectory replan (yeni milestone'lar)
```

---

## 8. Aşamalı İmplementasyon Planı

Her aşama **kanıt/not toplar** (docs/paper2-notes/) — paper en son yazılır.

### Aşama A — Ontolojik Tipler (minimal, test odaklı)
**Hedef:** Trajectory/Milestone/Task/MetricPredicate tiplerini kodla, unit test.
**Dosyalar:** `crates/osp-core/src/trajectory.rs` (yeni), `coords.rs` genişletme.
**Kapsam Dışı:** Agent döngüsü, UI, gerçek planner.
**Paper2 notları:** Ontolojik kararlar, invariant'ların korunması ispatı.
**Efor:** S

### Aşama B — Predicate Gate (Q5.b)
**Hedef:** Engine'e Q5.b Predicate Gate ekle. `check_claim_predicate(claim, task)`.
**Dosyalar:** `engine.rs` genişletme, `trajectory.rs`.
**Test:** Predicate ihlali → reject, geçerse → witness'a geç.
**Paper2 notları:** Deterministik reddin metric kaliteye etkisi.
**Efor:** S-M

### Aşama B2 — TaskAttempt Ledger (review 1 — evidence olmadan Paper 2 eksik)
**Hedef:** `TaskAttempt` + `TrajectoryEvidence` kayıt sistemi. Her attempt'in
predicate_result (Satisfied/Improved/Regressed), token cost, duration'u kaydedilir.
**Dosyalar:** `trajectory.rs` (TaskAttempt, TrajectoryEvidence, PredicateGateResult).
**Test:** 3-attempt senaryosu (0.82→0.71→0.63→0.53) → Improved→Improved→Satisfied.
**Paper2 notları:** **RQ6/RQ7/RQ8 ham verisi** — kaç attempt'te success, gate kaç
reddi önledi, token cost attempt başına. Bu aşama olmadan Paper 2 ölçüm eksik kalır.
**Efor:** S-M

### Aşama C — Planner (Milestone → Task → Intent)
**Hedef:** `Milestone.target_vector` → `Task.target_predicate` dönüşümü (operator/planner).
**Dosyalar:** `trajectory.rs`, `witness.rs` (Intent::from_task).
**Risk:** N1 — predicate ↔ koordinat tutarlılığı (§9).
**Paper2 notları:** Dematerialization of tasks — koordinattan predicate'e dönüşüm matematiği.
**Efor:** M

### Aşama D — Agent Döngüsü (Navigator)
**Hedef:** Agent'a Task serialize → DeltaProposal → Claim → gate → trajectory update.
**Dosyalar:** `agent.rs`, yeni `navigator.rs` modülü.
**Test:** svelte corpus'ta gerçek task simulasyonu.
**Paper2 notları:** **Token maliyeti** (RQ6 adayı), task success (RQ7 adayı).
**Efor:** M-L

### Aşama E — Trajectory Correction + UI (opsiyonel, 3D viewer ile birlikte)
**Hedef:** Commit sonrası progress tracking, trajectory replan, 3D viewer'da gösterim.
**Bağımlılık:** 3D viewer Aşama 4 (selection glow) tamamlanmalı.
**Paper2 notları:** Adaptive control loop'un gerçek reponlarda davranışı.
**Efor:** M-L

---

## 9. Açık Formalizasyon Sorunları (çözülecek)

### F1: Predicate ↔ koordinat tutarlılığı — ÇÖZÜLDÜ (review 1)
Multi-axis predicate için `Milestone.target_vector` **tek nokta değil, TargetRegion** oldu
(§4.2). Region = predicate bölgesi + preferred_vector (ideal merkez, navigasyon için).
Her predicate engine-measured; preferred_vector sert kriter değil. Margin sorunu
`target_predicate.tolerance` (ε) içinde çözülür.

### F2: ε tolerance anlamı
Epsilon **Milestone başına** mı (ara hedefe yakınlık), **Task başına** mı (tek ölçümde
kabul), **Trajectory parçası** başına mı (Yorum 3'ün uyarısı: tek commit'te coupling
0.82→0.55 inmez)?
**Çözüm yolu:** Muhtemelen **Trajectory parçası başına** — her Milestone birkaç task/commit.

### F3: Genesis (Yorum 3'ün A/B sorusu)
`S_target`'ı kim tanımlar?
- **Seçenek A (önerilen):** İnsan mimar (God Mode) — invariant'lar korunur.
- **Seçenek B:** Agent PRD'den çıkarır — inv #15 ihlali riski.
**Karar:** AŞAMA A'da Seçenek A. B opsiyonel (paper conjecture, kanıtlanmamış).

### F4: Task kapanma kriteri
"Bug çözüldü ama coupling arttı → task bitmedi" (Yorum 1). Yani task kapanması
**sadece predicate == true** ile mi, yoksa + ek kalite kontrolü ile?
**Çözüm yolu:** Predicate ana kriter, Q5/Q5.b/Q6 ek güvenlik. INV-T6 (failure ≠
regression) ile completion (predicate satisfied) vs directional improvement ayrılır.

### F5: Axis Oscillation (eksen salınımı — review 3)
Ajan bir ekseni düzeltirken (coupling ↓) farkında olmadan başka eksen bozabilir
(instability ↑). Sonraki adımda o ekseni düzeltirken ilk eksen tekrar kaçar → salınım.
**Çözüm yolu (Aşama C):** Tekil eksen optimizasyonu yerine **çok boyutlu kayıp fonksiyonu**
(Loss Function) veya Pareto optimizasyonu. `PredicateGateResult::UnsatisfiedAndRegressed`
bu durumu tespit eder (distance arttı) → agent'a "eksenler arası gerilimi minimize et"
sinyali. `preferred_vector` navigasyon merkezi olarak salınımı kırar.
**Paper materyali:** RQ8 adayı — multi-axis trajectory'nin single-axis'ten üstünlüğü.

### F6: Sonsuz döngü / token patlaması (Maneuver Limit — review 3)
Agent üst üste aynı kuralı çiğnemeye devam ederse context-loop'a girer, token maliyeti
patlar. **Çözüm yolu (INV-T7):** Bir Task için ardışık N (default 5) reddedilen attempt
sonra **Trajectory Deviation Alert** → operatöre (God Mode) kontrol devri. N operator
tarafından yapılandırılır (task bazlı: karmaşık refactor = 10, basit = 3).
**Paper materyali:** "deterministic gating'in token maliyetine etkisi" — RQ6 verisi.

---

## 10. Paper 2 Not Toplama Mekanizması

Dizin: `docs/paper2-notes/` (her aşamada ayrı not)

```
docs/paper2-notes/
  README.md                    — dizin yapısı + hangi not ne zaman yazıldı
  stage-A-ontology.md          — ontolojik kararlar, invariant ispatları
  stage-B-predicate-gate.md    — Q5.b deterministik reddin etkisi
  stage-B2-attempt-ledger.md   — TaskAttempt evidence, RQ6/RQ7/RQ8 ham veri
  stage-C-planner.md           — task dematerialization matematiği, axis oscillation
  stage-D-agent-loop.md        — token maliyeti, task success, maneuver limit
  stage-X-failures.md          — başarısız denemeler (review 1 — Paper 2 için değerli)
  evidence/                    — ham ölçümler (JSON), corpus sonuçları
    trajectory-bench-svelte.json
    predicate-gate-stats.json
```

**`stage-X-failures.md` (review 1 önerisi):** Başarısız denemeler Paper 2'yi güçlendirir:
agent predicate'i yanlış yorumladı, predicate çok katıydı, placeholder metric ile task
kapatılmaya çalışıldı, multi-axis predicate conflict oluştu, DeltaProposal doğruydu ama
Q6 rule gate reddetti. Bu edge case'ler makaleyi gerçekçi kılar — sadece başarıları değil,
sistemin sınırlarını da gösterir.

**Disiplin:** Her implementasyon aşaması bitiminde o aşamanın notu yazılır.
Paper 2, bu notlardan data-driven yazılır (iddia değil, kanıt).

---

## 11. Paper Stratejisi (iki makale)

### Paper 1 (Mevcut — Statik Uzay) — TAMAMLANDI
SCIP + tree-sitter + 5-axis + vision + witness + tri-state. Kanıtlanmış (23 repo).
**Trajectory referansı:** §11 Future Work'te 1-2 paragraf — "biz bu boşluğun farkındayız".

### Paper 2 (Dinamik/Agent) — bu dokümanın omurgası
"Architectural Trajectory Navigation: From Static Space to Dynamic Software Physics"
- §1 Trajectory ontolojisi (§4 bu doküman)
- §2 Task dematerialization (§9 F1 çözüldü — TargetRegion)
- §3 Adaptive control loop (§7)
- §4 Deterministic predicate gating (Aşama B) — progress ayrımı (INV-T6)
- §5 Attempt evidence & token cost (Aşama B2+D — RQ6)
- §6 Task success (Aşama D — RQ7)
- §7 Multi-axis oscillation (Aşama C, F5 — RQ8 adayı)

**Tek makaleye sığdırma tuzağı:** Trajectory + Navigator + döngü + maliyet hepsini
bir paper'a sıkıştırmak → hakemler "çok iddialı, deneysel doğrulama yok" der.
Paper 1 statik uzayı oturtur, Paper 2 onun üzerine inşa eder.

---

## 12. Karar Günlüğü

| Tarih | Karar | Gerekçe |
|---|---|---|
| 2026-06-30 | İki makaleye bölme | Paper 1 kanıtlanmış, Paper 2 teorik. Bölünme reddi önler. |
| 2026-06-30 | Hibrit model (predicate + coordinate) | Epistemolojik güven + matematiksel güç birlikte. |
| 2026-06-30 | Task = measurement predicate | INV-T3 (engine ölçer) korunur. |
| 2026-06-30 | Genesis = Seçenek A (operator) | INV-T2 (operator tanımlar) korunur. |
| 2026-06-30 | Paper en son, kod önce | Kanıtsız makale zaman kaybı. Data-driven yazım. |
| 2026-06-30 | 3D viewer durduruldu | Görsel geliştirme, agent işleri öncelik. |
| 2026-06-30 | 6 iyileştirme (3 review) — v0.2 | "Task=vektör"→predicate, AgentTaskView ayrımı, TaskAttempt/Ledger (B2), TargetRegion (F1), Maneuver Limit (F6), INV-T6/T7, OpKind, failures.md. |
| 2026-06-30 | INV-T6 (failure ≠ regression) | review 1 — completion vs directional improvement ayrımı. |
| 2026-06-30 | INV-T7 (maneuver limit) | review 3 — sonsuz döngü/token patlaması önleme, God Mode devri. |
| 2026-06-30 | F1 ÇÖZÜLDÜ — TargetRegion | review 1 — tek nokta değil bölge + preferred_vector. |
| 2026-06-30 | F5 Axis Oscillation | review 3 — multi-axis kayıp fonksiyonu, Aşama C. |
| 2026-06-30 | F6 Maneuver Limit | review 3 — INV-T7 ile çözüldü. |
| 2026-06-30 | invariant-spec.md önceliği | review 1 — kod yazmadan formal invariant spec. |

### Review kaynakları (v0.2 iyileştirmeleri)
- **Review 1 (teknik):** AgentTaskView/InternalTaskPlan ayrımı, TaskAttempt/Ledger, PredicateGateResult, TargetRegion, INV-T6, failures.md, B2 aşaması, "task=vektör" düzeltme.
- **Review 2 (stratejik):** OpKind enum, multi-agent Paper 3 kapısı.
- **Review 3 (risk):** Maneuver Limit (INV-T7), Axis Oscillation (F5), 3D durdurma teyidi.

---

## 13. Sonraki Adım

Bu doküman **review** için hazır. Rafinasyon sonrası:
1. `docs/paper2-notes/` dizin yapısını kur
2. Aşama A (ontolojik tipler) implementasyonuna başla
3. Her aşamada paper2-notes güncelle

---

*Bu doküman beyin fırtınası (3 yorum + kullanıcı değerlendirmesi) sonucu ortaya çıktı.
Kaynak: Yorum 1 (Vision Path/Trajectory), Yorum 2 (t_f/t_m/t_c uyumu), Yorum 3
(fizik metaforu, Sınır Değer Problemi). Hibrit predicate model: kullanıcı kararı.*
