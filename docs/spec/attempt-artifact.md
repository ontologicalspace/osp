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
2. **`--out <path>` (opsiyonel):** aynı içeriğin çağırıcıya ait kopyası (atomic yazım;
   üzerine yazmak çağırıcının açık isteğidir — canonical garanti 1. maddededir).

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
    "repository_head": "<40-hex SHA>"
  },
  "execution_measurement": {
    "subject_authority": "task_scope",               // #95-A MD-1
    "provenance_authority": "engine_native_per_axis", // #96 MD-2
    "provenance_native": true
  },
  "result": { "kind": "completed", "attempts": 2 },
  "evidence": [ /* TrajectoryEvidence[]: before/after + gate/mutation/completion kararları */ ]
}
```

`result.kind`: `completed | awaiting_witnesses | exceeded_maneuver_limit |
requires_revision | requires_operator_approval | awaiting_cold_start_approval |
task_not_found | witness_evaluation_error |
pending_authorization_persistence_failure | system_failure | llm_error`.

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
