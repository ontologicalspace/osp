# OSP Interaction Surfaces — Design Document

> **⚠️ Out-of-scope note.** This note is intentionally outside the Paper 3 claim surface.
> It is a brainstorming artifact for future interaction surfaces and does not define
> protocol invariants. It does not modify, extend, or contradict any claim in
> `paper3-draft-v1.md`. Decision numbers use the **DS** prefix (DS1–DS3) to avoid
> collision with the `concept-anchoring-design.md` decision namespace (D1–D18).

> **Sürüm:** v0.1-draft (Brainstorming)
> **Tarih:** 2026-07-03
> **Durum:** Kavramsal model olgunlaştırma dokümanı — **strictly out of Paper 3 scope**.
> **İlişki:** `docs/concept-anchoring-design.md` (Paper 3), `docs/agent-trajectory-roadmap.md` (Paper 2), `docs/paper-draft-v2.6.md` (Paper 1). Bu doküman, bu üç katmanın üzerine inşa edilen tüm kullanıcı/arayüz etkileşim modellerini tanımlar.

---

## 1. Vizyon ve Tasarım Prensibi

OSP'nin temel mimari ilkesi, mevcut invariant'larda da vurgulandığı gibi, **motorun arayüzden bağımsız olmasıdır.** Bu, projenin epistemolojik bütünlüğünü korur: bir bilgi hangi kanaldan sunulursa sunulsun aynı `Project Reality Graph`'a dayanır.

Bu dokümanın ana tezi şudur:

> **OSP, tek bir kullanıcı arayüzüne değil, aynı gerçekliğe bakan çok katmanlı bir etkileşim modeline sahiptir. Herkes aynı proje gerçekliğini görür; ama herkesin ihtiyacı kadar derinlik açılır.**

Temel mimari:

```text
osp-core (headless, deterministic engine)
  ├── osp-analyzer (repo analysis, code metrics)
  ├── osp-anchoring (Concept Anchoring, Genesis Layer)
  └── Interaction Surfaces (bu doküman)
        ├── Headless / MCP Mode
        ├── CLI Mode
        ├── Simple Overview UI
        ├── Expert Cockpit
        └── Report Surfaces
```

---

## 2. Beş Etkileşim Yüzeyi (Interaction Surfaces)

OSP, aşağıdaki beş yüzey üzerinden dış dünyayla iletişim kurar. Her yüzey aynı `Project Reality Graph`'ı referans alır, aynı `OperatorCapability` gate'lerinden geçer, ancak kullanıcıya sunduğu derinlik seviyesi farklıdır.

### 2.1 Headless / MCP Mode

Bu yüzey, OSP'yi bir **governance oracle** olarak konumlandırır. Kullanıcı bir GUI açmaz; doğrudan sorgular, analiz başlatır ve karar alır. Büyük ekipler zaten kendi dashboard, CI, Slack, Jira, GitHub, Linear, internal tools sistemlerini kullandığı için, OSP onlara "UI dayatmak" yerine bir motor gibi bağlanır.

- **Hedef kitle:** CI/CD pipeline'ları, AI agent sistemleri, teknik ekipler.
- **Araç seti (örnek MCP tools):**
  - `osp.analyze_repo`
  - `osp.get_project_health`
  - `osp.list_evidence_gaps`
  - `osp.list_unanchored_concepts`
  - `osp.explain_anchor`
  - `osp.approve_candidate`
  - `osp.reject_candidate`
  - `osp.export_report`
  - `osp.create_task_from_candidate`
  - `osp.get_drift_summary`
  - `osp.query_project_space`
  - `osp.check_predicate`
- **Örnek kullanım:**
  - *"Bu repo'daki yüksek riskli ama evidence'sız kavramları listele."*
  - *"Son commit'ten sonra vision drift arttı mı?"*
  - *"Candidate Rule'ları göster, kabul edilecekleri ayır."*
  - *"Payment ile ilgili accepted vision → code → evidence zincirini çıkar."*
- **Çıktı formatı:** JSON (programatik), Markdown (okunabilir), CSV (analiz).

### 2.2 CLI Mode

Headless modun insan tarafından okunabilir bir sarmalayıcısıdır. Renklendirilmiş çıktılar, özet tablolar ve hızlı durum sorguları sunar. Geliştiricinin günlük iş akışına entegre olur.

