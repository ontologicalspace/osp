# OSP Quickstart — First Analysis, First Gate Rejection

**Time budget:** the three steps below run in about **2 minutes**. The one-time
build in Step 0 takes a few minutes on first run.

**What you will do:**

1. **Analyze** a tiny Python repository — every module positioned in OSP's
   five-axis conceptual space, with per-metric provenance.
2. **Play the agent**: submit scripted change proposals and watch OSP's
   deterministic gate **reject** every one of them.
3. **Prove it:** the gate never touched your repository.

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

git add main.py utils.py && git commit -m "fixture"
cd ..
```

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

## Step 2 — First gate rejection (~60 s)

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

**What just happened?** Task 1 is the built-in demo task (defined in
[`crates/osp-cli/src/commands/mod.rs`](../crates/osp-cli/src/commands/mod.rs)):
it demands `coupling ≤ 0.55` on node 0 (`main.py`) **with SCIP provenance**.
The quickstart runs Tier-1 analysis only, so the after-state measurement
carries a `tree_sitter` coupling — and OSP **fails closed on provenance**:
the predicate cannot complete on a measurement from the wrong source, so the
mutation is rejected no matter what the number says. (This was verified with
an isolated-module proposal that leaves node 0 numerically untouched — still
`NotCompleted`.)

The agent retried five times (the default maneuver limit — INV-T7 caps
agent-correctable retries), the gate rejected five times, and OSP ended the
run with exit code **12** (`EXCEEDED_MANEUVER_LIMIT`, part of the stable CLI
exit-code contract). With a Tier-2 SCIP index the same predicate becomes
evaluable — see the README section *Generating SCIP Indices*.

## Step 3 — Nothing was mutated

The whole point of OSP: gates run **before** mutation.

```bash
git -C demo log --oneline
git -C demo status --short
```

```
c23cec5 fixture
```

One commit — exactly what you created. Five proposals went in, zero mutations
came out; the working tree is clean. An OSP-gated agent cannot mutate your
mainline until its proposal survives the deterministic gates.

---

## The 30-second version

- **Space:** every module has coordinates (coupling, cohesion, instability,
  entropy, witness-depth); every metric carries its source, confidence and
  coverage.
- **Task = measurement predicate:** work is expressed as conditions on the
  space (e.g. "coupling ≤ 0.55 on this node, measured by SCIP"), not as text.
- **Navigator loop:** proposal → claim gates → measure the after-state →
  predicate evaluation → mutation decision. Rejected proposals never reach
  the repository.
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
