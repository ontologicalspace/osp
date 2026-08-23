# Handoff — #95-A MD-1 Subject Cutover (TAMAMLANDI — merge pending; sıradaki #95-B)

**Tarih:** 2026-08-23. Branch: `feat/95a-md1-subject-cutover` (origin/main `158eb79`).
**Durum:** #95-A implementasyonu TAMAMLANDI — workspace 41 suite yeşil (fmt + clippy
`-D warnings` + test). PR açıldı; review sonrası merge → **#95-B** (MD-1 cleanup).

## Oturum nasıl başlamalı

> Handoff: **#95 — MD-1: #95-A PR review/merge sonrası #95-B cleanup** (observer +
> SubjectAuthorityDriftObservation + legacy subject producer + MD-1 sidecar alanları +
> legacy fiziksel isimler; kalır: uniform-Scip reference #96'nın evi, MD-3 yüzeyleri).
> Branch: main üzerinden yeni branch.
> Notlar: `docs/notes/96-implementation-handoff.md` (bu dosya — #96/#95-A zinciri) +
> `docs/notes/faz8-p2-migration-decisions.md` MD-1 "#95-A implementation" bölümü.

**İlk adımlar:** (1) bu dosya, (2) `git log --oneline -4`, (3)
`export PATH="$HOME/.cargo/bin:$PATH"` + `cargo test --locked --workspace
--all-features --exclude osp-desktop` (yeşil başlangıç).

## #95-A teslim edilenler (kritik zincir)

```text
draft.try_new(proposal, raw, task, …)  — Q4 structural → canonical_task_subject_scope(task) capture
measure_attempt_native_with_md1_shadow(draft, task)  — proposal param YOK (capability reduction)
  TEK session: authority = shadow = canonical task scope; native provenance sabit (#96)
finalize  — draft scope ↔ token scope (LegacySubjectBindingMismatch; adı #95-B'ye kadar legacy)
commit_task_claim
  resolve → validate_for_commit
  → canonical_task_subject_scope(current): Err→Derivation(SubjectDerivationFailed); ≠token→TaskSubjectBindingMismatch
  → #96 5-fence (DOKUNULMADI) → Q5 → PredicateGate → Q6 → witness
```

- **P0 fence** (tur-2): registry-overwrite negatif e2e ×2 (scope drift + heterojen
  derivation) — Q5/witness/mutation'a ulaşmaz.
- **Affected-irrelevance executable theorem** (tur-3 P1): full-path metamorphic —
  Δaffected → subject/bits/sources/digests/finalize/gate/decision/basis.measured invariant.
- **Dogfood Run C** (divergent fixture — scope [3] vs affected [3,2]): envelope
  `task_scope`; authority subject [3] ≠ legacy [3,2]; MD-1 lane parity; MD-2 observer
  intentional divergence intact; Completed.
- Regolden (eski+yeni+reason `subject-cutover (#95-A)`): 002 cross-pin'leri → parity;
  finalize mix-negatifleri → yeni pozitif/negatif; module-scope → draft terminal;
  CLI/LLM/parity/MCP pin'leri.
- Fiziksel legacy isimler (`NativeLegacySubjectMeasurement`, `legacy_subject_ids`,
  `LegacySubjectBindingDigest/Mismatch`) BILİNÇLİ kaldı — doc truth-surface
  pre/post-#95-A tablosuyla; yeniden adlandırma #95-B.

## #95-B scope (sıradaki)

Silinir: `subject_authority.rs` observer + `SubjectAuthorityDriftObservation` +
`produce_legacy_subject_measurement`/`derive_v1_legacy_measurement_subject` +
`effective_legacy_measure_set` + MD-1 sidecar alanları (wire üç-epoch typed rejection
yalnız durable wires) + MD-1 comparison testleri + legacy fiziksel isimler.
Kalır: uniform-Scip reference projection (#96'nın evi — #100), `compute_raw_from_delta`
(#100 machinery), MD-3 yüzeyleri (#97). "Compatibility code tamamen kaldırılmış"
kriteri **MD-1 compatibility code** olarak okunur.

## Sıra

**#95-B** → #97 (MD-3) → #100 (Faz 8a engine cutover — MD-2 fiziksel kaldırım dahil).

## Ortam notları (Windows)

- `export PATH="$HOME/.cargo/bin:$PATH"`; `command grep`; `node -e` (python yok)
- **ASLA `git add -A`**; commit formatı `feat: #95 …` (scope parens YOK)
- Dogfood fixture: `C:/Users/ervol/AppData/Local/Temp/osp-95a-runc/` (disposable)
- CI parity: fmt --check; clippy --locked --workspace --all-targets --all-features
  --exclude osp-desktop -- -D warnings; test aynı target seti
