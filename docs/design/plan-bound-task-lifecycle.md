# Plan-Bound Task Lifecycle

> **Status:** Design sketch — distilled from raw planning conversation (2026-07), NOT yet adopted.
> **Label:** Paper 4 candidate material.
> **Companion artifacts:** [paper outline](plan-bound-paper-outline.md) ·
> [experiment plan draft](plan-bound-experiment-plan.md) (Phase 0 freeze
> checklist included).
> **Source:** `docs/notes/planlama-tasarım-eskiz.txt` (raw conversation transcript; contains a duplicated region at lines 864–1720).

## Özet

Bu doküman, OSP'ye önerilen "Plan-Bound Task Lifecycle" katmanının ham tasarım konuşmasından damıtılmış karar kaydıdır. Temel fikir: plan review (Gₚ) task'ın yapılmasına izin verir, completion review (G꜀) yapılan işin proje gerçekliğine kabulüne izin verir; bunlar iki ayrı protokol kapısıdır. Plan tek bir yerde yaşamaz: okunabilir içeriği repoda versioned artifact olarak (byte digest ile), kabul kararı grafta non-spatial PlanNode olarak, yaptırımı ise deterministik lowering ile üretilen `PlanBinding` olarak durur. Plan koordinat uzayına girmez — `PlanVector` icat edilmez; doğru sıra "önce kaydet, sonra metrik, sonra uzay"dır. Kimlik üç digest'ten oluşur: artifact_digest + lowering_version + binding_digest ve `lower_vN(artifact) == persisted_binding` invariant'ı ile bağlanır. Executor tamamlanma ilan edemez, yalnızca kanıtlı `CompletionClaim` üretir; sapmalar `ExecutorDeclared / ReviewerDiscovered / EngineDetected` olarak ayrı izlenir. Plan, rota değil "reviewed maneuver envelope"dur — bu sayede Paper 2'nin predicate-tabanlı serbest navigasyonu ile çatışmaz. Risk-bazlı politika önerilir (low-risk task'a formal plan yok) ancak risk eşikleri henüz tanımsızdır.

---

## 1. Problem Statement

The original question was procedural: how should pre-task plan review and post-task implementation review be modeled? The conversation converged on a stronger, ontological question:

> What kind of entity is a plan in project reality; where does it live, which part of it is accepted, and which part binds execution?

The two reviews are not two phases of one review process. They are two distinct protocol gates evaluating different objects and different claims:

- **Plan Claim** — "This approach is sufficient, safe, and feasible to realize the task." A forward-looking authorization claim.
- **Completion Claim** — "The produced change satisfies the accepted plan and the task predicates." An evidence-based acceptance claim about realized work.

Plan review evaluates the plan, not code (code may not exist yet). It checks: intent fidelity (task intent → plan objective, no semantic drift), predicate sufficiency (measurable closure conditions instead of "improve the store layer"), scope completeness (in scope / out of scope / deferred / assumptions / dependencies), architectural fit (layering, ownership, identity/provenance/lifecycle boundaries, invalid-state surface), verification sufficiency (tests become acceptance criteria at plan time, not after implementation), and risk/rollback coverage (backward compatibility, migration strategy, rollback, data-corruption risk, partial failure, atomicity).

## 2. Core Decision: Plan as Maneuver Envelope, Not Route

Paper 2's core idea: the agent is given no target coordinate or detailed route — it is given a predicate and navigates. A plan looks, at first glance, like a step-by-step route. The conflict dissolves if the plan is bounded correctly.

A plan defines: authorization boundary, scope, allowed operations, forbidden operations, risk controls, evidence obligations, verification obligations, rollback conditions.

A plan must NOT define: the agent's hidden target coordinate, the single possible implementation route, exact per-file edits, or a completion definition that rewrites the predicate.

> **Plan is not a navigation route; it is a reviewed maneuver envelope.**

The peace treaty with Paper 2, in one line each:

```text
Task predicate defines completion.
Plan binding defines permitted execution.
```

Equivalently: the plan does not describe the solution; it describes the accepted boundaries of the solution space. This preserves the agent's freedom of navigation inside a reviewed safety envelope.

## 3. Three-Layer Placement

The plan does not live in a single space. It decomposes into three layers, each with a natural home that already exists:

