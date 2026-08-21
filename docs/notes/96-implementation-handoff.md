# Handoff — #96 MD-2 Implementation (PR AÇIK — W1-W7 kodlandı, workspace 39/39 yeşil)

**Tarih:** 2026-08-21. Plan v4-FİNAL APPROVED — W1-W7 + 3 review düzeltmesi kodlandı.
**Bu dosya:** Implementation durum devri + kalan iş (W6 testleri + W8 + W9).

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 implementation PR review'ı + W6-W9 kalanı** ile devam.
> Branch: `feat/96-md2-native-provenance-authority` (push edilmiş, PR açık).
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku, durum kontrolü yap.

İlk adımlar: (1) bu dosya, (2) `gh pr view` (PR numarası + review durumu),
(3) `git log --oneline -8`, (4) `export PATH="$HOME/.cargo/bin:$PATH"` +
`cargo test --workspace --exclude osp-desktop` (**39 binary, 0 failure** — doğrula).

## Mevcut durum — PR commit zinciri (squash öncesi)

| Commit | Kapsam |
|---|---|
| `0e31cd3` | W1: opaque token + tek-session producer + singleton fast-path + cross-pins |
| `54602b7` | W2-W4: navigator/MCP cutover + binding verification + proof-sourced basis |
| `3b2dd90` | Review P1: `NativeAuthority(NativeLegacyMeasurementBindingError)` typed family |
| `e24055d` | Review tur-2 P1: `LegacySubjectBindingDigest` + draft capture + finalize→Result |
| `d822359` | `serde_json float_roundtrip` (inv_t9_72 kök neden!) + native-honest fixture regolden |
| `31e1ade` | W7: CLI iki-eksen vocabulary (`engine_native_per_axis`) |
| `120dacb` | W5: `ProvenanceAuthorityDriftObservation` + wire + navigator/MCP wiring |
| `c101a09` | fmt (test dosyaları) |

**Test:** workspace 39 binary 0 failure; fmt/clippy `-D warnings` temiz.

## Tamamlanan işlerin özeti

### W1 — Opaque token + tek-session producer
`NativeLegacySubjectMeasurement` (measurement.rs): private fields; `legacy_subject_ids`'den
türetilen `legacy_subject_binding` digest (tur-2 P1); `raw() = measured.to_raw()`.
`CoreAxisEpochStamp` (coords.rs) atomik capture'dan. `measure_attempt_native_with_md1_shadow`
(engine.rs): TEK BoundMeasurementSession; legacy subject `effective_legacy_measure_set`
(draft×producer tek truth); md1_shadow aynı session; `verify_unchanged` sonunda.
Singleton fast-path recovered (`021bd5f`).

### W2-W4 — Caller cutover + commit-time verification
`task_measurement.rs` (pub mod): `StructurallyValidatedClaimDraft::try_new` (probe + Q4
structural tek adımda; `legacy_subject_binding` private capture) + `finalize(&token) →
Result<Claim, LegacySubjectBindingMismatch>` + `MeasurementFailureDisposition` (17 varyant
exact tablo). `TaskCommitInput` private fields + `new()` (`measured` → `measurement:
&NativeLegacySubjectMeasurement`). `verify_native_legacy_measurement_binding` (5 kontrol:
delta digest / raw bits / revision / context / epochs ABA) → `VerifiedNativeLegacyMeasurementBinding`
private proof → `build_authorization_context` proof'tan okur (ikinci TOCTOU kapalı).
Navigator + MCP: draft→producer→finalize ordering (structural Q4 önce).

### Review düzeltmeleri
- **P1-tur1 (ontology):** `MeasurementBindingVerificationError::NativeAuthority(
  NativeLegacyMeasurementBindingError)` typed family; mevcut `Mismatch` (caller-authority)
  ailesi dokunulmaz. Navigator: `NativeAuthority` → `Unknown` gate_decision.
- **P1-tur2 (subject binding):** `LegacySubjectBindingDigest` + draft capture + finalize
  karşılaştırması. 2 negatif test: (1) aynı delta + farklı affected → mismatch; (2) aynı
  delta + AYNI raw bits + farklı subject → YİNE mismatch.

