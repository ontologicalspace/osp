//! Agent semantiği — Faz 5 stub tipleri (agent-prompt-semantics.md).
//!
//! Bu modül Faz 5 (LLM OSP Codec) tasarımının tip iskeletini içerir. Implementasyonlar
//! (compute_space_slice, LLM runtime, witness feedback) Faz 5'te gelir. Şu an sadece
//! tipler tanımlı — `engine.rs` Q4/Q6 gate'leri ve `EngineCommitError` variant'ları
//! bunlara referans verir.
//!
//! **Önemli (inv #11-14):**
//! - LLM durumsuzdur, durum Agent kabuğunda
//! - `DeltaProposal` pozisyon **içermez** — engine compute eder (inv #4)
//! - `PermissionMask` God Mode tarafından atanır, Agent değiştiremez (inv #13)
//! - Prompt doğal dil değil, tiplenmiş pakettir (inv #14)

use std::collections::HashSet;

use crate::coords::{AxisId, RawPosition};
use crate::space::{EdgeKind, NodeId};
use crate::witness::ClaimId;

// ═══════════════════════════════════════════════════════════════════════════════
// PermissionMask (inv #13 — God Mode atanır, Agent değiştiremez)
// ═══════════════════════════════════════════════════════════════════════════════

/// Agent'ın okuma/yazma yetki matrisi (agent-prompt-semantics.md §2.1).
///
/// God Mode (insan-operatör veya bootstrap config) tarafından Intent hedef alanına
/// ve Agent rolüne göre atanır. Agent kabuğu ve LLM kendi yetkilerini genişletemez.
///
/// Üç-nokta savunma derinliği:
/// 1. `compute_space_slice()` — okuma izni olmayan düğümleri projeksiyondan çıkarır
/// 2. Agent kabuğu — yazma izni olmayan mutasyonları erken reddeder
/// 3. `SpaceEngine::commit()` — nihai zorunlu kontrol (atlanamaz)
#[derive(Debug, Clone, Default)]
pub struct PermissionMask {
    /// Agent'ın değiştiremeyeceği, sadece okuyabileceği düğümler.
    pub read_only_nodes: HashSet<NodeId>,
    /// Agent'ın yeni düğüm ekleyebileceği veya koordinat güncelleyebileceği eksenler.
    pub writable_axes: HashSet<AxisId>,
    /// Agent'ın oluşturamayacağı kenar türleri (örn: Approves → sadece Witness).
    pub forbidden_edge_kinds: HashSet<EdgeKind>,
    /// Agent'ın pozisyon güncelleyebileceği maksimum sapma (θ_max yetki sınırı).
    pub max_position_deviation: f64,
}

impl PermissionMask {
    /// Default: tüm node'lar read-write, tüm axis'ler writable, sınırsız deviation.
    /// Faz 2'de no-op/full-access; Faz 5'te God Mode config'ten yüklenir.
    pub fn full_access() -> Self {
        Self {
            read_only_nodes: HashSet::new(),
            writable_axes: HashSet::new(),
            forbidden_edge_kinds: HashSet::new(),
            max_position_deviation: f64::MAX,
        }
    }

