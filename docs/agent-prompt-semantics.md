# OSP Protocol Design Note: Agent Semantics & Epistemic Projection

> Bu doküman Faz 5 (LLM OSP Codec) tasarımının temel sözleşmesidir. `OSP-formalism.md`
> §4 (Witness) ve §5 (Vision)之上 inşa edilir; `space-engine-design.md` §4 (Commit Pipeline)
> ile entegre çalışır.
>
> **Öncelik:** bu doküman > `OSP-formalism.md §5` (vision/θ kavramları) > yorumsel kararlar.
> Çelişki olursa bu doküman bağlayıcıdır (Faz 5 kapsamındaki Agent/LLM/Prompt konularında).
>
> **Status:** Approved / Architecture Spec
> **Version:** 1.0-final · 2026-06-21

---

## 0. Üç-Katmanlı Ontolojik Harita

OSP'nin zaman modeli, her katmana bir birincil ontolojik kategori atayarak epistemolojik
temizlik sağlar (Gerilim 1 çözümü — `OSP-formalism.md §1.2` ile senkron):

| Katman | Sembol | Birincil Kategori | Epistemik Statü | OSP Karşılığı |
|---|---|---|---|---|
| **Gelecek (potansiyel)** | `t_f` | **Intent** | Potansiyel — gradyan, hedef | issue, roadmap, "feature X istiyoruz" |
| **Miş'li (öznel)** | `t_m` | **Belief** (Claim Candidate) | Aday — henüz şahitlenmemiş | feature branch, açık PR, ΔS önerisi |
| **Şimdiki (nesnel)** | `t_c` | **Knowledge** (Commit) | Gerçekleşmiş — şahitlenmiş | main branch, production |

**Bilgi akışı:** `Intent (t_f) → projeksiyon (§2-3) → Agent (§1) → Belief (t_m) → [Witness W] → Knowledge (t_c)`

Intent uzayı doğrudan mutate etmez; t_f'den t_m'ye gradyan, `OspPrompt` projeksiyonu
üzerinden yayılır. Bu, "miş'li zaman" felsefesiyle uyumludur: gelecek şimdikiyi çeker
ama deterministik neden olmaz — Agent'ın t_m'de ajansı vardır.

**Terminoloji kararı:** `Claim` veri yapısı (struct) olarak korunur; `Belief`/`Knowledge`
aynı Claim'in epistemik durumlarını ifade eden alias'lardır. Bir Claim `t_m`'de iken
`Belief`, `t_c`'ye commit edildiğinde `Knowledge`'a terfi eder.

---

## 1. Ontolojik Terminoloji Sözlüğü

Sistemdeki kavramsal kaymaları önlemek amacıyla, LLM entegrasyonu ve Agent semantiği
aşağıdaki kesin tanımlarla yürütülür:

