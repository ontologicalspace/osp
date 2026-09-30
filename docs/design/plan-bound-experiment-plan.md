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
| Rework-attempt burden | `Σ duration(rework attempts)`, where a rework attempt is an `attempt_started` whose `origin_event_id` (D8) references a `rework_requested` decision event, and `duration = attempt_submitted_at − attempt_started_at` | execution events + cost timestamps (§9) + D8 |
| Replan count | `#plan_revision_superseded` where successor is a *revision* (not rejection) per task | Gₚ events |
| Review disagreement | share of decisions whose reviewer note is marked contested (D3) | `plan_review_decided`, `completion_review_decided` + note structure (D3) |
| Scope drift frequency | share of `basis_checked` rows with `component == scope_digest, changed == true` (D1) | `basis_checked` component rows (D1) |
| Commit drift frequency | share of `basis_checked` rows with `component == commit, changed == true` (D1) | `basis_checked` component rows (D1) |
| Verification obligation density | obligations in binding ÷ scope items, per accepted plan | `plan_artifact_submitted` + binding |
| Undeclared deviation rate | `ReviewerDiscovered + EngineDetected` ÷ all deviations, per task / overall — **counted by unique `deviation_id` (D7), never by event count** | `deviation_declared` / `deviation_discovered` + D7 |
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
  fewer attempts and a lower rework count. (Count, not rate — no rate
  denominator is frozen; at per-task granularity a rate over tasks carries
  no information beyond the count.)
- **Variables:** predictor = plan richness; outcomes = attempt count,
  rework count; covariates = risk tier (D5), scope size, allowed-operation
  count — task difficulty confounds predictor and outcomes alike (harder
  tasks earn richer plans *and* more rework).
- **Analysis:** descriptive association across dogfooded plan-bound tasks —
  direction of effect, checked within covariate strata; no inferential
  statistics (§3 preamble).
- **Falsification signal:** no association, or positive association
  (richer plans ↔ more rework) stable across covariate strata.

### RQ-P2 — Rework/replan distinguishability

- **H2:** the rework-vs-replan boundary is stable: decisions are rarely
  contested and rarely revised after the fact.
- **Variables:** predictor = decision kind (`CompletionReviewDecision`) plus
  contested flag (D3); outcome = **reclassification** — a later decision
  whose `supersedes_decision_id` (D9) references an earlier decision of a
  different classification.
- **Analysis:** decision-kind distribution; contested share (D3);
  reclassification rate **measured via the D9 lineage relation only** —
  never inferred from textual contradiction between event bodies; time
  pressure checked via decision timestamps.
- **Falsification signal:** high contested share, or systematic
  reclassification of rework as replan under time pressure.

### RQ-P3 — Drift granularity

- **H3:** scope-digest drift fires substantially less often than commit-digest
  drift; the ratio quantifies what scope derivation buys over "revalidate on
  every merge".
- **Instrument (D1 — `basis_checked`, not a single drifted_component enum):**
  the control condition "commit changed, scope unchanged" requires observing
  *both* components at the same check. Each basis check therefore emits one
  `basis_checked` event carrying **a mandatory row per component**
  (`check_id`, `component`, `before`, `after`, `changed`) — no component may
  be silently absent. A `changed == true` row is the drift signal for that
  component.
- **Denominator:** drift frequencies are shares over `basis_checked` events
  (per-component rows share the `check_id`). Merge count is *not* the
  denominator; it is itself observable as commit-changed rows, which keeps
  "how many checks were triggered by unrelated merges" a separate,
  answerable question.
- **Variables:** per component, outcome = share of rows with
  `changed == true`; the joint distribution of (commit-changed,
  scope-changed) across checks is itself the object of study.
- **Analysis:** per-component changed-shares and their ratio; cross-tab of
  joint outcomes — the "commit changed, scope unchanged" cell is the
  free-control cell.
