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

## 2. Task dosyası: authority profile (exhaustive matris) + constraints vs predicates

Task dosyası şeması = CLI harness formatı (`schema_version: 1`, HEAD-bound;
şablon: `docs/quickstart.md` Step 2a). Üç disiplin:

1. **Her axis, Live Contract v1 authority profile'ına uymak zorundadır**
   (attempt pipeline'da preflight uygular — `validate_attempt_measurement_authority`;
   sessiz geçiş yok, matris exhaustive'tir):

   | Eksen | v1 kuralı | Uymayan durum |
   |---|---|---|
   | coupling | `required_source: TreeSitter` **declare edilmeli** | başka değer VEYA None → `UnsupportedMeasurementAuthority` |
   | instability | `required_source: TreeSitter` **declare edilmeli** | — " — |
   | cohesion | attempt v1'de **kullanılamaz** (Tier-1, Scip ölçüm yok) | her durumda → `ScipMeasurementUnavailable` |
   | entropy | `required_source` **declare EDİLMEMELİ** (pipeline-derived preset; INV-T4 ölçümde değerlendirir) | declare edilirse → `UnsupportedMeasurementAuthority` |
   | witness_depth | `required_source` **declare EDİLMEMELİ** | — " — |
   | RiskScore / MainSequenceDistance / Custom | attempt v1'de **reddedilir** | `UnsupportedPredicateAxis` — `MeasuredRawPosition::axis()` bu eksenlerde legacy coupling fallback'u yapar; sessiz ontolojik fallback önlenir |

   Gerekçe (INV-T9 #70): topology source coupling + instability provenance'ını birlikte
   bağlar — SCIP index yüklemek bu authority'yi değiştirmez. "SCIP index'ten veri
   yüklendi" ≠ "bu eksenin authoritative kaynağı Scip".

   *Validation-sırası notu:* `required_source: Mixed` çoğu durumda bu preflight'tan
   DAHA ÖNCE reddedilir — core `task.validate()` (harness loader'ın çağırdığı)
   `InvalidRequiredMetricSource` ile fail-closed yapar. Daha erken fail-closed'dür;
   preflight satırı, Mixed'in core'dan geçtiği varsayımsal yollar için yedek savunmadır.

3. **Operation profile: v1 op-matrix (delta alanı ↔ OpKind karşılığı).**
   `allowed_operations` policy'si navigator'da **her delta alanı için karşılık gelen
   OpKind'i ister** — sessiz yapısal genişletme yoktur:

   | DeltaProposal alanı | Gerekli OpKind |
   |---|---|
   | `removed_edges` | `RemoveImport` |
   | `new_nodes` | `AddNode` |
   | `new_edges` | `AddEdge` |
   | `modified_entities` | `ModifyEntity` |

   (`connected_to`, `new_nodes` spesifikasyonunun parçasıdır — `AddNode` kapsamında.)
   İzin verilmeyen op → `RejectedByRule` (gate katmanı), task'ın izin poliçesiyle
   denetlenmeyen conceptual genişletme imkânsızdır. Diğer `OpKind` varyantları
   (`AddImport`, `AddAbstraction`, `ExtractModule`, `RemoveNode`, `RemoveEdge`…)
   delta-alan karşılığı gelmedikçe task'ta listelense de hiçbir alanı açmaz.
   Agent, task'ın izin vermediği structural operation'ları yapamaz; ilk dogfood
   dataset'inin epistemik temizliği bu matrix'e dayanır.

4. **`constraints` insan-taraflı taahhütlerdir, `predicates` ölçülebilir
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

**Raw dogfood artifact'ları lokal ve versiyonsuzdur** (`/dogfood/` → `.gitignore`):
`applied.patch`, `baseline.json` ve path'ler gerçek projeden bilgi taşır; public
repo'ya YAYIN yalnızca açık bir **sanitized export** ile yapılır (Paper 4 için:
`raw dogfood → sanitize/export → docs/results/paper4-dataset/`). Aynı gerekçe
self-hosting (Faz 2) içindir: OSP kendi checkout'unda harness clean-worktree ister —
dogfood checkout dışında/ignore'lı kalmalıdır.

Her run bir JSONL satırı üretir (markdown yalnızca render'dır; analiz
`load_dogfood_runs()` ile JSONL'den okur):

```json
{
  "schema_version": "live-ledger-v1",
  "run_id": "2026-09-30-extract-pricing-adapter",
  "contract_version": "v1",
  "repository": "nexus",
  "repository_head": "<sha>",
  "osp_revision": "<osp-checkout-HEAD-sha>",
  "osp_version": "<crate-version>",
  "analysis_profile": "tier1 | tier2-scip",
  "commands": {
    "baseline_analysis": "osp analyze <repo>",
    "attempt": "osp trajectory attempt 1 --repo … --task … --execution-mode harness …",
    "after_analysis": "osp analyze <repo> | null"
  },
  "scip_index_digest": null,
  "task_ref": "dogfood/runs/<run>/task.json",
  "task_digest": "sha256:… | null",
  "baseline_ref": "…/baseline.json",
  "proposal_refs": ["…/proposals.json"],
  "proposal_digest": "sha256:… | null",
  "attempt_ref": "…/attempt.json",
  "decision": "accept | reject | defer",
  "patch_ref": "…/applied.patch | null",
  "patch_digest": "sha256:… | null",
  "after_ref": "…/after.json | null",
  "decision_utility": "decision-changed | decision-confirmed | none",
  "counterfactual": "verified-bad | inspected-ok | unverified | null",
  "human_override": false,
  "friction": ["task-prep: …", "cli: …"],
  "notes": "…"
}
```

**Reproducibility alanları zorunludur:** aynı `repository_head + task + proposal`
farklı OSP revizyonlarında farklı measurement/decision üretebilir; `osp_revision` /
`osp_version` / `analysis_profile` / `commands` (üç adım ayrı ayrı — baseline
analizi, attempt, after analizi) / (`tier2-scip`'te) `scip_index_digest` olmadan
"aynı girdi → aynı karar" denetlenemez. `*_digest` alanları v1'de opsiyoneldir
(null), Paper 4 dataset'ine export edilmeden önce doldurulurlar.

`decision_utility` / `counterfactual` programın iki **kritik-eşik olayının**
alanlaşmış hâlidir (#151): "OSP yüzünden başka implementasyon seçtim"
(`decision-changed`) ve "red gerçek bir bozulmayı gösteriyordu"
(`verified-bad`). İlk gerçekleşmeler ayrıca `notes.md`'de olay olarak
kaydedilir.

## 5. İlk görev adayı — Nexus (Faz 1 başlangıç taslağı)

Tip: **refactoring — gereksiz/doğrudan bir bağımlılığı kaldırarak coupling'i düşürme**
(küçük, ölçülebilir, RemoveImport ile representable ilk görev). "Adapter extraction"
(`Service → Adapter → Provider`): v1 op-matrix'te dürüstçe temsil edilebilir hâldedir
(`AddNode` + `AddEdge` izniyle) ama ilk görev olarak bilinçli olarak İKİNCİ sıradadır —
delta-alan karşılığı yeni açıldı; ilk run tek-op (RemoveImport) profilinde kalsın.
Task iskeleti (değerler ilk baseline koşusuyla dolar; `node_id`/path binding'ler
Nexus analizinden gelir):

```json
{
  "schema_version": 1,
  "repository_head": "<nexus-HEAD>",
  "scope_bindings": [{ "node_id": 0, "expected_path": "<PricingService-path>" }],
  "task": {
    "id": 1, "milestone_id": 1, "label": "remove unnecessary direct provider dependency (coupling ↓)",
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
