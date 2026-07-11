# PR D Plan — Evidence Projection + In-Process Wiring Proof (yeni oturum için)

> **Dal:** `feat/evidence-projection-wiring` (main `d7f61bc` üstünde — PR #58 merged)
> **Scope:** osp-cli only (osp-core untouched — PR C evidence model hazır)
> **2 tur plan review sonucu: implementation-ready**

## Özet

CLI metric drafts (`ProjectedCodeMetric`) → core evidence (`ObservedCodeEvidence` via
`ObservedPhysicalMetrics`) conversion. Yeni `evidence_projection.rs` modülü (draft→evidence
boundary). **Production path:** `graph init --analyze` evidence üretir + diagnostics yazar.
**Compatibility proof:** integration test evidence → `InMemoryCodeEvidenceProvider` →
`AnchorGateContext` → scorer/gate seam'in çalıştığını kanıtlar (production consumer DEĞİL).
**Persistence KAPSAM DIŞI** — ayrı milestone (PR G); PR C Serialize-only sınırı persistence için
kendi restore modelini gerektirir.

## Review tur 2 kararları (4 zorunlu düzeltme)

Plan tur 1'de iki bloklayıcı tutarsızlık vardı; tur 2'de düzeltildi:

1. **P1 — Provider production path'te kullanılmıyor:** `graph init` `AnchorPipeline`/scorer/gate
   çalıştırmıyor. Provider production path'te construct edilip drop edilir → yanıltıcı. **Çözüm:**
   production path evidence + diagnostics üretir; provider construction **integration test**'e
   taşınır (compatibility proof).
2. **P1 — Empty-node report mümkün değil:** `project_observed_evidence(metrics: &[...])` yalnız
   emit edilmiş metric'leri görür. Tüm axis'leri skip edilen node `metrics` slice'ında yok.
   `distinct_nodes` / empty-node skip / `ObservedPhysicalMetricsError::Empty` unreachable. **Çözüm:**
   report sadeleştirildi (input semantiğiyle uyumlu).
3. **P2 — Error model eksik + duplicate validation:** `EvidenceStrength::new` hatası için varyant
   yoktu; `PhysicalAxisValue::new` core constructor'da zaten tekrarlanıyordu. **Çözüm:**
   `InvalidStrength` varyantı eklendi; duplicate value validation kaldırıldı (core constructor'a bırakıldı).
4. **P2 — Review provider gap yanlış consumer:** review CLI operator transition yüzeyi; anchoring
   scorer/gate pipeline'ı çalıştırmıyor. **Çözüm:** "anchoring consumer gap" olarak yeniden adlandırıldı.

---

## Üretim + compatibility ayrımı (tur 2 net sınırı)

### Production path (`graph init --analyze`)
```
AnalysisResult → project_code_metrics → ProjectedCodeMetric[]
  → project_observed_evidence → ObservedCodeEvidence[]
  → BridgeRunOutput.evidence_projection
  → graph init diagnostics (stderr)
```
**Provider construct EDİLMEZ** — production consumer (`AnchorPipeline`) bu command'da yok.

### Compatibility proof (integration test)
```
EvidenceProjectionOutput.evidence
  → InMemoryCodeEvidenceProvider::from_evidence
  → AnchorGateContext::with_code_evidence
  → AnchorPipeline::run_with_source (scorer + gate)
```
Bu integration test seam'in çalıştığını kanıtlar; production consumer DEĞİL.

### Out-of-scope
```
evidence persistence (store write)
ObservedCodeEvidence Deserialize
store schema changes
new anchoring CLI command (packet evaluation)
review re-analysis
long-lived provider lifecycle
```

---

## osp-cli değişiklikleri

### A. Yeni `crates/osp-cli/src/evidence_projection.rs` — draft→evidence conversion boundary

`metric_projection.rs` durur (draft üretir); bu modül draft → core evidence dönüşümünün **tek
sahibi**. Source-scan guard bu ownership'i mekanik doğrular (D maddesi).

```rust
/// Conversion context — wall-clock inject edilir (temporal nondeterminism yalnız caller'da).
/// Unit testlerde sabit değer: TEST_MEASURED_AT = 1_700_000_000.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvidenceProjectionContext {
    pub(crate) measured_at: u64,
}

pub(crate) struct EvidenceProjectionOutput {
    pub(crate) evidence: Vec<ObservedCodeEvidence>,
    pub(crate) report: EvidenceProjectionReport,
}

/// Report — input yüzeyiyle uyumlu (tur 2 P1 düzeltme).
/// Conversion yalnız emit edilmiş metric'leri görür. Hiç projected metric'i olmayan analysis
/// node'ları bu boundary'nin dışında kalır ve doğal olarak provider lookup'ında bulunmaz.
pub(crate) struct EvidenceProjectionReport {
    pub(crate) input_metric_values: usize,
    pub(crate) evidence_objects_created: usize,
    pub(crate) partial_evidence_objects: usize,  // < 5 axis (try_to_physical_vector Err)
}

/// Draft metric'leri core evidence'a dönüştürür.
///
/// Input yüzeyi: `metrics` yalnız **emit edilmiş** (admitted) metric'leri içerir — Placeholder/
/// Heuristic/zero-confidence metric'ler `project_code_metrics` tarafından zaten çıkarıldı.
/// Bu nedenle her grouped node'un en az bir metric'i vardır; `ObservedPhysicalMetricsError::Empty`
/// bu conversion yolunda unreachable (defensive olarak handle edilir ama gerçekleşmez).
pub(crate) fn project_observed_evidence(
    metrics: &[ProjectedCodeMetric],
    context: EvidenceProjectionContext,
) -> Result<EvidenceProjectionOutput, EvidenceProjectionError>;
```

**Sorumluluklar:**
1. `ConceptNodeId` bazında group (deterministik sıra — sort by `node_id.0`).
2. CLI `PhysicalCodeAxis` → core `PhysicalCodeMetricAxis` map (5 variant exhaustive).
3. Newtype dönüşümü — **duplicate validation YOK** (tur 2 P2 düzeltme):
   - `metric.provenance().confidence().get()` → `EvidenceStrength::new(...)` (InvalidStrength)
   - `metric.provenance().coverage().get()` → `EvidenceCoverage::new(...)` (InvalidCoverage)
   - `metric.value().get()` → raw `f64` olarak `ObservedPhysicalMetric::new`'e geçir (core kendi
     `PhysicalAxisValue::new` validation'ını yapar; InvalidObservation altında gelir)
4. `ObservedPhysicalMetric::new(axis, value, source, strength, coverage)` çağır.
5. Her node için `ObservedPhysicalMetrics::try_new(Vec<...>)` — non-empty (input yüzeyi garantisidir)
   + unique-axis (`project_code_metrics` zaten `(ConceptNodeId, axis)` dedup yaptı) + sort_order.
6. `ObservedCodeEvidence::new(node_id, observations, context.measured_at)`.
7. Deterministik sıra.

**`measured_at` içeride üretilmez** — context'ten inject (test deterministic + replay).
`project_analysis` wall-clock okumaz; temporal nondeterminism yalnız caller'ın verdiği `measured_at`.

### Typed error model (tur 2 P2 düzeltme — InvalidStrength eklendi, InvalidAxisValue kaldırıldı)

```rust
pub(crate) enum EvidenceProjectionError {
    /// EvidenceStrength dönüşümü hatası (defensive contract-drift — projection confidence'ı
    /// zaten [0,1] doğruluyor, ama typed conversion sınırında eksiksiz olmalı).
    InvalidStrength {
        node_id: ConceptNodeId,
        axis: PhysicalCodeAxis,
        source: EvidenceStrengthOutOfRange,
    },
    InvalidCoverage {
        node_id: ConceptNodeId,
        axis: PhysicalCodeAxis,
        source: MetricScalarViolation,
    },
    /// ObservedPhysicalMetric::new hatası (InvalidValue + ZeroStrength dahil; core constructor
    /// axis/value context zaten taşıyor — InvalidAxisValue ayrı varyant YOK).
    InvalidObservation {
        node_id: ConceptNodeId,
        axis: PhysicalCodeAxis,
        source: ObservedPhysicalMetricError,
    },
    InvalidCollection {
        node_id: ConceptNodeId,
        source: ObservedPhysicalMetricsError,
    },
}
```

`analysis_bridge.rs` → `BridgeError::EvidenceProjection` sarar.

### B. `analysis_bridge.rs` — orchestrator (conversion implementation YOK)

```rust
// BridgeRunOutput + evidence_projection field
#[derive(Debug, Clone)]
pub(crate) struct BridgeRunOutput {
    pub(crate) candidate_seed: AnalysisCandidateSeed,
    pub(crate) identity_index: AnalysisProjectionIndex,
    pub(crate) graph_report: BridgeRunReport,
    pub(crate) metric_projection: AnalysisMetricProjection,
    pub(crate) evidence_projection: EvidenceProjectionOutput,  // YENİ
}

// project_analysis — temporal nondeterminism yalnız caller'ın measured_at değeridir.
pub(crate) fn project_analysis(
    analysis: &AnalysisResult,
    policy: PathCasePolicy,
    evidence_context: EvidenceProjectionContext,
) -> Result<BridgeRunOutput, BridgeError> {
    let candidate_proj = project_candidate_nodes(analysis, policy, scheme)?;
    let metric_projection = project_code_metrics(analysis, &candidate_proj.identity_index)?;
    let evidence_projection = project_observed_evidence(
        &metric_projection.metrics,
        evidence_context,
    )?;
    Ok(BridgeRunOutput { ..., evidence_projection })
}
```

### C. `commands/graph.rs` — clock + diagnostics (conversion YOK, provider construct YOK)

- `now_unix_secs()` helper (`std::time::SystemTime` — chrono yok; `engine.rs:920-925` pattern):
  ```rust
  fn now_unix_secs() -> u64 {
      std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .map(|d| d.as_secs())
          .unwrap_or(0)
  }
  ```
- `EvidenceProjectionContext { measured_at: now_unix_secs() }` inject → `project_analysis`.
- **Stderr flip (tur 2 dürüst consumer beyanı):**
  ```
  Evidence construction: completed
  Evidence objects: {N}
  Partial evidence objects: {P}  // < 5 axis (analyzer 3 axis üretir → partial)
  Evidence runtime consumer: none in graph init
  Evidence persistence: disabled
  ```
- **Provider construct YOK** production path'te — integration test'te (E maddesi).

### D. Guard matrisi (`architecture_guards.rs`) — ownership guard (tur 2 P3 güçlendirme)

Mevcut guard korunur + yeni **ownership guard** eklenir (substring denial'dan daha güçlü):

```rust
// Mevcut — korunur (metric_projection.rs core evidence YOK)
#[test]
fn metric_projection_does_not_construct_complete_core_evidence() {
    // metric_projection.rs içinde ObservedCodeEvidence / PhysicalCodeVector YOK
}

// YENİ — ownership: core evidence construction yalnız evidence_projection.rs'de
#[test]
fn core_evidence_construction_owned_by_evidence_projection() {
    // Tüm CLI production source dosyalarını tara (src/**/*.rs):
    //   ObservedPhysicalMetric::new
    //   ObservedPhysicalMetrics::try_new
    //   ObservedCodeEvidence::new
    // token'ları yalnız src/evidence_projection.rs'de bulunabilir.
    // analysis_bridge.rs, commands/graph.rs, metric_projection.rs'de YOK.
}
```

Bu guard alias/helper ile aşılabilir ama ownership iddiasını doğrudan ifade eder.

### E. Integration test — compatibility proof (tur 2 P1 — provider production path'te değil)

`crates/osp-cli/tests/evidence_wiring_proof.rs` (yeni) veya `analyze_bridge_flow.rs` içinde yeni test:

```rust
#[test]
fn evidence_projection_feeds_pipeline_scorer_and_gate() {
    // analysis → project_analysis → evidence_projection.evidence
    // → InMemoryCodeEvidenceProvider::from_evidence
    // → AnchorGateContext::with_code_evidence
    // → AnchorPipeline::run_with_source (veya direkt scorer.score + gate.decide)
    // Assert: provider lookup code_entity_id → Some; evidence_strength > 0.
}
```

Bu **compatibility proof** — production consumer DEĞİL. PR açıklaması bunu "in-process wiring
proof" olarak adlandırır; production consumer'ın henüz bulunmadığı dürüstçe belirtilir.

---

## Test stratejisi (tur 2 P3 — factory + minimum matris)

### Test factory (tur 2 P3 — `ProjectedCodeMetric` private fields)

`metric_projection.rs` içine `#[cfg(test)]` factory (production constructor DEĞİL):
```rust
#[cfg(test)]
pub(crate) fn projected_metric_for_tests(
    node_id: ConceptNodeId,
    axis: PhysicalCodeAxis,
    value: f64,
    source: ObservedCodeMetricSource,
    confidence: f64,
    coverage: f64,
) -> ProjectedCodeMetric
```
Mevcut validated newtype'ları kullanır (`MetricAxisValue::new`, `MetricConfidence::new`,
`MetricCoverage::new`). Bu sayede `evidence_projection.rs` testleri 5 axis'i (Entropy/WitnessDepth
dahil) kapsayabilir — synthetic `AnalysisResult` yalnız analyzer'ın ürettiği 3 axis'i (Coupling/
Cohesion/Instability) verir.

### Minimum test matrisi
```
groups_metrics_by_node_deterministically
maps_all_five_axis_variants_exhaustively         // Coupling/Cohesion/Instability/Entropy/WitnessDepth
preserves_mixed_provenance                        // TreeSitter + Scip aynı node
uses_injected_measured_at                         // TEST_MEASURED_AT = 1_700_000_000
creates_partial_evidence_for_three_axes           // analyzer 3 axis → partial
rejects_invalid_strength_with_context             // InvalidStrength { node_id, axis, source }
rejects_invalid_coverage_with_context             // InvalidCoverage
defensively_handles_observation_contract_mismatch // InvalidObservation (core constructor hatası)
empty_metric_slice_produces_empty_output          // input_metric_values=0, evidence_objects=0
provider_wiring_proof_reaches_scorer_and_gate     // integration test (E maddesi)
```

---

## Kabul kriterleri

1. `evidence_projection.rs` yeni modül — draft→evidence conversion tek sahibi
2. metric_projection.rs durur (draft üretir), core evidence YOK (guard korunur)
3. `PhysicalCodeAxis` → `PhysicalCodeMetricAxis` map (CLI→core adopt, 5 variant exhaustive)
4. `MetricConfidence` → `EvidenceStrength` (re-validate, InvalidStrength varyantı)
5. `MetricCoverage` → `EvidenceCoverage` (re-validate, InvalidCoverage varyantı)
6. `MetricAxisValue.get()` → raw `f64` (duplicate validation YOK; core constructor'a bırak)
7. `measured_at` context'ten inject (wall-clock graph.rs'de, deterministic test)
8. Report input yüzeyiyle uyumlu (input_metric_values / evidence_objects_created / partial; distinct_nodes/empty-skip YOK)
9. Production path: evidence + diagnostics (provider construct YOK)
10. Compatibility proof: integration test provider → scorer/gate seam
11. Stderr flip: "deferred" → "completed"; "consumer: none in graph init" (dürüst)
12. analyze_bridge_flow.rs stderr assertion update (lockstep)
13. Guard matrisi: metric_projection.rs deny korunur + ownership guard yeni (evidence_projection.rs tek sahibi)
14. Persistence KAPSAM DIŞI (store'a yazma YOK; store schema büyütme YOK; ObservedCodeEvidence Deserialize YOK)
15. osp-core untouched (PR C evidence model hazır)
16. Typed error (anyhow YOK) — node/axis context korunur; InvalidStrength dahil
17. Test factory (`projected_metric_for_tests`) + minimum test matrisi (9 test)

---

## Uygulama sırası

1. `evidence_projection.rs` (types + `project_observed_evidence` + typed error + unit testler)
2. `projected_metric_for_tests` factory (`metric_projection.rs` `#[cfg(test)]`)
3. `analysis_bridge.rs` (`BridgeRunOutput` + `BridgeError::EvidenceProjection` + orchestrator)
4. `commands/graph.rs` (clock helper + context inject + stderr flip — provider YOK)
5. `architecture_guards.rs` (metric_projection.rs deny korunur + ownership guard yeni)
6. `evidence_wiring_proof.rs` integration test (compatibility proof)
7. `analyze_bridge_flow.rs` (stderr assertion lockstep update)
8. Workspace validation (`RUSTFLAGS="-D warnings" cargo test --workspace --exclude osp-desktop`)
9. Doküman/count updates (HANDOFF/STATUS/run-metadata — frozen/current ayrımı)

---

## Doküman güncellemeleri (frozen/current ayrımı — tur 2 P3)

- **run-metadata.md:** yalnız **Current protocol metadata** güncellenir (osp-cli test counts,
  evidence_projection.rs boundary). **Frozen snapshot değişmez.**
- **run-metadata.json:** frozen snapshot olduğu için değişmez (`cumulative_trybuild_context: 26`
  korunur — PR D compile-fail eklemez).
- **STATUS.md:** osp-cli test counts + Faz 8b/PR D completion.
- **HANDOFF.md:** PR D completion entry + PR E/G/F roadmap güncelleme.

---

## HANDOFF bullet'leri (PR D sonrası)

- **Anchoring consumer gap (tur 2 P2 düzeltme — review DEĞİL):** production consumer henüz yok.
  `AnchorPipeline::run_with_source` çağıran anchoring/ingest/evaluate CLI surface future work.
  Provider gerçek consumer `AnchorGateContext.code_evidence` + scorer; `graph init` bunu çalıştırmıyor.
- **Persistence milestone (PR G):** `PersistedObservedCodeEvidence` schema version + validated
  restore + latest/history politikası + deterministic ordering + upsert/append semantics + snapshot
  integration + corruption tests. `ObservedCodeEvidence` Deserialize VERİLMEZ — serde-friendly
  persisted DTO `try_restore()` → runtime tip (PR C smart-constructor boundary korunur).
- **EvidenceSource abstraction (future):** `EvidenceSource = fresh analysis` (PR D) →
  `EvidenceSource = validated persisted DTO` (PR G). Consumer değişmez; provider'ı besleyen source değişir.
- **v1.4 pending paper edits:** evidence_projection.rs boundary + compatibility proof semantics.
- **`measured_at` policy:** PR D `now_unix_secs()` inject; PR G wall-clock source (NTP/system) policy.

---

## run-metadata: current protocol — osp-cli test counts update; frozen snapshot 26 unchanged.
