# osp-mcp — Ontological Space Protocol MCP Server

> **Agent access surface.** MCP is not an authority layer — it exposes typed access to
> `osp-core` command handlers. INV-T1..T8 are **bypass-proof**.

The OSP MCP server lets AI agents (Claude, Cursor, GPT) interact with the Ontological
Space Protocol — software architecture analysis + trajectory navigation — over the
[Model Context Protocol](https://modelcontextprotocol.io/). Agents can observe the
architectural space, check measurement predicates, and submit structural deltas; they
**never** see target coordinates, cannot create trajectories, and cannot bypass the
deterministic engine gates.

## Build

```bash
cargo build -p osp-mcp --release
# Binary: target/release/osp-mcp
```

## Run

```bash
# Agent mode (default — operator tools disabled)
osp-mcp --workspace /path/to/repo

# Operator mode (operator tools enabled — human/trusted orchestrator)
osp-mcp --mode operator --workspace /path/to/repo

# With SCIP index (real LCOM4 cohesion)
osp-mcp --workspace /path/to/repo --scip /path/to/index.scip
```

The server speaks JSON-RPC over stdio (MCP 2024-11-05 protocol version).

## MCP Client Configuration

### Claude Desktop / Claude Code (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "osp": {
      "command": "/absolute/path/to/osp-mcp",
      "args": ["--workspace", "/path/to/your/repo"]
    }
  }
}
```

### Cursor (`.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "osp": {
      "command": "/absolute/path/to/osp-mcp",
      "args": ["--workspace", "/path/to/your/repo"]
    }
  }
}
```

> Use `--mode operator` to enable trajectory/task management tools (for human-driven
> architecture planning). Default `agent` mode is the safe read+execute surface.

## Tools (Aşama G1)

| Tool | Category | INV | Description |
|---|---|---|---|
| `osp_analyze_workspace` | Observation | #4, #8 | Repo → space snapshot (node/edge count, coverage) |
| `osp_get_agent_task_view` ⭐ | Observation | **T1** | INV-T1 epistemic projection — predicate + current measurement, **NEVER** target coordinates |
| `osp_check_predicate` | Validation | T3, T4 | Evaluate predicate against current engine-measured position |
| `osp_submit_delta` | Execution | T6, T7, T8 | DeltaProposal → engine measure → PredicateGate → mutation decision |

## INV-T1 guarantee (the core thesis)

The agent-facing `osp_get_agent_task_view` output is verified at two levels:

1. **Type-level:** `AgentTaskView` struct has no `preferred_vector` / `target_region` /
   `milestone_target_vector` fields (compile-time enforcement).
2. **Runtime:** Every agent-facing tool output passes through
   `McpEnvelope::assert_no_coordinate_leak()` — a string scan that replaces the envelope
   with a `TARGET_COORDINATE_LEAK_BLOCKED` error if a forbidden token appears.

Verified on the live server: `preferred_vector`, `target_region`, and
`milestone_target_vector` are **absent** from every agent-facing tool response.

## Output envelope (`osp.mcp.v1`)

Every tool returns a standard envelope so agents can self-correct using deterministic
error codes:

```json
// Success
{
  "ok": "true",
  "schema_version": "osp.mcp.v1",
  "request_id": "req_0",
  "tool": "osp_get_agent_task_view",
  "result": { "task_id": 1, "target_predicate": { ... } },
  "invariants_checked": ["INV-T1"],
  "warnings": []
}

// Error (deterministic code)
{
  "ok": "false",
  "schema_version": "osp.mcp.v1",
  "request_id": "req_1",
  "tool": "osp_submit_delta",
  "error": {
    "error_code": "TASK_NOT_FOUND",
    "message": "task 99 not found in registry",
    "invariants_checked": ["INV-T5"],
    "recoverable": true
  }
}
```

## Testing

```bash
cargo test -p osp-mcp
# 8 unit tests + 7 INV-T1 integration tests
```

The INV-T1 integration test (`tests/inv_t1_leak_test.rs`) serializes every agent-facing
tool's output and asserts no forbidden coordinate tokens leak — this is Paper 2's
epistemological security thesis made concrete.

## Design document

See [`docs/mcp-design.md`](../../docs/mcp-design.md) for the full design:
6 "never" principles, tool categories, INV protection matrix, output envelope spec.
