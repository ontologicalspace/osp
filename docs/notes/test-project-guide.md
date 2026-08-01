# OSP Test Project Guide — Completed-loop Harness (Faz 8)

Bu rehber, OSP CLI'nin Faz 8 test-project Completed-loop harness'ini nasıl
kullanacağınızı açıklar: snapshot-bound task dosyası, harness execution mode,
state-dir isolation, ve Completed-loop senaryosu.

## Genel bakış

OSP CLI iki execution mode sunar (Paper 2 harness/production ayrımı):

| Mode | Witness policy | State-dir | Task dosyası | Kullanım |
|---|---|---|---|---|
| `production` (default) | `production` (quorum=2) | CWD veya `--state-dir` | legacy hardcoded (no `--task`) | Deployment |
| `harness` | `harness-auto-approve` (quorum=0) | **repo dışı zorunlu** | **snapshot-bound zorunlu** | Controlled experiment |

Harness mode, controlled experiment için witness quorum'unu gevşetir (Completed-loop
üretir), ama bunu güvenli sınırlar içinde yapar:
- **Snapshot-bound task**: HEAD binding + NodeId→path scope binding + Node-only V1 scope
- **State-dir outside repo**: Held artifacts analyzed repo'yu dirty yapamaz
- **Clean worktree zorunlu**: analysis sırasında drift fail-closed

## Kurulum

### 1. Test repository hazırla

Completed-loop için: bir node 2+ outgoing import'a sahip olmalı (coupling > threshold),
RemoveImport proposal bir edge kaldırınca coupling ≤ threshold'a düşmeli.

Örnek topoloji: `main.rs` → `a.rs` + `b.rs` (2 outgoing imports → coupling 2/3 = 0.667).

```bash
mkdir my-test-repo && cd my-test-repo
git init -q
git config core.autocrlf false   # CRLF-driven dirty-worktree önlenir
echo 'mod a; mod b; pub fn main() { a::a(); b::b(); }' > main.rs
echo 'pub fn a() {}' > a.rs
echo 'pub fn b() {}' > b.rs
git add -A && git commit -qm "init"
```

### 2. NodeId'leri öğren

Analyzer dosyaları alfabetik sıralar: `a.rs`=0, `b.rs`=1, `main.rs`=2. `osp analyze`
çıktısından doğrulayın:

```bash
osp analyze . --format json | grep -E '"node_id"|"path"'
```

### 3. Task dosyası yaz (CliHarnessTaskFileV1)

`task.v1.json` — **repository DIŞINDA** bir path'e koyun (örn. `/tmp/task.v1.json`):

```json
{
  "schema_version": 1,
  "repository_head": "<full-40-char-SHA — git rev-parse HEAD>",
  "scope_bindings": [
    {"node_id": 2, "expected_path": "main.rs"}
  ],
  "task": {
    "id": 7,
    "milestone_id": 1,
    "label": "completed-loop fixture",
    "target_predicate_set": {
      "mode": "All",
      "predicates": [{
        "predicate": {
          "metric": "Coupling",
          "operator": "Le",
          "threshold": 0.55,
          "scope": {"Node": 2},
          "required_source": "Scip",
          "tolerance": 0.0
        },
        "weight": null
      }],
      "preferred_vector": {"x": 0.55, "y": 0.6, "z": 0.5, "w": 0.5, "v": 0.3}
    },
    "policy": {
      "predicate_failure_policy": "StrictReject",
      "min_improvement_delta": 0.02,
      "max_axis_regression": 0.15,
      "maneuver_limit": 3,
      "allow_progress_checkpoint": false
    },
    "allowed_operations": ["RemoveImport"],
    "constraints": [],
    "status": "Pending"
  }
}
```

**Kritik alanlar:**
- `repository_head`: `git rev-parse HEAD` ile alınan **full 40-char SHA** (kısa SHA reject).
- `scope_bindings[0].node_id`: predicate scope Node ID'si ile **exact-set** eşleşmeli.
- `scope_bindings[0].expected_path`: analyze çıktısındaki path ile **birebir** eşleşmeli.
- `preferred_vector`: harness task için **zorunlu** (navigation target).
- `status`: `Pending`/`Assigned`/`InProgress` (terminal `Completed`/`Blocked` reject).

