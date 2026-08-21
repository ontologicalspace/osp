//! #96 MD-2 P1-1 (PR review tur 5) — MCP native failure ontology contract test.
//!
//! **Kapanan bulgu:** MCP native measurement/binding/TCB failure'larını
//! `gate_decision: "RejectedBySyntax"` JSON'u ile yayıyordu — navigator aynı
//! gerçeklik için `NavigatorResult::SystemFailure` dönerken. Ortak typed mapper
//! (`osp_core::task_measurement::{MeasurementFailureDisposition::agent_surface,
//! commit_error_agent_surface}`) ile tekilleştirildi.
//!
//! **Wire sözleşmesi (fabrication YOK — gözlenmeyen gate kararı gözlenmiş gibi
//! sunulmaz):**
//! - Gerçek structural Q4 (draft aşaması) → `RejectedBySyntax` attempt_outcome
//!   (agent delta şeklini düzeltir; md1_subject_authority_sidecar test'inde pinli).
//! - Native measurement failure (terminal disposition) → `system_failure` JSON
//!   (class + typed disposition + retryable=false); `attempt_outcome` YOK.
//! - Retryable commit error (Syntax/Vision/Rule) → attempt_outcome ile GERÇEK
//!   gate kararı (hardcode `RejectedBySyntax` DEĞİL).

use std::fs;
use std::sync::Arc;

use osp_mcp::workspace::Workspace;
use osp_mcp::OspMcpServer;

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

use tempfile::TempDir;

fn make_server_handle() -> Arc<std::sync::Mutex<Workspace>> {
    let dir = make_fixture_repo();
    let workspace = Workspace::analyze(dir.path(), None).expect("workspace analyze");
    let llm: Arc<dyn osp_core::navigator::LlmClient> =
        Arc::new(osp_core::navigator::MockLlmClient::new(Vec::new()));
    let server = OspMcpServer::new(workspace, osp_mcp::ServerMode::Agent, llm);
    server.workspace_handle()
}

/// Geçerli Node(0)-scope coupling task'ı (W8 fixture paylaşımı).
fn node_scope_coupling_task() -> osp_core::trajectory::Task {
    osp_core::trajectory::Task {
        id: 1,
        milestone_id: 1,
        label: "Node-scope coupling (valid)".into(),
        target_predicate_set: osp_core::trajectory::PredicateSet {
            mode: osp_core::trajectory::PredicateMode::All,
            predicates: vec![osp_core::trajectory::WeightedPredicate {
                predicate: osp_core::trajectory::MetricPredicate {
                    metric: osp_core::trajectory::PredicateAxis::Coupling,
                    operator: osp_core::trajectory::ComparisonOp::Le,
                    threshold: 0.55,
                    scope: osp_core::trajectory::PredicateScope::Node(0),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: None,
        },
        policy: osp_core::trajectory::TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: osp_core::trajectory::TaskStatus::Pending,
    }
}

/// `task_id` param ≠ `task.id` → native measurement `TaskBindingMismatch` →
/// disposition `TerminalIdentityViolation` → wire: typed system failure
/// (navigator SystemFailure mirror). Eski davranış: `RejectedBySyntax`
/// attempt_outcome — gözlenmeyen gate kararını gözlenmiş gibi sunmak
/// (fabrication) idi.
#[test]
fn native_measurement_failure_emits_typed_system_failure_not_syntax_rejection() {
    let handle = make_server_handle();
    // Geçerli Node-scope task; failure KAYNAĞI identity wiring — draft claim'i
    // task_id=999'a bağlar, measurement defensive check'i task.id=1 ile
    // karşılaştırır → TaskBindingMismatch (TerminalIdentityViolation).
    let task = osp_core::trajectory::Task {
        id: 1,
        milestone_id: 1,
        label: "Node-scope (valid)".into(),
        target_predicate_set: osp_core::trajectory::PredicateSet {
            mode: osp_core::trajectory::PredicateMode::All,
            predicates: vec![osp_core::trajectory::WeightedPredicate {
                predicate: osp_core::trajectory::MetricPredicate {
                    metric: osp_core::trajectory::PredicateAxis::Coupling,
                    operator: osp_core::trajectory::ComparisonOp::Le,
                    threshold: 0.55,
                    scope: osp_core::trajectory::PredicateScope::Node(0),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: None,
        },
        policy: osp_core::trajectory::TaskPolicy::default(),
        allowed_operations: vec![],
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
        reasoning: "identity wiring fixture".into(),
        ..Default::default()
    };

    let mut ws = handle.lock().unwrap();
    // task_id=999 ≠ task.id=1 → measurement defensive binding check.
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 999)
        .expect("attempt");

    // Typed system failure yüzeyi.
    let sys = outcome
        .get("system_failure")
        .expect("terminal measurement failure → system_failure mevcut");
    assert_eq!(
        sys["class"], "NativeMeasurementFailed",
        "failure class typed — navigator ile ortak ontology"
    );
    assert_eq!(
        sys["disposition"], "TerminalIdentityViolation",
        "typed disposition wire'da (string inference YOK)"
    );
    assert_eq!(
        sys["retryable"], false,
        "terminal — budget yok, LLM retry yok (navigator SystemFailure mirror)"
    );

    // **Fabrication YOK:** gate hiç çalışmadı — attempt_outcome ÜRETİLMEZ
    // (eski `RejectedBySyntax` davranışı kapanır).
    assert!(
        outcome.get("attempt_outcome").is_none(),
        "gözlenmeyen gate kararı gözlenmiş gibi sunulmaz: {outcome}"
    );
    // Sidecar yok — measurement tamamlanmadı (comparison-surviving değil).
    assert!(outcome.get("subject_authority_drift").is_none());
    assert!(outcome.get("provenance_authority_drift").is_none());
    // apply_target yansız kalır; message typed disposition taşır.
    assert_eq!(outcome["apply_target"], "NotApplied");
    let msg = outcome["message"].as_str().expect("message");
    assert!(
        msg.contains("TerminalIdentityViolation"),
        "message typed disposition içerir: {msg}"
    );
}

/// **W8-b — Q4-vs-measurement yarış (MCP yüzeyi):** proposal hem structural Q4
/// ihlali (self-import) hem de measurement-failing koşul (task_id ≠ task.id →
/// TaskBindingMismatch) taşıyorsa DRAFT aşaması kazanır — structural Q4,
/// fallible native measurement'tan ÖNCE koşar. Response measurement
/// system_failure DEĞİL, Q4 syntax reddi olmalı (navigator'daki kabul
/// kriterinin MCP mirror'ı — server.rs ordering yorumu pinlenmiş hâlde).
#[test]
fn w8_q4_syntax_precedes_measurement_failure_on_mcp_surface() {
    let handle = make_server_handle();
    let task = node_scope_coupling_task();

    // Self-import (Q4 structural ihlali) + task_id=999 (measurement defensive
    // binding check'i tetikleyecek) — ikisi de aynı proposaldan.source
    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![osp_core::agent::NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 100.0,
            connected_to: vec![],
        }],
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 10_000,
            to: 10_000, // self-import → structural Q4
            kind: osp_core::space::EdgeKind::Imports,
        }],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "Q4 + measurement-failure yarış fixture'ı".into(),
        ..Default::default()
    };

    let mut ws = handle.lock().unwrap();
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 999)
        .expect("attempt");

    // Yarış kazananı: DRAFT Q4 — RejectedBySyntax attempt_outcome (gerçek syntax
    // reddi; fabrication değil) + "claim draft" mesajı.
    assert_eq!(
        outcome["attempt_outcome"]["gate_decision"], "RejectedBySyntax",
        "structural Q4 draft aşamasında yakalanır (measurement ÖNCESİ)"
    );
    let msg = outcome["message"].as_str().expect("message");
    assert!(
        msg.contains("claim draft"),
        "Q4 draft hatası measurement'dan önce yüzeye çıkar: {msg}"
    );
    // Measurement HİÇ çalışmadı → system_failure YOK, sidecar YOK.
    assert!(
        outcome.get("system_failure").is_none(),
        "Q4 precedence: measurement failure yüzeye çıkmaz: {outcome}"
    );
    assert!(outcome.get("subject_authority_drift").is_none());
    assert!(outcome.get("provenance_authority_drift").is_none());
}

