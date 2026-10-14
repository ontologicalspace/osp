# CompletionClaim — PR gövdesi şablonu (plan-bound task'lar)

> **Konum:** Planın özet yüzü — PR gövdesine gömülür (mevcut pratikle marjinal
> maliyet ~0). Plan NİYETİ dondurur; CompletionClaim GERÇEKLEŞMEYİ kanıtlar.
> İkisi birbirinin yerine GEÇMEZ: PlanEvidenceContract ≠ ObservedEvidence.
> Normatif sözleşme: `docs/spec/plans-stratum.md`.

## Plan bağı

| Alan | Değer |
|---|---|
| plan (aktif) | `dogfood/runs/<run-dir>/plan-v<N>.md` — digest `<sha256:…>` |
| originating plan | `plan-v0.md` — digest `<sha256:…>` |
| amendment zinciri | `<yok \| v1: previous=…, ne/neden; v2: …>` — her halkanın freeze kaydı plans.jsonl'de |
| scope-trigger | `<public API \| authz \| finansal \| persistence-schema>` |

## Kriter → kanıt tablosu (gerçek GÖZLEMLER — sözleşme değil)

| K | Kanıt sözleşmesi (plandan) | Gözlemlenen kanıt | Karşı-örnek probe'ları | Sonuç |
|---|---|---|---|---|
| K-1 | <identity/cardinality/fields/scope özeti> | <ne gözlendi — gerçek satır/yanıt/ölçüm> | CE-1 `<probe adı>` → gözlenen ret: `<gerçek sınıf>` = beklendi ✓ / ≠ beklendi ✗ | karşılandı |
| K-2 | … | … | … | … |

**Ret-sebebi kuralı:** her karşı-örnek probe'unun GERÇEK gözlenen ret sınıfı
yazılır; beklenen kimlikle eşleşmeyen RED (altyapı hatası, beklenmeyen hata)
başarılı negatif test SAYILMAZ — tabloda ✗ olarak dürüst kalır.

**PENDING dürüstlüğü:** probe'u koşulmamış/hazırlanmamış kriter
"karşılandı" yazılamaz — PENDING olarak listelenir.

## Başarısızlık-modu kapanışı

| Mod (plan enumerasyonu) | Durum |
|---|---|
| yetki/tenant sınırı | <kapalı — nasıl kanıtlandı / taskta geçerli değildi — neden> |
| idempotens bağlamı | … |
| kaynak tükenmesi | … |
| başarısızlıkta kanıt dayanıklılığı | … |
| kanonik tek-kaynak | … |

## Pilot ölçüm kaydı (plan-pilotu denek kolunda doldurulur)

- Bulgu listesi: `finding_id` · `first_detected_stage` (plan / self-probe /
  review) · `axis` (tasarım / kanıt-hesabı / ikisi).
- R_self-probe = N_pre-PR / (N_pre-PR + N_review-only) = …
- R_enumerable (donmuş sürümde öngörülen / tüm review bulguları) = …
- Uygun bulgu yoksa: "metrik tanımsız" (yapay %100 yok).

## Bilinçli kapsam dışı

- <planın kapsam-dışı bölümü + uygulama sırasında doğan yeni dışlamalar>

## Sabit hatırlatmalar

- Plan değişikliği gerektiyse: CompletionClaim'den ÖNCE amendment (yeni sürüm
  + freeze + plans.jsonl satırı) — donmuş sürüm geriye dönük düzeltilemez.
- Bu claim tek reviewer'ın (n=1) yaklaşım denetimiyle eşleşir; bağımsız
  doğrulama iddiası taşımaz.
