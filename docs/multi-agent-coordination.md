# OSP Protocol Proposal: Multi-Agent Coordination & Shared Horizons

> **Status:** Proposal / Architecture Spec — **Faz 6+ target**
>
> Bu doküman OSP'nin mevcut tekil-ajan modeline (Faz 5) çoklu-ajan koordinasyonu için
> bir **uzantı önerisidir**. Şu an implement edilmemelidir. Konseptin olgunlaştırılması
> ve Faz 5 (tekil LLM codec) tamamlandıktan sonra değerlendirilmesi için dokümante edilir.
>
> **Öncelik:** mevcut dokümanlar > bu proposal. Çelişki olursa mevcut dokümanlar bağlayıcıdır.
> Bu dokümandaki tipler Rust crate'lerinde **yok** — forward-looking spec'tir.
>
> **Version:** 0.1-draft · 2026-06-22

---

## 0. Motivasyon ve Scope

OSP'nin Faz 5 tasarımı tekil ajan varsayar: bir `Intent`, bir `Agent`, bir `Claim` pipeline'ı.
Gerçek üretim senaryolarında çoklu ajan eşzamanlı çalışır:

- **İzole tasklar:** bağımsız bug-fix'ler, paralel refactoring — ajanlar birbirini etkilemez
- **Epic-level koordinasyon:** büyük mimari dönüşüm, paralel mikroservis geliştirme — ajanlar
  birbirinin işine bağımlı

Mevcut modelde ikinci senaryo tehlikeli: iki ajan aynı modülü değiştirirse çakışma ancak
`Big Bang` (commit) anında — yani çok geç — fark edilir. Bu proposal, çakışmayı **commit
öncesi** yakalamak için bir **Shared Horizon** (Dolanık Uzay Horizons) modeli önerir.

> **İsimlendirme notu:** Önerilen orijinal isim "Entangled Space Horizons" idi, ama "quantum
> entanglement" anlık/nedensel-olmayan korelasyon ifade eder. OSP'de ajan B, A'nın inancını
> **paylaşılan veri yapısı üzerinden** görür — bu message passing, anlık değil, nedensel.
> "Shared Horizon" daha doğru: bilgi sınırı (horizon) paylaşılır, ama nedensellik korunur.

---

## 1. İşbirliği Modelleri: Private Horizon vs Shared Horizon

`SpaceEngine` çoklu-ajan sürecini yönetmek için iki geometrik mod sunar. Mod seçimi
`God Mode` (orkestratör) tarafından `Intent` seviyesinde belirlenir.

| Özellik | Private Horizon (İzole) | Shared Horizon (Dolanık) |
|---|---|---|
| **Kapsam** | Tek Agent, tek bağımsız Intent | Birden fazla Agent, ortak parent Intent (Epic) |
| **Görünürlük** | Diğer paralel Agent'ların Belief'lerine tamamen kördür | Aynı havuzdaki Agent'ların `t_m_pool` inançlarını görür |
| **Kütleçekim** | Sadece `S_c` + kendi lokal ΔS | `S_c` + havuzun tüm `t_m_pool` inançları (sanal kütle) |
| **Şahitlik** | Standart Q1-Q3 (dış şahitler) | Sub-witnessing (havuz içi) + dış şahitler (quorum için) |
| **Senaryo** | Bağımsız bug-fix, refactor | Büyük mimari dönüşüm, paralel modül geliştirme |

---

## 2. Ontolojik Zaman Genişlemesi: `t_m_pool` Alt-Katmanı

Mevcut OSP üç zaman katmanına sahiptir (`OSP-formalism.md §3.1`, `agent-prompt-semantics.md §0`):

- `t_f` — Gelecek (Intent, potansiyel gradyan)
- `t_m` — Miş'li (Belief, aday, lokal öznel)
- `t_c` — Şimdiki (Knowledge, commit edilmiş, nesnel)

**Problem:** Mevcut tanımda `t_m` **lokal ve izole** — ajanın kendi öznel uzayı. Ajan B'nin
Ajan A'nın Belief'ini görmesi, `t_m`'nin "lokal" tanımını deler.

