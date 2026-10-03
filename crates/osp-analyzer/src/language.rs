//! Language Adapter System — Faz 3.1 skeleton.
//!
//! Her dil için syntactic analiz arayüzü (Tier 1 — tree-sitter).
//! Adapter implementasyonları Faz 3.2+ (Python/TS/JS migration, Rust/Go new).

use std::path::Path;

use crate::adapters::shared::{GoPackageIndex, ImportResolver};
use crate::contract::{ClassDef, ImportStatement, ResolvedImport};

// ═══════════════════════════════════════════════════════════════════════════════
// RepoContext (§10 #10 — Faz 3.3'te tam tanım)
// ═══════════════════════════════════════════════════════════════════════════════

/// Import çözümleme için repo context.
///
/// `ImportResolver` HashMap'i build edilmiş halde taşır — O(1) lookup.
#[derive(Debug, Clone)]
pub struct RepoContext {
    pub repo_root: std::path::PathBuf,
    /// Tüm kaynak dosyalar (diagnostic için).
    pub all_files: Vec<std::path::PathBuf>,
    /// Faz 3.9.1: HashMap-based import resolver.
    pub resolver: ImportResolver,
    /// Go: `go.mod` module path (internal import detection için). Go repo değilse None.
    pub go_module_path: Option<String>,
    /// Go: package directory → files index (O(1) import lookup). Non-Go repo'da boş.
    pub go_package_index: GoPackageIndex,
    /// C# (#137): namespace → declaring files index (`using N.S;` çözümlemesi).
    /// Non-C# repo'da boş (build yalnız `.cs` dosyalarını parse eder).
    pub csharp_namespace_index: crate::adapters::shared::CSharpNamespaceIndex,
}

impl RepoContext {
    pub fn new(repo_root: std::path::PathBuf, all_files: Vec<std::path::PathBuf>) -> Self {
        let resolver = ImportResolver::build(&all_files);
        let go_module_path = crate::adapters::shared::detect_go_module_path(&repo_root);
        let go_package_index = GoPackageIndex::build(&repo_root, &all_files);
        let csharp_namespace_index =
            crate::adapters::shared::CSharpNamespaceIndex::build(&all_files);
        Self {
            repo_root,
            all_files,
            resolver,
            go_module_path,
            go_package_index,
            csharp_namespace_index,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LanguageAdapter trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Her dil için syntactic analiz arayüzü (Tier 1).
///
/// Implementasyonlar: `PythonAdapter`, `TypeScriptAdapter`, `JavaScriptAdapter`,
/// `RustAdapter`, `GoAdapter`, `CSharpAdapter` (#137) — Faz 3.2+.
pub trait LanguageAdapter: Send + Sync {
    /// Dil adı: "python", "typescript", "rust", "go", "csharp".
    fn name(&self) -> &str;

    /// Desteklenen dosya uzantıları: [".py"], [".rs"], [".go"], [".cs"].
    fn extensions(&self) -> &[&str];

    /// Import deyimlerini çıkar (syntactic).
    fn extract_imports(&self, source: &str) -> Vec<ImportStatement>;

    /// Import'u gerçek dosyaya çözümle (contextual).
    /// External/stdlib import'ları internal edge gibi sayılmamalı.
    fn resolve_import(
        &self,
        import: &ImportStatement,
        from_file: &Path,
        repo: &RepoContext,
    ) -> Option<ResolvedImport>;

    /// Class/function tanımlarını çıkar (abstractness için).
    fn extract_class_defs(&self, source: &str) -> Vec<ClassDef>;

    /// #167: tip-düzeyi referans çözümlemesi (using satırı → referans verilen
    /// TİP dosyaları + aynı-ns çapraz-dosya referansları).
    ///
    /// `resolve_import`'tan farkı: (i) ÇOK hedef döndürür (1 using → N kenar),
    /// (ii) import eden dosyanın KAYNAĞINA ihtiyaç duyar (tip-adı geçiş denetimi).
    ///
    /// Default `None` = dil tip-düzeyi çözümlemesi desteklemiyor → pipeline
    /// `TypeImports`/`SameNsType` kenarı üretmez, `ModuleMetrics::coupling_type`
    /// `None` kalır (alan snapshot'ta görünmez). Şu anda yalnız C# override eder.
    fn resolve_type_references(
        &self,
        _source: &str,
        _imports: &[ImportStatement],
        _from_file: &Path,
        _repo: &RepoContext,
    ) -> Option<crate::contract::TypeReferenceResolution> {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// AdapterRegistry — skeleton (Faz 3.2'de adapter implementasyonları eklenir)
// ═══════════════════════════════════════════════════════════════════════════════

/// Adapter kayıt defteri — uzantıya göre doğru adapter bulur.
///
/// Faz 3.1: boş skeleton. Faz 3.2'de Python/TS/JS adapter'ları eklenir.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl AdapterRegistry {
    /// Boş registry. Faz 3.2'de `default_all()` tüm adapter'larla gelir.
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Adapter sayısı.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Adapter ekle (builder).
    pub fn with<A: LanguageAdapter + 'static>(mut self, adapter: A) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    /// Uzantıya göre adapter bul.
    pub fn adapter_for_extension(&self, ext: &str) -> Option<&dyn LanguageAdapter> {
        let normalized = if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{ext}")
        };
        self.adapters
            .iter()
            .find(|a| a.extensions().iter().any(|&e| e == normalized))
            .map(|a| a.as_ref())
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry() {
        let reg = AdapterRegistry::new();
        assert!(reg.is_empty());
        assert!(reg.adapter_for_extension(".py").is_none());
    }

    #[test]
    fn repo_context_holds_root() {
        let ctx = RepoContext::new(
            std::path::PathBuf::from("/repo"),
            vec![std::path::PathBuf::from("/repo/main.py")],
        );
        assert_eq!(ctx.repo_root, std::path::PathBuf::from("/repo"));
        assert_eq!(ctx.all_files.len(), 1);
    }
}
