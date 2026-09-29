# Plan-Bound Task Lifecycle — Experiment Plan (Draft)

> **Status:** Draft for Phase 0 freeze — derived from
> [`plan-bound-task-lifecycle.md`](plan-bound-task-lifecycle.md) §9 (event ledger),
> §11 (research questions), §12 (phased plan). Not frozen yet; the freeze checklist
> in §5 of this document defines what "frozen" means. No results are claimed
> anywhere in this document — data collection starts with the first dogfooded
> task in Phase 1.
> **Label:** Paper 4 candidate material · companion artifact:
> [`plan-bound-paper-outline.md`](plan-bound-paper-outline.md)

## 1. Purpose and design philosophy

The experiment is not bolted on after the system is built. The event ledger
(design §9) is the measurement instrument, its schema is frozen at Phase 0,
and every dogfooded task from Phase 1 onward produces data under that schema.
This is the project's **writing-last** discipline: the Paper 4 evaluation
section may only contain numbers derivable from ledger events.

Overall design: **single-project observational dogfood study** with frozen
schema, plus one construction experiment (the Meta-RQ). It is explicitly not
a controlled experiment — threats and confounds are recorded per RQ (§4) and
aggregated in §6.

Population: all tasks executed under the plan protocol on the OSP project
itself from Phase 1 onward. Risk-based policy (design §7) censors the sample:
low-risk tasks take no formal plan and therefore produce no Gₚ events. The
censoring is itself recorded (`task_created` carries a risk tier — freeze
decision D5) so the sampling frame is reconstructible.

## 2. Derived metrics (operationalization from ledger fields)

Every metric below is a pure function of event fields listed in design §9
plus the freeze-decision fields of §5. "Source" names the fields used; if a
field is marked D*, it exists only after the corresponding freeze decision.

| Metric | Derivation | Source fields |
|---|---|---|
| Plan review latency | `plan_accepted_at − plan_submitted_at` per plan revision | cost timestamps (§9) |
| Execution latency | `attempt_submitted_at − execution_started_at` per attempt | cost timestamps (§9) |
| Attempt count | `#attempt_started` per task | execution events |
| Rework count | `#rework_requested` per task | G꜀ events |
| Replan count | `#plan_revision_superseded` where successor is a *revision* (not rejection) per task | Gₚ events |
| Review disagreement | share of decisions whose reviewer note is marked contested (D3) | `plan_review_decided`, `completion_review_decided` + note structure (D3) |
| Scope drift frequency | `#basis_drift_detected` with `drifted_component == scope_digest` (D1) | execution events + D1 |
| Commit drift frequency | same event type with `drifted_component == commit` (D1) | execution events + D1 |
| Verification obligation density | obligations in binding ÷ scope items, per accepted plan | `plan_artifact_submitted` + binding |
| Undeclared deviation rate | `ReviewerDiscovered + EngineDetected` ÷ all deviations, per task / overall | `deviation_discovered` vs `deviation_declared` |
| Completion lead time | `task_completed − task_created` per task | intake + G꜀ events |
| Plan richness | verification obligation count per plan (primary); constraint count (secondary) | binding fields |

## 3. RQ operationalization

Small-n honesty applies to every RQ: with a single dogfooded project the
analysis plan is **descriptive statistics plus direction-of-effect**, with
covariates recorded for later pooling; no inferential claims (no p-values)
until n is defensible. Pooling across future projects is the stated route to
inferential work — out of scope for Paper 4 v1.

### RQ-P1 — Rework hypothesis

- **H1:** higher plan richness (verification obligations) associates with
  fewer attempts and lower rework rate.
- **IV:** plan richness. **DV:** attempt count, rework count.
- **Procedure:** correlate across dogfooded plan-bound tasks.
- **Confound — task difficulty** affects both IV and DV (harder tasks earn
  richer plans *and* more rework). Recorded covariates: risk tier (D5),
  scope size, allowed-operation count.
- **Falsification signal:** no association, or positive association
  (richer plans ↔ more rework) stable across covariate strata.

### RQ-P2 — Rework/replan distinguishability

- **H2:** the rework-vs-replan boundary is stable: decisions are rarely
  contested and rarely revised after the fact.
- **Observables:** distribution of `CompletionReviewDecision` values;
  contested-share (D3); revisions of decisions (later events contradicting
  earlier classification, detected in analysis).
- **Falsification signal:** high contested share, or systematic reclassification
  of rework as replan under time pressure (checked via timestamps).

### RQ-P3 — Drift granularity

- **H3:** scope-digest drift fires substantially less often than commit-digest
  drift; the ratio quantifies what scope derivation buys over "revalidate on
  every merge".
- **Design:** log both digest kinds on every `basis_drift_detected` (D1).
  Unrelated merges that change the commit digest but not the scope digest are
  the free control group (design §11).
- **Falsification signal:** scope drift ≈ commit drift frequency — scope
  derivation buys nothing.

### RQ-P4 — Undeclared deviation rate

- **H4:** executor self-reporting undercounts deviations
  (`undeclared rate > 0`).
