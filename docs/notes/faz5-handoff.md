# INV-T9 #70 Faz 8-P2 — Handoff (Faz 8-P2 characterization + migration decisions TAMAM)

## Repository state

```
PR #85: MERGED (squash e4675b2) — Faz 8-P2 P2-0A+B characterization
PR #91 (#87-A): MERGED (squash efede00) — policy fixture (Part 1/2)
PR #93 (#87-B): MERGED (squash 7c670ff) — Q5 exact theta (Part 2/2, Closes #87)
PR #98: MERGED (squash f463af0) — migration decisions MD-1/MD-2/MD-3 Accepted + spec extensions

Faz 8-P2 characterization + ontolojik migration kararları TAMAM.
Issue #87 KAPANDI. Üç implementation tracking + P2-1 + Faz 8a + 2 evidence gate açık.
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

### Faz 8-P2 characterization + migration decisions — TAMAM ✓
- PR #85 (squash `e4675b2`): V1/V2 measurement characterization — 3 ontolojik boyut KANITLANDI
- PR #91 (squash `efede00`): Migration 3(b) policy/decision divergence KANITLANDI
- PR #93 (squash `7c670ff`): Q5 theta parity koşullu KANITLANDI; Issue #87 KAPANDI
- PR #98 (squash `f463af0`): MD-1/MD-2/MD-3 normative decisions Accepted + spec planned extensions

### Açık işler (10 issue/PR)

**P2-0B characterization kalan (evidence gate'ler):**
- **#86** (P2-0B.7) — MCP Workspace before-baseline characterization (Yüzey 2)
- **#88** — mixed_per_axis_sources dedicated matrisi (MD-2 evidence gate)
- **#89** — task-snapshot drift adversarial characterization (resolver A vs B)
- **#92** — subject-authority → raw → Q5 theta downstream (MD-1 cutover gate)

**MD implementation (normative decisions Accepted, implementation pending):**
- **#95** — MD-1 subject-authority caller migration (Faz 8a)
- **#96** — MD-2 provenance enforcement (engine-internal cutover, Faz 8a öncesi)
- **#97** — MD-3 baseline policy (AcceptAsColdStart + ColdStartPolicy)

**Orchestration:**
- **#99** — P2-1 measure_task_delta_checked public boundary (additive Faz 5)
- **#100** — Faz 8a engine cutover (commit_task_claim → V2 authorization consumer + cleanup)

### Deployment dependency DAG

```
Normative decisions (PR #98 MERGED)
│
├─ MD-1 (Subject):
│  #99 P2-1 additive observation → #95-A
│  → #92 drift characterization (cutover gate)
│  → #95-B Faz 8a caller cutover
│
├─ MD-2 (Provenance):  [engine-internal, MD-1'den bağımsız]
│  #88 frozen mixed-source matrix (evidence gate)
│  → #96 dual evaluation → engine-internal authority cutover (Faz 8a öncesi)
│
└─ MD-3 (Baseline Policy):  [bağımsız]
   #97 policy implementation (AcceptAsColdStart + ColdStartPolicy)
   → bağımsız ilerleyebilir

#86, #89 — bağımsız P2-0B characterization (MD'lere bağlı değil)

Faz 8a (#100) — #95-B + #96 + #97 tamamlandıktan sonra
```

### Önerilen sıradaki adım

1. **#88 (MD-2 evidence gate)** — mixed_per_axis_sources matrisi; #96 (MD-2 implementation) önkoşulu
2. **#97 (MD-3 policy)** — AcceptAsColdStart + ColdStartPolicy; bağımsız, P2-1 açmadan başlanabilir
3. **#99 (P2-1)** — measure_task_delta_checked public boundary; MD-1 compatibility producer

Kritik yol: MD-2 (#88→#96) → Faz 8a engine cutover (#100). MD-1 (#99→#95) ve MD-3 (#97) paralel.

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
