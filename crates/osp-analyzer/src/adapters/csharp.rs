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
use crate::contract::{ClassDef, ImportKind, ImportStatement, ResolvedImport};
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
}
