# Faz 8-P2 — P2-0B: V1/V2 Semantic Divergence Characterization Report

**Tarih:** 2026-07-27
**Scope:** P2-0B characterization (production kodu DEĞİŞTİRMEZ).
**Plan turu:** Tur 7 (APPROVED).
**Durum:** ✅ Characterization tamamlandı — ontolojik subject authority kararı bekler.

---

## Yönetici Özeti

P2-0B, V1 (`compute_raw_from_delta` + `provenanced_from_raw`) ile V2-candidate
(`measure_task_delta` + V1 compatibility projection) arasındaki **gözlemlenebilir
semantic divergence'ı** somut olarak ölçtü. Sonuç:

> **Exact V1/V2 semantic parity KANITLANAMAZ.** İki **kanıtlanmış** ontolojik
> divergence boyutu + bir **hipotez** boyutu mevcut. P2-1 (caller migration), kanıtlanmış
> iki authority kararı verilmeden açılamaz; üçüncü (baseline policy) P2-0B.8 fixture bekliyor.

### Üç ontolojik boyut (review tur 2/4/5 birikimi)

1. **Subject authority divergence — KANITLANDI.** V1 subject = `proposal.affected_nodes`
   (LLM-declared), V2 subject = `task.predicate.scope` (task-derived). Case 2/3'te
   farklı node set → farklı centroid → farklı measured values.

2. **Provenance authority divergence (INV-T4) — KANITLANDI.** Subject authority'den
   **BAĞIMSIZ**. V1 `provenanced_from_raw(..., Scip)` tüm axis'lere uniform Scip verir;
   V2 engine gerçek axis implementation source'ları (coupling=TreeSitter). **Matching
   scope'ta BİLE** bu divergence var — `required_source` predicate'leri V1/V2 arasında
   farklı PredicateSet kararı üretir (matrix kanıtı yukarıda).

3. **Baseline-availability representation divergence — KANITLANDI; policy/decision
   divergence — HİPOTEZ (review tur 5 P0-1).** Case 4 subject/value parity KORUR ama
   baseline representation diverge eder (V1 typed değil, V2 UnavailableAllIntroduced).
   Policy/decision divergence kanıtlanmadı — Case 4 `Completed` → projection etkisiz;
   NotCompleted + improvement-sensitive fixture P2-0B.8 gerek.

**Önemli kapsam sınırlaması (review tur 5):** Gözlemlenen value/source/baseline divergence
production-reachable; ancak pipeline mutation-decision drift ölçülemedi (Cases 1/2/3 Q5
Vision NotReached, Case 4 Held ReachedButUnsurfaced). PredicateSet-level INV-T4 decision
divergence kanıtlandı (required_source matrix). Pipeline-level ölçüm P2-0B.8 bekliyor.

Bu rapor, üç ontolojik boyut için pro/con analizi sağlar ve kararı bekler.

---

## Karakterizasyon Metodolojisi

### Test altyapısı (P2-0B.1-5)

- **Frozen fixture modeli:** Builder + manifest + BLAKE3 digest hibrit. Corpus
  değişikliği digest doğrulaması ile tespit edilir (`cases.blake3` sidecar + per-case
  `builder_digest_blake3`). `.gitattributes text eol=lf` ile cross-platform raw-byte
  determinism.