- **Falsification signal:** scope-changed share ≈ commit-changed share —
  scope derivation buys nothing.

### RQ-P4 — Undeclared deviation rate

- **H4:** executor self-reporting undercounts deviations
  (`undeclared rate > 0`).
- **Variables:** outcome = deviation composition by
  `DeviationDiscoverySource` (ExecutorDeclared / ReviewerDiscovered /
  EngineDetected), counted by unique `deviation_id` (D7); stratifier = task
  and task ordinal (learning effects).
- **Analysis:** undeclared deviation rate per task and overall; trend over
  task ordinals before any pooling.
- **Falsification signal:** undeclared rate ≈ 0 across tasks — conformance
  self-reporting is trustworthy (also a valuable finding).

### RQ-P5 — Cost (descriptive)

- **H5 (descriptive, no counterfactual):** how does plan overhead compare
  with the *observed* rework burden? The original "cost avoided" framing is
  **not measurable** in this design: observed rework cost is what happened
  *with* a plan; how much rework would have occurred plan-less is unknowable
  without a comparator. A counterfactual strategy (plan-less paired tasks /
  historical matched comparators) is explicitly out of scope for Paper 4 v1
  and recorded as future work.
- **Operationalization:** plan overhead = authoring + review latency
  (timestamps); rework burden = **`Σ duration(rework attempts)`** (metrics
  table, D8-linked) — summed actual attempt durations, not `count × mean
  latency`, so tasks with differently-sized attempts do not distort the
  proxy.
- **Variables:** predictor = plan overhead; outcome = observed rework-attempt
  burden.
- **Analysis:** per-task overhead-to-burden comparison; ratio pattern and
  its trend as task count grows.
- **Falsification signal:** overhead exceeds observed rework burden stably as
  task count grows — the "process overhead" objection wins for this protocol
  and the risk-based policy tiers must tighten. (Note: this falsifies the
  *descriptive* balance claim, not a causal avoidance claim, which this
  design never makes.)

### RQ-P6 — Restrictiveness vs solution quality

- **H6a:** tighter bindings (fewer allowed ops, more constraints) associate
  with less rework. **H6b (tension):** tighter bindings also associate with
  more material deviations and lower reviewer-rated solution quality —
  better/simpler alternatives blocked.
- **IV:** binding tightness (allowed-op count, constraint count).
  **DV:** material-deviation count, solution quality rating (D2, with its
  missingness contract), attempt count.
- **Analysis:** descriptive association of tightness with each outcome
  *separately* — H6a and H6b evaluated independently (both-outcome framing);
  every analysis reports rating coverage per the D2 contract and runs a
  complete-case sensitivity check against the all-decisions baseline.
- **Either outcome is valuable** (design §11); the RQ is written so that
  confirming H6a, confirming H6b, or both-weak is publishable.
- **Falsification signal:** no association in either direction.

### Meta-RQ — Machinery generality (construction experiment)

- **Question:** can Paper 3's acceptance machinery absorb a second
  decision-object kind (plan) **without modification**?
- **Method:** every deviation from "no modification" required during Phase
  2/3 implementation is logged as a structured finding: what changed, which
  invariant/session assumed single-kind, whether the fix generalized the
  machinery or special-cased the plan kind. **Baseline frozen first (D6):**
  findings are judged against a pinned baseline (commit + component set +
  modification definition + attribution rules), so later refactors cannot be
  post-hoc reclassified as "would have been needed anyway" or vice versa.
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
  design §9 plus the decisions below, JSON Schema-ized. **D1 replaces the
  drift event:** `basis_drift_detected` as a single-component event cannot
  express "commit changed while scope did not" — see D1.
- [ ] **D1 — `basis_checked` event (replaces single `drifted_component`
  enum):** one event per basis check with **mandatory per-component rows**
  (`check_id`, `component ∈ {commit, store_schema, api_digest, task_digest,
  plan_digest, scope_digest}`, `before`, `after`, `changed`). Cardinality:
  exactly one event per check; every component present in every event. Drift
  = a row with `changed == true`. (Required by RQ-P3; single-enum design was
  rejected in review round-1 — it loses the joint observation.)