| Terim | Ontolojik Statüsü | Teknik Karşılığı |
| --- | --- | --- |
| **Intent** | Gelecek Zaman (`t_f`) potansiyeli | Uzayda mutasyon yapmayan, sadece Agent'ın arama yönünü etkileyen hedef gradyan alanı. |
| **OspPrompt** | Epistemik Projeksiyon Paketi (`π_A`) | Ana uzaydan (`S_c`) Agent'a açılan, izinleri ve kısıtları belirlenmiş yarı-geçirgen alt-graf kesiti. |
| **Agent** | Lokal Özne Kabuğu (Subject Shell) | Protokol kurallarına uyan, girdiyi alan ve kontrata uygun Claim üreten durum makinesi aktörü. |
| **LLM** | Stokastik Tahmin Motoru | Agent kabuğunun içinde çalışan, olasılık dağılımlarından örnekleme yapan **durumsuz (stateless)** motor (inv #11). |
| **Belief** | İddia Adayı (Claim Candidate) | Agent'ın kendi miş'li zamanında (`t_m`) ürettiği, henüz şahitlenmemiş `ΔS` önerisi. |
| **Knowledge** | Şahitlenmiş Gerçeklik (Commit) | Şahitlik (`W`) fonksiyonundan geçerek belirsizliği çökmüş ve `S_c` zaman çizgisine yazılmış bilgi. |
| **Hallucination** | Yolsuz İnanç (Non-realizable) | Lokal projeksiyonda (`S_{A_local}`) geçerli görünen ancak objektif uzayda (`S_c`) yürütülebilir bir patikası olmayan sapma (§4.1 sınıflandırması). |

---

## 2. Epistemik Projeksiyon Mekanizması (Prompt Yapısı)

Prompt, doğal dilde yazılmış bir yönlendirme metni **değildir** (inv #14); `SpaceEngine`
tarafından üretilen tiplenmiş bir veri paketidir:

```rust
struct OspPrompt {
    space_slice: SpaceSlice,          // Dinamik kütleçekim ile seçilmiş alt-graf (§3)
    intent: Intent,                   // t_f katmanındaki hedef gradyanı
    vision: VisionVector,             // Mimari rota ve Q5 Gate kriterleri (core + custom)
    axis_manifest: AxisManifest,      // Aktif custom axis listesi + calibration (§2.3, inv #15)
    time_ref: TimeLayer,              // Mevcut şimdiki zaman (t_c) referansı
    rules: Vec<Rule>,                 // İhlal edilemez sistem invariant'ları (Q6)
    permissions: PermissionMask,      // Mutasyona açık eksenlerin ve düğümlerin sınırları (§2.1)
    evidence_context: EvidenceSummary,// Hold/Reject durumlarından gelen geçmiş kanıtlar
    output_contract: OutputContract,  // Üretilmesi zorunlu olan DeltaProposal şeması (§2.2)
}
```

> **`axis_manifest`:** Core 5 axis sabittir (x,y,z,w,v); custom N axis proje-bazlı değişir.
> Manifest, Agent'a hangi custom axis'lerin aktif olduğunu ve θ hesabına nasıl girdiğini
> söyler. LLM custom axis tanımlayamaz — sadece God Mode register eder (inv #15).

### 2.1 PermissionMask

Agent'ın hangi düğümlerde ve hangi eksenlerde değişiklik yapabileceğini belirleyen yetki
matrisi. **God Mode** tarafından Intent'in hedef alanına ve Agent'ın rolüne göre atanır;
Agent kendi yetkilerini genişletemez (inv #13).

```rust
pub struct PermissionMask {
    /// Agent'ın değiştiremeyeceği, sadece okuyabileceği düğümler
    pub read_only_nodes: HashSet<NodeId>,
    /// Agent'ın yeni düğüm ekleyebileceği veya koordinat güncelleyebileceği eksenler
    pub writable_axes: HashSet<AxisId>,
    /// Agent'ın oluşturamayacağı kenar türleri (örn: Approves kenarları sadece Witness'lar içindir)
    pub forbidden_edge_kinds: HashSet<EdgeKind>,
    /// Agent'ın pozisyon güncelleyebileceği maksimum sapma (θ_max yetki sınırı)
    pub max_position_deviation: f64,
}
```

**Üç-nokta savunma derinliği (Defense in Depth — Gerilim 6 çözümü):**

PermissionMask üç ayrı noktada denetlenir; her nokta farklı bir güven katmanı sağlar:

| Denetim Noktası | Ne Yapar | Güven Katmanı |
|---|---|---|
| **1. `compute_space_slice()`** | Agent'ın okuma izni olmayan düğümleri projeksiyondan çıkarır | Bilgi sızdırmazlık — Agent görmemesi gerekeni görmez |
| **2. Agent kabuğu** | Üretilen `DeltaProposal`'da yazma izni olmayan düğümler/kenarlar varsa erken reddeder | Token tasarrufu — LLM çıktısı şahitlere gitmeden filtrelenir |
| **3. `SpaceEngine::commit()` (nihai)** | Claim'i ana uzaya uygulamadan önce PermissionMask'i zorunlu kontroller | Son savunma hattı — atlanamaz, güvenilir (trusted) kod yolu |

Nihai merci `SpaceEngine::commit()`'tir; ilk iki nokta optimizasyon ve bilgi gizliliği için,
üçüncüsü güvenlik için. Saldırgan Agent kabuğu 1. ve 2.'yi atlsa bile 3. engeller.

**Evidence/permission conflict (bilinen edge case):** `compute_space_slice()` denetim
noktası 1'de, witness'ların evidence için talep ettiği bir node'a Agent'ın read yetkisi
yoksa üç policy seçeneği var:
- **(a) Drop + warning log** (MVP default) — Agent evidence talebini göremez; engine loglar
- **(b) Redacted göster** — Agent node'un varlığını bilir, detayları görmez, God Mode'dan
  geçici yetki talep eder (Faz 5+)
- **(c) Projeksiyon reject** — engine "evidence/permission conflict" ile Agent'ı bekletir,
  God Mode çözene kadar Hold (Faz 5+)

MVP (a)'yı uygular; (b)/(c) Faz 5 policy kararları. Bu, §3 algoritmasındaki filter-then-drop
sırasının anlamlı olduğu yerdir.

### 2.2 OutputContract

LLM'den beklenen çıktı formatı (inv #12). Agent kabuğu, LLM çıktısını bu şemaya göre
deserialize eder; uymayan çıktılar Q4 Syntax Gate'inde **deterministik olarak** reddedilir:

```rust
pub struct DeltaProposal {
    /// Yeni eklenecek ontolojik düğümler
    pub new_nodes: Vec<NewNodeSpec>,
    /// Yeni eklenecek tiplenmiş kenarlar
    pub new_edges: Vec<NewEdgeSpec>,
    /// Mevcut düğümlerin entity özelliklerinde değişiklikler (kind, mass, metadata — POZİSYON DEĞİL)
    pub modified_entities: Vec<EntityChangeSpec>,
    /// LLM'in pozisyonla ilgili tavsiyeleri — ADVISORY ONLY, authoritative değil (aşağıda)
    pub position_hints: Vec<PositionHint>,
    /// LLM'in kararlarını açıklayan gerekçe (şahitler tarafından okunabilir)
    pub reasoning: String,
}

pub struct NewNodeSpec {
    pub kind: NodeKind,
    pub initial_mass: f64,
    pub connected_to: Vec<(NodeId, EdgeKind)>,
}

pub struct NewEdgeSpec {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

pub struct EntityChangeSpec {
    pub node_id: NodeId,
    pub changes: EntityChanges,  // kind/mass/metadata değişiklikleri — RawPosition HARIÇ
}

/// LLM'in "bu node şu pozisyonda olmalı" tavsiyesi — engine tarafından authoritative
/// kabul EDİLMEZ. Sadece Q5 sonrası diagnostic/comparison için kullanılır.
pub struct PositionHint {
    pub node_id: NodeId,
    pub suggested_raw: RawPosition,  // LLM'in iddia ettiği pozisyon
    pub rationale: String,            // neden bu pozisyonu öneriyor
}
```

**Kritik: Pozisyonlar LLM tarafından declare EDİLMEZ (P0 fix — epistemolojik bütünlük):**

`DeltaProposal` pozisyon **içermez** — sadece yapısal değişiklikler (node/edge/entity) içerir.
Pozisyonlar `SpaceEngine` tarafından **compute** edilir:

```
DeltaProposal (yapısal)
  → Claim (yapısal ΔS)
  → analyzer/coord_system node'ları yeniden ölçer (coupling/cohesion/instability/entropy/
    witness-depth + custom axis'ler)
  → RawPosition compute edilir (inv #4: computed, not declared)
  → Q5 Vision Gate: θ(computed_P_raw, V_vision) ≤ θ_bound?
```

**Neden:** Eğer LLM `Position` authoritatively set edebilirse, Q5 "LLM'in iddiası LLM'in
iddiasıyla uyuşuyor mu?" olur — **totolojik**. LLM "coupling=0.3" declare edip gerçek
coupling=0.8 olan kodu gizleyebilir. Bu, God Mode deterministik validity çizgisini deler
(connected to inv #4 + #12 + #14).

`position_hints` yalnızca advisory'dir — engine actual pozisyonu compute ettikten sonra
"LLM bunu önerdi, actual şuydu" diagnostic'i üretir. Agent kabuğu hint'leri kullanabilir
(kendi internal reasoning için) ama engine Q5 için **asla** hint'i kullanmaz.

**Node kind'a göre position computation:**
- **Module** node'lar: code analyzer'dan (tree-sitter/SCIP → coupling/cohesion/instability)
- **Concept/Feature/Rule** node'lar: ilişki/gravity-based (kenar ağırlıkları + rule contributions)
- **Custom axis değerleri**: axis-specific analyzer (örn. `security.audit` → CVE scanner)

Tüm durumda pozisyon **engine'de compute edilir**, LLM'de değil.

---

## 3. Üç Katmanlı Alt-Uzay Seçim Motoru (Space Slice Engine)

`GraphGravity` motorunun bir `Intent` düğümüne bakarak `space_slice` kapsamını (Agent'ın
vizyon alanını) otomatik olarak nasıl belirleyeceğinin matematiksel ve algoritmik modeli.

> **inv #6 ayrımı (Gerilim 3 çözümü):** Slice engine **projeksiyon** için k=2 hop kullanır
> (Agent'ın anlaması gereken bağlam). Big Bang **mutasyon** sonrası reposition için sadece
> `N₁(ΔV)` = 1-hop kullanır (inv #6 incremental). 2-hop etkiler bir sonraki commit'lerde
> ortaya çıkar. Farklı operasyonlar, farklı skoplar.

```
+--------------------------------------------------------+
| 3. PERMISSION & EVIDENCE (Yetki ve Kanıt Filtresi)     |
|    +----------------------------------------------+    |
|    | 2. VISION & RULES (Risk Tabanlı Genişleme)    |    |
|    |    +------------------------------------+    |    |
|    |    | 1. INTENT GRAVITY (Core Node + K-Hop)|   |    |
|    |    +------------------------------------+    |    |
|    +----------------------------------------------+    |
+--------------------------------------------------------+
```

### Katman 1: Intent-Driven Gravity (Çekirdek Çevre)

Intent düğümünün doğrudan hedef aldığı düğüm kümesi (`ΔV`) tespit edilir. Bu küme merkez
kabul edilerek graf üzerinde k-hop komşuluk taraması (varsayılan `k=2`) yapılır. Graf
topolojisindeki bağ dereceleri (coupling weights) kütleçekim skoru olarak hesaplanır.

### Katman 2: Vision & Rule-Based Risk Expansion (Güvenlik Duvarı)

Sistem invariant'larının (`Rules`) ihlal edilme riski olan kritik sınır hatları grafiğe
dahil edilir. Örneğin; "A modülü B modülüne doğrudan erişemez" kuralı varsa ve Intent A
modülünü değiştiriyorsa, B modülü ve aradaki soyutlama katmanları potansiyel ihlal alanı
olarak `space_slice` içerisine zorunlu olarak çekilir.

### Katman 3: Permission & Context Trimming (Yetki ve Budama)

`PermissionMask` filtrelemesi çalışır (§2.1 denetim noktası 1). Agent'ın okuma veya yazma
yetkisinin olmadığı korumalı uzay bölgeleri projeksiyondan temizlenir. Eğer süreç daha
önceki bir `Hold` durumunun tekrarıysa, `EvidenceContext` şahitlerin talep ettiği eksik
düğümleri pakete ekler.

### İmplementasyon Algoritması (Rust)

```rust
pub fn compute_space_slice(
    intent: &Intent,
    space: &Space,
    rules: &[Rule],
    mask: &PermissionMask,
    evidence: &EvidenceSummary
) -> SpaceSlice {
    let mut nodes_bucket = HashSet::new();

    // 1. Katman: Intent Çekirdeği ve K-Hop Komşuluk Grafiği
    let core_nodes = &intent.etki_alani.nodes;
    nodes_bucket.extend(core_nodes.clone());

    for core_node in core_nodes {
        let neighbors = space.get_neighbors_within_hops(core_node, 2);
        nodes_bucket.extend(neighbors);
    }

    // 2. Katman: Kütleçekim Alanı Hesaplama
    let gravity_scores = space.calculate_gravity_field(intent.target_coordinates);
    for (node, score) in gravity_scores {
        if score >= 0.60 { // Kütleçekim eşik değeri (Faz 5 kalibrasyon)
            nodes_bucket.insert(node);
        }
    }

    // 3. Katman: Kural İhlal Riski Barındıran Sınır Düğümleri (Risk Sensors)
    let dynamic_risk_nodes = space.detect_rule_boundary_nodes(core_nodes, rules);
    nodes_bucket.extend(dynamic_risk_nodes);

    // 4. Katman: Yetkilendirme ve Geçmiş Kanıt Budaması (PermissionMask denetim noktası 1)
    //
    // GÜVENLİK (P0 fix): evidence node'ları permission filtresinden GEÇEREK eklenmeli.
    // Önce retain() sonra extend() sırası, witness'ların talep ettiği node'ları permission
    // dışı bile olsa slice'a sızdırır — God Mode bilgi sızdırmazlık ilkesini deler.
    //
    // Policy: evidence node'u permission dışı ise → drop + warning log.
    // (MVP default; alternatif policy'ler için §2.1 "Evidence/permission conflict")
    nodes_bucket.extend(
        evidence.required_nodes_for_witnessing
            .iter()
            .copied()
            .filter(|node| mask.has_read_permission(node)),
    );
    nodes_bucket.retain(|node| mask.has_read_permission(node));

    SpaceSlice::build_subgraph(space, nodes_bucket)
}
```

---

## 4. Protokol Yaşam Döngüsü (Deterministik Pipeline)

1. **Intent Ekolojisi:** `t_f` katmanında bir `Intent` düğümü belirir ve uzayda kütleçekim
   gradyanı yaratır.
2. **Epistemik Projeksiyon (`π_A`):** `SpaceEngine`, `compute_space_slice` algoritmasını
   çalıştırarak `OspPrompt` paketini üretir.
3. **Lokal İnanç Üretimi:** Agent kabuğu, içindeki LLM motorunu koşturur. LLM'e serialize
   edilmiş `OspPrompt` ve sistem mesajı olarak *"Bu bir OSP projeksiyon paketidir.
   OutputContract'a uygun DeltaProposal üret."* talimatı gönderilir. Çıktı kontrata göre
   deserialize edilerek `t_m` katmanında bir `Belief` (Claim Candidate) oluşturulur.
4. **Claim-Based Gates (Q4-Q6 — deterministik, witness öncesi):**
   - **Q4 — Syntax Gate:** `DeltaProposal` geçerli bir şemaya sahip mi? `OutputContract`'a
     uyuyor mu? (inv #12)
   - **Q5 — Vision Gate:** `θ(P_raw(C), V_vision_raw) ≤ θ_bound`? (varsayılan `θ_bound = 0.25`,
     formalizm §5.2 ile uyumlu)
   - **Q6 — Rule Gate:** Önerilen yeni kenarlar/düğümler herhangi bir `Rule`'u ihlal ediyor mu?

   Herhangi bir aşamada başarısız olursa → `Invalid Hallucination` (§4.1) olarak işaretlenir,
   kalibrasyon vektörüyle Agent'a iade edilir. **Q4-Q6 witness'lardan önce çalışır** —
   sentaks hatası olan bir Claim şahitlere gösterilmez.
5. **Witness-Based Gates (Q1-Q3 — `WitnessSet Ω` üzerinden):** Bağımsız şahitler iddiayı
   doğrular.
   - Q1: `min_approvers ≥ 2`
   - Q2: `support(Ω) ≥ θ_quorum` (varsayılan 1.5)
   - Q3: honest-reject yok
   - `Hold` dönerse → `UndersupportedClaim` (Eksik kanıtlar talep edilir).
   - `Reject` dönerse → `WitnessHallucination` (Gerekçeye göre revize istenir).
6. **Big Bang (Commit):** Tüm gate'ler geçilince iddia `S_c` katmanına yazılır, belirsizlik
   dalgası çöker ve uzay şimdiki zamanı ilerletir (`t_c ← t_c + 1`). Commit sonrası,
   `ΔV`'deki düğümlerin LRU cache entry'leri invalidate edilir (incremental invalidation, §5).

> **Pipeline sırası (önemli):** Q4-Q6 (claim-based) → Q1-Q3 (witness-based). Bu, Faz 2'nin
> tek-phase vision pre-check'inden (eski tek `Q4` = vision, artık `Q5`) genişletilmiş
> halidir. Claim-based gate'ler deterministik olduğu için witness'lardan önce koşmak
> kaynak israfını önler.

### 4.1 Hallucination Sınıflandırması (Gerilim 5 çözümü)

Her gate başarısızlığı farklı bir halüsinasyon türü üretir; kalibrasyon geri bildirimi
için kritik sınıflandırma:

| Gate | Başarısızlık Türü | Halüsinasyon Sınıfı | Kalibrasyon Stratejisi |
|---|---|---|---|
| **Q4 (Syntax)** | OutputContract'e uymayan çıktı | `StructuralHallucination` | "Çıktı formatın hatalı, şu şemaya uy" |
| **Q5 (Vision)** | `θ > θ_bound` | `VisionHallucination` | "Vizyondan saptın, şu açıyla düzelt" |
| **Q6 (Rule)** | Rule ihlali | `RuleHallucination` | "Şu kuralı ihlal ettin, alternatif yol bul" |
| **Q1-Q3 (Witness)** | Honest reject | `WitnessHallucination` | "Şahit seni reddetti, gerekçeye göre revize et" |
| **Q1-Q2 (Witness)** | Hold (yetersiz şahit) | `UndersupportedClaim` | "Yeterli şahit yok, bekle veya ek şahit talep et" |

`StructuralHallucination` deterministik ve anında geri beslenir (LLM yeniden koşturulur,
şema hatası düzeltilmiş çıktı istenir). Diğerleri kalibrasyon vektörüyle (θ düzeltme önerisi,
ihlal edilen kural referansı, şahit gerekçesi) Agent'a iade edilir.

---

## 5. Hibrit Gravity Index Stratejisi

### 5.1 Temel Karar

| Yaklaşım | Performans | Doğruluk | Esneklik | Tavsiye |
|---|---|---|---|---|
| Tamamen Statik | Çok İyi | Düşük | Düşük | Reddedildi |
| Tamamen Dinamik | Kötü | Çok İyi | Çok İyi | Sadece küçük repolarda |
| **Statik + Lazy Dynamic** | **İyi** | **İyi** | **İyi** | **Kabul Edildi** |

### 5.2 Statik Katman (Pre-computed Boundaries)

**Ne saklanır?**
- Değişmez veya çok yavaş değişen **Hard Rules**'ın tetiklediği sınır düğümleri ve kenarları.
- Örnek kurallar: domain modellerinin infrastructure'a doğrudan bağımlılığı, public API'lerin
  testsiz var olması, mimari katman ihlalleri.

**Ne zaman hesaplanır?**
- Sistem başlangıcında (bootstrap), yeni bir Hard Rule eklendiğinde, veya God Mode
  tarafından manuel tetiklendiğinde.

```rust
struct StaticGravityIndex {
    rule_boundaries: HashMap<RuleId, Vec<NodeId>>,
    last_updated: DateTime<Utc>,
}
```

### 5.3 Lazy Dynamic Katman

**Ne zaman hesaplanır?**
- `OspPrompt` üretilirken, ilgili `Intent` daha önce cache'lenmemişse, cache'in TTL'si
  dolmuşsa veya invalidation tetiklenmişse.

**Cache Stratejisi:**
- `Intent` + `Vision` kombinasyonuna göre **LRU Cache** kullanılır.
- Cache anahtarı: `IntentId + VisionHash`
- Cache invalidation tetikleyicileri: yeni commit (sadece `ΔV`'deki düğümlerin cache entry'leri),
  Rule güncellemesi, God Mode manuel komutu.

### 5.4 Cache Invalidation Skopu (Gerilim 4 çözümü)

Cache key `IntentId + VisionHash`, ama commit `ΔV` node set'i üzerinden etkili olur. Bir
node'un değişmesinin hangi Intent'lerin cache'ini etkilediği, MVP'de **tam temizlik** ile
çözülür; Faz 5'te incremental optimizasyona geçilir:

| Faz | Strateji | Invalidation Skopu | Performans |
|---|---|---|---|
| **Faz 5 MVP** | Tam dynamic cache temizliği | Her commit → tüm dynamic cache boşaltılır | Küçük/orta repolarda ihmal edilebilir |
| **Faz 5+ Optimizasyon** | Incremental (tersine indeks) | `Node → [IntentId]` reverse index; sadece etkilenen Intent cache'leri | Büyük repolar (50k+ node) |

**Gerekçe:** MVP'de LRU cache zaten sıcak Intent'leri tutar; tam temizlik sonrası ilk birkaç
projeksiyon cache-rebuild maliyeti çeker ama bu, ortalama repo boyutunda kabul edilebilir.
Tersine indeks'in ek karmaşıklığı (reverse index tutma, commit sonrası güncelleme) ancak
 ölçek baskısı ortaya çıkınca haklı.

### 5.5 God Mode Manuel Tetikleme Desteği

```rust
// Örnek CLI / API komutları
osp gravity rebuild-static              // Statik index'i tamamen yeniden hesapla
osp gravity invalidate --intent-id <id> // Belirli bir Intent cache'ini temizle
osp gravity update-rule --rule-id <id>  // Tek bir kuralın sınırlarını güncelle
osp gravity status                      // Index durumu ve yaş bilgisi
```

### 5.6 Güncelleme ve Invalidation Stratejisi

| Olay | Statik Index | Dynamic Cache (MVP) | Dynamic Cache (Faz 5+) | Yaklaşım |
|---|---|---|---|---|
| Normal Commit (Big Bang) | Incremental update | **Tam temizlik** | `ΔV`'deki entry'ler invalidate | Otomatik |
| Yeni Hard Rule ekleme | Tam yeniden hesaplama | Tam cache temizleme | Tam cache temizleme | Otomatik + Log |
| Rule güncellemesi | Incremental | Tam temizlik | Etkilenen cache'ler invalidate | Otomatik |
| God Mode manuel tetikleme | İsteğe bağlı | İsteğe bağlı | İsteğe bağlı | Manuel |
| Uzayda büyük değişiklik | Lazy (sonraki erişimde) | Tam invalidate | Tam invalidate | Otomatik |

---

## 6. Özet Karar Tablosu

| Konu | Karar |
|---|---|
| **Intent zaman katmanı** | `t_f` (gelecek) — potansiyel gradyan; §0 ontolojik harita |
| **Prompt'un Ontolojik Statüsü** | Epistemik Projeksiyon Paketi (`π_A`) — doğal dil değil (inv #14) |
| **LLM'in Rolü** | Agent kabuğu içindeki durumsuz stokastik motor (inv #11) |
| **Hallucination Tanımı** | Objektif uzayda yürütülebilir patikası olmayan sapma (§4.1) |
| **Alt-Uzay Seçimi** | Üç katmanlı: Intent Gravity (k=2) → Vision/Rules → Permission/Evidence |
| **k=2 vs N₁(ΔV)** | Projeksiyon k=2, mutasyon reposition N₁(ΔV) — farklı operasyonlar (inv #6) |
| **Gravity Index** | Statik + Lazy Dynamic Hibrit |
| **Claim-Based Gates** | Q4 Syntax → Q5 Vision → Q6 Rule (witness öncesi, deterministik) |
| **Witness-Based Gates** | Q1 min_approvers → Q2 quorum → Q3 no-honest-reject |
| **PermissionMask Denetimi** | Üç nokta: slice engine → Agent kabuğu → `commit()` nihai (inv #13) |
| **OutputContract** | Uymayan çıktı = Q4 deterministik reject (inv #12) |
| **Pozisyon authority** | LLM declare ETMEZ — engine compute eder (inv #4); `position_hints` advisory only |
| **Custom Axis Modeli** | 5 core + N custom + derived; Agent axis tanımlayamaz (inv #15) |
| **Permission filter sırası** | Evidence node'ları permission'dan önce filter edilerek eklenir (§3, güvenlik) |
| **Cache Stratejisi** | LRU + Intent+Vision anahtarlı; MVP'de tam temizlik, Faz 5+'ta incremental |
| **God Mode Kontrolü** | Manuel tetikleme komutları ile tam operasyonel kontrol |

---

## 7. İnvariant Cross-References

Bu doküman aşağıdaki invariant'ları tanıtır veya kullanır (bkz.
`implementation-invariants.md`):

| İnvariant | Bu Dokümanda Nerede | Tür |
|---|---|---|
| #4 (RawPosition computed, not declared) | §2.2 (LLM pozisyon declare edemez) | kullanır + güçlendirir |
| #6 (incremental Big Bang) | §3 (k=2 vs N₁ ayrımı) | kullanır + netleştirir |
| #8 (provider optional) | §2.1 (PermissionMask lokal) | kullanır |
| **#11 (LLM durumsuz)** | §1 (Terminoloji), §4.3 | **yeni** |
| **#12 (OutputContract deterministic reject)** | §2.2, §4 Q4 | **yeni** |
| **#13 (PermissionMask God Mode atanır)** | §2.1 (üç-nokta denetim) | **yeni** |
| **#14 (Prompt tiplenmiş paket)** | §2 (OspPrompt struct) | **yeni** |
| **#15 (Custom axis God Mode only)** | §2 (`axis_manifest`), §2.1 PermissionMask `writable_axes` | **yeni** |

---

*Bu doküman, OSP'nin Faz 5 (LLM Codec) tasarımı için temel sözleşmeyi oluşturur. Tüm
implementasyon kararları burada belirtilen ontolojik tanımlara ve veri yapılarına uygun
olmalıdır. Sürüm: 1.0-final · 2026-06-21.*
