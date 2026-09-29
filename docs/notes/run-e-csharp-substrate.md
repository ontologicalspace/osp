# Dogfood Run E — C# Substrate Canlı Uçtan Uca Koşusu (scip-dotnet Tier-2)

**Tarih:** 2026-09-29. Branch `feat/137-csharp-run-e` (main `497b9ff` üzerinden —
#137 Aşama 1 merge SONRASI). Aşama 1 parity baz yeşildi (fmt + clippy `-D warnings`
+ 46 suite / 0 fail).

**Amaç:** #137 Aşama 2 — C# Tier-1 (`CSharpAdapter`, PR #138) + Tier-2
(SCIP) zincirini **kullanıcının gerçek çalışma projesi** üzerinde uçtan uca
koşmak. Run A/B/C/D kalıbı (gerçek analyze, canlı araç zinciri, gözlemler
kayda geçer); bu kez motor değil **substrat** (yeni dil) sınanıyor.

## Hedef profil (gerçek çalışma projesi)

- **Repo:** `P:\Work\EntCore\Nexus` (`Nexus.sln`) — **125 projeli** kurumsal
  çözüm: ABP katmanlaması (`*.Domain` / `*.Application` / `*.HttpApi` / ...
  servis başına), Aspire AppHost, shared kernel, infrastructure, clients.
- **SDK:** dotnet 10.0.300 (`global.json` 10.0.201 + `rollForward: latestMinor`).
- **Boyut:** bin/obj dışı **2035 `.cs`**; polyglot komşuluk: **66 `.ts`**
  (clients) + **513 `.js`** (services — wwwroot/betik) → adapter keşfi toplam
  **2614 kaynak dosya** (uzay tasarım gereği çok dilli).
- obj/ altında **448 compiler-generated `.cs`** (AssemblyInfo,
  *.GlobalUsings) — keşif kirliliği kaynağı (aşağıda).

## Araç zinciri bulguları (Aşama 2 öncesi gerçeklik kontrolü)

1. **`scip-csharp` ölmüş:** NuGet paketi 404, `sourcegraph/scip-csharp`
   reposu 404. Proje **`sourcegraph/scip-dotnet`** olarak yeniden adlandırıldı
   (C# + VB; repo canlı). Dağıtım: NuGet paketi + Docker image (GitHub
   release'lerinde binary yok). Kurulum: `dotnet tool install --global
   scip-dotnet` → **v0.2.14** çalışır durumda. (Bağlam: SCIP Sourcegraph'tan
   vendor-neutral yönünde ayrılıyor — scip-code.org.)
2. **İndeksleme:** `scip-dotnet index Nexus.sln --output <temp>/nexus-index.scip`
   (çıktı bilinçli olarak kullanıcı reposu DIŞINA) → restore + build + indeks
   **93 saniye**, **30.3 MB** indeks.
   - Ölümcül olmayan tek hata: `clients/nexus-backoffice/nexus-backoffice.esproj`
     (TypeScript projesi) — "file extension not associated with a language";
     C# indeksleyici için doğru davranış.
   - Yan bulgu (kullanıcı bilgilendirmesi): restore sırasında **NU1903 —
     `Newtonsoft.Json` 9.0.1 bilinen yüksek şiddetli zafiyet uyarısı**
     (`Nexus.Docs.Web`), GHSA-5crp-9r3c-p9vr.

## Gözlenen koşular (planned vs observed)

### 1. Tier-1 as-is (keşif düzeltmesi ÖNCESİ)

`osp-analyze P:/Work/EntCore/Nexus` → **3062 node / 3482 edge / A=0.082 /
I=0.727 / D=0.191**, ~22 sn. Node sayısındaki kirlilik tespit edildi: keşif
`bin`/`obj` dizinlerini dışlamıyordu → obj/ altındaki **448 üretilmiş `.cs`**
düğüm oluyordu. (Pipeline dışlama listesi node_modules/target/build/dist/…
taşıyordu; C# üretim artifact'ları eksikti.)

### 2. Keşif düzeltmesi (bu PR)

`walk_dir` dışlama listesine `bin` + `obj` eklendi (yorum: Run E gözlemi,
Nexus'ta 448 dosyalık fark). Temiz keşif bağımsız üç sayıcıyla çapraz
doğrulandı (find, find -L, node fs walk — 2035 `.cs` hemfikir; 2614 = çok
dilli toplam).

### 3. Tier-1 temiz baz

**2614 node / 3438 edge / κ=1.32 / A=0.083 / I=0.742 / D=0.175**, ~11 sn.
Kenje deltaları anlamlı: as-is→temiz kenje 3482→3438 (obj dosyalarının iç
using'leri çözümleniyordu). Abstractness: 217 abstract (interface'ler +
abstract class/record) / 2384 concrete.

### 4. Tier-2 — SCIP gerçek LCOM4 cohesion

`osp-analyze --scip nexus-index.scip P:/Work/EntCore/Nexus`:

- **SCIP yükleyici C# sembollerini doğru ayrıştırdı:** `scip_classes=2320`,
  `scip_files=2416` — `Ad.Alanı.Sınıf` sembol yapısı yükleyicinin
  dil-agnostik ayrıştırmasına oturdu; **gerçek LCOM4 aktif**.
- **y = 0.77** (placeholder 0.50'a karşılık — ortalama cohesion yüksek;
  LCOM4-fragmente sınıflar az).
- **SCIP coverage = 0.9242** (2416/2614). Kalan %7.6 = `.ts`/`.js`
  düğümleri — scip-dotnet yalnız C#/VB indeksler; bu düğümler placeholder
  cohesion'da kalır (bilinen sınır, aşağıda).
- Toplam koşu ~37 sn (SCIP protobuf yükleme 9 sn dahil).

## Bulgular ve boşluklar

1. **[FIXED — bu PR]** Keşif `bin`/`obj` dışlamıyordu → C# üretim
   artifact'ları node kirliliği (Nexus: +448 node, +44 edge).
2. **[DÖKÜMANTE]** `scip-csharp` → `scip-dotnet` yeniden adlandırması; kurulum
   kanalı NuGet/Docker. Issue #137 güncellendi.
3. **[BOŞLUK — bilinçli erteleme]** Polyglot repolarda Tier-2 coverage
   tavanı: scip-dotnet C#/VB dışını indekslemez → `.ts`/`.js` düğümleri
   placeholder cohesion'da (Nexus'ta %7.6). Kapatma yolu: scip-typescript
   eşlikçisi — ayrı iş, #137 kapsamı dışı.
4. **[YAN BULGU — kullanıcıya]** NU1903: Newtonsoft.Json 9.0.1 yüksek
   şiddetli zafiyet uyarısı (Nexus.Docs.Web; GHSA-5crp-9r3c-p9vr).
   OSP kapsamı dışı, bilgilendirme amaçlı kayıt.

## Sonuç

C# substrate uçtan uca **çalışıyor**: Tier-1 (tree-sitter-c-sharp 0.23.1
exact-pin + namespace-backed using çözümlemesi) gerçek 125-projeli kurumsal
repo'da 11 saniyede uzay kuruyor; Tier-2 (scip-dotnet) 93 saniyelik indeksle
**gerçek LCOM4 cohesion (y=0.77) %92.4 coverage ile** aktive ediyor. Aşama 1
(PR #138) + Aşama 2 (bu koşu + keşif düzeltmesi) ile **#137 tamamlanır**.
Run E gözlemleri: keşif düzeltmesi repoya alındı; Tier-1/Tier-2 karşılaştırma
verisi (placeholder 0.50 ↔ gerçek 0.77) kalıcı kayıtta.
