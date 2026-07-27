# Faz 8-P2 — P2-0A: Construction Feasibility Closure

**Tarih:** 2026-07-27
**Scope:** P2-0A (construction feasibility) — production kodu DEĞİŞTİRMEZ, sadece characterization.
**Plan turu:** Tur 7 (APPROVED)
**Durum:** ✅ TAMAMLANDI

---

## Amaç

Planlanmış P2-1 caller migration'ının (navigator.rs:873 + osp-mcp/server.rs:878 → `measure_task_delta`) **yapısal önkoşullarını** kanıtlamak. P2-1'in açılması ayrıca ontolojik **subject authority kararı** gerektirir (P2-0B raporu ile).

Bu doküman, herhangi bir production kod değişikliği yapmadan çalıştırılabilen 4 characterization test'inin bulgularını belgeler.

---

## Bulgular (4 characterization test)

### 1. Probe → final Claim akışı: binding-relevant structural identity parity ✅

**Test:** `faz8_p2_probe_final_claim_preserves_measurement_and_binding_identity`
(`crates/osp-core/src/engine.rs` test modülü)

**Soru:** Probe Claim (placeholder `computed_raw`) → `measure_task_delta` → final Claim (`computed_raw = measurement.after().to_raw()`) akışı, tüm binding-relevant digest'leri korur mı?