1. **Content layer — artifact in the repo.** The human-readable, versioned plan document, e.g. `docs/plans/TASK-057/v3.md`. It carries rationale, approach, risks, scope, verification expectations, out-of-scope decisions, and assumptions. This is what humans review. Its identity is the **byte digest of the exact reviewed bytes** (`artifact_digest = SHA256(exact reviewed bytes)`), per INV-C12 (informed acceptance): what matters is what the reviewer actually saw. After acceptance, even a typo fix is a new revision — strict, but correct for an audit system, provided revisions are cheap.

2. **Decision layer — non-spatial node in the graph.** The graph carries the acceptance/rejection decision and lineage, not the plan text. `Claim`, `Intent`, `Agent`, `Witness` already follow the pattern "graph holds `NodeId` references; semantic payload lives in the owning module" (osp-core-design §2.3). The plan node joins this latent class of non-spatial decision citizens, with edges:

```text
Plan --PlansFor--> Task
PlanV3 --Supersedes--> PlanV2
Attempt --ExecutedUnder--> PlanV3
CompletionClaim --EvaluatedAgainst--> PlanV3
```

   This makes the full provenance query possible: "which intent, task, accepted plan, attempt, and completion review decision did this mainline commit derive from?"

3. **Enforcement layer — deterministic lowering into `PlanBinding`.** The machine-consumable parts (scope set → scope-digest derivation, verification obligations, permit inputs) lower deterministically from the artifact into a typed `PlanBinding`. This reuses Paper 3's central pattern: sentence → lowering → typed candidate; *"words do not mutate project reality."* Plan prose does not authorize execution — the accepted binding does. Two disciplines are non-negotiable: lowering reads only structured front-matter/sections, never free text (no lexical inference in load-bearing positions); and it is **fail-closed** — a missing or ambiguous field is a lowering error and the plan cannot be accepted (INV-C6 spirit: observable incompleteness over silent completion).

The core sentence of the approach:

> **The reviewer accepts the artifact; the engine enforces the binding; the graph carries the decision.**

## 4. What the Plan Is NOT

Four explicit "no"s:

- **Not in the physical/objective space.** A plan is not measured code reality. Plan files stay outside analyzer scope; a plan has no coupling, no cohesion, no instability, no entropy, and does not enter Q5 vision-deviation as if it were architecture.
- **Not a position family.** Inventing `PlanVector` is tempting (ConceptualIntent already has `risk` and `confidence` axes), but D8 rejected exactly this move: SemanticEmbedding could not become a fourth family because *"it is an index/retrieval structure, not an ontological position."* Same logic: inv #4 says "the engine measures," and today no engine measures plans. Unmeasured coordinates are decorative and anti-OSP.
- **Not in the trajectory loop or the agent's view.** Plan prose never reaches the executor (INV-T1). The plan's executor-facing shadow compiles into the already-existing, already leak-scanned channel: enriched `allowed_operations`/`constraints` plus the verification obligations G꜀ will consume. No new channel is opened.
- **Not a parallel acceptance subsystem.** The binding chain's entire claim is navigability within a single provenance structure. Splitting `Intent → Concept → Task → Plan → Attempt → Completion → Commit` across two ledgers would turn "what is the full derivation of this mainline commit?" into a cross-system join and puncture the chain thesis.

The correct order, and the wrong order:

```text
record events → define metrics → measure repeatedly → build a space if needed
invent axes first → guess values            ← wrong
```

If the plan one day earns coordinates, its axes will be defined by measurements produced by the event ledger: verification density, historical rework rate, plan conformance rate, scope-drift resilience, review disagreement rate, execution attempt efficiency. Coordinates are earned by measurement, not by decree.

## 5. Identity: Three-Digest Scheme

Plan identity requires three distinct digests, bound atomically to the acceptance record:

```rust
// distilled sketch — not compiled
pub struct AcceptedPlanIdentity {
    pub artifact_digest: ArtifactDigest,   // the exact bytes the reviewer accepted
    pub lowering_version: LoweringVersion, // which deterministic rules produced the binding
    pub binding_digest: BindingDigest,     // the typed structure the engine actually enforces
}
```

```text
artifact (bytes, repo)   → artifact_digest   (what the reviewer accepted — INV-C12 anchor)
   │  deterministic lowering, versioned
   ▼
PlanBinding (typed)      → binding_digest    (what the engine enforces)
   │
   ▼
Plan node (graph)        → DecisionStatus + edges  (the decision the chain carries)
```

