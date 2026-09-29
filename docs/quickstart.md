# OSP Quickstart — First Analysis, First Rejected Proposal

**Time budget:** the three steps below run in about **2 minutes**. The one-time
build in Step 0 takes a few minutes on first run.

**What you will do:**

1. **Analyze** a tiny Python repository — every module positioned in OSP's
   five-axis conceptual space, with per-metric provenance.
2. **Play the agent**: submit scripted change proposals and watch OSP **reject
   every one of them** — fail-closed on measurement provenance, not on numbers.
3. **Read the evidence:** what OSP refused, which decision layer refused it,
   and what that does (and does not yet) mean for your files.

All commands and outputs below are real — captured from a live run against
OSP `main` on 2026-09-30 (commit `fea02f5`). Commands work in any POSIX shell
(Git Bash on Windows included). `osp` is `osp.exe` on Windows.

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
    "head": "c23cec594c38f28891acc61aad00c9274d8b1937"
  }
}
```

Two things to notice:

- Every module got **coordinates** on the five axes (coupling, cohesion,
  instability, entropy, witness-depth).
- Every metric carries its **source** — `tree_sitter` for coupling here,
  `placeholder` for cohesion. In OSP, *where a number comes from* is a
  first-class citizen. That matters in the next step.

## Step 2 — First rejected proposal (~60 s)

Now act as the coding agent. You will submit five identical scripted
proposals — "add a new module connected to node 0" — through a **mock LLM**
(no API key needed):

```bash
cat > proposals.json <<'EOF'
[
  { "new_nodes": [ { "kind": "Module", "initial_mass": 100.0, "connected_to": [[0, "Imports"]] } ],
    "new_edges": [], "modified_entities": [], "position_hints": [],
    "reasoning": "add module connected to node 0" },
  { "new_nodes": [ { "kind": "Module", "initial_mass": 100.0, "connected_to": [[0, "Imports"]] } ],
    "new_edges": [], "modified_entities": [], "position_hints": [],
    "reasoning": "add module connected to node 0" },
  { "new_nodes": [ { "kind": "Module", "initial_mass": 100.0, "connected_to": [[0, "Imports"]] } ],
    "new_edges": [], "modified_entities": [], "position_hints": [],
    "reasoning": "add module connected to node 0" },
  { "new_nodes": [ { "kind": "Module", "initial_mass": 100.0, "connected_to": [[0, "Imports"]] } ],
    "new_edges": [], "modified_entities": [], "position_hints": [],
    "reasoning": "add module connected to node 0" },
  { "new_nodes": [ { "kind": "Module", "initial_mass": 100.0, "connected_to": [[0, "Imports"]] } ],
    "new_edges": [], "modified_entities": [], "position_hints": [],
    "reasoning": "add module connected to node 0" }
]
EOF

/path/to/osp/target/debug/osp trajectory attempt 1 --repo demo --proposals proposals.json
echo "exit code: $?"
```

Output (real, first of five entries shown; all five identical in the fields
that matter):

```
✗ Maneuver limit exceeded after 5 attempts
  Evidence entries: 5
