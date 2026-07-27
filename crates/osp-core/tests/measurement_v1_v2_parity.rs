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

    // 2. 5-axis value bits parity.
    let (vals_v1, vals_v2) = measurement_value_bits(&obs_v1, &obs_v2, &case.id);
    assert_eq!(
        vals_v1, vals_v2,
        "matching scope: 5-axis value bits parity; case {}\nV1={:?}\nV2={:?}",
        case.id, vals_v1, vals_v2
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

    // === Baseline ===
    // V2-candidate: matching scope → Available baseline (node space'te).
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
}

/// `removed-edge-external-source-001`: removed_edges.from external node divergence.
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
}

/// `delta-introduced-subject-001`: V2 baseline semantics divergence.
///
/// Subject node 10000 delta-introduced (base space'te yok, delta ile geliyor —
/// `node_from_spec` id 10_000+0).
/// - V1: current_measured her zaman var → loss_before hesaplanır → improvement evaluable.
/// - V2: `MeasurementBaseline::Unavailable { AllMembersIntroducedByDelta }` →
///   project_v1_loss_before_compatibility fail-closed (loss_before=loss_after →
///   improved=false → Reject).
///
/// **Review P0-1 fix:** Önceki fixture task scope `Node(1)` + delta `NewNodeSpec`
/// (id=10000) kullanıyordu → kimlik uyuşmazlığı → SubjectScope hatası (baseline
/// unavailable DEĞİL). Şimdi task scope `Node(10000)` → gerçek AllMembersIntroducedByDelta.
///
/// **Karakterizasyon bulgusu:** Bu case V1'in "her zaman current_measured var"
/// semantiği ile V2'nin "baseline yoksa progress kanıtlanamaz" (fail-closed)
/// semantiği arasındaki ontolojik ayrımın somut kanıtı.
#[test]
fn delta_introduced_subject_shows_baseline_semantics_divergence() {
    let case = build_all_cases()
        .into_iter()
        .find(|c| c.class == CaseClass::DeltaIntroducedSubject)
        .expect("DeltaIntroducedSubject case olmalı");
    let (obs_v1, obs_v2) = observe_case(&case);

    // V2 measurement: Produced + UnavailableAllIntroduced (kesin pin — review P0-1).
    // Önceki "Unavailable VEYA SubjectScope" toleransı characterization için fazla
    // genişti; fixture düzeltmesi ile artık exact beklenen observation dondurulur.
    match &obs_v2.measurement {
        MeasurementObservation::Produced { baseline_kind, .. } => {
            assert_eq!(
                *baseline_kind,
                Some(common::BaselineKind::UnavailableAllIntroduced),
                "delta-introduced subject (task scope Node(10000) = delta node id) → \
                 V2 baseline UnavailableAllIntroduced (fail-closed). Got baseline_kind: {baseline_kind:?}"
            );
        }
        other => panic!(
            "V2 measurement Produced+UnavailableAllIntroduced olmalı (fixture P0-1 fix \
             ile subject node delta'da mevcut); got {other:?}"
        ),
    }

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

    // **Review tur 3 P0-2 fix:** Pipeline (decision) karşılaştırması.
    // Case 4'te V1/V2 pipeline observation'ları log'la — decision-drift var mı yok mu
    // gerçek observation ile göster. Rapor "decision-drift = 1" iddia ediyordu ama
    // test kanıtlamıyordu. Bu case Q5 Vision'da duruyor (placeholder computed_raw),
    // yani PredicateGate'e ulaşmıyor — gerçek mutation decision drift BURADA ölçülemez.
    eprintln!("  delta-introduced V1 pipeline: {:?}", obs_v1.pipeline);
    eprintln!("  delta-introduced V2 pipeline: {:?}", obs_v2.pipeline);
    // Pipeline observation'larını dondur (rapor için) — ama decision-drift iddiası
    // KALDIRILDI (Cases Q5 Vision'da durur, PredicateGate'e ulaşmaz). Bu case sadece
    // baseline-semantics divergence kanıtlar (V2 fail-closed loss_before projection).
    // P2-0B.8 (non-default computed_raw ile Q5 geçişi) decision-drift ölçümü için gerekli.
}

fn assert_none_or_not_unavailable(baseline_kind: &Option<common::BaselineKind>, msg: &str) {
    match baseline_kind {
        None => {}                                  // V1 → None (tracking yok)
        Some(common::BaselineKind::Available) => {} // hypothetical V1 Available
        Some(other) => panic!("{msg}; got {other:?}"),
    }
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
/// pinleniyor. Bu, PredicateGate decision divergence'ını gerçek evaluation ile kanıtlar.
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
