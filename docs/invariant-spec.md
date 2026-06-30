# OSP Invariant Specification — Formal Epistemological Contracts

> **Durum:** v0.1 — Aşama A kodlamadan ÖNCE formal kesinlik (review 1 önerisi)
> **Tarih:** 2026-06-30
> **Amaç:** Kod yazarken epistemolojik sınırların bulanıklaşmasını önlemek.
> En büyük tehlike (review 1): feature eklendikçe invariant'ların erozyona uğraması.
> Bu spec her invariant için **yapısal garanti** (type-level) + **test** + **ihlal örneği** tanımlar.

## İlişkili Dokümanlar
- `docs/agent-trajectory-roadmap.md` — INV-T1..T7'nin ontolojik bağlamı
- `docs/paper-draft-v2.6.md` — INV #1..#15'in paper karşılığı
- Kod: `crates/osp-core/src/*.rs` — type-level enforcement

---

## Bölüm A — Mevcut 15 Invariant (Paper 1, statik uzay)

Bu invariant'lar zaten implemente edildi, type-level enforced, test edildi.
Aşama A kodlaması bunları **bozmamalı** — her yeni tip bu listeye uyum kontrolünden geçer.

### INV #1 — Author-witness rejection
**Tanım:** Bir Claim'in yazarı, kendi claim'inin şahidi (approver) olamaz.
**Yapısal garanti:** `CanonicalWitnessSet::canonicalize_for(author)` author'ı çıkarır.
Agent'ın kendi çalışmasını onaylaması ontolojik olarak imkânsız (typle-level, runtime değil).
**Test:** `author_excluded_from_approvers`.
**İhlal örneği:** Agent kendi PR'ini onaylar → self-dealing.

### INV #2 — EvidenceEvent dedup
**Tanım:** Kanıt dedup birimi `(source, actor, claim)` üçlüsüdür.
**Yapısal garanti:** `EvidenceEvent` dedup key bu üçlü; HashSet ile otomatik.
**Test:** `duplicate_evidence_collapsed`.
**İhlal örneği:** Aynı CI run iki kez sayılır → quorum şişer.

### INV #3 — Tri-state witness classification
**Tanım:** Bir Claim'in ontolojik durumu 3'ten fazladır değil: Unobservable-locally /
Unobservable-globally / Observable. Negative claim yok (placeholder → "bilmiyoruz").
**Yapısal garanti:** `WitnessStatus` enum (3 variant).
**Test:** `placeholder_is_not_negative`.
**İhlal örneği:** "Bu repo test yok" (negative) yerine "locally unobservable" (epistemik).

### INV #4 — RawPosition/DerivedPosition separation
**Tanım:** Raw pozisyon (x/y/z/w/v) θ hesabına **girdi**; Derived (u/θ/risk) **çıktı**.
Dairesellik yok. Agent pozisyon declare edemez — engine compute eder.
**Yapısal garanti:** `Position { raw, derived }` ayrık tipler; `compute_derived` tek yönlü.
**Test:** `derived_not_in_theta_input`.
**İhlal örneği:** Agent "coupling 0.3" der, engine kabul eder → hallucination.

### INV #5 — Lazy diffusion
**Tanım:** Diffusion hesabı lazy ( talep edilince), eager değil.
**Yapısal garanti:** `DiffusionDeviation` stub, lazy evaluation.
**Test:** (stub — Faz 6+).
**İhlal örneği:** Her commit'te tam diffusion → O(n²) maliyet.

### INV #6 — Incremental space commit
**Tanım:** Space mutation incremental (apply_delta), full rebuild değil.
**Yapısal garanti:** `apply_delta` mutation-only, infallible.
**Test:** `apply_delta_incremental`.
**İhlal örneği:** Her commit'te space sıfırdan kurulur → event-sourcing bozulur.

### INV #7 — Admin override flag
**Yapısal garanti:** `WitnessResult::Override` admin flag ile.
**Test:** `admin_override_bypasses_quorum`.
**İhlal örneği:** Admin quorum'u manuel skip eder → denetimsiz.

