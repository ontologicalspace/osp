//! Architecture guard — EI3-a: resolution API evidence isolation.
//!
//! Paper 3 v1.4 evidence-identity layer (§3.5, Table EI row EI3-a).
//!
//! ## Normative claim (narrow)
//!
//! The evaluated public resolution API signatures do not accept or expose the listed
//! evidence-source or evidence-mutation capability types. This is an API-shape policy
//! verified by a static architecture guard — it is not a type-level impossibility
//! (violation can be written, the guard catches it before merge).
//!
//! Enforcement class: **ARCH-GUARD** (API-shape policy + static source-scan test).
//!
//! ## What this guard checks
//!
//! The public resolution surface — `apply_resolution`, `CodeEntityResolutionSession::resolve`,
//! `ResolutionApplication`, `PresentedResolutionBasis`, `ResolutionRecord` — must not carry
//! evidence-source or evidence-mutation capability types in their public signatures
//! (function parameters, return types, struct fields reachable through public accessors).
//!
//! Forbidden tokens (capability surface — generic `<P: CodeEvidenceProvider>` atlatma önlemi
//! için tüm ilgili type isimleri listede):
//! - `CodeEvidenceProvider` (mutable provider capability trait — `&mut dyn` veya generic bound)
//! - `ResolvedCodeEvidenceProvider` (PR F adapter — lookup+source compose)
//! - `CodeEvidenceSource` (key-facing evidence source trait)
//! - `ObservedCodeEvidence` (evidence object)
//! - `InMemoryCodeEvidenceSource` (source implementation)
//! - `&mut dyn CodeEvidenceProvider` (mutable evidence provider capability)
//!
//! ## What this guard does NOT check
//!
//! - Type-level impossibility (a determined caller could add the token; the guard surfaces
//!   it as a CI failure before merge).
//! - Internal implementation usage (a private helper inside a module may legitimately
//!   compose evidence for its own purposes — the claim is about the public resolution API
//!   surface, not the whole module).
//! - That resolution "can never mutate evidence" in a global sense — only that the evaluated
//!   public signatures do not accept or expose the capability.
//!
//! ## Relationship to existing guards
//!
//! `crates/osp-cli/tests/architecture_guards.rs` guards CLI production source for evidence
//! construction ownership. This guard is the osp-core analog for the resolution API surface
//! (a negative-capability claim: the resolution API does not carry evidence capability).

use std::path::Path;

/// Forbidden evidence-capability tokens that must not appear in the public resolution
/// API signature surface.
const FORBIDDEN_TOKENS: &[&str] = &[
    "CodeEvidenceProvider",
    "ResolvedCodeEvidenceProvider",
    "CodeEvidenceSource",
    "ObservedCodeEvidence",
    "InMemoryCodeEvidenceSource",
];

/// Public resolution API surface item names — the signatures we isolate.
const RESOLUTION_API_ITEMS: &[&str] = &[
    "apply_resolution",
    "CodeEntityResolutionSession",
    "ResolutionApplication",
    "PresentedResolutionBasis",
    "ResolutionRecord",
];

/// Strip Rust line comments (`//...` and `///...`) and block comments (`/* ... */`)
/// from source text. Prevents documentation/comment false positives.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut str_delim = '\0';
    let mut in_char = false;
    let mut prev = '\0';

    while let Some(c) = chars.next() {
        // String/char literal tracking (skip comment detection inside literals).
        if !in_char && !in_str && (c == '"' || c == '\'') {
            // Heuristic: if previous non-ws was an identifier char or `)`, it's a lifetime/label
            // or a char close; we still treat as string/char start conservatively except lifetimes.
            if c == '\'' && (prev.is_alphanumeric() || prev == '_' || prev == ')') {
                // Likely a lifetime — don't enter string mode. Emit and continue.
                out.push(c);
                prev = c;
                continue;
            }
            if c == '"' {
                in_str = true;
                str_delim = '"';
                out.push(c);
                prev = c;
                continue;
            } else {
                in_char = true;
                str_delim = '\'';
                out.push(c);
                prev = c;
                continue;
            }
        }
        if in_str {
            out.push(c);
            if c == str_delim && prev != '\\' {
                in_str = false;
            }
            prev = c;
            continue;
        }
        if in_char {
            out.push(c);
            if c == str_delim && prev != '\\' {
                in_char = false;
            }
            prev = c;
            continue;
        }
        // Comment detection.
        if c == '/' {
            if let Some(&n) = chars.peek() {
                if n == '/' {
                    // Line comment — skip to end of line.
                    for cc in chars.by_ref() {
                        if cc == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    prev = '\n';
                    continue;
                }
                if n == '*' {
                    // Block comment — skip to closing `*/`.
                    chars.next(); // consume '*'
                    let mut prev_star = false;
                    while let Some(cc) = chars.next() {
                        if prev_star && cc == '/' {
                            break;
                        }
                        prev_star = cc == '*';
                    }
                    out.push(' ');
                    prev = ' ';
                    continue;
                }
            }
        }
        out.push(c);
        prev = c;
    }
    out
}

