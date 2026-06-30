# Paper 2 Notes — Architectural Trajectory Navigation

> **Amaç:** Paper 2 ("Architectural Trajectory Navigation") için kanıt toplama.
> **Disiplin:** Paper en son yazılır. Bu dizin, implementasyon sırasında ortaya
> çıkan kararları, ölçümleri, edge case'leri toplar. Paper 2 bu notlardan
> **data-driven** yazılır (iddia değil, kanıt).
> **İlişki:** `docs/agent-trajectory-roadmap.md` (omurga) → bu dizin (kanıtlar)

## Dizin Yapısı

```
docs/paper2-notes/
  README.md                    ← bu dosya (indeks + disiplin)
  stage-A-ontology.md          — Aşama A: ontolojik kararlar, invariant ispatları
  stage-B-predicate-gate.md    — Aşama B: Q5.b deterministik reddin etkisi
  stage-B2-attempt-ledger.md   — Aşama B2: TaskAttempt evidence, RQ6/RQ7/RQ8 ham veri
  stage-C-planner.md           — Aşama C: task dematerialization, axis oscillation (F5)
  stage-D-agent-loop.md        — Aşama D: token maliyeti, task success, maneuver limit
  stage-X-failures.md          — başarısız denemeler (review 1 — Paper 2 için değerli)
  evidence/                    — ham ölçümler (JSON), corpus sonuçları
```

## İlişkili Formal Spec

`docs/invariant-spec.md` — Aşama A kodlamadan ÖNCE INV-T1..T7 + mevcut INV #1..#15'in
formal tanımı. Her invariant: yapısal garanti (type-level) + test + ihlal örneği.
Review 1'in önerisi: kod yazarken epistemolojik sınırların bulanıklaşmasını önlemek.

## Not Yazma Disiplini

Her implementasyon aşaması bitiminde o aşamanın notu yazılır. Not şunları içerir:

1. **Karar:** Ne karar verildi (örn. "ε tolerance Trajectory parçası başına")
2. **Gerekçe:** Neden (örn. "tek commit'te coupling 0.82→0.55 inmez")
3. **Kanıt:** Ölçüm/test (örn. "svelte corpus'ta StoreRepository 7 commit'te indi")
4. **Edge case:** Karşılaşılan tuaf durum (örn. "multi-axis predicate margin sorunu")
5. **Paper materyali:** Bu not Paper 2'nin hangi bölümüne gider

## Paper 2 Bölüm Haritası (notlar → bölümler)

| Paper 2 bölümü | Not kaynağı |
|---|---|
| §1 Trajectory ontolojisi | stage-A-ontology.md |
| §2 Task dematerialization | stage-C-planner.md |
| §3 Adaptive control loop | stage-D-agent-loop.md |
| §4 Deterministic predicate gating | stage-B-predicate-gate.md |
| §5 Token cost (RQ6) | stage-D-agent-loop.md + evidence/ |
| §6 Task success (RQ7) | stage-D-agent-loop.md + evidence/ |

## Mevcut Durum (2026-06-30)

- **Aşama A-D:** Henüz başlanmadı (roadmap review sonrası)
- **3D viewer:** DURDURULDU (Aşama E için gerekli, ama agent işleri öncelik)
- **Paper 1:** Tamamlandı (statik uzay, kanıtlanmış)

## RQ Adayları (Paper 2 için)

- **RQ6:** OSP trajectory prompt, raw dump'a göre token maliyetini düşürür mü?
  (Paper 1 RQ5'in dinamik uzantısı — prompt-size değil, task-boyunca toplam)
- **RQ7:** Predicate gate'li agent, gatesiz agent'a göre task success'i artırır mı?
  (Kod kalitesi: coupling/cohesion ölçümü)
- **RQ8 (opsiyonel):** Trajectory correction, tek-shot planlamadan daha iyi mi?
  (Adaptive control loop'un değeri)

---

*Bu dizin `docs/agent-trajectory-roadmap.md` §10 ile uyumludur.*
