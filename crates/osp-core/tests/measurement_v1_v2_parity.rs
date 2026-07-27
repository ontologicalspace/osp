//! Faz 8-P2 P2-0B — V1/V2 semantic divergence characterization (Yüzey 1: core).
//!
//! Bu integration test, V1 (`compute_raw_from_delta` + `provenanced_from_raw`) ile
//! V2-candidate (`measure_task_delta` + V1 compatibility projection) arasında
//! **gözlemlenebilir davranış farkını** ölçer. Her case iki ayrı engine instance'ında
//! çalıştırılır (state izolasyonu — commit/witness space mutate edebilir).
//!
//! ## Gate (plan Tur 4 P0-4)
//!
//! P2-1 (caller migration) ancak tüm metriklerde exact parity gösterilirse açılır:
//! 1. Subject set (node ID kümesi) — exact eşitlik
//! 2. Axis values (`to_bits`) — exact eşitlik
//! 3. Axis sources — exact eşitlik veya source-sensitive predicate matrisi aynı
//! 4. Q5 disposition — exact parity
//! 5. Q5.b completion + mutation decision — exact parity
//! 6. Apply target — exact parity
//! 7. Witness reachability — exact parity
//!
//! **Bilinen yapısal gerçek:** V1 subject = `proposal.affected_nodes` (LLM-declared),
//! V2 subject = `task.predicate.scope` (task-derived). Bu test o farkı KANITLAMAZ —
//! somut case'lerde sıklık ve decision impact ölçer. Ontolojik subject authority
//! kararı raporla (P2-0B.10) verilir.
//!
//! ## Bu test NE DEĞİLDİR
//!
//! - Production migration DEĞİL (commit_task_claim body değişmez)
//! - P2-1 implementation DEĞİL (measure_task_delta_checked henüz yok)
//! - "V2 production path" DEĞİL — V2 **candidate projection** (test-only harness)

mod common;

use common::{
    build_all_cases, engine_with_case_space, evaluate_v1_case, evaluate_v2_candidate_case,
    CaseClass, CharacterizationCase, CharacterizationObservation, MeasurementObservation,
    PipelineObservation, WitnessReachability,
};
use osp_core::authorization::SpaceDigest;

// ═══════════════════════════════════════════════════════════════════════════════
// State isolation verification
// ═══════════════════════════════════════════════════════════════════════════════

