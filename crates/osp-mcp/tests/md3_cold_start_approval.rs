//! #97 MD-3 S3 — MCP cold-start onay akışı contract test.
//!
//! Uçtan uca zincir (INV-T9 extension): `submit_delta_attempt` →
//! `SuspendedColdStart` (mutation yok) → `approve_cold_start` (operatör
//! otoritesi — witness quorum değil) → `AcceptAsColdStart` + Sandbox apply +
//! `ColdStartAcceptanceEvidence`. Mainline promotion mekanizması YOK
//! (INV-T8 extension); improvement iddiası taşınmaz (INV-T6 extension).

use std::fs;
use std::sync::Arc;

use osp_mcp::workspace::Workspace;
use osp_mcp::OspMcpServer;
use tempfile::TempDir;

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

fn make_server_handle() -> Arc<std::sync::Mutex<Workspace>> {
    let dir = make_fixture_repo();
    let workspace = Workspace::analyze(dir.path(), None).expect("workspace analyze");
    let llm: Arc<dyn osp_core::navigator::LlmClient> =
        Arc::new(osp_core::navigator::MockLlmClient::new(Vec::new()));
    let server = OspMcpServer::new(workspace, osp_mcp::ServerMode::Agent, llm);
    server.workspace_handle()
}

/// Cold-start task: scope Node(10_000) base'te yok + delta ile giriyor,
/// Coupling Le -1 → daima NotCompleted, policy RequireOperatorApproval.
fn cold_start_task() -> osp_core::trajectory::Task {
    osp_core::trajectory::Task {
        id: 1,
        milestone_id: 1,
        label: "md3 cold-start MCP contract".into(),
        target_predicate_set: osp_core::trajectory::PredicateSet {
            mode: osp_core::trajectory::PredicateMode::All,
            predicates: vec![osp_core::trajectory::WeightedPredicate {
                predicate: osp_core::trajectory::MetricPredicate {
                    metric: osp_core::trajectory::PredicateAxis::Coupling,
                    operator: osp_core::trajectory::ComparisonOp::Le,
                    threshold: -1.0,
                    scope: osp_core::trajectory::PredicateScope::Node(10_000),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: None,
        },
        policy: osp_core::trajectory::TaskPolicy {
            predicate_failure_policy:
                osp_core::trajectory::PredicateFailurePolicy::AcceptImprovement,
            allow_progress_checkpoint: true,
            cold_start_policy: osp_core::trajectory::ColdStartPolicy::RequireOperatorApproval,
            ..Default::default()
        },
        allowed_operations: vec![],
        constraints: vec![],
        status: osp_core::trajectory::TaskStatus::Pending,
    }
}

/// Tek delta-introduced node (allocator sözleşmesi: 10_000+index).
fn cold_start_proposal() -> osp_core::agent::DeltaProposal {
    osp_core::agent::DeltaProposal {
        new_nodes: vec![osp_core::agent::NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 1.0,
            connected_to: vec![],
        }],
        new_edges: vec![],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "md3 cold-start MCP fixture".into(),
        ..Default::default()
    }
}

/// **Askı → onay → Sandbox:** submit SuspendedColdStart döner (mutation yok);
/// onay typed evidence ile uygulanır; ikinci onay unknown_suspension (tek kullanım).
#[test]
fn mcp_cold_start_suspends_then_operator_approval_applies_sandbox() {
    let handle = make_server_handle();
    let task = cold_start_task();
    let proposal = cold_start_proposal();

    // 1. Askı — INV-T9 extension: mutation uygulanmaz.
    let claim_id = {
        let mut ws = handle.lock().unwrap();
        let outcome = ws
            .submit_delta_attempt(&proposal, &task)
            .expect("attempt json");
        assert_eq!(outcome["commit_result"], "SuspendedColdStart", "{outcome}");
        assert_eq!(outcome["commit_state"], "cold_start_suspended");
        assert_eq!(outcome["mainline_mutation"], "not_applied");
        assert_eq!(outcome["next_action"], "operator_approval");
        outcome["claim_id"].as_u64().expect("claim_id u64")
    };

    // 2. Onay — operatör otoritesi; evidence kanıtla bağlanır.
    let mut registry = osp_core::trajectory::InMemoryTaskRegistry::new();
    registry.insert(task);
    let mut ws = handle.lock().unwrap();
    let approved = ws
        .approve_cold_start(1, claim_id, "op-alice", "APR-97-1", &registry)
        .expect("approve json");
    assert_eq!(approved["applied"], true, "{approved}");
    assert_eq!(approved["commit_result"], "AcceptAsColdStart");
    assert_eq!(approved["apply_target"], "Sandbox");
    // INV-T8 extension pin — Mainline promotion YOK.
    assert_eq!(approved["mainline_promotion"], "not_available");
    // Evidence — issue #97 8 alan wire'da.
    assert_eq!(approved["evidence"]["operator_id"], "op-alice");
    assert_eq!(approved["evidence"]["authorization_id"], "APR-97-1");
    assert_eq!(approved["evidence"]["policy"], "RequireOperatorApproval");
    assert_eq!(approved["evidence"]["improvement_claimed"], false);
    assert!(
        approved["evidence"]["subject_digest"].is_array(),
        "subject_digest typed wire (32-byte)"
    );
    assert_eq!(
        approved["evidence"]["baseline_reason"]["AllMembersIntroducedByDelta"]["members"],
        serde_json::json!([10_000])
    );

    // 3. Tek kullanım — suspension kaydı düştü.
    let again = ws
        .approve_cold_start(1, claim_id, "op-alice", "APR-97-1", &registry)
        .expect("json kanalı");
    assert_eq!(again["error"], "unknown_suspension", "{again}");
    assert_eq!(again["applied"], false);
    assert_eq!(again["retryable"], false);
}

/// **Boş kimlik fail-closed:** operator_id/authorization_id boş → typed hata,
/// motor çağrısına ULAŞMAZ (mutasyon imkânsız).
#[test]
fn mcp_cold_start_approval_rejects_empty_identities() {
    let handle = make_server_handle();
    let task = cold_start_task();
    let proposal = cold_start_proposal();
    let claim_id = {
        let mut ws = handle.lock().unwrap();
        let outcome = ws
            .submit_delta_attempt(&proposal, &task)
            .expect("attempt json");
        assert_eq!(outcome["commit_result"], "SuspendedColdStart");
        outcome["claim_id"].as_u64().expect("claim_id u64")
    };

    let registry = osp_core::trajectory::InMemoryTaskRegistry::new();
    let mut ws = handle.lock().unwrap();
    let bad = ws
        .approve_cold_start(1, claim_id, "   ", "APR-97-1", &registry)
        .expect("json kanalı");
    assert_eq!(bad["error"], "empty_operator_id", "{bad}");
    assert_eq!(bad["applied"], false);
}
