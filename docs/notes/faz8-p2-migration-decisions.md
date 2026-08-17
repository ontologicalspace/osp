# Faz 8-P2 Migration Decisions

**Status:** Accepted (Faz 5 closure — semantic decisions; implementation pending)
**Decision date:** 2026-07-29
**Evidence baseline:** PR #85 (P2-0A+B characterization), PR #91 (#87-A policy fixture),
PR #93 (#87-B Q5 exact theta), Issue #92 (subject-authority → raw → Q5 follow-up)

Bu belge Faz 8-P2 V1/V2 measurement semantic divergence characterization'ının üç ontolojik
migration boyutu için **canonical karar kaydıdır**. Tartışma günlüğü değil; normatif kararın
kaynağıdır. Her karar bağımsız kimlik taşır (MD-1/MD-2/MD-3); biri değiştiğünde diğerleri
otomatik değişmiş sayılmaz.

**Authority sırası (kanıt → karar → norm → uygulama):**
```
characterization tests / PR'lar (evidence)
        ↓
migration decision note (bu belge — karar)
        ↓
spec / invariants.md (norm — sistem ne yapmak zorunda)
        ↓
implementation issues + PR'lar (uygulama)
```

Issue'lar kararın kanıt ve uygulama geçmişidir, kararın kendisi değildir.

## Karar Özeti

| Karar | Normatif hedef | Compatibility (geçici) | Cutover |
|---|---|---|---|
| **MD-1** Subject | task scope authority | Yol 1 compat producer (affected_nodes) | Faz 8a caller cutover |
| **MD-2** Provenance | engine-native per-axis | V1 uniform Scip projection | engine-internal, Faz 8a öncesi |
| **MD-3** Baseline Policy | typed Unavailable + fail-closed | V1 DefaultFallback (legacy) | policy implementation |

Deployment sırası: Normatif karar sırası MD-1 → MD-2 → MD-3'tür (nedensel bağımlılık);
deployment sırası seri DEĞİLDİR. Her karar aşağıdaki "Migration ordering" dependency DAG
içinde kendi evidence gate'i üzerinden bağımsız ilerler (MD-2 engine-internal — MD-1 caller
cutover'ı beklemeden; MD-3 bağımsız).

---

## MD-1 — Subject Authority

### Context

Task-bound değerlendirmede ölçüm subject'ini (hangi node kümesi ölçülüyor) hangi authority
belirler?

- **V1:** `proposal.affected_nodes` (LLM-declared) — navigator/MCP caller'ın sunduğu küme.
  Mirror: `AgentNavigator::run_task` legacy affected_nodes producer; MCP claim-evaluation
  affected_nodes producer (`proposal.affected_nodes.clone()`).
- **V2:** `task.predicate.scope` (task-derived) — task declaration'ının canonical scope'u.

### Observed evidence (PR #85, KANITLANDI)

- **Case 2** (`wide-affected-scope-001`): V1 affected={1,2,3}, V2 scope={1} → coupling
  **0.166 → 0.5 (3x)**, instability 0.5 → 1.0.
- **Case 3** (`removed-edge-external-source-001`): V1 affected={1,9} (`removed_edges.from`
  eklenir), V2 scope={1} → coupling 0.25 vs 0.5.
