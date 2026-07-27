//! Characterization tests for PR A (declaration-policy extraction).
//!
//! These tests pin the EXACT `extract_class_defs` output — the full
//! `(name, is_abstract)` list per adapter, with exact counts — across the
//! `shared::walk_class_defs` refactor. They are deliberately behavior-freezing:
//! a test name here does NOT assert the behavior is *correct*, only that the
//! refactor did not *change* it.
//!
//! Several tests encode KNOWN-DIVERGENCE cases (pre-existing bugs preserved
//! verbatim by PR A). Fixing any of them is a separate bug-fix PR that must
//! rerun the corpus and update these tests as a deliberate, documented event.
//!
//! Coverage rationale: the pre-existing inline adapter unit tests only checked
//! `is_abstract` presence via `.any(...)` and rarely the exact count or the
//! `name` field. That leaves grouped-type under-count and name extraction
//! unguarded. These characterization tests close those gaps.

use osp_analyzer::adapters::go::GoAdapter;
use osp_analyzer::adapters::javascript::JavaScriptAdapter;
use osp_analyzer::adapters::python::PythonAdapter;
use osp_analyzer::adapters::rust::RustAdapter;
use osp_analyzer::adapters::typescript::TypeScriptAdapter;
use osp_analyzer::language::LanguageAdapter;

/// Collect (name, is_abstract) pairs in the order `extract_class_defs` returns
/// them. Order matters: the refactor must preserve DFS/push order verbatim.
fn defs_of(adapter: &dyn LanguageAdapter, src: &str) -> Vec<(String, bool)> {
    adapter
        .extract_class_defs(src)
        .into_iter()
        .map(|d| (d.name, d.is_abstract))
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Python
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn characterization_python_abc_abstract_plain_concrete() {
    let src = "\
class Animal(ABC):
    def speak(self): pass

class Dog(Animal):
    def bark(self): pass
";
    let got = defs_of(&PythonAdapter, src);
    assert_eq!(
        got,
        vec![
            ("Animal".to_string(), true),  // (ABC) → LegacyTextContains hit
            ("Dog".to_string(), false),    // plain → concrete
        ]
    );
}

#[test]
fn characterization_python_protocol_abstract() {
    let src = "\
class Comparable(Protocol):
    def cmp(self): pass
";
    assert_eq!(defs_of(&PythonAdapter, src), vec![("Comparable".to_string(), true)]);
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeScript
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn characterization_ts_class_interface_abstract_alias() {
    let src = "\
abstract class Shape { abstract area(): number; }
class Circle extends Shape { area() { return 1; } }
interface Drawable { draw(): void; }
type Handler = () => void;
";
    let got = defs_of(&TypeScriptAdapter, src);
    // abstract_class_declaration → Always (text contains 'abstract class')
    // class_declaration          → LegacyTextContains (no 'abstract class'/'interface ')
    // interface_declaration      → Always (old force_abstract)
    // type_alias_declaration     → Always (old force_abstract)
    assert_eq!(
        got,
        vec![
            ("Shape".to_string(), true),
            ("Circle".to_string(), false),
            ("Drawable".to_string(), true),
            ("Handler".to_string(), true),
        ]
    );
}

#[test]
fn characterization_ts_enum_not_counted() {
    // PRESERVED EXCLUSION: `enum_declaration` exists in the TS grammar but was
    // never in the old is_class_def list → not counted. Adding it would change
    // Nc and break bit-identical. This test fails loudly if someone adds it.
    let src = "enum Color { Red, Green, Blue }\n";
    assert_eq!(defs_of(&TypeScriptAdapter, src), Vec::<(String, bool)>::new());
}

#[test]
fn characterization_ts_anonymous_class_expr_not_counted() {
    // PRESERVED EXCLUSION: anonymous `class` expression node is not a
    // `class_declaration` → not counted.
    let src = "const C = class { m() {} };\n";
    assert_eq!(defs_of(&TypeScriptAdapter, src), Vec::<(String, bool)>::new());
}

// ─────────────────────────────────────────────────────────────────────────────
// JavaScript
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn characterization_js_class_always_concrete() {
    let src = "\
class Animal { speak() {} }
class Dog extends Animal { bark() {} }
";
    assert_eq!(
        defs_of(&JavaScriptAdapter, src),
        vec![
            ("Animal".to_string(), false),
            ("Dog".to_string(), false),
        ]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Rust
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn characterization_rust_trait_abstract_struct_enum_concrete() {
    let src = "\
pub trait Speak { fn speak(&self); }
pub struct Dog { name: String }
pub enum Color { Red, Green }
";
    let got = defs_of(&RustAdapter, src);
    // trait_item text contains 'trait ' → abstract; struct/enum do not → concrete.
    assert_eq!(
        got,
        vec![
            ("Speak".to_string(), true),
            ("Dog".to_string(), false),
            ("Color".to_string(), false),
        ]
    );
}

#[test]
fn characterization_rust_abst_003_struct_with_trait_word_is_abstract() {
    // KNOWN-DIVERGENCE(RUST-ABST-003): a struct whose text contains the substring
    // "trait " (here in a doc comment) is marked abstract by the legacy substring
    // test. This is a PRESERVED bug. If a fix flips this to concrete, that is a
    // deliberate bug-fix PR and this test must be updated there — not silently.
    let src = "\
/// Implements the Speak trait for dogs.
pub struct Dog { name: String }
";
    let got = defs_of(&RustAdapter, src);
    assert_eq!(got, vec![("Dog".to_string(), true)]); // <-- preserved false-positive
}

// ─────────────────────────────────────────────────────────────────────────────
// Go
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn characterization_go_interface_abstract_struct_concrete() {
    let src = "\
package main
type Animal interface { Speak() }
type Dog struct { Name string }
";
    let got = defs_of(&GoAdapter, src);
    assert_eq!(
        got,
        vec![
            ("Animal".to_string(), true),
            ("Dog".to_string(), false),
        ]
    );
}

#[test]
fn characterization_go_abst_002_struct_with_interface_field_is_abstract() {
    // KNOWN-DIVERGENCE(GO-ABST-002): a struct with an `interface{}` field has the
    // substring "interface" in its full text → marked abstract (false-positive).
    // PRESERVED verbatim.
    let src = "\
package main
type Registry struct { Handler interface{} }
";
    let got = defs_of(&GoAdapter, src);
    assert_eq!(got, vec![("Registry".to_string(), true)]); // <-- preserved false-positive
}

#[test]
fn characterization_go_type_001_grouped_type_counted_as_one() {
    // KNOWN-DIVERGENCE(GO-TYPE-001): a grouped `type ( A ...; B ... )` block is a
    // single `type_declaration` wrapper node → counted as ONE ClassDef, and
    // FirstIdentifierFallback returns only the first inner name ("Alpha").
    // A correct implementation would yield TWO defs. PRESERVED verbatim; fixing it
    // (per-`type_spec` counting) is a separate bug-fix PR that will change Go Nc.
    let src = "\
package main
type (
    Alpha struct { x int }
    Beta  struct { y int }
)
";
    let got = defs_of(&GoAdapter, src);
    assert_eq!(got.len(), 1, "grouped type under-counted (preserved)");
    assert_eq!(got[0].0, "Alpha", "only first inner name (preserved)");
}