**Çözüm (proposal):** `t_m` alt-katmanlarına böl:

```
t_m_private  — Ajan'ın kendi inançları (kimse görmez). Klasik t_m.
t_m_pool     — Havuza broadcast edilen inançlar (üyeler görür, dış dünya görmez).
               Bu inançlar sanal kütle oluşturur (gravity).
t_c          — Commit edilmiş bilgi (herkes görür). Değişmedi.
```

**Broadcast semantics:** Bir Belief'i havuza publish etmek, onu `t_m_private` → `t_m_pool`'a
taşır. Sadece `t_m_pool`'deki inançlar sanal kütle (gravity) oluşturur; `t_m_private`'deki
inishler havuzun fiziksel alanını etkilemez.

**Bilgi akışı (multi-agent):**
```
Intent (t_f, parent epic)
  → projeksiyon → her Agent'a ayrı OspPrompt
  → Agent'lar çalışır, Belief'ler üretir (t_m_private)
  → Agent Belief'i havuza publish eder (t_m_pool) — sanal kütle
  → diğer Agent'lar bu sanal kütleyi görür, kendi çalışmasını adapte eder
  → her Agent Claim üretir, Q4-Q6 + Q1-Q3'ten geçer
  → Commit başarılı → t_m_pool → t_c (Knowledge)
```

---

## 3. Gravitational Interference (Sanal Kütle Modeli)

Shared Horizon modunda, Ajan A'nın ürettiği ama henüz commit edilmemiş Belief (`t_m_pool`),
uzayda **sanal bir kütle (virtual mass)** gibi davranır. `compute_space_slice` (Faz 5)
Ajan B için hesap yaparken bu sanal kütleyi hesaba katar.

```
[Ana Uzay S_c]  (t_c — commit edilmiş, herkes görür)
       │
       ├─► [Shared Horizon / Pool]  (t_m_pool — havuz üyeleri görür)
       │         ├─► Agent A (producer) ──► Belief_A (virtual mass)
       │         │                                   │ (gravitational pull)
       │         └─► Agent B (observer) ◄────────────┘
       │              Agent B'nin space_slice'ı Belief_A'nın
       │              etkilediği node'ları içerir
       │
       └─► [Private Horizon]  (t_m_private — sadece Agent C görür)
                 └─► Agent C (tamamen kör, sadece S_c'yi görür)
```

**Sanal kütle hesabı (kabaca):** Bir `t_m_pool` Belief'inin kütleçekim skoru, commit edilmiş
node'lardan daha düşük olmalıdır (henüz şahitlenmemiş). Önerilen ağırlık:

```
gravity_score(t_m_pool node) = base_gravity × confidence_factor
  where confidence_factor ∈ [0.3, 0.7]  (kalibrasyon: Faz 6)
```

Commit edilmiş node'lar `confidence_factor = 1.0`. Bu, sanal kütlenin "yakındakileri çeker ama
uzaktakileri fazla etkilemez" davranışını verir — agent'lar birbirlerinin çekim alanına girince
haberdar olur ama uzağa sızmayıp.

---

## 4. `EntanglementPool` Veri Yapısı (Proposal)

> **Not:** Bu tip Rust crate'lerinde YOK. Forward-looking spec'tir.

```rust
use std::collections::HashSet;

/// Ajanların shared horizon'da birlikte çalışmasını yöneten havuz.
/// God Mode tarafından parent Intent (Epic) ile ilişkilendirilerek oluşturulur.
pub struct EntanglementPool {
    pub pool_id: PoolId,
    /// Bu havuzda birlikte çalışan agent kabukları
    pub active_agents: Vec<AgentId>,
    /// Ortak hedefi tanımlayan üst intent (Epic)
    pub parent_intent_id: IntentId,
    /// Havuza broadcast edilmiş, henüz commit edilmemiş inançlar (t_m_pool)
    pub shared_beliefs: Vec<SharedBelief>,
    /// Havuzun uzayda kilitlediği ortak korumalı alan (rezerve node'lar)
    pub reserved_nodes: HashSet<NodeId>,
}

/// t_m_pool'a publish edilmiş bir ajan inancı.
pub struct SharedBelief {
    pub author_agent_id: AgentId,
    pub claim_candidate: ClaimCandidate,  // agent'ın henüz finalize etmediği ΔS önerisi
    pub affected_nodes: HashSet<NodeId>,   // bu inancın dokunduğu node'lar (gravity kaynağı)
    pub signals: Vec<PoolSignal>,          // yapısal koordinasyon sinyalleri (§6)
    pub published_at: Timestamp,
}
```

