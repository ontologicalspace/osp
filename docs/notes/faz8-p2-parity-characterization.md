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

> **Exact V1/V2 semantic parity KANITLANAMAZ.** Bilinen production-reachable
> subject-scope divergence mevcuttur. P2-1 (caller migration), ontolojik subject
> authority kararı verilmeden açılamaz.

Bu rapor, üç ontolojik yol için pro/con analizi sağlar ve kararı bekler.

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
| `delta_introduced_subject` | current_measured (always) | Unavailable OR SubjectScope fail | divergence |

---

## Bulgular (4 case, frozen fixture incidence)

### Case 1: `matching-single-node-001` (baseline parity) ✅

**Girdi:** Task `Node(1)` scope, proposal `affected_nodes=[1]`, node 1 space'te.

**Sonuç:** ✅ **Tüm metriklerde parity.**
- Q5 disposition: V1=Passed, V2=Passed
- Predicate completion, mutation decision, apply target, witness reachability: eşit
- Baseline: V1 None (tracking yok), V2 Available

**Yorum:** Harness çalışıyor. Matching scope'da divergence beklenmiyor ve gözlenmedi.

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

### Case 4: `delta-introduced-subject-001` (baseline semantics divergence) ⚠️

**Girdi:** Task `Node(1)` scope, node 1 delta-introduced (base space'te YOK).

**Sonuç:** ⚠️ **Baseline semantics + fallibility divergence.**

| Metrik | V1 | V2 | Divergence |
|---|---|---|---|
| Measurement | Produced (infallible compute_raw) | **Failed: SubjectScope** | **fallibility** |
| Baseline | None (current_measured always) | N/A (measurement failed) | ontolojik |
| Pipeline | StoppedBeforeCommit (Vision) | StoppedBeforeCommit (Other/TaskValidation) | farklı stage |

**Kritik yorum:**
- V1 `compute_raw_from_delta` **infallible** — delta-introduced node'lar için bile
  measurement üretir (0.0 default values).
- V2 `measure_task_delta` **fallible** — delta-introduced subject için SubjectScope
  failure (subject scope derivation base space üzerinden, node yok).
- Bu, V1 "her zaman current_measured var" ile V2 "baseline yoksa progress
  kanıtlanamaz" arasındaki ontolojik ayrımın somut kanıtı.

**Not:** Test'iniz V2'nin SubjectScope error ürettiğini gösterdi (teori: V2'nin
baseline Unavailable döndürmesi beklenirdi, ama implementation subject scope
derivation'ı önce yapıyor — bu da bir karakterizasyon bulgusu).

---

## Divergence Özeti (frozen fixture incidence)

| Class | Tested | Divergent | Decision-drift |
|---|---:|---:|---:|
| matching_scope | 1 | 0 | 0 |
| wide_affected_scope | 1 | 1 | 0 (Q5 Vision'da durdu, PredicateGate'e ulaşmadı) |
| removed_edge_external_source | 1 | 1 | 0 (Q5 Vision'da durdu) |
| delta_introduced_subject | 1 | 1 | 1 (pipeline stage farklı) |
| **Toplam** | **4** | **3** | **1** |

**Frozen fixture incidence:** 4 case'te 3 divergence (75%). Decision-drift 1/4
(delta-introduced-subject'te pipeline stage farkı). Q5 Vision gate test case'lerinin
çoğunu PredicateGate'e ulaşmadan durdurdu — gerçek decision-drift oranı daha yüksek
olabilirdi ama Q5'in placeholder `RawPosition::default()` ile fail olması buna
engel oldu (bu da ayrı bir bulgu — Q5 characterization için non-default
`computed_raw` gerekir, P2-0B.8 Q5 exact theta unit'te ele alınacak).

---

## Q5 vs Measurement Error Precedence (P2-0A finding, somutlaştı)

P2-0A'da belgelenen characterization finding'ı P2-0B'de somutlaştı:

> `measure_task_delta` fallible; Q5 final `claim.computed_raw` gerektirir → exact
> V1 precedence sağlanamaz.

Case 4'te gözlemlendi: V1 Vision gate'de durdu (`StoppedBeforeCommit{Vision}`),
V2 measurement producer hatasında durdu (`StoppedBeforeCommit{Other}`).
Aynı girdide farklı pipeline stage — exact V1 error precedence P2-1'de
sağlanamaz.

---

## Ontolojik Subject Authority Kararı — Üç Yol

P2-1'in açılması için subject authority'nin nasıl belirleneceğine karar verilmeli.

### Yol 1: V1 compatibility subject producer

**Tanım:** Yeni bir producer, legacy `affected_nodes` set'ini ölçer (task scope DEĞİL).
`measure_task_delta_checked`'e subject override parametresi DEĞİL — ayrı explicit
compatibility producer.

**Pro:**
- V1 semantic parity tam korunur (Case 2/3/4 divergence'ı kapanır)
- P2-1 caller migration güvenli açılır
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
- Case 2/3/4 divergence'ı "expected semantic change" olur (regolden değil, kabul)
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
- **Semantic:** ❌ Exact V1/V2 parity sağlanamaz (Case 2/3/4 divergence)
- **Ontolojik:** ⚠️ Subject authority kararı gerekli (Yol 1/2/3)

**P2-1 durumu:** `BLOCKED — ontological subject authority decision required`

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
| `delta_introduced_subject_shows_baseline_semantics_divergence` | measurement_v1_v2_parity.rs | ✅ |

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