### INV #8 — Network-free core
**Tanım:** osp-core ağ/IO bağımlılığı yok (pure computation).
**Yapısal garanti:** `Cargo.toml` — no network crates.
**Test:** `cargo tree` ile network crate yokluğu.
**İhlal örneği:** core'a HTTP client eklenir → determinizm kaybı.

### INV #9 — WitnessSet-based operator
**Tanım:** W(C, Ω) pure fonksiyon — Ω `WitnessSet`, mutasyon yok.
**Yapısal garanti:** `evaluate(claim, omega) -> WitnessResult` (no &mut).
**Test:** `evaluate_is_pure`.
**İhlal örneği:** evaluate space'i mutate eder → side effect.

### INV #10 — Pure Instability axis
**Tanım:** z (instability) Martin I saf — D (main-seq distance) ayrı metric, z'ye gömülü değil.
**Yapısal garanti:** `DerivedPosition.main_sequence_distance` ayrı alan.
**Test:** `instability_not_conflated_with_d`.
**İhlal örneği:** D, z'ye gömülür → Martin teorisi bozulur.

### INV #11 — LLM stateless
**Tanım:** osp-llm-runtime agent state tutmaz (stateless HTTP).
**Yapısal garanti:** `complete()` tek istek, no session.
**Test:** `runtime_stateless`.
**İhlal örneği:** Runtime conversation history tutar → context drift.

### INV #12 — OutputContract deterministic reject
**Tanım:** LLM çıktısı şemaya uymazsa Q4'te deterministik reddedilir.
**Yapısal garanti:** `OutputContract::validate()` -> `Result<(), SyntaxViolation>`.
**Test:** `output_contract_rejects_invalid`.
**İhlal örneği:** Malformed JSON "best-effort" parse edilir → hallucination.

### INV #13 — PermissionMask trusted-operator assigned
**Tanım:** PermissionMask agent tarafından alınmaz, trusted operator atar.
**Yapısal garanti:** `PermissionMask` constructor operator-only.
**Test:** `permission_operator_assigned`.
**İhlal örneği:** Agent kendi permission'ını yükseltir → privilege escalation.

### INV #14 — Prompt as typed data packet
**Tanım:** Prompt doğal dil string değil, typed `OspPrompt` data packet.
**Yapısal garanti:** `OspPrompt` struct, serde serialize.
**Test:** `prompt_typed_not_string`.
**İhlal örneği:** Prompt string concat → injection, drift.

### INV #15 — Custom axis registration trusted-operator only
**Tanım:** Agent yeni axis tanımlayamaz, sadece trusted operator.
**Yapısal garanti:** `Axis` registration operator-only API.
**Test:** `agent_cannot_register_axis`.
**İhlal örneği:** Agent "vibes" axis ekler → "software physics" bozulur.

---

## Bölüm B — Yeni 7 Trajectory Invariant (Paper 2, dinamik katman)

Bu invariant'lar Aşama A'da implement edilecek. INV #1..#15 ile **çelişmez**,
onların üzerine inşa edilir. Her biri yapısal (type-level) garanti ister.

### INV-T1 — Predicate epistemolojisi ( Hibrit modelin kalbi)
**Tanım:** Task, agent'a **predicate** olarak verilir. Koordinat hedefi
(`Milestone.target_region.preferred_vector`, `InternalTaskPlan.milestone_target_vector`)
agent'a serialize edilmez. Sadece `AgentTaskView` (koordinatsız) agent'a gider.
**Yapısal garanti:** `AgentTaskView` serde struct'ında koordinat alanı YOK.
`InternalTaskPlan` ve `AgentTaskView` ayrık tipler; dönüşüm tek yönlü (engine→agent).
**Test:** `agent_task_view_has_no_coordinate_fields` — serde çıktısında "vector"/"target_raw"
string geçmemeli.
**İhlal örneği:** Agent "hedef coupling 0.55" koordinatını görür → "AI söylediği için
doğru" — INV #4 (engine ölçer) erozyona uğrar.
**Koruma mekanizması:** `serialize_agent_view()` fonksiyonu — sadece AgentTaskView'ı
serialize eder, InternalTaskPlan'ı reddeder (compile-time: farklı tip).

