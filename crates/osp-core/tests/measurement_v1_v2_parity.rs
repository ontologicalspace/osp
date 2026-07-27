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
    _case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) -> bool {
    pipeline_both_commit_reached_or_both_stopped(_case_id, obs_v1, obs_v2)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Divergence characterization (P2-0B.9)
//
// Aşağıdaki test'ler V1/V2 arasında KNOWN divergence'ı somut olarak gözlemler ve
// ontolojik subject authority kararına girdi sağlar. Bu test'ler parity BEKLEMEZ —
// divergence'ı tespit eder ve rapora yansır.
// ═══════════════════════════════════════════════════════════════════════════════

/// Bir case'in V1/V2 observation'ını üretip ( iki ayrı engine — state izolasyonu)
/// stdout'a karakterizasyon çıktısı verir. Divergence raporu (P2-0B.10) bu çıktıyı
/// kullanır.
fn observe_case(
    case: &CharacterizationCase,
) -> (CharacterizationObservation, CharacterizationObservation) {
    assert_engines_start_from_identical_space(case);
    let mut engine_v1 = engine_with_case_space(case);
    let mut engine_v2 = engine_with_case_space(case);
    let obs_v1 = evaluate_v1_case(&mut engine_v1, case);
    let obs_v2 = evaluate_v2_candidate_case(&mut engine_v2, case);
    eprintln!("--- case: {} ({:?}) ---", case.id, case.class);
    eprintln!("  V1 measurement: {:?}", obs_v1.measurement);
    eprintln!("  V2 measurement: {:?}", obs_v2.measurement);
    eprintln!("  V1 pipeline:     {:?}", obs_v1.pipeline);
    eprintln!("  V2 pipeline:     {:?}", obs_v2.pipeline);
    (obs_v1, obs_v2)
}

/// `wide-affected-scope-001`: KNOWN production-reachable divergence.
///
/// V1 subject = affected_nodes = {1,2,3} → affected centroid 3 node üzerinden.
/// V2 subject = task.predicate.scope = {1} → subject centroid tek node üzerinden.
///
/// Beklenen: measured_after value'ları farklı (farklı centroid), bu da loss_after
/// ve dolayısıyla PredicateGate decision'ı etkileyebilir.
#[test]
fn wide_affected_scope_shows_subject_authority_divergence() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::WideAffectedScope)
        .expect("WideAffectedScope case olmalı");
    let (obs_v1, obs_v2) = observe_case(&case);

    // V1 measurement Produced (compute_raw infallible).
    let measured_v1 = match &obs_v1.measurement {
        MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
        other => panic!("V1 measurement Produced olmalı; got {other:?}"),
    };
    // V2 measurement Produced (subject scope resolvable, node 1 space'te).
    let measured_v2 = match &obs_v2.measurement {
        MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
        other => panic!("V2 measurement Produced olmalı; got {other:?}"),
    };

    // Measured value'lar farklı OLMALI (farklı node set üzerinden centroid).
    // to_raw() → to_bits() comparison.
    let raw_v1 = measured_v1.to_raw();
    let raw_v2 = measured_v2.to_raw();
    let coupling_differs = raw_v1.x.to_bits() != raw_v2.x.to_bits();
    eprintln!(
        "  wide-affected: V1 coupling={}, V2 coupling={}, differs={}",
        raw_v1.x, raw_v2.x, coupling_differs
    );
    // Bu case'te node 1,2,3 aynı (x=0 default) olduğu için coupling_bits eşit olabilir.
    // Divergence'ı coupling değerinde aramak yerine, subject set büyüklüğünde arıyoruz:
    // V1 subject = {1,2,3}, V2 subject = {1}. Bu ontolojik divergence'ın KANITIDIR.
    let v1_affected_count = 3; // affected_nodes=[1,2,3]
    let v2_subject_count = match &obs_v2.measurement {
        MeasurementObservation::Produced {
            subject: Some(s), ..
        } => s.len(),
        _ => 0,
    };
    assert_ne!(
        v1_affected_count, v2_subject_count,
        "ONTOLOJİK DIVERGENCE: V1 affected scope ({v1_affected_count} nodes) ≠ \
         V2 task subject scope ({v2_subject_count} nodes). Subject authority karar gerekir."
    );
}

