# Concept Anchoring: From Human Sentences to Bound Project Work

**OSP Paper 3 Draft v1.0 (skeleton)** · Target: arXiv then ACM TOSEM
**Authors:** Volkan ER
**Date:** 2026-07-05
**Companion papers:** *Ontological Space Protocol: Modeling Software as a Conceptual Space with Epistemological Witnessing* (Paper 1, v2.6 — static space); *Architectural Trajectory Navigation: From Target Coordinates to Measurement Predicates* (Paper 2, v1.2 — dynamic, agent-driven). This paper establishes the **genesis layer** that precedes both: how a human sentence becomes bound, accepted, measured project work.
**Revision:** skeleton (Stage 2). Evidence frozen at commit `481690d` (hash₁); metadata at `8431c9e` (hash₂). Stages 3a–3c fill the heart sections (Abstract, §2, §5); Stages 3d+ fill the remaining sections.

> **Title note (deliberate asymmetry with Paper 2).** Paper 2 is titled *From Target Coordinates to Measurement Predicates*; this paper is *From Human Sentences to Bound Project Work*. The asymmetry is intentional: Paper 2's title carries a formal contrast (coordinate vs. predicate), while Paper 3's title carries an ontological one (sentence vs. work). The companion framing — *genesis layer* — is stated in the front matter rather than the title.

---

> **Methodological note (review insight).** *An invariant that is not under test will be violated.* This paper does not merely present the ontological binding chain; the evidence artifacts themselves are structurally verifiable (the pre-flight canonical table, §0 / Appendix A).

## Abstract

A human sentence may introduce project intent, but it does not by itself create project work. Contemporary embedding-first anchoring — retrieval-augmented generation, GraphRAG, and their kin — can propose *candidate proximity* between a sentence and a concept, but it cannot decide the sentence's ontological role, the authority behind it, whether it has been accepted, or whether it is executable. Proximity is a property of vectors; commitment is a property of protocol acts, and the two cannot substitute for one another.

We present **Concept Anchoring**, the genesis layer of the Ontological Space Protocol (OSP), which turns a human sentence into bound, accepted, measured project work only through a type-enforced chain: candidate isolation, operator acceptance, predicate lowering, cross-family translation, operator binding, and capability-gated task genesis. Each transition is a gate with a named invariant (INV-C1..C8 for anchoring, INV-P1..P3 for lowering and translation), and each gate is enforced at the type boundary, the constructor boundary, or the regression-test boundary — not as a judgment call. A rule lowers to a `PredicateStub` (structured uncertainty), never to an executable predicate; a translator proposes candidate axis meaning, but only operator binding creates commitment; a task is born only when an accepted candidate, a bound predicate set, and an operator capability meet.

We verify the chain rather than benchmark a semantic extractor. Across 11 Paper-3-specific type-level invariants (18 cumulative with Papers 1–2), 13 golden fixtures, and 5 held-out adversarial sentences spanning a known semantic false-positive, a negation case, a multi-axis case, and a bilingual (English/Turkish) alias chain, we report a five-state conformance (12 conform, 2 partial, 2 known limitation, 2 reject-as-expected, 0 unexpected failure). An end-to-end binding-chain replay exercises the real Faz-1 pipeline from sentence to navigator-ready task, and four rejected-path replays prove the gates refuse invalid input. A gate that only passes is indistinguishable from no gate.

This is a verification paper, not a semantic-extraction benchmark. The deterministic keyword matcher that drives translation is a placeholder for Faz 6 concept synthesis, and it admits a known lexical false-positive (a software *coupling* vs. a pipe *coupling*). The matcher is not the contribution; the binding protocol is.

## 1. Introduction