- **Stage-aware observation modeli:** `CharacterizationObservation` (measurement +
  pipeline ayrımı). Erken duruşlar (syntax/bind/validate/measurement) temsil
  edilebilir. Q5 exact theta ayrı engine-unit (public API'den alınamaz).
- **V1 vs V2-candidate harness:** İki ayrı engine instance'ı (state izolasyonu),
  aynı başlangıç `SpaceDigest`. V2 "candidate projection" (production V2 consumer
  henüz yok).

### Divergence class'ları

| Class | Subject authority (V1) | Subject authority (V2) | Beklenen |
|---|---|---|---|
| `matching_scope` | affected_nodes = task scope | task predicate scope | parity |
| `wide_affected_scope` | affected_nodes ⊃ task scope | task scope (dar) | **divergence** |
| `removed_edge_external_source` | affected + removed_edges.from | task scope (harici node hariç) | divergence |
| `delta_introduced_subject` | current_measured (always) | UnavailableAllIntroduced (no prior baseline) | baseline-availability divergence |

---

## Bulgular (4 case, frozen fixture incidence)

### Case 1: `matching-single-node-001` (subject/value parity, source divergence) ⚠️

**Girdi:** Task `Node(1)` scope, proposal `affected_nodes=[1]`, node 1 space'te.

**Sonuç:** ⚠️ **Subject + value parity KORUNDU, ama source divergence var (review tur 2 P0-2).**

- Subject set: V1={1}, V2={1} — **parity** ✅
- 5-axis value bits: **parity** ✅ (aynı node üzerinden centroid)
- Q5 disposition: **parity** (ikisi de Q5 Vision'da durdu — NotReached)
- predicate completion / mutation decision / apply target / witness: **NotReached**
  (Q5 Vision'da durdu, PredicateGate çalışmadı — ölçülemedi, "parity" DEĞİL)
- **5-axis sources: DIVERGENCE** ⚠️ — V1 coupling=Scip (provenanced_from_raw override),
  V2 coupling=TreeSitter (engine axis default)

| Axis | V1 source | V2 source | Divergence |
|---|---|---|---|
| coupling | **Scip** | **TreeSitter** | **VAR** (INV-T4) |
| cohesion | Scip | **Placeholder** | **VAR** |
| instability | Scip | **TreeSitter** | **VAR** |
| entropy | Scip | **Heuristic** | **VAR** |
| witness_depth | Scip | **Heuristic** | **VAR** |

V1=[Scip; 5], V2=[TreeSitter, Placeholder, TreeSitter, Heuristic, Heuristic] (engine
axis defaults). Exact snapshot test'lerde pinlenmiştir (review tur 4 P1-1).

**Kritik yorum (review tur 2 P0-2):** Matching scope'ta subject/value parity KORUNSA
bile, **source divergence subject authority'den BAĞIMSIZ** bir ontolojik boyut
(Migration 2 — provenance authority). V1 `provenanced_from_raw(..., Scip)` tüm axis'lere
uniform Scip verir; V2 engine gerçek axis implementation source'ları. Bu, INV-T4
`required_source` predicate'lerini etkiler — `required_source = Some(Scip)` predicate
V1'de `Completed`, V2'de `SourceInsufficient` (required_source matrix yukarıda).

**Pipeline mutation-decision (review tur 6 canonical):** matching case Q5 Vision'da
durdu (placeholder `computed_raw` → theta ihlali) → PredicateGate **NotReached**
(çalışmadı). Pipeline mutation-decision parity "ölçüldü eşit" DEĞİL — **ölçülemedi**
(NotReached).

---

### Case 2: `wide-affected-scope-001` (KNOWN production-reachable divergence) ⚠️

**Girdi:** Task `Node(1)` scope, proposal `affected_nodes=[1,2,3]`, node 1/2/3 space'te.

**Sonuç:** ⚠️ **Önemli value + source divergence.**

| Metrik | V1 (affected={1,2,3}) | V2 (subject={1}) | Divergence |
|---|---|---|---|
| Subject node count | 3 | 1 | **ontolojik** |
| Coupling value | **0.1667** | **0.5** | **3x fark** |
| Cohesion | 0.5 | 0.5 | eşit (default axis) |
| Instability | 0.5 | 1.0 | **farklı** |
| Entropy | 0.5 | 0.5 | eşit |
| Witness depth | 0.4094 | 0.4094 | eşit |
| Source provenance | uniform `Scip` (hepsi) | karışık (`TreeSitter`/`Placeholder`/`Heuristic`) | **farklı** |
| Baseline | None (current_measured) | Available | farklı tracking |
| Pipeline | StoppedBeforeCommit (Vision) | StoppedBeforeCommit (Vision) | eşit (tesadüfen — Q5 her ikisinde de fail) |

**Kritik yorum:**
- Coupling değerinde **3x fark** (0.1667 → 0.5): V1 3-node centroid, V2 1-node.
- Source provenance radikal farklı: V1 uniform `Scip` (LLM-declared), V2 per-axis
  gerçek axis implementation kaynakları (TreeSitter/Placeholder/Heuristic).
- Pipeline burada tesadüfen aynı (her ikisi de Vision gate'de fail oldu) — ama
  bu, PredicateGate'e ulaşan bir case'te **decision divergence** üretirdi.

**Ontolojik anlam:** Bu case, subject authority'nin (LLM-affected vs task-derived)
yalnızca node set'i değil, **ölçülen değeri ve provenance'ı** da değiştirdiğini
somut olarak kanıtlar.

---

### Case 3: `removed-edge-external-source-001` (affected contamination) ⚠️

**Girdi:** Task `Node(1)` scope, `affected_nodes=[1]` + `removed_edges[0].from=9`
(node 9 external).

**Sonuç:** ⚠️ **Affected set contamination divergence.**

| Metrik | V1 (affected={1,9}) | V2 (subject={1}) | Divergence |
|---|---|---|---|
| Subject node count | 2 (1 + removed.from=9) | 1 | ontolojik |
| Coupling | 0.25 | 0.5 | farklı |
| Pipeline | StoppedBeforeCommit (Vision) | StoppedBeforeCommit (Vision) | eşit (tesadüfen) |

**Yorum:** V1 navigator pattern'i `removed_edges[*].from`'u affected set'e dahil
eder; V2 task scope harici node'ları ölçmez. Bu, navigator.rs:810-815 mirror
kodu ile somutlaştı.

---

### Case 4: `delta-introduced-subject-001` (baseline representation divergence) ✅

**Girdi:** Task `Node(10000)` scope, node 10000 delta-introduced via `NewNodeSpec`
(`node_from_spec` id = 10_000 + 0).

**Sonuç (review tur 6 P0-1/P0-2 canonical):**

| Boyut | V1 | V2 | Sonuç |
|---|---|---|---|
| Subject set | {10000} | {10000} | **parity** (subject authority divergence yok) |
| Axis value bits | 1-node centroid | aynı centroid | **parity** |
| Baseline representation | None (typed değil) | **UnavailableAllIntroduced** | **representation divergence KANITLANDI** |
| Source | uniform Scip | per-axis engine defaults | INV-T4 divergence |
| Pipeline mutation-decision | **Observed(AcceptAsCompleted)** | **Observed(AcceptAsCompleted)** | **parity (ölçüldü, eşit)** |
| Apply target | Lane(Mainline) | Lane(Mainline) | parity |
| Witness | Held | Held | parity |

**Kritik (review tur 6 P0-1):** Case 4 predicate `Coupling ≤ 0.5` + measured coupling 0.0
→ `PredicateSetResult::Completed` → `MutationDecision::AcceptAsCompleted` (completion-first
decision core `improved`'a bakmaz). Held `AuthorizationContext` gerçek outcome taşır
(engine.rs:1133) → `Observed(AcceptAsCompleted)` V1/V2 parity. **Pipeline mutation-decision
ölçüldü ve eşit** — NotReached/ReachedButUnsurfaced DEĞİL (tur 5 yanlıştı).

**Baseline policy/decision divergence — HİPOTEZ (kanıtlanmadı):** Case 4 `Completed`
olduğu için `project_v1_loss_before_compatibility` (loss_before=loss_after) projection'ın
decision etkisi **observable DEĞİL**. Decision etkisi SADECE `NotCompleted` +
improvement-sensitive policy dalında olabilir — o fixture P2-0B.8 gerek.

**Özet (review tur 6 canonical):**
- Representation divergence: **kanıtlandı** (V1 typed değil vs V2 UnavailableAllIntroduced).
- Policy/decision divergence: **kanıtlanmadı** (hipotez).
- Case 4 exact mutation decision: **V1/V2 AcceptAsCompleted parity**.
- P2-0B.8: `NotCompleted` + `AcceptImprovement` policy fixture gerekli.

**Fixture notu:** task scope `Node(10000)` = delta node id (`node_from_spec` 10_000+0).
Önceki fixture `Node(1)` kullanıyordu → kimlik uyuşmazlığı → SubjectScope hatası.

---

## Divergence Özeti (frozen fixture incidence — review tur 6 canonical)

**Önemli (review tur 6 P0-1):** Pipeline mutation-decision Held için **Observed** —
`EngineCommitResult::Held` `AuthorizationContext` taşır (engine.rs:1133), gerçek
`AttemptOutcome` (predicate_completion + mutation_decision) verir. Tur 5 "Held fabrication"
yanlıştı; Held outcome observation'a taşınır (authoritative engine çıktısı).

**Frozen fixture corpus (4 case) — üç boyutta ayrı incidence + pipeline tri-state:**

| Class | Subject-set div | Axis-value div | Baseline-rep div | Source div | Pipeline mutation-decision |
|---|---:|---:|---:|---:|---|
| matching_scope | 0 (parity) | 0 (parity) | 0 (V1 None / V2 Available) | **1** (INV-T4) | NotReached (Q5 Vision) |
| wide_affected_scope | **1** | **1** | 0 (V1 None / V2 Available) | **1** | NotReached (Q5 Vision) |
| removed_edge_external_source | **1** | **1** | 0 (V1 None / V2 Available) | **1** | NotReached (Q5 Vision) |
| delta_introduced_subject | **0** (parity) | 0 (parity) | **1** (V1 None / V2 UnavailableAllIntroduced) | **1** | **Observed(AcceptAsCompleted), V1/V2 parity** |
| **Toplam (frozen fixture)** | **2/4** | **2/4** | **1/4** | **4/4** | NotReached 3/4 + Observed+Equal 1/4 |

**Kritik (review tur 4 P0-1 + tur 6):** Case 4 subject/value divergence GÖSTERMEZ —
subject authority divergence yok ({10000}=={10000}), axis value da aynı. Ayrışan tek
boyut **baseline representation**. **Case 4 pipeline mutation-decision ölçüldü ve eşit**
(Observed(AcceptAsCompleted) V1/V2 parity) — Held fabrication yanlıştı, Held gerçek
outcome taşır.

**Pipeline mutation-decision (tur 6 canonical):**
- Cases 1/2/3: Q5 Vision'da durdu → **NotReached** (PredicateGate çalışmadı).
- Case 4: Q5 passed, PredicateGate'e ulaştı, Held → **Observed(AcceptAsCompleted)**,
  V1/V2 parity ( Held `authorization.outcome` real).
- Pipeline-level decision-drift: **ölçüldü, eşit** (Case 4). Cases 1/2/3 NotReached.
- ReachedButUnsurfaced: **0/4** (Held gerçek outcome taşır).

**Inline required_source matrix (frozen corpus DIŞI ayrı yüzey, 5 case):**

| `required_source` | PredicateSet decision divergence |
|---|---:|
| `None` | 0 |
| `Some(Scip)` | **1** (V1 Completed, V2 SourceInsufficient) |
| `Some(TreeSitter)` | **1** (V1 SourceInsufficient, V2 Completed) |
| `Some(Placeholder)` | 0 |
| `Some(Heuristic)` | 0 |
| **Toplam (inline matrix)** | **2/5** |

**Önemli (review tur 6 canonical):** Pipeline mutation-decision:
- Frozen fixture Cases 1/2/3 Q5 Vision'da durdu → **NotReached 3/4** (PredicateGate çalışmadı).
- Frozen fixture Case 4 PredicateGate'e ulaştı, **Held** → **Observed(AcceptAsCompleted)
  1/4, V1/V2 parity** (Held `AuthorizationContext` gerçek outcome taşır — tur 5
  "ReachedButUnsurfaced" yanlıştı, Held fabrication çürütüldü).
- Inline required_source matrix: **2/5** PredicateSet decision divergence (pipeline
  level değil).

Pipeline-level (commit outcome) mutation-decision drift için non-default `computed_raw`
+ Evaluated outcome P2-0B.8 (Cases 1/2/3'ü PredicateGate'e ulaştırmak + Case 4'te
Evaluated witness) bekliyor.

**required_source decision matrix (review tur 3 P0-1 — gerçek `PredicateSet::evaluate_completion`):**

| `required_source` | V1 coupling=Scip | V2 coupling=TreeSitter | Decision divergence |
|---|---|---|---|
| `None` | `Completed` | `Completed` | yok |
| `Some(Scip)` | **`Completed`** | **`SourceInsufficient`** | **VAR** |
| `Some(TreeSitter)` | **`SourceInsufficient`** | **`Completed`** | **VAR** |
| `Some(Placeholder)` | `SourceInsufficient` | `SourceInsufficient` | yok |
| `Some(Heuristic)` | `SourceInsufficient` | `SourceInsufficient` | yok |

**Kritik:** Bu matrix, subject authority'den **BAĞIMSIZ** bir INV-T4 provenance-
authority decision divergence'ı gerçek `PredicateSet::evaluate_completion` ile
kanıtlar. Matching scope'ta bile `required_source = Some(Scip)` predicate V1'de
`Completed`, V2'de `SourceInsufficient` → PredicateSet decision farklı.

**Önemli ayrım (review tur 3):** Bu matrix frozen corpus DIŞI ayrı bir characterization
yüzeyidir (inline case'ler, frozen fixture'lara dahil DEĞİL). Frozen matching fixture
`required_source = None` kullanır → frozen corpus'ta matching decision parity korunur.
Inline matrix ayrı bir kanıt yüzeyi olarak değerlendirilmeli.

**⚠️ Selection bias caveat:** Bu oran fixture selection'a bağlıdır — 4 case'ten 3'ü
bilinçli olarak divergence sınıfları olarak tasarlandı. "100%" production divergence
sıklığı tahmini DEĞİLDİR; sadece "seçilen adversarial case'lerde divergence gözlemlendi"
anlamına gelir. Production divergence sıklığı bu rapordan çıkarılamaz — gerçek corpus
characterization'ı (navigator/MCP fixture'ları) P2-0B kalan iş kapsamında.

---

## Q5 vs Measurement Error Precedence (P2-0A finding)

P2-0A'da belgelenen characterization finding'ı:

> `measure_task_delta` fallible; Q5 final `claim.computed_raw` gerektirir → exact
> V1 precedence sağlanamaz.

**Review tur 3 P0-2 düzeltmesi:** Önceki rapor Case 4'te "V1 Vision'da durdu, V2
StoppedBeforeCommit{Other}" diyordu. Bu, **Case 4 fixture kimlik hatasına** (tur 2
P0-1) dayanıyordu — düzeltme sonrası V2 measurement `Produced` (UnavailableAllIntroduced),
error değil. Bu yüzden Case 4'te pipeline stage farkı KANITLANMADI.

**Review tur 6 canonical:** Cases 1/2/3 Q5 Vision gate'inde durur (placeholder
`RawPosition::default()` → theta ihlali) → PredicateGate **NotReached**. Case 4 Q5'i
geçti (coupling 0.0 ≤ vision bound), PredicateGate'e ulaştı, **Held** oldu — Held
`AuthorizationContext` gerçek outcome taşır → **Observed(AcceptAsCompleted)** V1/V2
parity (tur 6 P0-1: Held fabrication yanlıştı). Pipeline mutation-decision: **ölçüldü,
eşit** (Case 4); Cases 1/2/3 NotReached. Sadece `required_source` matrix (yukarıda)
INV-T4 PredicateSet decision divergence kanıtladı. Pipeline-level decision-drift için
non-default `computed_raw` + Evaluated outcome P2-0B.8'de (Cases 1/2/3'ü PredicateGate'e
ulaştırmak + Case 4'te Evaluated witness).

---

## Üç Ayrı Semantic Migration (review tur 4 ontolojik değerlendirme)

P2-1, **üç ontolojik boyutu** içerir — ikisi **kanıtlanmış** migration kararı, biri
**hipotez** (P2-0B.8 fixture bekliyor). Bunlar birleştirilmemeli — subject authority,
provenance authority ve baseline availability aynı ontolojik karar değildir (review tur 4
P0-1 bulgusu: Case 4 subject/value parity KORUR ama baseline representation diverge eder).
**Review tur 5 P0-1 daraltma:** Migration 3 policy/decision divergence kanıtlanmadı —
representation divergence kanıtlandı, policy etkisi hipotez.

### Migration 1: Subject Authority — KANITLANDI

V1 subject = `proposal.affected_nodes` (LLM-declared); V2 subject =
`task.predicate.scope` (task-derived). Üç yol (detay aşağıda):

1. V1 compatibility subject producer
2. Task-authoritative migration (Faz 8a atomik cutover)
3. Proposal invariant'ı (`affected_nodes == task.predicate.scope`)

### Migration 2: Provenance Authority (INV-T4) — normatif hedef, KANITLANDI

V1 `provenanced_from_raw(..., Scip)` tüm axis'lere uniform Scip verir; V2 engine gerçek
per-axis source'ları (TreeSitter/Placeholder/Heuristic). Bu bir seçenek DEĞİL; normatif
hedeftir — Paper 1/Paper 2 epistemik ilkesi "ölçüm kaynağı gerçekte neyse o
taşınmalıdır; bilinmeyen V1 SCIP olarak yeniden etiketlenmemelidir." Verilmesi gereken
karar:

> Gerçek provenance'a hangi migration sınırı ve hangi backward-compatibility
> politikasıyla geçeceğiz?

V1 uniform Scip davranışı compatibility gerçeğidir; V2 engine-native per-axis
provenance normatif hedeftir. `required_source` matrix (yukarıda) bu migration'ın
PredicateSet-level decision impact'ini kanıtladı.

### Migration 3: Baseline-Availability Representation & Policy (review tur 5 daraltma)

Case 4'ün ortaya çıkardığı üçüncü boyut. **İki cümle ayrımı kritik (review tur 5 P0-1):**

1. **"Geçmiş baseline yoktur" (representation divergence) — KANITLANDI.**
   - V1: `current_measured` her zaman var (engine space centroid) → loss_before hesaplanır;
     baseline typed olarak korunmuyor.
   - V2: `MeasurementBaseline::Unavailable { AllMembersIntroducedByDelta }` → typed
     representation; geçmiş baseline yok.

2. **"Bu yokluk farklı policy/decision kararı üretir" — HENÜZ KANITLANMADI (hipotez).**
   Test-only `project_v1_loss_before_compatibility` bu yokluğu `loss_before = loss_after`
   olarak uydurma scalar'a çevirir. **AMA** Case 4 predicate `Coupling ≤ 0.5` + measured
   coupling 0.0 → `PredicateSetResult::Completed` → `AcceptAsCompleted` (completion-first
   decision core `improved`'a bakmaz). Yani projection'ın decision etkisi Case 4'te
   **observable DEĞİL**. Decision etkisi SADECE `NotCompleted` + improvement-sensitive
   policy dalında olabilir — o fixture henüz yok (P2-0B.8).

**Bu yüzden Migration 3 "zorunlu migration kararı" olarak resmileştirilmedi.** Üç olası
yorum:
- (a) Representation divergence yeterli — typed `Unavailable` korunmalı, policy kararı
  INV-T9 normatif çerçevesinde zaten belirlenmiş olabilir (referans gerek).
- (b) Policy/decision divergence gerçek bir olasılık — `NotCompleted` + `AcceptImprovement`
  fixture ile P2-0B.8'de kanıtlanmalı.
- (c) Üçüncü migration kararı olarak resmileştirilmeli — Reject / Held-Suspended /
  RequireOperatorApproval policy ayrı karar.

Bu PR (a) ve (b) arasında bir sonuca varmaz; P2-0B.8 fixture'ı (b)'yi açıklar.

### Reviewer uzun vadeli yön önerisi

Subject authority için Yol 2 (task-authoritative):

```
task.predicate.scope  → measurement subject authority
structural delta      → impact authority
affected_nodes        → agent-declared impact hint / telemetry
```

Bu, subject ve impact kümelerini ayrı tutar (Paper 3 predicate scope binding, Paper 2
INV-T2 operator-defines-target). Yol 3 (`affected == scope`) reddedilir — subject/impact
ayrımını çökertir (bir delta `Node(1)` hedeflerken Node 9'u yapısal olarak etkileyebilir;
bu meşru).

Provenance tarafında V2 engine-native per-axis source normatif hedeftir; Paper 2 INV-T4
bunu doğrudan destekler. Baseline-availability tarafında OSP'nin epistemik çizgisi
uydurma scalar baseline yerine typed Unavailable korumayı tercih eder; sonrasında
Held/Suspended, operator escalation veya fail-closed rejection politikalarından hangisi
uygulanacağı ayrıca belirlenir.

---

## Ontolojik Subject Authority Kararı — Üç Yol (Migration 1 detayı)

Subject authority migration için üç yol. Provenance migration (yukarıda) ayrı.

### Yol 1: V1 compatibility subject producer

**Tanım:** Yeni bir producer, legacy `affected_nodes` set'ini ölçer (task scope DEĞİL).
`measure_task_delta_checked`'e subject override parametresi DEĞİL — ayrı explicit
compatibility producer.

**Pro:**
- V1 subject-set/value semantic parity korunur (Case 2/3 subject-authority divergence kapanır)
- **Not (review tur 4 P0-1):** Case 4'te subject authority divergence zaten yoktu — Yol 1
  Case 4 baseline-availability divergence'ı KAPATMAZ (V2 hâlâ UnavailableAllIntroduced).
- P2-1 caller migration güvenli açılır (subject authority için)
- navigator/MCP mevcut `affected_nodes` declaration model'ini korur

**Con:**
- V2'nin "task-authoritative subject scope" invariant'ı uygulanmaz
- Geçici compatibility producer (Faz 8a'da task-authoritative migration gerekir)
- İki paralel measurement ontology (affected vs task scope) — temporary ontolojik çoğulluk
- Generic `Option<&[NodeId]>` override reviewer tarafından reddedildi — explicit ayrı producer gerekir

### Yol 2: Task-authoritative migration (Faz 8a atomik cutover)

**Tanım:** Subject authority = task scope (semantic change açıkça kabul). P2-1 + Faz 8a
birleşik atomik cutover. V1 davranış değişir (affected_nodes geniş scope → task dar scope).

**Pro:**
- En temiz ontolojik sonuç (tek subject authority: task scope)
- Case 2/3 subject-authority divergence'ı "expected semantic change" olur (regolden değil, kabul)
- **Not (review tur 4 P0-1):** Case 4 subject authority divergence yoktur — Yol 2 Case 4
  baseline-availability divergence'ı kapatmaz (Migration 3 ayrı).
- V2 invariant'ı tam uygulanır
- Geçici compatibility producer YOK

**Con:**
- **En büyük davranış değişimi** — V1 callers (navigator/MCP) farklı measurement değerleri görür
- Case 2: coupling 0.166→0.5 değişimi (3x) production decision'ları etkileyebilir
- Corpus-wide regolden + semantic drift test güncellemesi gerekir
- PR #84'ün "Faz 5 = semantic closure, Faz 8a = orchestration cutover" sınırı bulanıklaşır
- P2-1 "additive caller migration" olmaktan çıkar, Faz 8a ile birleşir

### Yol 3: Proposal invariant'ı (`affected_nodes == task.predicate.scope`)

**Tanım:** Creation/validation seviyesinde `affected_nodes == task.predicate.scope`
zorunlu. LLM proposal'ı bu invariant'a uymak zorunda (validation reject).

**Pro:**
- Case 2/3 divergence'ı kökünden kapatılır (affected_nodes daraltılır)
- Subject authority tek kaynak (task scope) — V1/V2 parity matematiksel olarak sağlanır
- Ontolojik olarak en tutarlı (LLM task scope dışında affected declare edemez)

**Con:**
- **Proposal generation + validation değişikliği** — LLM output contract'ı değişir
- Navigator/MCP proposal validation akışına invariant check eklenir
- Mevcut corpus'ta `affected_nodes ⊃ task scope` case'leri reject edilir (backward-compat kırılır)
- LLM'e "sadece task scope içinde affected declare et" kısıtı — ergonomi etkisi
- En büyük scope (proposal layer + validation + LLM prompting)

---

## Karar İçin Önerilen Kriterler

Ontolojik subject authority kararı verilirken:

1. **Semantic continuity:** PR #84 "Faz 5 closure" sınırı ne kadar kritik? (Yol 2 bu
   sınırı bulanıklaştırır)
2. **LLM contract ergonomisi:** `affected_nodes` LLM'e ne kadar esneklik tanıyor?
   (Yol 3 en katı)
3. **Production decision drift tolerance:** Case 2'deki 3x coupling farkı production'da
   ne sıklıkta emerge olur? (Yol 2 kabul, Yol 1/3 kapatır)
4. **Implementation scope:** Hangi yol en dar implementation scope'a sahip?
   (Yol 1 dar, Yol 3 geniş)

---

## P2-1 Implementation Readiness

P2-0A yapısal önkoşulları kanıtladı (probe→final digest parity, type alias, binding-
before-measurement). **P2-0B, yapısal önkoşulların yeterli OLMADIĞINI** somut olarak
gösterdi:

- **Yapısal:** ✅ Probe→final akış, type alias, harness çalışıyor
- **Semantic (subject authority):** ❌ Exact parity sağlanamaz (Case 2/3 subject-set divergence) — **kanıtlandı**
- **Semantic (provenance authority, INV-T4):** ❌ Matching scope'ta BİLE source divergence
  + `required_source` matrix PredicateSet decision divergence (Scip/TreeSitter) — **kanıtlandı**
- **Semantic (baseline availability):** ⚠️ Case 4 baseline **representation** divergence
  (V1 typed değil, V2 UnavailableAllIntroduced) — **kanıtlandı**. **Policy/decision**
  divergence — **hipotez** (Case 4 Completed → projection etkisiz; NotCompleted fixture
  P2-0B.8 gerek).
- **Pipeline mutation-decision (tur 6):** Case 4 **Observed(AcceptAsCompleted)** V1/V2
  parity (ölçüldü, eşit — Held fabrication çürütüldü). Cases 1/2/3 NotReached (Q5 Vision).
  Pipeline-level Evaluated-witness drift için P2-0B.8 bekliyor.
- **Ontolojik:** ⚠️ **İki kanıtlanmış + bir hipotez** migration kararı:
  1. Subject authority (Yol 1/2/3) — **kanıtlandı**
  2. Provenance authority (INV-T4 normatif hedef) — **kanıtlandı** (migration sınırı + backward-compat)
  3. Baseline-availability policy — **hipotez** (representation divergence kanıtlandı,
     policy/decision divergence P2-0B.8 fixture bekliyor — Case 4 Completed'da projection
     etkisiz, AcceptAsCompleted parity gözlendi)

**P2-1 durumu:** `BLOCKED — iki kanıtlanmış migration kararı + bir hipotez (P2-0B.8)`

**Reviewer önerisi (uzun vadeli yön):** Task-authoritative migration (Yol 2) —
`task.predicate.scope → subject authority`, `structural delta → impact authority`,
`affected_nodes → agent-declared impact hint/telemetry`. Yol 3 (affected == scope)
reddedilir çünkü subject/impact ayrımını çökertir. Provenance authority için V1
uniform Scip override'dan V2 gerçek axis source'larına geçiş normatif hedef (INV-T4).
Baseline-availability için OSP epistemik çizgisi uydurma scalar yerine typed Unavailable
korumayı tercih eder; sonrasında Held/Suspended, operator escalation veya fail-closed
rejection politikalarından hangisi uygulanacağı ayrıca belirlenir.

---

## Test Envanteri (P2-0B)

| Test | Dosya | Sonuç |
|---|---|---|
| `manifest_loads_and_sidecar_digest_matches` | faz8_p2_manifest_guard.rs | ✅ |
| `every_case_builder_digest_matches_manifest` | faz8_p2_manifest_guard.rs | ✅ |
| `path_traversal_outside_root_rejected` | faz8_p2_manifest_guard.rs | ✅ |
| `existing_fixture_loads_within_root` | faz8_p2_manifest_guard.rs | ✅ |
| `print_case_digests_for_manifest_update` | faz8_p2_manifest_bootstrap.rs | ✅ |
| `print_sidecar_digest_for_manifest_update` | faz8_p2_manifest_bootstrap.rs | ✅ |
| `matching_scope_baseline_case_shows_parity` | measurement_v1_v2_parity.rs | ✅ |
| `wide_affected_scope_shows_subject_authority_divergence` | measurement_v1_v2_parity.rs | ✅ |
| `removed_edge_external_source_shows_affected_contamination` | measurement_v1_v2_parity.rs | ✅ |
| `delta_introduced_subject_shows_baseline_representation_divergence` | measurement_v1_v2_parity.rs | ✅ (review tur 4/5/6: subject/value parity + baseline representation + Observed(AcceptAsCompleted)) |
| `required_source_matrix_shows_provenance_authority_divergence` | measurement_v1_v2_parity.rs | ✅ (review tur 3: gerçek `PredicateSet::evaluate_completion`) |

---

## Kalan P2-0B İş (sonraki oturum)

- **P2-0B.7:** Yüzey 2 — MCP `Workspace::current_measured()` vs
  `EngineMeasurement.before()` characterization (`workspace.rs #[cfg(test)]`).
- **P2-0B.8:** Q5 exact theta engine-unit characterization
  (`Q5ThetaCharacterization`) — Case 2/3 Q5 Vision'da durduğu için PredicateGate
  decision-drift ölçülemedi; non-default `computed_raw` ile Q5'i geçip PredicateGate'e
  ulaşan case'ler gerekir.
- **mixed_per_axis_sources class:** Case 2 zaten source divergence gösterdi ama
  dedicated `required_source` matrisi (Any/Exact(Scip)/Exact(Heuristic)/mixed/mismatch)
  eksik.
- **task-snapshot drift adversarial:** `resolver first read → Task A`, `resolver commit
  read → Task B` characterization.

## Implementation Notu — Canonical Serialization

P2-0B sırasında önemli bir bug keşfedildi: `serialize_case_bytes` düz
`serde_json::to_vec` kullandığında, `Space.nodes: HashMap<NodeId, Node>` iteration
order'ı Rust process-random seed'ine bağlı olduğu için **nondeterministik digest**
üretiyordu (aynı kod farklı run'larda farklı digest). Çözüm: önce `serde_json::Value`'ya
round-trip (BTreeMap-backed `Map` → sorted keys) → deterministic canonical bytes.

Bu, characterization fixture modeli için kritik bir düzeltme — frozen fixture
contract'ı deterministic serialization gerektirir. `common::serialize_case_bytes`
artık `canonical_json_bytes` kullanıyor.

---

## Sonraki Faz Kararı

**Subject authority kararı** (P2-0B raporu ile, bu plan dışında):
1. V1 compatibility subject producer
2. Task-authoritative migration (Faz 8a atomik cutover)
3. Proposal invariant'ı

**Karar sonrası:** P2-1 (caller migration, ayrı plan) → Faz 8a (engine cutover).