---

## 5. Group Space Slice Computation

Ajan B Shared Horizon'da çalışıyorsa, `compute_space_slice` (Faz 5, agent-prompt-semantics.md §3)
üçüncü bir katman ekler:

```rust
pub fn compute_group_space_slice(
    agent_id: AgentId,
    intent: &Intent,
    space: &Space,
    pool: &EntanglementPool,   // Shared Horizon referansı
    rules: &[Rule],
    mask: &PermissionMask,
    evidence: &EvidenceSummary,
) -> SpaceSlice {
    let mut nodes_bucket = HashSet::new();

    // Katman 1: Intent çekirdek düğümleri + K-Hop (Faz 5 mevcut)
    let core_nodes = &intent.etki_alani.nodes;
    nodes_bucket.extend(core_nodes.clone());
    for core_node in core_nodes {
        nodes_bucket.extend(space.get_neighbors_within_hops(core_node, 2));
    }

    // Katman 2 (YENİ): Shared Horizon sanal kütlesi
    // Havuzdaki diğer ajanların t_m_pool inançlarının etkilediği node'lar
    for belief in &pool.shared_beliefs {
        if belief.author_agent_id != agent_id {  // kendi inancını tekrar ekleme
            nodes_bucket.extend(&belief.affected_nodes);
        }
    }

    // Katman 3: Statik kurallar + risk expansion (Faz 5 mevcut)
    nodes_bucket.extend(space.detect_rule_boundary_nodes(core_nodes, rules));

    // Katman 4: Permission filter (Faz 5 mevcut — evidence'dan önce, güvenlik)
    nodes_bucket.extend(
        evidence.required_nodes_for_witnessing
            .iter()
            .copied()
            .filter(|node| mask.has_read_permission(*node)),
    );
    nodes_bucket.retain(|node| mask.has_read_permission(node));

    SpaceSlice::build_subgraph(space, nodes_bucket)
}
```

**Önemli:** Katman 2 (Shared Horizon) PermissionMask filter'ından (Katman 4) ÖNCE eklenir ama
`shared_beliefs`'ten gelen node'lar da permission filter'ından geçer. Bir ajanın read yetkisi
olmayan node'a başka bir ajanın inancı üzerinden sızması engellenir.

---

## 6. İletişim Kanalı: Typed `PoolSignal` (Free-form Chat DEĞİL)

**Karar:** Entangled ajanlar free-form "group chat" kanalıyla haberleşmez. İletişim
**data (code/claims) + typed yapısal sinyaller** üzerinden olur.

**Neden free-form chat reddedildi:**
- inv #14'ü deler (prompt doğal dil değil, tiplenmiş paket — agent-prompt-semantics.md §2)
- Mesajlar pozisyonlanamaz, Q5 Vision Gate'ten geçemez
- İkinci ontolojik kanal → kompleksite patlaması
- LLM-interpretli → non-deterministik (OSP'nin temel iddiası)

**Neden typed sinyaller kabul edildi:** Free-form chat'in expressivity avantajı (koordinasyon
ihtiyacı) typed signals ile karşılanır, ama determinizm korunur.