- **Observables:** deviation counts by `DeviationDiscoverySource`
  (ExecutorDeclared / ReviewerDiscovered / EngineDetected).
- **Falsification signal:** undeclared rate ≈ 0 across tasks — conformance
  self-reporting is trustworthy (also a valuable finding).

### RQ-P5 — Cost

- **H5:** plan authoring + review cost is offset by rework cost avoided.
- **Operationalization (proxy, honestly limited):** plan cost = authoring +
  review latency (timestamps); avoided-cost proxy = rework events ×
  execution latency. **No counterfactual exists** (the same task was not also
  run plan-less), so this RQ reports the ratio pattern and its trend, not a
  causal claim.
- **Falsification signal:** overhead exceeds avoided-cost proxy stably as
  task count grows — the "process overhead" objection wins and the risk-based
  policy tiers must tighten.

### RQ-P6 — Restrictiveness vs solution quality

- **H6a:** tighter bindings (fewer allowed ops, more constraints) associate
  with less rework. **H6b (tension):** tighter bindings also associate with
  more material deviations and lower reviewer-rated solution quality —
  better/simpler alternatives blocked.
- **IV:** binding tightness (allowed-op count, constraint count).
  **DV:** material-deviation count, solution quality rating (D2), attempt
  count.
- **Either outcome is valuable** (design §11); the RQ is written so that
  confirming H6a, confirming H6b, or both-weak is publishable.
- **Falsification signal:** no association in either direction.

### Meta-RQ — Machinery generality (construction experiment)

- **Question:** can Paper 3's acceptance machinery absorb a second
  decision-object kind (plan) **without modification**?
- **Method:** every deviation from "no modification" required during Phase
  2/3 implementation is logged as a structured finding: what changed, which
  invariant/session assumed single-kind, whether the fix generalized the
  machinery or special-cased the plan kind.
- **Outcome framing:** near-zero modifications = generality evidence; a map
  of required changes = boundary characterization. Both are findings; neither
  is a failure. The log lives next to the ledger (same dogfood directory),
  schema frozen at Phase 0 (D4).

## 4. Threats to validity

- **Single project / single human:** roles separated on record (design §6)
  but often played by the same human in early phases; role fields make this
  analyzable, not absent.
- **Selection (risk-based censoring):** low-risk tasks produce no plan events;
  sampling frame reconstructed from `task_created` risk tiers (D5).
- **Learning effects:** protocol skill grows over tasks; task ordinal recorded
  implicitly via timestamps — trend analysis must precede pooling.
- **Small n:** descriptive + direction-of-effect only (§3 preamble).
- **Instrument reactivity:** the team knows the metrics exist; mitigation is
  none — recorded as an accepted limitation.
- **Schema evolution:** frozen at Phase 0; any later change is a versioned v2
  with migration notes (design §5 pinned-lowering discipline applies to the
  ledger too).

## 5. Phase 0 freeze checklist (decisions required before freeze)

Freeze means: the listed items get an explicit version tag
(`planbound-ledger-v1`, `planbound-rq-v1`, `planbound-frontmatter-v1`) and a
recorded digest, following the project's frozen-characterization discipline.
After the tag, changes require a new version tag plus migration note.

- [ ] **F1 — Event schema (`planbound-ledger-v1`):** the 16 event types of
  design §9 plus the decision fields below, JSON Schema-ized.
- [ ] **D1 — `basis_drift_detected.drifted_component`** enum:
  `commit | store_schema | api_digest | task_digest | plan_digest | scope_digest`
  (required by RQ-P3).
- [ ] **D2 — `completion_review_decided.solution_quality_rating`** (optional
  integer 1–5; required by RQ-P6).
- [ ] **D3 — Reviewer-note structure:** how a note marks a decision as
  *contested* and references evidence (required by RQ-P2).
- [ ] **D4 — Meta-RQ modification log schema** (finding: component, change,
  invariant touched, generalization-vs-special-case classification).
- [ ] **D5 — `task_created.risk_tier`** (`low | medium | high`) so the
  risk-policy censoring is reconstructible (required by RQ-P1 covariates).
- [ ] **F2 — RQ set (`planbound-rq-v1`):** RQ-P1..P6 + Meta-RQ as worded in
  §3 of this document (hypotheses may be refined; RQ identity does not
  change).
- [ ] **F3 — Front-matter schema (`planbound-frontmatter-v1`):** design
  Appendix A minimal shape, field types made exact.
- [ ] **F4 — Decision vocabularies:** `PlanReviewDecision`,
  `CompletionReviewDecision`, `PlanConformance`, `DeviationDiscoverySource`,
  `PlanDecisionStatus` (reused Paper 3 enum — design §6).

## 6. Data handling

Events are immutable JSONL in the repo (dogfood directory), one file per
task, evidence-pack compatible; derivations run by a checked-in script whose
output is the only permitted source of Paper 4 evaluation numbers. Numbers in
prose that cannot be traced to script output are a claim-discipline violation
and must not survive review.
