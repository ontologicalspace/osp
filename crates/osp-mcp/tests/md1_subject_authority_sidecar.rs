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

/// Pre-P2-1 generic error response key set'i (retryable commit error branch).
const ERROR_RESPONSE_LEGACY_KEYS: &[&str] = &[
    "attempt_outcome",
    "apply_target",
    "loss_after",
    "measured_after",
    "message",
];

fn make_server_handle() -> std::sync::Arc<std::sync::Mutex<Workspace>> {
    let dir = make_fixture_repo();
    let workspace = Workspace::analyze(dir.path(), None).expect("workspace analyze");
    let llm: Arc<dyn osp_core::navigator::LlmClient> =
        Arc::new(osp_core::navigator::MockLlmClient::new(Vec::new()));
    let server = OspMcpServer::new(workspace, osp_mcp::ServerMode::Agent, llm);
    server.workspace_handle()
}

fn assert_error_response_additive_contract(
    outcome: &serde_json::Value,
) -> osp_core::subject_authority::SubjectAuthorityDriftObservation {
    // Sidecar mevcut ve typed observation'a deserialize olur.
    let sidecar = outcome
        .get("subject_authority_drift")
        .expect("surviving retryable error → sidecar mevcut");
    let obs: osp_core::subject_authority::SubjectAuthorityDriftObservation =
        serde_json::from_value(sidecar.clone()).expect("sidecar deserializes to typed observation");

    // Additive contract: sidecar çıkarılınca kalan key seti pre-P2-1 ile exact.
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
    let mut expected = ERROR_RESPONSE_LEGACY_KEYS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        stripped_keys, expected,
        "error response − subject_authority_drift == pre-P2-1 key set (legacy JSON semantiği değişmez)"
    );
    obs
}

/// **EK review P2-3:** MCP generic-Err branch'inin retryable composition'u e2e —
/// Q4 `SyntaxViolation` (self-import) → sidecar `NotReached{Q4SyntaxRejection}`.
#[test]
fn md1_q4_syntax_violation_response_carries_not_reached_sidecar() {
    let handle = make_server_handle();
    let task = md1_trivially_satisfied_task();

    // Self-import (new node id = 10_000) → engine Q4 SyntaxViolation (retryable).
    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![osp_core::agent::NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 100.0,
            connected_to: vec![],
        }],
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 10_000,
            to: 10_000,
            kind: osp_core::space::EdgeKind::Imports,
        }],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "self-import → Q4".into(),
        ..Default::default()
    };

    let mut ws = handle.lock().unwrap();
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 1)
        .expect("attempt");

    // Legacy error JSON yüzeyi (bilinçli olarak değişmez).
    assert_eq!(
        outcome["attempt_outcome"]["gate_decision"], "RejectedBySyntax",
        "legacy error JSON semantiği korunur"
    );
    let obs = assert_error_response_additive_contract(&outcome);
    assert_eq!(
        obs.v1.downstream,
        osp_core::subject_authority::V1DownstreamObservation::NotReached {
            reason: osp_core::subject_authority::V1DownstreamNotReachedReason::Q4SyntaxRejection,
        }
    );
}

/// Test-only Q6 rule — her değerlendirmede ihlal üretir (Q4'ün geçtiği delta ile
/// Q6 yüzeyine ulaşmak için).
struct AlwaysViolateRule {
    id: String,
}

impl osp_core::rule::Rule for AlwaysViolateRule {
    fn id(&self) -> &osp_core::rule::RuleId {
        &self.id
    }
    fn descriptor(&self) -> osp_core::authorization::RuleDescriptor {
        osp_core::authorization::RuleDescriptor {
            rule_id: self.id.clone(),
            semantics_version: 1,
            canonical_parameters: vec![],
        }
    }
    fn evaluate(
        &self,
        _new_nodes: &[osp_core::space::Node],
        _new_edges: &[osp_core::space::Edge],
        _space: &osp_core::space::Space,
    ) -> Option<osp_core::rule::RuleViolation> {
        Some(osp_core::rule::RuleViolation {
            rule_id: self.id.clone(),
            detail: "md1 mcp sentinel — always violates".to_string(),
            severity: osp_core::rule::RuleSeverity::Hard,
        })
    }
}

/// Trivially-satisfied task — predicate Completed → mutation != Reject → Q6 çalışır.
fn md1_trivially_satisfied_task() -> osp_core::trajectory::Task {
    osp_core::trajectory::Task {
        id: 1,
        milestone_id: 1,
        label: "md1 trivially satisfied".into(),
        target_predicate_set: osp_core::trajectory::PredicateSet {
            mode: osp_core::trajectory::PredicateMode::All,
            predicates: vec![osp_core::trajectory::WeightedPredicate {
                predicate: osp_core::trajectory::MetricPredicate {
                    metric: osp_core::trajectory::PredicateAxis::Coupling,
                    operator: osp_core::trajectory::ComparisonOp::Le,
                    threshold: 10.0,
                    scope: osp_core::trajectory::PredicateScope::Node(0),
                    required_source: None,
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
            predicate_failure_policy:
                osp_core::trajectory::PredicateFailurePolicy::AcceptImprovement,
            ..Default::default()
        },
        allowed_operations: vec![],
        constraints: vec![],
        status: osp_core::trajectory::TaskStatus::Pending,
    }
}

/// **EK review P2-3:** Q6 `RuleViolation` e2e → sidecar
/// `ReachedButUnavailable{Q6RuleViolationAfterPredicateGate}`.
#[test]
fn md1_q6_rule_violation_response_carries_reached_but_unavailable_sidecar() {
    let handle = make_server_handle();
    let task = md1_trivially_satisfied_task();

    // Q6 sentinel rule kaydet (production register_rule API).
    {
        let mut ws = handle.lock().unwrap();
        ws.engine_mut()
            .register_rule(Box::new(AlwaysViolateRule {
                id: "test.md1_mcp_always_violate".to_string(),
            }))
            .expect("rule registration");
    }

    // Role-bearing Module node → Q5 yüzeyi açık (002 pattern); Q4 geçer.
    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![osp_core::agent::NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 100.0,
            connected_to: vec![],
        }],
        new_edges: vec![],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "abstract module → Q6 sentinel".into(),
        ..Default::default()
    };

    let mut ws = handle.lock().unwrap();
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 1)
        .expect("attempt");

    // Q6 yüzeyine ulaşıldığını doğrula (message'da rule id görünür).
    let message = outcome["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("md1_mcp_always_violate") || message.contains("rule"),
        "fixture must reach Q6 RuleViolation surface, got message: {message}"
    );
    let obs = assert_error_response_additive_contract(&outcome);
    assert_eq!(
        obs.v1.downstream,
        osp_core::subject_authority::V1DownstreamObservation::ReachedButUnavailable {
            reason:
                osp_core::subject_authority::V1DownstreamUnavailableReason::Q6RuleViolationAfterPredicateGate,
        }
    );
}
