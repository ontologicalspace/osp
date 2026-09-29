# Paper 4 (candidate) — Manuscript Outline

> **Status:** Outline skeleton — no manuscript exists yet. Derived from
> [`plan-bound-task-lifecycle.md`](plan-bound-task-lifecycle.md); evaluation
> content will be written only from ledger data per the
> [experiment plan](plan-bound-experiment-plan.md) (writing-last).
> **Working title:** *Plan-Bound Task Lifecycle: Two Protocol Gates for
> Authorized Agent Execution*.
> Alternatives considered: Dual-Gate Task Lifecycle; Plan-Bound Execution;
> Task Epistemic Lifecycle (full list in design §14).

## Abstract skeleton

1. **Gap:** agent-execution governance protocols decide *whether* an agent may
   act and *whether* its result enters the project — today the middle stretch
   (accepted task → authorized execution → accepted result) is bridged by
   informal plans and conversation (design §14).
2. **Model:** two distinct protocol gates — Gₚ (plan acceptance: authorization
   claim) and G꜀ (completion acceptance: evidence claim); the plan as a
   *reviewed maneuver envelope*, not a navigation route (design §2).
3. **Architecture:** three-layer placement (repo artifact / graph decision /
   deterministic binding), three-digest identity with a re-derivable binding
   invariant (design §3, §5).
4. **System:** phased implementation on a real project running its own
   protocol, with a frozen event-ledger schema collecting measurement data
   from the first dogfooded task (design §12).
5. **Evaluation:** RQ-P1..P6 + Meta-RQ (one sentence each; results TBD —
   placeholder only, no numbers).

## Section outline

### 1. Introduction
- The informal gap `Accepted Task → ? → Agent Execution → ? → Accepted Result`.
- Two reviews are two protocol gates evaluating different objects (Plan Claim
  vs Completion Claim — design §1).
- Contribution list: the model, the identity scheme, the invariants, the
  dogfooded system + frozen-ledger measurement design.

### 2. Background & Related Work
- OSP chain Papers 1–3 in one paragraph each (space/witness; predicate
  navigation; concept anchoring) — placement diagram of design §14.
- Related strands to scan (scan not yet done — flagged): plan review in
  software process compliance; agent governance / permission models;
  audit-log research; plan-recognition. *Scan is a pre-writing task; outline
  slot reserved here so it is not skipped at writing time.*

### 3. The Model
- Plan = reviewed maneuver envelope; the one-line treaty with predicate
  navigation (design §2): *task predicate defines completion; plan binding
  defines permitted execution.*
- Three-layer placement: content artifact (byte digest, INV-C12) / graph
  decision node (non-spatial, lineage edges) / enforcement binding
  (deterministic fail-closed lowering) (design §3).
- What the plan is NOT — four "no"s incl. the D8-precedent rejection of
  `PlanVector` (design §4).

### 4. Identity & Lifecycle
- Three-digest scheme; `lower_vN(artifact) == persisted_binding` invariant
  and its four re-derivation points (design §5).
- Lifecycle state machine incl. rework vs replan (design §6); decision
  vocabularies (non-binary review decisions); stale-basis drift handling.
- Completion *claims*, never completion declarations; deviation sources
  (executor-declared / reviewer-discovered / engine-detected).

### 5. Invariants
- The twelve invariants table with enforcement-layer targets (design §8);
  risk-based policy and its undefined thresholds stated as open (design §7,
  §13.1).

### 6. Implementation (phased; Meta-RQ as evidence)
- Phase 0–4 plan; artifact+ledger schemas never change across phases
  (design §12).
- Meta-RQ: modifications required to absorb a second decision-object kind
  into Paper 3's machinery, as a generality finding (design §11).

### 7. Evaluation
- Method: single-project observational dogfood, frozen schema, writing-last
  (experiment plan §1).
- One subsection per RQ (P1–P6): hypothesis, observables, result table from
  the derivation script — **all result slots empty until ledger data exists**.
- Threats subsection (experiment plan §4).

### 8. Discussion
- Risk-based policy and social cost of byte digests (design §13);
  restrictiveness vs solution quality (RQ-P6 both-outcomes framing);
  earned-coordinates path for plans via ledger metrics (design §4).

### 9. Conclusion
- The four fixing principles (design §14) as closing statements.

## RQ → evaluation mapping

| RQ | Eval § | Primary observables | Ledger metrics (experiment plan §2) |
|---|---|---|---|
| RQ-P1 rework hypothesis | 7.1 | richness vs attempts/rework | plan richness, attempt/rework count |
| RQ-P2 boundary stability | 7.2 | decision distribution, contested share | review disagreement |
| RQ-P3 drift granularity | 7.3 | scope vs commit drift frequency | drift frequencies (D1) |
| RQ-P4 undeclared deviations | 7.4 | deviation source split | undeclared deviation rate |
| RQ-P5 cost | 7.5 | plan cost vs avoided-rework proxy | review/execution latency |
| RQ-P6 restrictiveness | 7.6 | tightness vs deviations/quality | binding tightness, rating (D2) |
| Meta-RQ | 6 | modification log | D4 findings |

## Expected figures & tables

- F1 lifecycle state machine (design §6 diagram, redrawn).
- F2 three-layer placement (artifact / graph / binding).
- F3 three-digest identity + binding invariant flow.
- F4 event ledger schema overview (16 event types grouped by phase).
- T1 invariants × enforcement layer (design §8).
- T2–T7 per-RQ result tables (empty until data; structure fixed now).
- F5+ evaluation plots (decided when data shape is known).

## Writing rules for this paper

1. Writing-last: §7 contains only script-derivable numbers (experiment
   plan §6).
2. Design-doc claims carry section references; every empirical sentence
   traces to a ledger event class.
3. Negative results are publishable here by construction (RQ-P4/P6/H6
   framings) — do not soften them at writing time.
