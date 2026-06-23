# OSP — Implementation Invariants

> Formalizm büyüdükçe implementation kararları kaybolabilir. Bu dosya **yük-taşıyan**
> kararları kilitler — her invariant bir specific drift/bug'a karşı korur.
>
> İhlal eden PR reddedilir. İstisna için: invariant'ı değiştir + bu dosyayı güncelle +
> `OSP-formalism.md`'i güncelle + nedenini (rationale) yaz.
>
> Kaynak: `docs/OSP-formalism.md` (matematiksel gerekçe) + reviewer notları (matematiksel
> incelemeler) + `docs/agent-prompt-semantics.md` (Faz 5 Agent/LLM semantiği).
> Sürüm: 1.2 · 15 invariant (1.0: #1-#10, 1.1: #11-#14 Faz 5 eklentisi, 1.2: #15 custom axis God Mode).

---

## İnvariant'lar

### #1 — Author kendi claim'ine witness olamaz (self-merge prevention)

`author.id ∉ Ω.approvers`. Author'ın kendi iddiasına weight'i **0**'dır.

**Neden:** Lemma 1 (`OSP-formalism.md §7.3`) alt sınırının taşıyıcısı — bu aksiyom olmazsa
`n=2` ile safety vektörü kapanamaz, BFT reduction çöker. Ayrıca ontolojik olarak saçma:
"miş'li zaman"dan çıkış için self-witness yeter diyemeyiz.

**Nerede:** `osp-core::witness::WitnessSet::canonicalize_for(author)` — `CanonicalWitnessSet`
döner; author yapısal dışta. `evaluate()` ilk satırda çağırır.
**Test:** `witness_author_excluded_from_approvers` (`canonicalize_for` sonrası author yok).

---

### #2 — EvidenceEvent dedup (same source/actor/claim → strongest only)

Aynı `(source, actor, claim)` üçlüsü için yalnızca **en yüksek weight**'li evidence sayılır.

**Neden:** Aynı PR'ın hem `MergeCommit` (1.0) hem `PRMerged` (0.8) olarak sayılması
double-counting (Lemma 2(b)'deki hatanın yapısal versiyonu). Dedup olmadan metrik şişer.

**Nerede:** `osp-core::witness::WitnessSet::canonicalize_for()` — `(source, actor, claim)`
HashMap key'e göre max-weight tutar; çıktı `CanonicalWitnessSet` (dedup garanti).
`CanonicalWitnessSet::support()` doğrudan güvenli (raw events değil).
**Test:** `dedup_keeps_strongest_drops_weaker`.

---

### #3 — Tri-state witness: {Witnessed, Unwitnessed, Unobservable-locally}

`WitnessStatus ∈ {Witnessed, Unwitnessed, Unobservable-locally}`. "Görünmüyor" ≠ "yok".

**Neden:** Lokal God-mode squash/rebase + trailersız workflow'da review'yi göremez. Faz 0'da
fastapi/django "foam" sanıldı — doğru etiket `Unobservable-locally`'idi. Epistemolojik
dürüstlük: iddia şahitleri gözlemlenemiyorsa yalan değil, kanıt-bekleyen'dir.

**Nerede:** `WitnessProfile.witness_status: WitnessStatus` enum. Raporlar `Unwitnessed` ile
`Unobservable`'ı net ayırt eder; kullanıcıya "provider API bağla → confidence artır" önerisi.
**Test:** `squash_no_trailer_yields_unobservable_not_unwitnessed`.

---

### #4 — `u` derived'dır, θ hesabına girdi olamaz (RawPosition/DerivedPosition)

`Position { raw: RawPosition, derived: DerivedPosition }`. `θ` SADECE `raw`'ı okur.
`compute_theta(full_P)` **derleme hatası** olmalı (tip ayrımı).

**Neden:** `u = 1 − θ` ve `θ = deviation(P, V_vision)`. Eğer `u ∈ P` ise dairesel.
Tip ayrımı daireselliği yapısal garanti eder — runtime bug değil, compile-time koruma.

**Nerede:** `osp-core::coords::{RawPosition, DerivedPosition, Position}` ayrı tipler.
`DeviationMetric::theta(raw: &RawPosition, vision: &RawVision)` — `full Position` argüman olamaz.
**Test:** `theta_signature_rejects_full_position` (compile-fail test, `trybuild`).

---

### #5 — DiffusionDeviation commit path'inde çalışmaz (lazy)

`DiffusionDeviation` `commit()` akışında çağrılamaz. Sadece: `osp analyze`, PR-open, dashboard.

**Neden:** `K_t = e^{−tL}` full hesap `O(n³)` — her commit'te çağrılırsa God-mode real-time
olamaz. Commit akışında naif `CosineDeviation` (`O(n)`); diffusion lazy/periodik.

**Nerede:** `osp-core::engine::commit()` commit pipeline'ı sadece `CosineDeviation` kullanır
(Q5 claim-based gate). `bigbang::apply_delta()` mutasyon fazı diffusion çağırmaz.
`DiffusionDeviation` yalnızca `osp-core::engine::full_reposition()` içinde (analyze/dashboard).
**Test:** `commit_uses_cosine_not_diffusion` (diffusion çağrı sayacı = 0).

---

### #6 — Big Bang incremental update default (ΔV ∪ N₁(ΔV))

`commit()` sonrası position recompute sadece yeni düğümler + 1-hop komşuları. Tam recompute lazy.

**Neden:** `O(|V|)` recompute her commit'te büyük repolarda öldürücü. `O(|ΔV|·⟨deg⟩)` incremental
+ lazy full-recompute üretimi mümkün kılar. Faz 2'de k-hop threshold tune edilebilir.

**Nerede:** `SpaceEngine::commit()` pipeline'ı: Q4-Q6 claim-based gate → Q1-Q3 witness
(`evaluate`) → `bigbang::apply_delta()` mutasyon → reposition (sadece `ΔV ∪ N₁(ΔV)`).
Tam recompute `osp-core::engine::full_reposition()` (lazy entry point, analyze/dashboard).
**Test:** `commit_updates_only_delta_and_neighbors`.

**k=2 vs N₁(ΔV) ayrımı (Faz 5 netleştirme — `agent-prompt-semantics.md §3`):** Slice engine,
Agent'a **projeksiyon** (alt-graf görüntüsü) için k=2 hop kullanır — Agent'ın anlaması gereken
bağlam. Big Bang **mutasyon** sonrası reposition için sadece `N₁(ΔV)` = 1-hop kullanır. Farklı
operasyonlar: projeksiyon okuma (k=2), mutasyon yazma (1-hop). 2-hop etkiler bir sonraki
commit'lerde ortaya çıkar (incremental doğanın sonucu). Bu ayrım kasıtlıdır — "neden bazen
1-hop bazen 2-hop?" sorusunun cevabı: ilki reposition maliyeti, ikincisi Agent bağlamı.

---

### #7 — Admin override safety'i zayıflatır, raporda açıkça işaretlenir

`admin_override` kullanıldığında çıktıya `safety_weakened: true` + reason eklenir. **Asla sessiz.**

**Neden:** Lemma 2b (liveness) admin override'ı `Hold` çözmek için opsiyon olarak sayar AMA
Safety'i zayıflatır (tek-witness accept). Sessiz kullanım, OSP'nin BFT güvenilirliğini gizlice
aşındırır. Şeffaflık zorunlu.

**Nerede:** `WitnessResult::Commit { safety_weakened: bool, override_reason: Option<String> }`.
Rapor/JSON mutlaka gösterir.
**Test:** `admin_override_sets_safety_weakened_flag`.

---

### #8 — Provider API (GitHub/GitLab) sadece confidence artırır, core dependency değil

`osp-core` asla `gh`, `reqwest`, provider crate'lerine bağımlı olmaz. Provider = opsiyonel
zenginleştirme katmanı (enrichment crate'leri).

**Neden:** God-mode felsefesi (`SoftwarePhysics.txt §181`) internet-bağımsızlığını gerektirir.
Core provider'a bağımlı olursa offline çalışamaz, lokal egemenlik bozulur. Provider API yalnızca
`Unobservable-locally` → `Witnessed/Unwitnessed`'e terfi ettirir.

**Nerede:** `osp-core` Cargo.toml — `gh`/`reqwest` YOK. Ayrı `osp-provider-github` crate
(enrichment), `osp-core`'a `EvidenceProvider` trait'i üzerinden enjekte edilir.
**Test:** `osp_core_has_no_network_deps` (Cargo.toml parse, network crate reddi).

---

### #9 — W operatörü WitnessSet tabanlı (fixed-arity değil)

`W(C, Ω)` — `Ω: WitnessSet`. `min_approvers` (default 2) + `θ_quorum` (default 1.5) birlikte.

**Neden:** 2-güçlü, 3-zayıf, maintainer+CI-bot gibi kombinasyonlar fixed-arity `(C, W₁, W₂)`
imzasıyla desteklenemez. `WitnessSet` gelecekteki quorum çeşitlerini doğal kapsar.

**Nerede:** `osp-core::witness::evaluate(claim: &Claim, omega: &WitnessSet) -> WitnessResult`.
`min_approvers` ve `θ_quorum` `WitnessSet` config'inden okunur.
**Test:** `w_accepts_three_weak_witnesses` + `w_rejects_single_strong_witness`.

---

### #10 — `z` = saf Martin Instability, `D` ayrı derived metric

`z = I = Ce/(Ca+Ce)` (saf). `D = |A + I − 1|` ayrı derived metric (`P_derived`). `z ≠ I × (1−D)`.

**Neden:** `I × (1−D)` çarpımı bilgi kaybeder — I ve D ayrı kurtarılamaz. "Zone of Pain"
(concrete + rigid) vs "Zone of Uselessness" (abstract + unstable) ayırımı için ikisi de lazım.
Saf `z=I` + ayrı `D` her diagnoses'i mümkün kılar.

**Neden (önceki karardan dönüş):** Q7'de `z = I × (1−D)` seçilmişti; reviewer notu (arkadaş
incelemesi) bilgi kaybı + naming muğlaklığına işaret etti. Saf `z=I`'ye dönüldü.

**Nerede:** `osp-core::axes::InstabilityAxis` → `compute = I` (saf). `D` ayrı:
`osp-core::coords::DerivedPosition.main_sequence_distance`. θ'ya bileşen: `θ_eff = θ × (1 + α·D)`.
**Test:** `instability_axis_returns_pure_I` + `D_is_separate_derived_field`.

---

### #11 — LLM durumsuzdur, durum Agent kabuğundadır

LLM bir **stokastik tahmin motorudur** — her çağrıda bağımsız, belleksiz. Konuşma durumu,
ajans, bellek ve protokol uyumu **Agent kabuğunda** (subject shell) yaşar. LLM'in kendisi
uzayın ya da zamanın bir parçası değildir.

**Neden:** God-mode felsefesi (`SoftwarePhysics.txt §181`): LLM bir motordur, uzayın kendisi
değil. Durum Agent kabuğunda olunca, OSP'nin ontolojik primitifleri (Intent/Claim/Witness)
güvenilir kod yolunda kalır; LLM çıktısı yalnızca `Belief` (aday) üretir, `Knowledge` (gerçeklik)
asla. LLM yeniden başlatılsa, Agent kabuğu durumu korur. Bu, "machine epistemology"nın
temel ayrımıdır: stokastik üretim ≠ epistemik commit.

**Nerede:** Agent kabuğu (`osp-agent` crate, Faz 5) LLM'e stateless call yapar; LLM'den
dönen ham çıktıyı `OutputContract`'a göre deserialize eder ve `Belief` oluşturur. LLM crate'i
(`osp-llm-runtime`) `&self` state taşımaz — saf fonksiyon: `(OspPrompt, seed) → DeltaProposal_raw`.
**Test:** `llm_runtime_is_stateless` (LLM runtime struct'ında `&mut self` yok, config dışında
field yok) + `agent_shell_preserves_state_across_llm_restart` (LLM restart sonrası Agent
kabuğu aynı Intent'i takip eder).

---

### #12 — OutputContract'e uymayan çıktı = otomatik reject (Q4 deterministik)

LLM'den gelen `DeltaProposal`, `OutputContract` şemasına uymuyorsa **deterministik olarak**
reddedilir (Q4 Syntax Gate). Şahitlere (`WitnessSet`) **gösterilmez** — sentaks hatası
epistemik bir mesele değildir, teknik hatadır.

**Neden:** Witness'lar semantic/mavi-kol kararlar verir; sentaks hatası onların uzmanlığı
değildir. Şahitlere hatalı-formatlı çıktı göstermek (a) onların zamanını israf eder, (b)
sentaks hatasını "yetersiz şahitlik" (`Hold`) ile karıştırır. Q4 deterministik reject bu
ikisini ayırır: `StructuralHallucination` (sentaks) vs `UndersupportedClaim` (şahit eksik).
`agent-prompt-semantics.md §4.1` hallucination sınıflandırması bu ayrıma dayanır.

**Nerede:** `osp-core::engine::check_claim_syntax()` (Q4) — `commit()` pipeline'ının ilk
gate'i. `DeltaProposal::is_well_formed()` validation: `new_nodes`/`new_edges`/`modified_entities`
tipleri geçerli, bağlı `NodeId`'ler mevcut, `EdgeKind`'ler legal. Failure → `Err(SyntaxViolation)`
(pre-mutation, witness öncesi). `space-engine-design.md §4.3`.
**Test:** `q4_syntax_violation_rejects_before_witness` (hatalı-formatlı Claim → `WitnessSet`
hiç çağrılmadan `Err(SyntaxViolation)`).

---

### #13 — PermissionMask God Mode tarafından atanır, Agent değiştiremez

`PermissionMask` (Agent'ın okuma/yazma yetkileri) **God Mode** (insan-operatör veya
hardcoded bootstrap config) tarafından atanır. Agent kabuğu ve LLM kendi yetkilerini
**genişletemez, değiştiremez**. Sadece God Mode CLI (`osp permission grant/revoke`) ile
güncellenir.

**Neden:** İnsan egemenliği (God-mode felsefesinin temeli): Agent'a "yalnızca X modülünü
değiştir" denildiyse, Agent kendine "aslında Y'yi de değiştireyim" diyemez. Bu, LLM
hallucination/instruction-injection saldırılarına karşı son savunma hattıdır — saldırgan
prompt LLM'i ikna etse bile, `commit()` PermissionMask'i zorla uygular.

**Nerede:** Üç-nokta denetim (`agent-prompt-semantics.md §2.1`):
1. `compute_space_slice()` — okuma izni olmayan düğümleri projeksiyondan çıkarır.
2. Agent kabuğu — yazma izni olmayan mutasyonları erken reddeder (token tasarrufu).
3. `osp-core::engine::check_permissions()` — `commit()` öncesi **nihai zorunlu** kontrol.
   Atlanamaz; güvenilir (trusted) kod yolu. Failure → `Err(PermissionDenied)`.

PermissionMask kaynağı: God Mode config (`osp-permissions.toml`), runtime'da Agent'a parametre
olarak verilir. `osp-core::PermissionMask` immutable (Agent'a verildikten sonra).
**Test:** `permission_mask_cannot_be_self_expanded` (Agent kabuğu `mask.writable_axes`'e
yeni axis ekleyemez — immutable ref) + `commit_rejects_unauthorized_mutation` (mask dışı
node'a yazma → `Err(PermissionDenied)` nihai gate'de).

---

### #14 — Prompt doğal dil değil, tiplenmiş veri paketidir

`OspPrompt` bir doğal dil yönlendirme metni değil, `SpaceEngine` tarafından üretilen
**tiplenmiş veri paketidir** (`π_A` — epistemik projeksiyon). LLM'e serialize edilmiş yapısal
veri olarak gönderilir; LLM çıktısı da `OutputContract`'a göre deserialize edilir.

**Neden:** OSP'nin temel yeniliği (literatür taraması §7): RAG/GraphRAG "daha çok metin"
göndererek çözmeye çalışır; OSP "daha az veri (koordinat + topoloji)" göndererek çözer —
ontolojik sıkıştırma. Doğal dil prompt (a) belirsizdir (ambiguity), (b) LLM'e serbestlik
verir (hallucination vektörü), (c) deterministic parse edilemez. Tiplenmiş paket + tipli
çıktı kontratı: OSP'nin "God-mode filter" mekanizmasının (LLM çıktısı uzay-dışına düşerse
reddet) çalışması için zorunlu. Bu, OSP'yi bir "iletişim protokolü" yapar (HTTP benzeri),
prompt mühendisliği değil.

**Nerede:** `osp-core::prompt::OspPrompt` (veya `osp-agent::prompt`) struct'ı — §2
`agent-prompt-semantics.md`. LLM'e gönderilen mesaj: `serialize(OspPrompt) + system_instruction
("Bu bir OSP projeksiyon paketidir. OutputContract'a uygun DeltaProposal üret.")`. LLM çıktısı
`DeltaProposal`'a deserialize edilir; başarısız → Q4 reject (inv #12).
**Test:** `prompt_is_typed_not_natural_language` (`OspPrompt` serialize edilmiş çıktısı
JSON/bincode şemasına uyuyor; serbest metin alanı yok) + `llm_output_must_match_output_contract`
(şema-dışı çıktı → Q4 SyntaxViolation).

---

### #15 — Custom axis'leri sadece God Mode register eder, Agent tanımlayamaz

`CoordinateSystem.custom: Vec<Box<dyn Axis>>` — custom raw axis'ler (security, accessibility,
domain-specific) **sadece God Mode** tarafından `register_custom_axis()` API'si ile eklenir.
Agent kabuğu ve LLM yeni axis **tanımlayamaz**, mevcut axis'in `compute()` fonksiyonunu
**değiştiremez**, axis listesini **mutate edemez**.

**Neden:** Q5 Vision Gate θ'yi tüm raw eksenler (core + custom) üzerinden hesaplar. Eğer
LLM "fake_security" axis inject ederse (her zaman 1.0 döner), θ yapay olarak düşürülür ve
gizlice kötü niyetli claim'ler Q5'i geçer. Bu, inv #13 (PermissionMask God Mode) ile aynı
güven modeli: uzayın fizik kuralları insan-kontrolündedir, AI'ın değil. OSP'nin "software
physics engine" kimliği için kritik — fizik kuralları gözlemci tarafından değiştirilemez.

**Nerede:** `osp-core::coords::CoordinateSystem::register_custom_axis(axis)` — yalnızca
God Mode CLI (`osp axis register <pkg>`) veya bootstrap config çağırır. Agent kabuğu bu
API'ye erişemez (capability-based security: God Mode `RegisterAxisCapability` token'ı
vermedikçe). `osp-core::agent` crate'inin `register_custom_axis` çağrısı compile-time'da
yok (bağımlılık yok). Formalizm §2.2 "Custom Axis Extensibility".
**Test:** `agent_cannot_register_custom_axis` (Agent kabuğu module'ünde `register_custom_axis`
referansı yok, compile-time garanti) + `custom_axis_requires_god_mode_capability`
(runtime'da capability token olmadan `register_custom_axis` → panic/reject).

---

## İhlal Prosedürü

1. PR'da invariant ihlali → reviewer reddeder, bu dosyaya atıfla gerekçe ister.
2. İstisna gerekiyorsa: (a) invariant'ı değiştir, (b) bu dosyayı güncelle, (c) `OSP-formalism.md`
   ilgili bölümü güncelle, (d) commit mesajında rationale yaz.
3. İnvariant'lar birer "yapısal savunma"dır — her biri specific bir hataya karşı. Kaldırmak
   o hatayı geri getirir.

---

## Cross-References

| İnvariant | Formalizm bölümü | Reviewer kökeni |
|---|---|---|
| #1 author-witness | §7.3 Lemma 1 | kullanıcı (matematiksel inceleme #2) |
| #2 dedup | §4.4.1 EvidenceEvent | kullanıcı (Lemma 2b) + arkadaş |
| #3 tri-state | §4.5 | arkadaş |
| #4 u derived | §2.1 | arkadaş |
| #5 lazy diffusion | §5.3 | kullanıcı |
| #6 incremental Big Bang | §6 | kullanıcı |
| #7 admin override flag | §7.4 Corollary 3 | arkadaş |
| #8 provider optional | §4.4.3 | kullanıcı + arkadaş |
| #9 WitnessSet W | §4.3 | arkadaş |
| #10 z=I pure | §2 KARAR | arkadaş (Q7 revisit) |
| #11 LLM durumsuz | agent-prompt-semantics.md §1, §4 | Faz 5 Agent semantiği |
| #12 OutputContract deterministic reject | agent-prompt-semantics.md §2.2, §4 Q4 | Faz 5 Q4-Q6 split |
| #13 PermissionMask God Mode | agent-prompt-semantics.md §2.1 (üç-nokta denetim) | Faz 5 insan egemenliği |
| #14 Prompt tiplenmiş paket | agent-prompt-semantics.md §2 (OspPrompt) | Faz 5 OSP codec |
| #15 Custom axis God Mode only | OSP-formalism.md §2.2, agent-prompt-semantics.md §2 | Custom axis marketplace |

---

*Sürüm: 1.2 · 15 invariant · `docs/OSP-formalism.md` (5 core + N custom + derived) + `docs/agent-prompt-semantics.md` 1.0-final ile senkron*
