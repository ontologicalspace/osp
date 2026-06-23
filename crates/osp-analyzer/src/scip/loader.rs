//! SCIP index loader — .scip protobuf → SemanticIndex (Faz 3.6).
//!
//! `scip` crate (SourceGraph official) ile .scip dosyasını parse eder.
//! SemanticIndex → LCOM4 cohesion hesabı için kullanılır.
//!
//! SCIP occurrence format:
//! - `symbol`: "python pkg.mod ClassName#method(" veya "python pkg.mod ClassName.field"
//! - `range`: [line, col, end_col] veya [line, col, end_line, end_col]
//! - `symbol_roles`: bitfield (bit 0 = Definition, bit 1 = Import)

use std::collections::HashMap;
use std::path::Path;

use protobuf::Message;

use super::index::{ClassSemanticInfo, FieldAccess, SemanticIndex};

/// SCIP SymbolInformation.Kind'dan çıkarsanan sembol kategorisi (LCOM4 için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferredKind {
    Class,
    Method,
    Field,
    Other,
}

impl InferredKind {
    /// SCIP proto Kind değerinden kategori çıkar (kind != 0 durumu).
    fn from_kind_value(val: i32) -> Self {
        match val {
            // Class-like
            7 | 49 | 53 | 56 | 75 | 33 => Self::Class,
            // Method-like
            26 | 9 | 66 | 68 | 69 | 70 | 71 | 74 | 76 | 80 => Self::Method,
            // Field-like
            15 | 41 | 77 | 79 | 81 | 18 | 45 => Self::Field,
            _ => Self::Other,
        }
    }
}

/// Sembol string'inden descriptor suffix çıkarımı (kind=0 fallback).
///
/// SCIP sembol formatı (descriptor path — son space'ten sonraki segment):
/// - `...ClassName#` → Type (class) — `#` suffix
/// - `...fieldName.` → Term (field) — `.` suffix (ama `).` değil)
/// - `...methodName().` → Method — `().` suffix
/// - `...(paramName)` → Parameter — `)` suffix (içerikli parantez)
/// - `...path/` → Package/path — `/` suffix
fn infer_kind_from_symbol(symbol: &str) -> InferredKind {
    // Son space'ten sonraki segment = descriptor path
    let last = symbol.rsplit(' ').next().unwrap_or(symbol);

    if last.ends_with('#') {
        InferredKind::Class
    } else if last.ends_with(").") {
        // `methodName().` → Method (empty parens + dot separator)
        InferredKind::Method
    } else if last.ends_with(')') {
        // `(paramName)` → Parameter (içerikli parantez) — LCOM4 için "Other"
        InferredKind::Other
    } else if last.ends_with('.') {
        // `fieldName.` → Term (field)
        InferredKind::Field
    } else {
        InferredKind::Other
    }
}

/// SCIP index'ten SemanticIndex kur.
///
/// .scip dosyasını parse eder → class/method/field/field-access çıkarır.
/// LCOM4 hesabı için gereken tüm veriyi toplar. `classes_by_file` map'i
/// pipeline'ın per-module cohesion hesabı için dosya → class ilişkisini saklar.
pub fn load_scip_index(scip_path: &Path) -> anyhow::Result<SemanticIndex> {
    let bytes = std::fs::read(scip_path)?;
    parse_scip_bytes(&bytes)
}

