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

## Integration test isolation pattern'i

`osp trajectory attempt` integration testleri (binary'yi `assert_cmd` ile çağıran)
iki tuzağa karşı izole edilmeli. Bu bölüm, `HarnessFixture` pattern'ini
(`crates/osp-cli/tests/completed_loop.rs`) dokümante eder — gelecek integration
test yazarlarının aynı teşhis sürecini yaşamaması için.

### Tuzak 1: `.osp/` writes → repo dirty → snapshot eligibility fail

`FilesystemPendingAuthorizationStore::new(root)` `root` altında
`.osp/pending-authorizations/` dizinine Held artifact yazıyor. Production
default (`--state-dir` verilmezse) **CWD**'ye yazar.

Eğer test CWD'yi analyzed repo içine ayarlarsa (`current_dir(repo_path)`):
`.osp/` analyzed repo'ya yazılır → `git status` dirty → `ensure_snapshot_eligible`
reject → test fail. Bu bir workaround değil, **production fix**'tir (PR #104):
`--state-dir` harness mode'da zorunlu ve analyzed repo **dışında** olmalı.

### Tuzak 2: Paralel test CWD contamination

Integration testler `cargo test` ile paralel koşer. Eğer testler aynı CWD'yi
paylaşırsa, `.osp/` writes cross-test contamination'a uğrar — bir testin Held
artifact'i diğer testin state-dir'inde belirir, flaky failure'lar üretir.

### Çözüm: `HarnessFixture` — repo/work tempdir ayrımı

Her test **iki ayrı tempdir** sahibi:

```rust
struct HarnessFixture {
    repo: tempfile::TempDir,  // analyzed git repo (clean, immutable HEAD)
    work: tempfile::TempDir,  // CWD — task/proposal dosyaları + state-dir burada
    head: String,
}
```

- `repo`: analyzed git repository. Test boyunca **salt okunur** (HEAD binding
  snapshot eligibility için). `.osp/` BURAYA yazılmaz.
- `work`: CWD. Task/proposal JSON dosyaları ve `--state-dir` burada. **Analyzed
  repo DIŞINDA** → `.osp/` writes git status'ü kirletmez.
- Binary çağrısı: `cmd.current_dir(work.path())` + `--state-dir work/osp-state`

### Deterministic fixture (CRLF + commit dates)

Paralel/makine bağımsız kararlılık için iki konfigürasyon zorunlu:

1. **`core.autocrlf=false` + `core.eol=lf`**: Windows'ta CRLF normalization
   çalıştıktan sonra repo dirty görünmesin (analyzer dosya byte'larını okur,
   git index ile karşılaştırır).

2. **`GIT_AUTHOR_DATE` + `GIT_COMMITTER_DATE`**: Fixed commit tarihleri →
   kararlı SHA. Aksi halde makine saati farklılığı cross-machine/cross-run
   HEAD mismatch üretir (`RepositoryHeadMismatch`).

```rust
let commit = |args: &[&str]| {
    Command::new("git")
        .args(["-C", repo.to_str().unwrap()])
        .args(args)
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .status()
};
```

### `OSP_ATTEMPT_LOCK` defensive serialization

`trajectory attempt` Held artifact disk I/O yapar. Paralel testler aynı
`--state-dir`'e yazmasın diye `OSP_ATTEMPT_LOCK` environment variable'ı
file-lock serialization sağlar (defensive — test isolation tempdir'lerle
zaten sağlanır, ama işlem-hiyerarşisi contamination'a karşı ek güvenlik).

### Ne zaman hangi pattern?

| Test tipi | Pattern | Neden |
|---|---|---|
| `osp analyze` only | `fixture_repo()` tek tempdir (bkz. `analyze_provenance_flow.rs`) | `.osp/` yazılmaz, CWD ayrımı gereksiz |
| `osp trajectory attempt` | `HarnessFixture` (repo + work ayrımı) | `.osp/` writes → dirty worktree riski |
| `osp trajectory attempt` harness mode | `HarnessFixture` + `--state-dir work/` zorunlu | harness invariant: state-dir repo dışı |

### Anti-pattern'ler

- ❌ `current_dir(repo_path)` — `.osp/` analyzed repo'ya yazılır, dirty worktree.
- ❌ Aynı CWD'yi paylaşan paralel testler — `.osp/` cross-contamination.
- ❌ `--state-dir repo/.osp` — harness mode reject eder (production fix).
- ❌ CRLF normalization açık — Windows'ta flaky dirty-worktree failure.
- ❌ Commit tarihleri default (makine saati) — kararsız SHA, cross-machine fail.

## Test referansı

Bakınız: `crates/osp-cli/tests/completed_loop.rs` — 20-senaryo integration matrix
(mode-matrix guard, task-loader fail-closed, Completed-loop happy path, state-dir
invariant, fail-closed no-side-effects, `HarnessFixture` pattern referans
implementation). `crates/osp-cli/tests/analyze_provenance_flow.rs` — analyze-only
tek-tempdir pattern örneği.
