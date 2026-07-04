# Paper 3 Taslak Yazımı — Handoff Notu (Yeni Oturum)

> **Tarih:** 2026-07-05
> **Dal:** `docs/paper3-draft` (main `0d269e7`'den)
> **Durum:** Aşama 1 (evidence freeze) tamam. Aşama 2 (iskelet) + Aşama 3 (bölüm dolgu) bekliyor.

---

## Nerede duruyoruz

Paper 3 (Concept Anchoring / Genesis Layer) **omurgası tamamlandı** — Faz 0-5.1 + 3 hardening PR.
Şu an **makale taslağı yazılıyor**. Evidence freeze (D1 disiplini: kanıt önce, yazım sonra)
aşaması bitti; uçtan uca binding chain replay JSON'u donduruldü.

## Bu oturumda yapılanlar

1. **Faz 5.1 (PR36) merge edildi** — Cross-family translation semantics (INV-P3)
2. **PR36 review patchleri uygulandı** — gerçek NFC (unicode-normalization crate), CrossFamilyHint::new sort, decomposed İ gerçek test, witnessdepth pattern, mixed-template sıkılaştırma doc, bare-witness vacuous fix
3. **Paper 3 yazımına karar verildi** — omurga tamam, makale yazımı en yüksek değer
4. **Paper 3 yazım planı onaylandı** (2 review turu sonra) — verification framing, evidence önce
5. **Aşama 1a: Evidence freeze tamam** — uçtan uca zincir JSON (`e2e-binding-chain-replay.json`) + test (`paper3_evidence.rs`)

## Yeni oturumda yapılacaklar (sıra ile)

### Aşama 1b: Held-out/adversarial cümle seti
5-8 cümle: "requires two witnesses" (bare witness), "accounting" (false positive), negation vakaları, çok-eksenli cümleler. Geliştirmede kullanılmamış → totoloji olmayan RQ1. `paper3-notes/evidence/held-out-adversarial-fixtures.json`.

### Aşama 1c: Run-metadata tablosu
Paper 2 pattern: commit hash, fixture sayısı (13), trybuild listesi (11 Paper 3'e özgü), test count (447), evidence strata.

### Aşama 1d: Conformance sonuçları
13 golden fixture + held-out set koş, sonuçları dondur.

### Aşama 2: İskelet
`docs/paper3-draft-v1.md` — tüm bölüm başlıkları + abstract (~350 kelime) + contributions (4 madde).

### Aşama 3: Bölüm dolgu
Abstract → §1-§12 → Appendix → References.

---

## Paper 3 Taslak Planı (onaylı — 2 review turu sonrası)

### Çerçeve kararları

- **Ana tez:** *"The ontological role of a human sentence in project reality is determined not by embedding proximity but by the completeness of the ontological binding chain."*
- **Anti-RAG → embedding-first anchoring eleştirisi:** *"Embedding can propose candidate proximity, but it cannot decide ontological role, authority, acceptance, or executability."*
- **Verification makalesi (evaluation değil):** §7 = "Verification Evidence", §7.5 "What this does not evaluate"
- **Lowering = placeholder:** *"The keyword matcher is not the contribution. The contribution is the binding protocol."*
- **Dil:** İngilizce (Paper 1/2 uyumu)

### Bölüm yapısı

```
Front matter (title + companion P1+P2 + revision)
§Abstract (~350 kelime: problem → approach → evidence)
§1 Introduction (1.1 Problem / 1.2 Approach / 1.3 Contributions — 4 madde)
§2 Motivating Example (uçtan uca zincir — "sentence never becomes a task by itself")
§3 Genesis Ontology (INV-C1..C8, 4-column table)
§4 Predicate Lowering (INV-P1 — "a rule is not a predicate")
§5 Cross-Family Translation (INV-P3 — "translation preserves candidate meaning")
§6 Binding & Task Genesis (INV-P2 — 3-kapılı API, 2 token)
§7 Verification Evidence (7.1-7.5, evidence strata tablosu)
§8 Related Work (~6: requirements traceability, CNL, GraphRAG, program analysis, AI agents, P1+P2)
§9 Discussion (ontological binding vs embedding, determinism-first, "storage is not epistemology")
§10 Threats to Validity (self-authored gold standard, keyword matcher placeholder, stub provider)
§11 Future Work (Faz 6/7, gerçek bridge, persistence)
§12 Conclusion ("Words do not mutate project reality.")
Appendix A: Golden Fixtures
Appendix B: End-to-End Binding-Chain Replay
References (~12-15, numeric [N])
```

### Review düzeltmeleri (uygulanacak)

- **D1-1:** Evidence freeze yazımdan önce (Aşama 1 tamam → devam)
- **D1-2:** Held-out/adversarial set + self-authored gold standard threat §10
- **D1-3:** 11 Paper 3'e özgü invariant (18 değil, kümülatif 18 bağlam olarak)
- **D1-5:** Contribution ↔ bölüm sırası uyumlu, terminoloji eşlemesi girişte
- **D2-1:** "anti-RAG" → "embedding-first anchoring eleştirisi"
- **D2-2:** "precision/recall" → "golden fixture conformance"
- **D2-3:** §4 CrossFamilyHint kısa, §5 asıl epistemik argüman
- **D2-4:** §7 = "Verification Evidence"
- **D2-5:** KuzuDB story discussion'da kısa ("Storage is not epistemology")
- **D2-6:** Abstract ~350 kelime hedef

---

## Önemli dosyalar

| Dosya | Açıklama |
|---|---|
| `docs/paper3-notes/HANDOFF.md` | **BU DOSYA** — handoff notu |
| `docs/paper3-notes/evidence/e2e-binding-chain-replay.json` | Aşama 1a: uçtan uca zincir JSON |
| `crates/osp-core/tests/paper3_evidence.rs` | E2E evidence generator test |
| `docs/concept-anchoring-design.md` | Paper 3 tasarım dokümanı (Türkçe, v0.2.1+) |
| `docs/paper2-draft-v1.md` | Paper 2 draft (template — yapısal pattern) |
| `docs/paper3-notes/README.md` | Faz tablosu + stage notları indeksi |
| `docs/STATUS.md` | Proje durumu (güncellendi) |

## Commit durumu

`docs/paper3-draft` dalında **commit'lenmemiş değişiklikler var:**
- `crates/osp-core/tests/paper3_evidence.rs` (yeni — evidence generator)
- `docs/paper3-notes/evidence/e2e-binding-chain-replay.json` (yeni — evidence freeze)
- `docs/STATUS.md` (güncellendi — Paper 3 bölümü)
- `docs/paper3-notes/HANDOFF.md` (yeni — bu dosya)

**Yeni oturumda ilk iş:** bu değişiklikleri commit'le, sonra Aşama 1b'den devam et.

## Review notu

Kullanıcı yeni oturumda **Paper 3 plan review sonuçlarını** iletecek. Bu review'lar zaten plan
revizyonlarına yansıdı (yukarıdaki "Review düzeltmeleri"). Yeni review gelirse uygula, sonra
Aşama 1b'den devam et.
