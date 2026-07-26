# INV-T9 #70 Commit 4b Faz 5 — Handoff (Checkpoint A complete: Adım 1-20)

## Repository state

```
Branch: wip/inv-t9-70-commit4b
HEAD: 488be1b (Adım 17 P0-B + P0-C restore validators)
Remote HEAD: 488be1b (sync — local = remote)
Push status: Adım 16-20 + Adım 17 PUSHED (8fe5fa5, 488be1b)
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
Commands:
  cargo fmt --all -- --check
  RUSTFLAGS="-D warnings" cargo build -p osp-core --lib
  RUSTFLAGS="-D warnings" cargo test -p osp-core --lib
  RUSTFLAGS="-D warnings" cargo test -p osp-core --test engine_measurement_single_producer
  RUSTFLAGS="-D warnings" cargo test -p osp-core --test measurement_binding_typelevel

Results:
  fmt: clean (0 diff)
  build: 0 warning (-D warnings)
  lib test: 1244 passed, 0 failed
  engine_measurement_single_producer: green (compile_fail)
  measurement_binding_typelevel: green

Notable: 1245 → 1244 (commit2_rejected_gate testi Faz 8'e ertelendi — bundle
sadece GatePassed üretir, RejectedByGate Faz 8 hard-gate).
```

## Checkpoint A — Tamamlandı (Adım 1-20, issue #83)

### Faz 5 adımları (issue #83 Checkpoint A = Adım 1-20)
- **Adım 1-15**: reverse projection, canonical V2 infrastructure, digest continuity,
  decision core, error taxonomy, module structure (13 commit, b8bdd00..cacdabd)
- **Adım 16-20** (`8fe5fa5`, push edildi): atomik V2 proof bundle + TOCTOU closure
  - Adım 16: AuthorizationBasisV2 13→17 field + 4-field commitment parity + digest encoder
  - Adım 18: VerifiedGateEvaluationBundleV2 (opaque, from_gate_passed private)
  - Adım 19: evaluate_task_gate_v2 (3 digest recheck + predicate exactly once +
    completion-first loss matrisi + decision core)
  - Adım 20: build_authorization_context_v2(bundle,...) — single bundle consume,
    engine.rs → gate_v2.rs taşındı (self. = 0)
- **Adım 17** (`488be1b`, push edildi): restore validators (P0-B + P0-C)
  - validate_predicate_basis_semantics_v2: semantics version + PredicateSet restore +
    validate_predicate_goal_for_commit (P0-B: All+Some reject)
  - validate_gate_decision_semantics_v2: completion-first matris recheck (P0-C)

