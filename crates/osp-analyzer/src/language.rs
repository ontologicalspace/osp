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

    // `adapter_for_language` (review R2 P2'de kaldırıldı): per-dil ANY-uzantı
    // eşleşmesi custom adapter'larda exact-equality invariant'ı olmadığı için
    // yanıltıcıydı; keşif hattı dosya-bazlı `adapter_for_extension` kullanır.
    // İleride gerçek language-level capability gerekirse semantiği açık bir
    // API (ör. `supports_language_fully`) tasarlanır.
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
/// A new variant is added only together with the corresponding
/// `KnownLanguage` catalog entry, in the PR that introduces the adapter.
/// `CSharp` + its catalog entry arrive together in the PR B refresh because
/// `CSharpAdapter` (#137) landed on main while PR B was pending — the pair
/// still lands as one change so the catalog never claims to know a language
/// before OSP actually recognizes its source files.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Python,
    TypeScript,
    JavaScript,
    Rust,
    Go,
    CSharp,
}

/// One language OSP's catalog recognizes, independent of compiled/registered
/// adapter availability. `extensions` must match the corresponding adapter's
/// `LanguageAdapter::extensions()` output exactly (verified against each
/// adapter 2026-07-29; CSharp added and re-verified at the 2026-10-06
/// refresh): Python `[".py"]`, TypeScript `[".ts",".tsx"]`, JavaScript
/// `[".js",".jsx"]`, Rust `[".rs"]`, Go `[".go"]`, CSharp `[".cs"]`.
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
        KnownLanguage {
            id: LanguageId::CSharp,
            display_name: "C#",
            extensions: &[".cs"],
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
/// of platform — the guarantee is enforced at construction (review R1 P1): a
/// path outside `repo_root` is REJECTED by [`RepoRelativePath::from_absolute`],
/// never stored as an absolute fallback, so a completeness reason can never
/// carry a raw absolute machine path into a report or snapshot. Callers fail
/// closed on `None`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    /// `None` when `absolute` is not under `repo_root` — an out-of-scope path
    /// is an invariant violation for a completeness reason, not a fallback to
    /// the absolute machine path.
    pub fn from_absolute(repo_root: &Path, absolute: &Path) -> Option<Self> {
        let rel = absolute.strip_prefix(repo_root).ok()?;
        Some(Self(rel.to_string_lossy().replace('\\', "/")))
    }

    /// #199 (review P1-2): insan-taraflı repo-göreli path METNİNİ kimlik-ekseni
    /// kurallarına göre doğrular. Ölçülen düğüm path'leri filesystem
    /// yürüyüşünden kanonik gelir (`from_absolute`); beyan edilen (henüz var
    /// olmayan) yeni-düğüm path'leri aynı kurallara UYMAK zorunda — aksi halde
    /// `./main.rs` gibi bir alias hipotetik grafta `main.rs`'ten ayrı ikinci bir
    /// düğüm olarak sayılabilir ve #199'un onardığı ölçüm doğruluğu başka bir
    /// kimlik-alias üzerinden yeniden bozulur.
    ///
    /// Kurallar (ölçülen tarafın invariant'ının tek-path ifadesi): boş değil;
    /// yalnız `/` ayracı (`\` yok); absolute değil (`/` ile başlamaz); segmentler
    /// boş / `.` / `..` değil. Kanonik OLMAYANI normalize ETMEZ — reddeder
    /// (ölçülen kimlikle birebir eşleşme guarantee'si ancak böyle dürüst olur).
    pub fn from_repo_relative_str(s: &str) -> Option<Self> {
        if s.is_empty() || s.contains('\\') || s.starts_with('/') {
            return None;
        }
        if s.split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        {
            return None;
        }
        Some(Self(s.to_string()))
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
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("CustomerAppService.cs")),
            Some(LanguageId::CSharp)
        );
    }

    #[test]
    fn catalog_returns_none_for_non_source_extensions() {
        // These are NOT "unsupported languages" — they're simply out of scope.
        // The catalog must never claim knowledge of a file type it doesn't map.
        for name in [
            "README.md",
            "config.json",
            "logo.png",
            "script.sh",
            "data.csv",
        ] {
            assert_eq!(
                LanguageCatalog::language_for_path(Path::new(name)),
                None,
                "{name} must not resolve to any LanguageId"
            );
        }
    }

    #[test]
    fn catalog_returns_none_for_no_extension() {
        assert_eq!(
            LanguageCatalog::language_for_path(Path::new("Makefile")),
            None
        );
    }

    #[test]
    fn catalog_known_all_has_six_languages() {
        // Literal self-check of the catalog table. The adapter cross-check it
        // used to imply by name lives in
        // `catalog_extensions_match_builtin_adapters_exactly` (review R1 P1:
        // a test named "matching_adapter_extensions" must actually call the
        // adapters, not restate the literals).
        let known = LanguageCatalog::known_all();
        assert_eq!(known.len(), 6);
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
        let cs = known.iter().find(|k| k.id == LanguageId::CSharp).unwrap();
        assert_eq!(cs.extensions, &[".cs"]);
    }

    #[test]
    fn catalog_extensions_match_builtin_adapters_exactly() {
        // Review R1 P1: REAL cross-check — instantiate the built-in adapters
        // and compare their actual `extensions()` output with the catalog, per
        // language, exact slice equality. If an adapter gains/loses an
        // extension (e.g. ".csx") without the catalog following — or the
        // catalog claims one no adapter serves — this goes red. The
        // exact-equality assumption behind bit-identical discovery is thereby
        // test-enforced, not comment-enforced.
        use crate::adapters::csharp::CSharpAdapter;
        use crate::adapters::go::GoAdapter;
        use crate::adapters::javascript::JavaScriptAdapter;
        use crate::adapters::python::PythonAdapter;
        use crate::adapters::rust::RustAdapter;
        use crate::adapters::typescript::TypeScriptAdapter;

        let builtin: &[(LanguageId, &[&'static str])] = &[
            (LanguageId::Python, PythonAdapter.extensions()),
            (LanguageId::TypeScript, TypeScriptAdapter.extensions()),
            (LanguageId::JavaScript, JavaScriptAdapter.extensions()),
            (LanguageId::Rust, RustAdapter.extensions()),
            (LanguageId::Go, GoAdapter.extensions()),
            (LanguageId::CSharp, CSharpAdapter.extensions()),
        ];
        assert_eq!(
            builtin.len(),
            LanguageCatalog::known_all().len(),
            "every catalog language must have a built-in adapter cross-check row"
        );
        for (id, adapter_exts) in builtin {
            let known = LanguageCatalog::known_all()
                .iter()
                .find(|k| k.id == *id)
                .unwrap_or_else(|| panic!("no catalog entry for {id:?}"));
            assert_eq!(
                known.extensions, *adapter_exts,
                "catalog/adapter extension drift for {}",
                known.display_name
            );
        }
    }

    // ── RepoRelativePath ─────────────────────────────────────────────────────

    #[test]
    fn repo_relative_path_strips_prefix_and_normalizes_separators() {
        let repo = Path::new("/repo");
        let abs = Path::new("/repo/src/models/user.py");
        let rel = RepoRelativePath::from_absolute(repo, abs).expect("path is under repo root");
        assert_eq!(rel.as_str(), "src/models/user.py");
    }

    #[test]
    fn repo_relative_path_rejects_path_outside_root() {
        // Review R1 P1: the "guaranteed repository-relative" claim must be real.
        // A path outside repo_root is rejected — never stored as an absolute
        // machine-path fallback that could leak into reports/snapshots.
        let repo = Path::new("/repo");
        let outside = Path::new("/elsewhere/main.py");
        assert!(RepoRelativePath::from_absolute(repo, outside).is_none());
        // Under-root paths still normalize separators to `/`.
        let inside = RepoRelativePath::from_absolute(repo, Path::new("/repo/src/a.py"))
            .expect("under-root path must construct");
        assert_eq!(inside.as_str(), "src/a.py");
    }

    // ── RepoRelativePath::from_repo_relative_str (#199 review P1-2) ──────────

    #[test]
    fn repo_relative_str_accepts_canonical_paths() {
        for ok in [
            "main.rs",
            "src/models/user.py",
            "a.b.c.rs",
            "deep/nested/dir/x.ts",
        ] {
            assert_eq!(
                RepoRelativePath::from_repo_relative_str(ok)
                    .expect("canonical")
                    .as_str(),
                ok
            );
        }
    }

    #[test]
    fn repo_relative_str_rejects_identity_aliases() {
        // #199 review P1-2: beyan edilen yeni-düğüm path'i ölçülen kimlik ekseniyle
        // AYNI kanonik biçimde olmalı — alias, ikinci bir düğüm olarak sayılırdı.
        for bad in [
            "",            // boş
            "./main.rs",   // nokta-segment alias
            "main.rs/",    // sondaki ayraç → boş segment
            "src//x.rs",   // çift ayraç → boş segment
            "../x.rs",     // traversal
            "src/../x.rs", // gömülü traversal
            "/tmp/x.rs",   // absolute
            "src\\x.rs",   // Windows ayracı
            ".",           // nokta kökü
        ] {
            assert!(
                RepoRelativePath::from_repo_relative_str(bad).is_none(),
                "{bad:?} must be rejected as non-canonical"
            );
        }
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
            path: RepoRelativePath::from_absolute(Path::new("/repo"), Path::new("/repo/a.py"))
                .expect("path is under repo root"),
        }];
        let c = AnalysisCompleteness::from_reasons(reasons.clone());
        assert_eq!(c, AnalysisCompleteness::Partial { reasons });
        assert!(!c.is_complete());
    }

    #[test]
    fn completeness_default_is_complete() {
        assert_eq!(
            AnalysisCompleteness::default(),
            AnalysisCompleteness::Complete
        );
    }
}
