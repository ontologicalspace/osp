# Faz 2 Evidence — INV-C1..C8 Type-Level Enforcement Hardening

> **Tarih:** 2026-07-02
> **Durum:** ✅ Tamamlandı — review + commit
> **Tasarım:** [`docs/roadmap/paper3-design.md`](../roadmap/paper3-design.md) v0.2.1, §11 Faz 2

## Özet

Faz 1'in **runtime** INV'leri **compile-time/type-level** garantiler yükseltildi.
"Make illegal states unrepresentable" — Rust tip sistemi epistemik güvenliği zorunlu kılar.

```
Faz 1: "invariant çalışıyor" (runtime testler)
Faz 2: "invariant ihlali imkansız" (compile-fail testler)
```

## INV-C1..C8 type-level durum

| INV | Faz 1 | Faz 2 | Mekanizma |
|---|---|---|---|
| **C1** (embedding proposes) | shape (0.0 placeholder) | ✅ type-level | `ScalarSimilarity` newtype + sealed `embedding` mod; scorer vector görmez |
| **C2** (position family separation) | enum field stub | ✅ type-level | `PositionVector` enum (3 ayrı concrete vector type); compiler karıştırmayı reddeder |
| **C3** (candidate isolation) | `OperatorAcceptance` token | ✅ + graph kapsülleme | `ConceptGraph` private fields; external `Accepted` write compile error |
| **C4** (supersede authority) | runtime reject | ✅ type-level API | `SupersedeAuthority` capability + `AnchorGateContext`; Faz 8 hazır |
| **C5** (inferred is not accepted) | runtime | runtime (kasıtlı — typestate serde kırar) | — |
| **C6** (code-derived intent) | runtime | runtime (kasıtlı — epistemik ayrım) | — |
| **C7** (high-stake explainable) | runtime `Option<String>` | ✅ + emptiness type-level | `NonEmptyExplanation` newtype; boş string public constructor tarafından `Err` ile reddedilir, oluşturulmuş değer type-level non-empty; presence runtime |
| **C8** (concept canonicalized) | runtime gate | ✅ type-level | `AnchorPlan` private fields; "AnchorPlan almak = canon gate'ten geçmiş" |

## Yeni primitifler (crate'te ilk kullanım)

- **`Sealed` trait** (`mod.rs`) — external impl engeller (closed set of implementors)
- **`NonEmptyExplanation`** newtype — private inner + fallible `new()`; boş string reject
- **`ScalarSimilarity`** newtype — `[0,1]` range-check; `zero()`/`one()`/`get()`
- **`SupersedeAuthority`** capability — 3 değer, `_private` + `pub(crate) issue_*`
- **`AnchorGateContext`** — gate authority taşıyıcı
- **Sealed `embedding` mod** — `Embedding` private inner, `cosine` private; Faz 7 placeholder
- **`PositionVector` enum** + 3 concrete vector (`PhysicalCodeVector`/`ConceptualIntentVector`/`EvidenceVector`) — typed accessor'lar

## Pratik: pub(crate) field + public read accessor

`AnchorPlan`/`AnchorCandidate`/`ConceptGraph` field'ları `pub(crate)`:
- External crate struct literal construct edemez (Rust tüm field visible gerektirir) → INV-C8 by-pass engelli
- Crate içi TCB (store/gate/pipeline) erişir
- Public read accessor'lar (`candidates()`, `decision()` vb.) external read sağlar

## Başarı kriterleri (değerlendirme 1)

```
✅ Dış crate ConceptGraph'a doğrudan Accepted yazamaz.         (INV-C3)
✅ Dış crate AnchorPlan uydurup canon gate'i bypass edemez.    (INV-C8)
✅ Boş explanation graph'a giremez.                            (INV-C7)
✅ Supersedes authority olmadan üretilemez.                    (INV-C4)
✅ Position family'leri compile-time karışamaz.                (INV-C2)
✅ Scorer embedding vector alamaz.                             (INV-C1)
✅ Audit raporu her kararı açıklayabilir.                      (audit)
```

## Compile-fail testler (`tests/compile_fail/`, trybuild)

| Case | INV | Derleme hatası |
|---|---|---|
| `c3_graph_private.rs` | C3 | `field 'nodes' of struct 'ConceptGraph' is private` |
| `c8_anchorplan_literal.rs` | C8 | `field 'packet_id' of struct 'AnchorPlan' is private` |
| `c2_family_incompatible.rs` | C2 | `expected PhysicalCodeVector, found EvidenceVector` |
| `c3_operator_acceptance_construct.rs` | C3 | `field '_private' of struct 'OperatorAcceptance' is private` |

Bu testler IHMAL edilemez: runtime testler invariant çalıştığını gösterir,
**compile-fail testler ihlalin İMKANSIZ olduğunu kanıtlar.**

## Audit raporu (INV-C7 explainability)

`AnchorPlan::to_audit_json()` + `to_audit_markdown()` — her karar explainable:
packet_id, decision, threshold, candidates (explanation dahil), redirects, negative_assertions.
Paper 3 eval metodolojisi için.

## Test kapsamı

```
~640 test yeşil (workspace, -D warnings)
├── 77 anchoring unit (Faz 1 + Faz 2 type-level)
├── 11 Faz 0 fixture format (API stabilitesi)
├── 10 Faz 1 MVP integration
├── 1 Faz 2 trybuild compile-fail (4 case)
└── ~540 Paper 1/2 + osp-core (regresyon yok)
```
Anchoring clippy-temiz. fmt-check temiz. Yeni runtime dep YOK (trybuild dev-dep).

## Faz 2 → Faz 3 geçişi

Faz 2 INV'ler compile-time garanti. Faz 3 = **Kuzu persistence** (`osp-kuzu` crate):
- `InMemoryAnchorStore` → `KuzuAnchorStore` (aynı `AnchorStore` trait, Faz 7'de)
- INV-C3/C8 type-level garanti Kuzu'da da korunur (store tek geçiş)
- C1/C2/C4/C7 type-level altyapı Faz 3+ 'da kullanıma hazır

## TCB notu

`pub(crate)` constructor'lar (OperatorAcceptance, SupersedeAuthority, AnchorPlan::from_gate)
crate içi TCB API'sı — osp-core modülleri (store/gate/pipeline) TCB içinde. External crate
(osp-cli/osp-mcp) ve integration test'leri (`tests/`) üretemez. Faz 8 operator console
gerçek API ile bu gate'leri açar.
