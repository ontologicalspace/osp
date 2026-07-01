//! OSP MCP Server — rmcp `ServerHandler` implementation (`docs/mcp-design.md` §6).
//!
//! 4 initial tools (Aşama G1):
//! - `osp_analyze_workspace` — Observation: repo snapshot (nodes, edges, coverage)
//! - `osp_get_agent_task_view` ⭐ — Observation: INV-T1 projection (NO coordinates)
//! - `osp_check_predicate` — Validation: current predicate status
//! - `osp_submit_delta` — Execution: navigator single attempt (DeltaProposal → gate)
//!
//! ## INV-T1 leak protection
//! Her agent-facing tool çıktısı `McpEnvelope::assert_no_coordinate_leak()`'ten geçer.
//! `preferred_vector`/`target_region`/`milestone_target_vector` string geçerse envelope
//! `TargetCoordinateLeakBlocked` ile değiştirilir (panic-level MCP bug).

use std::sync::Arc;

use osp_core::agent::DeltaProposal;
use osp_core::coords::MetricSource;
use osp_core::navigator::{build_claim_from_proposal, provenanced_from_raw};
use osp_core::trajectory::{
    InMemoryTaskRegistry, InternalTaskPlan, OperatorCapability, PredicateSetResult,
    ProvenancedRawPosition, Task, TaskId, TaskResolver,
};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo, ToolsCapability};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::Mutex as AsyncMutex;

use crate::envelope::{EnvelopeError, ErrorCode, McpEnvelope};
use crate::mode::ServerMode;
use crate::workspace::{SharedWorkspace, Workspace};

// ═══════════════════════════════════════════════════════════════════════════════
// Tool input schemas (schemars JsonSchema — rmcp agent'a JSON Schema gösterir)
// ═══════════════════════════════════════════════════════════════════════════════

/// `osp_analyze_workspace` input. Agent path VEREMEZ — workspace startup'ta alınır.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeWorkspaceInput {
    /// workspace_id gelecekte (WorkspaceRegistry). Şimdilik yok — tek workspace.
    #[serde(default)]
    pub workspace_id: Option<String>,
}

/// `osp_get_agent_task_view` input. INV-T1 ⭐ merkez tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetAgentTaskViewInput {
    /// Task ID (registry'de lookup).
    pub task_id: u64,
}

/// `osp_check_predicate` input.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CheckPredicateInput {
    /// Task ID — predicate task'a bağlı (INV-T5).
    pub task_id: u64,
}

/// `osp_submit_delta` input. DeltaProposal JSON (structural only — NO positions, inv #4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubmitDeltaInput {
    /// Task ID (navigator bu task için çalışır).
    pub task_id: u64,
    /// DeltaProposal JSON (new_nodes, new_edges, modified_entities, reasoning).
    /// Pozisyon YOK — engine ölçer (INV-T4).
    pub delta_json: JsonValue,
}

// ═══════════════════════════════════════════════════════════════════════════════
// OspMcpServer — rmcp ServerHandler + tool router
// ═══════════════════════════════════════════════════════════════════════════════

/// OSP MCP server. `SharedWorkspace` + `TaskRegistry` + `ServerMode` taşır.
///
/// **INV-T2:** `operator_capability: None` (agent mode) veya `Some` (operator mode).
/// Agent mode'da operator tool'lar çağrılamaz (`allows_operator_tools()` false).
///
/// **State:** Workspace analyze-once, registry startup'ta demo task ile kurulu.
pub struct OspMcpServer {
    /// Analyze edilmiş workspace (Arc<Mutex> — async-safe).
    workspace: SharedWorkspace,
    /// Task registry (InMemory — agent task view / submit delta için).
    registry: Arc<AsyncMutex<InMemoryTaskRegistry>>,
    /// Server mode (agent/operator).
    mode: ServerMode,
    /// OperatorCapability (operator mode'da Some — INV-T2). Request'ten ASLA.
    #[allow(dead_code)]
    operator_capability: Option<OperatorCapability>,
    /// Tool router (#[tool_handler] bu field'ı kullanır: `self.tool_router`).
    tool_router: ToolRouter<OspMcpServer>,
}

impl OspMcpServer {
    /// Yeni server kur (startup'ta main.rs çağırır).
    pub fn new(workspace: Workspace, mode: ServerMode) -> Self {
        let operator_capability = if mode.allows_operator_tools() {
            Some(OperatorCapability::issue())
        } else {
            None
        };
        let mut registry = InMemoryTaskRegistry::new();
        // G1: default demo task ekle (coupling <= 0.55). Operator-only task_add Aşama G2'de.
        registry.insert(default_demo_task());
        Self {
            workspace: Arc::new(std::sync::Mutex::new(workspace)),
            registry: Arc::new(AsyncMutex::new(registry)),
            mode,
            operator_capability,
            tool_router: Self::get_tool_router(),
        }
    }

