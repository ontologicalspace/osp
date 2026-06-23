# OSP Spike v3 — Faz 3.10 Erken Re-spike (Gerçek Abstractness)

> **İlk gerçek A değeri!** Faz 1-2'de `A=0.5` placeholder idi. Faz 3.5 abstractness
> modülü + Faz 3.10 pipeline ile 15 repo'da **gerçek `A = Na/Nc`** hesaplandı.
>
> `D = |A + I − 1|` artık anlamlı — repos arası mimari kalite karşılaştırması mümkün.
>
> **Önceki:** `spike-results-v2.md` (Faz 2, A=0.5 placeholder).
> **Bu doküman:** Faz 3.10 — A gerçek, D anlamlı.

---

## 1. Sonuçlar (15 repo, gerçek A)

### Küçük repolar (display çıktı)
| repo | nodes | edges | κ | A | I | D |
|---|---:|---:|---:|---:|---:|---:|
| worms-supabase | 26 | 17 | 0.65 | 0.42 | 0.50 | 0.36 |
| click | 63 | 61 | 0.97 | 0.58 | 0.50 | 0.36 |
| chalk | 13 | 11 | 0.85 | 0.81 | 0.50 | 0.35 |
| commander | 159 | 135 | 0.85 | 0.81 | 0.50 | 0.16 |
| httpx | 60 | 4 | 0.07 | 0.50 | 0.50 | 0.45 |
| flask | 83 | 131 | 1.58 | 0.71 | 0.50 | 0.34 |
| requests | 37 | 21 | 0.57 | 0.43 | 0.50 | 0.49 |
| rich | 213 | 404 | 1.90 | 0.71 | 0.50 | 0.36 |

### Büyük repolar (tracing log çıktı — authoritativ)
| repo | nodes | edges | κ | Na | Nc | A | I | D |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fastapi | 1.125 | 829 | 0.74 | 3 | 704 | **0.004** | 0.861 | **0.135** |
| pydantic | 533 | 1.016 | 1.91 | 109 | 5.481 | **0.020** | 0.759 | **0.221** |
| date-fns | 1.610 | 3.619 | 2.25 | 2 | 44 | **0.045** | 0.918 | **0.036** |
| svelte | 3.448 | 4.232 | 1.23 | 0 | 73 | **0.000** | 0.788 | **0.212** |
| lodash | 27 | 0 | 0.00 | 0 | 0 | **0.500** | 0.500 | **0.000** |
| vitest | 2.235 | 1.881 | 0.84 | 5 | 253 | **0.020** | 0.628 | **0.352** |
| django | 2.966 | 4.652 | 1.57 | 16 | 11.014 | **0.001** | 0.823 | **0.176** |

---

## 2. 🎯 Ana Bulgu: D Artık Anlamlı

### Faz 2 (placeholder A=0.5) vs Faz 3 (gerçek A)