    /// Node'a okuma izni var mı? (compute_space_slice denetim noktası 1)
    pub fn has_read_permission(&self, _node: NodeId) -> bool {
        // Stub: full access — Faz 5'te read_only_nodes kontrolü gelir
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DeltaProposal (LLM çıktısı — structural only, NO positions)
// ═══════════════════════════════════════════════════════════════════════════════

/// LLM'den beklenen çıktı (inv #12). Agent kabuğu LLM çıktısını bu şemaya göre
/// deserialize eder; uymayan çıktılar Q4 Syntax Gate'inde deterministik reddedilir.
///
/// **KRİTİK (inv #4):** Pozisyon **içermez** — sadece yapısal değişiklikler.
/// Pozisyonlar `SpaceEngine` tarafından compute edilir (agent-prompt-semantics.md §2.2).
#[derive(Debug, Clone, Default)]
pub struct DeltaProposal {
    /// Yeni eklenecek ontolojik düğümler.
    pub new_nodes: Vec<NewNodeSpec>,
    /// Yeni eklenecek tiplenmiş kenarlar.
    pub new_edges: Vec<NewEdgeSpec>,
    /// Mevcut düğümlerin entity özelliklerinde değişiklikler (kind/mass/metadata — POZİSYON DEĞİL).
    pub modified_entities: Vec<EntityChangeSpec>,
    /// LLM'in pozisyonla ilgili tavsiyeleri — ADVISORY ONLY, authoritative değil.
    pub position_hints: Vec<PositionHint>,
    /// LLM'in kararlarını açıklayan gerekçe (şahitler tarafından okunabilir).
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct NewNodeSpec {
    pub kind: crate::space::NodeKind,
    pub initial_mass: f64,
    pub connected_to: Vec<(NodeId, EdgeKind)>,
}

#[derive(Debug, Clone)]
pub struct NewEdgeSpec {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone)]
pub struct EntityChangeSpec {
    pub node_id: NodeId,
    // Faz 5: pub changes: EntityChanges (kind/mass/metadata — RawPosition hariç)
}

/// LLM'in "bu node şu pozisyonda olmalı" tavsiyesi — engine tarafından authoritative
/// kabul EDİLMEZ. Sadece diagnostic amaçlı (agent-prompt-semantics.md §2.2).
#[derive(Debug, Clone)]
pub struct PositionHint {
    pub node_id: NodeId,
    pub suggested_raw: RawPosition,
    pub rationale: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// OutputContract (DeltaProposal şema doğrulaması — Q4 Syntax Gate)
// ═══════════════════════════════════════════════════════════════════════════════

/// LLM'den beklenen çıktı şeması (inv #12). Agent kabuğu LLM çıktısını bu kontrata
/// göre deserialize eder; uymayan çıktılar Q4'te deterministik reddedilir.
///
/// Faz 5 stub — şimdilik tüm DeltaProposal'ları geçerli sayar. Faz 5'te gerçek
/// şema doğrulama (node kind legal, EdgeKind legal, NodeId mevcut, vb.) gelir.
#[derive(Debug, Clone, Default)]
pub struct OutputContract {
    // Faz 5: allowed_node_kinds, allowed_edge_kinds, required_fields, vb.
}

impl OutputContract {
    /// DeltaProposal şema doğrulaması (Q4 Syntax Gate).
    ///
    /// Stub: her zaman `Ok(())` — Faz 5'te gerçek validation.
    pub fn validate(&self, _proposal: &DeltaProposal) -> Result<(), SyntaxViolation> {
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SyntaxViolation (Q4 failure — EngineCommitError::SyntaxViolation)
// ═══════════════════════════════════════════════════════════════════════════════

/// Q4 Syntax Gate failure — DeltaProposal OutputContract'a uymuyor (inv #12).
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxViolation {
    pub claim_id: ClaimId,
    pub detail: String,
}

impl std::fmt::Display for SyntaxViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Q4 syntax violation (claim {}): {}",
            self.claim_id, self.detail
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OspPrompt (inv #14 — tiplenmiş paket, doğal dil değil)
// ═══════════════════════════════════════════════════════════════════════════════

/// Epistemik Projeksiyon Paketi (`π_A`) — agent-prompt-semantics.md §2.
///
/// `SpaceEngine` tarafından üretilen tiplenmiş veri paketi. LLM'e serialize edilir.
/// Faz 5 stub — `compute_space_slice()` implementasyonu Faz 5'te gelir.
#[derive(Debug, Clone)]
pub struct OspPrompt {
    pub vision: crate::vision::VisionVector,
    pub time_ref: crate::space::TimeLayer,
    pub permissions: PermissionMask,
    pub output_contract: OutputContract,
    // Faz 5: space_slice, intent, axis_manifest, rules, evidence_context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_mask_full_access_allows_all() {
        let mask = PermissionMask::full_access();
        assert!(mask.has_read_permission(999));
        assert!(mask.read_only_nodes.is_empty());
    }

    #[test]
    fn output_contract_stub_accepts_anything() {
        let contract = OutputContract::default();
        let proposal = DeltaProposal::default();
        assert!(contract.validate(&proposal).is_ok());
    }

    #[test]
    fn delta_proposal_has_no_position_field() {
        // inv #4 — DeltaProposal pozisyon İÇERMEZ (engine compute eder)
        let proposal = DeltaProposal::default();
        // Sadece structural fields var: new_nodes, new_edges, modified_entities, position_hints, reasoning
        assert!(proposal.new_nodes.is_empty());
        assert!(proposal.new_edges.is_empty());
        assert!(proposal.modified_entities.is_empty());
        assert!(proposal.position_hints.is_empty());
        assert!(proposal.reasoning.is_empty());
    }
}
