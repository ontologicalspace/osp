# INV-T9 #70 Commit 4b Faz 5 — Handoff (Checkpoint A)

## Repository state

```
Branch: wip/inv-t9-70-commit4b
HEAD: cacdabd5a670bbf3fc844b069872216641c8e0aa
Remote HEAD: b8bdd005101b9bebd725b424b0dd1c35e7685aeb
Push status: 13 commits LOCAL-ONLY (push edilmemiş)
Worktree: clean (source) — sadece untracked docs var
Untracked files:
- docs/design/plan-bound-task-lifecycle.md
- docs/notes/planlama-tasarım-eskiz.txt
- docs/notes/proje-adaylari.md
- docs/notes/sohbet-konu.txt
- docs/osp-3ay-hedef-karti.md
- docs/osp-papers-comparison.md

Series base: main @ 45686dc
Faz 5 review base (allow karşılaştırma): b8bdd00
```

## Last fully verified state

```
Verified SHA: cacdabd5a670bbf3fc844b069872216641c8e0aa

Commands:
  cargo fmt --all -- --check
  RUSTFLAGS="-D warnings" cargo build -p osp-core --lib
  RUSTFLAGS="-D warnings" cargo test -p osp-core --lib
  RUSTFLAGS="-D warnings" cargo test -p osp-core --test engine_measurement_single_producer
  RUSTFLAGS="-D warnings" cargo test -p osp-core --test measurement_binding_typelevel

Results:
  fmt: clean (0 diff)
  build: 0 warning
  lib test: 1245 passed, 0 failed
  engine_measurement_single_producer: 7 passed
  measurement_binding_typelevel: 1 passed

No source changes were made after this verification.
```

## Completed (13 commit, b8bdd00..cacdabd)

### Reverse projection + canonical V2 infrastructure (Item 1-5, 10)
- `9ad30db` 4 manuel `From<Tag> for Domain` (P1-3) — makroya blanket YOK
- `eca91f4` 2 projection + `TryFrom<&ProvenancedMeasuredResult>` (P1-3)
- `bbb0cf5` CanonicalTaskGoalEvidenceV2 + CanonicalWeightedPredicateV2 (declared_weight) + reverse TryFrom
- `2fd213d` validate_predicate_goal_for_commit extraction (P0-1)

### Digest continuity + semantics versioning (Item 7, 8, 11)
- `0245c9f` TaskGoal+Measurement shared writer + compute_from_canonical (P0-2)
- `461e8c3` GATE_EVALUATION_SEMANTICS_V1 + CanonicalPredicateEvaluationBasisV2 (P0-2)
- `6038953` PredicateGatePolicyDigestV2 + VerifiedTaskMeasurementBinding TOCTOU commitment capture

### Decision core + private proof (Item 12, 14)
- `9e29b15` ImprovementAssessment + evaluate_decision_core extraction (P0-1)
- `7ceca86` VerifiedCanonicalTaskGoalEvidenceV2 private restore proof (P0-1)

### Error taxonomy + module structure (Item 13)
- `71f336a` gate_v2 child module (mod gate_v2;) + error taxonomy (P1-1)

