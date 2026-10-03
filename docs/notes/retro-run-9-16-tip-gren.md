# Retro run 9-16 — tip-gren yeniden ölçüm ve nesil-ayrımı kapanışı (Dalga 1a)

**Tarih:** 2026-10-04 · **Binary:** `d9e214a` derlemesi (bu oturumda `cargo build` ile
yeniden doğrulandı — kaynak ağaç = d9e214a, tracked-değişiklik yok) · **Enstrüman:**
`scripts/neighborhood_accounting.py` v1.3 (d9e214a'dan `git show` ile teyit; working-tree
kopyası normalize-diff ile özdeş, fark yalnız CRLF) · **Yöntem:** 9 commit'in her biri
için Nexus'ta detached worktree (`Temp/nexus-<sha>`, MUTLAK yol; clean snapshot,
`repository.clean=true`) → `osp analyze --format json --out dogfood/retro/run9-16/an-<sha>.json`
(~75-90 sn/commit) → metrik çıkarımı (`Temp/retro_metrics.py` → `metrics.json`) +
mahalle ×16 (`Temp/retro_neighborhoods.sh` → `nb-r<N>-{namespace,type}.json`).

## 0. Envanter — zincir yapısı (yapısal bulgu)

Run 9-16 kesintisiz bir promote zinciri: her run'ın after'u bir sonrakinin baseline'ı.
8 run için yalnız **9 farklı commit** — her commit bir kez analiz edilip iki run'da
kullanıldı (analyze, commit ağacının saf fonksiyonu; determinizm bkz. §5).

| run | baseline | after | run-id |
|---|---|---|---|
| 9 | 96619f9906 | f01f0b863e | casino-mapper-risk-audit-ayrimi |
| 10 | f01f0b863e | bcb6783f0d | conversation-streaming-boundary |
| 11 | bcb6783f0d | 843b9032a8 | payment-fraud-gate-callback-boundary |
| 12 | 843b9032a8 | 16820a4ffb | bonus-grant-ledger-boundary |
| 13 | 16820a4ffb | 3e3fa51c9d | provider-config-ledger-boundary |
| 14 | 3e3fa51c9d | be55d09553 | bonus-wallet-ledger-boundary |
| 15 | be55d09553 | 43e431c0cf | store-mutation-ledger-boundary |
| 16 | 43e431c0cf | d344669022 | approval-ledger-boundary |

Canlı run'ların OSP revizyonları: 9 = b6666c3, 10 = 0b88e7e, 11-16 = c5b4cd6.
Ledger `repository_head` alanları after (promote) SHA'larıyla özdeş doğrulandı.

## 1. Nesil ayrımı KAPANDI (ns-gren ekseninde birebirlik)

Karşılaştırma: canlı run dizinlerindeki `baseline.json`/`after.json` (eski binary'ler,
canlı checkout) ↔ retro `an-*.json` (yeni binary, temiz worktree), path-anahtarlı:

- **T düğümü x-değerleri 16/16 birebir** (8 run × baseline/after; ör. run 16:
  0.888889→0.800000 iki tarafta da). El-tablo ĉ değerlerinin tamamı makine çıktısında aynen.
- **ns-gren mahalle sınıfları 8/8 ÖZDEŞ** (moved/multiplied/unchanged/removed/new/reduced
  ve out-sum; canlı donmuş 97e8bbb çıktılarıyla hücre hücre). Run-16 birebirliği — tur-4
  binary'sine aitti — v3 binary ile YENİDEN DOĞRULANDI.
- **Ortak dosya evreninde imports kenar kümeleri 9/9 commit'te birebir.**

