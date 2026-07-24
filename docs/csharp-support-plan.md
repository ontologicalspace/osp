# OSP C# Dil Desteği — PR Serisi Planı

**Tarih:** 2026-07-24
**Repo:** `ontologicalspace/osp`
**Hedef:** `osp-analyzer`'a altıncı dil (C#) desteği eklemek — mevcut beş dilin ölçüm
semantiğini bozmadan, ve C# hakkında ölçülmemiş hiçbir metrik iddiası yayınlamadan.
**Plan durumu:** Reviewer + implementer üç turda mutabık. Implementation-ready (PR A).

---

## 0. Yönlendirici ilke

Bu iş bir tree-sitter adapter'ı eklemek değil. `shared.rs` içinde birikmiş **örtük dil
semantiğini** adapter sınırlarına geri taşımak, ve C#'ın namespace/manifest modelinin OSP'nin
import-tabanlı coupling ontolojisine meydan okumasını dürüstçe temsil etmektir.

Korunan ayrımlar (her biri bir PR sınırına karşılık gelir):

```
refactor              ≠ bug fix
syntax evidence       ≠ dependency
manifest declaration  ≠ source coupling
known language        ≠ compiled adapter
unavailable           ≠ low metric
```

---

## 1. Kanıtlanmış olgular (bu plan bunların üzerine kuruludur)

Aşağıdakiler repo/grammar üzerinde doğrulandı — çıkarım değil, ölçüm:

1. **Tek kayıt noktası.** `AdapterRegistry::default_all()` (`adapters/mod.rs:19`) adapter kaydının
   tek yeri. Dosya keşfi (`pipeline.rs:526`) registry-driven — `.cs` kaydedildiğinde keşif
   otomatik gelir. `default_all()` dışında hiçbir yerde adapter **sayısı** assert edilmiyor.

2. **`name` field riski Go'da gerçek.** 12 declaration kind'ı beş grammar'a karşı ölçüldü.
   Go `type_declaration` **field taşımıyor**; isim `type_spec`/`type_alias` alt-node'unun
   kendi `name` field'ında. Global `child_by_field_name("name")` Go declaration'larını sıfırlar.
   C# `type_declaration` bir **supertype** (subtypes: class/delegate/enum/interface/record/struct)
   — ağaçta concrete node olarak görünmez, dolayısıyla Go ile çift sayım riski yok.

3. **Coupling/instability yalnızca `Imports` sayıyor.** `axes.rs:64, 258-259`
   `out_degree_value(node.id, EdgeKind::Imports)`. Yeni bir `EdgeKind` variant'ı coupling
   formülüne kendiliğinden **sızmaz**.

4. **`EdgeKind`/`NodeKind` yeni variant maliyeti yüksek.** Core'da 34 dosya, 263 kullanım.
   Her ikisi de `CanonicalEdgeKind`/`CanonicalNodeKind` ile **stable numeric wire representation**
   taşıyor (`canonical_tags.rs`), bincode ile persist ediliyor (`persistence.rs`, versiyonlu),
   ve `EdgeKind` **frozen evidence identity'nin parçası** (`authorization.rs:56` —
   `removed_edges = CanonicalEdgeIdentity(from,to,kind)`). Dört yerde "8 varyant"/"9 varyant"
   sayısı sabitlenmiş (`types.rs:1621` concept-anchoring dahil). → C# uğruna enum genişletmek
   ayrı ontolojik karar (PR C2), Tier 1 kararı değil.

5. **`AnalysisResult` genişletilebilir.** Zaten `diagnostics: Vec<AnalysisDiagnostic>` ve
   `DiagnosticCode` enum'ı taşıyor (`contract.rs:73, 135`). Yeni alan/variant eklemek wrapper
   tip gerektirmez. Struct-literal construction 6 yerde (1 production `pipeline.rs:349`,
   5 test fixture).