/// Collect `pub` signature lines (function signatures, struct/enum/impl blocks) from
/// comment-stripped source. A "signature line" is a line containing a `pub` keyword
/// (or within a known public item context) that we treat as the API surface.
///
/// For robustness, this returns any line that either:
/// (a) contains a forbidden token AND a resolution API item name (direct collision), or
/// (b) is a `pub fn`/`pub struct`/`pub enum`/`pub trait`/`impl` signature line containing
///     a resolution API item name.
///
/// We then check forbidden-token presence within those collected blocks.
fn collect_resolution_api_signature_lines(stripped: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let lines: Vec<&str> = stripped.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        // Detect public signature anchors.
        let is_pub_sig = trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub(crate) fn ")
            || trimmed.starts_with("pub unsafe fn ")
            || trimmed.starts_with("pub const fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub(crate) struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub(crate) enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("pub(crate) trait ")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("impl<");

        if !is_pub_sig {
            continue;
        }

        // Collect a signature block: the anchor line + continuation lines until we hit
        // the opening `{` or terminating `;` (signatures may span multiple lines).
        // We cap at 8 lines so a stray missing brace does not swallow the whole file.
        let mut block = String::new();
        for cont in lines[i..].iter().take(8) {
            block.push_str(cont);
            block.push('\n');
            if cont.contains('{') || cont.trim_end().ends_with(';') {
                break;
            }
        }

        // Check if this signature block mentions a resolution API item.
        let mentions_resolution_api =
            RESOLUTION_API_ITEMS.iter().any(|item| block.contains(item));
        if mentions_resolution_api {
            hits.push(block);
        }
    }

    hits
}

/// Recursively collect `.rs` files under a directory.
#[allow(dead_code)]
fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// EI3-a: the public resolution API signatures do not accept or expose evidence-source
/// or evidence-mutation capability types.
///
/// This guard scans the three modules that define the resolution surface and asserts
/// that no public signature block mentioning a resolution API item also contains a
/// forbidden evidence-capability token.
#[test]
fn resolution_api_has_no_evidence_capability() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let anchoring_dir = manifest.join("src/anchoring");

    // The resolution surface lives across these modules.
    let targets = [
        anchoring_dir.join("review.rs"),
        anchoring_dir.join("store.rs"),
        anchoring_dir.join("resolved_implementation.rs"),
    ];

    let mut violations: Vec<String> = Vec::new();

    for target in &targets {
        let src = std::fs::read_to_string(target)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", target.display()));
        let stripped = strip_comments(&src);
        let sig_blocks = collect_resolution_api_signature_lines(&stripped);

        for block in &sig_blocks {
            for token in FORBIDDEN_TOKENS {
                if block.contains(token) {
                    // Locate the first line of the block for a clearer error.
                    let first_line = block.lines().next().unwrap_or("").trim();
                    violations.push(format!(
                        "{}: forbidden evidence-capability token `{token}` in resolution API signature block: `{first_line}`",
                        target.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "EI3-a violation — resolution API signatures carry evidence-capability types \
             (expected: API-shape isolation, ARCH-GUARD):\n  - {}\n\n\
             Normative claim: the evaluated public resolution API signatures do not accept \
             or expose the listed evidence-source or evidence-mutation capability types. \
             If a signature legitimately needs an evidence capability, the claim must be \
             narrowed or the API redesigned; do not weaken this guard without updating \
             manuscript §3.5 Table EI row EI3-a.",
            violations.join("\n  - ")
        );
    }

    // Positive assertion: the guard ran and found the resolution surface (sanity check
    // that the scan is not silently a no-op due to a path change). We verify at least one
    // of the target files contains the apply_resolution trait method.
    let mut found_apply_resolution = false;
    for target in &targets {
        let src = std::fs::read_to_string(target).unwrap_or_default();
        if src.contains("fn apply_resolution") {
            found_apply_resolution = true;
            break;
        }
    }
    assert!(
        found_apply_resolution,
        "EI3-a guard sanity check failed: `fn apply_resolution` not found in scanned targets — \
         scan paths may be stale"
    );
}

/// Sanity test: the comment-stripper correctly ignores documentation comments that
/// mention forbidden tokens. Prevents false positives from doc comments like this file
/// or the resolution modules' own documentation.
#[test]
fn strip_comments_removes_doc_lines_mentioning_forbidden_tokens() {
    let src = r#"
/// Documents that `ObservedCodeEvidence` is NOT used here.
pub fn apply_resolution(&mut self, app: ResolutionApplication) -> Result<ResolutionRecord, Self::Error> {
    // Internal note: CodeEvidenceProvider is unrelated.
    unimplemented!()
}
"#;
    let stripped = strip_comments(src);
    assert!(
        !stripped.contains("Documents that"),
        "doc comment should be stripped"
    );
    assert!(
        !stripped.contains("Internal note"),
        "line comment should be stripped"
    );
    // The signature itself is preserved.
    assert!(stripped.contains("pub fn apply_resolution"));
    assert!(stripped.contains("ResolutionApplication"));
    assert!(stripped.contains("ResolutionRecord"));
}