**Yöntem:** İki-call feasibility modeli (production single-call source-contract P2-1'de pinlenecektir):
```
probe Claim (RawPosition::default()) → measure_task_delta → probe_measurement
final Claim (computed_raw = probe_measurement.after().to_raw()) → measure_task_delta → final_measurement
```

**Sonuç:** ✅ Tüm binding-relevant digest'ler ve typed field'lar **parity**:

| Alan | Probe vs Final | Kanıt |
|---|---|---|
| `EngineMeasurementDigest` | identical | `as_bytes()` eşit |
| `before()` (MeasurementBaseline) | identical | typed equality |
| `after()` (MeasuredRawPosition) | identical | typed equality |
| `request()` (MeasurementRequest) | identical | typed equality |
| `TaskClaimDigest` | identical | binding-relevant structural identity |
| `computed_raw` (projection) | **bilinçli farklı** | placeholder vs measurement-derived |

**Mimari gerekçe (kod doğrulaması):**
- `measure_task_delta` (engine.rs:2366-2541) `claim.computed_raw`/`claim.intent`'i **HİÇ okumaz** — sadece `task_id` + structural delta (`delta_nodes`, `delta_edges`, `removed_edges`).
- `TaskClaimDigest::compute` (measurement.rs:560-572) yalnızca `claim_id + task_id + claim.author + structural_delta_digest` bağlar. `computed_raw`/`intent` commitment'a dahil DEĞİL.
- `Intent.target_raw`'ın hiç production reader'ı yok (vestigial).

**Önemli terminoloji:** "Claim tamamen aynıdır" iddiası **yanlış** — sadece **binding-relevant structural identity** aynıdır; computed projection bilinçli olarak değişir (placeholder → measurement-derived). Bu distinction rapor ve plan dokümantasyonunda korunmalı.

**P2-1 implication:** Probe → measure → final Claim akışı P2-1'de uygulanabilir. Digest/evidence koruması altında. İkinci gerçeklik (compute_raw_from_delta) YOK.

---

### 2. Type alias kanıtı: `ProvenancedRawPosition = MeasuredRawPosition` ✅

**Test:** `faz8_p2_provenanced_raw_position_is_measured_raw_position_alias`

**Soru:** `measurement.after()` (`&MeasuredRawPosition`) doğrudan `TaskCommitInput.measured` alanına (`ProvenancedRawPosition`) assignable mi?

**Sonuç:** ✅ **Evet** — `ProvenancedRawPosition` `MeasuredRawPosition`'ın `pub use` re-export alias'ıdır (trajectory.rs:124-125). Compile-time ve runtime kanıtlandı.

**P2-1 implication:** Reviewer Tur 1 P1-2 endişesi (explicit per-axis conversion gerekliği) **geçersiz** — `clone()` yeterlidir. Bu, bridge helper'ı basitleştirir:

```rust
// Bridge helper (P2-1, karar sonrası):
struct LegacyCommitProjection {
    measured: ProvenancedRawPosition,
    loss_before: f64,
}
impl LegacyCommitProjection {
    fn from_measurement(measurement: &EngineMeasurement, legacy_target: RawPosition) -> Self {
        Self {
            measured: measurement.after().clone(),  // alias — clone yeterli
            loss_before: project_v1_loss_before_compatibility(measurement, &legacy_target),
        }
    }
}
```

**Uyarı (trajectory.rs:120-123 doc):** Alias `type AxisMetric = ...` formuna DEĞİŞTİRİLEMEZ — struct literal construction (`AxisMetric { value, source }`) kırılır. Bu sözleşme P2-1 sonrası da korunmalı.

---

### 3. Binding-before-measurement: mevcut producer contract'ı ✅

**Test:** `measure_task_delta_rejects_binding_mismatch_before_measurement_work`

**Soru:** Mevcut `measure_task_delta` binding verification'ı (`claim.task_id == task.id`) measurement work'tan ÖNCE mi çalışır?

**Sonuç:** ✅ **Evet** — `claim.task_id != task.id` (structural forgery) `TaskBindingMismatch` üretir, measurement work (subject scope derivation, centroid computation) başlamadan.

**Önemli kapsam notu:** Bu test **mevcut** `measure_task_delta` producer'ını pinler. Gelecekteki `measure_task_delta_checked` boundary'si (P2-1) için **"binding must precede `Task::validate_for_commit`"** contract test'i P2-1 implementation'ında eklenecektir — bu scope'ta `measure_task_delta_checked` henüz YOK.

**Sentinel stratejisi:** Test `claim.task_id != task.id` ile birlikte geçersiz task declaration verebilirdi; eğer producer binding kontrolünü measurement work'tan sonra yapsaydı başka bir hata dönebilirdi. `TaskBindingMismatch`'ın önceliği pinlendi.

---

### 4. Q5-vs-measurement error precedence: characterization finding ⚠️

**Test:** `faz8_p2_q5_vs_measurement_precedence_characterization`

**Soru:** Exact V1 pipeline error precedence (`Q4 → bind → validate → Q5 → PredicateGate → Q6 → witness`) P2-1'de korunabilir mi?

**Sonuç:** ⚠️ **Hayır — exact V1 precedence sağlanamaz.** Bu bir **characterization finding**'dir (migration requirement DEĞİL).

**Gerekçe:**
- `compute_raw_from_delta` (V1 producer) **infallible** `RawPosition` döndürür → Q5'ten önce çalışır ama hata üretmez.
- `measure_task_delta` (V2-candidate producer) **fallible** — revision mismatch, scope resolution failure, coordinate measurement error, axis drift, invalid mass, missing after subject gibi hatalar üretir.
- Q5 final `claim.computed_raw` gerektirir → measurement Q5'ten ÖNCE çalışmalı.
- Dolayısıyla birleşik durumda (Q5 failure + measurement failure): V1 `VisionViolation`, P2-candidate `MeasurementError` üretir.

**Somut örnek (test):** Module scope'lu task → `measure_task_delta` `SubjectScopeResolutionFailed` üretir. Aynı claim'in V1 yolu Q5 vision'ı çalıştırır (placeholder `computed_raw` ile) ve farklı error üretir.

**P2-1 kabul kriteri daralması (plan Tur 3+ ile uyumlu):**

| Precedence | P2-1 durumu |
|---|---|
| Q4 syntax | ✅ Korunur (`measure_task_delta_checked` Phase 0a) |
| Task binding | ✅ Korunur (`validate_task_bound_identity` shared helper) |
| Task declaration (`validate_for_commit`) | ✅ Korunur (`measure_task_delta_checked` Phase 0b+) |
| Q5 vision | ⚠️ Measurement errors Q5'ten önce oluşabilir — yeni operational boundary |
| PredicateGate / Q6 / witness | ✅ Korunur (`commit_task_claim` body değişmez) |

**V1 yolunun aynı testte çalıştırılması:** Production V2 consumer henüz olmadığı için bu scope'ta mümkün değil. P2-0B'de iki-engine izolasyon ile tam characterization yapılacaktır.

---

## P2-1 Implementation Readiness (kapsam DIŞI — karar bekler)

P2-0A, P2-1'in **yapısal** önkoşullarını kanıtladı:
- ✅ Probe → final Claim akışı digest-preserving
- ✅ Type alias — explicit conversion gerekmez
- ✅ Binding-before-measurement mevcut producer contract'ı pinli
- ✅ Q5-vs-measurement precedence characterization finding belgelendi

**Ancak P2-1 açılması için ontolojik subject authority kararı gerekir** (P2-0B raporu ile):

V1 subject (`proposal.affected_nodes`, LLM-declared) ≠ V2 subject (`task.predicate.scope`, task-derived). Aralarında invariant yok ve bu fark **production-reachable**. Üç ontolojik yol:

1. **V1 compatibility subject producer:** legacy `affected_nodes` set'ini ölçen ayrı producer
2. **Task-authoritative migration:** semantic change kabul, Faz 8a atomik cutover
3. **Proposal invariant'ı:** `affected_nodes == task.predicate.scope` creation/validation seviyesinde

---

## Test Envanteri (P2-0A)

| Test | Dosya | Sonuç |
|---|---|---|
| `faz8_p2_probe_final_claim_preserves_measurement_and_binding_identity` | engine.rs test modülü | ✅ pass |
| `faz8_p2_provenanced_raw_position_is_measured_raw_position_alias` | engine.rs test modülü | ✅ pass |
| `measure_task_delta_rejects_binding_mismatch_before_measurement_work` | engine.rs test modülü | ✅ pass |
| `faz8_p2_q5_vs_measurement_precedence_characterization` | engine.rs test modülü | ✅ pass |

---

## CI Doğrulama

```
cargo fmt --all -- --check                                                    ✅ clean
RUSTFLAGS="-D warnings" cargo build --workspace --all-targets --exclude osp-desktop  ✅ clean
RUSTFLAGS="-D warnings" cargo test -p osp-core --lib                          ✅ 1271 passed, 0 failed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test engine_measurement_single_producer  ✅ 7 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test measurement_binding_typelevel        ✅ 1 passed
```

**Production kod değişmedi → golden byte riski YOK.** Mevcut testler aynı kaldı (1267 → 1271: 4 yeni P2-0A test'i).

---

## Sonraki Adım

**P2-0B:** V1/V2 semantic divergence characterization (frozen fixtures + sidecar + `.gitattributes` LF pin + 3 test yüzeyi + stage-aware observation model). Çıktı: ontolojik subject authority kararı için kanıt raporu.

**P2-0B'den sonra:** Subject authority kararı (bu plan dışında) → P2-1 (caller migration, ayrı plan).