6. **`RepoMetrics` A/D zorunlu `MetricValue`, `MetricValue.value` zorunlu `f64`** (NaN/Inf
   yasak — `contract.rs:105-109`, `coords.rs:68-77`). C# analiz edilince bu struct inşa
   edilirse bir `f64` yazmak **zorunlu** → placeholder baskısı. Çözüm: C# için `RepoMetrics`'i
   hiç inşa etme (PR C).

7. **`MetricSource` variant'ları mevcut** (`coords.rs:86`): TreeSitter, Scip, Placeholder,
   Heuristic, Mixed. Provenance altyapısı var; bu bir yetenek boşluğu değil, politika kararı.

8. **`walk_class_defs` `force_abstract` taşıyor** (`shared.rs:273`): `interface_declaration`
   ve `type_alias_declaration` kind-seviyesinde abstract işaretleniyor; diğerleri substring
   pattern'e bakıyor. Refactor bu ikili mantığı bit-identical korumalı.

9. **`tree-sitter-c-sharp` uyumlu.** 0.23.0/0.23.1/0.23.5 hepsi `tree-sitter-language ^0.1`
   (normal dep); `tree-sitter ^0.24/^0.25` yalnızca **dev-dependency**, downstream'i
   kısıtlamıyor. API: `tree_sitter_c_sharp::LANGUAGE` (LanguageFn) → `shared::parse_root(src,
   LANGUAGE.into())`. Node-kind'lar **0.23.0** node-types.json üzerinden doğrulandı → `=0.23.0`
   pinlenir; upgrade ayrı PR. `parser.c` = 34 MB (Go 1.4 / Python 3.3 / Rust 5.8 / TS 16.9 ile
   kıyasla en büyük) → feature-gate gerekçesi.

---

## 2. PR serisi

| PR | Kapsam | Core'a dokunur | Davranış değişir | Kabul kriteri |
|---|---|---|---|---|
| **A** | Shared declaration policy extraction | Hayır | Hayır (invariant) | 23-repo normalize→SHA256 identical |
| **B** | Language catalog + feature gates + completeness | Hayır | Evet (kapalı-dil semantiği) | feature-off `.cs`→structured diagnostic; mevcut diller identical |
| **C** | C# Tier 1 syntax + lightweight `.csproj` evidence | Hayır | Evet (yeni dil, unavailable) | declarations ✓; A/I/D/coupling C# repo-metrics'e girmez; `using`→0 edge |
| **C2** *(gerekirse)* | Project-topology projection | **Evet** | Evet | SCIP dosya-seviyesi yeterliyse hiç açılmaz |
| **D** | C# Tier 2 (scip-dotnet) + empirical closure | Evet (`MetricValue` sarma) | Evet (ölçüm) | scip-dotnet index→gerçek edges; partial tek symbol; corpus A/D measured |
| **E** | Doküman/evidence propagation | Hayır | Hayır | Dört yüzey senkron |

**Sıra:** A → B → C → (C2?) → D → E.
**Gerekçe:** A davranış-koruyan zemin; B fail-closed altyapı (C'nin diagnostic'i buna dayanır);
C Tier 1 (empirik iddia yok); C2 ontolojik genişleme (yalnız gerekiyorsa); D empirik kapanış;
E propagasyon. Her PR bağımsız merge-edilebilir ve kendi kabul kriteriyle donar.

---

## 3. PR A — Declaration Policy Extraction (saf refactor)

### Amaç
`shared.rs`'teki sabit birleşik kind listesi + substring abstractness + naive isim aramasını
adapter-sahipli **policy**'ye taşımak. Sıfır davranış değişikliği.

### Yeni tipler (`shared.rs` veya `language.rs`)

