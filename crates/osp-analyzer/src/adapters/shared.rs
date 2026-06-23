//! Shared tree-sitter helpers — AST walk patterns reusable across adapters.

use tree_sitter::{Language, Node, Parser};

use crate::contract::ClassDef;

/// Parse source with given language → root Node. Returns None on parse failure.
pub fn parse_root(source: &str, language: Language) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

/// Walk AST, collect import path strings from nodes matching `import_node_kinds`.
/// For Python: `["import_statement", "import_from_statement"]`.
/// For JS/TS: `["import_statement"]`.
pub fn walk_imports(root: Node, source_bytes: &[u8], import_node_kinds: &[&str]) -> Vec<String> {
    let mut imports = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if import_node_kinds.contains(&n.kind()) {
            extract_import_path(&n, source_bytes, &mut imports);
        }
        // Push children right-to-left → processed left-to-right (DFS order fix)
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    imports
}

/// Python: `import_from_statement` → `module_name` field; `import_statement` → `dotted_name` children.
/// JS/TS: `import_statement` → `source` field (string literal, strip quotes).
fn extract_import_path(node: &Node, source: &[u8], imports: &mut Vec<String>) {
    let kind = node.kind();
    if kind == "import_from_statement" {
        // Python: `from foo.bar import x` → module_name = "foo.bar"
        if let Some(module) = node.child_by_field_name("module_name") {
            if let Ok(text) = module.utf8_text(source) {
                imports.push(text.trim().to_string());
            }
        }
    } else if kind == "import_statement" {
        // Check if this is Python (dotted_name children) or JS/TS (source field)
        if let Some(src) = node.child_by_field_name("source") {
            // JS/TS: source is a string literal → strip quotes
            if let Ok(text) = src.utf8_text(source) {
                let stripped = text
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .trim()
                    .to_string();
                if !stripped.is_empty() {
                    imports.push(stripped);
                }
            }
        } else {
            // Python: `import a.b, c.d` → each dotted_name is a separate import
            for i in 0..node.child_count() {
                if let Some(c) = node.child(i) {
                    if c.kind() == "dotted_name" {
                        if let Ok(text) = c.utf8_text(source) {
                            imports.push(text.trim().to_string());
                        }
                    }
                }
            }
        }
    } else if kind == "use_declaration" {
        // Rust: `use crate::foo::Bar;` → strip "use " and ";"
        if let Ok(text) = node.utf8_text(source) {
            let path = text
                .trim_start_matches("use ")
                .trim()
                .trim_end_matches(';')
                .trim();
            // Handle grouped: `use foo::{a, b}` → take "foo"
            let path = path.split('{').next().unwrap_or(path).trim().trim_end_matches("::");
            if !path.is_empty() && path != "self" && path != "crate" && path != "super" {
                imports.push(path.to_string());
            }
        }
    } else if kind == "import_declaration" {
        // Go: walk for interpreted_string_literal children
        let mut stk = vec![*node];
        while let Some(n) = stk.pop() {
            if n.kind() == "interpreted_string_literal" {
                if let Ok(text) = n.utf8_text(source) {
                    let path = text.trim_matches('"').to_string();
                    if !path.is_empty() {
                        imports.push(path);
                    }
                }
            }
            for i in 0..n.child_count() {
                if let Some(c) = n.child(i) {
                    stk.push(c);
                }
            }
        }
    }
}

/// Walk AST, collect class definitions.
/// Matches any node kind containing "class" (class_definition, class_declaration, etc.).
pub fn walk_class_defs(
    root: Node,
    source: &str,
    _class_node_kind: &str,  // ignored — uses contains("class") for robustness
    abstract_patterns: &[&str],
) -> Vec<ClassDef> {
    let source_bytes = source.as_bytes();
    let mut defs = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        // Match actual class definition nodes (not class_body, class_heritage etc.)
        let k = n.kind();
        let is_class_def = k == "class_definition"        // Python
            || k == "class_declaration"                     // JS/TS
            || k == "abstract_class_declaration"            // TS abstract
            || k == "struct_item"                           // Rust concrete
            || k == "trait_item"                            // Rust abstract (trait)
            || k == "enum_item"                             // Rust concrete (enum)
            || k == "type_declaration";                     // Go
        if is_class_def {
            if let Some(def) = extract_class_def(&n, source_bytes, abstract_patterns) {
                defs.push(def);
            }
        }
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    defs
}

fn extract_class_def(
    node: &Node,
    source: &[u8],
    abstract_patterns: &[&str],
) -> Option<ClassDef> {
    let full_text = node.utf8_text(source).ok()?.to_string();
    let is_abstract = abstract_patterns.iter().any(|&p| full_text.contains(p));

    // Robust name search: walk children for first identifier/type_identifier
    let name = find_first_identifier(node, source)?;

    // Robust method search: recursive walk for function_definition/method_definition
    let methods = find_methods(node, source);

    Some(ClassDef {
        name,
        is_abstract,
        methods,
        source_location: node.start_byte(),
    })
}

