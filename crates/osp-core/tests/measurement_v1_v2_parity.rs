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

    // === Review P0-2: exact subject/value/source parity (gate kriterleri) ===

    // 1. Subject set parity — matching scope'ta V1 affected = V2 task scope = {1}.
    let (subj_v1, subj_v2) = measurement_subjects(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        sorted(&subj_v1),
        sorted(&subj_v2),
        "matching scope: V1 affected ({:?}) == V2 subject ({:?}); case {}",
        subj_v1,
        subj_v2,
        case.id
    );

    // 2. 5-axis value bits — exact golden snapshot (review tur 4 P1-1).
    // Önceki assert_eq!(vals_v1, vals_v2) golden DEĞİLDİ (ikisi birlikte değişebilir).
    // Artık her iki taraf ayrı exact pin — measurement implementation değişirse stale.
    let (vals_v1, vals_v2) = measurement_value_bits(&obs_v1, &obs_v2, &case.id);
    eprintln!("  matching value bits: V1={:?} V2={:?}", vals_v1, vals_v2);
    assert_eq!(
        vals_v1,
        [
            4602678819172646912u64,
            4602678819172646912,
            4607182418800017408,
            4602678819172646912,
            4601046424471046557
        ],
        "V1 matching 1-node centroid exact bits; case {}",
        case.id
    );
    assert_eq!(
        vals_v2,
        [
            4602678819172646912u64,
            4602678819172646912,
            4607182418800017408,
            4602678819172646912,
            4601046424471046557
        ],
        "V2 matching 1-node centroid exact bits (subject parity); case {}",
        case.id
    );

    // 3. 5-axis sources — **matching scope'ta BİLE divergence beklenir** (review P0-2).
    // V1 provenanced_from_raw(..., Scip) → uniform Scip; V2 engine gerçek axis source'ları
    // (coupling=TreeSitter). Bu, subject-authority'den BAĞIMSIZ bir provenance-authority
    // geçişi — INV-T4 required_source predicate'lerini etkiler.
    let (src_v1, src_v2) = measurement_sources(&obs_v1, &obs_v2, &case.id);
    let source_divergence = src_v1 != src_v2;
    eprintln!(
        "matching-scope source check: V1={:?} V2={:?} diverges={}",
        src_v1, src_v2, source_divergence
    );
    // **Review tur 3 P1-1 fix:** Exact 5-axis source snapshot pin (sadece coupling
    // assert_ne DEĞİL). V1 uniform Scip (provenanced_from_raw override); V2 gerçek
    // engine axis defaults. Bu snapshot measurement implementation değişirse stale
    // kalır → test fail (golden pin).
    use osp_core::coords::MetricSource;
    assert_eq!(
        src_v1,
        [MetricSource::Scip; 5],
        "V1 uniform Scip (provenanced_from_raw override); case {}",
        case.id
    );
    assert_eq!(
        src_v2,
        [
            MetricSource::TreeSitter,  // coupling
            MetricSource::Placeholder, // cohesion (default axis)
            MetricSource::TreeSitter,  // instability
            MetricSource::Heuristic,   // entropy
            MetricSource::Heuristic,   // witness_depth
        ],
        "V2 gerçek engine axis defaults (INV-T4 native provenance); case {}",
        case.id
    );

    // === Pipeline parity (Q5/Q5.b/apply/witness) ===
    assert_q5_disposition_parity(&case.id, &obs_v1, &obs_v2);
    assert_predicate_completion_parity(&case.id, &obs_v1, &obs_v2);
    assert_mutation_decision_parity(&case.id, &obs_v1, &obs_v2);
    assert_apply_target_parity(&case.id, &obs_v1, &obs_v2);
    assert_witness_reachability_parity(&case.id, &obs_v1, &obs_v2);

    // === Corpus observation golden (review tur 5/6 P0-2/P1-2) ===
    // Mutation decision: matching V1/V2 Q5 Vision'da durur (placeholder computed_raw →
    // theta ihlali) → PredicateGate'e ulaşmadı → NotReached.
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::NotReached,
        "matching V1: Q5 Vision → PredicateGate NotReached; case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::NotReached,
        "matching V2: Q5 Vision NotReached; case {}",
        case.id
    );
    // **Review tur 6 P1-1:** exact pipeline stage pin (NotReached yalnız tri-state;
    // Vision stage değil TaskBinding/TaskValidation'da da durursa yeşil kalabilirdi).
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::StoppedBeforeCommit {
            stage: common::PipelineStage::Vision,
            error: common::PipelineFailureClass::Vision,
        },
        "matching V1 exact pipeline: StoppedBeforeCommit{{Vision, Vision}}; case {}",
        case.id
    );
    assert_eq!(
        obs_v2.pipeline,
        common::PipelineObservation::StoppedBeforeCommit {
            stage: common::PipelineStage::Vision,
            error: common::PipelineFailureClass::Vision,
        },
        "matching V2 exact pipeline: StoppedBeforeCommit{{Vision, Vision}}; case {}",
        case.id
    );
    // V1 baseline exact golden (review tur 8 P0-2): AffectedCentroid + tüm alanlar.
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::LegacyComputed {
                values_bits,
                sources,
                loss_bits,
                derivation,
            },
        ..
    } = &obs_v1.measurement
    {
        assert_eq!(
            *derivation,
            common::LegacyBaselineDerivation::AffectedCentroid
        );
        assert_eq!(
            *values_bits,
            [
                0u64,
                4602678819172646912,
                4602678819172646912,
                4602678819172646912,
                4601046424471046557
            ],
            "matching V1 baseline values_bits; case {}",
            case.id
        );
        assert_eq!(
            *sources,
            [MetricSource::Scip; 5],
            "matching V1 baseline sources; case {}",
            case.id
        );
        assert_eq!(
            *loss_bits, 4604544271217802189,
            "matching V1 baseline loss_bits; case {}",
            case.id
        );
    } else {
        panic!(
            "matching V1 LegacyComputed baseline olmalı; case {}",
            case.id
        );
    }

    // V2-candidate Available baseline exact golden (review tur 8 P0-2).
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::Available {
                values_bits,
                sources,
                loss_bits,
            },
        ..
    } = &obs_v2.measurement
    {
        assert_eq!(
            *values_bits,
            [
                0u64,
                4602678819172646912,
                4602678819172646912,
                4602678819172646912,
                4601046424471046557
            ],
            "matching V2 baseline values_bits; case {}",
            case.id
        );
        // V2 baseline sources engine-native (Scip DEĞİL) → INV-T4 provenance divergence
        // baseline boyutunda da var (review tur 8 P0-2).
        assert_eq!(
            *sources,
            [
                MetricSource::TreeSitter,
                MetricSource::Placeholder,
                MetricSource::TreeSitter,
                MetricSource::Heuristic,
                MetricSource::Heuristic
            ],
            "matching V2 baseline sources engine-native; case {}",
            case.id
        );
        assert_eq!(
            *loss_bits, 4604544271217802189,
            "matching V2 baseline loss_bits; case {}",
            case.id
        );
    } else {
        panic!("matching V2 Available baseline olmalı; case {}", case.id);
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
///
/// **Review P1-1 fix:** Önceki test `eprintln!` + hardcode count kullanıyordu;
/// artık exact observed snapshot assertion'ları (subject set + value bits + sources).
#[test]
fn wide_affected_scope_shows_subject_authority_divergence() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::WideAffectedScope)
        .expect("WideAffectedScope case olmalı");
    let (obs_v1, obs_v2) = observe_case(&case);

    // === Subject set divergence (exact) ===
    // V1 affected = {1,2,3}, V2 subject = {1}. PIN (hardcode DEĞİL, gerçek observation).
    let (subj_v1, subj_v2) = measurement_subjects(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        sorted(&subj_v1),
        vec![1, 2, 3],
        "V1 affected = {{1,2,3}} (case design); got {subj_v1:?}"
    );
    assert_eq!(
        sorted(&subj_v2),
        vec![1],
        "V2 subject = {{1}} (task scope); got {subj_v2:?}"
    );
    assert_ne!(
        sorted(&subj_v1),
        sorted(&subj_v2),
        "ONTOLOJİK SUBJECT AUTHORITY DIVERGENCE: V1 affected ≠ V2 task scope"
    );

    // === Value bits divergence (exact 5-axis snapshot — review tur 3 P1-1) ===
    // V1 3-node centroid ({1,2,3}), V2 1-node ({1}). PIN exact bits.
    let (vals_v1, vals_v2) = measurement_value_bits(&obs_v1, &obs_v2, &case.id);
    eprintln!(
        "  wide-affected value bits: V1={:?} V2={:?}",
        vals_v1, vals_v2
    );
    assert_eq!(
        vals_v1,
        [
            4595172819793696085u64,
            4602678819172646912,
            4602678819172646912,
            4602678819172646912,
            4601046424471046557
        ],
        "V1 wide-affected 3-node centroid exact bits; case {}",
        case.id
    );
    assert_eq!(
        vals_v2,
        [
            4602678819172646912u64,
            4602678819172646912,
            4607182418800017408,
            4602678819172646912,
            4601046424471046557
        ],
        "V2 wide-affected 1-node (task scope) exact bits; case {}",
        case.id
    );
    // Coupling (axis 0) ve instability (axis 2) farklı — 3-node vs 1-node centroid.
    assert_ne!(vals_v1[0], vals_v2[0], "coupling divergence");
    assert_ne!(vals_v1[2], vals_v2[2], "instability divergence");

    // === Source divergence (INV-T4 — exact 5-axis snapshot) ===
    use osp_core::coords::MetricSource;
    let (src_v1, src_v2) = measurement_sources(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        src_v1,
        [MetricSource::Scip; 5],
        "V1 uniform Scip (provenanced_from_raw override); case {}",
        case.id
    );
    assert_eq!(
        src_v2,
        [
            MetricSource::TreeSitter,
            MetricSource::Placeholder,
            MetricSource::TreeSitter,
            MetricSource::Heuristic,
            MetricSource::Heuristic,
        ],
        "V2 engine axis defaults; case {}",
        case.id
    );

    // === Corpus observation golden (review tur 5/6 P0-2/P1-2) ===
    // wide-affected: Q5 Vision'da durur → PredicateGate'e ulaşmadı → NotReached.
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::NotReached,
        "wide-affected V1: Q5 Vision'da durdu → PredicateGate NotReached; case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::NotReached,
        "wide-affected V2: Q5 Vision NotReached; case {}",
        case.id
    );
    // **Review tur 6 P1-1:** exact pipeline stage pin (Vision/Vision).
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::StoppedBeforeCommit {
            stage: common::PipelineStage::Vision,
            error: common::PipelineFailureClass::Vision,
        },
        "wide-affected V1 exact: StoppedBeforeCommit{{Vision, Vision}}; case {}",
        case.id
    );
    assert_eq!(
        obs_v2.pipeline,
        common::PipelineObservation::StoppedBeforeCommit {
            stage: common::PipelineStage::Vision,
            error: common::PipelineFailureClass::Vision,
        },
        "wide-affected V2 exact: StoppedBeforeCommit{{Vision, Vision}}; case {}",
        case.id
    );
    // V1 baseline exact golden (review tur 8 P0-2): AffectedCentroid + tüm alanlar.
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::LegacyComputed {
                values_bits,
                sources,
                loss_bits,
                derivation,
            },
        ..
    } = &obs_v1.measurement
    {
        assert_eq!(
            *derivation,
            common::LegacyBaselineDerivation::AffectedCentroid
        );
        assert_eq!(
            *values_bits,
            [
                0u64,
                4602678819172646912,
                4602678819172646912,
                4602678819172646912,
                4601046424471046557
            ]
        );
        assert_eq!(*sources, [osp_core::coords::MetricSource::Scip; 5]);
        assert_eq!(*loss_bits, 4604544271217802189);
    } else {
        panic!("V1 LegacyComputed baseline olmalı; case {}", case.id);
    }
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::Available {
                values_bits,
                sources,
                loss_bits,
            },
        ..
    } = &obs_v2.measurement
    {
        assert_eq!(
            *values_bits,
            [
                0u64,
                4602678819172646912,
                4602678819172646912,
                4602678819172646912,
                4601046424471046557
            ]
        );
        assert_eq!(
            *sources,
            [
                osp_core::coords::MetricSource::TreeSitter,
                osp_core::coords::MetricSource::Placeholder,
                osp_core::coords::MetricSource::TreeSitter,
                osp_core::coords::MetricSource::Heuristic,
                osp_core::coords::MetricSource::Heuristic
            ]
        );
        assert_eq!(*loss_bits, 4604544271217802189);
    } else {
        panic!(
            "wide-affected V2 Available baseline olmalı; case {}",
            case.id
        );
    }
}
///
/// V1 affected = {1, 9} (9 removed_edges.from'dan); V2 subject = {1}.
///
/// **Review P1-1 fix:** exact subject set assertion (hardcode count DEĞİL).
#[test]
fn removed_edge_external_source_shows_affected_contamination() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::RemovedEdgeExternalSource)
        .expect("RemovedEdgeExternalSource case olmalı");
    let (obs_v1, obs_v2) = observe_case(&case);

    // V1 affected = {1, 9} (9 removed_edges.from ile eklenir). PIN exact set.
    let (subj_v1, subj_v2) = measurement_subjects(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        sorted(&subj_v1),
        vec![1, 9],
        "V1 affected = {{1,9}} (affected_nodes + removed_edges.from); got {subj_v1:?}"
    );
    assert_eq!(
        sorted(&subj_v2),
        vec![1],
        "V2 subject = {{1}} (task scope, 9 external); got {subj_v2:?}"
    );
    assert_ne!(
        sorted(&subj_v1),
        sorted(&subj_v2),
        "AFFECTED CONTAMINATION: V1 removed_edges.from external node ile genişler"
    );

    // === Value bits divergence (exact 5-axis snapshot — review tur 3 P1-1) ===
    // V1 2-node centroid ({1,9}), V2 1-node ({1}). PIN exact bits.
    let (vals_v1, vals_v2) = measurement_value_bits(&obs_v1, &obs_v2, &case.id);
    eprintln!(
        "  removed-edge value bits: V1={:?} V2={:?}",
        vals_v1, vals_v2
    );
    assert_eq!(
        vals_v1,
        [
            4598175219545276416u64,
            4602678819172646912,
            4602678819172646912,
            4602678819172646912,
            4601046424471046557
        ],
        "V1 removed-edge 2-node {{1,9}} centroid exact bits; case {}",
        case.id
    );
    assert_eq!(
        vals_v2,
        [
            4602678819172646912u64,
            4602678819172646912,
            4607182418800017408,
            4602678819172646912,
            4601046424471046557
        ],
        "V2 removed-edge 1-node {{1}} exact bits; case {}",
        case.id
    );
    // Coupling (axis 0) ve instability (axis 2) farklı.
    assert_ne!(vals_v1[0], vals_v2[0], "coupling divergence");
    assert_ne!(vals_v1[2], vals_v2[2], "instability divergence");

    // === Source divergence (INV-T4 — exact 5-axis snapshot) ===
    use osp_core::coords::MetricSource;
    let (src_v1, src_v2) = measurement_sources(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        src_v1,
        [MetricSource::Scip; 5],
        "V1 uniform Scip; case {}",
        case.id
    );
    assert_eq!(
        src_v2,
        [
            MetricSource::TreeSitter,
            MetricSource::Placeholder,
            MetricSource::TreeSitter,
            MetricSource::Heuristic,
            MetricSource::Heuristic,
        ],
        "V2 engine axis defaults; case {}",
        case.id
    );

    // === Corpus observation golden (review tur 5 P0-2/P1-2) ===
    // removed-edge: Q5 Vision'da durur → NotReached.
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::NotReached,
        "removed-edge V1: Q5 Vision NotReached; case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::NotReached,
        "removed-edge V2: Q5 Vision NotReached; case {}",
        case.id
    );
    // **Review tur 6 P1-1:** exact pipeline stage pin (Vision/Vision).
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::StoppedBeforeCommit {
            stage: common::PipelineStage::Vision,
            error: common::PipelineFailureClass::Vision,
        },
        "removed-edge V1 exact: StoppedBeforeCommit{{Vision, Vision}}; case {}",
        case.id
    );
    assert_eq!(
        obs_v2.pipeline,
        common::PipelineObservation::StoppedBeforeCommit {
            stage: common::PipelineStage::Vision,
            error: common::PipelineFailureClass::Vision,
        },
        "removed-edge V2 exact: StoppedBeforeCommit{{Vision, Vision}}; case {}",
        case.id
    );
    // V1 baseline exact golden (review tur 8 P0-2): AffectedCentroid + tüm alanlar.
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::LegacyComputed {
                values_bits,
                sources,
                loss_bits,
                derivation,
            },
        ..
    } = &obs_v1.measurement
    {
        assert_eq!(
            *derivation,
            common::LegacyBaselineDerivation::AffectedCentroid
        );
        assert_eq!(
            *values_bits,
            [
                0u64,
                4602678819172646912,
                4602678819172646912,
                4602678819172646912,
                4601046424471046557
            ]
        );
        assert_eq!(*sources, [osp_core::coords::MetricSource::Scip; 5]);
        assert_eq!(*loss_bits, 4604544271217802189);
    } else {
        panic!("V1 LegacyComputed baseline olmalı; case {}", case.id);
    }
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::Available {
                values_bits,
                sources,
                loss_bits,
            },
        ..
    } = &obs_v2.measurement
    {
        assert_eq!(
            *values_bits,
            [
                0u64,
                4602678819172646912,
                4602678819172646912,
                4602678819172646912,
                4601046424471046557
            ]
        );
        assert_eq!(
            *sources,
            [
                osp_core::coords::MetricSource::TreeSitter,
                osp_core::coords::MetricSource::Placeholder,
                osp_core::coords::MetricSource::TreeSitter,
                osp_core::coords::MetricSource::Heuristic,
                osp_core::coords::MetricSource::Heuristic
            ]
        );
        assert_eq!(*loss_bits, 4604544271217802189);
    } else {
        panic!(
            "removed-edge V2 Available baseline olmalı; case {}",
            case.id
        );
    }
}

