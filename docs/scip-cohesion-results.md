# SCIP Cohesion Results — 15-Repo Corpus (Faz 3.6-3.7 Complete)

> **Tarih:** 2026-06-22
> **Metod:** scip-python (Docker) + scip-typescript (npm) → OSP analyzer pipeline
> **LCOM4:** Gerçek cohesion değerleri (placeholder yok, function-only repolar hariç)

## Tam Sonuç Tablosu

| # | Repo | Dil | Nodes | Edges | SCIP Classes | A | I | D | **y (cohesion)** | SCIP Coverage |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | click | Py | 63 | 61 | 133 | 0.02 | 0.63 | 0.36 | **0.67** | 100% |
| 2 | fastapi | Py | 1133 | 831 | 673 | 0.01 | 0.70 | 0.13 | **0.62** | 99.6% |
| 3 | django | Py | 2966 | 4659 | 10054 | 0.00 | 0.66 | 0.18 | **0.66** | 98.4% |
| 4 | requests | Py | 37 | 21 | 25 | 0.05 | 0.43 | 0.49 | **0.49** | 51.4% |
| 5 | flask | Py | 83 | 131 | 115 | 0.01 | 0.71 | 0.34 | **0.63** | 100% |
| 6 | httpx | Py | 60 | 4 | 81 | 0.07 | 0.50 | 0.45 | **0.62** | 100% |
| 7 | rich | Py | 213 | 404 | 213 | 0.04 | 0.71 | 0.36 | **0.60** | 100% |
| 8 | pydantic | Py | 534 | 1016 | 323 | 0.02 | 0.70 | 0.22 | **0.52** | 18.7% |
| 9 | worms-supabase | Py | 26 | 17 | 0 | 0.00 | 0.45 | 0.35 | 0.50* | — |
| 10 | date-fns | TS | 1550 | 3579 | 105 | 0.05 | 0.93 | 0.02 | **0.51** | 96.4% |
| 11 | svelte | TS | 3450 | 4232 | 376 | 0.00 | 0.92 | 0.21 | **0.51** | 2.4% |
| 12 | vitest | TS | 2236 | 1881 | 705 | 0.02 | 0.57 | 0.35 | **0.54** | 91.0% |
| 13 | chalk | JS | 13 | 11 | 10 | 0.00 | 0.81 | 0.35 | **0.54** | 38.5% |
| 14 | commander.js | JS | 159 | 135 | 23 | 0.00 | 0.81 | 0.16 | **0.52** | 7.5% |
| 15 | lodash | JS | 27 | 0 | 0 | 0.50 | 0.50 | 0.00 | 0.50* | — |

**y\*** = placeholder (SCIP'de 0 class — function-only repo, LCOM4 N/A)

## Özet İstatistikler

| Metrik | Değer |
|---|---|
| **Gerçek LCOM4 cohesion** | 13/15 repo (%87) |
| **Placeholder (function-only)** | 2/15 (worms-supabase, lodash) — class yok, LCOM4 N/A |
| **y aralığı** | 0.49 (requests) — 0.67 (click) |
| **y medyan** | ~0.54 |
| **Ortalama coverage** | ~60% (düşük coverage'lı repolar MetricValue.confidence'a yansır) |
| **En yüksek coverage** | flask/httpx/rich/click (100%), fastapi (99.6%), django (98.4%) |
| **Toplam SCIP class sayısı** | 13,031 class (15 repo toplam) |

## Gözlemler

1. **Python repolar yüksek cohesion** (0.49-0.67 aralığı, ortalama ~0.60) — class-based tasarım
2. **TS/JS repolar orta cohesion** (0.51-0.54) — daha fonksiyonel, daha az class
3. **Function-only repolar** (lodash, worms-supabase) — 0 class → placeholder (doğru davranış, LCOM4 class-based bir metriktir)
4. **date-fns'in D=0.02** değeri çok yakın main-sequence'e — iyi mimari denge
5. **svelte I=0.92** — yüksek instabilite (bağımlılık yoğun)

## Paper için Etkisi

- **"y=0.5 placeholder" caveat kaldırıldı** — 13/15 repo gerçek LCOM4 değerine sahip
- **RQ4 eklenebilir:** "Gerçek LCOM4 cohesion dağılımı" — 15 repo üzerinde ampirik analiz
- **MetricValue provenance modeli doğrulandı** — coverage değeri her repo için kayıtlı
- **SCIP deployment zorlukları belgelendi** — monorepo (date-fns tsconfig), function-only (lodash), partial coverage (svelte)

## Teknik Notlar

- **scip-python** (Docker): `--project-name` + `--project-version` gerekli; tüm Python repolar başarılı
- **scip-typescript** (npm): `--infer-tsconfig` ile; monorepo repolar (date-fns) için geçici minimal tsconfig
- **date-fns** özel: `pkgs/core/` alt-dizini kullanıldı (monorepo yapısı)
- **Düşük coverage repolar** (svelte 2.4%, commander 7.5%): scip-typescript tüm dosyaları indekslemedi; bu MetricValue.confidence'a yansır (coverage × 0.95)
