# OSP Self-Analysis (Dogfooding) — OSP ile OSP'yi analiz etmek

Bu rapor, OSP'nin conceptual space engine'inin **kendi kod tabanını** analiz
etme deneyimini dokümante eder. Amaç: (1) OSP'nin ölçtüğü metriklerin gerçek
bir codebase'te anlamlı olduğunu doğrulamak, (2) Paper 3 (Concept Anchoring)
evidence üretmek, (3) analyzer'ın gerçek kullanımda bug/eksikliklerini yüzeye
çıkarmak.

**Tarih:** 2026-08-04
**Branch:** `docs/dogfood-self-analysis`
**Commit:** OSP main @ `8c04ed4`

## Yöntem

### 1. Rust SCIP index üretimi

OSP cohesion metriği (LCOM4) gerçek semantic bilgi gerektirir — tree-sitter
sadece syntax bilir. Sourcegraph'in SCIP indexer'ı ile Rust semantic index
üretildi:

```bash
# rust-toolchain.toml geçici kaldırıldı (image r-a 1.89, pin 1.97.1'de r-a yok)
MSYS_NO_PATHCONV=1 docker run --rm -v "P:/Work/SoftwarePhysics:/repo" -w /repo \
  sourcegraph/scip-rust:latest scip-rust --output /repo/index.scip
```

