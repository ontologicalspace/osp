# Handoff — Faz 8a düzeltilmiş sıra: #96 ÖNCE (#96 detay planı v4-FİNAL APPROVED)

**Tarih:** 2026-08-18 (oturum 2: Faz 8a orchestration v1→v5 [4 tur] + #96 detay planı v1→v4 [4 tur, son tur APPROVE 9.8/10])
**Önceki oturumlar:** plan v1→v6 (5 tur) → PR #122 (P2-1, `be79675`) → handoff #123 (`f003a17`)
**Bu dosya:** Faz 8a sıralaması düzeltildi + #96 detay planı **v4-FİNAL APPROVED**. Yeni oturum
**#96 implementation** ile başlamalı. (Tarihçe: `f003a17` ilk sürüm — git geçmişinde.)

## Oturum nasıl başlamalı

Kullanıcı bu mesajı iletecek:

> Handoff: **#96 MD-2 implementation** ile devam ediyoruz (detay planı v4-FİNAL APPROVED).
> Notlar: `docs/notes/95-md1-cutover-handoff.md` — önce oku, durum kontrolü yap.

**İlk adımlar:** (1) bu dosyayı oku, (2) `git status` + `git log --oneline -5`,
(3) `gh issue view 96 --comments` — okuma zinciri: **v4-FİNAL contract → PR #124 review
addendum (binding-error bullet supersession) → PR #124 review tur-2 addendum (subject
binding)**; yalnızca bu iki bullet superseded, diğer v4-FİNAL pin'leri değişmez;
+ `gh issue view 103`,
(4) implementation branch aç (commit formatı `feat: #96 …` — scope parens YOK), W1'den başla.

**Yerel implementation branch (push EDİLMEDİ):** `feat/96-md2-native-provenance-authority`
@ `4d4d724` — W1-W4 + iki review düzeltmesi; ilerleyen oturum burdan devam eder
(durum: `docs/notes/96-implementation-handoff.md`).

## Mevcut durum

- **P2-1 merged** (`be79675`): `subject_authority.rs` additive observation + wiring + testler.
- **#95 OPEN** (Faz 8a, #96 sonrasına). **#96 OPEN** — detay planı v4-FİNAL APPROVED; sıradaki
  iş **implementation**. **#103** supersession/transferred kaydı ile kapanır. **#88** CLOSED.
- **Faz 8a orchestration v5** (4 review turu) düzeltilmiş sıra: `#96 → #95-A → #95-B → #97 → #100`
  (canonical decision doc: MD-2 "engine-internal, Faz 8a öncesi"; authority order: decision note >
  issue/handoff). Her PR yalnız **bir epistemik authority eksenini** değiştirir:
  #96 subject=affected_nodes (sabit) + provenance=native (değişir); #95-A subject=task scope
  (değişir) + provenance=sabit; #95-B MD-1 cleanup; #97 baseline/loss; #100 V2 algebra + fiziksel.

## #96 v4-FİNAL authority zinciri (onaylı — implementasyon bu şekle)

