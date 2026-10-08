//! INV-T10 — Structural Realizability (#196; type-split, kod-öncesi onaylı).
//!
//! **#171 D5a'nın ölçtüğü gap'in tip karşılığı:** graph predicate success
//! build-valid declared realization ANLAMINA GELMEZ (B: E_c = 0 iken build
//! çöktü; A: graph 2/3 ama build+test 3/3 — [D5a 6041466076], analiz notu
//! PR #195). Graph uzayı, code uzayının analyzer görüntüsünden geniştir:
//! `∃C': A(C') = G'` koşulsuz her ΔG graph'ta geçerlidir; ödül `f(G')` iken
//! gerçek amaç `f(C', G', B')`.
//!
//! Bu modül tamamlama İDDİASININ kanıt zeminini tiplenir:
//!
//! ```text
//! Proposal → Hypothetical Graph → GraphCompleted        (Q5.b — DEĞİŞMEZ)
//!         → Source Realization → Build/Test → Reanalysis
//!         → RealizedCompleted   ⇒  Predicate(G_observed) ∧ Verify(C')
//! ```
//!
//! - `MutationDecision::AcceptAsCompleted` (Paper-2 lane sözlüğü) DOKUNULMAZ —
//!   bu eksen tamam **iddia-stratum**'udur (#196 karar 5).
//! - **Karar (0) DONDURULDU** (freeze: #196 yorum 6050269005; kullanıcı onayı
//!   2026-10-08): "beyan dışı semantic work yeni proposal sayılır" —
//!   `DeclaredRealizationBuildInvalid` yalnız beyan-realization'ın build
//!   sonucunu adlandırır; varoluşsal "unrealizable" etiketi bu normatif karar
//!   verilerek TÜRETİLMİŞ bir label'dır (deney sonucu değil).
//! - Ölçü dili (#195 review P1-2): `E_c = c_observed − c_predicted` (imzalı
//!   target-axis residual) ile `D_G = d(G_predicted, G_observed)` (graph-genişliği
//!   distance; RealizationMatch tolerans semantiği #196 karar 1) AYRI eksenlerdir.
//! - **Gelecek gate API pini (#197 review tur-2):** gate YALNIZ graph-completed
//!   iddia üzerinde koşar ve bunu yapısal kılar —
//!   `GraphCompletedProof (opaque witness) + RealizationEvidence (raw) →
//!   RealizationGate → RealizationVerdict`. İki üretici girişin de
//!   (`VerifiedRealization`, `FailedDeclaredRealization` üretim yolları) witness
//!   tüketmesi, `BuildInvalid.completion_basis() == Some(Graph)` zemininin örtük
//!   varsayımını kanıta çevirir (build-failure kanıtı tek başına graph
//!   predicate'in completed olduğunu KANITLAMAZ).

use serde::{Deserialize, Serialize};

/// Tamamlama iddiasının kanıt zemini — hangi katmanda kuruldu (#196).
/// Wire: snake_case (`"graph" | "realized"` — envelope'un diğer kind'leriyle
/// tutarlı; #197 review P2-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionBasis {
    /// Hypothetical-graph predicate pass (Q5.b çıktısı). **Mainline tamamlama
    /// iddiası TAŞIMAZ** — realized katmanı hakkında hiçbir şey söylemez.
    Graph,
    /// Source-space realization uygulanmış, doğrulanmış (parse/build/test) ve
    /// yeniden analiz edilen graph predicate'i sağlıyor — INV-T10 zinciri tam.
    Realized,
}

/// Build sonucu — D5a evidence alanının typed karşılığı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildOutcome {
    Succeeded,
    /// CS hata sayısıyla (D5a: 60 / 122 / 76).
    Failed {
        error_count: u64,
    },
}

/// Test sonucu (koşulmadıysa None — dürüst boşluk).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestOutcome {
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
}