The binding invariant:

```text
lower_vN(artifact_bytes) == persisted_plan_binding
```

This check is re-derivable at acceptance time, at execution-permit issuance, in CI (round-trip fixture class), and during replay. `lowering_version` is pinned: if lowering evolves, previously accepted plans are verified against their pinned version (same discipline as frozen characterization). `ExecutionPermit` carries both digests; the re-derivation invariant guarantees their consistency. This blocks two failure modes: lowering behavior changing while the artifact looks identical, and a different binding being handed to execution after the artifact was accepted.

## 6. Lifecycle & Gates

```text
Accepted Intent → Task Genesis → Plan Candidate → Plan Review
   ├─ Accepted Plan → Execution Permit → Work Attempt → Completion Claim → Result Review
   │     ├─ Complete → Mainline
   │     ├─ Rework → new attempt (same plan)
   │     ├─ Replan → new plan revision
   │     └─ Reject
   └─ Revision / Reject
```

Two gates:

- **Gₚ — Plan Acceptance Gate.** Authorizes that the task may be executed under this plan.
- **G꜀ — Completion Acceptance Gate.** Authorizes that the performed work is accepted into project reality.

The compact form of the model:

```text
AcceptedTask + AcceptedPlan + ExecutionCapability → WorkAttempt

WorkAttempt + MeasuredPredicateSatisfaction + PlanConformance
          + VerificationEvidence + CompletionWitness → CompletedTask
```

### Decision status

An accepted plan is never mutated; changes produce a new revision (`PlanRevision 1 — SupersededAccepted`, `PlanRevision 2 — Accepted`), matching Paper 3's acceptance/supersession approach.

```rust
// distilled sketch — not compiled
pub enum PlanDecisionStatus {
    Candidate,
    Accepted,
    Rejected,
    SupersededAccepted,
}
```

Design note from the conversation: `PlanDecisionStatus` is isomorphic to Paper 3's `DecisionStatus`, so it should **literally be the same enum** rather than a separate one — making the isomorphism literal lets the plan lifecycle inherit `OperatorReviewSession` / `SupersedeSession` / INV-C13 / INV-C15 / the TOCTOU machinery with zero copying. INV-C15's "same kind + position family" gate degenerates gracefully for a non-spatial kind. The one real piece of work is kind-scoped projection: plan nodes must not pollute concept-mainline queries (INV-C14 is status-based; a kind filter must be added).

Operational state is kept out of ontological status: `PlanDecisionStatus` (epistemic), `ReviewSessionStatus { Open, Claimed, Closed, Expired }` (operational), and `ExecutionStatus { NotStarted, Running, Submitted, ReworkRequested, Completed, Aborted }` remain separate enums. "A reviewer is currently examining the plan" is not the plan's epistemic status; the plan is still `Candidate` with an open review session.

### Plan acceptance produces a capability

```rust
// distilled sketch — not compiled
pub struct AcceptedPlanRef {
    task_id: TaskId,
    plan_revision_id: PlanRevisionId,
    plan_digest: PlanDigest,
    accepted_at: Timestamp,
}

pub struct ExecutionPermit {
    task_id: TaskId,
    accepted_plan: AcceptedPlanRef,
    baseline_digest: ProjectStateDigest,
    permission_mask: PermissionMask,
}

pub fn start_execution(
    task: &Task,
    permit: ExecutionPermit,
) -> Result<WorkAttempt, ExecutionError>;
```

### Stale plans and basis drift

A plan accepted against one baseline (`main commit`, `store schema version`, `public API surface`) may go stale after another merge. The permit therefore carries an `ExecutionBasis` (repository commit, store schema version, relevant API digest, task digest, plan digest). At execution start or claim time: `current basis == accepted execution basis?`. On mismatch:

```rust
// distilled sketch — not compiled
pub enum BasisDriftDecision {
    StillApplicable,
    RevalidationRequired,
    ReplanRequired,
}
```

This is Paper 3's `PresentedBasis` / stale-basis / TOCTOU check applied at the plan layer.

### Completion claims, not completion declarations

The executor never says "task completed." It produces a `CompletionClaim`; the completion decision belongs to the reviewer/gate — the same line as Paper 2's "the agent cannot declare coordinates or completion; the engine measures."

