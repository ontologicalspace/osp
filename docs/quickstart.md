# OSP Quickstart — First Analysis, First Accepted Proposal, First Provenance Refusal

**Time budget:** the steps below run in about **3 minutes**. The one-time
build in Step 0 takes a few minutes on first run.

**What you will do:**

1. **Analyze** a tiny Python repository — every module positioned in OSP's
   five-axis conceptual space, with per-metric provenance.
2. **Play the agent**: submit a scripted structural proposal and watch OSP
   **accept it** — a completed loop across all three decision layers.
3. **Cross the authority line**: submit a task demanding the wrong
   measurement source and watch OSP refuse it **at task-load time** —
   fail-closed on provenance, with the reason spelled out.
4. **Read the evidence:** what OSP accepted, which layer decided it, and
   what that does (and does not yet) mean for your files.

All commands and outputs below are real — captured from a live run on
2026-09-30 against the `fix/144-attempt-authority-alignment` branch (issue
[#144](https://github.com/ontologicalspace/osp/issues/144): the demo task's
measurement authority is now declared explicitly). Commands work in any
POSIX shell (Git Bash on Windows included). `osp` is `osp.exe` on Windows.

---

## Step 0 — Install (one-time)

```bash
# Rust toolchain (https://rustup.rs), then:
git clone https://github.com/ontologicalspace/osp.git
cd osp
cargo build -p osp-cli
```

You now have the `osp` binary at `target/debug/osp`. (The workspace builds 7
crates; `osp-desktop` needs Tauri system packages and is excluded from
headless builds.)

## Step 1 — First analysis (~30 s)

OSP is **snapshot-bound**: trajectory runs (Step 2) require a clean git
worktree, so the fixture gets its own git repository.

```bash
mkdir osp-quickstart && cd osp-quickstart
git init demo && cd demo

cat > main.py <<'EOF'
from utils import helper

def main():
    helper()
EOF

cat > utils.py <<'EOF'
def helper():
    return 42
EOF

git add main.py utils.py
git -c user.name="OSP Quickstart" -c user.email="quickstart@local" commit -m "fixture"
cd ..
```

(The inline identity flags make the block work on a fresh machine with no
git identity configured.)

Analyze it (adjust the path to your OSP checkout):

```bash
/path/to/osp/target/debug/osp analyze demo
```

Output (real values, abridged):

```json
{
  "node_count": 2,
  "edge_count": 1,
  "edges": [ { "from": 0, "is_type_only": false, "kind": "imports", "to": 1 } ],
  "nodes": [
    {
      "path": "main.py",
      "node_id": 0,
      "kind": "module",
      "coupling":    { "value": 0.5, "source": "tree_sitter", "confidence": 0.75, "coverage": 1.0 },
      "cohesion":    { "value": 0.5, "source": "placeholder", "confidence": 0.0,  "coverage": 0.0 },
      "instability": { "value": 1.0, "source": "tree_sitter", "confidence": 0.75, "coverage": 1.0 },
      "…": "mass, role, classification, entropy, witness-depth per node"
    }
  ],
  "repository": {
    "binding": "observed_worktree_unbound",
    "clean": true,
    "head": "…"
  }
}
```

Two things to notice:

- Every module got **coordinates** on the five axes (coupling, cohesion,
  instability, entropy, witness-depth).
- Every metric carries its **source** — `tree_sitter` for coupling here,
  `placeholder` for cohesion. In OSP, *where a number comes from* is a
  first-class citizen. That matters in Step 2.

## Step 2 — First accepted proposal (~60 s)

Now act as the coding agent — in the **harness execution mode**, which is
the supported live-development loop today (snapshot-bound task file,
auto-approved witnesses, isolated state directory).

### 2a — Declare the task (measurement predicate + authority)

```bash
cat > task.json <<EOF
{
  "schema_version": 1,
  "repository_head": "$(git -C demo rev-parse HEAD)",
  "scope_bindings": [{ "node_id": 0, "expected_path": "main.py" }],
  "task": {
    "id": 1, "milestone_id": 1, "label": "quickstart first-accept fixture",
    "target_predicate_set": {
      "mode": "All",
      "predicates": [{
        "predicate": {
          "metric": "Coupling", "operator": "Le", "threshold": 0.55,
          "scope": { "Node": 0 }, "required_source": "TreeSitter", "tolerance": 0.0
        },
        "weight": null
      }],
      "preferred_vector": { "x": 0.55, "y": 0.6, "z": 0.4, "w": 0.5, "v": 0.3 }
    },
    "policy": {
      "predicate_failure_policy": "StrictReject",
      "min_improvement_delta": 0.02, "max_axis_regression": 0.15,
      "maneuver_limit": 3, "allow_progress_checkpoint": false
    },
    "allowed_operations": ["RemoveImport"], "constraints": [], "status": "Pending"
  }
}
EOF
```

Note what the task declares: **coupling ≤ 0.55 on node 0 (`main.py`),
measured from TreeSitter** — the source that actually has authority over
coupling in this pipeline (INV-T9 #70: the topology source binds coupling
and instability provenance together). The task file is snapshot-bound: it
pins your repository's exact HEAD and the node↔path binding.

### 2b — Propose a structural change

One scripted proposal (mock LLM, no API key): remove the `main.py → utils.py`
import edge — an allowed operation aimed straight at the predicate:

```bash
cat > proposals.json <<'EOF'
[{ "new_nodes": [], "new_edges": [],
   "removed_edges": [{ "from": 0, "to": 1, "kind": "Imports" }],
   "affected_nodes": [0], "modified_entities": [], "position_hints": [],
   "reasoning": "remove import to reduce coupling" }]
EOF
```

### 2c — Attempt

```bash
mkdir -p state
/path/to/osp/target/debug/osp trajectory attempt 1 \
  --repo demo --task task.json \
  --execution-mode harness --witness harness-auto-approve \
  --state-dir state --proposals proposals.json
echo "exit code: $?"
```

Output (real):

```
✓ Task completed in 1 attempts
  Total tokens: 0
  Evidence entries: 1
[
  {
    "trajectory_id": 1, "milestone_id": 1, "task_id": 1, "attempt_id": 1,
    "before": { "x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3 },
    "after":  { "x": 0.0, "y": 0.5, "z": 0.5, "w": 0.46153846153846156, "v": 0.3496052731635571 },
    "gate_decision": "PassedAll",
    "predicate_completion": "Completed",
    "mutation_decision": "AcceptAsCompleted",
    "token_cost": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
    "duration_ms": 0
  }
]
exit code: 0
```

**What just happened — three separate decisions.** OSP reports the outcome
as three distinct layers, and the evidence shows each one:

| Decision layer | Evidence field | Observed | Meaning |
|---|---|---|---|
| Hard claim gates | `gate_decision` | `PassedAll` | syntax / vision / rule checks on the proposal all passed |
| Task predicate | `predicate_completion` | `Completed` | coupling measured 0.5 ≤ 0.55 **from TreeSitter** — the required source, so the condition was established |
| Mutation policy | `mutation_decision` | `AcceptAsCompleted` | predicate met ⇒ the proposed conceptual-space mutation was accepted |

The after-state moved measurably: `x` fell 0.7 → 0.0 (the import edge is
gone from the conceptual space). And still — nothing happened to your
files (Step 3 proves it).

## Step 2½ — First provenance refusal (~30 s)

Now cross the authority line. This task declares the same predicate but
demands it from the **wrong source**: `required_source: Scip` for coupling.
The fixture ships with the repo:

```bash
sed "s/REPLACE_WITH_YOUR_FIXTURE_REPO_HEAD_SHA/$(git -C demo rev-parse HEAD)/" \
  /path/to/osp/docs/fixtures/provenance-fail.task.json > provenance-fail.task.json

/path/to/osp/target/debug/osp trajectory attempt 1 \
  --repo demo --task provenance-fail.task.json \
  --execution-mode harness --witness harness-auto-approve \
  --state-dir state --proposals proposals.json
echo "exit code: $?"
```

Output (real):

```
Error: UnsupportedMeasurementAuthority: Coupling in the attempt pipeline is measured from tree_sitter (INV-T9 #70: the topology source binds coupling and instability provenance together — loading a SCIP index would not change this authority), but the task requires scip. Restructure the task predicate or see issue #144
exit code: 1
```

**Fail-closed on provenance, at task-load time.** Two layers of defense:

1. **This preflight** fails the task before a single proposal is measured,
   with the reason spelled out: coupling's authority is TreeSitter under the
   INV-T9 #70 contract — *loading a SCIP index would not change that*.
   ("Data was loaded from a SCIP index" ≠ "this axis's authoritative source
   is Scip".)
2. **INV-T4 at measurement time** still guards the runtime: even if a
   measured value claims the wrong source, the predicate cannot complete on
   it.

No silent fallback exists in either layer. (Cohesion has the mirror-image
status: it *is* Scip-authoritative, but the attempt pipeline runs Tier-1
analysis — a cohesion predicate with `required_source: Scip` fails this same
preflight with `ScipMeasurementUnavailable` and points you to
`osp analyze --scip`.)

## Step 3 — What the acceptance did (and did not) touch

- **The conceptual space was mutated — your files were not.** The evidence
  is the entry above: `mutation_decision: "AcceptAsCompleted"` applied the
  structural delta (edge removal) to OSP's in-memory model of your
  architecture, `x: 0.7 → 0.0`.
- **This CLI flow does not apply source-code patches.** Proposals are
  structural; translating an accepted structural delta into real code edits
  is a separate, not-yet-built layer. Even an accepted proposal did not
  rewrite `main.py`.
- **The worktree check confirms the run left no incidental writes:**

```bash
git -C demo log --oneline
git -C demo status --short
```

```
<your-fixture-sha> fixture
```

One commit — exactly what you created — and an empty `git status`. Harness
run artifacts live under `state/` (outside the repo), and the task file
pins your HEAD, so the whole decision is reproducible.

## Known boundary — the production legacy demo (for completeness)

Running without a task file (`--execution-mode production`, the legacy
hardcoded demo task) currently stops at a deeper, *known* layer: with the
authority mismatch gone, the loop now reaches witness suspension, which
requires persisted space identity — not yet implemented:

```
✗ System failure: cross-process suspension requires persisted space identity (ephemeral identity cannot survive process restart)
  Evidence entries: 0
exit code: 70
```

Tracked in [#152](https://github.com/ontologicalspace/osp/issues/152); the
harness loop above is the supported path until then.

---

## The 30-second version

- **Space:** every module has coordinates (coupling, cohesion, instability,
  entropy, witness-depth); every metric carries its source, confidence and
  coverage.
- **Task = measurement predicate + declared authority:** e.g. "coupling ≤
  0.55 on this node, from TreeSitter". A task demanding an authority the
  pipeline cannot have fails at load time — no silent fallback, ever.
- **Navigator loop:** proposal → hard claim gates → measure the after-state
  → predicate evaluation → mutation decision — three distinct layers
  (`gate_decision`, `predicate_completion`, `mutation_decision`).
- **Structural, not source:** proposals mutate the conceptual space;
  applying accepted deltas as source-code patches is a future layer.
- **Exit codes are a contract:** `0` completed, `10` awaiting witnesses,
  `11` requires revision, `12` maneuver limit exceeded, `13` operator
  approval required, `14` cold-start approval required, `70` system failure
  (full list: `crates/osp-cli/src/commands/mod.rs`, `exit_codes` module).

## Next steps

- **Analyze your own repository** — README → *Quick Start*.
- **Real cohesion (Tier 2):** generate a SCIP index (Python/Rust/Go via
  Docker, TypeScript via npm) — README → *Generating SCIP Indices*.
- **Live-use program:** the dogfood roadmap — Live Contract, CLI dogfood on
  a real project, self-hosting, then MCP — is tracked in
  [#151](https://github.com/ontologicalspace/osp/issues/151).
- **Run OSP as an MCP server** for Claude / Cursor agents — README → *Run
  the MCP Server*, or [`crates/osp-mcp/README.md`](../crates/osp-mcp/README.md).
- **Deeper:** [`docs/spec/formalism.md`](spec/formalism.md) (mathematical
  model), [`docs/STATUS.md`](STATUS.md) (project status).
