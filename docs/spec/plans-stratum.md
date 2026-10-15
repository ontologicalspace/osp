# plans-v1 stratum — normatif sözleşme (#206 kesim-1)

Durum: **DONDURULMUŞ** (karar kayıtları: #206 [issuecomment-6169252150],
#213 [issuecomment-6169252679]; 3 plan-review turu sonrası). Şema donar —
sonraki değişiklikler additive backward-compat; RQ'lar pre-registration +
amendment ile revize kalır (#171 disiplini).

## 1. Konum ve dürüstlük işareti

`plans.jsonl`, bilinçli AYRI stratum olarak yaşar — `live-ledger-v1` /
`llm-author-v1` / `adversarial-probe-v1` yanında (hepsi `dogfood/` altında,
git-dışı yerel yaşam). Her satır şu işareti taşır (stratum kalıbı):

> `"stratum": "plan-intent (pre-code); NOT organic evidence; NOT event-2; never promoted"`

Plan verisi ölçüm/realization kanıtı DEĞİLDİR; kod-öncesi niyet kaydıdır ve
asla organik kanıt statüsüne terfi ettirilmez. `task_id` + `task_ref` ile
task/ledger kayıtlarına çapraz-bağlanır.

**plans.jsonl dosyası ilk pilot run'ında doğar.** Bu sözleşme şemayı önceden
dondurur; runtime/üretici bu şeritte YOK (Aşama-2 kararı pilot verisiyle).

## 2. ContentIdentity ≠ FreezeEvidence

SHA-256 bir plan sürümünün içerik KİMLİĞİNİ kanıtlar; o içeriğin **koddan önce
var olduğunu** tek başına kanıtlamaz. Pre-code freeze için dış ANKOR gerekir:

- **Freeze anchor (yerel)** = pre-registration NOTES freeze kaydı: `{tarih-saat,
  plan sürümü, digest, osp_revision, run bağlamı}`.
- **Freeze evidence (dış — #223 kararı, yön a)** = GitHub freeze-ankörü yorumu:
  strip'in issue'suna freeze ANINDA yayımlanan TEK yorum; manifest + planın TAM
  baytları. Zaman kanıtı GitHub sunucusundadır (`created_at`).

### 2.1 Dış freeze kanıtı sözleşmesi (GitHub yorum-ankörü)

**Yorum yapısı** (marker'lar `<run_id>` + `v<sürüm>` ile benzersizleştirilir):

```
<!-- osp:freeze-manifest:<run_id>:v<N> -->
{manifest JSON: schema=osp.freeze-manifest.v1, run_id, task_id, task_ref,
 plan_version, plan_digest, criteria[], osp_revision, frozen_at_local,
 decision_record_ref, previous_digest, originating_digest}
<!-- /osp:freeze-manifest:<run_id>:v<N> -->

<!-- osp:plan-bytes:<run_id>:v<N>:begin -->
<plan dosyasının TAM baytları — LF, UTF-8, BOM'suz, tam bir sondaki LF>
<!-- osp:plan-bytes:<run_id>:v<N>:end -->
```

**Normatif bayt-çıkarım kuralı** (bağımsız yeniden-implemente edilebilir; ham
Markdown `body` okunur — `body_text`/HTML DEĞİL):

```
body  = response["body"].encode("utf-8")
begin = (begin_marker + "\n").encode("utf-8")
end   = end_marker.encode("utf-8")
start = body.index(begin) + len(begin)
stop  = body.index(end, start)
published_plan_bytes = body[start:stop]
```

Güvenlik koşulları: (1) begin/end marker'ların HER BİRİ tam bir kez bulunmalı
(kardinalite ≠ 1 → RED `ByteExtractionMalformed`); (2) `start < stop` zorunlu —
empty-plan RED; (3) çıkarılan bayt yeniden kodlanmadan doğrudan hash'lenir;
end-marker ek LF tüketmez (sondaki LF planın son baytıdır). SHA-256 daima
dosyanın **gerçek baytları** üzerinde hesaplanır (LF + BOM'suz yazım; CRLF/BOM/
metin-normalizasyonu aynı görünen Markdown'da farklı digest üretir).

**Kabul koşulu:** `published_plan_bytes_sha256 == freeze_manifest.plan_digest`.

**Alan tablosu (normatif ayrım):**

| Alan | Normatif anlam |
|---|---|
| `freeze_anchor_ref` | yerel NOTES freeze kaydı (değişmez) |
| `freeze_evidence_ref` | GitHub freeze-ankörü yorum URL'si — `plans.jsonl` YENİ satırlarında zorunlu; URL hash'lenen plan dosyasında TAŞINMAZ (döngüsellik) |
| `frozen_at` | yerel beyan zamanı |
| `freeze_evidence_created_at` | sunucudan okunan oluşturma zamanı — bağımsız doğrulama kaydında yaşar |

**İmmutability + tekillik:** freeze yorumu hiçbir koşulda DÜZENLENMEZ
(`lastEditedAt == null` / `updated_at == created_at`); düzenlenme = normatif
RED (`FreezeAnchorEdited`), yeni-yorumla-onarım YOK — yalnız gerçek amendment
(vN+1) yeni ankör alır. Aynı `run_id + plan_version` için TEK ankör
(`FreezeAnchorDuplicate`). Yorumun silinebilmesi bilinçli sınırdır (§7).

**POST timeout-idempotency (beş durum):**

| POST / sorgu durumu | Karar |
|---|---|
| POST 201, yorum doğrulandı | Devam |
| Timeout, tam-eşleşen TEK yorum | mevcut yorumla devam |
| Timeout, İKİ eşleşen yorum | RED (`IdempotencyViolation`) |
| Timeout, eşleşen YOK | UNVERIFIABLE → sorgu yeniden denenir; otomatik ikinci POST ASLA |
| Sorgu hatası | BLOCKED |

Kalıcı belirsizlikte freeze denemesi BAŞARISIZ sayılır ve kayda geçer.

**İki-aşamalı K-2 (erken PASS yasağı):**

```
K2.PASS iff content_integrity==PASS AND anchor_integrity==PASS
         AND chronology==PASS AND record_binding==PASS
```

`chronology` yalnız ilk PushEvent gözlendikten sonra değerlendirilir;
`chronology == PENDING/UNVERIFIABLE` iken K-2=PASS yazılması RED
(`PrematureVerdict`). Kronoloji üçlüsü: `ankör.created_at < push.created_at`
PASS · `>` RED (`FreezeChronologyViolation`) · `==` UNVERIFIABLE
(`ChronologyIndeterminate` — eşit saniyede sıra çıkarılamaz).
**Karşılaştırma push'u sürüme göredir:** `v0` kronolojisi dalın İLK push'u ile
değerlendirilir; `vN` (N ≥ 1) amendment kronolojisi, amendment'ın kapsadığı İLK
SONRAKİ kod push'u ile değerlendirilir (amendment'ın kendi kod değişikliğinden
önce donduğunu kanıtlar) — v1 ankörünün ilk push'tan SONRA oluşturulması bu
nedenle RED DEĞİLDİR; karşılaştırma noktası amendment'a ait push'tur.

**Doğrulayıcı prosedür + `fixture_ref ≠ execution_ref`:** doğrulama girdisi
(fixture) ile yürütme kanıtı (gerçek koşu kaydı) AYRI kayıtlardır; koşu kaydı
`probe_ref · fixture_ref · fixture sha256 · verifier_command ·
verifier_revision (script sha256) · code_revision (Git commit SHA) ·
executed_at · gözlenen ret · sonuç` taşır. Negatif probe'lar yalnız izole
fixture'larla çalışır — gerçek issue yalnız üretim freeze olgularını taşır.
İlke: *freeze kanıtını doğrulayan mekanizma, doğruladığı kanıtın geçmişini
değiştirmez.* Kanıt seviyeleri ayrı tutılır: bağımsız platform kanıtı
(GitHub'daki ankör + içerik + sunucu zamanları) ile yerel yürütme kanıtı
(verifier koşuları/fixture/sonuçlar) birbirinin yerine GEÇMEZ.

**Dürüstlük sınırları:** NOTES kaydı kriptografik timestamp DEĞİLDİR. GitHub
`created_at` sunucu-saatlidir; ANCAK kronoloji en az iki doğrulanmış OLAYın
ilişkisidir — sunucu zamanının varlığı tek başına kronoloji ispatı değildir
(PushEvent bağı şart; §7).

## 3. Digest döngüselliği yasağı + immutable sürüm zinciri

**Kural 1 — kendi digest'ini taşıma:** hiçbir plan sürümü KENDİ digest'ini
dosya içinde taşımaz. `plan-v0` originating digest DAHİL hiçbir digest alanı
içermez (v0'nın originating digest'i kendi digest'i olurdu → döngüsel tanım).
`plan-vN` (N ≥ 1) `previous_digest` + `originating_digest` taşıyabilir. Bir
sürümün **kendi** digest'i yalnız DÖRT dış kayıtta yaşar: (1) NOTES freeze
kaydı, (2) `plans.jsonl` satırı, (3) CompletionClaim, (4) GitHub freeze-ankörü
yorumundaki manifest (#223'ten itibaren). Dış ankörün URL'si (`freeze_evidence_ref`)
hash'lenen plan dosyasında TAŞINMAZ (§2.1 döngüsellik kuralı). **Önceki sürümlerin**
digest'lerinin (`previous_digest`/`originating_digest`) vN dosyasında
taşınması bu kuralı İHLAL ETMEZ — yasak olan yalnızca bir sürümün KENDİ
digest'ini içermesidir (döngüsel tanım).

**Kural 2 — immutable sürümler:** her amendment TAM YENİ SÜRÜM üretir
(`plan-v0.md`, `plan-v1.md`, …). Her sürüm donmuş TAM plandır; önceki sürüm
dosyası ASLA değiştirilmez; yeni sürüm öncekinin tamamını + amendment zinciri
satırını içerir.

**Kural 3 — append-only kayıt:** `plans.jsonl` her donmuş sürüm için AYRI
satır append eder; eski satırlar güncellenmez. `previous_digest` önceki
sürümün gerçek içerik kimliğidir.

**Kural 4 — aktif sürüm tanımı:** aktif plan = **freeze kaydı VE kesintisiz
amendment zinciriyle doğrulanmış en yüksek N** (yalnız sayısal en yüksek N
değil — zinciri kopuk/donmamış sürüm aktif sayılmaz). CompletionClaim aktif
digest + originating digest + amendment zincirini AYRIT eder.

## 4. Satır şeması (plans-v1)

```json
{
  "schema_version": "plans-v1",
  "stratum": "plan-intent (pre-code); NOT organic evidence; NOT event-2; never promoted",
  "run_id": "2026-10-15-217-numeric-validity",
  "task_id": "31",
  "task_ref": "dogfood/runs/<run-dir>/task.json",
  "plan_version": 0,
  "plan_ref": "dogfood/runs/<run-dir>/plan-v0.md",
  "plan_digest": "sha256:…",
  "originating_digest": null,
  "previous_digest": null,
  "frozen_at": "2026-10-15T09:00:00+03:00",
  "freeze_phase": "pre-code",
  "freeze_anchor_ref": "NOTES pre-registration bölümü — BU SÜRÜMÜN freeze kaydı (tarih + osp_revision + run bağlamı)",
  "freeze_evidence_ref": "https://github.com/<org>/<repo>/issues/<n>#issuecomment-<id>",
  "scope_trigger": "authz",
  "failure_modes": ["yetki/tenant sınırı", "idempotens bağlamı", "kaynak tükenmesi", "başarısızlıkta kanıt dayanıklılığı", "kanonik tek-kaynak"],
  "criteria": [
    {
      "id": "K-1",
      "evidence_contract": {
        "identity": "InvalidWitnessEvidence sınıfı (INV-T9 operational fault)",
        "cardinality": "her geçersiz kanıt olayı için tam bir ret",
        "required_fields": ["weight", "quorum_threshold", "min_approvers"],
        "measurement_scope": "kanonikleştirme/değerlendirme sınırı (wire parse değil)"
      },
      "counterexamples": [
        {
          "id": "CE-1",
          "input": "programmatic NaN-weight evidence (issue #217)",
          "expected_verdict": "RED",
          "rejection_layer": "engine",
          "expected_rejection": "InvalidWitnessEvidence",
          "probe_ref": "nan_weight_rejected"
        }
      ]
    }
  ]
}
```

**Alan notları:**

- **`rejection_layer` ZORUNLU — ret hangi katmanda üretilmeli?** Motor RED'i
  ile doğrulayıcı RED'i ayrılır: `engine` = uygulamanın normatif hata sınıfı
  üretmesi (ör. #217: NaN ağırlık → `InvalidWitnessEvidence`); `verifier` =
  senaryo/kanıt doğrulayıcısının karşı-örnek girdiyi BAŞARI SAYMAYARAK
  reddetmesi (ör. boş sonuç kümesi → `K1.EmptyEvidenceSet` — burada
  uygulamanın hata üretmesi GEREKMEZ; doğru davranış doğrulayıcının
  başarısızlık kararıdır). Ret kimliği ilgili kriterin doğrulama sözleşmesiyle
  adlandırılır. Bu ayrım olmadan motor hatası ile verifier ret'i birbirine
  karışır — #213'ün çıkış noktası ikinci gruptur.
- **`freeze_phase`:** `v0` için zorunlu `"pre-code"`; `vN ≥ 1` için
  dondurulduğu yürütme aşaması (örn. `"mid-execution"`). Amendment'ın ne
  TARİHTE değil yürütmenin HANGİ AŞAMASINDA donduğu böyle denetlenir — stratum
  etiketi `pre-code` başlangıç planını niteler, amendment'ı değil.
- **`freeze_anchor_ref`:** genel NOTES açıklaması DEĞİL — İLGİLİ SÜRÜMÜN
  gerçek freeze kaydını gösterir.
- **`freeze_evidence_ref`:** dış freeze kanıtının (GitHub yorum-ankörü, §2.1)
  URL'si — #223'ten itibaren YENİ satırlarda ZORUNLU; eski satırlar historical
  (geriye-dönük eklenmez). URL hash'lenen plan dosyasında taşınmaz.
- **PlanEvidenceContract ≠ ObservedEvidence:** `evidence_contract` +
  `counterexamples` alanları **önceden tanımlanmış kanıt BEKLENTİSİ** taşır
  (gözlemlenen sonuç değil — gözlem yalnız CompletionClaim'de raporlanır).

**`expected_rejection` ZORUNLU:** RED'in VARLIĞI değil doğru RET KİMLİĞİ
kanıtlanır; ret kimliği `rejection_layer` ile birlikte konu issue'nun
normatif kararıyla eşleşir. Beklenmeyen hata / altyapı hatasıyla üretilen RED
**başarılı negatif test SAYILMAZ** (ör. #217: NaN ağırlık → engine katmanında
`InvalidWitnessEvidence` beklenirken JSON parse hatası veya quorum-ret
gözlenirse kanıt GEÇERSİZDİR).

## 5. Kanıt-hesabı durum matrisi (#213)

| Durum | Sonuç |
|---|---|
| Kriterin karşı-örneği yok | **RED — kanıt-tanımsız** (pre-registration readiness'te reddedilir) |
| Karşı-örnek tanımlı, probe koşulmadı | **PENDING** (başarı sayılmaz) |
| Probe beklenen yanlış-başarıyı beklenen ret kimliğiyle reddetti | Doğrulanmış negatif test kanıtı |

Aşama zinciri: **Plan niyeti (kod öncesi) → Pre-registration Readiness
(eksik karşı-örnek = RED) → Probe Execution → CompletionClaim (gerçek gözlem +
digest bağı + kapsam).** Probe test adının planda yer alması testin koştuğunu
kanıtlamaz.

**Ret üçlüsü: ExpectedRejection ≠ ObservedRejection ≠ VerifiedRejection.**
Beklenen ret planın kanıt sözleşmesinde tanımlanır; gözlenen ret probe
çıktısında okunur; DOĞRULANMIŞ ret ancak gözlemin `execution_ref` (hangi
koşu/log) + `code_revision` (hangi kod sürümü) bağıyla MATCH olarak
bağlanmasıyla oluşur. CompletionClaim bu üçünü ayrı alanlarda taşır
(`docs/templates/completion-claim.md`).

## 6. Pilot ölçümleri (Aşama-2 kararının girdisi)

- **R_self-probe = N_pre-PR / (N_pre-PR + N_review-only)** — PR öncesi
  probe'larla yakalanan uygun bulgular / tüm keşfedilmiş uygun bulgular.
  Aynı bulgu tekilleştirilir; uygunluk sınıfı ÖNCEDEN tanımlanır. Payda
  yalnız review bulguları DEĞİLDİR (pre-PR yakalamalar kaybolur).
- **R_enumerable = planın DONMUŞ sürümünde öngörülmüş review bulguları /
  tüm review bulguları.** Amendment ile sonradan eklenen karşı-örnekler ilk
  planın öngörü başarısına SAYILMAZ.
- Uygun bulgu çıkmayan pilotta oranlar %100 sayılmaz — metrik **tanımsız**
  olarak kaydedilir. Review tur sayısı tek başına metrik değildir.
- Her bulgu kaydı: `finding_id` + `first_detected_stage` (plan / self-probe /
  review) + `axis` (tasarım-niyeti / kanıt-hesabı / ikisi de).
- **Araştırma sorusu:** karşı-örneklerin önceden tanımlanması yalnız review
  maliyetini düşürüyor mu, yoksa çözümün doğrulanabilirlik kalitesini gerçekten
  yükseltiyor mu? **Bağımsızlık sınırı (dürüst):** karşı-örnekleri çözümü
  yazan agent'ın aynı zihni yazıyorsa bağımsızlık bütünüyle sağlanmış
  değildir (run-27 yazar-tekilliği kör noktası); etkinlik ampirik kalır.

## 7. Bilinçli sınırlar

- **Rust/runtime YOK** — sürümleme motoru, otomatik şema üreticisi, gate YOK
  (bu şerit normatif metin + şablon).
- **Op-matrix = #172 K5'in genelleştirilmesi** (paralel mekanizma YOK);
  plandaki operasyon beyanı BEYAN EDİLMİŞ NİYET'tir, K5'in ölçülmüş
  doğrulamasıyla eşdeğer sayılmaz. `modified_entities` için #202
  graph-ifade edilememe sınırı geçerlidir.
- **D8:** plan koordinat/uzay-pozisyon içermez.
- **n=1 reviewer:** plan-kapısı insan denetimidir; bağımsız-reviewer iddiası
  taşımaz.
- **Dış ankör sınırları (#223):** freeze yorumu silinebilir (GitHub tarafında) —
  ankörün yokluğu freeze'i geçersiz KILMAZ ama bağımsız kronoloji kanıtı düşer;
  `freeze_evidence_ref` plans.jsonl'de kalır. Events API geçmişi sınırlıdır
  (≤300 olay, ≤30 gün; görünürlük 30sn–6sa gecikebilir) — uzun vadeli kanıt
  deposu DEĞİL; PushEvent bulunamaması = UNVERIFIABLE (RED değil), sonradan
  yeniden denenebilir; arşivlenen yerel event JSON'u bağımsız sunucu kaydıyla
  eş-güvenli SAYILMAZ; her doğrulama `verified_at` taşır.
- **K-2 yayım fallback tablosu (#223):** tam plan baytları ankörde = normal
  PASS yolu · manifest + ilk kod PR'inde bayt-özdeş kopya = digest
  doğrulanırsa uygun · manifest-only = K-2 PENDING (karşılanmadı —
  yanlış-success YOK).
- `--bar-elicitation` kanıt-sorusu genişletmesi (#213 §2 üretim akışı) AYRI
  şerittir.

## 8. Yaşam döngüsü

```
plan-v0.md yazılır (K-N kanıt-hesabı dahil, digest alanı YOK)
  → freeze: digest = sha256(baytlar)
      → GitHub freeze-ankörü yorumu (manifest + TAM baytlar; §2.1 — kronoloji:
        ankör.created_at < ilk kod push'unun PushEvent.created_at)
      → NOTES freeze kaydı + plans.jsonl satırı (freeze_evidence_ref = ankör URL'si)
  → KOD
  → (gerekirse) amendment → plan-v1.md [previous_digest=v0digest, originating_digest=v0digest]
      → yeni freeze kaydı + YENİ ankör yorumu (vN; eski ankör dokunulmaz)
        + YENİ plans.jsonl satırı (append; eski satır durur)
  → probe'lar koşulur (her CE: beklenen ret kimliği doğrulanır; fixture_ref ≠ execution_ref)
  → CompletionClaim (PR gövdesi): aktif + originating digest + zincir
      + K→sözleşme→GÖZLEMLENEN kanıt→gerçek ret sınıfı tablosu
      + freeze-ankör satırı (URL + created_at + chronology_verdict + verified_at)
```

Şablonlar: `docs/templates/plan-v0.md`, `docs/templates/completion-claim.md`.