```rust
// distilled sketch — not compiled
pub struct CompletionClaim {
    pub task_id: TaskId,
    pub attempt_id: WorkAttemptId,
    pub accepted_plan: AcceptedPlanRef,
    pub baseline_digest: ProjectStateDigest,
    pub resulting_delta: ChangeSetDigest,
    pub predicate_results: Vec<PredicateResult>,
    pub verification_results: Vec<VerificationResult>,
    pub produced_evidence: Vec<EvidenceArtifact>,
    pub plan_conformance: PlanConformanceReport,
    pub deviations: Vec<PlanDeviation>,
    pub unresolved_items: Vec<UnresolvedItem>,
}
```

Post-task review checks three things separately: (A) task predicate satisfaction — including measurement-source sufficiency (`SourceInsufficient` blocks completion); (B) plan conformance; (C) engineering quality — "predicate satisfied" and "work accepted" are distinct (`build green` ≠ `task accepted completed`).

```rust
// distilled sketch — not compiled
pub enum PlanConformance {
    Exact,
    Equivalent,
    MinorDeviation,
    MaterialDeviation,
    OutsidePlan,
}
```

`Exact` is not required — a better solution may be found during execution — but every difference must be visible. The executor declares facts only; classification belongs to the reviewer/engine:

```rust
// distilled sketch — not compiled
pub struct DeclaredPlanDeviation { pub planned: String, pub actual: String, pub reason: String }
pub struct ReviewedPlanDeviation { pub declaration: DeclaredPlanDeviation, pub reviewer_impact: DeviationImpact }

pub enum DeviationDiscoverySource {
    ExecutorDeclared,
    ReviewerDiscovered,
    EngineDetected,
}
```

Tracking the three sources separately turns "how much can conformance self-reporting be trusted?" into an empirical question.

### Review decisions are not binary

```rust
// distilled sketch — not compiled
pub enum PlanReviewDecision {
    Accept,
    RequestRevision,
    SplitTask,   // the plan is not wrong; the task is too large
    Defer,
    Reject,
}

pub enum CompletionReviewDecision {
    AcceptCompleted,   // predicates satisfied; acceptable to mainline
    AcceptAsProgress,  // useful work; task not complete; stored as trajectory checkpoint
    RequestRework,     // plan still valid; implementation must be fixed; no new plan
    ReplanRequired,    // the accepted approach is now invalid or insufficient; revision bumps
    RejectAttempt,     // this attempt is unusable; task may stay open
    AbortTask,         // task should no longer be done, or the intent has lapsed
}
```

### Rework vs replan

- **Rework:** the plan is right, the implementation is incomplete or wrong → same plan, new attempt ("add a test for this error path").
- **Replan:** the plan itself is wrong or insufficient → old revision becomes `SupersededAccepted`, new revision goes through Gₚ, new permit is issued ("this metadata should not live on `ConceptNode`; it should be a store-owned binding").

Without this distinction, a team quietly changes the fundamental approach while behaving as if it were making small implementation fixes.

### Reviewer independence

Roles — Planner, Plan Reviewer, Executor, Completion Reviewer, Operator — are separated on the record even when the same human or model plays several. Stronger policies (`PlannerAgent != PlanReviewerAgent`, `ExecutorAgent != CompletionReviewerAgent`) and layered review for high-risk tasks (deterministic validation + independent model review + human acceptance; deterministic gates + independent code review + witness quorum) can be added later by policy. First stage: role and evidence separation only.

## 7. Normative vs Informative Content + Risk-Based Policy

A plan's binding and explanatory parts must be separated, or plan review degenerates into "implementation review done in advance" and erodes the agent's value:

```text
Normative:    scope, constraints, allowed operations, forbidden operations,
              verification obligations, risk controls
Informative:  rationale, possible implementation, examples, alternatives considered
```

The executor is bound by the normative part; the informative part guides but is not a mandatory route.

The protocol's value scales with decision density, not plan length. Not every task needs a large plan — a risk-based policy:

```text
Low risk:    no formal plan, or a short inline plan
Medium risk: versioned plan + single review
High risk:   plan review + independent reviewer + completion review + witness
```

A documentation typo fix and a persisted store schema + migration change must not go through the same process. **Open problem:** the risk thresholds themselves are undefined — the source names the tiers but gives no classification criteria.

What this structure does NOT solve: bad reviewers, plans written with missing information, badly defined task predicates, or unknowns that only surface during execution. Overly strict bindings can also shrink the solution space unnecessarily (see RQ-P6).

