# Yazar-ayrımı deneyi — tasarım notu ve pre-registration (issue #171)

**Durum:** DONDURULDU **v2** (2026-10-06; ilk dondurma aynı gün — v2, review
geri-bildirimiyle veri üretilmeden ÖNCE yapıldı: INV-E1 ayrımı, 2×2 faktöriyel
yeniden adlandırma + estimand'lar, kör endpoint hiyerarşisi, replikat dürüstlüğü).
**v2 onay aldı**; onaylayan review'un iki P2'si de nota işlendi (aynı gün,
hâlâ veri yok): imzalı estimand formülleri (§ D4) + label-blinded terminoloji,
değiştirilemez-girdiden türetilen seed ve `recognized_authorship` alanı (§4, §3).
Bu not, issue #171'in "deney öncesi dondurulur" başlığındaki açık kararların
yanıtlarıdır ve herhangi bir kol koşulmadan önce yazılmıştır (adversarial probe
emsali: `hypothesis_registered_at`). **Kapsam:** planlama + tasarım dondurma.
Uygulama PR'ı (motor işi) ayrı; envanter §7'de.

## 0. Deneyin iddiası (ölçülecek tek cümle)

Karar katmanı (bar + τ sıralaması + üç-karar) şimdiye kadar **tek yazarlı** (analist):
run 11–16'nın decision-utility verisi, barı ve yamayı aynı tarafın yazdığı düzende.
Soru: **bar, yazar-önyargısı DIŞINDAKİ önerileri ayırt ediyor mu?** Bu deney o
ayrım gücünün ilk ölçümüdür — yanıtın "evet" çıkması değil, **ölçülebilmesi** hedef.

## 1. Zemin: kod bugün ne yapıyor (doğrulanmış, 2026-10-06, main `6aa8c7e`)

Araştırma sırasında doğrulanan kod gerçekleri — tasarım bunlara yaslanır, varsayım değil:

1. **Besleme kanalı bugün dar — ama bu INV-T1'in garantisi DEĞİL (v2 düzeltmesi).**
   Gerçek-LLM yolu (`osp trajectory attempt --llm real` → `RuntimeLlmClient`)
   yalnızca **`AgentTaskView`** besler: odak node'un **ölçülmüş** koordinatları,
   ölçülmüş import listesi (STRUCTURAL CONTEXT), predicate görünümü, izin maskesi.
   Analojinin yüz analizi / using tablosu / ĉ el hesapları bu yola bugün
   **girmiyor** — fakat bu, D3 yolunun AgentTaskView-dışı bir şey serialize
   etmemesi olgusu (contingent implementation fact), tip-düzeyi bir garanti değil.
   INV-T1'in kendi kapsamı **hedef-koordinat görünmezliğidir** (preferred_vector
   agent'a görünmez); "yazar-sızması" için tip-garantisi yok. Bu yüzden deney
   kendi invariant'ını tanımlar: **INV-E1** (D2). Yazar-ayrımı körlüğü
   **procedural blindness**'tır ve aşağıdaki sıra disipliniyle kurulur.
2. **Model yapılandırması bugün yok.** `RuntimeConfig::default()` → model
   `"gpt-4o-mini"`, endpoint OpenAI; `from_env()` yalnız `OPENAI_API_KEY` okur
   (`crates/osp-llm-runtime/src/runtime.rs:113`). Karar D3'te.
3. **Envelope'da dürüst boşluk zaten tasarlanmış.** #178 sonrası attempt envelope
   tüketilen proposals dosyası yoksa (llm-real durumu) digest'i **null** yazar
   (`run_envelope.rs:333` yorumu: "yoksa null (legacy/llm-real)"). Kol B'nin
   önerileri ayrı kanıt artifact'ına yazılacak (§5); main ledger envelope'u
   dokunulmaz.
4. **Prompt incelemesi mümkün.** `Runtime::complete_raw` (system, user) çiftini
   ham döndürür — deneyin prompt-dondurma/persist gereksinimi bu API üzerinden
   karşılanır (persist bugün yok; §7 envanter).
5. **Bağımlılık #166 TAMAM:** kanonik attempt artifact bugün main'de (PR #187,
   2026-10-06). #169 (hypothesize-extraction) AÇIK → kol B'nin öngörü denetimi
   bu deneyde **elle** kalır (transkripsiyon; ön koşul değil — issue de öyle diyor).

