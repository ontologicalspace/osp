# Handoff — Faz 8a düzeltilmiş sıra: #96 ÖNCE (plan v5 approved)

**Tarih:** 2026-08-18 (oturum 2: Faz 8a orchestration planı v1→v5, 4 review turu, APPROVED)
**Önceki oturumlar:** plan v1→v6 (5 tur) → PR #122 (P2-1, `be79675`) → handoff #123 (`f003a17`)
**Bu dosya:** Faz 8a sıralaması düzeltildi + plan v5 sözleşmeleri. Yeni oturum **#96 MD-2
detay plan turu** ile başlamalı. (Tarihçe: `f003a17`'teki önceki sürüm — git geçmişinde.)

## Oturum nasıl başlamalı

Kullanıcı bu mesajı iletecek:

> Handoff: **#96 MD-2 detay plan turu** ile devam ediyoruz (Faz 8a sıralaması düzeltildi).
> Notlar: `docs/notes/95-md1-cutover-handoff.md` — önce oku, durum kontrolü yap, plan moduna gir.

**İlk adımlar:** (1) bu dosyayı oku, (2) `git status` + `git log --oneline -5`,
(3) `gh issue view 96` (+ `--comments`: supersession kaydı + #103 subtask digest incident),
(4) plan modunda **#96 D-A/D-C/D-E + reference-lane detay planı** hazırla (kanıt tabanı
aşağıda + `docs/notes/faz8-p2-migration-decisions.md` MD-2 section).

## Mevcut durum

- **P2-1 merged** (`be79675`): `subject_authority.rs` additive compatibility observation +
  navigator/MCP wiring + wire sidecar'ları + testler. **#95 OPEN** (Faz 8a, #96 sonrasına).
- **Faz 8a orchestration planı v5 APPROVED** (4 review turu): sıralama düzeltildi —
  **#95→#96 DEĞİL, #96→#95**. Gerekçe: canonical migration decision MD-2'yi "engine-internal,
  Faz 8a öncesi" tanımlıyor; authority order: decision note > issue/handoff. Önceki
  handoff'un kritik yol satırı bu gerekçeyle düzeltildi.
- **#96 OPEN** (MD-2 provenance authority) — sıradaki iş: detay plan turu (D-A/D-C/D-E).
  **#103** (digest parity subtask) OPEN, #96'nın girdisi. **#88** CLOSED (kanıt hazır).

## Onaylı sıra (plan v5)

```
#96   provenance authority değişir (subject aynı, baseline/loss aynı)
#95-A subject authority değişir    (provenance aynı, baseline/loss aynı)
#95-B yalnız MD-1 gözlem/compat yüzeyi silinir
#97   baseline/loss semantic policy değişir
#100  V2 decision algebra + smart ctor/legacy field + fiziksel cleanup
```

Her PR yalnız **bir epistemik authority eksenini** değiştirir → bir golden değiştiğinde
nedeni söylenebilir (subject mi, provenance mı, baseline mı — karışmaz).

## Plan v5'in dondurulmuş sözleşmeleri

1. **#96 yalnız `measured → measurement` authority migration.** `TaskCommitInput`
   `{ claim, omega, task_resolver, measurement: EngineMeasurement, target, loss_before }` —
   target/loss_before KALIR (semantik authority #97; **fiziksel** removal #100 — #97'ye
   önceden atanmaz: #97 baseline/policy issue'su, #100 smart ctor + legacy field cleanup
   scope'unda tutuyor).
2. **Q4 structural/raw ayrımı (#95-A):** `check_claim_syntax` = `check_claim_structure` +
   `check_raw_position_finite(computed_raw)`. Shared boundary: probe Claim → **Q4
   structural** → task binding → `Task::validate_for_commit` → revision →
   `measure_task_delta` → **Q4 final-raw finite** → final Claim → `commit_task_claim`
   (structural + final-raw defensive repeat; Q5→Predicate→Q6→witness değişmez).
   Structural Q4 → measurement sırası KORUNUR. Yarış acceptance testi (navigator + MCP):
   structural-Q4-invalid proposal + measurement-failing task aynı fixture'ta → sonuç
   daima `SyntaxViolation`, measurement error asla gözlemlenmez.
3. **MeasurementFailureDisposition matrisi (17 varyant exact, wildcard YOK):**
   `ClaimNotTaskBound`/`TaskBindingMismatch` → TerminalIdentityViolation;
   `HeterogeneousPredicateScopes`/`EmptySubjectScope`/`SubjectScopeResolutionFailed`/
   `SubjectMemberUnresolvable` → TerminalTaskDeclaration (agent `NewNodeSpec`'te ID seçemez
   — builder `10_000+index` atar); **geri kalan 10 varyant → SystemFailure** (`RevisionMismatch`
   dahil: atomik baseline-refresh kontratı (rev+baseline+loss_before-context+same
   proposal+bounded) tasarlanmadan RegenerateMeasurement YOK; `MeasurementContextDrift` =
   interior-mutability/TCB instability; `Digest(_)` fail-closed — StructuralCanonicalization
   hem agent-delta hem task-scope kaynaklı üretebiliyor, `detail:String` ayıramaz, **string
   parsing YASAK**, typed `StructuralCanonicalizationOrigin` sonrası agent-shape arm'ı
   unlock; `SubjectMemberMissingAfterDelta`: grammar'da node silme yok). *
   `RetryAgentProposal` + `RegenerateMeasurement` bugün üreticisiz — vocabulary olarak
   kalır (repo prejededi: `DecisionDriftClass`); unlock koşulları tabloda belgeli.
4. **CLI iki-eksen vocabulary:** `subject_authority` ("affected_nodes" → #95-A'da
   `"task_scope"`) + `provenance_authority` (#96'da `"engine_native_per_axis"`) +
   `provenance_native` (#96'da `true`). Eski `authority` alanı deprecated alias —
   **yalnız `provenance_authority`'yi mirror eder** (pre-#96 `legacy_projected_v1` →
   post-#96…#100 `engine_native_per_axis`; **#95-A değerine dokunmaz**). #100'e kadar kalır.
5. **MD-2 reference lane:** aynı `EngineMeasurement` token — native `after()` = mutation
   authority; **aynı değerlerin** uniform-Scip projection = MD-2 reference telemetry
   (`LegacySubjectMeasurement`'dan bağımsız; projection fonksiyonu MD-2 observation modülüne
   taşınır). #95-A task-scope'a geçince reference lane otomatik yeni subject'i izler; #95-B
   MD-1 modülünü güvenle siler. Fiziksel kaldırma #100. (#96 supersession kaydı issue'ya
   yazıldı + body sync edildi.)
6. **Wire üç-epoch sözleşme (#95-B):** typed `TransientMigrationEpochRejected` **yalnız
   durable wires**: `PendingAuthorization` + `RevisionRequired` (presence-aware custom
   Deserialize; missing-vs-null ayrımı; `"subject_authority_drift": null` VE dolu
   observation şekilleri ikisi de typed reject; generic unknown-field DEĞİL — migration-epoch
   mesajı). `TrajectoryEvidence` kapsam DIŞI — outbound/untrusted telemetry; alan silindikten
   sonra transient `null` unknown-field olarak sessizce tolere edilir (bilinçli, dokümante).
   **Epoch guard:** garanti yalnız P2-1 hiçbir durable release boundary'ye girmemişken
   geçerli; release/tag/gerçek persisted deployment oluşursa **#95-B'den ÖNCE migration
   reader** gerekir.
7. **#99 kapanış ledger'ı:** MD-1 compatibility producer → **#122 delivered**; MD-2 dual
   evaluation → **#96 transferred**; checked boundary → **#95-A delivered** → #99 complete.
   (Ön-ledger comment'i issue'ya yazıldı.)
8. **#95-A kaynar noktaları:** `loss_before` init (navigator.rs:631) + progress update
   (:1132) DOKUNULMAZ; `measured = token.after()` native (#96 sonrası meşru — bridge YOK);
   `build_claim_from_proposal` → `measurement/task_measurement.rs` neutral evi (MCP→navigator
   bağımlılığı dönmesin); `OutputContract.validate` navigator-only agent-shell preflight
   (MCP'ye preflight eklenmez); checked boundary typed carrier `CheckedTaskMeasurement`;
   `affected_nodes` şemada kalır, prompt dili advisory.

## #96 kanıt tabanı (bu oturumda toplandı)

- **#88 CLOSED** (mixed_per_axis_sources matrisi frozen); PR #85 2/5 required_source
  divergence; dogfood Run A canlı confound (aşağıda).
- **#103 digest incident:** `AuthorizationBasisDigest` mismatch FilesystemStore reload
  (`authorization.rs:8253` `BasisDigestMismatch`; `inv_t9_72_held_production_path_exact`
  navigator.rs:3226 — `ProcessLocalFilesystemTestStore` reload zinciri). Kök neden:
  `TaskCommitInput` legacy ↔ `EngineMeasurement` native **yarı-birleşik** authority
  modelleri → persistence wire divergence. Çözüm: **atomik** `measured → measurement`
  migration + native basis + persistence wire version + reload verification. Başarısız
  deneme: unmerged `faz8-test-project/completed-loop` branch'i (revert `021bd5f`+`1e0f590`).
- **Hazır scaffolding:** `persist_v2`/`load_versioned` (authorization.rs:8886-8915) +
  `AuthorizationBasisV2` (:2730) + `gate_v2::compute_completion_first_loss_and_decision` —
  hepsi `#[allow(dead_code, "Faz 8a navigator consumer")]`.
- **measured'nin commit etkileri:** (a) `evaluate_completion` source-decisive (INV-T4:
  SourceInsufficient → her zaman Reject), (b) `AuthorizationBasis.measured_result`
  per-axis value+source digest preimage (`authorization.rs:3433-3439`); loss/evidence
  value-only (etkilenmez).
- **D-A (tasarım sorusu):** legacy-subject native producer — `measured_centroid_of`
  `pub(crate)`+generic (engine.rs:2745, space+member_ids); `try_compute_raw_from_delta`
  native ölçüp source'ları `to_raw()` ile ATIOR; public native yüzey şekli VEYA
  EngineMeasurement'ın legacy-subject üretimi; `EngineMeasurement::new` single-producer
  kontratının genelleşmesi (`tests/engine_measurement_single_producer.rs`). **#96 subject
  DEĞİŞTIREMEZ** (izolasyon #96 içinde de geçerli).
- **D-C:** #103 checklist 6 madde (native→Held→persist→reload exact digest parity; 5-axis
  bits/sources exact; value/source tamper fail-closed ×2; Null/Filesystem aynı digest) +
  `AuthorizationBasisV2`/`persist_v2` wiring.
- **D-D:** iki-eksen CLI envelope (Bölüm "sözleşmeler" madde 4) + `run_navigator`
  hardcoded `current_measured` (commands/mod.rs:763-772) → engine-derived.
- **D-E:** singleton centroid fast-path ULP notu (V1 authorization digest'i etkiledi —
  dikkat; `measured_centroid_in_session`'a geri ekleme fence'i).
- **Beyan:** #96'nın expected semantic change'i = required_source karar flip'leri (frozen
  matrislerle birebir: Scip-required → SourceInsufficient→Reject; TreeSitter-required →
  Completed).

## Dogfood gözlem penceresi kanıtı (2026-08-18, mini — #96'nın kritik girdisi)

Gerçek CLI akışı (`osp trajectory attempt`, gerçek analyze + navigator, mock LLM):

**Run A — Completed (harness + auto-approve):** evidence kaydında `subject_authority_drift`
sidecar canlı görüldü:

```text
v1: subject [2], sources [Scip×5] (compatibility projection), Q5 passed,
    downstream Observed{Completed, AcceptAsCompleted}   ← authoritative, gerçek
v2: subject [2] (digest PARITY), sources engine-native
    [TreeSitter, Placeholder, TreeSitter, Heuristic, Heuristic],
    Q5 passed (aynı theta bits), downstream NotCompleted/Reject
    (SourceInsufficient — required_source=Scip karşılanamıyor)
```

**Bu, gerçek üretim akışında canlı MD-2 confound kanıtıdır:** subject ve θ parity
olmasına rağmen downstream V1/V2 arasında diverge ediyor — neden provenance (MD-2),
subject authority DEĞİL. #96'nın expected semantic change beyanının doğrudan malzemesi.

**Run B — Held (production witness): BAŞARISIZ, tasarım gereği:** CLI
`FilesystemPendingAuthorizationStore` (CrossProcess) + engine hâlâ `Ephemeral`
space identity üretiyor (persisted identity lifecycle "Commit 4" — engine.rs:2283;
INV-T9 #72 D3 kuralı fail-closed). **CLI'de Held yüzeyi bugün structurally kapalı**
(bizim regresyonumuz değil; completed_loop'da da Held e2e testi yok). Held kapsamı:
navigator unit (`inv_t9_72_held_production_path_exact` — #96'nın digest-parity testi de
bu yüzeyde) + MCP e2e.

## P2-1'den devralınan varlıklar

1. **`crates/osp-core/src/subject_authority.rs`** — MD-1 compatibility semantics (observer,
   producer üçlüsü, eligibility, `v1_downstream_from_engine_commit_error`). #95-B'de
   tamamen silinir; uniform-Scip projection kısmı #96'da MD-2 reference modülüne taşınır.
2. **Taşıyıcılar:** `TrajectoryEvidence.subject_authority_drift` (outbound telemetry),
   `PendingAuthorization.subject_authority_drift` (identity-bound), 
   `RevisionRequired.try_with_subject_authority_drift`. Digest preimage'lerine girmez.
3. **Test envanteri:** `tests/subject_authority_drift_observation.rs` (8), navigator md1
   (2), authorization wire/identity (3), MCP sidecar e2e (3), engine axis TCB (2 — #95-B'de
   `measure_task_delta`'ya re-anchor), modül unit (8). Corpus'a case eklenmedi.

## Plan/PR dersleri (birikmiş — uygula)

- **Conventional-commit + issue ref TUZAĞI:** issue referansından önce scope parantezi
  KULLANMA — `feat: #95 …` güvenli, `feat(core): #95 …` değil (squash merge'de closing
  keyword olarak yorumlanıyor; #95 yanlışlıkla kapanmıştı).
- Plan disiplini: 3-5 review turu normal; production SEMANTIC değişikliği yoksa açıkça
  söyle; refactor'ları işaretle; elle fixture sabiti YAZMA (probe-then-freeze); exact
  snapshot dondur; enum equality (debug-string değil); doğrulama komutlarında
  `||`/`2>/dev/null` YOK.
- CI parity ritual (exact): `cargo fmt --all -- --check`; `cargo clippy --locked
  --workspace --all-targets --all-features --exclude osp-desktop -- -D warnings`;
  `cargo test --locked --workspace --all-features --exclude osp-desktop`; + targeted
  suites (`-p osp-core`, `-p osp-mcp`, `-p osp-cli --test completed_loop`).
- Corpus'a case eklerken ID-bazlı `case_by_id`; class find DEĞİL.
- GitHub PR yazarı kendi PR'ını onaylayamaz.
- `index.scip` (16.4 MB) repo kökünde lokal; yeniden üretim: rust-toolchain.toml'u geçici
  taşı + `MSYS_NO_PATHCONV=1 docker run --rm -v "P:/Work/SoftwarePhysics:/repo" -w /repo
  sourcegraph/scip-rust:latest scip-rust --output /repo/index.scip`.

## Ortam notları (Windows)

- Shell her sıfırlanmada `export PATH="$HOME/.cargo/bin:$PATH"` gerekebilir.
- `grep` ZCode function'ı — `command grep` kullan.
- `python` yok; JSON için `node -e` (temp path'ler `C:/Users/ervol/...` formatında).
- **ASLA `git add -A`** — kullanıcı untracked kişisel notları var
  (`docs/notes/planlama-tasarım-eskiz.txt`, `proje-adaylari.md`, `sohbet-konu.txt`,
  `docs/osp-*.md`, `docs/design/`, `dump.scip`, `crates/osp-desktop/gen/`).
- Dogfood temp fixture: `C:/Users/ervol/AppData/Local/Temp/osp-md1-dogfood/`
  (silinmeye hazır; task/proposal/repo şablonları `crates/osp-cli/tests/completed_loop.rs`
  mirror'idir).

## Sıradaki iş önerisi

1. **#96 MD-2 detay plan turu** — D-A/D-C/D-E + reference-lane + dual evaluation
   tasarımı; bu dosyadaki kanıt tabanı + `faz8-p2-migration-decisions.md` MD-2 section
   temelinde. Kendi sıkı review'u olacak.
2. #96 implementasyonu (atomik measured→measurement + digest parity + CLI iki-eksen).
3. #95-A (MD-1 caller cutover — plan v5 Bölüm 4 hazır), #95-B (MD-1 cleanup).
4. Sonra #97 (MD-3). Ara iş: #110 MSRV.
