//! Rule engine stub — Q6 Rule Gate (agent-prompt-semantics.md §4, space-engine-design.md §4).
//!
//! Sistem invariant'larını (`Rule`) temsil eder. Bir Claim'in ΔS'i kuralları ihlal
//! ederse Q6 Rule Gate reddeder (pre-mutation, witness öncesi).
//!
//! **Faz 2 stub** — tipler tanımlı, implementasyon Faz 5'te gelir.
//! Engine'de `check_claim_rules()` stub olarak her zaman `Ok(())` döner.

use crate::space::Space;

// ═══════════════════════════════════════════════════════════════════════════════
// RuleId + Rule trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Kural tanımlayıcısı — `"arch.layer_separation"`, `"security.no_unsanitized_sql"`.
pub type RuleId = String;

/// Sistem invariant'ı (agent-prompt-semantics.md §4 Q6).
///
/// Bir Rule, Claim'in ΔS'i (yeni node/edge/mutasyon) üzerinde değerlendirilir.
/// İhlal tespit edilirse `Some(RuleViolation)` döner → Q6 reject.
///
/// **Hard Rule vs Soft Rule:**
/// - Hard Rule: statik, değişmez (mimari katman ihlalleri) — StaticGravityIndex'te cache
/// - Soft Rule: dinamik, context-bağımlı — lazy compute
///
/// Faz 5'te gerçek Rule implementasyonları gelir (örn: LayerSeparationRule,
/// NoDirectDbAccessRule). Şu an sadece trait + stub.
pub trait Rule: Send + Sync {
    /// Kural tanımlayıcısı.
    fn id(&self) -> &RuleId;

    /// Kuralın ΔS üzerinde ihlal durumunu değerlendir.
    ///
    /// `None` = ihlal yok (Q6 geçer). `Some(violation)` = ihlal tespit edildi (Q6 reject).
    ///
    /// Stub implementasyonu: her zaman `None` döner (Faz 5'te gerçek logic).
    fn evaluate(
        &self,
        _new_nodes: &[crate::space::Node],
        _new_edges: &[crate::space::Edge],
        _space: &Space,
    ) -> Option<RuleViolation> {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RuleViolation (Q6 failure — EngineCommitError::RuleViolation)
// ═══════════════════════════════════════════════════════════════════════════════

/// Q6 Rule Gate failure — ΔS bir Rule'u ihlal ediyor.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleViolation {
    pub rule_id: RuleId,
    pub detail: String,
    /// İhlal edilen kural kategorisi (kalibrasyon geri bildirimi için).
    pub severity: RuleSeverity,
}

impl std::fmt::Display for RuleViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Q6 rule violation ({:?} rule {}): {}",
            self.severity, self.rule_id, self.detail
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSeverity {
    /// Kritik mimari ihlal — kesin reject.
    Hard,
    /// Yumuşak ihlal — warning + reject (Faz 5'te policy-bağımlı olabilir).
    Soft,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{Edge, Node};

    /// Stub Rule — her zaman geçer (ihlal yok).
    struct StubRule {
        id: RuleId,
    }

    impl Rule for StubRule {
        fn id(&self) -> &RuleId {
            &self.id
        }
    }

    #[test]
    fn stub_rule_never_violates() {
        let rule = StubRule {
            id: "stub.always_pass".to_string(),
        };
        let space = Space::new();
        let nodes: Vec<Node> = vec![];
        let edges: Vec<Edge> = vec![];
        assert!(rule.evaluate(&nodes, &edges, &space).is_none());
    }

    #[test]
    fn rule_violation_carries_severity() {
        let v = RuleViolation {
            rule_id: "arch.layer".to_string(),
            detail: "domain → infrastructure direct".to_string(),
            severity: RuleSeverity::Hard,
        };
        assert_eq!(v.severity, RuleSeverity::Hard);
    }
}
