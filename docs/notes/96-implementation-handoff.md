# Handoff — #95-B MD-1 Cleanup (AKTİF — branch açık, W0 tamam; #95-A merged)

**Tarih:** 2026-09-28 (oturum başlangıcı). #95-A **PR #128 MERGED** (squash `e3f44c8`;
review: onaylanabilir, P2+P3'ler düzeltildi, thread'ler resolve edildi). #95-B branch:
`feat/95b-md1-cleanup` @ `35adf59` (main `e3f44c8` üzerinden). **W0 tamam:**
`effective_legacy_measure_set` kaldırıldı (çağrıcısız dead pub yüzey).

## Oturum nasıl başlamalı

> Handoff: **#95-B — MD-1 cleanup, W1'den devam** (W0 done: `effective_legacy_measure_set`).
> Branch: `feat/95b-md1-cleanup`. Plan: bu dosyanın "#95-B W1-W8 stage planı" bölümü.
> Notlar: `docs/notes/faz8-p2-migration-decisions.md` MD-1 bölümü + PR #128 review yorumları.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -4` (W0 `35adf59` üstünde
çalışılmalı), (3) `export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test --locked
--workspace --all-features --exclude osp-desktop` (yeşil başlangıç).

## #95-B envanter (2026-09-28 tarama — kanıtlanmış gerçekler)

- **MD-2 ayrışması temiz:** `provenance_authority.rs` (MD-2 observer) `NativeAttemptMeasurement`
  / `md1_shadow`'a referans ETMİYOR — MD-1 siliniyor, MD-2 yüzeyine dokunma.
- **`produce_legacy_subject_measurement` üretim çağrıcısız** (grep: yalnız subject_authority.rs
  tanımı + drift-observation test) — mekanik silme; navigator/MCP #95-A'da
  `measure_attempt_native_with_md1_shadow`'a geçmiş.
- **Kullanım haritası:** observer/`SubjectAuthorityDriftObservation` 16 dosyada;
  `md1_shadow` lane engine.rs'te (`NativeAttemptMeasurement.md1_shadow:717`,
  `measure_md1_shadow_in_session:2900`, cross-pin testleri `:8302-8460`); navigator
  observer çağrısı `navigator.rs:910`; ölçüm çağrısı `navigator.rs:835`.
- **Karar gerektiren nokta:** `tests/measurement_v1_v2_parity.rs` (434 satır) başlığında
  "bilinçli korunan tarihsel pre-#96 karakterizasyon" diyor (MD-2 geçmişini de belgeler).
  Handoff scope'u "MD-1 comparison testleri" diyor — bu dosya V1/V2 subject+provenance
  karşılaştırması. Silme kararı: yalnız MD-1-subject karşılaştırma bölümleri silinmeli,
  MD-2 tarihsel karakterizasyon kısmı #100'e kadar KALIR (uniform-Scip reference'ın
  görgü tanıkları). Belirsizlik varsa silme — #100'de netleşir.

## #95-B W1-W8 stage planı (bağımlılık sırasıyla; her stage commit + yeşil test)

- **W1 engine:** `NativeAttemptMeasurement`'dan `md1_shadow` alanı düşür; fn
  `measure_attempt_native_with_md1_shadow` → `measure_attempt_native` (imza aynı,
  yalnız dönüş bundle'ı); `measure_md1_shadow_in_session` + cross-pin testleri
  (engine.rs `:8302-8460`) sil. `TaskScopeNativeMaterial` kullanan kalmıyorsa sil
  (MD-3 yüzeylerinde kullanım VARSA dokunma — grep önce).
- **W2 navigator:** `observe_subject_authority_drift` çağrısı (`navigator.rs:910`) +
  drift draft finalize/sidecar wiring (Held → `PendingAuthorization.subject_authority_drift`;
  Rejected → `RevisionRequired.with_subject_authority_drift()`) kaldır; eligibility
  sözleşmesi (`v1_downstream_from_engine_commit_error`) kullanımdan düşer.
- **W3 wire sidecar:** `TrajectoryEvidence.subject_authority_drift` + `PendingAuthorization`/
  `RevisionRequired` sidecar alanları — **durable wire üç-epoch typed rejection** kurallarına
  göre kaldır (yeni reader eski wire'ı kabul eder; eski reader yeni wire'ı reddeder —
  serde epoch makinesine uygun yön). MCP response JSON additive sidecar'ları (error
  semantiği değişmeden eklenmişti) kaldır — additive geri alma non-breaking.
- **W4 modül silme:** `subject_authority.rs` tamamı + `lib.rs` mod deklarasyonu +
  `tests/subject_authority_drift_observation.rs` + `tests/md1_subject_authority_sidecar.rs`
  (MCP) sil. `MeasurementSubjectDigest`/`RawMeasurementObservation` vb. başka yerde
  kullanılıyorsa (MD-2 observer kendi tiplerini kullanıyor — doğrula) taşınmadan sil.