## 8. Invariants

Twelve invariants from the source. Enforcement layer per the OSP taxonomy (convention → runtime-asserted → type-enforced); the per-invariant layer assignment is an interpretive mapping by phase, not verbatim from the source.

| ID | Name | Statement | Enforcement (target) |
|---|---|---|---|
| PLAN-1 | No execution without accepted plan | `WorkAttempt` creation requires an `AcceptedPlanRef` | type (permit in `start_execution` signature) |
| PLAN-2 | Accepted plans are immutable | An accepted plan is never mutated; a new revision is created | runtime → convention (cheap revisions) |
| PLAN-3 | Plan decision requires a record | Accept/Reject/Revision cannot be applied without a `DecisionRecord` | runtime |
| PLAN-4 | Review basis must be fresh | The digest of the reviewed plan must equal the digest at decision time | runtime (TOCTOU check) |
| EXEC-1 | Execution is bound to exact plan revision | Every `WorkAttempt` is bound to exactly one accepted plan revision | type |
| EXEC-2 | Material deviation requires replan | A `CompletionClaim` carrying `MaterialDeviation` cannot complete directly | runtime gate (G꜀) |
| EXEC-3 | Executor cannot declare completion | The executor produces a `CompletionClaim`; it cannot produce `Completed` status | type |
| REVIEW-1 | Plan acceptance is not completion acceptance | An accepted plan does not mean the work is accepted | convention / design |
| REVIEW-2 | Completion requires measured evidence | No completion decision without predicate results and verification evidence | runtime gate |
| REVIEW-3 | Rework and replan are distinct | Implementation flaw → new attempt under same plan; plan flaw → new plan revision | convention → runtime (ledger classification) |
| REVIEW-4 | Mainline only after completion review | A `WorkAttempt` or `AcceptAsProgress` cannot reach mainline directly | runtime (witness integration) |
| REVIEW-5 | No decision without reviewer identity | Every decision carries operator/reviewer attribution | runtime (record schema) |

## 9. Event Ledger

The ledger is designed not only as an audit trail but as an experiment dataset: immutable JSONL / event store, evidence-pack compatible. Every phase (MVP included) writes to the same schema, frozen at Phase 0, so measurement data flows continuously across phases. The `role` field is filled even when the same human plays multiple roles, so independent-reviewer policy can later be analyzed retroactively.

```json
{
  "schema_version": 1,
  "event_type": "completion_review_decided",
  "event_id": "...",
  "task_id": "...",
  "plan_id": "...",
  "plan_revision": 3,
  "artifact_digest": "...",
  "binding_digest": "...",
  "attempt_id": "...",
  "actor_id": "...",
  "actor_role": "completion_reviewer",
  "decision": "request_rework",
  "timestamp": "...",
  "evidence_refs": ["..."]
}
```

Event types (16 listed in the source, grouped):

- Task/plan intake: `task_created`, `plan_artifact_submitted`
- Gₚ: `plan_review_started`, `plan_review_decided`, `plan_revision_superseded`
- Execution: `execution_permit_issued`, `attempt_started`, `attempt_submitted`, `basis_drift_detected`
- Deviations & measurement: `deviation_declared`, `deviation_discovered`, `predicate_measured`
- G꜀: `completion_review_decided`, `rework_requested`, `replan_required`, `task_completed`

Cost/latency timestamps to capture: `plan_authoring_started_at`, `plan_submitted_at`, `plan_accepted_at`, `execution_started_at`, `attempt_submitted_at`, `completion_accepted_at`. Derivable metrics: plan review latency, execution latency, attempt count, rework count, replan count, review disagreement, scope drift frequency, verification obligation density, unreported deviation rate, completion lead time.

## 10. Reuse of Paper 1–3 Machinery

