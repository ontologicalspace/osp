# Handoff — #96 MD-2 PR #125 (review tur-6 sonrası durum)

**Tarih:** 2026-08-21 (oturum 5 sonu). Branch: `feat/96-md2-native-provenance-authority`.
**Implementation head:** `b703370`'dan sonraki kod commit'i (tur-6 düzeltmeleri —
`git log --oneline -4` ile gör; bu dosyayı güncelleyen docs commit'i head'den
YENİ olabilir, SHA assert YOK). CI parity komutları yerelde yeşil
(fmt + clippy `-D warnings` + test workspace).

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 PR #125 — W6/W8 acceptance suite'i tamamla** (merge öncesi
> tek kalan engel; review tur-6'nın diğer maddeleri kapandı).
> Branch: `feat/96-md2-native-provenance-authority`.
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -4` (docs-sync commit'i
head olabilir — SHA beklemesi YAPMA), (3) `export PATH="$HOME/.cargo/bin:$PATH"`
+ `cargo test --locked --workspace --all-features --exclude osp-desktop`
(yeşil başlangıç).

## Review tur durumu (tur-6, REQUEST CHANGES: 2 P1 + 2 P2)

| Bulgu | Öncelik | Durum |
|---|---|---|
| V1 truth-surface — CommitReached ile production reachability fabrication'ı | P1 | **KAPANDI (tur-6)** — `PipelineObservation::ReferenceEvaluation` variantı; `CommitReached` yalnız gerçek `commit_task_claim` yüzeyinde |
| W6/W8 acceptance suite eksik | P1 | **AÇIK — merge öncesi tek engel** (tur-5'ten devir, değişmedi) |
| Navigator commit-error sınıflandırması shared mapper'ı tüketmiyor (iki truth) | P2 | **KAPANDI (tur-6)** — navigator `commit_error_agent_surface()` ile sınıflandırır; inner match yalnızca payload/mesaj çıkarımı |
| Handoff head talimatı kendi commit'iyle stale oluyor | P2 | **KAPANDI (tur-6)** — SHA assert kaldırıldı (bu dosya) |

Önceki turların kapanışları için PR #125 body'sine bakın (tur-4 P0 ×2, tur-5
P1×2 + P2×3 — hepsi kapandı ve tur-6 review'i onayladı).

## tur-5 (b703370) + tur-6 (bu oturum) yapılanlar

### P1 — V1 honest-encoding (truth-surface; tur-5 + tur-6 iki adımda kapandı)
- tur-5: `tests/common/mod.rs` V1 evaluator sentetik `EngineCommitResult` ÜRETMEZ
  artık; `Q5Observation::NotObserved` additive variant'ı geldi.
- tur-6: sonuç artık `PipelineObservation::ReferenceEvaluation` variant'ında —
  **`CommitReached` ("commit_task_claim tam çalıştı") yalnız gerçek commit
  yüzeyinde kullanılır**; V1 reference evaluation production reachability
  iddiası yapmaz (Q5/Q6/witness/TaskValidation gözlenmez — variant'ın kendisi
  bunu belirtir). Karar alanları (predicate/mutation/apply) gerçek gate çıktısı.
- Parity test pin'leri (`matching`/`wide-affected`/`removed-edge`/`delta-introduced`
  ×2/`counter`/`mixed`/`q92` helper) ReferenceEvaluation'a re-pinlendi; parity
  helper'ları `decision_surface()` extractor'ı ile iki variant'tan da karar
  yüzeyini karşılaştırır.

### P1 — MCP failure ontology (ortak typed mapper; tur-5 + tur-6)
- `osp-core/src/task_measurement.rs`: `NativeFailureSurface` enum
  (SyntaxRejection / RetryAgentProposal / SystemFailure / TaskNotFound /
  WitnessEvaluationError) + `MeasurementFailureDisposition::agent_surface()` +
  `commit_error_agent_surface()` (EngineCommitError 14 varyant exhaustive).
- **tur-6:** navigator commit-error üretim arm'ı da `commit_error_agent_surface()`
  ile sınıflandırır (inner match yalnızca payload/mesaj çıkarımı — mesajlar
  birebir korundu). Navigator + MCP tek tabloyu tüketir: iki truth YOK.
- `osp-mcp/server.rs`: measurement failure → `system_failure` JSON (class + typed
  disposition + retryable:false), `attempt_outcome` YOK (fabrication kapandı);
  finalize binding mismatch → `system_failure`; retryable commit error → GERÇEK
  gate kararı (`gate_decision_from_engine_error`); gerçek structural Q4 (draft)
  ve raw-finite `RejectedBySyntax` KALIR (navigator parity bilinçli).
- Pin: `osp-mcp/tests/native_failure_ontology.rs` (TaskBindingMismatch fixture —
  `task_id=999` ≠ `task.id=1`) + osp-core mapper tablo test'leri.

### P2 — Sidecar diagnostic + accessor + private ctor (tur-5)
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
