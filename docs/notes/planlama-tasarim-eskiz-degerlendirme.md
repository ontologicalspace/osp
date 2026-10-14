# Plan-Bound Task Lifecycle — değerlendirme notu (2026-10-07)

Değerlendirilen: `docs/notes/planlama-tasarım-eskiz.txt` (üç katmanlı konuşma derlemesi:
öneri → "plan tek uzayda yaşamaz" revizyonu → genel değerlendirme). Soru: OSP için
artısı/eksisi ne olur, benimsenmeli mi?

## Genel hüküm

**Doğru fikir, OSP'ye ontolojik olarak çok uygun — ama bugünün kuyruğunda tam
boyutuyla yeri değil.** Defter/ledger ritüelinin plan katmanına uygulanmış hâli;
OSP'nin imza hamlesi olan "mevcut pratiği typed, ölçülebilir protokole çevir"
işleminin bir tekrarı. Gücü de riski de buradan geliyor.

## Artılar (OSP'ye özgü, kanıtlı)

1. **Zaten olanı formalize ediyor.** #191 tur-2 P1 ("clippy önerisi ontolojik
   olarak aynı işlem değil") fiilen `ReplanRequired`; #190 tur-1 P2 (doküman
   daraltması) fiilen `RequestRework`. Review'ler bu ayrımı bugün yapıyor;
   model görünür kılıyor. Yeni süreç icat yok.
2. **Paper 3 makinesini yeniden kullanıyor = generality kanıtı.**
   `PlanDecisionStatus`'un kabul/supersession ile izomorf olması tembellik değil;
   Meta-RQ ("ikinci karar-nesnesi kind'ı modifikasyonsuz absorbe edilebiliyor
   mu?") implementasyon işini kanıt üretimine çeviriyor.
3. **Embriyo kod tabanında var.** #171 pre-call `freeze_and_claim` (izin →
   claim → outcome ⇒ anchor) dar kaynaklı `ExecutionPermit`; #172 K5 op-matrix
   plan binding çekirdeğinin önemli kısmı. Aynı şekil iki yerde bağımsız doğmuş.
4. **Ölçüm-disiplini + "koordinat yok" restrayıntı.** RQ'ların süreçten önce
   tasarlanması #171 pre-registration kültürüyle aynı; `PlanVector` reddi D8
   emsali. "Reviewed maneuver envelope for agents" hikâyesi güncel ve OSP'nin
   araştırma kimliğini güçlendirir.

## Eksiler / riskler

1. **Fırsat maliyeti (asıl itiraz).** Darboğaz kadans (≥3 gerçek task/hafta,
   1/3) ve Paper-4 canlı-kullanım verisi. Süreç altyapısı aynı dikkat bütçesinden
   yer: INV-T9 ×12, #171 dual-run, Paper-4 ile rekabet. "Kötü fikir" değil,
   "yanlış sıradaki iyi fikir".
2. **n=1 reviewer.** Bağımsız-reviewer değeri zayıf tanımlı; dürüst kapsam:
   model OSP'de asıl olarak "insan, agent'ın yaklaşımını kod öncesi denetler"
   kapısı. RQ yorumlarında bu sınır açık yazılmalı.
3. **Op-matrix örtüşmesi (en somut açık — doküman address etmiyor).**
   `PlanBinding.allowed_operations` ile mevcut task op-matrix'i (#172 K5) büyük
   ölçüde aynı yüzey. Doğru tasarım paralel mekanizma DEĞİL, mevcut yüzeyin
   genelleştirilmesi. Bu karar verilmeden Aşama 2'ye geçilmez.
4. **Ledger parçalanması.** live-ledger-v1 + llm-author.jsonl + plan event
   ledger = üçüncü stratum. Karar gerek (tercih: bilinçli ayrı stratum +
   task_id çapraz-bağ).
5. **Sürtünme bütçesi + veri birikme hızı.** Mevcut ritüelin sürtünmesi belgeli
   (run-17: 7 sürtünme); kesintisiz ölçüm için düşük riskli tasklar da loglamak
   zorunda (sabit maliyet). Haftada ~3 task'la RQ sinyali aylar alır → ambient
   benimseme zorunlu.
6. **Paper-4 kanıt bütçesi rekabeti.** Plan verisi Paper-4'ü bekletmemeli;
   açıkça "Paper 5 / sonrası bölüm" olarak ayrıştırılmalı.

## Benimseme önerim (belirtilmişse, bu şekilde)

- Sıra: #171 dual-run önce; plan lifecycle arkasında, Rust'sız kesitle.
- İlk kesit: yalnız yüksek riskli tasklar (persistence/schema/identity/public
  API). Dogfood malzemesi bugün var: #192, #193 bu sınıfın gerçek taskları.
- **Şemayı dondur, RQ'ları değil**: event schema + digest disiplini donar
  (additive backward-compat); RQ'lar pre-registration + amendment kaydıyla
  revize edilebilir (#171 disiplini).
- CompletionClaim şablonu PR gövdesine (mevcut pratikle marjinal maliyet ~0).
- Aşama 2 kararı veriyle: 3-5 yüksek riskli task sonrası op-matrix kararı +
  lowering v1 kapsamı.

## Karar (kullanıcı, 2026-10-07)

**ERKEN.** "Paper 4 ve planlama-tasarım-eskiz için henüz erken — önce çalışma
verilerini görelim." Öncelik: #171 dual-run + kadans verisi. Bu not
yeniden-değerlendirme çağrısında başlangıç noktasıdır; eskiz dosyası
(`planlama-tasarım-eskiz.txt`) referans malzemesi olarak bekler.

## Yeniden değerlendirme (2026-10-12, run-26 sonrası — karar-kaydı)

"ERKEN" kararını güncelleyen kanıt seti ve önerilen Rust'sız ilk kesit **#206**'da
donduruldu (kullanıcı onayıyla açıldı): run-26 PR #655'in 5 review turundaki
bulguların ~%100'ü plan-enumerable tasarım-niyeti eksikliği sınıfında; INV-T10
D_G makineleri yapısal niyeti tüketmeye hazır; #202 sinerjisi (config-run hafif
şeridi). Bu notun "Artılar/Eksiler" bölümü aynen geçerli; benimseme şekli #206
gövdesine taşındı. Öncelik paketinde sıra: #203 (gate×resume) → #204 (spec
re-anchor) → #205 (witness QoL) → #206 (bu pilot).