- **Paper 3 acceptance machinery, zero-copy:** making `PlanDecisionStatus` literally Paper 3's `DecisionStatus` inherits `OperatorReviewSession`, `SupersedeSession`, INV-C13, INV-C15, and the TOCTOU/stale-basis machinery. INV-C15's "same kind + position family" gate degenerates for non-spatial kinds; consolidation semantics (a successor merging multiple plans) comes for free. Kind-scoped projection must be added so plan nodes do not pollute concept-mainline queries (INV-C14).
- **INV-C12 (informed acceptance):** anchors `artifact_digest` to the exact reviewed bytes.
- **INV-C6 spirit:** fail-closed lowering — observable incompleteness over silent completion.
- **INV-T1:** plan prose never reaches the executor; only the compiled binding shadow travels the existing leak-scanned channel.
- **"Words do not mutate project reality":** the plan's text does not change project reality; the accepted, digest-bound `PlanBinding` limits execution authority.
- **Paper 2's "engine measures" (inv #4):** no unmeasured coordinates; the executor claims, the gate decides.
- **D8 precedent (SemanticEmbedding rejected as a position family):** the same argument rejects `PlanVector` today.
- **Frozen-characterization discipline:** pinned `lowering_version` for verifying old accepted plans.
- **Paper 1:** witnessed space commit remains the terminal step; REVIEW-4 routes mainline entry through completion review.

## 11. Research Questions

- **RQ-P1 (rework hypothesis):** correlation between plan richness (number of verification obligations) and attempt count / rework rate. Makes "tests must be defined at plan time" measurable for the first time.
- **RQ-P2 (rework/replan distinguishability):** decision distribution plus which classifications are contested (reviewer notes are data too).
- **RQ-P3 (drift granularity):** scope-digest drift frequency vs commit-digest drift frequency — log both. Commit digest changes on every unrelated merge, giving a free control group that quantifies what scope derivation buys.
- **RQ-P4 (undeclared deviation rate):** executor-declared vs reviewer-discovered deviations; turns the conformance self-grading objection into an empirical question.
- **RQ-P5 (cost):** plan authoring + review time vs rework cost avoided; the answer — or falsification — of the "process overhead" objection.
- **RQ-P6 (restrictiveness vs solution quality):** do tighter bindings reduce rework while also blocking better or simpler alternatives? Signals: allowed-operation count, constraint count, material-deviation count, reviewer-rated solution quality, implementation complexity, attempt count. Tests the plan-bound execution vs free-navigation tension empirically; either outcome is valuable.
- **Meta-RQ:** can the Paper 3 machinery absorb a second decision-object kind **without modification**? Every required change is itself a finding — either proof of generality or a map of its boundary. This turns implementation work into evidence production.

## 12. Phased Plan

The MVP is not a throwaway prototype; it is the permanent content layer of the final architecture. The artifact layer (markdown + front-matter + digest, in repo) never changes — only enforcement strength migrates: convention-enforced → runtime-asserted → type-enforced.

```text
Phase 0 (now):     ledger schema + RQs frozen            — measurement design
Phase 1 (RW test): repo MVP — files + CI checks          — convention
Phase 2:           graph node + lowering + session (Rust) — runtime-asserted
Phase 3:           permit + ExecutorPlanView + G꜀        — type-enforced
Phase 4:           Paper
```

- **Phase 0 — semantics & event schema:** plan artifact schema, plan front-matter, decision vocabulary, rework-vs-replan semantics, event ledger schema, RQ set — all frozen before a single line of paper is written (writing-last).
- **Phase 1 — repository MVP:** markdown plan artifacts, front-matter, byte digest, CI validation, PR template, JSONL events. Dogfooded on the real project. CI checks: plan exists? status accepted? digest matches? PR carries task id? completion-evidence section present? deviation section non-empty? required test classes run?
- **Phase 2 — Rust runtime model:** concepts stable on real data become types: `PlanNode`, `PlanBinding`, `AcceptedPlanRef`, `ExecutionPermit`, `CompletionClaim`, `ReviewDecision`.
- **Phase 3 — enforcement:** no accepted plan → no attempt; basis-drift detection; binding verification; completion-review projection; mainline witness integration.
- **Phase 4 — Paper:** written on observations of a real system running its own protocol: rework rate, plan revision counts, review duration, deviation distribution, drift data, process overhead.

The single property to protect: at no phase do the artifact and ledger schemas change. Then every row of data collected doubles as Gₚ/G꜀ design validation, the Paper 4 evaluation section, and the empirical basis for the plan's future place in the space.

## 13. Open Questions