### Review geçmişi (issue #83 v8 plan + kod review)
- v8 plan frozen (9.8/10 APPROVED, 8 review turu)
- Adım 16-20 kod review: REQUEST CHANGES (5 P0 + 5 P1) → tümü address edildi
  - P0-1: predicate_basis binding epoch'unda DEĞIL (bundle-scoped)
  - P0-2: builder completion-first loss (preferred-vector'dan DEĞIL)
  - P0-3: 4-field commitment parity (digest + identity)
  - P0-4: dedicated predicate basis encoder (result + semantics_version + policy)
  - P1-1: TryFrom<&Task> authoritative projection (unwrap YOK)
  - P1-2: dedicated policy digest error variant
  - P1-3: nested wire tipleri deny_unknown_fields
  - P1-4: predicate_gate_policy_digest (generic policy_digest DEĞIL)
  - P1-5: allow 19 (17 değil)

## Still blocking (Faz 8 scope dışı)

### Faz 5 Faz 8 wiring bekleyen (3 allow)
- `engine.rs`: bundle accessor (predicate_gate_policy_digest) — Faz 8 production wiring
- `authorization.rs`: VerifiedCanonicalTaskGoalEvidenceV2 (struct + impl) — Faz 8
  context-restore katmanı consumer bekleyen
- `gate_v2.rs`: ProducedTrajectoryLossEvidence producer (new/is_finite/loss_before/
  loss_after) — Adım 17 helper doğrudan canonical üretti, producer-side evidence
  Faz 7/restore bekleyen

Bu 3 allow Adım 17 sonrası "gerçek" dead-code — Faz 8 production wiring +
Faz 7 as_owned + context-restore katmanı consumer olmadığı için. Faz 5 Checkpoint A
kapsamında 0'a inemez (Faz 8/Faz 7 işi).

### Faz 8 (kapsam dışı)
- production wiring (commit_task_claim V2 projection)
- RejectedByGate proof producer + NotEvaluated/hard-gate evidence
- navigator/persistence V2
- AuthorizationReceiptV2 + EngineCommitError mapping
- TaskCommitInput smart ctor

### Faz 9 (kapsam dışı)
- V1 adapter + V1 path remove
- TaskGoalDigest domain sep V1 naming değerlendirme

## Allow closure (Faz 5 temporary allow)
```
Before (Adım 16-20 öncesi): 19
After Adım 16-20: 7 (consumer'ı olanlar kaldırıldı)
After Adım 17: 3 (restore validator allow'ları kaldırıldı — PredicateBasisConsistencyError
              + GateSemanticConsistencyError artık consumer'a sahip)
Kalan 3: Faz 8 wiring + Faz 7 as_owned + context-restore bekleyen (Faz 5 dışı)
```

## Golden byte contracts (Adım 16-20 + Adım 17 sonrası)
- `FAZ4_BASIS_V2_GOLDEN_HEX`: `696d39df204325cfb9781c621c8dcd6cfcfde42557f9731a0c7f32064ce36c17`
  (Adım 17 regolden — fixture loss NotRequired oldu, matris consistency)
- `FAZ4_CONTEXT_V2_GOLDEN_HEX`: `f2818b8bf55b9cc22d6f0fbe68f968fcf5afc3be326547ceb94d954dab568890`
- `OSP/TASK-GOAL/V1` golden (`03a3ad38...`): UNCHANGED — TryFrom<&Task> delegate byte-identical
- `authorization_basis_v2_wire.json`: regen (+4 field Adım 16, loss NotRequired Adım 17)

## Frozen conversion decisions (güncel)
- Makroya blanket reverse conversion YOK
- Tam 4 manuel From<Tag> for Domain
- Structural canonical restore checked TryFrom
- TryFrom<&Task> for CanonicalTaskGoalEvidenceV2 — tek authoritative forward projection
- TaskGoalDigest::compute delegate eder (iki paralel projection YOK)
- All mode + None weight (P0-B: All+Some reject)

## Review düzeltmeleri (tamamlanan, tekrar YAPILMAYACAK)
- ✅ Adım 16-20 kod review tüm P0/P1 (5+5)
- ✅ Adım 17 P0-B (restore task-goal validation) + P0-C (semantics version matris)

## Key file:line references (Adım 16-20 + 17 sonrası)
- `AuthorizationBasisV2` (17 field): authorization.rs ~L2712-2757
- `AuthorizationBasisV2::new` (17 param): authorization.rs ~L2775-2810
- `validate_semantics` (+ 4-field parity + Adım 17 validator'lar): authorization.rs ~L2830
- `validate_predicate_basis_semantics_v2` (P0-B): authorization.rs ~L2970
- `validate_gate_decision_semantics_v2` (P0-C matris): authorization.rs ~L3040
- `encode_canonical_predicate_evaluation_basis_v2` (P0-4 dedicated): authorization.rs ~L3660
- `AuthorizationBasisDigestV2::compute` (+4 encoding): authorization.rs ~L3295
- `VerifiedGateEvaluationBundleV2`: gate_v2.rs ~L165
- `evaluate_task_gate_v2`: gate_v2.rs ~L312
- `compute_completion_first_loss_and_decision` (matris): gate_v2.rs ~L497
- `build_authorization_context_v2` (free fn, bundle): gate_v2.rs ~L640
- `TryFrom<&Task> for CanonicalTaskGoalEvidenceV2`: authorization.rs ~L427
- `faz4_basis_v2_raw_parts` (fixture, +4 field, NotRequired loss): authorization.rs ~L14353

## Do not change (plan negatif koşulları)
- commit_task_claim / EngineCommitResult / TaskCommitInput in Faz 5
- V1 wire/digests (golden korumasız DEĞİL — frozen)
- RejectedByGate producer + NotEvaluated before Faz 8
- bundle decomposition visibility (private)
- declared_weight None vs Some(1.0) digest distinction
- OSP/TASK-GOAL/V1 golden hex (03a3ad38...) — compute_from_canonical continuity
- authorization.rs → mod.rs taşıma YOK
- #[from] YOK in Faz 5 error taxonomy (explicit map_err)
- predicate exactly once (tek evaluate_completion çağrısı)
- restore'da 2. predicate evaluator YOK

## Next execution order (Faz 8 veya sonraki faz)
1. **Verify HEAD/worktree/push state** (Adım 17 commit + push)
2. **Faz 8 production wiring** — commit_task_claim V2 projection, RejectedByGate producer,
   navigator/persistence V2, AuthorizationReceiptV2
3. **Faz 9** — V1 adapter + V1 path remove
4. **Faz 11** — osp-desktop #80

---

## Faz 8-P1 — Downstream V2 Readiness (TAMAMLANDI — Adım 1-11)

**Plan:** 5 review turunda sağlamlaştırıldı, onaylandı (frozen implementation-ready).
**Durum:** Adım 1-11 TAMAMLANDI. CI workspace-wide `-D warnings` green.

### CI durumu (tam doğrulandı)
```
cargo fmt --all -- --check: clean
RUSTFLAGS="-D warnings" cargo build --workspace --all-targets --exclude osp-desktop: clean
RUSTFLAGS="-D warnings" cargo test -p osp-core --lib: 1252 passed, 0 failed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test engine_measurement_single_producer: 7 passed
RUSTFLAGS="-D warnings" cargo test -p osp-core --test measurement_binding_typelevel: 1 passed
```

### Adım 1-11 tamam
1. `SuspendedAttemptEvidenceV2` + digest (context digest bound, `OSP/ATTEMPT-EVIDENCE/V2`)
2. `validate_suspension_eligibility_v2` (8 invariant, private fn)
3. `PendingAuthorizationV2` (asimetrik creation/load API — P0-1)
4. `RevisionRequiredV2` (runtime-only, NO Serialize — P0-1)
5. `PersistedAuthorizationContextV2` wrapper + wire DTO + private `restore_from_wire` (P0-2)
6. `PendingAuthorizationEnvelopeV2` (`try_new_held` tek public root, defensive verify)
7. Versioned dispatch + sum-type + `load_pending_authorization_versioned`
8. Store trait additive `persist_v2` + Filesystem V2 override + `load_versioned`
9. `LowerHex32` strict wire parse (Adım 7'de inline)

### Tip genişletmeleri (Adım 5 riski doğrulandı, çözüldü)
- `WitnessNotRequiredReason` Deserialize (additive, V1 frozen)
- `CanonicalWitnessRequirementV2::required`/`not_required` constructor + accessors
- `CanonicalWitnessRequirementV2Error` +2 variant
- `AuthorizationContextDigestV2::from_bytes` + `AuthorizationContextV2::restore` (private)

### Kalan dead-code (3, Faz 8a navigator consumer)
- `PendingAuthorizationV2Error::WitnessHoldReasonMismatch`/`WitnessSnapshotMismatch` variants
- `PendingAuthorizationV2::attempt_num` accessor
- `PersistedAuthorizationContextV2::into_context` accessor

### Adım 10-11 (sonraki oturum)
- **Adım 10:** V2 adversarial test matrisi (Faz 5 `commit2_build_authorization_context_v2_pipeline`
  reuse). Kalan 3 dead-code'u consumer yapar. Tamper schema test `.v99`'a retarget EDİLDİ.
- **Adım 11:** CI workspace-wide `-D warnings` green.

### Faz 8-P1 frozen kararlar (5 review turu)
- `AuthorizationContextDigestV2` reuse (var, auth.rs:3673)
- V2 evidence context digest'e bağlı (basis DEĞİL)
- Envelope tam context (basis + gate_evaluation + witness_requirement)
- Raw DTO → checked domain restore
- Generic `AuthorizationReceiptV2` bu fazda YOK
- Navigator methods Faz 8a'ya
- V1/V2 strict schema dispatch; V1 artifact backward compat
- Evidence constructor digest üretmez (V1 pattern mirror)
- Asimetrik creation/load API; load outer stored korur (overwrite ETMEZ)
- `try_new_held` caller-provided digest/identity kabul etmez
- `from_verified_parts` her zaman `envelope.verify()` çağırır
- Wire loader `try_new_held` çağırmaz (creation/restore ayrı)
- `RevisionRequiredV2`: Serialize YOK, Deserialize YOK
- `PersistedAuthorizationContextV2` wrapper frozen; restore private method
- Public entrypoints: `try_new_held` + `load_pending_authorization_versioned` +
  `load_versioned` + `persist_v2`
- `validate_suspension_eligibility_v2` private (Faz 8a `witness_dispatch_requirement_v2`)

### Faz 8 roadmap (review 1. tur)
```
Faz 8-P1: additive downstream V2 readiness (BU — Adım 1-9 done, 10-11 next)
Faz 8-P2: measurement caller migration (navigator/MCP compute_raw → measure_task_delta)
Faz 8a:   engine cutover (commit_task_claim → V2, V1 producer silme, atomik)
Faz 8b:   AuthorizationReceiptV2 (Satisfied/application receipt)
Faz 8c:   persistence + downstream migration (PendingAuthorizationEnvelopeV2 restore)
```

### Sonraki adım (Faz 8-P2)
Faz 8-P1 tamamlandı, CI green. Sıradaki:
1. **Commit:** Faz 8-P1 atomik (Adım 1-11, tüm review kararları frozen)
2. **Faz 8-P2:** Measurement caller migration (navigator/MCP compute_raw →
   measure_task_delta). `TaskCommitInput::new` + `EngineMeasurement` typed result.
3. **Faz 8a:** Engine cutover (commit_task_claim → V2, V1 producer silme, atomik)