- **Hedef kitle:** Terminal odaklı geliştiriciler, hızlı kontrol yapmak isteyen mimarlar.
- **Örnek komutlar:**
  ```bash
  osp project health --repo ./my-project --format summary
  osp candidate review --show-all --filter high-risk
  osp drift --since HEAD~10
  osp evidence-gaps --repo ./my-project
  osp export --type project-space-summary --output report.md
  ```
- **Özellikler:** Pipe-friendly, CI'da `cargo test` gibi çalışabilir, exit code'lar anlamlı (0: temiz, 1: uyarı var, 2: kritik risk var).

### 2.3 Simple Overview UI (Project Health Dashboard)

Bu, OSP'nin "karar odaklı" ilk ekranıdır. Amacı, kullanıcıya projenin mevcut gerçekliğini en sade ve en eyleme geçirilebilir şekilde sunmaktır. **Asla bir graph ile açılmaz.**

- **Hedef kitle:** Teknik olmayan paydaşlar, ürün sahipleri, solo geliştiriciler.
- **Temel sorular:**
  - *Projem mimari olarak sağlıklı mı?*
  - *En büyük 3 risk nerede?*
  - *Hangi kararlar beni bekliyor?*
  - *Kod vizyondan sapıyor mu?*
  - *Son değişiklik neyi bozdu?*
- **Bileşenler:**
  1.  **Health Score:** Genel durum göstergesi (İyi / Dikkat / Riskli). Arkasında engine-measured metrikler ve confidence değerleri vardır.
  2.  **Needs Decision:** Kabul edilmeyi bekleyen `RuleCandidate`, `TaskCandidate` ve çelişkiler. Her kartın altında "Detayları göster" bağlantısı.
  3.  **What Changed?:** Son analizden bu yana drift, yeni riskler, evidence gap değişimleri.
- **Not:** Basit kullanıcıya metriklerin `confidence` değerini göstermeye gerek yoktur; ama düşük confidence'lı bir veri varsa, "Bu konuda yeterli veri yok" mesajı gösterilmelidir. Bu, INV-C1 ve Paper 1'in MetricValue provenance disiplininin UI'a yansımasıdır.

### 2.4 Expert Cockpit (Project Reality Cockpit)

Bu, profesyonel kullanıcılar için tam kapsamlı bir "uzay navigasyon panelidir." Paper 1-2-3'te tanımlanan tüm metrikler, grafikler ve kontrol mekanizmaları burada görünür olur.

- **Hedef kitle:** Yazılım mimarları, platform mühendisleri, senior geliştiriciler.
- **Paneller:**
  1.  **Project Space Topology:** Kod uzayı, coupling/cohesion/instability/entropy/witness-depth. Uzman kullanıcı x/y axis seçer, node kind filtreler, risk rengine göre boyar, mass'a göre ölçekler.
  2.  **Concept / Vision Map:** İnsan vizyonu → Decision → Rule → Task → Code → Evidence zinciri. Bu, OSP'nin ürün olarak farkını gösteren en önemli görünümdür.
  3.  **Anchor Review Queue:** Genesis Layer'ın operasyonel kalbi. Candidate Concept, Rule, Risk, Task, Contradiction, Weakly anchored packet'lar burada listelenir. Her item için Accept / Reject / Merge / Convert to task aksiyonları.
  4.  **Evidence Gap Monitor:** Yüksek riskli ama testsiz kavramlar, accepted rule var ama evidence yok, CodeEntity var ama witness zayıf.
  5.  **Drift & Risk Radar:** Son commit'lerde drift artışı, risk skoru yükselen modüller, vision alignment düşen alanlar.
  6.  **Trajectory / Task Monitor:** Hangi task hangi predicate'e bağlı? Agent ne önerdi? Claim ölçüldü mü? PredicateGate ne karar verdi? Progress checkpoint var mı?
  7.  **Decision Ledger:** Tüm accepted/deprecated/rejected kararların denetlenebilir kaydı. Kim söyledi? Hangi evidence ile desteklendi? Hangi code entity'leri etkiliyor?
  8.  **Reports & Exports:** Rapor üretimi ve dışa aktarım.
