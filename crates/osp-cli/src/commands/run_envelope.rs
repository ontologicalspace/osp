//! Trajectory run envelope — versioned JSON truth-surface for `osp trajectory attempt`.
//!
//! Faz 8 test-project (review B-3 P0): Completed-loop exact kanıtı. Navigator çıktısını
//! versioned JSON envelope olarak sunar — result kind, attempts, evidence (before/after
//! measured positions + gate/mutation/completion decisions).
//!
//! ## #96 MD-2 cutover — iki-eksen authority vocabulary
//!
//! `execution_measurement`: `subject_authority: "affected_nodes"` (legacy family
//! label; #95-A'da değeri `"task_scope"`a çevrilir), `provenance_authority:
//! "engine_native_per_axis"`, `provenance_native: true`; deprecated `authority`
//! alias yalnız provenance mirror'i (#100'e kadar). Navigator ölçümü artık
//! engine-native per-axis (`measure_attempt_native_with_md1_shadow` — opaque
//! token). Analyze envelope'taki `analyzer_axis_specific` provenance'dan ayrıdır.

#![allow(
    dead_code,
    reason = "Faz 8 test-project: wired incrementally across commits"
)]

use osp_core::navigator::NavigatorResult;
use osp_core::trajectory::{MutationDecision, TrajectoryEvidence};

/// CLI run-result mirror — serializable (NavigatorResult Serialize türetmiyor).
///
/// Reviewer'ın istediği exact pin: result.kind (completed/awaiting/exceeded/...), attempts.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliRunResultKind {
    Completed,
    AwaitingWitnesses,
    ExceededManeuverLimit,
    RequiresRevision,
    RequiresOperatorApproval,
    TaskNotFound,
    WitnessEvaluationError,
    PendingAuthorizationPersistenceFailure,
    SystemFailure,
    LlmError,
}

/// Serializable run result (mirror of NavigatorResult's user-relevant fields).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliRunResult {
    pub kind: CliRunResultKind,
    pub attempts: usize,
}

impl CliRunResult {
    pub fn from_navigator(result: &NavigatorResult) -> Self {
        let (kind, attempts) = match result {
            NavigatorResult::Completed { attempts, .. } => (CliRunResultKind::Completed, *attempts),
            NavigatorResult::AwaitingWitnesses { .. } => (CliRunResultKind::AwaitingWitnesses, 0),
            NavigatorResult::ExceededManeuverLimit { attempts, .. } => {
                (CliRunResultKind::ExceededManeuverLimit, *attempts)
            }
            NavigatorResult::RequiresRevision(_) => (CliRunResultKind::RequiresRevision, 0),
            NavigatorResult::RequiresOperatorApproval { attempts, .. } => {
                (CliRunResultKind::RequiresOperatorApproval, *attempts)
            }
            NavigatorResult::TaskNotFound => (CliRunResultKind::TaskNotFound, 0),
            NavigatorResult::WitnessEvaluationError(_) => {
                (CliRunResultKind::WitnessEvaluationError, 0)
            }
            NavigatorResult::PendingAuthorizationPersistenceFailure { .. } => {
                (CliRunResultKind::PendingAuthorizationPersistenceFailure, 0)
            }
            NavigatorResult::SystemFailure(_) => (CliRunResultKind::SystemFailure, 0),
            NavigatorResult::LlmError(_) => (CliRunResultKind::LlmError, 0),
        };
        Self { kind, attempts }
    }
}

/// Execution-mode wire tag (snake_case, distinct from clap ValueEnum).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliRunExecutionMode {
    Production,
    Harness,
}

impl CliRunExecutionMode {
    pub fn from_cli(mode: crate::commands::CliExecutionMode) -> Self {
        match mode {
            crate::commands::CliExecutionMode::Production => Self::Production,
            crate::commands::CliExecutionMode::Harness => Self::Harness,
        }
    }
}

/// Witness-mode wire tag (snake_case).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliRunWitnessMode {
    Production,
    HarnessAutoApprove,
}

impl CliRunWitnessMode {
    pub fn from_cli(mode: crate::commands::CliWitnessMode) -> Self {
        match mode {
            crate::commands::CliWitnessMode::Production => Self::Production,
            crate::commands::CliWitnessMode::HarnessAutoApprove => Self::HarnessAutoApprove,
        }
    }
}

