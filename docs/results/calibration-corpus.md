# OSP Kalibrasyon Korpusu (Faz 1.11)

> 15 repo — `MERGE_RATIO_OBSERVABLE` eşik + witness ağırlıkları + eksen normalizasyon
> sabitlerini tune etmek için. Python/JS/TS (graph extractor desteği); Rust/Go Faz 3 SCIP.
> Witness analizi dil-agnostik → tri-state heuristic tüm diller için geçerli.

---

## 1. Korpus Seçimi (15 repo)

| # | Repo | Dil | Kategori | Beklenen Workflow | Boyut (≈) |
|---|---|---|---|---|---|
| 1 | `pallets/click` | Py | küçük-temiz | merge-commit | küçük |
| 2 | `tiangolo/fastapi` | Py | modern framework | squash | orta |
| 3 | `django/django` | Py | büyük-olgun | squash | büyük |
| 4 | `psf/requests` | Py | olgun kütüphane | merge-commit (?) | orta |
| 5 | `pallets/flask` | Py | olgun framework | merge-commit (?) | orta |
| 6 | `encode/httpx` | Py | modern kütüphane | squash (?) | küçük |
| 7 | `Textualize/rich` | Py | modern kütüphane | squash (?) | orta |
| 8 | `pydantic/pydantic` | Py | modern validation | squash (?) | orta |
| 9 | `date-fns/date-fns` | TS | modüler kütüphane | rebase | büyük |
| 10 | `chalk/chalk` | JS | küçük-temiz | ? | küçük |
| 11 | `sveltejs/svelte` | TS | medium framework | ? | orta |
| 12 | `tj/commander.js` | JS | küçük CLI | ? | küçük |
| 13 | `lodash/lodash` | JS | olgun utility | merge-commit (?) | büyük |
| 14 | `vitest-dev/vitest` | TS | modern test framework | squash (?) | büyük |
| 15 | `pipeposse/worms-supabase` | Py | **foam** | direct (solo) | küçük |

**(?)** = bilinmiyor, kalibrasyon bunu ortaya çıkaracak (kör-test bütünlüğü).

## 2. Çeşitlilik Matrisi

| Boyut | Kapsam |
|---|---|
| **Dil** | Python (8), TypeScript (3), JavaScript (3), Foam-Python (1) |
| **Maturity** | olgun-büyük (3: django/lodash/vitest) · olgun-orta (3: requests/flask/date-fns) · modern (5: fastapi/httpx/rich/pydantic/svelte) · küçük-temiz (3: click/chalk/commander) · foam (1) |
| **Workflow (hipotez)** | merge-commit (click + ?) · squash (fastapi/django + ?) · rebase (date-fns + ?) · direct (worms) |
| **Ölçek** | 26 node (worms) → 3000+ node (django) |

## 3. Kalibrasyon Hedefleri

| Parametre | Mevcut değer | Tune kriteri |
|---|---|---|
| `MERGE_RATIO_OBSERVABLE` | %10 | merge-commit workflow'lu olgun repolar `Witnessed`, squash repolar `UnobservableLocally` çıkmalı |
| `WitnessKind::default_weight` | 1.0/0.8/0.7/0.4 | Korpus dağılımına göre ( Faz 1.11'de teorik kalıyor — ground-truth review verisi yok) |
| `θ_quorum` | 1.5 | Tek-merge-commit `Hold` (self-merge prevention), çift-merge `Commit` doğru çalışıyor mu |
| `EntropyAxis` cap (`H/12.0`) | 12.0 | Korpus üst-quartile H değeriyle değiştir |
| `WitnessDepthAxis` soft-normalize | `raw/(1+raw)` | Dağılım [0,1]'de makul spread veriyor mu |

## 4. Metodoloji

1. 15 repo full clone (spike-repos.md lesson: `--depth` witness'ı bozar)
2. `osp-spike analyze-v2` hepsine uygula → `V2Report` JSON
3. Dağılımları hesapla: merge_ratio, witness_status, coupling_density, entropy, witness_depth
4. Mevcut parametreleri korpus dağılımına göre validate/tune
5. Sonuç: `docs/calibration-results.md`

## 5. Clone Komutları

```powershell
# Mevcut (5): click, fastapi, django, date-fns, worms-supabase — P:\repos\osp-spike\ altında

# Yeni (10):
git clone https://github.com/psf/requests           P:\repos\osp-spike\requests
git clone https://github.com/pallets/flask          P:\repos\osp-spike\flask
git clone https://github.com/encode/httpx           P:\repos\osp-spike\httpx
git clone https://github.com/Textualize/rich        P:\repos\osp-spike\rich
git clone https://github.com/pydantic/pydantic      P:\repos\osp-spike\pydantic
git clone https://github.com/chalk/chalk            P:\repos\osp-spike\chalk
git clone https://github.com/sveltejs/svelte        P:\repos\osp-spike\svelte
git clone https://github.com/tj/commander.js        P:\repos\osp-spike\commander
git clone https://github.com/lodash/lodash          P:\repos\osp-spike\lodash
git clone https://github.com/vitest-dev/vitest      P:\repos\osp-spike\vitest
```

---

*Sürüm: 1.0 (Faz 1.11 corpus) · Sonraki: clone + analyze + `docs/calibration-results.md`*