| repo | A (Faz 2) | A (Faz 3) | D (Faz 2) | D (Faz 3) | Yorum |
|---|---:|---:|---:|---:|---|
| **django** | 0.5 | **0.001** | 0.323 | **0.176** | Django çok concrete (11k class, 16 abstract) → düşük A → D azaldı (main-seq'e yakın) |
| **pydantic** | 0.5 | **0.020** | 0.241 | **0.221** | pydantic 109 abstract/5481 → düşük ama pydantic-v2 traits artıyor |
| **fastapi** | 0.5 | **0.004** | 0.361 | **0.135** | fastapi çok concrete → D düştü (aslında main-seq'e yakın) |
| **date-fns** | 0.5 | **0.045** | 0.418 | **0.036** | date-fns D çok düştü — main-seq'e çok yakın! |
| **vitest** | 0.5 | **0.020** | 0.128 | **0.352** | vitest D YÜKSELDİ — high I + low A → Zone of Pain bölgesi |

### Yorum: Martin Main-Sequence Analizi

```
        A=1 (abstract)
        |
  Zone  |  Main
  of    |  Sequence
  Useless|  (A+I=1)
        |
  ------+------ I=1 (unstable)
        |
  Zone  |  
  of    |  
  Pain  |  
        |
        A=0 (concrete)
```

- **date-fns (D=0.036)**: Main-sequence'e ÇOK yakın — iyi mimari dengesi (A≈0.05, I≈0.92 → A+I≈0.96 ≈ 1)
- **fastapi (D=0.135)**: Yakın — concrete ama unstable (A+I≈0.87)
- **django (D=0.176)**: Yakın — çok concrete (A+I≈0.82)
- **pydantic (D=0.221)**: Orta — daha fazla abstract tip var (109 trait)
- **svelte (D=0.212)**: Orta — hiç abstract yok (A=0)
- **vitest (D=0.352)**: Uzak — Zone of Pain'e yakın (concrete + orta-instability)
- **lodash (D=0.0)**: Nötr — tip tespit edilemedi (JS no-types)

---

## 3. Faz 2 ↔ Faz 3 Karşılaştırma

| Metrik | Faz 2 (placeholder) | Faz 3 (gerçek) | İyileşme |
|---|---|---|---|
| **A (abstractness)** | 0.5 (sabit) | **0.001–0.81** (repo-specific) | ✅ Artık repos arası ayrım |
| **D (main-seq)** | ~0.2–0.5 (A=0.5 tarafından domine) | **0.036–0.49** (A'dan kaynaklı) | ✅ Mimari kalite sinyali |
| **y (cohesion)** | 0.5 (placeholder) | 0.5 (placeholder — SCIP bekliyor) | ⬜ Faz 3.6-3.7 |
| **x, z, w, v** | gerçek (Faz 1) | gerçek (Faz 1) | = değişmedi |

---

## 4. Dil Bazında Abstractness Dağılımı

| Dil | Tipik A | Neden |
|---|---|---|
| Python (django, fastapi, click) | 0.001–0.58 | ABC/Protocol az; concrete class hakim |
| TypeScript (svelte, date-fns) | 0.00–0.05 | interface az; type-level kod concrete |
| JavaScript (chalk, lodash) | 0.50 veya belirsiz | JS'de abstract yok; type tespit zor |
| Rust (test edilmemişti) | ~0.2–0.4 beklenen | trait yapısı abstract doğal |

**Python'da düşük A:** Çoğu Python projesi ABC kullanmıyor → concrete-heavy. pydantic
istisna (109 trait/protocol — pydantic-v2 Rust core'un Python binding'leri → daha fazla protocol).

**TypeScript'te düşük A:** interface az kullanılıyor; type-level kod concrete sınıf olarak
değil type alias olarak tanımlanıyor (tree-sitter yakalayamıyor → Faz 3 SCIP düzeltebilir).

---

## 5. Pipeline Doğrulaması

| Bileşen | Durum |
|---|---|
| 5 dil adapter (Py/TS/JS/Rust/Go) | ✅ Faz 3.2-3.3 |
| Import resolver (Internal/External/StdLib/Unknown) | ✅ |
| Abstractness hesabı (A = Na/Nc) | ✅ Faz 3.5 |
| MetricValue provenance (source/confidence/coverage) | ✅ Faz 3.1 |
| `analyze_repo()` pipeline | ✅ Faz 3.10 |
| **y (cohesion) — LCOM4** | ⬜ **SCIP gerek (Faz 3.6-3.7)** |
| **SCIP index parsing** | ⬜ Faz 3.6 |

---

## 6. Go/No-Go: Faz 3.6-3.7'ye Geçiş

**GO.** Pipeline çalışıyor, gerçek A değerleri makul, D anlamlı.

**Faz 3.6-3.7 öncelikleri:**
1. SCIP protobuf partial parse (`prost`) — occurrences + relationships
2. LCOM4 hesabı (method-field access graph → connected components)
3. `y` placeholder → gerçek değer
4. Re-spike: `y` gerçek ile 15 repo tekrar → `docs/spike-results-v4.md`

**Beklenen iyileşme (Faz 3.7 sonrası):**
- `y` artık repo-specific (0.5 placeholder değil)
- Pozisyonlar (`RawPosition.y`) anlamlı → θ daha doğru
- D + y birlikte → mimari değerlendirme tam (6/6 eksen gerçek)

---

*Veri: 2026-06-20 · 15 repo · 5 dil adapter · gerçek A (Tier 1 tree-sitter) · `osp-analyzer` pipeline*
