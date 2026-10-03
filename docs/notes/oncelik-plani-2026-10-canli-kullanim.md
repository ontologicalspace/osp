# Öncelik Planı — canlı kullanım öncelikli (2026-10-04)

Karar: canlı kullanım öne alınır (2026-09-30 standing decision record ile tutarlı: **live use before arXiv**); **retro TAM yapılır** (atlanmaz — enstrüman nesil ayrımını kapatan tek iş); epistemik enstrüman işleri (#169/#170) **talep-güdümlü** çağrılır; yazı-son ilkesi aynen.

## Dalga 1 — Zemin: retro + operasyonel küçükler (~2 oturum)

### 1a. Retro run 9-16 (TAM; yeni binary `d9e214a`)
- **Envanter:** her run dizinindeki `baseline.json`/`after.json` → `repository.head` SHA'ları; worktree'ler MUTLAK yolla (`git -C ... worktree add --detach P:/Work/SoftwarePhysics/Temp/nexus-rN-... <sha>`; NEXUS'TA Temp/ YOK).
- **Ölçüm:** yeni binary ile her iki commit'te analyze → (x, x_type) çifti + same-ns maskelenme oranı + tip-gren mahalle (donmuş enstrüman: `git show d9e214a:scripts/neighborhood_accounting.py` → v1.3; sembol anahtarları metadata tarzı `Outer+Inner` / ``Box`1``).
- **Altın standart:** run 14-16 el HER-SATIR/HER-TİP tablolarıyla makine eşleşmesi (run-16 birebirliği tur-4 binary'sine aitti — yeniden doğrulanır; S2a≅S2b ayrışması ve run-15 pencere-içi durumu artık AYIRT EDİLEBİLMELİ).
- **Sapma sınıflandırması:** nested/arity kenarlarının görünür olması; üye-konumu yanlış-düz kalkmaları. Nexus geneli referans (tur-4→tur-5): type_imports 6657→6574, %98.7 düz anahtar kararlı, 159 nested + 31 arity kenar, 7 çift ayrıştı, −93 üye-konumu.
- **Çıktı:** `docs/notes/` retro notu + per-run tablo. **Tüketenler:** Paper-4 "coupling granularity sensitivity" ekseni; karar robustluğu (pre-registered kararlar tip-grende tutuyor mu?); #169'in ampirik spesifikasyonu.
- **Yorumlama kapısı:** bu retro bitmeden canlı run'lardaki x_type/mahalle sürprileri "nesil ayrımı mı gerçek değişim mi" ayrıştırılamaz — retro atlanamazın gerekçesi bu.

### 1b. #173 fixture-vacuity (bug, ~çeyrek oturum)
`completed_loop` mod-fixture'ı güncel adaptörde kenar üretmiyor → coupling testleri vacuous. #166'nın `HarnessFixture::new_with_use_edges()` ile gerçek kenarlı fixture; test-güvenilirliği zemini (#172 doğrulamaları buna yaslanır).

### 1c. #172 defter-absorbe (~1 oturum, tek PR)
`draft-task` / `suggest-targets` / `finalize-run` — temsilci doğrulama + SHA + digest motorun işi; yorum insanda kalır. Run ritüeli ~20 dk ucuzlar; SHA-yazım/tırnak hatası sınıfları ölür (#166 artifact'ını tüketir). **Canlı kadansın bir numaralı bariyeri ritüel maliyeti — bu yüzden Dalga 1'de.**

## Dalga 2 — Canlı kullanım kadansı (Faz 1 proper; süreklilik)

- **Kural:** gerçek Nexus task'ları OSP döngüsünden geçer (analyze → task → proposals → attempt → üç-karar → promote); hedef seçimi **gerçek işten** (mutasyon-yüzlü havuz kısıtı yok — run-16 değerlendirmesi havuzun inceldiğini söylüyordu; gerçek iş belirler).
- **Kadans hedefi:** haftada ≥3 gerçek task (Faz 1 hedefi).
- **Ölçüm disiplini (olgunluk anekdot kalmasın):** (i) sürtünme günlüğü — her canlı döngüde ne zorladı; (ii) ritüel süresi trendi; (iii) iki eşik olayına atış sayısı (Event-1: OSP beklenmedik gerekçeyle ezer; Event-2: yakalanır — 9 run'dır sıfır; daha çok gerçek kullanım = daha çok şans).
- **Ritüel korunur, ucuzlatılır:** her canlı döngü ledger + neighborhood stratumuna yazılır; τ yalnız el tahminlerinden; aday şekiller bar'dan önce. (Ritüel atlamak olgunluk değil ölçüm kaybıdır.)
- **#171 yazar-ayrımı deneyi — canlı döngülerden BİRİ olarak:** LLM-önerili gerçek task (insan-kol / LLM-kol; `--llm real`). Tek-yazar daireselliği eleştirisinin ilk verisi; Paper-4'ün en çok ihtiyaç duyduğu kolon.

## Dalga 3 — Talep-güdümlü enstrüman (çağrıldığında)

- **#169 tahmin kanalı:** el tahmini canlı run'da acıttığında (retro tabloları spesifikasyon olarak hazır — retro'suz tasarlamak #167 öncesi hatasıydı; retro Dalga 1'de tamamlandı).
- **#170 verify-patch:** promote'a yakın bir kaçış yaşandığında (öneri-öngörülen using-set vs aday worktree gerçek yüzeyi; LLM-önerili koşularda değer zirvesi).
- Tetikleyici kullanım olayları ledger/notes'ta kayda geçer — "ne zaman çağrıldı ve ne tetikledi" planın parçası.

## Dalga 4 — Yazım & kalan

- **Paper-4 sentez:** malzeme = retro tabloları (granularity ekseni + robustluk) + canlı kayıt (sürtünme/süre/olay) + #171 yazar-ayrımı kolonu. Yazı-son.
- **INV-T9 seçmeli:** #95 / #89 / #86 üçlüsü önde (MCP workspace / task-snapshot drift / MD-1); kalanı (#70–79) Papers 1–3 kanıt tamlığı dalgasında. Paper-4'ü bloklamaz.
- **#110 MSRV:** herhangi bir küçükler oturumuna katılır (metadata dürüstlüğü).
- **MCP / P5:** Faz 4'te kalır (decision record aynen — OSP sorunları ile entegrasyon/UX sorunlarını karıştırmama).

## Açık issue yerleşimi (18)

| Issue | Dalga |
|---|---|
| #173 (bug, fixture-vacuity) | 1b |
| #172 (defter-absorbe) | 1c |
| #171 (LLM-yazar deneyi) | 2 (canlı döngü olarak) |
| #169 (tahmin kanalı) | 3 (talep-güdümlü; spec hazır) |
| #170 (verify-patch) | 3 (talep-güdümlü) |
| #95 / #89 / #86 (INV-T9 üçlü) | 4 |
| #70–#79 (INV-T9 çekirdek ×9) | 4 (Papers 1–3 dalgaı) |
| #110 (MSRV) | 4 (küçükler oturumu) |

## İlk somut adım
Retro envanteri: run 9-16 dizinlerinden baseline/after SHA tablosu + worktree planı → yeni binary koşuları.