/// `delta-introduced-subject-001`: baseline epistemic availability divergence
/// (review tur 7-9: availability divergence kanıtlandı, policy/decision hipotez).
///
/// Subject node 10000 delta-introduced (base space'te yok, delta ile geliyor —
/// `node_from_spec` id 10_000+0). Subject authority divergence YOK (V1 affected={10000}
/// == V2 task scope={10000}). Ayrışan ontolojik boyut: **baseline epistemic availability**.
/// - V1: `DefaultFallback` — subject base'de yok → `compute_raw_from_delta` empty
///   positions → `RawPosition::default()` (sıfır koordinat).
/// - V2: `MeasurementBaseline::Unavailable { AllMembersIntroducedByDelta }` (typed).
///
/// **Review tur 7-9:** Bu case **availability divergence** kanıtlar (V1 yokluk → sıfır,
/// V2 typed Unavailable). **Policy/decision divergence KANITLAMAZ** — Case 4 predicate
/// `Coupling ≤ 0.5` + measured coupling 0.0 → `Completed` → `AcceptAsCompleted`
/// (completion-first decision core improved'a bakmaz). Policy etkisi SADECE `NotCompleted`
/// + improvement-sensitive policy dalında observable (P2-0B.8 fixture).
/// dalında observable (P2-0B.8 fixture).
#[test]
fn delta_introduced_subject_shows_baseline_epistemic_availability_divergence() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::DeltaIntroducedSubject)
        .expect("DeltaIntroducedSubject case olmalı");
    let (obs_v1, obs_v2) = observe_case(&case);

    // === Subject set parity (P0-1: subject authority divergence YOK) ===
    let (subj_v1, subj_v2) = measurement_subjects(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        sorted(&subj_v1),
        vec![10_000],
        "V1 affected = {{10000}} (case design); got {subj_v1:?}"
    );
    assert_eq!(
        sorted(&subj_v2),
        vec![10_000],
        "V2 subject = {{10000}} (task scope); got {subj_v2:?}"
    );
    assert_eq!(
        sorted(&subj_v1),
        sorted(&subj_v2),
        "Case 4 SUBJECT AUTHORITY PARITY: V1 affected == V2 task scope (ikisi {{10000}}). \
         Subject authority divergence YOK — ayrışan boyut baseline availability."
    );

    // === Axis value bits parity (subject aynı → centroid aynı) ===
    let (vals_v1, vals_v2) = measurement_value_bits(&obs_v1, &obs_v2, &case.id);
    eprintln!(
        "  delta-introduced value bits: V1={:?} V2={:?}",
        vals_v1, vals_v2
    );
    // Subject aynı ({10000}) → after centroid aynı → value bits eşit.
    assert_eq!(
        vals_v1,
        [
            0u64,
            4602678819172646912,
            4602678819172646912,
            4602678819172646912,
            4601046424471046557
        ],
        "V1 delta-introduced 1-node centroid exact bits; case {}",
        case.id
    );
    assert_eq!(
        vals_v2,
        [
            0u64,
            4602678819172646912,
            4602678819172646912,
            4602678819172646912,
            4601046424471046557
        ],
        "V2 delta-introduced 1-node centroid exact bits (subject parity); case {}",
        case.id
    );
    assert_eq!(
        vals_v1, vals_v2,
        "value parity (subject authority divergence yok)"
    );

    // === Source divergence (INV-T4 — provenance authority, exact snapshot) ===
    use osp_core::coords::MetricSource;
    let (src_v1, src_v2) = measurement_sources(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        src_v1,
        [MetricSource::Scip; 5],
        "V1 uniform Scip; case {}",
        case.id
    );
    assert_eq!(
        src_v2,
        [
            MetricSource::TreeSitter,
            MetricSource::Placeholder,
            MetricSource::TreeSitter,
            MetricSource::Heuristic,
            MetricSource::Heuristic,
        ],
        "V2 engine axis defaults; case {}",
        case.id
    );

    // === Baseline epistemic availability divergence (Migration 3, review tur 7) ===
    // **Review tur 7 P0:** V1 yokluğu sıfır koordinata çevirir (DefaultFallback);
    // V2 typed UnavailableAllIntroduced olarak korur. Bu "typed/untyped representation"
    // değil, **epistemik availability yorumu** farkı. Case 4 subject node 10000 base
    // space'te yok → V1 compute_raw_from_delta empty positions → RawPosition::default()
    // (engine.rs:2329-2330); V2 typed Unavailable.
    match &obs_v2.measurement {
        MeasurementObservation::Produced {
            baseline: common::BaselineObservation::Unavailable(kind),
            ..
        } => {
            assert_eq!(
                *kind,
                common::BaselineKind::UnavailableAllIntroduced,
                "V2 baseline UnavailableAllIntroduced; case {}",
                case.id
            );
        }
        other => panic!("V2 UnavailableAllIntroduced baseline olmalı; got {other:?}"),
    }
    // V1 baseline DefaultFallback exact golden (review tur 8 P0-2): node 10000 base'de
    // yok → compute_raw_from_delta empty positions → RawPosition::default() (tümü sıfır).
    // OSP "bilinmeyeni ölçülmüş değer gibi sunmama" çizgisine aykırı — explicit pin.
    if let MeasurementObservation::Produced {
        baseline:
            common::BaselineObservation::LegacyComputed {
                values_bits,
                sources,
                loss_bits,
                derivation,
            },
        ..
    } = &obs_v1.measurement
    {
        assert_eq!(
            *derivation,
            common::LegacyBaselineDerivation::DefaultFallback
        );
        // RawPosition::default() → tüm axis değerleri 0.0 → to_bits() = 0.
        assert_eq!(
            *values_bits, [0u64; 5],
            "delta-introduced V1 baseline default (all zero); case {}",
            case.id
        );
        assert_eq!(*sources, [osp_core::coords::MetricSource::Scip; 5]);
        // loss_before = trajectory_loss(zero, target); Case 4 target = preferred_vector
        // (None → RawPosition::default() = zero) → loss = 0.
        assert_eq!(
            *loss_bits, 0u64,
            "delta-introduced V1 baseline loss 0 (zero baseline, zero target); case {}",
            case.id
        );
    } else {
        panic!(
            "delta-introduced V1 LegacyComputed baseline olmalı; case {}",
            case.id
        );
    }
    // V2 Unavailable — value/source/loss N/A (review tur 8 P0-2: Unavailable tarafında
    // bu alanlar yoktur; divergence availability düzeyindedir, value düzeyinde DEĞİL).

    // === Pipeline observation golden (review tur 6 P0-1) ===
    // Case 4 Q5 PASSED → PredicateGate'e ulaştı. Held AuthorizationContext taşır
    // (engine.rs:1133) → authorization.outcome gerçek AttemptOutcome verir. Case 4
    // Completed → AcceptAsCompleted (completion-first core improved'a bakmaz).
    // **Review tur 6 P0-1:** Held fabrication yanlıştı — outcome observable.
    eprintln!("  delta-introduced V1 pipeline: {:?}", obs_v1.pipeline);
    eprintln!("  delta-introduced V2 pipeline: {:?}", obs_v2.pipeline);
    use osp_core::trajectory::{ApplyTarget, CommitLane, MutationDecision, PredicateCompletion};
    let expected_case4_pipeline = common::PipelineObservation::CommitReached {
        q5: common::Q5Observation::Passed,
        predicate_completion: Some(PredicateCompletion::Completed),
        mutation_decision: Some(MutationDecision::AcceptAsCompleted),
        apply_target: Some(ApplyTarget::Lane(CommitLane::Mainline)),
        witness_reachability: common::WitnessReachability::Held,
    };
    assert_eq!(
        obs_v1.pipeline, expected_case4_pipeline,
        "delta-introduced V1 exact pipeline (Held+AcceptAsCompleted+Mainline); case {}",
        case.id
    );
    assert_eq!(
        obs_v2.pipeline, expected_case4_pipeline,
        "delta-introduced V2 exact pipeline (Held+AcceptAsCompleted+Mainline); case {}",
        case.id
    );
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::Observed(MutationDecision::AcceptAsCompleted),
        "delta-introduced V1 Observed(AcceptAsCompleted); case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::Observed(MutationDecision::AcceptAsCompleted),
        "delta-introduced V2 Observed(AcceptAsCompleted); case {}",
        case.id
    );
    // **Review tur 6 sonuçu:** Case 4 pipeline mutation-decision ÖLÇÜLDÜ ve eşit
    // (Observed(AcceptAsCompleted) V1/V2 parity). Baseline policy/decision divergence
    // hâlâ hipotez — Case 4 Completed → projection etkisiz. P2-0B.8 NotCompleted +
    // AcceptImprovement fixture gerek.
}