1. **Risk thresholds undefined.** The low/medium/high-risk policy tiers are named but the classification criteria are not.
2. **LLM executor interpretability of `PlanBinding`.** Is `allowed_operations: ["add_store_owned_binding"]` semantically rich enough for an autonomous code-generating agent to understand and apply directly, or does it require additional context? Identified in the source as one of the most interesting future research areas.
3. **Fail-closed lowering friction.** Plan authors must write the binding-relevant part (front-matter, structured sections) with schema discipline — a "programming" dimension added to plan writing. Mitigation suggested in the source: enrich lowering gradually (scope first, then verification, etc.).
4. **Graph naming undecided.** `ConceptGraph` generalized to `ProjectRealityGraph` (or `EpistemicGraph`) vs node-capability split (`Spatiality { Spatial(PositionFamily), NonSpatial }` / `GraphNode::spatial_position() -> Option<&Position>`). The source's preference is the combination: a `ProjectRealityGraph` with spatial nodes (Module, Concept, Feature) and non-spatial protocol nodes (Intent, Claim, Task, Plan, Attempt, ReviewDecision, Witness) — but no final decision was made.
5. **Social cost of byte digest.** Post-acceptance, even a typo fix is a new revision — technically correct, but requires the team to internalize that revisions are cheap, or it will be perceived as bureaucracy.

Honest cost list from the source: (a) non-spatial kind discipline — any code path assuming "every node has a family position" will be exposed (latent debt; the plan is only the forcing function); (b) kind-scoped projection work; (c) the byte-digest social cost above; (d) lowering conservatism is non-negotiable — nothing enters the binding that is not in the front-matter.

## 14. Relationship to Papers

```text
Paper 3:            How does a human sentence become an accepted task?
New plan layer:     How does an accepted task become an authorized execution space?
Paper 2:            How does the agent navigate toward the predicate inside that space?
Paper 1:            How does the resulting claim get witnessed into project reality?
```

The plan protocol fills the currently informal gap `Accepted Task → ? → Agent Execution`, which today is bridged by informal plans and human conversation. Placement in the chain:

```text
Concept Anchoring → Task Genesis → Planning Protocol (new)
  → Trajectory Navigation → Completion Review Protocol (new) → Witnessed Space Commit
```

Why this is Paper 4 candidate material: it is not a merely conceptual proposal. The phased plan yields a real system running its own protocol, with a frozen event-ledger schema producing continuous measurement data (RQs P1–P6 + Meta-RQ) from the first dogfooded task — and the Meta-RQ turns the implementation itself into a generality test of the Paper 3 machinery.

Naming: "Two-Phase Commit" was rejected (distributed-transaction connotation too strong, semantically inexact). Candidates considered: Dual-Gate Task Lifecycle, Plan–Execution–Witness Chain, Reviewed Task Lifecycle, Plan and Completion Protocol, Task Assurance Pipeline, Plan-Bound Execution, Double-Witnessed Work, Task Epistemic Lifecycle. **Chosen: Plan-Bound Task Lifecycle**, with gates Gₚ and G꜀.

The four principles that fix the model:

> **The task predicate determines when the task is complete.**
> **The plan binding determines the boundaries within which work may be done.**
> **The executor does not declare completion; it produces an evidenced completion claim.**
> **The reviewer does not manufacture the result; they accept or request correction within the space the measurement permits.**

---

## Appendix A — Repository-Level Conventions (shortened from source)

Plan artifact front-matter (minimal MVP shape):

```yaml
task_id: TASK-057
plan_revision: 3
status: accepted

scope:
  include: [osp-core/store, persisted schema]
  exclude: [analyzer, MCP protocol]

allowed_operations:
  - add_store_owned_binding
  - bump_schema_version
  - add_migration

forbidden_operations:
  - add_persistence_fields_to_ConceptNode
  - weaken_existing_invariants

verification:
  - old schema loads successfully
  - new schema round-trip passes
  - invalid binding rejects
  - compile-fail boundaries remain intact

risks:
  - schema compatibility
  - digest instability
  - partial migration
```

Commit / PR metadata convention:

```text
OSP-Task-Id:
OSP-Plan-Revision:
OSP-Plan-Digest:
OSP-Attempt-Id:
```

Standard PR-description sections (shortened): `## Accepted Plan` (Plan-Id, Plan-Revision, Plan-Digest, Baseline-Commit), `## Plan Conformance` (Exact / Deviations / Deferred), `## Predicate Results`, `## Verification Evidence` (unit / integration / compile-fail / migration tests, evidence artifacts), `## Known Residual Risks`. This makes "is the reviewed implementation really the implementation of the accepted plan?" a deterministic question.