/// Bir declared-realization'ın **raw** kanıt kaydı — D5a şemasının
/// (`dogfood/runs/2026-10-07-author-exp/realization-evidence.jsonl`) motor
/// tarafı karşılığı. Ham veridir: alanları herkes doldurabilir ve tek başına
/// HİÇBİR tamam-iddia kanıtı taşımaz (#197 review P1-1 — kanıtlayan sarmalayıcı
/// `VerifiedRealization`'dır; gate PR'ı üretici yapar, v1'de elle doldurulur).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealizationEvidence {
    /// Beyan (ΔG + reasoning) gerçekleştirildi mi (patch üretildi mi).
    pub patch_created: bool,
    /// Kaynak ayrıştırılabilir mi (tree-sitter seviyesi).
    pub parse: bool,
    pub build: BuildOutcome,
    pub tests: Option<TestOutcome>,
    /// Yeniden analiz sonrası target-axis coupling (after.json).
    pub c_observed: Option<f64>,
    /// Önerinin/sim'in öngördüğü target-axis coupling.
    pub c_predicted: Option<f64>,
    /// Yeniden analiz predicate'i sağladı mı (D5a amendment alanı; reanalysis
    /// yoksa `None` — `c_observed` TEK BAŞINA predicate kanıtı değildir: task
    /// coupling dışında eksen/provenance/çoklu predicate taşıyabilir).
    pub predicate_after_reanalysis: Option<bool>,
}

impl RealizationEvidence {
    /// **`E_c = c_observed − c_predicted`** — imzalı target-axis residual
    /// (#195 review P1-2). Pozitif = gözlem öngörüden kötü; negatif = favorable.
    /// Eksendeki graph-genişliği sapmaları (`D_G`) bu değere GİRMEZ.
    pub fn e_c(&self) -> Option<f64> {
        match (self.c_observed, self.c_predicted) {
            (Some(o), Some(p)) => Some(o - p),
            _ => None,
        }
    }
}

/// **Proof-carrying tamam-iddia jetonu** (#197 review P1-1 / #198 review P1) —
/// `RealizedCompleted` ancak bunu taşıyabilir. Alanları özeldir ve **mühürleme
/// yolu (`from_gate`) modül-özelidir**: yalnız `evaluate_gate` çağırabilir —
/// graph-completion witness'ı olmayan hiçbir caller (osp-core içi dâhil)
/// `VerifiedRealization` üretemez. Mühür koşulları **Verify(C') v1** (freeze:
/// #196/6051272311): `patch_created ∧ parse ∧ build=Succeeded ∧
/// predicate_after_reanalysis=Some(true)` — aksi hâlde `None`. "Illegal state
/// temsil edilemez": `RealizedCompleted` oluşturulabiliyorsa kanıt zaten
/// doğrulanmış olmak ZORUNDADIR. (`Deserialize` türetilMEZ — wire constructor
/// deliği; `for_tests` yalnız `#[cfg(test)]`.)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerifiedRealization {
    raw: RealizationEvidence,
    /// Gate'in predicate-doğrulama sonucu (kanıtın parçası — dışarıdan set edilemez).
    predicate_verified: bool,
}

impl VerifiedRealization {
    /// **Gate'in kontrollü girişi (modül-özel)** — yalnız `evaluate_gate`.
    fn from_gate(raw: RealizationEvidence) -> Option<Self> {
        let verified = raw.patch_created
            && raw.parse
            && matches!(raw.build, BuildOutcome::Succeeded)
            && raw.predicate_after_reanalysis == Some(true);
        verified.then_some(Self {
            raw,
            predicate_verified: true,
        })
    }

    /// Mühürlü kanıt (read-only).
    pub fn raw(&self) -> &RealizationEvidence {
        &self.raw
    }
}

#[cfg(test)]
impl VerifiedRealization {
    /// Yalnız çekirdek testleri — production constructor'ü gate PR'ıdır.
    pub(crate) fn for_tests(raw: RealizationEvidence, predicate_verified: bool) -> Self {
        Self {
            raw,
            predicate_verified,
        }
    }
}