### 4. Proposals JSON yaz (mock LLM)

`proposals.json` — RemoveImport proposal (coupling düşürme):

```json
[{
  "new_nodes": [],
  "new_edges": [],
  "removed_edges": [{"from": 2, "to": 1, "kind": "Imports"}],
  "affected_nodes": [2],
  "modified_entities": [],
  "position_hints": [],
  "reasoning": "remove import to reduce coupling below threshold"
}]
```

**Not:** `reasoning` boş olamaz (`OutputContract::strict()` reject eder). `from` = coupling
taşıyan node, `to` = kaldırılacak dependency.

## Çalıştırma

### Harness mode (Completed-loop)

```bash
osp trajectory attempt 7 \
  --repo ./my-test-repo \
  --execution-mode harness \
  --witness harness-auto-approve \
  --llm mock \
  --proposals /tmp/proposals.json \
  --task /tmp/task.v1.json \
  --state-dir /tmp/osp-state
```

**State-dir zorunlu** ve analyzed repo **DIŞINDA** olmalı. Aksi halde:

```
Error: --state-dir ./my-test-repo/.osp is inside the analyzed repository;
harness mode requires state-dir outside repo (Held artifacts would dirty git status)
```

### Production mode (legacy backward-compat)

```bash
osp trajectory attempt 7 \
  --repo ./my-test-repo \
  --execution-mode production \
  --witness production \
  --llm mock \
  --proposals /tmp/proposals.json
# --task opsiyonel (verilmezse legacy hardcoded coupling ≤ 0.55 task kullanılır)
# --state-dir opsiyonel (default CWD)
```

## Exit codes (INV-T9 CLI exit-code contract)

| Code | Anlam |
|---|---|
| 0 | COMPLETED — predicate satisfied, mainline applied |
| 10 | AWAITING_WITNESSES — production witness authorization bekleme (expected, hata değil) |
| 11 | REQUIRES_REVISION — explicit witness rejection |
| 12 | EXCEEDED_MANEUVER_LIMIT — ardışık reject limiti |
| 13 | REQUIRES_OPERATOR_APPROVAL — critical domain |
| 20 | WITNESS_EVALUATION_ERROR — operational fault |
| 40 | PENDING_AUTHORIZATION_PERSISTENCE_FAILURE — terminal |
| 70 | SYSTEM_FAILURE — persistence/internal error |
| 80 | TASK_NOT_FOUND |
| 90 | LLM_ERROR (NoMoreProposals, parse, network) |

## Fail-closed senaryoları (task loader)

Task dosyası yüklenirken şu kontroller fail-closed uygulanır:

| Senaryo | Beklenen |
|---|---|
| `repository_head` kısa SHA / yanlış | `RepositoryHeadMismatch` |
| `schema_version ≠ 1` | `UnsupportedSchemaVersion` |
| positional ID ≠ `task.id` | `TaskIdMismatch` |
| scope `Subgraph`/`Module` | `UnsupportedHarnessScope` |
| farklı Node ID'lerinde predicate'ler | `HeterogeneousHarnessScope` |
| `preferred_vector: null` | `MissingPreferredVector` |
| initial status `Completed`/`Blocked` | `InvalidInitialTaskStatus` |
| `maneuver_limit: 0` (core validation) | `TaskValidation` |
| scope binding path analyze'de yok | `ScopeBindingPathMismatch` |
| scope binding Node ID analyze'de yok | `UnknownScopeBindingNode` |
| duplicate/extra scope binding | `DuplicateScopeBinding` / `ScopeBindingSetMismatch` |
| malformed JSON | `Parse` |
| dirty worktree | `ensure_snapshot_eligible` reject |

## Test referansı

Bakınız: `crates/osp-cli/tests/completed_loop.rs` — 20-senaryo integration matrix
(mode-matrix guard, task-loader fail-closed, Completed-loop happy path, state-dir
invariant, fail-closed no-side-effects).
