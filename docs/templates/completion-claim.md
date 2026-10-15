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
| freeze ankörü (dış) | `freeze_evidence_ref = <yorum URL'si>` — created_at `<GitHub UTC>` · chronology_verdict `<PASS\|RED\|UNVERIFIABLE>` · verified_at `<UTC>` · `published_plan_bytes_sha256 == freeze_manifest.plan_digest` |
| scope-trigger | `<public API \| authz \| finansal \| persistence-schema>` |

## Kriter → kanıt tablosu (gerçek GÖZLEMLER — sözleşme değil)

| K | Kanıt sözleşmesi (plandan) | Gözlemlenen kanıt |
|---|---|---|
| K-1 | <identity/cardinality/fields/scope özeti> | <ne gözlendi — gerçek satır/yanıt/ölçüm> |
| K-2 | … | … |

### Karşı-örnek probe sonuçları (her CE için ZORUNLU bağ)

| probe_ref | fixture_ref | execution_ref | code_revision / verifier_revision | expected (layer:ret) | observed_rejection | result |
|---|---|---|---|---|---|---|
| `<CE-1 probe adı>` | `<girdi dosyası / canlı yanıt>` | `<CI koşusu / koşu kaydı / log>` | `<commit sha> / <script sha256>` | `engine:InvalidWitnessEvidence` | `<gerçek ret sınıfı>` | `MATCH` |
| `<CE-2 probe adı>` | `…` | `…` | `…` | `verifier:K1.EmptyEvidenceSet` | `<gerçek>` | `MATCH \| MISMATCH \| PENDING` |

> **Bağ zorunluluğu:** `probe_ref` + `execution_ref` + `code_revision`
> olmadan yazılmış bir sonuç YALNIZCA İDDİADIR — "testin adı tabloya
> yazılmış" ile "test koşuldu ve doğru ret kimliği gözlendi" birbirinden
> ayırt edilir (ExpectedRejection ≠ ObservedRejection ≠ VerifiedRejection;
> doğrulanmış ret = gözlemin koşu + kod-sürümü bağıyla MATCH olması).
> **`fixture_ref ≠ execution_ref`:** doğrulama GİRDİSİ (fixture / canlı yanıt)
> ile GERÇEK KOŞU kaydı ayrı referanslardır — fixture'ın varlığı testin
> çalıştığını kanıtlamaz. `verifier_revision` = doğrulayıcı script'in sha256'ı
> (`code_revision` = Git commit SHA'dan ayrı).
> **Kanıt seviyeleri:** bağımsız platform kanıtı (GitHub'daki ankör/içerik/
> sunucu zamanları) ile yerel yürütme kanıtı (verifier koşuları/fixture/
> sonuçlar) İKİ AYRI iddia olarak yazılır; ikincisi birincinin yerine geçmez.

**Ret-sebebi kuralı:** her probe'un GERÇEK gözlenen ret sınıfı yazılır;
beklenen `rejection_layer` + ret kimliğiyle eşleşmeyen RED (altyapı hatası,
beklenmeyen hata, yanlış katman) başarılı negatif test SAYILMAZ — tabloda
`MISMATCH` olarak dürüst kalır.

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
