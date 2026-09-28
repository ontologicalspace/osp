//! #95 MD-1 P2-1 — `SubjectAuthorityDriftObservation` integration acceptance tests
//! (plan v6 §5). Frozen corpus'a YENİ case eklenmez; mevcut 001/002/direct family'ler
//! `case_by_id` ile kullanılır.
//!
//! Kabul kriterleri (plan v6):
//! 1. Non-mutation (integration katmanı: revision + node/edge parity; axis katmanı
//!    engine.rs unit testlerinde).
//! 3. Q5 reachability — V2 Violated → downstream None.
//! 4. Q6 RuleViolation → ReachedButUnavailable.
//! 6. Provenance-confound sentinel (subject/raw parity + provenance/downstream divergent).
//! 7. Measurement failure transparency — V2 failure V1'i etkilemez.
//! 9. 002 cross-pin'ler (#92 engine-unit + PR #120 integration golden'larıyla).
//! 10. 001 draft characterization + non-surviving drop.
//! 13. Serde legacy — eski TrajectoryEvidence JSON → None.
//!
//! (2 Held / 5 Draft invariant / 8 exhaustiveness / 11 RevisionRequired wire /
//! 12 MCP additive-contract kriterleri ilgili dosyalarda: navigator.rs tests,
//! subject_authority.rs tests, authorization.rs tests, osp-mcp/tests.)

mod common;

use common::{case_by_id, engine_from_space, engine_with_case_space};
use osp_core::coords::{MetricSource, RawPosition};
use osp_core::space::{Edge, EdgeKind, Node, NodeKind, Space};
use osp_core::subject_authority::{
    observe_subject_authority_drift, v1_downstream_from_engine_commit_error,
    AuthoritativeDownstreamObservation, EvaluatedQ5Verdict, LaneQ5Observation,
    Q5ObservationFailure, V1DownstreamObservation, V1DownstreamUnavailableReason, V2LaneOutcome,
};
use osp_core::trajectory::{
    ComparisonOp, InMemoryTaskRegistry, MetricPredicate, MutationDecision, PredicateAxis,
    PredicateCompletion, PredicateFailurePolicy, PredicateGate, PredicateGateInput, PredicateMode,
    PredicateScope, PredicateSet, Task, TaskBoundClaim, TaskPolicy, TaskResolver, TaskStatus,
    WeightedPredicate,
};
use osp_core::witness::WitnessSet;

// ═══════════════════════════════════════════════════════════════════════════════
// Fixture — tek setup kaynağı (evaluate_v1_case mapping mirror: probe claim →
// claim.delta_nodes tek mapping kaynağı; loss_before navigator mirror).
// ═══════════════════════════════════════════════════════════════════════════════

struct CaseSetup {
    engine: osp_core::engine::SpaceEngine,
    claim: osp_core::witness::Claim,
    task: Task,
    native: osp_core::engine::NativeAttemptMeasurement,
    /// **#96 MD-2 P0-tur4:** Sealed carrier (draft.finalize ürünü) — commit
    /// pipeline yalnız bunu kabul eder (finalize bypass type-level imkânsız).
    carrier: osp_core::task_measurement::FinalizedNativeTaskClaim,
    loss_before: f64,
    target: RawPosition,
}

fn setup_case(case_id: &str) -> CaseSetup {
    let case = case_by_id(case_id);
    let engine = engine_with_case_space(&case);
    setup_with_engine(engine, &case)
}

