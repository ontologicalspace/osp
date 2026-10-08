# Canonical Attempt Artifact (`osp trajectory attempt`) — #166

Başarılı preflight'in ardından **navigator'a ulaşan her attempt** şemalı run envelope
v1'i iki yerde kalıcı yazar; kanıt hattı (`dogfood/ledger.jsonl` → `attempt_ref`)
stdout biçiminden bağımsızlaşır:

- **Kapsam (P2-1):** mod-guard/state-dir fence/analyze/task-load/proposals/LLM-init
  hataları navigator'a ULAŞMAZ — artifact üretmezler. Artifact, `nav.run_task()`
  sonucu ÜRETEN her yürütme içindir (Completed dahil tüm sonuç türleri).
- **Yayın sırası (P1-1):** navigator yürütmesi → **final snapshot fence
  (`validate_post_attempt_snapshot`) HER sonuç için** → canonical publish → emit →
  exit. Fence başarısızsa canonical artifact YOKTUR (katı yol: drift'li attempt için
  ayrı diagnostic artifact bilinçli olarak yazılmaz — "canonical" unvanı
  fence-geçmiş kanıta özeldir; `measured + subject-validity + persistence-validity
  = canonical evidence`).

Yazım yerleri:

1. **Kalıcı kayıt (daima):** `<state-dir>/attempts/task-<task_id>-<unix_millis>-<pid>[-N].json`
2. **`--out <path>` (opsiyonel):** aynı içeriğin çağırıcıya ait kopyası — unique
   same-dir temp + `create_new` + write/sync + rename (temp adı caller verisine
   değmez); üzerine yazmak çağırıcının açık isteğidir — canonical garanti 1.
   maddededir. **Preflight hedef fence'i:** analyzed repo içi ve `<state-dir>/.osp/**`
   + `<state-dir>/attempts/**` canonical mağazaları reddedilir; reserved root'lar
   symlink/junction üzerinden GERÇEK hedeflerine çözülerek karşılaştırılır
   (`canonicalize_with_missing_tail`; state-dir kökü serbest; relative `--out`'ta
   CWD çözülemazsa fail-closed).

**Persistence modeli (P1-2)** — pending-authorization/space-identity precedent'i:
same-dir temp (`create_new`) → `write_all` + `sync_all` → **`hard_link` no-clobber
publish** (`fs::rename` hedefi REPLACE eder — no-clobber değildir) → parent-dir sync.
Crash penceresi: hedef ya YOKtur ya TAM içeriktir; yarım canonical dosya üretilemez.
Kimlik `task_id + unix_millis + pid` (+ çakışmada `-N` soneki, 64 deneme bütçesi);
zaman tek başına kimlik değildir — `hard_link` AlreadyExists ayrımı zorlar (fail-closed).
Her attempt kendi artifact'ını alır; yeniden koşu üstüne yazmaz.

## Akış ayrımı (human modu)

- **stdout:** HER ZAMAN geçerli **evidence JSON dizisi** (`TrajectoryEvidence[]`;
  zero-evidence outcome'larda `[]` — P1-3; `serde_json::from_reader(stdout)` her
  durumda çalışır).
- **stderr:** progress satırları (`✓ Task completed …`, `Evidence entries: N`,
  `✗ Task {id} not found`, …) ve artifact path bildirimleri (`Canonical attempt
  artifact: …`, `Attempt artifact (--out): …`).
- **`--format json`:** stdout'a tam envelope (tek JSON nesnesi), diagnostics stderr'de.

## Envelope şeması (v1)

