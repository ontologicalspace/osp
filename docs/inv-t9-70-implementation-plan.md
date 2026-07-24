# INV-T9 #70 — Engine-Issued Per-Axis Measurement Provenance Implementation Plan

**Tarih:** 2026-07-20
**Branch:** `fix/inv-t9-witness-suspension`
**PR:** #69 (https://github.com/ontologicalspace/osp/pull/69)
**Current head:** `920a1dc`
**Issue:** #70 (https://github.com/ontologicalspace/osp/issues/70)
**Plan approval:** Reviewer v4 APPROVED (9.7/10) — implementation-ready

---

## Yeni oturumda ilk komut

```bash
cd P:/Work/SoftwarePhysics
git checkout fix/inv-t9-witness-suspension
git pull  # 920a1dc head olmalı
git log --oneline -3
RUSTFLAGS="-D warnings" cargo test -p osp-core --lib  # 888 test geçmeli
```

Sonra bu belgeyi oku, #70 implementation'a başla (Commit 1 ile).

---

## Kazanımlar (bu oturumdan)

INV-T9 #72 (embedded attempt-evidence integrity) **tamamen bitti** — 5 implementation + 5 closure commit:
- `83031e1` → `5ea3cfe` (implementation)
- `9070a48` → `920a1dc` (closures, reviewer P0/P1/P2)

#72 evidence-integrity scope complete:
- Held evidence propagation: navigator end-to-end exact test (`inv_t9_72_held_production_path_exact`)
- Rejected evidence construction: production mapper direct test (`make_revision_required_from_rejection`)
- Rejected witness-gate reachability: #73'te takip ediliyor

**888 osp-core lib tests**, workspace build + tests temiz, clippy baseline.

---

## #70 — Sorun (code-verified)

`provenanced_from_raw(raw, source)` (`navigator.rs:169`) tek bir `MetricSource` alıp **tüm 5 eksene** yazıyor. Tüm production caller'lar `MetricSource::Scip` geçiyor:
- `navigator.rs:832` — `AgentNavigator::run_task`
- `osp-mcp/src/server.rs:768, 867` — `current_measured`, `submit_delta_attempt`
- `osp-cli/src/commands/mod.rs:309` — CLI initial

Sonuç: INV-T4 source-requirement bypass edilebilir — `required_source = Scip` predicate entropy ekseninde (commit-history-derived) **geçersiz** ölçümle geçer.

### Per-axis gerçek originler
| Axis | Gerçek origin | Bugün claimed |
|---|---|---|
| coupling | graph topology (TreeSitter) | Scip |
| cohesion | node.cohesion Some→Scip, None→Placeholder | Scip |
| instability | graph topology (TreeSitter) | Scip |
| entropy | commit-entropy config (Heuristic) | Scip |
| witness_depth | witness config (Heuristic) | Scip |

### İkincil sorun
`TaskCommitInput.measured` public, caller-set. `commit_task_claim` olduğu gibi kabul ediyor — re-measure yok, source validation yok, context↔measurement binding yok.

---

## Authority surface modeli (net sınır)

```
EngineMeasurement → ölçüm authority'si (yalnız fiziksel ölçüm, loss YOK)
commit_task_claim → resolved target + loss authority'si (task resolve → preferred_vector → loss compute)
derive_task_subject_scope → tek scope authority helper'ı (task'tan engine türetir)
```