```text
DeltaProposal
  → StructurallyValidatedClaimDraft::try_new        (pub osp-core; probe Claim + Q4 STRUCTURAL tek adımda;
                                                     claim_id tek inkrement; MCP kopyalamaz — tek truth;
                                                     **tur-2 P1: legacy_subject_binding private capture** —
                                                     Claim affected_nodes TAŞIMAZ, proposal identity draft'ta)
  → engine derives legacy subject INTERNALLY        (effective_legacy_measure_set — draft×producer TEK truth:
                                                     derive_v1 ordered union; boşsa delta-ids fallback;
                                                     serbest Vec<NodeId> parametresi YOK)
  → BoundMeasurementSession (TEK session; begin atomik capture: descriptors→MeasurementInputDigest
                             VE epochs→CoreAxisEpochStamp; measurement SONRASI CoordinateSystem
                             yeniden dolaşılmaz — ikinci observation TOCTOU açar)
      ├─ native legacy-subject measurement → authority token
      ├─ MD-1 task-scope shadow material            (geçici köprü; #95-B'de silinir)
      └─ final verify_unchanged
  → NativeLegacySubjectMeasurement (opaque; ctor YALNIZ SpaceEngine)
      { measured, legacy_subject_ids, legacy_subject_binding (ids'den TÜRETİLİR — tur-2 P1),
        delta_digest: MeasurementDeltaDigest (mevcut tek canonicalization truth; affected_nodes
        İÇERMEZ — subject binding ayrı kanıt ister), base_revision, measurement_input_digest,
        axis_epoch_stamp }
      — raw() = measured.to_raw() (bağımsız alan DEĞİL; SAME value bits = construction property)
  → draft.finalize(&token) → Result                 (**tur-2 P1: legacy_subject_binding karşılaştırması —
                                                     LegacySubjectBindingMismatch; aynı structural delta +
                                                     farklı affected_nodes artifact mix'i reddedilir;
                                                     raw check bağımsız kanıt DEĞİL** — finalize raw'ı
                                                     token'dan enjekte eder; yalnız computed_raw/Intent
                                                     enjekte, structural + claim_id aynı object)
  → Q4 FINAL-RAW finite validation
  → commit_task_claim → verify_native_legacy_measurement_binding (5 kontrol: delta digest,
     computed_raw bits, current SpaceViewRevision, current descriptors, current axis epochs —
     A→B→A ABA fence) → VerifiedNativeLegacyMeasurementBinding (private proof; "stale replay
     fence" — aynı context'te meşru resubmit [Held+witness] engellenmez; "cannot be replayed"
     dili KULLANILMAZ) → Q5 → PredicateGate → Q6 → AuthorizationBasis V1 (revision/context/
     measured_result PROOF'TAN okunur — yeniden okumaz)
```

