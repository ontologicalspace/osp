# OSP — Session Handoff Document

> **Tarih:** 2026-06-29 (ikinci oturum)
> **Session:** CI fix + Node-level witness #9 (composite risk §3.2)
> **Sonraki session:** arXiv submission (#7) veya 3D visualization (#8)
> **Branch:** `feat/desktop-vision-semantics-pass` → **PR #1 MERGED → main**

---

## Bu Session'da Yapılanlar (Özet)

İki ana iş: **(1) PR #1 CI fix** (önceki handoff "her şey tamam" demişti ama CI
fail ediyordu) + **(2) Node-level witness #9** (git history → composite risk_score,
paper §3.2 stub dolduruldu). PR #1 merged edildi. 414 → **436 test**.

### CI fix (kritik — önceki oturumun gözden kaçırdığı)

Önceki handoff "5 commit, her şey tamam" diyordu ama `gh pr status` "All checks
failing" gösterdi. İki ayrı kök neden bulundu:

**1. `engine.rs` unused mut (5 yer)** — `RUSTFLAGS="-D warnings"` altında hard
error. Lokal Windows'ta `cargo test` (warnings flag'siz) geçiyordu → yanılgı.
5 test'te `let mut engine` → `let engine` (engine `&self` metodları kullanıyor).
+ `cargo fmt --all` (329 konum repo-wide drift temizlendi).

**2. `osp-desktop` (Tauri) CI'da derlenmiyor** — ilk teşhisim (engine.rs) aslında
yanlıştı. İlk CI log'undaki satırlar `cargo fmt --check` diff'idir (CI `|| true`
ile geçiştiriliyor), rustc error değil. Gerçek fail her zaman Tauri idi:
`glib-sys` build script `pkg-config` ile `glib-2.0 >= 2.70` arıyor, Ubuntu runner'da
yok. Windows'ta Tauri WebView2 (Edge) kullanır → lokalde derlenir.

**Çözüm:** `ci.yml`'a `--exclude osp-desktop` (build/test/examples). Tauri binary
headless CI'da zaten çalışmaz; lokal/release-workflow konusu. CI 4 library
crate'i test eder: osp-core/analyzer/spike/llm-runtime.

```
Commit: 613fc6a (engine.rs fix + fmt), a986ede (ci.yml exclude)
PR #1 CI: fail → pass (1m4s)
```

### Node-level witness #9 (composite risk §3.2 dolduruldu)

"Risky AMA historically stable mı?" sorusu artık cevaplanır. Review'in
"battle-tested vs speculative" konusu. 3 katman:

**Katman 1 — NodeWitness tipi + git extraction**
- `osp-core/space.rs`: `NodeWitness` struct (commits_touching, distinct_authors,
  last_modified_days_ago, churn, ownership_concentration) + `neutral()` ctor.
  Node/Edge ile aynı yer (dependency yönü core←analyzer).
- `osp-analyzer/witness.rs` (yeni): `extract_witness()` — tek
  `git log --numstat --format=... --no-merges` pas'ı tüm repo history'yi tarar,
  per-file aggregate. svelte (34710 file) saniyeler mertebesinde. Graceful
  no-git fallback (SCIP-yok paralel). Binary skip, rename arrow handled.

**Katman 2 — composite risk_score (§3.2 stub dolduruldu)**
- `osp-core/vision.rs`: `compute_risk_score(theta, witness, vision_confidence)`
  + `RiskBreakdown` (vision/volatility/recency/solo/speculative) + `risk_weights`
  modülü (VISION 0.40 / VOL 0.20 / REC 0.15 / SOLO 0.15 / SPEC 0.10).
- 3 mod: witness=None → neutral 0.5; vision-conf=0 → vision katkısı 0
  ("Vision: not loaded" + "high risk" paradoks çözümü).
- `compute_derived` değişmedi (commit-path, witness erişimi yok) — risk ayrı
  analiz katmanında compute edilir.

**Katman 3 — wire-through + UI**
- `pipeline.rs`: `extract_witness()` → `AnalysisResult.node_witnesses`.
- desktop `NodeJson`: witness_* alanları (`#[serde(default)]`).
- frontend: Node Inspector "Witness (git history)" + "Composite risk (θ × witness)"
  breakdown bar'ları (scalar DEĞİL — gamification tuzağı önlemi).

**Diagnostic:** `examples/witness_breakdown.rs`.

### Bug bulundu (svelte corpus ile)

`days_between` çift bölme yapıyordu: `days_from_civil` GÜN döndürür, kod
`(l-e)/86400` yapmış → 12/86400 = 0. **Tüm dosyalar "0 gün önce" çıkıyordu.**
Fix: bölme yok (gün - gün = gün). Regression test eklendi. Ayrıca `%aI`
(author date) → `%cI` (commit date): author date rebase'te korunur, son
değişikliği maskeler.