[
  {
    "trajectory_id": 1,
    "milestone_id": 1,
    "task_id": 1,
    "attempt_id": 1,
    "before": { "x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3 },
    "after":  { "x": 0.5, "y": 0.5, "z": 0.5, "w": 0.46153846153846156, "v": 0.3496052731635571 },
    "gate_decision": "PassedAll",
    "predicate_completion": "NotCompleted",
    "mutation_decision": "Reject",
    "token_cost": { … }
  },
  … 4 more entries, every one with "mutation_decision": "Reject"
]
exit code: 12
```

**What just happened — three separate decisions.** OSP deliberately reports
the outcome as three distinct layers, and the evidence shows each one:

| Decision layer | Evidence field | Observed | Meaning |
|---|---|---|---|
| Hard claim gates | `gate_decision` | `PassedAll` | syntax / vision / rule checks on the proposal all passed |
| Task predicate | `predicate_completion` | `NotCompleted` | the task's measurement condition could not be established |
| Mutation policy | `mutation_decision` | `Reject` | predicate unmet + StrictReject policy ⇒ the proposed conceptual-space mutation was refused |

So this run is **not** a hard-gate rejection — the gates passed. It is a
**fail-closed predicate rejection**: the proposal died at the predicate
layer, and the mutation policy turned that into a refusal.

Task 1 is the built-in demo task (defined in
[`crates/osp-cli/src/commands/mod.rs`](../crates/osp-cli/src/commands/mod.rs)):
it demands `coupling ≤ 0.55` on node 0 (`main.py`) **with SCIP provenance**
(`required_source: Scip`). The `trajectory attempt` path currently performs
Tier-1 analysis only — it accepts no SCIP index — so the after-state
measurement carries a `tree_sitter` coupling, and OSP **fails closed on
provenance**: a predicate cannot complete on a measurement from the wrong
source, no matter what the number says. (Verified with an isolated-module
proposal that leaves node 0 numerically untouched — still `NotCompleted`.)
Through this CLI path the demo task is therefore **structurally
unsatisfiable today** — that gap and the possible fix (`--scip` wiring) are
tracked in [#144](https://github.com/ontologicalspace/osp/issues/144).

The agent retried five times (the default maneuver limit — INV-T7 caps
agent-correctable retries), was refused five times, and OSP ended the run
with exit code **12** (`EXCEEDED_MANEUVER_LIMIT`, part of the stable CLI
exit-code contract).

## Step 3 — What the refusal did (and did not) touch

- **The conceptual space was not mutated.** `mutation_decision: "Reject"` is
  the evidence: OSP refused to apply the proposed change to its in-memory
  model of your architecture. That refusal is the OSP guarantee in action.
- **This CLI flow does not apply source-code patches.** Proposals are
  structural (nodes and edges in the conceptual space); translating an
  accepted structural delta into real code edits is a separate, not-yet-built
  layer. Even an accepted proposal would not have rewritten `main.py`.
- **The worktree check confirms the run left no incidental writes:**

```bash
git -C demo log --oneline
git -C demo status --short
```

```
c23cec5 fixture
```

One commit — exactly what you created — and an empty `git status`. The
honest summary: five proposals in, five refusals out, zero accepted
conceptual-space mutations, zero filesystem changes.

---

## The 30-second version

- **Space:** every module has coordinates (coupling, cohesion, instability,
  entropy, witness-depth); every metric carries its source, confidence and
  coverage.
- **Task = measurement predicate:** work is expressed as conditions on the
  space (e.g. "coupling ≤ 0.55 on this node, measured by SCIP"), not as text.
- **Navigator loop:** proposal → hard claim gates → measure the after-state →
  predicate evaluation → mutation decision — three distinct layers
  (`gate_decision`, `predicate_completion`, `mutation_decision`).
- **Structural, not source:** proposals mutate the conceptual space; applying
  accepted deltas as source-code patches is a future layer.
- **Exit codes are a contract:** `0` completed, `10` awaiting witnesses,
  `11` requires revision, `12` maneuver limit exceeded, `13` operator
  approval required, `14` cold-start approval required (full list:
  `crates/osp-cli/src/commands/mod.rs`, `exit_codes` module).

## Next steps

- **Analyze your own repository** — README → *Quick Start*.
- **Real cohesion (Tier 2):** generate a SCIP index (Python/Rust/Go via
  Docker, TypeScript via npm) — README → *Generating SCIP Indices*.
- **Run OSP as an MCP server** for Claude / Cursor agents — README → *Run
  the MCP Server*, or [`crates/osp-mcp/README.md`](../crates/osp-mcp/README.md).
- **Deeper:** [`docs/spec/formalism.md`](spec/formalism.md) (mathematical
  model), [`docs/STATUS.md`](STATUS.md) (project status).