```rust
pub struct DeclarationKindSpec {
    pub kind: &'static str,
    pub abstractness: AbstractnessRule,
    pub name_strategy: NameStrategy,
}

pub enum AbstractnessRule {
    Always,                              // kind-seviyesi abstract (force_abstract yerine)
    Never,
    DirectModifier(&'static str),        // C# için yeni-doğru yol (PR C'de kullanılır)
    // Davranış-koruma amaçlı; önerilen yeni tasarım DEĞİL. Bug-fix PR'larında
    // adapter'lar teker teker Always/Never/DirectModifier'a göçürülecek.
    LegacyTextContains(&'static [&'static str]),
}

pub enum NameStrategy {
    DirectField(&'static str),                                   // child_by_field_name(f)
    DescendantField { container_kind: &'static str, field: &'static str },  // Go
    FirstIdentifierFallback,                                     // mevcut davranış
}
```

### `walk_class_defs` yeni imzası

```rust
pub fn walk_class_defs(root: Node, source: &str, specs: &[DeclarationKindSpec]) -> Vec<ClassDef>
```

Eski `_class_node_kind: &str` (ölü parametre) ve `abstract_patterns: &[&str]` kaldırılır.

**`is_abstract` OR-semantiği (kesin — envanterle doğrulandı):** mevcut kod
`is_abstract = force_abstract(kind) OR substring_match(patterns)`. `force_abstract` yalnızca
`interface_declaration` + `type_alias_declaration` için true (`shared.rs:273`). Refactor'de
`AbstractnessRule::Always` bu iki kind için, `LegacyTextContains` diğerleri için kullanılır ve
**`Always` substring sonucundan bağımsız true yazar** (OR yapısını korur).

### Adapter policy'leri (bit-identical eşleme — grammar envanteriyle doğrulandı 2026-07-24)

Envanter sonucu: `is_class_def` listesindeki **9 kind'ın her biri tam bir grammar'a ait**
(hiçbiri paylaşılmıyor). Her adapter yalnızca kendi grammar'ında **fiilen listeye giren**
kind'lar için policy verir.

| Adapter | kind | AbstractnessRule | NameStrategy | Not |
|---|---|---|---|---|
| python | `class_definition` | `LegacyTextContains(["ABC","Protocol","ABCMeta"])` | `FirstIdentifierFallback` | KNOWN-DIVERGENCE(PY-ABST-004) |
| typescript | `class_declaration` | `LegacyTextContains(["abstract class","interface "])` | `FirstIdentifierFallback` | |
| typescript | `abstract_class_declaration` | `Always` | `FirstIdentifierFallback` | |
| typescript | `interface_declaration` | `Always` | `FirstIdentifierFallback` | eski force_abstract |
| typescript | `type_alias_declaration` | `Always` | `FirstIdentifierFallback` | eski force_abstract |
| javascript | `class_declaration` | `Never` | `FirstIdentifierFallback` | eski `__NEVER_MATCH__` |
| rust | `struct_item` | `Never` | `FirstIdentifierFallback` | |
| rust | `trait_item` | `LegacyTextContains(["trait "])` | `FirstIdentifierFallback` | KNOWN-DIVERGENCE(RUST-ABST-003) |
| rust | `enum_item` | `Never` | `FirstIdentifierFallback` | |
| go | `type_declaration` | `LegacyTextContains(["interface"])` | `FirstIdentifierFallback` | KNOWN-DIVERGENCE(GO-TYPE-001, GO-ABST-002) |

> **KORUNAN DIŞLAMALAR (bit-identical için sayılmayan, dokunulmayacak):**
> - **TS `enum_declaration`** grammar'da VAR ama `is_class_def` listesinde YOK → sayılmıyor.
>   Policy'ye **eklenmez** (eklemek TS enum'larını Nc'ye sokar, bit-identical'ı bozar).
> - **JS `interface_declaration`** JS grammar'ında YOK (yalnız TS'te) → JS'te hiç tetiklenmiyor.
>   JS policy'sine **konmaz** (ölü satır olurdu).
> - **`class` node kind'ı** (JS/TS anonymous/expression class) listede YOK → sayılmıyor. Korunur.
>
> Bu üç dışlama envanterle sabitlendi; PR A ilk adımı bunları characterization-test ile dondurur.

