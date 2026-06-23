//! Faz 0.5 — Metrik hesaplayıcı.
//!
//! Formüller `docs/roadmap.md` §6 ile uyumlu. Graf (Faz 0.3) tam oturana kadar
//! graf-bağımlı metrikler 0.0 döner; witness-bağımlı metrikler şimdi hesaplanır.

use crate::model::{DepGraph, Metrics, WitnessProfile};

/// Graf + şahitlik profilinden tüm spike metriklerini hesaplar.
pub fn compute(graph: &DepGraph, witness: &WitnessProfile) -> Metrics {
    Metrics {
        coupling_density: coupling_density(graph),
        hub_ratio: hub_ratio(graph),
        commit_entropy: commit_entropy(witness),
        witness_depth: witness_depth(witness),
        deviation_proxy: deviation_proxy(graph, witness),
    }
}

/// Kenar / düğüm oranı. Boş grafta 0.
fn coupling_density(g: &DepGraph) -> f64 {
    if g.nodes.is_empty() {
        return 0.0;
    }
    g.edges.len() as f64 / g.nodes.len() as f64
}

/// In-degree dağılımının top-%10'a konsantrasyonu (Lorenz/eşik yaklaşımı).
/// Hub varsa → 1.0'a yakın; homojen dağılım → 0.1'a yakın.
fn hub_ratio(g: &DepGraph) -> f64 {
    if g.nodes.is_empty() {
        return 0.0;
    }
    let mut in_deg: Vec<u32> = vec![0; g.nodes.len()];
    for e in &g.edges {
        if (e.to as usize) < in_deg.len() {
            in_deg[e.to as usize] += 1;
        }
    }
    in_deg.sort_unstable_by(|a, b| b.cmp(a));
    let top10 = (g.nodes.len().div_ceil(10)).max(1);
    let top_sum: u32 = in_deg.iter().take(top10).sum();
    let total: u32 = in_deg.iter().sum();
    if total == 0 {
        return 0.0;
    }
    top_sum as f64 / total as f64
}

/// Şahitlik derinliği: witnessed_ratio × ln(1 + distinct_authors).
/// Faz 1'in `v` ekseninin ampirik göstergesi.
fn witness_depth(w: &WitnessProfile) -> f64 {
    if w.distinct_authors == 0 {
        return 0.0;
    }
    w.witnessed_ratio * (1.0 + w.distinct_authors as f64).ln()
}

/// Commit entropisi (w-ekseni adayı). witness.rs'de `git log --name-only`
/// üzerinden Shannon H = -Σ p_i log2 p_i olarak hesaplanır.
fn commit_entropy(w: &WitnessProfile) -> f64 {
    w.commit_entropy
}

/// θ_proxy — geçici sapma göstergesi. Faz 1'de gerçek `cos θ` ile değişecek.
/// Spike tahmini: hub_ratio yüksek + witness_depth düşük → sapma yüksek.
fn deviation_proxy(g: &DepGraph, w: &WitnessProfile) -> f64 {
    let hub = hub_ratio(g);
    let wd = witness_depth(w);
    // [0,1] aralığa normalize: (hub) × (1 - tanh(wd))
    hub * (1.0 - wd.tanh())
}