[TODO: Paper 1/2'den ayrım — "static space" ve "dynamic navigation" öncesi "genesis layer".]

### 1.1 The Problem

Embedding-first anchoring (RAG, GraphRAG) proposes candidate proximity but cannot decide ontological role, authority, acceptance, or executability. [D2-1: "anti-RAG" → "embedding-first anchoring eleştirisi".]

### 1.2 Our Approach: Ontological Binding Chain

A sentence becomes project work only by traversing a type-enforced binding chain: candidate isolation → operator acceptance → predicate lowering → operator binding → task genesis. Each gate is type-level enforced (INV-C1..C8, INV-P1..P3).

### 1.3 Contributions

This paper makes four contributions:

1. **Genesis ontology with candidate isolation (Section 3).** A human sentence produces a *candidate* — never accepted, never executable. INV-C1..C8 type-level enforcement. *"Embedding proposes; it does not decide."*
2. **Predicate lowering — a rule is not a predicate (Section 4).** A `RuleCandidate` lowers to a `PredicateStub` (structured uncertainty), never an `ExecutablePredicateSet`. INV-P1. *"The keyword matcher is not the contribution. The contribution is the binding protocol."*
3. **Cross-family translation semantics (Section 5).** Concept→Physical translation preserves candidate meaning (INV-P3); binding alone creates commitment. [D2-3: §4 kısa, §5 asıl epistemik argüman.]
4. **Three-gate task genesis (Section 6).** Accepted intent + operator-bound predicate + operator capability → `trajectory::Task`. INV-P2 (keyword ≠ executable), INV-T2 (capability-gated genesis).

### 1.4 Terminology mapping [D1-5]

| Term (this paper) | OSP core type | Paper 1/2 connection |
|---|---|---|
| Candidate | `ConceptNode` (DecisionStatus::Candidate) | pre-Trajectory |
| Accepted | `ConceptNode` (DecisionStatus::Accepted) | pre-Task intent |
| Task | `trajectory::Task` | Paper 2 navigator input |

## 2. Motivating Example

> *"The sentence never becomes a task by itself."*

To make the protocol concrete, we walk a single human sentence through the entire binding chain, from text to a navigator-ready `trajectory::Task`. Every step below is reproduced by a frozen evidence artifact; the prose follows the artifact's step numbering exactly. The sentence is deliberately ordinary — an architectural rule a senior engineer might utter in a design review:

> *"Coupling must not exceed module threshold."*

Nothing in this sentence is executable. It carries no metric identifier, no threshold, no scope, no operator signature. The chain that turns it into work has eight steps, and each step is a gate.

**Step 1 — Pipeline run (real, not seeded).** The sentence enters the deterministic Faz 1 pipeline as a `ConceptPacket`. A rule-based classifier detects the *"must not"* marker and sets a rule signal; the extractor emits a `DerivesRule` candidate whose target is a freshly named `RuleCandidate:CouplingMustNot` (the name is derived deterministically from the first three words of the sentence). The pipeline applies its plan to the concept graph, and the gate writes the new node with `DecisionStatus::Candidate` (INV-C5). At this point the sentence has produced a *candidate rule* — nothing more. It is not yet a predicate, not yet accepted, not yet a task.

**Step 2 — Candidate isolation (INV-C3).** The `RuleCandidate` now lives in the concept graph, but in the *candidate lane*, segregated from mainline knowledge. No query that reads the project's accepted state can see it. The sentence has a name and a place, but no authority.

**Step 3 — Predicate lowering (INV-P1).** The candidate rule is lowered into a `PredicateStub` — a structured-uncertainty value that records, in type, *what is missing*: four unresolved slots (metric, threshold, scope, comparator), a suggested template (`MetricThreshold`), and zero completeness. The lowering never produces an `ExecutablePredicateSet`; a rule and a predicate are different ontological categories, and lowering names the gap between them rather than papering over it.

**Step 4 — Cross-family translation (INV-P3).** The stub carries a `CrossFamilyHint` that translates the conceptual intent into the physical-code family. Scanning the candidate's canonical form, the translator finds the *coupling* keyword and proposes a single axis candidate (`SingleCandidate(Coupling)`) with `KeywordMatch` source. This is the only step where the sentence's wording influences the physical interpretation, and even here the output is a *proposal of candidate meaning*, not a commitment. The full argument for why this proposal cannot silently harden into certainty occupies Section 5.

**Step 5 — Operator binding (INV-P2).** An operator — holding `OperatorCapability` — binds the stub's unresolved slots: axis `Coupling`, scope `Node(1)`, comparator `Le`, threshold `0.55`. The two-token boundary (the capability token plus the function boundary of `bind_metric_threshold`) is the only route from a `PredicateStub` to an `ExecutablePredicateSet`. The keyword hint suggested the axis; the operator decided it.

**Step 6 — Accepted verification (INV-C3, seeded in this replay).** The chain now needs an *accepted* intent to bind the predicate to. Candidate-to-Accepted promotion requires an unforgeable `OperatorAcceptance` token that, by design, integration tests cannot mint; the in-crate unit test `store_promotion_requires_operator_acceptance` exercises the real promotion path, while this replay seeds the post-promotion state. We mark this honestly: the acceptance *gate* is enforced and tested, but its *traversal* in this particular replay is simulated. The full operator console that performs real promotion is Faz 8 future work.

**Step 7 — Task genesis (INV-T2).** With an accepted reference and a bound `ExecutablePredicateSet`, a third capability-gated call — `create_task_from_accepted_candidate` — finally produces a `trajectory::Task`: a deterministic ID derived from the candidate, a `Pending` status, the bound predicate set, and an operator-supplied `allowed_operations` list (`RemoveImport`). The sentence has, at last, become executable work — but work that a Paper 2 navigator can run, not a free-floating instruction.

**Step 8 — Registry.** The task enters the in-memory registry and resolves as navigator-compatible. From here, Paper 2's `AgentNavigator.run_task` can execute it under measurement predicates, maneuver limits, and witness policies. The genesis layer's job is done.

At no point did the sentence become a task by proximity; only type-enforced gates moved it forward — and one of them (acceptance) is exercised in-crate rather than in this replay. The sentence's wording influenced exactly one step (Step 4, translation), and even there it produced a candidate, not a commitment. Every other transition was a protocol act requiring a capability token the sentence could not supply.

> **Evidence.** The full eight-step chain — including the real Step 1 pipeline trace, the cross-checked canonical, the deterministic task ID, and the registry resolution — is reproduced in `e2e-binding-chain-replay.json` (Appendix B; integrity recorded in `run-metadata.json`). The chain's *rejected* counterpaths are reproduced in `e2e-rejected-paths-replay.json` (Section 7.5).

## 3. Genesis Ontology (INV-C1..C8)

4-column table: invariant → type-level enforcement → what it prevents → evidence. [TODO: INV-C1 embedding-proposes, C2 family, C3 candidate-isolation, C4 supersede-authority, C5 inferred-not-accepted, C6 code-intent-hypothesis, C7 explainable, C8 canonicalized.]

## 4. Predicate Lowering (INV-P1)

*"A rule is not a predicate. A predicate is a rule whose measurable slots have been bound."*

RuleCandidate → PredicateStub (structured uncertainty: unresolved slots + suggested templates + cross-family hint). Never ExecutablePredicateSet. [D2-3: this section is kept short; the full epistemic argument lives in §5.]

## 5. Cross-Family Translation

> *"Translation preserves candidate meaning; binding alone creates commitment."*

The chain in Section 2 turns on a single epistemological hinge: Step 4, where a conceptual rule (*"coupling must not exceed..."*) is translated into a physical-code axis candidate (`Coupling`). This is the only step where the sentence's wording influences the physical interpretation of the eventual task. It is also the step where the most tempting shortcut lives — and the step where the protocol's discipline matters most. This section makes the argument explicit.

### 5.1 The temptation the invariant defeats

Consider what a less disciplined system would do at Step 4. The translator has found one axis candidate with high confidence; the operator will almost certainly bind it. The temptation is to *collapse the translation into a decision*: take the highest-confidence candidate, declare it the axis, and hand the operator a pre-bound predicate. This collapses two ontological categories — *candidate meaning* (what the sentence suggests) and *executable commitment* (what the project will measure) — into one, in the name of convenience.

In OSP, this shortcut is not a philosophical disagreement; it is a **protocol-level rejection** — outside the protocol boundary, not inside it as a judgment call. The translator and the binder are separated by a function boundary and an `OperatorCapability` token; there is no path, internal to the lowering, that turns a `CrossFamilyHint` into an `ExecutablePredicateSet`. The operator, and only the operator, crosses that boundary. INV-P3 exists to keep that boundary load-bearing.

### 5.2 Ambiguity as a computed value

The translator's output is a `CrossFamilyHint` carrying zero or more axis candidates, and an *ambiguity* level derived from the candidate count. INV-P3 is not a design aspiration we argue for; it is a property we *compute*. The `ambiguity()` accessor is a pure match on `axis_candidates.len()`, so the stored representation and the derived ambiguity cannot fall out of sync — there is no field to drift.

The three ambiguity states are ontological, not merely technical:

- **`SingleCandidate`** (one candidate axis). Translation narrowed the field to one, but did not bind it. The commitment ball is in the operator's court; a mismatch at binding is a strict `AxisMismatch` reject. We renamed this state from `Certain` during review precisely to shed the ontological-certainty connotation — *single* is a count, not a guarantee.
- **`MultipleCandidates`** (two or more axes). Translation genuinely could not disambiguate, and the protocol refuses to silently pick one. The ambiguity is preserved as a first-class value and bounded to the candidate set: the operator may choose one of the proposed axes, but choosing outside the set is an `AxisNotInCandidates` reject.
- **`NoAxisCandidate`** (zero axes). Conceptually, this denotes that translation ran and proposed no physical axis. In the current lowering representation, this *is represented as* the absence of a `CrossFamilyHint` (asserted in held_005), not a stored empty hint; the invariant is the same: no executable commitment is created. This is deliberately distinct from `NoTemplateMatch`, where no template was suggested at all — a rule containing *"azalt"* (reduce) suggests the `MetricDelta` template while still proposing no axis.

### 5.3 Two hint sources — identity and translation

Axis candidates carry a *source* that records how the candidate was reached, and the protocol recognizes two:

- **`KeywordMatch`** (default confidence 1.0). The rule's canonical form contains the axis's own English name as a substring — *coupling*, *cohesion*, *instability*, *entropy*, the witness-depth family. `KeywordMatch` is substring-level identity — which is precisely why held_002 can match the wrong domain: a sentence about couplings in a pipe assembly produces a `Coupling` hint that is lexically correct and semantically wrong (Section 10). The substring nature is the mechanism; the false positive is its honest consequence.
- **`LanguageAlias`** (default confidence 0.9). The rule's canonical form contains a folded Turkish equivalent — *bağımlılık* → *bagiml* → `Coupling`. This is *translation* rather than identity: a weaker, but still deterministic, evidence of intent.

These confidence values are conventional ordering constants for tie-breaking during hint merge, not calibrated estimates. Confidence is never aggregated as a pseudo-probability; the merge rule takes the winning hint whole (all four fields — axis, confidence, source, reason — from one candidate), never blending a hybrid.

### 5.4 The normalize pipeline — a controlled shared space

For KeywordMatch and LanguageAlias to land in the same comparison space, the protocol normalizes both the rule canonical and the stored patterns through a fixed pipeline: NFC composition, then a Turkish-character fold (`I/İ/ı → i`, `Ğ/ğ → g`, …), then ASCII lowercase — in that exact order. The order is load-bearing: fold must precede ASCII lowercase so that a Turkish *bağımlılık* and an English *coupling* reach the same ASCII comparison space.

The protocol sacrifices locale-correct lowercasing to create a controlled matching space shared by the current English/Turkish fixture set — deterministic, not provably complete. A well-intentioned future "fix" that made the lowercasing Unicode-aware would silently shift that space and break every Turkish alias match; the ordering constraint exists precisely to make that shift a visible regression rather than a silent semantic drift. The held-out fixture held_001 pins the result: *"Modüller arası bağımlılık azaltılmalı."* lowers to a canonical containing *bagiml*, which the alias table matches to `Coupling`.

### 5.5 The membership rule — where binding creates commitment

Translation proposes; binding commits. The single binding rule makes this executable in three branches: if the candidate set is empty, the operator is free to bind any axis; if the set contains the operator's chosen axis, binding proceeds; otherwise, the binding is rejected — with a *precise* error type that names the actual violation: `AxisMismatch` when there was exactly one candidate and the operator chose another, `AxisNotInCandidates` when there were several and the operator chose outside the set. The error type itself carries epistemic information.

The translator may propose a bounded candidate set; only operator binding crosses the boundary into executable commitment.

### 5.6 Restraint as protocol boundary

INV-P3's restraints — no executable predicate from translation, no confidence aggregation, no ontological certainty attached to `SingleCandidate` — are not promises the protocol makes and keeps. Some of them are *structurally impossible to violate*: ambiguity is computed from the candidate count, never stored, so a hint cannot carry a stale or inconsistent ambiguity. The rest are *rejected at the protocol boundary*, enforced by the type boundary, the smart-constructor boundary, and the regression-test suite together. None of them is a promise; a promise can be broken by a determined caller, but a structural impossibility cannot be violated at all, and a protocol boundary can only be crossed by a caller who already holds the capability token.

The example in Section 2 is therefore not a lucky successful parse. It is a demonstration that even a successful parse remains non-executable until binding — and that the binding, in turn, is an act the sentence itself can never perform.

## 6. Binding & Task Genesis (INV-P2, INV-T2)

Three-gate API: (1) verify_accepted_task_candidate, (2) bind_metric_threshold (OperatorCapability), (3) create_task_from_accepted_candidate (OperatorCapability). *"Accepted intent ≠ executable work."*

## 7. Verification Evidence

*"A gate that only passes is indistinguishable from no gate. These paths prove the gates reject."* [D2-4: §7 = "Verification Evidence".]

### 7.1 Type-level trybuild (stratum 1)
11 Paper 3'e özgü (kümülatif 18 bağlam). `tests/anchoring_typelevel.rs`.

### 7.2 Golden fixture conformance (stratum 2)
13 fixture, 5-state: Conform 9, PartialConform 2, RejectAsExpected 2. `conformance-results.json`. The five-state classification is *test-referenced but analyst-assigned*: each fixture cites the test that reproduces its behavior, but the conformance state (Conform / PartialConform / KnownLimitation / RejectAsExpected / UnexpectedFailure) reflects an analyst's judgment about how closely the observed behavior matches the fixture's expected semantics, not a binary pass/fail from a single assertion.

### 7.3 Held-out adversarial (stratum 3)
5 fixture (4 held_out + 1 regression_anchored). Conform 3, KnownLimitation 2. `held-out-adversarial-fixtures.json`.

### 7.4 End-to-End Binding-Chain Replay (stratum 4)
`e2e-binding-chain-replay.json`. Adım 1 gerçek pipeline; Adım 6 INV-C3 seeded.

### 7.5 End-to-End Rejected Paths Replay (stratum 5)
`e2e-rejected-paths-replay.json`. 4 yol: AxisMismatch, AxisNotInCandidates, TemplateNotSuggested, NotAccepted.

### 7.6 What this does not evaluate
Self-authored gold standard, keyword matcher placeholder, stub provider, Faz 8 operator console (acceptance seeded). [TODO: §10'a da akar.]

## 8. Related Work

[TODO ~6: requirements traceability, CNL, GraphRAG, program analysis, AI agents, P1+P2.]

## 9. Discussion

### 9.1 Ontological binding vs embedding
*"Embedding can propose candidate proximity, but it cannot decide ontological role, authority, acceptance, or executability."*

### 9.2 Determinism-first discipline
Every mechanism in this paper is first proven deterministic in isolation (Faz 1 pipeline, in-memory graph, lexical classifier), with stochastic layers (embedding-assisted candidate generation, LLM-assisted synthesis) deferred to Faz 6/7. The binding chain's correctness does not depend on any model output; a deterministic stub provider reproduces every result in Section 7.

### 9.3 Storage is not epistemology [D2-5]
KuzuDB (Ekim 2025 arşiv) story kısa: persistence epistemic gate'i zayıflatmaz. *"Persistence does not weaken epistemic gates."*

### 9.4 Fixture design must be verifiable (review insight)
Across four review rounds of the evidence layer, every defect that was caught reduced to one of two failure classes: *constraint non-propagation* (a constraint discovered for one sentence was not applied to all — the canonical-truncation and marker-omission traps, caught three and one times respectively) and *claim-implementation divergence* (an artifact stated a property the test did not actually verify). The pre-flight canonical table (§0 / Appendix A) makes both classes structurally impossible by running the real pipeline and the real lowering on every evidence sentence, asserting the canonical, the rule signal, the ambiguity, and the axis candidates in one test. *An invariant that is not under test will be violated* — applied here to the paper's own evidence files, not only to its protocol claims.

## 10. Threats to Validity

- **Self-authored gold standard.** The 13 golden fixtures were authored by the paper's author (D1-2). The held-out set (5) provides non-tautological evidence — its sentences were not used during development of the lowering — but it remains self-authored, and a fully independent gold standard is future work.
- **Keyword matcher placeholder** — INV-P1 lowering canonical keyword taraması yapar; semantik false-positive üretür (held_002 "couplings in pipe assembly" → YANLIŞ Coupling hint). *"The deterministic matcher can confuse lexical coupling with software coupling. The matcher is not the contribution; the binding protocol is."*
- **Coarse classifier** — negasyon yakalayamaz (held_003), Decision: typed-prefix parse edemeyebilir (fix_007). Faz 6 calibration.
- **Stub provider** — code evidence in-memory, gerçek SCIP entegrasyonu Faz 4 sonrası.
- **Acceptance seeded** — INV-C3 OperatorAcceptance pub(crate) → integration test promote yapamaz, acceptance state seeded. Faz 8 operator console gerçek API.

## 11. Future Work

- Faz 5.2/5.3: MetricDelta + EvidenceRequired + RelationExists executable
- Faz 6: Concept Synthesis (code repo → concept hipotezleri)
- Faz 7: Embedding + LLM-assisted candidate generation
- Faz 8: Desktop integration (Project Reality Cockpit) + operator console (real INV-C3 promote)

## 12. Conclusion

*"Words do not mutate project reality. Only bound, accepted, measured structures can."*

---

## Appendix A: Pre-flight Canonical + Marker Tablosu

*"Test altına alınmayan invariant ihlal edilir."* — 4 review turunda 3 kez yakalanan canonical-kesme tuzağı (A1→B1→B5) + 1 marker-kaçırma tuzağı yapısal imkânsız kılındı.

| Cümle | Canonical (ilk 3 kelime) | Normalize | Rule signal | Ambiguity | Axes |
|---|---|---|---|---|---|
| Coupling must not exceed module threshold. | CouplingMustNot | couplingmustnot | true (must not) | SingleCandidate | [Coupling] |
| The couplings in the pipe assembly must not be reused. | TheCouplingsIn | thecouplingsin | true (must not) | SingleCandidate | [Coupling] (YANLIŞ — fiziksel boru) |
| Modüller arası bağımlılık azaltılmalı. | ModüllerArasıBağımlılık | modullerarasibagimlilik | true (malı) | SingleCandidate | [Coupling] (via bagiml alias) |
| Coupling rule must not be enforced during tests. | CouplingRuleMust | couplingrulemust | true (must not) | SingleCandidate | [Coupling] |
| Coupling and cohesion must not diverge. | CouplingAndCohesion | couplingandcohesion | true (must not) | MultipleCandidates | [Coupling, Cohesion] |
| Witness count must not create metric evidence. | WitnessCountMust | witnesscountmust | true (must not) | NoAxisCandidate | [] (bare witness excluded) |

Tablo `paper3_evidence.rs::preflight_canonical_and_rule_signal_for_paper3_evidence_sentences` testinde gerçek pipeline koşusuyla pinlenmiştir. Gelecekte fixture ekleyen herkes aynı ağa takılır.

## Appendix B: End-to-End Binding-Chain Replay

`e2e-binding-chain-replay.json` (commit `481690d`). 8 adım: sentence → RuleCandidate (REAL pipeline) → PredicateStub → CrossFamilyHint → operator binding → ExecutablePredicateSet → verify accepted (SEEDED, INV-C3) → create task → registry.

## References

[TODO ~12-15, numeric [N]: requirements traceability, CNL, GraphRAG, program analysis, AI agents, P1/P2.]
