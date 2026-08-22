# Handoff — #96 MD-2 (W6/W8 PR #126 APPROVE — merge sonrası W9)

**Tarih:** 2026-08-22 (oturum 7). Branch: `feat/96-md2-w6-w8-acceptance-suite`
(PR #126; origin/main `f1f0c09` — PR #125 merge commit'i üzerinden).
**Durum:** PR #125 (W1-W7 + review tur-4/5/6) MERGED. PR #126 (W6/W8) review
tur-8 **APPROVE** aldı (0 P0/P1; 2 P2 docs-sync bu commit'le kapanıyor).
CI parity yeşil (fmt + clippy `-D warnings` + 41 test suite).
**Sonraki adım:** PR #126 merge → **W9** (dogfood Run A rerun + docs/issues +
migration-decisions MD-2 record).

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 — W9: dogfood Run A rerun + docs/issues +
> migration-decisions MD-2 record** (W6/W8 PR #126 merge sonrası tek kalan).
> Branch: main üzerinden yeni branch (PR #126 merge'ünden sonra).
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -4`, (3)
`export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test --locked --workspace
--all-features --exclude osp-desktop` (41 suite yeşil başlangıç).

## W6/W8 envanteri (bu branch'te eklenen test'ler)

### W6 — authority boundary kabul kanıtı
- **trybuild compile-fail ×6** (`tests/md2_authority_typelevel.rs` +
  `tests/compile_fail/md2_*.rs`): carrier elle kurulum (private ctor + struct
  literal), kaldırılan `new_characterization_legacy` (P0-1 fence — geri
  gelirse derleme hatası), `TaskCommitInput` artifact-mix şekli (P0-2 fence),
  **mevcut `NativeLegacySubjectMeasurement::new` pub(crate) pin'i + token
  struct literal** (review tur-7 P1 — `pub` genişletilirse yalnız bu fixture
  yakalar; diğerleri yeşil kalırdı).
- **Verifier ×5 negatif** (engine.rs unit): StructuralDeltaMismatch (çapraz
  delta), RawMismatch (aynı delta + farklı space içeriği), StaleSpaceRevision
  (space mutasyonu sonrası replay — hem public verifier hem GERÇEK commit
  funnel), MeasurementContextMismatch (W6DeferredAxis sentinel — ölçüm SONRASI
  descriptor değişimi), **AxisEpochMismatch (gerçek A→B→A: descriptor
  A→B→A döner; verify ÖNCESİ current context digest == ölçüm anı digest'i
  ayrıca assert edilir — digest fence'in kör olduğu KANITLANIR, yalnız monoton
  epoch yakalar; review tur-7 P2)**.
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
- **NativeFailureSurface wire pin'leri** (native_failure_ontology.rs +
  task_measurement.rs; review tur-7 P2 kapanışı):
  - `NativeFailureWireShape` contract (5 surface → 5 wire ailesi, 5/5 table
    test) — bir arm'ın JSON anlamının sessiz drift'i bu pinle yakalanır
    (compiler yalnız yeni variant yakalar). Rol notu: MCP producer henüz
    tüketmiyor (bilinçli — consume/#100 sürecinde karar).
  - NativeMeasurementFailed (TaskBindingMismatch e2e) + EngineCommitFailed
    (Mixed required_source → commit TaskValidation e2e) — attempt_outcome
    üretilmez (fabrication yok).
  - **Retryable aile — GERÇEK gate kararı**: Q6 RuleViolation e2e
    (`RejectedByRule` + system_failure YOK + surviving sidecar'lar) +
    osp-core VisionViolation→RejectedByVision / RuleViolation→RejectedByRule
    mapping pin'leri (eski hardcode RejectedBySyntax fabrication'ının
    negatifi). TaskNotFound/WitnessEvaluationError submit yüzeyinden
    unreachable (dormant notları tabloda).
  - Not: NativeBindingFailed MCP yüzeyinden bugün erişilemez (draft×token
    aynı proposaldan — dormant; arm server.rs'de mevcut).
- **Sidecar identity-mismatch load-path — TAM simetri** (subject/provenance ×
  PendingAuthorization/RevisionRequired dört yol): Pending subject + provenance
  (`ProvenanceAuthorityDriftIdentityMismatch` — kendi typed varyantı; strict
  wire reject) + RevisionRequired subject + **provenance mirror** (review
  tur-7 önerisi: checked builder `DriftSidecarIdentityMismatch` + strict wire).

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