fn setup_with_engine(
    engine: osp_core::engine::SpaceEngine,
    case: &common::CharacterizationCase,
) -> CaseSetup {
    // **#96 MD-2:** draft (probe + Q4 structural) → tek-session native bundle →
    // finalize. Observer artık native bundle üzerinden (V1 lane = authority native;
    // V2 lane = md1_shadow) — provenance iki lane'de native, gözlem yalnız SUBJECT
    // farkı taşır (re-anchor).
    let draft = osp_core::task_measurement::StructurallyValidatedClaimDraft::try_new(
        &case.proposal,
        RawPosition::default(),
        &case.task,
        100,
        1,
    )
    .expect("draft (probe + structural Q4) should succeed for corpus case");
    let native = engine
        .measure_attempt_native_with_md1_shadow(&draft, &case.task)
        .expect("native measurement should succeed for corpus case");
    let finalized = draft
        .finalize(native.authority())
        .expect("subject binding: draft ve token ayni proposal");
    let claim = finalized.claim().clone();
    let target = case
        .task
        .target_predicate_set
        .preferred_vector
        .unwrap_or_default();
    // loss_before: navigator mirror (evaluate_v1_case P0-2 fix) — pre-delta
    // affected centroid üzerinden (running scalar — #96/#97'de DEĞİŞMEZ).
    let affected =
        osp_core::subject_authority::derive_v1_legacy_measurement_subject(&case.proposal);
    let pre_raw = engine.compute_raw_from_delta(&[], &[], &[], &affected);
    let current_measured = osp_core::navigator::provenanced_from_raw(pre_raw, MetricSource::Scip);
    let loss_before = osp_core::trajectory::trajectory_loss(&current_measured, &target);
    CaseSetup {
        engine,
        claim,
        task: case.task.clone(),
        native,
        carrier: finalized,
        loss_before,
        target,
    }
}

/// Observer sözleşmesi: `native.authority().raw() == claim.computed_raw` bit-exact
/// (production wiring — navigator claim'i draft.finalize(token) ile kurar; #96).
fn assert_legacy_claim_raw_contract(s: &CaseSetup) {
    let token_bits = [
        s.native.authority().raw().x.to_bits(),
        s.native.authority().raw().y.to_bits(),
        s.native.authority().raw().z.to_bits(),
        s.native.authority().raw().w.to_bits(),
        s.native.authority().raw().v.to_bits(),
    ];
    let claim_bits = [
        s.claim.computed_raw.x.to_bits(),
        s.claim.computed_raw.y.to_bits(),
        s.claim.computed_raw.z.to_bits(),
        s.claim.computed_raw.w.to_bits(),
        s.claim.computed_raw.v.to_bits(),
    ];
    assert_eq!(
        token_bits, claim_bits,
        "authority token raw must equal claim.computed_raw bit-exact"
    );
}

