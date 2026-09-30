# OSP Live Contract — Canlı Kullanım Davranış Kontratı (v1)

> **Durum:** #151 canlı kullanım programı Faz 0 artefaktı (2026-09-30).
> Bu belge kod değil **davranış kontratı** dondurur: canlı koşularda hangi zincir
> izlenir, hangi artifact saklanır, ledger'a ne yazılır. Paper 4 deney planındaki
> Phase 0 freeze (D1-D9/F1-F4) **ayrı ve sonraki** bir iştir — isim benzerliği
> kapsam karışıklığı yaratmasın.
>
> Değişiklik disiplini: bu kontrat canlı koşular toplandıkça **versiyonlanır**
> (`v1 → v2`); her run hangi kontrat sürümünde üretildiğini ledger'da kaydeder.

## 1. Karar zinciri ve artifact'ler

Her canlı görev bir **run**'dur ve zincirin her adımının artifact'ı saklanır:

```
Baseline → Task → Proposal → Attempt → Decision → Patch → Reanalysis
   M(S0)                                                            M(S1)
```

Run dizini (OSP checkout'ı altında, çalışılan repo'ya işaret eder):

```text
dogfood/runs/<YYYY-MM-DD>-<task-slug>/
  task.json          # harness formatında task dosyası — goal + constraints + predicates (§2)
  baseline.json      # M(S0): `osp analyze <repo>` çıktısı (Tier-1 günlük; Tier-2 haftalık)
  proposals.json     # agent'ın ürettiği structural proposal'lar
  attempt.json       # `osp trajectory attempt` kanıtı: üç-katman AttemptOutcome
  decision.json      # insan/agent karar kaydı: kabul, red, erteleme + gerekçe
  applied.patch      # proposal ≠ applied change — GERÇEK uygulanan değişiklik
  after.json         # M(S1): yama sonrası `osp analyze` çıktısı
  notes.md           # gözlemler, sürtünme notları, counterfactual değerlendirme
```

**Neden:** birkaç ay sonra "OSP bunu neden kabul/red etmişti?" sorusu artifact'larla
**yeniden üretilebilir** olmalı. OSP bu kontratla static analyzer'dan **decision
provenance system**'e geçer.

## 2. Task dosyası: constraints vs predicates + authority declaration

Task dosyası şeması = CLI harness formatı (`schema_version: 1`, HEAD-bound;
şablon: `docs/quickstart.md` Step 2a). İki ekleme disiplini:

