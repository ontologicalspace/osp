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
- **`produce_legacy_subject_measurement` tamamen çağrıcısız** (PR #129 review P3
  düzeltmesi: grep'te test referansı da yok — yalnız tanım) — mekanik silme;
  navigator/MCP #95-A'da `measure_attempt_native_with_md1_shadow`'a geçmiş.
  **Ancak `derive_v1_legacy_measurement_subject` için durum TERSİ:** engine.rs'te
  W1/W4 kapsamı DIŞINDA q92 çağrıcıları var (`:7865` `q92_observe_divergent_pair`
  helper, `:7971` `q92_001_raws` helper; besleyen test
  `q5_theta_subject_divergent_same_context_shows_theta_divergence` ≈`:7990`) —
  bkz. W5 disposal kararı.
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

> **Yürütme sırası düzeltmesi (implementasyon sırasında, W1 başlarken):** engine
> `md1_shadow` düşürmeden ÖNCE tüketiciler kalkmalı (observer `native.md1_shadow()`
> çağırıyor — engine-önce sıra ağacı kırar). Uygulama sırası:
> **E1** navigator+MCP observer çağrıları → **E2** wire sidecar alanları + drift/sidecar
> testleri + engine observer testi (`:8979` session-verify testi) → **E3** q92 disposal
> (onaylı) → **E4** modül silme → **E5** engine `md1_shadow` düşürme + fn rename +
> cross-pin silme + `TaskScopeNativeMaterial` (yalnız engine'de — MD-3 kullanmıyor,
> doğrulandı) → **E6/E7/E8** = W6/W7/W8 (isimler, `_task_id`, kabul).

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
  `RevisionRequired` sidecar alanları. **Yön mekaniği face-face FARKLI (PR #129 review P2
  düzeltmesi — eski "üç-epoch typed rejection" ifadesi repo'da kanonik tanımsızdı, kaldırıldı):**
  - `TrajectoryEvidence` (trajectory.rs `:1233`): `deny_unknown_fields` YOK + alan
    `#[serde(default)]` Option → silme sonrası **iki yön de kabul** (eski reader yeni
    kaydı default-None ile, yeni reader eski kaydı unknown-field-ignore ile) — sorun yok.
  - `PendingAuthorization`/`RevisionRequired` **durable record** yüzeyi
    (authorization.rs `:5358`; custom Deserialize + `deny_unknown_fields`,
    yorum `:5316`): silme sonrası **yeni reader eski kayıtları unknown-field REJECT
    eder** (yazılan yönün tersi). Digest etkisi YOK (alan preimage'e girmiyor — pinned).
  - **W3'ün ilk adımı (zorunlu):** face-by-face kısa karar notu — hangi yüzeyler durable,
    hangi yön reject, reject bilinçli epoch ilerlemesiyse reason `md1-cleanup (#95-B)`.
    Dogfood durable kayıtları yeniden üretilebilirse (Temp fixture'lar — doğrula)
    epoch ilerlemesi kabul edilebilir; edilemezse eski kayıtları okuyacak tolerasyon
    yolunun (ör. serde alias/ignore) W3'te mi #100'de mi çözüleceği yazılır.
  - MCP response JSON additive sidecar'ları (error semantiği değişmeden eklenmişti)
    kaldır — additive geri alma non-breaking.
- **W4 modül silme:** `subject_authority.rs` tamamı + `lib.rs` mod deklarasyonu +
  `tests/subject_authority_drift_observation.rs` + `tests/md1_subject_authority_sidecar.rs`
  (MCP) sil. `MeasurementSubjectDigest`/`RawMeasurementObservation` vb. başka yerde
  kullanılıyorsa (MD-2 observer kendi tiplerini kullanıyor — doğrula) taşınmadan sil.
  Comment cleanup (PR #129 review P3): `navigator.rs:149`'daki
  `derive_v1_legacy_measurement_subject` atfı W4/W5 sonrası bayat kalır — aynı stage'de
  güncelle.
- **W5 legacy producer + q92 disposal kararı (PR #129 review P1):**
  `produce_legacy_subject_measurement` + `LegacySubjectMeasurement` +
  `legacy_compatibility_projection` sil (tamamen çağrıcısız).
  `derive_v1_legacy_measurement_subject` silinirken engine.rs'teki **q92 kanıt testleri**
  (`q92_observe_divergent_pair` `:7865`, `q92_001_raws` `:7971`,
  `q5_theta_subject_divergent_same_context_shows_theta_divergence` ≈`:7990`) da silinir —
  **karar: sil**; gerekçe: bunlar MD-1 comparison'un ta kendisi (#92 evidence — V1
  legacy-union ↔ V2 task-scope θ diverjansı) ve live test deleted production koduna
  bağımlı olamaz. Tarihçe değeri KAYBOLMAZ: karar kaydı
  (`faz8-p2-migration-decisions.md` #92/MD-1 bölümleri) + frozen goldens + regolden
  disiplini (reason: `md1-cleanup (#95-B)`) taşır. Alternatif (testi koru + legacy
  producer'sız yeniden ifade et) W5 silmeyle çelişir — bilinçli olarak reddedildi.
  Bu karar W1 başlamadan mimar onayına sunulmalı (review'da açık karar istendi).
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

## #95-B scope (özet — yetkili kaynak W1-W8 stage planıdır)

Silinir: `subject_authority.rs` observer + `SubjectAuthorityDriftObservation` +
`produce_legacy_subject_measurement`/`derive_v1_legacy_measurement_subject`
(üretim çağrıcısız; q92 test çağrıcıları W5 disposal kararıyla) + ~~`effective_legacy_measure_set`~~
(**W0'da silindi — tamam**) + MD-1 sidecar alanları (yön mekaniği W3'te face-face) +
MD-1 comparison testleri (q92 dahil — W5 kararı) + legacy fiziksel isimler (W6) +
`osp_mcp` `submit_delta_attempt`'ın kullanılmayan `_task_id: TaskId` parametresi (W7;
PR #128 review P3 — task kimliği `task` nesnesinden türetiliyor).
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