```rust
/// Yapısal koordinasyon sinyali — typed, positionable, Q5'e tabi.
/// Free-form chat DEĞİL — "traffic signal" gibi.
pub enum PoolSignal {
    /// "Şu node'lar üzerinde çalışıyorum, exclusive" — lock benzeri.
    /// Diğer ajanlar bu node'lara dokunmadan önce beklemeli/uyarı almalı.
    Reservation {
        node_ids: Vec<NodeId>,
        ttl: Duration,  /// rezervasyonun süresi (timeout = otomatik release)
    },

    /// "Claim'im şu node'lara dokunuyor, side-effect bekle."
    /// Diğer ajanlar bu node'ların pozisyonunun değişeceğini bilir.
    Warning {
        affected: Vec<NodeId>,
        claim_id: ClaimId,
    },

    /// "Local belief'im stabilize oldu, üstüne inşa edebilirsin."
    /// Diğer ajanlar bu inancı güvenle baz alabilir (ama henüz commit değil).
    Checkpoint {
        claim_id: ClaimId,
        confidence: f64,  /// 0.0-1.0, ne kadar stabilize olduğu
    },
}
```

**Semantik:**
- `Reservation` = optimistic locking (TTL var, deadlock yok)
- `Warning` = bildirim (diğer ajanlar serbest, ama uyarılı)
- `Checkpoint` = kooperatif (diğer ajanlar bağımlı çalışabilir)

