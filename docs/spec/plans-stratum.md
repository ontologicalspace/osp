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

- **Freeze anchor** = pre-registration NOTES freeze kaydı: `{tarih-saat,
  plan sürümü, digest, osp_revision, run bağlamı}`.
- Dürüstlük sınırı: bu ankör kriptografik timestamp DEĞİLDİR — ritüelin
  şeffaflığına (NOTES dizisi + run-dir artefaktları) bağlı, sonradan
  denetlenebilir bir kayıttır.

## 3. Digest döngüselliği yasağı + immutable sürüm zinciri

**Kural 1 — kendi digest'ini taşıma:** hiçbir plan sürümü KENDİ digest'ini
dosya içinde taşımaz. `plan-v0` originating digest DAHİL hiçbir digest alanı
içermez (v0'nın originating digest'i kendi digest'i olurdu → döngüsel tanım).
`plan-vN` (N ≥ 1) `previous_digest` + `originating_digest` taşıyabilir. Digest
kimlikleri yalnız ÜÇ dış kayıtta yaşar: (1) NOTES freeze kaydı, (2)
`plans.jsonl` satırı, (3) CompletionClaim.

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
  "freeze_anchor_ref": "NOTES pre-registration bölümü (tarih + osp_revision + run bağlamı)",
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
          "expected_rejection": "InvalidWitnessEvidence",
          "probe_ref": "nan_weight_rejected"
        }
      ]
    }
  ]
}
```

**PlanEvidenceContract ≠ ObservedEvidence:** `evidence_contract` +
`counterexamples` alanları **önceden tanımlanmış kanıt BEKLENTİSİ** taşır
(gözlemlenen sonuç değil — gözlem yalnız CompletionClaim'de raporlanır).

**`expected_rejection` ZORUNLU:** RED'in VARLIĞI değil doğru RET KİMLİĞİ
kanıtlanır; ret kimliği konu issue'nun normatif hata kararıyla eşleşir.
Beklenmeyen hata / altyapı hatasıyla üretilen RED **başarılı negatif test
SAYILMAZ** (ör. #217: NaN ağırlık → `InvalidWitnessEvidence` beklenirken JSON
parse hatası veya quorum-ret gözlenirse kanıt GEÇERSİZDİR).

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
- `--bar-elicitation` kanıt-sorusu genişletmesi (#213 §2 üretim akışı) AYRI
  şerittir.

## 8. Yaşam döngüsü

```
plan-v0.md yazılır (K-N kanıt-hesabı dahil, digest alanı YOK)
  → freeze: digest = sha256(baytlar) NOTES freeze kaydına + plans.jsonl satırına
  → KOD
  → (gerekirse) amendment → plan-v1.md [previous_digest=v0digest, originating_digest=v0digest]
      → yeni freeze kaydı + YENİ plans.jsonl satırı (append; eski satır durur)
  → probe'lar koşulur (her CE: beklenen ret kimliği doğrulanır)
  → CompletionClaim (PR gövdesi): aktif + originating digest + zincir
      + K→sözleşme→GÖZLEMLENEN kanıt→gerçek ret sınıfı tablosu
```

Şablonlar: `docs/templates/plan-v0.md`, `docs/templates/completion-claim.md`.
