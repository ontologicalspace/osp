# Handoff — #96 MD-2 PR #125 Review Düzeltmeleri (P0-tur4 SEALED CARRIER — yarım kalan refactor)

**Tarih:** 2026-08-21 (oturum 3 sonu). PR #125 review REQUEST CHANGES (2 P0 + 2 P1 + 1 P2).
**Bu dosya:** P0 düzeltmelerinin mevcut durumu + kalan 19 test hatası + tamamlanmamış işler.

## Oturum nasıl başlamalı

> Handoff: **#96 MD-2 PR #125 P0-tur4 düzeltmelerini tamamla** (sealed carrier refactor
> yarım kaldı — lib derliyor, 19 test hatası var).
> Branch: `feat/96-md2-native-provenance-authority` (working tree'de COMMIT EDİLMEMİŞ değişiklikler var!).
> Notlar: `docs/notes/96-implementation-handoff.md` — önce oku.

**İlk adımlar:** (1) bu dosya, (2) `git status --short` (7 modified file — commit EDİLMEMİŞ),
(3) `export PATH="$HOME/.cargo/bin:$PATH"` + `cargo check -p osp-core` (**0 error — lib DERLİYOR**),
(4) `cargo check -p osp-core --all-targets` → **19 error** (tamamı test kodu; lokasyonlar aşağıda).

## ⚠️ ÇOK ÖNEMLİ: Commit edilmemiş değişiklikler var!

Working tree'de 7 dosya modified (P0 refactor yarım):
```
crates/osp-core/src/engine.rs
crates/osp-core/src/measurement.rs
crates/osp-core/src/navigator.rs
crates/osp-core/src/task_measurement.rs
crates/osp-core/tests/common/mod.rs
crates/osp-core/tests/measurement_v1_v2_parity.rs
crates/osp-core/tests/subject_authority_drift_observation.rs
```
**Önce `git stash` YAPMA** — bu değişiklikler P0 refactor'un kendisi. `git diff` ile incele,
sonra kalan 19 test hatasını düzelt, sonra commit'le.

## P0 Refactor — Tamamlanan (lib derliyor, 0 error)

### P0-2 — `FinalizedNativeTaskClaim` sealed carrier

**`task_measurement.rs`:** `finalize` artık `Result<FinalizedNativeTaskClaim, ...>` döner
(eski: `Result<Claim, ...>`). `FinalizedNativeTaskClaim { claim, measurement }` — private
fields; `claim()` ve `measurement()` accessor'lar. `#[cfg(test)] pub(crate) fn
new_test_with_measured(...)` — crate-içi unit test helper (external erişilemez).

**`engine.rs`:** `TaskCommitInput::new` artık 5 argüman alır:
```rust
// ESKİ (6 arg): TaskCommitInput::new(&claim, &omega, &resolver, target, loss_before, &token)
// YENİ (5 arg): TaskCommitInput::new(&finalized_carrier, &omega, &resolver, target, loss_before)
```
`finalized_carrier: &FinalizedNativeTaskClaim` — ayrı claim+measurement **type-level
unrepresentable**.

**`measurement.rs`:** `NativeLegacySubjectMeasurement`'a `#[derive(Clone)]` eklendi
(sealed carrier sahiplenmesi için).

**`navigator.rs` production kod:** Güncellendi — `finalized` değişkeni kullanıyor;
`TaskCommitInput::new(&finalized, ...)` doğru. Observer çağrıları `finalized.claim()`
kullanıyor.

### P0-1 — `new_characterization_legacy` TAMAMEN KALDIRILDI

**`measurement.rs`:** `pub fn new_characterization_legacy` silindi (eski `#[doc(hidden)] pub`).
Production API'de authority tipini forge edecek YOK. `pub(crate) fn new` tek üretici
(engine.rs `measure_attempt_native_with_md1_shadow`).

**`tests/common/mod.rs` (V1 harness):** `characterization_native_token` fn'i kaldırıldı.
V1 lane artık commit pipeline KULLANMAZ — non-authoritative evaluator:
Q4 structural → PredicateGate direkt (motorun private metodları çağrılmıyor).
V2 lane gerçek native flow kullanır: draft → measure → finalize → sealed carrier → commit.

## Kalan 19 test hatası (lokasyonlar + düzeltme şekli)

### navigator.rs (5 hata — unit test'ler, `characterization_carrier` var)

| Satır | Hata | Düzeltme |
|---|---|---|
| 1719 | `TaskCommitInput::new` 6→5 arg | `&claim` yerine `&commit_token` (carrier'ı 1. arg yap) |
| 1806 | type mismatch | `commit_token`'ı `FinalizedNativeTaskClaim` olarak geçir |
| 1916 | type mismatch | aynı pattern |
| 1963 | type mismatch | aynı pattern |
| 3674 | 6→5 arg | aynı pattern |

**Pattern:** Test'lerde `characterization_carrier(&engine, &claim, measured)` zaten
`FinalizedNativeTaskClaim` döner. Eski kod `carrier.measurement()` ayrı geçiriyordu.
**Düzeltme:** `TaskCommitInput::new(&carrier, ...)` — carrier'ı direkt ver, measurement
ayrı verilmez.

### engine.rs (2 hata)

| Satır | Hata | Düzeltme |
|---|---|---|
| 4321 | 6→5 arg | `characterization_carrier_test` dönen carrier'ı 1. arg yap |
| 8634 | observe fn arg mismatch | `observe_subject_authority_drift(..., finalized.claim(), ...)` — claim accessor kullan |

### subject_authority.rs (1 hata)

| Satır | Hata | Düzeltme |
|---|---|---|
| 1292 | `finalize` dönüş tipi | `finalized.claim()` çağır (FinalizedNativeTaskClaim → Claim) |

### tests/subject_authority_drift_observation.rs (5 hata)

| Satır | Hata | Düzeltme |
|---|---|---|
| 82 | `finalize` dönüş tipi | setup_with_engine'da `finalized.claim().clone()` |
| 390 | `s_native_carrier` not found | `setup_with_engine`'dan carrier döndür; commit'te kullan |
| 661 | 6→5 arg | engine A/B testinde carrier kullan |
| 702 | `s_native_carrier` not found | aynı |
| 873 | `s_native_carrier` not found | aynı |

**Yaklaşım:** `CaseSetup`'a `carrier: FinalizedNativeTaskClaim` field ekle;
commit çağrılarında `TaskCommitInput::new(&s.carrier, ...)`.

### tests/common/mod.rs (1 hata)

| Satır | Hata | Düzeltme |
|---|---|---|
| 2470 | V2 TaskCommitInput 6→5 | `&native_token` (artık sealed carrier) zaten 1. arg — 6. arg kaldır |

### tests/measurement_v1_v2_parity.rs (1 hata)

| Satır | Hata | Düzeltme |
|---|---|---|
| 2186 | syntax error (bozuk regex) | `commit_invalid_mixed_case` fn'inde bozuk kod — gerçek native flow ile yaz |

## Henüz uygulanmamış review bulguları

### P1-1 — MCP/navigator birleşik failure ontology
MCP `NativeAuthority` hatalarını `RejectedBySyntax` JSON'u ile yayıyor; navigator
`SystemFailure` dönüyor. Ortak typed mapper gerekli:
- Gerçek structural/raw Q4 → `RejectedBySyntax`
- Native binding/TCB/operational → system failure JSON
- Agent-correctable → retry surface

### P1-2 — W6/W8 test'leri merge'den önce tamamlanmalı
(W6: ×5 negatif + ABA + cross-pin; W8: observer envanteri + yarış + exhaustiveness)

### P2 — Sidecar diagnostic + accessor
- `PendingAuthorization` provenance mismatch → `SubjectAuthorityDriftIdentityMismatch`
  (yanlış isim — `ProvenanceAuthorityDriftIdentityMismatch` olmalı)
- `RevisionRequired.provenance_authority_drift()` public accessor eksik

### P0 test'leri (yazılmalı)
- 3. negatif: finalize'ı bypass ederek commit → **asla mümkün olmamalı** (compile test
  veya runtime test — `FinalizedNativeTaskClaim` sealed olduğundan artık bypass
  type-level imkânsız; buna rağmen regression test pinlenmeli)
- External crate'in `new_characterization_legacy`'yi çağıramadığını doğrulayan
  compile-fail test (zaten `pub` değil — ama pin)

## Düzeltme sırası (öneri)

1. **navigator.rs 5 hata** — `characterization_carrier` var, sadece 1. arg'ı değiştir
2. **engine.rs 2 hata** — `characterization_carrier_test` var, aynı pattern
3. **subject_authority.rs 1 hata** — `finalized.claim()` accessor
4. **drift_observation 5 hata** — `CaseSetup`'a carrier field ekle
5. **common/mod.rs 1 hata** — V2'de zaten `native_token` (sealed carrier) var, 6. arg kaldır
6. **parity test 1 hata** — bozuk fonksiyonu elle yaz (gerçek native flow)
7. `cargo test --workspace --exclude osp-desktop` → yeşil
8. `cargo fmt && cargo clippy -- -D warnings`
9. Commit + push

## PR #125 mevcut durumu

- **Head:** `eb73d63` (W1-W7 + review turları) — CI yeşil
- **Working tree:** P0-tur4 refactor (commit edilmemiş) — lib derliyor, test'ler kırık
- **Sonra:** Test'ler düzelince commit + push → review turu → W6/W8 → merge

## Ortam notları (Windows)

- `export PATH="$HOME/.cargo/bin:$PATH"` her shell'de
- `command grep`; `python` yok → `node -e`
- **ASLA `git add -A`** (untracked kişisel notlar var)
- CI parity: `cargo fmt --all -- --check`; `cargo clippy --locked --workspace --all-targets
  --all-features --exclude osp-desktop -- -D warnings`; `cargo test --locked --workspace
  --all-features --exclude osp-desktop`