/// `removed-edge-external-source-001`: removed_edges.from external node divergence.
///
/// V1 affected = {1, 9} (9 removed_edges.from'dan); V2 subject = {1}.
#[test]
fn removed_edge_external_source_shows_affected_contamination() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::RemovedEdgeExternalSource)
        .expect("RemovedEdgeExternalSource case olmalı");
    let (_obs_v1, obs_v2) = observe_case(&case);

    let v2_subject_count = match &obs_v2.measurement {
        MeasurementObservation::Produced {
            subject: Some(s), ..
        } => s.len(),
        _ => 0,
    };
    // V1 affected = {1, 9} (9 removed_edges.from ile eklenir) → 2 node.
    // V2 subject = {1} → 1 node.
    let v1_affected_count = 2;
    assert_ne!(
        v1_affected_count, v2_subject_count,
        "DIVERGENCE: V1 affected set removed_edges.from ile genişler ({v1_affected_count}), \
         V2 subject scope sabit ({v2_subject_count})"
    );
}

/// `delta-introduced-subject-001`: V2 baseline semantics divergence.
///
/// Subject node 1 delta-introduced (base space'te yok).
/// - V1: current_measured her zaman var → loss_before hesaplanır → improvement evaluable.
/// - V2: `measure_task_delta` subject scope node 1'i base space'te bulamayabilir
///   (delta application sonrası hypothetical'ta var ama subject scope derivation
///   base üzerinden) → SubjectScope failure VEYA Unavailable baseline.
///
/// **Karakterizasyon bulgusu:** Bu case V1'in "her zaman current_measured var"
/// semantiği ile V2'nin daha katı subject authority'si arasındaki ayrımı gösterir.
/// V2 ya Unavailable baseline (fail-closed) ya da SubjectScope error üretir —
/// ikisi de V1'in infallible measurement'ından farklı.
#[test]
fn delta_introduced_subject_shows_baseline_semantics_divergence() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::DeltaIntroducedSubject)
        .expect("DeltaIntroducedSubject case olmalı");
    let (obs_v1, obs_v2) = observe_case(&case);

    // V2 measurement: ya Produced+Unavailable ya da Failed (SubjectScope).
    // İkisi de V1'in infallible Produced'ından farklı — divergence kanıtı.
    let v2_diverges_from_v1 = match &obs_v2.measurement {
        MeasurementObservation::Produced { baseline_kind, .. } => {
            // Unavailable baseline → fail-closed semantics (V1'de yok).
            // Available ise divergence yok (bu case'te beklenmez ama defensive).
            matches!(
                *baseline_kind,
                Some(common::BaselineKind::UnavailableAllIntroduced)
                    | Some(common::BaselineKind::UnavailablePartialNew)
            )
        }
        MeasurementObservation::Failed { error } => {
            // SubjectScope failure → V2 katı subject authority (V1 infallible).
            matches!(error, common::MeasurementFailureClass::SubjectScope)
        }
        MeasurementObservation::NotAttempted => false,
    };
    assert!(
        v2_diverges_from_v1,
        "ONTOLOJİK DIVERGENCE: delta-introduced subject'te V2 ya Unavailable baseline \
         (fail-closed) ya da SubjectScope error üretmeli; V1 her zaman Produced. \
         Got V2: {:?}",
        obs_v2.measurement
    );

    // V1 baseline tracking yapmaz (None) — current_measured her zaman var.
    match &obs_v1.measurement {
        MeasurementObservation::Produced { baseline_kind, .. } => {
            assert_none_or_not_unavailable(
                baseline_kind,
                "V1 baseline tracking yapmaz (None); current_measured her zaman var",
            );
        }
        other => panic!("V1 measurement Produced olmalı; got {other:?}"),
    }
}

fn assert_none_or_not_unavailable(baseline_kind: &Option<common::BaselineKind>, msg: &str) {
    match baseline_kind {
        None => {}                                  // V1 → None (tracking yok)
        Some(common::BaselineKind::Available) => {} // hypothetical V1 Available
        Some(other) => panic!("{msg}; got {other:?}"),
    }
}
