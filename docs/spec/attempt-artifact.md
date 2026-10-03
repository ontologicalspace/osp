# Canonical Attempt Artifact (`osp trajectory attempt`) — #166

`osp trajectory attempt` her çalışmada **şemalı run envelope v1**'i iki yerde kalıcı
yazar; kanıt hattı (`dogfood/ledger.jsonl` → `attempt_ref`) stdout biçiminden
bağımsızlaşır:

1. **Kalıcı kayıt (daima):** `<state-dir>/attempts/task-<task_id>-<unix_millis>.json`
   — her attempt kendi artifact'ını alır (yeniden koşu üstüne yazmaz).
2. **`--out <path>` (opsiyonel):** aynı içerik çağırıcının seçtiği yola kopyalanır.

Yazım, exit-code dallanmasından **önce** yapılır: maneuver-limit / awaiting-witnesses
gibi sıfır-dışı çıkışlarda da kanıt eksiksizdir.

## Akış ayrımı (human modu)

- **stdout:** yalnız makine-okunur **evidence JSON dizisi** (`TrajectoryEvidence[]`;
  evidence boşsa stdout boş) — elle ayıklama gerekmez.
- **stderr:** progress satırları (`✓ Task completed …`, `Evidence entries: N`) ve
  artifact path bildirimleri (`Canonical attempt artifact: …`, `Attempt artifact
  (--out): …`).
- **`--format json`:** stdout'a tam envelope (tek JSON nesnesi), diagnostics stderr'de
  (önceden olduğu gibi).

## Envelope şeması (v1)

```json
{
  "schema_version": 1,
  "run": {
    "execution_mode": "harness",          // production | harness
    "witness_mode": "harness_auto_approve", // production | harness_auto_approve
    "task_source": "harness_task_file",   // harness_task_file | legacy_hardcoded
    "repository_head": "<40-hex SHA>"
  },
  "execution_measurement": {
    "subject_authority": "task_scope",          // #95-A MD-1
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
  kullanır), #172 (finalize-run absorbe — ledger `attempt_ref` bu artifact'ı gösterir).
- Kod: `crates/osp-cli/src/commands/run_envelope.rs` (şema),
  `commands/mod.rs` (`persist_canonical_attempt_artifact`, `print_human_result`).