/// İki engine'in başlangıç space digest'i exact eşit olmalı (V1 ve V2 ayrı engine,
/// aynı başlangıç state). Bu invariant olmadan divergence ölçümü anlamsız.
fn assert_engines_start_from_identical_space(case: &CharacterizationCase) {
    let engine_v1 = engine_with_case_space(case);
    let engine_v2 = engine_with_case_space(case);
    // Space'in structural digest'ini karşılaştır (engine space'i constructor'da verildi).
    let digest_v1 = SpaceDigest::compute(engine_v1.space()).expect("space digest V1");
    let digest_v2 = SpaceDigest::compute(engine_v2.space()).expect("space digest V2");
    assert_eq!(
        digest_v1.as_bytes(),
        digest_v2.as_bytes(),
        "V1/V2 engine başlangıç space digest eşit olmalı (case {})",
        case.id
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Baseline parity case: matching-single-node-001
// ═══════════════════════════════════════════════════════════════════════════════

/// `matching-single-node-001`: Task Node(1) scope, proposal affected_nodes=[1].
/// Subject set identical V1/V2 → tüm metriklerde parity beklenir.
///
/// Bu test fail olursa harness kendisi bozuktur — baseline olamaz.
#[test]
fn matching_scope_baseline_case_shows_parity() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::MatchingScope)
        .expect("MatchingScope case olmalı (matching-single-node-001)");
    assert_eq!(case.id, "matching-single-node-001");

    assert_engines_start_from_identical_space(&case);

    let mut engine_v1 = engine_with_case_space(&case);
    let mut engine_v2 = engine_with_case_space(&case);
    let obs_v1 = evaluate_v1_case(&mut engine_v1, &case);
    let obs_v2 = evaluate_v2_candidate_case(&mut engine_v2, &case);

    // Q5 disposition parity.
    assert_q5_disposition_parity(&case.id, &obs_v1, &obs_v2);
    // Q5.b predicate completion + mutation decision parity.
    assert_predicate_completion_parity(&case.id, &obs_v1, &obs_v2);
    assert_mutation_decision_parity(&case.id, &obs_v1, &obs_v2);
    // Apply target parity.
    assert_apply_target_parity(&case.id, &obs_v1, &obs_v2);
    // Witness reachability parity.
    assert_witness_reachability_parity(&case.id, &obs_v1, &obs_v2);

    // Measurement: V1 NotAttempted değil, Produced olmalı (compute_raw infallible).
    assert!(
        matches!(obs_v1.measurement, MeasurementObservation::Produced { .. }),
        "V1 measurement Produced olmalı (compute_raw infallible); case {}",
        case.id
    );
    // V2-candidate: matching scope → Available baseline (node space'te), measurement Produced.
    match &obs_v2.measurement {
        MeasurementObservation::Produced { baseline_kind, .. } => {
            assert_eq!(
                *baseline_kind,
                Some(common::BaselineKind::Available),
                "matching scope (node space'te) → V2 Available baseline; case {}",
                case.id
            );
        }
        other => panic!(
            "V2-candidate measurement Produced olmalı; case {} got {:?}",
            case.id, other
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Parity assertion helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn pipeline_both_commit_reached_or_both_stopped(
    _case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) -> bool {
    match (&obs_v1.pipeline, &obs_v2.pipeline) {
        (PipelineObservation::CommitReached { .. }, PipelineObservation::CommitReached { .. }) => {
            true
        }
        (
            PipelineObservation::StoppedBeforeCommit { .. },
            PipelineObservation::StoppedBeforeCommit { .. },
        ) => true,
        _ => {
            // Pipeline stage mismatch — bu da bir divergence (measurement error V1'de
            // yok, V2'de var → V2 erken durabilir). Rapor için belgele.
            false
        }
    }
}

fn assert_q5_disposition_parity(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    use common::Q5Observation;
    let q5_v1 = q5_disposition(obs_v1);
    let q5_v2 = q5_disposition(obs_v2);
    assert_eq!(
        q5_v1, q5_v2,
        "Q5 disposition parity fail; case {case_id}: V1={q5_v1:?} V2={q5_v2:?}"
    );
    let _ = (Q5Observation::Passed,); // import hint
}

fn q5_disposition(obs: &CharacterizationObservation) -> Q5Disposition {
    match &obs.pipeline {
        PipelineObservation::CommitReached { q5, .. } => match q5 {
            common::Q5Observation::Passed => Q5Disposition::Passed,
            common::Q5Observation::Rejected { .. } => Q5Disposition::Rejected,
            common::Q5Observation::NotReached => Q5Disposition::NotReached,
        },
        PipelineObservation::StoppedBeforeCommit { stage, .. } => match stage {
            common::PipelineStage::Vision => Q5Disposition::Rejected,
            _ => Q5Disposition::NotReached,
        },
    }
}

#[derive(Debug, PartialEq)]
enum Q5Disposition {
    Passed,
    Rejected,
    NotReached,
}

fn assert_predicate_completion_parity(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    let (pc_v1, pc_v2) = match (&obs_v1.pipeline, &obs_v2.pipeline) {
        (
            PipelineObservation::CommitReached {
                predicate_completion: a,
                ..
            },
            PipelineObservation::CommitReached {
                predicate_completion: b,
                ..
            },
        ) => (a, b),
        _ => return, // en az biri stopped → Q5.b çalışmadı, parity N/A
    };
    assert_eq!(
        pc_v1, pc_v2,
        "PredicateCompletion parity fail; case {case_id}: V1={pc_v1:?} V2={pc_v2:?}"
    );
}

fn assert_mutation_decision_parity(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    let (md_v1, md_v2) = match (&obs_v1.pipeline, &obs_v2.pipeline) {
        (
            PipelineObservation::CommitReached {
                mutation_decision: a,
                ..
            },
            PipelineObservation::CommitReached {
                mutation_decision: b,
                ..
            },
        ) => (a, b),
        _ => return,
    };
    assert_eq!(
        md_v1, md_v2,
        "MutationDecision parity fail; case {case_id}: V1={md_v1:?} V2={md_v2:?}"
    );
}

fn assert_apply_target_parity(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    let (at_v1, at_v2) = match (&obs_v1.pipeline, &obs_v2.pipeline) {
        (
            PipelineObservation::CommitReached {
                apply_target: a, ..
            },
            PipelineObservation::CommitReached {
                apply_target: b, ..
            },
        ) => (a, b),
        _ => return,
    };
    assert_eq!(
        at_v1, at_v2,
        "ApplyTarget parity fail; case {case_id}: V1={at_v1:?} V2={at_v2:?}"
    );
}

fn assert_witness_reachability_parity(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    let wr_v1 = witness_reachability(obs_v1);
    let wr_v2 = witness_reachability(obs_v2);
    assert_eq!(
        wr_v1, wr_v2,
        "WitnessReachability parity fail; case {case_id}: V1={wr_v1:?} V2={wr_v2:?}"
    );
}

fn witness_reachability(obs: &CharacterizationObservation) -> WitnessReachability {
    match &obs.pipeline {
        PipelineObservation::CommitReached {
            witness_reachability,
            ..
        } => witness_reachability.clone(),
        PipelineObservation::StoppedBeforeCommit { .. } => WitnessReachability::NotReached,
    }
}

// pipeline_both_commit_reached_or_both_stopped helper'ı P2-0B.9'da (divergence matrisi)
// kullanılacak; şimdilik baseline parity için inline parity assertion'lar yeterli.
#[allow(dead_code, reason = "P2-0B.9 divergence matrisinde kullanılacak")]
fn _reserved_pipeline_stage_helper(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) -> bool {
    pipeline_both_commit_reached_or_both_stopped(case_id, obs_v1, obs_v2)
}
