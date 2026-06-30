# OSP MCP Server — Design Document

> **Durum:** v0.1 tasarım (implementasyon öncesi şema)
> **Tarih:** 2026-06-30
> **İlişki:** `docs/agent-trajectory-roadmap.md` §8 Aşama G (osp-mcp), `docs/invariant-spec.md`
> **Kaynak:** Beyin fırtınası (arkadaş yorumu — CLI first, MCP second) + review revizyonu

---

## 1. Tez — MCP Access Layer (Authority DEĞİL)

**6 "never" cümlesi (dokümanın kalbi — INV-T1..T8 bypass edilemez):**

```
MCP is not an authority layer.
MCP never computes architectural truth.
MCP never bypasses osp-core.
MCP never exposes target coordinates to agent-facing tools.
MCP never grants OperatorCapability from request input.
MCP exposes typed access to osp-core command handlers.
```

**MCP tool'ları free-form architectural claim KABUL ETMEZ** — typed input schema alır,
typed OSP result döndürür (INV #14 — prompt as typed data packet uyumlu). AI agent doğal
dil "mimari kuralı" veremez; sadece typed DeltaProposal/predicate üretir.

---

## 2. Ürün Mimarisi (5 katman)

```
osp-core    — authority layer (engine ölçer, INV-T1..T8)
osp-cli     — human execution surface (truth surface)
osp-mcp     — agent access surface (BU DOKÜMAN)
osp-sdk     — integration surface (LangGraph/CrewAI, Aşama H)
osp-ui/3d   — exploration surface (kanıt sonrası, Aşama E)
```

**CLI ↔ MCP paralelliği (kritik prensip):**
> *CLI and MCP expose the same osp-core command handlers. CLI is the human truth surface;
> MCP is the agent access surface. Neither bypasses osp-core.*

MCP, osp-cli'yi **subprocess ile çağırmaz** — ikisi aynı `osp-core` command handler
fonksiyonlarını kullanır. CLI insan için, MCP agent için; altta aynı deterministik çekirdek.

---

## 3. MCP Protocol + rmcp

**Resmi Rust SDK:** `rmcp` ([modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)),
tokio async runtime, stdio transport. MCP 2025-06-18 spec uyumlu.

**Server startup:**
```bash
osp-mcp --mode agent --workspace P:/repos/osp-spike/svelte
osp-mcp --mode operator --workspace P:/repos/osp-spike/svelte
```

MCP client (AI agent — Claude/Cursor/GPT) stdio üzerinden JSON-RPC ile tool çağırır.
Server `osp-core` API'sine delegate eder, INV test yapar, typed result döner.

---

## 4. Workspace Güvenliği (review)

AI agent **raw `repo_path` ALAMAZ** (path traversal riski: `/etc`, `../../another-project`).

**İki model:**

### İlk sürüm (basit)
Startup config `--workspace <path>`. Agent mevcut workspace üzerinde çalışır, raw path veremez.
Tool'lar `workspace_id` almaz — server startup'taki tek workspace kullanılır.

### Sonraki sürüm (WorkspaceRegistry)
```rust
pub struct WorkspaceRegistry { workspaces: HashMap<WorkspaceId, Workspace> }
pub struct Workspace { id: WorkspaceId, path: PathBuf, space: Option<Space> }
```
- `osp_register_workspace(path)` → operator-only (INV-T2), workspace_id döner
- Tool input `workspace_id` (raw path DEĞİL)
- Agent sadece registered workspace'ler üzerinde çalışır

**Güvenlik:** Workspace path validation (canonicalize, allowlist), sandbox escape önleme.

---

## 5. Server Mode: Agent vs Operator (INV-T2)

```
osp-mcp --mode agent     → observation + validation + policy-bound execution; operator tools DISABLED
osp-mcp --mode operator  → + trajectory_init, task_add, milestone_decompose, policy_update, register_workspace
```

**OperatorCapability startup'ta inject edilir, MCP request'ten ASLA.** Tool input'ta
`operator=true` veya `capability` alanı YOK. Agent MCP client operator tool çağıramaz
(compile-time: tool registry mode-filtered).

**Neden:** AI agent kendi `Trajectory`'sini yaratamaz, `Task` ekleyemez, `Milestone`
decompose edemez (INV-T2 — operator only). Bu tools sadece operator mode MCP client'tan
(insan veya trusted orchestrator) çalışır.

---

## 6. Tool Set (4 kategori — review revizyonu)

### 6.1 Observation (read-only — agent keşif)

#### `osp_analyze_workspace(workspace_id?)` → SpaceSnapshot
- Repo'yu analiz eder (mevcut `osp-analyzer::analyze_repo`), space snapshot döner.
- **INV:** #8 (network-free core), #4 (engine computes raw position).
- **Output:** nodes, edges, repo_metrics, semantic_coverage, node_witnesses.

#### `osp_get_node(node_id)` → Node + ProvenancedRawPosition
- Tek node + measured position (source dahil).
- **INV:** #4.
- **INV-T1 test:** preferred_vector İÇERMEZ (sadece node'un kendi measured position'ı).

#### `osp_get_agent_task_view(task_id)` → AgentTaskView ⭐ MERKEZ TOOL
- **INV-T1'in gerçek dünya testi.** Sadece şunları döndürür:
  - `task_id`, `label`
  - `current_measurement` (mevcut engine-measured durum — SERBEST)
  - `target_predicate` (AgentPredicateView — predicate'lar, mode)
  - `allowed_operations` (OpKind listesi)
  - `constraints` (RuleRef listesi)
- **ASLA döndürmez:** `preferred_vector`, `target_region`, `milestone_target_vector`,
  `internal_task_plan`, trajectory target coordinate.
- **Serde-level enforce:** `AgentTaskView` struct'ında bu alanlar yok (Aşama A, INV-T1 test).

#### `osp_get_relevant_context(task_id, k_hops)` → SpaceSlice
- Task'ın ilgili node'larının k-hop neighborhood'i (mevcut `compute_space_slice`).
- **INV:** T1 (preferred_vector sızıntısı yok), #14 (typed, not free-form).
- **Kullanım:** Agent'a kompakt context — tüm repo dump YERINE.

#### `osp_get_attempt_history(task_id)` → Vec<TrajectoryEvidence>
- Task'ın tüm attempt evidence'ları (token cost, outcome, before/after).
- **INV:** T5.
- **RQ6/RQ7:** Paper 2 evidence — agent geçmişinden öğrenir.

### 6.2 Validation (check, NO mutation)

#### `osp_bind_claim(task_id, delta_json)` → TaskBoundClaim | BindingError
- DeltaProposal → Claim (task-bound) validation. Mutation YOK.
- **INV:** T5 (Task≠Claim, binding zorunlu).
- **Output:** `Ok(TaskBoundClaim)` veya `Err(MissingTaskId | TaskNotFound)`.

#### `osp_check_predicate(task_id)` → PredicateSetResult
- **Mevcut** `claim.computed_raw` ile predicate değerlendir (yeni delta YOK).
- **INV:** T3 (engine ölçer), T4 (source).
- **Kullanım:** "Şu anki durum predicate'i karşılıyor mu?"

#### `osp_dry_run_delta(task_id, delta_json)` → DryRunAttemptOutcome ⭐
- Delta'yı **sandbox/temp** uygula → engine re-analyze → `simulated_after` → AttemptOutcome.
- **MUTATION YOK:** `committed_after = None`, `apply_target = NotApplied`.
- **INV:** T3, T4, T6 (simulated, no commit).
- **Output:**
  ```rust
  DryRunAttemptOutcome {
      measured_before: ProvenancedRawPosition,
      simulated_after: ProvenancedRawPosition,
      committed_after: None,  // dry-run — NO commit
      mutation_decision: MutationDecision,  // Reject/AcceptAsProgress/...
      apply_target: ApplyTarget::NotApplied,  // tartışmasız: no mutation
  }
  ```
- **Kullanım:** Agent delta önerir, "commit etmeden ne olur?" sorusu. check vs dry_run ayrımı:
  - `check_predicate`: mevcut position ile (delta yok)
  - `dry_run_delta`: yeni delta simüle et (no commit)

### 6.3 Execution (mutate — policy/gate zorunlu)

#### `osp_submit_delta(task_id, delta_json)` → NavigatorResult
- Navigator loop: LLM → DeltaProposal → Claim → measure → PredicateGate → commit → evidence.
- **INV:** T6 (failure≠regression), T7 (maneuver limit), T8 (progress≠merge).
- **Output:** `Completed | ExceededManeuverLimit | RequiresOperatorApproval | LlmError`.
- **Not:** Alias yok (review) — tek `submit_delta`, `trajectory_attempt` kaldırıldı.

#### `osp_record_attempt(task_id, delta_json)` → TrajectoryEvidence
- Evidence ledger yazma (kod mutation olmasa bile **state mutation** — ledger update).
- **INV:** T5, T6, T7.
- **Kullanım:** Manual attempt kaydı (agent dışı kaynak).

### 6.4 Operator-only (INV-T2 — sadece operator mode)

#### `osp_trajectory_init(vision_json)` → Trajectory
- Yeni Trajectory + VisionVector. OperatorCapability zorunlu.
- **INV:** T2 (operator only).

#### `osp_task_add(milestone_id, predicate_json, policy)` → Task
- Milestone'a Task ekle. OperatorCapability zorunlu.

#### `osp_milestone_decompose(milestone_id, strategy)` → Vec<Task>
- MilestoneDecomposer (Aşama C). OperatorCapability zorunlu.

#### `osp_policy_update(task_id, policy)` → TaskPolicy
- Task policy güncelle (maneuver limit, failure policy).

#### `osp_register_workspace(path)` → WorkspaceId
- WorkspaceRegistry'e workspace ekle (sonraki sürüm).

---

## 7. Standart Output Envelope (review)

Her tool ortak envelope döner — AI agent deterministic error code ile kendini düzeltir.

### Success
```json
{
  "ok": true,
  "schema_version": "osp.mcp.v1",
  "request_id": "req_abc123",
  "tool": "osp_check_predicate",
  "result": { "predicate_completion": "Completed" },
  "invariants_checked": ["INV-T3", "INV-T4"],
  "warnings": [],
  "evidence_ref": null
}
```

### Error
```json
{
  "ok": false,
  "schema_version": "osp.mcp.v1",
  "request_id": "req_abc123",
  "tool": "osp_get_agent_task_view",
  "error_code": "TARGET_COORDINATE_LEAK_BLOCKED",
  "message": "AgentTaskView serialization contained preferred_vector — INV-T1 violation",
  "invariants_checked": ["INV-T1"],
  "recoverable": false
}
```

**Deterministic error codes (örnek):**
- `TARGET_COORDINATE_LEAK_BLOCKED` — INV-T1 ihlali (preferred_vector sızdı)
- `PLACEHOLDER_METRIC_INSUFFICIENT` — INV-T4 (placeholder ile task kapatılamaz)
- `MANEUVER_LIMIT_EXCEEDED` — INV-T7
- `OPERATOR_CAPABILITY_REQUIRED` — INV-T2 (agent operator tool çağırdı)
- `TASK_NOT_FOUND` — binding hatası
- `WORKSPACE_NOT_REGISTERED` — workspace güvenliği

---

## 8. INV Koruma Matrisi (genişletilmiş — review)

| Tool | INV test | Kategori |
|---|---|---|
| `analyze_workspace` | INV #8, #4 | Observation |
| `get_node` | INV #4, T1 | Observation |
| `get_agent_task_view` | **INV-T1** (preferred_vector ASLA, serde-level) | Observation |
| `get_relevant_context` | INV-T1, #14 | Observation |
| `get_attempt_history` | INV-T5 | Observation |
| `bind_claim` | INV-T5 (Task≠Claim, binding) | Validation |
| `check_predicate` | INV-T3, T4 | Validation |
| `dry_run_delta` | INV-T3, T4, T6 (simulated, no commit) | Validation |
| `submit_delta` | INV-T6, T7, T8 | Execution |
| `record_attempt` | INV-T5, T6, T7 | Execution |
| `trajectory_init` | **INV-T2** (OperatorCapability) | Operator-only |
| `task_add` | INV-T2 | Operator-only |
| `milestone_decompose` | INV-T2 | Operator-only |
| `policy_update` | INV-T2 | Operator-only |
| `register_workspace` | INV-T2 | Operator-only |

**Tüm agent-facing read tools için INV-T1 snapshot testi** — preferred_vector başka tool'dan
da sızabilir (`get_relevant_context`, `get_node`, `get_attempt_history`). Her tool'un serde
çıktısında `preferred_vector`/`target_region`/`milestone_target_vector` string GEÇMEMELİ.

---

## 9. Crate Yapısı

```
crates/osp-mcp/
  Cargo.toml          — rmcp dependency, osp-core, tokio
  src/
    lib.rs            — MCP server bootstrap (rmcp ServerHandler)
    handlers.rs       — tool → osp-core command handler mapping
    envelope.rs       — standart output envelope (McpResult)
    workspace.rs      — WorkspaceRegistry (sonraki sürüm)
    mode.rs           — AgentMode vs OperatorMode filtering
  tests/
    inv_t1_snapshot.rs — tüm read tool'lar preferred_vector içermiyor
    operator_only.rs  — agent mode operator tool çağıramaz
    dry_run_no_commit.rs — dry_run_delta committed_after None
```

**osp-core command handlers (CLI ile paylaşılan):**
```rust
// crates/osp-core/src/commands.rs (yeni, Aşama F/G paylaşılan)
pub fn handle_analyze(workspace: &Workspace) -> SpaceSnapshot
pub fn handle_get_agent_task_view(task_id) -> AgentTaskView
pub fn handle_dry_run_delta(task_id, delta) -> DryRunAttemptOutcome
pub fn handle_submit_delta(task_id, delta) -> NavigatorResult
// ... CLI ve MCP aynı fonksiyonları çağırır
```

---

## 10. Implementasyon Sırası (D2 + osp-cli ile paralel)

1. **D2:** Gerçek engine measure + commit() Q5.b (navigator gerçekçi) — MCP buna bağlı
2. **osp-core/src/commands.rs:** Paylaşılan command handlers (CLI + MCP)
3. **osp-mcp crate:** rmcp server, tool → handler mapping, envelope
4. **INV-T1 snapshot testleri:** Tüm read tool'lar preferred_vector içermiyor
5. **Operator mode filtering:** Agent mode operator tool çağıramaz
6. **Workspace güvenliği:** İlk sürüm startup config, sonra WorkspaceRegistry

---

## 11. Paper 2 Evidence (MCP'den)

MCP, Paper 2 için **kontrollü agent test** sağlar:
- **RQ6 (token cost):** `submit_delta` navigator loop token accumulate → evidence ledger
- **RQ7 (task success):** `get_attempt_history` → success/fail ratio, maneuver limit hits
- **RQ8 (trajectory correction):** `dry_run_delta` → "ne olur?" simülasyon, Aşama E

MCP olmadan: agent tüm repo dump alır, RAG maliyeti patlar. MCP ile: `get_agent_task_view`
+ `get_relevant_context` → kompakt typed context (Paper 1 RQ5 token reduction'ın dinamik uzantısı).

---

## 12. Karar Günlüğü

| Tarih | Karar | Gerekçe |
|---|---|---|
| 2026-06-30 | MCP access layer (authority DEĞİL) | INV-T1..T8 bypass edilemez. 6 "never" cümlesi. |
| 2026-06-30 | rmcp (resmi Rust SDK) | modelcontextprotocol/rust-sdk, tokio async. |
| 2026-06-30 | 4 kategori (Observation/Validation/Execution/Operator) | review — operator-only ayrı kategori. |
| 2026-06-30 | Workspace güvenliği (raw path YOK) | review — path traversal riski. |
| 2026-06-30 | Agent vs Operator mode | INV-T2 — OperatorCapability request'ten ASLA. |
| 2026-06-30 | dry_run_delta (simulated, no commit) | review — check vs dry-run ayrımı. |
| 2026-06-30 | Standart output envelope | review — deterministic error code, agent self-correct. |
| 2026-06-30 | Alias yok (tek submit_delta) | review — erken alias API yüzeyini büyütür. |
| 2026-06-30 | CLI↔MCP aynı core handler (subprocess değil) | review — typed result, performans. |

---

## Kaynaklar

- [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) — resmi rmcp
- [MCP spec 2025-06-18](https://modelcontextprotocol.io/) — protocol schema
- `docs/agent-trajectory-roadmap.md` §8 Aşama G
- `docs/invariant-spec.md` INV-T1..T8
- Beyin fırtınası: arkadaş yorumu (CLI first, MCP second) + review revizyonu