## 2. Donmuş kararlar

### D1 — Görev seçimi: kural dondurulur, örnek oturum açılışında bağlanır

- **Kural (donduruldu):** hedef gerçek-iş yüzünden, `osp suggest-targets`'ın
  ölçülmüş aday tablosundan; iki kol AYNI task + AYNI baseline commit + aynı
  sözleşme üzerinde koşar. Seçim anı: deney oturumunun açılışı, herhangi bir kol
  koşmadan ÖNCE `dogfood/runs/<run>/task.json` + baseline digest olarak kayda geçer.
- **Kısıt (yeni, 2026-10-06):** hedef, Nexus'ta o an aktif agent işleriyle **ayrık
  bir domain'den** seçilir (bu hafta: wallet compiled-query tenant-isolation +
  customer PII yüzeyleri dokunulmaz; aktif dalların diff'ine giren dosyalar hariç).
- **Tekrar sayısı:** **3 farklı domain, tek oturum** (issue önerisi benimsendi) —
  task başına 2 kol → 6 öneri seti. (Tek-task'lı "tek run" reddedildi: task-idiosinkrasi
  ile yazar-etkisi ayrıştırılamazdı.)

### D2 — Körleme: INV-E1 + prompt B'den ÖNCE donar, B'nin çıktısı A yazılana kadar mühürlü

**INV-E1 — Author-view isolation (deneyin kendi invariant'ı; INV-T1'den AYRI ve
onun yerine geçmez):** Kol-B prompt artifact'ı YALNIZCA önceden tanımlı
baseline/task DTO'sundan türetilebilir (motor ölçümü `AgentTaskView` serialization
+ sabit system prompt); prompt freeze anında A tarafına ait hiçbir artifact mevcut
olamaz; prompt digest A yazımından ÖNCE kayda geçer. Denetlenebilirlik: freeze
adımında prompt'u üreten girdi listesi (okunan dosyalar + kullandığı motor
çıktıları) kayda geçer — "yalnızca şu girdiler" iddiası after-the-fact doğrulanabilir.