Bu sinyaller `OspPrompt`'a serialize edilir, LLM'e iletilir — ama LLM bunları **doğal dil
yorumla** değil, typed alanlar olarak işler (inv #14 ile uyumlu).

---

## 7. Şahitlik Semantiği — KRİTİK: Sub-Witnessing ≠ Quorum

**En tehlikeli tasarım hatası riski:** "Havuz içindeki ajanlar birbirlerinin Claim'lerine
şahitlik yapabilir" ifadesi, Theorem 1'in bağımsızlık assumptions'ını ihlal eder.

**Neden:** Aynı epic'te çalışan A ve B:
- Aynı parent Intent'i paylaşıyorlar (ortak hedef → ortak bias)
- Birbirlerinin kodunu görüyorlar (koordinasyon → bağımlı kararlar)
- Biri başarısız olursa diğeri de etkileniyor (korelasyon)

Bunların şahitliği **independent evidence DEĞİL** — koordinasyon sinyali. Quorum'a sayılmamalı.

**Kural (BFT güven modelini korumak için):**

```
Pool-internal witnessing = conflict detection (internal validation)
External witnessing (pool dışı) = Q1-Q3 quorum evidence
```

- **Sub-witnessing (havuz içi):** Sadece **çakışma tespiti** için. "A'nin inancı B'nin
  inancıyla çelişiyor mu?" kontrolü. Quorum'a KATKISI YOK.
- **Quorum witnessing (dış):** Her Claim, havuz dışından en az `min_approvers` bağımsız şahit
  almalı (inv #1: author-witness rejection + havuz-üyeleri de author gibi davranır).

**Implementasyon implication:** `WitnessSet::canonicalize_for(author)` (inv #1) genişletilmeli:
sadece Claim author'ı değil, **aynı havuzun tüm üyeleri** approver listesinden çıkarılmalı.

```rust
// Faz 6+ extension (mevcut değil):
impl WitnessSet {
    pub fn canonicalize_for_pool(&self, author: AgentId, pool: &EntanglementPool) -> CanonicalWitnessSet {
        // inv #1 genişletilmiş: author + aynı havuzun tüm üyeleri exclude
        let excluded: HashSet<AgentId> = pool.active_agents.iter().copied().collect();
        // ... mevcut dedup + (source, actor, claim) + max-weight ...
        // events retain sadece actor ∉ excluded olanlar
    }
}
```

---

## 8. Commit Semantiği — Atomic vs Partial (Açık Soru)

**Orijinal öneri:** "Havuzdaki tüm ajanlar tamamlandığında tek paket (atomic transaction)
halinde commit." Bu çok katı.

**Sorunlu senaryolar:**
1. Ajan A erken bitiriyor, B hala çalışıyor → A beklemek zorunda mı?
2. A'nın işi iyi ama B'nin Claim'i Q5'te fail ediyor → A'nın iyi işi de mi reddedilir?
3. Dış şahitler B'nin parçayı reddediyor → A, B'nin reddedilen inancına baymışsa **cascade
   rework** gerekir.

**İki opsiyon (karar bekliyor):**

**Opsiyon A — Atomic All-or-Nothing (basit, katı):**
- Tüm havuz tek transaction. Biri fail → hepsi rollback.
- **Artı:** basit semantik, tutarlılık garanti.
- **Eksi:** yavaş ajan tüm havuzu bloklar; iyi işler kötü işlerle birlikte reddedilir.

**Opsiyon B — Partial Commit + Cascade Detection (esnek, karmaşık):**
- Her Claim bağımsız commit adayı. Conflict yoksa her biri ayrı Q4-Q6 + Q1-Q3'ten geçer.
- Conflict (iki Claim aynı node'u farklı yönde değiştiriyorsa) → havuz-seviye reject +
  kalibrasyon feedback.
- Reddedilen Claim'e bağımlı diğer Claim'ler **cascade-rework** olarak işaretlenir.
- **Artı:** esnek, hızlı ajanlar bloke olmaz.
- **Eksi:** cascade rework karmaşık, dependency graph takibi gerek.

**Öneri (şu an):** Opsiyon B'ye eğilimli, ama karar Faz 6 tasarımında verilmeli. Bu proposal
sadece **iki opsiyonu da belgeler** — seçimimplementation öncesi spike gerektirir.

---

## 9. Liveness — Takılı Ajan Sorunu (Açık Soru)

Shared Horizon'da bir ajan stuck olursa (LLM timeout, sonsuz döngü, crash):
- Atomic modelde (Opsiyon A): tüm epic bloke
- Partial modelde (Opsiyon B): o ajanın rezervasyonları TTL dolunca release olur, diğerleri
  devam eder

**Gerekli mekanizmalar (Faz 6):**
- `Reservation` TTL timeout (otomatik release)
- Agent heartbeat / liveness probe (God Mode monitor)
- Stuck agent detection + graceful degradation (agent'in Claim'i drop, bağımlılar cascade
  rework)

Bu, dağıtık sistemlerin klasik fault tolerance sorunudur — OSP'nin BFT mapping'i (§7 BFT
proof) burada da geçerli olmalı ama multi-agent liveness ek bir concern.

---

## 10. Kritik Açık Sorunlar (Özet)

Bu proposal implementasyona geçmeden önce çözülmesi gereken sorunlar:

| # | Sorun | Mevcut Durum | Öncelik |
|---|---|---|---|
| 1 | `t_m_pool` ontolojik alt-katman — mevcut `t_m` tanımıyla çelişki | Proposal olarak dokümante edildi, formalizme entegre DEĞİL | Yüksek (ontoloji temeli) |
| 2 | Sub-witnessing ≠ quorum — BFT güven modeli korunmalı | §7'de kural belgelendi, implementasyon yok | Kritik (güven) |
| 3 | Atomic vs Partial commit semantiği | İki opsiyon belgelendi, seçim bekliyor | Yüksek (spike gerek) |
| 4 | Cascade rework — reddedilen inanca bağımlı Claim'ler | Tespit edildi, mekanizma tasarlanmadı | Orta |
| 5 | Liveness — takılı ajan tüm havuzu bloklar | TTL + heartbeat önerildi, impl yok | Orta |
| 6 | Sanal kütle ağırlık kalibrasyonu (0.3-0.7 aralığı) | Tahmini değer, ampirik kalibrasyon gerek | Düşük (Faz 6 kalibrasyon) |
| 7 | `canonicalize_for_pool` — inv #1 genişletilmesi | Tasarlandı, mevcut `canonicalize_for`'a backward-compat değil | Yüksek (breaking change) |

---

## 11. İnvariant Etkileri

Bu proposal mevcut invariant'lara dokunmaz (implementasyon yok), ama Faz 6'da şu etkiler
olacak:

| İnvariant | Etki | Açıklama |
|---|---|---|
| #1 (author-witness rejection) | **Genişletilir** | `canonicalize_for` → `canonicalize_for_pool`: havuz üyeleri de exclude |
| #9 (WitnessSet W operatörü) | **Etkilenmez** | Quorum hala dış şahitlerden; sub-witnessing conflict-detection only |
| #13 (PermissionMask God Mode) | **Genişletilir** | Shared Horizon'da PermissionMask havuz-seviyesinde de uygulanmalı |
| #15 (custom axis God Mode) | **Etkilenmez** | Multi-agent axis tanımlamayı etkilemez |
| **#16 (yeni aday)** | **Eklenebilir** | Sub-witnessing quorum'a sayılmaz (pool witnessing ≠ evidence) |
| **#17 (yeni aday)** | **Eklenebilir** | `t_m_pool` inançları sanal kütle oluşturur, `t_m_private` oluşturmaz |

**Not:** #16 ve #17 şu an eklenmedi — bu proposal'ın implementasyonu (Faz 6) sırasında
değerlendirilecek.

---

## 12. Faz Pozisyonu ve Roadmap

Bu proposal **Faz 6** için hedeflenmiştir (`roadmap.md`'de henüz tanımlı değil — eklenecek).
Faz sıralaması:

```
Faz 5 (mevcut target): Tekil LLM OSP Codec (agent-prompt-semantics.md)
  ↓
Faz 5+: Custom Axis Marketplace (roadmap.md Faz 8)
  ↓
Faz 6 (bu proposal): Multi-Agent Coordination & Shared Horizons
  - Sub-witnessing semantics (conflict detection only)
  - t_m_pool ontolojik genişleme
  - PoolSignal typed coordination channel
  - Atomic vs Partial commit spike
  - Cascade rework mechanism
  - Liveness / fault tolerance
```

**Faz 6'ya geçiş önkoşulları:**
- Faz 5 (tekil LLM codec) stabilize olmuş ve üretime girmiş
- Custom axis modeli (Faz 8) olgunlaşmış — multi-agent pool'lar custom axis kullanabilir
- Gerçek multi-agent kullanım senaryosu ortaya çıkmış (premature değil)

---

## 13. Özet Karar Tablosu

| Konu | Karar |
|---|---|
| **İletişim kanalı** | Typed `PoolSignal` (Reservation/Warning/Checkpoint) — free-form chat REDDEDİLDİ |
| **Zaman katmanı** | `t_m` → `t_m_private` + `t_m_pool` alt-katmanları (proposal) |
| **Sub-witnessing** | Quorum'a sayılmaz — sadece conflict detection (BFT güven modeli korunur) |
| **Commit semantiği** | Atomic vs Partial — karar bekliyor (Faz 6 spike) |
| **Sanal kütle** | `t_m_pool` inançları gravity oluşturur, `t_m_private` oluşturmaz |
| **Liveness** | TTL + heartbeat + graceful degradation (Faz 6) |
| **Ontolojik konum** | Proposal — implementasyon Faz 6, mevcut dokümanları etkilemez |
| **İsimlendirme** | "Shared Horizon" (entangled değil — nedensellik korunur) |

---

## 14. Referanslar

- `docs/agent-prompt-semantics.md` — tekil ajan semantiği (Faz 5), bu proposal'ın tabanı
- `docs/OSP-formalism.md` §3.1 — zaman katmanları (t_m, t_c, t_f), bu proposal `t_m_pool` ekler
- `docs/OSP-formalism.md` §4.3 — commit operator (Q4-Q6 + Q1-Q3), sub-witnessing etkisi §7
- `docs/implementation-invariants.md` — inv #1 (author-witness), #13 (PermissionMask), #14 (typed prompt)
- `docs/space-engine-design.md` §4 — commit pipeline (multi-agent genişlemesi Faz 6)
- `docs/roadmap.md` — Faz yapısı (Faz 6 eklenecek)

---

*Bu doküman bir proposal'dur. Implementasyon Faz 6'da, Faz 5 stabilize olduktan sonra
değerlendirilecektir. Şu ankod tabanında hiçbir etkisi yoktur — konseptin kaybolmaması
ve olgunlaştırma için dokümante edilmiştir.*

*Sürüm: 0.1-draft · 2026-06-22 · Status: Proposal (Faz 6+ target)*
