# Handoff — #96 MD-2 TAMAMLANDI (W9 dahil) — sıradaki #95-A

**Tarih:** 2026-08-22 (oturum 8). Branch: `feat/96-md2-w9-dogfood-and-records`
(origin/main `13acfd7` — PR #126 merge commit'i üzerinden).
**Durum:** #96 MD-2 UÇTAN UCA tamamlandı — PR #125 (W1-W7 + review turları)
MERGED, PR #126 (W6/W8 acceptance) MERGED, W9 bu branch'te:
dogfood Run A rerun kanıtı + migration-decisions MD-2 implementation record +
INV-T4 spec status + #96/#103 kapanış-supersession yorumları.
**Sıradaki:** **#95-A** (MD-1 subject cutover — `subject_authority`
`"affected_nodes"` → `"task_scope"` flip; typed draft temeli #96'dan hazır)
→ #95-B → #97 → #100.

## Oturum nasıl başlamalı

> Handoff: **#95-A — MD-1 subject cutover** (`subject_authority` değerini
> `"affected_nodes"` → `"task_scope"` flip; #96 typed draft temeli hazır).
> Önce: `docs/notes/faz8-p2-migration-decisions.md` MD-1 bölümü + bu dosyanın
> W9 kaydı (dogfood rerun prosedürü #95-A rerun'unda da kullanılır).
> Branch: main üzerinden yeni branch.
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -4`, (3)
`export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test --locked --workspace
--all-features --exclude osp-desktop` (yeşil başlangıç).

## W9 kapanışı — PR #127 (Completed)

### Dogfood Run A rerun (2026-08-22) — confound kapanışı CANLI kanıt

Aynı fixture (main.rs→a,b; RemoveImport 2→1; deterministic git HEAD
`da637fbed`), gerçek analyze + navigator + mock LLM, harness mode:

**Lane adlandırması (net ayrım):**
- **MD-1 subject-observer lane'leri (V1/V2):** subject + native sources +
  downstream **PARITY** — V1 lane authority token'ın native provenance'ını,
  V2 lane aynı session'ın native `md1_shadow`'unu kullanır (re-anchor; V1
  lane artık compat [Scip;5] projeksiyonu DEĞİL).
- **MD-2 provenance observer (native/reference):** aynı value bits;
  **bilinçli** source-label farkı — native authority vs uniform-Scip
  **reference** (non-authoritative, karar ÜRETMEZ; fark intentional
  telemetry olarak kalır).

- **Regolden task (`required_source: null`)**: **Completed** (1 attempt).
  MD-1 lane'leri: subject [2] + engine-native sources
  `[TreeSitter, Placeholder, TreeSitter, Heuristic, Heuristic]` + aynı θ
  bits + downstream `Completed/AcceptAsCompleted` — hepsi PARITY.
  `provenance_authority_drift` sidecar canlı: native vs uniform-Scip
  reference, value-bits parity (construction property). Envelope:
  `provenance_authority: "engine_native_per_axis"`, `provenance_native: true`.
- **Run A senaryosu (`required_source: Scip`)**: expected semantic change
  MATERIALIZED — native TreeSitter coupling Scip şartını karşılamaz →
  `NotCompleted/Reject` (legacy uniform projeksiyonda Completed olurdu);
  MD-1 lane'leri downstream PARITY (Reject) — farkın nedeni task tanımı,
  gözlem confound'u değil. Termination: `llm_error(NoMoreProposals)` — tek
  scripted proposal tüketildi; ikinci LLM çağrısında mock kuyruğu boştu.
  **Maneuver limit exhaustion DEĞİL** (limit 3'e ulaşılmadı); tek attempt
  evidence'da iki sidecar'la kayıtlı.
- 2026-08-18 Run A kaydı (konfund kanıtı): `95-md1-cutover-handoff.md` —
  tarihsel bağ korunur; kapanış kaydı migration-decisions MD-2 bölümünde.
- Prosedür notları: fixture `C:/Users/ervol/AppData/Local/Temp/osp-md2-w9/`
  (disposable); task/proposal şablonları `crates/osp-cli/tests/completed_loop.rs`
  mirror'u; SCIP index GEREKMEDİ (tree-sitter analyzer yeterli — Run A ile aynı).

### Docs/issues
- `faz8-p2-migration-decisions.md`: MD-2 implementation record + Required
  implementation durumları (**madde 3: taxonomy SUPERSEDED — raw observation
  modeli; gerekçe supersession kaydında**) + Karar Özeti tablosu + follow-up
  listesi (#95-A sıradaki; #100 compatibility kaldırımı; #103 transferred).
- `docs/spec/invariants.md` INV-T4 MD-2 notu: planned → **implemented**.
- GitHub: #96 kapanış kaydı + #103 supersession yorumu (link'ler issue'larda).

## W6/W8 kabul kanıtı — PR #126 (merged)

### W6 — authority boundary
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

## Sıradaki

#95-A (MD-1 subject cutover) → #95-B (MD-1 cleanup) → #97 (MD-3) → #100
(Faz 8a engine cutover — MD-2 compatibility fiziksel kaldırımı dahil;
reviewer tur-6 notu: #100 öncesi V1 lane kaldırım planı ayrıca değerlendirilir).

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