- **Auditability şartı:** Her anchor edge için skor kırılımı (semantic_similarity: 0.71, ontology_score: 0.90, graph_context: 0.62, domain_match: 0.80, code_evidence: 0.40, temporal_trust: 0.95, decision_status: 0.50 → final_score: 0.73) ve gerekçe görüntülenebilir olmalıdır. Özellikle high-stake edge'lerde (DerivesRule, DerivesTask, DerivesRisk, Contradicts, Supersedes, vb.) bu zorunludur (INV-C7).

### 2.5 Report Surfaces

Otomatik veya manuel olarak üretilen, insan tarafından okunabilir dokümanlardır. Kurumsal yönetişim ve şeffaflık için kritiktir. Enterprise ekiplerin sevdiği türden çıktılardır.

- **Örnek raporlar:**
  - `PROJECT-SPACE-SUMMARY.md`: Projenin genel fiziksel durumu.
  - `OBSERVED-ARCHITECTURE.md`: Mevcut kod yapısının özeti.
  - `RISKS-AND-GAPS.md`: Riskler ve kanıt boşlukları.
  - `RULE-CANDIDATES.md`: Onay bekleyen kurallar.
  - `CODE-TO-CONCEPT-TRACE.md`: Her bir kod modülünün hangi vizyon/konsepte bağlandığının izi.
  - `AGENT-TASK-BACKLOG.md`: Agent için önerilen görevler ve gerekçeleri.

---

## 3. Görünürlük ve Kontrol: Progressive Disclosure

Bu model, "Progressive Disclosure" ilkesini benimser: her kullanıcı seviyesi sadece ihtiyacı olan derinlikteki bilgiyi ve kontrolü görür.

| Kullanıcı Seviyesi | Tipik Rol | Görünürlük | Kontrol | Birincil Arayüz |
|---|---|---|---|---|
| **Agent / CI** | AI agent, pipeline | Predicate, görev, ret nedeni, mevcut ölçüm | `DeltaProposal` gönderme | MCP / CLI |
| **Simple User** | Ürün sahibi, solo dev | Sağlık skoru, risk listesi, karar bekleyenler | Adayları onaylama/reddetme | Simple UI |
| **Reviewer** | Tech lead, mimar | Aday listesi, çelişkiler, kanıt durumu | Kabul et, reddet, birleştir, göreve dönüştür | Review Console |
| **Expert** | Senior mimar, platform ekibi | Koordinatlar, metrik kaynakları, kanıt zinciri, sapma açısı, skor kırılımları | Yörünge tanımlama, kural yazma, manuel anchor | Expert Cockpit |
| **Enterprise** | Yönetim, denetim | Trend grafikleri, uyumluluk raporları, takım bazlı witness sağlığı | Rapor talebi, politika güncelleme | Reports / Cockpit |

---

## 4. Yeni Tasarım Kararları

Bu etkileşim modeli, OSP'nin mevcut ontolojisine şu kararları ekler:

### DS1 — UI as a Lens, Not the Engine

**Karar:** `osp-core` headless ve deterministik bir motor olarak kalır. Tüm kullanıcı arayüzleri (CLI, MCP, GUI, Raporlar), aynı `Project Reality Graph`'a bakan ve hiçbiri motorun temel invariant'larını değiştiremeyen "merceklerdir".

**Gerekçe:** Motoru arayüzden ayırmak, OSP'nin temel felsefi duruşudur. GUI'nin getireceği kısıtlamalar, motorun gelişimini yavaşlatmamalıdır.

### DS2 — Decision-First UI

**Karar:** OSP'nin ilk ekranı (Overview) bir topoloji grafiği değil, bir karar paneli olmalıdır. Kullanıcıya önce "Ne yapmalıyım?" sorusunun cevabı verilir; graph ve metrikler isteğe bağlı drill-down olarak sunulur.

**Gerekçe:** Graph etkileyici görünür ama çoğu kullanıcı için yorucudur. OSP'nin değeri, öncelikle aksiyon alınabilir çıktılar üretmesindedir.

### DS3 — Interaction ≠ Authority

**Karar:** Bir kullanıcının bir arayüzde "Onayla" butonuna basabilmesi, onun ontolojik olarak o işlemi yapmaya yetkili olduğu anlamına gelmez. Tüm yazma işlemleri (kabul, red, supersede, promote) arkada `OperatorCapability` ve `SupersedeAuthority` gate'lerinden geçer.

