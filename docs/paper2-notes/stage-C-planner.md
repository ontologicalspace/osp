# Stage C — Planner / Milestone Decomposition Notes (Paper 2 evidence)

> **Aşama:** C (Planner / Milestone Decomposition) — TAMAMLANDI
> **Tarih:** 2026-06-30
> **Tez:** "Milestone decomposition planner/operator seviyesinde yapılır. Planner,
> milestone scope ve loss dağılımına göre deterministic decomposition policies ile
> 1 veya daha fazla Task üretir. Agent decomposition yapmaz."
> **Testler:** 9 yeni test, osp-core 256→265, workspace 461→470
> **Spec:** `docs/roadmap/paper2-roadmap.md` §8 Aşama C

---

## Karar 1: Planner decides — deterministic decomposition policies

**Karar:** `DecompositionStrategy` enum (OneTask / SplitByNodeTopK / SplitByRole /
SplitByAxis). Planner seçer; agent görmez/yapamaz (INV-T2).
**Gerekçe:** Saf 1:1 fazla kaba (geniş task → rastgele refactor → token patlaması).
Saf 1:N (her node) task patlaması. En doğru: scope + loss + role'a göre deterministic.
**Kanıt:** `decompose_milestone(milestone, space, policy, strategy, cap)` — deterministic
(same input → same labels, test 8). max_tasks_per_milestone enforced (test 4).
**Paper materyali:** §2 Task dematerialization — "Deterministic decomposition strategies."
RQ adayı: hangi strateji daha az token/attempt?

---

## Karar 2: Intent.target_raw = preferred_vector (INV-T1)

**Karar:** `Intent::from_task(agent, plan)` → target_raw = plan.milestone_target_vector
(preferred_vector). Internal-only, agent'a serialize edilmez.
**Gerekçe:** preferred_vector zaten planner/operator seviyesinde tanımlı navigasyon merkezi.
Predicate'lerden nominal coordinate icat etme (coupling≤0.55 → 0.55) zayıf — 0.55 sınır,
hedef değil. Multi-axis'te bulanıklaşır.
**Kanıt:** `intent_from_task_uses_preferred_vector` (test 6) — target_raw == milestone_target_vector.
AgentTaskView ayrık (INV-T1, Aşama A/B).
**Paper materyali:** §1 ontology — "Intent internal-only; agent sees predicate not coordinate."

---

## Karar 3: Milestone başarı ≠ tüm Task'lar Done

**Karar:** `Milestone::is_achieved(measured)` = TargetRegion.predicates satisfied
(engine-measured). Task'lar Done olmasa bile milestone achieved olabilir.
**Gerekçe:** Task'lar araç; asıl otorite engine measurement. Task fail edebilir ama
başka bir task/katkı milestone region'ı tatmin edebilir.
**Kanıt:** `milestone_achieved_when_region_satisfied_not_all_tasks_done` (test 7).
**Paper materyali:** §3 Adaptive control loop — "Milestone completion = region, not task list."

---

## Karar 4: DecompositionSpace minimal (Aşama D'de engine feeding)

**Karar:** DecompositionSpace { nodes: Vec<DecompositionNode>, preferred_vector }.
DecompositionNode { id, role, measured, loss_contribution }. Aşama C'de test fixture;
Aşama D'de engine'den beslenir.
**Gerekçe:** Planner engine'e doğrudan bağlı değil — space snapshot alır (decoupling).
**Kanıt:** top_offenders() deterministic (loss desc, id asc tie-break). by_role() grouping.
**Paper materyali:** §2 — "DecompositionSpace as planner input (decoupled from engine)."

---

## Kanıt: 9 done-criteria test — hepsi geçti

| # | Test | Sonuç |
|---|---|---|
| 1 | one_task_decomposition_for_single_node_scope | ✅ tek task |
| 2 | split_by_node_topk_produces_offender_tasks | ✅ offender + cleanup |
| 3 | split_by_role_groups_by_architectural_role | ✅ 3 role → 3 task |
| 4 | max_tasks_per_milestone_enforced | ✅ cap enforced |
| 5 | decomposer_requires_operator_capability (INV-T2) | ✅ capability zorunlu |
| 6 | intent_from_task_uses_preferred_vector (INV-T1) | ✅ preferred_vector |
| 7 | milestone_achieved_when_region_satisfied_not_all_tasks_done | ✅ region-based |
| 8 | decomposition_deterministic_same_input_same_output | ✅ deterministic labels |
| 9 | aggregate_cleanup_task_for_remaining_nodes | ✅ cleanup task |

---

## Aşama C'de YAPILMAYAN (Aşama D+)

- **Agent Navigator loop** (DeltaProposal → Claim → gate → trajectory update) — Aşama D
- **Real engine feeding** (DecompositionSpace engine'den beslenir) — Aşama D
- **Multi-task scheduling / dependency graph** — Aşama D+
- **Trajectory correction** (commit sonrası replan) — Aşama E
- **WeightedPredicate full loss** (F5 tam impl) — Aşama D
- **OperationPolicy** (scope + max_delta) — Aşama D+

---

## Test özeti (Aşama C)

osp-core: 256 → **265 test** (+9), workspace 461 → **470**, -D warnings temiz, fmt temiz.

---

*Bu not `docs/paper2-notes/README.md` disiplinine uyar: karar + gerekçe + kanıt + edge case + paper materyali.*