### KNOWN-DIVERGENCE (dondurulacak, düzeltilmeyecek)

```rust
// KNOWN-DIVERGENCE(GO-TYPE-001):
// `type_declaration` wrapper; grouped `type ( A struct{}; B struct{} )` tek node →
// tek ClassDef sayılıyor (under-count). type_spec is_class_def listesinde değil.
// Refactor PR'ında bilinçli korunmuştur. Düzeltme: issue #...

// KNOWN-DIVERGENCE(GO-ABST-002):
// abstract pattern ["interface"] node metninin tamamında aranıyor →
// `Handler interface{}` alanı içeren struct abstract sayılıyor (false-positive).
// Düzeltme: issue #...

// KNOWN-DIVERGENCE(RUST-ABST-003):
// ["trait "] substring; doc comment veya string literal'de "trait " geçmesi
// struct/enum'ı abstract işaretleyebilir. Düzeltme: issue #...

// KNOWN-DIVERGENCE(PY-ABST-004):
// ["ABC","Protocol","ABCMeta"] substring; sınıf gövdesinde bu isimler geçen
// (ör. yorumda) concrete sınıf abstract sayılabilir. Düzeltme: issue #...
```

### Uygulama sırası (kesin)

1. **Baseline snapshot altyapısı — refactor'den ÖNCE.** Kanonik normalizer + 23-repo golden
   fixture. "Değişmedi" kanıtı ancak baseline varsa mümkün; refactor'ü baseline'sız yapmak
   kabul kriterini test-edilemez bırakır.
2. **`is_class_def` envanteri** characterization-test'lerle dondurulur (üç korunan dışlama dahil).
3. `DeclarationKindSpec`/`AbstractnessRule`/`NameStrategy` tipleri + `walk_class_defs` yeni imza.
4. Beş adapter policy'ye göçürülür (yukarıdaki tablo).
5. Baseline snapshot yeniden çalıştırılır → SHA256 identical.

### Test katmanları

- **unit:** her adapter'ın `extract_class_defs` çıktısı refactor öncesi/sonrası aynı (mevcut
  adapter testleri değişmeden geçmeli).
- **characterization:** `characterization_go_grouped_type_currently_counts_wrapper`,
  `characterization_go_interface_field_currently_abstract`, `characterization_ts_enum_not_counted`,
  `characterization_anonymous_class_not_counted` — mevcut davranışı sabitler; test adı "doğru"
  demez, "refactor sırasında değişmemeli" der.
- **golden/snapshot:** 23-repo kanonik-normalize→SHA256.

### Kabul kriteri (kesin — normalizer gereği doğrulandı 2026-07-24)

Normalize edilmiş analiz çıktısı 23 repo için **semantik olarak birebir**. Karşılaştırılan alanlar:
`source file count, node count, internal edge count, type-only edge count, Nc, Na, A, I, D,
per-node coupling, per-node instability, diagnostic code counts`.