**Doğrulama (svelte, 34710 files):**
- runtime.js: commits=316, authors=18, **days_ago=12** (önceden 0 — bug).
- witness extraction saniyeler mertebesinde.

```
Commit: 275063c (node-level witness #9, tüm katmanlar)
```

---

## Mevcut Durum (Test/Build)

```
436 test, 0 fail, 0 warning (CI-equivalent: -D warnings, exclude osp-desktop)
5 crate: osp-core + osp-analyzer + osp-spike + osp-desktop + osp-llm-runtime
PR #1 MERGED → main üzerinde
CI: Build & Test pass (osp-desktop excluded — Tauri headless CI'da derlenmez)
```

---

## Sıradaki Implementation Öncelikleri

### #1: arXiv submission (YÜKSEK ÖNCELİK — roadmap #7)

**Durum:** Paper v2.6 içerik olarak hazır. Node-level witness ile §3.2 risk_score
stub'ı doldu — paper'a eklenmeli.

**Çözüm:**
- paper-draft-v2.6.md → LaTeX derleme (arXiv şablonu)
- Bibliyografya (.bib) derleme
- Figure'ler (Figure 1-3) vektör format
- §3.2: NodeWitness + compute_risk_score + RiskBreakdown ekle
- §6.5: Node Inspector witness bölümü + risk breakdown UI
- §7.8: svelte witness doğrulama sonuçları (runtime.js days_ago fix)

**Efor:** M (format derleme + yeni sonuçların entegrasyonu)

---

### #2: 3D OSP Space visualization (YÜKSEK ÖNCELİK — roadmap #8)

**Neden şimdi:** Mode semantics (role/risk/metric) + type-only ayrımı + node-level
witness temiz. 3D artık anlamlı — 5 eksenli uzayın keşfi için.

**Çözüm:**
- Three.js / plotly 3D scatter
- 3 eksen: coupling/cohesion/instability; mass=size, churn/witness-depth=color
- risk_score → node boyut/halo (scalar değil, breakdown tooltip)

**Efor:** M-L

---

### #3: Task-success benchmark (ORTA ÖNCELİK — roadmap #6)

**Sorun:** RQ5 token savings'i gösteriyor ama task success (kod kalitesi) boşluğu var.

**Çözüm:** Real LLM ile OSP prompt vs raw dump → üretilen kodun kalitesi karşılaştırması.

**Efor:** L (evaluation framework gerekli)

---

### #4: Witness author normalization (DÜŞÜK ÖNCELİK — roadmap #10)

