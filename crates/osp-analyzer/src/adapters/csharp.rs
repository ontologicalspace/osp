//! C# adapter — tree-sitter-c-sharp (#137 Aşama 1).
//!
//! Import patterns: `using N.T;`, `using static N.T.M;`, `global using N;`,
//! `using Alias = N.T;` (hedef alınır — bağımlılık alias'ta değil hedefte).
//! Abstract patterns: `abstract` modifier (class/struct/record), interface
//! (daima abstract — Martin), enum (daima concrete).
//!
//! **Resolution (üç kademe):**
//! 1. **Namespace-backed:** repo içinde declare edilmiş namespace ise Internal
//!    (`CSharpNamespaceIndex` — `namespace N.S;` bildirimlerinden kurulur; Rust'ın
//!    `crate::` işaretine karşılık C#'ta sözdizimsel internal işaret olmadığından
//!    declare edilmişlik o kanıtı sağlar). Kenar hedefi temsilci dosyadır
//!    (sorted-first, deterministik — GoPackageIndex ilkesi).
//! 2. **Type-backed:** `using App.Svc.MailService;` → ImportResolver suffix
//!    key'leri (dizin ≈ namespace + tip ≈ dosya stem'i konvansiyonu,
//!    trailing-segment drop).
//! 3. BCL kökleri (`System`/`Microsoft`/`netstandard`/`Windows`) →
//!    StandardLibrary; kalan → External.
//!
//! Tier-1 sınırı: repo'da declare edilmemiş VE dosyaya oturmayan using'ler
//! External sınıflanır (üçüncü parti ya da çözülemeyen) — Tier-2 SCIP
//! (Aşama 2) gerçek sembol grafiyle kapatır.

use std::path::Path;

use super::shared;
use crate::contract::{
    ClassDef, ImportKind, ImportStatement, ResolvedImport, TypeReferenceResolution,
};
use crate::language::{LanguageAdapter, RepoContext};

pub struct CSharpAdapter;

impl LanguageAdapter for CSharpAdapter {
    fn name(&self) -> &str {
        "csharp"
    }

    fn extensions(&self) -> &[&str] {
        &[".cs"]
    }

    fn extract_imports(&self, source: &str) -> Vec<ImportStatement> {
        let tree = match shared::parse_root(source, tree_sitter_c_sharp::LANGUAGE.into()) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let paths = shared::walk_imports(tree.root_node(), source.as_bytes(), &["using_directive"]);
        paths
            .into_iter()
            .enumerate()
            .map(|(i, path)| ImportStatement {
                path,
                source_location: i,  // approximate
                is_type_only: false, // C#'ta using seviyesinde type-only kavramı yok
            })
            .collect()
    }