fn find_first_identifier(node: &Node, source: &[u8]) -> Option<String> {
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        if n.kind() == "identifier"
            || n.kind() == "type_identifier"
            || n.kind() == "property_identifier"
        {
            return n.utf8_text(source).ok().map(|s| s.trim().to_string());
        }
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    None
}

fn find_methods(node: &Node, source: &[u8]) -> Vec<String> {
    let mut methods = Vec::new();
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        if n.kind() == "function_definition" || n.kind() == "method_definition" {
            if let Some(name) = find_first_identifier(&n, source) {
                methods.push(name);
            }
        }
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    methods
}

/// Strip JS/TS file extension from import path.
/// `./foo.js` → `./foo`, `./types.d.ts` → `./types`
pub fn strip_js_extension(s: &str) -> &str {
    for ext in [".d.ts", ".mjs", ".cjs", ".tsx", ".ts", ".jsx", ".js"] {
        if s.ends_with(ext) {
            return &s[..s.len() - ext.len()];
        }
    }
    s
}

/// HashMap-based import resolver — O(depth) build + O(1) lookup.
///
/// Faz 3.9.1 refactor: O(N×M) linear scan → O(N×depth + M) HashMap.
/// django 119s → <2s (60x speedup).
///
/// Her dosya için tüm dotted-path suffix'leri key olarak saklanır:
/// `repo.src.foo.bar` → keys: `bar`, `foo.bar`, `src.foo.bar`, `repo.src.foo.bar`
#[derive(Debug, Clone)]
pub struct ImportResolver {
    map: std::collections::HashMap<String, std::path::PathBuf>,
}impl ImportResolver {
    /// Tüm dosyalardan HashMap kur. O(N × avg_depth).
    pub fn build(all_files: &[std::path::PathBuf]) -> Self {
        let mut map = std::collections::HashMap::new();
        for f in all_files {
            let normalized = path_normalized_dotted(&f.to_string_lossy());
            let parts: Vec<&str> = normalized.split('.').collect();
            for i in 0..parts.len() {
                let key = parts[i..].join(".");
                map.entry(key).or_insert_with(|| f.clone());
            }
        }
        Self { map }
    }

    /// Import path → dosya yolu. O(1) lookup.
    pub fn resolve(&self, import_path: &str) -> Option<&std::path::PathBuf> {
        let cleaned = import_path
            .trim_start_matches("./")
            .trim_start_matches("../");
        let cleaned = strip_js_extension(cleaned);
        let normalized = cleaned.replace(['/', '\\'], ".");
        self.map.get(&normalized)
    }

    /// HashMap entry count (diagnostic).
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Eski linear resolver — geri uyumluluk için (deprecated, ImportResolver kullanın).
#[deprecated(note = "ImportResolver::build + resolve kullanın — O(1) lookup")]
pub fn try_resolve_internal(import_path: &str, all_files: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    ImportResolver::build(all_files).resolve(import_path).cloned()
}

fn path_normalized_dotted(s: &str) -> String {
    // Extract parent dirs + file stem, join with dots
    let path = std::path::Path::new(s);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let parents: Vec<&str> = path
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect()
        })
        .unwrap_or_default();
    if parents.is_empty() {
        stem.to_string()
    } else {
        format!("{}.{}", parents.join("."), stem)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler — ImportResolver (O(1) HashMap lookup, 60x speedup claim)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    // --- ImportResolver::build + resolve roundtrip ---

    #[test]
    fn resolver_resolves_simple_filename() {
        let files = vec![pb("/repo/utils.py")];
        let resolver = ImportResolver::build(&files);
        // Import "utils" → /repo/utils.py
        assert_eq!(resolver.resolve("utils"), Some(&pb("/repo/utils.py")));
    }

    #[test]
    fn resolver_resolves_dotted_path() {
        let files = vec![pb("/repo/src/models/user.py")];
        let resolver = ImportResolver::build(&files);
        // All suffix keys: "user", "models.user", "src.models.user", "repo.src.models.user"
        assert_eq!(resolver.resolve("models.user"), Some(&pb("/repo/src/models/user.py")));
        assert_eq!(
            resolver.resolve("src.models.user"),
            Some(&pb("/repo/src/models/user.py"))
        );
    }

    #[test]
    fn resolver_resolves_path_with_separators() {
        // Import paths use / or \ — resolver normalizes to dots
        let files = vec![pb("/repo/src/foo/bar.py")];
        let resolver = ImportResolver::build(&files);
        assert_eq!(resolver.resolve("src/foo/bar"), Some(&pb("/repo/src/foo/bar.py")));
        assert_eq!(resolver.resolve("src\\foo\\bar"), Some(&pb("/repo/src/foo/bar.py")));
    }

    #[test]
    fn resolver_strips_relative_prefixes() {
        let files = vec![pb("/repo/utils.py")];
        let resolver = ImportResolver::build(&files);
        assert_eq!(resolver.resolve("./utils"), Some(&pb("/repo/utils.py")));
        assert_eq!(resolver.resolve("../utils"), Some(&pb("/repo/utils.py")));
    }

    #[test]
    fn resolver_strips_js_extensions() {
        let files = vec![pb("/repo/components/Button.tsx")];
        let resolver = ImportResolver::build(&files);
        // Import "./Button.tsx" → strip ext → "Button" → resolve
        assert_eq!(
            resolver.resolve("./Button.tsx"),
            Some(&pb("/repo/components/Button.tsx"))
        );
        assert_eq!(
            resolver.resolve("./Button.js"),
            Some(&pb("/repo/components/Button.tsx"))
        );
    }

    #[test]
    fn resolver_returns_none_for_unknown_import() {
        let files = vec![pb("/repo/utils.py")];
        let resolver = ImportResolver::build(&files);
        assert_eq!(resolver.resolve("nonexistent"), None);
        assert_eq!(resolver.resolve("foo.bar.baz"), None);
    }

    #[test]
    fn resolver_handles_multiple_files() {
        let files = vec![
            pb("/repo/main.py"),
            pb("/repo/utils.py"),
            pb("/repo/models/user.py"),
        ];
        let resolver = ImportResolver::build(&files);
        assert_eq!(resolver.resolve("main"), Some(&pb("/repo/main.py")));
        assert_eq!(resolver.resolve("utils"), Some(&pb("/repo/utils.py")));
        assert_eq!(resolver.resolve("user"), Some(&pb("/repo/models/user.py")));
        assert_eq!(resolver.resolve("models.user"), Some(&pb("/repo/models/user.py")));
    }

    #[test]
    fn resolver_len_reflects_all_suffix_keys() {
        // Relative path (cross-platform — no root prefix ambiguity)
        let files = vec![pb("repo/utils.py")];
        let resolver = ImportResolver::build(&files);
        // path_normalized_dotted("repo/utils.py") = "repo.utils"
        // suffixes: "utils", "repo.utils" → 2 keys
        assert_eq!(resolver.len(), 2);
    }

    // --- strip_js_extension ---

    #[test]
    fn strip_js_extension_handles_all_variants() {
        assert_eq!(strip_js_extension("foo.ts"), "foo");
        assert_eq!(strip_js_extension("foo.tsx"), "foo");
        assert_eq!(strip_js_extension("foo.js"), "foo");
        assert_eq!(strip_js_extension("foo.jsx"), "foo");
        assert_eq!(strip_js_extension("foo.mjs"), "foo");
        assert_eq!(strip_js_extension("foo.cjs"), "foo");
        assert_eq!(strip_js_extension("foo.d.ts"), "foo");
    }

    #[test]
    fn strip_js_extension_no_match_returns_original() {
        assert_eq!(strip_js_extension("foo.py"), "foo.py");
        assert_eq!(strip_js_extension("foo"), "foo");
        assert_eq!(strip_js_extension("./utils"), "./utils");
    }

    #[test]
    fn strip_js_extension_prefers_longest_match() {
        // ".d.ts" (5 chars) vs ".ts" (3 chars) — .d.ts should win
        assert_eq!(strip_js_extension("types.d.ts"), "types");
        // But plain .ts on a file named "foo.ts" still works
        assert_eq!(strip_js_extension("foo.ts"), "foo");
    }

    // --- path_normalized_dotted (helper) ---

    #[test]
    fn path_normalized_dotted_extracts_parents_and_stem() {
        // Relative paths (cross-platform — absolute paths include root prefix on Windows)
        assert_eq!(path_normalized_dotted("repo/src/models/user.py"), "repo.src.models.user");
        assert_eq!(path_normalized_dotted("user.py"), "user");
        assert_eq!(path_normalized_dotted("a/b/c.py"), "a.b.c");
    }

    // --- deprecated try_resolve_internal backward compat ---

    #[test]
    #[allow(deprecated)]
    fn deprecated_resolver_matches_new_resolver() {
        let files = vec![pb("/repo/utils.py"), pb("/repo/models/user.py")];
        // Old API
        let old_result = try_resolve_internal("utils", &files);
        // New API
        let resolver = ImportResolver::build(&files);
        let new_result = resolver.resolve("utils").cloned();

        assert_eq!(old_result, new_result);
        assert_eq!(old_result, Some(pb("/repo/utils.py")));
    }
}