### Review REQUEST CHANGES düzeltmeleri
- `a2124a9` **P0-2** assess_improvement_v1 ayrımı + **P0-3** policy digest cryptographic binding (task_id+task_goal_digest, V2 sep) + **P1-2** EffectiveImproPolicyBasisV2 canonical struct + **P1-1** tek encoder (domain kaldırıldı)
- `1cf78c8` **P2** mod gate_v2 #[path] kaldırıldı (Rust 2018 auto-resolve) + **P1-3** CI matrisi
- `cacdabd` gereksiz allow kaldırıldı (PredicateGatePolicyDigestV2::compute zaten consumer'a sahip)

## Still blocking

### P0-A — Faz 5 allow(dead_code) closure (Item 15-17 consumer bekleyen)
Kalan 17 allow (cacdabd üzerinde). Her birinin consumer'ı Item 15-17 ile gelmeli:

| Allow (dosya:satır) | Item | Consumer |
|---|---|---|
| gate_v2.rs:32 GateEvaluationV2Error | 17 | evaluate_task_gate_v2 |
| gate_v2.rs:60 TrajectoryLossProductionError | 17 | evaluate_task_gate_v2 |
| gate_v2.rs:100,120,126,132 ProducedTrajectoryLossEvidence | 17 | evaluate_task_gate_v2 |
| authorization.rs:1058 PredicateBasisConsistencyError | 16 | restore validator |
| authorization.rs:1075 GateSemanticConsistencyError | 16 | restore validator |
| authorization.rs:4400 re-export unused_imports | 15-17 | consumers |
| authorization.rs:5070,5085 VerifiedCanonicalTaskGoalEvidenceV2 | 16/17 | restore validator |
| measurement.rs:716 MeasurementDigest::compute_from_canonical | 15 | validate_semantics |
| measurement.rs:1433 TaskGoalDigest::compute_from_canonical | 15 | validate_semantics |
| measurement.rs:1723 PredicateGatePolicyDigestV2::compute_from_canonical | 15 | validate_semantics |
| measurement.rs:1743,1748 PredicateGatePolicyDigestV2 as_bytes/to_hex | 15 | validate_semantics |
| engine.rs:555 predicate_gate_policy_digest accessor | 15 | validate_semantics |

Closure kriteri: `Faz 5 kaynaklı yeni allow(dead_code) = 0` ve `allow(unused_imports) = 0`.

### P0-B — Restore task-goal production validation (Item 16 semantic blocker)
Restore edilen PredicateSet, `validate_predicate_goal_for_commit(task_id, &predicate_set)`'ten geçmeli:
- boş predicate set reddi, mode/weight shape, finite threshold/tolerance, Mixed source reddi, weight positivity, preferred vector finite.

**Kritik:** `All + Some(1.0)` digest continuity testinde temsil edilebilir AMA authorization restore semantiğinde **geçersiz declaration olarak reddedilmeli** (validate_predicate_goal_for_commit UnexpectedWeightForUnweightedMode döner).

### P0-C — Persisted gate evaluation semantics version kapsamı (Item 16 semantic blocker)
`gate_evaluation_semantics_version` şu semantik kümeyi bağlamalı:
```
predicate composition + source propagation + trajectory loss +
baseline handling + improvement assessment + mutation decision
```
Restore unsupported version → fail-closed reject.

## Frozen conversion decisions (güncel)

- **Makroya blanket reverse conversion YOK.**
- **Tam 4 manuel `From<Tag> for Domain`:** PredicateAxis, ComparisonOp, MetricSource, PredicateMode.
- **Structural canonical restore checked `TryFrom` olarak kalır** (ProvenancedMeasuredResult→MeasuredRawPosition, CanonicalWeightedPredicateV2→WeightedPredicate, CanonicalTaskGoalEvidenceV2→PredicateSet).
- **Testler:** 4 round-trip + PredicateMode forward exact mapping.

## Review düzeltmeleri (tamamlanan, tekrar YAPILMAYACAK)

- ✅ P0-2: `assess_improvement_v1` loss+hard-cap ayrı; `evaluate_decision_core` assessment-only
- ✅ P0-3: policy digest preimage task_id + task_goal_digest + V2 sep
- ✅ P1-1: tek encoder — domain `encode_weighted_predicate_to_vec` KALDIRILDI, V2 encoder tek source
- ✅ P1-2: `EffectiveImproPolicyBasisV2` ayrı canonical struct (alias DEĞİL), TryFrom checked
- ✅ P2: `mod gate_v2;` (#[path] gereksiz)
- ✅ P0-4: commit message'lar "commitment capture"/"closure" doğru kullanım

## Next execution order

1. **Verify HEAD/worktree/push state** (13 commit local-only — push gerekli)
2. **Item 15:** AuthorizationBasisV2 13→17 field (measured_after, task_goal_evidence, predicate_basis, policy_digest) + validate_semantics policy digest reverify + AuthorizationBasisDigestV2::compute +4 encoding + wire DTO +4 field + from_wire/from_domain + build_authorization_context_v2 +4 arg + fixture +4 + **golden regoldening** (FAZ4_BASIS_V2_GOLDEN_HEX + FAZ4_CONTEXT_V2_GOLDEN_HEX + JSON fixture)
3. **Item 16:** branch-aware restore validators (validate_predicate_basis_semantics_v2 + validate_gate_decision_semantics_v2) — P0-B + P0-C address
4. **Item 17:** VerifiedGateEvaluationBundleV2 + evaluate_task_gate_v2 (3 digest recheck TOCTOU closure) + build_authorization_context_v2 move (engine.rs → gate_v2.rs free fn, self. kullanımı 0)
5. **allow kaldırma:** Item 15-17 consumer'ları gelince yukarıdaki 17 allow kaldırılır
6. **Full CI matrix** + push + remote HEAD kaydet

## Do not change (plan negatif koşulları)

- `commit_task_claim` / `EngineCommitResult` / `TaskCommitInput` in Faz 5
- V1 wire/digests (golden korumasız DEĞİL — frozen)
- `NotEvaluated` or `RejectedByGate` producer before Faz 8
- bundle decomposition visibility
- `declared_weight` None vs Some(1.0) digest distinction
- OSP/TASK-GOAL/V1 golden hex (03a3ad38...) — compute_from_canonical continuity ile korunur
- authorization.rs → mod.rs taşıma YOK (mod gate_v2; child module)
- `#[from]` YOK in Faz 5 error taxonomy (explicit map_err)

## Key file:line references

- `AuthorizationBasisV2` (13 field): authorization.rs:2632-2659
- `AuthorizationBasisV2::new` (13 param): authorization.rs:2665-2697
- `validate_semantics`: authorization.rs:2703-2777
- `AuthorizationBasisDigestV2::compute`: authorization.rs:3106-3141
- `FAZ4_BASIS_V2_GOLDEN_HEX`: authorization.rs:13943 (regoldening gerekli)
- `FAZ4_CONTEXT_V2_GOLDEN_HEX`: authorization.rs:13969 (regoldening gerekli)
- `RawAuthorizationBasisV2` (deny_unknown_fields): authorization.rs:6994-7010
- `from_wire` / `from_domain`: authorization.rs:7521+ / 7106+
- `build_authorization_context_v2` (engine.rs): engine.rs:858-961, `new()` call at 943
- `Faz4BasisV2RawParts` fixture: authorization.rs:13339-13353
- `faz4_basis_v2_raw_parts`: authorization.rs:13355-13426
- `faz4_provenanced_measured_result` (measured_after için): authorization.rs:13463-13478
- JSON fixture: crates/osp-core/tests/fixtures/authorization_basis_v2_wire.json (45 lines, regoldening)
