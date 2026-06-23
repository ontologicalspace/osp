//! Ontolojik primitifler — `Node`, `Edge`, `Space` (OSP-formalism.md §1).

use std::collections::HashMap;

use crate::coords::Position;

/// Kararlı düğüm tanımlayıcı.
///
/// Faz 1: `u64` (sequential). Faz 2+: içerik-adresli hash (özgünlük + immutability).
pub type NodeId = u64;

/// Meta-ontoloji düğüm türleri (OSP-formalism.md §1.2).
///
/// Üst-ontoloji: yazılım-süreç modelini (Feature/Bug/Rule) + epistemolojik rolleri
/// (Agent/Intent/Claim/Witness) birleştirir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum NodeKind {
    /// Kaynak dosya / paket (Faz 0 spike seviyesi). Default tür.
    #[default]
    Module,
    /// Domain kavramı (DDD aggregate root gibi) — Faz 1+ AST + isim-analizi.
    Concept,
    /// Kullanıcı-görünür yetenek — Big Bang tetikleyicisi.
    Feature,
    /// Hata / negatif-uzay işareti (θ > θ_eşik'nin somut karşılığı).
    Bug,
    /// Mimari/domain kuralı — kütleçekim kaynağı (`G`'yi besler).
    Rule,
    /// AI-agent (LLM sürücüsü) — `Intent` üretir, `Claim` üretir.
    Agent,
    /// Agent'a verilen görev — **`t_f` (Gelecek) katmanında yaşar** (potansiyel gradyan;
    /// agent-prompt-semantics.md §0 ontolojik harita, OSP-formalism.md §1.2 + §3.1).
    Intent,
    /// Agent'ın ürettiği iş (PR) — `t_m`'de Belief → `t_c`'de Knowledge (witness sonrası).
    Claim,
    /// Onay/red veren kimlik — `W` operatörüne girdi.
    Witness,
}

/// Tipleşmiş kenar türleri (OSP-formalism.md §1.3).
///
/// Klasik yazılım grafindan (yalnız `DependsOn`) farklı olarak OSP'de epistemolojik
/// ilişkileri (`Witnesses`, `Approves`, `Violates`) de modeller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EdgeKind {
    /// `from` modülü `to` modülünü import ediyor (modül-düzeyi bağımlılık).
    Imports,
    /// `from` fonksiyonu `to` fonksiyonunu çağırıyor (fonksiyon-düzeyi).
    Calls,
    /// Genel bağımlılık (mimari, runtime).
    DependsOn,
    /// Mereolojik parça ilişkisi (`from` ∈ `to` aggregate).
    PartOf,
    /// Kalıtım / generalizasyon (is-a, subsumption).
    DerivesFrom,
    /// `from` witness'ı `to` claim'ini şahitlik ediyor.
    Witnesses,
    /// `from` witness'ı `to` claim'ini onaylıyor (`Witnesses` + verdict=Approve).
    Approves,
    /// `from` düğümü `to` kuralını ihlal ediyor (negatif-uzay sinyali).
    Violates,
}

/// Kavramsal uzay düğümü (OSP-formalism.md §1.1).
///
/// `Default`: `id=0, kind=Module, mass=0.0, position=[]` — builder/test kolaylığı için;
/// **`id` gerçek değerle override edilmeli** (insert sırasında).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Düğüm kütlesi `∈ ℝ⁺`. Faz 0: LOC. Faz 1: `α·LOC + β·AST + γ·in_degree`.
    pub mass: f64,
    /// `(x, y, z, w, v, u, ...)` koordinat — `CoordinateSystem::position_of` ile hesaplanır.
    pub position: Position,
    /// Analyzer-tarafından set edilen LCOM4 cohesion `∈ [0, 1]` (Faz 3.6+).
    /// `None` = ölçülmemiş → CohesionAxis fallback (0.5) kullanır.
    /// CouplingAxis/InstabilityAxis graf'ten compute ettiği için burada yok —
    /// sadece cohesion external (SCIP) veri gerektirir.
    #[serde(default)]
    pub cohesion: Option<f64>,
}

