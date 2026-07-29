# INV-T9 #70 Faz 8-P2 — Handoff (PR #85/#91/#93 MERGED; PR #98 migration decisions HAZIR)

## Repository state

```
PR #85: MERGED (squash e4675b2) — Faz 8-P2 P2-0A+B characterization
PR #91 (#87-A): MERGED (squash efede00) — policy fixture (Part 1/2)
PR #93 (#87-B): MERGED (squash 7c670ff) — Q5 exact theta (Part 2/2, Closes #87)
PR #98: migration decisions HAZIR — MD-1/MD-2/MD-3 Accepted (review bekliyor)
Issue #92: subject-authority → raw → Q5 theta downstream (follow-up, açıldı)
Issue #95/#96/#97: MD-1/MD-2/MD-3 implementation tracking
```

## PR #85 Özet (MERGED)

**Faz 8-P2 P2-0A+B — V1/V2 Measurement Characterization.** Production koduna
dokunulmadı — sadece test altyapısı + docs + fixture. Squash merge `e4675b2` → main.
Üç ontolojik boyut karakterize edildi: subject authority (KANITLANDI), provenance
authority INV-T4 (KANITLANDI), baseline epistemic availability (availability KANITLANDI,
policy HİPOTEZ → **PR #87-A ile KANITLANDI**).

### Commit zinciri (squash → e4675b2, orijinal 13 commit)

- `483e8a4` P2-0A construction feasibility (4 test + rapor)
- `2b4377a` P2-0B altyapı + observation model + harness + baseline parity
- `58f60e4` P2-0B divergence characterization + ontolojik karar raporu
- `fb040fe`..`2b88a8d` 9 review fix turu

### PR #85 Review zinciri (9 tur)

| Tur | Ana bulgu | Düzeltme |
|---|---|---|
| 1 | 4 P0 (Held/Rejected fabrication, V1 loss_before, Test 3 sentinel, rapor overclaim) | Tur 6'da Held fabrication çürütüldü |
| 2 | 2 P0 (Case 4 fixture ID Node(10000), matching source divergence + required_source matrix) | required_source matrix gerçek PredicateSet ile |
| 3 | 2 P0 (required_source matrix gerçek evaluate_completion, Case 4 decision-drift kaldırma) | PredicateSet-level decision divergence |
| 4 | 2 P0 (Case 4 üçüncü boyut, pipeline NotReached) | Baseline availability 3. migration boyutu |
| 5 | 2 P0 (representation vs policy divergence, NotReached vs ReachedButUnsurfaced) | Case 4 Completed → projection etkisiz |
| 6 | 2 P0 (Held outcome observable, Case 4 Observed(AcceptAsCompleted)) | Tur 1 fabrication çürütüldü |
| 7 | 1 P0 (baseline DefaultFallback — V1 yokluk → sıfır koordinat) | BaselineObservation model |
| 8 | 2 P0 (schema 1/4↔4/4 çelişkisi, exact baseline golden) | 4/4 structural + exact golden |
| 9 | 1 P0 (Migration 3 canonical sync + fail-closed) | 6 boyuta ayrım + tüm metadata senkron |

## Üç Ontolojik Boyut (P2-1 blocked)

### Migration 1: Subject authority — KANITLANDI
V1 subject = `proposal.affected_nodes` (LLM-declared), V2 subject = `task.predicate.scope`
(task-derived). Case 2/3'te farklı node set → farklı centroid → farklı measured values.

Reviewer tercihi: **Yol 2 (task-authoritative)** — `task.predicate.scope → subject authority`,
`structural delta → impact authority`, `affected_nodes → impact hint/telemetry`. Yol 3
(affected == scope) reddedilir — subject/impact ayrımını çökertir.

### Migration 2: Provenance authority (INV-T4) — KANITLANDI
V1 `provenanced_from_raw(..., Scip)` uniform Scip; V2 engine gerçek per-axis source'ları
(TreeSitter/Placeholder/Heuristic). Normatif hedef (Paper 1/2). Matching scope'ta BİLE
source divergence var. `required_source` matrix ile PredicateSet-level decision divergence
kanıtlandı (Scip/TreeSitter).

### Migration 3: Baseline epistemic availability & policy — KANITLANDI (PR #87-A)
V1 `DefaultFallback` (yokluk → `RawPosition::default()` sıfır koordinat), V2 typed
`UnavailableAllIntroduced`. Case 4 `Completed` → `AcceptAsCompleted` parity (projection
etkisiz). **Policy/decision divergence KANITLANDI** — `delta-introduced-subject-policy-001`
fixture (PR #87-A): V1 `AcceptAsProgress` (TrajectoryCheckpoint, Held) vs V2 `Reject`
(NotApplied, Evaluated). Counter-fixture (min_delta=1.0) V1 kabulünün improvement kaynaklı
olduğunu doğrular. Migration 3 (b) yorumu doğrulandı — policy/decision divergence
production-reachable structural topology üzerinde migration-relevant.

## Divergence Özeti (tur 9 canonical)

| Boyut | Frozen incidence |
|---|---|
| Subject-set divergence | 2/4 (Case 2/3) |
| Axis-value divergence | 2/4 (Case 2/3) |
| Baseline schema (structural) | 4/4 |
| Baseline availability | 1/4 (Case 4) |
| Baseline value bits | 0/3 + N/A (Case 4) |
| Baseline source | 3/3 + N/A (Case 4) |
| Baseline loss bits | 0/3 + N/A (Case 4) |
| Source divergence (INV-T4) | 4/4 |
| Pipeline mutation-decision | NotReached 3/4 + Observed+Equal 1/4 |
| Inline PredicateSet decision (required_source matrix) | 2/5 |

## Key file:line references

- `TaskCommitInput` struct: engine.rs:108-121 (legacy fields korunur — Faz 8a smart ctor)
- `measure_task_delta`: engine.rs:2366-2541 (pub, production-accessible)
- `compute_raw_from_delta`: engine.rs:2284-2342 (legacy, infallible — empty positions → default)
- `commit_task_claim`: engine.rs:1380-1518 (V1 path — body değişmez)
- `EngineCommitResult::Held { authorization }`: engine.rs:1133 (gerçek outcome taşır)
- `AuthorizationContext.outcome`: authorization.rs:5514 (AttemptOutcome)
- `PredicateSet::evaluate_completion`: trajectory.rs:326
- `MetricPredicate::evaluate`: trajectory.rs:233 (INV-T4 required_source)

### Test infrastructure
- `crates/osp-core/tests/common/mod.rs` — shared helper (case builders, manifest loader,
  observation model, V1/V2 harness, BaselineObservation, MutationDecisionObservation)
- `crates/osp-core/tests/measurement_v1_v2_parity.rs` — Yüzey 1 characterization (5 test)
- `crates/osp-core/tests/faz8_p2_manifest_guard.rs` — frozen fixture guard (4 test)
- `crates/osp-core/tests/faz8_p2_manifest_bootstrap.rs` — digest update helper (2 test)
- `tests/data/faz8_p2_characterization/cases.json` + `cases.blake3` — frozen manifest
- `crates/osp-core/src/engine.rs` test modülü — P2-0A construction feasibility (4 test)

## Sıradaki adımlar

### PR #85 — MERGED ✓
Squash merge `e4675b2` → main. Characterization canonical.

### PR #91 (#87-A) — MERGED ✓
Squash merge `efede00` → main. `delta-introduced-subject-policy-001` fixture ile Migration 3
(b) policy/decision divergence KANITLANDI.

### PR #93 (#87-B) — MERGED ✓
Squash merge `7c670ff` → main. Q5 theta parity koşullu KANITLANDI. Issue #87 KAPANDI.

### Ontolojik migration kararları — PR #98 HAZIR (MD-1/MD-2/MD-3 Accepted, review bekliyor)
**MD-1/MD-2/MD-3 canonical decision note + spec planned extensions.** `docs/notes/faz8-p2-migration-decisions.md`:
- **MD-1 Subject Authority:** task scope normatif (Yol 2); Yol 1 compat producer P2-1; caller cutover Faz 8a (#95).
- **MD-2 Provenance Authority:** engine-native per-axis normatif; uniform Scip compat; engine-internal cutover Faz 8a öncesi (#96).
- **MD-3 Baseline Policy:** typed Unavailable normatif + `AcceptAsColdStart` (Sandbox) + `ColdStartPolicy` (#97).
Spec planned extensions: INV-T3 (MD-1; dayanak INV-T2), INV-T4 (MD-2), INV-T6/T8/T9 (MD-3).

### P2-0B kalan iş (GitHub issue)
- **#86 (P2-0B.7):** MCP Workspace before-baseline characterization
- **#87 (P2-0B.8):** KAPANDI (PR #91 + #93) — policy fixture + Q5 exact theta
- **#88:** mixed_per_axis_sources dedicated matrisi (MD-2 evidence gate)
- **#89:** task-snapshot drift adversarial
- **#92:** subject-authority → raw → Q5 theta downstream (MD-1 cutover gate)
- **#95:** MD-1 implementation — subject-authority caller migration (Faz 8a)
- **#96:** MD-2 implementation — provenance enforcement (engine-internal cutover)
- **#97:** MD-3 implementation — baseline policy (AcceptAsColdStart + ColdStartPolicy)

### P2-1 (caller migration, migration kararları Accepted — additive Faz 5)
- `measure_task_delta_checked` (public boundary)
- Probe Claim → preflight-measure → final Claim → V1 commit_task_claim
- Lokal adapter (navigator + MCP) + repo-level oracle
- MD-1 compatibility producer (Yol 1, shadow observation) + SubjectAuthorityDriftObservation

### Faz 8a (engine cutover)
- `commit_task_claim` → V2 authorization consumer
- Smart ctor + legacy field kaldırma
- Target authority birleştirme
- Preflight tekrarı kaldırma

## CI Doğrulama (HEAD 2b88a8d)

```
cargo fmt --all -- --check: clean
RUSTFLAGS="-D warnings" cargo build --workspace --all-targets --exclude osp-desktop: clean
RUSTFLAGS="-D warnings" cargo test -p osp-core --lib: 1271 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test engine_measurement_single_producer: 7 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test measurement_binding_typelevel: 1 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test measurement_v1_v2_parity: 5 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test faz8_p2_manifest_guard: 4 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test faz8_p2_manifest_bootstrap: 2 passed
```

## Not

Bu PR **migration yapmıyor** — characterization + üç ontolojik boyut için kanıt üretiyor.
PR #84'ün "Faz 5 = semantic closure, Faz 8a = orchestration cutover" sınırını koruyor.
P2-1 ancak migration kararları sonrası açılır.

Referans: `docs/notes/faz8-p2-parity-characterization.md` (tur 9 canonical).
`docs/notes/faz8-p2-construction-feasibility.md` (P2-0A raporu).