**Sorun:** Node witness author sayısı bot/alias (örn "dependabot", aynı kişinin
 farklı email'leri) yüzünden şişebilir.

**Çözüm:** `.mailmap` + bot detection (commit author pattern).

**Efor:** S-M

---

## Yeni Session'da İlk Soru

```
"arXiv (#7) ile başlayalım mı? Paper §3.2 risk_score artık gerçek değerle dolu."
```

Dosya: `docs/paper-draft-v2.6.md` → LaTeX derleme
Başlangıç noktası: §3.2 stub'ı (`risk_score = 0.0` yazıyordu) artık compute_risk_score
ile doldu. NodeWitness + RiskBreakdown paper'a eklenmeli.

---

## Bu Session'da Öğrenilen Teknik Dersler

### 1. CI log'unda fmt diff ≠ rustc error

**Bulgu:** PR #1 "All checks failing" idi. İlk CI log'unda `Verdict::Warning` gibi
satırlar gördüm → "engine.rs mut hatası" sandım. Ama bunlar `cargo fmt --check`
diff'idir (CI `|| true` ile geçiştiriyor). Gerçek fail her zaman Tauri `glib-sys`.

**Ders:** CI log'undaki `+`/`-` satırları önce fmt diff mi rustc error mu doğrula.
Gerçek fail adımını (`gh run view --job`) ve `failed to run custom build command`
satırını ara.

### 2. Windows lokal test vs Linux CI: `-D warnings` farkı

**Bulgu:** Lokalde `cargo test` (warnings flag'siz) 414 test geçti. CI `-D warnings`
ile fail. 5 gereksiz `let mut engine` warning'ı lokalde sessiz, CI'da hard error.

**Ders:** Lokal doğrulama CI ile aynı flag'lerle: `RUSTFLAGS="-D warnings" cargo test`.
Handoff'ta "X test geçti" demeden önce CI-equivalent koşullarda doğrula.

### 3. Tauri binary headless CI'da derlenmez

**Bulgu:** `osp-desktop` (Tauri v2) `cargo build --workspace` Linux CI'da
`webkit2gtk`/`glib-sys`/`gtk-sys` çekiyor → `pkg-config` sistem paketleri istiyor.
Windows'ta WebView2 (Edge) kullanır → lokalde derlenir.

**Ders:** Native GUI binary'lerini headless PR CI'ından exclude et. CI library
crate'leri test etsin. Binary lokal/release-workflow konusu.

### 4. days_from_civil birimi → çift bölme bug'ı

**Bulgu:** `days_from_civil` GÜN döndürür. `days_between` bunu `(l-e)/86400`'e
bölmüş → gün / saniye = 0. Svelte corpus'ta 34710 dosyanın HEPSİ "0 gün önce".
Diagnostic tool olmasaydı sessiz metric bozulması.

**Ders:** Tarih/zaman hesabında birimleri net belirt. Çift dönüştürme tuzağı:
fonksiyon adı `_days` ise birimi gün, başka bölme yapma. Büyük corpus'ta diagnostic
run etmeden metric doğru varsayma.

### 5. Author date vs commit date (rebase maskesi)

**Bulgu:** `git log %aI` (author date) rebase'te korunur. Svelte'te eski dosyalar
author date'i yıllar geride. `%cI` (commit date) rebase'te güncellenir → gerçek
"en son dokunma".

**Ders:** "last modified" için `%cI` kullan. Author date "orijinal yazım" içindir.

### 6. Composite risk: scalar değil breakdown

**Bulgu:** Risk tek sayı (0.62) gamification'a davet. Yerine RiskBreakdown bar
grafiği — "neden riskli" anlatır. Önceki oturumun #5 dersinin node-level uzantısı.

**Ders:** Karar destek metriklerinde scalar'lardan kaçın. risk_score Inspector'da
total + breakdown birlikte gösterilir.

---

## Önemli Dosyalar

| Dosya | İçerik |
|---|---|
| `docs/paper-draft-v2.6.md` | Paper v2.6 (arXiv target; §3.2 risk_score artık dolu) |
| `docs/roadmap.md` | Faz durum + öncelikler (bu session'la güncellendi, 0.7.0) |
| `crates/osp-core/src/space.rs` | NodeWitness struct (yeni) |
| `crates/osp-core/src/vision.rs` | compute_risk_score + RiskBreakdown + risk_weights (yeni) |
| `crates/osp-analyzer/src/witness.rs` | extract_witness — git log parse (yeni) |
| `crates/osp-analyzer/examples/witness_breakdown.rs` | Witness distribution diagnostic (yeni) |
| `crates/osp-analyzer/src/pipeline.rs` | node_witnesses wire-through |
| `crates/osp-analyzer/src/contract.rs` | AnalysisResult.node_witnesses + NodeWitness re-export |
| `crates/osp-desktop/src/lib.rs` | NodeJson witness_* alanları |
| `crates/osp-desktop/frontend/index.html` | Node Inspector witness + risk breakdown bölümleri |
| `.github/workflows/ci.yml` | osp-desktop exclude (Tauri headless) |

---

## Git Commit History (bu session)

```
275063c feat(core,analyzer,desktop): node-level witness (#9) — git history → composite risk
a986ede ci: exclude osp-desktop (Tauri) from headless CI build
613fc6a ci: fix unused mut in engine.rs tests + cargo fmt across workspace
```

Tümü `feat/desktop-vision-semantics-pass` branch'indeydi, **PR #1 olarak merged** edildi.
https://github.com/ervolkan/osp/pull/1

---

## Sonraki Session İçin Hızlı Başlangıç

```
1. docs/session-handoff.md oku (bu dosya)
2. docs/roadmap.md "Sonraki Session İçin Hızlı Başlangıç" bölümü
3. PR #1 MERGED → main üzerinde çalış:
     git checkout main && git pull
     (yeni feature branch'te çalış)
4. Kod durum: 436 test, 5 crate
   CI: osp-desktop exclude (Tauri headless CI'da derlenmez)
5. Corpus repo'lar: P:\repos\osp-spike\ (svelte dahil)
6. Lokal doğrulama her zaman CI-equivalent:
     RUSTFLAGS="-D warnings" cargo test --workspace --exclude osp-desktop
7. Diagnostic tool'lar:
     cargo run --example role_diagnostic -- <repo>      (classifier)
     cargo run --example vision_breakdown -- <repo>     (verdict)
     cargo run --example witness_breakdown -- <repo>    (git history)
8. İlk soru: "arXiv (#7) ile başlayalım mı? §3.2 risk_score artık gerçek değerle dolu."
```

---

*Sürüm: 0.7.0 · 2026-06-29 · node-level witness #9 + CI fix · 436 test · 5 crate · PR #1 MERGED*
