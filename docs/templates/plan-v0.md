# Plan — <task başlığı> (plan-v0 şablonu)

> **Kullanım:** Yüksek riskli task sınıfı (public API / authz / finansal /
> persistence-schema) için, KOD YAZILMADAN ÖNCE dondurulür. Doldurulmamış
> zorunlu bölüm = pre-registration RED (kanıt-tanımsız kriter dahil — aşağıda).
> Normatif sözleşme: `docs/spec/plans-stratum.md` (#206/#213 karar kayıtları).
> Plan-dışı runlarda K-N kanıt-hesabı bölümü yalnız pre-registration NOTES'a düşer.

## Üst bilgi

| Alan | Değer |
|---|---|
| task_id / task_ref | `<id>` / `dogfood/runs/<run-dir>/task.json` |
| run_ref | `dogfood/runs/<run-dir>/` |
| scope-trigger | hangi yüksek-risk sınıfı: `public API \| authz \| finansal \| persistence-schema` — gerekçe: `<kısa>` |
| sürüm | `v0` (bu dosya `plan-v0.md`) |
| frozen_at | `<tarih-saat>` — freeze kaydı (digest dahil) pre-registration NOTES'a yazılır |

**Digest kuralı (döngüsellik yasağı):** Bu dosya KENDİ digest'ini **taşımaz** —
`plan-v0` originating digest DAHİL hiçbir digest alanı içermez (v0'nın
originating digest'i kendi digest'i olurdu). `plan-vN` (N ≥ 1) yalnız
`previous_digest` + `originating_digest` taşıyabilir. Tüm digest kimlikleri ÜÇ
dış kayıtta yaşar: (1) NOTES freeze kaydı, (2) `plans.jsonl` satırı,
(3) CompletionClaim. Bkz. `docs/spec/plans-stratum.md`.

## Amendment zinciri (yalnız N ≥ 1 sürümlerde doldurulur)

> Her amendment TAM YENİ SÜRÜM üretir (`plan-v1.md`, `plan-v2.md`, … — önceki
> sürüm dosyası ASLA değiştirilmez; her yeni sürüm önceki sürümün TAMAMINI
> içerir + bu tabloya bir satır ekler). Eski sürüme geri dönük düzenleme YOK.

| Sürüm | previous_digest | originating_digest | Ne değişti | Neden | freeze kaydı |
|---|---|---|---|---|---|
| v1 | `<sha256:…>` | `<sha256:…>` | `<kısa>` | `<gerekçe>` | `<NOTES kayıt ref>` |

## Hedef

<Ne olmasını istiyoruz — bir-iki cümle, davranışsal dil. Plan koordinat
ALMAZ (D8): pozisyon/koordinat beyanı RED.>

## Başarı ölçütleri (K-1 .. K-n)

- **K-1:** <ölçülebilir/gözlemlenen başarı iddiası>
- **K-2:** …

<!-- Her K-n için aşağıdaki kanıt-hesabı bölümü DOLDURULMAZSA plan RED:
     "karşı-örneksiz kriter = kanıt-tanımsız = pre-registration RED" (#213). -->

## K-N kanıt-hesabı (#213 — her kriter için ZORUNLU)

### K-1 kanıt-hesabı
- **KANIT (kanıt sözleşmesi — önceden tanımlanmış BEKLENTİ, gözlem değil):**
  - *identity:* <başarı hangi İŞ-KURALI/ürün kimliğiyle gözlemlenir? yanıt/durum/kayıt kimliği ne?>
  - *cardinality:* <kanıt KAÇ sonuç/satır/olay ister? (boş/tek sonuç kümesi vacuous geçmemeli)>
  - *required_fields:* <eşleşen kaydın HANGİ alanları doğrulanmalı? (aynı kimlik + yanlış alan = başarısız)>
  - *measurement_scope:* <doğrulama NEYİ kapsıyor? kapsam dışı kalan ne? (deadline/bütçe/sayfa-tamlığı)>
- **KARŞI-ÖRNEKLER** (her biri "bu kriter hangi ZAYIF kanıtla YANLIŞ geçer?"):
  1. `<yanlış-kimlik | eksik-kardinalite | zayıf-snapshot | kapsam-dışı | yanlış-alan>`: `<girdi>`
     → BEKLENEN: **RED** `<normatif ret kimliği>` — probe: `<probe test adı yuvası>`
  2. …

### K-2 kanıt-hesabı
<aynı yapı>

**Durum matrisi (hatırlatma):** karşı-örnek YOK → **RED (kanıt-tanımsız)**;
karşı-örnek var + probe koşulMAdı → **PENDING**; probe beklenen yanlış-başarıyı
**beklenen ret kimliğiyle** reddetti → doğrulanmış negatif kanıt. Probe test
adının burada olması testin koştuğunu KANITLAMAZ — gözlem CompletionClaim'de.

**Soru-kalıpları (run-27 5-sınıf taksonomisi — tablo doldururken sor):**
kimlik-pinleme ("RED'in iş-kuralı kimliği ne? bunu taşımayan hangi yanıt RED olmalı?") ·
kardinalite ("kanıt kaç sonuç ister? daha az/çok sonuç ne zaman başarı görünür?") ·
kapsam-tamlığı ("eşleşen satırın hangi alanları doğrulanıyor?") ·
ölçüm-scope ("deadline/bütçe NEYİ kapsıyor? dışarıda kalan hangi çağrı aşabilir?") ·
snapshot-tamlığı ("sayım NEYİN sayımı — sunucu-otoriter mi görünüm mü? eksik/örtüşen snapshot hangi başarıyı sahte üretir?").

## Dokunulacak yüzeyler

> Mevcut draft-task spec'ini ABSORBE eder (paralel beyan YOK): new_nodes /
> new_edges / modified_entities. Operasyon beyanı (op-matrix) burada BEYAN
> EDİLMİŞ NİYET'tir — #172 K5'in ölçülmüş temsilci/izin-doğrulamasıyla
> EŞDEĞER DEĞİLDİR (Aşama-2 sınırı). `modified_entities` için #202 sınırı
> geçerli: graph-ifade edilemeyen delta (config/yorum/betik) burada SÖZCÜK
> evi bulur, graf kanıtı üretemez — bu bilinçli sınırdır, aşılmaz.

- <dosya/yüzey 1>: <beklenen değişiklik türü>
- …

## Başarısızlık-modu enumerasyonu (run-26 başlangıç seti)

> Kod YAZILMADAN önce: bu sınıfların her biri bu taskta nasıl açığa çıkabilir?
> İşaretle + kısa gerekçe; yeni sınıf eklenebilir (amendment ile).

- [ ] **yetki/tenant sınırı** — <nasıl?>
- [ ] **idempotens bağlamı** — <hangi kimlik eksenleri? çakışma ne üretmeli?>
- [ ] **kaynak tükenmesi** — <limit/quota/retry sınırları?>
- [ ] **başarısızlıkta kanıt dayanıklılığı** — <hata anında hangi kanıt kalır?>
- [ ] **kanonik tek-kaynak** — <aynı değeri iki yerde türeten yol var mı?>

## Doğrulanacak invaryantlar

- <INV/kural>: <nasıl doğrulanır (probe/ölçüm/test)>

## Bilinçli kapsam dışı

- <neyi YAPMIYORUZ — açıkça>

## Yapısal niyet (opsiyonel — D_G girdisi)

> Beklenen yeni kenarlar (namespace→dosya / tip bağımlılığı); INV-T10 D_G
> makinelerinin tüketebileceği yapısal öngörü. Kesin sayı verme zorunluluğu
> yok; "yeni kenar BEKLENMİYOR" da geçerli bir beyandır.

- <beklenen kenar / beklenmiyor beyanı>

## Sabit notlar

- **D8:** plan koordinat/uzay-pozisyon içermez; içerirse RED.
- **n=1 reviewer:** plan-kapısı tek reviewer'ın (insan) yaklaşım denetimi;
  bağımsız-reviewer iddiası taşımaz — yorumlar bu sınırla okunur.
- **Dondurma:** kod ÖNCESİ donar; sonradan gereken değişiklik = amendment
  (yeni sürüm, yukarıdaki zincir) — donmuş sürümü geriye dönük düzeltmek YOK.