- **Divergence 2/4** frozen corpus case'inde.
- **Q5 theta downstream etkisi:** Subject-authority divergent topology'de raw farklı → theta
  parity varsayılamaz (Issue #92, production cutover evidence gate).

### Reviewer ontolojik çerçevesi (kabul)

```
task.predicate.scope  → measurement subject authority
structural delta      → impact authority
affected_nodes        → agent-declared impact hint / telemetry
```

Subject ve impact ayrı tutulur. Bir delta `Node(1)` hedeflerken Node 9'u yapısal olarak
etkileyebilir; bu meşru — affected her zaman scope'tan geniş olabilir.

### Decision

**Normatif hedef:** Task-bound measurement subject authority = canonical task predicate
scope. Caller-declared `affected_nodes` measurement authority DEĞİL; impact hint / telemetry
/ compatibility observation olarak.

**Uygulama geçişi (P2-1, additive):** Yol 1 compatibility producer — caller davranışı
değişmez. Canonical task-derived subject üretimi + compatibility gözlemi additive eklenir.
**Status açık:** Yol 1 = compatibility projection, Yol 2 = normative authority.

**Caller authority cutover (Faz 8a):** Gerçek cutover + Case 2/3 expected semantic-change
regolden (tarihsel bağ korunarak — sessiz overwrite değil) + compatibility yüzeyi kaldırma
+ downstream Q5/predicate güncellemeleri.

### Normative rule

> Task-bound measurement subject authority canonical task predicate scope'tur. Caller-declared
> `affected_nodes` authority değildir; yalnız impact hint veya compatibility observation olarak
> kullanılabilir.
>
> Canonical task-derived measurement subject, `derive_task_subject_scope(task)` ile elde edilir:
> `Task.target_predicate_set.predicates[*].predicate.scope` değerlerinin ayrı ayrı
> `CanonicalSubjectScope` olarak çözülmesi. Bütün predicate scope'ları aynı canonical member
> set'e çözülmelidir; heterojen scope'lar `MeasurementError::HeterogeneousPredicateScopes`
> typed error ile fail-closed reddedilir. `Node(id)` singleton scope, `Subgraph(ids)` canonicalize
> edilir; `Module(name)` ancak canonical module resolver mevcutsa çözülür, aksi durumda
> `SubjectScopeResolutionError::ModuleResolutionUnavailable` üretir.

### Rejected alternatives

- **Yol 3 (`affected == scope` invariant):** Ontolojik olarak yanlış — subject/impact
  ayrımını çökertir. Bir delta Node(1) hedeflerken Node 9'u yapısal olarak etkileyebilir;
  bu meşru. `affected == scope` zorunluluğu bu gerçeği inkâr eder. Ayrıca LLM output contract
  değişir + backward-compat kırılır.
- **Doğrudan Yol 2 (P2-1'de cutover):** Semantic closure (Faz 5) ile orchestration migration
  (Faz 8a) aynı anda — review edilebilirliği düşürür, rollback zor, causal attribution
  (drift'in subject/aggregate-source/exact-enforcement'tan mı geldiği) bulanıklaşır.

### Compatibility consequence

- PR #84 "Faz 5 closure / Faz 8a cutover" sınırı **korunur** (9/10 kritik).
- P2-1 additive: iki subject üretimi (canonical + compat shadow) aynı anda observable.
- Case 2/3 divergence frozen evidence olarak korunur; Faz 8a'da "expected semantic change"
  olarak regolden (eski golden tarihsel bağ ile).

### Required implementation

- Canonical task-derived subject üretimi (`measure_task_delta` zaten task scope kullanır).
- Compatibility producer (legacy affected_nodes ölçen, shadow observation).
- SubjectAuthorityDriftObservation (P2-1 additive).
- Issue #92: Case 2/3 tam decision drift matrisi (subject→raw→theta→Q5→predicate→decision). **TAMAMLANDI** — role-bearing 002 variants (intervention purity: raw(002)==raw(001) bit-exact) + production V1 subject helper (dual pinning). Sonuç: subject/raw Divergent, theta Divergent (θV1≠θV2, same context), karar yüzeyi NoDrift (Passed/Passed, Completed parity) — bkz. `faz8-p2-parity-characterization.md` #92 section.

### Cutover acceptance criteria (Faz 8a gate, Issue #92 kanıtı sonrası)

- Tüm driftler açıklanabilir ve subject-set farkına bağlanabilir.
- Sessiz veya tesadüfi context drift yok.
- Q5/predicate/policy değişiklikleri sınıflandırılmış (drift türü: NoDrift /
  SourceLabelOnly / PredicateResultDrift / PolicyDecisionDrift / MutationDecisionDrift).
- Eski ve yeni golden'lar tarihsel korunmuş.
- Caller cutover sonrası yalnız Yol 2 (task scope) mutation authority.

---

## MD-2 — Provenance Authority

### Context

Predicate source şartı (INV-T4 `required_source`) aggregate label üzerinden mi, per-axis
provenance üzerinden mi uygulanır? Ölçülen eksen değerlerinin hangi kaynakları karar vermeye
yetkilidir?

- **V1:** `provenanced_from_raw(..., Scip)` — tüm axis'lere **uniform Scip**. OSP epistemik
  ilkesine aykırı ("bilinmeyeni Scip etiketleme" — source laundering).
- **V2:** Engine-native per-axis source — source engine axis implementation'ından gelir ve
  per-axis korunur (PR #85 characterization coordinate system'inde gözlenen: coupling/instability
  TreeSitter, cohesion Placeholder, entropy/witness_depth Heuristic — fixture-scoped örnek,
  normative "source değerleri bunlardır" DEĞİL).

### Observed evidence (PR #85 + INV-T4, KANITLANDI)

- **required_source matrix (2/5 PredicateSet decision divergence):** `required_source =
  Some(Scip)` → V1 `Completed` (uniform Scip), V2 `SourceInsufficient` (engine TreeSitter).
  `Some(TreeSitter)` → V1 `SourceInsufficient`, V2 `Completed`.
- **Subject-authority'den BAĞIMSIZ** — matching scope'ta bile source divergence (Case 1).
- **INV-T4 (normatif, spec'lenmiş):** Explicit `required_source` şartı bulunan predicate,
  eşleşmeyen source ile task'ı tamamlayamaz (Placeholder/Heuristic/Mixed exact authority
  şartını karşılayamaz). `required_source=None` ise source authority constraint yoktur.
  `ProvenancedRawPosition` type-level enforce.

### Decision

**Normatif hedef:** Engine-native per-axis provenance = normatif authority. V1 uniform-source
projection normatif evidence DEĞİL; yalnız açıkça versioned compatibility observation olarak
geçici.

**Mixed decision matrix:**
- **Axis'ler arası heterojenlik** (coupling=TreeSitter, cohesion=Heuristic) → güvenilmez
  DEĞİL; per-axis değerlendirme (predicate kendi axis'ini kontrol eder).
- **Hedef axis `Mixed` + `Exact(X)`** → `SourceInsufficient` (fail-closed — "içinde X olabilir"
  yetmez; `Mixed` içeriksiz, contribution oranını taşımaz).
- **`Mixed` + `None`** → numeric değerlendirme devam; `Mixed` evidence içinde korunur.
- **Başka axis Mixed** → predicate etkilenmez (coupling predicate ≠ whole-vector provenance
  predicate).

**Cutover (engine-internal, Faz 8a öncesi):** Subject-authority caller cutover'dan bağımsız.
Authority cutover Faz 8a öncesi mümkün; compatibility code removal Faz 8a temizlik.

### Normative rule

> Predicate source authority, engine-native per-axis provenance'dır. Exact source gereksinimi,
> değerlendirilen eksenin kendi provenance'ına uygulanır. Aggregate veya synthetic source
> etiketi, per-axis evidence'ın yerine geçemez.
>
> **`required_source` davranış matrisi (Exact(X) ≡ `required_source = Some(X)` shorthand):**
> - `required_source = None` → source authority constraint yok; tüm `MetricSource` değerleri
>   (Placeholder/Heuristic/Mixed dahil) numeric evaluation'a girebilir. Gerçek provenance
>   evidence içinde aynen korunur (laundering yok).
> - `required_source = Some(X)` → target axis source tam olarak X olmalı. `Mixed` hiçbir
>   `Some(X)`'i karşılamaz → `SourceInsufficient` (fail-closed).
>
> `Exact(X)` bu dokümanda `required_source = Some(X)` için kavramsal shorthand'tir; production
> type modelinde ayrı `Exact` requirement enum'u YOK.

### Rejected alternatives

- **Per-axis gradual (coupling→native, cohesion→V1):** Aynı predicate değerlendirmesinde iki
  authority modelinin karışması yeni ara semantik üretir. Kademelilik shadow/authoritative
  evaluator seviyesinde olmalı, eksen bazında değil.
- **MD-1 ile birleşik cutover (Faz 8a):** Subject + provenance aynı anda — causal attribution
  zorlaşır (drift subject-set'tan mı, aggregate source'tan mı, exact enforcement'tan mı).

### Compatibility consequence

- V1 uniform Scip projection geçici telemetry/compatibility report/migration diagnostics
  için korunur; **karar üretmez** (authority değil).
- `Mixed` davranışı mevcut `MetricSource::Mixed` (içeriksiz) ile; gelecekte `Mixed(BTreeSet)`
  (#78) richer policy mümkün.

### Required implementation

1. **#88 (mixed_per_axis_sources matrisi)** — IMPLEMENTED IN PR (merge pending). 6-case
   MixedPerAxisSources frozen evidence contract: cohesion axis, Subgraph[1,2] scope. V2
   measured cohesion = Mixed (aggregate [Scip, Placeholder]); V1 legacy projected = Scip.
   5 predicate evaluation case (None/Scip/TreeSitter/Heuristic/Placeholder — Mixed fail-closed)
   + 1 declaration-validation case (required_source=Mixed → InvalidRequiredMetricSource commit
   reject). Mixed gerçek cohesion aggregation hattından doğar. Ayrıca DirectPerAxisAuthority
   family (5 case, Coupling/Node(1)) pozitif-matching kontrolü (PR #85 inline matrix frozen
   corpus'a taşındı). Measured-subject digest V1 — tek canonicalization authority (SpaceDigest,
   PredicateAxisTag, CanonicalPredicateScope, CanonicalSubjectScope, CanonicalStructuralDelta).
   Normative evidence mainline after merge.
2. **Dual evaluation** (shadow comparison): compat (V1 uniform) vs native (V2 per-axis),
   ProvenanceAuthorityDriftObservation. Candidate yol mutation authority değil.
3. **Drift sınıflandırması:** NoDrift / SourceLabelOnly / PredicateResultDrift /
   PolicyDecisionDrift / MutationDecisionDrift / UnexpectedContextDrift.
4. **Native provenance authoritative** (#88 + dual-evaluation kanıtı sonrası).
5. **Compatibility kaldırma** (Faz 8a temizlik).

### Gelecekte `Mixed(BTreeSet)` (#78)

- `Exact(X)` yalnız `source == X` veya `constituent set == {X}` iken geçmeli.
- `Mixed({TreeSitter, Heuristic})` → `Exact(TreeSitter)` karşılamaz.
- Yeni source requirement türleri düşünülebilir: `Contains(X)` / `AllOf({X,Y})` /
  `AtLeastAuthorityLevel(...)` — ama bu ayrı bir karardır.

### #88 evidence-schema notu (duplicate rejection)

Duplicate proposal elements are rejected by the `MEASUREMENT_SUBJECT:V1` evidence schema to
preserve one canonical representation. This characterization does not assert that every
equivalent raw `DeltaProposal` is currently rejected by the production proposal ingress.
Production `CanonicalStructuralDelta::try_new` kullanıldığı için node/edge structural duplicate
ve cross-list conflict production semantiğiyle reddedilir; `affected_nodes` duplicate-reject
`CanonicalSubjectScope::try_new` üzerinden (test değil, production).

---

## MD-3 — Baseline Availability Policy

### Context

Typed unavailable baseline karar hattında neye dönüşür? Baseline mevcut değilken sistem hangi
normatif davranışı göstermeli?

- **V1:** `DefaultFallback` (delta-introduced subject yokluk → `RawPosition::default()` sıfır
  koordinat) → improvement hesaplanabilir (loss_before > loss_after).
- **V2:** Typed `Unavailable { reason }` — `AllMembersIntroducedByDelta` / `PartialNewSubject`.

### Observed evidence (PR #91, KANITLANDI)

- **Policy/decision divergence:** `delta-introduced-subject-policy-001` — V1 DefaultFallback
  → `AcceptAsProgress` (TrajectoryCheckpoint, Held); V2 fail-closed → `Reject` (NotApplied,
  Evaluated). Counter-fixture (`min_improvement_delta=1.0`): V1 de Reject — improvement
  kaynaklı kanıt.
- **`AllMembersIntroducedByDelta` ≠ `PartialNewSubject`** — aynı ontolojik durum DEĞİL.

### Decision

**Normatif kurallar:**
1. Baseline availability reason typed + normatif evidence; synthetic numeric baseline'a
   dönüştürülemez.
2. **`AcceptAsProgress` yalnız `Available` baseline ile kanıtlanabilir** — Unavailable altında
   ASLA (invariant).
3. **`PredicateSetResult::Completed` baseline availability reason'dan bağımsızdır.**
   `Available`, `AllMembersIntroducedByDelta` ve `PartialNewSubject` altında
   `AcceptAsCompleted` üretir — improvement iddiası taşımaz; after-state doğrudan task
   predicate'ini karşılar.
4. **`AllMembersIntroducedByDelta`** = typed cold-start; default fail-closed `Reject`; typed
   `ColdStartPolicy` opt-in.
5. Opt-in improvement/`AcceptAsProgress` üretmez → `Suspended(ColdStartAuthorizationRequired)`
   → operator approves → `AcceptAsColdStart` (yeni karar sınıfı, Sandbox).
6. **`NotCompleted` + `PartialNewSubject`** ilk sürümde terminal `Reject` üretir (no mutation).
   Before/after subject identity'si karşılaştırılabilir değil (centroid üyelik kümesi
   değişti); cold-start override paylaşılmaz. `BaselineUnavailableReason::PartialNewSubject`
   evidence içinde korunur. Gelecekte ayrı karar ile `PartialBaselinePolicy::RequireRebaselining`
   → `Suspended(PartialBaselineRebaselineRequired)` eklenebilir (suspension reason + resume
   authority + beklenecek evidence + maneuver budget davranışı + intended apply target
   tanımlanarak). Bu PR'da `Suspended` seçilseydi bu beş alanın tanımlanması gerekirdi;
   ilk sürüm `Reject` bu belirsizliği kapatır.

**Reason-aware policy matrisi (ilk sürüm, dar):**

| Predicate | Baseline | Policy | Sonuç |
|---|---|---|---|
| `Completed` | `Available` | herhangi | `AcceptAsCompleted` |
| `Completed` | `AllMembersIntroduced` | herhangi | `AcceptAsCompleted` (improvement iddiası yok) |
| `Completed` | `PartialNewSubject` | herhangi | `AcceptAsCompleted` (completion baseline'dan bağımsız) |
| `NotCompleted` | `Available` | `StrictReject` | `Reject` |
| `NotCompleted` | `Available` | `AcceptImprovement` | Gerçek improvement hesabı |
| `NotCompleted` | `AllMembersIntroduced` | default (Disallow) | fail-closed `Reject` |
| `NotCompleted` | `AllMembersIntroduced` | `RequireOperatorApproval` | `Suspended(ColdStartAuthorizationRequired)` → `AcceptAsColdStart` |
| `NotCompleted` | `PartialNewSubject` | herhangi | terminal `Reject` (cold-start override yok; PartialBaselinePolicy gelecekte ayrı) |
| herhangi | Measurement error | herhangi | typed error (policy değil) |
| herhangi | Source insufficient | herhangi | provenance sonucu (MD-2, baseline policy değil) |

Future reason'lar için uydurma davranış yok: `unknown/unhandled reason → fail-closed`.

### `AcceptAsColdStart` — yeni karar sınıfı

- `AcceptAsColdStart ≠ AcceptAsProgress ≠ AcceptAsCompleted`.
- `AcceptAsColdStart → ApplyTarget::Lane(CommitLane::Sandbox)` (INV-T8).
- **Neden Sandbox, TrajectoryCheckpoint değil:** TrajectoryCheckpoint "ölçülmüş ilerleme" için
  ayrılmış; cold-start improvement kanıtlanamaz. Sandbox "operator-authorized isolated
  application" semantiği.
- **Lifecycle iki aşamalı:**
  - Onay öncesi: `Suspended(ColdStartAuthorizationRequired)` → mutation uygulanmaz (INV-T9),
    maneuver budget tüketilmez, agent retry başlatılmaz.
  - Onay sonrası: `AcceptAsColdStart → Sandbox` apply.
- **Mainline'a promote edilmez** — sonraki engine measurement altında normal
  `AcceptAsCompleted` gerekir.

### Normative rule (spec'e INV-T6/T8/T9 extension olarak)

- **INV-T6 extension:** `MeasurementBaseline::Unavailable` → improvement assessment
  yapılamaz → `AcceptAsProgress` üretilemez. Typed unavailable baseline hiçbir compatibility
  projection ile synthetic numeric baseline'a çevrilerek progress kanıtı oluşturamaz.
- **INV-T8 extension:** `AcceptAsColdStart → ApplyTarget::Lane(CommitLane::Sandbox)`.
  Negatif: `AcceptAsColdStart ↛ Mainline`, `↛ TrajectoryCheckpoint`.
- **INV-T9 extension:** `AllMembersIntroducedByDelta` + `ColdStartPolicy::RequireOperatorApproval`
  → `Suspended(ColdStartAuthorizationRequired)` → mutation uygulanmaz.

### Rejected alternatives

- **Tek global `Reject` (tüm Unavailable):** Güvenli ama anlam kaybettirir — "kanıtlanmış
  başarısızlık" ile "karar vermeye yetecek bilginin bulunmaması" aynı outcome'a çöker.
  `AllMembersIntroducedByDelta` cold-start'ı `Reject` ile eşlemek ontolojik bilgi kaybettirir.
- **Yeni `INV-M3` invariant:** MD-3 davranışı zaten INV-T6 (improvement epistemolojisi) +
  INV-T8 (isolation mapping) + INV-T9 (authorization suspension) kesişiminde. Yeni invariant
  dört yerde tekrar + drift riski.
- **`AcceptAsColdStart → TrajectoryCheckpoint`:** Lane ontolojisini genişletir —
  TrajectoryCheckpoint hem kanıtlanmış progress hem progress olduğu bilinmeyen operator
  istisnası olur. INV-T8 temiz ayrımı zayıflar.
- **`Allow` cold-start policy varyantı:** Yeni node ekleyen her task'ın karşılaştırmasız
  progress üretmesine dönüşür. Boolean `allow_cold_start` belirsiz.

### Type model (planned)

```rust
enum ColdStartPolicy {
    Disallow,              // default — fail-closed Reject
    RequireOperatorApproval,
    // AllowAutomatically — ŞİMDİLİK YOK (karşılaştırmasız progress riski)
}

enum MutationDecision {
    Reject,
    AcceptAsProgress,
    AcceptAsCompleted,
    RequireOperatorApproval,
    AcceptAsColdStart,  // MD-3 — improvement/completion iddiası taşımaz
}
```

`PartialNewSubject` için gelecekte ayrı `PartialBaselinePolicy` enum (Suspend/RequireRebaselining).

### Required implementation (spec planned, implementation ayrı)

- `ColdStartPolicy` typed enum + default Disallow.
- `AcceptAsColdStart` append-only canonical tag + serde backward compat.
- `ApplyTarget = Sandbox`; unavailable baseline altında `AcceptAsProgress` imkansız.
- Operator approval öncesi no mutation (INV-T9 Suspended).
- Authorization evidence/replay parity + `ColdStartAcceptanceEvidence`.
- Exact matrix tests.

---

## Cross-decision invariants

Üç kararın birlikte tutarlılığı:

1. **Üçü de "normative hedef şimdi + compatibility geçici" pattern'i** — MD-1 (Yol 2 normatif,
   Yol 1 compat), MD-2 (engine-native normatif, uniform Scip compat), MD-3 (typed Unavailable
   normatif, DefaultFallback legacy).
2. **INV-T4/INV-T8/INV-T9 spec'leriyle uyumlu** — tüm kararlar mevcut invariant'ları
   güçlendiriyor, zayıflatmıyor. MD-3 yeni INV-M3 AÇMAZ, mevcut üç invariant'a planned
   extension ekler.
3. **`AcceptAsProgress` semantic korunuyor** — MD-3 bunu yalnız Available baseline ile
   sınırlayarak INV-T8 (progress checkpoint) anlamını koruyor.
4. **Fail-closed default** — MD-3 default Reject, MD-2 Mixed exact fail-closed, MD-1 subject
   authority task-derived (caller'a güvenmiyor).
5. **MD-1 ↔ MD-3 tutarlılık:** AllMembersIntroducedByDelta zaten task scope üyeleri
   delta-introduced. MD-1 (task authority subject'i belirler) + MD-3 (cold-start bu subject'in
   özelliği) tutarlı.
6. **MD-2 ↔ MD-3 ayrım:** SourceInsufficient baseline policy DEĞİL (MD-3 matrisinde
   "provenance sonucu"). Doğru ayrım.

## Migration ordering

Üç karar **nedensel bağımlılık sırasında** karar verildi (MD-1 subject → MD-2 provenance → MD-3
baseline policy), ama **deployment bağımsız yollar** izleyebilir. Seri liste yerine dependency DAG:

```
PR #98 Accepted (BU BELGE — MD-1/MD-2/MD-3 normative decisions)
│
├─ MD-1 (Subject Authority):
│  P2-1 additive subject observation (#95-A)
│  → #92 drift characterization (cutover evidence gate)
│  → Faz 8a caller cutover (#95-B)
│
├─ MD-2 (Provenance Authority):  [MD-1'den bağımsız, engine-internal]
│  #88 frozen mixed-source matrix (evidence gate)
│  → #96 dual evaluation (shadow comparison)
│  → engine-internal authority cutover (Faz 8a ÖNCESİ)
│
└─ MD-3 (Baseline Policy):  [MD-1/MD-2'den bağımsız]
   #97 policy implementation (AcceptAsColdStart + ColdStartPolicy)
   → bağımsız ilerleyebilir
   → cold-start opt-in kullanılmadan önce tamamlanmalı

Faz 8a compatibility cleanup
→ ilgili authority cutover'ları tamamlandıktan sonra
```

**Bağımsızlık:** MD-2 engine-internal (caller'a görünmez) — MD-1 caller cutover'ı beklemeden
authority cutover yapabilir. MD-3 policy implementation MD-1/MD-2'den bağımsız. #92 (MD-1 gate)
MD-2'nin önkoşulu DEĞİL — seri liste bunu yanlış bağlıyordu.

## Spec changes

Bu decision note kabul edildikten sonra (aynı PR veya hemen sonrası):

- **INV-T6 planned extension:** unavailable baseline → `AcceptAsProgress` imkansız.
- **INV-T8 planned extension:** `AcceptAsColdStart → Sandbox`; negatif Mainline/TrajectoryCheckpoint.
- **INV-T9 planned extension:** cold-start authorization öncesi `Suspended`, no mutation.
- **MD-1 normative note:** subject authority = task scope (INV-T2 operator-defines-target
  bağlantısı).
- **MD-2 normative note:** provenance authority = engine-native per-axis (INV-T4 bağlantısı).

Status: `planned — MD-x accepted, implementation pending`. Production Rust koduna dokunulmaz.

## Follow-up issues

- **Subject-authority caller migration** (MD-1 implementation — Faz 8a).
- **Provenance enforcement** (MD-2 implementation — engine-internal cutover).
- **Baseline policy implementation** (MD-3 implementation — AcceptAsColdStart + ColdStartPolicy).
- **#88** mixed_per_axis_sources matrisi (MD-2 evidence gate).
- **#92** subject-authority → raw → Q5 theta downstream (MD-1 cutover gate).
- **#96** MD-2 implementation — navigator native measurement migration + singleton
  fast-path + digest parity + Completed-loop exact pin (review B-3 P1-3 versioned JSON
  envelope).

## Faz 8 test-project Completed-loop — V1 compatibility harness deferral

**Status:** Accepted (Faz 8 test-project — V1 compatibility harness, MD-2 ertelendi)
**Decision date:** 2026-08-01
**Branch:** `faz8-test-project/completed-loop`

Faz 8 test-project Completed-loop harness (B-1/B-2/B-3) MD-2 **implementation**'ını
içermez. Navigator native measurement migration + singleton fast-path + digest parity
bütünüyle **#96'ya (MD-2 implementation)** taşındı.

### Keşif — digest divergence

Navigator native `measure_task_delta` migration'ı denenirken `inv_t9_72_held_production_path_exact`
(FilesystemStore reload) fail etti: `authorization basis digest mismatch`. NullStore Held
test geçti → sorun reload/verify zincirinde. Kök neden hipotezi: `TaskCommitInput` (legacy)
ile `EngineMeasurement` (native) iki authority modeli yarı yolda birleşince persistence wire
representation divergence üretiyor.

### V1 compatibility harness — ne yapar (B-1/B-2/B-3)

1. Snapshot-bound task loading (HEAD + scope binding + Node-only V1).
2. Dual snapshot-binding contracts (`--require-clean-snapshot`).
3. Per-axis provenance envelope (`analyzer_axis_specific`, native değil).
4. State-dir externalization (`--state-dir` + harness invariant).
5. Mode-matrix guard + core delegation (`Task::validate()` façade).
6. Exact-set node identity + path bijection.

### V1 compatibility harness — ne yapmaz (MD-2 deferral)

1. Native 5-axis engine measurement (authorization basis hâlâ legacy `TaskCommitInput`).
2. Provenance-native execution path (uniform Scip projection).
3. Versioned trajectory JSON envelope (Completed-loop çıktısı human stdout, P1-3 deferred).

### MD-2 cutover acceptance criteria (#96)

1. **Digest parity**: Native EngineMeasurement → Held → Filesystem persist → reload →
   exact authorization basis digest parity → exact 5-axis value bits → exact 5-axis source tags.
2. **Singleton centroid bit-parity**: `measured_centroid_in_session` fast-path →
   direct `measured_position_of` bit-identical.
3. **Reload verification**: `inv_t9_72_held_production_path_exact` native migration altında
   geçmeli (FilesystemStore reload).
4. **Execution provenance**: trajectory attempt native 5-axis engine measurement
   kullanır (hardcoded `provenanced_from_raw` değil).

### V1 metadata dürüstlüğü

- **Analyze envelope**: `analysis.provenance_model: analyzer_axis_specific` (gerçek
  axis-specific analyzer provenance — MD-2 native authority'den ayrı).
- **Navigator execution** (P1-3 deferred): hedef `execution_measurement.authority:
  legacy_projected_v1` — navigator'ın legacy V1 projection'ını dürüstçe beyan eder.

Test referansı: `crates/osp-cli/tests/completed_loop.rs` (20-senaryo integration matrix).
Kullanım rehberi: `docs/notes/test-project-guide.md`.

