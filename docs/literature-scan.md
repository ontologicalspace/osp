# OSP — Literatür Taraması (Faz 0.0)

> Amaç: OSP'nin **özgünlüğünü** (varoluşsal risk) teyit etmek; hangi teknikleri
> **ödünç alabileceğimizi** belirlemek; makalenin *Related Work* bölümünü hazırlamak.
>
> Yöntem: 5 köklü alan + 2 çapraz kesit, Wikipedia/aratolojik kaynak + arxiv (2024–2026).
> Bu bir *scan*, *systematic review* değil — derinlik Faz 1'de gelecek.

---

## 0. Özet Karar (TL;DR)

**OSP'nin sentezi özgün.** Hiçbir tek çalışma, beş kavramı birleştirmiyor:
(1) yazılım için ontolojik kavramsal uzay, (2) epistemolojik iki-şahit commit
(BFT-quorum'un bilgiye uyarlanması), (3) hata-kalibrasyonu olarak topolojik sapma,
(4) LLM çıktısı üzerinde local "God-mode" filtre, (5) LLM iletişimi için
koordinat-sıkıştırma protokolü (OSP).

Her **sütunün** köklü literatürü var — OSP icat etmiyor, **köprü kuruyor**:
Yazılım Metrikleri + Ontoloji Mühendisliği + Dağıtık Consensus + LLM Context
Mühendisliği + TDA. Bu, bir makale için ideal konumlandırmadır.

**En yakın akrabalar (ve farklarımız):**
| Çalışma | Yakınlık | OSP Farkı |
|---|---|---|
| **GraphRAG** (Microsoft, 2024) | En yakın — LLM için grafik-tabanlı context | GraphRAG *text çıkarımı* + *retrieval*; OSP *fizik-kurallı* ontoloji + *God-mode filtre* + *koordinat protokolü* (sıkıştırma) |
| **Graph-based Agent Memory** (Yang et al. 2026, arxiv:2602.05665) | Çok yakın — agent belleği graf | Zaman/şahitlik epistemolojisi yok; sapma/θ yok; LLM-sıkıştırma protokolü yok |
| **CodeCity / polymetric views** (Wettel & Lanza 2007) | Görsel akraba | Yalnızca görselleştirme; fizik-kuralı değil, agentı kısıtlamıyor |
| **BFT consensus** (Paxos/PBFT/Dolev-Strong) | Kavramsal akraba (şahitlik) | OSP, BFT quorum'u *yazılım bilgi-commit'ine* uyarlar (W₁,W₂ → ana uzay) |

---

## 1. Yazılım Görselleştirme (Software Visualization)

**Köklü alan.** Yapı, davranış ve evrimin görsel temsili.

**Anahtar kaynaklar:**
- Wettel & Lanza (2007) — *CodeCity*: yazılımı 3B şehir olarak (polymetric views).
- Lanza (2004) — *CodeCrawler*: polymetric view'lar.
- Marcus, Feng & Maletic (2003) — 3D yazılım görselleştirme.
- Diehl (2007) — *Software Visualization* kitabı (Springer).
- Koschke (2003) — görselleştirme survey (reverse eng. / maintenance).
- Limberger et al. (2013) — etkileşimli yazılım haritaları.
- Bohnet & Döllner (2011) — kod kalitesi + geliştirme aktivitesini haritalama.

**OSP ne ödünç alır:**
- **"Kavramsal uzayın görsel haritası"** metaforu (Faz 6 dashboard için doğrudan ilham).
- **Kod-değişikliği ↔ yürütme-izi projeksiyonu** (Bohnet 2009) — OSP'nin "Big Bang genişlemesini" vizyon hattına projeksiyon fikrine benzer.

**OSP ne ekler (fark):**
- Görselleştirme araçları **gözlemci** konumundadır; OSP'nin uzayı **agentı kısıtlayan fizik kuralları** içerir (God-mode). CodeCity okunur; OSP'de agent *o uzayın içinde yaşar* ve fizik kurallarına çarpar.

---

## 2. Yazılım Metrikleri (Software Metrics)

**Çok köklü ama krizdeki alan.** Bu, OSP'nin tezinin en güçlü ampirik zemini.

**Anahtar kavramlar ve kaynaklar:**
- **Coupling & Cohesion** — Parnas'tan beri mimari temel; OSP'nin `x` (kuplaj) ve `y` (kohezyon) aday eksenlerinin kökeni.
- **McCabe Cyclomatic Complexity** (1976).
- **Halstead Complexity Measures** (1977) — OSP'nin `commit_entropy` (w ekseni) için hesap mimarisi.
- **Function Points** (Albrecht) — OMG Automated Function Points standardı.
- **Maintainability Index** — OSP'nin "istikrar" ekseniyle örtüşür.
- Fenton & Bieman (2014) — *Software Metrics: A Rigorous and Practical Approach* (3. baskı). **Ana ders kitabı.**
- **Amit & Feitelson (2020) — "Corrective Commit Probability"** (arXiv:2007.10912): git geçmişinden *kod kalite metrikleri* çıkarır. **OSP'nin witness analizcisiyle aynı damar** — buradan teknik ödünçlenecek.
- **Tempero & Ralph (2026) — "Making Software Metrics Useful"** (arXiv:2603.16012): yaygın metriklerin karar-vermede pratikte kullanışsız olduğunu savunur. **OPS'NİN TEZİNİ DOĞRULAYAN EN ÖNEMLİ BULGU.**

**OSP ne ödünç alır:**
- Tüm aday eksenlerin (kuplaj/kohezyon/soyutlama/istikrar) standart formüllerini → `metrics.rs` (Faz 0.5).
- "Corrective Commit Probability" yöntemi → witness analizi zenginleştirmek için.

**OSP ne ekler (fark — TEZİN KALBİ):**
- Tempero & Ralph'ın eleştirisi: metrikler tek başına karar-vermeye yetmiyor. **OSP'nin cevabı: metrikleri tek tek sunmak yerine, onları ontolojik uzayda *konum* (coordinate) ve vizyon vektörüne *sapma açısı (θ)* olarak konumlandır.** Yani metrikler birer skaler tablo değil, geometrik bir navigasyon sistemi. Bu, "metriklerin kullanışsızlığı" krizine doğrudan bir yanıttır ve makalenin giriş/katkı bölümünün omurgası.

---

## 3. Topolojik Veri Analizi (TDA)

**OSP'nin matematiksel derinliği için en zengin kaynak.**

**Anahtar kavramlar:**
- **Persistent Homology** — Edelsbrunner et al. (2002); Zomorodian & Carlsson (2005); Barannikov (1994, canonical forms).
- **Persistence Diagrams / Barcodes** — görselleştirme + sınıflandırma.
- **MAPPER algoritması** — Singh, Carlsson et al. (2007); Ayasdi'nin ticari temeli.
- **Stability theorems** — Cohen-Steiner et al. (2006): küçük gürültü → küçük persistence değişimi. **OPS'NİN "KALİBRASYON" SÜRECİNİN MATEMATİKSEL MODELİ.**
- **Bottleneck & Wasserstein mesafeleri** — iki persistence diyagramı arası uzaklık.
- Yazılımlar: GUDHI, Ripser, javaPlex, Dionysus, PHAT, Topology ToolKit.

**OSP ne ödünç alır (büyük fırsat):**
- OSP'nin `cos θ = (V_vision · P_agent) / (||V_vision|| ||P_agent||)` formülü şu an naif bir kosinüs. **Faz 1'de bunu persistence diyagramları arası bottleneck/Wasserstein mesafesi olarak yeniden formüle edebiliriz** — bu, matematiksel derinliği ve akademik meşruiyeti anında yükseltir.
- **Stability theorem** → OSP'nin "erken sapma tespiti" (early-exit) tezinin teorik temini: küçük kod değişikliği küçük topolojik değişiklik yapmalı; büyük değişiklik = anomali = θ > eşik.
- **Multidimensional persistence** (Carlsson-Zomorodian) → OSP'nin çok-eksenli uzayı (x,y,z,w,v,u) için doğal matematik (multi-param persistence tam uygun).

**OSP ne ekler (fark):**
- Klasik TDA *veri* üzerinde; OSP **yazılım ontolojisi** üzerinde ve **zaman/şahitlik** dinamikleriyle birleşmiş. TDA + epistemoloji sentezi yeni.

---

## 4. Knowledge Graphs (Bilgi Grafikleri)

**2024+ patlama alanı; OSP'nin en yakın rekabet alanı.**

**Anahtar kaynaklar:**
- Hogan et al. (2021, ACM Computing Surveys) — *Knowledge Graphs*: **THE survey**.
- **GraphRAG (Microsoft, Edge et al. 2024, arXiv:2404.16130)** — LLM-üretilen graf + retrieval. OSP'ye en yakın endüstri çalışması.
- Lewis et al. (2020, NeurIPS) — RAG'ın orijinali.
- Zhou et al. (2020) — *Graph Neural Networks: A Review*.
- Su et al. (2025, arXiv:2510.21131) — *LLM + Text-Attributed Graphs survey*.
- Bian (2025, arXiv:2510.20345) — *LLM-empowered KG construction survey*.
- **Yang et al. (2026, arXiv:2602.05665) — *Graph-based Agent Memory***: agent belleği graf olarak, taxonomy (short/long, knowledge/experience, non-structural/structural). **OSP'ye kavramsal olarak en yakın.**
- Jia et al. (2026, arXiv:2601.09113) — *The AI Hippocampus*: LLM/MLLM belleği (implicit/explicit/agentic).

**OSP ne ödünç alır:**
- KG modeling (entity-relation-attribute) → OSP'nin ontolojik düğüm/kenar modeli.
- KG embedding yöntemleri (RDF2Vec, GNN'ler) → OSP koordinat sisteminin öğrenilebilir versiyonu (Faz 5+).
- **Entity alignment** (Bird rendorf 2020; Hogan 2023 LLM-based) → **OPS'NİN "O MU? BU MU?" KARAR MATRİSİNİN TEKNİK TEMELİ**. İki repo-uzayını hizalama, tam bir entity alignment problemidir.

**OSP ne ekler (fark — net konumlandırma):**
1. **GraphRAG**: metin→graf çıkarımı + retrieval. OSP'de graf zaten *proje ontolojisinden* geliyor (kod+review+kurallar), ve OSP **retrieval yapmaz, fizik-kuralı uygular** (God-mode: LLM çıktısı uzay-dışıysa reddedilir). Farklı problem.
2. **Agent memory graphs**: bellek organizasyonu. OSP'de *bilgi doğrulama* (şahitlik) + *protokol sıkıştırma* (OSP paketi) ek katmanları var.
3. **KG'ler genelde statik anlık görüntü**; OSP'nin uzayı **zaman/şahitlik durum makinesi** ile canlı.

---

## 5. Ontoloji Bileşenleri (Ontology Engineering)

**Formal sözlüğün kaynağı.**

**Standart bileşenler (Wikipedia/usul):**
- **Individuals** (instances), **Classes** (concepts), **Attributes**, **Relations**, **Function terms**, **Restrictions**, **Rules**, **Axioms**, **Events**, **Actions**.
- **is-a** (subsumption → taxonomy), **part-of** (mereology → DAG).
- Gómez-Pérez, Fernandez-Lopez & Corcho (2006) — *Ontological Engineering* (Springer). **Ana ders kitabı.**
- Donnelly & Guizzardi (2012) — FOIS; formal ontoloji temelleri.

**OSP ne ödünç alır:**
- Tüm ontolojik primitiflerin isimlendirmesi ve formal yapısı (Faz 1 `osp-core`).
- Meta-ontoloji: `Feature/Bug/Rule/Agent/Branch/Issue/PR/Review` OSP'nin *üst ontolojisi*; bunlar yazılım süreçleri için bir **meta-ontology** oluşturur.

**OSP ne ekler (fark):**
- Klasik ontoloji **statik mantık** (OWL/RL rules). OSP'ye iki dinamik katman eklenir:
  - **Zaman katmanı** (miş'li/şimdiki/gelecek) — ontolojiye *epistemolojik durum* ekler.
  - **Şahitlik operatörü** W — bir ontolojik iddianın *gerçekliğe geçiş* kuralı. Bu, ontoloji mühendisliğinde standart değildir (genelde her assertion anında true sayılır).

---

## 6. Dağıtık Consensus (BFT, Quorum, Sybil) — ÇAPRAZ KESİT 1

**OPS'NİN İKİ-ŞAHİT KURALININ MEŞRULUK KAYNAĞI. Bu literatürün OSP'ye entegrasyonu makalenin en güçlü argümanlarından biri olacak.**

**Anahtar kavramlar:**
- **Consensus problem**: Termination, Integrity, Agreement, Validity.
- **Byzantine Fault Tolerance (BFT)** — Pease-Shostak-Lamport (1980); Lamport-Shostak-Pease (1982, "Byzantine Generals").
- **Eşikler**: oral-messages modelde `n > 3f`; written (authenticated) modelde `n > f+1` (Dolev-Strong 1983).
- **Paxos** (Lamport), **Raft**, **PBFT** (Castro-Liskov 1999), **Multi-Paxos**.
- **FLP Impossibility** (Fischer-Lynch-Paterson 1985): asenkron deterministik consensus crash-fault ile imkânsız → randomized çözümler.
- **Sybil attack** + **permissionless consensus**: PoW (Bitcoin), PoS, PoA, Po-space, proof-of-personhood.
- **Phase King algorithm** (Garay-Berman) — polynomial binary BFT.

**OSP ne ödünç alır (devrimsel bağlantı):**
- **OPS'NİN `W(I_A, W₁, W₂) → δS` OPERATÖRÜ, BİR BFT QUORUM-COMMIT'İN BİLGİ ALANINA UYARLANMASIDIR.** Bu bağlantı kurulduğunda, OSP'nin iki-şahit kuralı aniden dağıtık sistemlerin 40 yıllık teorisine dayanır:
  - "Miş'li zaman" = **commit öncesi öneri** (Paxos proposer).
  - "Şahitlik (review)" = **quorum acknowledge**.
  - "Şimdiki zamana geçiş" = **commit / decide**.
  - Ana branch = **replicated log**.
- **Eşik bağlantısı**: Dolev-Strong (authenticated, `n > f+1`) → OSP'de **iki bağımsız non-author şahit**
  authenticated-review modeline uyar (`f=1` için `n=3` = author + 2 witness).

  **Güncel formalizm notu (revize):** OSP'nin iki-şahit kuralı *tam BFT equivalence* olarak değil,
  **authenticated BFT quorum modelinin bilgi-commit alanındaki safety-refinement**'ı olarak konumlandırılır
  (bkz. `OSP-formalism.md §7`). Strict liveness `n=3` ile garanti edilmez — `Hold` durumları 3. witness,
  timeout/retry veya partial synchrony ile çözülür (Lemma 2b). **Safety tarafı (kötü commit engelleme)
  optimal; liveness pratik mekanizmalarla.** Bu ayrım makale eleştirisine karşı korur.

**OSP ne ekler (fark):**
- BFT *state-machine replication* için; OSP **bilgi-ontolojisine commit** için. Yeni uygulama alanı.
- **Faz 4 Sybil Resistance** (kullanıcının geri bildirimiyle yola haritasına eklediğimiz): bu literatürden doğrudan gelir. KG/entity-weighted trust score, PoW/PoS/proof-of-personhood analogları → "şahit güvenilirlik skoru".

---

## 7. LLM Context Mühendisliği — ÇAPRAZ KESİT 2

**OPS'NİN SIKIŞTIRMA PROTOKOLÜ (OSP) İÇİN REKABET ALANI.**

**Anahtar kaynaklar:**
- Lewis et al. (2020) — RAG (NeurIPS).
- Edge et al. (2024) — GraphRAG (arXiv:2404.16130).
- Petrova et al. (2025, arXiv:2507.10644) — *Web of Agents*: 3 kuşaklık agent-web evrimi (MAS → Semantic Web → LLM-agen). OSP'yi *4. kuşak* olarak konumlandırma fırsatı.
- Yang et al. (2026) — graph-based agent memory.

**Yaygın problem (OSP'nin niş alanı):**
- **Context window şişmesi** — 1M+ token istekleri, quadratic attention maliyeti, enerji israfı.
- **Context drift** — agent'ın dünü/bugünü/yarını karıştırması.
- **Hallucination** — üretilen içerik corpus-dışı.

**OSP'nin cevabı (protokol katkısı):**
- RAG/GraphRAG **daha çok veri göndererek** çözmeye çalışır. OSP **daha az veri (koordinat + topoloji)** göndererek çözer — *ontolojik sıkıştırma*.
- Bu, bir **iletişim protokolü** olarak adlandırılabilir (HTTP gibi): eğer LLM'ler OSP koordinat formatını öğrenirse, lokal God-mode ↔ büyük-LLM arası trafik token-tabanlı değil, semantic-delta-tabanlı olur.

**OSP ne ekler (fark):**
- **Deterministik God-mode filtre**: GraphRAG yine de LLM'i serbest bırakır; OSP'de LLM çıktısı önce uzay-fiziğinden geçer, geçemezse reddedilir (early-exit).
- **Hallucination guardrail**: üretilen koordinat ontoloji-dışıysa = otomatik reddet.

---

## 8. Özgünlük Matrisi (Novelty Defense)

| Katkı | GraphRAG | Agent-Memory-Graph | TDA-on-code | BFT-Consensus | OSP |
|---|---|---|---|---|---|
| Yazılım-özgü ontolojik uzay | ✗ (generic text) | ✗ | kısmen | ✗ | **✓** |
| İki-şahit epistemolojik commit | ✗ | ✗ | ✗ | ✓ (state için) | **✓ (bilgi için)** |
| Geometrik/topolojik sapma (θ) | ✗ | ✗ | ✓ (statik) | ✗ | **✓ (dinamik, kalibrasyon)** |
| God-mode local filter | ✗ | ✗ | ✗ | ✗ | **✓** |
| Koordinat-sıkıştırma protokolü (OSP) | ✗ | ✗ | ✗ | ✗ | **✓** |
| Zaman durum makinesi (miş'li/şimdiki/gelecek) | ✗ | kısmen (bellek) | ✗ | ✓ (log) | **✓ (birleşik)** |

**Sonuç:** OSP'nin hiçbir parçası *icat edilmemiş* — ama **beşini birleştiren tek çalışma yok**. Bu, makalenin *Contribution* bölümünün omurgası.

---

## 9. Ödünç Alınacak Teknikler → Faz Ataması

| Teknik | Kaynak | OSP'de Nereye | Faz |
|---|---|---|---|
| Coupling/cohesion/complexity formülleri | McCabe, Halstead, Parnas | `metrics.rs` aday eksenler | 0.5/1 |
| "Corrective Commit Probability" metodu | Amit-Feitelson 2020 | witness analizcisi zenginleştirme | 0.7 |
| Persistence diagramları + bottleneck mesafe | Edelsbrunner, Cohen-Steiner | `cos θ` → gerçek topolojik sapma | 1 (math formalizm) |
| Multidimensional persistence | Carlsson-Zomorodian | çok-eksenli uzay temsili | 1 |
| KG entity alignment | Berrendorf 2020; Hogan 2023 | "O mu? Bu mu?" rezonans motoru | 4 |
| BFT quorum eşikleri (n>3f / n>f+1) | Dolev-Strong, Lamport | iki-şahit kuralının optimallik ispatı | 1 (paper) + 4 (Sybil) |
| Sybil resistance (PoS/PoA/proof-of-personhood analogları) | Bitcoin, Ethereum literatürü | Malicious Witness Detection | 4 |
| Polymetric views (CodeCity) | Wettel-Lanza 2007 | dashboard görselleştirme | 6 |
| Stability theorem | Cohen-Steiner 2006 | early-exit teorik temini | 1 + 5 |

---

## 10. Makale İçin Açık Sorular (Faz 1'de ele alınacak)

1. **`cos θ` reformülasyonu**: Kosinüs → bottleneck mesafesi ne zaman geçerli? Çok-eksenli uzayda persistence diagramı nasıl tanımlanır (her eksen için ayrı mı, birleşik mi)?
2. **İki-şahit eşiğinin optimallik ispatı**: Dolev-Strong `n > f+1`'in OSP bilgi-commit problemine uyarlanmasının formal reduction'ı.
3. **OSP protokolü determinizm**: LLM çıktısının parse-edilebilir OSP paketi olduğunun zorlanması — bu bir context-engineering mi yoksa fine-tuning mi gerektirir?
4. **"Miş'li zaman" ↔ FLP**: Asenkron consensus'ta deterministik imkânsızlık → OSP'de agent'lar asenkron çalıştığında şahitlik ne zaman *liveness* garanti eder?
5. **Kambriyen patlaması / köpük**: "AI-üretilen proje kalitesi" ölçümü için OSP'nin negatif-uzay yoğunluğu literatürde karşılıksız mı? (Görünen o ki evet — bu *pratik niş* makalenin case study'sü olabilir.)

---

## 11. Tam Kaynak Listesi (bib, Faz 7'de derlenecek)

### Yazılım Görselleştirme
- Diehl, S. (2007). *Software Visualization*. Springer.
- Wettel, R. & Lanza, M. (2007). Visualizing Software Systems as Cities. VISSOFT.
- Lanza, M. (2004). CodeCrawler — polymetric views. ASE.
- Marcus, A., Feng, L. & Maletic, J.I. (2003). 3D representations for software visualization. SoftVis.
- Koschke, R. (2003). Software visualization in maintenance/RE: a survey. *J. Softw. Maint. Evol.* 15(2).
- Limberger, D. et al. (2013). Interactive software maps. Web3D.
- Bohnet, J. & Döllner, J. (2011). Monitoring code quality by software maps. ICSE Wkshp.

### Yazılım Metrikleri
- Fenton, N.E. & Bieman, J. (2014). *Software Metrics: A Rigorous and Practical Approach* (3rd ed.). CRC.
- McCabe, T. (1976). A Complexity Measure. *IEEE TSE*.
- Halstead, M.H. (1977). Elements of Software Science. Elsevier.
- Amit, I. & Feitelson, D.G. (2020). The Corrective Commit Probability. arXiv:2007.10912.
- Tempero, E. & Ralph, P. (2026). Making Software Metrics Useful. arXiv:2603.16012.
- Gill, G.K. & Kemerer, C.F. (1991). Cyclomatic complexity density. *IEEE TSE* 17(12).

### Topolojik Veri Analizi
- Edelsbrunner, H., Letscher, D. & Zomorodian, A. (2002). Topological persistence and simplification. *Discrete & Computational Geometry*.
- Zomorodian, A. & Carlsson, G. (2005). Computing persistent homology. *Discrete & Computational Geometry*.
- Carlsson, G. (2009). Topology and Data. *Bull. AMS*.
- Cohen-Steiner, D., Edelsbrunner, H. & Harer, J. (2006). Stability of persistence diagrams. *Discrete & Computational Geometry*.
- Singh, G., Mémoli, F. & Carlsson, G. (2007). Topological Methods for the Analysis of High-Dimensional Data Sets (MAPPER). EuroVis.
- Barannikov, S. (1994). Canonical forms / Morse theory persistence.
- Edelsbrunner, H. & Harer, J. (2010). *Computational Topology: An Introduction*. AMS.

### Knowledge Graphs & LLM Context
- Hogan, A. et al. (2021). Knowledge Graphs. *ACM Computing Surveys* 54(4). arXiv:2003.02320.
- Lewis, P. et al. (2020). Retrieval-Augmented Generation. NeurIPS.
- Edge, D. et al. (2024). From Local to Global: A Graph RAG Approach. arXiv:2404.16130.
- Su, G. et al. (2025). LLMs Meet Text-Attributed Graphs: A Survey. arXiv:2510.21131.
- Bian, H. (2025). LLM-empowered KG construction: A survey. arXiv:2510.20345.
- Yang, C. et al. (2026). Graph-based Agent Memory: Taxonomy, Techniques, Applications. arXiv:2602.05665.
- Jia, Z. et al. (2026). The AI Hippocampus. arXiv:2601.09113.
- Petrova, T. et al. (2025). From Multi-Agent Systems and the Semantic Web to Agentic AI. arXiv:2507.10644.
- Zhou, J. et al. (2020). Graph neural networks: A review. *AI Open*.
- Berrendorf, M. et al. (2020). KG entity alignment with GCNs. ECIR.

### Ontoloji Mühendisliği
- Gómez-Pérez, A., Fernandez-Lopez, M. & Corcho, O. (2006). *Ontological Engineering*. Springer.
- Donnelly, M. & Guizzardi, G. (2012). FOIS.

### Dağıtık Consensus
- Lamport, L., Shostak, R. & Pease, M. (1982). The Byzantine Generals Problem. *ACM TOPLAS*.
- Pease, M., Shostak, R. & Lamport, L. (1980). Reaching Agreement in the Presence of Faults. *JACM*.
- Dolev, D. & Strong, H.R. (1983). Authenticated algorithms for Byzantine agreement. *SIAM J. Comput.* 12(4).
- Fischer, M.J., Lynch, N. & Paterson, M. (1985). Impossibility of distributed consensus with one faulty process (FLP). *JACM*.
- Lamport, L. (1998). The Part-Time Parliament (Paxos). *Distributed Computing*.
- Ongaro, D. & Ousterhout, J. (2014). In Search of an Understandable Consensus Algorithm (Raft). USENIX ATC.
- Castro, M. & Liskov, B. (1999). Practical Byzantine Fault Tolerance (PBFT). OSDI.
- Douceur, J. (2002). The Sybil Attack. IPTPS.

---

*Sürüm: 0.1 (Faz 0.0) · Yöntem: Wikipedia + arxiv scan · Derinlik Faz 1'de genişletilecek*