**Kanonik normalizer ZORUNLU (ham serialize SHA256'yı yanlış-negatif kırar):**
- `pipeline.rs` çıktısında `node_map`/`node_paths`/`node_semantics`/`node_witnesses` **HashMap** →
  key'e (NodeId veya rel-path) göre **sıralı** serialize edilmeli.
- `diagnostics` **Vec**, faz-sırasına göre push ediliyor → snapshot öncesi `sort()`.
- `f64` alanlar (coupling, instability, A, I, D) → **sabit ondalık hassasiyet** (ör. 6 hane) ile
  yazılmalı, ham bit değil (platform floating-point farkı SHA256'yı kırar).
- **Teyit edilmiş iyi durum:** `node_paths` zaten repo-relative (`strip_prefix(&repo)`) ve
  ayırıcı-normalize (`\`→`/`); `files.sort()` (`pipeline.rs:490`) NodeId atamasını deterministik
  yapıyor. Kalan iş yalnızca yukarıdaki map/vec/f64 kanonikleştirmesi.

---

## 4. PR B — Language Catalog + Feature Gates + Completeness

### Yeni tipler (`osp-analyzer`, `language.rs` veya yeni `languages.rs`)

```rust
pub enum LanguageId { Python, TypeScript, JavaScript, Rust, Go, CSharp }

pub struct KnownLanguage {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub feature_name: &'static str,
}

// Parser feature'larından BAĞIMSIZ derlenir — csharp feature kapalıyken bile
// sistem `.cs`'nin ne olduğunu bilir.
pub const KNOWN_LANGUAGES: &[KnownLanguage] = &[ /* 6 dil */ ];
```

**Karar:** `LanguageId` `osp-analyzer`'da yaşar (core dil-bağımsız kalır). İleride analyzer/MCP/
desktop paylaşımı zorunlu olursa `osp-analysis-contract` crate'i çıkarılır — bugün erken soyutlama.

### Registry ayrımı + sert kesme

```rust
// default_all() KALDIRILIR (sert kesme, pre-1.0). Yeni:
impl AdapterRegistry {
    pub fn default_enabled() -> Self { /* compile edilmiş + enabled adapter'lar */ }
}
```

13 çağrı yeri güncellenir: `adapters/mod.rs`, `osp-cli/commands/mod.rs` (×3), `commands/graph.rs`,
`pipeline.rs` (×3), `osp-analyze.rs`, `osp-mcp/workspace.rs`, `osp-desktop/lib.rs` (×3),
examples (`g2c_corpus_matrix`, `token_benchmark`, `multi_repo_bench`).

Migration notu (PR açıklamasında):
```
BREAKING (pre-1.0):
AdapterRegistry::default_all() renamed to default_enabled()
because feature-gated builds do not contain all known languages.
```

### Feature symmetry (`Cargo.toml`)

```toml
[features]
default = ["all-languages"]
all-languages = ["python","typescript","javascript","rust","go","csharp"]
python     = ["dep:tree-sitter-python"]
typescript = ["dep:tree-sitter-typescript"]
javascript = ["dep:tree-sitter-javascript"]
rust       = ["dep:tree-sitter-rust"]
go         = ["dep:tree-sitter-go"]
csharp     = ["dep:tree-sitter-c-sharp"]  # =0.23.0
```
`osp-analyze` CLI: C# default-on. Library consumer: `--no-default-features --features rust,go`.

### Completeness modeli (`AnalysisResult`'a in-place alan)

```rust
pub struct AnalysisCompleteness {
    pub discovered_files: usize,
    pub analyzed_files: usize,
    pub exclusions: Vec<AnalysisExclusion>,
}
pub struct AnalysisExclusion {
    pub language: Option<LanguageId>,
    pub reason: ExclusionReason,
    pub file_count: usize,
}
// PR B'de YALNIZCA bu iki variant — her variant doğduğu PR'da test edilebilir olmalı.
// Generated/Vendor/TestPolicy/ParseFailure, kendilerini üreten source-policy PR'ında eklenir.
pub enum ExclusionReason { FeatureDisabled, UnsupportedLanguage }

impl AnalysisCompleteness {
    pub fn is_complete(&self) -> bool { self.exclusions.is_empty() }
}
```
`AnalysisResult`'a `pub completeness: AnalysisCompleteness` eklenir (6 construction-site;
`Default` türetilebilirse fixture'lar `..Default::default()` ile dokunulmadan geçer).

`DiagnosticCode::LanguageSupportDisabled` eklenir.

### Kapalı-dil davranışı (fail-closed)

`walk_dir` (`pipeline.rs:498`) bilinen ama feature-disabled bir uzantı görürse: **sessiz atlama
YOK**. `exclusions`'a `AnalysisExclusion { CSharp, FeatureDisabled, N }` + structured diagnostic.
Repo-level coupling comparison bir bilinen-dil dışlandığında `partial`/`unavailable` olarak
işaretlenir — asla "sağlıklı düşük coupling" gibi gösterilmez.

### Test katmanları
- unit: `LanguageCatalog::known_all()` 6 dil; `default_enabled()` feature-koşullu adapter sayısı.
- integration: feature-off `.cs` fixture → `Partial` + diagnostic; mevcut diller identical.
- derleme: 6 construction-site + `--no-default-features --features rust,go` derlenmeli.

---

## 5. PR C — C# Tier 1 + Lightweight Manifest Evidence

### `CSharpAdapter` (`adapters/csharp.rs`)

```rust
impl LanguageAdapter for CSharpAdapter {
    fn name(&self) -> &str { "csharp" }
    fn extensions(&self) -> &[&str] { &[".cs"] }
    fn extract_imports(...) -> Vec<ImportStatement> { /* using_directive */ }
    fn resolve_import(...) -> Option<ResolvedImport> { /* HER ZAMAN edge YOK */ }
    fn extract_class_defs(...) -> Vec<ClassDef> { /* declaration specs */ }
}
```

### Declaration specs (PR A altyapısını kullanır)

| kind | AbstractnessRule | NameStrategy |
|---|---|---|
| `class_declaration` | `DirectModifier("abstract")` | `DirectField("name")` |
| `record_declaration` | `DirectModifier("abstract")` | `DirectField("name")` |
| `interface_declaration` | `Always` | `DirectField("name")` |
| `struct_declaration` | `Never` | `DirectField("name")` |
| `enum_declaration` | `Never` | `DirectField("name")` |

`DirectModifier` implementasyonu: `modifier` child node'larını okur (substring değil) →
`public abstract partial class` ve nested tip bulaşması sorunlarından bağışık.
`DirectField("name")`: C# `[Serializable] class Foo` → `Serializable` yerine `Foo` (grammar'da
`name` field required).

### `using` — edge üretmez, metadata taşır

`using` name-resolution context'idir, dependency değil (kullanılmayabilir; hangi tipe
erişildiğini söylemez; tam-nitelikli referans `using`'siz olabilir; `global`/`static`/alias
formları var). `resolve_import` C# için **her zaman edge yok** döner.

Form/scope metadata (`ImportStatement`'a değil, C#-özel yapıda tutulur — genel tipi şişirme):
```rust
enum ImportScope { File, Project }   // global using → Project
enum ImportForm  { Namespace, StaticType, Alias }
```
Grammar doğrulaması: alias branch'inde `name` field = **alias adı** (hedef değil; hedef `=`
sonrası `type`); plain/static branch'te hedef `_name`. `global using` project-wide etkir.

### Lightweight `.csproj` manifest (Seviye 1 — XML gözlem, MSBuild evaluation YOK)

```rust
pub struct CSharpProjectManifest {
    pub project_path: PathBuf,
    pub explicit_root_namespace: Option<String>,     // yalnız açıkça yazılmışsa observed
    pub implicit_usings: ExplicitSetting<bool>,       // Enabled raporlanır, using üretilmez
    pub project_references: Vec<ProjectReferenceEvidence>,
    pub target_frameworks: Vec<String>,
}
pub struct ProjectReferenceEvidence {
    pub from_project: PathBuf,
    pub declared_include: String,
    pub resolved_path: Option<PathBuf>,
    pub condition: Option<String>,          // $(...) / Condition → unresolved diagnostic
    pub resolution: ProjectReferenceResolution,
}
```
`AnalysisResult`'a **side-metadata** olarak: `pub csharp_projects: Vec<CSharpProjectManifest>`.
Graph'a (Node/Edge) **yazılmaz** — project-topology projection PR C2'ye ait.

Sınırlar: `$(RootNamespace)` property-expanded → unresolved. `Directory.Build.props/targets`,
SDK defaults, conditional item groups, globbing, multi-targeting → Seviye 2 (PR D). PR C
".csproj okur ama MSBuild değerlendirdiğini iddia etmez."

### C# metrikleri — `RepoMetrics` HİÇ inşa edilmez

C# analiz edilince `RepoMetrics` inşa etmek zorunlu `f64` yazmayı gerektirir (olgu #6) →
placeholder baskısı. Çözüm: C# repo'su için `RepoMetrics` **hiç üretilmez**; bunun yerine
`AnalysisCompleteness.exclusions`'a `ExclusionReason::SemanticIndexRequired` (PR C'de bu variant
eklenir). "Unavailable" bilgisi sentinel f64'te değil, completeness modelinde yaşar.

> `MetricObservation`/`MetricAvailability<T>` sarma tipi **PR C'de eklenmez** — ihtiyaç PR D'de
> (Tier 2 gelince "measured vs still-unavailable" gerçek ayrımı) doğar. Tip soyutlaması da
> ölçümle kazanılır, önceden decree edilmez.

### `obj/` — evet, `bin/` — hayır

`obj/` C# generated (`*.g.cs`, `GlobalUsings.g.cs`, `AssemblyInfo.cs`). `bin/` **global değil** —
Python/JS/CLI repolarında gerçek kaynak (`bin/cli.js`). `walk_dir` global skip listesine `bin`
**eklenmez**. `obj/` yalnız C# generated-policy aktifken hariç tutulur; ideal olarak hard-coded
liste yerine `AnalysisConfig.exclude_generated` bağlanmalı (o bağlama ayrı source-policy PR'ı).

### Test katmanları
- unit: interface→abstract, `abstract class`→abstract, `class`→concrete, `struct`→concrete,
  `record`→modifier'a göre; `[Attr] class Foo`→isim `Foo`; `public abstract partial class`→abstract.
- unit: `using`→0 edge (her form).
- integration: ProjectReference → `csharp_projects` side-metadata; graph'a edge girmedi;
  C# repo'da `RepoMetrics` yok, completeness `SemanticIndexRequired`.

---

## 6. PR C2 — Project-Topology Projection *(yalnızca gerekiyorsa)*

`NodeKind`/`EdgeKind::ReferencesProject` + canonical tag (tag=8/9) + manifest-evidence →
graph projection. **Frozen evidence identity uzayını genişletir** (olgu #4) → INV-EI ailesi
etkisi değerlendirilir; dört doküman/test sayı güncellemesi; persistence version bump ihtimali.

**Tetikleyici:** PR D'nin ürettiği SCIP semantic type-reference graph'ı **dosya-seviyesinde
yetersizse**. Eğer scip-dotnet `A.cs → B.cs` referanslarını dosya granülaritesinde veriyorsa,
ProjectReference'ların ayrı manifest-evidence olarak kalması daha doğru → **C2 hiç açılmaz**.
Karar PR D planlanırken gerçek scip-dotnet çıktısına karşı verilir (şu an elde gerçek C# SCIP
index yok).

Edge provenance (eklenirse `MetricSource`'a DEĞİL):
```rust
pub enum DependencyEvidenceSource { SyntaxImport, ManifestProjectReference, SemanticTypeReference }
```
Core `Edge`'e doğrudan eklemek yerine analyzer side-table (düşük risk):
`HashMap<EdgeId, DependencyEvidence>`. Genel `EdgeProvenance` enum'ı doğru ama büyük model —
ihtiyaç kanıtlanınca.

---

## 7. PR D — C# Tier 2 (scip-dotnet) + Empirical Closure

- **`scip-dotnet`** (Roslyn tabanlı, Docker imajı; `.NET restore`/derleme gerektirir → corpus
  repoları "restore edilebilir" kısıtı). Mevcut Docker desenine oturur (scip-python/rust/go gibi).
- Semantic type-reference edges → authoritative coupling/instability.
- **Partial-merge fixture** (varsayımı test etmeden kabul etme):
  ```
  Customer.Core.cs:       partial class Customer { int id; }
  Customer.Operations.cs: partial class Customer { void Save(){ ...id... } }
  Kabul: logical type count=1, method ownership=Customer, field ownership=Customer,
         LCOM4 input = merged symbol.
  ```
- **Nested-type isolation:** `symbol_belongs_to_class` (`scip/loader.rs:373`) desen-1 prefix
  match; `Outer#Inner#Method().` → Inner üyeleri Outer'a yazılmamalı (false-positive testi).
- Corpus seçimi: Paper 1 §Future Work #8 **4-kategori/dil** şeması (stable-heavy, stable-modern,
  AI-era-volatile A/B).
- C# A/D/coupling/cohesion **ölçülür**; `MetricObservation`/`MetricAvailability<T>` *burada*
  doğar (measured vs unavailable gerçek ayrım):
  ```rust
  pub enum MetricObservation { Measured(MetricValue), Unavailable { reason: MetricUnavailableReason } }
  pub enum MetricUnavailableReason {
      SemanticIndexRequired, PartialTypeIdentityUnresolved,
      DependencyGraphUnavailable, LanguageFeatureDisabled,
  }
  ```

---

## 8. PR E — Documentation / Evidence Propagation

"5 dil" iddiası feature-koşullu hale gelir — **sabit sayı yayınlanmaz**. Güncellenecek yüzeyler:
- `README.md:12, 136`
- `docs/STATUS.md:24`
- `crates/osp-mcp/src/workspace.rs:63` (doc comment — "5 dil" sayısı kaldırılır)
- `examples/token_benchmark.rs:47`, `osp-llm-runtime/examples/multi_repo_bench.rs:289`
  (extension listeleri)
- `language.rs` doc comment'leri
- Corpus tabloları + Paper 1 (yalnızca C# corpus run tamamlanınca; değer değişmeyen mevcut
  diller için Paper revize edilmez).

Yayınlanmış Zenodo deposit'leri (5 dil / 23 repo) geriye dönük değişmez — adapter eklemek onları
etkilemez. C# hakkında hiçbir cohesion/abstractness iddiası corpus run **olmadan** yazılmaz.

---

## 9. Açık uç

**C2 gerçekten opsiyonel mi?** PR D'nin scip-dotnet çıktısının dosya-granülaritesine bağlı.
Gerçek bir C# SCIP index elde edilmeden karar verilemez → PR D planlanırken çözülür.

---

## 10. İlkelerle bağ

- **Evidence-first, writing-last:** C# metrik iddiası (A/D/cohesion) yalnız PR D corpus run'ından
  sonra (PR E). PR C empirik iddia içermez.
- **Fail-closed over silent fallback:** kapalı-dil (PR B) ve C# metrik (PR C) sessiz düşük-değer
  değil, açık exclusion/diagnostic.
- **Measurement/judgment separation:** `LegacyTextContains` ismi refactor'ü (koru) bug-fix'ten
  (düzelt) ayırır; characterization testleri "doğru" demez, "değişmedi" der.
- **Count propagation:** PR E dört+ yüzeyi senkronlar; sayı feature-koşullu.
- **Options never collapse to one:** coupling üç kaynağa ayrık (SyntaxImport / ManifestProjectRef /
  SemanticTypeReference) — tek "dependency" kavramına indirgenmez.
- **Coordinates earned through measurement:** C# koordinatları (A/I/D) ölçülene (PR D) kadar
  unavailable; tip soyutlaması (`MetricObservation`) da ihtiyaç kazanılınca doğar.
