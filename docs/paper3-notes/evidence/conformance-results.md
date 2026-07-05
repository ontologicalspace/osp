# Paper 3 — Conformance Results (5-state, 18 cümle)

> 13 golden fixture + 5 held-out = **18 cümlenin** sıkı conformance sonuçları.
> 5-state dürüst sınıflandırma (Review 2 v4): `Conform` / `PartialConform` / `KnownLimitation`
> / `RejectAsExpected` / `UnexpectedFailure`. **Non-conform/KnownLimitation çıkan fixture
> SİLİNMEZ** — RQ1 inandırıcılığı o satırlardan gelir.

## Metodoloji

Her cümle Faz 1 deterministic pipeline'ından (`AnchorPipeline::run_with_source` + `apply_plan`)
geçirilir. Expected fixture davranışı (packet_type, edges, anchor_decision, negative_assertions)
gerçek pipeline çıktısıyla karşılaştırılır. Sapma 5-state ile raporlanır:

- **Conform** — tüm expected davranışlar üretildi
- **PartialConform** — bazı expected'lar üretildi, bazıları coarse-grained classifier limitasyonu
- **KnownLimitation** — özellikle limitation göstermek için (negation, semantik false-positive, alias coverage)
- **RejectAsExpected** — bilinçli INV ihlali → GateError bekleniyor (fix_010, fix_011 default context)
- **UnexpectedFailure** — beklenmeyen (bu tabloda sıfır — hepsi açıklandı)

## Golden fixture conformance (13)

| ID | Cümle (kısa) | Pkt | Decision | Conformance | Not |
|---|---|---|---|---|---|
| fix_001 | "Kullanıcı ödeme yaparken..." | UserVision | RequireOperatorReview | **Conform** | DerivesRisk + Mentions üretildi (anchoring_mvp_fix_001_derives_risk) |
| fix_002 | "Domain katmanı...bağımlı olmamalı" | Requirement | RequireOperatorReview | **Conform** | DerivesRule:NoDomainToInfrastructureDependency üretildi |
| fix_003 | "Event Sourcing kararını referans al..." | Decision | TentativeLink | **Conform** | DependsOnDecision (precedence #2) |
| fix_004 | "Controller'larda business logic olmaması gerekiyor" | AntiGoal | RequireOperatorReview | **Conform** | AntiGoalOf (precedence #1) |
| fix_005 | "...varsayılıyor" | Assumption | TentativeLink | **Conform** | Mentions only (precedence #3) |
| fix_006 | "...AuthService module'ünde implement edilmeli" | UserVision | RequireOperatorReview | **PartialConform** | ImplementedBy Faz 4 evidence ister; default context reject (anchoring_mvp_fix_011_implemented_by_rejected_without_provider paralel) — expected edges üretildi ama gate evidence gerektirir |
| fix_007 | "...çelişiyor" | UserVision | MarkContradiction | **PartialConform** | Contradicts üretilirse MarkContradiction + neg assertions; coarse classifier Decision: referansını typed-prefix olarak parse edemeyebilir (Faz 2 calibration) |
| fix_008 | "Sistere yeni bir ödeme (payment)..." | UserVision | TentativeLink | **Conform** | INV-C8 canon gate redirect (ödeme→Payment) |
| fix_009 | "...bildirimler göndermek faydalı olabilir" | UserVision | CreateNode | **Conform** | Mentions:Notification |
| fix_010 | "Belki hafta sonu bazı şeyleri gözden geçirmek lazım" | Assumption | MarkUnanchored | **RejectAsExpected** | Vague → boş candidates (anchoring_mvp_fix_010_unanchored_empty) |
| fix_011 | "CodeEntity:AuthService...implement eder" | UserVision | RequireOperatorReview | **RejectAsExpected** | Default context (provider yok) → ImplementedByRequiresCodeEvidence gate reject (anchoring_mvp_fix_011_implemented_by_rejected_without_provider) |
| fix_012 | "TaskCandidate:AuthServiceRefactor görev olarak..." | UserVision | RequireOperatorReview | **Conform** | DerivesTask (task signal + typed ref, Faz 5a Patch 7) |
| fix_013 | "NoHighCouplingDependency kuralı coupling azaltmalı" | UserVision | RequireOperatorReview | **Conform** | DerivesRule + lowering PredicateStub (INV-P1, Faz 5a) |

## Held-out conformance (5)

| ID | Cümle (kısa) | Pkt | Canonical | Ambiguity | Conformance | Not |
|---|---|---|---|---|---|---|
| held_001 | "Modüller arası bağımlılık azaltılmalı" (TR) | Requirement | ModüllerArasıBağımlılık | Single(Coupling) | **Conform** | TR alias chain (bagiml) + TR rule marker (malı) |
| held_002 | "The couplings in the pipe assembly must not be reused" | Requirement | TheCouplingsIn | Single(Coupling) | **KnownLimitation** | Semantik false-positive: fiziksel boru → YANLIŞ Coupling hint. Matcher alan-ayrımı yapamaz (§10) |
| held_003 | "Coupling rule must not be enforced during tests" | Requirement | CouplingRuleMust | Single(Coupling) | **KnownLimitation** | Negasyon: anlam NEGATİF ama RuleCandidate üretildi. Negation Faz 6 |
| held_004 | "Coupling and cohesion must not diverge" | Requirement | CouplingAndCohesion | Multiple(Coupling,Cohesion) | **Conform** | MultipleCandidates (A6 AxisNotInCandidates ile ortak) |
| held_005 | "Witness count must not create metric evidence" | Requirement | WitnessCountMust | NoAxis (hint None) | **Conform** | Bare-witness exclusion (PR36): witnessdepth değil → NoAxisCandidate |

## Özet

| State | Golden (13) | Held-out (5) | Toplam (18) |
|---|---|---|---|
| **Conform** | 9 | 3 | 12 |
| **PartialConform** | 2 | 0 | 2 |
| **KnownLimitation** | 0 | 2 | 2 |
| **RejectAsExpected** | 2 | 0 | 2 |
| **UnexpectedFailure** | 0 | 0 | 0 |

## Yorum

- **Conform oranı %67 (12/18)** — Faz 1 deterministic classifier'ın golden fixture coverage'ı güçlü.
- **PartialConform (2)** coarse-grained classifier itirafı: fix_006/fix_011 ImplementedBy evidence gating,
  fix_007 Decision: typed-prefix parsing. Hepsi belgeli limitation, kalıcı hata değil.
- **KnownLimitation (2)** held-out setin değerini kanıtlar: semantik false-positive (held_002) ve
  negasyon (held_003) — ikisi de makalede §10 Threats malzemesi. *"The matcher is not the contribution;
  the binding protocol is."*
- **RejectAsExpected (2)** INV ihlallerinin doğru reject edildiğini kanıtlar: fix_010 (unanchored),
  fix_011 (ImplementedBy evidence yok).
- **UnexpectedFailure = 0** — hiçbir fixture beklenmeyen şekilde başarısız olmadı. Tüm davranışlar açıklandı.

## Reproducibility

```bash
# Golden fixture conformance (mevcut):
cargo test -p osp-core --test anchoring_mvp

# Held-out conformance:
cargo test -p osp-core --test paper3_heldout

# Tüm Paper 3 evidence:
cargo test -p osp-core --test paper3_evidence --test paper3_heldout
```

Frozen evidence commit: `481690d` (hash₁). Tüm volatile bilgiler `run-metadata.json`'da.
