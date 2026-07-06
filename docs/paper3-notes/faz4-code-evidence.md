# Faz 4 — Code Evidence Integration Evidence

> **Tarih:** 2026-07-02
> **Durum:** ✅ Tek PR — review + commit
> **Tasarım:** [`docs/roadmap/paper3-design.md`](../roadmap/paper3-design.md) v0.2.1, §11 Faz 4, §7.2/§9 INV-C6, D15 (yeni)

## Özet

Faz 4, OSP'nin epistemik omurgasındaki en kritik ayrımı tip sistemine taşır:

```
A code-looking anchor is not an implementation claim.
ImplementedBy requires observed code evidence.

Observed code reality is evidence, not acceptance.
```

`ExpectedImplementation` (vizyon kod bekler, *bulunmayabilir*) ↔ `ImplementedBy` (kod
*mevcut*, ölçülmüş kanıt gerektirir) ayrımı artık **evidence-gated**. INV-C6 (koddan çıkarılan
niyet hipotezdir; kod metric'leri Observed'dır) type-level uygulanır.

## Stratejik ertelemeler

### KuzuDB persistence (Faz 3b/c) → ertelendi

**Gerekçe:** KuzuDB Ekim 2025'te **arşivlendi** — Kùzu Inc. Apple tarafından satın alındı,
repo 10 Ekim 2025'te arşivlendi, v0.11.3 son sürüm. Resmî duyuru: *"We will no longer be
actively supporting KuzuDB."*

PR30 (Faz 3a) persistence-safety'nin araştırma-değerli kısmını (INV-C3/C8 serde boundary,
`ConceptGraphSnapshot`, `PersistedAnchorPlanAudit`, `restore_trusted_snapshot`) zaten
**backend-bağımsız** teslim etti. `AnchorStore` trait (D7) osp-core'u backend-agnostic tuttuğu
için backend değişimi sadece tek crate'i etkiler. Gerçek graph backend successor projeler
(LadybugDB, SurrealDB, DuckPGQ) olgunlaşınca tekrar değerlendirilecek.

### osp-analyzer bridge → ertelendi

**Gerekçe:** osp-analyzer symbol-granular index üretmiyor — file-granular metric only
(`ModuleMetrics`, `NodeWitness`, import edges). Symbol ID, qualified name, line-col span,
kind-tag, nesting, call-graph yok. SCIP tier bu veriyi hesaplayıp LCOM4 için aggregate
ediyor, sonra **atıyor**. Bu yüzden gerçek bridge ertelendi; Faz 4 deterministik stub
(`InMemoryCodeEvidenceProvider`) ile **mechanism proof** sağlar.

`CodeEvidenceProvider` trait (D7-abstraction) osp-core'u analyzer-agnostic tutar — AnchorStore
pattern'ini tekrarlar. Gerçek bridge ayrı PR'da/crate'te impl edilir.

## INV-C6 modelleme kararı (D15 — provenance yorumu)

**Soru:** "Observed" yeni bir `DecisionStatus` variantı mı olmalı?

**Cevap (§9 yapısal garantiye sadık):** **Hayır.** İki lane net ayrılır:

```
DecisionStatus        = graph acceptance lane (Candidate→InReview→Accepted)
ObservedCodeEvidence  = epistemik provenance lane (MetricSource'tan)
```

"Observed code reality is evidence, not acceptance." — bir CodeEntity node'unun observed
olması operator-accepted decision anlamına gelmez; **Candidate kalır**, observed olma durumu
`ObservedCodeEvidence` içinde taşınır.

§9'un yapısal garantisi şunu söyler: structural facts (`MetricSource::TreeSitter/Scip`,
Observed) ve intent hypotheses (`DecisionStatus::Candidate`, Inferred). Simetrik olarak
"Observed" da MetricSource provenance'ın anlamıdır — ayrı enum slot değil.

**Reddedilen alternatif:** `DecisionStatus::Observed`. Geniş kullanılan enum'u büyütür,
graph acceptance lifecycle (Candidate→InReview→Accepted) ile epistemik sınıfı (Observed)
karıştırır, §9'un asimetrik (MetricSource ↔ DecisionStatus) garantisine uymaz.

## 7 onay patch'i (review'dan geldi, uygulandı)

### Patch 1 — `ObservedCodeEvidence`: private fields + public smart constructor
`pub(crate)` **kullanılmaz** — dış crate (gelecekteki osp-analyzer bridge) `new(...)` ile
geçerli evidence üretebilir. Field'lar private → struct literal ile geçersiz evidence enjekte
edilemez (trybuild `c6_observed_evidence_literal`).

