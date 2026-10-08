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

/// **Proof-carrying tamam-iddia jetonu** (#197 review P1-1) — `RealizedCompleted`
/// ancak bunu taşıyabilir ve bu tipin **public constructor'ü YOKTUR**: alanları
/// özeldir; yalnız RealizationGate'in kontrollü girişi (`try_from_gate` — gate
/// PR'ı) üretebilir. "Illegal state temsil edilemez" disiplininin INV-T10
/// karşılığı: `RealizedCompleted` oluşturulabiliyorsa kanıt zaten doğrulanmış
/// olmak ZORUNDADIR — tip sisteminin garantisi `∃evidence` değil,
/// `Verify(C') ∧ Predicate(G_observed)` sürecinden geçmiş olmaktır.
///
/// Bu PR'da (type-split) jeton bilinçLE OPAK kalır: gate var olana dek dışarıda
/// `RealizedCompleted` üretilemez (`Deserialize` de türetilMEZ — wire okuma
/// constructor deliği açar; gate PR'ı kontrollü deserializasyon kararıyla ekler).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerifiedRealization {
    raw: RealizationEvidence,
    /// Gate'in predicate-doğrulama sonucu (kanıtın parçası — dışarıdan set edilemez).
    predicate_verified: bool,
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
    DeclaredRealizationBuildInvalid { evidence: RealizationEvidence },
    /// Gate koşulmadı (v1'in fiilî durumu — realization ritüel dışı/elle).
    NotAttempted,
}

impl RealizationVerdict {
    /// Verdict'in tamam-iddia zemini (#197 review P2-1: iddia yoksa `None`).
    /// `RealizedCompleted → Some(Realized)`; `BuildInvalid → Some(Graph)`
    /// (graph-katmanı iddia ayaktadır, realization kırık); `NotAttempted →
    /// None` (verdict tek başına hiçbir iddia kurulduğunu söyleyemez).
    pub fn completion_basis(&self) -> Option<CompletionBasis> {
        match self {
            Self::RealizedCompleted { .. } => Some(CompletionBasis::Realized),
            Self::DeclaredRealizationBuildInvalid { .. } => Some(CompletionBasis::Graph),
            Self::NotAttempted => None,
        }
    }

    /// Build-invalid yolunun hata sayısı (raw evidence içinden).
    pub fn build_error_count(&self) -> Option<u64> {
        match self {
            Self::DeclaredRealizationBuildInvalid { evidence } => match evidence.build {
                BuildOutcome::Failed { error_count } => Some(error_count),
                BuildOutcome::Succeeded => None,
            },
            _ => None,
        }
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
            evidence: RealizationEvidence {
                build: BuildOutcome::Failed { error_count: 122 },
                c_observed: Some(0.0),
                c_predicted: Some(0.0),
                tests: None,
                ..ok_evidence()
            },
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
}
