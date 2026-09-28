# Handoff — #97 MD-3 Baseline Availability (TAMAMLANDI — PR açıldı) / #95-B kaydı

**Tarih:** 2026-09-28 (S3+S4 oturumu). **#97 S1-S4 TAMAM** — branch
`feat/97-md3-baseline-availability` (main `fb995fb` üzerinden), 7 commit push'landı,
her aşama yeşil (40 suite / 0 fail / clippy+fmt temiz). **PR açıldı** (bkz. `gh pr list`).

## Oturum nasıl başlamalı

> Handoff: **#97 MD-3 TAMAMLANDI** (S1-S4; PR açık). Sonraki iş: PR review +
> merge → **#100 Faz 8a engine cutover** (MD-2 fiziksel kaldırım dahil). Bu dosyanın
> #97 teslim kaydı + Run D önerisi bölümleri bilgi amaçlı kalır.

## #97 teslim edilenler (commit zinciri)

- `7db890f` kickoff docs (envanter + tasarım + S-planı)
- `923d14a` **S1 tip modeli:** `ColdStartPolicy` (default Disallow; `TaskPolicy`
  alanı `#[serde(default)]` — eski task wire'ı Disallow'a düşer, test pin'li) +
  `MutationDecision::AcceptAsColdStart` (→Sandbox, INV-T8; negatif pin'ler) +
  `MutationDecisionTag` append-only **4** + faz8-p2 manifest regolden (18 digest;
  reason: `task-policy-adds-cold-start (#97 S1)`)
- `a7ab56e` **S2 motor karar matrisi:** `BaselineAvailabilityClass` (değer
  TAŞIMAYAN sınıflandırma) + commit Phase 0d'de INV-T6 extension zorlaması
  (Unavailable+NotCompleted altında skaler improvement REJECT'e düşer) +
  `EngineCommitResult::SuspendedColdStart` (Held'den AYRI otorite) +
  navigator/MCP/CLI typed map'ler (CLI exit 14) + 4 exact matris testi
- `42e49c1` docs: S1+S2 handoff
- `caf54f1` **S3 onay akışı:** `approve_cold_start` engine metodu — in-flight
  suspension kaydı (kalıcı DEĞİL; motor ömrüyle sınırlı — `PendingAuthorization`
  persist modeline bilinçli benzeşmezlik) + onay anında deterministik revalidation
  (task bind + #95-A scope fence + #96 5-fence stale + Q5 vision + MD-3
  sınıflandırma/policy/completion + Q6 rules — **operatör onayı güvenlik çekirdeğini
  bypass ETMEZ**) + operatör otoritesiyle witness-bypass Sandbox apply
  (`bigbang::prospective_delta_from_claim` — witness::evaluate ile TEK preimage
  truth; `time.advance` çağrılmaz; t_c ilerler) + tek kullanım (kayıt düşer) +
  `ColdStartAcceptanceEvidence` (issue'nun 8 alanı + task/claim bağlamı;
  `improvement_claimed` ctor sabiti false) + `ColdStartOperatorId`/
  `ColdStartAuthorizationId` newtype + navigator INV-T9 pin (retry/evidence/mutation
  YOK — tek proposal'lı mock kanıtı) + MCP `osp_approve_cold_start` operator-only
  tool (INV-T2 gate; typed error sınıfları: unknown_suspension/stale_binding/
  policy_changed/...) + 9 yeni test (6 motor + 1 navigator + 2 MCP uçtan uca)
- `463c575` **S4 kabul:** spec INV-T6/T8/T9 MD-3 bölümleri planned→**implemented**
  (test isimleriyle kanıt bağlandı; INV-T8 spec kod örneğine AcceptAsColdStart map
  satırı eklendi) + matrisin kalan satırı: `md3_completed_with_partial_new_subject_
  still_completes` (rule 3'ün en geniş claim'i — PartialNew altında bile completion
  bağımsız; SuspendedColdStart asla)

## #97 dogfood Run D önerisi (PR merge sonrası aday)

Cold-start fixture'ı ile uçtan uca canlı akış: (1) task scope'u base'te olmayan
node'a bağlı + `ColdStartPolicy::RequireOperatorApproval` + NotCompleted predicate;
(2) `osp_run_task`/`osp_submit_delta` → `SuspendedColdStart` (mainline_mutation:
not_applied); (3) `osp_approve_cold_start` (operator mode) → `AcceptAsColdStart` +
Sandbox apply + evidence 8 alan; (4) ikinci onay → unknown_suspension; (5) onay
öncesi space'e başka apply → stale_binding fail-closed. MCP contract testleri
(`crates/osp-mcp/tests/md3_cold_start_approval.rs`) aynı zinciri zaten kanıtlıyor —
Run D bunu canlı repo üzerinde envelope üretimiyle tekrarlar (Run A/B/C kalıbı).

## #97 MD-3 envanter (2026-09-28 tarama — mevcut durum)

- **V2 tip modeli ZATEN VAR:** `MeasurementBaseline::{Available, Unavailable}`
  (measurement.rs:818) + `BaselineUnavailableReason::{AllMembersIntroducedByDelta,
  PartialNewSubject}` (:1746). Engine üretiyor (:2656-2667).
- **Kontrol çekirdeği:** `gate_v2.rs:591 compute_completion_first_loss_and_decision`
  (completion × policy matrisi). AcceptImprovement + Unavailable → improved=false +
  Reject (PR#84 P1) — MD-3 default satırı ZATEN doğru; eksik olan politika opt-in'i.
- **Karar enum:** `MutationDecision` (trajectory.rs:1033, 4 varyant) —
  `AcceptAsColdStart` eklenecek. `apply_target()` (:1099) — Sandbox map.
- **Etiket kaydı:** `MutationDecisionTag(u8)` (authorization.rs:4600; append-only —
  yeni varyant yeni u8 alır, TryFrom u8 red kapalı).
- **TaskPolicy** (trajectory.rs:510): `cold_start_policy: ColdStartPolicy` alanı
  (serde default Disallow — TaskPolicy wire'da task tanımlarında taşınıyor).
- **Bekleme mekanizması:** `WitnessHoldReason` (witness.rs:275) tanık-odaklı
  (Q1/Q2/inv#3) — soğuk başlatma OPERATÖR odağı; Held yeniden kullanımı otorite
  karışımı yapar.

## #97 tasarım kararı (S2'ye giriş)

**`EngineCommitResult::SuspendedColdStart` yeni varyant** (Held ile paralel yapı,
ayrı otorite): `{ authorization: AuthorizationContext, reason:
ColdStartAuthorizationRequired }` — tanık snapshot'ı YOK (operatör onayı, quorum
anlamsız). Navigator → yeni `NavigatorResult::AwaitingOperatorApproval` benzeri
map; MCP → typed JSON. Onay sonrası: engine `approve_cold_start(...)` →
`MutationDecision::AcceptAsColdStart` → `ApplyTarget::Lane(Sandbox)` apply +
`ColdStartAcceptanceEvidence` (issue'daki 8 alan). `PartialNewSubject` → terminal
Reject (reason evidence'da korunur — matris satırı; override yok).

## #97 S-planı (TÜMÜ TAMAMLANDI)

- **S1 tip modeli:** ✅ `923d14a`
- **S2 motor:** ✅ `a7ab56e`
- **S3 onay akışı:** ✅ `caf54f1`
- **S4 kabul:** ✅ `463c575` (spec flip + matris pin) + PR + Run D önerisi (yukarıda)

---

# #95-B kaydı (E0-E8 TAMAMLANDI — MERGED `fb995fb`)

**Tarih:** 2026-09-28. #95-A **PR #128 MERGED** (squash `e3f44c8`).
#95-B branch: `feat/95b-md1-cleanup` (main `e3f44c8` üzerinden). **E0-E8 tamamlandı**
(8 commit; net ~−3.700 satır). CI parity yeşil: fmt + clippy `-D warnings` +
39 suite / 1942 test / 0 fail. Kabul kriteri: **"MD-1 compatibility code tamamen
kaldırılmış" (MD-1 scope) — sağlandı** (kalan `subject_authority` hit'leri: CLI
zarf etiketi `execution_measurement.subject_authority: "task_scope"` (#95-A'nın
kendi çıktısı — kalıcı) + tarihsel tombstone'lar).

## Teslim edilenler (commit zinciri)

- `35adf59` W0: `effective_legacy_measure_set` (çağrıcısız)
- `9d68bd5`+`f8e1ab1`+`22e919b`+`d50e706` docs: plan + PR review revizyonu (P1 q92
  disposal kararı, P2 wire yönü face-face) + tüketici-önce sırası + karar notu
- `5478c48` E1: navigator+MCP observer çağrıları + MD-1 sidecar JSON/testleri
- `5e0c2bb` E2: wire sidecar alanları (TrajectoryEvidence/PendingAuthorization/
  RevisionRequired — durable yüzeylerde bilinçli epoch ilerlemesi) + drift-observation
  test dosyası + engine observer testi (−1249)
- `bbb75ee` E3: q92 subject-divergence bölümü (mimar onaylı disposal)
- `e31b7f7` E4+E5: `subject_authority.rs` modülü silindi; ortak Q5 gözlem tipleri
  `provenance_authority.rs`'e taşındı (MD-2 dokunulmadı — byte-identical, serde
  değişmedi); engine `md1_shadow` lane düştü; `measure_attempt_native` rename;
  legacy producer ailesi + `V2MeasurementFailure` + `TaskScopeNativeMaterial` (−1701)
- `f1781bc` E7: `submit_delta_attempt` `_task_id` parametresi
- `94c6f67` E6: legacy isimler → `NativeSubjectMeasurement`,
  `NativeMeasurementBindingError`, `SubjectBindingDigest`, `SubjectBindingMismatch`
  (draft×token digest varyantı — `TaskSubjectBindingMismatch`'tan FARKLI, ikisi de
  yaşıyor), `subject_binding`, `subject_member_ids()`; compile-fail stderr'ler re-bless

## Bilinçli keep'ler (#100'e not)

- CLI zarf etiketi `execution_measurement.subject_authority` — #95-A authority mode
  beyanı, MD-1 compat DEĞİL.
- `b"osp.legacy-subject-binding.v1\0"` DOMAIN_SEPARATOR — digest preimage sabiti;
  değiştirmek değer churn'u üretir, fayda yok.
- MD-2 prose'unda geçen tarihsel "MD-1" anlatımları — W8 kapsamında bilinçli korundu.
- `measurement_v1_v2_parity.rs` — MD-2 tarihsel karakterizasyon (plan karar notu).

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