/// Versioned trajectory run envelope — V1 honest metadata + result + evidence.
///
/// Bu Completed-loop'un canonical kanıtıdır (review B-3 P0). `state.before/after` ve
/// `result` + `evidence` exact pinlenmiş state-transition'ı taşır.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliRunEnvelopeV1 {
    pub schema_version: u32,
    pub run: CliRunMeta,
    pub execution_measurement: CliExecutionMeasurement,
    pub result: CliRunResult,
    pub evidence: Vec<TrajectoryEvidence>,
}

/// Run metadata — execution mode, witness, task source, repository head.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliRunMeta {
    pub execution_mode: CliRunExecutionMode,
    pub witness_mode: CliRunWitnessMode,
    /// "harness_task_file" veya "legacy_hardcoded" (task dosyası verilmedi).
    pub task_source: &'static str,
    pub repository_head: String,
}

/// V1 execution measurement metadata — **#96 MD-2 iki-eksen authority vocabulary**
/// (PR #124 review P2-tur1: MD-1/MD-2 eksenleri tek string'de ezilmez).
///
/// - `subject_authority`: ölçüm subject'inin kaynağı — **#96'da girer** (legacy
///   authority-family label `"affected_nodes"`; literal subject set DEĞİL — gerçek
///   V1 subject `derive_v1_legacy_measurement_subject()`'in ordered union'ıdır:
///   affected_nodes ∪ unseen removed_edges.from). **#95-A yalnız değerini**
///   `"task_scope"`a çevirir.
/// - `provenance_authority`: ölçüm provenance'ı — #96 ile `"engine_native_per_axis"`.
/// - `provenance_native`: #96 ile `true`.
/// - `authority`: **deprecated alias — yalnız `provenance_authority`'yi mirror eder**
///   (pre-#96 `legacy_projected_v1` → post-#96 `engine_native_per_axis`; #95-A
///   bu alana DOKUNMAZ). #100'e kadar yaşar.
///
/// Bootstrap `current_measured` tohumu bu metadata'ya DAHİL DEĞİL — yalnız
/// proposal/commit measurement authority'yi anlatır (bootstrap current-state
/// authority ayrı migration).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliExecutionMeasurement {
    pub subject_authority: &'static str,
    pub provenance_authority: &'static str,
    pub provenance_native: bool,
    /// Deprecated alias — yalnız provenance mirror (#100'e kadar).
    pub authority: &'static str,
}

impl CliExecutionMeasurement {
    /// **#96 MD-2 cutover:** engine-native per-axis provenance authority.
    pub fn engine_native_per_axis() -> Self {
        Self {
            subject_authority: "affected_nodes",
            provenance_authority: "engine_native_per_axis",
            provenance_native: true,
            authority: "engine_native_per_axis",
        }
    }
}

/// Build the V1 run envelope from navigator output + run context.
pub fn build_run_envelope_v1(
    result: &NavigatorResult,
    evidence: &[TrajectoryEvidence],
    execution_mode: crate::commands::CliExecutionMode,
    witness_mode: crate::commands::CliWitnessMode,
    task_source: &'static str,
    repository_head: &str,
) -> CliRunEnvelopeV1 {
    CliRunEnvelopeV1 {
        schema_version: 1,
        run: CliRunMeta {
            execution_mode: CliRunExecutionMode::from_cli(execution_mode),
            witness_mode: CliRunWitnessMode::from_cli(witness_mode),
            task_source,
            repository_head: repository_head.to_string(),
        },
        execution_measurement: CliExecutionMeasurement::engine_native_per_axis(),
        result: CliRunResult::from_navigator(result),
        evidence: evidence.to_vec(),
    }
}