// ═══════════════════════════════════════════════════════════════════════════════
// Measurement snapshot extraction helpers (review P0-2 / P1-1)
// ═══════════════════════════════════════════════════════════════════════════════

fn measurement_subjects(
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
    case_id: &str,
) -> (Vec<u64>, Vec<u64>) {
    let s1 = match &obs_v1.measurement {
        MeasurementObservation::Produced { subject, .. } => subject.clone(),
        other => panic!("V1 measurement Produced olmalı; case {case_id} got {other:?}"),
    };
    let s2 = match &obs_v2.measurement {
        MeasurementObservation::Produced { subject, .. } => subject.clone(),
        MeasurementObservation::Failed { .. } => vec![], // V2 failure — subject boş
        other => panic!("V2 measurement Produced/Failed olmalı; case {case_id} got {other:?}"),
    };
    (s1, s2)
}

fn measurement_value_bits(
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
    case_id: &str,
) -> ([u64; 5], [u64; 5]) {
    let v1 = match &obs_v1.measurement {
        MeasurementObservation::Produced { values_bits, .. } => *values_bits,
        other => panic!("V1 Produced olmalı; case {case_id} got {other:?}"),
    };
    let v2 = match &obs_v2.measurement {
        MeasurementObservation::Produced { values_bits, .. } => *values_bits,
        other => panic!("V2 Produced olmalı; case {case_id} got {other:?}"),
    };
    (v1, v2)
}

