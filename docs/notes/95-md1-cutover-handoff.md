# Handoff — #95 MD-1 Implementation (Subject-authority caller migration, Faz 8a)

**Tarih:** 2026-08-17
**Önceki oturum:** clippy serisi (66→0 + CI hardening) → dogfooding → analyze edges → #92 characterization
**Bu dosya:** Yeni oturumda #95 ile devam etmek için tüm bağlam.

## Oturum nasıl başlamalı

Kullanıcı bu mesajı iletecek:

> Handoff: #95 MD-1 (Faz 8a) ile devam ediyoruz.
> Notlar: `docs/notes/95-md1-cutover-handoff.md` — önce onu oku, durum kontrolü yap, plan moduna gir.

**İlk adımlar:** (1) bu dosyayı oku, (2) `git status` + `git log --oneline -5` ile main durumunu doğrula,
(3) `gh issue view 95` ile issue'nun güncel halini gör, (4) plan moduna girip MD-1 planını hazırla.

## Mevcut durum (bu oturumun sonunda)

- **Main:** `f05ed5a` (#120 merge) — tüm CI yeşil (fmt + clippy -D warnings gerçek gate, #116'dan beri)
- **#92 CLOSED** (PR #120, 3 review turu): subject→raw→theta downstream karakterizasyonu tamamlandı
  - Kanıt: `different subject → different raw + same context → different theta, AMA theta ≠ zorunlu verdict`
  - Karar yüzeyi: `NoDrift` (Passed/Passed, Completed, Mainline, Held — exact snapshot pinli)
- **Clippy:** workspace `--all-targets` tamamen temiz (PR #108/#112/#114/#115/#116 serisi)
- **Dogfooding:** `docs/notes/dogfood-self-analysis.md` (PR #118) + analyze `edges` array (PR #119)

## #95 MD-1 nedir (issue'dan özet)

**Normative rule (accepted):** Task-bound measurement subject authority = `task.predicate.scope`
(canonical). Caller-declared `affected_nodes` measurement authority DEĞİL; yalnız impact hint
veya compatibility observation.

### Implementation scope (issue'nun 2 parçası)

**P2-1 (additive, Faz 5):**
- Canonical task-derived subject üretimi (`measure_task_delta` zaten task scope kullanıyor)
- Compatibility producer: legacy `affected_nodes` ölçen ayrı explicit producer (subject override DEĞİL)
- `SubjectAuthorityDriftObservation`: V1/V2 subject digest + raw bits + theta + Q5 verdict + predicate
  result + decision
- P2-1 caller davranışı değişmez — additive observation

**Faz 8a caller cutover:**
- Caller migration: navigator/MCP `affected_nodes` → impact hint/telemetry (subject authority değil)
- Compatibility yüzeyi kaldırma
- Case 2/3 expected semantic-change regolden (eski golden + yeni golden + reason note)
- Downstream Q5/predicate güncellemeleri

### Cutover acceptance criteria (#92 kanıtı SONRASI — karşılandı)

- Caller cutover sonrası yalnız task scope (Yol 2) mutation authority
- Eski + yeni golden'lar tarihsel korunmuş
- Q5/predicate/policy değişiklikleri sınıflandırılmış (drift türü: NoDrift / SourceLabelOnly /
  PredicateResultDrift / PolicyDecisionDrift / MutationDecisionDrift)
- Compatibility code tamamen kaldırılmış
- **Tüm driftler açıklanabilir + subject-set farkına bağlanabilir; sessiz/tesadüfi context drift yok**

## #92'den devralınan hazır varlıklar (MD-1'de kullanılacak)

1. **`derive_v1_legacy_measurement_subject(&proposal)`** (navigator.rs, pub(crate)) — V1 subject
   tek truth. Kontrat: ordered legacy union (affected sırası korunur; unseen removed_edges.from
   proposal sırasıyla append; HashSet/sort YOK — aggregation sırası rounding bitleri etkiler).
   Dual pinning: navigator unit test + integration mirror bağımsız pinli.

2. **Frozen corpus 001+002 family'leri** (tests/data/faz8_p2_characterization, 18 case):
   - 001: base (Q5 kapalı — GlobalDefault pre-theta reject)
   - 002: izole role-bearing node (theta yüzeyi açık; `raw(002)==raw(001)` bit-exact — intervention
     purity kanıtlı, `variant_002_raw_purity_matches_001_frozen_corpus` integration test'i)

3. **`case_by_id(id)`** (tests/common) — uniqueness guard'lı resolution (0→panic, 2+→duplicate panic).
   Class bazlı `.find()` KULLANMA (002 family'ler eklenince ambiguous).

4. **Exact downstream snapshot pattern'i** — `q92_assert_exact_downstream_snapshot` + `Option<DecisionDriftClass>`
   (yalnız BothReached'te sınıflandır; stopped → None). Karar yüzeyi literal pinlenir, parity
   türetilmez.

5. **Corpus ritual:** builder yaz → `build_all_cases()` ekle →
   `cargo test -p osp-core --test faz8_p2_manifest_bootstrap print_case_digests -- --nocapture`
   (digest'ler) + `print_sidecar` (cases.blake3) → cases.json'a yapıştır + divergence_classes
   açıklamalarını family-kapsayıcı güncelle → guard testler yeşil olmalı.

6. **Engine-unit Q5 yardımcıları** (engine.rs #[cfg(test)] Q5 section):
   `observe_q5_theta`, `q92_observe_divergent_pair` (gerçek DeltaProposal →
   build_claim_from_proposal → node_from_spec production yolu!), `q92_assert_context_identity`
   (effective_vision_bits DAHİL), `q92_001_raws`.

7. **#92 kanıt değerleri** (docs/notes/faz8-p2-parity-characterization.md #92 section):
   wide θ 0.15249/0.11711, removed θ 0.13770/0.11711, Passed/Passed, karar NoDrift.

## MD-1 planında dikkat edilecekler (önceki oturumların review dersleri)

- **Plan review disiplini yüksek:** Bu projede planlar 3-5 review turu geçiyor. MD-1 planını
  v1'den başlat, kullanıcı review'ları paste'liyor. Bilinen hassas noktalar:
  - "production kod değişikliği yok" vs helper paylaşımı çelişkisi → "production SEMANTIC
    değişikliği yok" de; refactor'ları açıkça işaretle
  - Elle fixture/subject sabiti YAZMA — production helper'dan türet
  - Exact snapshot dondur; "eşitlik" assert etme (NotCompleted==NotCompleted tuzağı)
  - `Option<...>` sınıflandırma: ulaşılmayan yüzeyi sınıflandırma
  - Debug-string equality değil enum equality
  - Doğrulama komutlarında `||` fallback / `2>/dev/null` YOK (failure masking)
  - Corpus'a case eklerken: ID bazlı resolve, class find değil
- **`evaluate_v2_candidate_case` subject observation'ı** (tests/common) heterogeneous/Module
  scope'ları sessiz düzleştirir — Node/Subgraph fixture'ları kullan, Module scope'dan kaçın.
- **CI komutu:** `cargo clippy --locked --workspace --all-targets --all-features --exclude osp-desktop -- -D warnings`
- **`index.scip`** (16.4 MB dogfooding SCIP index'i) repo kökünde lokal olarak durabilir —
  .gitignore'da zaten. Yeniden üretmek: rust-toolchain.toml'u geçici taşı +
  `MSYS_NO_PATHCONV=1 docker run --rm -v "P:/Work/SoftwarePhysics:/repo" -w /repo
  sourcegraph/scip-rust:latest scip-rust --output /repo/index.scip` + pin'i geri getir.

## Ortam notları (Windows)

- Shell her sıfırlanmada `export PATH="$HOME/.cargo/bin:$PATH"` gerekebilir (cargo bulunamıyor hatası)
- `grep` ZCode function'ı — `-oE` çakışıyor; `command grep` kullan veya awk
- `python` yok; JSON analizi için `node -e` kullan (C:/Users/ervol/AppData/Local/Temp path'leri
  Windows formatında ver)
- Kullanıcının untracked kişisel notları var (docs/notes/planlama-tasarım-eskiz.txt, proje-adaylari.md,
  sohbet-konu.txt, docs/osp-*.md, docs/design/, crates/osp-analyzer/tests/declaration_policy_characterization.rs,
  crates/osp-desktop/gen/) — **ASLA `git add -A` kullanma; her zaman dosya bazlı add**
  (bu oturumda bir kere yanlışlıkla eklendi, soft reset ile kurtarıldı)

## Sıradaki iş önerisi (yeni oturumda)

1. **#95 MD-1** — P2-1 additive observation ile başla (SubjectAuthorityDriftObservation),
   sonra Faz 8a caller cutover. Bağlı: #96 (MD-2) #95 sonrası, #100/#99/#103 #96 sonrası.
2. Alternatif küçük iş: #110 MSRV (tech-debt; incompatible_msrv allow'ları buna bağlı)

## Açık issue snapshot (2026-08-17)

#95 (MD-1, sıradaki), #96 (MD-2), #97 (MD-3), #99, #100, #103, #110 (MSRV),
#72-#79 (INV-T9 ileri tasarım), #86, #89 (karakterizasyon backlog'ları)
