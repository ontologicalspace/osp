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
stratejiyle tutarlı biçimde insan barlarından daha agresif (üç domainin
ikisinde: 0.55 vs 0.875/0.89; casino'da eşit: 0.90 = 0.90); tek-kenar öngörüleri
(0.62–0.72) mekanik gerçekten (~0.91 tek-kenar) ~0.25 sapmış — sıralama
bağlayısız (gerçek etkiler tekdüze; tie'lerde inversion tanımsız).

## Sonuç 3 — D5a declared realization (tez doğrulaması)

Gerçekleştirme birimi = önerinin beyanı (ΔG + reasoning). Build kapsamı:
etkilenen service + test projesi. **"Realization" bu tabloda "build-valid
declared realization" anlamındadır** — varoluşsal gerçekleştirilemezlik iddiası
DEĞİL (bkz. Bulgu 1 ve INV-T10'daki normatif karar).

| kol | graph | build | test | gözlemlenen c | E_c (imzalı) |
|---|---|---|---|---|---|
| A-18 | ✓ 0.875 | ✓ | 271/271 | 0.875 | 0.875−0.8889 = −0.0139 (favorable) |
| A-19 | ✓ 0.8889 | ✓ | 298/298 | 0.8889 | 0.8889−0.90 = −0.0111 (favorable) |
| A-20 | ✗ 0.9167 | ✓ | 175/1s | 0.9167 | **0 (üçlü tam)** |
| B-18 | ✓ 0.0 | ✗ 60 CS | — | 0.0 | 0 |
| B-19 | ✓ 0.0 | ✗ 122 CS | — | 0.0 | 0 |
| B-20 | ✓ 0.0 | ✗ 76 CS | — | 0.0 | 0 |

**B: graph 3/3 + build-valid declared realization 0/3; A: graph 2/3 + build-valid
declared realization 3/3.** D5b koşulmadı: B'nin beyanının ötesinde kurtarma,
bağımlılıkları yeniden gerçekleştirmek (adapter/facade/yerel implementasyon)
anlamına gelir — bu "imkânsız" etiketi DEĞİL; "beyan dışı semantic work yeni
proposal mıdır?" normatif kararı INV-T10'ya aittir (o karar verilirse "beyan
olarak unrealizable" OSP semantiği içinde tanımlanır).

## Dört bulgu

1. **Graph predicate, declared-realization aşamasında build-valid olmayan
   öneriyi build-valid önerinin üstünde sıralayabiliyor** (ölçülmüş gerçek).
   Daha güçlü bir "realize-edilemezlik" iddiası bilinçli KULLANILMIYOR:
   `∄C': A(C') = G' ∧ Verify(C')` ancak (a) D5b tükenmesiyle ya da (b) INV-T10'da
   donacak normatif kararla ("beyan dışı semantic work yeni proposal sayılır")
   tanımlanabilir hâle gelir — deney sonucu değil, protokol tasarım kararıdır.
   B'nin "başarısı" yalnızca simülatör yüzeyinde; A'nın graph'taki iki
   borderline-geçişi gerçek kodda da duruyor, customer'daki dürüst fail'i de.
   Bu, Paper 2'nin "structural-proposal success ≠ patch correctness" caveat'ının
   deneysel karşılığı — artık epistemik güvenlik gereksinimi.
2. **Metric prediction fidelity ≠ source realizability.** Target-coupling
   izdüşümü oldukça sadık: customer'da exact (öngörü = sim = gözlem 0.9167),
   ai/casino'da küçük favorable imzalı residual'lar (E_c = −0.0139 / −0.0111:
   same-ns kardeş tasarımı + ölü-using temizliği — beyan-dışı, kayıtlı);
   node-düzeyinde yeni düğümlerin gerçek importları declared
   `connected_to`'dan +2-3 fazla (ayrı ölçüm ekseni). B'de ise E_c = 0 iken
   build çöküyor: **ölçüm tahmini doğru olduğu halde program gerçekliği
   başarısız.** Gap ölçümde değil, **admissible action manifold'ta**: `∃C':
   A(C') = G'` koşulsuz her ΔG graph uzayında geçerli; ödül `f(G')` iken gerçek
   amaç `f(C', G', B')`.
3. **Ölçüm katmanları ayrıştı:** analyzer da silmeleri görüyor (B gözlemlenen
   c=0.0 ×3, E_c = 0) — parse/analyze/build üçlüsünden yalnızca build ayrışıyor.
   INV-T10'un boundary'si tam burası: `GraphCompleted ≠ RealizedCompleted`.
4. **Dürüst dünya-modeli vs simulator-itaat:** customer'da doğru model taşıyan
   yazar (12→11 mümkün, bar 12→≤8 değil) graph katmanında kaybetti; kötü model
   taşıyan (hepsini sil) graph katmanında kazandı — build katmanında ise tam
   tersi. Değerlendirme sistemi intelligence değil simulator obedience seçiyor.

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
  (Entities/Enums/Domain.Providers — metod imzaları); node-düzeyi sapma
  graph-genişliği olgusudur (D_G eksenine ait, E_c'ye değil); task ekseni
  etkilenmez.
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
`AcceptAsCompleted → Mainline` önüne RealizationGate gelir. Öngörü-gözlem
ölçümleri iki ayrı eksende tutulur (birinci sınıf veri; prediction ≠
observation witness'ı): **`E_c = c_observed − c_predicted`** (imzalı,
target-axis residual — bu notun tablolarındaki değer) ve ileride RealizationMatch
tolerans çalışması için **`D_G = d(G_predicted, G_observed)`** (graph-genişliği
distance; node-düzeyi import sapmaları bu eksene aittir). Açık kararlar
(design issue'da donacak): **"beyan dışı semantic work yeni proposal sayılır"
normatif kararı** (ancak bu karar verilince "beyan olarak unrealizable" OSP
semantiği içinde tanımlanır — bu notun dilini daraltan gerekçe);
`RealizationMatch` tolerans semantiği (analyzer-gürültüsü: tip-düzeyi kenarlar,
FQN, sim'in geri-kenar körlüğü); gate'in konumu (trajectory/cli-ritüel/witness);
realization'ı kim yazar (AddNode belirsizliği); verification kapsamı
yapılandırması.

**Paper-4 bağlantısı:** bu deneyin ana ve savunulabilir cümlesi —

> **Graph predicate success ⇏ build-valid declared realization** — graph-only
> değerlendirme, beyanının gerçeklenmesi build-kırılan öneriyi build-geçenin
> üstünde sıralayabiliyor (B: E_c = 0 iken build çöktü; A: graph 2/3 ama
> build+test 3/3).

— Paper 4'ün canlı-kullanım bölümünün merkez adaylarından biri (ölçüm hattı
kuruldu; ilk noktalar yukarıda).
