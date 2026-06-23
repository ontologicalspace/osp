//! SemanticIndex — SCIP'ten bağımsız soyutlama.
//!
//! SCIP index'ten çıkarılan semantik bilgiyi taşır. SCIP crate'i olmadan da
//! synthetic data ile test edilebilir (LCOM4 algoritması bu struct'a çalışır).
//!
//! **Naming notu:** Bu (`SemanticIndex`) SCIP index'inin *parser çıktısı* — class/method/field
//! yapısı. `contract::SemanticCoverage` ise ayrı bir tiptir — SCIP coverage *kalitesi*
//! (files_with_scip, coverage_ratio, stale). İkisi farklı concern: biri "ne var", diğeri
//! "ne kadar güvenilir". Karıştırma.

use std::collections::HashMap;

/// Bir class'ın semantik bilgisi (SCIP'ten çıkarılır).
#[derive(Debug, Clone)]
pub struct ClassSemanticInfo {
    /// Class adı (tam yolu ile: "package.module.ClassName").
    pub name: String,
    /// Method adları.
    pub methods: Vec<String>,
    /// Field adları.
    pub fields: Vec<String>,
    /// Method → erişilen field'lar (SCIP occurrence verisi).
    pub field_access: Vec<FieldAccess>,
}

/// Bir method'un hangi field'lara eriştiği.
#[derive(Debug, Clone)]
pub struct FieldAccess {
    pub method: String,
    pub field: String,
}

/// Tüm repo'nun semantik indeksi.
///
/// SCIP index'ten çıkarılır. SCIP yoksa boş → LCOM4 placeholder (0.5).
#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    /// Tüm tespit edilen class'lar (flat list — backward compat, toplam istatistik için).
    pub classes: Vec<ClassSemanticInfo>,
    /// Dosya → class'lar map (pipeline per-module LCOM4 cohesion için).
    /// Key = SCIP relative_path (örn "fastapi/app.py"), platform-normalize edilmiş.
    pub classes_by_file: HashMap<String, Vec<ClassSemanticInfo>>,
    /// SCIP coverage (kaç dosya indekslendi).
    pub files_indexed: usize,
    pub files_total: usize,
}

impl SemanticIndex {
    /// Boş index (SCIP yok — LCOM4 placeholder modu).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Coverage ratio [0, 1].
    pub fn coverage(&self) -> f64 {
        if self.files_total == 0 {
            0.0
        } else {
            self.files_indexed as f64 / self.files_total as f64
        }
    }

    /// SCIP var mı?
    pub fn is_available(&self) -> bool {
        !self.classes.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_index_has_zero_coverage() {
        let idx = SemanticIndex::empty();
        assert!(!idx.is_available());
        assert!((idx.coverage() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn coverage_ratio() {
        let idx = SemanticIndex {
            classes: vec![],
            files_indexed: 70,
            files_total: 100,
            ..Default::default()
        };
        assert!((idx.coverage() - 0.7).abs() < 1e-9);
    }
}