Ham fark (−66 düğüm, −34 kenar; tüm karşılaştırmalarda sabit) **enstrüman-nesli değil,
kapsam artefaktı**: `clients/nexus-backoffice` bir submodule; worktree'de initialize
edilmediği için .ts dosyaları diskte yok, canlı checkout'ta populated. Kanıt: yeni binary
canlı checkout'ta koşturulduğunda (scope-test) 67 .ts düğümü GÖRÜYOR ve `type_imports=6574`
— #167 tur-4→tur-5 geliştirme ölçümündeki referans değerin ta kendisi. Retro tablonun iç
tutarlılığı korunur (9 analizin kapsamı aynı); run hedefleri (C# servisler) kapsamın her
iki durumda ortak parçasında.

## 2. (x, x_type) tablosu — ilk iki-gren ölçümü

x = out/(out+1) (imports kenarları); x_type = out_type/(out_type+1) (type_imports
kenarları). Formüller 2547 düğümün tamamında alan-değerleriyle doğrulandı.
**x_type same-ns tip referanslarını SAYMAZ** (onlar `same_ns_type` kenar sınıfı;
görünürlük için bkz. §3).

| run | T out→out | x_b→x_a | out_type_b | x_type_b | out_type_a | x_type_a | born (out/x → out_t/x_type) |
|---|---|---|---|---|---|---|---|
| 9 | 12→8 | 0.9231→0.8889 | 63 | 0.9843 | 40 | 0.9756 | CasinoRiskMapper 4/0.80 → 22/0.9565 |
| 10 | 12→5 | 0.9231→0.8333 | 35 | 0.9722 | 11 | 0.9167 | ConversationStreamingService 11/0.9167 → 27/0.9643 |
| 11 | 8→5 | 0.8889→0.8333 | 17 | 0.9444 | 10 | 0.9091 | PaymentFraudGate 5/0.8333 → 6/0.8571; PaymentCallbackProcessor 6/0.8571 → 8/0.8889 |
| 12 | 8→6 | 0.8889→0.8571 | 20 | 0.9524 | 12 | 0.9231 | BonusGrantLedger 6/0.8571 → 11/0.9167 |
| 13 | 7→4 | 0.8750→0.8000 | 29 | 0.9655 | 22 | 0.9565 | ProviderConfigLedger 6/0.8571 → 13/0.9286 |
| 14 | 7→5 | 0.8750→0.8333 | 15 | 0.9375 | 8 | 0.8889 | BonusWalletLedger 6/0.8571 → 12/0.9231 |
| 15 | 7→5 | 0.8750→0.8333 | 15 | 0.9375 | 11 | 0.9167 | StoreMutationLedger 4/0.80 → 9/0.9000 |
| 16 | 8→4 | 0.8889→0.8000 | 16 | 0.9412 | 6 | 0.8571 | ApprovalLedger 7/0.8750 → 15/0.9375 |

Gözlemler: (i) x_type sistematik olarak x'ten yüksek (tip çokluğu kenar çokluğundan
büyük); (ii) her run x VE x_type ekseninde düşüş üretti (iki-gren karar yönü aynı);
(iii) gren farkı düğüm sınıfına göre değişiyor — mapper'larda uç boyutlu (run 9 born:
4 ns kenarı ↔ 22 tip kenarı), defter çıkarmalarında ılımlı (run 15: 4↔9).

**Repo-genel:** type_imports 6524→6572 (+48, 8 promote boyunca ~monoton); same_ns_type
626→639; **same-ns maskelenme oranı %8.8-8.9 — zincir boyunca değişmez** (yapısal
değişimlere rağmen; defter çıkarmaları same-ns kompozisyonu ürettiği için maske payı
hafif yükselişe eğilimli: 8.8→8.9).

## 3. Same-ns görünürlüğü — ns-gren'in göremediği bağımlılıklar

El tablolarındaki "aynı ns — KENAR YOK" istisnaları artık veride: run 16 T baseline
`ApprovalNotApprovedException` (el: "KENAR YOK"); run 14 `WalletMapper`, run 15
`StoreDtoMapper` (el: "using GEREKMEZ"); run 16 sonrası T→born same-ns bağları
(`ApprovalLedger`, `StoreMutationLedger`, `BonusWalletLedger`) doğrudan görünür.
En zengin durum run 13: T'nin **6 same-ns tip bağımlılığı** var (`Nexus.Casino.Providers`
ns'i Contracts-alışkanlığının dışında — 5 sorgu Input tipi + interface aynı ns'te).
Domain'in ns yerleşim adetine göre same-ns cebi derinleşiyor; bu, ns-gren coupling'in
domain'ler arası karşılaştırılabilirliğini sessizce bozan bir etken — Paper-4
granularity eksenine girdi.