### `inv_t9_72` KÖK NEDEN — `serde_json float_roundtrip`
serde_json default float parser **1 ULP kaybediyor** (6/13 gibi 17-hane shortest f64'de).
Basis digest in-memory değeriyle hesaplanıyor; reload kayıplı parse → `BasisDigestMismatch`.
**Çözüm:** workspace `serde_json`'a `float_roundtrip` feature. Mikro-probe kanıtı:
`a=6.0/13.0 → json → parse → a ≠ a` (before) → `a == a` (after). Frozen wire değişmez.

### W7 — CLI iki-eksen vocabulary
`CliExecutionMeasurement`: `subject_authority: "affected_nodes"` (#95-A'da `task_scope`'a
çevrilir; family-label — literal subject set DEĞİL) + `provenance_authority:
"engine_native_per_axis"` + `provenance_native: true` + deprecated `authority` alias.
Bootstrap `current_measured` tohumu SABİT (metadata yalnız proposal/commit authority).

### W5 — MD-2 observer modülü
`provenance_authority.rs`: `uniform_scip_reference_projection` (reference-only;
`legacy_compatibility_projection`'ın yeni evi — fiziksel kaldırma #100) +
`observe_provenance_authority_drift` (AYNI token; native↔uniform-Scip; counterfactual
PredicateGate asla "production observed" değil) + `ProvenanceDownstreamObservation`
üç-durum (`Q4SyntaxRejection` arm'ı YOK) + `provenance_downstream_from_engine_commit_error`.
Wire: `TrajectoryEvidence.provenance_authority_drift` (serde default) +
`PendingAuthorization` (validate_internal identity-bound) + `RevisionRequired.
try_with_provenance_authority_drift` (checked builder) — hepsi digest preimage DIŞINDA.
Navigator + MCP: Held/Rejected/Evaluated/retryable-Q5/Q6 tüm comparison-surviving yollarda.

### Native-honest fixture regolden'ler (dogfood Run A izdüşümü)
- Navigator fixture'ları: coupling axis Placeholder→Scip (`coupling_task`'ın
  `required_source=Some(Scip)` native'de onurlanır); `make_engine` gerçek axis'lere.
- CLI harness task şablonları: `required_source: Scip → None`.
- MCP e2e Held: `required_source: None` + V1 subject `[10_000]` (fallback) + native sources.
- MCP e2e Q4: precedence-correction beklentisi (draft-stage → sidecar YOK).
- `navigator_accepts_progress` / `navigator_records_evidence`: HarnessAutoApprove.
- 002 cross-pin V1 sources: `[TreeSitter, Placeholder, TreeSitter, Heuristic, Heuristic]`.

## Kalan işler (sıra ile)

### W6 kalan — commit verifier ×5 negatif + basis↔token cross-pins
`inv_t9_72` tabanında (ProcessLocalFilesystemTestStore, gerçek persist/reload):
- **×5 negatif:** (1) StructuralDeltaMismatch — claim delta değiştir → token mismatch;
  (2) RawMismatch — claim.computed_raw bits değiştir; (3) StaleSpaceRevision — space mutate
  → commit → stale reject; (4) MeasurementContextMismatch — axis descriptor mutate → reject;
  (5) **AxisEpochMismatch ABA** — A→B→A axis mutation → epoch reject (monoton fence).
- **Cross-pin'ler:** persisted `AuthorizationBasis.base_space_view_revision` ==
  token.base_revision; `measurement_input_digest` == token'ınki; `measured_result` ==
  token.measured (reload sonrası bits+sources exact).
- **Tamper ×2:** value tamper → fail-closed; source tamper → fail-closed.
- **Null/Filesystem parity:** aynı logical Held → aynı basis digest.
- Not: mevcut `inv_t9_72` testi zaten reload digest-parity pinliyor (float_roundtrip sonrası
  geçiyor) — bunlar ekine get stronger kanıtlar.

### W8 kalan — MD-2 observer test envanteri + yarış + exhaustiveness
- **MD-2 observer mirror envanteri:** `ProvenanceAuthorityDriftObservation` integration
  testleri (drift observation suite'ine ekle — SAME value bits pin, source divergence pin,
  eligibility Q5Violated/ReachedButUnavailable/Observed, Held sidecar wire/identity).
- **Q4-vs-measurement yarış:** structural-Q4-invalid proposal + measurement-failing task →
  daima `SyntaxViolation`; navigator VE MCP ayrı ayrı (plan v4 acceptance'ı).
- **Disposition exhaustiveness:** 17 varyantın hepsinin `MeasurementFailureDisposition`
  eşlemesi doğru (wildcard-free — zaten compile-time garantili ama test pinlemesi).
- **Navigator provenance sidecar testleri:** `md2_completed_evidence_carries_provenance_observation`
  + `md2_held_pending_authorization_carries_provenance_observation` (mirror of MD-1).

### W9 — docs/issues + dogfood + PR finalize
- **Dogfood Run A rerun:** handoff fixture (`C:/Users/ervol/AppData/Local/Temp/osp-md1-dogfood/`)
  ile `osp trajectory attempt` — yeni envelope iki-eksen + sidecar'lar canlı kanıt.
- **Docs:** migration-decisions MD-2 implementation record; INV-T4 status;
  `95-md1-cutover-handoff.md` refresh (#95-A sıradaki).
- **Issues:** #96 close comment (`feat: #96 …` scope-parens YOK); #100'e "W5+W7 teslim"
  yorumu (uniform-Scip reference projection `provenance_authority.rs`'te yaşıyor — fiziksel
  kaldırma #100'de).
- **PR body güncelle:** W5-W7 teslim listesi + characterization ctor disclosure zaten var.

## Kritik notlar (PR body'de beyan edildi)

- **`new_characterization_legacy` (doc-hidden pub ctor):** V1 reference lane harness'inin
  survival'ı için bilinçli istisna — production forge edilebilirlik kapanışı `new()`'un
  pub(crate) olmasından gelir. #100'de V1 lane kaldırılınca silinir.
- **`serde_json float_roundtrip`:** workspace feature — parse correctly-rounded; yazım
  tarafı (ryu shortest) ve frozen wire değişmez. Pre-existing artifact'lar etkilenmez
  (internal-consistency her zaman doğru taraf).
- **MCP `current_measured()` sabiti:** loss_before gate girdisi — bootstrap ayrı migration.
- **osp-desktop** build'e dokunulmadı (ritual'de exclude).

## Ortam notları (Windows)

- `export PATH="$HOME/.cargo/bin:$PATH"` her shell'de.
- `command grep`; `python` yok → `node -e`; temp `C:/Users/ervol/...`.
- **ASLA `git add -A`** (untracked kişisel notlar var).
- CI parity ritual (exact): `cargo fmt --all -- --check`; `cargo clippy --locked --workspace
  --all-targets --all-features --exclude osp-desktop -- -D warnings`; `cargo test --locked
  --workspace --all-features --exclude osp-desktop`.
