//! Paylaşılan veri modelleri — OSP ontolojisinin spike-level izdüşümü.
//!
//! Not: Bunlar Faz 0 spike'ı için sadeleştirilmiş türlerdir. Faz 1'de
//! `osp-core` crate'inde tam ontolojik primitiflere (Node/Edge/Space/Intent/
//! Claim/Witness/TimeState) dönüşecekler.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub type NodeId = u32;

/// Bir repo analizinin tüm çıktısı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoAnalysis {
    pub repo_path: PathBuf,
    pub graph: DepGraph,
    pub witness: WitnessProfile,
    pub metrics: Metrics,
}

/// Bağımlılık grafı — uzayın (S) spike-level temsili: V (düğümler) + E (kenarlar).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Kaynak dosya yolu (repo-köklü).
    pub path: String,
    /// Düğümün kütlesi (LOC veya AST ağırlığı). Kütleçekim hesabında kullanılır.
    pub mass: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeKind {
    Module,
    Class,
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeKind {
    /// `from`, `to`'yu import ediyor (modül-düzeyi bağımlılık).
    Imports,
    /// `from`, `to`'yu çağırıyor (fonksiyon-düzeyi bağımlılık).
    Calls,
}

/// Git geçmişinden türetilen epistemolojik şahitlik profili.
///
/// OSP'nin zaman teorisinin ampirik izdüşümü: bir değişikliğin "miş'li zaman"dan
/// (tek başına commit) çıkıp "şimdiki zaman"a (onaylanmış/şahitli merge) geçiş oranı.
///
/// # Bilinen sınırlama — FF/squash/rebase workflow'ları
/// Fast-forward, squash-merge ve rebase-merge (GitHub default modları dahil) sıfır
/// merge-commit üretir; bu nedenle `merge_commits = 0` olur ve `witnessed_ratio`
/// ne kadar titiz review yapılırsa yapılsın 0 döner. `likely_ff_workflow` bu
/// körlüğü işaretler; bayrak set iken `witnessed_ratio`/`witness_depth` yorumlanırken
/// dikkatli olunmalıdır. Alternatif gerçek sayım: `gh pr list --state merged`
/// (Faz 0.7 Go/No-Go sırasında değerştirilecek).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WitnessProfile {
    pub total_commits: usize,
    /// Default branch'e ulaşan (şahitli) merge-commit sayısı.
    pub merge_commits: usize,
    /// Default branch'e doğrudan (şahitsiz) atılan commit sayısı.
    pub direct_to_default: usize,
    /// Distinct yazar (author) sayısı.
    pub distinct_authors: usize,
    /// `merge / (merge + direct_to_default)` — [0,1].
    pub witnessed_ratio: f64,
    /// `merge_commits == 0 && direct_to_default > 0` — FF/squash/rebase körlüğü sinyali.
    pub likely_ff_workflow: bool,
    /// Commit→dosya dağılımı üzerinden Shannon entropisi (w-ekseni adayı, Faz 0.5).
    /// Yüksek = dosyalara yayılmış değişim (kararlı); düşük = az dosyada yığılma (volatil).
    pub commit_entropy: f64,
    /// Default branch adı (main / master / ...).
    pub default_branch: String,
}

/// Spike metrikleri — Faz 1'in aday eksenlerinin ampirik göstergeleri.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    /// Kenar / düğüm oranı — kuplaj yoğunluğu (x-ekseni adayı).
    pub coupling_density: f64,
    /// In-degree dağılımının top-%10'a konsantrasyonu — "kütleçekim merkezi" ölçüsü.
    pub hub_ratio: f64,
    /// Commit-başına-dosya dağılımı üzerinden Shannon entropisi — istikrar (w-ekseni adayı).
    pub commit_entropy: f64,
    /// Şahitlik derinliği (v-ekseni adayı) = witnessed_ratio * ln(1+distinct_authors).
    pub witness_depth: f64,
    /// Vizyon hattından sapma proxy'si (θ_proxy) — Faz 1'de gerçek θ ile değişecek.
    pub deviation_proxy: f64,
}
