# Handoff — #96 MD-2 (PR #125 MERGED sonrası; W6/W8 bu branch'te tamamlandı)

**Tarih:** 2026-08-22 (oturum 6). Branch: `feat/96-md2-w6-w8-acceptance-suite`
(origin/main `f1f0c09` — PR #125 merge commit'i üzerinden).
**Durum:** PR #125 (W1-W7 + review tur-4/5/6) MERGED. W6/W8 acceptance suite
BU BRANCH'te tamamlandı — CI parity yeşil (fmt + clippy `-D warnings` + 41 test
suite). Sonraki adım: bu branch'in PR'i + review; ardından W9.

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 — W6/W8 PR'i review'da; W9 (dogfood Run A rerun + docs +
> migration-decisions MD-2 record) sıradaki**.
> Branch: `feat/96-md2-w6-w8-acceptance-suite` (PR açık).
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -4`, (3)
`export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test --locked --workspace
--all-features --exclude osp-desktop` (41 suite yeşil başlangıç).

## W6/W8 envanteri (bu branch'te eklenen test'ler)

### W6 — authority boundary kabul kanıtı
- **trybuild compile-fail ×4** (`tests/md2_authority_typelevel.rs` +
  `tests/compile_fail/md2_*.rs`): carrier elle kurulum (private ctor + struct
  literal), kaldırılan `new_characterization_legacy` (P0-1 fence — geri
  gelirse derleme hatası), `TaskCommitInput` artifact-mix şekli (P0-2 fence).
- **Verifier ×5 negatif** (engine.rs unit): StructuralDeltaMismatch (çapraz
  delta), RawMismatch (aynı delta + farklı space içeriği), StaleSpaceRevision
  (space mutasyonu sonrası replay — hem public verifier hem GERÇEK commit
  funnel), MeasurementContextMismatch (W6DeferredAxis sentinel — ölçüm SONRASI
  descriptor değişimi), AxisEpochMismatch (A→B→A: descriptor sabit + epoch
  monoton — digest fence'in göremediği revert).
- **Basis ↔ token cross-pin**: Held arm AuthorizationContext basis'i
  (revision + input digest + measured) token'ın taşıdığı değerlerle birebir —
  proof'tan okuma (yeniden ölçüm yok) pinlendi.

### W8 — observer envanteri + yarış + exhaustiveness
- **17-varyant disposition tablo pin'i** (task_measurement.rs):
  `measurement_failure_disposition_full_inventory` — frozen plan v5 tablosu +
  dormant producer notu (RetryAgentProposal/RegenerateMeasurement üreticisiz).
- **MCP Q4-vs-measurement yarış**: self-import + task-binding-mismatch aynı
  proposalda → draft Q4 kazanır (measurement system_failure YOK) — navigator
  kabul kriterinin MCP mirror'ı.
- **NativeFailureSurface wire pin'leri** (native_failure_ontology.rs):
  NativeMeasurementFailed (TaskBindingMismatch e2e) + EngineCommitFailed
  (Mixed required_source → commit TaskValidation e2e) — attempt_outcome
  üretilmez (fabrication yok). Not: NativeBindingFailed MCP yüzeyinden bugün
  erişilemez (draft×token aynı proposaldan — dormant; arm server.rs'de mevcut).
- **Provenance sidecar identity-mismatch load-path**: `validate_internal` →
  `ProvenanceAuthorityDriftIdentityMismatch` (kendi typed varyantı; subject
  adı kullanılmaz) + strict wire deserialize reject.

## Kalan (W9 — plan v5)

1. Dogfood Run A rerun (native authority ile; fixture regolden'ler PR #125'te).
2. docs/issues + migration-decisions MD-2 record.
3. (Opsiyonel, reviewer tur-6 notu) `#100` öncesi V1 lane kaldırım planı.

## PR #125'te (merged) kapananlar — özet

- **tur-4 P0 ×2:** authority-token forgeability (`new_characterization_legacy`
  kaldırıldı) + finalize bypass (sealed `FinalizedNativeTaskClaim` carrier).
- **tur-5 + tur-6 P1:** V1 truth-surface (`PipelineObservation::ReferenceEvaluation`;
  gözlenmeyen yüzey fabrication YOK) + MCP/navigator ortak failure ontology
  (`NativeFailureSurface` shared mapper — iki truth yok).
- **tur-5 P2 ×3:** ProvenanceAuthorityDriftIdentityMismatch + accessor + private
  ctor. Detay: PR #125 body + eski handoff revizyonları (git history).

## Ortam notları (Windows)

- `export PATH="$HOME/.cargo/bin:$PATH"` her shell'de
- `command grep`; `python` yok → `node -e`
- **ASLA `git add -A`** (untracked kişisel notlar var)
- CI parity: `cargo fmt --all -- --check`; `cargo clippy --locked --workspace
  --all-targets --all-features --exclude osp-desktop -- -D warnings`;
  `cargo test --locked --workspace --all-features --exclude osp-desktop`