/// #197 review tur-2 P1 — failure tarafı da proof-carrying: **"BuildInvalid ama
/// build Succeeded" temsil EDİLEMEZ.** Alan özeldir; tek giriş `try_new`'dir ve
/// yalnız `BuildOutcome::Failed` taşıyan raw evidence kabul eder (Succeeded →
/// `Err(BuildWasNotFailed)`). Böylece verdict'in adı ile taşıdığı tip aynı
/// önermeyi kanıtlar — `RealizedCompleted → VerifiedRealization` simetrisinin
/// karşılığı: `DeclaredRealizationBuildInvalid → FailedDeclaredRealization`.
///
/// (Gelecek gate API'si — pin: `from_gate(GraphCompletedProof, raw)` — witness
/// tüketimini de yapılandırır; bkz. modül dokümanı.)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FailedDeclaredRealization {
    raw: RealizationEvidence,
}

/// `FailedDeclaredRealization::try_new` reddi: kanıtın build'i başarılıysa
/// "build-invalid" verdict'i yalan söylemiş olur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildWasNotFailed;

impl std::fmt::Display for BuildWasNotFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "build-invalid verdict requires BuildOutcome::Failed evidence (found Succeeded)"
        )
    }
}

impl std::error::Error for BuildWasNotFailed {}

impl FailedDeclaredRealization {
    /// Kontrollü constructor (**modül-özel** — #198 review P1: mühürleme
    /// yolları gate dışından erişilemez; yalnız `evaluate_gate` ve modül
    /// testleri) — yalnız build'i KIRIK raw evidence kabul eder.
    fn try_new(raw: RealizationEvidence) -> Result<Self, BuildWasNotFailed> {
        match raw.build {
            BuildOutcome::Failed { .. } => Ok(Self { raw }),
            BuildOutcome::Succeeded => Err(BuildWasNotFailed),
        }
    }

    /// Kanıtın kendisi (read-only).
    pub fn raw(&self) -> &RealizationEvidence {
        &self.raw
    }

    /// Hata sayısı — invariant sayesinde HER ZAMAN vardır (Succeeded dalı
    /// `try_new` tarafından temsil edilemez kılındı).
    pub fn build_error_count(&self) -> u64 {
        match self.raw.build {
            BuildOutcome::Failed { error_count } => error_count,
            // try_new bu dalı üretmez; unreachable invariant'ın kendisidir.
            BuildOutcome::Succeeded => unreachable!(
                "FailedDeclaredRealization invariant: build Succeeded temsil edilemez (try_new reddeder)"
            ),
        }
    }
}

/// #198 — üçüncü seal: **"predicate gözlemde sağlanMADI"** (D5a A-20 hücresi:
/// build-geçer declared realization + reanalysis predicate'i düşer). Yalnız
/// `predicate_after_reanalysis == Some(false)` taşıyan raw kanıtı mühürler —
/// "unsatisfied ama predicate Some(true)" temsil edilemez.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnsatisfiedPredicateEvidence {
    raw: RealizationEvidence,
}

/// `UnsatisfiedPredicateEvidence::try_new` reddi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredicateWasNotUnsatisfied;

impl std::fmt::Display for PredicateWasNotUnsatisfied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "predicate-unsatisfied verdict requires predicate_after_reanalysis == Some(false)"
        )
    }
}

impl std::error::Error for PredicateWasNotUnsatisfied {}

impl UnsatisfiedPredicateEvidence {
    /// Kontrollü constructor (**modül-özel** — #198 review P1) — yalnız
    /// reanalysis predicate'ini DÜŞÜRMÜŞ kanıt.
    fn try_new(raw: RealizationEvidence) -> Result<Self, PredicateWasNotUnsatisfied> {
        match raw.predicate_after_reanalysis {
            Some(false) => Ok(Self { raw }),
            _ => Err(PredicateWasNotUnsatisfied),
        }
    }

    /// Kanıtın kendisi (read-only).
    pub fn raw(&self) -> &RealizationEvidence {
        &self.raw
    }
}