- [ ] **D2 — `completion_review_decided.solution_quality_rating` +
  missingness contract + anchored rubric (required by RQ-P6):** integer
  1–5, **mandatory on decision kinds that evaluate the solution**
  (`AcceptCompleted`, `AcceptAsProgress`, `RequestRework`); on kinds that
  do not evaluate it (`RejectAttempt`, `AbortTask`, `ReplanRequired`), the
  event carries `rating_status: not_rated` + a `not_rated_reason` enum —
  absence is never silent. Every RQ-P6 analysis reports rating coverage
  and runs a complete-case sensitivity check; without this contract,
  selective missingness (MNAR) would bias the DV irrecoverably.
  **Scale semantics are part of the freeze** — draft rubric v1:
  1 = below acceptability (plan-conformance failure or introduced
  regressions); 2 = marginal (predicates met, notable quality concerns);
  3 = acceptable (meets plan and predicates, ordinary engineering quality);
  4 = strong (clearly exceeds plan expectations without material-deviation
  cost); 5 = superior (notably better or simpler than the accepted
  approach envisioned). Every rating event pins
  `rating_rubric_version: v1`; rubric text is a freeze artifact (F1).
  Unanchored 1–5 would make a 3↔3 comparison across time untrustworthy
  under rater learning/drift — unrecoverable after data collection.
- [ ] **D3 — Reviewer-note structure:** how a note marks a decision as
  *contested* and references evidence (required by RQ-P2).
- [ ] **D4 — Meta-RQ modification log schema** (finding: component, change,
  invariant touched, generalization-vs-special-case classification). Every
  finding **must reference the D6 baseline**.
- [ ] **D5 — `task_created.risk_tier`** (`low | medium | high`) so the
  risk-policy censoring is reconstructible (required by RQ-P1 covariates).
- [ ] **D6 — Meta-RQ baseline (freeze artifact, not just a field):**
  `baseline_commit` (the machinery state Phase 2 starts from), the machinery
  component/module set considered "Paper 3 machinery", the definition of
  *modification* (core machinery change), the boundary between extension /
  new client code and core change, and the attribution rule deciding whether
  a change was required by plan-kind support. Recorded and digest-pinned
  before the first Phase 2 commit; without it, "near-zero modifications"
  degrades into post-hoc classification.
- [ ] **D7 — Deviation identity & dedup:** every deviation carries a stable
  `deviation_id` and exactly one `discovery_source`. If a declared deviation
  is later independently re-caught (reviewer/engine), the discovery event
  **links to the same `deviation_id`** (lineage field) instead of creating a
  new deviation — event counts are never the RQ-P4 denominator, unique
  `deviation_id`s are.
- [ ] **D8 — Attempt origin relation (total, typed):** `attempt_started`
  carries `origin_event_id`, referencing the event that made this attempt
  possible. The permitted origin kinds are **exhaustive and frozen**:
  (a) execution-permit issuance — first attempt; (b) a `rework_requested`
  decision — rework attempt; (c) an `AcceptAsProgress` decision —
  progress-continuation attempt under the same accepted plan (the task
  stays open; design §6). An `attempt_started` whose origin is none of
  these kinds is a schema violation (fail-closed — no silent fourth
  origin). Rework-attempt burden (metrics table, RQ-P5) is derived from
  this typed reference alone; timestamp ordering is never used as causal
  evidence (ambiguous across multiple review/rework cycles).
- [ ] **D9 — Decision identity & lineage:** every `plan_review_decided` and
  `completion_review_decided` carries a stable `decision_id`; a later
  decision that revises or reclassifies an earlier one references it via
  `supersedes_decision_id`. RQ-P2 reclassification is measured only through
  this typed relation.
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