Tek-analist gerçekliği altında tek yönlü sızıntı kanalı **insan** (LLM stateless,
inv #11 — oturum-öncesi hafızası yok). Bu yüzden sıra şöyledir:

1. Task + baseline donar (D1).
2. **Kol-B prompt'u mekanik olarak kurulur ve digest'i kayda geçer** — yalnız motor
   çıktılarından (AgentTaskView serialization + system prompt); analist bu adımda
   yüz analizi YAPMAZ (yapmasa da yapmasa da prompt aynı olur — kanal zaten yok; ritual
   yine de sırayı sabitler).
3. Kol B koşar; çıktı (`llm-proposals.json` + ham yanıt) **mühürlenir**: parse
   makinidir, analist içerik/`reasoning` alanını OKUMAZ.
4. Kol A ritüeli tamamen her zamanki gibi yürür (yüz analizi, using tablosu, ĉ el
   hesapları, el tahminleriyle bar, proposals.json). A'nın `proposals.json` +
   attempt tahminleri diske yazılınca...
5. Mühür açılır; iki kol birlikte ölçülür (§4).

Yürütme sırasının kendisi (A-önce/B-önce) epistemik olarak yük taşıMAZ; taşıyan
**yazım sırasıdır** (prompt < A-yazımı < B-okuması). Yine de operasyonel sıra
yukarıdaki gibidir (B'nin mühürlü beklemesi A'yı besleyemez).

### D3 — Model seçimi yapılandırılabilir YAPILIR (küçük motor işi, §7)

`OSP_LLM_MODEL` + `OSP_LLM_ENDPOINT` env override'ları `RuntimeConfig::from_env`'e
eklenir; default `gpt-4o-mini` kalır. Gerekçe: (i) Faz-2 "istediğin kadar LLM bağla"
hedefinin girişi — aynı protokol tekrarlanarak model-karşılaştırma verisi birikir;
(ii) deney satırlarında `model` alanı dolu olmalı ki veri sonradan yorumlanabilir
olsun (hangi model koştu kayıtsız kalmasın). v1 deneyi tek modelle koşar
(`gpt-4o-mini`); model çeşitliliği v1'in iddiası DEĞİL.

### D4 — Bar ve deneyin kimliği: **2×2 faktöriyel tasarım** (v2 yeniden adlandırma)

Issue "TEK fark öneri yazarlığı" diyordu; donmuş tasarım bar yazarlığını da
değiştirdiği için dürüst adı bu: **iki faktörlü faktöriyel** —
**proposal-author (insan/LLM) × bar-author (insan/LLM)**.

- **Kol A** = (insan-öneri, insan-bar): bar ritüelin her zamanki kuralından — el
  tahminlerinden donar (oncelik-plani §2 "τ yalnız el tahminlerinden" aynen).
- **Kol B** = (LLM-öneri, LLM-bar): aynı kör view ile **ikinci bir completion**:
  "kabul barın olarak öngörülen değişim-sonrası coupling değerini bildir ve
  önerilerini öngörülen etkiye göre sırala". LLM'in barı ve τ sıralaması kayda geçer.
- **Çapraz yargı hücreleri** (off-diagonal): A önerileri B'nin barıyla,
  B önerileri A'nın barıyla değerlendirilir — factorial'in eksik hücreleri
  bar-ONLY ve öneri-ONLY okumalara izin verir.

**Önceden donmuş estimand'lar** (deney sonrası eklenmez; v1'de betimsel —
istatistiksel iddia §6 sınırına tabi). Y = hücrenin bar-geçme skoru (v1: binary;
domainler üzerinden oran). Formüller **imzalı ve sabit** — etkileşimin yönü
sonradan yeniden yorumlanamaz (review onayı ile v2'ye işlendi):

```text
PE_H = Y(LLM-öneri,  insan-bar) − Y(insan-öneri, insan-bar)
PE_L = Y(LLM-öneri,  LLM-bar)   − Y(insan-öneri, LLM-bar)
INT   = PE_L − PE_H      (pozitif ⇒ proposal-author etkisi LLM barında daha büyük)

BE_H = Y(insan-öneri, LLM-bar)   − Y(insan-öneri, insan-bar)
BE_L = Y(LLM-öneri,  LLM-bar)    − Y(LLM-öneri,  insan-bar)
BE    = BE_L − BE_H     (pozitif ⇒ bar-author etkisi LLM önerilerinde daha büyük)
```

(v2 yorumundaki PE1/PE2/BE1/BE2 adları ≡ PE_H/PE_L/BE_H/BE_L.) Issue'nun özgün
sorusu ("bar, yazar-önyargısı dışındaki önerileri ayırt ediyor mu") tam INT ve
BE hücrelerinde görünür.

**Cebirsel not (merge onayı review'undan; yeni tur gerektirmeyen ekleme):**
2×2 faktöriyelin simetrisi gereği **INT ile BE AYNI interaction contrast'tır** —

```text
INT = [Y(L,L) − Y(H,L)] − [Y(L,H) − Y(H,H)]
BE  = [Y(L,L) − Y(L,H)] − [Y(H,L) − Y(H,H)]   ⟹   INT == BE
```

— ikisi iki bağımsız etkileşim sonucu gibi raporlanMAZ; biri proposal-author,
diğeri bar-author perspektifinden aynı contrast. İki formül kayıtta kalır ve
`INT == BE` implementation'da **symmetry-check invariant'ı** olarak pinlenir
(schema/test güvencesi — sayısal eşitsizlik yalnızız hesaplama hatasını
gösterebilir).

**Ölçümler:** (i) her hücrenin bar-geçme durumu; (ii) **τ-sırası ihlali oranı** —
her kolda öngörülen sıralama vs ölçülen sıralama (adversarial probe P1-2 bulgusunun
LLM tarafındaki karşılığı; #169 yokluğunda el transkripsiyonuyla).

### D5 — Uygulama ve promote: yok (ölçüm stratumu)

- Kol B (ve A) yamaları **yalnızca atıl worktree'de** uygulanır ve ölçülür
  (`after.json`); **promote EDİLMEZ** — "engineered pairing; not organic" etiketi
  bunun ta kendisi. Ölçüm doğruluğu ≠ değişim geçerliliği (run-1 dersi aynen).
- Nexus tarafı tamamen salt-okunur + OSP `Temp/` altında atıl worktree
  (Nexus kuralları 2026-10-06: kod girişi yalnız branch+PR — bu deneyde giriş
  zaten sıfır; worktree add/remove yerel metadata işlemidir, agent'lar yoğunsa
  boşlukta yapılır).

### D6 — Başarısızlık da veridir (dürüst boşluk)

- Parse/schema ihlali: **tamir döngüsü YOK** (v1). Ham yanıt + hitalet sebebi
  kayda geçer, `proposals_digest: null, reason: ...` — issue'nin "finalize null
  bırakır, dürüst boşluk" ilkesinin kol-B karşılığı.
- Tek istisna: geçici ağ hatasında **aynı prompt'la bir kez** yeniden denenebilir
  (retry kayda geçer; prompt değişmez). Prompt'u değiştiren/iyileştiren ikinci
  deneme = yeni satır değil YASAK (statelessness ölçümünü kirletir).

## 3. Kayıt stratum'ı: `dogfood/llm-author.jsonl`

Adversarial.jsonl emsali (ayrı kanıt katmanı; main ledger'a YAZILMAZ; event-2
SAYILMAZ). Satır şeması `llm-author-v1`:

```json
{
  "schema_version": "llm-author-v1",
  "stratum": "engineered pairing; NOT organic evidence; NOT event-2; never promoted",
  "preregistered_at": "2026-10-06 (docs/notes/yazar-ayrimi-deney-tasarimi-171.md)",
  "run_id": "<task-kimliği>",
  "arm": "A(human) | B(llm)",
  "task_ref": "dogfood/runs/<run>/task.json",
  "target": "<dosya + node id>",
  "baseline_commit": "<sha>",
  "osp_revision": "<osp binary sha>",
  "model": "gpt-4o-mini",
  "temperature": 0.3,
  "prompt_digest": "sha256:...",          // yalnız B: donmuş (system,user) çifti
  "proposals_digest": "sha256:... | null",
  "proposals_null_reason": null,
  "bar": {"kind": "hand-estimate | llm-elicited", "value": ...},
  "tau_predicted_order": [...],
  "tau_violations": null,
  "bar_pass": {"own": true, "cross": true},
  "hypothetical_vs_measured": {"armA": ..., "armB": ...},
  "overlap_with_human": null,             // yalnız B satırında — label-blinded değerlendirme sonucu
  "blind_eval": {"seed_derivation": "sha256(llm-author-v1/blind-order||task||baseline)",
                 "presentation_order": […],
                 "recognized_authorship": "yes | no | uncertain"},
  "token_usage": {"prompt": n, "completion": n}
}
```

## 4. Endpoint hiyerarşisi (önceden kayıtlı — sonradan eklenmez; v2)

**PRIMARY — deterministik motor ölçümleri** (yazar-taraflılığı taşımayan kanal):

1. Bar-geçme durumu (2×2 hücrelerinin her biri; D4).
2. Öngörü-ölçüm sapması (her kolda hypothetical vs ölçülen after).
3. τ-sırası ihlali oranı (D4).

**SECONDARY — betimsel (descriptive) endpoint, LABEL-BLINDED değerlendirmeyle:**

4. İnsan tercihiyle örtüşme/çelişme. Aynı analist A önerilerinin YAZARI olduğu
   için bu endpoint **self-author bias** taşır (v2 kabulü); körlük teknik olarak
   **label-blinded**'ır — etiket kalksa da yazar kendi cümlesini/tasarımını
   tanıyabilir; bu sorun endpoint'in descriptive kalmasıyla yönetilir. Prosedür:
   (a) sonuç aşamasında öneriler **A/B kimliğinden arındırılır** ve
   **değiştirilemez girdilerden deterministik türetilen seed**'le belirlenen
   sırada sunulur — seed = `SHA256("llm-author-v1/blind-order" ‖ task_digest ‖
   baseline_digest)`; analist sırayı seçemez, cherry-pick edilebilir order yok
   (uygulama notu: digest'ler fixed-width olduğundan concatenation sınırı
   pratikte belirgindir; şema ileride değişirse domain-separation string'i
   yanına length-prefix ya da canonical tuple serialization eklenir — v1'de
   yeterli, blocker değil);
   (b) değerlendirme SONRASI analist her öneri için `recognized_authorship:
   yes | no | uncertain` beyan eder (kendi metnini tanıyıp tanımadığı — kayda
   geçer, yorumda kullanılır); (c) etiket açıkça "descriptive" kalır — primary
   kanal motor ölçümleridir. Körleme prosedürü (seed + türetim formülü + sunum
   sırası + arındırma kuralı) `llm-author.jsonl` satırına yazılır.

**Yan-veri (ücretsiz):** token maliyeti — RQ5 hattıyla uyumlu.

## 5. Kol-B artifact'ları (run dizininde)

`dogfood/runs/<run>/` altına: `llm-prompt.json` (donmuş system+user + digest),
`llm-raw-response.txt`, `llm-proposals.json` (parse edilmiş DeltaProposal),
`llm-bar.json` (D4 elicitation). Bunlar envelope'un read-once zincirine GİRMEZ
(tüketici değil üretici kanalı; #178 invariant'ları dokunulmaz).

## 6. Riskler / bilinen sınırlar (dürüstlük listesi; v2 genişletilmiş)

- **n=3 task, n=1 model, n=1 analist, tek oturum:** ilk probe — iddia "ayırt
  edilebilirlik var/yok" değil, "ölçüm hattı kuruldu ve ilk noktalar şöyle".
  **İstatistiksel "author effect" iddiası bu stratumdan ÇIKMAZ** (PE1/PE2/BE1/BE2/INT
  v1'de betimsel okunur). Sonraki stratum: domain başına **bağımsız LLM
  replicate'ları** (aynı donmuş prompt, ayrı çağrılar; LLM stateless ama sampling
  stokastik — temperature satırda kayıtlı) + çok-analist genişlemesi ayrı bir
  tasarım kararıdır. Paper 2'nin tek-run stokastisite kabulü burada da geçerli:
  tek koşunun sonuçları buna göre sınırlandırılarak yazılır.
- **Aynı analist iki kolda da hakem** (üç-karar + çapraz yargı): tam ikinci-hakem
  körlemesi v1'de yok; kısmi telafiler — çapraz-bar hücreleri (D4) + secondary
  endpoint'in kimlik-arındırılmış sunumu (§4).
- **GPT-4o-mini'nin format uyumu** önceden biliniyor (LLM geçmişi: ProposalParse
  riski) — D6'nın varlık sebebi.
- Agent-etkinliği çakışması: D1 domain-ayrıklık kısıtıyla sınırlı; worktree işlemleri
  zamanlaması kullanıcıyla senkron.

## 7. Uygulama envanteri (motor PR'ı — #171 implementation; ayrı dalga/PR)

1. `RuntimeConfig::from_env`: `OSP_LLM_MODEL` / `OSP_LLM_ENDPOINT` (D3).
2. `trajectory attempt --llm real`: prompt + ham yanıt + usage persist'i (§5)
   — `complete_raw` üzerinden; mock yolu değişmez.
3. Bar elicitation ikinci completion'ı (D4) — aynı kör view; `--bar-elicitation`
   veya denk bir bayrak.
4. (Ertelenebilir) `llm-author.jsonl` satır üretimi motora alınır — v1'de oturum
   elle yazar (adversarial.jsonl emsali); sürtünme ölçülür, sonra #172 hattıyla
   absorbe kararı.

Sıra bağımlılıkları: #166 ✓ (bugün main'de). #188 (ledger attempt_digest) bu
deneyle AYRIŞIK — dokunma yüzeysiz; istenirse aynı küçükler oturumunda ayrı PR.

## 8. Zincir

- Issue #171 (bu notun kaynağı; açık kararlar §2'de tek tek yanıtlandı).
- **v2 re-freeze (2026-10-06, review geri-bildirimiyle):** INV-T1 kapsam
  düzeltmesi + INV-E1 tanımı; faktöriyel yeniden adlandırma + estimand dondurma;
  kör secondary endpoint + hiyerarşi; replikat dürüstlük sınırı. v1→v2 arasında
  HİÇBİR kol koşulmadı, veri üretilmedi — pre-registration bütünlüğü korundu.
- Adversarial probe notu (ayrı stratum disiplinin emsali): `dogfood/probes/`.
- Onceilik planı §2 (Dalga 2, "#171 canlı döngülerden BİRİ olarak").
- Run-1 dersi (ölçüm doğruluğu ≠ değişim geçerliliği) → D5.
- 2026-10-06 Nexus kuralları → D1/D5 kısıtları.