**Gerekçe:** UI'daki bir "Accept" butonu, agent'ın bir önerisini kabul etmek kadar ciddi bir epistemik işlemdir. Yetki kontrolü UI katmanında değil, core motor katmanında yapılmalıdır.

---

## 5. Yeni Invariant Önerileri (Proposal)

Bu etkileşim modeli, mevcut invariant'lara (INV #1-15, INV-T1-T8, INV-C1-C8) şunları ekler. Durum: proposal — implementation sırasında `docs/invariant-spec.md` Bölüm D'ye taşınır.

### INV-C9 — Interaction Surfaces Cannot Mutate Accepted Knowledge Without Capability

**Tanım:** Hiçbir etkileşim yüzeyi (UI, MCP, CLI), `OperatorCapability` ve gerekli `SupersedeAuthority` onayı olmadan `Accepted` durumundaki bir bilgiyi değiştiremez, silemez veya geçersiz kılamaz.

**Yapısal garanti:** Tüm yazma işlemleri (Accept, Reject, Supersede, Promote) core motor katmanında `OperatorCapability` kontrolünden geçer. UI katmanı bu kontrolü atlayamaz veya cache'leyemez.

**İhlal örneği:** UI'daki bir "Accept Candidate" butonu, yetkisiz bir kullanıcı için bile aktif olur ve candidate'ı accepted yapar → INV-C9 ihlal.

### INV-C10 — Queries Are Read-Only and Provenance-Aware

**Tanım:** Herhangi bir arayüzden yapılan tüm salt okunur sorgular, döndürdükleri her bir `MetricValue` için `source`, `confidence` ve `coverage` bilgilerini de eksiksiz olarak iletmek zorundadır.

**Yapısal garanti:** MCP/CLI/UI sorgu katmanı, `MetricValue`'yu düz `f64`'e indirgeyemez; provenance bilgisi cevap zarfında her zaman taşınır.

**İhlal örneği:** Simple UI'da "Coupling: 0.55" gösterilir ama bu değerin placeholder (confidence=0.0) olduğu belirtilmez → kullanıcı yanlış karar verir, INV-C10 ihlal.

---

## 6. Modlar Arası Geçiş (Mode Switch)

Aynı ürün içinde kullanıcı seviyesine göre mod değiştirilebilir olmalıdır:

```text
Overview  →  Review  →  Cockpit
```

- **Overview:** Ne durumdayım? (Simple UI)
- **Review:** Neye karar vermeliyim? (Candidate/Accepted geçişleri)
- **Cockpit:** Gerçekliği detaylı incele. (Expert panel)

Bu üç mod, üç ayrı ürün değil, aynı uygulamanın üç sekmesidir. Kullanıcı istediği zaman derinlik seviyesini değiştirebilir.

---

## 7. Ürün Mimarisi Önerisi

```text
osp-core           → deterministic engine, invariants, gates
osp-analyzer       → repo analysis, code metrics
osp-anchoring      → Concept Anchoring, Genesis Layer
osp-mcp            → MCP tools (headless oracle)
osp-cli            → terminal usage
osp-desktop        → Tauri Project Reality Cockpit (Overview + Review + Cockpit modları)
osp-reports        → Markdown/JSON/PDF exports
osp-integrations   → GitHub, GitLab, Jira, Linear, Slack bağlayıcıları
```

---

## 8. Sonuç

Bu doküman, OSP'nin sadece bir motor değil, aynı zamanda bir **etkileşim protokolü** olduğunu vurgular. MCP, CLI ve UI katmanları, motorun farklı kullanıcı profillerine uyarlanmış arayüzleridir. Hepsi aynı `Project Reality Graph`'a bakar, aynı invariant'lara tabidir, aynı `OperatorCapability` gate'lerinden geçer.

Temel ilke:

> **OSP her kullanıcıya aynı proje gerçekliğini gösterir; ama herkesin ihtiyacı kadar derinlik açar.**

Bu çok katmanlı etkileşim modeli, OSP'nin geniş kitlelerce benimsenmesinin anahtarı olacaktır.

---

*Bu doküman, OSP'nin kavramsal olgunlaştırma sürecinin bir parçasıdır. Paper 3'ün "Interaction Surfaces" bölümüne adaydır. INV-C9 ve INV-C10, implementation sırasında `docs/invariant-spec.md`'ye taşınır.*