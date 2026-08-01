# Faz 8 test-project — Completed loop harness handoff

Tarih: 2026-08-01
Branch: `faz8-test-project/completed-loop` (main'den, merge edilmemiş)
Plan: 8 revizyon turu sonunda APPROVE (9.8/10) — `docs/notes/` içinde plan history.

## Oturum özeti

### Tamamlanan işler

1. **PR #102 MERGED** (squash `bd425ed`) — Faz 8-P2 #88 MD-2 evidence contract (frozen corpus).
   - İki evidence family (16 case): DirectPerAxisAuthority + MixedPerAxisSources.
   - Measured-subject digest V1 (tek canonicalization authority).
   - 3 review turu sonunda APPROVE; tüm P0/P1 kapandı.
   - Issue #88 KAPANDI (completed).

2. **OSP production-readiness analizi** — test projesinde Completed loop için 2 P0 blocker tespiti:
   - **P0-1**: Navigator `witness_policy: Production` hardcoded → ilk claim hep `Held` → `AwaitingWitnesses`. `HarnessAutoApprove` CLI'dan set edilemiyordu.
   - **P0-2**: Task `commands/mod.rs`'de hardcoded (coupling ≤ 0.55, Node(0)). `osp analyze` NodeId'leri göstermiyordu.

3. **Completed loop planı** — 8 revizyon turu, APPROVE (9.8/10). Önemli kararlar:
   - Snapshot-bound controlled harness (HEAD + NodeId→path binding).
   - Task-canonical subject authority (agent `affected_nodes` authority üretmez).
   - Typed execution/witness mode guard (harness-auto-approve yalnız execution=harness).
   - Node-only CLI harness V1 (Subgraph core kapasitesi korunur).

4. **Navigator native migration denendi → digest divergence keşfi → #96'ya taşındı.**
   - Navigator `measure_task_delta` (native per-axis provenance) migration'ı authorization/persistence reload'da digest divergence üretti.
   - `inv_t9_72_held_production_path_exact` (FilesystemStore reload) fail etti — `authorization basis digest mismatch`.
   - NullStore Held test geçti → sorun reload/verify zincirinde.
   - **Karar**: Navigator native migration + singleton fast-path bütünüyle **#96'ya (MD-2)** taşındı. Bu PR V1 compatibility harness olarak etiketlendi.

### Commit durumu (branch `faz8-test-project/completed-loop`)

```
6b67074 feat(cli): typed execution/witness modes + guard (V1 harness foundation)
1e0f590 Revert "fix(measurement): preserve bit parity for singleton centroid (review v7 Commit A)"
021bd5f fix(measurement): preserve bit parity for singleton centroid (review v7 Commit A)  ← REVERTED
bd425ed (main) INV-T9 #88: Faz 8-P2 — mixed_per_axis_sources MD-2 evidence contract (#102)
```

**Not**: `021bd5f` (singleton fast-path) revert edildi (`1e0f590`). Commit A navigator migration'ın önkoşuluydu; navigator #96'ya gidince Commit A da gitti. Bu commit'ler PR diff'inden temizlenmeli (squash/rebase sırasında revert pair kaldırılabilir).

### Foundation (commit `6b67074` — V1 harness)

- `CliExecutionMode` / `CliWitnessMode` typed ValueEnum (Default=Production).
- `validate_execution_witness_combination` guard (harness-auto-approve → execution=harness zorunlu).
- `TrajectoryAttemptArgs`: `--execution-mode`, `--witness`, `--task`, `--format`, `--maneuver-limit` (Option).
- `run_navigator` witness_policy artık `args.witness`'a göre (hardcoded Production DEĞİL).

### Yarı-tamam modüller (working tree'de DEĞİL — `6b67074`'te yok)

- `crates/osp-cli/src/commands/repo_snapshot.rs` yazıldı ama commit edilmedi (oturum sonu working tree temizlendi). **Yeniden yazılmalı.**
  - `GitCommitId` (40-char validated newtype), `RepositorySnapshot` (head + tracked_paths + clean),
    `ensure_snapshot_eligible` (clean worktree), `validate_analyzed_paths_tracked`, pre/post drift fence.

## Kalan işler (sıradaki oturum)

### Commit B-1: RepositorySnapshot + task file (V1 harness)

1. **`commands/repo_snapshot.rs`** (yeniden yaz — yukarıdaki tasarım):
   - `GitCommitId::try_from(String)` — 40-char hex validated, `unknown`/short reject.
   - `RepositorySnapshot::capture(repo)` — `git rev-parse HEAD` + `git ls-tree -r --name-only HEAD` + `git status --porcelain`.
   - `ensure_snapshot_eligible` — clean worktree zorunlu (review P0-3).
   - `validate_analyzed_paths_tracked` — node_paths ⊆ tracked_paths (review P1-2).
   - pre/post drift fence: `before == after` (RepositoryChangedDuringAnalysis fail-closed).

2. **`commands/harness_task.rs`** (yeni):
   - `CliHarnessTaskFileV1` (serde): `schema_version: 1`, `repository_head: String` (full SHA), `scope_bindings: Vec<CliScopeBinding>`, `task: osp_core::trajectory::Task`.
   - `CliScopeBinding`: `node_id: u64`, `expected_path: String`.
   - Load validate sırası (review): deserialize → schema==1 → repo HEAD kontrol → clean worktree → analyzed-path⊆tracked → scope binding (NodeId→path) → task ID consistency (args.task_id == task.id) → maneuver override → `task.validate()` → **harness profile** (preferred_vector gerekli, non-terminal initial status) → **`validate_harness_scope` Node-only** → registry insert.
   - `validate_harness_scope`: tüm predicates aynı `PredicateScope::Node(id)`; Subgraph/module → `UnsupportedHarnessScope`; heterogeneous → `HeterogeneousHarnessScope`.
   - Dokümantasyon: "Bu dosya production task genesis yerine geçmez; controlled harness trusted operator fixture."

3. **`run_trajectory_attempt`** wiring:
   - RepositorySnapshot pre-capture + `ensure_snapshot_eligible`.
   - `--task` verilirse `CliHarnessTaskFileV1` load + validate; task registry'ye insert.
   - `--task` yoksa hardcoded legacy task (backward-compat).
   - RepositorySnapshot post-capture + drift fence.
   - analyzed-path⊆tracked validation.

### Commit B-2: CliMetricSource DTO + analyze provenance output (V1 honest metadata)

4. **`CliMetricSource` / `CliAxisMeasurement`** (serde DTO — core `MetricSource` serialization'a bağlanmaz):
   ```rust
   #[derive(Serialize)]
   #[serde(rename_all = "snake_case")]
   enum CliMetricSource { TreeSitter, Scip, Placeholder, Heuristic, Mixed }
   ```
   - Golden test: `["tree_sitter","scip","placeholder","heuristic","mixed"]` exact.

5. **`run_analyze` output** genişletme:
   - `schema_version: 1`, `repository.head` (full SHA), `nodes` (NodeId ascending + path + kind + classification + role + mass + per-axis `CliAxisMeasurement`).
   - `node_paths` + `module_metrics` kullanılır; missing path fail-closed.
   - Golden snapshot (deterministic Git commit ile stable).

6. **V1 honest metadata** (review v7 karar — navigator #96'ya taşındı):
   - CLI envelope `measurement_authority: "legacy_projected"`, `provenance_native: false`.
   - Legacy V1 path uniform `Scip` projection kullanır; native TreeSitter provenance gösterilmez (dürüst).
   - `--format json` stdout'a yalnız JSON; security warning + diagnostics stderr'e.

### Commit B-3: Fixture + integration tests

7. **`fixtures/completed-loop/`** (Model B — review P1-3):
   ```
   Cargo.toml, src/main.rs, src/a.rs, src/b.rs   # analyzed subject
   task.template.json                              # runtime materialize (snapshot dışı)
   proposals.json                                  # frozen mock (RemoveImport)
   ```
   - Topoloji: target node → A → B; proposal bir edge kaldırır → predicate satisfied.
   - Integration test: tempdir → git init → deterministic commit (fixed dates) → full SHA → `osp analyze` → stable NodeId/path assert → runtime `task.v1.json` materialize → `osp trajectory attempt --execution-mode harness --witness harness-auto-approve`.

8. **`crates/osp-cli/tests/completed_loop.rs`** (integration test matrisi):
   | Senaryo | Beklenen |
   |---|---|
   | harness + valid task + valid proposal | exit 0 / Completed |
   | production + witness | Completed değil (AwaitingWitnesses veya SystemFailure) |
   | invalid witness enum | clap exit 2 |
   | malformed JSON | config error |
   | valid JSON, invalid task semantics | typed task error (`Task::validate()`) |
   | positional ID ≠ task.id | fail-closed |
   | repo HEAD mismatch | fail-closed |
   | dirty worktree | fail-closed |
   | node path binding mismatch | fail-closed |
   | extra/duplicate/empty scope binding | fail-closed |
   | Subgraph harness task | UnsupportedHarnessScope preflight |
   | preferred_vector=null | MissingPreferredVector |
   | initial Completed status | InvalidInitialTaskStatus |
   | repository değişimi analysis sırasında | RepositoryChangedDuringAnalysis |
   | analyze nodes | ID sıralı, path + provenance mevcut |
   | `--format json` | stdout JSON + stderr diagnostics; JSON deserialize |

### Commit C: docs

9. **`docs/notes/test-project-guide.md`** — Rust test projesi kurulumu + task JSON örneği + Completed loop.
10. **`docs/notes/faz8-p2-migration-decisions.md`** — MD-2 navigator migration #96 deferral notu (full cutover acceptance criteria).

### PR hazırlığı

- Commit A revert pair (`021bd5f` + `1e0f590`) squash sırasında temizlenmeli.
- `cargo test -p osp-cli` CI'ya eklenmeli.
- PR body: "V1 compatibility Completed-loop harness" + #96 navigator migration deferral.

## Issue'lar (bu oturumda açıldı/yönetildi)

- **#88** KAPANDI (PR #102 merged).
- **#96** (MD-2 implementation) — navigator native migration + singleton fast-path + digest parity bu issue'ya bağlandı. Acceptance criteria'ya eklenmeli: "Native EngineMeasurement → Held authorization → Filesystem persist → process-independent reload → exact authorization basis digest parity → exact 5-axis value bits → exact 5-axis source tags."
- **Yeni issue**: Navigator digest divergence characterization (bu oturumda keşfedildi, #96 alt-bulgu — ayrı issue açıldı).

## Önemli teknik notlar

### Navigator digest divergence (keşif — #96 kapsamı)

- Navigator `measure_task_delta` (native per-axis provenance) kullanınca `inv_t9_72_held_production_path_exact` (FilesystemStore reload) fail etti.
- Hata: `authorization basis digest mismatch — artifact may be tampered or corrupted`.
- NullStore Held test geçti → sorun reload/verify zincirinde (in-memory üretim değil).
- Kök neden hipotezi: `TaskCommitInput` (legacy: dış target/loss_before/measured) ile `EngineMeasurement` token (native: before+after+context+request) iki authority modeli yarı yolda birleşince persistence wire representation divergence üretiyor.
- Tam çözüm: `TaskCommitInput → EngineMeasurement` migration + authorization basis native measurement + persistence wire version + reload verification → MD-2 cutover (#96).
- **Bu PR'da yapılmamalı** — V1 path korundu, navigator migration #96'ya.

### Singleton centroid bit-parity (Commit A — revert edildi)

- `measured_centroid_in_session` + `compute_raw_from_delta` tek üyeli subject için gereksiz `mass*value/mass` yapıyordu → 1 ULP fark.
- Singleton fast-path: direkt `measured_position_of` / `raw_position_of` delegasyon → bit-identical.
- Characterization test yazıldı (singleton ↔ direct bit-parity).
- **Ancak**: `compute_raw_from_delta` fast-path V1 authorization basis digest'i değiştirdi → `inv_t9_72_held_production_path_exact` reload mismatch.
- Navigator migration #96'ya gidince Commit A da gereksizleşti → revert edildi.
- **#96'da**: singleton fast-path `measured_centroid_in_session`'a geri eklenebilir (navigator fence için); `compute_raw_from_delta` fast-path V1 digest'i etkilediği için dikkatli.

### Plan history (8 revizyon)

Plan 8 tur sonunda APPROVE aldı. Önemli kararlar:
- v1: CLI flag + analyze node görünürlüğü.
- v2 (review): harness/production ayrımı + snapshot-bound task.
- v3-v4: navigator context task'tan türet + provenance-aware.
- v5 (review): task-canonical subject + Node-only harness V1.
- v6 (review): boş affected_nodes backward-compat + typed conditional baseline fence.
- v7 (APPROVE): last_outcome + CLI provenance DTO + baseline fence — implementation sırasında digest divergence keşfi.
- **Son karar**: navigator native migration → #96, V1 compatibility harness bu PR.

## Out of scope (ayrı issue'lar)

- Operator witness inject (production policy release) — INV-T9 witness model.
- CLI state kalıcılığı — snapshot-bound task dosyası subject drift'i kapatır.
- osp-desktop fix (#80) — mekanik 2 satır.
- MD-1 subject-authority caller migration (#95).
- MD-3 baseline policy (#97).
- MD-2 implementation (#96) — navigator native migration + digest parity.