### Patch 2 — `ObservedCodeMetricSource` typed enum
Genel `MetricSource` (Paper 1'de `Placeholder`/`Heuristic` içerir) yerine
`{ Scip, TreeSitter, StaticAnalyzer }`. `HumanVision`/`LlmGuess`/`InferredIntent` imkansız.

### Patch 3 — `EvidenceStrength` newtype
`ScalarSimilarity` (INV-C1) paterni — `[0,1]` range-check + `is_finite()`. NaN, ±∞, negatif,
>1 reject.

### Patch 4 — Provider: tek `CodeEvidenceError` (object-safe)
Associated `Error` yerine tek concrete error → `&dyn CodeEvidenceProvider` ile kullanılabilir;
pipeline/gate/scorer imzalarını büyütmez.

### Patch 5 — CodeEntity node status Candidate kalır; observed provenance `ObservedCodeEvidence`'da
İki lane ayrımı (D15). Gate `ImplementedBy` için node status'a değil evidence provider'a bakar.

### Patch 6 — `GraphSeed.code_entities` otomatik evidence sayılmaz
`InMemoryCodeEvidenceProvider` **sadece explicit `ObservedCodeEvidence` seed** ile beslenir.
CodeEntity node varlığı ≠ observed code evidence.

### Patch 7 — `PositionSnapshot`/`HasPosition` graph wiring → Faz 4.1
`physical_vector` şimdilik `ObservedCodeEvidence` içinde kalır. Bu PR'ın ana hedefi graph
position storage değil; *"ImplementedBy requires observed code evidence"* mekanizması.

## 5 uygulama notu (review'dan geldi, uygulandı)

### Not 1 — `EvidenceStrength::new` sağlam kontrol
`is_finite()` + `[0,1]` — NaN/inf önce yakalanır.

### Not 2 — `confidence: EvidenceStrength` (tek newtype)
İki newtype gerekmez; sonradan çak `f64`'a gevşetilmez.

### Not 3 — `ObservedCodeEvidence` Deserialize compile-fail
3. INV-C6 trybuild fixture: `c6_observed_evidence_deserialize.rs` — Serialize-only (PR30
serde boundary paterni).

### Not 4 — Gate sıralaması: explanation FIRST, evidence bypass etmez
Gate karar sırası: 1. INV-C7 explanation → 2. ImplementedBy evidence → 3. diğer kontroller.
Pozitif ImplementedBy = evidence VAR **ve** explanation VAR.

### Not 5 — Scorer `evidence_strength`, Gate `find_evidence`
Net ayrım: scorer strength skalar (weight 0.10), gate **object varlığı**. Strength yüksek
ama object yok → gate reject ("strength yüksek, provider lookup eksik" edge case).

## Teslim edilenler

### Yeni tipler (`types.rs`)
| Tip | Açıklama |
|---|---|
| `ObservedCodeMetricSource` | typed enum {Scip, TreeSitter, StaticAnalyzer} — INV-C6 filtre |
| `EvidenceStrength` | `[0,1]` newtype (is_finite + range-check) |
| `EvidenceStrengthOutOfRange` | error |
| `ObservedCodeEvidence` | private fields + `pub fn new` + Serialize-only, 5 accessor |

### Yeni modül (`code_evidence.rs`)
| Tip | Açıklama |
|---|---|
| `CodeEvidenceError` | thiserror + serde, tek concrete error |
| `CodeEvidenceProvider` | object-safe trait (`find_evidence` + `evidence_strength`) |
| `InMemoryCodeEvidenceProvider` | deterministik stub (explicit seed, builder) |

### Pipeline değişiklikleri
- **`AnchorGateContext<'a>`** — `code_evidence: Option<&'a dyn CodeEvidenceProvider>` field.
  Manual Debug impl (`dyn` Debug değil). `no_authority()` → None (backward-compat).
  `with_code_evidence(authority, provider)` yeni constructor.
- **`AnchorScorer::score()`** — 4. parametre `Option<&dyn CodeEvidenceProvider>`.
  `code_evidence_strength()` helper — code-related edge kind'lerde provider'a sorar.
- **`AnchorGate::validate_edge_kind()`** — ImplementedBy blanket-reject → **evidence-gated**.
  `find_evidence()` ile object varlığı (Not 5); explanation önce (Not 4).
- **`Extractor`** — deterministic ImplementedBy trigger: typed `CodeEntity:` ref +
  `has_implement_lemma()` ("implement eder"/"implements"/"implemented by"/"implemente edilir").

### Backward-compat
Provider `None` (default) → `code_evidence_score=0`, ImplementedBy reject. Tüm Faz 1-2
fixture'ları + mevcut testler kırılmaz.

### Testler
- `code_evidence.rs` unit: provider lookup, evidence_strength, empty backward-compat,
  GraphSeed.code_entities evidence sayılmaz, builder overwrite, EvidenceStrength range,
  accessors.
- `scorer.rs`: code_evidence_score zero-without-provider, zero-non-code-edge, from-provider.
- `gate.rs`: accept-with-evidence, evidence-but-no-explanation (Not 4), object-required (Not 5).
- `anchoring_mvp.rs`: fix_011 reject-without-provider + accept-with-provider.
- **3 trybuild compile-fail** (INV-C6): literal construct, intent-carries-physical-vector,
  deserialize. Toplam 10 type-level invariant.

### Fixture
- **`fix_011_implemented_by_with_evidence.json`** — pozitif ImplementedBy: seeded
  `CodeEntity:AuthService` + "implement eder" lemması + explicit `ObservedCodeEvidence`.

## Başarı kriterleri (review'dan, Faz 4 kapanış koşulları)

```
✅ ImplementedBy evidence yoksa reddedilir.
✅ ImplementedBy evidence varsa VE explanation varsa kabul edilebilir. (Not 4)
✅ ExpectedImplementation hâlâ beklenti/candidate düzeyindedir.
✅ CodeEntity node varlığı evidence sayılmaz. (Patch 6)
✅ ObservedCodeEvidence dışarıdan literal construct edilemez. (Patch 1 + trybuild)
✅ ObservedCodeEvidence deserialize edilemez. (Not 3 + trybuild)
✅ PhysicalCodeVector yerine ConceptualIntentVector verilirse compile-fail. (INV-C2 + INV-C6)
```

## Kapsam dışı (ertelenen)

- Gerçek osp-analyzer bridge (symbol-index genişletme dahil).
- KuzuDB persistence (Faz 3b/c) — ertelendi (arşivlendi).
- `PositionSnapshot`/`HasPosition` graph wiring (Faz 4.1).
- Concept/Rule/Task-level normalized ImplementedBy (Faz 5/6).
- Doğal dil ImplementedBy parser genişletme (Faz 5+).
- `EvidencedBy → Evidence` node üretimi.
- Embedding/LLM (Faz 7), Concept Synthesis (Faz 6), Operator console (Faz 8).
