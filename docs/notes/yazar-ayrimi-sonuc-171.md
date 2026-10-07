# Yazar-ayrımı deneyi — sonuç ve analiz notu (#171, 2026-10-07)

**Durum:** deney İCRA EDİLDİ (dual-run + D5a declared realization). Bu not,
pre-registration dizisinin (`yazar-ayrimi-deney-tasarimi-171.md` v2 + issue
amendment'ları) kapanış analizidir. Ham veri `/dogfood/` altında local-only
(#151 Live Contract); kalıcı kayıt issue yorumları + bu not.

## Kayıt zinciri (deney bütünlüğü)

1. Pre-registration v2 (2026-10-06, veri üretilmeden önce) + P2'ler
   (imzalı estimand'lar, label-blinded terminoloji, seed türetimi).
2. Amendment'lar: bar-split (6019105190), retryable küme (6019658939),
   D5a/D5b declared-realization (**6040870349**, veri öncesi onaylı).
3. Açılış kaydı (kol koşulmadan önce): `dogfood/runs/2026-10-07-author-exp/OPENING.md`
   — seçim (#7/#3/#5, kullanıcı onayı), D1 ayıklama (aktif NX-05 işi: payment/
   identityadmin/settings/wallet/shared-events hariç), τ̂ el tahminleri.
4. Veri: dual-run [6040750371]; D5a sonuç [6041466076];
   satırlar `dogfood/llm-author.jsonl` (6) + `realization-evidence.jsonl` (6).

## Zemin

Nexus `2e10c53d` (PR #600) detached worktree; OSP `5e38687`; gpt-4o-mini
(default; D3 override kullanılmadı); açılış dumanı SMOKE_OK (gerçek HTTP
yolunun ilk canlı doğrulaması — 2437ms, usage 23/1/24).

| task | hedef (domain) | out | τ̂ (A-barı) | B-barı (elicited) |
|---|---|---|---|---|
| 18 | ConversationStreamingService.cs (ai) | 11 | 0.875 | 0.55 |
| 19 | Bridge.Host/Program.cs (casino-bridge) | 13 | 0.90 | 0.90 |
| 20 | CustomerLedger.cs (customer) | 12 | 0.89 | 0.55 |

## Sonuç 1 — 2×2 (sim; PRIMARY, betimsel; n=3)

| | insan-bar | LLM-bar | |
|---|---|---|---|
| **insan-öneri** | 2/3 | 1/3 | |
| **LLM-öneri** | 3/3 | 3/3 | |

PE_H=+0.333 · PE_L=+0.667 · INT=+0.333 · BE_H=−0.333 · BE_L=0 · BE=+0.333;
**INT==BE symmetry-check PASS** (cebirsel özdeşlik uygulama sabitinde de tuttu).

## Sonuç 2 — B'nin stratejisi: dejenere

B (gpt-4o-mini) üç domainde de **"tüm çıkış importlarını sil"** önerdi
(11/13/12 kenar, sıfır telafi yapı): *"remove all outgoing Imports from this
node"*. Sim'de triviyel geçer (c→0). B'nin barları (0.55/0.90/0.55) bu
stratejiyle tutarlı biçimde insan barlarından agresif; tek-kenar öngörüleri
(0.62–0.72) mekanik gerçekten (~0.91 tek-kenar) ~0.25 sapmış — sıralama
bağlayısız (gerçek etkiler tekdüze; tie'lerde inversion tanımsız).

## Sonuç 3 — D5a declared realization (tez doğrulaması)

Gerçekleştirme birimi = önerinin beyanı (ΔG + reasoning). Build kapsamı:
etkilenen service + test projesi.

| kol | graph | build | test | gözlemlenen c | E_r |
|---|---|---|---|---|---|
| A-18 | ✓ 0.875 | ✓ | 271/271 | 0.875 | 0.8889→0.875 (−1, iyi yönde) |
| A-19 | ✓ 0.8889 | ✓ | 298/298 | 0.8889 | 0.90→0.8889 (−1, ölü using) |
| A-20 | ✗ 0.9167 | ✓ | 175/1s | 0.9167 | **0 (üçlü tam)** |
| B-18 | ✓ 0.0 | ✗ 60 CS | — | 0.0 | 0 |
| B-19 | ✓ 0.0 | ✗ 122 CS | — | 0.0 | 0 |
| B-20 | ✓ 0.0 | ✗ 76 CS | — | 0.0 | 0 |

**B: graph 3/3 + realization 0/3; A: graph 2/3 + realization 3/3.**

## Dört bulgu

1. **Graph predicate, realize-edilemez yazarı realize-edilebilirin üstünde
   sıralıyor.** Ölçülmüş gerçek: B'nin "başarısı" yalnızca simülatör yüzeyinde;
   A'nın graph'taki iki borderline-geçişi gerçek kodda da duruyor, customer'daki
   dürüst fail'i de. Bu, Paper 2'nin "structural-proposal success ≠ patch
   correctness" caveat'ının deneysel karşılığı — artık epistemik güvenlik
   gereksinimi.
2. **Simülatör sadakati suçlu değil.** Gerçekleşebilir önerilerde sim-öngörü =
   gözlem (customer üçlü 0.9167; ai/casino tek-notch FAVORABLE sapma: same-ns
   kardeş tasarımı + ölü-using temizliği — beyan-dışı, kayıtlı). Gap ölçümde
   değil, **admissible action manifold'ta**: `∃C': A(C') = G'` koşulsuz her
   ΔG graph uzayında geçerli; ödül `f(G')` iken gerçek amaç `f(C', G', B')`.
3. **Ölçüm katmanları ayrıştı:** analyzer da silmeleri görüyor (B gözlemlenen
   c=0.0 ×3, E_r=0) — parse/analyze/build üçlüsünden yalnızca build ayrışıyor.
   INV-T10'un boundary'si tam burası: `GraphCompleted ≠ RealizedCompleted`.
4. **Dürüst dünya-modeli vs simulator-itaat:** customer'da doğru model taşıyan
   yazar (12→11 mümkün, bar 12→≤8 değil) kaybetti; kötü model taşıyan (hepsini
   sil) kazandı. Değerlendirme sistemi intelligence değil simulator obedience
   seçiyor.

## Yan bulgular / dersler

- **out sayısı ≠ removability:** customer (out=12, removable≈1) vs casino
  (out=13, removable≥6). Aynı coupling, farklı mimari durum — `R(M) =
  structurally removable dependency mass` gelecekte ayrı ölçüm adayı.
- τ̂ el tahmini customer'da yanlış, ai/casino'da sınırda doğru — freeze-time
  tahmin kalitesi de veri; yanlış tahmin deney arızası değil.
- AddNode deltası **metinsel olarak belirsiz** (yeni düğümün gövdesi deltasda
  yok): A'nın realization'ı `declared_realization_work` (yazarlık) gerektirir;
  B'ninki mekanik silme. "Literal realization" tanımı bu asimetri yüzünden
  "declared realization" olarak kesinleştirildi (amendment 6040870349).
- Yeni düğümlerin gerçek importları declared `connected_to`'dan +2-3 fazlaydı
  (Entities/Enums/Domain.Providers — metod imzaları); node-düzeyi E_r ayrı
  kayıt; task ekseni etkilenmez.
- Mock tek-öneri + motor-Red → `llm_error` terminal: dürüst fail'in mekanik
  şekli (D6'nın mock tarafı).
- Operasyonel: sed ile C# blok cerrahisi yasak (adres-siz pattern bir dosyayı
  19 yerden bozdu; git checkout kurtardı) — Edit aracı sıfır dikiş hatası.

## Sınırlar (v1 dürüstlük listesi aynen)

n=3 domain, n=1 model (gpt-4o-mini, temperature 0.3), n=1 analist, tek oturum;
betimsel — istatistiksel author-effect iddiası bu stratumdan çıkmaz. B tek
örnekleminde tek strateji gösterdi (all-delete); strateji çeşitliliği için
replicate'lar (aynı donmuş prompt, ayrı çağrılar) sonraki stratum.

## Yön: INV-T10 — Structural Realizability

> Bir task, yalnızca predicate'i hypothetical graph üzerinde geçtiği için
> tamamlanmış sayılamaz; önerinin source-space realization'ı uygulanmalı,
> doğrulanmalı (parse/build/test) ve yeniden analiz edilen graph predicate'i
> sağlamalıdır.

Zincir: `Proposal → Hypothetical Graph → GraphCompleted → Source Realization
→ Build/Test → Reanalysis → RealizedCompleted → Mainline`. Q5.b değişmez;
`AcceptAsCompleted → Mainline` önüne RealizationGate gelir. `E_r =
d(Ĝ_after, G_observed)` birinci sınıf veri (prediction≠observation witness'ı).
Açık kararlar (design issue'da donacak): `RealizationMatch` tolerans semantiği
(analyzer-gürültüsü: tip-düzeyi kenarlar, FQN, sim'in geri-kenar körlüğü);
gate'in konumu (trajectory/cli-ritüel/witness); realization'ı kim yazar
(AddNode belirsizliği); verification kapsamı yapılandırması.

**Paper-4 bağlantısı:** bu deneyin ana cümlesi — *"graph-only evaluator,
realizability kısıtı olmadan dejenere stratejiyi ödüllendirir; realizability
gate'i eklenince dürüst model taşıyan yazar öne geçer"* — Paper 4'ün
canlı-kullanım bölümünün merkez adaylarından biri (ölçüm hattı kuruldu; ilk
noktalar yukarıda).
