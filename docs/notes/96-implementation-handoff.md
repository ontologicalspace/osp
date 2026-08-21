# Handoff — #96 MD-2 PR #125 (review tur-5 sonrası durum)

**Tarih:** 2026-08-21 (oturum 4 sonu). Branch: `feat/96-md2-native-provenance-authority`.
**Head:** `b703370` — CI parity komutları yerelde yeşil (fmt + clippy `-D warnings` + test 40 suite).

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 PR #125 — W6/W8 acceptance suite'i tamamla** (merge öncesi
> tek kalan engel; review tur-5'in diğer maddeleri kapandı).
> Branch: `feat/96-md2-native-provenance-authority`.
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -5` (head `b703370` olmalı),
(3) `export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test --locked --workspace
--all-features --exclude osp-desktop` (40 suite yeşil başlangıç).

## Review tur durumu (tur-5, REQUEST CHANGES: 3 P1 + 3 P2)

| Bulgu | Öncelik | Durum | Commit |
|---|---|---|---|
| P0-1 authority-token forgeability | P0 | **KAPANDI** (tur-4'te; tur-5 onayladı) | `6bc765d` |
| P0-2 finalize bypass (artifact mix) | P0 | **KAPANDI** (tur-4'te; tur-5 onayladı) | `6bc765d` |
| V1 characterization fabrication (Q5=Passed / witness=Evaluated gözlenmemişken) | P1 | **KAPANDI** — honest-encoding | `b703370` |
| MCP failure ontology (navigator ile ayrık) | P1 | **KAPANDI** — ortak typed mapper | `b703370` |
| W6/W8 acceptance suite eksik | P1 | **AÇIK — merge öncesi tek engel** | — |
| Sidecar isim + accessor | P2 | **KAPANDI** | `b703370` |
| Handoff/PR body stale | P2 | **KAPANDI** (bu dosya + PR body güncellendi) | docs |
| `FinalizedNativeTaskClaim::new` pub(crate) | P2 | **KAPANDI** — private ctor | `b703370` |

## tur-5'te yapılanlar (b703370)

### P1 — V1 honest-encoding (truth-surface)
- `tests/common/mod.rs` V1 evaluator sentetik `EngineCommitResult` ÜRETMEZ artık;
  doğrudan `PipelineObservation` kurar. Motor-private aşamalar dürüstçe temsil
  edilir: `Q5Observation::NotObserved` (yeni additive variant) +
  `WitnessReachability::NotReached`. Yalnız gerçekten çalışan yüzeyler gözlemlenir
  (Q4 structural + raw finite + PredicateGate).
- Parity test pin'leri (`matching`/`wide-affected`/`removed-edge`/`delta-introduced`
  ×2/`counter`/`mixed`/`q92` helper) honest değerlere re-pinlendi.

### P1 — MCP failure ontology (ortak typed mapper)
- `osp-core/src/task_measurement.rs`: `NativeFailureSurface` enum
  (SyntaxRejection / RetryAgentProposal / SystemFailure / TaskNotFound /
  WitnessEvaluationError) + `MeasurementFailureDisposition::agent_surface()` +
  `commit_error_agent_surface()` (EngineCommitError 14 varyant exhaustive).
- Navigator üretim arm'ı da aynı mapper'a bağlandı (tek ontology).
- `osp-mcp/server.rs`: measurement failure → `system_failure` JSON (class + typed
  disposition + retryable:false), `attempt_outcome` YOK (fabrication kapandı);
  finalize binding mismatch → `system_failure`; retryable commit error → GERÇEK
  gate kararı (`gate_decision_from_engine_error`); gerçek structural Q4 (draft)
  ve raw-finite `RejectedBySyntax` KALIR (navigator parity bilinçli).
- Pin: `osp-mcp/tests/native_failure_ontology.rs` (TaskBindingMismatch fixture —
  `task_id=999` ≠ `task.id=1`) + osp-core mapper tablo test'leri.

### P2 — Sidecar diagnostic + accessor + private ctor
- `PendingAuthorizationLoadError::ProvenanceAuthorityDriftIdentityMismatch`
  (provenance yolu artık subject varyantının adını kullanmıyor).
- `RevisionRequired::provenance_authority_drift()` accessor (subject mirror).
- `FinalizedNativeTaskClaim::new` private — yalnız `finalize` üretir; "sealed"
  crate içinde de gerçek.
- **NOT:** sidecar identity-mismatch load path'lerinin derin pin test'leri W8
  envanterine bırakıldı (bugün hiçbir test iki tarafı da pinlemiyordu).

## Kalan tek engel: W6/W8 acceptance suite (P1, merge öncesi şart)

**W6 — verifier ×5 negatif + ABA + cross-pin** (sealed carrier sonrası şekil değişikliği):
1. Finalize-bypass negatifi artık COMPILE-test olmalı (`FinalizedNativeTaskClaim`
   private ctor — external crate elle kuramaz; `tests/` içinde sadece
   `draft.finalize` ile üretilebilir). Runtime-test DEĞİL.
2. `new_characterization_legacy` erişilemezliği compile-fail pin (ctor yok —
   `pub use` yüzeyinde bulunmadığını doğrulayan test).
3. ×5 negatifin kalanı: replay/tamper/stale/context-digest/mixed-artifact ailesi
   (`MeasurementBindingVerificationError` varyantları üzerinden).
4. ABA: measure → space mutate → commit (revision fence).
5. Basis ↔ token cross-pin (engine unit ↔ integration dual).

**W8 — observer envanteri + yarış + exhaustiveness:**
- `MeasurementFailureDisposition` 17-varyant tablosunun tamamı için producer
  envanteri (bugün `RetryAgentProposal`/`RegenerateMeasurement` üreticisiz —
  dormant pin).
- Q4-vs-measurement yarış testi (MCP yüzeyi için de — server.rs yorumunda pin notu var).
- `NativeFailureSurface` wire exhaustiveness (MCP JSON class'ları ↔ mapper).
- Sidecar identity-mismatch (subject + provenance) load-path pin'leri.

## Ortam notları (Windows)

- `export PATH="$HOME/.cargo/bin:$PATH"` her shell'de
- `command grep`; `python` yok → `node -e`
- **ASLA `git add -A`** (untracked kişisel notlar var)
- CI parity: `cargo fmt --all -- --check`; `cargo clippy --locked --workspace
  --all-targets --all-features --exclude osp-desktop -- -D warnings`;
  `cargo test --locked --workspace --all-features --exclude osp-desktop`