```json
{
  "schema_version": 1,
  "run": {
    "task_id": 7,                          // P2-2: evidence boş olsa da self-describing
    "execution_mode": "harness",           // production | harness
    "witness_mode": "harness_auto_approve", // production | harness_auto_approve
    "task_source": "harness_task_file",    // harness_task_file | legacy_hardcoded
    "repository_head": "<40-hex SHA>",
    "task_digest": "sha256:<64-hex>",      // #178: --task baytları (read-once tampon)
    "proposals_digest": "sha256:<64-hex> | null" // #178: --proposals baytları; null = tüketilmedi (#171)
  },
  "execution_measurement": {
    "subject_authority": "task_scope",               // #95-A MD-1
    "provenance_authority": "engine_native_per_axis", // #96 MD-2
    "provenance_native": true
  },
  "completion_basis": "graph", // INV-T10 (#196): tamam-iddia kanıt zemini —
                               // yalnız result.kind "completed" için "graph",
                               // diğer kind'lar null (iddia yoksa zemin de yok).
                               // "realized" attempt-anında temsil EDİLEMEZ — yalnız
                               // RealizationGate kanıt-jetonu (VerifiedRealization)
                               // üzerinden gelir. "completed" = GraphCompleted'tir —
                               // mainline tamamlama iddiası DEĞİLDİR (#171 D5a:
                               // graph predicate success ⇏ build-valid declared
                               // realization; B: E_c=0 iken build çöktü).
  "result": { "kind": "completed", "attempts": 2 },
  "evidence": [ /* TrajectoryEvidence[]: before/after + gate/mutation/completion kararları */ ]
}
```

`result.kind`: `completed | awaiting_witnesses | exceeded_maneuver_limit |
requires_revision | requires_operator_approval | awaiting_cold_start_approval |
task_not_found | witness_evaluation_error |
pending_authorization_persistence_failure | system_failure | llm_error`.

`completion_basis`: `graph | realized | null` — INV-T10 tip-ayrımının wire yüzü
(`osp_core::realization::CompletionBasis`; snake_case). `realized`, attempt-anında
temsil edilemez: yalnız `RealizationVerdict::RealizedCompleted` üzerinden — ve o
varyant **kanıt-jetonu** (`VerifiedRealization`: private alanlar, public
constructor YOK; gate PR'ının `try_from_gate` girişi) taşır. Karar (0) (freeze
#196/6050269005): beyan dışı semantic work yeni proposal'dır;
`DeclaredRealizationBuildInvalid` gerçek-dünya hükmü değil, beyan-realization'ın
build sonucudur ve **tam raw evidence taşır** (E_c=0 ∧ BuildFailed eşleşmesi
kaybolmaz). Eski (alansız) envelope'lar: missing ≡
`kind==completed ? graph : null` (realized katmanı hiç var olmadı).

### #178 — tüketilen-girdi digest'leri ve güven kökü

`run.task_digest` / `run.proposals_digest`, attempt'in **karar verdiği exact girdi
baytlarının** `sha256:<64-hex>` özetleridir: dosya TEK okumayla alınır, digest ve
parse AYNI tampondan üretilir (`load_and_validate_harness_task_str` /
`load_proposals_str`); navigator sırasında diskteki dosya değişse bile zarfın
beyanı tükettiği tampona bağlı kalır. `--llm real` proposals dosyası tüketmez →
`proposals_digest: null` (#171 dürüst boşluğu).

**Güven kökü hiyerarşisi (#178 tur-2/tur-3):** digest'ler yalnızca onları taşıyan
zarf güvenilir olduğunda kanıttır; `run/attempt.json` caller-owned, overwrite
edilebilir bir **kopyadır**. `osp finalize-run` bu yüzden (a) digest
fence'lerinden ÖNCE kopyayı no-clobber canonical mağazayla (`--state-dir`;
default probe `<run_dir>/../../state`) **bayt-özdeşliğe** göre doğrular:
mağazada task için artifact VARSA eşleşmeyen kopya RED (tutarlı-tamper ve
alan-silme bypass'larının ikisi de kopyayı değiştirir, canonical'ı değil);
(b) **anchor bulunamadığında (mağaza yok / taşınmış run-dir / task kaydı yok)
otomatik legacy kabul YOKTUR** — digest alanlı (yeni-şekil) zarf her koşulda
RED (anchor zorunlu; attempt yeniden koşulmalı ya da `--state-dir`
geri getirilmeli); legacy-şekil (digest alansız) zarf da RED, yalnız
`--allow-unanchored-legacy` **açık trust downgrade**'iyle geçer — bu bayrak
yalnızca pre-#178 historical artifact'lar için anlamlıdır ("gerçekten eski
artifact" ile "alanları silinmiş yeni artifact + taşınmış run-dir" dış bilgi
olmadan ayırt edilemez; sessiz `trusted→untrusted` geçişi epistemik olarak
kabul edilemez) ve kullanımı ledger satırına `unanchored_legacy: true` olarak
yazılır (anahtar YALNIZ downgrade'de eklenir — anchor'lu satırlarda alan
yoktur; missing ≠ false ≠ null-beyan). Presence semantiği: alan YOK =
legacy-şekil ≠ `null` = "tüketilmedi"
(run-dir'de proposals.json varsa RED) ≠ değer = "bu baytlar" (dosya zorunlu +
hash eşit; silinmek RED).

## INV-T10 — `osp realization-gate` (#196 uygulama-2; karar 2: gate konumu = CLI ritüeli)

Graph-completed attempt'i source gerçekliğine bağlar (#171 D5a: graph predicate
success ⇏ build-valid declared realization):

