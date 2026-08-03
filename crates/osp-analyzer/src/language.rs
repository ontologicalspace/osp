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
}

impl RepoContext {
    pub fn new(repo_root: std::path::PathBuf, all_files: Vec<std::path::PathBuf>) -> Self {
        let resolver = ImportResolver::build(&all_files);
        let go_module_path = crate::adapters::shared::detect_go_module_path(&repo_root);
        let go_package_index = GoPackageIndex::build(&repo_root, &all_files);
        Self {
            repo_root,
            all_files,
            resolver,
            go_module_path,
            go_package_index,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LanguageAdapter trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Her dil için syntactic analiz arayüzü (Tier 1).
///
/// Implementasyonlar: `PythonAdapter`, `TypeScriptAdapter`, `JavaScriptAdapter`,
/// `RustAdapter` 🆕, `GoAdapter` 🆕 — Faz 3.2+.
pub trait LanguageAdapter: Send + Sync {
    /// Dil adı: "python", "typescript", "rust", "go".
    fn name(&self) -> &str;

    /// Desteklenen dosya uzantıları: [".py"], [".rs"], [".go"].
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

    /// Bir `LanguageId`'ye kayıtlı adapter var mı — katalog üzerinden (bir dilin
    /// birden çok uzantısı olabilir; ilk eşleşen uzantı yeterli, çünkü bir dilin
    /// tüm uzantıları aynı adapter'a gider).
    pub fn adapter_for_language(&self, language: LanguageId) -> Option<&dyn LanguageAdapter> {
        let known = LanguageCatalog::known_all()
            .iter()
            .find(|k| k.id == language)?;
        known
            .extensions
            .iter()
            .find_map(|ext| self.adapter_for_extension(ext))
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LanguageId + LanguageCatalog (PR B — declaration policy for known languages)
// ═══════════════════════════════════════════════════════════════════════════════

/// Identity of a language OSP knows about (independent of whether a compiled
/// adapter for it exists in a given `AdapterRegistry`).
///
/// PR B ships the five languages that already have adapters. A new variant is
/// added only in the PR that also adds the corresponding `KnownLanguage` catalog
/// entry — e.g. `CSharp` is added in the PR that introduces `CSharpAdapter`, not
/// before, so the catalog never claims to know a language before OSP actually
/// recognizes its source files.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Python,
    TypeScript,
    JavaScript,
    Rust,
    Go,
}

/// One language OSP's catalog recognizes, independent of compiled/registered
/// adapter availability. `extensions` must match the corresponding adapter's
/// `LanguageAdapter::extensions()` output exactly (verified against each
/// adapter 2026-07-29): Python `[".py"]`, TypeScript `[".ts",".tsx"]`,
/// JavaScript `[".js",".jsx"]`, Rust `[".rs"]`, Go `[".go"]`.
#[derive(Debug, Clone, Copy)]
pub struct KnownLanguage {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
}

/// The catalog of languages OSP's `osp-analyzer` knows about. This is
/// deliberately independent of `AdapterRegistry` — a registry may hold a
/// subset of these (e.g. `AdapterRegistry::new().with(RustAdapter)`), and the
/// catalog is what lets that gap be *observed* rather than silently dropped.
///
/// NOT used to classify arbitrary non-source files (`.md`, `.json`, `.png`,
/// ...) as "unsupported languages" — an extension absent from this catalog is
/// simply out of scope, not a claim that OSP knows of and doesn't support that
/// language. Only extensions actually mapped to a `KnownLanguage` participate
/// in `AnalysisCompleteness`.
pub struct LanguageCatalog;

impl LanguageCatalog {
    pub const KNOWN_LANGUAGES: &'static [KnownLanguage] = &[
        KnownLanguage {
            id: LanguageId::Python,
            display_name: "Python",
            extensions: &[".py"],
        },
        KnownLanguage {
            id: LanguageId::TypeScript,
            display_name: "TypeScript",
            extensions: &[".ts", ".tsx"],
        },
        KnownLanguage {
            id: LanguageId::JavaScript,
            display_name: "JavaScript",
            extensions: &[".js", ".jsx"],
        },
        KnownLanguage {
            id: LanguageId::Rust,
            display_name: "Rust",
            extensions: &[".rs"],
        },
        KnownLanguage {
            id: LanguageId::Go,
            display_name: "Go",
            extensions: &[".go"],
        },
    ];

    /// All languages OSP's catalog knows about (not necessarily compiled/registered).
    pub fn known_all() -> &'static [KnownLanguage] {
        Self::KNOWN_LANGUAGES
    }

    /// Resolve a file path to a catalog-known language by extension, if any.
    /// Returns `None` for files with no extension, an unrecognized extension, or
    /// a non-source extension (`.md`, `.json`, `.png`, ...) — these are simply
    /// out of the catalog's scope, not "unsupported languages".
    pub fn language_for_path(path: &Path) -> Option<LanguageId> {
        let ext = path.extension().and_then(|e| e.to_str())?;
        let dotted = format!(".{ext}");
        Self::KNOWN_LANGUAGES
            .iter()
            .find(|k| k.extensions.contains(&dotted.as_str()))
            .map(|k| k.id)
    }
}

/// A path guaranteed to be repository-relative with `/` separators, regardless
/// of platform. Constructed only via [`RepoRelativePath::from_absolute`], which
/// mirrors the `strip_prefix(&repo)` + `\`→`/` normalization already used
/// elsewhere in the pipeline (e.g. `pipeline.rs` `node_paths` construction) —
/// centralized here as a type so a completeness reason can never carry a raw
/// absolute machine path into a report or snapshot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    pub fn from_absolute(repo_root: &Path, absolute: &Path) -> Self {
        let rel = absolute
            .strip_prefix(repo_root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| absolute.to_string_lossy().replace('\\', "/"));
        Self(rel)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RepoRelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a catalog-known source file was not analyzed.
///
/// `#[non_exhaustive]`: more reasons are added in later PRs, each in the PR that
/// actually produces it (e.g. `AdapterNotCompiled` once Cargo feature-gating
/// exists) — never added speculatively ahead of the code path that creates it.
/// `AdapterUnavailable` is the one reason PR B can produce today: the registry
/// passed to `analyze_repo_with`/`analyze_repo_with_config` is public and can be
/// partial (`AdapterRegistry::new().with(RustAdapter)`), so a catalog-known file
/// (e.g. `.py`) can already lack a registered adapter — independent of any
/// Cargo feature system.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncompleteReason {
    AdapterUnavailable {
        language: LanguageId,
        path: RepoRelativePath,
    },
}

/// Whether an `AnalysisResult` covers every catalog-known source file the
/// registry could see, or is missing some due to `IncompleteReason`s.
///
/// `#[non_exhaustive]`: `Partial`'s meaning may gain new reason variants
/// without becoming a breaking change for exhaustive-matching downstream code.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AnalysisCompleteness {
    #[default]
    Complete,
    Partial {
        reasons: Vec<IncompleteReason>,
    },
}