fn space_fingerprint(engine: &osp_core::engine::SpaceEngine) -> (Vec<u64>, Vec<(u64, u64)>) {
    let mut node_ids: Vec<u64> = engine.space().nodes.keys().copied().collect();
    node_ids.sort_unstable();
    let mut edges: Vec<(u64, u64)> = engine
        .space()
        .edges
        .iter()
        .map(|e| (e.from, e.to))
        .collect();
    edges.sort_unstable();
    (node_ids, edges)
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1 — Non-mutation (integration katmanı)
// ═══════════════════════════════════════════════════════════════════════════════

/// Gözlem öncesi/sonrası `SpaceViewRevision` (content digest dahil) + node/edge set
/// exact parity. Axis descriptor/epoch katmanı engine.rs unit testlerinde.
#[test]
fn observation_does_not_mutate_engine_state() {
    let s = setup_case("wide-affected-scope-002");
    let revision_before = s
        .engine
        .current_space_view_revision()
        .expect("revision computation must succeed");
    let fingerprint_before = space_fingerprint(&s.engine);

    let draft = observe_subject_authority_drift(
        &s.engine,
        &s.claim,
        &s.task,
        &s.native,
        s.loss_before,
        &s.target,
    );
    let _finalized = draft.finalize(V1DownstreamObservation::Observed(
        AuthoritativeDownstreamObservation {
            predicate_completion: PredicateCompletion::NotCompleted,
            mutation_decision: MutationDecision::Reject,
        },
    ));

    let revision_after = s
        .engine
        .current_space_view_revision()
        .expect("revision computation must succeed after observation");
    assert_eq!(
        revision_before, revision_after,
        "observation must not change space view revision (content digest included)"
    );
    assert_eq!(
        fingerprint_before,
        space_fingerprint(&s.engine),
        "observation must not change node/edge sets"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9 — 002 cross-pin'ler (#92 kanıt değerleriyle)
// ═══════════════════════════════════════════════════════════════════════════════

/// `wide-affected-scope-002`: V1 subject {1,2,3} vs V2 {1}; θ bit goldens engine.rs
/// #92 testleriyle (4594662147918958728 / 4593103093345799528) cross-pin; raw bit
/// goldens PR #120 parity testleriyle (raw(002)==raw(001) purity).
#[test]
fn wide_affected_scope_002_observation_cross_pinned_to_92_goldens() {
    let s = setup_case("wide-affected-scope-002");
    assert_legacy_claim_raw_contract(&s);
    let draft = observe_subject_authority_drift(
        &s.engine,
        &s.claim,
        &s.task,
        &s.native,
        s.loss_before,
        &s.target,
    );

    // **#95-A regolden (subject-cutover):** V1 lane artık canonical task scope
    // taşır — subject/raw/θ PARITY. *(Historical pre-#95-A golden: V1 affected
    // union [1,2,3] vs V2 [1] divergence; reason: `subject-cutover (#95-A)`)*
    assert_eq!(draft.v1().subject.ids, vec![1]);
    let v2 = match draft.v2() {
        V2LaneOutcome::Measured(v2) => v2,
        V2LaneOutcome::MeasurementFailed(f) => {
            panic!("002 V2 lane must measure; got failure: {f:?}")
        }
    };
    assert_eq!(v2.subject.ids, vec![1]);
    assert_eq!(draft.v1().subject.digest, v2.subject.digest);

    // **#95-A regolden:** iki lane aynı task scope'u aynı session'da ölçer —
    // raw bits PARITY (task-scope golden; historical V1 union bits:
    // [4595172819793696085, ...] — reason: `subject-cutover (#95-A)`).
    assert_eq!(
        v2.raw.bits,
        [
            4602678819172646912,
            4602678819172646912,
            4607182418800017408,
            4602678819172646912,
            4601046424471046557,
        ],
        "task-scope raw bits (frozen corpus #92/#120 goldens)"
    );
    assert_eq!(draft.v1().raw.bits, v2.raw.bits, "lane raw parity");

    // Provenance — **#96 MD-2 re-anchor (regolden):** V1 lane artık NATIVE per-axis
    // kaynaklar taşır (eski pin: uniform [Scip;5] compatibility projection — tarihsel;
    // değerler/bitler yukarıdaki #92/#120 goldens'leriyle AYNEN korundu, yalnız kaynak
    // etiketleri native). Gözlem artık yalnız SUBJECT farkı taşır; provenance ekseni
    // #96 MD-2 observer'ına (provenance_authority.rs) aittir.
    assert_eq!(
        draft.v1().raw.sources,
        [
            MetricSource::TreeSitter,
            MetricSource::Placeholder,
            MetricSource::TreeSitter,
            MetricSource::Heuristic,
            MetricSource::Heuristic,
        ],
        "V1 lane native per-axis sources (fixture axis seti)"
    );
    assert_ne!(v2.raw.sources, [MetricSource::Scip; 5]);

    // Q5 same-context divergence — #92 engine-unit goldens.
    match &draft.v1().q5 {
        LaneQ5Observation::Evaluated {
            theta_bits,
            verdict,
            ..
        } => {
            // **#95-A:** V1 θ = task-scope θ (V2 ile parity; historical union
            // θ 4594662147918958728 — reason: `subject-cutover (#95-A)`).
            assert_eq!(
                *theta_bits, 4593103093345799528,
                "V1 θ = task-scope θ (≈0.11711)"
            );
            assert_eq!(*verdict, EvaluatedQ5Verdict::Passed);
        }
        other => panic!("002 V1 Q5 must be Evaluated: {other:?}"),
    }
    match &v2.q5 {
        LaneQ5Observation::Evaluated {
            theta_bits,
            verdict,
            ..
        } => {
            assert_eq!(*theta_bits, 4593103093345799528, "V2 θ bits (≈0.11711)");
            assert_eq!(*verdict, EvaluatedQ5Verdict::Passed);
        }
        other => panic!("002 V2 Q5 must be Evaluated: {other:?}"),
    }

    // V2 shadow downstream — Q5 Passed → production PredicateGate ile hesaplanır.
    let downstream = v2
        .downstream
        .as_ref()
        .expect("002 V2 Q5 Passed → downstream must be computed");
    assert_eq!(
        downstream.predicate_completion,
        PredicateCompletion::Completed
    );
}

/// `removed-edge-external-source-002`: V1 subject {1,9} (removed_edge.from folded)
/// vs V2 {1}; θ bits 4594129220971291796 / 4593103093345799528 (#92 goldens).
#[test]
fn removed_edge_external_source_002_observation_cross_pinned_to_92_goldens() {
    let s = setup_case("removed-edge-external-source-002");
    let draft = observe_subject_authority_drift(
        &s.engine,
        &s.claim,
        &s.task,
        &s.native,
        s.loss_before,
        &s.target,
    );

    // **#95-A regolden (subject-cutover):** V1 = canonical task scope [1] —
    // parity. *(Historical: V1 affected-union [1,9] (removed_edge.from folded),
    // θ 4594129220971291796; reason: `subject-cutover (#95-A)`)*
    assert_eq!(draft.v1().subject.ids, vec![1]);
    let v2 = match draft.v2() {
        V2LaneOutcome::Measured(v2) => v2,
        V2LaneOutcome::MeasurementFailed(f) => {
            panic!("removed-002 V2 lane must measure; got failure: {f:?}")
        }
    };
    assert_eq!(v2.subject.ids, vec![1]);
    assert_eq!(draft.v1().subject.digest, v2.subject.digest);

    match &draft.v1().q5 {
        LaneQ5Observation::Evaluated { theta_bits, .. } => {
            assert_eq!(
                *theta_bits, 4593103093345799528,
                "V1 θ = task-scope θ (≈0.11711)"
            );
        }
        other => panic!("removed-002 V1 Q5 must be Evaluated: {other:?}"),
    }
    match &v2.q5 {
        LaneQ5Observation::Evaluated { theta_bits, .. } => {
            assert_eq!(*theta_bits, 4593103093345799528, "V2 θ bits (≈0.11711)");
        }
        other => panic!("removed-002 V2 Q5 must be Evaluated: {other:?}"),
    }
    assert!(
        v2.downstream.is_some(),
        "Q5 Passed → V2 downstream computed"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10 — 001: Draft characterization + non-surviving drop
// ════════════════════════════════════════════════════════════════════════════════

/// 001 a) Draft characterization: GlobalDefault pre-theta yüzeyi — iki lane'in Q5'si
/// de aynı captured-context hatasıyla `NotEvaluated{VisionAuthorityInsufficient}`;
/// V2 ölçümü bağımsız olarak başarılı (vision'a ihtiyaç duymaz) ama downstream
/// üretilmez (counterfactual yasak).
///
/// 001 b) Production eligibility: commit `VisionContextInvalid` (terminal,
/// non-surviving) → `v1_downstream_from_engine_commit_error` None → observation
/// emit EDİLMEZ.
#[test]
fn wide_affected_scope_001_draft_characterization_and_non_surviving_drop() {
    let mut s = setup_case("wide-affected-scope-001");
    let draft = observe_subject_authority_drift(
        &s.engine,
        &s.claim,
        &s.task,
        &s.native,
        s.loss_before,
        &s.target,
    );

    // (a) Draft characterization — iki lane aynı NotEvaluated reason.
    assert_eq!(
        draft.v1().q5,
        LaneQ5Observation::NotEvaluated {
            reason: Q5ObservationFailure::VisionAuthorityInsufficient,
        },
        "001 V1 Q5: GlobalDefault insufficient authority (pre-theta surface)"
    );
    let v2 = match draft.v2() {
        V2LaneOutcome::Measured(v2) => v2,
        V2LaneOutcome::MeasurementFailed(f) => {
            panic!("001 V2 measurement itself must succeed (vision-independent): {f:?}")
        }
    };
    assert_eq!(
        v2.q5,
        LaneQ5Observation::NotEvaluated {
            reason: Q5ObservationFailure::VisionAuthorityInsufficient,
        }
    );
    assert!(
        v2.downstream.is_none(),
        "Q5 NotEvaluated → counterfactual downstream üretilmez"
    );

    // (b) Non-surviving drop — commit terminal VisionContextInvalid.
    let mut registry = InMemoryTaskRegistry::new();
    registry.insert(s.task.clone());
    let result = s
        .engine
        .commit_task_claim(osp_core::engine::TaskCommitInput::new(
            &s.carrier,
            &WitnessSet::new(vec![]),
            &registry as &dyn TaskResolver,
            s.target,
            s.loss_before,
        ));
    match &result {
        Err(osp_core::engine::EngineCommitError::VisionContextInvalid(_)) => {}
        other => panic!("001 commit must fail terminal VisionContextInvalid: {other:?}"),
    }
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert_eq!(
        v1_downstream_from_engine_commit_error(&err),
        None,
        "VisionContextInvalid = non-surviving → observation emit YOK (eligibility contract)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6 — Provenance-confound sentinel
// ═══════════════════════════════════════════════════════════════════════════════

/// Inline required-source fixture (corpus `direct-coupling-required-scip-*`
/// semantics'ini taşır; corpus case'lerinde role-bearing delta node YOK → Q5
/// NotEvaluated → downstream quadruple o fixture'da erişilemez — bu yüzden inline).
///
/// Sentinel quadruple: subject **parity** + raw bits **parity** + provenance
/// **divergent** (V1 compatibility-projected [Scip;5] vs V2 engine-native) +
/// downstream **divergent** (required_source=Scip yalnız V1 measured'ı karşılar).
/// Bu quadruple, downstream farkının "subject authority caused predicate drift"
/// olarak yanlış okunmasını imkânsız kılar — farkın görünür nedeni provenance (MD-2).
#[test]
fn direct_per_axis_required_scip_provenance_confound_sentinel() {
    use osp_core::agent::{NewEdgeSpec, NewNodeSpec};

    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: NodeKind::Module, // role-bearing → Q5 yüzeyi açık (002 pattern)
            initial_mass: 1.0,
            connected_to: vec![],
        }],
        new_edges: vec![NewEdgeSpec {
            from: 1,
            to: 2,
            kind: EdgeKind::Imports,
        }],
        removed_edges: vec![],
        affected_nodes: vec![1], // V1 subject == task scope → parity by construction
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "provenance sentinel fixture".to_string(),
    };
    let task = Task {
        id: 1,
        milestone_id: 1,
        label: "required_source=Scip sentinel".into(),
        target_predicate_set: PredicateSet {
            mode: PredicateMode::All,
            predicates: vec![WeightedPredicate {
                predicate: MetricPredicate {
                    metric: PredicateAxis::Coupling,
                    operator: ComparisonOp::Le,
                    threshold: 10.0,
                    scope: PredicateScope::Node(1),
                    required_source: Some(MetricSource::Scip),
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: None,
        },
        policy: TaskPolicy {
            maneuver_limit: 5,
            predicate_failure_policy: PredicateFailurePolicy::AcceptImprovement,
            ..Default::default()
        },
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };
    let case = common::CharacterizationCase {
        id: "inline-required-source-scip-sentinel".into(),
        class: common::CaseClass::DirectPerAxisAuthority,
        source: common::CaseSource::SyntheticAdversarial,
        description: "inline fixture — provenance confound sentinel".into(),
        space: module_scope_space(),
        task,
        proposal,
    };
    let s = setup_with_engine(engine_from_space(case.space.clone()), &case);
    let draft = observe_subject_authority_drift(
        &s.engine,
        &s.claim,
        &s.task,
        &s.native,
        s.loss_before,
        &s.target,
    );

    // Subject parity — digest'ler eşit (subject-authority drift YOK).
    assert_eq!(draft.v1().subject.ids, vec![1]);
    let v2 = match draft.v2() {
        V2LaneOutcome::Measured(v2) => v2,
        V2LaneOutcome::MeasurementFailed(f) => panic!("sentinel V2 must measure: {f:?}"),
    };
    assert_eq!(v2.subject.ids, vec![1]);
    assert_eq!(
        draft.v1().subject.digest,
        v2.subject.digest,
        "sentinel: subject parity — aynı digest"
    );

    // Raw parity — aynı subject üzerinden aynı ölçüm.
    assert_eq!(
        draft.v1().raw.bits,
        v2.raw.bits,
        "sentinel: raw bits parity — subject aynı, ölçüm aynı"
    );

    // **#96 MD-2 re-anchor:** provenance artık iki lane'de de NATIVE — MD-1
    // observation provenance confound'u TAŞIMAZ (dogfood Run A senaryosunun MD-1
    // gözlemindeki izdüşümü kaldırıldı; provenance ekseni #96 MD-2 observer'ına
    // [provenance_authority.rs] aittir). Source PARITY pinlenir.
    assert_eq!(
        draft.v1().raw.sources,
        v2.raw.sources,
        "re-anchor: her iki lane native — source parity (confluence subject+provenance)"
    );
    assert_ne!(draft.v1().raw.sources, [MetricSource::Scip; 5]);

    // Q5 her iki lane'de açık ve Passed (role-bearing node) — downstream karşılaştırılabilir.
    assert!(matches!(
        &v2.q5,
        LaneQ5Observation::Evaluated {
            verdict: EvaluatedQ5Verdict::Passed,
            ..
        }
    ));

    // Downstream PARITY — required_source=Scip iki native lane'de de karşılanamaz
    // → SourceInsufficient → NotCompleted. (Eski beklenen: V1 Completed vs V2
    // NotCompleted = MD-2 confound — re-anchor ile MD-1 gözlemden GİTTİ; historical
    // değer reason-note: frozen #88 matrisi + dogfood Run A.)
    let v2_downstream = v2
        .downstream
        .as_ref()
        .expect("sentinel V2 Q5 Passed → downstream computed");
    assert_eq!(
        v2_downstream.predicate_completion,
        PredicateCompletion::NotCompleted,
        "V2 engine-native sources → SourceInsufficient → NotCompleted"
    );
    // V1 lane downstream finalize commit outcome'uyla gelir (draft'ta yok); iki lane
    // subject+raw+sources parity'de olduğundan downstream parity yapısal olarak izler.
    // MD-2 ekseni (reference-vs-native) hâlâ görünür — uniform-Scip REFERENCE
    // projection'ın counterfactual'ı: aynı token değerlerinin Scip izdüşümü
    // required_source=Scip'i KARŞILARDI. Bu karşılaştırma W5 MD-2 observer'ının
    // malzemesidir; burada köprü olarak inline pinlenir.
    let bound = TaskBoundClaim {
        claim: &s.claim,
        task: &s.task,
    };
    let reference_measured =
        osp_core::navigator::provenanced_from_raw(s.native.authority().raw(), MetricSource::Scip);
    let reference_gate = PredicateGate.evaluate(PredicateGateInput {
        bound,
        measured: &reference_measured,
        loss_before: s.loss_before,
        target: &s.target,
    });
    assert_eq!(
        reference_gate.outcome.predicate_completion,
        PredicateCompletion::Completed,
        "uniform-Scip REFERENCE projection → Completed (MD-2 axis: #96 observer malzemesi)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7 — Measurement failure transparency (shadow failure → authoritative etkilenmez)
// ═══════════════════════════════════════════════════════════════════════════════

/// Inline Module-scope task (corpus'a DOKUNMADAN): V2 lane
/// `SubjectScopeResolutionFailed{ModuleResolutionUnavailable}` ile fail-closed
/// sınıflanır; aynı kurulumun gözlemsiz koşumuyla (engine A: produce→claim→commit,
/// engine B: produce→observe→claim→commit) V1 akışı ve engine state'ı bit-identical
/// kalır — shadow lane failure'ı authoritative lane'e sızmaz.
#[test]
fn module_scope_task_fails_at_draft_terminal() {
    use osp_core::agent::{NewEdgeSpec, NewNodeSpec};

    // **#95-A regolden:** Module-scope task'in subject authority'si artık
    // AUTHORITY'dir — türetilemediğinde ölçüm HİÇ çalışmaz (draft aşaması,
    // TerminalTaskDeclaration). *(Historical pre-#95-A: authority lane legacy
    // affected-union ölçüyor, yalnız shadow/V2 lane fail-closed oluyordu;
    // cutover ile iki lane aynı canonical scope'tan türetir, eski senaryo
    // temsili kalmadı.)*
    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: NodeKind::Module,
            initial_mass: 1.0,
            connected_to: vec![],
        }],
        new_edges: vec![NewEdgeSpec {
            from: 1,
            to: 2,
            kind: EdgeKind::Imports,
        }],
        removed_edges: vec![],
        affected_nodes: vec![1],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "module-scope draft-failure fixture".into(),
    };
    let mut task = common::case_by_id("wide-affected-scope-002").task;
    task.target_predicate_set.predicates[0].predicate.scope =
        osp_core::trajectory::PredicateScope::Module("core".into());

    let err = osp_core::task_measurement::StructurallyValidatedClaimDraft::try_new(
        &proposal,
        RawPosition::default(),
        &task,
        1,
        1,
    )
    .expect_err("Module scope → draft aşamasında türetilemez");
    assert!(
        matches!(
            &err,
            osp_core::task_measurement::ClaimDraftError::TaskSubjectScope(
                osp_core::measurement::MeasurementError::SubjectScopeResolutionFailed(_)
            )
        ),
        "TaskSubjectScope(SubjectScopeResolutionFailed) bekleniyordu; got: {err:?}"
    );
}

fn module_scope_space() -> Space {
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
    space.insert_edge(Edge {
        from: 1,
        to: 2,
        kind: EdgeKind::Imports,
        is_type_only: false,
    });
    space
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4 — Q6 RuleViolation → ReachedButUnavailable
// ═══════════════════════════════════════════════════════════════════════════════

/// Test-only Q6 rule — her değerlendirmede ihlal üretir. (Self-import YOK: Q4 syntax
/// gate'i onu daha erken yakalıyor — Q6 yüzeyi için Q4'ün geçtiği bir delta gerekir.)
struct AlwaysViolateRule {
    id: osp_core::rule::RuleId,
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
        _new_nodes: &[Node],
        _new_edges: &[Edge],
        _space: &Space,
    ) -> Option<osp_core::rule::RuleViolation> {
        Some(osp_core::rule::RuleViolation {
            rule_id: self.id.clone(),
            detail: "md1 test sentinel — always violates".to_string(),
            severity: osp_core::rule::RuleSeverity::Hard,
        })
    }
}

/// Production sırası Q5 → PredicateGate → Q6: RuleViolation anında PredicateGate
/// çalışmış olabilir (bu fixture'da çalışır — coupling ≤ 10 trivially satisfied,
/// mutation != Reject → Q6 çalışır) ama gerçek outcome error'da taşınmaz →
/// `ReachedButUnavailable{Q6RuleViolationAfterPredicateGate}`. Sentetik
/// NotCompleted/Reject authoritative diye taşınmaz.
#[test]
fn q6_rule_violation_finalizes_reached_but_unavailable() {
    use osp_core::agent::{NewEdgeSpec, NewNodeSpec};

    let mut engine = engine_from_space(module_scope_space());
    engine
        .register_rule(Box::new(AlwaysViolateRule {
            id: "test.md1_always_violate".to_string(),
        }))
        .expect("rule registration");

    let proposal = osp_core::agent::DeltaProposal {
        new_nodes: vec![NewNodeSpec {
            kind: NodeKind::Module, // role-bearing → vision yüzeyi açık (002 pattern)
            initial_mass: 1.0,
            connected_to: vec![],
        }],
        new_edges: vec![NewEdgeSpec {
            from: 1,
            to: 2,
            kind: EdgeKind::Imports,
        }],
        removed_edges: vec![],
        affected_nodes: vec![1],
        modified_entities: vec![],
        position_hints: vec![],
        reasoning: "q6 fixture".to_string(),
    };
    let task = Task {
        id: 1,
        milestone_id: 1,
        label: "Q6 fixture".into(),
        target_predicate_set: PredicateSet {
            mode: PredicateMode::All,
            predicates: vec![WeightedPredicate {
                predicate: MetricPredicate {
                    metric: PredicateAxis::Coupling,
                    operator: ComparisonOp::Le,
                    threshold: 10.0, // trivially satisfied → mutation != Reject → Q6 çalışır
                    scope: PredicateScope::Node(1),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            }],
            preferred_vector: None,
        },
        policy: TaskPolicy {
            maneuver_limit: 5,
            predicate_failure_policy: PredicateFailurePolicy::AcceptImprovement,
            ..Default::default()
        },
        allowed_operations: vec![],
        constraints: vec![],
        status: TaskStatus::Pending,
    };
    let case = common::CharacterizationCase {
        id: "inline-q6-rule-violation-fixture".into(),
        class: common::CaseClass::DirectPerAxisAuthority,
        source: common::CaseSource::SyntheticAdversarial,
        description: "inline fixture — Q6 RuleViolation".into(),
        space: module_scope_space(),
        task: task.clone(),
        proposal: proposal.clone(),
    };
    let s = setup_with_engine(engine, &case);
    let draft = observe_subject_authority_drift(
        &s.engine,
        &s.claim,
        &s.task,
        &s.native,
        s.loss_before,
        &s.target,
    );

    // Shadow lane: Q5 role-bearing node ile açık + trivially-satisfied predicate.
    let v2 = match draft.v2() {
        V2LaneOutcome::Measured(v2) => v2,
        V2LaneOutcome::MeasurementFailed(f) => panic!("Q6 fixture V2 must measure: {f:?}"),
    };
    assert!(
        matches!(v2.q5, LaneQ5Observation::Evaluated { .. }),
        "Q6 fixture: role-bearing delta node → Q5 evaluable"
    );
    assert!(
        v2.downstream.is_some(),
        "Q5 Passed → V2 shadow downstream computed"
    );

    // Authoritative lane: commit → RuleViolation (Q6, PredicateGate SONRASI).
    let mut registry = InMemoryTaskRegistry::new();
    registry.insert(s.task.clone());
    let mut engine_b = s.engine;
    let result = engine_b.commit_task_claim(osp_core::engine::TaskCommitInput::new(
        &s.carrier,
        &WitnessSet::new(vec![]),
        &registry as &dyn TaskResolver,
        s.target,
        s.loss_before,
    ));
    let err = match result {
        Err(osp_core::engine::EngineCommitError::RuleViolation { .. }) => {
            // Beklenen yüzey.
            match result {
                Err(e) => e,
                Ok(_) => unreachable!(),
            }
        }
        other => panic!("Q6 fixture must fail with RuleViolation: {other:?}"),
    };

    // Navigator/MCP ortak path→finalize eşlemesi: ReachedButUnavailable.
    assert_eq!(
        v1_downstream_from_engine_commit_error(&err),
        Some(V1DownstreamObservation::ReachedButUnavailable {
            reason: V1DownstreamUnavailableReason::Q6RuleViolationAfterPredicateGate,
        })
    );
    // Finalize edilen gözlem bu sınıflandırmayı taşır (evidence'a yazılan şekliyle).
    let finalized =
        draft.finalize(v1_downstream_from_engine_commit_error(&err).expect("surviving"));
    assert_eq!(
        finalized.v1.downstream,
        V1DownstreamObservation::ReachedButUnavailable {
            reason: V1DownstreamUnavailableReason::Q6RuleViolationAfterPredicateGate,
        }
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3 — Q5 reachability (Violated → downstream None) — engine-unit katmanında
//     extreme-vision fixture; burada ek olarak NotEvaluated→None 001'de pinlendi.
// ═══════════════════════════════════════════════════════════════════════════════

// (001 testi NotEvaluated → None'ı zaten pinler; Violated → None engine.rs unit
// testinde extreme-vision fixture ile pinlenir — md1_observer_q5_violated_*.)

// ═══════════════════════════════════════════════════════════════════════════════
// 13 — Serde legacy: eski TrajectoryEvidence JSON → subject_authority_drift None
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn legacy_trajectory_evidence_json_deserializes_with_drift_none() {
    // Eski wire — alan YOK (G2c-1b backward-compat deseni).
    let legacy_json = r#"{
        "trajectory_id": 1,
        "milestone_id": 1,
        "task_id": 7,
        "attempt_id": 3,
        "before": {"x": 0.7, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "after": {"x": 0.6, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.3},
        "predicate_completion": "Completed",
        "mutation_decision": "AcceptAsCompleted",
        "token_cost": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
        "duration_ms": 42
    }"#;
    let evidence: osp_core::trajectory::TrajectoryEvidence =
        serde_json::from_str(legacy_json).expect("legacy JSON must deserialize (serde default)");
    assert!(
        evidence.subject_authority_drift.is_none(),
        "eski wire → subject_authority_drift None"
    );
}