/// SCIP bytes → SemanticIndex (test için ayrılmış).
pub fn parse_scip_bytes(bytes: &[u8]) -> anyhow::Result<SemanticIndex> {
    let index = scip::types::Index::parse_from_bytes(bytes)?;

    let mut classes: Vec<ClassSemanticInfo> = Vec::new();
    let mut classes_by_file: HashMap<String, Vec<ClassSemanticInfo>> = HashMap::new();
    let mut files_indexed = 0;

    for doc in &index.documents {
        if doc.relative_path.is_empty() {
            continue;
        }
        files_indexed += 1;
        // Normalize: SCIP Windows index'ler backslash kullanır, pipeline forward-slash arar.
        // Tüm path separator'ları forward-slash'a normalize et (cross-platform matching).
        let rel_path = doc.relative_path.replace('\\', "/");

        // Collect symbol definitions + occurrences for this document
        let mut class_defs: Vec<(String, String)> = Vec::new(); // (symbol, display_name)
        let mut method_defs: Vec<(String, String, Vec<i32>)> = Vec::new(); // (symbol, name, range)
        let mut field_defs: Vec<(String, String)> = Vec::new(); // (symbol, display_name)

        // SymbolInformation: definitions
        for sym_info in &doc.symbols {
            let symbol = &sym_info.symbol;
            let display = if sym_info.display_name.is_empty() {
                extract_last_segment(symbol)
            } else {
                sym_info.display_name.clone()
            };

            let kind_val = sym_info.kind.value();
            // SCIP SymbolInformation.Kind enum values (scip.proto — SourceGraph):
            // Class-like (LCOM4 subjects — concrete types with method bodies):
            //   7=Class, 49=Struct, 53=Trait, 56=TypeClass, 75=SingletonClass, 33=Object
            // Method-like:
            //   26=Method, 9=Constructor, 66=AbstractMethod, 68=ProtocolMethod,
            //   69=PureVirtualMethod, 70=TraitMethod, 71=TypeClassMethod,
            //   74=MethodAlias, 76=SingletonMethod, 80=StaticMethod
            // Field-like:
            //   15=Field, 41=Property, 77=StaticDataMember, 79=StaticField,
            //   81=StaticProperty, 18=Getter, 45=Setter
            // Ref: scip 0.5 proto — SymbolInformation.Kind enum
            //
            // FALLBACK: bazı indexer'lar (scip-typescript) kind=0 (UnspecifiedKind) bırakır.
            // Bu durumda sembol string'inden descriptor suffix çıkarımı yap.
            let inferred = if kind_val == 0 {
                infer_kind_from_symbol(symbol)
            } else {
                InferredKind::from_kind_value(kind_val)
            };
            match inferred {
                InferredKind::Class => { class_defs.push((symbol.clone(), display)); }
                InferredKind::Method => { method_defs.push((symbol.clone(), display, vec![])); }
                InferredKind::Field => { field_defs.push((symbol.clone(), display)); }
                InferredKind::Other => {}
            }
        }

        // Occurrences: method definition range'lerini topla (sadece Method sembolleri)
        for occ in &doc.occurrences {
            let is_definition = occ.symbol_roles & 1 != 0;
            if !is_definition {
                continue;
            }
            let symbol = &occ.symbol;
            // Sadece method sembolleri topla — class/field/parameter değil
            if infer_kind_from_symbol(symbol) != InferredKind::Method {
                continue;
            }
            let name = extract_last_segment(symbol);
            let range = occ.range.clone();
            if let Some(m) = method_defs.iter_mut().find(|(s, _, _)| s == symbol) {
                m.2 = range;
            } else {
                method_defs.push((symbol.clone(), name, range));
            }
        }

        // For each class: find its methods, fields, and field-accesses
        for (class_symbol, class_name) in &class_defs {
            // class_symbol zaten '#' ile bitiyor (SCIP Type descriptor).
            // Method/field sembolleri doğrudan class_symbol'den sonra gelir:
            //   method: "...ClassName#methodName()." — starts_with(class_symbol)
            //   field:  "...ClassName#fieldName."   — starts_with(class_symbol)
            let class_prefix = class_symbol.clone();
            let class_methods: Vec<(String, Vec<i32>)> = method_defs
                .iter()
                .filter(|(s, _, _)| s.starts_with(&class_prefix))
                .map(|(_, name, range)| (name.clone(), range.clone()))
                .collect();

            let class_fields: Vec<String> = field_defs
                .iter()
                .filter(|(s, _)| s.starts_with(&class_prefix))
                .map(|(_, name)| name.clone())
                .collect();

            // Field access: for each method, find field references within its body.
            //
            // SCIP method definition range sadece method İMZASINI kapsar (örn [3,2,9] =
            // "addItem" identifier'ı line 3 col 2-9). Method BODY'nin range'i verilmez.
            // Çözüm: method'ları definition line'a göre sırala, her method'un body'si
            // kendisinden sonraki method'un definition line'ına kadar uzanır.
            let mut field_accesses = Vec::new();

            // Method'ları start line'a göre sırala
            let mut methods_by_line: Vec<(String, Vec<i32>)> = class_methods
                .iter()
                .map(|(name, range)| (name.clone(), range.clone()))
                .collect();
            methods_by_line.sort_by_key(|(_, range)| range.first().copied().unwrap_or(0));

            for (idx, (method_name, method_range)) in methods_by_line.iter().enumerate() {
                let method_start = method_range.first().copied().unwrap_or(0);
                // Body: bu method'dan sonraki method'a kadar (son method → dosya sonu)
                let method_end = if idx + 1 < methods_by_line.len() {
                    methods_by_line[idx + 1]
                        .1
                        .first()
                        .copied()
                        .unwrap_or(i32::MAX)
                        - 1
                } else {
                    i32::MAX // last method: body extends to end of file
                };

                for occ in &doc.occurrences {
                    let occ_symbol = &occ.symbol;
                    let suffix = occ_symbol.rsplit(' ').next().unwrap_or(occ_symbol);

                    // Skip method occurrences (method definitions set ranges, not field accesses)
                    if suffix.ends_with("().") {
                        continue;
                    }
                    // Skip parameter references: `(paramName)` ile bitenler field değildir
                    if suffix.ends_with(')') && !suffix.ends_with("()") {
                        continue;
                    }
                    // Sadece bu class'ın field'larına erişimleri say
                    // (definition = self.x = ... write, reference = self.x read — ikisi de access)
                    if !occ_symbol.starts_with(&class_prefix) {
                        continue;
                    }
                    let occ_line = occ.range.first().copied().unwrap_or(-1);
                    if occ_line >= method_start && occ_line <= method_end {
                        let field_name = extract_last_segment(occ_symbol);
                        field_accesses.push(FieldAccess {
                            method: method_name.clone(),
                            field: field_name,
                        });
                    }
                }
            }

            if !class_methods.is_empty() || !class_fields.is_empty() {
                let info = ClassSemanticInfo {
                    name: class_name.clone(),
                    methods: class_methods.iter().map(|(n, _)| n.clone()).collect(),
                    fields: class_fields,
                    field_access: field_accesses,
                };
                classes.push(info.clone());
                classes_by_file.entry(rel_path.clone()).or_default().push(info);
            }
        }
    }

    Ok(SemanticIndex {
        classes,
        classes_by_file,
        files_indexed,
        files_total: files_indexed,
    })
}

