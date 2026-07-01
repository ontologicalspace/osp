//! INV-T1 ⭐ integration test — agent-facing tool outputs NEVER leak target coordinates.
//!
//! `docs/invariant-spec.md` INV-T1: "Agent'a serialize edilen view HEDEF KOORDİNAT
//! İÇERMEZ (preferred_vector / target_region / milestone_target_vector)."
//!
//! Bu test, MCP server'ın gerçek AgentTaskView + workspace snapshot serialization'ını
//! JSON string'e çevirir ve içinde forbidden token'lar GEÇMEMELİDİR. Bu, Paper 2'nin
//! epistemolojik güven tezinin somut doğrulamasıdır (Q5: "agent hedef koordinatı
//! göremez").
//!
//! **Test kapsamı:**
//! 1. `get_agent_task_view` — AgentTaskView serialization (preferred_vector YOK)
//! 2. `check_predicate` — current_measurement serialization (coordinate YOK)
//! 3. `analyze_workspace` — snapshot serialization (coordinate YOK)
//! 4. `submit_delta` — attempt outcome serialization (coordinate YOK)
//!
//! Forbidden tokens: `preferred_vector`, `target_region`, `milestone_target_vector`.

use std::fs;
use std::sync::Arc;

use osp_mcp::workspace::Workspace;
use osp_mcp::OspMcpServer;
use tempfile::TempDir;

/// Küçük fixture repo (2 dosya, Python — analyzer default_all adapter'ı ile parse).
fn make_fixture_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("main.py"),
        "from utils import helper\n\nclass App:\n    pass\n",
    )
    .unwrap();
    fs::write(dir.path().join("utils.py"), "class Helper:\n    pass\n").unwrap();
    dir
}

/// Bir success envelope'ın JSON serialization'ında forbidden token YOK mu?
fn assert_no_leak(json_str: &str, tool_name: &str) {
    let forbidden = [
        "preferred_vector",
        "target_region",
        "milestone_target_vector",
    ];
    for token in &forbidden {
        assert!(
            !json_str.contains(token),
            "INV-T1 VIOLATION: tool '{tool_name}' output leaked '{token}'. \
             Agent-facing serialization must NEVER contain target coordinates.",
        );
    }
}

/// Workspace + server kur (fixture repo ile). Agent mode (operator tools disabled).
fn make_server() -> (Arc<std::sync::Mutex<Workspace>>, OspMcpServer) {
    let dir = make_fixture_repo();
    let workspace = Workspace::analyze(dir.path(), None).expect("workspace analyze");
    let server = OspMcpServer::new(workspace, osp_mcp::ServerMode::Agent);
    let handle = server.workspace_handle();
    (handle, server)
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T1 test 1: get_agent_task_view — preferred_vector YOK
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn inv_t1_get_agent_task_view_has_no_preferred_vector() {
    let (_handle, _server) = make_server();
    // AgentTaskView'ı direkt üret (MCP server bunu serialize eder).
    // InternalTaskPlan → to_agent_view dönüşümü preferred_vector düşürür.
    use osp_core::coords::RawPosition;
    use osp_core::trajectory::{AgentPredicateView, AgentTaskView, PredicateMode};
    let view = AgentTaskView {
        task_id: 1,
        label: "Reduce coupling to 0.55".into(),
        current_measurement: RawPosition {
            x: 0.7,
            y: 0.5,
            z: 0.5,
            w: 0.5,
            v: 0.3,
        },
        target_predicate: AgentPredicateView {
            mode: PredicateMode::All,
            predicates: vec![],
        },
        allowed_operations: vec![osp_core::trajectory::OpKind::RemoveImport],
        constraints: vec![],
        feedback_history: vec![],
    };
    let json = serde_json::to_string(&view).expect("serialize");
    assert_no_leak(&json, "osp_get_agent_task_view");
    // Pozitif assertion: task_id ve label GÖRÜNÜR (agent bildiği şeyler).
    assert!(json.contains("task_id"));
    assert!(json.contains("current_measurement"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T1 test 2: workspace snapshot — coordinate YOK
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn inv_t1_workspace_snapshot_has_no_coordinates() {
    let (handle, _server) = make_server();
    let ws = handle.lock().unwrap();
    let snapshot = ws.snapshot_summary();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    assert_no_leak(&json, "osp_analyze_workspace");
    // Pozitif: node_count ve coverage GÖRÜNÜR (agent'a izin verilen metadata).
    assert!(json.contains("node_count"));
    assert!(json.contains("semantic_coverage"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T1 test 3: check_predicate measured position — coordinate YOK
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn inv_t1_current_measured_has_no_target_coordinate() {
    let (handle, _server) = make_server();
    let ws = handle.lock().unwrap();
    let measured = ws.current_measured();
    let json = serde_json::to_string(&measured).expect("serialize");
    // ProvenancedRawPosition coupling/cohesion/.../source içerir — preferred_vector YOK.
    assert_no_leak(&json, "osp_check_predicate");
    // Pozitif: measured values GÖRÜNÜR (agent nerede olduğunu bilmeli).
    assert!(json.contains("coupling"));
    assert!(json.contains("source"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T1 test 4: submit_delta attempt outcome — coordinate YOK
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn inv_t1_submit_delta_outcome_has_no_target_coordinate() {
    use osp_core::agent::{DeltaProposal, NewNodeSpec};
    use osp_core::coords::MetricSource;
    use osp_core::space::NodeKind;
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, OpKind, PredicateAxis, PredicateFailurePolicy,
        PredicateMode, PredicateScope, PredicateSet, Task, TaskPolicy, TaskStatus,
        WeightedPredicate,
    };

    let (handle, _server) = make_server();
    // Demo task (coupling <= 0.55, preferred_vector internal).
    let task = Task {
        id: 1,
        milestone_id: 1,
        label: "Reduce coupling".into(),
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
            preferred_vector: Some(osp_core::coords::RawPosition {
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
    };
    // DeltaProposal (tek node, structural only — NO positions).
    let proposal = DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: NodeKind::Module,
            initial_mass: 100.0,
            connected_to: vec![],
        }],
        new_edges: vec![],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "reduce coupling by abstracting imports".into(),
    };
    let mut ws = handle.lock().unwrap();
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 1)
        .expect("attempt");
    let json = serde_json::to_string(&outcome).expect("serialize");
    assert_no_leak(&json, "osp_submit_delta");
    // Pozitif: attempt_outcome ve mutation_decision GÖRÜNÜR (agent ne olduğunu bilmeli).
    assert!(json.contains("attempt_outcome"));
    assert!(json.contains("mutation_decision"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T2 test: agent mode'da operator capability YOK
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn inv_t2_agent_mode_allows_operator_tools_false() {
    assert!(!osp_mcp::ServerMode::Agent.allows_operator_tools());
    assert!(osp_mcp::ServerMode::Operator.allows_operator_tools());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Workspace security test: path canonicalize + exists kontrolü
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn workspace_security_rejects_nonexistent_path() {
    use osp_mcp::WorkspaceError;
    let result = Workspace::analyze(std::path::Path::new("/nonexistent/fake/path/xyz"), None);
    assert!(matches!(result, Err(WorkspaceError::PathNotFound(_))));
}

#[test]
fn workspace_security_rejects_file_not_directory() {
    use osp_mcp::WorkspaceError;
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("not_a_dir.txt");
    fs::write(&file_path, "hello").unwrap();
    let result = Workspace::analyze(&file_path, None);
    assert!(matches!(result, Err(WorkspaceError::NotADirectory(_))));
}