## 4. Altın standart run 14-16 — el tablolarıyla hücre hücre eşleşme

**Run 16 (ApprovalAppService→ApprovalLedger):** L1-L8 tablosunun tamamı makine tip
envanterinde birebir — T baseline 16 tip kenarı (Dtos×3, IApprovalAppService,
ToolExecutionResult, IToolExecutorService, AiApprovalRequest+AiMessage, 3 repo,
4 enum, AiSettings) + 1 same-ns (exception); A sonrası düşenler tam olarak
Tools/Entities/Enums/Settings (L4/L5/L7/L8 "GİDER" ✓); L6'da yalnız
`IAiApprovalRepository` kalır ✓; born 7 ns ✓ / 15 tip + exception ✓. **Tek sapma:**
makine L7'de 4. enum tipi **`ActionType`** bulur (kaynak satır 77 `actionType:
input.ActionType`, 187 ToDto) — el tablosu 3 tip saymıştı. Kenar-ölçüm (ns-gren)
doğruydu; el TİP listesi eksikti. Sınıf: **el-tamamlanmışluk eksiği — makine tamamlayan.**

**Run 14 (BonusWalletAppService→BonusWalletLedger):** T baseline 15 tip + WalletMapper
(same-ns) el envanteriyle özdeş (Dtos×5, Entities WalletAuditLog+WalletTransaction,
3 repo, WalletTransactionService, WalletMetrics, FundType+TransactionType); born
öngörüsü 6 ns / el tip envanteri makinede aynen (Enums'ta yalnız TransactionType
doğar; FundType T'de kalır — el-ince-notların ikisi de makinede). **Sapma (yama+el
kaçırdı, makine yakalar):** T sonrası `using Nexus.Wallet.Domain.Entities;` DURUYOR
ama o ns'ten hiç tip adı yazılmıyor (okuma yüzü `var wallet` ile çalışır — `Wallet`
adı hiç yazılmaz) → **ölü using**. ns-gren kenar olarak sayıyordu (out=5); tip-gren
sıfır tip gösterir. Karara etki yok (temiz out=4 → x=0.8 ≤ bar 0.845, iki durumda da
GREEN) ama ölçüm-hijyeni bulgusu: **ns-gren ölü using'leri coupling'e sayar; tip-gren
bu sınıfı yakalar.** Sınıf: **var-okuma ad-görünmezliği + ölü using.**

**Run 15 (StoreAppService→StoreMutationLedger):** T baseline Entities kenarının
yalnız `StoreAddress` (ctor alanı `IRepository<StoreAddress, Guid>`) tarafından
sabitlendiği el tespiti makinede aynen; born 4 ns (Events×3 Eto, Dtos×4, StoreAddress,
IStoreRepository) + same-ns StoreDtoMapper ✓. **Sapma (enstrüman sınırı):**
`Domain.Entities.Store.Create(...)` (nitelikli üretim, ledger satır 43) **tip kenarı
üretmiyor** — nitelikli isim çözümlemesi type_imports'a girmiyor; niteliksiz
`StoreAddress.Create` giriyor. Sınıf: **FQN üretim görünürlüğü — bilinçli kapsam
dışı mı, eksik mi** ayrımı #167 issue'sunda netleştirilmeli (aşağıda öneri).

**İki-gren tamamlayıcılığı (genel ders):** hiçbir gren diğerini kapsamıyor.
ns-gren görür-tip-gren görmez: using satırı ama ad yazılmıyor (ölü using / var-okuma).
Tip-gren görür-ns-gren görmez: same-ns tipler (§3). Birlikte kesin daha bilgilendirici;
yalnız biri taşınırsa ölçüm kör kalır.

## 5. Determinizm ve kapsam notları

Aynı binary (d9e214a), aynı commit (d3446690), iki checkout: temiz worktree ↔ canlı
(kirli). Farklar tamamen açıklandı: (i) submodule working-tree'i (67. .ts düğümü,
submodule işaretçisi değişmiş); (ii) canlı checkout'un takipli-dosya yerel değişiklikleri
(`IdentityAdminDbMigrationService.cs`, `SettingsHttpApiHostModule.cs` — 2+2 kenar ve 2
mass farkının kaynağı; `git status` teyitli). Ortak-temiz evrende sıfır fark → **retro
ölçümü için worktree yöntemi doğrulandı**; canlı checkout drift'i retro'ya sızmaz.

## 6. Pencere-içi şekiller tip-gren'de AYRILIYOR (türetilmiş gösterim)

Tur-4'ün ayırt edemediği üç vaka. Sayılar makine tip envanterleri + el şekil
tanımlarından TÜRETİLMİŞTİR (karşı-olgusal yamalar yok; ölçüm değil gösterim):

| run | çift (ikisi de τ penceresi içinde) | ns-gren | tip-gren kalıntıları | ayıran tip |
|---|---|---|---|---|
| 14 | S2a (dar defter) vs S2b (tam defter) | 5/6 ≡ 5/6 | 9/10=0.9 vs 8/9=0.8889 | `IWalletTransactionRepository` (idempotency) |
| 15 | S3 (mutasyon defteri) vs S4 (servis-yüzü) | 5/6 vs 4/5 (ikisi ≤0.845) | 11/12=0.9231 vs 10/11=0.9091 | `IStoreRepository` (okuma yüzü) |
| 16 | S2′ (guard-serviste) vs S3 (tam defter) | 5/6 vs 4/5 (ikisi ≤0.84) | 7/8=0.875 vs 6/7=0.8571 | `ApprovalStatus` (guard enum'u) |

Her çifti ayıran tip, mimari ayrımı TAŞIYAN tipin kendisi (idempotency repo'su /
okuma-face repo'su / guard enum'u). **Karar-robustluk yanıtı:** verilen kararlar iki
grende de aynı yön değişimine sahip (§2-ii); tip-gren pencere-içi ayrışmayı mümkün
kılıyor ama run 14-16 kararlarını geriye dönük DEĞİŞTİRMİYOR (barlar ns-gren'de
donduruldu, tercihler ayrı gerekçeliydi).

## 7. Tüketiciler

- **Paper-4 "coupling granularity sensitivity":** §2 tablo + §3 maske payı + §4
  tamamlayıcılık + §6 ayrışma gösterimi — ekseni dört kolonla besler.
- **#169 (el tahmin kanalı) ampirik spesifikasyonu:** el HER-TİP denetiminin makine
  karşılığı T/born tip envanteri çıktısı; τ türetimi artık tip-gren pencereyle de
  yapılabilir (pencere-içi çiftler için ayrıştırıcı kanal). Retro tabloları spec girdisi.
- **Enstrüman hattası takibi:** FQN üretim (§4 run 15) — #167 issue'suna "bilinçli
  ret mi eksik mi" notu düşülmeli; aynı soru ölü-using tespiti için bir `--report`
  fikri verir (ns-kenarı var, tip kenarı yok sınıfı).

## 8. Artefakt envanteri

- `dogfood/retro/run9-16/`: `an-<sha10>.json` ×9 (+stderr), `scope-test-live-d3446690.json`,
  `metrics.json`, `nb-r{9..16}-{namespace,type}.{json,md}` ×16.
- Worktree'ler: `Temp/nexus-<sha>` ×9 (non-versioned; not gözden geçtikten sonra
  `git -C P:/Work/EntCore/Nexus worktree remove` ile temizlenebilir).
- Script'ler (yeniden üretilebilirlik): `Temp/retro_metrics.py`, `Temp/retro_neighborhoods.sh`.
- Bu not doğrulanana kadar commit edilmedi (plan dosyası da öyle).
