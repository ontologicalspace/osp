//! Rust adapter — tree-sitter-rust.
//!
//! Import patterns: `use crate::foo`, `use super::bar`, `use std::collections::HashMap`
//! Abstract patterns: `trait X` (trait = abstract interface)

use std::path::Path;

use crate::contract::{ClassDef, ImportStatement, ImportKind, ResolvedImport};
use crate::language::{LanguageAdapter, RepoContext};
use super::shared;

pub struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn name(&self) -> &str {
        "rust"
    }

    fn extensions(&self) -> &[&str] {
        &[".rs"]
    }

    fn extract_imports(&self, source: &str) -> Vec<ImportStatement> {
        let tree = match shared::parse_root(source, tree_sitter_rust::LANGUAGE.into()) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let paths = shared::walk_imports(tree.root_node(), source.as_bytes(), &["use_declaration"]);
        paths
            .into_iter()
            .enumerate()
            .map(|(i, path)| ImportStatement { path, source_location: i })
            .collect()
    }

    fn resolve_import(
        &self,
        import: &ImportStatement,
        _from_file: &Path,
        repo: &RepoContext,
    ) -> Option<ResolvedImport> {
        // crate:: or super:: or self:: → internal
        if import.path.starts_with("crate::") || import.path.starts_with("super::") || import.path.starts_with("self::") {
            if let Some(target) = repo.resolver.resolve(&import.path).cloned() {
                return Some(ResolvedImport { kind: ImportKind::Internal, target_path: Some(target) });
            }
            return Some(ResolvedImport { kind: ImportKind::Unknown, target_path: None });
        }
        // std:: → standard library
        if import.path.starts_with("std::") || import.path == "std" {
            return Some(ResolvedImport { kind: ImportKind::StandardLibrary, target_path: None });
        }
        // Otherwise → external crate
        Some(ResolvedImport { kind: ImportKind::External, target_path: None })
    }

    fn extract_class_defs(&self, source: &str) -> Vec<ClassDef> {
        let tree = match shared::parse_root(source, tree_sitter_rust::LANGUAGE.into()) {
            Some(t) => t,
            None => return Vec::new(),
        };
        // Rust: trait = abstract, struct/enum = concrete
        shared::walk_class_defs(tree.root_node(), source, "", &["trait "])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::RepoContext;
    use std::path::PathBuf;

    #[test]
    fn rust_imports_extracted() {
        let src = "use std::collections::HashMap;\nuse crate::foo::Bar;\nuse serde::Serialize;\n";
        let adapter = RustAdapter;
        let imports = adapter.extract_imports(src);
        assert!(imports.iter().any(|i| i.path.contains("std")), "{:?}", imports);
        assert!(imports.iter().any(|i| i.path.contains("crate")), "{:?}", imports);
        assert!(imports.iter().any(|i| i.path.contains("serde")), "{:?}", imports);
    }

    #[test]
    fn rust_trait_is_abstract() {
        let src = "trait Animal { fn speak(&self); }\nstruct Dog;\nimpl Animal for Dog { fn speak(&self) {} }\n";
        let adapter = RustAdapter;
        let defs = adapter.extract_class_defs(src);
        assert!(defs.iter().any(|d| d.is_abstract), "trait should be abstract");
        assert!(defs.iter().any(|d| !d.is_abstract), "struct should be concrete");
    }

    #[test]
    fn rust_resolve_stdlib() {
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = RustAdapter;
        let import = ImportStatement { path: "std::collections::HashMap".into(), source_location: 0 };
        let resolved = adapter.resolve_import(&import, Path::new("/repo/main.rs"), &repo).unwrap();
        assert_eq!(resolved.kind, ImportKind::StandardLibrary);
    }

    #[test]
    fn rust_resolve_external() {
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = RustAdapter;
        let import = ImportStatement { path: "serde::Serialize".into(), source_location: 0 };
        let resolved = adapter.resolve_import(&import, Path::new("/repo/main.rs"), &repo).unwrap();
        assert_eq!(resolved.kind, ImportKind::External);
    }
}
