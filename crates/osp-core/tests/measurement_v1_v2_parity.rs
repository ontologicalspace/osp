//! **HISTORICAL PRE-#96 CHARACTERIZATION CONTRACT.**
//! Bu dosya bilinçli olarak legacy/reference karşılaştırmasını korur — #96 MD-2
//! production cutover SONRASI üretim authority truth'u DEĞİLDİR (post-#96:
//! engine-native per-axis = mutation authority; production native flow
//! draft → measure → finalize → sealed carrier → commit kullanır; bkz.
//! `docs/notes/faz8-p2-migration-decisions.md` MD-2 implementation).
//! V1 lane non-authoritative reference evaluation'dır (commit pipeline yok —
//! `PipelineObservation::ReferenceEvaluation`).
//!
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
    // #92: ID bazlı resolution — class bazlı find 002 family eklenince ambiguous olurdu.
    let case = common::case_by_id("matching-single-node-001");
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
    // **#96 MD-2 P0-tur5:** V1 lane non-authoritative evaluator (commit pipeline
    // YOK) → Q5 vision gözlenemez (NotObserved; fabrication YOK), PredicateGate
    // direkt çalışır (gerçek karar yüzeyi). V2 gerçek native flow → vision'da
    // durur → Q5 Rejected. Eski "iki lane de Vision'da durur" parity gözlemi
    // artık geçerli DEĞİL.
    assert_q5_disposition_divergence_v1_evaluator_v2_pipeline(&case.id, &obs_v1, &obs_v2);
    assert_predicate_completion_parity(&case.id, &obs_v1, &obs_v2);
    assert_mutation_decision_parity(&case.id, &obs_v1, &obs_v2);
    assert_apply_target_parity(&case.id, &obs_v1, &obs_v2);
    // **P0-tur5:** witness yüzeyi — V1 evaluator witness aşamasına hiç gitmez
    // (NotReached; eski Evaluated pin'i gözlenmeyen yüzeyi gözlenmiş gibi
    // kodluyordu), V2 gerçek pipeline vision'da durur → NotReached.
    assert_eq!(
        witness_reachability(&obs_v1),
        WitnessReachability::NotReached,
        "witness V1 exact = NotReached (evaluator witness aşamasına gitmez); case {}",
        case.id
    );
    assert_eq!(
        witness_reachability(&obs_v2),
        WitnessReachability::NotReached,
        "witness V2 exact = NotReached (pipeline vision'da durdu); case {}",
        case.id
    );

    // === Corpus observation golden (review tur 5/6 P0-2/P1-2; #96 MD-2 P0-tur5) ===
    // V1 evaluator PredicateGate'e ULAŞIR (gerçek karar yüzeyi); Q5 vision
    // motor-private olduğundan gözlenemez → NotObserved (fabrication YOK). V2
    // gerçek pipeline vision'da durur → NotReached (değişmedi).
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::Observed(
            osp_core::trajectory::MutationDecision::AcceptAsCompleted
        ),
        "matching V1: evaluator PredicateGate'e ulaşır → Observed(AcceptAsCompleted); case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::NotReached,
        "matching V2: Q5 Vision NotReached; case {}",
        case.id
    );
    // **Review tur 6 P1-1:** exact pipeline pin. V1 (P1-tur6): ReferenceEvaluation
    // — production pipeline ÇALIŞMADI, yalnız PredicateGate koştu (karar alanları
    // gerçek gate çıktısı). V2: StoppedBeforeCommit{Vision, Vision}.
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::ReferenceEvaluation {
            predicate_completion: Some(osp_core::trajectory::PredicateCompletion::Completed),
            mutation_decision: Some(osp_core::trajectory::MutationDecision::AcceptAsCompleted),
            apply_target: Some(osp_core::trajectory::ApplyTarget::Lane(
                osp_core::trajectory::CommitLane::Mainline
            )),
        },
        "matching V1 exact pipeline (ReferenceEvaluation; #96 P1-tur6); case {}",
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

fn q5_disposition(obs: &CharacterizationObservation) -> Q5Disposition {
    match &obs.pipeline {
        PipelineObservation::CommitReached { q5, .. } => match q5 {
            common::Q5Observation::Passed => Q5Disposition::Passed,
            common::Q5Observation::Rejected { .. } => Q5Disposition::Rejected,
            common::Q5Observation::NotReached => Q5Disposition::NotReached,
            // **P0-tur5:** evaluator Q5'i çalıştıramadı — gözlenmedi (fabrication YOK).
            common::Q5Observation::NotObserved => Q5Disposition::NotObserved,
        },
        // **P1-tur6:** reference evaluation — production pipeline çalışmadı;
        // Q5 gözlenmedi (variant'ın kendisi bunu belirtir).
        PipelineObservation::ReferenceEvaluation { .. } => Q5Disposition::NotObserved,
        PipelineObservation::StoppedBeforeCommit { stage, .. } => match stage {
            common::PipelineStage::Vision => Q5Disposition::Rejected,
            _ => Q5Disposition::NotReached,
        },
    }
}

/// **#96 MD-2 P0-tur5 (review P1 — truth-surface):** V1 non-authoritative
/// evaluator Q5 vision'ı çalıştıramaz (motor-private) → `NotObserved`
/// (eski turdaki `Passed` pin'i gözlenmeyen yüzeyi gözlenmiş gibi kodluyordu —
/// fabrication). V2 gerçek pipeline vision'da durur → `Rejected`. Eski
/// `assert_q5_disposition_parity` her iki yüzden de geçersiz — lane'lerin
/// exact disposition'ları ayrı ayrı pinlenir.
fn assert_q5_disposition_divergence_v1_evaluator_v2_pipeline(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    assert_eq!(
        q5_disposition(obs_v1),
        Q5Disposition::NotObserved,
        "Q5 disposition V1 exact = NotObserved (evaluator vision'ı çalıştıramaz); case {case_id}"
    );
    assert_eq!(
        q5_disposition(obs_v2),
        Q5Disposition::Rejected,
        "Q5 disposition V2 exact = Rejected (gerçek pipeline vision'da durur); case {case_id}"
    );
}

#[derive(Debug, PartialEq)]
enum Q5Disposition {
    Passed,
    Rejected,
    NotReached,
    NotObserved,
}

/// **P1-tur6:** Karar yüzeyi extractor — hem gerçek commit yüzeyinden
/// (`CommitReached`) hem reference evaluation'dan (`ReferenceEvaluation`)
/// karar alanlarını çıkarır. Stopped → None (parity N/A). Bu sayede parity
/// assertion'ları V1 evaluator/V2 pipeline şeklini ayırt ederken karar
/// karşılaştırması yaşamaya devam eder.
fn decision_surface(
    obs: &CharacterizationObservation,
) -> Option<(
    Option<osp_core::trajectory::PredicateCompletion>,
    Option<osp_core::trajectory::MutationDecision>,
    Option<osp_core::trajectory::ApplyTarget>,
)> {
    match &obs.pipeline {
        PipelineObservation::CommitReached {
            predicate_completion,
            mutation_decision,
            apply_target,
            ..
        }
        | PipelineObservation::ReferenceEvaluation {
            predicate_completion,
            mutation_decision,
            apply_target,
        } => Some((*predicate_completion, *mutation_decision, *apply_target)),
        PipelineObservation::StoppedBeforeCommit { .. } => None,
    }
}