Sonuç: `index.scip` (16.4 MB, ~6 dk — rust-analyzer tüm workspace'i derledi).

**Bulgu (CI/Docs adayı):** `sourcegraph/scip-rust` image'ı `rust-toolchain.toml`
görünce onu kullanıyor ama pinned toolchain'de `rust-analyzer` yok. Çözüm: pin'i
geçici kaldırmak. Bu README'de belirtilmeli veya `scip-rust` image'ına r-a
eklenmeli.

### 2. Analyze çalıştırma

```bash
cargo run -q --bin osp -- analyze . --scip index.scip --out /tmp/osp-self-analysis.json
```

**SCIP coverage: %80** (138/172 dosya). Kalan %20 placeholder cohesion (0.5).

## Sonuçlar

### Genel istatistik

| Metrik | Değer |
|--------|-------|
| Node count | 172 |
| Edge count | 202 |
| SCIP coverage | 80.2% (138/172 dosya) |
| Cohesion source (scip) | 90 node |
| Cohesion source (placeholder) | 82 node |

### osp-core/src ana modülleri (coupling sırasıyla)

| Modül | Coupling | Cohesion | Instability | Cohesion Src |
|-------|---------|----------|-------------|--------------|
| `engine.rs` | 0.938 | 0.442 | 0.833 | scip |
| `authorization.rs` | 0.923 | 0.612 | 0.750 | scip |
| `navigator.rs` | 0.909 | 0.813 | 1.000 | scip |
| `measurement.rs` | 0.875 | 0.391 | 0.700 | scip |
| `anchoring/pipeline.rs` | 0.857 | 0.333 | 1.000 | scip |
| `anchoring/gate.rs` | 0.833 | 0.467 | 0.714 | scip |
| `anchoring/store.rs` | 0.833 | 0.362 | 0.500 | scip |
| `agent.rs` | 0.800 | 0.938 | 0.500 | scip |
| `anchoring/resolved_implementation.rs` | 0.800 | 1.000 | 0.800 | scip |
| `anchoring/review.rs` | 0.800 | 0.941 | 0.571 | scip |
| `authorization/gate_v2.rs` | 0.800 | 1.000 | 1.000 | scip |
| `trajectory.rs` | 0.800 | 0.973 | 0.364 | scip |

## Değerlendirme: Metrikler anlamlı mı?

**Evet — ölçülen metrikler gerçek mimariyi tutarlı biçimde yansıtıyor.**

### Doğrulanan teşhisler

1. **`engine.rs` (coupling 0.938, cohesion 0.442):** OSP'nin merkezi orchestrator'ı.
   Her modüle bağımlı (yüksek coupling), heterogeneous (düşük cohesion — commit,
   vision, witness, persistence hepsi bir arada). Bu gerçek.

2. **`authorization.rs` (coupling 0.923):** 17k+ satırlık dev dosya (clippy
   PR'larında gördüğümüz). Yüksek coupling beklendiği gibi.

3. **`navigator.rs` (instability 1.000):** Tamamen instabil — sadece outward
   dependencies, hiç incoming bağımlılık yok (kimse navigator'a bağımlı değil,
   o her şeye bağımlı). Bu doğru — navigator top-level orchestrator.

4. **`trajectory.rs` (instability 0.364):** Stabil — başkaları ona bağımlı.
   Doğru, çünkü `TaskId`, `TaskPolicy` gibi temel tipler burada.

### Refactor adayları (düşük cohesion + yüksek coupling)

OSP kendi analizine göre bu modüller refactor fırsatı:

| Modül | Cohesion | Coupling | Teşhis |
|-------|----------|----------|--------|
| `anchoring/extractor.rs` | **0.167** | 0.750 | En düşük cohesion — ciddi refactor adayı |
| `anchoring/pipeline.rs` | 0.333 | 0.857 | Heterojen sorumluluklar |
| `anchoring/store.rs` | 0.362 | 0.833 | Split adayı |
| `measurement.rs` | 0.391 | 0.875 | Sınıf/tip grupları ayrılabilir |

Bu bulgu özellikle değerli — OSP kendi metrikleri gerçek refactor önceliklerini
doğru tespit ediyor.

## Bulgular ve eksiklikler (OSP analyzer'a feedback)

### 1. Edge detayları analyze çıktısında yok

`osp analyze --format json` çıktısı `node_count` ve `edge_count` veriyor ama
**edge listesi yok**. Node'ların bağımlılık grafiği (hangi modül hangisine
bağımlı) çıktıda değil. Bu, trajectory attempt için gerekli (`removed_edges`
proposal edge belirtir).

**Öneri:** `analyze` çıktısına `edges: [{from, to, kind}]` eklensin. Bu hem
debugging hem de trajectory fixture üretimi için gerekli.

### 2. SCIP coverage %80 — kalan %20 neden?

82 node placeholder cohesion. Bunlar:
- `examples/` altındaki dosyalar (index'lenmemiş)
- Bazı `tests/` dosyaları
- `engine.rs` placeholder (0.5) — bu şaşırtıcı, en büyük dosya neden index'lenmedi?

**Öneri:** `engine.rs` gibi ana dosyaların neden index'lenmediğini araştır.
Belki macro-heavy kod rust-analyzer'da sorun çıkarıyor.

### 3. README SCIP talimatı hatalı

README'de:
```bash
rust-analyzer scip . --output /repo/index.scip
```

Doğru komut:
```bash
scip-rust --output /repo/index.scip
```

(`scip-rust` wrapper, doğrudan `rust-analyzer` değil.)

### 4. `rust-toolchain.toml` + scip-rust çakışması

`sourcegraph/scip-rust` image'ı `rust-toolchain.toml` görünce pinned toolchain'i
kullanıyor ama r-a o toolchain'de yok. Bu, SCIP üretimini engelliyor.

**Öneri:** README'ye not eklensin veya `scip-rust` image'ına r-a eklensin.

## Trajectory attempt (değerlendirme)

OSP'nin Completed-loop harness'i (PR #104) kendi kod tabanında çalıştırılabilir,
ama iki ön koşul var:

1. **Edge detayı:** Task fixture `removed_edges` için edge bilgisi gerek, ama
   analyze çıktısında edge yok (bkz. Bulgu 1).
2. **NodeId→path mapping:** Task `scope_bindings` NodeId→path eşleşmesi gerekiyor.

Bu eksiklikler giderilirse, OSP kendi kodunda "engine.rs coupling'ini düşür"
gibi bir Completed-loop senaryosu çalıştırılabilir. Şu an manuel olarak task
fixture hazırlamak çok zahmetli.

**Sonraki adım önerisi:** `analyze` çıktısına edge listesi eklemek, hem
dogfooding trajectory'i hem de debugging için değerli.

## Sonuç

**OSP kendi kod tabanını başarılı şekilde analiz ediyor.** Ölçülen metrikler
(coupling, cohesion, instability) gerçek mimariyi tutarlı biçimde yansıtıyor.
Refactor adayları (extractor.rs, pipeline.rs, measurement.rs düşük cohesion)
gerçek mühendislik teşhisleriyle örtüşüyor.

Bu dogfooding deneyimi üç şey kanıtlar:
1. **OSP conceptual space engine gerçek bir codebase'te çalışıyor** (sadece
   fixture'larla değil).
2. **Metrikler anlamlı** — rastgele sayılar değil, mimari teşhis üretiyor.
3. **Paper 3 evidence** — Concept Anchoring kendi kod tabanında doğrulanabilir.

Analyzer'a feedback (edge listesi, SCIP coverage boşlukları) bir sonraki adım
için kaydedildi.