/// RealizationGate verdict — karar (0) sözlüğü (freeze: #196 yorum 6050269005).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RealizationVerdict {
    /// Beyan-realization uygulandı, build/tests geçti, reanalysis predicate'i
    /// sağladı. Kanıt **yapıda** taşınır (`VerifiedRealization` — public
    /// constructor yok; INV-T10'un tip-düzeyi karşılığı).
    RealizedCompleted { evidence: VerifiedRealization },
    /// Beyan-realization uygulandı ve build kırıldı. **Raw evidence TAM taşınır**
    /// (#197 review P1-3): bu bulgunun epistemik değeri `E_c = 0 ∧ BuildFailed`
    /// eşleşmesindedir — hata sayısı tek başına sim-öngörü/build ayrışmasını
    /// kaybettirirdi. Karar (0): beyan dışı kurtarma YENİ proposal'dır; bu
    /// verdict'in ötesine geçmez.
    DeclaredRealizationBuildInvalid { evidence: FailedDeclaredRealization },
    /// Beyan-realization uygulandı, build/tests geçti AMA yeniden analiz edilen
    /// graph predicate'i sağlamıyor (D5a A-20: 0.9167 > τ̂=0.89 — dürüst fail'in
    /// gerçekleşmiş hâli). Build-geçer ≠ task-tamamlandı: iddia graph katmanında
    /// kalır VE realized katmanında da kurulamadı.
    PredicateUnsatisfiedAfterReanalysis {
        evidence: UnsatisfiedPredicateEvidence,
    },
    /// Gate koşulmadı (graph-completed iddia yok, reanalysis kanıtı yok — v1'in
    /// fiilî durumu: realization ritüel dışı/elle).
    NotAttempted,
}

impl RealizationVerdict {
    /// Verdict'in tamam-iddia zemini (#197 review P2-1: iddia yoksa `None`).
    /// `RealizedCompleted → Some(Realized)`; `BuildInvalid` VE
    /// `PredicateUnsatisfied` → `Some(Graph)` (graph-katmanı iddia
    /// `evaluate_gate`'in GraphCompletedProof-pini sayesinde ayaktadır —
    /// gate yalnız graph-completed iddia üzerinde koşar; build-failure /
    /// predicate-failure kanıtı tek başına graph-completion'ı kanıtlamaz,
    /// yapısal bağlam gate girişidir); `NotAttempted → None` (verdict tek
    /// başına hiçbir iddia kurulduğunu söyleyemez).
    pub fn completion_basis(&self) -> Option<CompletionBasis> {
        match self {
            Self::RealizedCompleted { .. } => Some(CompletionBasis::Realized),
            Self::DeclaredRealizationBuildInvalid { .. }
            | Self::PredicateUnsatisfiedAfterReanalysis { .. } => Some(CompletionBasis::Graph),
            Self::NotAttempted => None,
        }
    }

    /// Build-invalid yolunun hata sayısı — `FailedDeclaredRealization`
    /// invariant'ı sayesinde o varyantta HER ZAMAN vardır (Succeeded temsil
    /// edilemez); diğer varyantlarda `None` (build kırılganlığı söz konusu değil).
    pub fn build_error_count(&self) -> Option<u64> {
        match self {
            Self::DeclaredRealizationBuildInvalid { evidence } => {
                Some(evidence.build_error_count())
            }
            _ => None,
        }
    }
}

/// Gate girdisi — **GraphCompletedProof pini icrası** (#197 review tur-2):
/// doğrulanmış kanonik zarfın graph-completion gerçeleri. Caller (CLI gate),
/// bu gerçeleri #178/#188 canonical-store bayt-doğrulamasından sonra doldurur;
/// `evaluate_gate` facts'i değil KURALI denetler: graph-completed olmayan
/// iddia üzerinde gate koşmaz (`NotAttempted`) — böylece "BuildInvalid ⇒
/// graph iddiası ayakta" zemin varsayımı yapısal hâle gelir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphCompletionFacts {
    pub task_id: u64,
    /// Kanonik zarf referansı (`attempts/task-<id>-<millis>-<pid>[-N].json`).
    pub canonical_attempt_ref: String,
    /// Zarfın `result.kind` wire değeri — gate yalnız `"completed"` üzerinde koşar.
    pub result_kind: String,
    /// Zarfın `completion_basis` alanı — `Some(Graph)` beklenir.
    pub completion_basis: Option<CompletionBasis>,
}