1. **Authority declaration zorunludur.** Her predicate kullandığı ölçütün
   `required_source`'unu declare eder. Uyuşmazlık task yüklenirken
   `UnsupportedMeasurementAuthority` / `ScipMeasurementUnavailable` ile reddedilir
   (sessiz fallback yok — PR #153). Geçerli kombinasyonlar (v1, attempt pipeline):

   | Eksen | Attempt authority | Not |
   |---|---|---|
   | coupling | `TreeSitter` | INV-T9 #70: topology source coupling+instability'i birlikte bağlar |
   | instability | `TreeSitter` | — " — |
   | cohesion | (kullanılamaz) | `ScipMeasurementUnavailable` — `analyze --scip` sağlar (#144-C ikinci iterasyon) |
   | entropy / witness_depth | unset bırak | ölçüm anında INV-T4 değerlendirir |

2. **`constraints` insan-taraflı taahhütlerdir, `predicates` ölçülebilir
   koşullardır.** "Yeni dependency cycle yok" gibi taahhütler task `constraints`
   alanında METİN olarak kalır ve `notes.md`'de insani olarak değerlendirilir;
   ölçülebilir koşullar `predicates`'te metric+operator+threshold+scope ile
   ifade edilir. Ölçülemeyeni ölçülebilir gibi yazmak yoktur.

## 3. Patch köprüsü (bilinçli tasarım)

```
OSP → Decision → İnsan / coding agent → Patch → Reanalysis
```

- Kabul edilen proposal **kaynak koda otomatik uygulanmaz** (ayrı, henüz
  kurulmamış katman). Yama, kararı izleyen insan/agent adımıdır.
- **`applied.patch` ledger'ın parçasıdır**: agent proposal'ı uygularken başka
  şeyler de değiştirebilir; gerçek deney `Δ = M(S1) − M(S0)`'dır ve yalnızca
  gerçek yama sonrası ölçümle kurulur.
- Reddedilen proposal'da `applied.patch` YOKTUR (`decision.json` "rejected" +
   boş bırakılır) — uzay ve dosyalar before durumunda kalır; bu da deneydir.

## 4. Ledger (canonical: `dogfood/ledger.jsonl`)

Her run bir JSONL satırı üretir (markdown yalnızca render'dır; analiz
`load_dogfood_runs()` ile JSONL'den okur):

```json
{
  "schema_version": "live-ledger-v1",
  "run_id": "2026-09-30-extract-pricing-adapter",
  "contract_version": "v1",
  "repository": "nexus",
  "repository_head": "<sha>",
  "task_ref": "dogfood/runs/<run>/task.json",
  "baseline_ref": "…/baseline.json",
  "proposal_refs": ["…/proposals.json"],
  "attempt_ref": "…/attempt.json",
  "decision": "accept | reject | defer",
  "patch_ref": "…/applied.patch | null",
  "after_ref": "…/after.json | null",
  "decision_utility": "decision-changed | decision-confirmed | none",
  "counterfactual": "verified-bad | inspected-ok | unverified | null",
  "human_override": false,
  "friction": ["task-prep: …", "cli: …"],
  "notes": "…"
}
```

`decision_utility` / `counterfactual` programın iki **kritik-eşik olayının**
alanlaşmış hâlidir (#151): "OSP yüzünden başka implementasyon seçtim"
(`decision-changed`) ve "red gerçek bir bozulmayı gösteriyordu"
(`verified-bad`). İlk gerçekleşmeler ayrıca `notes.md`'de olay olarak
kaydedilir.

## 5. İlk görev adayı — Nexus (Faz 1 başlangıç taslağı)

Tip: **refactoring** (greenfield değil — karşılaştırılabilir "before").
Aday: provider-özel erişimi adapter boundary arkasına alma. Task iskeleti
(değerler ilk baseline koşusuyla dolar; `node_id`/path binding'ler Nexus
analizinden gelir):

```json
{
  "schema_version": 1,
  "repository_head": "<nexus-HEAD>",
  "scope_bindings": [{ "node_id": 0, "expected_path": "<PricingService-path>" }],
  "task": {
    "id": 1, "milestone_id": 1, "label": "extract provider access behind adapter boundary",
    "target_predicate_set": {
      "mode": "All",
      "predicates": [
        { "predicate": { "metric": "Coupling", "operator": "Le", "threshold": "<baseline-ölçüldükten sonra>", "scope": { "Node": 0 }, "required_source": "TreeSitter", "tolerance": 0.0 }, "weight": null }
      ],
      "preferred_vector": { "x": 0.5, "y": 0.6, "z": 0.5, "w": 0.5, "v": 0.3 }
    },
    "policy": { "predicate_failure_policy": "StrictReject", "min_improvement_delta": 0.02, "max_axis_regression": 0.15, "maneuver_limit": 5, "allow_progress_checkpoint": false },
    "allowed_operations": ["RemoveImport"],
    "constraints": ["public API unchanged", "no new dependency cycles"],
    "status": "Pending"
  }
}
```

Bilinen sınırlar (v1): attempt Tier-1 ölçer (#151 ilk görev seti
coupling/instability odaklı); production modunda witness-suspension persisted
space identity gerektirir (#152) — **canlı döngü harness akışıyla yürür**
(`--execution-mode harness --witness harness-auto-approve --state-dir`, kanıt:
quickstart first-accept, exit 0).

## 6. Başarı ölçütleri (referans)

Dört eksen ve iki kritik-eşik olayının tanımı umbrella issue
[#151](https://github.com/ontologicalspace/osp/issues/151)'dedir; bu kontrat
onların veri üretim biçimini belirler.
