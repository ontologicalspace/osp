//! Language Adapter System — Python/TypeScript/JavaScript (Faz 3.2).
//!
//! osp-spike'tan tree-sitter entegrasyonu migrate edildi. Her dil kendi adapter'ı:
//! - `extract_imports`: AST'den import deyimleri
//! - `resolve_import`: internal/external/stdlib ayrımı
//! - `extract_class_defs`: class tanımları (abstractness için)

pub mod go;
pub mod javascript;
pub mod python;
pub mod rust;
pub mod shared;
pub mod typescript;

use crate::language::AdapterRegistry;

impl AdapterRegistry {
    /// Tüm Tier 1 adapter'ları (Python + TypeScript + JavaScript + Rust + Go).
    pub fn default_all() -> Self {
        Self::new()
            .with(python::PythonAdapter)
            .with(typescript::TypeScriptAdapter)
            .with(javascript::JavaScriptAdapter)
            .with(rust::RustAdapter)
            .with(go::GoAdapter)
    }
}