/// **RealizationGate (#196 uygulama-2, karar 2: CLI ritüelinde).** INV-T10
/// zincirinin kararı üreten çekirdeği:
///
/// ```text
/// GraphCompletionFacts (witness) + RealizationEvidence (raw)
///         → evaluate_gate → RealizationVerdict
/// ```
///
/// Tolerans v1 (karar 1'in dürüst kesiti — eşik verisi birikene dek sabit
/// sayısal `D_G` sınırı UYDURULMAZ, "koordinat ölçümle kazanılır" ilkesi):
/// RealizedCompleted yalnız `build == Succeeded ∧ predicate_after_reanalysis ==
/// Some(true)` ile verilir; `c_observed/c_predicted` (E_c) kanıtta YAŞAR ama
/// gate'i bağlamaz — graph-genişliği `D_G` toleransı ayrı çalışmadır.
pub fn evaluate_gate(
    graph: GraphCompletionFacts,
    realization: RealizationEvidence,
) -> RealizationVerdict {
    let graph_completed =
        graph.result_kind == "completed" && graph.completion_basis == Some(CompletionBasis::Graph);
    if !graph_completed {
        return RealizationVerdict::NotAttempted;
    }
    match realization.build {
        BuildOutcome::Failed { .. } => RealizationVerdict::DeclaredRealizationBuildInvalid {
            evidence: FailedDeclaredRealization::try_new(realization)
                .expect("evaluate_gate: build Failed dalı try_new tarafından kabul edilir"),
        },
        BuildOutcome::Succeeded => match realization.predicate_after_reanalysis {
            // Verify(C') v1 (freeze #196/6051272311): patch/parse/build/predicate
            // koşullarından biri tutmazsa mühür ÜRETİLEMEZ → kanıt yok → NotAttempted.
            Some(true) => VerifiedRealization::from_gate(realization)
                .map(|evidence| RealizationVerdict::RealizedCompleted { evidence })
                .unwrap_or(RealizationVerdict::NotAttempted),
            Some(false) => RealizationVerdict::PredicateUnsatisfiedAfterReanalysis {
                evidence: UnsatisfiedPredicateEvidence::try_new(realization)
                    .expect("evaluate_gate: Some(false) try_new tarafından kabul edilir"),
            },
            // Reanalysis yok → kanıt yok (dürüst boşluk): realized iddiası kurulamaz.
            None => RealizationVerdict::NotAttempted,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_evidence() -> RealizationEvidence {
        RealizationEvidence {
            patch_created: true,
            parse: true,
            build: BuildOutcome::Succeeded,
            tests: Some(TestOutcome {
                passed: 271,
                failed: 0,
                skipped: 0,
            }),
            c_observed: Some(0.9167),
            c_predicted: Some(0.9167),
            predicate_after_reanalysis: Some(true),
        }
    }

    #[test]
    fn e_c_is_signed_target_axis_residual() {
        // D5a A-18: 0.875 − 0.8889 = −0.0139 (favorable) — imzalı, target-axis.
        let ev = RealizationEvidence {
            c_observed: Some(0.875),
            c_predicted: Some(0.8889),
            ..ok_evidence()
        };
        assert!((ev.e_c().unwrap() - (-0.0139)).abs() < 1e-9);
        // Eksik eksen → dürüst boşluk (None), uydurma sıfır değil.
        assert_eq!(
            RealizationEvidence {
                c_predicted: None,
                ..ok_evidence()
            }
            .e_c(),
            None
        );
    }

    /// #197 review P1-1: `RealizedCompleted` yalnız gate-jetonuyla kurulabilir —
    /// raw evidence'dan DEĞİL. Bu test yalnız çekirdek-içi test constructor'üyle
    /// wire biçimini pinler (public yol yoktur; Deserialize de türetilmemiştir).
    #[test]
    fn realized_completed_carries_verified_realization_jetoonu() {
        let v = RealizationVerdict::RealizedCompleted {
            evidence: VerifiedRealization::for_tests(ok_evidence(), true),
        };
        assert_eq!(v.completion_basis(), Some(CompletionBasis::Realized));
        let wire = serde_json::to_string(&v).unwrap();
        assert!(wire.contains("\"RealizedCompleted\""), "{wire}");
        assert!(wire.contains("\"c_observed\":0.9167"), "{wire}");
        assert!(wire.contains("\"predicate_verified\":true"), "{wire}");
    }

    /// #197 review P1-3: build-invalid TAM raw evidence taşır — hata sayısı
    /// `evidence.build` içinden erişilir; E_c = 0 ∧ BuildFailed eşleşmesi kaybolmaz.
    #[test]
    fn build_invalid_carries_full_evidence_and_exposes_error_count() {
        // D5a B-19: E_c = 0 (ölçüm tahmini doğru) ∧ build 122 hatayla çöktü.
        let invalid = RealizationVerdict::DeclaredRealizationBuildInvalid {
            evidence: FailedDeclaredRealization::try_new(RealizationEvidence {
                build: BuildOutcome::Failed { error_count: 122 },
                c_observed: Some(0.0),
                c_predicted: Some(0.0),
                tests: None,
                ..ok_evidence()
            })
            .expect("Failed evidence kabul edilmeli"),
        };
        assert_eq!(invalid.build_error_count(), Some(122));
        assert_eq!(invalid.completion_basis(), Some(CompletionBasis::Graph));
        let wire = serde_json::to_string(&invalid).unwrap();
        assert!(wire.contains("122"), "{wire}");
        // Tam raw-kanıt gövdesi wire'da: E_c=0 eşleşmesi (c_observed=c_predicted=0)
        // hata sayısıyla birlikte taşınır — P1-3'ün bizzat kendisi.
        assert!(wire.contains("\"c_observed\":0.0"), "{wire}");
        assert!(wire.contains("\"c_predicted\":0.0"), "{wire}");
    }

    /// #197 review tur-2 P1: "BuildInvalid ama build Succeeded" temsil EDİLEMEZ —
    /// kontrollü constructor reddeder; verdict'in adı ile taşıdığı tip aynı
    /// önermeyi kanıtlar (simetri: RealizedCompleted→VerifiedRealization,
    /// DeclaredRealizationBuildInvalid→FailedDeclaredRealization).
    #[test]
    fn build_invalid_with_succeeded_evidence_is_unrepresentable() {
        let refused = FailedDeclaredRealization::try_new(ok_evidence())
            .expect_err("Succeeded build ile build-invalid kurulamaz");
        assert_eq!(refused, BuildWasNotFailed);
        assert_eq!(
            refused.to_string(),
            "build-invalid verdict requires BuildOutcome::Failed evidence (found Succeeded)"
        );
    }

    /// #197 review P2-1: iddia yoksa zemin de yok — `NotAttempted` None döner.
    #[test]
    fn not_attempted_establishes_no_completion_claim() {
        assert_eq!(RealizationVerdict::NotAttempted.completion_basis(), None);
        assert_eq!(RealizationVerdict::NotAttempted.build_error_count(), None);
    }

    /// #197 review P2-2: wire snake_case — envelope'un diğer kind'leriyle tutarlı.
    #[test]
    fn completion_basis_wire_is_stable_snake_case() {
        assert_eq!(
            serde_json::to_string(&CompletionBasis::Graph).unwrap(),
            "\"graph\""
        );
        assert_eq!(
            serde_json::to_string(&CompletionBasis::Realized).unwrap(),
            "\"realized\""
        );
    }

    fn facts() -> GraphCompletionFacts {
        GraphCompletionFacts {
            task_id: 18,
            canonical_attempt_ref: "attempts/task-18-990-1.json".to_string(),
            result_kind: "completed".to_string(),
            completion_basis: Some(CompletionBasis::Graph),
        }
    }

    /// #198: gate'in mutlu yolu — yalnız KANITLI mühür. `from_gate` redleri:
    /// predicate kanıtı yoksa / build kırıksa seal ÜRETİLEMEZ.
    #[test]
    fn gate_realizes_only_with_verified_predicate() {
        let v = evaluate_gate(facts(), ok_evidence());
        assert!(matches!(v, RealizationVerdict::RealizedCompleted { .. }));
        assert_eq!(v.completion_basis(), Some(CompletionBasis::Realized));
        assert!(
            VerifiedRealization::from_gate(RealizationEvidence {
                predicate_after_reanalysis: None,
                ..ok_evidence()
            })
            .is_none(),
            "reanalysis kanıtı yok → mühür yok"
        );
        assert!(
            VerifiedRealization::from_gate(RealizationEvidence {
                build: BuildOutcome::Failed { error_count: 1 },
                ..ok_evidence()
            })
            .is_none(),
            "build kırık → RealizedCompleted mührü yok"
        );
        // #198 review P1: Verify(C') v1 — realization UYGULANMAMIŞSA mühür yok.
        assert!(
            VerifiedRealization::from_gate(RealizationEvidence {
                patch_created: false,
                ..ok_evidence()
            })
            .is_none(),
            "patch üretilmedi → mühür yok"
        );
        assert!(
            VerifiedRealization::from_gate(RealizationEvidence {
                parse: false,
                ..ok_evidence()
            })
            .is_none(),
            "kaynak ayrıştırılamadı → mühür yok"
        );
        // Gate de dürüst: eksik verify → NotAttempted (panic değil).
        assert!(matches!(
            evaluate_gate(
                facts(),
                RealizationEvidence {
                    patch_created: false,
                    ..ok_evidence()
                }
            ),
            RealizationVerdict::NotAttempted
        ));
    }

    /// #198: verdict matrisi — D5a'nın üç hücresi (B-19 / A-20 / dürüst-boşluk).
    #[test]
    fn gate_matrix_build_invalid_predicate_unsatisfied_not_attempted() {
        // D5a B-19: E_c = 0 ∧ BuildFailed(122).
        let inv = evaluate_gate(
            facts(),
            RealizationEvidence {
                build: BuildOutcome::Failed { error_count: 122 },
                c_observed: Some(0.0),
                c_predicted: Some(0.0),
                tests: None,
                ..ok_evidence()
            },
        );
        assert_eq!(inv.build_error_count(), Some(122));
        assert_eq!(inv.completion_basis(), Some(CompletionBasis::Graph));

        // D5a A-20: build-geçer ama reanalysis predicate'i düşer (0.9167 > 0.89).
        let unsat = evaluate_gate(
            facts(),
            RealizationEvidence {
                predicate_after_reanalysis: Some(false),
                c_observed: Some(0.9167),
                ..ok_evidence()
            },
        );
        assert!(matches!(
            unsat,
            RealizationVerdict::PredicateUnsatisfiedAfterReanalysis { .. }
        ));
        assert_eq!(unsat.completion_basis(), Some(CompletionBasis::Graph));

        // Dürüst boşluk: reanalysis yok → realized iddiası kurulamaz.
        let none = evaluate_gate(
            facts(),
            RealizationEvidence {
                predicate_after_reanalysis: None,
                ..ok_evidence()
            },
        );
        assert!(matches!(none, RealizationVerdict::NotAttempted));
        assert_eq!(none.completion_basis(), None);
    }

    /// GraphCompletedProof pini: graph-completed olmayan iddia üzerinde gate koşmaz.
    #[test]
    fn gate_refuses_non_graph_completed_witness() {
        let mut f = facts();
        f.result_kind = "llm_error".to_string();
        assert!(matches!(
            evaluate_gate(f, ok_evidence()),
            RealizationVerdict::NotAttempted
        ));
        let mut f2 = facts();
        f2.completion_basis = None;
        assert!(matches!(
            evaluate_gate(f2, ok_evidence()),
            RealizationVerdict::NotAttempted
        ));
    }

    /// Üçüncü seal: "unsatisfied" adı ancak Some(false) kanıtıyla kurulabilir.
    #[test]
    fn unsatisfied_seal_requires_false() {
        assert!(UnsatisfiedPredicateEvidence::try_new(ok_evidence()).is_err());
    }
}