fn measurement_sources(
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
    case_id: &str,
) -> (
    [osp_core::coords::MetricSource; 5],
    [osp_core::coords::MetricSource; 5],
) {
    use osp_core::coords::MetricSource;
    let default_arr = || [MetricSource::Placeholder; 5];
    let s1 = match &obs_v1.measurement {
        MeasurementObservation::Produced { sources, .. } => *sources,
        other => panic!("V1 Produced olmalı; case {case_id} got {other:?}"),
    };
    let s2 = match &obs_v2.measurement {
        MeasurementObservation::Produced { sources, .. } => *sources,
        // V2 failure → sources meaningless; default placeholder for diff display.
        MeasurementObservation::Failed { .. } => default_arr(),
        other => panic!("V2 Produced/Failed olmalı; case {case_id} got {other:?}"),
    };
    (s1, s2)
}

fn sorted(v: &[u64]) -> Vec<u64> {
    let mut v = v.to_vec();
    v.sort_unstable();
    v
}

// ═══════════════════════════════════════════════════════════════════════════════
// required_source decision matrix (review P0-2 — INV-T4 provenance-authority)
// ═══════════════════════════════════════════════════════════════════════════════

/// **Review tur 2 P0-2 + tur 3 P0-1 fix: required_source decision matrix.**
///
/// Matching scope'ta BİLE V1 coupling source = Scip (provenanced_from_raw override),
/// V2 coupling source = TreeSitter (engine axis default). Bu, subject-authority'den
/// **bağımsız** bir provenance-authority divergence'ı.
///
/// **Tur 3 P0-1 fix:** Önceki test sadece lokal `bool` karşılaştırması yapıyordu
/// (`Scip != TreeSitter` kanıtı). Artık gerçek `PredicateSet::evaluate_completion`
/// çağrılıp `PredicateSetResult` (Completed/SourceInsufficient/NotCompleted) exact
/// pinleniyor. Bu, **PredicateSet-level** decision divergence'ı gerçek evaluation ile
/// kanıtlar (PredicateGate/pipeline-level DEĞİL — review tur 6 terminoloji).
#[test]
fn required_source_matrix_shows_provenance_authority_divergence() {
    use osp_core::coords::MetricSource;
    use osp_core::space::{Node, NodeKind};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        PredicateSetResult, TaskPolicy, TaskStatus, WeightedPredicate,
    };

    // Inline matching case: node 1 space'te, task Node(1) scope, affected=[1].
    let mut space = osp_core::space::Space::new();
    space.insert_node(Node {
        id: 1,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    // V1 coupling source = Scip, V2 = TreeSitter (sabit — engine axis defaults).
    let v1_coupling_source = MetricSource::Scip;
    let v2_coupling_source = MetricSource::TreeSitter;

    // Her required_source için beklenen PredicateSetResult exact matrix (tur 3 P0-1).
    // Predicate: Coupling ≤ 0.5 (coupling axis threshold; gerçek measured coupling 0.5
    // veya altında olduğu sürece completion source-driven olur).
    use osp_core::agent::{DeltaProposal, NewEdgeSpec};
    use osp_core::space::EdgeKind;
    let space_for_case = space.clone();

    let cases: [(Option<MetricSource>, PredicateSetResult, PredicateSetResult); 5] = [
        // (required_source, V1_expected, V2_expected)
        (
            None,
            PredicateSetResult::Completed,
            PredicateSetResult::Completed,
        ),
        (
            Some(MetricSource::Scip),
            PredicateSetResult::Completed,
            PredicateSetResult::SourceInsufficient,
        ),
        (
            Some(MetricSource::TreeSitter),
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::Completed,
        ),
        (
            Some(MetricSource::Placeholder),
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
        (
            Some(MetricSource::Heuristic),
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
    ];

    for (req_source, expected_v1, expected_v2) in cases {
        let predicate = MetricPredicate {
            metric: PredicateAxis::Coupling,
            operator: ComparisonOp::Le,
            threshold: 0.5,
            scope: PredicateScope::Node(1),
            required_source: req_source,
            tolerance: 0.0,
        };
        let ps = PredicateSet {
            mode: PredicateMode::All,
            predicates: vec![WeightedPredicate {
                predicate,
                weight: None,
            }],
            preferred_vector: None,
        };
        let task = osp_core::trajectory::Task {
            id: 42,
            milestone_id: 0,
            label: format!("required-source-{:?}-inline", req_source),
            target_predicate_set: ps.clone(),
            policy: TaskPolicy::default(),
            allowed_operations: vec![],
            constraints: vec![],
            status: TaskStatus::Pending,
        };
        let proposal = DeltaProposal {
            new_edges: vec![NewEdgeSpec {
                from: 1,
                to: 2,
                kind: EdgeKind::Imports,
            }],
            affected_nodes: vec![1],
            ..Default::default()
        };
        let case = common::CharacterizationCase {
            id: format!("required-source-{:?}-inline", req_source),
            class: common::CaseClass::MatchingScope,
            source: common::CaseSource::SyntheticAdversarial,
            description: "inline required_source matrix case".to_string(),
            space: space_for_case.clone(),
            task: task.clone(),
            proposal,
        };

        let mut engine_v1 = common::engine_with_case_space(&case);
        let mut engine_v2 = common::engine_with_case_space(&case);
        let obs_v1 = common::evaluate_v1_case(&mut engine_v1, &case);
        let obs_v2 = common::evaluate_v2_candidate_case(&mut engine_v2, &case);

        // V1/V2 measured_after'ı çek.
        let measured_v1 = match &obs_v1.measurement {
            MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
            other => panic!("V1 Produced olmalı; req_source={req_source:?} got {other:?}"),
        };
        let measured_v2 = match &obs_v2.measurement {
            MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
            other => panic!("V2 Produced olmalı; req_source={req_source:?} got {other:?}"),
        };

        // **Tur 3 P0-1 fix:** GERÇEK PredicateSet::evaluate_completion çağrısı.
        let v1_result = ps.evaluate_completion(&measured_v1);
        let v2_result = ps.evaluate_completion(&measured_v2);

        eprintln!(
            "  required_source={:?}: V1 coupling src={:?} result={:?} (expected {:?}), \
             V2 coupling src={:?} result={:?} (expected {:?})",
            req_source,
            v1_coupling_source,
            v1_result,
            expected_v1,
            v2_coupling_source,
            v2_result,
            expected_v2
        );

        // Exact PredicateSetResult pin.
        assert_eq!(
            v1_result, expected_v1,
            "V1 PredicateSetResult mismatch; required_source={req_source:?}"
        );
        assert_eq!(
            v2_result, expected_v2,
            "V2 PredicateSetResult mismatch; required_source={req_source:?}"
        );

        // Decision divergence kanıtı: Scip/TreeSitter için V1/V2 farklı karar.
        if req_source == Some(MetricSource::Scip) || req_source == Some(MetricSource::TreeSitter) {
            assert_ne!(
                v1_result, v2_result,
                "INV-T4 PROVENANCE-AUTHORITY DECISION DIVERGENCE: required_source={:?} \
                 için V1 {:?} ≠ V2 {:?} — PredicateSet::evaluate_completion farklı sonuç",
                req_source, v1_result, v2_result
            );
        }
    }
}
