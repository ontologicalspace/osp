# INV-T9 #70 Commit 4b Faz 3 — Engine Binding & Derivation Plan

**Tarih:** 2026-07-21
**WIP branch:** `wip/inv-t9-70-commit4b` (head: `e4c6756`)
**Base:** `baa90a8` (Commit 4a APPROVED 9.9/10)
**Draft PR:** #81 (review-only, non-mergeable)
**Önceki:** Faz 1+2 TAMAM (scoped review #1→#4, 1067 osp-core test green)

---

## Faz 3 sözleşmesi

Faz 3, Commit 4b'nin kalbidir — presented `EngineMeasurement` token'ını claim/task/subject/impact/delta/revision/context karşısında doğrulayan binding implementation'ı + engine-owned target/loss derivation + `commit_task_claim` refactor.

**Reviewer v2 karar 4 (full binding) + v3 P1-4 (Mismatch vs Derivation ayrımı) + v4 P1-2 (somut Rust error tipi) + v4 P1-3 (disposition sınıflandırma):**

- `verify_measurement_binding` → `Result<VerifiedMeasurementBinding, MeasurementBindingVerificationError>`
- 8 binding check (task/subject/impact/structural_delta/revision/context_digest/current_context/request_digest)
- `VerifiedMeasurementBinding` basis builder consume eder (re-derivation yok)
- Mismatch (disposition: Regenerate vs Reject) vs Derivation (system failure) ayrı EngineCommitError
- `BoundMeasurementSession` current context verify + `verify_unchanged()` binding sonunda

---

## Zemin (Faz 1+2'den hazır tipler)

Faz 1+2'de tanımlanan tipler (Faz 3 bunları consume eder):

| Tip | Konum | Faz 3 kullanımı |
|---|---|---|
| `TaskValidationError` + `Task::validate_for_commit()` | trajectory.rs | commit_task_claim guard |
| `MeasurementBindingMismatch` (7 varyant + disposition) | measurement.rs | verify_measurement_binding dönüş |
| `MeasurementBindingDerivationError` (7 varyant) | measurement.rs | verify_measurement_binding dönüş |
| `MeasurementBindingVerificationError` (Mismatch \| Derivation) | measurement.rs | verify_measurement_binding dönüş tipi |
| `MeasurementBindingDisposition` | measurement.rs | navigator retry/reject (Faz 8) |
| `VerifiedMeasurementBinding` | engine.rs | verify_measurement_binding üretir, basis builder consume |
| `EngineCommitError::TaskValidation` | engine.rs | `?` yayılım |
| `EngineCommitError::MeasurementBindingMismatch` | engine.rs | `?` yayılım |
| `EngineCommitError::MeasurementBindingFailed` | engine.rs | `?` yayılım |
| `impl From<MeasurementBindingVerificationError> for EngineCommitError` | engine.rs | `?` yayılım |
| `MeasurementRequest::canonical_evidence()` | measurement.rs | basis builder request snapshot |
| `TrajectoryLossEvidence<'a>` / `OwnedTrajectoryLossEvidence` | trajectory.rs | gate input + downstream |
| `TrajectoryEvidenceBaseline<'a>` | trajectory.rs | gate input |
| `CanonicalTrajectoryEvidenceBaseline` + `try_from_reason` | authorization.rs | basis v2 (Faz 4) |
| `CanonicalTrajectoryLossEvidence` | authorization.rs | basis v2 (Faz 4) |
| `check_claim_structure` / `check_raw_position_finite` / `check_vision_raw_with_context` | engine.rs | Q4/Q5 task-bound |

---

## Implementation (alt-parçalar — her biri WIP commit)

### Alt-parça 1: `verify_measurement_binding` helper

**Konum:** engine.rs (VerifiedMeasurementBinding ile aynı modül — constructor private)

**İmza:**
```rust
fn verify_measurement_binding(
    &self,
    claim: &crate::witness::Claim,
    task: &crate::trajectory::Task,
    measurement: &crate::measurement::EngineMeasurement,
) -> Result<VerifiedMeasurementBinding, crate::measurement::MeasurementBindingVerificationError>
```

**8 check (reviewer v2 karar 4):**

1. **TaskMismatch:** `claim.task_id == Some(task.id)` — değilse `MeasurementBindingMismatch::TaskMismatch`
2. **SubjectMismatch:** `derive_task_subject_scope(task)` → `measurement.request().subject()` karşılaştır. Derivation hatası → `SubjectDerivationFailed`; mismatch → `SubjectMismatch { expected, presented }`
3. **ImpactMismatch:** `derive_impact_scope(claim)` → `measurement.request().impact()`. Derivation → `ImpactDerivationFailed`; mismatch → `ImpactMismatch { expected, presented }`
4. **StructuralDeltaMismatch:** `canonical_structural_delta_from_claim(claim).digest()` → `measurement.request().structural_delta_digest()`. Derivation → `StructuralCanonicalizationFailed`; mismatch → `StructuralDeltaMismatch { expected, presented }`
5. **RevisionMismatch:** `self.current_space_view_revision()` → `measurement.request().base_revision()`. Derivation → `RevisionComputationFailed`; mismatch → `RevisionMismatch { expected, presented }`
6. **ContextDigestMismatch:** `MeasurementInputDigest::compute(measurement.context())` → `measurement.request().measurement_input_digest()`. Derivation → `ContextConstructionFailed`; mismatch → `ContextDigestMismatch { expected, presented }`
7. **CurrentContextMismatch:** `BoundMeasurementSession::begin(&self.coord_system)` → `MeasurementInputContext::try_new(session.axis_descriptors())` → `measurement.context()` karşılaştır. Derivation → `CurrentContextCaptureFailed`/`ContextConstructionFailed`; mismatch → `CurrentContextMismatch { expected, presented }`. **Önemli:** session sonunda `session.verify_unchanged()` çağrılmalı (reviewer v2 P2-3).
8. **RequestDigest (commitment derivation):** `MeasurementRequestDigest::compute(measurement.request())` → `VerifiedMeasurementBinding.request_digest`. Bu bir "check" değil, commitment derivation — basis v2'ye taşınır (reviewer scoped P1-5).

**Return:** `VerifiedMeasurementBinding::new(subject, impact, canonical_delta, current_revision, current_context, request_digest)` — modül-private constructor (engine.rs aynı modül).

**Test'ler (Faz 3 içinde):**
- `verify_measurement_binding_rejects_task_mismatch` (claim.task_id ≠ task.id)
- `verify_measurement_binding_rejects_subject_mismatch` (re-derived subject ≠ token subject)
- `verify_measurement_binding_rejects_revision_mismatch` (stale token)
- `verify_measurement_binding_rejects_current_context_mismatch` (axis drift)
- `verify_measurement_binding_succeeds_for_valid_token` (exact match)
- `verify_measurement_binding_no_side_effect_on_failure` (space değişmedi, witness çağrılmadı)

### Alt-parça 2: `TrajectoryLossEvidence` derivation

**Konum:** engine.rs (commit_task_claim içinde)

Engine-owned loss derivation (reviewer v3 P0 — caller target/loss_before veremez):

```rust
// preferred_vector task'tan derive (None geçerli — reviewer v3 P0)
let preferred_vector = task.target_predicate_set.preferred_vector;

let (loss_evidence, baseline) = match &measurement.before {
    crate::measurement::MeasurementBaseline::Available(before_measured) => {
        match preferred_vector {
            Some(target) => {
                let raw_before = before_measured.to_raw(); // MeasuredRawPosition → RawPosition
                let raw_after = measurement.after().to_raw();
                let loss_before = trajectory_loss_measured(before_measured, &target);
                let loss_after = trajectory_loss_measured(measurement.after(), &target);
                (
                    TrajectoryLossEvidence::Available { target: &target, loss_after },
                    TrajectoryEvidenceBaseline::Available { measured_before: before_measured },
                )
            }
            None => (
                TrajectoryLossEvidence::Unavailable { reason: TrajectoryLossUnavailableReason::NoPreferredVector },
                TrajectoryEvidenceBaseline::Available { measured_before: before_measured },
            )
        }
    }
    crate::measurement::MeasurementBaseline::Unavailable { reason } => {
        // baseline unavailable — loss evidence target olsa bile before yok
        match preferred_vector {
            Some(target) => (
                TrajectoryLossEvidence::Available { target: &target, loss_after: trajectory_loss_measured(measurement.after(), &target) },
                TrajectoryEvidenceBaseline::Unavailable { reason },
            ),
            None => (
                TrajectoryLossEvidence::Unavailable { reason: TrajectoryLossUnavailableReason::NoPreferredVector },
                TrajectoryEvidenceBaseline::Unavailable { reason },
            ),
        }
    }
};
```

**Önemli:** `trajectory_loss` şu an `ProvenancedRawPosition` (= `MeasuredRawPosition`) alıyor — imza uyumlu. `trajectory_loss_measured` helper veya direkt `trajectory_loss` kullan.

### Alt-parça 3: `commit_task_claim` refactor

**Konum:** engine.rs:529-672 (mevcut)

**Yeni guard sırası (reviewer v2 karar 2 + v4 P2-2):**
```
1. Q4 structural syntax: check_claim_structure(input.claim)  ← claim.computed_raw'a dokunmaz
2. Task bind: TaskBoundClaim kurulumu (claim.task_id yok → PermissionDenied)
3. Task declaration validation: bound.task.validate_for_commit()?  ← TaskValidation (tag 7)
4. Measurement binding: verify_measurement_binding(claim, task, &measurement)?  ← MeasurementBindingMismatch/Failed (tag 8)
5. Verified measurement value validation: check_raw_position_finite(claim.id, "measurement.after", &measurement.after().to_raw())?  ← binding sonrası (scoped P2-2)
6. Q5 vision: check_vision_raw_with_context(claim.id, &measurement.after().to_raw(), &vision_context)?  ← raw_after (P0-2)
7. PredicateGate: evaluate(measured_after + TrajectoryEvidenceBaseline + TrajectoryLossEvidence + target)  ← completion-first (Faz 5 refactor öncesi mevcut gate)
8. Q6 rule check
9. build_authorization_context_v2(measurement, verified_binding, baseline, loss_evidence)  ← re-derivation yok
10. witness
```

**ÖNEMLİ — TaskCommitInput henüz değişmiyor (Faz 8):** Mevcut `TaskCommitInput { claim, omega, task_resolver, target, loss_before, measured }` korundu (Faz 1'de TODO eklendi). Faz 3'te commit_task_claim bu field'ları kullanmaya DEVAM EDER — ama `target`/`loss_before`'u caller'dan değil engine-owned derivation'dan alacak şekilde refactor edilir. `measured` field'ı ise `EngineMeasurement`'a dönüşür — ama bu Faz 8'de (caller migration ile).

**Faz 3 geçici yaklaşımı:** commit_task_claim içine `measurement` parametresi EKLEME — bunun yerine mevcut `input.measured`'ı kullanıp, engine-owned target/loss derivation'ı uygula. verify_measurement_binding için geçici olarak `input`'tan TaskBoundClaim + dummy EngineMeasurement kurmak gerekebilir. **Bu Faz 3'ün en karmaşık noktası** — TaskCommitInput smart constructor Faz 8'e kadar beklerse, verify_measurement_binding çağrılamaz (EngineMeasurement yok).

**Çözüm seçenekleri:**
- **Seçenek A (önerilen):** Faz 3'te TaskCommitInput'a `measurement: EngineMeasurement` field'ı EKLE (target/loss_before/measured korunsun — backward-compat). commit_task_claim measurement field'ını kullanır. Faz 8'de target/loss_before/measured kaldırılır. Bu Faz 3'ü mümkün kılar.
- **Seçenek B:** Faz 3'ü erteleyip önce Faz 8 (caller migration + smart constructor) yap. Ama plan sırasını bozar.

**Seçenek A'yı uygula** — TaskCommitInput'a `measurement` field'ı ekle (Option veya zorunlu). commit_task_claim measurement kullanır.

### Alt-parça 4: `build_authorization_context_v2`

**Konum:** engine.rs:669-822 (mevcut build_authorization_context refactor)

**Değişiklikler:**
- Parametre: `verified_binding: &VerifiedMeasurementBinding` (re-derivation yok)
- `measured_result` (line 765-770) → `measurement.after()`'dan (token authority)
- `MeasurementInputContext::try_from(&coord_system)` (line 779-780) → `verified_binding.current_context()` (drift kapatma — reviewer v3)
- `measurement_input_digest` → `verified_binding.current_context()`'ten compute veya `measurement.request().measurement_input_digest()`
- Basis v2 field'ları (Faz 4'te struct değişir ama Faz 3 wiring hazırlar)

**Dikkat:** build_authorization_context_v2 henüz v2 struct KULLANMAZ (Faz 4 v2 struct'ı tanımlar). Faz 3 mevcut v1 AuthorizationBasis'i doldurur ama verified_binding'den alarak — drift kapatma.

---

## CI doğrulaması (her alt-parça sonrası)

```bash
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo build -p osp-core --lib
RUSTFLAGS="-D warnings" cargo test -p osp-core --lib  # 1067 → ~1075+
```

Workspace build her WIP commit'te doğrulanmaz (Faz 8 caller migration öncesi MCP/CLI kırılabilir). Sadece osp-core lib + test.

---

## Risk & notlar

- **TaskCommitInput smart constructor Faz 8:** Faz 3 `measurement` field'ı ekler (Seçenek A), target/loss_before/measured Faz 8'de kaldırılır.
- **PredicateGate completion-first Faz 5:** Faz 3 mevcut gate kullanır (loss_before/loss_after skalar). Faz 5 typed TrajectoryLossEvidence gate input'a refactor. Faz 3 wiring hazırlar.
- **AuthorizationBasis v2 struct Faz 4:** Faz 3 v1 struct'ı doldurur (verified_binding'den), Faz 4 v2 struct + custom Deserialize + validate_v2.
- **BoundMeasurementSession verify_unchanged:** binding check 7 sonrası çağrılmalı (reviewer v2 P2-3) — current context capture + verify.
- **WIP bazlı:** her alt-parça WIP commit + push. draft PR #81 incremental review. Final tüm fazlar bitince squash.

---

## Sonraki fazlar (Faz 3 sonrası)

- Faz 4: AuthorizationBasis v1→v2 (struct + validate_v2 + custom Deserialize + DOMAIN_SEPARATOR split + v1 frozen fixture + v2 golden)
- Faz 5: PredicateGate completion-first refactor (TrajectoryLossEvidence gate input)
- Faz 6: Navigator state migration (current_loss Option + typed progression)
- Faz 7: Downstream typed loss (TaskCommitResult.evaluation, TrajectoryEvidence typed baseline, MCP JSON)
- Faz 8: Caller migration + TaskCommitInput smart constructor (target/loss_before/measured kaldır)
- Faz 9: Deprecation + module-wide syn AST + legacy_projection modülü
- Faz 10: trybuild compile-fail guards
- Faz 11: #80 osp-desktop fix
- Faz 12: Tests (tüm matrisler + red-test'ler)
- Faz 13: CI + truth-surface sync + squash to atomic commit + push

---

*Bu belge INV-T9 #70 Commit 4b Faz 3 implementation planıdır. Faz 1+2 tamamlandı (scoped review #1→#4, 1067 osp-core test green). Faz 3 Commit 4b'nin kalbidir — engine binding & derivation.*