```text
osp realization-gate <run-dir> [--state-dir S] --evidence <declared.json>
```

- **Girdiler:** `attempt.json` (canonical-anchor bayt-doğrulamalı — legacy
  downgrade kapı bağlamı DEĞİL), `task.json` (v1: tek Coupling/Le predicate +
  Path scope), `after.json` (reanalysis gözlemi) ve `--evidence`
  (insan-beyanlı: `patch_created/parse/build{outcome,error_count}/tests`;
  deny_unknown_fields).
- **Motor türetimleri (D5a şemasının motorlaşması):** `c_observed` =
  after.json scope-node coupling'i; `c_predicted` = zarf evidence son
  `after.x` (sim öngörüsü); `predicate_after_reanalysis` = observed ≤
  threshold + tolerance; `E_c = c_observed − c_predicted` kanıtta yaşar.
- **Verdict:** `evaluate_gate` → `realization-verdict.json` (run dizinine,
  **no-clobber — tek yazım**; yeniden değerlendirme = yeni run-dir). Exit:
  0 = RealizedCompleted · 2 = DeclaredRealizationBuildInvalid ·
  3 = PredicateUnsatisfiedAfterReanalysis · 4 = NotAttempted.
- **Tolerans v1 (karar 1 dürüst kesiti):** RealizedCompleted yalnız
  `build=Succeeded ∧ predicate_after_reanalysis=Some(true)`; sayısal `D_G`
  eşiği YOK — eşik verisi birikene dek uydurulmaz ("koordinat ölçümle
  kazanılır"). `completion_basis` ledger'a finalize-run üzerinden taşınır
  (yalnız graph-completed satırlar `"graph"`).

## Geriye uyumluluk notu

Run 10 (`0b88e7e`) çıktısı `schema_version: 1` zarflıydı; ara revizyonlarda
default-insan çıktısı progress + çıplak diziye döndü ve kanıt stdout'tan elle
ayıklanmak zorunda kaldı (run 11 sürtünme kaydı, issue #166). Bu sözleşme zarfı
**kalıcı artifact** olarak geri getirir; stdout biçimi artık sözleşmenin parçası
değildir (yalnızca evidence dizisi + akış ayrımı taahhüdü).

## İlgili

- Issue #166 (süreç kaynağı), #171 (LLM-önerili stratum — aynı artifact sözleşmesini
  kullanır), #172 (finalize-run absorbe — ledger `attempt_ref` bu artifact'ı gösterir),
  #173 (fixture-vacuity — graf önermesi sanity-testi disiplini).
- Kod: `crates/osp-cli/src/commands/run_envelope.rs` (şema),
  `commands/mod.rs` (`AttemptExecution`, `persist_canonical_attempt_artifact`,
  `emit_attempt_output`, `print_human_result`).