    /// Server mode referansı (main.rs serve için).
    pub fn mode(&self) -> ServerMode {
        self.mode
    }

    /// Shared workspace clone (test için).
    pub fn workspace_handle(&self) -> SharedWorkspace {
        Arc::clone(&self.workspace)
    }
}

// ── Tool handler'ları (#[tool_router] on impl block, #[tool] on methods) ──────
//
// Not: `#[tool_router(router = get_tool_router)]` generated function'ı yeniden adlandırır.
// Bu sayede struct field `tool_router` (#[tool_handler] default) ile collision olmaz.

#[tool_router(router = get_tool_router)]
impl OspMcpServer {
    /// `osp_analyze_workspace` — repo snapshot döndür (node/edge count, coverage).
    ///
    /// **INV:** #8 (network-free core), #4 (engine computes raw position).
    #[tool(
        name = "osp_analyze_workspace",
        description = "Analyze the workspace and return a space snapshot (node count, edge count, repo metrics, SCIP coverage). The workspace is fixed at server startup — no path input from the agent."
    )]
    async fn osp_analyze_workspace(
        &self,
        _input: Parameters<AnalyzeWorkspaceInput>,
    ) -> Result<String, String> {
        let summary = {
            let ws = self.workspace.lock().map_err(|e| e.to_string())?;
            ws.snapshot_summary()
        };
        let envelope = McpEnvelope::success(
            "osp_analyze_workspace",
            summary,
            vec!["INV-#4".into(), "INV-#8".into()],
        );
        let checked = envelope.assert_no_coordinate_leak("osp_analyze_workspace");
        serde_json::to_string(&checked).map_err(|e| e.to_string())
    }

    /// `osp_get_agent_task_view` ⭐ — INV-T1 merkez tool.
    ///
    /// Agent'a sadece predicate + current measurement + allowed_ops döner.
    /// preferred_vector / target_region / milestone_target_vector ASLA.
    #[tool(
        name = "osp_get_agent_task_view",
        description = "Get the AgentTaskView for a task — INV-T1 epistemic projection. Returns task_id, label, current_measurement, target_predicate, allowed_operations, constraints. NEVER returns target coordinates (preferred_vector, target_region, milestone_target_vector)."
    )]
    async fn osp_get_agent_task_view(
        &self,
        Parameters(input): Parameters<GetAgentTaskViewInput>,
    ) -> Result<String, String> {
        // 1. Task resolve.
        let task = {
            let reg = self.registry.lock().await;
            match reg.resolve(input.task_id) {
                Some(t) => t.clone(),
                None => {
                    let env = McpEnvelope::error(
                        "osp_get_agent_task_view",
                        EnvelopeError::new(
                            ErrorCode::TaskNotFound,
                            format!("task {} not found in registry", input.task_id),
                        ),
                    );
                    return serde_json::to_string(&env).map_err(|e| e.to_string());
                }
            }
        };
        // 2. Current measurement (engine-measured).
        let current_raw = {
            let ws = self.workspace.lock().map_err(|e| e.to_string())?;
            ws.current_raw()
        };
        // 3. InternalTaskPlan → AgentTaskView (INV-T1 — koordinat düşürülür).
        let target_vector = task
            .target_predicate_set
            .preferred_vector
            .unwrap_or_default();
        let plan = InternalTaskPlan {
            task_id: task.id,
            milestone_target_vector: target_vector,
            task_predicate: task.target_predicate_set.clone(),
            tolerance: 0.02,
        };
        let agent_view = plan.to_agent_view(
            &task.label,
            current_raw,
            task.allowed_operations.clone(),
            task.constraints.clone(),
            Vec::new(),
        );
        // 4. Serialize + INV-T1 leak check.
        let view_json = serde_json::to_value(&agent_view).map_err(|e| e.to_string())?;
        let envelope =
            McpEnvelope::success("osp_get_agent_task_view", view_json, vec!["INV-T1".into()]);
        let checked = envelope.assert_no_coordinate_leak("osp_get_agent_task_view");
        serde_json::to_string(&checked).map_err(|e| e.to_string())
    }

    /// `osp_check_predicate` — mevcut position ile predicate değerlendir.
    #[tool(
        name = "osp_check_predicate",
        description = "Evaluate the task's predicate against the CURRENT engine-measured position (no delta). Returns predicate_completion: Completed / SourceInsufficient / NotCompleted."
    )]
    async fn osp_check_predicate(
        &self,
        Parameters(input): Parameters<CheckPredicateInput>,
    ) -> Result<String, String> {
        let task = {
            let reg = self.registry.lock().await;
            match reg.resolve(input.task_id) {
                Some(t) => t.clone(),
                None => {
                    let env = McpEnvelope::error(
                        "osp_check_predicate",
                        EnvelopeError::new(
                            ErrorCode::TaskNotFound,
                            format!("task {} not found", input.task_id),
                        ),
                    );
                    return serde_json::to_string(&env).map_err(|e| e.to_string());
                }
            }
        };
        let (measured, completion) = {
            let ws = self.workspace.lock().map_err(|e| e.to_string())?;
            let m = ws.current_measured();
            let result = task.target_predicate_set.evaluate_completion(&m);
            (m, result)
        };
        let completion_str = match completion {
            PredicateSetResult::Completed => "Completed",
            PredicateSetResult::SourceInsufficient => "SourceInsufficient",
            PredicateSetResult::NotCompleted => "NotCompleted",
        };
        let measured_json = serde_json::to_value(&measured).map_err(|e| e.to_string())?;
        let envelope = McpEnvelope::success(
            "osp_check_predicate",
            serde_json::json!({
                "predicate_completion": completion_str,
                "current_measurement": measured_json,
            }),
            vec!["INV-T3".into(), "INV-T4".into()],
        );
        let checked = envelope.assert_no_coordinate_leak("osp_check_predicate");
        serde_json::to_string(&checked).map_err(|e| e.to_string())
    }

    /// `osp_submit_delta` — DeltaProposal → engine measure → PredicateGate → outcome.
    ///
    /// **INV:** T6 (failure≠regression), T7 (maneuver limit), T8 (progress≠merge).
    #[tool(
        name = "osp_submit_delta",
        description = "Submit a DeltaProposal (structural-only, NO positions) for a task. Engine measures the simulated-after position, PredicateGate evaluates, returns mutation decision. INV-T6/T7/T8 enforced."
    )]
    async fn osp_submit_delta(
        &self,
        Parameters(input): Parameters<SubmitDeltaInput>,
    ) -> Result<String, String> {
        // 1. Parse DeltaProposal (Q4 syntax).
        let proposal: DeltaProposal = match serde_json::from_value(input.delta_json.clone()) {
            Ok(p) => p,
            Err(e) => {
                let env = McpEnvelope::error(
                    "osp_submit_delta",
                    EnvelopeError::new(
                        ErrorCode::InvalidDeltaProposal,
                        format!("DeltaProposal parse failed: {e}"),
                    ),
                );
                return serde_json::to_string(&env).map_err(|e| e.to_string());
            }
        };
        // 2. Task resolve.
        let task = {
            let reg = self.registry.lock().await;
            match reg.resolve(input.task_id) {
                Some(t) => t.clone(),
                None => {
                    let env = McpEnvelope::error(
                        "osp_submit_delta",
                        EnvelopeError::new(
                            ErrorCode::TaskNotFound,
                            format!("task {} not found", input.task_id),
                        ),
                    );
                    return serde_json::to_string(&env).map_err(|e| e.to_string());
                }
            }
        };
        // 3. Single-attempt submit (engine measure + PredicateGate).
        let outcome_json = {
            let mut ws = self.workspace.lock().map_err(|e| e.to_string())?;
            ws.submit_delta_attempt(&proposal, &task, input.task_id)
                .map_err(|e| e)?
        };
        let envelope = McpEnvelope::success(
            "osp_submit_delta",
            outcome_json,
            vec!["INV-T6".into(), "INV-T7".into(), "INV-T8".into()],
        );
        let checked = envelope.assert_no_coordinate_leak("osp_submit_delta");
        serde_json::to_string(&checked).map_err(|e| e.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ServerHandler trait impl (#[tool_handler] default: self.tool_router field)
// ═══════════════════════════════════════════════════════════════════════════════

#[tool_handler]
impl ServerHandler for OspMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "osp-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            instructions: Some(format!(
                "OSP MCP Server — Ontological Space Protocol agent access surface. \
                 Mode: {}. Tools: osp_analyze_workspace, osp_get_agent_task_view, \
                 osp_check_predicate, osp_submit_delta. INV-T1..T8 enforced — \
                 target coordinates never exposed to agents.",
                self.mode.as_str()
            )),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Workspace helpers (MCP-specific ops on osp-core engine)
// ═══════════════════════════════════════════════════════════════════════════════

impl Workspace {
    /// Mevcut engine-measured RawPosition.
    /// G1: default measured position (osp-cli ile uyumlu). Engine full re-measure G2'de.
    pub fn current_raw(&self) -> osp_core::coords::RawPosition {
        osp_core::coords::RawPosition {
            x: 0.7,
            y: 0.5,
            z: 0.5,
            w: 0.5,
            v: 0.3,
        }
    }

    /// Mevcut ProvenancedRawPosition (INV-T4 source ile).
    pub fn current_measured(&self) -> ProvenancedRawPosition {
        provenanced_from_raw(self.current_raw(), MetricSource::Scip)
    }

    /// Tek DeltaProposal'ı değerlendir (single attempt — no LLM loop).
    ///
    /// **Akış:** proposal → engine.compute_raw_from_delta (measure) → Claim build →
    /// commit_task_claim → AttemptOutcome. INV-T6/T7/T8 gate içinde enforced.
    pub fn submit_delta_attempt(
        &mut self,
        proposal: &DeltaProposal,
        task: &Task,
        task_id: TaskId,
    ) -> Result<JsonValue, String> {
        use osp_core::space::{Edge, Node, NodeId};
        use osp_core::witness::WitnessSet;

        // Empty proposal check.
        if proposal.new_nodes.is_empty() && proposal.new_edges.is_empty() {
            return Ok(serde_json::json!({
                "attempt_outcome": {
                    "gate_decision": "RejectedBySyntax",
                    "predicate_completion": "NotCompleted",
                    "mutation_decision": "Reject",
                    "witness_status": null,
                },
                "apply_target": "NotApplied",
                "loss_after": null,
                "measured_after": null,
                "message": "DeltaProposal has no nodes/edges",
            }));
        }

        // 1. DeltaProposal → delta_nodes + delta_edges.
        let delta_nodes: Vec<Node> = proposal
            .new_nodes
            .iter()
            .enumerate()
            .map(|(i, spec)| Node {
                id: (10_000 + i as NodeId),
                kind: spec.kind,
                mass: spec.initial_mass,
                ..Default::default()
            })
            .collect();
        let mut delta_edges: Vec<Edge> = proposal
            .new_edges
            .iter()
            .map(|spec| Edge {
                from: spec.from,
                to: spec.to,
                kind: spec.kind,
                is_type_only: false,
            })
            .collect();
        for (i, spec) in proposal.new_nodes.iter().enumerate() {
            let node_id = delta_nodes[i].id;
            for (target, kind) in &spec.connected_to {
                delta_edges.push(Edge {
                    from: node_id,
                    to: *target,
                    kind: *kind,
                    is_type_only: false,
                });
            }
        }

        // 2. Engine measure (INV-T3 — agent değiştiremez).
        let computed_raw = self
            .engine_mut()
            .compute_raw_from_delta(&delta_nodes, &delta_edges);

        // 3. Claim build + commit_task_claim.
        let claim = match build_claim_from_proposal(&proposal, computed_raw, task_id, 1, 1) {
            Ok(c) => c,
            Err(e) => {
                return Ok(serde_json::json!({
                    "attempt_outcome": {
                        "gate_decision": "RejectedBySyntax",
                        "predicate_completion": "NotCompleted",
                        "mutation_decision": "Reject",
                        "witness_status": null,
                    },
                    "apply_target": "NotApplied",
                    "loss_after": null,
                    "measured_after": null,
                    "message": format!("claim build: {e}"),
                }))
            }
        };
        let measured = provenanced_from_raw(claim.computed_raw, MetricSource::Scip);
        let target = task
            .target_predicate_set
            .preferred_vector
            .unwrap_or_default();
        let loss_before = osp_core::trajectory::trajectory_loss(&self.current_measured(), &target);
        let omega = WitnessSet::new(Vec::new());
        let mut tmp_reg = InMemoryTaskRegistry::new();
        tmp_reg.insert(task.clone());
        let result = match self
            .engine_mut()
            .commit_task_claim(osp_core::engine::TaskCommitInput {
                claim: &claim,
                omega: &omega,
                task_resolver: &tmp_reg as &dyn TaskResolver,
                target,
                loss_before,
                measured: measured.clone(),
            }) {
            Ok(r) => r,
            Err(e) => {
                return Ok(serde_json::json!({
                    "attempt_outcome": {
                        "gate_decision": "RejectedBySyntax",
                        "predicate_completion": "NotCompleted",
                        "mutation_decision": "Reject",
                        "witness_status": null,
                    },
                    "apply_target": "NotApplied",
                    "loss_after": null,
                    "measured_after": null,
                    "message": format!("commit_task_claim: {e}"),
                }))
            }
        };

        // 4. Serialize outcome.
        let gate_str = match result.outcome.gate_decision {
            osp_core::trajectory::GateDecision::PassedAll => "PassedAll",
            osp_core::trajectory::GateDecision::RejectedBySyntax => "RejectedBySyntax",
            osp_core::trajectory::GateDecision::RejectedByVision => "RejectedByVision",
            osp_core::trajectory::GateDecision::RejectedByRule => "RejectedByRule",
            osp_core::trajectory::GateDecision::BlockedByManeuverLimit => "BlockedByManeuverLimit",
        };
        let pred_str = match result.outcome.predicate_completion {
            osp_core::trajectory::PredicateCompletion::Completed => "Completed",
            osp_core::trajectory::PredicateCompletion::NotCompleted => "NotCompleted",
        };
        let mut_str = match result.outcome.mutation_decision {
            osp_core::trajectory::MutationDecision::Reject => "Reject",
            osp_core::trajectory::MutationDecision::AcceptAsProgress => "AcceptAsProgress",
            osp_core::trajectory::MutationDecision::AcceptAsCompleted => "AcceptAsCompleted",
            osp_core::trajectory::MutationDecision::RequireOperatorApproval => {
                "RequireOperatorApproval"
            }
        };
        let apply_str = match result.apply_target {
            osp_core::trajectory::ApplyTarget::NotApplied => "NotApplied",
            osp_core::trajectory::ApplyTarget::Lane(lane) => match lane {
                osp_core::trajectory::CommitLane::Mainline => "Mainline",
                osp_core::trajectory::CommitLane::TrajectoryCheckpoint => "TrajectoryCheckpoint",
                osp_core::trajectory::CommitLane::Sandbox => "Sandbox",
            },
        };
        Ok(serde_json::json!({
            "attempt_outcome": {
                "gate_decision": gate_str,
                "predicate_completion": pred_str,
                "mutation_decision": mut_str,
                "witness_status": null,
            },
            "apply_target": apply_str,
            "loss_after": result.loss_after,
            "measured_after": serde_json::to_value(&measured).map_err(|e| e.to_string())?,
        }))
    }
}

/// Demo task — G1 için coupling <= 0.55 predicate (operator-only task_add Aşama G2'de).
fn default_demo_task() -> Task {
    use osp_core::coords::{MetricSource, RawPosition};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, OpKind, PredicateAxis, PredicateFailurePolicy,
        PredicateMode, PredicateScope, PredicateSet, TaskPolicy, TaskStatus, WeightedPredicate,
    };
    Task {
        id: 1,
        milestone_id: 1,
        label: "Reduce coupling to 0.55 (demo task)".into(),
        target_predicate_set: PredicateSet {
            mode: PredicateMode::All,
            predicates: vec![WeightedPredicate {
                predicate: MetricPredicate {
                    metric: PredicateAxis::Coupling,
                    operator: ComparisonOp::Le,
                    threshold: 0.55,
                    scope: PredicateScope::Node(0),
                    required_source: Some(MetricSource::Scip),
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: Some(RawPosition {
                x: 0.55,
                y: 0.6,
                z: 0.4,
                w: 0.5,
                v: 0.3,
            }),
        },
        policy: TaskPolicy {
            maneuver_limit: 5,
            predicate_failure_policy: PredicateFailurePolicy::StrictReject,
            ..Default::default()
        },
        allowed_operations: vec![OpKind::RemoveImport],
        constraints: vec![],
        status: TaskStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::McpEnvelope;

    #[test]
    fn envelope_error_codes_round_trip() {
        let err = EnvelopeError::new(ErrorCode::TaskNotFound, "task 99 not found");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("TASK_NOT_FOUND"));
        assert!(json.contains("INV-T5"));
    }

    #[test]
    fn success_envelope_no_leak_passes() {
        let env = McpEnvelope::success(
            "osp_get_agent_task_view",
            serde_json::json!({ "task_id": 1, "label": "clean" }),
            vec!["INV-T1".into()],
        );
        let checked = env.assert_no_coordinate_leak("osp_get_agent_task_view");
        assert!(matches!(checked, McpEnvelope::Success { .. }));
    }

    #[test]
    fn success_envelope_with_leak_blocked() {
        let env = McpEnvelope::success(
            "osp_get_agent_task_view",
            serde_json::json!({ "target_region": { "x": 0.5 } }),
            vec![],
        );
        let checked = env.assert_no_coordinate_leak("osp_get_agent_task_view");
        match checked {
            McpEnvelope::Error { error, .. } => {
                assert_eq!(error.error_code, ErrorCode::TargetCoordinateLeakBlocked);
            }
            _ => panic!("should have blocked"),
        }
    }
}