- **W5 legacy producer:** `produce_legacy_subject_measurement` +
  `LegacySubjectMeasurement` + `legacy_compatibility_projection` +
  `derive_v1_legacy_measurement_subject` sil (çağrıcısız — W4 sonrası kalan test
  referanslarıyla birlikte).
- **W6 isimler:** `NativeLegacySubjectMeasurement` → `NativeSubjectMeasurement`
  (subject artık canonical task scope — "legacy" yanıltıcı); `legacy_subject_binding`
  alanı → `subject_binding`; `LegacySubjectBindingDigest` → `SubjectBindingDigest`;
  `NativeLegacyMeasurementBindingError` → `NativeMeasurementBindingError` (tür ailesi
  içindeki `TaskSubjectBindingMismatch` zaten #95-A'da doğru adı taşıyor). Compile-fail
  fixture'ları + stderr'leri yeni isimlerle re-bless (TRYBUILD=overwrite).
- **W7 `_task_id`:** `osp_mcp submit_delta_attempt` imzasından `_task_id: TaskId` düşür
  (server.rs:778 + iç çağrı `:392` + kalan test çağrıları; md1_sidecar testi W4'te gitti).
- **W8 kabul + regolden:** "MD-1 compatibility code tamamen kaldırılmış" (MD-1 scope).
  Sidecar/observer referanslı golden varsa regolden — reason: `md1-cleanup (#95-B)`;
  eski değerler tarihçe için korunur (regolden disiplini). CI parity: fmt + clippy
  `-D warnings` + tam paket.


## #95-A teslim edilenler (kritik zincir)

```text
draft.try_new(proposal, raw, task, …)  — Q4 structural → canonical_task_subject_scope(task) capture
measure_attempt_native_with_md1_shadow(draft, task)  — proposal param YOK (capability reduction)
  TEK session: authority = shadow = canonical task scope; native provenance sabit (#96)
finalize  — draft scope ↔ token scope (LegacySubjectBindingMismatch; adı #95-B'ye kadar legacy)
commit_task_claim
  resolve → validate_for_commit
  → canonical_task_subject_scope(current): Err→Derivation(SubjectDerivationFailed); ≠token→TaskSubjectBindingMismatch
  → #96 5-fence (DOKUNULMADI) → Q5 → PredicateGate → Q6 → witness
```

- **P0 fence** (tur-2): registry-overwrite negatif e2e ×2 (scope drift + heterojen
  derivation) — Q5/witness/mutation'a ulaşmaz.
- **Affected-irrelevance executable theorem** (tur-3 P1): full-path metamorphic —
  Δaffected → subject/bits/sources/digests/finalize/gate/decision/basis.measured invariant.
- **Dogfood Run C** (divergent fixture — scope [3] vs affected [3,2]): envelope
  `task_scope`; authority subject [3] ≠ legacy [3,2]; MD-1 lane parity; MD-2 observer
  intentional divergence intact; Completed.
- Regolden (eski+yeni+reason `subject-cutover (#95-A)`): 002 cross-pin'leri → parity;
  finalize mix-negatifleri → yeni pozitif/negatif; module-scope → draft terminal;
  CLI/LLM/parity/MCP pin'leri.
- Fiziksel legacy isimler (`NativeLegacySubjectMeasurement`, `legacy_subject_ids`,
  `LegacySubjectBindingDigest/Mismatch`) BILİNÇLİ kaldı — doc truth-surface
  pre/post-#95-A tablosuyla; yeniden adlandırma #95-B.

## #95-B scope (sıradaki)

Silinir: `subject_authority.rs` observer + `SubjectAuthorityDriftObservation` +
`produce_legacy_subject_measurement`/`derive_v1_legacy_measurement_subject` +
`effective_legacy_measure_set` (çağrıcısız — PR #128 review P3 notu) + MD-1 sidecar
alanları (wire üç-epoch typed rejection yalnız durable wires) + MD-1 comparison
testleri + legacy fiziksel isimler + `osp_mcp` `submit_delta_attempt`'ın kullanılmayan
`_task_id: TaskId` parametresi (PR #128 review P3 — task kimliği `task` nesnesinden
türetiliyor; imza değişikliği cleanup PR'ına ait).
Kalır: uniform-Scip reference projection (#96'nın evi — #100), `compute_raw_from_delta`
(#100 machinery), MD-3 yüzeyleri (#97). "Compatibility code tamamen kaldırılmış"
kriteri **MD-1 compatibility code** olarak okunur.

## Sıra

**#95-B** → #97 (MD-3) → #100 (Faz 8a engine cutover — MD-2 fiziksel kaldırım dahil).

## Ortam notları (Windows)

- `export PATH="$HOME/.cargo/bin:$PATH"`; `command grep`; `node -e` (python yok)
- **ASLA `git add -A`**; commit formatı `feat: #95 …` (scope parens YOK)
- Dogfood fixture: `C:/Users/ervol/AppData/Local/Temp/osp-95a-runc/` (disposable)
- CI parity: fmt --check; clippy --locked --workspace --all-targets --all-features
  --exclude osp-desktop -- -D warnings; test aynı target seti
