# #100 Kickoff — Faz 8a Engine Cutover

**Tarih:** 2026-09-28. Branch `feat/100-faz8a-engine-cutover` (main `0409f3d` üzerinden).
Başlangıç yeşil: fmt + clippy `-D warnings` + tam paket (40 suite / 0 fail) — main üzerinde
doğrulandı. Önceki durum: **#97 MD-3 MERGED** (PR #130 squash `0409f3d`, issue CLOSED);
#95-A/#95-B/#96/#97 authority cutover'ları tamam — #100 engine'i V2 tüketiciye taşır +
V1 compatibility fiziksel kaldırır.

**Yetkili kaynaklar:** issue #100; `docs/notes/faz8-p2-migration-decisions.md` (MD-1/2/3
karar kaydı); `docs/notes/96-implementation-handoff.md` (#95-B bilinçli keep'ler);
`authorization.rs:9386-9404` (Faz 8 wiring planı — Faz 4 döneminden).

## Envanter (2026-09-28 tarama — kanıtlanmış gerçekler)

### V1 bugünkü commit zinciri (`commit_task_claim`, engine.rs:1907-2187)

Q4 → task bind → `validate_for_commit` → **#95-A MD-1 subject fence** (:1957) →
**#96 5-fence** `verify_native_measurement_binding` → `VerifiedNativeMeasurementBinding`
(:1983; üretim :3555 — delta digest/raw bits/revision/context+epoch) → Q5 vision (captured
context) → **Phase 0d: V1 `PredicateGate.evaluate`** (:2001; girdi: caller `input.loss_before`
skaler + `input.target` + proof'tan measured) → **#97 MD-3 matrisi** (:2023-2102;
`classify_baseline_availability` (:1843) partition'ı + INV-T6 düşürmesi `AcceptAsProgress`→
`Reject`) → Q6 → **V1 `build_authorization_context`** (:2381; loss_before/loss_after/
improvement_policybasis'te) → witness `time.advance` → `EngineCommitResult`.

`approve_cold_start` (:2213) 8. adımda `PredicateGate.evaluate` ile completion
revalidation (:2315) — kayıt `SuspendedColdStartRecord{target, loss_before}` taşır.

### V2 zinciri (HAZIR — `#[allow(dead_code)] "Faz 8 wiring"`, yalnız test çağrıcılı)

- `evaluate_task_gate_v2(binding, measurement, task)` (gate_v2.rs:365): 3-digest TOCTOU
  recheck + task identity + predicate EXACTLY ONCE + `compute_completion_first_loss_and_
  decision` (:591; completion-first matris, loss_before `measurement.before()`+preferred
  vector'dan derive — caller skaler YOK; Unavailable+AcceptImprovement → improved=false →
  **Reject native**; NoPreferredVector → Reject) → `VerifiedGateEvaluationBundleV2`.
- `build_authorization_context_v2(bundle, witness_requirement, measurement)` (gate_v2.rs:774)
  → `AuthorizationContextV2` (authorization.rs:6129; proof-gated ctor; **Serialize bilinçli
  YOK — "tek serialization yolu wire DTO (VersionedAuthorizationBasis)"**; V1 wire FROZEN:
  authorization.rs:9465 "V1 frozen (wire format HİÇ değişmez)").
- Girdi tipi `VerifiedTaskMeasurementBinding` (engine.rs:478) — üreticisi Faz 3
  `verify_measurement_binding` (:971, test-only artık): task_claim_digest + task_goal_
  evidence/digest + engine_measurement_digest + preferred_vector_snapshot +
  predicate_gate_policy_digest taşır. **#96 5-fence bunları ÜRETMEZ** — köprü bu.
- `EngineMeasurement` (measurement.rs:1766): before/after/context/request. **Token DEĞİL**:
  `NativeSubjectMeasurement` (measurement.rs:1946) baseline TAŞIMAZ ("EngineMeasurement
  DEĞİLDİR — baseline yoktur (MD-3 = #97'nindir)" :1929). Baseline'i yalnız
  `measure_task_delta` (:3210, partition :3291-3333) üretir.

### Kaldırım yüzeyi

- **MD-2 observer:** `provenance_authority.rs` tamamı — `uniform_scip_reference_projection`
  (:188, doc "fiziksel kaldırma #100") + `observe_provenance_authority_drift` (:330;
  navigator.rs:903 wiring) + counterfactual `PredicateGate.evaluate` (:363). Wire sidecar:
  `TrajectoryEvidence.provenance_authority_drift` (serde(default) Option — iki yön OK) +
  `PendingAuthorization`/`RevisionRequired` telemetry sidecar'ları (durable strict
  `deny_unknown_fields` — alan silinince yeni reader eski kaydı REJECT eder; #95-B W3
  emsaliyle bilinçli epoch ilerlemesi, dogfood kayıtları Temp fixture — yeniden üretilebilir)
  + MCP additive JSON + CLI run envelope alanı (run_envelope.rs:250).
- **`provenanced_from_raw`** (navigator.rs:166): production çağrıcı MCP server.rs:840 + CLI
  mod.rs:766 — ikisi de "G1 bootstrap seed" (hardcoded (0.7,0.5,0.5,0.5,0.3) + uniform
  Scip). Kaynaklar çıktı yüzeyinde gözlenmez (TrajectoryEvidence yalnız raw taşır);
  loss_before bootstrap'i cutover'la ölüyor.
- **DefaultFallback:** `compute_raw_from_delta` (:3128; `RawPosition::default()` :3137/:3174)
  + `try_compute_raw_from_delta` (:3896 :3905) — production çağrıcı yalnız osp-desktop demo
  (lib.rs:351; crate zaten INV-T9 #80 kırık + CI exclude). Karakterizasyon V1 lane test
  çağrıcısı (tests/common:2213-2258).
- **`PredicateGate`** (trajectory.rs:1608; evaluate :1646 — `gate_decision` SABİT
  `PassedAll`, outcome completion+decision'dan): production çağrıcılar commit (:2001),
  approve (:2315), observer counterfactual (:363). `assess_improvement_v1` +
  `evaluate_decision_core` + `trajectory_loss` V2'nin de kullandığı shared machinery —
  KALIR.
- **CLI deprecated `authority` alias** (run_envelope.rs:162; "deleted mirror until #100"
  pin completed_loop.rs:811-814).
- **`TaskCommitInput.target`/`loss_before`** (engine.rs:108-145; doc: "fiziksel removal
  #100 smart ctor tamamlaması"). Çağrıcılar: navigator :923 (+test :1736/:1826/:1938/:1987/
  :2081/:3587), MCP server.rs:1032, tests/common V2 lane (:2469 + `project_v1_loss_before_
  compatibility_v2` :2608 — "Faz 8a V2 typed loss evidence cutover'ında kaldırılacak").
- **`classify_baseline_availability`** (engine.rs:1843): measurement-yolu partition'ının
  commit'te ikinci üretimi (#131 P3-4 drift riski) — token baseline ile tek truth'a iner.
- Bayat doc'lar: INV-T3 MD-1 notu silinmiş `subject_authority.rs` modülünü anıyor
  (invariants.md:190-200); agent.rs:102/118 `affected_nodes` hâlâ "ölçüm scope"; server.rs:845
  `compute_raw_from_delta` atfı; common:2355 "commit_task_claim is still V1" notu.

### Spec durumu

INV-T6/T8/T9 MD-3 extension'ları #97 S4'te implemented; INV-T4 MD-2 notu implemented diyor
ama header "planned (Aşama A)" (:203) + item 5 "AÇIK — #100" (decisions:339). Karar
kaydının global header'ı + MD-3 "planned" başlıkları bayat.

## Tasarım kararları

**TD-1 — Token baseline (ölçüm anı, tek session):** `NativeSubjectMeasurement` +
`baseline: MeasurementBaseline` alanı. `measure_attempt_native` subject partition'ını
(measure_task_delta :3291-3333 ile AYNI mantık) koşar: Available → before centroid (base
space, AYNI `BoundMeasurementSession`); Unavailable{AllMembers|PartialNew} → typed reason.
Deserialize YOK (token zaten Serialize-only) — wire kırılımı sıfır. Commit artık baseline'i
ARTIFACT'ten okur; `classify_baseline_availability` ikinci üretimi silinir (#131 P3-4 tek
truth'a iner — partition ölçüm anında bir kez).

**TD-2 — Commit-anı artifact rekonstrüksiyonu + outer proof:** 5-fence başarıyla bittikten
sonra engine `EngineMeasurement` kurar: before = token.baseline; after = token.measured;
context = 5-fence'in zaten açtığı session'dan; request = `MeasurementRequest::try_new
(token.subject_scope, derive_impact_scope(claim), token.base_revision, canonical_delta,
context)`. Ardından Faz 3 commitment türetim bloğu (task_claim/task_goal/policy digest +
preferred_vector_snapshot, engine.rs:1166-1236) commit yoluna taşınır →
`VerifiedTaskMeasurementBinding`. 5-fence (delta/raw/revision/context/epoch) Faz 3
check'lerini kapsar; task identity MD-1 fence + evaluator'ın TaskIdentityMismatch'i.
Eski `verify_measurement_binding` (Faz 3, test-only) silinir; `measure_task_delta` public
measure API olarak kalır.

**TD-3 — V2 tüketim + V1 wire projection (frozen):** `commit_task_claim` Phase 0d →
`evaluate_task_gate_v2` → MD-3 matrisi V2 çıktıları üzerinde (completion =
`predicate_basis.result`; baseline class = `measurement.before()`; policy task'tan) → Q6 →
`build_authorization_context_v2` (otorite checkpoint — witness validate_for + gate↔basis
parity) → **V1 `AuthorizationContext` wire record V2 parçalarından türetilir** (V1 wire
FROZEN; `EngineCommitResult`/navigator/MCP/persistence yüzeyi değişmez). AttemptOutcome
map: `gate_decision: PassedAll` (V1'de de sabitti), completion/decision V2'den;
`loss_after` telemetry skaleri `trajectory_loss(after, preferred)` olarak kalır.
INV-T6 düşürme bloğu SİLİNİR — V2 (`compute_completion_first_loss_and_decision`)
Unavailable altında improved=false/Reject'i native üretir; md3_* matris testleri yeşil
kalmalı. `AuthorizationReceiptV2`/persistence-write-V2 (Faz 8 plan notu) BU issue'da
YOK — wire frozen ilkesiyle sınırlı; ayrı karar.

**TD-4 — `TaskCommitInput` smart ctor tamamlama:** `target`/`loss_before` alan+param
düşer; `new(finalized, omega, task_resolver)`. Loss target = task `preferred_vector`
(binding snapshot'ından), loss_before = `measurement.before()` derive. Semantik
değişimler (sınıflandırılmış — "expected semantic change"):
1. loss_before: navigator running skaleri (bootstrap seed'li) → ölçüm-türevli baseline
   loss. Available baseline'da ilk attempt'te değer değişir (bootstrap (0.7,0.5,...)
   değil before-centroid) → improved/decision kaymaları mümkün.
2. preferred_vector=None + AcceptImprovement + NotCompleted → V1'de caller target'ıyla
   progress üretilebiliyordu; V2 NoPreferredVector → **Reject** (fail-closed — INV-T6
   epistemolojisi).
3. Unavailable + improvement: V1 MD-3 düşürmesi ≡ V2 native — davranış korunur.
Regolden reason kodu: **`engine-cutover (#100)`** (eski değerler tarihçe notuyla korunur).

**TD-5 — `approve_cold_start` V2 revalidation:** kayıt `target`/`loss_before` düşer
(artık commit girdisi yok); 8. adım `evaluate_task_gate_v2` completion revalidation'ına
döner (token baseline'iyle aynı rekonstrüksiyon). Cold-start kabul kanıtı değişmez.

**TD-6 — MD-2 observer fiziksel kaldırım:** `provenance_authority.rs` + navigator wiring
(:903, finalize çağrıları, `provenance_authority_drift` alanları) + MCP additive JSON +
CLI envelope alanı + ilgili testler. Durable sidecar silinimi → yeni reader eski kaydı
reject (bilinçli epoch; #95-B W3 emsali — karar notu stage içinde face-face tabloyla).
Faz 8-P2 karakterizasyon KORPUSU (manifest + test dosyaları) dokunulmaz — tarihsel
görgü tanıkları; sadece production observer gider.

**TD-6 wire-yönü karar notu (S3 uygulandı — #95-B W3 emsali):**

| Yüzey | Serde karakteri | Kaldırma etkisi | Karar |
|---|---|---|---|
| `TrajectoryEvidence` (trajectory.rs) | derive; `deny_unknown_fields` YOK; alan `#[serde(default)]` Option | İki yön de kabul (eski reader yeni kaydı default-None; yeni reader eski kaydı ignore) | Sorun yok — direkt kaldırıldı |
| `PendingAuthorization` (durable record) | custom Deserialize + `deny_unknown_fields` (INV-T9 strict wire) | **Yeni reader eski kaydı unknown-field reject eder** | **Bilinçli epoch ilerlemesi** — reason `engine-cutover (#100)`; mevcut durable kayıtlar yalnız dogfood Temp fixture'ları (yeniden üretilebilir), production deployment yok (#95-B W3 tablosuyla aynı gerçeklik) |
| `RevisionRequired` (durable record) | custom Deserialize strict (aynı aile) | Aynı yön | Aynı karar |
| MCP response JSON | additive (error semantiği değişmemişti) | Geri alma non-breaking | Kaldırıldı |
| Digest güvenliği | alan hiçbir digest preimage'ine girmiyordu (test pinliydi) | — | Epoch ilerlemesi authorization identity'yi DEĞİŞTİRMEZ; yalnız eski kayıtların okunabilirliğini kapatır |

**TD-7 — `PredicateGate` üretimden silinir; karakterizasyon V1 lane test-lokal olur:**
`PredicateGate`/`PredicateGateInput`/`PredicateGateOutput` trajectory.rs'ten silinir
(observer S-i'inde önce gider — tek production çağrıcı kalmaz). tests/common V1 reference
evaluator'ı (`ReferenceEvaluation`, common:2292) test-lokal ince wrapper'a taşınır
(shared `assess_improvement_v1`/`evaluate_decision_core`/`trajectory_loss` production'da
kalır — V2 kullanıyor). `measurement_v1_v2_parity.rs` (3039 satır) tarihsel karakterizasyon
OLARAK KALIR (#95-B kararı): V1 lane non-authoritative lokal evaluator, V2 lane artık
gerçek V2 commit yolundan geçer — "tüm Faz 8-P2 characterization testleri yeni V2 path
ile yeşil" kabulü böyle sağlanır.

**TD-8 — V1 ölçüm projeksiyon silinimi:** `provenanced_from_raw` production'dan silinir;
bootstrap seed (MCP/CLI) synthetic pozisyonu **Placeholder** damgasıyla üretir (uniform
Scip = source laundering; kaynaklar çıktıda gözlenmediği için davranış etkisi yalnız
iç telemetry). `compute_raw_from_delta`/`try_compute_raw_from_delta` (DefaultFallback)
silinir; osp-desktop demo çağrıcısı en küçük dürüst yamayla onarılır (crate zaten CI
dışında/kırık). Karakterizasyon harness'i ihtiyaç duyarsa V1 fonksiyonların test-lokal
kopyalarını alır (q92 emsali — history goldens + karar kaydında yaşar).

**TD-9 — CLI alias + doc cleanup:** deprecated `authority` mirror alanı + envelope +
pin'li test güncellenir ("alias absorbe"). agent.rs `affected_nodes` doc'u advisory
impact metadata'ya çevrilir; bayat yorumlar (server.rs:845, common:2355, INV-T3 MD-1
notu) düzeltilir.

**TD-10 — Spec/karar kaydı:** INV-T4 header + MD-2 item 5 kapanışı; INV-T3 MD-1 notu
(veya o not issue #100 kapsamında düzeltilir); INV-T6/T8/T9 zaten implemented (#97) —
engine cutover bağlantılı cümleler varsa güncellenir. Karar kaydı: global header, MD-2
item 5 "TAMAMLANDI (#100)", MD-3 "planned" başlıkları, #100 follow-up bölümü teslim
kaydı. Case 2/3: #95-A zaten parity'ye regoldenledi (`subject-cutover`); #100'nin
getireceği değer değişimleri `engine-cutover (#100)` reason'ıyla regoldenlenir.

## S-planı (her stage commit + fmt/clippy/test yeşil)

- **S0** Bu doküman + 96-handoff merge kaydı güncellemesi.
- **S1** Token baseline: `NativeSubjectMeasurement.baseline` + `measure_attempt_native`
  partition/before üretimi (aynı session) + partition-parite pin'leri (measure_task_delta
  before ile eşdeğerlik). Salt additive.
- **S2** Engine cutover (en büyük): TD-2 rekonstrüksiyon + outer proof üretimi;
  commit_task_claim V2 zinciri (TD-3); MD-3 matrisi V2 çıktılarına; INV-T6 bloğu silinir;
  V1 wire projection builder; TaskCommitInput imza daralması (TD-4) + navigator/MCP/
  test çağrıcıları; approve_cold_start V2 (TD-5) + kayıt slimming. Eski Faz 3
  `verify_measurement_binding` silinir (commit2_* standalone test'leri yeni yolla).
- **S3** MD-2 observer fiziksel kaldırım (TD-6): modül + wiring + sidecar'lar + MCP JSON +
  CLI alan + testler; wire-yön face-face karar notu (commit mesajında + bu dosyada).
- **S4** `PredicateGate` silinimi (TD-7) + tests/common V1 reference evaluator test-lokal
  taşınır; parity testleri yeni V2 commit yoluyla yeşil; `project_v1_loss_before_
  compatibility_v2` silinir (seam kapandı).
- **S5** V1 ölçüm projeksiyon silinimi (TD-8) + bootstrap Placeholder damgası +
  osp-desktop onarımı + `authority` alias (TD-9) + doc cleanup.
- **S6** Kabul: spec flip'ler (TD-10) + regolden (`engine-cutover (#100)`) + karar kaydı
  teslim bölümü + CI parity tam paket + PR.

## Kabul checklist (issue #100)

- [ ] `commit_task_claim` V2 authorization consumer (legacy PredicateGate üretimden silik)
- [ ] V1 compatibility projection'lar tamamen kaldırılmış (uniform-Scip reference lane,
      DefaultFallback, provenanced_from_raw, observer, alias)
- [ ] Case 2/3 + etkilenen golden'lar expected semantic-change regolden (tarihsel bağ)
- [ ] Q5/predicate/policy güncellemeleri sınıflandırılmış (yukarıdaki 3 sınıf + kayıt)
- [ ] INV-T4 (ve bağlantılı INV-T3 notu) planned→implemented; INV-T6/T8/T9 #97'den
      implemented — cutover cümleleri güncel
- [ ] Tüm Faz 8-P2 characterization testleri yeni V2 path ile yeşil

## Ortam notları (Windows)

`export PATH="$HOME/.cargo/bin:$PATH"`; `command grep`; `node -e` (python yok). ASLA
`git add -A`. Commit formatı `feat: #100 …` (scope parens YOK). CI parity: `cargo fmt
--all -- --check` + `cargo clippy --locked --workspace --all-targets --all-features
--exclude osp-desktop -- -D warnings` + aynı target setinde tam test paketi.
