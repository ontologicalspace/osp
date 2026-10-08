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
//! - **Karar (0):** "beyan dışı semantic work yeni proposal sayılır" —
//!   `DeclaredRealizationBuildInvalid` yalnız beyan-realization'ın build
//!   sonucunu adlandırır; varoluşsal "unrealizable" etiketi bu normatif karar
//!   verilerek TÜRETİLMİŞ bir label'dır (deney sonucu değil).
//! - Ölçü dili (#195 review P1-2): `E_c = c_observed − c_predicted` (imzalı
//!   target-axis residual) ile `D_G = d(G_predicted, G_observed)` (graph-genişliği
//!   distance; RealizationMatch tolerans semantiği #196 karar 1) AYRI eksenlerdir.

use serde::{Deserialize, Serialize};

/// Tamamlama iddiasının kanıt zemini — hangi katmanda kuruldu (#196).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Bir declared-realization'ın kanıt kaydı — D5a şemasının
/// (`dogfood/runs/2026-10-07-author-exp/realization-evidence.jsonl`) motor
/// tarafı karşılığı (gate PR'ı üretici yapacak; v1'de elle doldurulur).
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

/// RealizationGate verdict — karar (0) sözlüğü.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RealizationVerdict {
    /// Beyan-realization uygulandı, build/tests geçti, reanalysis predicate'i
    /// sağladı. **Kanıt yapıda taşınır** — `RealizationEvidence`'sız
    /// `RealizedCompleted` kurulamaz (INV-T10'un tip-düzeyi karşılığı).
    RealizedCompleted { evidence: RealizationEvidence },
    /// Beyan-realization uygulandı ve build kırıldı (karar (0): beyan dışı
    /// kurtarma YENİ proposal'dır — bu verdict'in ötesine geçmez).
    DeclaredRealizationBuildInvalid { build_error_count: u64 },
    /// Gate koşulmadı (v1'in fiilî durumu — realization ritüel dışı/elle).
    NotAttempted,
}

impl RealizationVerdict {
    /// Verdict'in tamam-iddia zemini.
    pub fn completion_basis(&self) -> CompletionBasis {
        match self {
            Self::RealizedCompleted { .. } => CompletionBasis::Realized,
            Self::DeclaredRealizationBuildInvalid { .. } | Self::NotAttempted => {
                CompletionBasis::Graph
            }
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

    #[test]
    fn realized_completed_requires_evidence_by_construction() {
        // `RealizedCompleted { evidence }` — evidence'sız varyant yazılamaz
        // (derleme hatası); bu test yalnızca wire biçimini pinler.
        let v = RealizationVerdict::RealizedCompleted {
            evidence: ok_evidence(),
        };
        assert_eq!(v.completion_basis(), CompletionBasis::Realized);
        let wire = serde_json::to_string(&v).unwrap();
        assert!(wire.contains("\"RealizedCompleted\""), "{wire}");
        assert!(wire.contains("\"c_observed\":0.9167"), "{wire}");
    }

    #[test]
    fn build_invalid_and_not_attempted_stay_graph_basised() {
        // Karar (0): DeclaredRealizationBuildInvalid gerçek-dünya hükmü DEĞİL —
        // beyan-realization'ın build sonucu; iddia zemini graph'ta kalır.
        let invalid = RealizationVerdict::DeclaredRealizationBuildInvalid {
            build_error_count: 122,
        };
        assert_eq!(invalid.completion_basis(), CompletionBasis::Graph);
        assert_eq!(
            RealizationVerdict::NotAttempted.completion_basis(),
            CompletionBasis::Graph
        );
        let wire = serde_json::to_string(&invalid).unwrap();
        assert!(wire.contains("122"), "{wire}");
    }

    #[test]
    fn completion_basis_wire_is_stable_lowercase() {
        assert_eq!(
            serde_json::to_string(&CompletionBasis::Graph).unwrap(),
            "\"Graph\""
        );
        assert_eq!(
            serde_json::to_string(&CompletionBasis::Realized).unwrap(),
            "\"Realized\""
        );
    }
}