/// SCIP symbol string'inden son identifier segmenti çıkar.
///
/// SCIP descriptor suffix'leri: `#` (Type), `.` (Term/field), `()` (Method),
/// `(name)` (Parameter), `/` (Package/path).
fn extract_last_segment(symbol: &str) -> String {
    let last_segment = symbol.rsplit(' ').next().unwrap_or(symbol);

    // Parameter: `(identifier)` — içeride basit bir identifier varsa
    if last_segment.ends_with(')') {
        if let Some(open) = last_segment.rfind('(') {
            let inside = &last_segment[open + 1..last_segment.len() - 1];
            if !inside.is_empty()
                && !inside.contains('.')
                && !inside.contains('(')
                && !inside.contains(')')
                && !inside.contains('#')
            {
                return inside.to_string();
            }
        }
    }

    // Trailing descriptor suffix'leri strip et
    let stripped = last_segment
        .trim_end_matches('#') // Type suffix
        .trim_end_matches("().") // Method suffix
        .trim_end_matches('.') // Term suffix
        .trim_end_matches('/'); // Package suffix

    // Son `#` veya `/` delimiter'ından sonraki kısım = identifier
    if let Some(pos) = stripped.rfind(|c: char| c == '#' || c == '/') {
        stripped[pos + 1..].to_string()
    } else {
        stripped.to_string()
    }
}

