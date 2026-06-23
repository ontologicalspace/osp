# OSP Spike — Repo Seçimi (Faz 0.1)

> **Metot:** Kör spektrum testi. Repolar önceden "iyi/spagetti/köpük" olarak
> **etiketlenmez** (önyargıyı önlemek için). 5 farklı noktada seçilir, metrikler
> kör çalıştırılır, clustering'in anlamlı çıkıp çıkmadığına bakılır.
>
> **Hipotez:** OSP metrikleri (şahitlik oranı, entropi, kuplaj, sapma-proxy)
> "üretim-olgunluğu" olan projeleri bir kümede, düşük-olgunluklu (az şahit,
> tek yazar, az yapı) projeleri ayrı kümede toplamalı.

---

## 1. 5-Repo Spektrumu

| # | Repo | Dil | Boyut | Created | Rol (gözlemlenebilir) |
|---|---|---|---|---|---|
| 1 | [`pallets/click`](https://github.com/pallets/click) | Python | orta | 2014 | küçük-odaklı, uzun-olgun, çok-yazarlı çapa |
| 2 | [`tiangolo/fastapi`](https://github.com/tiangolo/fastapi) | Python | büyük | 2018 | orta/büyük, modern framework, güçlü-review kültürü |
| 3 | [`django/django`](https://github.com/django/django) | Python | çok büyük | 2012 | büyük-olgun, sıkı review, scale-test çapası |
| 4 | [`date-fns/date-fns`](https://github.com/date-fns/date-fns) | TypeScript | büyük | 2015 | JS çapası, modüler yapı |
| 5 | [`pipeposse/worms-supabase`](https://github.com/pipeposse/worms-supabase) | Python | küçük (~2.3MB) | 2026-05 | yeni (~5 hafta), solo, 0-star, README-ağır beklenisi |

> **5. slota not:** Etiketleme kör-test ile çelişir — bu yüzden "foam" demek yerine
> **gözlemlenebilir sinyalleri** listeliyoruz (created 2026-05, 0 stars, 0 forks,
> single-owner). Metrikler bize "düşük-olgunluk"u gösterirse hipotez doğrulanır;
> göstermezse ya metrik zayıftır ya da repo olgun-laştırılmıştır. İkisi de bilgidir.
> **Makale aşamasında (Faz 7) bu repo anonimleştirilir veya sentetik fixture ile değiştirilir.**

---

## 2. Neden Bu 5? (spektrum kapsamı)

| Boyut | 1 (click) | 2 (fastapi) | 3 (django) | 4 (date-fns) | 5 (worms) |
|---|---|---|---|---|---|
| Yaş (yıl) | ~12 | ~8 | ~14 | ~11 | <0.1 |
| Beklenen commit sayısı | orta (~binler) | yüksek (~binler) | çok yüksek (~on binler) | yüksek | düşük (~onlar) |
| Beklenen yazar sayısı | orta | yüksek | çok yüksek | orta | 1 |
| Beklenen PR-oranı | yüksek | yüksek | çok yüksek | yüksek | düşük |
| Beklenen bağımlılık kompleksitesi | düşük-orta | orta | yüksek | orta | düşük |
| Dil | Python | Python | Python | TS | Python |

Bu, spike metriklerini **çoklu boyutta** sınar: yaş × ölçek × review-kültürü × dil.
Eğer OSP uzayı 1-4'ü bir kümede, 5'i uzakta konumlandırırsa → tez geçer.

---

## 3. Clone Talimatları

Aşağıdaki dizine clone önerilir (workspace dışı, sibling):

```powershell
# Konvensiyon: P:\repos\osp-spike\<repo-adi>
New-Item -ItemType Directory -Force -Path P:\repos\osp-spike | Out-Null

# ÖNEMLİ: --depth KULLANMA. Shallow clone, merge-commit geçmişini keser ve
# şahitlik analizini (witnessed_ratio) bozar. spike-results.md §7.1'de doğrulandı.
git clone https://github.com/pallets/click             P:\repos\osp-spike\click
git clone https://github.com/tiangolo/fastapi          P:\repos\osp-spike\fastapi
git clone https://github.com/django/django             P:\repos\osp-spike\django
git clone https://github.com/date-fns/date-fns         P:\repos\osp-spike\date-fns
git clone https://github.com/pipeposse/worms-supabase  P:\repos\osp-spike\worms-supabase
```

**Önemli notlar:**
- **Tam clone zorunlu** — `--depth N` merge-commit'leri keser, şahitlik metriklerini
  sıfırlar. Django ~34k commit (~250MB) clone'u birkaç dakika sürer ama gerekli.
- `worms-supabase` küçük (50 commit) — clone anlık.
- Eğer disk alanı kritikse, `git clone --filter=blob:none` (blobless clone) merge
  geçmişini korurken dosya içeriklerini lazy-fetch eder — iyi bir orta yol.

> **UYARI (kör-test bütünlüğü):** Clone öncesi repoların README/dosya yapısını
> incelemeyin. Hipotez önyargısını önlemek için metrikler çıkana kadar kör kalın.

---

## 4. Çalıştırma (Faz 0.3 tamamlandıktan sonra)

```powershell
cargo run -p osp-spike -- analyze P:\repos\osp-spike\click --compare `
  P:\repos\osp-spike\fastapi `
  P:\repos\osp-spike\django `
  P:\repos\osp-spike\date-fns `
  P:\repos\osp-spike\worms-supabase
```

Çıktı: `spike-output.json` + `docs/spike-results.md`'ye yorumlu analiz (Faz 0.7).

---

## 5. Go/No-Go Kriterleri (Faz 0.7'de uygulanacak)

| Kriter | Geçer | Kalır |
|---|---|---|
| 5 repo'yu metrikler ayırıyor mu? | worms-supabase diğerlerinden uzak; 1-4 bir kümede | hepsi aynı aralıkta |
| Şahitlik oranı (w_ratio) anlamlı dağılım veriyor mu? | 1-4 > 0.5, worms < 0.2 | hepsi ~0 veya ~1 |
| Entropi/kuplaj, "framework" (django/fastapi) ile "lib" (click) ayrımını gösteriyor mu? | django kuplajı > click | ayırt edemiyor |
| `likely_ff_workflow` flag'i doğru ateşliyor mu? | rebase-kullanan repolarda `*` | hiç ateşlemiyor veya hep ateşliyor |

Eğer ≥3 kriter geçerse → **Faz 1 Go**: aday eksenler (x,y,z,w,v,u) veri-güdümlü sabitlenir.
Eğer <3 geçerse → metrikleri gözden geçirip Faz 0.5'te yeniden tasarlanır.

---

## 6. Etik Not

`pipeposse/worms-supabase` gerçek bir kişinin repo'sudur. Bu spike internal doğrulama
içindir; **public makalede (Faz 7) isimlendirilmeyecek** — ya anonimleştirilecek
ya da sentetik kontrollü bir foam fixture ile değiştirilecek (reprodüüsibilite için
sentetik aslında daha iyi: bilinen ground-truth ile test).

Diğer 4 repo kamuya açık olgun OSS projelerdir; analizlerinde etik kaygı yok.

---

*Sürüm: 0.1 (Faz 0.1) · Sonraki adım: Faz 0.3 — tree-sitter bağımlılık grafı*
