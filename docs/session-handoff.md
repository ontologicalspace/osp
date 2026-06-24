# OSP — Session Handoff Document

> **Tarih:** 2026-06-24
> **Session:** #1 (uzun — paper v2.3→v2.7 + kod refactor + desktop + SCIP + LLM)
> **Sonraki session:** Implementation odaklı

---

## Bu Session'da Yapılanlar (Özet)

### Paper (v2.3 → v2.7)
- v2.3: SCIP deployment, RQ4 cohesion, BFT A1-A4
- v2.4: Token compression §9.4, hallucination §9.5
- v2.5: RQ5 token benchmark, Related Work GraphRAG/RepoCoder/SWE-agent
- v2.6: 23 repo, 5 dil, Rust/Go cohesion gradient
- **v2.7 (MAJOR REFRAME):** Theorem→Property, BFT analogy'ye indir, §2 designed-behavior disclaimer, architecture conformance related work, RQ5 "compression"→"compact representation", §7.8 Preliminary Usage Observations (dogfooding + real GPT-4o-mini)

### Kod
- osp-core: 330+ test, 15 invariant, Q4-Q6 gates, compute_raw_from_delta, compute_space_slice, PermissionMask, HallucinationType, mock LLM e2e
- osp-analyzer: 5 dil tree-sitter + SCIP (4 toolchain), 28-repo corpus
- osp-desktop: 5 panel (Topology + Vision + Witness + Pipeline + Graveyard) + Tauri native
- Token benchmark: 13 repo + real GPT-4o-mini

### Altyapı
- Git: github.com/ervolkan/osp, 30+ commit
- CI/CD: GitHub Actions
- Apache-2.0, README, reproduce script
- Internal docs private (8 dosya .gitignore)

---

## Mevcut Durum (Test/Build)

```
330+ test, 0 fail, 0 warning
4 crate: osp-core (142) + osp-analyzer (97) + osp-spike (32) + osp-desktop (bin)
Tauri native window çalışıyor
5 desktop panel test edildi
```

---

## Sıradaki Implementation Öncelikleri

### #1: Rust/Go Tree-sitter Edge Extraction (YÜKSEK ÖNCELİK)

**Sorun:** Rust/Go reposlarında `edges=0` — tree-sitter adapter `use`/`import` statement'lerini parse etmiyor. Bu coupling (x) ve instability (z) değerlerini etkiliyor.

**Çözüm:**
- `crates/osp-analyzer/src/adapters/rust.rs` — `use` statement extraction
- `crates/osp-analyzer/src/adapters/go.rs` — `import` statement extraction
- Mevcut Python/TS/JS adapter'larda nasıl yapıldığını örnek al

**Etki:** Rust/Go reposları tam koordinat sistemi alır (coupling + instability + cohesion). Paper Appendix C'deki `†` işaretleri kalkar. "5-language evaluation" tam anlamıyla geçerli olur.

**Efor:** 4-6 saat (2 adapter × test)

---

### #2: osp-llm-runtime Crate (YÜKSEK ÖNCELİK)

**Sorun:** LLM entegrasyonu PowerShell script'inde (`scripts/llm-token-bench.ps1`). Rust crate olarak olmalı.

**Çözüm:**
- `crates/osp-llm-runtime/` yeni crate
- `reqwest` + `serde_json` ile OpenAI API çağrısı
- Input: `OspPrompt` (serialize edilmiş JSON)
- Output: `DeltaProposal` (deserialize edilmiş JSON)
- Token ölçümü: API response'tan `usage.prompt_tokens` / `usage.completion_tokens`

**Etki:** "Real LLM integration" artık kodda var. Paper §7.8 verisi daha fazla run ile istatistiksel anlamlılık kazanır.

**Efor:** 4-6 saat

---

### #3: Daha Fazla LLM Benchmark Run (ORTA ÖNCELİK)

**Sorun:** Şu an 2 run var (4-node + 10-node). İstatistik için 10+ run gerekli.

**Çözüm:**
- osp-llm-runtime ile 10 farklı repo'da çalıştır
- Her repo için: OSP prompt vs raw 2-hop dump
- Median, IQR, min-max hesapla
- GPT-4o (mini değil) ile de dene

**Efor:** 2-3 saat (API cost ~$0.50)

---

### #4: Rust/Go scip-rust field_access Fix (ORTA ÖNCELİK)

**Sorun:** scip-rust `field_access` verisi üretmiyor → tüm Rust class'ları LCOM4=1 → y=0.50.

**Çözüm:**
- scip-rust'ın field-access detection mekanizmasını araştır
- Belki rust-analyzer version upgrade
- Alternatif: SCIP occurrence'ları manuel parse et (field reference detection)

**Efor:** Bilinmiyor (araştırma gerekiyor)

---

### #5: OSP Desktop — Node Inspector + Snapshot (DÜŞÜK ÖNCELİK)

**Sorun:** Space Topology'de node'a tıklayınca detay panel yok. Snapshot kaydetme yok.

**Çözüm:**
- Frontend: node click → inspector panel (sağ sidebar)
- Backend: `cmd_get_node_detail(node_id)` 
- Snapshot: `cmd_save_snapshot()` / `cmd_load_snapshot()`

**Efor:** 3-4 saat

---

## Yeni Session'da İlk Soru

```
"Rust/Go edge extraction (#1) ile başlayalım mı?"
```

Dosya: `crates/osp-analyzer/src/adapters/rust.rs` ve `go.rs`
Örnek: `crates/osp-analyzer/src/adapters/python.rs` (çalışan `import` extraction)

---

## Önemli Dosyalar

| Dosya | İçerik |
|---|---|
| `docs/paper-draft-v2.5-edited.md` | Paper v2.7 (submit target) |
| `docs/roadmap.md` | Faz durum + öncelikler |
| `docs/usage-dogfooding.md` | Dogfooding verisi |
| `docs/usage-llm-benchmark.md` | Real GPT-4o-mini verisi |
| `docs/corpus28-results.md` | 28-repo extended corpus |
| `docs/llm-apikey.md` | OpenAI API key (gitignore'd!) |
| `crates/osp-core/src/` | Core types + engine + gates |
| `crates/osp-analyzer/src/adapters/` | 5 dil tree-sitter adapter |
| `crates/osp-desktop/frontend/index.html` | 5 panel UI |
| `scripts/llm-token-bench.ps1` | LLM benchmark script |

---

## Git Commit History (son 10)

```
21c3288 paper: §7.8 Preliminary Usage Observations
5fdcf33 docs: real LLM benchmark — GPT-4o-mini tiktoken
0a33803 docs: dogfooding report
1c4e0ad docs: real-usage evidence plan
670d747 paper: MAJOR REFRAME — honest positioning (v2.7)
76253e2 paper: §6.5 OSP Desktop + Figure 3
6a2f79b paper: §5.3 rewrite — practical safety + attack scenario
7e57a00 paper: Theorem 1 strengthening + reviewer fixes
ef3e234 paper: v2.6 Yol C — 23 repos, 5 languages
7a85371 paper: v2.6 claim discipline pass
```
