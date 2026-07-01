# Stage G1 — osp-mcp MCP Server Notes (Paper 2 evidence)

> **Aşama:** G1 (osp-mcp — AI access surface, MCP) — TAMAMLANDI
> **Tarih:** 2026-06-29
> **Tez:** "MCP authority DEĞİL, access layer'dır. AI agent OSP çekirdeğini INV-T1..T8
> bypass edemeden kullanır. Özellikle INV-T1'in (agent hedef koordinat göremez) canlı
> server üzerinde doğrulaması."

## Mimari

### 1. Crate `crates/osp-mcp/` (rmcp 0.8, tokio async, stdio)
Resmi Rust MCP SDK ([modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)).
`#[tool_router]` on impl block, `#[tool]` on async methods, `#[tool_handler]` on
`ServerHandler` impl. 4 tool G1 scope.

### 2. 4 agent-facing tool
- `osp_analyze_workspace` — Observation: repo snapshot (node/edge count, coverage, metrics)
- `osp_get_agent_task_view` ⭐ — Observation: INV-T1 projection (predicate + current measurement)
- `osp_check_predicate` — Validation: current position ile predicate evaluate
- `osp_submit_delta` — Execution: DeltaProposal → engine measure → PredicateGate → outcome

### 3. Standart output envelope (`osp.mcp.v1`)
Her tool ortak envelope: `{ ok, schema_version, request_id, tool, result, invariants_checked,
warnings }` veya `{ ok:false, error: { error_code, message, invariants_checked, recoverable } }`.
Deterministic error codes agent'ın self-correct etmesini sağlar.

### 4. Workspace güvenliği (startup config)
`--workspace <path>` startup'ta alınır, canonicalize + exists kontrolü. Agent raw path
veremez (path traversal önü). Analyze-once: SpaceEngine in-memory saklanır, her tool call
re-analyze ETMEZ.

## INV-T1 doğrulaması (canlı server) ⭐

**En kritik Paper 2 evidence:** INV-T1'in production server üzerinde somut doğrulaması.

### İki-katmanlı koruma
1. **Type-level:** `AgentTaskView` struct'ında `preferred_vector`/`target_region`/
   `milestone_target_vector` alanları YOK (compile-time). `InternalTaskPlan::to_agent_view`
   dönüşümü bunları düşürür (tek yönlü engine→agent).
2. **Runtime:** `McpEnvelope::assert_no_coordinate_leak()` her agent-facing tool çıktısında
   forbidden token (`preferred_vector`, `target_region`, `milestone_target_vector`) tarar.
   Leak varsa envelope `TARGET_COORDINATE_LEAK_BLOCKED` error ile değiştirilir.

### Canlı doğrulama
```
$ osp-mcp --workspace /tmp/mcp-test-repo  (stdio)
# osp_get_agent_task_view task_id=1 çağrısı → JSON response

preferred_vector:         0 occurrence (PASS)
target_region:            0 occurrence (PASS)
milestone_target_vector:  0 occurrence (PASS)

task_id:                  present (agent bildiği)
target_predicate:         present (agent bildiği)
current_measurement:      present (agent bildiği)
```

**Tez:** *Hibrit modelin epistemolojik güven katmanı (predicate) agent'a sızdırılmadan
matematiksel güç (koordinat) operator seviyesinde tutulur.* Bu test bunu production'da
somer. Paper 2 RQ5 (epistemic projection) için ham veri.

## INV koruması matrisi (G1)

| INV | Koruma | Test |
|---|---|---|
| T1 | serde-level + runtime leak scan | inv_t1_*_has_no_* (7 test) |
| T2 | mode flag (agent/operator), OperatorCapability startup-only | inv_t2_agent_mode |
| T3 | engine.compute_raw_from_delta (agent declare edemez) | check_predicate |
| T4 | ProvenancedRawPosition + MetricSource | envelope leak scan |
| T5 | TaskNotFound error code (claim task-bound değilse) | envelope |
| T6/T7/T8 | commit_task_claim (Q5.b PredicateGate + policy) | submit_delta |

## RQ6/RQ7/RQ8 etkisi (Paper 2)
- **RQ5 (epistemic projection):** MCP olmadan agent tüm repo dump alır. MCP ile
  `get_agent_task_view` + `check_predicate` → kompakt typed context. INV-T1 test bunun
  güvenli olduğunu kanıtlar (koordinat sızdırmadan).
- **RQ6 (token cost):** G2'de navigator loop MCP üzerinden → evidence ledger accumulation.
- **RQ8 (trajectory correction):** G2'de dry_run_delta → "ne olur?" simülasyon.

## G1 sınırları (G2'de gelecek)
- Tek demo task (coupling <= 0.55). Operator-only task_add G2'de.
- Single-attempt submit (LLM loop yok). Navigator.run_task G2'de.
- Engine current_measured sabit (default). Full re-measure G2'de.
- WorkspaceRegistry yok (tek workspace). Multi-workspace G2'de.

## Doğrulama
- osp-mcp: 8 unit test + 7 INV-T1 integration test, 0 fail
- Tüm workspace test pass (osp-core 278, osp-analyzer 148, osp-mcp 15)
- Canlı stdio server test (initialize + tools/list + 4 tool call) — JSON-RPC protocol OK
- INV-T1 canlı server üzerinde forbidden token yok (grep verification)

## Crate yapısı
```
crates/osp-mcp/
  Cargo.toml          — rmcp 0.8, tokio, osp-core, osp-analyzer
  README.md           — MCP client config (Claude/Cursor), INV-T1 guarantee
  src/
    lib.rs            — modül deklarasyonu
    envelope.rs       — McpEnvelope + ErrorCode + assert_no_coordinate_leak
    mode.rs           — ServerMode (Agent/Operator)
    workspace.rs      — Workspace (analyze-once, INV security)
    server.rs         — OspMcpServer (ServerHandler + 4 tools)
    main.rs           — clap CLI + tokio main + stdio serve
  tests/
    inv_t1_leak_test.rs — 7 INV-T1/INV-T2/security integration test
```
