# Handoff — #96 MD-2 Implementation (branch: feat/96-md2-native-provenance-authority)

**Tarih:** 2026-08-18 (oturum 2 sonu). Plan v4-FİNAL APPROVED — W1-W4 kodlandı.
**Bu dosya:** Implementation durum devri. Kalan iş + 3 test failure teşhisi + debug planı.

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 implementation** ile devam (branch `feat/96-md2-native-provenance-authority`;
> W1-W4 commit'li: `0e31cd3` + `caefc73`).
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku, durum kontrolü yap.

İlk adımlar: (1) bu dosya, (2) `git log --oneline -3` (caefc73+), (3)
`export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test -p osp-core --lib` (1294 pass /
3 fail — aşağıda teşhis), (4) kalan 3 failure'ı çöz → W5'ten devam et.

## Tamamlanan (W1-W4)

- **W1** (`0e31cd3`): `NativeLegacySubjectMeasurement` opaque token (measurement.rs) +
  `CoreAxisEpochStamp`/`axis_epochs()` (coords.rs, atomik capture) +
  `measure_attempt_native_with_md1_shadow` (engine.rs — TEK session; legacy subject
  engine-internal; md1_shadow aynı session'dan) + singleton fast-path (recovered) +
  `task_measurement.rs` (draft + validate_claim_structure/raw_finite + disposition).
  Testler: singleton bit-parity, md1_shadow cross-pin (measure_task_delta.after ile
  bits+sources EXACT), legacy union construction contract (Case 3 [1,9] + fallback 10_000).
- **W4** (`caefc73`): `verify_native_legacy_measurement_binding` (5 kontrol) +
  `VerifiedNativeLegacyMeasurementBinding` proof + `TaskCommitInput` private/new +
  basis proof-sourcing (revision/context/measured_result proof'tan) +
  `MeasurementBindingMismatch`+RawMismatch+AxisEpochMismatch (funnel
  `MeasurementBindingVerification`; disposition: AxisEpoch→Regenerate, Raw→Reject).
  `canonical_structural_delta_from_claim` pub yapıldı; `compute_measurement_input_digest`
  pub(crate).
- **W2/W3** (`caefc73`): navigator + MCP cutover (draft→producer→finalize; structural Q4
  önce; disposition 17 varyant exact; loss_before/target/bootstrap DOKUNULMADI).
  MD-1 observer re-anchor (V1=native authority; V2=md1_shadow; observe'ın kendi
  measure_task_delta çağrısı SİLİNDİ). Axis TCB producer'a taşındı.
- **Test göçü:** `characterization_native_token` (common/mod.rs + navigator/engine
  crate-internal helper'lar; `new_characterization_legacy` doc-hidden pub ctor —
  V1 reference lane'in survival'ı; **PR açıklamasında reviewer'a explicitly belirtilmeli**).
  Fixture düzeltmeleri: navigator fixture coupling axis **Placeholder→Scip**
  (coupling_task `required_source=Some(Scip)` — native Placeholder dürüstçe
  SourceInsufficient→Reject üretiyordu = #96'nın expected flip'i, dogfood Run A ile
  aynı; loop-mekaniği testleri Scip axis ile onurlanır), `make_engine` axis'siz
  CoordinateSystem'dan gerçek axis'lere (legacy sessiz-default kaldı), md1_completed
  V1 sources native regolden `[Scip, Placeholder, Scip, Heuristic, Heuristic]`,
  engine fixture `measured.v 0.0 vs claim.v 0.5` tutarsızlığı (RawMismatch fence'in
  yakaladığı gerçek fixture hatası) düzeltildi.
- **Test durumu:** osp-core lib **1294/1297 PASS**. Workspace lib derliyor (osp-mcp dahil).
  Integration testler (tests/) henük KAÇIRILMADI — `cargo test -p osp-core` tam paket
  çalıştırılmalı (parity + drift + authorization testlerinde regolden beklenebilir).

## Kalan 3 test failure (teşhisli)

1. **`navigator_accepts_progress_checkpoint` + `navigator_records_evidence_per_attempt`**
   (evidence boş): make_engine tabanlı boş-space fixture'ları — gerçek ölçümle akış
   early-exit ediyor (muhtemelen tek-proposal → Completed→Held terminal veya vision
   authority yüzeyi; assert'e `{result:?}` ekleyip sonucu gör). Çözüm: fixture'ı
   g2c3 pattern'ine taşı (make_balanced_engine + incremental proposals + progress
   policy) VEYA beklentiyi yeni dürüst akışa regolden et (reason note ile).
2. **`inv_t9_72_held_production_path_exact`** (reload `BasisDigestMismatch`) — **W6'nın
   ana yüzeyi**: in-memory persist verify GEÇİYOR, reload verify recomputesi uyuşmuyor.
   Bu tam #103 incident yüzeyi — ama bu sefer BENİM basis değişikliklerimle.
   **Debug planı:** (a) persist sırasında `record.authorization_basis_digest` ile
   reload-sonrası recompute digest'i yazdır; (b) basis field'larını serialize öncesi
   vs deserialize sonrası field-wise karşılaştır (öncelikli şüpheliler:
   `measurement_input_digest` [token context'ten — session descriptors vs eski
   try_from(coord_system) üretimi; VALUE değişmiş olabilir ama internal consistency
   bozulmamalı], `base_space_view_revision` [token'tan — Ephemeral(0) sequence],
   `measured_result` f64 round-trip, `witness_snapshot support: -0.0` [serde "-0.0"
   round-trip + canonical encoding normalize farkı]); (c) serialize_envelope_v2_json
   DEĞİL — V1 pretty JSON yolu. Not: in-memory verify → persist aynı basis üzerinde
   tutarlı; fail yalnızca wire→domain restore'da → deserialize kaybı/aradaki encoding
   asymmetry'si ara.
   **W6 kapsam notu:** bu test native akışta geçtiğinde (a)-(g) cross-pin'leri +
   negatif stale/context/ABA testleri eklenecek (plan W6).

## Kalan workstream'ler (plan v4-FİNAL sırası)

- **W5:** `provenance_authority.rs` — `ProvenanceAuthorityDriftObservation` (SAME
  subject/value bits; native=authority vs uniform-Scip reference projection —
  `legacy_compatibility_projection` bu modüle taşınır; üç-durum eligibility;
  Q4SyntaxRejection arm YOK; sidecar'lar additive + digest DIŞI + identity binding
  iki durable wire'da; MCP response sidecar).
- **W6:** yukarıdaki inv_t9_72 debug + parity testleri.
- **W7:** CLI iki-eksen envelope (`subject_authority: "affected_nodes"`,
  `provenance_authority: "engine_native_per_axis"`, `provenance_native: true`,
  `authority` alias=provenance mirror) + banner; bootstrap seed SABİT.
  `run_envelope.rs` `legacy_projected_v1()` → native ctor; `completed_loop` pin
  authority alanları (before pin'leri sabit; after pin'leri probe-then-freeze).
- **W8:** integration test regolden (parity suite'i + drift suite'i + MCP e2e +
  completed_loop henüz koşulmadı!) + yeni testler: Q4-vs-measurement yarış (nav+MCP),
  binding 5 mismatch (SystemFailure/no-budget/no-retry), disposition exhaustiveness,
  MD-2 observer envanteri. Reason-note şablonu: "provenance-driven (MD-2) —
  frozen #88/#85 + dogfood Run A; subject-set etkisi YOK".
- **W9:** docs/issues (#96 kapanış `feat: #96 …` scope-parens YOK; INV-T4 status;
  migration-decisions MD-2 record; handoff refresh), CI parity ritual (fmt/clippy/
  test --locked --all-features --exclude osp-desktop), dogfood Run A rerun.

## Kritik notlar

- **Characterization ctor gözden geçirme:** `NativeLegacySubjectMeasurement::
  new_characterization_legacy` (doc-hidden pub) — reviewer P0-tur3'ün forgeability
  kapanışının bilinçli istisnası; PR body'de açıkça beyan edilmeli (#100'de V1 lane
  ile silinir).
- MCP `current_measured()` sabiti KALDI (loss_before gate girdisi — bootstrap ayrı).
- osp-desktop build'e dokunulmadı (ritual'de exclude).
- PR #124 (docs) hâlâ açık — merge edilmesi bekleniyor (implementation branch'inden
  bağımsız).