### INV-T2 — Operator tanımlar hedef (Genesis)
**Tanım:** `Trajectory` ve `Milestone.target_region` trusted operator tarafından
tanımlanır (INV #13, #15 ile uyumlu). Agent hedef belirlemez; sadece oraya giden
structural change (DeltaProposal) üretir.
**Yapısal garanti:** `Trajectory::new()` constructor operator PermissionMask gerektirir.
**Test:** `trajectory_requires_operator_permission` — agent PermissionMask ile
Trajectory::new() çağırırsa compile/runtime reject.
**İhlal örneği:** Agent PRD okuyup kendi Trajectory'sini yaratır → halüsinasyon kaskadı
(review 3 — Seçenek B reddedildi).
**Koruma mekanizması:** Seçenek A (insan mimar / God Mode) — kodlamada bu.

### INV-T3 — Engine ölçer (korunmuş, INV #4'ün dinamik uzantısı)
**Tanım:** Task predicate'i **engine-measured** değer üzerinde değerlendirilir
(`claim.computed_raw`). Agent ölçmez; engine, DeltaProposal'ı apply_delta ile
uygulayıp re-analyze eder, P_raw'ı ölçer.
**Yapısal garanti:** `MetricPredicate::evaluate(raw: &RawPosition)` — input engine'dan,
agent değiştiremez. Predicate `Claim.computed_raw`'ı okur, agent'ın PositionHint'ini değil.
**Test:** `predicate_uses_computed_raw_not_hint` — PositionHint ile predicate geçse bile
computed_raw ile fail etmeli.
**İhlal örneği:** Agent "coupling 0.4 oldu" der, predicate bunu kabul eder → INV #4 ihlali.
**Koruma mekanizması:** `check_claim_predicate(claim, task)` — claim.computed_raw zorunlu.

### INV-T4 — Predicate provenance
**Tanım:** MetricPredicate `required_source` ile "measured/scip" zorunlu kılabilir.
Placeholder/heuristic kaynaklı ölçümlerle task kapatılamaz (epistemolojik bütünlük).
**Yapısal garanti:** `MetricValue.source` enum; predicate evaluate source'u kontrol eder.
**Test:** `placeholder_metric_cannot_close_task` — source=Placeholder ile predicate satisfied
olsa bile task Done olmaz.
**İhlal örneği:** "Coupling ölçülmedi (placeholder 0.5) ama 0.55'in altında" → task kapanır →
ölçülmemiş başarı iddiası.

### INV-T5 — Task ≠ Claim
**Tanım:** Task bir **şart** (predicate), Claim bir **iş** (structural delta).
Bir task birden fazla claim/attempt gerektirebilir (TaskAttempt); bir claim bir task'a
hizmet eder (Claim.task_id).
**Yapısal garanti:** `Claim.task_id: TaskId` (opsiyonel değil, required); `Task.attempts`
Vec<TaskAttempt>.
**Test:** `one_task_many_attempts` — 3 attempt senaryosu, hepsi aynı task_id.
**İhlal örneği:** Task = Claim → bir reddedilen claim task'ı öldürür → katı sistem.

### INV-T6 — Failure ≠ regression (review 1)
**Tanım:** Predicate failure negative progress *gerektirmez*. Bir DeltaProposal
predicate'i sağlamasa bile milestone'a yaklaşmış olabilir
(`PredicateGateResult::UnsatisfiedButImproved`). OSP completion (predicate satisfied)
ile directional improvement'ı ayırır.
**Yapısal garanti:** `PredicateGateResult` enum — 3 variant (Satisfied/Improved/Regressed);
`distance` alanları ile quantitative.
**Test:** `improved_not_treated_as_failure` — 0.82→0.71 (target 0.55) → Improved,
attempt kaydedilir, task açık kalır ama progress sinyali.
**İhlal örneği:** Her reject "başarısız" sayılır → uzun refactor task'larında agent
ödüllendirilmez, sürekli reject → token patlaması (F6).

### INV-T7 — Maneuver limit (review 3)
**Tanım:** Bir Task için ardışık N (default 5, operator-configurable) reddedilen attempt
sonra sistem **Trajectory Deviation Alert** üretir ve operatöre (God Mode) kontrol devreder.
Sonsuz context-loop ve token patlaması önlenir.
**Yapısal garanti:** `Task.attempts` sayacı + `TrajectoryDeviationAlert` event;
`ManeuverLimit { max_attempts }` operator config.
**Test:** `maneuver_limit_triggers_alert` — 5 reject sonra alert, 6. attempt blocked.
**İhlal örneği:** Agent 50 kez dener, token $50 harcar, hiç ilerlemez → kaynak israfı.
**Koruma mekanizması:** N aşıldığında task `Blocked` → operator replan veya N'i artır.

---

## Bölüm C — İnvariant Çapraz Kontrol Matrisi

Yeni invariant'ların mevcutlarla **çelişmediğinin** formal doğrulanması.

| Yeni | Etkileşim | Mevcut | Uyum? | Not |
|---|---|---|---|---|
| INV-T1 | genişletir | INV #4 | ✅ | INV #4 raw position; INV-T1 task predicate'inin agent'a koordinat sızdırmaması. |
| INV-T1 | genişletir | INV #14 | ✅ | INV #14 prompt typed; INV-T1 AgentTaskView typed + koordinatsız. |
| INV-T2 | genişletir | INV #13 | ✅ | INV #13 permission operator; INV-T2 trajectory operator. |
| INV-T2 | genişletir | INV #15 | ✅ | INV #15 axis operator; INV-T2 milestone region operator. |
| INV-T3 | genişletir | INV #4 | ✅ | INV #4 engine compute raw; INV-T3 predicate computed_raw'ı okur. |
| INV-T3 | bağımsız | INV #12 | ✅ | INV #12 Q4 syntax; INV-T3 Q5.b predicate — farklı gate'ler. |
| INV-T4 | genişletir | INV #3 | ✅ | INV #3 tri-state; INV-T4 placeholder metric task kapatmaz. |
| INV-T5 | yeni | — | ✅ | Task≠Claim ontolojik ayrım. |
| INV-T6 | genişletir | INV #9 | ✅ | INV #9 evaluate pure; INV-T6 PredicateGateResult pure çıktı. |
| INV-T7 | yeni | INV #7 | ✅ | INV #7 admin override; INV-T7 maneuver limit → admin devralma. |

**Çatışma yok.** Her yeni invariant mevcutları genişletir veya bağımsız ekler.

---

## Bölüm D — İnvariant Erozyonuna Karşı Savunma

Review 1'in ana uyarısı: *"feature eklendikçe epistemolojik sınırlar bulanıklaşır."*
Savunma mekanizmaları:

### D1 — Type-level enforcement (compile-time)
İnvariant'lar runtime check değil, **type-level**. Örnek: `AgentTaskView`'da koordinat
alanı yoksa, serde derive ile agent'a koordinat serialize etmek compile error.
Runtime'da "unutulamaz."

### D2 — Test matrisi
Her invariant için bir test (`*_invariant` isim şablonu). CI'da çalışır. İnvariant
erozyona uğrarsa test fail. Hedef: `crates/osp-core/tests/invariants.rs` (Aşama A).

### D3 — Review checklist (her PR)
Her PR bu spec'in bir invariant'ına dokunuyorsa, PR açıklamasında:
- Hangi invariant?
- Nasıl korunduğu (type/test)?
- Çapraz kontrol matrisi etkileniyor mu?

### D4 — İnvariant spec versiyonlama
Bu doküman versioned. İnvariant eklenirse/değişirse, bu spec güncellenir + paper
karşılığı. "Sessiz erozyon" önlenir.

---

## Sonraki Adım

Bu spec + `agent-trajectory-roadmap.md` oturunca:
1. Aşama A (ontolojik tipler) — bu spec'teki invariant'ları type-level enforce eden tipler.
2. `crates/osp-core/tests/invariants.rs` — D2 test matrisi.
3. Her aşamada spec'e uyum kontrolü.

---

*Bu doküman review 1'in "formal invariant spec" önerisi üzerine kuruldu.
Kaynak: INV #1..#15 (paper-draft-v2.6.md), INV-T1..T7 (agent-trajectory-roadmap.md + 3 review).*