impl AnalysisCompleteness {
    pub fn is_complete(&self) -> bool {
        matches!(self, AnalysisCompleteness::Complete)
    }

    /// Build from a collected list of reasons: empty → `Complete`.
    pub fn from_reasons(reasons: Vec<IncompleteReason>) -> Self {
        if reasons.is_empty() {
            AnalysisCompleteness::Complete
        } else {
            AnalysisCompleteness::Partial { reasons }
        }
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

    // ── LanguageCatalog ──────────────────────────────────────────────────────

    #[test]
    fn catalog_resolves_known_extensions() {
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("main.py")),
            Some(LanguageId::Python)
        );
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("x.tsx")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("x.jsx")),
            Some(LanguageId::JavaScript)
        );
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("main.rs")),
            Some(LanguageId::Rust)
        );
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("main.go")),
            Some(LanguageId::Go)
        );
    }

    #[test]
    fn catalog_returns_none_for_non_source_extensions() {
        // These are NOT "unsupported languages" — they're simply out of scope.
        // The catalog must never claim knowledge of a file type it doesn't map.
        for name in ["README.md", "config.json", "logo.png", "script.sh", "data.csv"] {
            assert_eq!(
                LanguageCatalog::language_for_path(Path::new(name)),
                None,
                "{name} must not resolve to any LanguageId"
            );
        }
    }

    #[test]
    fn catalog_returns_none_for_no_extension() {
        assert_eq!(LanguageCatalog::language_for_path(Path::new("Makefile")), None);
    }

    #[test]
    fn catalog_known_all_has_five_languages_with_matching_adapter_extensions() {
        // Cross-check against each adapter's actual extensions() output (verified
        // 2026-07-29) so the catalog can never silently drift from the adapters.
        let known = LanguageCatalog::known_all();
        assert_eq!(known.len(), 5);
        let py = known.iter().find(|k| k.id == LanguageId::Python).unwrap();
        assert_eq!(py.extensions, &[".py"]);
        let ts = known
            .iter()
            .find(|k| k.id == LanguageId::TypeScript)
            .unwrap();
        assert_eq!(ts.extensions, &[".ts", ".tsx"]);
        let js = known
            .iter()
            .find(|k| k.id == LanguageId::JavaScript)
            .unwrap();
        assert_eq!(js.extensions, &[".js", ".jsx"]);
        let rs = known.iter().find(|k| k.id == LanguageId::Rust).unwrap();
        assert_eq!(rs.extensions, &[".rs"]);
        let go = known.iter().find(|k| k.id == LanguageId::Go).unwrap();
        assert_eq!(go.extensions, &[".go"]);
    }

    // ── RepoRelativePath ─────────────────────────────────────────────────────

    #[test]
    fn repo_relative_path_strips_prefix_and_normalizes_separators() {
        let repo = Path::new("/repo");
        let abs = Path::new("/repo/src/models/user.py");
        let rel = RepoRelativePath::from_absolute(repo, abs);
        assert_eq!(rel.as_str(), "src/models/user.py");
    }

    #[test]
    fn repo_relative_path_falls_back_to_full_path_if_not_under_root() {
        // Defensive: strip_prefix fails if abs isn't under repo_root. Falls back
        // to the (normalized) full path rather than panicking — mirrors the
        // existing unwrap_or_else pattern in pipeline.rs node_paths construction.
        let repo = Path::new("/repo");
        let abs = Path::new("/elsewhere/main.py");
        let rel = RepoRelativePath::from_absolute(repo, abs);
        assert_eq!(rel.as_str(), "/elsewhere/main.py");
    }

    // ── AdapterRegistry::adapter_for_language ───────────────────────────────

    #[test]
    fn adapter_for_language_none_on_empty_registry() {
        let reg = AdapterRegistry::new();
        assert!(reg.adapter_for_language(LanguageId::Python).is_none());
    }

    // ── AnalysisCompleteness ─────────────────────────────────────────────────

    #[test]
    fn completeness_empty_reasons_is_complete() {
        let c = AnalysisCompleteness::from_reasons(vec![]);
        assert_eq!(c, AnalysisCompleteness::Complete);
        assert!(c.is_complete());
    }

    #[test]
    fn completeness_nonempty_reasons_is_partial() {
        let reasons = vec![IncompleteReason::AdapterUnavailable {
            language: LanguageId::Python,
            path: RepoRelativePath::from_absolute(Path::new("/repo"), Path::new("/repo/a.py")),
        }];
        let c = AnalysisCompleteness::from_reasons(reasons.clone());
        assert_eq!(c, AnalysisCompleteness::Partial { reasons });
        assert!(!c.is_complete());
    }

    #[test]
    fn completeness_default_is_complete() {
        assert_eq!(AnalysisCompleteness::default(), AnalysisCompleteness::Complete);
    }
}