Üç authority surface kapatıldı (reviewer P0'lar):
1. **Token loss'suz** — `MeasurementBaseline` enum (Existing/Unavailable), loss commit'te resolved target'tan
2. **Claim.computed_raw karar uzayından çıkarıldı** — task-bound yol token.after kullanır, computed_raw okumaz/compare etmez
3. **Engine-derived effective scope** — `measure_task_delta(task, ...)`, subject_scope task'tan, impact_scope structural delta'tan

---

## Yapısal tasarım (reviewer v4 onaylı + P1/P2 sabitleme)

### coords katmanı (P1-4 neutral, P1-1 descriptor encoding)

```rust
// coords.rs
pub struct AxisMeasurement { pub value: f64, pub source: MetricSource }  // public (P2-1)
pub struct MeasuredRawPosition {
    pub coupling: AxisMeasurement, pub cohesion: AxisMeasurement,
    pub instability: AxisMeasurement, pub entropy: AxisMeasurement,
    pub witness_depth: AxisMeasurement,
}
pub enum CoordinateMeasurementError { ... }  // coords katmanı error

// trajectory.rs alias:
pub use crate::coords::AxisMeasurement as AxisMetric;
pub use crate::coords::MeasuredRawPosition as ProvenancedRawPosition;
```

### MetricSource::Mixed (P1-2 heterojen aggregation)

```rust
// coords.rs — domain enum, numeric discriminant YOK
pub enum MetricSource { TreeSitter, Scip, Placeholder, Heuristic, Mixed }

// canonical_tags.rs — stable numeric tag (wire katmanı)
CanonicalMetricSourceTag: TreeSitter=0, Scip=1, Placeholder=2, Heuristic=3, Mixed=4

// aggregate_source(): homojen→o source, heterojen→Mixed, boş→Err
// Predicate: Mixed + required_source=X → SourceInsufficient
// Task requirement Mixed → reject (Mixed observation result, not desired source class)
```

### Axis trait (P1-3 compute() panic yok)

```rust
pub trait Axis: Send + Sync {
    fn name(&self) -> &'static str;
    fn descriptor(&self) -> Result<AxisDescriptor, AxisDescriptorError>;
    fn measure(&self, node: &Node, space: &Space) -> Result<AxisMeasurement, AxisMeasurementError>;
    #[deprecated]
    fn compute(&self, node: &Node, space: &Space) -> f64;  // her axis kendi compute_value helper'ı
    fn try_compute(&self, node: &Node, space: &Space) -> Result<f64, AxisMeasurementError> {
        Ok(self.measure(node, space)?.value)
    }
}
```

### P1-1 Descriptor source encoding (katman sınırı)

**ÖNEMLİ:** coords katmanı `CanonicalMetricSourceTag` kullanmaz (ters bağımlılık). Descriptor'a stable byte ID encode edilir:

```rust
// coords.rs — descriptor source encoding (stable byte ID, canonical tag DEĞIL)
fn descriptor_source_id(source: MetricSource) -> &'static [u8] {
    match source {
        MetricSource::TreeSitter => b"tree-sitter",
        MetricSource::Scip => b"scip",
        MetricSource::Placeholder => b"placeholder",
        MetricSource::Heuristic => b"heuristic",
        MetricSource::Mixed => b"mixed",
    }
}
// params.push_bytes(descriptor_source_id(self.source))?;

// Ayrım:
// AxisDescriptor parameter identity → stable source ID bytes (coords katmanı)
// Authorization wire representation → CanonicalMetricSourceTag (wire katmanı)
```

### Per-axis source + descriptor (semantics v2)

- **Coupling/Instability:** construction-time source (default Placeholder, production preset TreeSitter). Descriptor: semantics v2 + source ID bytes.
- **Cohesion:** `observed_source: MetricSource` (construction-time, default Placeholder; analyzer preset Scip) + `fallback: Option<AxisMeasurement>`. Per-node: Some→observed_source, fallback→fallback source, None→Placeholder. Descriptor: fallback policy + observed_source + per-node policy.
- **Entropy/WitnessDepth:** Heuristic sabit. Descriptor: source ID explicit.

### CoordinateSystem (P1-4 raw_position_of unchanged)

```rust
// Commit 2 add-only:
pub fn measured_position_of(&self, node, space) -> Result<MeasuredRawPosition, CoordinateMeasurementError>
pub fn try_raw_position_of(&self, node, space) -> Result<RawPosition, CoordinateMeasurementError>
pub fn raw_position_of(&self, node, space) -> RawPosition  // Commit 4'e kadar değişmeden
```

### P1-2 Subject/impact aggregate invariant (ÖNEMLİ)

```text
MeasuredRawPosition yalnız resolved subject_scope üyeleri üzerinden aggregate edilir.
impact_scope ölçüm sonucuna ek node katmaz; yalnız delta dependency/recomputation ve hint-validation kanıtıdır.

Örnek:
Task subject      = Node(0)
Remove import     = 0 → 1
Impact scope      = {0, 1}
Measured subject  = yalnız Node(0)  ← {0,1} centroid DEĞIL

Before/after aggregate aynı resolved subject membership.
Subject scope'un herhangi bir üyesi pre-mutation space'te bulunmuyorsa:
→ partial centroid hesaplanmaz → MeasurementBaseline::Unavailable
→ existing members asla silently partial baseline olarak ölçülmez.
```

### P1-4 MeasurementRequest + Digest aynı katman (tercihen yeni modül)

```rust
// crates/osp-core/src/measurement.rs (yeni neutral modül)
// Bağımlılık: coords ↓ measurement ↓ engine/authorization/trajectory

pub enum MeasurementMode { CurrentScope, HypotheticalDelta }
pub struct CanonicalMeasurementScope { ... }  // Node(id) / Subgraph(ids) / Module(name→ids resolved)
pub struct MeasurementRequest {
    structural_delta: CanonicalStructuralDelta,
    subject_scope: CanonicalMeasurementScope,       // task predicate scope (resolved Task authority)
    impact_scope: NonEmptyCanonicalNodeIds,          // engine-derived (structural delta + axis dependency)
    mode: MeasurementMode,
    aggregation_semantics_version: u32,
}
pub struct MeasurementRequestDigest([u8; 32]);  // BLAKE3, domain-separated (osp.measurement-request.v1\0)
pub enum MeasurementBaseline {
    Existing(MeasuredRawPosition),
    Unavailable { reason: BaselineUnavailableReason },  // NewSubject, EmptyPreMutationScope, PartialSubject
}
pub enum BaselineUnavailableReason { NewSubject, EmptyPreMutationScope, PartialSubject }
```

### EngineMeasurement token

```rust
// measurement.rs veya engine.rs
pub struct EngineMeasurement {
    before: MeasurementBaseline,  // loss YOK (P0-1)
    after: MeasuredRawPosition,
    measurement_input_digest: MeasurementInputDigest,
    base_space_view_revision: SpaceViewRevision,
    measurement_request_digest: MeasurementRequestDigest,
}
// Private fields, read-only accessors, Clone YOK (by-value consume)
```

### SpaceEngine API (P1-1 hint, P1-4 compute_raw_from_delta unchanged)

```rust
// Commit 3 add-only:
pub fn measure_task_delta(&self, task: &Task, delta_nodes: &[Node], delta_edges: &[Edge],
    removed_edges: &[EdgeRef], affected_nodes_hint: &[NodeId])
    -> Result<EngineMeasurement, MeasurementError>
// subject_scope = derive_task_subject_scope(task)? (tek helper, task predicate scope)
// impact_scope = derive_impact_scope(structural_delta)? (engine, axis dependency)
// hint mismatch → MeasurementError::ImpactScopeHintMismatch (commit öncesi)
// before: existing space + subject scope; after: hypothetical delta + same subject scope
// Centroid aggregation: yalnız subject_scope üyeleri, homojen→o source, heterojen→Mixed

pub fn measure_current_scope(&self, scope: &CanonicalMeasurementScope)
    -> Result<MeasuredRawPosition, MeasurementError>
// Planning projection (AgentTaskView) — authority token değil

pub fn try_compute_raw_from_delta(...) -> Result<RawPosition, MeasurementError>  // fallible projection

#[deprecated]
pub fn compute_raw_from_delta(...) -> RawPosition  // Commit 3'te unchanged, Commit 4'ten sonra kaldırılabilir
```

### TaskCommitInput (P0-2 subject_scope YOK)

```rust
pub struct TaskCommitInput<'a> {
    pub claim: &'a Claim,
    pub omega: &'a WitnessSet,
    pub task_resolver: &'a dyn TaskResolver,
    pub measurement: EngineMeasurement,
    // KALDIRILDI: target, loss_before, measured, subject_scope
}
```

### commit_task_claim (P0-1/2/3, P1-3 Mixed validation)

```rust
// task resolve → preferred_vector (MissingPreferredVector error)
// structural syntax check only (claim.computed_raw okuma — P0-2)
// verify measurement: context digest + revision + request digest (engine re-derives scope)
// subject_scope = derive_task_subject_scope(task)? — engine, caller değil
// target = task.preferred_vector
// loss_before = match before { Existing(b) => Some(trajectory_loss(b, &target)), Unavailable => None }
// loss_after = trajectory_loss(after, &target)
// Q5/PredicateGate/basis/evidence → token.after only
// P1-3: required_source=Mixed → TaskValidationError (commit authoritative guard)
```

### AuthorizationBasis v2 (P1-5 tek canonical, P1-7 consistency)

```rust
AuthorizationBasis {
    measured_before: CanonicalMeasurementBaseline,
    measured_after: ProvenancedMeasuredResult,  // tek canonical after (duplicate yok)
    measurement_input_digest: MeasurementInputDigest,
    measurement_request_digest: MeasurementRequestDigest,  // basis'te
    base_space_view_revision: SpaceViewRevision,
    predicate_evaluation: PredicateEvaluationBasis {
        target_vector, loss_before: Option<CanonicalF64>, loss_after, policy...
    },
}
// P1-7 consistency invariant:
//   baseline Existing ↔ loss_before Some (== distance(before, target))
//   baseline Unavailable ↔ loss_before None
//   loss_after == distance(after, target)
//   imkânsız kombinasyon → AuthorizationVerificationError::BaselineLossInconsistent
```

### Evidence baseline (P1-2 enum, iki Option değil)

```rust
pub enum TrajectoryEvidenceBaseline {
    Existing(RawPosition),
    Unavailable { reason: BaselineUnavailableReason },
}
pub struct TrajectoryEvidence {
    pub before: TrajectoryEvidenceBaseline,
    pub after: RawPosition,
    // ...
}
// Paralel üç yüzey: runtime MeasurementBaseline, authorization CanonicalMeasurementBaseline, evidence TrajectoryEvidenceBaseline
```

### Error katmanlama

```rust
coords::CoordinateMeasurementError  // coords katmanı
engine::MeasurementError { Coordinate(#[from]), EmptyMeasurementScope, MissingAffectedNode,
    ImpactScopeHintMismatch { declared, derived }, UnresolvedMeasurementScope, ... }
engine::EngineCommitError { MeasurementContextMismatch, StaleMeasurement { measured, current },
    MeasurementSubjectMismatch, MissingPreferredVector { task_id },
    HeterogeneousPredicateScopesUnsupported { scopes }, ... }
TaskValidationError { InvalidRequiredMetricSource { source: MetricSource }, ... }  // Mixed requirement
AuthorizationVerificationError { BaselineLossInconsistent, ... }
```

### P2 temizliği
- `ImprovementBaseline` adı tamamen kaldırılır — her yerde `MeasurementBaseline` / `CanonicalMeasurementBaseline` / `TrajectoryEvidenceBaseline`
- `ClaimMeasurementMismatch` kaldırıldı (task-bound computed_raw okumaz/compare etmez)
- `affected_nodes_hint` empty → engine derives freely; non-empty → must exactly equal derived impact scope (subset/superset kabul edilmez)

---

## Sürüm matrisi

| Sözleşme | Sürüm | Gerekçe |
|---|---|---|
| AuthorizationBasis schema | **v2** | before/after + request digest + baseline semantiği |
| Measurement semantics | **v2** | per-axis source, Mixed, aggregation, before/after |
| AxisDescriptor semantics (etkilenen) | **v2** | source axis davranışının parçası |
| MeasurementInputContext schema | **v1** | wire shape aynı |
| MeasurementInputDigest domain sep | **v1** | encoder aynı; içerik değişince digest değişir |
| AuthorizationBasisDigest domain sep | **v2** (`osp.authorization-basis.v2\0`) | preimage değişti |
| MeasurementRequestDigest (yeni) | **v1** (`osp.measurement-request.v1\0`) | ilk sürüm |

Eski AuthorizationBasis v1 artifact intentionally unsupported (strict-load negative fixture, no migration).

---

## Commit zinciri (6 commit)

### Commit 1 — `feat(coords): provenance-native axis measurement contract`
- `coords::AxisMeasurement` + `MeasuredRawPosition` + `CoordinateMeasurementError`
- `MetricSource::Mixed` + `aggregate_source()` + `CanonicalMetricSourceTag::Mixed=4`
- `Axis::measure()` + `compute()` deprecated + `try_compute()` (her axis compute_value helper)
- 5 core axis impl (source descriptor semantics v2 + **P1-1 stable byte ID encoding**)
- Coupling/Instability construction-time source (default Placeholder); Cohesion observed_source + fallback Option<AxisMeasurement>; Entropy/WitnessDepth Heuristic
- Tüm CoordinateSystem/test impl'leri hem measure hem compute (geçiş)
- **MeasurementInputDigest golden update** (source descriptor)

### Commit 2 — `feat(coords): provenance-aware position measurement` (add-only)
- `measured_position_of()` + `try_raw_position_of()`
- `raw_position_of` unchanged (Commit 4'e kadar)
- Centroid aggregation policy + heterojen Mixed testleri

### Commit 3 — `feat(engine): subject-bound EngineMeasurement tokens` (add-only, non-breaking)
- Yeni `measurement.rs` neutral modül (P1-4): MeasurementMode, CanonicalMeasurementScope, MeasurementRequest, MeasurementRequestDigest, MeasurementBaseline, BaselineUnavailableReason
- `EngineMeasurement` private-field token (before+after+context+revision+request, loss YOK)
- `measure_task_delta(task, ...)` + `measure_current_scope(scope)` planning
- `derive_task_subject_scope(task)` + `derive_impact_scope(structural_delta)` + hint validation
- **P1-2 subject/impact aggregate invariant** (subject_scope üyeleri only, partial→Unavailable)
- `try_compute_raw_from_delta` fallible; `compute_raw_from_delta` deprecated unchanged
- `MeasurementError` taxonomy
- **MeasurementRequestDigest golden (v1 yeni)**

### Commit 4 — `refactor(inv-t4): require EngineMeasurement across authority and evidence paths` (ATOMIK)
- `TaskCommitInput { claim, omega, task_resolver, measurement }` (subject_scope YOK — P0-2)
- `commit_task_claim`: task resolve + preferred_vector + engine-computed loss + measurement verify + **claim.computed_raw ignore** (P0-2) + structural syntax check ayrımı + **Mixed validation** (P1-3)
- `AuthorizationBasis v2` (before+after single canonical + request digest + **P1-7 baseline/loss consistency**)
- `PredicateGateInput` → token baseline/after
- `TrajectoryEvidenceBaseline` enum (P1-2)
- **Tüm caller migration atomik:** Navigator (current_measurement planning only), MCP, CLI, g2c_corpus_matrix, tüm test construction site'ları
- `provenanced_from_raw` production/evidence path'ten kaldır
- Domain sep `osp.authorization-basis.v2\0`
- `TaskValidationError` (Mixed requirement)
- **AuthorizationBasis v2 golden + v1 strict-reject fixture**
- Post-commit grep: `provenanced_from_raw(.*Scip` authority/evidence yolunda sonuç vermemeli

### Commit 5 — `test(inv-t4): adversarial measurement-binding regressions`
- 17+ regression test:
  1-2. descriptor source/digest farklılaştırma
  3-4. subject mismatch (delta/scope)
  5. caller loss_before veremez
  6. heterojen → Mixed
  7. stale revision → StaleMeasurement
  8. farklı CoordinateSystem → MeasurementContextMismatch
  9. entropy Heuristic + required Scip → SourceInsufficient
  10. token private-field API surface
  11. task_bound ignores legacy computed_raw (P0-2)
  12. baseline unavailable → improved=false (P0-1)
  13. heterogeneous predicate scopes → reject (P0-3)
  14. affected_nodes hint mismatch → MeasurementError (P1-1)
  15. AuthorizationBasis v1 artifact rejected (P1-6)
  16. required_source=Mixed → TaskValidationError (P1-3)
  17. baseline/loss consistency invariant (P1-7)
  18. **partial_pre_mutation_subject_produces_unavailable_baseline** (P1-2)
  19. **impact_scope_does_not_expand_subject_aggregate** (P1-2)

### Commit 6 — `docs(inv-t4): conformance + truth-surface`
- Conformance doc: #70 scope, authority surface closure, v1→v2 migration policy
- #70 acceptance checklist
- PR body sync (gerçek SHA + test count)

---

## v1 byte contract (commit yerleşimi)
- **Commit 1:** MeasurementInputDigest golden (source descriptor)
- **Commit 3:** MeasurementRequestDigest golden (yeni v1)
- **Commit 4:** AuthorizationBasis v2 golden + v1 strict-reject
- **Commit 6:** doc only (golden değil)

---

## CI simülasyonu (her commit öncesi)
```bash
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo build --workspace --examples --exclude osp-desktop
RUSTFLAGS="-D warnings" cargo test --workspace --exclude osp-desktop
cargo clippy -p osp-core --lib
```

---

## Governance / risk
- PR #69 GOVERNANCE §3 high-risk (evidence integrity + measurement provenance)
- #70 runtime semantic correctness blocker — INV-T4 source-requirement bypass kapatılır
- Her commit bağımsız derlenir + tek authority yüzeyi (Commit 4 atomik)
- Force-push yok, incremental normal push

---

## Out of scope (issue'lar açıldı)
- #74 — Deterministic Held fixture redesign (Q3 wiring related)
- #75 — Edge-level provenance model
- #76 — ScopedMeasurementSet (heterogeneous predicate scopes)
- #77 — StandaloneClaim/TaskStructuralClaim type separation
- #78 — Mixed(BTreeSet<MetricSource>) richer model

---

## Önemli dosya lokasyonları (doğrulanmış)

### #70 için
- `crates/osp-core/src/coords.rs` — AxisMeasurement, MeasuredRawPosition, MetricSource::Mixed, Axis trait, CoordinateSystem, CoordinateMeasurementError, descriptor_source_id
- `crates/osp-core/src/axes.rs` — 5 axis impl measure() + source descriptor (lines: CouplingAxis:33-61, EntropyAxis:74-109, WitnessDepthAxis:123-158, InstabilityAxis:176-208, CohesionAxis:221-285)
- `crates/osp-core/src/measurement.rs` — YENİ neutral modül (MeasurementRequest, MeasurementRequestDigest, MeasurementBaseline, vs.)
- `crates/osp-core/src/engine.rs` — EngineMeasurement, measure_task_delta, measure_current_scope, commit_task_claim (Commit 4), compute_raw_from_delta:1416-1474, current_space_view_revision:1341-1351, build_authorization_context:668-856
- `crates/osp-core/src/navigator.rs` — current_measurement planning, run_task migration, provenanced_from_raw:169 (kaldırılacak), run_task commit loop:790-852
- `crates/osp-core/src/trajectory.rs` — alias (AxisMetric, ProvenancedRawPosition), PredicateGateInput:1006-1014, trajectory_loss:859-865, TrajectoryEvidence
- `crates/osp-core/src/authorization.rs` — AuthorizationBasis v2, domain sep, MeasurementInputContext/Digest, AxisDescriptor
- `crates/osp-core/src/canonical_tags.rs` — CanonicalMetricSourceTag::Mixed=4
- `crates/osp-mcp/src/server.rs` — current_measured:753-769, submit_delta_attempt:775-885
- `crates/osp-cli/src/commands/mod.rs` — initial measurement:309
- `crates/osp-analyzer/examples/g2c_corpus_matrix.rs` — evidence runner:489,592,777,899

### Genel
- v1 golden vectors: `authorization.rs` test modülü
- DOMAIN_SEPARATOR'lar: `osp.authorization-basis.v1\0`, `osp.evaluation-context.v1\0`, `osp.measurement-input.v1\0`, `osp.space-content.v1\0`, yeni: `osp.authorization-basis.v2\0`, `osp.measurement-request.v1\0`

---

## Review zinciri yaklaşımı
INV-T9 #72 serisi incremental scoped review ile ilerledi. #70 için aynı yaklaşım:
1. Plan (EnterPlanMode) → reviewer onay (**TAMAMLANDI** — v4 APPROVED 9.7/10)
2. Implementation commit'leri (6 commit)
3. Scoped review → REQUEST CHANGES/APPROVED
4. Closure commit'leri
5. Issue kapatma (scoped APPROVAL sonrası)

---

*Bu belge INV-T9 #70 implementation'ına geçiş için handoff'tır. #72 evidence-integrity tamamen bitti.*
