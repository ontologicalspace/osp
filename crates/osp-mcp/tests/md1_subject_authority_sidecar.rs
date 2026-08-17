//! #95 MD-1 P2-1 — MCP `submit_delta_attempt` additive sidecar contract test.
//!
//! **Kabul kriteri (plan v6 §5-12):** yeni response'tan yalnız `subject_authority_drift`
//! çıkarıldığında mevcut (pre-P2-1) response ile exact parity — mevcut alan semantiği
//! (legacy `RejectedBySyntax` error JSON davranışı dahil) bu PR'da DEĞİŞMEZ; yalnız
//! surviving yollara sidecar eklenir.
//!
//! Held yolu e2e pinlenir (Production-empty witness → Held). Q4/Q5/Q6 retryable
//! error yollarının path→finalize eşlemesi `osp_core::subject_authority::
//! v1_downstream_from_engine_commit_error` ile tek noktada pinlidir (osp-core
//! testleri); MCP generic-Err branch bu shared helper'ı compose eder.

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

/// Held response'unun pre-P2-1 key set'i — additive contract'ın "eski JSON aynen"
/// kısmı bu kümeyle pinlenir (yeni response = bu küme ∪ {subject_authority_drift}).
const HELD_RESPONSE_LEGACY_KEYS: &[&str] = &[
    "commit_result",
    "witness_hold_reason",
    "witness_snapshot",
    "commit_state",
    "mainline_mutation",
    "measured_after",
    "next_action",
];

#[test]
fn md1_held_response_carries_drift_sidecar_additively() {
    let dir = make_fixture_repo();
    let workspace = Workspace::analyze(dir.path(), None).expect("workspace analyze");
    let llm: Arc<dyn osp_core::navigator::LlmClient> =
        Arc::new(osp_core::navigator::MockLlmClient::new(Vec::new()));
    let server = OspMcpServer::new(workspace, osp_mcp::ServerMode::Agent, llm);
    let handle = server.workspace_handle();

    // inv_t1 fixture mirror — demo task (coupling <= 0.55, Node(0), Scip).
    let task = osp_core::trajectory::Task {
        id: 1,
        milestone_id: 1,
        label: "Reduce coupling".into(),
        target_predicate_set: osp_core::trajectory::PredicateSet {
            mode: osp_core::trajectory::PredicateMode::All,
            predicates: vec![osp_core::trajectory::WeightedPredicate {
                predicate: osp_core::trajectory::MetricPredicate {
                    metric: osp_core::trajectory::PredicateAxis::Coupling,
                    operator: osp_core::trajectory::ComparisonOp::Le,
                    threshold: 0.55,
                    scope: osp_core::trajectory::PredicateScope::Node(0),
                    required_source: Some(osp_core::coords::MetricSource::Scip),
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
        policy: osp_core::trajectory::TaskPolicy {
            maneuver_limit: 5,
            predicate_failure_policy: osp_core::trajectory::PredicateFailurePolicy::StrictReject,
            ..Default::default()
        },
        allowed_operations: vec![osp_core::trajectory::OpKind::RemoveImport],
        constraints: vec![],
        status: osp_core::trajectory::TaskStatus::Pending,
    };
    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![osp_core::agent::NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 100.0,
            connected_to: vec![],
        }],
        new_edges: vec![],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "reduce coupling by abstracting imports".into(),
        ..Default::default()
    };

    let mut ws = handle.lock().unwrap();
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 1)
        .expect("attempt");

    // Held yüzeyi (inv_t1 mirror: boş witness set → Held).
    let commit_result = outcome
        .get("commit_result")
        .and_then(|v| v.as_str())
        .expect("commit_result present");
    assert_eq!(commit_result, "Held", "fixture must reach Held surface");

    // Sidecar mevcut ve typed observation'a deserialize olur.
    let sidecar = outcome
        .get("subject_authority_drift")
        .expect("Held = comparison-surviving → MCP response sidecar taşır");
    let obs: osp_core::subject_authority::SubjectAuthorityDriftObservation =
        serde_json::from_value(sidecar.clone()).expect("sidecar deserializes to typed observation");
    assert_eq!(obs.task_id, task.id);

    // V1 lane legacy compatibility subject'i taşır (affected+removed boş → []).
    assert!(obs.v1.subject.ids.is_empty());
    assert_eq!(
        obs.v1.raw.sources,
        [osp_core::coords::MetricSource::Scip; 5]
    );

    // **Additive contract:** sidecar çıkarılınca kalan key seti pre-P2-1 ile exact.
    let mut stripped = outcome.clone();
    stripped
        .as_object_mut()
        .expect("response is an object")
        .remove("subject_authority_drift");
    let mut stripped_keys: Vec<&str> = stripped
        .as_object()
        .expect("stripped response is an object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    stripped_keys.sort_unstable();
    let mut expected_keys = HELD_RESPONSE_LEGACY_KEYS.to_vec();
    expected_keys.sort_unstable();
    assert_eq!(
        stripped_keys, expected_keys,
        "new response − subject_authority_drift == pre-P2-1 key set (mevcut alan semantiği değişmez)"
    );
}
