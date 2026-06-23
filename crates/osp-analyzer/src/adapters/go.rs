//! Go adapter — tree-sitter-go.
//!
//! Import patterns: `import "fmt"`, `import "github.com/gin-gonic/gin"`
//! Abstract patterns: `interface X`

use std::path::Path;

use crate::contract::{ClassDef, ImportStatement, ImportKind, ResolvedImport};
use crate::language::{LanguageAdapter, RepoContext};
use super::shared;

pub struct GoAdapter;

impl LanguageAdapter for GoAdapter {
    fn name(&self) -> &str {
        "go"
    }

    fn extensions(&self) -> &[&str] {
        &[".go"]
    }

    fn extract_imports(&self, source: &str) -> Vec<ImportStatement> {
        let tree = match shared::parse_root(source, tree_sitter_go::LANGUAGE.into()) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let paths = shared::walk_imports(tree.root_node(), source.as_bytes(), &["import_declaration"]);
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
        _repo: &RepoContext,
    ) -> Option<ResolvedImport> {
        // Go: no relative imports (Go modules). Everything is a module path.
        // Internal packages (same module) vs external → needs go.mod analysis.
        // For now: everything non-stdlib-looking → External.
        // Go stdlib: single-word paths (fmt, os, net, http, etc.)
        let path = &import.path;
        let is_stdlib = !path.contains('.') && !path.contains('/');
        if is_stdlib {
            return Some(ResolvedImport { kind: ImportKind::StandardLibrary, target_path: None });
        }
        Some(ResolvedImport { kind: ImportKind::External, target_path: None })
    }

    fn extract_class_defs(&self, source: &str) -> Vec<ClassDef> {
        let tree = match shared::parse_root(source, tree_sitter_go::LANGUAGE.into()) {
            Some(t) => t,
            None => return Vec::new(),
        };
        // Go: interface = abstract, struct = concrete
        // type_declaration nodes contain struct_type or interface_type
        shared::walk_class_defs(tree.root_node(), source, "", &["interface"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::RepoContext;
    use std::path::PathBuf;

    #[test]
    fn go_imports_extracted() {
        let src = r#"
package main
import "fmt"
import "github.com/gin-gonic/gin"
"#;
        let adapter = GoAdapter;
        let imports = adapter.extract_imports(src);
        assert!(imports.iter().any(|i| i.path == "fmt"), "{:?}", imports);
        assert!(imports.iter().any(|i| i.path.contains("gin")), "{:?}", imports);
    }

    #[test]
    fn go_interface_is_abstract() {
        let src = "package main\ntype Animal interface { Speak() }\ntype Dog struct {}\n";
        let adapter = GoAdapter;
        let defs = adapter.extract_class_defs(src);
        assert!(defs.iter().any(|d| d.is_abstract), "interface should be abstract");
        assert!(defs.iter().any(|d| !d.is_abstract), "struct should be concrete");
    }

    #[test]
    fn go_resolve_stdlib() {
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = GoAdapter;
        let import = ImportStatement { path: "fmt".into(), source_location: 0 };
        let resolved = adapter.resolve_import(&import, Path::new("/repo/main.go"), &repo).unwrap();
        assert_eq!(resolved.kind, ImportKind::StandardLibrary);
    }

    #[test]
    fn go_resolve_external() {
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = GoAdapter;
        let import = ImportStatement { path: "github.com/gin-gonic/gin".into(), source_location: 0 };
        let resolved = adapter.resolve_import(&import, Path::new("/repo/main.go"), &repo).unwrap();
        assert_eq!(resolved.kind, ImportKind::External);
    }
}