/// Tipleşmiş yönlü kenar (OSP-formalism.md §1.3).
///
/// **Self-loop semantiği:** `from == to` bazı türler için anlamlı (`Calls` — rekürsiyon),
/// bazıları için değil (`Imports` — modül kendini import edemez; `Witnesses` — self-witness
/// reddi). Tür-bazlı self-loop validasyonu Faz 1.2/1.3 graf kurulumunda eklenecek.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

/// Kütleçekim vektörü — `Rule`'lardan gelen kısıt ağırlıkları (`ℝᵏ`).
///
/// Örn: `Rule = "Feature'lar Test olmadan var olamaz"` ihlali → ilgili düğümün
/// `gravity` değerini düşürür → negatif-uzay sinyali.
pub type GravityVector = Vec<f64>;

/// Zaman katmanı (OSP-formalism.md §3.1).
///
/// OSP'de zaman kronolojik sayaç değil **epistemolojik durum**'dur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TimeLayer {
    /// Miş'li (öznel, `t_m`) — agent'ın izole lokal uzayı: taslak iddialar, feature branch.
    Misli,
    /// Şimdiki (nesnel, `t_c`) — onaylanmış, kütleçekimli gerçek uzay: main branch.
    #[default]
    Simdiki,
    /// Gelecek (potansiyel, `t_f`) — henüz-doğmamış niyetler: issue'lar, roadmap.
    Gelecek,
}

/// Kavramsal uzay `S = (V, E, G, t_state)` (OSP-formalism.md §1.4).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Space {
    /// Düğüm kümesi `V`.
    pub nodes: HashMap<NodeId, Node>,
    /// Kenar kümesi `E ⊆ V × V × EdgeKind`.
    pub edges: Vec<Edge>,
    /// Kütleçekim `G: NodeId → ℝᵏ`. Faz 1: stored (manuel set). Faz 2: `Rule`'lerden computed.
    // TODO(Faz 2): `compute_gravity()` / `apply_rule()` — Rule düğümleri eklendiğinde gravity
    //              vektörleri otomatik güncellenmeli. Şu an pasif (manuel HashMap set).
    pub gravity: HashMap<NodeId, GravityVector>,
    /// Zaman katmanı `t_state`.
    pub time_layer: TimeLayer,
}

impl Space {
    /// Yeni boş uzay (default: `Simdiki` katman).
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            gravity: HashMap::new(),
            time_layer: TimeLayer::Simdiki,
        }
    }

    /// Düğüm ekle (id çakışmasında eskiyi overwrite).
    pub fn insert_node(&mut self, node: Node) -> &mut Self {
        self.nodes.insert(node.id, node);
        self
    }

    /// Kenar ekle.
    pub fn insert_edge(&mut self, edge: Edge) -> &mut Self {
        self.edges.push(edge);
        self
    }

    /// Düğüm sayısı `|V|`.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Kenar sayısı `|E|`.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Belirli türdeki kenar sayısı (ör. `Imports` için coupling hesabında).
    pub fn edge_count_of(&self, kind: EdgeKind) -> usize {
        self.edges.iter().filter(|e| e.kind == kind).count()
    }

    /// Bir düğümün in-degree'i (belirli tür için; `Witnesses`/`Approves` şahitlik derinliği).
    pub fn in_degree(&self, id: NodeId, kind: EdgeKind) -> usize {
        self.edges
            .iter()
            .filter(|e| e.to == id && e.kind == kind)
            .count()
    }

    /// Bir düğümün out-degree'i (belirli tür için).
    pub fn out_degree(&self, id: NodeId, kind: EdgeKind) -> usize {
        self.edges
            .iter()
            .filter(|e| e.from == id && e.kind == kind)
            .count()
    }
}

impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_node(id: NodeId, mass: f64) -> Node {
        Node {
            id,
            mass,
            ..Default::default()
        }
    }

    #[test]
    fn space_starts_empty_in_simdiki() {
        let s = Space::new();
        assert_eq!(s.node_count(), 0);
        assert_eq!(s.edge_count(), 0);
        assert_eq!(s.time_layer, TimeLayer::Simdiki);
    }

    #[test]
    fn insert_nodes_and_edges() {
        let mut s = Space::new();
        s.insert_node(mod_node(1, 100.0));
        s.insert_node(mod_node(2, 50.0));
        s.insert_edge(Edge { from: 1, to: 2, kind: EdgeKind::Imports });
        s.insert_edge(Edge { from: 2, to: 1, kind: EdgeKind::Calls });
        assert_eq!(s.node_count(), 2);
        assert_eq!(s.edge_count(), 2);
        assert_eq!(s.edge_count_of(EdgeKind::Imports), 1);
        assert_eq!(s.edge_count_of(EdgeKind::Calls), 1);
    }

    #[test]
    fn in_out_degree_by_kind() {
        let mut s = Space::new();
        s.insert_node(mod_node(1, 1.0));
        s.insert_node(mod_node(2, 1.0));
        s.insert_node(mod_node(3, 1.0));
        // 1 → 2 (Imports), 3 → 2 (Imports), 1 → 3 (Calls)
        s.insert_edge(Edge { from: 1, to: 2, kind: EdgeKind::Imports });
        s.insert_edge(Edge { from: 3, to: 2, kind: EdgeKind::Imports });
        s.insert_edge(Edge { from: 1, to: 3, kind: EdgeKind::Calls });

        assert_eq!(s.in_degree(2, EdgeKind::Imports), 2);
        assert_eq!(s.out_degree(1, EdgeKind::Imports), 1);
        assert_eq!(s.in_degree(3, EdgeKind::Calls), 1);
        assert_eq!(s.in_degree(2, EdgeKind::Calls), 0); // farklı tür
    }

    #[test]
    fn node_kind_and_edge_kind_distinct() {
        assert_ne!(NodeKind::Module, NodeKind::Concept);
        assert_ne!(NodeKind::Feature, NodeKind::Bug);
        assert_ne!(NodeKind::Agent, NodeKind::Witness);
        assert_ne!(EdgeKind::Imports, EdgeKind::Calls);
        assert_ne!(EdgeKind::Witnesses, EdgeKind::Approves);
        assert_ne!(EdgeKind::PartOf, EdgeKind::DerivesFrom);
    }

    #[test]
    fn time_layer_default_is_simdiki() {
        assert_eq!(TimeLayer::default(), TimeLayer::Simdiki);
    }

    #[test]
    fn builder_chain_returns_self() {
        let mut s = Space::new();
        s.insert_node(mod_node(1, 1.0))
            .insert_node(mod_node(2, 1.0))
            .insert_edge(Edge {
                from: 1,
                to: 2,
                kind: EdgeKind::Calls,
            });
        assert_eq!(s.node_count(), 2);
        assert_eq!(s.edge_count(), 1);
    }

    #[test]
    fn insert_node_overwrites_on_id_clash() {
        let mut s = Space::new();
        s.insert_node(Node {
            id: 1,
            kind: NodeKind::Module,
            mass: 100.0,
            ..Default::default()
        });
        s.insert_node(Node {
            id: 1,
            kind: NodeKind::Concept,
            mass: 50.0,
            ..Default::default()
        });
        assert_eq!(s.node_count(), 1, "aynı id overwrite etmeli, eklememeli");
        let n = s.nodes.get(&1).expect("id=1 mevcut olmalı");
        assert_eq!(n.kind, NodeKind::Concept, "overwrite son değeri tutmalı");
        assert!((n.mass - 50.0).abs() < 1e-9);
    }

    #[test]
    fn node_default_is_module_zero() {
        let n = Node::default();
        assert_eq!(n.id, 0);
        assert_eq!(n.kind, NodeKind::Module);
        assert!((n.mass).abs() < 1e-9);
        // Faz 1.4: position artık Position struct (raw + derived), ikisi de default
        assert_eq!(n.position.raw, crate::coords::RawPosition::default());
        assert_eq!(n.position.derived, crate::coords::DerivedPosition::default());
    }
}