/// **W8-c — NativeFailureSurface wire exhaustiveness (commit ailesi):**
/// `required_source: Mixed` task declaration'ı draft'ta değil COMMIT aşamasında
/// `TaskValidation(InvalidRequiredMetricSource)` üretir → shared mapper
/// `SystemFailure` sınıflar → wire `system_failure{class: "EngineCommitFailed"}`
/// (measurement sınıfı `NativeMeasurementFailed` DEĞİL — yüzey ayrımı typed).
#[test]
fn w8_commit_task_validation_maps_to_engine_commit_failed_wire() {
    let handle = make_server_handle();
    // Mixed = geçerli aggregate KAYNAK ama geçersiz REQUIRED source (task
    // declaration hatası — validate_for_commit reddeder).
    let mut task = node_scope_coupling_task();
    task.target_predicate_set.predicates[0]
        .predicate
        .required_source = Some(osp_core::coords::MetricSource::Mixed);

    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![osp_core::agent::NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 100.0,
            connected_to: vec![],
        }],
        new_edges: vec![],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "Mixed required-source fixture".into(),
        ..Default::default()
    };

    let mut ws = handle.lock().unwrap();
    let outcome = ws
        .submit_delta_attempt(&proposal, &task, 1)
        .expect("attempt");

    let sys = outcome
        .get("system_failure")
        .expect("TaskValidation terminal → system_failure mevcut");
    assert_eq!(
        sys["class"], "EngineCommitFailed",
        "commit-aşaması system failure — NativeMeasurementFailed ile ayrışır (typed surface)"
    );
    assert_eq!(
        sys["retryable"], false,
        "terminal — task config düzeltilmeli"
    );
    // Fabrication YOK: attempt_outcome üretilmedi (gate karar yüzeyi değildi).
    assert!(
        outcome.get("attempt_outcome").is_none(),
        "commit-time system failure gate kararı fabricate etmez: {outcome}"
    );
    // TaskValidation comparison-surviving DEĞİL → sidecar YOK.
    assert!(outcome.get("subject_authority_drift").is_none());
    assert!(outcome.get("provenance_authority_drift").is_none());
    let msg = outcome["message"].as_str().expect("message");
    assert!(
        msg.contains("commit_task_claim"),
        "commit funnel mesajı: {msg}"
    );
}