/// Extract the last accepted evidence entry (mutation applied) for state-transition checks.
/// Returns None if no evidence or no accepted entry.
pub fn last_accepted_evidence(evidence: &[TrajectoryEvidence]) -> Option<&TrajectoryEvidence> {
    evidence.iter().rev().find(|e| {
        matches!(
            e.mutation_decision,
            MutationDecision::AcceptAsCompleted | MutationDecision::AcceptAsProgress
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::coords::RawPosition;
    use osp_core::navigator::NavigatorResult;
    use osp_core::trajectory::{
        GateDecision, MutationDecision, PredicateCompletion, TokenCost, TrajectoryEvidence,
    };

    fn fake_evidence(
        before_x: f64,
        after_x: f64,
        mutation: MutationDecision,
    ) -> TrajectoryEvidence {
        TrajectoryEvidence {
            trajectory_id: 1,
            milestone_id: 1,
            task_id: 7,
            attempt_id: 0,
            before: RawPosition {
                x: before_x,
                y: 0.5,
                z: 0.5,
                w: 0.5,
                v: 0.3,
            },
            after: RawPosition {
                x: after_x,
                y: 0.6,
                z: 0.4,
                w: 0.5,
                v: 0.3,
            },
            gate_decision: GateDecision::PassedAll,
            predicate_completion: PredicateCompletion::Completed,
            mutation_decision: mutation,
            token_cost: TokenCost::default(),
            duration_ms: 10,
            subject_authority_drift: None,
            provenance_authority_drift: None,
        }
    }

    #[test]
    fn run_result_kind_serializes_snake_case() {
        let kinds = [
            CliRunResultKind::Completed,
            CliRunResultKind::AwaitingWitnesses,
            CliRunResultKind::ExceededManeuverLimit,
        ];
        let serialized: Vec<String> = kinds
            .iter()
            .map(|k| {
                serde_json::to_string(k)
                    .unwrap()
                    .trim_matches('"')
                    .to_string()
            })
            .collect();
        assert_eq!(
            serialized,
            ["completed", "awaiting_witnesses", "exceeded_maneuver_limit"]
        );
    }

    #[test]
    fn from_navigator_completed_extracts_attempts() {
        let result = NavigatorResult::Completed {
            attempts: 1,
            total_tokens: TokenCost::default(),
        };
        let cli = CliRunResult::from_navigator(&result);
        assert!(matches!(cli.kind, CliRunResultKind::Completed));
        assert_eq!(cli.attempts, 1);
    }

    #[test]
    fn envelope_serializes_with_native_per_axis_authority() {
        let result = NavigatorResult::Completed {
            attempts: 1,
            total_tokens: TokenCost::default(),
        };
        let evidence = vec![fake_evidence(
            0.667,
            0.5,
            MutationDecision::AcceptAsCompleted,
        )];
        let envelope = build_run_envelope_v1(
            &result,
            &evidence,
            crate::commands::CliExecutionMode::Harness,
            crate::commands::CliWitnessMode::HarnessAutoApprove,
            "harness_task_file",
            "0123456789abcdef0123456789abcdef01234567",
        );
        let json = serde_json::to_string(&envelope).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["run"]["execution_mode"], "harness");
        assert_eq!(v["run"]["witness_mode"], "harness_auto_approve");
        assert_eq!(v["run"]["task_source"], "harness_task_file");
        // **#96 MD-2 iki-eksen vocabulary:** her eksen kendi alanında; deprecated
        // `authority` alias yalnız provenance mirror (pre-#96: legacy_projected_v1).
        assert_eq!(
            v["execution_measurement"]["subject_authority"],
            "affected_nodes"
        );
        assert_eq!(
            v["execution_measurement"]["provenance_authority"],
            "engine_native_per_axis"
        );
        assert_eq!(v["execution_measurement"]["provenance_native"], true);
        assert_eq!(
            v["execution_measurement"]["authority"],
            "engine_native_per_axis"
        );
        assert_eq!(v["result"]["kind"], "completed");
        assert_eq!(v["result"]["attempts"], 1);
        assert_eq!(v["evidence"][0]["gate_decision"], "PassedAll");
        assert_eq!(v["evidence"][0]["predicate_completion"], "Completed");
        assert_eq!(v["evidence"][0]["mutation_decision"], "AcceptAsCompleted");
        assert_eq!(v["evidence"][0]["before"]["x"], 0.667);
        assert_eq!(v["evidence"][0]["after"]["x"], 0.5);
    }

    #[test]
    fn last_accepted_evidence_finds_completed() {
        let evidence = vec![
            fake_evidence(0.667, 0.667, MutationDecision::Reject),
            fake_evidence(0.667, 0.5, MutationDecision::AcceptAsCompleted),
        ];
        let last = last_accepted_evidence(&evidence).unwrap();
        assert!(matches!(
            last.mutation_decision,
            MutationDecision::AcceptAsCompleted
        ));
        assert_eq!(last.after.x, 0.5);
    }

    #[test]
    fn last_accepted_evidence_none_when_all_rejected() {
        let evidence = vec![fake_evidence(0.667, 0.667, MutationDecision::Reject)];
        assert!(last_accepted_evidence(&evidence).is_none());
    }
}