    fn resolve_import(
        &self,
        import: &ImportStatement,
        _from_file: &Path,
        repo: &RepoContext,
    ) -> Option<ResolvedImport> {
        let dotted = import.path.trim().replace('/', ".");
        let parts: Vec<&str> = dotted.split('.').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Some(ResolvedImport {
                kind: ImportKind::External,
                target_path: None,
            });
        }
        // KADE 1 — namespace-backed: `using N.S;` (baskın form). Exact match.
        if let Some(target) = repo.csharp_namespace_index.resolve(&dotted) {
            return Some(ResolvedImport {
                kind: ImportKind::Internal,
                target_path: Some(target.clone()),
            });
        }
        // KADE 2 — type-backed: trailing-segment drop (resolve_rust_use tarzı;
        // crate::-önek stripping YOK): "App.Svc.MailService" → "App.Svc" → "App".
        for end in (1..=parts.len()).rev() {
            if let Some(target) = repo.resolver.resolve(&parts[..end].join(".")) {
                return Some(ResolvedImport {
                    kind: ImportKind::Internal,
                    target_path: Some(target.clone()),
                });
            }
        }
        // KADE 3 — BCL kökleri → StandardLibrary (Tier-1 yaklaşım), kalan → External.
        match parts[0] {
            "System" | "Microsoft" | "netstandard" | "Windows" => Some(ResolvedImport {
                kind: ImportKind::StandardLibrary,
                target_path: None,
            }),
            _ => Some(ResolvedImport {
                kind: ImportKind::External,
                target_path: None,
            }),
        }
    }

    fn extract_class_defs(&self, source: &str) -> Vec<ClassDef> {
        let tree = match shared::parse_root(source, tree_sitter_c_sharp::LANGUAGE.into()) {
            Some(t) => t,
            None => return Vec::new(),
        };
        // C# için tasarlanmış spec'ler (shared.rs "PR C (C#)" notları):
        // - DirectField("name"): `[Attribute]`-öncesi-isim hatasını tip-düzeyinde
        //   kapatır (naive first-identifier attribute adını döndürürdü).
        // - DirectModifier("abstract"): modifier ÇOCUK düğümlerinden okur — sıra
        //   bağımsız (`public abstract partial`), nested-type text bleed yok.
        // - interface → Always (Martin), enum → Never.
        // NOT: `static class` impliclit abstract'tır ama Tier-1'de yalnız explicit
        // `abstract` modifier sayılır — belgeli sınırlama.
        use shared::{AbstractnessRule, DeclarationKindSpec, NameStrategy};
        const CSHARP_SPECS: &[DeclarationKindSpec] = &[
            DeclarationKindSpec::new(
                "class_declaration",
                AbstractnessRule::DirectModifier("abstract"),
                NameStrategy::DirectField("name"),
            ),
            DeclarationKindSpec::new(
                "struct_declaration",
                AbstractnessRule::DirectModifier("abstract"),
                NameStrategy::DirectField("name"),
            ),
            DeclarationKindSpec::new(
                "record_declaration",
                AbstractnessRule::DirectModifier("abstract"),
                NameStrategy::DirectField("name"),
            ),
            DeclarationKindSpec::new(
                "interface_declaration",
                AbstractnessRule::Always,
                NameStrategy::DirectField("name"),
            ),
            DeclarationKindSpec::new(
                "enum_declaration",
                AbstractnessRule::Never,
                NameStrategy::DirectField("name"),
            ),
        ];
        shared::walk_class_defs_with_specs(tree.root_node(), source, CSHARP_SPECS)
    }

    /// #167: tip-düzeyi referans çözümlemesi — dondurulmuş kurallar (issue
    /// #167 tasarım yorumu, "Eşleşme kuralları" 1-7):
    ///
    /// 1. Tip evreni: using'd ns altında repo'da declare edilen tipler
    ///    (partial → çoklu dosya, her dosyaya ayrı kenar; temsilci YOK).
    /// 2. Eşleşme: identifier düğümleri (comment/string node-kind doğal dışlanır;
    ///    using-direktifi alt ağacı ve ns-bildirim adı HARİÇ), case-sensitive tam ad.
    /// 3. Çıkarımlar: dosyanın KENDİ tip adları + kendi tip-parametre adları.
    /// 4. Belirsizlik: ad birden çok using'd ns'te → kenar YOK + ambiguity raporu
    ///    (CS0104 aynası — nitelikli kullanım Tier-1'de atırt edilemez).
    /// 5. Öncelik: kendi-ns bildirimi using'd adayı ezer (C# çözümleme önceliği)
    ///    → SameNsType üretilir, TypeImports üretilmez.
    /// 6. Hiçbir tip geçmiyorsa (unused using): 0 tip kenarı (ns-kenar kalır).
    /// 7. SameNsType: using gerektirmeyen aynı-ns çapraz-dosya referansları.
    ///
    /// **Review P0-1:** hedef kimliği TİP SEMBOLÜDÜR (`TypeImportTarget`
    /// namespace + type_name + file) — aynı dosyadaki 2 tip 2 hedef üretir;
    /// partial tip sembol-başına dosya-başına hedef üretir.
    /// **Review P1-1:** KADE-2 type-backed using'ler (`using static N.T;`,
    /// `using Alias = N.T;`) DIRECTIVE-REFERANSI olarak TypeImports'a girer —
    /// alias kullanımında tip adı kaynakta HİÇ geçmez (`Mail service`), occurrence
    /// match imkânsızdır; using'in kendisi açık tip referansıdır (Tier-1 sınır,
    /// belgelendi). Sembol: hedef dosyanın indeksindeki bildirimden; eşleşme
    /// yoksa path'ten türetilir (son segment = tip, kalanı ns) — deterministik.
    fn resolve_type_references(
        &self,
        source: &str,
        imports: &[ImportStatement],
        from_file: &Path,
        repo: &RepoContext,
    ) -> Option<TypeReferenceResolution> {
        use crate::contract::TypeImportTarget;
        let index = &repo.csharp_namespace_index;

        // Kural 2-3: identifier evreni (kendi tip adları + tip-param adları düşürülür).
        let (identifiers, type_params) = shared::collect_csharp_reference_names(source);
        let own_types = index
            .file_declared_types(from_file)
            .cloned()
            .unwrap_or_default();
        let available: std::collections::BTreeSet<&str> = identifiers
            .iter()
            .map(|s| s.as_str())
            .filter(|n| !type_params.contains(*n) && !own_types.contains(*n))
            .collect();

        // Aday using'd ns'ler: dotted path tam olarak declare edilmiş ns (KADE-1
        // koşulu). Type-backed using'ler (KADE 2) burada elenir ama aşağıda
        // directive-referans olarak ayrıca işlenir (P1-1).
        let candidate_ns: std::collections::BTreeSet<String> = imports
            .iter()
            .map(|imp| imp.path.trim().replace('/', "."))
            .filter(|p| index.is_declared_namespace(p))
            .collect();

        // Kural 7 + 5: kendi-ns tipleri (başka dosyalarda) → SameNsType; adları
        // using'd adaylarını ezer.
        let own_ns: std::collections::BTreeSet<String> = index
            .file_declared_namespaces(from_file)
            .cloned()
            .unwrap_or_default();
        let mut own_ns_names: std::collections::BTreeSet<String> = Default::default();
        let mut same_ns_targets: std::collections::BTreeSet<TypeImportTarget> = Default::default();
        for ns in &own_ns {
            if let Some(types) = index.types_under(ns) {
                for (name, files) in types {
                    if available.contains(name.as_str()) {
                        own_ns_names.insert(name.clone());
                        for f in files {
                            if f != from_file {
                                same_ns_targets.insert(TypeImportTarget {
                                    namespace: ns.clone(),
                                    type_name: name.clone(),
                                    file: f.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Kural 1-4-6: TypeImports (KADE-1) + belirsizlik.
        let mut type_import_targets: std::collections::BTreeSet<TypeImportTarget> =
            Default::default();
        let mut ambiguous: Vec<String> = Vec::new();
        for name in &available {
            let decl_ns: Vec<&String> = candidate_ns
                .iter()
                .filter(|ns| {
                    index
                        .types_under(ns.as_str())
                        .is_some_and(|types| types.contains_key(*name))
                })
                .collect();
            if decl_ns.is_empty() {
                continue; // bu using'd ns'lerde geçen bir tip değil (kural 6 zuhur etmedi)
            }
            if own_ns_names.contains(*name) {
                continue; // kural 5: kendi-ns bildirimi kazandı (SameNsType üretildi)
            }
            if decl_ns.len() > 1 {
                ambiguous.push((*name).to_string()); // kural 4: CS0104 aynası
                continue;
            }
            for f in &index.types_under(decl_ns[0].as_str()).unwrap()[*name] {
                if f != from_file {
                    type_import_targets.insert(TypeImportTarget {
                        namespace: decl_ns[0].clone(),
                        type_name: (*name).to_string(),
                        file: f.clone(),
                    });
                }
            }
        }

        // P1-1: KADE-2 directive-referansları. KADE-1 olmayan her using için
        // mevcut suffix-resolver koşulunu (uzuntan-kısa önek denemesi) tekrarla;
        // repo içi dosyaya çözümlenirse directive'in adlandırdığı TİP bir
        // bağımlılıktır. Sembol: hedef dosya bu adı declare ediyorsa indeks
        // ns'i; yoksa path'ten türetme (son segment tip, kalanı ns).
        for imp in imports {
            let dotted = imp.path.trim().replace('/', ".");
            let parts: Vec<&str> = dotted.split('.').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() || index.is_declared_namespace(&dotted) {
                continue; // KADE-1 (yukarıda işlendi) ya da boş
            }
            let mut resolved: Option<(std::path::PathBuf, &str)> = None;
            for end in (1..=parts.len()).rev() {
                if let Some(target) = repo.resolver.resolve(&parts[..end].join(".")) {
                    resolved = Some((target.clone(), parts[end - 1]));
                    break;
                }
            }
            let Some((target_file, type_name)) = resolved else {
                continue; // BCL/external — tip-gren katkısı yok
            };
            if target_file == from_file {
                continue;
            }
            // Sembol ns'i: hedef dosyada bu adda bildirim varsa indeksin ns'i;
            // yoksa path'ten türetme (son segment tip, kalanı ns; tek segmentse
            // global ns) — deterministik.
            let namespace = index
                .find_type_declaration(target_file.as_path(), type_name)
                .unwrap_or_else(|| {
                    if parts.len() >= 2 {
                        parts[..parts.len() - 1].join(".")
                    } else {
                        String::new()
                    }
                });
            type_import_targets.insert(TypeImportTarget {
                namespace,
                type_name: type_name.to_string(),
                file: target_file,
            });
        }

        Some(TypeReferenceResolution {
            type_import_targets: type_import_targets.into_iter().collect(),
            same_ns_targets: same_ns_targets.into_iter().collect(),
            ambiguous_type_names: ambiguous,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::RepoContext;
    use std::path::PathBuf;

    #[test]
    fn csharp_imports_all_using_forms_extracted() {
        let src = "\
using System;
using static System.Math;
global using System.Collections.Generic;
using Json = Newtonsoft.Json;
using App.Core.Domain;
";
        let adapter = CSharpAdapter;
        let imports = adapter.extract_imports(src);
        let paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"System"), "{paths:?}");
        assert!(paths.contains(&"System.Math"), "using static: {paths:?}");
        assert!(
            paths.contains(&"System.Collections.Generic"),
            "global using: {paths:?}"
        );
        assert!(
            paths.contains(&"Newtonsoft.Json"),
            "alias hedefi (sol taraf DEĞİL): {paths:?}"
        );
        assert!(
            !paths.contains(&"Json"),
            "alias adı import olmamalı: {paths:?}"
        );
        assert!(paths.contains(&"App.Core.Domain"), "{paths:?}");
    }

    #[test]
    fn csharp_class_defs_abstractness_matrix() {
        let src = "\
public abstract class Shape { public abstract double Area(); }
public class Circle : Shape { public override double Area() => 3.14; }
public abstract partial class Widget { }
public sealed struct Point { public int X; }
public abstract record BaseRec;
public record Rec(int A);
public interface IMailer { void Send(); }
public enum Kind { A, B }
";
        let adapter = CSharpAdapter;
        let defs = adapter.extract_class_defs(src);
        let by_name = |n: &str| {
            defs.iter()
                .find(|d| d.name == n)
                .unwrap_or_else(|| panic!("{n} yok: {defs:?}"))
        };

        assert!(by_name("Shape").is_abstract, "abstract class");
        assert!(!by_name("Circle").is_abstract, "concrete class");
        assert!(
            by_name("Widget").is_abstract,
            "abstract + partial (modifier sırası)"
        );
        assert!(!by_name("Point").is_abstract, "sealed struct concrete");
        assert!(by_name("BaseRec").is_abstract, "abstract record");
        assert!(!by_name("Rec").is_abstract, "record concrete");
        assert!(by_name("IMailer").is_abstract, "interface daima abstract");
        assert!(!by_name("Kind").is_abstract, "enum daima concrete");
    }

    #[test]
    fn csharp_attribute_does_not_shadow_class_name() {
        // DirectField("name") pin'i — naive first-identifier `[Serializable]`
        // attribute adını döndürürdü (shared.rs NameStrategy doc gerekçesi).
        let src = "\
[Serializable]
[DebuggerDisplay(\"{Name}\", Name = \"{Name}\")]
public class MailService { }
";
        let adapter = CSharpAdapter;
        let defs = adapter.extract_class_defs(src);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "MailService");
    }

    #[test]
    fn csharp_methods_collected_with_name_field() {
        // `MailResult Send()` — dönüş tipi de identifier; name FIELD'ı
        // döndürülürse "Send", first-identifier olsaydı "MailResult".
        let src = "\
public class MailService {
    public MailResult Send(string to) { return null; }
    public void Flush() { }
}
";
        let adapter = CSharpAdapter;
        let defs = adapter.extract_class_defs(src);
        assert_eq!(defs.len(), 1);
        assert!(
            defs[0].methods.contains(&"Send".to_string()),
            "{:?}",
            defs[0].methods
        );
        assert!(
            defs[0].methods.contains(&"Flush".to_string()),
            "{:?}",
            defs[0].methods
        );
        assert!(
            !defs[0].methods.contains(&"MailResult".to_string()),
            "dönüş tipi metot adı DEĞİL: {:?}",
            defs[0].methods
        );
    }

    #[test]
    fn csharp_resolve_internal_type_backed() {
        // namespace ≈ dizin konvansiyonu: App/Svc/MailService.cs
        // → ImportResolver suffix key "App.Svc.MailService".
        let repo = RepoContext::new(
            PathBuf::from("/repo"),
            vec![
                PathBuf::from("/repo/App/Svc/MailService.cs"),
                PathBuf::from("/repo/Program.cs"),
            ],
        );
        let adapter = CSharpAdapter;
        let import = ImportStatement {
            path: "App.Svc.MailService".into(),
            source_location: 0,
            ..Default::default()
        };
        let resolved = adapter
            .resolve_import(&import, Path::new("/repo/Program.cs"), &repo)
            .unwrap();
        assert_eq!(resolved.kind, ImportKind::Internal);
        assert_eq!(
            resolved.target_path.as_deref(),
            Some(Path::new("/repo/App/Svc/MailService.cs"))
        );
    }

    #[test]
    fn csharp_resolve_stdlib_bcl_roots() {
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = CSharpAdapter;
        for p in [
            "System",
            "System.Console",
            "System.Text.Json",
            "Microsoft.Extensions.Logging",
            "netstandard",
        ] {
            let import = ImportStatement {
                path: p.into(),
                source_location: 0,
                ..Default::default()
            };
            let resolved = adapter
                .resolve_import(&import, Path::new("/repo/Program.cs"), &repo)
                .unwrap();
            assert_eq!(resolved.kind, ImportKind::StandardLibrary, "{p}");
        }
    }

    #[test]
    fn csharp_resolve_external_third_party() {
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = CSharpAdapter;
        let import = ImportStatement {
            path: "Newtonsoft.Json".into(),
            source_location: 0,
            ..Default::default()
        };
        let resolved = adapter
            .resolve_import(&import, Path::new("/repo/Program.cs"), &repo)
            .unwrap();
        assert_eq!(resolved.kind, ImportKind::External);
    }

    #[test]
    fn csharp_resolve_namespace_backed_using_is_internal() {
        // KADE 1 pin'i: `using N.S;` (namespace-form, C#'ta baskın) — repo içinde
        // declare edilmiş namespace Internal. File-scoped VE block formları.
        let dir = std::env::temp_dir().join(format!(
            "osp-csharp-ns-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let svc = dir.join("Services");
        let models = dir.join("Models");
        std::fs::create_dir_all(&svc).unwrap();
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(
            svc.join("MailService.cs"),
            "namespace Fixture.Services;\n\npublic class MailService { }\n",
        )
        .unwrap();
        std::fs::write(
            models.join("Mail.cs"),
            "namespace Fixture.Models\n{\n    public record Mail(int Id);\n}\n",
        )
        .unwrap();

        let repo = RepoContext::new(
            dir.clone(),
            vec![svc.join("MailService.cs"), models.join("Mail.cs")],
        );
        let adapter = CSharpAdapter;

        let import = ImportStatement {
            path: "Fixture.Services".into(),
            source_location: 0,
            ..Default::default()
        };
        let resolved = adapter
            .resolve_import(&import, Path::new(&svc.join("MailService.cs")), &repo)
            .unwrap();
        assert_eq!(resolved.kind, ImportKind::Internal, "file-scoped namespace");
        assert_eq!(
            resolved.target_path.as_deref(),
            Some(svc.join("MailService.cs").as_path()),
            "temsilci dosya — namespace'i declare eden"
        );

        let import = ImportStatement {
            path: "Fixture.Models".into(),
            source_location: 0,
            ..Default::default()
        };
        let resolved = adapter
            .resolve_import(&import, Path::new(&svc.join("MailService.cs")), &repo)
            .unwrap();
        assert_eq!(resolved.kind, ImportKind::Internal, "block namespace formu");
        assert_eq!(
            resolved.target_path.as_deref(),
            Some(models.join("Mail.cs").as_path())
        );
    }

    #[test]
    fn csharp_resolve_undeclared_namespace_is_external_tier1_limit() {
        // Tier-1 sınırı pin'i: repo'da declare edilmemiş VE dosyaya oturmayan
        // namespace → External. Tier-2 SCIP (Aşama 2) kapatır — davranış bilinçli.
        let repo = RepoContext::new(PathBuf::from("/repo"), vec![]);
        let adapter = CSharpAdapter;
        let import = ImportStatement {
            path: "SomeOther.Lib".into(),
            source_location: 0,
            ..Default::default()
        };
        let resolved = adapter
            .resolve_import(&import, Path::new("/repo/Program.cs"), &repo)
            .unwrap();
        assert_eq!(
            resolved.kind,
            ImportKind::External,
            "Tier-1 belgeli sınırlama"
        );
    }

    #[test]
    fn csharp_usings_inside_namespace_declaration_found() {
        // C# idiomu: using'ler namespace bloğu İÇİNDE durur — walk tüm ağacı
        // dolaştığı için bulunmalı.
        let src = "\
namespace App.Web;

namespace App.Web.Controllers
{
    using App.Core.Domain;
    public class HomeController { }
}
";
        let adapter = CSharpAdapter;
        let imports = adapter.extract_imports(src);
        assert!(
            imports.iter().any(|i| i.path == "App.Core.Domain"),
            "{:?}",
            imports
        );
    }

    // --- #167: tip-düzeyi referans çözümlemesi (dondurulmuş kurallar 1-7) ---

    use tempfile::TempDir;

    /// (dosya, içerik) çiftlerinden C# fixture repo + RepoContext kur.
    fn make_type_ref_repo(files: &[(&str, &str)]) -> (TempDir, RepoContext) {
        let dir = TempDir::new().expect("temp dir");
        let mut paths = Vec::new();
        for (rel, content) in files {
            let p = dir.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, content).unwrap();
            paths.push(p);
        }
        paths.sort();
        let repo = RepoContext::new(dir.path().to_path_buf(), paths);
        (dir, repo)
    }

    const SVC_NS: &str = "namespace App.Svc;\n\npublic class MailService { }\n";

    #[test]
    fn type_imports_one_using_line_to_n_type_edges_not_delegate() {
        // Kural 1+2: 1 using satırı → dosyada geçen HER tip için ayrı kenar;
        // hedef tip dosyası (temsilci DEĞİL — temsilci sorted-first MailResult.cs
        // olurdu; MailService'e referans MailService.cs'e gitmeli).
        let (_dir, repo) = make_type_ref_repo(&[
            ("svc/MailResult.cs", "namespace App.Svc;\n\npublic class MailResult { }\n"),
            ("svc/MailService.cs", SVC_NS),
            ("svc/Unused.cs", "namespace App.Svc;\n\npublic enum Unused { A, B }\n"),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Svc;\n\nclass Program\n{\n    MailService s = new MailService();\n    MailResult r = new MailResult();\n}\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let imports = adapter.extract_imports(&std::fs::read_to_string(program).unwrap());
        let res = adapter
            .resolve_type_references(
                &std::fs::read_to_string(program).unwrap(),
                &imports,
                program,
                &repo,
            )
            .expect("C# destekler");

        assert_eq!(
            res.type_import_targets.len(),
            2,
            "{:?}",
            res.type_import_targets
        );
        assert!(res
            .type_import_targets
            .iter()
            .any(|p| p.file.ends_with("MailService.cs")));
        assert!(res
            .type_import_targets
            .iter()
            .any(|p| p.file.ends_with("MailResult.cs")));
        assert!(
            !res.type_import_targets
                .iter()
                .any(|p| p.file.ends_with("Unused.cs")),
            "referans verilmeyen tip kenar üretmez (kural 6): {:?}",
            res.type_import_targets
        );
        assert!(res.same_ns_targets.is_empty());
        assert!(res.ambiguous_type_names.is_empty());
    }

    #[test]
    fn type_imports_unused_using_yields_zero_type_edges() {
        // Kural 6: using var, tip referansı yok → 0 tip kenarı (using yüzeyi ≠
        // gerçek referans; ns-gren kenar resolve_import'ta aynen kalır).
        let (_dir, repo) = make_type_ref_repo(&[
            ("svc/MailService.cs", SVC_NS),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Svc;\n\nclass Program { }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "{:?}",
            res.type_import_targets
        );
    }

    #[test]
    fn type_imports_using_directive_identifiers_do_not_count() {
        // Kural 2 (dışlama): using satırındaki ns-segment identifier'ları tip
        // referansı DEĞİL — `using App.Mailer;` tek başına Mailer tipine kenar
        // üretmemeli.
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "svc/Mailer.cs",
                "namespace App.Mailer;\n\npublic class Mailer { }\n",
            ),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Mailer;\n\nclass Program { }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "using-direktifi identifier'ı kenar üretmez: {:?}",
            res.type_import_targets
        );
    }

    #[test]
    fn type_imports_partial_type_edges_to_each_declaring_file() {
        // Kural 1: partial tip → her declare eden dosyaya ayrı kenar (temsilci YOK).
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "ledger/WalletPart1.cs",
                "namespace App.Ledger;\n\npublic partial class Wallet { }\n",
            ),
            (
                "ledger/WalletPart2.cs",
                "namespace App.Ledger;\n\npublic partial class Wallet { }\n",
            ),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Ledger;\n\nclass Program { Wallet w; }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert_eq!(
            res.type_import_targets.len(),
            2,
            "{:?}",
            res.type_import_targets
        );
        assert!(res
            .type_import_targets
            .iter()
            .any(|p| p.file.ends_with("WalletPart1.cs")));
        assert!(res
            .type_import_targets
            .iter()
            .any(|p| p.file.ends_with("WalletPart2.cs")));
    }

    #[test]
    fn type_imports_own_declared_type_name_subtracted() {
        // Kural 3: dosyanın KENDİ declare ettiği ad using'd ns adayından önce
        // düşürülür (yerel bildirim kazanır).
        let (_dir, repo) = make_type_ref_repo(&[
            ("svc/MailService.cs", SVC_NS),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Svc;\n\nclass MailService { }\n\nclass Program { MailService m = new MailService(); }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "{:?}",
            res.type_import_targets
        );
    }

    #[test]
    fn type_imports_generic_type_parameter_subtracted() {
        // Kural 3: kendi generic tip-parametre adı using'd ns'teki tip adıyla
        // çakışırsa düşürülür (deterministik false-positive önleme).
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "svc/Target.cs",
                "namespace App.Svc;\n\npublic class Target { }\n",
            ),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Svc;\n\nclass Program\n{\n    void Run<Target>()\n    {\n        var t = default(Target);\n    }\n}\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "{:?}",
            res.type_import_targets
        );
    }

    #[test]
    fn type_imports_ambiguous_name_skipped_and_reported() {
        // Kural 4: ad birden çok using'd ns'te → kenar YOK + belirsizlik raporu
        // (CS0104 aynası — fail-visible).
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "a/Shared.cs",
                "namespace Ns.A;\n\npublic class Shared { }\n",
            ),
            (
                "b/Shared.cs",
                "namespace Ns.B;\n\npublic class Shared { }\n",
            ),
            (
                "app/Program.cs",
                "namespace App;\n\nusing Ns.A;\nusing Ns.B;\n\nclass Program { Shared s; }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "{:?}",
            res.type_import_targets
        );
        assert_eq!(res.ambiguous_type_names, vec!["Shared".to_string()]);
    }

    #[test]
    fn same_ns_reference_visible_without_using_and_wins_precedence() {
        // Kural 5+7 (B3): using gerektirmeyen aynı-ns çapraz-dosya referansı
        // görünür; kendi-ns bildirimi using'd adayı Ezer (TypeImports üretilmez).
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "pay/ApprovalFlow.cs",
                "namespace Nexus.Pay;\n\npublic class ApprovalFlow { }\n",
            ),
            // AYNI ad using'd ns'te de var — öncelik kendi-ns'te.
            (
                "other/ApprovalFlow.cs",
                "namespace Nexus.Other;\n\npublic class ApprovalFlow { }\n",
            ),
            (
                "pay/PaymentService.cs",
                "namespace Nexus.Pay;\n\nusing Nexus.Other;\n\npublic class PaymentService { ApprovalFlow flow; }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let svc = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("PaymentService.cs"))
            .unwrap();
        let source = std::fs::read_to_string(svc).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, svc, &repo)
            .unwrap();

        assert_eq!(res.same_ns_targets.len(), 1, "{:?}", res.same_ns_targets);
        assert!(
            res.same_ns_targets[0].file.ends_with("pay/ApprovalFlow.cs")
                || res.same_ns_targets[0]
                    .file
                    .ends_with("\\pay\\ApprovalFlow.cs")
        );
        // Öncelik: aynı ad using'd ns'te de declare edilmiş ama TypeImports ÜRETİLMEDİ
        // ve belirsizlik de raporlanmadı (kendi-ns çözdü).
        assert!(
            res.type_import_targets.is_empty(),
            "{:?}",
            res.type_import_targets
        );
        assert!(
            res.ambiguous_type_names.is_empty(),
            "{:?}",
            res.ambiguous_type_names
        );
    }

    #[test]
    fn same_ns_reference_run16_pattern_exception_visible() {
        // Run-16 altın standardı (B3 somut örneği): `ApprovalNotApprovedException`
        // T'nin KENDİ ns'inde başka dosyada — ns-gren grafta kenar YOK, #167'de
        // SameNsType olarak görünür.
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "approvals/ApprovalNotApprovedException.cs",
                "namespace Nexus.AI.Application.Approvals;\n\npublic class ApprovalNotApprovedException { }\n",
            ),
            (
                "approvals/ApprovalService.cs",
                "namespace Nexus.AI.Application.Approvals;\n\npublic class ApprovalService\n{\n    public void Review() { throw new ApprovalNotApprovedException(); }\n}\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let svc = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("ApprovalService.cs"))
            .unwrap();
        let source = std::fs::read_to_string(svc).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, svc, &repo)
            .unwrap();
        assert_eq!(res.same_ns_targets.len(), 1, "{:?}", res.same_ns_targets);
        assert!(res.type_import_targets.is_empty());
    }

    #[test]
    fn type_imports_delegate_declaration_indexed() {
        // Kural 1: delegate de tiptir — referans edilirse kenar üretir.
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "svc/SendMail.cs",
                "namespace App.Svc;\n\npublic delegate void SendMail(string to);\n",
            ),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Svc;\n\nclass Program { SendMail send; }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert_eq!(
            res.type_import_targets.len(),
            1,
            "{:?}",
            res.type_import_targets
        );
        assert!(res.type_import_targets[0].file.ends_with("SendMail.cs"));
    }

    #[test]
    fn type_imports_type_backed_using_not_a_candidate() {
        // Tasarım kararı (a): TypeImports yalnız KADE-1 (namespace-backed)
        // using'lerde üretilir — type-backed using'de çöküş yoktur (KADE-2 zaten
        // tip dosyasına çözümler; ns-gren kenar orada kalır). NOT: KADE-2'nin
        // dosya bulması dizin≈ns konvansiyonuna bağlıdır (ayrı mekanizma,
        // csharp_resolve_internal_type_backed pin'lidir) — burada yalnız
        // tip-gren aday OLMADIĞI sınanır.
        let (_dir, repo) = make_type_ref_repo(&[
            ("svc/MailService.cs", SVC_NS),
            (
                "app/Program.cs",
                "namespace App;\n\nusing static App.Svc.MailService;\n\nclass Program { MailService m = new MailService(); }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        // Tip-gren: "App.Svc.MailService" declare edilmiş bir ns DEĞİL → aday
        // değil → TypeImports üretmez (referans olsa bile).
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "{:?}",
            res.type_import_targets
        );
    }

    #[test]
    fn type_imports_two_types_in_one_file_are_two_dependencies() {
        // Review P0-1 pin'i (reviewer'ın örneği): Contracts.cs hem Request hem
        // Response declare eder; Service ikisini de referans verir → 2 AYRI
        // bağımlılık (2 hedef, aynı dosya). Dosya-çökmeli model 1 üretirdi;
        // x_type 2/3 olmalıydı, 1/2 DEĞİL.
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "contracts/Contracts.cs",
                "namespace App.Contracts;

public class Request { }
public class Response { }
",
            ),
            (
                "svc/Service.cs",
                "namespace App;

using App.Contracts;

class Service
{
    Request req;
    Response res;
}
",
            ),
        ]);
        let adapter = CSharpAdapter;
        let svc = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Service.cs"))
            .unwrap();
        let source = std::fs::read_to_string(svc).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, svc, &repo)
            .unwrap();
        assert_eq!(
            res.type_import_targets.len(),
            2,
            "{:?}",
            res.type_import_targets
        );
        let names: Vec<&str> = res
            .type_import_targets
            .iter()
            .map(|t| t.type_name.as_str())
            .collect();
        assert!(
            names.contains(&"Request") && names.contains(&"Response"),
            "{names:?}"
        );
        assert!(res
            .type_import_targets
            .iter()
            .all(|t| t.file.ends_with("Contracts.cs")));
    }

    #[test]
    fn type_imports_partial_type_one_symbol_two_file_targets() {
        // P0-1: partial Wallet 2 dosyada → 2 hedef (dosya-başına) AMA tek sembol
        // (x_type 1 bağımlılık sayar — TypeGranularCouplingAxis testi de pin'ler).
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "ledger/WalletPart1.cs",
                "namespace App.Ledger;

public partial class Wallet { }
",
            ),
            (
                "ledger/WalletPart2.cs",
                "namespace App.Ledger;

public partial class Wallet { }
",
            ),
            (
                "app/Program.cs",
                "namespace App;

using App.Ledger;

class Program { Wallet w; }
",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert_eq!(
            res.type_import_targets.len(),
            2,
            "{:?}",
            res.type_import_targets
        );
        assert!(res
            .type_import_targets
            .iter()
            .all(|t| t.type_name == "Wallet"));
        assert!(res
            .type_import_targets
            .iter()
            .all(|t| t.namespace == "App.Ledger"));
    }

    #[test]
    fn type_imports_type_backed_using_directive_is_a_reference() {
        // Review P1-1: KADE-2 type-backed using'ler directive-referansıdır.
        // (a) `using static N.T;` → T sembolüyle TypeImports;
        // (b) alias `using M = N.T;` → kullanım `M` adıyla olduğundan occurrence
        //     match imkânsız — directive'in kendisi T referansıdır.
        //     Dizin≈ns konvansiyonu: App/Svc/MailService.cs → "App.Svc.MailService".
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "App/Svc/MailService.cs",
                "namespace App.Svc;

public class MailService { }
",
            ),
            (
                "App/Program.cs",
                "namespace App;

using static App.Svc.MailService;
using M = App.Svc.MailService;

class Program
{
    MailService a = new MailService();
    M b;
}
",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        // İki directive de aynı sembolü adlandırır → dedup sonrası 1 hedef;
        // sembol ns'i indeks bildiriminden (App.Svc), türetmeden değil.
        assert_eq!(
            res.type_import_targets.len(),
            1,
            "{:?}",
            res.type_import_targets
        );
        let t = &res.type_import_targets[0];
        assert_eq!(t.type_name, "MailService");
        assert_eq!(t.namespace, "App.Svc");
        assert!(t.file.ends_with("MailService.cs"));
    }

    #[test]
    fn same_ns_global_namespace_references_visible() {
        // Review P1-2: namespace bildirmeyen iki dosya GLOBAL ns ("") üyesidir —
        // B3 çapraz-dosya referansı görünür olmalı.
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "a/A.cs",
                "class A
{
    B b;
}
",
            ),
            (
                "b/B.cs",
                "class B { }
",
            ),
        ]);
        let adapter = CSharpAdapter;
        let a = repo.all_files.iter().find(|f| f.ends_with("A.cs")).unwrap();
        let source = std::fs::read_to_string(a).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, a, &repo)
            .unwrap();
        assert_eq!(res.same_ns_targets.len(), 1, "{:?}", res.same_ns_targets);
        assert_eq!(res.same_ns_targets[0].type_name, "B");
        assert_eq!(res.same_ns_targets[0].namespace, "");
        assert!(res.same_ns_targets[0].file.ends_with("B.cs"));
    }

    #[test]
    fn type_imports_variance_type_parameter_subtracted_by_name_field() {
        // Review P1-3: `interface IFoo<out TTarget>` — tam metin "out TTarget"
        // yakalardı; name FIELD'ı "TTarget" verir → using'd ns'teki TTarget
        // tipine false kenar ÜRETİLMEZ.
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "svc/Target.cs",
                "namespace App.Svc;

public class TTarget { }
",
            ),
            (
                "app/Program.cs",
                "namespace App;

using App.Svc;

interface IFoo<out TTarget>
{
    TTarget Get();
}
",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        assert!(
            res.type_import_targets.is_empty(),
            "varyans tip-parametresi düşürülmeli: {:?}",
            res.type_import_targets
        );
    }

    #[test]
    fn type_imports_nested_type_indexed_under_declaring_ns() {
        // İç içe tip bildirimi de indekse girer (aynı ns, düz ad — Tier-1 düz
        // ad eşleşmesi).
        let (_dir, repo) = make_type_ref_repo(&[
            (
                "svc/Outer.cs",
                "namespace App.Svc;\n\npublic class Outer\n{\n    public class Inner { }\n}\n",
            ),
            (
                "app/Program.cs",
                "namespace App;\n\nusing App.Svc;\n\nclass Program { Outer.Inner pair; }\n",
            ),
        ]);
        let adapter = CSharpAdapter;
        let program = repo
            .all_files
            .iter()
            .find(|f| f.ends_with("Program.cs"))
            .unwrap();
        let source = std::fs::read_to_string(program).unwrap();
        let imports = adapter.extract_imports(&source);
        let res = adapter
            .resolve_type_references(&source, &imports, program, &repo)
            .unwrap();
        // P0-1 (sembol kimliği): `Outer.Inner pair` nitelikli referansı HER İKİ
        // tipi de adlandırır → 2 hedef (Outer + Inner), ikisi de Outer.cs'te.
        // Dosya-çökmeli eski model 1 beklerdi — sembol kimliği ayrımını görür.
        assert_eq!(
            res.type_import_targets.len(),
            2,
            "{:?}",
            res.type_import_targets
        );
        let names: Vec<&str> = res
            .type_import_targets
            .iter()
            .map(|t| t.type_name.as_str())
            .collect();
        assert!(
            names.contains(&"Outer") && names.contains(&"Inner"),
            "{names:?}"
        );
        assert!(res
            .type_import_targets
            .iter()
            .all(|t| t.file.ends_with("Outer.cs")));
    }
}