/// Synthetic SCIP index oluştur (test için).
pub fn build_synthetic_index(classes: Vec<ClassSemanticInfo>) -> SemanticIndex {
    SemanticIndex {
        classes,
        files_indexed: 1,
        files_total: 1,
        ..Default::default()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn class_info(name: &str, methods: &[&str], fields: &[&str], accesses: &[(&str, &str)]) -> ClassSemanticInfo {
        ClassSemanticInfo {
            name: name.into(),
            methods: methods.iter().map(|s| s.to_string()).collect(),
            fields: fields.iter().map(|s| s.to_string()).collect(),
            field_access: accesses.iter().map(|(m, f)| FieldAccess {
                method: m.to_string(), field: f.to_string()
            }).collect(),
        }
    }

    #[test]
    fn extract_last_segment_symbol() {
        // Type (class) — `#` suffix
        assert_eq!(extract_last_segment("scip npm . . path/File.ts/ClassName#"), "ClassName");
        // Method — `().` suffix
        assert_eq!(
            extract_last_segment("scip npm . . path/File.ts/ClassName#methodName()."),
            "methodName"
        );
        // Field — `.` suffix
        assert_eq!(
            extract_last_segment("scip npm . . path/File.ts/ClassName#fieldName."),
            "fieldName"
        );
        // Parameter — `(paramName)` suffix
        assert_eq!(
            extract_last_segment("scip npm . . path/File.ts/ClassName#methodName().(paramName)"),
            "paramName"
        );
        // Bare identifier (no descriptor suffix)
        assert_eq!(extract_last_segment("python pkg.mod ClassName"), "ClassName");
    }

    #[test]
    fn build_synthetic_and_compute_lcom4() {
        // Build synthetic index with known class structure
        let index = build_synthetic_index(vec![
            class_info("Cohesive", &["a", "b", "c"], &["x", "y"], &[
                ("a", "x"), ("b", "y"), ("c", "x"), ("c", "y") // c bridges → LCOM4=1
            ]),
            class_info("Fragmented", &["a", "b"], &["x", "y"], &[
                ("a", "x"), ("b", "y") // no bridge → LCOM4=2
            ]),
        ]);

        assert!(index.is_available());
        assert_eq!(index.classes.len(), 2);

        let results = crate::scip::lcom4::compute_all_lcom4(&index);
        assert_eq!(results[0].1.lcom4, 1, "Cohesive → LCOM4=1");
        assert_eq!(results[1].1.lcom4, 2, "Fragmented → LCOM4=2");
    }

    #[test]
    fn parse_real_scip_bytes_if_available() {
        // This test only runs if a real .scip file is available
        let scip_path = std::path::Path::new("test.scip");
        if !scip_path.exists() {
            eprintln!("Skipping real SCIP test (no test.scip file)");
            return;
        }

        let result = load_scip_index(scip_path);
        assert!(result.is_ok(), "SCIP parse should succeed");
        let index = result.unwrap();
        assert!(index.files_indexed > 0, "Should have indexed files");
    }

    /// End-to-end: synthetic SCIP → SemanticIndex → LCOM4 → MetricValue
    #[test]
    fn end_to_end_synthetic_lcom4_pipeline() {
        // Simulate a real Python class with field access
        let index = build_synthetic_index(vec![
            class_info("Article", &["__init__", "save", "get_summary"], &["title", "body", "tags"], &[
                ("__init__", "title"),
                ("__init__", "body"),
                ("__init__", "tags"),     // __init__ accesses all → bridges
                ("save", "title"),
                ("save", "body"),
                ("get_summary", "body"),
            ]),
        ]);

        let results = crate::scip::lcom4::compute_all_lcom4(&index);
        let lcom4_results: Vec<_> = results.iter().map(|(_, r)| r.clone()).collect();
        let cohesion_mv = crate::scip::lcom4::module_cohesion(&lcom4_results);

        // Article is cohesive (__init__ bridges) → LCOM4=1 → cohesion=1.0
        assert_eq!(results[0].1.lcom4, 1);
        assert!((cohesion_mv.value - 1.0).abs() < 1e-9);
        assert_eq!(cohesion_mv.source, crate::contract::MetricSource::Scip);
    }
}