**Error funnel (PR #124 review P1 düzeltmesi — ontology):** engine-issued token hataları
**`MeasurementBindingVerificationError::NativeAuthority(NativeLegacyMeasurementBindingError)`**
typed family'sinde yaşar: `{ StructuralDeltaMismatch, LegacySubjectBindingMismatch (tur-2 P1),
RawMismatch, StaleSpaceRevision, MeasurementContextMismatch, AxisEpochMismatch }`. Mevcut
`Mismatch` (presented-authority/caller) ailesi **yeniden anlamlandırılmaz ve dokunulmaz** —
opaque token + private `TaskCommitInput` sonrası `RawMismatch` caller hatası olmaktan
çıkmıştır (invariant violation/tamper); `AxisEpochMismatch` drift-kökenlidir. Tek funnel
korunur: `EngineCommitError::MeasurementBindingVerification(…)` (paralel ontology yok).
Navigator: `NativeAuthority` → SystemFailure | budget yok | LLM retry yok; `gate_decision`
etiketi `RejectedByMeasurementBinding` DEĞİL (`Unknown`). `LegacySubjectMismatch` EKLENMEZ
(token legacy_subject_ids = audited measurement subject, canonical task authority
DEĞİL — #96 sınırı).

## #96 beyan (fixture-scoped)

1. **Provenance authority (EXPECTED):** frozen #88/#85 + dogfood Run A'daki beklenen flip'ler
   (Scip-required + native non-Scip axis → SourceInsufficient→Reject; matching TreeSitter-required
   → numeric evaluation, fixture'da Completed). Genel teorem değil.
2. **Subject DEĞİŞMEZ** (ordered legacy union, engine-internal derivation).
3. **Baseline/loss DEĞİŞMEZ** (running scalar; CLI/MCP bootstrap seed'leri değer+Scip tag ile sabit;
   MCP `current_measured()` loss_before hesaplar = gate girdisi).
4. **ULP value parity (EXPECTED):** computed_raw session-bound native; 1-ULP farkları
   reason-note'lu regolden. Half-merge YASAK.

## #96 implementation checklist (W1-W9; P1/P2'ler dahil)

- **W1:** `NativeAttemptMeasurement { authority, md1_shadow }` +
  `measure_attempt_native_with_md1_shadow(draft, proposal)` (tek session); `BoundMeasurementSession`
  `axis_epochs()`/`captured_axis_state()` accessor (atomik capture'dan); `measure_subject_in_session`
  pub(crate); singleton fast-path `measured_centroid_in_session`'a geri + `021bd5f`'ten recovered
  test; `try_compute_raw_from_delta`/`compute_raw_from_delta` refactor EDİLMEZ; md1_shadow cross-pin
  (bits+sources == mevcut `measure_task_delta().after()` stable fixture'ta).
- **W2/W3:** navigator/MCP cutover (ordering yukarıda); 17-varyant disposition tablosu LITERAL
  (RevisionMismatch/MeasurementContextDrift → SystemFailure; Digest(_) fail-closed; string parsing
  YASAK); `TaskCommitInput::new(...)` private fields (`measured` → `measurement: &token`;
  target/loss_before KALIR); MD-1 observer saf fonksiyon (NativeAttemptMeasurement material'i);
  yarış testi (nav+MCP): structural-Q4-invalid + measurement-failing → daima SyntaxViolation.
- **W4:** verification + proof + basis proof-sourcing + Q4 helper extraction + error funnel.
- **W5:** `provenance_authority.rs` — ProvenanceAuthorityDriftObservation (SAME subject/value bits;
  native=authority vs uniform-Scip=reference; üç-durum eligibility: structural-Q4 reject →
  observation YOK [precedence correction]; Q5 violated → NotReached; Predicate+Q6 fail →
  ReachedButUnavailable; Evaluated/Held/Rejected → Observed; `Q4SyntaxRejection` arm'ı YOK);
  sidecar'lar additive + digest DIŞI + identity binding iki durable wire'da;
  `legacy_compatibility_projection` reference-only buraya taşınır.
- **W6:** V1-basis persistence parity (`inv_t9_72_held_production_path_exact` native akışta +
  reload bits/sources + tamper ×2 + Null/Filesystem aynı digest + basis↔token cross-pin'leri +
  negatif stale/context/**ABA epoch** testleri). Process-independent reload #100'de.
- **W7:** CLI iki-eksen envelope (`subject_authority: "affected_nodes"`,
  `provenance_authority: "engine_native_per_axis"`, `provenance_native: true`, `authority` alias =
  provenance mirror) + banner; bootstrap seed DOKUNULMAZ; completed_loop: before pin'leri sabit,
  post-measurement pin'leri probe-then-freeze (1-ULP ancak reason-note ile).
  **Ownership (PR #124 review P2):** `subject_authority` alanı **#96'da girer** (legacy authority
  label olarak, değer `"affected_nodes"`); **#95-A yalnız değerini** `"task_scope"`a çevirir.
  `"affected_nodes"` wire değeri **authority-family label'dır, literal subject set DEĞİL** —
  gerçek V1 subject `derive_v1_legacy_measurement_subject()`'in ordered union'ıdır
  (`affected_nodes` ∪ unseen `removed_edges.from`, sıra korunur).
- **W8:** regolden + yeni testler (yarış, construction contract, cross-pin,
  commit binding verifier ×5 + legacy-subject finalize binding ×2 negative tests,
  singleton, disposition exhaustiveness, MD-2 mirror envanteri).
  Not: finalize ×2 negatif test (farklı affected_nodes; aynı raw bits + farklı
  subject → yine mismatch) tür-2 P1 ile TESLİM EDİLDİ (feat/96 @ 4d4d724); commit
  verifier ×5 negatifi W8’de gelecek. Reason-note: "provenance-driven
  (MD-2) — frozen #88/#85 + dogfood Run A; subject-set etkisi YOK".
- **W9:** docs/issues (#96 kapanış `feat: #96 …`; #103 transferred; #100 absorbe; INV-T4;
  migration-decisions; handoff refresh — #95-A sıradaki).

## #96 kanıt tabanı (plan turlarından)

- **#103 digest incident:** `AuthorizationBasisDigest` mismatch FilesystemStore reload
  (`authorization.rs:8253`); kök neden: yarı-birleşik authority modelleri (deneme MD-1+MD-2
  birleşikti — navigator `measure_task_delta` kullanmıştı = subject değişimi). Doğru #96 subject'i
  sabit tutar; navigator `measure_task_delta` ÇAĞIRMAZ. Başarısız deneme commit'leri: `021bd5f`
  (singleton fast-path — recovered edilecek) + revert `1e0f590`; navigator migration hiç commit
  edilmedi (working tree temizlendi).