fn assert_predicate_completion_parity(
    case_id: &str,
    obs_v1: &CharacterizationObservation,
    obs_v2: &CharacterizationObservation,
) {
    let (Some((pc_v1, _, _)), Some((pc_v2, _, _))) =
        (decision_surface(obs_v1), decision_surface(obs_v2))
    else {
        return; // en az biri stopped → Q5.b çalışmadı, parity N/A
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
    let (Some((_, md_v1, _)), Some((_, md_v2, _))) =
        (decision_surface(obs_v1), decision_surface(obs_v2))
    else {
        return;
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
    let (Some((_, _, at_v1)), Some((_, _, at_v2))) =
        (decision_surface(obs_v1), decision_surface(obs_v2))
    else {
        return;
    };
    assert_eq!(
        at_v1, at_v2,
        "ApplyTarget parity fail; case {case_id}: V1={at_v1:?} V2={at_v2:?}"
    );
}

fn witness_reachability(obs: &CharacterizationObservation) -> WitnessReachability {
    match &obs.pipeline {
        PipelineObservation::CommitReached {
            witness_reachability,
            ..
        } => witness_reachability.clone(),
        // **P1-tur6:** reference evaluation — witness aşamasına hiç gidilmedi.
        PipelineObservation::ReferenceEvaluation { .. } => WitnessReachability::NotReached,
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
    // #92: ID bazlı — 001 tarihsel golden'ları 001'e bağlı (002 family eklendi).
    let case = common::case_by_id("wide-affected-scope-001");
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

    // === Corpus observation golden (review tur 5/6 P0-2/P1-2; #96 MD-2 re-anchor) ===
    // V1 evaluator vision'ı değerlendirmez → PredicateGate'e ulaşır. V2 gerçek
    // pipeline Q5 Vision'da durur → NotReached (değişmedi).
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::Observed(
            osp_core::trajectory::MutationDecision::AcceptAsCompleted
        ),
        "wide-affected V1: evaluator PredicateGate'e ulaşır → Observed(AcceptAsCompleted); case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::NotReached,
        "wide-affected V2: Q5 Vision NotReached; case {}",
        case.id
    );
    // **Review tur 6 P1-1:** exact pipeline pin. V1 (P1-tur6): ReferenceEvaluation
    // — production pipeline çalışmadı, yalnız PredicateGate koştu. V2:
    // StoppedBeforeCommit{Vision, Vision}.
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::ReferenceEvaluation {
            predicate_completion: Some(osp_core::trajectory::PredicateCompletion::Completed),
            mutation_decision: Some(osp_core::trajectory::MutationDecision::AcceptAsCompleted),
            apply_target: Some(osp_core::trajectory::ApplyTarget::Lane(
                osp_core::trajectory::CommitLane::Mainline
            )),
        },
        "wide-affected V1 exact pipeline (ReferenceEvaluation; #96 P1-tur6); case {}",
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
    // #92: ID bazlı — 001 tarihsel golden'ları 001'e bağlı (002 family eklendi).
    let case = common::case_by_id("removed-edge-external-source-001");
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

    // === Corpus observation golden (review tur 5 P0-2/P1-2; #96 MD-2 re-anchor) ===
    // V1 evaluator vision'ı değerlendirmez → PredicateGate'e ulaşır. V2 gerçek
    // pipeline Q5 Vision'da durur → NotReached (değişmedi).
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::Observed(
            osp_core::trajectory::MutationDecision::AcceptAsCompleted
        ),
        "removed-edge V1: evaluator PredicateGate'e ulaşır → Observed(AcceptAsCompleted); case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::NotReached,
        "removed-edge V2: Q5 Vision NotReached; case {}",
        case.id
    );
    // **Review tur 6 P1-1:** exact pipeline pin. V1 (P1-tur6): ReferenceEvaluation
    // — production pipeline çalışmadı, yalnız PredicateGate koştu. V2:
    // StoppedBeforeCommit{Vision, Vision}.
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::ReferenceEvaluation {
            predicate_completion: Some(osp_core::trajectory::PredicateCompletion::Completed),
            mutation_decision: Some(osp_core::trajectory::MutationDecision::AcceptAsCompleted),
            apply_target: Some(osp_core::trajectory::ApplyTarget::Lane(
                osp_core::trajectory::CommitLane::Mainline
            )),
        },
        "removed-edge V1 exact pipeline (ReferenceEvaluation; #96 P1-tur6); case {}",
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
/// (review tur 7-9: availability divergence kanıtlandı; policy/decision divergence
/// sibling `delta-introduced-subject-policy-001` tarafından KANITLANDI).
///
/// Subject node 10000 delta-introduced (base space'te yok, delta ile geliyor —
/// `node_from_spec` id 10_000+0). Subject authority divergence YOK (V1 affected={10000}
/// == V2 task scope={10000}). Ayrışan ontolojik boyut: **baseline epistemic availability**.
/// - V1: `DefaultFallback` — subject base'de yok → `compute_raw_from_delta` empty
///   positions → `RawPosition::default()` (sıfır koordinat).
/// - V2: `MeasurementBaseline::Unavailable { AllMembersIntroducedByDelta }` (typed).
///
/// Bu case **availability divergence** kanıtlar (V1 yokluk → sıfır, V2 typed
/// Unavailable). **Policy/decision divergence bu case'te observable DEĞİL** — Case 4
/// predicate `Coupling ≤ 0.5` + measured coupling 0.0 → `Completed` → `AcceptAsCompleted`
/// (completion-first decision core improved'a bakmaz). Policy etkisi sibling
/// `delta-introduced-subject-policy-001` fixture'ında (NotCompleted + AcceptImprovement
/// dalı) KANITLANDI — V1 AcceptAsProgress vs V2 Reject.
#[test]
fn delta_introduced_subject_shows_baseline_epistemic_availability_divergence() {
    // **Hardening:** Exact ID ile seçim — `delta_introduced_subject_policy_001` de aynı
    // `CaseClass::DeltaIntroducedSubject` altında olduğu için class-only selection bu testi
    // sessizce yanlış case'e kaydırabilirdi. Builder sırası değişse bile exact ID seçer.
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.id == "delta-introduced-subject-001")
        .expect("delta-introduced-subject-001 case olmalı");
    assert_eq!(
        case.class,
        CaseClass::DeltaIntroducedSubject,
        "class invariant — exact ID seçiminden sonra class hala DeltaIntroducedSubject"
    );
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

    // === Pipeline observation golden (review tur 6 P0-1; #96 MD-2 P1-tur6) ===
    // Case 4 karar yüzeyi (completion/mutation/apply) lane'ler arasında parity.
    // **P1-tur6:** V1 = ReferenceEvaluation — production pipeline çalışmadı,
    // yalnız PredicateGate koştu (reachability fabrication YOK). V2 gerçek
    // pipeline: Q5 Passed + boş witness ile Held (authorization.outcome gerçek
    // AttemptOutcome — fabrication DEĞİL).
    eprintln!("  delta-introduced V1 pipeline: {:?}", obs_v1.pipeline);
    eprintln!("  delta-introduced V2 pipeline: {:?}", obs_v2.pipeline);
    use osp_core::trajectory::{ApplyTarget, CommitLane, MutationDecision, PredicateCompletion};
    let expected_case4_pipeline_v1 = common::PipelineObservation::ReferenceEvaluation {
        predicate_completion: Some(PredicateCompletion::Completed),
        mutation_decision: Some(MutationDecision::AcceptAsCompleted),
        apply_target: Some(ApplyTarget::Lane(CommitLane::Mainline)),
    };
    let expected_case4_pipeline_v2 = common::PipelineObservation::CommitReached {
        q5: common::Q5Observation::Passed,
        predicate_completion: Some(PredicateCompletion::Completed),
        mutation_decision: Some(MutationDecision::AcceptAsCompleted),
        apply_target: Some(ApplyTarget::Lane(CommitLane::Mainline)),
        witness_reachability: common::WitnessReachability::Held,
    };
    assert_eq!(
        obs_v1.pipeline, expected_case4_pipeline_v1,
        "delta-introduced V1 exact pipeline (ReferenceEvaluation; gate gerçekten koştu); case {}",
        case.id
    );
    assert_eq!(
        obs_v2.pipeline, expected_case4_pipeline_v2,
        "delta-introduced V2 exact pipeline (Held+AcceptAsCompleted+Mainline; gerçek pipeline); case {}",
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
    // (Observed(AcceptAsCompleted) V1/V2 parity). Case 4 Completed → projection etkisiz;
    // policy/decision divergence bu case'te observable DEĞİL — sibling
    // `delta-introduced-subject-policy-001` fixture'ı (NotCompleted + AcceptImprovement)
    // divergence'ı KANITLADI.
}

// ═══════════════════════════════════════════════════════════════════════════════
// P2-0B.8 — Policy fixture (delta-introduced-subject-policy-001)
//
// V1 legacy DefaultFallback baseline projection ile V2 candidate fail-closed projection
// arasında migration-relevant policy/decision divergence üreten NotCompleted +
// AcceptImprovement case. Migration 3 (b) hipotezini besler. Case geometrisi reciprocal
// Imports edges (Ce=1, Ca=1 → instability 0.5) + preferred_vector (0.8, 0.5, 0.5) ile
// kurulu — tek outgoing edge Ce=1/Ca=0 → instability 1.0 → max_instability=0.85 hard-cap
// ihlali → improved=false olurdu; reciprocal edge bunu önler.
//
// Golden değerler probe ile ölçüldü, literal olarak pinlendi. Production-reachable
// structural topology üzerinde gelecekteki migration'ın policy/decision divergence'ını
// karakterize eder; iki production implementation'ı karşılaştırmaz.
// ═══════════════════════════════════════════════════════════════════════════════

/// **V1/V2 policy/decision divergence:** `delta-introduced-subject-policy-001` NotCompleted +
/// AcceptImprovement dalında iki projection arasında karar ayrışması üretir.
///
/// - V1: DefaultFallback zero baseline + target (0.8,0.5,0.5) → loss_before ≈ 1.068;
///   measured after (0.5,0.5,0.5) → loss_after = 0.3 → improved=true →
///   `AcceptAsProgress` → `Lane(TrajectoryCheckpoint)` (INV-T8) → `Held`.
/// - V2: `project_v1_loss_before_compatibility_v2` Unavailable dalı → loss_before=loss_after
///   → improved=false → `Reject` → `NotApplied` (INV-T8), witness değerlendirilmez →
///   `Evaluated` (engine.rs:1456-1464 Reject early-return).
///
/// Subject/value/source/baseline tüm alanlar frozen golden olarak pinlenir.
#[test]
fn delta_introduced_policy_shows_v1_v2_decision_divergence() {
    use osp_core::coords::MetricSource;
    use osp_core::trajectory::{ApplyTarget, CommitLane, MutationDecision, PredicateCompletion};
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.id == "delta-introduced-subject-policy-001")
        .expect("delta-introduced-subject-policy-001 case olmalı");
    assert_eq!(
        case.class,
        CaseClass::DeltaIntroducedSubject,
        "class invariant — exact ID seçiminden sonra class hala DeltaIntroducedSubject"
    );

    assert_engines_start_from_identical_space(&case);
    let (obs_v1, obs_v2) = observe_case(&case);

    // === Subject set parity (V1 affected == V2 task scope == {10000}) ===
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

    // === V1 measurement produced — frozen golden (probe'dan) ===
    let (v1_measured, v1_baseline_loss_bits) = match &obs_v1.measurement {
        common::MeasurementObservation::Produced {
            measured_after,
            values_bits,
            sources,
            baseline:
                common::BaselineObservation::LegacyComputed {
                    values_bits: bv,
                    sources: bs,
                    loss_bits,
                    derivation: common::LegacyBaselineDerivation::DefaultFallback,
                },
            ..
        } => {
            // V1 measured-after 5-axis value bits (probe golden).
            assert_eq!(
                *values_bits,
                [
                    4602678819172646912u64, // coupling 0.5
                    4602678819172646912,    // cohesion 0.5
                    4602678819172646912,    // instability 0.5
                    4602678819172646912,    // entropy 0.5
                    4601046424471046557,    // witness_depth ≈ 0.4094
                ],
                "V1 measured-after value bits (case {})",
                case.id
            );
            // V1 measured-after sources — uniform Scip (provenanced_from_raw override).
            assert_eq!(
                *sources,
                [MetricSource::Scip; 5],
                "V1 measured-after uniform Scip (case {})",
                case.id
            );
            // V1 baseline — DefaultFallback zero + uniform Scip + probe golden loss bits.
            assert_eq!(
                *bv, [0u64; 5],
                "V1 DefaultFallback zero baseline (case {})",
                case.id
            );
            assert_eq!(
                *bs,
                [MetricSource::Scip; 5],
                "V1 baseline uniform Scip (case {})",
                case.id
            );
            assert_eq!(
                *loss_bits, 4607487347736372296u64,
                "V1 DefaultFallback baseline loss bits (loss_before ≈ 1.0677); case {}",
                case.id
            );
            (measured_after.clone(), *loss_bits)
        }
        other => panic!("V1 produced + LegacyComputed(DefaultFallback) olmalı; got {other:?}"),
    };

    // === V2 measurement produced — UnavailableAllIntroduced + INV-T4 provenance divergence ===
    let v2_after_sources = match &obs_v2.measurement {
        common::MeasurementObservation::Produced {
            values_bits,
            sources,
            baseline:
                common::BaselineObservation::Unavailable(common::BaselineKind::UnavailableAllIntroduced),
            ..
        } => {
            // V2 measured-after value bits — V1 ile parity (subject/value authority parity).
            assert_eq!(
                *values_bits,
                [
                    4602678819172646912u64,
                    4602678819172646912,
                    4602678819172646912,
                    4602678819172646912,
                    4601046424471046557,
                ],
                "V2 measured-after value bits (subject parity); case {}",
                case.id
            );
            // V2 sources — engine-native per-axis (INV-T4) — V1 uniform Scip'ten DIVERGE.
            assert_eq!(
                *sources,
                [
                    MetricSource::TreeSitter,  // coupling
                    MetricSource::Placeholder, // cohesion
                    MetricSource::TreeSitter,  // instability
                    MetricSource::Heuristic,   // entropy
                    MetricSource::Heuristic,   // witness_depth
                ],
                "V2 gerçek engine axis sources (INV-T4 native provenance); case {}",
                case.id
            );
            *sources
        }
        other => {
            panic!("V2 produced + Unavailable(UnavailableAllIntroduced) olmalı; got {other:?}")
        }
    };
    assert_ne!(
        v2_after_sources,
        [MetricSource::Scip; 5],
        "INV-T4 PROVENANCE DIVERGENCE: V2 sources ≠ V1 uniform Scip; case {}",
        case.id
    );

    // === Decision-input scalar frozen evidence (PR #91 review P1) ===
    //
    // V2 candidate fail-closed projection'ın MERKEZİ iddiası: commit_task_claim'e
    // geçirilen loss_before == loss_after. Helper `project_v1_loss_before_compatibility_v2`
    // gelecekte değişse (Unavailable → 0.0 / NAN / loss_after-0.01) decision sonucu (Reject)
    // aynı kalabilir — bu yüzden projection scalar'ı kendisi frozen evidence olmalı.
    let v1_decision = obs_v1
        .decision_input
        .expect("V1 commit'e ulaştı → decision_input Some olmalı");
    let v2_decision = obs_v2
        .decision_input
        .expect("V2 commit'e ulaştı → decision_input Some olmalı");

    // V1: commit_task_claim'e geçirilen loss_before, observation baseline loss_bits ile
    // AYNI kaynak (trajectory_loss(current_measured, target)) — frozen evidence parity.
    assert_eq!(
        v1_decision.loss_before_bits, v1_baseline_loss_bits,
        "V1 decision-input loss_before == baseline loss_bits (aynı current_measured kaynağı); case {}",
        case.id
    );
    // V1 improved önkoşulu: loss_after < loss_before - min_delta.
    let loss_before = f64::from_bits(v1_decision.loss_before_bits);
    let loss_after = f64::from_bits(v1_decision.loss_after_bits);
    assert!(
        loss_after < loss_before - case.task.policy.min_improvement_delta,
        "V1 improved önkoşulu: loss_after ({}) < loss_before ({}) - min_delta ({}); case {}",
        loss_after,
        loss_before,
        case.task.policy.min_improvement_delta,
        case.id
    );

    // V2: fail-closed equality projection — loss_before == loss_after (frozen evidence).
    assert_eq!(
        v2_decision.loss_before_bits, v2_decision.loss_after_bits,
        "V2 candidate fail-closed projection: loss_before == loss_after; case {}",
        case.id
    );
    // V2 loss_after == V1 loss_after (aynı measured (0.5,0.5,0.5) + aynı target →
    // aynı trajectory_loss). loss_after literal ≈ 0.3 (sqrt(0.09)) — ULP-hassas olduğu
    // için literal pin YERİNE V1 loss_after parity'si (aynı computed loss) pinlenir.
    assert_eq!(
        v2_decision.loss_after_bits, v1_decision.loss_after_bits,
        "V2 loss_after == V1 loss_after (aynı measured + target → aynı loss); case {}",
        case.id
    );
    // V2 fail-closed: loss_before = loss_after (equality projection).
    assert_eq!(
        v2_decision.loss_before_bits, v2_decision.loss_after_bits,
        "V2 fail-closed: loss_before = loss_after (Unavailable → loss_after projection); case {}",
        case.id
    );

    // === Hard-cap önkoşulları direct assert (PR #91 review P1) ===
    //
    // improved için üç hard-cap assess_improvement_v1 içinde kontrol edilir. Threshold
    // EffectiveImprovementPolicy::current_semantics()'ten (literal 0.85/0.85/0.15 DEĞİL);
    // threshold değişirse bu assertion güncellenir.
    let improvement_policy = osp_core::trajectory::EffectiveImprovementPolicy::current_semantics();
    assert!(
        v1_measured.coupling.value < improvement_policy.max_coupling,
        "coupling hard-cap: {} < {}; case {}",
        v1_measured.coupling.value,
        improvement_policy.max_coupling,
        case.id
    );
    assert!(
        v1_measured.instability.value < improvement_policy.max_instability,
        "instability hard-cap: {} < {}; case {}",
        v1_measured.instability.value,
        improvement_policy.max_instability,
        case.id
    );
    assert!(
        v1_measured.cohesion.value > improvement_policy.min_cohesion,
        "cohesion hard-cap: {} > {}; case {}",
        v1_measured.cohesion.value,
        improvement_policy.min_cohesion,
        case.id
    );

    // === EXACT V1 pipeline — AcceptAsProgress → TrajectoryCheckpoint (INV-T8) ===
    // **P1-tur6:** V1 = ReferenceEvaluation — production pipeline çalışmadı;
    // yalnız gate koştu (gerçek pipeline V1'in eski Held pin'ine aitti).
    let expected_v1 = common::PipelineObservation::ReferenceEvaluation {
        predicate_completion: Some(PredicateCompletion::NotCompleted),
        mutation_decision: Some(MutationDecision::AcceptAsProgress),
        apply_target: Some(ApplyTarget::Lane(CommitLane::TrajectoryCheckpoint)),
    };
    assert_eq!(
        obs_v1.pipeline, expected_v1,
        "V1 AcceptAsProgress → TrajectoryCheckpoint (INV-T8; reference evaluation); case {}",
        case.id
    );

    // === EXACT V2 pipeline — Reject → NotApplied → Evaluated (witness yok, INV-T8) ===
    let expected_v2 = common::PipelineObservation::CommitReached {
        q5: common::Q5Observation::Passed,
        predicate_completion: Some(PredicateCompletion::NotCompleted),
        mutation_decision: Some(MutationDecision::Reject),
        apply_target: Some(ApplyTarget::NotApplied),
        witness_reachability: common::WitnessReachability::Evaluated,
    };
    assert_eq!(
        obs_v2.pipeline, expected_v2,
        "V2 Reject → NotApplied → Evaluated (Reject early-return, witness yok); case {}",
        case.id
    );

    // === DECISION DIVERGENCE — Migration 3 (b) hipotezi kanıtlandı ===
    assert_ne!(
        obs_v1.pipeline, obs_v2.pipeline,
        "POLICY/DECISION DIVERGENCE: V1 AcceptAsProgress ≠ V2 Reject (NotCompleted + \
         AcceptImprovement + DefaultFallback baseline projection); case {}",
        case.id
    );
    let md_v1 = common::MutationDecisionObservation::from_observations(&obs_v1.pipeline);
    let md_v2 = common::MutationDecisionObservation::from_observations(&obs_v2.pipeline);
    assert_eq!(
        md_v1,
        common::MutationDecisionObservation::Observed(MutationDecision::AcceptAsProgress),
        "V1 Observed(AcceptAsProgress); case {}",
        case.id
    );
    assert_eq!(
        md_v2,
        common::MutationDecisionObservation::Observed(MutationDecision::Reject),
        "V2 Observed(Reject); case {}",
        case.id
    );
}

/// **Counter-fixture:** `min_improvement_delta = 1.0` ile V1 AcceptAsProgress'improvement
/// kaynaklı olduğunu kanıtla. Aynı case, sadece `min_improvement_delta` değiştirilir
/// (loss drop ≈ 0.768 < 1.0 → improved=false → Reject). **Taze engine** ile çalışır
/// (state izolasyonu).
///
/// V1 measured-after + baseline bit'leri ana fixture ile AYNI olmalı — değişen tek
/// karar girdisi `min_improvement_delta`. Bu, V1 AcceptAsProgress'in improvement'tan
/// geldiğini exact gösterir (harici koşul değil).
#[test]
fn counter_fixture_min_delta_blocks_v1_accept_as_progress() {
    use osp_core::coords::MetricSource;
    use osp_core::trajectory::{ApplyTarget, MutationDecision, PredicateCompletion};
    // Aynı builder — sadece min_improvement_delta = 1.0 (loss drop ≈ 0.768 < 1.0).
    let mut case = build_all_cases()
        .into_iter()
        .find(|c| c.id == "delta-introduced-subject-policy-001")
        .expect("delta-introduced-subject-policy-001 case olmalı");
    case.task.policy.min_improvement_delta = 1.0;

    // TAZE engine — izolasyon (clone yetersiz, evaluate mutates).
    let mut engine_v1_counter = common::engine_with_case_space(&case);
    let obs_v1_counter = common::evaluate_v1_case(&mut engine_v1_counter, &case);

    // === Measured-after + baseline parity — değişen tek girdi min_improvement_delta ===
    let (
        counter_baseline_values,
        counter_baseline_sources,
        counter_baseline_loss_bits,
        counter_after_values,
        counter_after_sources,
    ) = match &obs_v1_counter.measurement {
        common::MeasurementObservation::Produced {
            values_bits,
            sources,
            baseline:
                common::BaselineObservation::LegacyComputed {
                    values_bits: bv,
                    sources: bs,
                    loss_bits,
                    derivation: common::LegacyBaselineDerivation::DefaultFallback,
                },
            ..
        } => (*bv, *bs, *loss_bits, *values_bits, *sources),
        other => panic!("counter V1 produced olmalı; got {other:?}"),
    };
    assert_eq!(
        counter_baseline_values, [0u64; 5],
        "counter baseline parity — DefaultFallback zero (case {})",
        case.id
    );
    assert_eq!(
        counter_baseline_sources,
        [MetricSource::Scip; 5],
        "counter baseline sources parity (case {})",
        case.id
    );
    assert_eq!(
        counter_baseline_loss_bits, 4607487347736372296u64,
        "counter baseline loss parity (case {})",
        case.id
    );
    assert_eq!(
        counter_after_values,
        [
            4602678819172646912u64,
            4602678819172646912,
            4602678819172646912,
            4602678819172646912,
            4601046424471046557,
        ],
        "counter measured-after values parity (case {})",
        case.id
    );
    assert_eq!(
        counter_after_sources,
        [MetricSource::Scip; 5],
        "counter measured-after sources parity (case {})",
        case.id
    );

    // === EXACT counter pipeline — Reject → NotApplied (INV-T8; P1-tur6) ===
    assert_eq!(
        obs_v1_counter.pipeline,
        common::PipelineObservation::ReferenceEvaluation {
            predicate_completion: Some(PredicateCompletion::NotCompleted),
            mutation_decision: Some(MutationDecision::Reject),
            apply_target: Some(ApplyTarget::NotApplied),
        },
        "counter V1: min_delta=1.0 → improved=false → Reject (reference evaluation); case {}",
        case.id
    );

    // Loss drop < min_delta (1.0) — improved=false kanıtı (decision-input scalar'ından).
    let counter_decision = obs_v1_counter
        .decision_input
        .expect("counter V1 commit'e ulaştı → decision_input Some olmalı");
    let loss_before = f64::from_bits(counter_decision.loss_before_bits);
    let loss_after = f64::from_bits(counter_decision.loss_after_bits);
    assert_eq!(
        counter_decision.loss_before_bits, counter_baseline_loss_bits,
        "counter decision-input loss_before == baseline loss_bits (aynı kaynak); case {}",
        case.id
    );
    assert!(
        loss_after >= loss_before - case.task.policy.min_improvement_delta,
        "counter: loss_after ({}) >= loss_before ({}) - min_delta (1.0) → improved=false; case {}",
        loss_after,
        loss_before,
        case.id
    );
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
// Faz 8-P2 #88 — DirectPerAxisAuthority behavioral matrix (MD-2 pozitif-matching)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Concrete V2 per-axis source required_source ile eşleşince predicate completion
// Completed; legacy V1 projection (uniform Scip) eşleşmeyince SourceInsufficient.
// Coupling axis, Node(1) scope. PR #85 inline matrix frozen corpus'a taşındı
// (assertion-equivalence map: her eski dal bir frozen case'de karşılandı).
//
// **Evidence level:** predicate completion (PredicateSet::evaluate_completion) —
// task/mainline/witness completion iddiası DEĞİL.
//
// **P1-1 affected_nodes:[1]:** compute_raw_from_delta affected_nodes'u ölçüm kümesi
// olarak kullanır → V1 measurement selector = Node(1) = V2 subject scope → V1/V2
// numeric equality (coupling 0.5). Threshold Le 0.5 (exact eski fixture, binary exact).

#[test]
fn direct_per_axis_required_source_matrix_accepts_matching_v2_predicate_authority() {
    use osp_core::coords::MetricSource;
    use osp_core::trajectory::PredicateSetResult;

    // Frozen corpus'tan DirectPerAxisAuthority family'yi yükle.
    let cases = common::load_cases_by_class(common::CaseClass::DirectPerAxisAuthority);
    assert_eq!(
        cases.len(),
        5,
        "DirectPerAxisAuthority family exact 5 case içermeli"
    );

    // Executable expected table — PR #85 inline matrix assertion-equivalence map.
    // (case_id, V1_predicate_result, V2_predicate_result)
    let expected: [(&str, PredicateSetResult, PredicateSetResult); 5] = [
        (
            "direct-coupling-required-none-001",
            PredicateSetResult::Completed,
            PredicateSetResult::Completed,
        ),
        (
            "direct-coupling-required-scip-001",
            PredicateSetResult::Completed,
            PredicateSetResult::SourceInsufficient,
        ),
        // Pozitif-matching kontrolü: concrete V2 source eşleşince V2 Completed.
        (
            "direct-coupling-required-tree-sitter-001",
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::Completed,
        ),
        (
            "direct-coupling-required-placeholder-001",
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
        (
            "direct-coupling-required-heuristic-001",
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
    ];

    for case in &cases {
        let exp = expected
            .iter()
            .find(|(id, _, _)| *id == case.id)
            .unwrap_or_else(|| {
                panic!(
                    "DirectPerAxisAuthority case {} expected table'da yok",
                    case.id
                )
            });

        // Fresh engine per case — önceki case'in mutation'ı sızıntı yapmaz.
        let mut engine_v1 = common::engine_with_case_space(case);
        let mut engine_v2 = common::engine_with_case_space(case);
        let obs_v1 = common::evaluate_v1_case(&mut engine_v1, case);
        let obs_v2 = common::evaluate_v2_candidate_case(&mut engine_v2, case);

        // Standalone measurement probe Produced — her iki harness'da.
        let measured_v1 = match &obs_v1.measurement {
            MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
            other => panic!("V1 {} measurement Produced olmalı; got {other:?}", case.id),
        };
        let measured_v2 = match &obs_v2.measurement {
            MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
            other => panic!("V2 {} measurement Produced olmalı; got {other:?}", case.id),
        };

        // Numeric contract — coupling after = 0.5 (Imports 1→2, Node(1) out-degree=1).
        // V1/V2 numeric equality: divergence yalnızca provenance authority'den.
        let v1_coupling = measured_v1.coupling.value;
        let v2_coupling = measured_v2.coupling.value;
        assert!(
            (v1_coupling - 0.5).abs() <= 1e-12,
            "{} V1 coupling after 0.5'e tolerance içinde olmalı; got {v1_coupling}",
            case.id
        );
        assert!(
            (v2_coupling - 0.5).abs() <= 1e-12,
            "{} V2 coupling after 0.5'e tolerance içinde olmalı; got {v2_coupling}",
            case.id
        );
        assert!(
            (v1_coupling - v2_coupling).abs() <= 1e-12,
            "{} V1/V2 coupling numeric equality — divergence yalnızca provenance'dan",
            case.id
        );

        // **P1-4 fix (review):** before coupling = 0.0 pin (izole Node(1), out-degree=0
        // pre-delta). Imports 1→2 delta coupling'i 0.0 → 0.5 değiştirir (intentional
        // measureability — Direct family metric-neutral DEĞİL). before/after farkı
        // provenance divergence değil gerçek structural delta.
        let baseline_v1 = match &obs_v1.measurement {
            MeasurementObservation::Produced { baseline, .. } => baseline.clone(),
            other => panic!("V1 {} baseline olmalı; got {other:?}", case.id),
        };
        let baseline_v2 = match &obs_v2.measurement {
            MeasurementObservation::Produced { baseline, .. } => baseline.clone(),
            other => panic!("V2 {} baseline olmalı; got {other:?}", case.id),
        };
        let v1_before_bits = match &baseline_v1 {
            common::BaselineObservation::LegacyComputed { values_bits, .. } => values_bits[0],
            other => panic!(
                "V1 {} LegacyComputed baseline olmalı; got {other:?}",
                case.id
            ),
        };
        let v2_before_bits = match &baseline_v2 {
            common::BaselineObservation::Available { values_bits, .. } => values_bits[0],
            other => panic!("V2 {} Available baseline olmalı; got {other:?}", case.id),
        };
        let before_coupling_v1 = f64::from_bits(v1_before_bits);
        let before_coupling_v2 = f64::from_bits(v2_before_bits);
        assert!(
            (before_coupling_v1 - 0.0).abs() <= 1e-12,
            "{} V1 before coupling 0.0 olmalı (izole Node(1)); got {before_coupling_v1}",
            case.id
        );
        assert!(
            (before_coupling_v2 - 0.0).abs() <= 1e-12,
            "{} V2 before coupling 0.0 olmalı (izole Node(1)); got {before_coupling_v2}",
            case.id
        );

        // Source matrix: V1 legacy projected = Scip, V2 measured = TreeSitter.
        assert_eq!(
            measured_v1.coupling.source,
            MetricSource::Scip,
            "{} V1 coupling source legacy projected Scip olmalı",
            case.id
        );
        assert_eq!(
            measured_v2.coupling.source,
            MetricSource::TreeSitter,
            "{} V2 coupling source measured TreeSitter olmalı (engine topology_source)",
            case.id
        );

        // Predicate completion — gerçek PredicateSet::evaluate_completion çağrısı.
        let ps = &case.task.target_predicate_set;
        let v1_result = ps.evaluate_completion(&measured_v1);
        let v2_result = ps.evaluate_completion(&measured_v2);
        assert_eq!(
            v1_result, exp.1,
            "{} V1 predicate result mismatch (expected {:?})",
            case.id, exp.1
        );
        assert_eq!(
            v2_result, exp.2,
            "{} V2 predicate result mismatch (expected {:?})",
            case.id, exp.2
        );

        // MD-2 divergence kanıtı: Scip/TreeSitter için V1/V2 farklı predicate result.
        if case.id == "direct-coupling-required-scip-001"
            || case.id == "direct-coupling-required-tree-sitter-001"
        {
            assert_ne!(
                v1_result, v2_result,
                "{} MD-2 provenance-authority divergence: V1 {:?} ≠ V2 {:?}",
                case.id, v1_result, v2_result
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Faz 8-P2 #88 — MixedPerAxisSources behavioral matrix (MD-2 fail-closed + decl-validation)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Mixed aggregate measured source — concrete authority şartını karşılamaz, declaration
// requirement olarak talep edilemez. Cohesion axis, Subgraph[1,2] scope.
//
// 5 predicate evaluation case (None/Scip/TreeSitter/Heuristic/Placeholder): Mixed hiçbir
// non-Mixed required-source token'ını karşılayamaz → SourceInsufficient (fail-closed).
// required_source=None → Completed (Mixed numeric değerlendirmeye katılır).
//
// 1 declaration-validation case (Mixed): commit_task_claim validate_for_commit →
// InvalidRequiredMetricSource → EngineCommitError::TaskValidation. Measurement'dan ÖNCE
// terminal reject. Standalone probe Mixed üretir, commit pipeline durur (probe vs pipeline).

#[test]
fn mixed_cohesion_required_source_matrix_rejects_concrete_authority_claims() {
    use osp_core::coords::MetricSource;
    use osp_core::trajectory::{PredicateSetResult, TaskValidationError};

    let cases = common::load_cases_by_class(common::CaseClass::MixedPerAxisSources);
    assert_eq!(
        cases.len(),
        6,
        "MixedPerAxisSources family exact 6 case içermeli"
    );

    // 5 predicate evaluation case için expected matrix.
    // (case_id, V1_predicate_result, V2_predicate_result)
    let predicate_expected: [(&str, PredicateSetResult, PredicateSetResult); 5] = [
        (
            "mixed-cohesion-required-none-001",
            PredicateSetResult::Completed,
            PredicateSetResult::Completed,
        ),
        (
            "mixed-cohesion-required-scip-001",
            PredicateSetResult::Completed,
            PredicateSetResult::SourceInsufficient,
        ),
        (
            "mixed-cohesion-required-tree-sitter-001",
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
        (
            "mixed-cohesion-required-heuristic-001",
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
        (
            "mixed-cohesion-required-placeholder-001",
            PredicateSetResult::SourceInsufficient,
            PredicateSetResult::SourceInsufficient,
        ),
    ];

    for case in &cases {
        if case.id == "mixed-cohesion-required-mixed-invalid-001" {
            // Declaration-validation case — ayrı blokta ele alınır.
            continue;
        }

        let exp = predicate_expected
            .iter()
            .find(|(id, _, _)| *id == case.id)
            .unwrap_or_else(|| {
                panic!(
                    "MixedPerAxisSources predicate case {} expected table'da yok",
                    case.id
                )
            });

        let mut engine_v1 = common::engine_with_case_space(case);
        let mut engine_v2 = common::engine_with_case_space(case);
        let obs_v1 = common::evaluate_v1_case(&mut engine_v1, case);
        let obs_v2 = common::evaluate_v2_candidate_case(&mut engine_v2, case);

        let measured_v1 = match &obs_v1.measurement {
            MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
            other => panic!("V1 {} measurement Produced olmalı; got {other:?}", case.id),
        };
        let measured_v2 = match &obs_v2.measurement {
            MeasurementObservation::Produced { measured_after, .. } => measured_after.clone(),
            other => panic!("V2 {} measurement Produced olmalı; got {other:?}", case.id),
        };

        // Cohesion source matrix: V1 legacy projected = Scip, V2 measured = Mixed.
        assert_eq!(
            measured_v1.cohesion.source,
            MetricSource::Scip,
            "{} V1 cohesion source legacy projected Scip olmalı",
            case.id
        );
        assert_eq!(
            measured_v2.cohesion.source,
            MetricSource::Mixed,
            "{} V2 cohesion source measured Mixed olmalı (aggregation [Scip, Placeholder])",
            case.id
        );

        // Numeric equality — V1/V2 cohesion value tolerance içinde eşit (0.55).
        let v1_cohesion = measured_v1.cohesion.value;
        let v2_cohesion = measured_v2.cohesion.value;
        assert!(
            (v1_cohesion - v2_cohesion).abs() <= 1e-12,
            "{} V1/V2 cohesion numeric equality — divergence yalnızca provenance'dan; v1={v1_cohesion}, v2={v2_cohesion}",
            case.id
        );
        assert!(
            (v1_cohesion - 0.55).abs() <= 1e-12,
            "{} cohesion value 0.55'e tolerance içinde olmalı (eşit mass centroid (0.6+0.5)/2); got {v1_cohesion}",
            case.id
        );

        // **P1-4 fix (review):** DependsOn delta 5 measured axis'te no-op — before (baseline)
        // ile after (measured) value bits eşit. coupling/instability sadece Imports okur,
        // cohesion edge-bağımsız, entropy/witness_depth edge'den bağımsız. DependsOn bunların
        // hiçbirini etkilemez → metric isolation. Bu kanıtlanmazsa ileride DependsOn coupling'i
        // etkilemeye başlarsa cohesion predicate sonuçları aynı kaldığı sürece testler yeşil kalır.
        let baseline_v1 = match &obs_v1.measurement {
            MeasurementObservation::Produced { baseline, .. } => baseline.clone(),
            other => panic!("V1 {} baseline olmalı; got {other:?}", case.id),
        };
        let baseline_v2 = match &obs_v2.measurement {
            MeasurementObservation::Produced { baseline, .. } => baseline.clone(),
            other => panic!("V2 {} baseline olmalı; got {other:?}", case.id),
        };
        let after_bits_v1 = common::axis_value_bits(&measured_v1);
        let after_bits_v2 = common::axis_value_bits(&measured_v2);
        // V1 baseline (LegacyComputed) ile after — 5 axis bits eşit.
        let before_bits_v1 = match &baseline_v1 {
            common::BaselineObservation::LegacyComputed { values_bits, .. } => *values_bits,
            other => panic!(
                "V1 {} LegacyComputed baseline olmalı; got {other:?}",
                case.id
            ),
        };
        assert_eq!(
            before_bits_v1, after_bits_v1,
            "{} V1 DependsOn delta 5 axis no-op olmalı (before==after bits)",
            case.id
        );
        // V2 baseline (Available) ile after — 5 axis bits eşit.
        let before_bits_v2 = match &baseline_v2 {
            common::BaselineObservation::Available { values_bits, .. } => *values_bits,
            other => panic!("V2 {} Available baseline olmalı; got {other:?}", case.id),
        };
        assert_eq!(
            before_bits_v2, after_bits_v2,
            "{} V2 DependsOn delta 5 axis no-op olmalı (before==after bits)",
            case.id
        );

        // **P2-2 fix (review):** DependsOn delta source-neutrality — before (baseline) ile
        // after (measured) 5-axis sources eşit. PR'ın konusu provenance authority olduğu için
        // izolasyonun source tarafı da pinlenmeli; ileride DependsOn numeric değeri değiştirmeyip
        // yalnız provenance üretimini değiştirirse fixture bunu yakalar.
        let before_sources_v1 = match &baseline_v1 {
            common::BaselineObservation::LegacyComputed { sources, .. } => *sources,
            other => panic!(
                "V1 {} LegacyComputed baseline olmalı; got {other:?}",
                case.id
            ),
        };
        let before_sources_v2 = match &baseline_v2 {
            common::BaselineObservation::Available { sources, .. } => *sources,
            other => panic!("V2 {} Available baseline olmalı; got {other:?}", case.id),
        };
        assert_eq!(
            before_sources_v1,
            common::axis_sources(&measured_v1),
            "{} V1 DependsOn delta 5 axis source no-op olmalı (before==after sources)",
            case.id
        );
        assert_eq!(
            before_sources_v2,
            common::axis_sources(&measured_v2),
            "{} V2 DependsOn delta 5 axis source no-op olmalı (before==after sources)",
            case.id
        );

        // Predicate completion — gerçek PredicateSet::evaluate_completion.
        let ps = &case.task.target_predicate_set;
        let v1_result = ps.evaluate_completion(&measured_v1);
        let v2_result = ps.evaluate_completion(&measured_v2);
        assert_eq!(
            v1_result, exp.1,
            "{} V1 predicate result mismatch (expected {:?})",
            case.id, exp.1
        );
        assert_eq!(
            v2_result, exp.2,
            "{} V2 predicate result mismatch (expected {:?})",
            case.id, exp.2
        );

        // MD-2 fail-closed divergence: Scip için V1 Completed, V2 SourceInsufficient.
        if case.id == "mixed-cohesion-required-scip-001" {
            assert_ne!(
                v1_result, v2_result,
                "{} MD-2 fail-closed: Mixed Scip şartını karşılamaz → V2 SourceInsufficient",
                case.id
            );
        }
    }

    // --- 6. case: declaration-validation reject (required_source=Some(Mixed)) ---
    let invalid_case = cases
        .iter()
        .find(|c| c.id == "mixed-cohesion-required-mixed-invalid-001")
        .expect("mixed-cohesion-required-mixed-invalid-001 olmalı");

    // Pre-aggregation production-path (P1-4): Node(1) → Scip, Node(2) → Placeholder,
    // Subgraph[1,2] → Mixed. Mixed gerçek aggregation hattından doğar.
    let node1_cohesion = measure_single_node_cohesion(invalid_case, 1);
    let node2_cohesion = measure_single_node_cohesion(invalid_case, 2);
    assert_eq!(
        node1_cohesion.source,
        MetricSource::Scip,
        "Node(1) cohesion source Scip olmalı (cohesion=Some → observed_source)"
    );
    assert_eq!(
        node2_cohesion.source,
        MetricSource::Placeholder,
        "Node(2) cohesion source Placeholder olmalı (cohesion=None → effective fallback)"
    );

    let mut engine_v1 = common::engine_with_case_space(invalid_case);
    let mut engine_v2 = common::engine_with_case_space(invalid_case);

    // **P1-3 fix (review):** No-mutation snapshot her engine için AYRI alınır (önce V1
    // snapshot engine_v1'den, V2 snapshot engine_v2'den). Önceki kod before'ı engine_v1'den
    // alıp after'ı engine_v1 ile karşılaştırıyordu — V2 probe/commit engine_v2'de çalıştığı
    // için V2 mutasyonu görünmüyordu. Snapshot SpaceDigest + revision + node/edge count.
    let v1_before = engine_snapshot(&engine_v1);
    let v2_before = engine_snapshot(&engine_v2);

    // Standalone probe (V2) — Mixed üretir (commit pipeline'dan ayrı).
    let obs_v2_probe = common::evaluate_v2_candidate_case(&mut engine_v2, invalid_case);
    let v2_measured = match &obs_v2_probe.measurement {
        MeasurementObservation::Produced {
            sources,
            measured_after,
            ..
        } => {
            // Mixed gerçekten aggregation'dan geldi — cohesion axis Mixed.
            assert_eq!(
                sources[1],
                MetricSource::Mixed,
                "V2 {} probe cohesion source Mixed olmalı",
                invalid_case.id
            );
            assert_eq!(
                measured_after.cohesion.source,
                MetricSource::Mixed,
                "V2 {} probe measured_after cohesion source Mixed olmalı",
                invalid_case.id
            );
            measured_after.clone()
        }
        other => panic!(
            "V2 {} standalone probe Mixed üretmeli (commit'den ayrı); got {other:?}",
            invalid_case.id
        ),
    };

    // Commit pipeline — TaskValidation reject. Probe zaten üretti ama commit tüketmedi.
    let obs_v1 = common::evaluate_v1_case(&mut engine_v1, invalid_case);
    // V1: compute_raw_from_delta standalone (infallible) üretir → measurement Produced.
    match &obs_v1.measurement {
        MeasurementObservation::Produced { .. } => {}
        other => panic!(
            "V1 {} standalone probe Produced olmalı (commit reject ayrı); got {other:?}",
            invalid_case.id
        ),
    }
    // **P1-tur6 re-anchor:** V1 = ReferenceEvaluation — production pipeline
    // ÇALIŞMAZ → motorun validate_for_commit'ı (InvalidRequiredMetricSource üreten
    // aşama) harness'te koşmaz; production reachability iddiası YAPILMAZ. Eski
    // StoppedBeforeCommit(TaskValidation) pin'i gerçek-pipeline V1'e aitti;
    // TaskValidation reject artık yalnız V2'de (ve aşağıdaki gerçek commit
    // çağrısında — commit_invalid_mixed_case exact error chain) gözlemlenir.
    // Gözlenen: PredicateGate(case inputs) — Mixed required_source ile predicate
    // karşılanamaz → NotCompleted → Reject → NotApplied (INV-T8); decision_input Some.
    assert_eq!(
        obs_v1.pipeline,
        common::PipelineObservation::ReferenceEvaluation {
            predicate_completion: Some(osp_core::trajectory::PredicateCompletion::NotCompleted),
            mutation_decision: Some(osp_core::trajectory::MutationDecision::Reject),
            apply_target: Some(osp_core::trajectory::ApplyTarget::NotApplied),
        },
        "{} V1 exact pipeline (ReferenceEvaluation; TaskValidation gözlenmez → gate Reject — production reachability iddiası YOK); got {:?}",
        invalid_case.id,
        obs_v1.pipeline
    );
    assert!(
        obs_v1.decision_input.is_some(),
        "{} V1 decision_input Some olmalı (evaluator decision surface'e ulaştı)",
        invalid_case.id
    );

    // V2 pipeline: aynı TaskValidation reject.
    assert!(
        matches!(
            &obs_v2_probe.pipeline,
            common::PipelineObservation::StoppedBeforeCommit {
                stage: common::PipelineStage::TaskValidation,
                ..
            }
        ),
        "{} V2 pipeline StoppedBeforeCommit(TaskValidation) olmalı; got {:?}",
        invalid_case.id,
        obs_v2_probe.pipeline
    );
    assert!(
        obs_v2_probe.decision_input.is_none(),
        "{} V2 decision_input None olmalı (decision surface'e ulaşmadı)",
        invalid_case.id
    );

    // **P1-3 fix:** No-mutation her engine için AYRI doğrulanır. Snapshot SpaceDigest +
    // revision + node/edge count içerir — tek sayı korunup içerik/revision değişirse yakalar.
    assert_eq!(
        engine_snapshot(&engine_v1),
        v1_before,
        "{} V1 probe + commit engine state'i değiştirmemeli (SpaceDigest/revision/count)",
        invalid_case.id
    );
    assert_eq!(
        engine_snapshot(&engine_v2),
        v2_before,
        "{} V2 probe + commit engine state'i değiştirmemeli (SpaceDigest/revision/count)",
        invalid_case.id
    );

    let _ = v2_measured; // probe measured (Mixed source) — kanıtlandı.

    // **P1-2 fix (review):** Exact error chain — gerçek commit_task_claim result'ından.
    // Önceki kod enum varyantını construct ediyordu (task_id: 0 yanlış) ama gerçek result'la
    // karşılaştırmıyordu. Şimdi commit_task_claim doğrudan çağrılıp EngineCommitError::
    // TaskValidation(TaskValidationError::InvalidRequiredMetricSource{...}) exact pinleniyor.
    let commit_err = commit_invalid_mixed_case(invalid_case);
    let task_id = invalid_case.task.id;
    assert!(
        matches!(
            &commit_err,
            Some(osp_core::engine::EngineCommitError::TaskValidation(
                TaskValidationError::InvalidRequiredMetricSource {
                    task_id: err_task_id,
                    predicate_index: 0,
                    required_source: MetricSource::Mixed,
                }
            )) if *err_task_id == task_id
        ),
        "{} exact error chain InvalidRequiredMetricSource{{task_id:{}, predicate_index:0, \
         required_source:Mixed}} olmalı; got {:?}",
        invalid_case.id,
        task_id,
        commit_err
    );
}

/// Engine state snapshot — no-mutation doğrulaması için (P1-3).
///
/// **P1-2 fix (review):** `revision.content_digest` zaten `SpaceDigest::compute(&self.space)`
/// ile aynı değerdir (engine.rs:2213) — `space_digest_hex` ile duplicate. Gerçek revision
/// hareketi `sequence: t_c` ve `view_id: Ephemeral(t_c)` alanlarında taşınır. Bu yüzden
/// snapshot'a `revision_sequence` (revision.sequence) ve `engine_t_c` (engine.t_c()) eklenir;
/// rejected commit'in space içeriğini değiştirmeden yalnızca t_c/sequence ilerlettiği
/// varsayımsal bir regresyon yakalanır. Checkpoint/mainline/audit engine public API'den
/// erişilemediği için snapshot'a dahil DEĞİL — iddia edilmez.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineSnapshot {
    space_digest_hex: String,
    revision_sequence: u64,
    engine_t_c: u64,
    node_count: usize,
    edge_count: usize,
}

fn engine_snapshot(engine: &osp_core::engine::SpaceEngine) -> EngineSnapshot {
    let space_digest = osp_core::authorization::SpaceDigest::compute(engine.space())
        .expect("SpaceDigest compute başarılı olmalı");
    let revision = engine
        .current_space_view_revision()
        .expect("revision computation başarılı");
    EngineSnapshot {
        space_digest_hex: hex::encode(space_digest.as_bytes()),
        revision_sequence: revision.sequence,
        engine_t_c: engine.t_c(),
        node_count: engine.space().nodes.len(),
        edge_count: engine.space().edges.len(),
    }
}

/// `commit_task_claim`'i invalid Mixed case ile çağırıp EngineCommitError döndürür (P1-2).
///
/// Exact error chain assertion için gerçek commit_task_claim result'ını yakalar — enum
/// varyantı construct etmek yerine production validation'ın gerçek çıktısını kullanır.
fn commit_invalid_mixed_case(
    case: &common::CharacterizationCase,
) -> Option<osp_core::engine::EngineCommitError> {
    use osp_core::coords::MetricSource;
    use osp_core::navigator::build_claim_from_proposal;
    use osp_core::trajectory::{InMemoryTaskRegistry, TaskResolver};
    use osp_core::witness::WitnessSet;

    let mut engine = common::engine_with_case_space(case);
    let probe_claim = build_claim_from_proposal(
        &case.proposal,
        osp_core::coords::RawPosition::default(),
        case.task.id,
        100,
        1,
    )
    .expect("probe claim build başarılı olmalı");

    let target = case
        .task
        .target_predicate_set
        .preferred_vector
        .unwrap_or_default();
    let measured =
        osp_core::navigator::provenanced_from_raw(probe_claim.computed_raw, MetricSource::Scip);

    let mut registry = InMemoryTaskRegistry::new();
    registry.insert(case.task.clone());
    let omega = WitnessSet::new(vec![]);

    // **#96 MD-2 P0-tur4:** gerçek native flow — draft → measure → finalize →
    // sealed carrier (eski characterization_native_token kaldırıldı; ayrı claim +
    // measurement artifact mix'i type-level unrepresentable). Commit TaskValidation
    // reject'i hâlâ production validation'ın gerçek çıktısı üzerinden pinlenir.
    let draft = osp_core::task_measurement::StructurallyValidatedClaimDraft::try_new(
        &case.proposal,
        osp_core::coords::RawPosition::default(),
        case.task.id,
        100,
        1,
    )
    .expect("draft (probe + structural Q4) başarılı olmalı");
    let native = engine
        .measure_attempt_native_with_md1_shadow(&draft, &case.proposal, &case.task)
        .expect("native measurement başarılı olmalı");
    let finalized = draft
        .finalize(native.authority())
        .expect("finalize (subject binding) başarılı olmalı");
    let result = engine.commit_task_claim(osp_core::engine::TaskCommitInput::new(
        &finalized,
        &omega,
        &registry as &dyn TaskResolver,
        target,
        osp_core::trajectory::trajectory_loss(&measured, &target),
    ));

    result.err()
}

/// Tek node'un cohesion ölçümünü production path üzerinden üretir (P1-4 pre-aggregation).
///
/// `measure_task_delta` Node(id) scope ile çağrılır → cohesion source. Bu, Mixed'in
/// gerçek aggregation hattından geldiğini kanıtlar: Node(1)=Scip, Node(2)=Placeholder,
/// Subgraph[1,2]=Mixed (aggregate_source).
fn measure_single_node_cohesion(
    case: &common::CharacterizationCase,
    node_id: u64,
) -> osp_core::coords::AxisMeasurement {
    use osp_core::navigator::build_claim_from_proposal;
    use osp_core::space::NodeId;
    use osp_core::trajectory::{
        MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        WeightedPredicate,
    };

    // Tek-node scope ile task üret (case'in task'ını geçici olarak değiştir).
    let mut single_task = case.task.clone();
    single_task.target_predicate_set = PredicateSet {
        mode: PredicateMode::All,
        predicates: vec![WeightedPredicate {
            predicate: MetricPredicate {
                metric: PredicateAxis::Cohesion,
                operator: case.task.target_predicate_set.predicates[0]
                    .predicate
                    .operator,
                threshold: case.task.target_predicate_set.predicates[0]
                    .predicate
                    .threshold,
                scope: PredicateScope::Node(node_id),
                required_source: None,
                tolerance: 0.0,
            },
            weight: None,
        }],
        preferred_vector: None,
    };

    let probe_claim = build_claim_from_proposal(
        &case.proposal,
        osp_core::coords::RawPosition::default(),
        single_task.id,
        100,
        1,
    )
    .expect("probe claim build başarılı olmalı");
    let probe_bound = osp_core::trajectory::TaskBoundClaim {
        claim: &probe_claim,
        task: &single_task,
    };

    let engine = common::engine_with_case_space(case);
    let revision = engine
        .current_space_view_revision()
        .expect("revision computation başarılı");
    let token = engine
        .measure_task_delta(&probe_bound, &revision, Some(&[node_id as NodeId][..]))
        .expect("single-node measurement başarılı olmalı");

    token.after().cohesion
}

// ═══════════════════════════════════════════════════════════════════════════════
// Faz 8-P2 #88 — Measured-subject digest V1 contract (negatif + metamorphic)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Exact-one predicate guard, Module scope unsupported, scope varyant ayrımı (Node≠Subgraph),
// scope order-insensitivity, error propagation ve metamorphic digest ayrımı (required_source
// measured-subject digest'i değiştirmez ama full-case digest'i değiştirir).

/// Zero-predicate digest girişi fail-closed reddedilir (exact-one invariant).
#[test]
fn measured_subject_digest_v1_rejects_zero_predicates() {
    use common::{compute_measured_subject_digest, load_cases_by_class, SubjectDigestError};

    let mut cases = load_cases_by_class(common::CaseClass::DirectPerAxisAuthority);
    let case = cases
        .first_mut()
        .expect("DirectPerAxisAuthority case olmalı");
    case.task.target_predicate_set.predicates.clear();

    let result = serialize_digest(case);
    assert!(
        matches!(
            result,
            Err(SubjectDigestError::ExpectedExactlyOnePredicate { actual: 0 })
        ),
        "zero-predicate digest ExpectedExactlyOnePredicate{{actual:0}} ile reddedilmeli; got {result:?}"
    );
    // Üst fonksiyon hatayı yutmaz (error propagation).
    let digest_result = compute_measured_subject_digest(case);
    assert!(
        matches!(
            digest_result,
            Err(SubjectDigestError::ExpectedExactlyOnePredicate { .. })
        ),
        "compute_measured_subject_digest zero-predicate hatayı yutmamalı; got {digest_result:?}"
    );
}

/// Multi-predicate digest girişi fail-closed reddedilir (exact-one invariant).
#[test]
fn measured_subject_digest_v1_rejects_multiple_predicates() {
    use common::{compute_measured_subject_digest, load_cases_by_class, SubjectDigestError};

    let mut cases = load_cases_by_class(common::CaseClass::DirectPerAxisAuthority);
    let case = cases
        .first_mut()
        .expect("DirectPerAxisAuthority case olmalı");
    let second = case.task.target_predicate_set.predicates[0].clone();
    case.task.target_predicate_set.predicates.push(second);

    let result = serialize_digest(case);
    assert!(
        matches!(
            result,
            Err(SubjectDigestError::ExpectedExactlyOnePredicate { actual: 2 })
        ),
        "multi-predicate digest ExpectedExactlyOnePredicate{{actual:2}} ile reddedilmeli; got {result:?}"
    );
    let digest_result = compute_measured_subject_digest(case);
    assert!(
        matches!(
            digest_result,
            Err(SubjectDigestError::ExpectedExactlyOnePredicate { .. })
        ),
        "compute_measured_subject_digest multi-predicate hatayı yutmamalı; got {digest_result:?}"
    );
}

/// Module scope V1 evidence schema'da desteklenmez — UnsupportedScope reject.
#[test]
fn measured_subject_digest_v1_rejects_module_scope() {
    use common::{load_cases_by_class, SubjectDigestError};
    use osp_core::trajectory::PredicateScope;

    let mut cases = load_cases_by_class(common::CaseClass::DirectPerAxisAuthority);
    let case = cases
        .first_mut()
        .expect("DirectPerAxisAuthority case olmalı");
    case.task.target_predicate_set.predicates[0].predicate.scope =
        PredicateScope::Module("core".into());

    let result = serialize_digest(case);
    assert!(
        matches!(result, Err(SubjectDigestError::UnsupportedScope { .. })),
        "Module scope UnsupportedScope ile reddedilmeli; got {result:?}"
    );
}

/// Node(1) ve Subgraph([1]) farklı ontolojik scope'lar — farklı digest (scope_tag ayrımı).
///
/// Production `CanonicalPredicateScope.scope_tag()`: Node=0, Subgraph=2. Bu tag ayrımı
/// olmadan iki scope aynı digest üretirdi. Production canonical tip kullanıldığı için
/// ayrım otomatik korunur.
#[test]
fn measured_subject_digest_v1_node_scope_differs_from_subgraph_singleton() {
    use common::load_cases_by_class;
    use osp_core::trajectory::PredicateScope;

    // Node(1) scope — direct family'den al.
    let mut node_cases = load_cases_by_class(common::CaseClass::DirectPerAxisAuthority);
    let node_case = node_cases.first_mut().expect("direct case olmalı");

    // Subgraph([1]) scope — mixed family'nin scope'unu değiştir.
    let mut sub_cases = load_cases_by_class(common::CaseClass::MixedPerAxisSources);
    let sub_case = sub_cases.first_mut().expect("mixed case olmalı");
    sub_case.task.target_predicate_set.predicates[0]
        .predicate
        .scope = PredicateScope::Subgraph(vec![1]);

    // Space'i de node_case ile eşitle ki sadece scope farkı olsun (axis de cohesion →
    // direct case'in axis'ini cohesion'a çek). Önce node_case'in axis'ini cohesion yap.
    node_case.task.target_predicate_set.predicates[0]
        .predicate
        .metric = osp_core::trajectory::PredicateAxis::Cohesion;
    // Space ve proposal'ı da eşitle — sub_case'in space'ini kullan.
    node_case.space = sub_case.space.clone();
    node_case.proposal = sub_case.proposal.clone();

    let node_digest = serialize_digest(node_case).expect("Node(1) digest üretilebilmeli");
    let sub_digest = serialize_digest(sub_case).expect("Subgraph([1]) digest üretilebilmeli");

    assert_ne!(
        node_digest, sub_digest,
        "Node(1) ve Subgraph([1]) farklı ontolojik scope'lar — production scope_tag ayrımı farklı digest üretmeli (Node=0, Subgraph=2)"
    );
}

/// Subgraph scope order-insensitive: [1,2] ve [2,1] aynı digest.
///
/// Production `CanonicalSubgraphScope::try_new` sort_unstable yaptığı için member order
/// digest'i etkilemez. Bu, characterization case'lerde member sırasının donmuş digest'i
/// bozmasını engeller.
#[test]
fn measured_subject_digest_v1_subgraph_scope_is_order_insensitive() {
    use common::load_cases_by_class;
    use osp_core::trajectory::PredicateScope;

    let mut cases = load_cases_by_class(common::CaseClass::MixedPerAxisSources);
    let case = cases.first_mut().expect("mixed case olmalı");

    // Orijinal scope [1,2].
    let digest_12 = serialize_digest(case).expect("[1,2] digest üretilebilmeli");

    // Ters sıra [2,1].
    case.task.target_predicate_set.predicates[0].predicate.scope =
        PredicateScope::Subgraph(vec![2, 1]);
    let digest_21 = serialize_digest(case).expect("[2,1] digest üretilebilmeli");

    assert_eq!(
        digest_12, digest_21,
        "Subgraph([1,2]) ve Subgraph([2,1]) aynı digest üretmeli (CanonicalSubgraphScope sort)"
    );
}

/// Metamorphic digest testi: required_source measured-subject digest'i DEĞİŞTİRMEZ,
/// full-case digest'i DEĞİŞTİRİR.
///
/// **P1-1 fix (review):** Önceki test iki farklı frozen case kullanıyordu (id/description/
/// label da farklı) — `assert_ne!(full_digest)` required_source'tan bağımsız geçerdi.
/// Düzeltme: tek case clone edilir, yalnızca required_source değiştirilir. Böylece iki
/// digest katmanının görev ayrımı gerçekten kanıtlanır:
/// - measured-subject digest: required_source digest girdisi değil → aynı kalır
/// - full-case digest: required_source full-case identity parçası → değişir
#[test]
fn required_source_changes_full_case_digest_not_measured_subject_digest() {
    use common::{
        blake3_hex, compute_measured_subject_digest, load_cases_by_class, serialize_case_bytes,
    };
    use osp_core::coords::MetricSource;

    let cases = load_cases_by_class(common::CaseClass::DirectPerAxisAuthority);
    let none_case = cases
        .iter()
        .find(|c| c.id == "direct-coupling-required-none-001")
        .expect("none case olmalı");

    // Clone + yalnızca required_source değişir. id/description/label/space/proposal aynı.
    let mut scip_case = none_case.clone();
    scip_case.task.target_predicate_set.predicates[0]
        .predicate
        .required_source = Some(MetricSource::Scip);
    assert_eq!(
        none_case.id, scip_case.id,
        "id aynı kalmalı (yalnızca required_source değişir)"
    );
    assert_eq!(
        none_case.description, scip_case.description,
        "description aynı kalmalı"
    );

    // Measured-subject digest: aynı (required_source digest girdisi değil).
    let none_subject = compute_measured_subject_digest(none_case)
        .expect("none measured-subject digest üretilebilmeli");
    let scip_subject = compute_measured_subject_digest(&scip_case)
        .expect("scip measured-subject digest üretilebilmeli");
    assert_eq!(
        hex::encode(&none_subject),
        hex::encode(&scip_subject),
        "required_source measured-subject digest'i değiştirmemeli (farklı declaration, aynı subject)"
    );

    // Full-case digest: farklı (required_source full-case digest girdisi).
    let none_full = blake3_hex(&serialize_case_bytes(none_case));
    let scip_full = blake3_hex(&serialize_case_bytes(&scip_case));
    assert_ne!(
        none_full, scip_full,
        "required_source full-case digest'i değiştirmeli (declaration full-case identity parçası); \
         id/description/label aynı olduğundan fark yalnızca required_source'tan gelmeli"
    );
}

/// `serialize_measured_subject_bytes` helper — negatif testlerde tekrar tekrar çağırmamak için.
fn serialize_digest(
    case: &common::CharacterizationCase,
) -> Result<Vec<u8>, common::SubjectDigestError> {
    common::serialize_measured_subject_bytes(case)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Faz 8-P2 #88 — Production-effective digest closure regression testleri (P2, non-blocking)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Review (APPROVE turu): manifest digest taşıyan Direct/Mixed family'leri new_nodes/
// connected_to/removed_edges kullanmıyor — bu closure yolları compilation + code
// inspection ile doğrulanıyor ama doğrudan metamorphic regression testiyle pinlenmiyor.
// Bu iki test, P1-1 fixup'ın gelecekte geri kaymasını yakalar.

/// `NewNodeSpec.connected_to` edge'leri structural digest'i değiştiriyor (P1-1 regression).
///
/// Production `build_claim_from_proposal` connected_to edge'lerini delta_edges'e ekler.
/// Serializer production claim üzerinden structural delta kurduğu için connected_to dolaylı
/// olarak digest'e girer. Bu test, connected_to değişince measured-subject digest'in
/// değiştiğini doğrudan pinler — ileride serializer raw proposal.new_edges'e geri dönerse
/// (connected_to düşer) test kırılır.
#[test]
fn connected_to_changes_measured_subject_digest() {
    use osp_core::agent::{DeltaProposal, NewNodeSpec};
    use osp_core::space::EdgeKind;

    let base = direct_coupling_base_for_regression();

    // Variant A: new_nodes boş (connected_to yok), tek new_edge. affected_nodes dolu ki
    // effective selector non-empty olsun (CanonicalSubjectScope empty reject eder).
    let mut without = base.clone();
    without.proposal = DeltaProposal {
        new_nodes: vec![],
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 1,
            to: 2,
            kind: osp_core::space::EdgeKind::Imports,
        }],
        affected_nodes: vec![1],
        ..Default::default()
    };

    // Variant B: new_nodes + connected_to edge. Production claim delta_edges'e ekler.
    // new_nodes boş olmadığı için build_claim_from_proposal empty reject etmez.
    let mut with = base.clone();
    with.proposal = DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: osp_core::space::NodeKind::Module,
            initial_mass: 1.0,
            connected_to: vec![(10, EdgeKind::Imports)],
        }],
        new_edges: vec![],
        affected_nodes: vec![1],
        ..Default::default()
    };

    let digest_without = serialize_digest(&without).expect("without digest üretilebilmeli");
    let digest_with = serialize_digest(&with).expect("with digest üretilebilmeli");

    assert_ne!(
        digest_without, digest_with,
        "connected_to edge structural digest'i değiştirmeli — production claim transformation \
         (build_claim_from_proposal connected_to → delta_edges) digest'e taşınmalı"
    );
}

/// `removed_edges.from` effective legacy selector'a dahil (P1-1 regression).
///
/// V1 characterization yolu affected_nodes ∪ removed_edges.from ölçer. Serializer
/// effective selector olarak bu birleşimi kullanır. Bu test, removed_edges.from içeren
/// case'in farklı measured-subject digest ürettiğini pinler — ileride serializer raw
/// affected_nodes'a geri dönerse (removed_edges.from düşer) test kırılır.
#[test]
fn removed_edge_source_is_part_of_effective_legacy_selector() {
    use osp_core::agent::{DeltaProposal, EdgeRef};

    let base = direct_coupling_base_for_regression();

    // Variant A: removed_edges boş → effective selector = affected_nodes = [1].
    let mut without_removed = base.clone();
    without_removed.proposal = DeltaProposal {
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 1,
            to: 2,
            kind: osp_core::space::EdgeKind::Imports,
        }],
        affected_nodes: vec![1],
        removed_edges: vec![],
        ..Default::default()
    };

    // Variant B: removed_edges [9→1] → effective selector = {1} ∪ {9} = [1,9].
    let mut with_removed = base.clone();
    with_removed.proposal = DeltaProposal {
        new_edges: vec![osp_core::agent::NewEdgeSpec {
            from: 1,
            to: 2,
            kind: osp_core::space::EdgeKind::Imports,
        }],
        affected_nodes: vec![1],
        removed_edges: vec![EdgeRef {
            from: 9,
            to: 1,
            kind: osp_core::space::EdgeKind::Imports,
        }],
        ..Default::default()
    };

    let digest_without = serialize_digest(&without_removed).expect("without-removed digest");
    let digest_with = serialize_digest(&with_removed).expect("with-removed digest");

    assert_ne!(
        digest_without, digest_with,
        "removed_edges.from effective legacy selector'a dahil olmalı — V1 ölçüm subject'i \
         affected_nodes ∪ removed_edges.from, serializer bu birleşimi kullanmalı"
    );
}

/// Regression test'leri için minimal direct-coupling taban case (manifest'e dahil DEĞİL).
///
/// Sadece serialize_measured_subject_bytes'in production-effective closure yollarını
/// test etmek için — space/task/proposal minimal ve tutarlı.
fn direct_coupling_base_for_regression() -> common::CharacterizationCase {
    use osp_core::space::{Node, NodeKind, Space};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, PredicateSet,
        TaskPolicy, TaskStatus, WeightedPredicate,
    };

    let mut space = Space::new();
    space.insert_node(Node {
        id: 1,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });
    space.insert_node(Node {
        id: 2,
        kind: NodeKind::Module,
        mass: 1.0,
        ..Default::default()
    });

    let predicate = MetricPredicate {
        metric: PredicateAxis::Coupling,
        operator: ComparisonOp::Le,
        threshold: 0.5,
        scope: PredicateScope::Node(1),
        required_source: None,
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
        id: 99,
        milestone_id: 0,
        label: "direct-coupling-regression-base".to_string(),
        target_predicate_set: ps,
        policy: TaskPolicy::default(),
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };

    common::CharacterizationCase {
        id: "direct-coupling-regression-base".to_string(),
        class: common::CaseClass::DirectPerAxisAuthority,
        source: common::CaseSource::SyntheticAdversarial,
        description: "regression test base (manifest'e dahil değil)".to_string(),
        space,
        task,
        proposal: osp_core::agent::DeltaProposal::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Issue #92 — downstream decision drift matrisi (002 role-bearing variants)
//
// Zincir: subject → raw → theta → Q5 verdict → reachability → predicate →
// mutation → decision_drift_class. İki eksen (review P1-2/P1-4):
// - `DecisionDriftClass`: MD-1 CANONICAL taxonomy — yalnız KARAR yüzeyini
//   (predicate/policy/mutation) sınıflandırır. `NoDrift` = "karar yüzeyinde
//   drift yok" — "hiçbir şey drift etmedi" DEĞİL.
// - Orthogonal gözlemler: subject/raw/theta drift + pipeline reachability —
//   taxonomy'ye karışmaz, bağımsız raporlanır.
// ═══════════════════════════════════════════════════════════════════════════════

/// MD-1 canonical cutover acceptance taxonomy (migration-decisions.md) — değiştirilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecisionDriftClass {
    NoDrift,
    #[allow(dead_code)]
    SourceLabelOnly,
    #[allow(dead_code)]
    PredicateResultDrift,
    #[allow(dead_code)]
    PolicyDecisionDrift,
    #[allow(dead_code)]
    MutationDecisionDrift,
}

/// Orthogonal drift gözlemi (canonical taxonomy'den bağımsız).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DriftFlag {
    Divergent,
    Parity,
}

/// Orthogonal pipeline reachability gözlemi (canonical taxonomy'den bağımsız).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineReachability {
    BothReached,
    #[allow(dead_code)]
    V1StoppedBeforeCommit,
    #[allow(dead_code)]
    V2StoppedBeforeCommit,
    #[allow(dead_code)]
    BothStoppedBeforeCommit,
}

/// #92 dondurülmüş downstream empirical snapshot (review P1-3): characterization
/// "iki taraf eşit" DEĞİL, "iki taraf ŞU exact durumda eşit" pinler. Aksi halde
/// NotCompleted==NotCompleted + Reject==Reject durumunda da NoDrift üretilir ama
/// dondurulan #92 kanıtı sessizce bozulur.
fn q92_assert_exact_downstream_snapshot(
    case_id: &str,
    obs: &CharacterizationObservation,
    lane: &str,
) {
    use osp_core::trajectory::{ApplyTarget, MutationDecision, PredicateCompletion};
    match (&obs.pipeline, lane) {
        // **P1-tur6 (review — truth-surface):** V1 = ReferenceEvaluation —
        // production pipeline ÇALIŞMADI; yalnız PredicateGate gerçek girdilerle
        // doğrudan koştu (Q5/witness gözlenmez; CommitReached fabrication'ı kapanır).
        // Karar yüzeyi (predicate/mutation/apply) gerçek gate çıktısı.
        (
            PipelineObservation::ReferenceEvaluation {
                predicate_completion,
                mutation_decision,
                apply_target,
            },
            "V1",
        ) => {
            assert_eq!(
                *predicate_completion,
                Some(PredicateCompletion::Completed),
                "{case_id} V1: predicate exact = Some(Completed) (reference evaluation)"
            );
            assert_eq!(
                *mutation_decision,
                Some(MutationDecision::AcceptAsCompleted),
                "{case_id} V1: mutation exact = Some(AcceptAsCompleted) (reference evaluation)"
            );
            // P2-1: enum equality (ApplyTarget PartialEq/Eq derive'lu) — Debug
            // representation semantic contract değildir.
            assert_eq!(
                *apply_target,
                Some(ApplyTarget::Lane(
                    osp_core::trajectory::CommitLane::Mainline
                )),
                "{case_id} V1: apply exact = Some(Lane(Mainline)) (reference evaluation)"
            );
        }
        // V2 = gerçek commit_task_claim yüzeyi: Q5 Passed + boş witness ile Held
        // (authorization.outcome gerçek AttemptOutcome — fabrication DEĞİL).
        (
            PipelineObservation::CommitReached {
                q5,
                predicate_completion,
                mutation_decision,
                apply_target,
                witness_reachability,
            },
            "V2",
        ) => {
            assert_eq!(
                *q5,
                common::Q5Observation::Passed,
                "{case_id} V2: Q5 exact = Passed (gerçek pipeline)"
            );
            assert_eq!(
                *predicate_completion,
                Some(PredicateCompletion::Completed),
                "{case_id} V2: predicate exact = Some(Completed)"
            );
            assert_eq!(
                *mutation_decision,
                Some(MutationDecision::AcceptAsCompleted),
                "{case_id} V2: mutation exact = Some(AcceptAsCompleted)"
            );
            assert_eq!(
                *apply_target,
                Some(ApplyTarget::Lane(
                    osp_core::trajectory::CommitLane::Mainline
                )),
                "{case_id} V2: apply exact = Some(Lane(Mainline))"
            );
            assert_eq!(
                *witness_reachability,
                common::WitnessReachability::Held,
                "{case_id} V2: witness exact = Held"
            );
        }
        (other, lane) => {
            panic!("{case_id} {lane}: unexpected pipeline shape: {other:?}")
        }
    }
}

/// Tek 002 case'inin tam downstream drift matrisini üretir + subject/raw
/// kanıtlarını assert eder. Mirror pinning: integration harness'ın kendi V1
/// subject derivation'ı aynı exact setleri üretir (navigator/unit pin'i ile
/// dual — cross-crate assertion kurulamaz, iki yüz bağımsız pinlenir).
///
/// Downstream yüzey exact snapshot olarak pinlenir (P1-3) — parity türetmesi
/// değil: zincirin her halkası literal dondurulur, üstüne NoDrift sınıflanır.
///
/// `decision_drift_class: Option<…>` (P2-2): yalnız `BothReached` için
/// sınıflandırılır — karar yüzeyine ulaşılmayan (stopped) durumlarda stage/error
/// equality'siz `NoDrift` üretmek yanlış sınıflandırma riski taşır; `None` =
/// "karar yüzeyi sınıflandırılmadı".
fn q92_drift_matrix(
    case_id: &str,
) -> (
    DriftFlag,
    DriftFlag,
    PipelineReachability,
    Option<DecisionDriftClass>,
) {
    let case = common::case_by_id(case_id);
    let (obs_v1, obs_v2) = observe_case(&case);

    // ── Subject drift (mirror pinning yanı) ──
    let (subj_v1, subj_v2) = measurement_subjects(&obs_v1, &obs_v2, case_id);
    let subject_drift = if subj_v1 != subj_v2 {
        DriftFlag::Divergent
    } else {
        DriftFlag::Parity
    };

    // ── Raw drift (value bits) ──
    let (bits_v1, bits_v2) = measurement_value_bits(&obs_v1, &obs_v2, case_id);
    let raw_drift = if bits_v1 != bits_v2 {
        DriftFlag::Divergent
    } else {
        DriftFlag::Parity
    };

    // ── Reachability + exact downstream snapshot (P1-3 + P2-1 + P2-2) ──
    // q5/predicate/mutation/apply/witness EXACT dondurulur (discard yok);
    // karar sınıflandırması yalnız BothReached'te — exact snapshot'tan gelir.
    // **P1-tur6:** karar yüzeyine ulaşma = CommitReached (gerçek commit) VEYA
    // ReferenceEvaluation (yalnız gate koştu) — iki şekil de karar sınıflandırılır.
    let (reachability, decision_drift_class) = match (&obs_v1.pipeline, &obs_v2.pipeline) {
        (
            PipelineObservation::CommitReached { .. }
            | PipelineObservation::ReferenceEvaluation { .. },
            PipelineObservation::CommitReached { .. }
            | PipelineObservation::ReferenceEvaluation { .. },
        ) => {
            q92_assert_exact_downstream_snapshot(case_id, &obs_v1, "V1");
            q92_assert_exact_downstream_snapshot(case_id, &obs_v2, "V2");
            // Exact snapshot pinlendikten sonra parity construction-by-design:
            // her iki taraf aynı literal durumda → karar yüzeyi NoDrift.
            (
                PipelineReachability::BothReached,
                Some(DecisionDriftClass::NoDrift),
            )
        }
        (
            PipelineObservation::StoppedBeforeCommit { .. },
            PipelineObservation::StoppedBeforeCommit { .. },
        ) => (PipelineReachability::BothStoppedBeforeCommit, None),
        (PipelineObservation::StoppedBeforeCommit { .. }, _) => {
            (PipelineReachability::V1StoppedBeforeCommit, None)
        }
        (_, PipelineObservation::StoppedBeforeCommit { .. }) => {
            (PipelineReachability::V2StoppedBeforeCommit, None)
        }
    };

    eprintln!(
        "#92 drift matrix [{}]: subject={:?} raw={:?} reachability={:?} → decision_class={:?} (exact snapshot pinned)",
        case_id, subject_drift, raw_drift, reachability, decision_drift_class
    );

    (subject_drift, raw_drift, reachability, decision_drift_class)
}

#[test]
fn wide_affected_scope_002_shows_downstream_decision_drift_matrix() {
    let (subject, raw, reachability, decision) = q92_drift_matrix("wide-affected-scope-002");

    // Orthogonal gözlemler: subject + raw DIVERGENT (001'den korunur).
    assert_eq!(
        subject,
        DriftFlag::Divergent,
        "V1 {{1,2,3}} vs V2 {{1}} — 001 divergence korunur"
    );
    assert_eq!(raw, DriftFlag::Divergent, "raw divergence 001'den korunur");

    // Karar yüzeyi: BothReached + NoDrift (theta drift verdict'e yayılmaz —
    // engine-unit goldens: θ 0.15249/0.11711, ikisi de bound 0.3 altında).
    assert_eq!(reachability, PipelineReachability::BothReached);
    assert_eq!(
        decision,
        Some(DecisionDriftClass::NoDrift),
        "decision surface parity: Q5 Passed/Passed, Completed/Completed, AcceptAsCompleted parity"
    );
}

#[test]
fn removed_edge_external_source_002_shows_downstream_decision_drift_matrix() {
    let (subject, raw, reachability, decision) =
        q92_drift_matrix("removed-edge-external-source-002");

    // V1 {1,9} (removed_edges.from fold) vs V2 {1}.
    assert_eq!(subject, DriftFlag::Divergent, "V1 {{1,9}} vs V2 {{1}}");
    assert_eq!(raw, DriftFlag::Divergent);
    assert_eq!(reachability, PipelineReachability::BothReached);
    assert_eq!(
        decision,
        Some(DecisionDriftClass::NoDrift),
        "decision surface parity (θ 0.13770/0.11711 — ikisi de bound altında)"
    );
}

/// #92 review P1-2: intervention purity GERÇEK frozen corpus üzerinde kanıtlanır —
/// engine-unit lokal fixture'ları arasında değil. Manifest-korumalı 001/002
/// builder'larının ürettiği observation'lar arasında bit-exact raw eşitliği.
///
/// 002'nin tek deneysel müdahalesi "izole role-bearing new_node ile Q5 theta
/// yüzeyini açmak"; subject/raw divergence 001'den aynen korunur:
/// raw_v1(002) == raw_v1(001) VE raw_v2(002) == raw_v2(001) — her iki family için.
#[test]
fn variant_002_raw_purity_matches_001_frozen_corpus() {
    for (id_001, id_002, expected_v1_subject, expected_v2_subject) in [
        (
            "wide-affected-scope-001",
            "wide-affected-scope-002",
            vec![1u64, 2, 3],
            vec![1u64],
        ),
        (
            "removed-edge-external-source-001",
            "removed-edge-external-source-002",
            vec![1u64, 9],
            vec![1u64],
        ),
    ] {
        let case_001 = common::case_by_id(id_001);
        let case_002 = common::case_by_id(id_002);
        let (obs_001_v1, obs_001_v2) = observe_case(&case_001);
        let (obs_002_v1, obs_002_v2) = observe_case(&case_002);

        // Subject: 002, 001'in exact subject setlerini korur (mirror pinning —
        // integration harness derivation'ı, navigator pin'i ile dual).
        let (s_001_v1, s_001_v2) = measurement_subjects(&obs_001_v1, &obs_001_v2, id_001);
        let (s_002_v1, s_002_v2) = measurement_subjects(&obs_002_v1, &obs_002_v2, id_002);
        assert_eq!(s_001_v1, expected_v1_subject, "{id_001} V1 subject exact");
        assert_eq!(s_001_v2, expected_v2_subject, "{id_001} V2 subject exact");
        assert_eq!(
            s_002_v1, expected_v1_subject,
            "{id_002} V1 subject == 001 (korunur)"
        );
        assert_eq!(
            s_002_v2, expected_v2_subject,
            "{id_002} V2 subject == 001 (korunur)"
        );

        // Intervention purity: raw bit-exact eşitlik — gerçek corpus builders arasında.
        let (r_001_v1, r_001_v2) = measurement_value_bits(&obs_001_v1, &obs_001_v2, id_001);
        let (r_002_v1, r_002_v2) = measurement_value_bits(&obs_002_v1, &obs_002_v2, id_002);
        assert_eq!(
            r_002_v1, r_001_v1,
            "{id_002} V1 raw == {id_001} V1 raw (bit-exact; izole node tek müdahale)"
        );
        assert_eq!(
            r_002_v2, r_001_v2,
            "{id_002} V2 raw == {id_001} V2 raw (bit-exact)"
        );

        // Divergence korunur (bu fixture'larda V1 raw != V2 raw).
        assert_ne!(
            r_002_v1, r_002_v2,
            "{id_002} subject divergence → raw divergence"
        );
    }
}