- **V2 zinciri dead-code scaffolding** (`AuthorizationBasisV2`/`persist_v2`/`load_versioned`/
  `gate_v2`/`verify_measurement_binding`) → tamamı **#100'e** (EngineMeasurement task-scope token +
  smart ctor + V2 algebra). #96 bunlara DOKUNMAZ.
- **measured'nin commit etkileri:** `evaluate_completion` source-decisive (INV-T4);
  `AuthorizationBasis.measured_result` per-axis value+source digest preimage; loss/evidence
  value-only.
- **Public `TaskCommitInput.measured` forge edilebilirliği** → opaque token + private fields çözer
  (engine.rs:102-107'nin öngördüğü smart-ctor adımının #96 payı; #100 tamamlar).

## Dogfood gözlem penceresi kanıtı (2026-08-18 — #96'nın kritik girdisi)

Gerçek CLI akışı (`osp trajectory attempt`, gerçek analyze + navigator, mock LLM). **Run A —
Completed:** `subject_authority_drift` sidecar canlı: v1 subject [2], sources [Scip×5] (compat
projection), Q5 passed, downstream Observed{Completed, AcceptAsCompleted}; v2 subject [2] (digest
PARITY), sources engine-native [TreeSitter, Placeholder, TreeSitter, Heuristic, Heuristic], Q5
passed (aynı theta bits), downstream NotCompleted/Reject (SourceInsufficient — required_source=Scip).
**Canlı MD-2 confound kanıtı:** subject+θ parity'ye rağmen downstream diverge — neden provenance.
#96'nın expected semantic change beyanının doğrudan malzemesi. **Run B — Held:** D3/Commit-4
kısıtı (Ephemeral space identity + CrossProcess store) — CLI Held yüzeyi yapısal kapalı; parity
testleri `ProcessLocalFilesystemTestStore` üzerinde.

## Plan/PR dersleri (uygula)

- Conventional-commit tuzağı: issue ref'ten önce scope parens YOK (`feat: #96 …` güvenli).
- Probe-then-freeze; elle fixture sabiti YAZMA; exact snapshot dondur; enum equality;
  doğrulamada `||`/`2>/dev/null` YOK.
- CI parity ritual (exact): `cargo fmt --all -- --check`; `cargo clippy --locked --workspace
  --all-targets --all-features --exclude osp-desktop -- -D warnings`; `cargo test --locked
  --workspace --all-features --exclude osp-desktop`; + targeted `-p osp-core`/`-p osp-mcp`/
  `-p osp-cli --test completed_loop`.
- Corpus: ID-bazlı `case_by_id`. PR yazarı kendi PR'ını onaylayamaz.
- `index.scip` yeniden üretim: rust-toolchain.toml geçici taşı + `MSYS_NO_PATHCONV=1 docker run
  --rm -v "P:/Work/SoftwarePhysics:/repo" -w /repo sourcegraph/scip-rust:latest scip-rust
  --output /repo/index.scip`.

## Ortam notları (Windows)

- Shell sıfırlanınca `export PATH="$HOME/.cargo/bin:$PATH"`.
- `command grep`; `python` yok → `node -e` (temp path'ler `C:/Users/ervol/...`).
- **ASLA `git add -A`** (untracked kişisel notlar: `docs/notes/planlama-tasarım-eskiz.txt`,
  `proje-adaylari.md`, `sohbet-konu.txt`, `docs/osp-*.md`, `docs/design/`, `dump.scip`,
  `crates/osp-desktop/gen/`).
- Dogfood temp fixture: `C:/Users/ervol/AppData/Local/Temp/osp-md1-dogfood/` (disposable).

## Sıradaki iş önerisi

1. **#96 implementation** (W1→W9; bu dosyadaki zincir + checklist).
2. #95-A (MD-1 subject cutover — typed draft temeli #96'dan hazır), #95-B (MD-1 cleanup).
3. #97 (MD-3), sonra #100. Ara iş: #110 MSRV.
