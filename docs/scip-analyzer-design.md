# OSP SCIP Analyzer — Tasarım Dokümanı (Faz 3)

> Bu doküman Faz 3'ün çekirdek görevini tanımlar: **gerçek semantik analiz**
> (LCOM4 cohesion + abstractness) ile Faz 1-2'nin placeholder değerlerini doldurmak
> → `y` (cohesion) ve `A` (abstractness) gerçek değerlerle gelir → `D` anlamlı olur.
>
> **Öncelik:** bu doküman > `OSP-formalism.md §2` (eksenler) > yorumsal kararlar.
> **Girdiler:** `osp-core-design.md`, `calibration-results.md`, `spike-results-v2.md`.
>
> **Sürüm:** 1.0-draft

---

## 1. Amaç & Sınırlar

| Faz 3 şunları kilitler | Şunları değil |
|---|---|
| SCIP two-tier stratejisi (syntactic + semantic) | LLM entegrasyonu (Faz 5) |
| Language Adapter System (Py/TS/JS + **Rust/Go**) | Dashboard görselleştirme (Faz 6) |
| LCOM4 cohesion hesabı (`y` placeholder → gerçek) | Malicious witness detection (Faz 4) |
| Abstractness hesabı (`A` placeholder → gerçek → D anlamlı) | Makale yazımı (Faz 7) |
| Scale test: 100k+ node → KùzuDB karar | Multi-repo sync (Faz 4) |
| `osp-analyzer` crate (osp-spike'tan graduate) | OSP protokol sıkıştırma (Faz 5) |

---

## 2. SCIP vs Tree-sitter — Two-Tier Strateji

### 2.1 Mevcut Durum (Faz 0-2)

| Eksen | Kaynak | Placeholder? |
|---|---|---|
| `x` coupling | tree-sitter import out-degree | ❌ gerçek |
| `z` instability | tree-sitter Ce/(Ca+Ce) | ❌ gerçek |
| `w` entropy | git log Shannon H | ❌ gerçek |
| `v` witness-depth | git merge/trailer | ❌ gerçek |
| **`y` cohesion** | **0.5 placeholder** | ✅ **LCOM4 gerek** |
| **`A` abstractness** | **0.5 placeholder** | ✅ **class-type gerek** |
| **`D` main-seq** | `|A+I−1|` ama A=0.5 placeholder | ✅ **A gerçek olunca anlamlı** |

### 2.2 Two-Tier Karar

| Tier | Araç | Ne sağlar | Ne zaman |
|---|---|---|---|
| **Tier 1 (always-on)** | tree-sitter | Syntactic: imports, class/function tanımlar, dosya grafi | Her zaman — hızlı, language-server gerektirmez |
| **Tier 2 (optional enrichment)** | SCIP | Semantic: method-field access (LCOM4), inheritance, abstract type info | SCIP index mevcut olduğunda — zenginleştirme |

**Gerekçe:** tree-sitter her sistemde çalışır (sıfır ek bağımlılık). SCIP language-server gerektirir
(rust-analyzer/pyright/tsc/gopls) → opsiyonel. Tier 1 her zaman çalışır; Tier 2 varsa `y`/`A`
yerine gerçek değer koyar, yoksa placeholder kalır (Faz 1-2 davranışı).

### 2.3 SCIP Nedir?

**SCIP (SourceGraph Code Intelligence Protocol):** Language-server'ların (LSP) semantik
analiz sonucunu standart bir protobuf formatında serialize etmesi. Bir SCIP index dosyası
(`index.scip`) içerir:
- **Occurrences:** her sembolün (class/method/field) kaynak konumu
- **Relationships:** inheritance, implementation, method override
- **Symbols:** tanımlı semboller (abstract class, interface, concrete class)

```
SCIP index pipeline:
  repo → language-server (pyright/rust-analyzer/...) → index.scip → osp-analyzer parse
```

### 2.4 Dil Bazında SCIP Durumu

| Dil | SCIP indexer | Olgunluk | osp-analyzer'de |
|---|---|---|---|
| Python | `pyright --scip-python` | ✅ stable | Faz 3.2 |
| TypeScript | `scip-typescript` | ✅ stable | Faz 3.2 |
| Rust | `scip-rust` (rust-analyzer tabanlı) | 🟡 beta | Faz 3.3 |
| Go | `gopls -o index.scip` | 🟡 experimental | Faz 3.3 |
| C/C++ | `clangd` tabanlı | 🔴 alpha | Faz 3+ (stretch) |

---

## 3. Language Adapter System

### 3.1 Adapter Trait

```rust
/// Her dil için syntactic analiz arayüzü (Tier 1 — tree-sitter).
pub trait LanguageAdapter: Send + Sync {
    fn name(&self) -> &str;                    // "python", "typescript", "rust", "go"
    fn extensions(&self) -> &[&str];           // [".py"], [".ts", ".tsx"], [".rs"], [".go"]
    fn language(&self) -> tree_sitter::Language;
    
    /// Import deyimlerini çıkar (syntactic — ne import edildiğini söyler).
    fn extract_imports(&self, source: &str) -> Vec<ImportStatement>;
    
    /// Import'u gerçek dosyaya çözümle (contextual — reviewer 1 #4).
    /// External/stdlib import'ları internal edge gibi sayılmamalı (coupling şişmesi).
    fn resolve_import(&self, import: &ImportStatement, from_file: &Path, repo: &RepoContext)
        -> Option<ResolvedImport>;
    
    /// Class/function tanımlarını çıkar (abstractness için).
    fn extract_class_defs(&self, source: &str) -> Vec<ClassDef>;
}

pub struct ImportStatement {
    pub path: String,           // "foo.bar" (Python) / "./foo" (JS) / "crate::foo" (Rust)
    pub source_location: usize,
}

/// Reviewer 1 #4 — internal/external/stdlib ayrımı (coupling şişmesini önler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    Internal,           // repo içindeki dosyaya → edge oluştur
    External,           // üçüncü-parti paket → edge YOK
    StandardLibrary,    // std/stdlib → edge YOK
    Unknown,            // çözümlenemedi → config'e göre Internal veya atla
}

pub struct ResolvedImport {
    pub kind: ImportKind,
    pub target_path: Option<PathBuf>,  // Internal ise çözümlenen dosya yolu
}

pub struct ClassDef {
    pub name: String,
    pub is_abstract: bool,       // interface/abstract class/trait → true
    /// Reviewer 2 #4 — Rust: `trait X` = abstract; `impl X for Y` = concrete (değil).
    pub methods: Vec<String>,
    pub source_location: usize,
}
```

### 3.2 Adapter Gerçeklemeleri

| Adapter | tree-sitter grammar | Import pattern | Abstract pattern |
|---|---|---|---|
| `PythonAdapter` | tree-sitter-python | `import x`, `from x import y` | `class X(ABC):`, `class X(Protocol):` |
| `TypeScriptAdapter` | tree-sitter-typescript | `import x from "y"`, `export * from` | `abstract class X`, `interface X` |
| `JavaScriptAdapter` | tree-sitter-javascript | `import x from "y"`, `require("y")` | (JS'de abstract yok — A=0) |
| `RustAdapter` 🆕 | tree-sitter-rust | `use crate::foo`, `mod foo` | `trait X`, `abstract` yok ama trait≈abstract |
| `GoAdapter` 🆕 | tree-sitter-go | `import "path/to/pkg"` | `interface X` |

### 3.3 Adapter Registry

```rust
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl AdapterRegistry {
    pub fn default_all() -> Self {
        Self {
            adapters: vec![
                Box::new(PythonAdapter),
                Box::new(TypeScriptAdapter),
                Box::new(JavaScriptAdapter),
                Box::new(RustAdapter),    // Faz 3.1
                Box::new(GoAdapter),      // Faz 3.1
            ],
        }
    }
    
    pub fn adapter_for_extension(&self, ext: &str) -> Option<&dyn LanguageAdapter>;
}
```

---

## 4. LCOM4 Cohesion Hesabı (`y` placeholder → gerçek)

### 4.1 LCOM4 Algoritması

LCOM4 (Lack of Cohesion in Methods, variant 4) — bir class'ın method'larının field'lar
üzerinden bağlı bileşen sayısı:

```
LCOM4(C):
  1. C'nin method'ları: M = {m₁, m₂, ..., mₖ}
  2. C'nin field'ları: F = {f₁, f₂, ..., fₙ}
  3. Field-access relations: her mᵢ için Access(mᵢ) = {f ∈ F : mᵢ f'ye erişir}
  4. Bipartite graf G = (M ∪ F, E), (mᵢ, fⱼ) ∈ E iff fⱼ ∈ Access(mᵢ)
  5. Connected components(G) → LCOM4(C) = bileşen sayısı
```

**Örnek:**
```
Class User { name, email; getName(), getEmail(), validate() }
Access(getName)   = {name}
Access(getEmail)  = {email}
Access(validate)  = {name, email}     ← validate her iki grubu bağlıyor

Bipartite: getName-name, getEmail-email, validate-name, validate-email
→ validate iki grubu birleştirir → 1 bileşen → LCOM4 = 1 (cohesive ✓)

Eğer validate olmasaydı:
→ {getName, name} ∪ {getEmail, email} → 2 bileşen → LCOM4 = 2 (not cohesive ✗)
```

### 4.2 Field-Access Verisi: SCIP (Tier 2)

SCIP index'ten her method'un hangi field'lara eriştiğini çıkar:
```
SCIP occurrence: symbol="User.getName" location=...
SCIP occurrence: symbol="User.name" location=... (getName içinde)
→ field-access: (User.getName, User.name)
```

tree-sitter DAHİL field-access'i güvenilir şekilde çıkaramaz (dinamik dispatch, macro,
reflection). Bu yüzden LCOM4 **SCIP (Tier 2) gerektirir**. SCIP yoksa → placeholder 0.5 kalır.

### 4.3 Modül (Dosya) Seviyesi Aggregation

CohesionAxis per-module (dosya). Bir dosyada birden fazla class olabilir:

```
File F contains classes C₁, C₂, ..., Cₘ
LCOM4(F) = weighted_average(LCOM4(Cᵢ), weight = |Cᵢ.methods| / Σ|Cⱼ.methods|)
cohesion(F) = 1 / max(LCOM4(F), 1)   ∈ [0, 1]
```

- Tüm class'lar LCOM4=1 → cohesion(F) = 1.0 (tam kohezif)
- Ortalama LCOM4=3 → cohesion(F) = 0.33
- Class yok (sadece fonksiyonlar) → cohesion(F) = 1.0 (convention: fonksiyon-only modül kohezif)

### 4.4 SCIP Yoksa (Graceful Degradation)

```
if SCIP index available:
    cohesion = real LCOM4-based (§4.3)
else:
    cohesion = 0.5 (placeholder — same as Faz 1-2)
    log warning: "SCIP index missing, using placeholder y=0.5"
```

### 4.5 LCOM4 Edge-Case Kuralları (reviewer 1 #5, reviewer 2 #1)

| Durum | LCOM4 | Açıklama |
|---|---|---|
| 0 veya 1 method | **1** | Tek method her zaman kohezif |
| Field yok, method var | **1** (ama confidence düşük) | Method'lar aynı şeyi yapıyor olabilir ama yapısal bilgi yetersiz |
| **Inherited methods** (reviewer 2 #1) | **Harıç** | Sadece **o class'ta tanımlı** method'lar (standart LCOM4). Inherited methods field'lara erişse bile connected component'i değiştirmez |
| Constructor field access | **Dahil** | `__init__` / `new` field'lara erişirse component birleştirir |
| Getter/setter | **Dahil** (düşük ağırlık opsiyonel) | Basit getter/setter'lar component'i yapay birleştirebilir; opsiyonel düşük ağırlık |
| Static method → instance field | **Bağımsız component** | Static method instance field'a erişemez → ayrı component |
| Generated/vendor code | **Exclude** (config) | `generated/`, `vendor/`, `.gen.rs` gibi dizinler config ile hariç tutulur |
| Test/helper class | **Exclude** (config, opsiyonel) | `*_test.*`, `test_*` pattern'leri opsiyonel exclude |
| **Standalone functions alongside classes** (reviewer 2 #8) | **Warning** | Fonksiyonlar LCOM4'e girmez ama cohesion'ı olduğundan yüksek gösterebilir → diagnostic üret |

---

## 5. Abstractness Hesabı (`A` placeholder → gerçek)

### 5.1 Algoritma

Martin's Abstractness: `A = Nₐ / Nc` ∈ [0, 1]
- Nₐ = abstract type sayısı (interface, abstract class, trait, protocol)
- Nc = toplam type sayısı

### 5.2 Dil Bazında "Abstract" Tanımı

| Dil | Abstract türleri | Algılama |
|---|---|---|
| Python | `ABC` direkt subclass, `Protocol` | **Direkt:** tree-sitter `class X(ABC):` / `class X(Protocol):` → tespit edilir. **Dolaylı:** `class X(MyBase)` + `MyBase: ABC` → inheritance chain çözülemezse Placeholder (R1#5, R2#2) |
| TypeScript | `abstract class`, `interface` | tree-sitter: keyword check |
| Rust | `trait` | tree-sitter: `trait X` |
| Go | `interface` | tree-sitter: `interface X` |
| JavaScript | (yok — JS'de abstract yok) | A = 0 her zaman |

**Tier 1 (tree-sitter) yeterli:** abstractness için semantic SCIP gerektirmez — syntactic
keyword check yeterli. `A` gerçek değeri Tier 1'den gelebilir (SCIP opsiyonel).

### 5.3 Modül → Repo Aggregation

Abstractness per-module değil, **per-repo veya per-package** seviyesinde:

```
Repo R'deki tüm modüller:
  Nₐ(R) = Σ abstract types across all modules
  Nc(R) = Σ total types across all modules
  A(R) = Nₐ(R) / Nc(R)
```

Bu `EngineConfig.abstractness`'e verilir → `compute_derived`'da `D = |A + I − 1|` anlamlı olur.

---

## 6. Crate Yapısı: `osp-analyzer`

osp-spike (Faz 0 spike, frozen) → **osp-analyzer** (production code→space mapper):

```
crates/
├── osp-core/          # tipler (değişmez)
├── osp-spike/         # Faz 0 spike (frozen — reference only)
├── osp-analyzer/      # YENİ — production analyzer
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── adapters/           # Language Adapter System (Tier 1)
│   │   │   ├── mod.rs          # LanguageAdapter trait + Registry
│   │   │   ├── python.rs
│   │   │   ├── typescript.rs
│   │   │   ├── javascript.rs
│   │   │   ├── rust.rs         # 🆕 Faz 3.1
│   │   │   └── go.rs           # 🆕 Faz 3.1
│   │   ├── scip/               # Semantic indexer (Tier 2)
│   │   │   ├── mod.rs          # SCIP index loader
│   │   │   ├── lcom4.rs        # cohesion computation (§4)
│   │   │   ├── abstractness.rs # abstractness computation (§5)
│   │   │   └── proto.rs        # SCIP protobuf parse
│   │   ├── graph.rs            # tree-sitter → DepGraph (osp-spike'tan migrate)
│   │   ├── pipeline.rs         # full analysis: adapters + SCIP + bridge → Space
│   │   └── scale.rs            # scale benchmarks (§7)
│   └── tests/
│       ├── lcom4_test.rs       # LCOM4 unit tests (known class structures)
│       └── integration.rs      # end-to-end: repo → Space with real y/A
└── ...
```

### 6.1 MetricValue Provenance Model (reviewer 1 #1)

Her metric, **değerin nereden geldiğini** taşır — "0.5 çünkü bilmiyoruz" ≠ "0.5 çünkü gerçekten orta":

```rust
#[derive(Debug, Clone)]
pub enum MetricSource {
    TreeSitter,   // Tier 1 syntactic
    Scip,         // Tier 2 semantic
    Placeholder,  // veri yok
    Heuristic,    // approximate
}

#[derive(Debug, Clone)]
pub struct MetricValue {
    pub value: f64,
    pub source: MetricSource,
    pub confidence: f64,   // [0,1] — source'a + coverage'a bağlı
    pub coverage: f64,     // [0,1] — SCIP varsa index coverage; yoksa 0
}

/// Reviewer #7 — Confidence hesaplama formülü:
/// `confidence = source_base × coverage × stale_penalty`
///
/// | Source | base | coverage | stale_penalty |
/// |---|---|---|---|
/// | TreeSitter | 0.75 | 1.0 (syntactic全覆盖) | 1.0 |
/// | Scip | 0.95 | index coverage ratio | 0.5 if stale else 1.0 |
/// | Placeholder | 0.0 | 0.0 | — |
/// | Heuristic | 0.4–0.7 | depends | 1.0 |
impl MetricValue {
    pub fn placeholder(value: f64) -> Self {
        Self { value, source: MetricSource::Placeholder, confidence: 0.0, coverage: 0.0 }
    }
    pub fn tree_sitter(value: f64, coverage: f64) -> Self {
        // R1#3 — coverage her zaman 1.0 değil (parse error, unsupported ext, generated exclude)
        Self { value, source: MetricSource::TreeSitter, confidence: 0.75 * coverage, coverage }
    }
    /// `coverage` = SemanticCoverage.coverage_ratio ile aynı (R2#1 naming consistency).
    pub fn scip(value: f64, coverage: f64, stale: bool) -> Self {
        let stale_penalty = if stale { 0.5 } else { 1.0 };
        Self {
            value, source: MetricSource::Scip,
            confidence: 0.95 * coverage * stale_penalty,
            coverage,
        }
    }
}
```

### 6.2 SemanticCoverage (reviewer 1 #2)

SCIP index partial/stale olabilir → confidence düşer:

```rust
#[derive(Debug, Clone)]
pub struct SemanticCoverage {
    pub files_total: usize,
    pub files_with_scip: usize,
    pub classes_total: usize,
    pub classes_with_field_access: usize,
    pub coverage_ratio: f64,       // files_with_scip / files_total
    pub index_commit: Option<String>,  // SCIP index'in üretildiği commit
    pub repo_head: String,              // repo'nun güncel HEAD'i
    pub stale: bool,                    // index_commit ≠ repo_head
}

impl SemanticCoverage {
    /// SCIP yok — coverage=0, stale=false (reviewer #5: repo_head zorunlu parametre).
    pub fn none(repo_head: String) -> Self {
        Self {
            files_total: 0, files_with_scip: 0,
            classes_total: 0, classes_with_field_access: 0,
            coverage_ratio: 0.0, index_commit: None,
            repo_head, stale: false,
        }
    }
}
```

### 6.3 AnalysisResult Output Contract (reviewer 1 #3,10)

```rust
pub struct AnalysisResult {
    pub space: Space,                           // graph + positions (osp-core)
    pub module_metrics: HashMap<NodeId, ModuleMetrics>,
    pub repo_metrics: RepoMetrics,
    pub semantic_coverage: SemanticCoverage,
    pub graph: DepGraph,                        // raw dependency graph
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

pub struct ModuleMetrics {
    pub coupling: MetricValue,      // x
    pub cohesion: MetricValue,      // y (SCIP ise gerçek LCOM4; yoksa Placeholder)
    pub instability: MetricValue,   // z
}

pub struct RepoMetrics {
    pub abstractness: MetricValue,              // A — Tier 1 keyword check
    pub main_sequence_distance: MetricValue,    // D = |A + I - 1|
    /// Reviewer 1 #6 — package-level breakdown (opsiyonel, rapor için)
    pub abstractness_by_package: Option<HashMap<String, f64>>,
}

pub struct AnalysisDiagnostic {
    pub severity: DiagnosticSeverity,  // Info, Warning, Error (R1#3)
    pub code: DiagnosticCode,          // R1#3 — structured diagnostic
    pub message: String,
    pub file: Option<String>,
}

/// R1#3 — Error: parse failed ama pipeline devam; Warning: degraded; Info: note.
#[derive(Debug, Clone)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// R1#3 — structured diagnostic codes (raporlama + test için).
#[derive(Debug, Clone)]
pub enum DiagnosticCode {
    UnknownImport,
    ScipIndexStale,
    ParseFailed,
    PlaceholderMetric,
    GeneratedExcluded,
    CoverageLow,
}
```

### 6.4 Pipeline

```rust
/// Full analysis pipeline: repo → AnalysisResult with real y/A (when SCIP available).
pub fn analyze_repo(repo: &Path, config: &AnalysisConfig) -> Result<AnalysisResult> {
    // Tier 1: tree-sitter syntactic (always)
    let (graph, class_defs) = extract_syntactic(repo, &config.adapters)?;
    let abstractness = compute_abstractness(&class_defs)?;  // Tier 1 keyword check
    
    // R1#2/R2#5 — repo_instability: module-level I'ların mass-weighted average
    let module_instability: HashMap<NodeId, f64> = compute_module_instability(&graph);
    let repo_instability = weighted_average_by_mass(&module_instability, &graph);
    
    // Tier 2: SCIP semantic (optional — reviewer 1 #2 coverage tracking)
    let (cohesion, coverage) = if let Some(scip_path) = &config.scip_index {
        let index = load_scip_index(scip_path)?;
        let cov = compute_coverage(&index, repo)?;
        let lcom4 = compute_lcom4_per_module(&index, &class_defs)?;
        (lcom4, cov)
    } else {
        // R1#5 — none() repo_head parametresi alır
        (MetricValue::placeholder(0.5), SemanticCoverage::none(repo_head(repo)?))
    };
    
    // R2#3 — TODO: bridge_to_space pozisyonları MetricValue'larla başlatacak mı,
    // yoksa SpaceEngine sonradan reposition mu yapacak? Faz 3.1'de netleştir.
    let space = bridge_to_space(&graph);  // graph yapısı → Space (positions default)
    
    Ok(AnalysisResult {
        space,
        module_metrics: build_module_metrics(&graph, &cohesion, &module_instability),
        repo_metrics: RepoMetrics {
            abstractness: abstractness.clone(),
            // R1#2 — D = |A + I − 1|; A repo-level, I module-level → D_module per-module
            // D_repo = weighted_average(D_module, weight=mass)
            main_sequence_distance: compute_d(&abstractness, &repo_instability),
            abstractness_by_package: None,  // Faz 3 opsiyonel (reviewer 1 #6)
        },
        semantic_coverage: coverage,
        graph,
        diagnostics: vec![],
    })
}
```

### 6.5 AnalysisConfig (R1#4, R2#6)

```rust
/// Unknown import'lar için politika.
#[derive(Debug, Clone)]
pub enum UnknownImportPolicy {
    DiagnosticOnly,  // edge YOK, diagnostic üret (default — coupling şişmez)
    Skip,            // sessizce atla
    TreatAsInternal, // internal edge gibi say (riskli)
}

pub struct AnalysisConfig {
    pub adapters: AdapterRegistry,
    pub scip_index: Option<PathBuf>,
    pub unknown_import_policy: UnknownImportPolicy,
    pub exclude_generated: bool,   // generated/, .gen.rs
    pub exclude_vendor: bool,     // vendor/, node_modules/
    pub exclude_tests: bool,      // *_test.* — default false (R1#4: test'ler mimari için faydalı)
}
```

> **R2#4 — `RepoContext`** (`resolve_import` parametresi): `repo_root` + `module_map`
> içerir. Faz 3.3'te tanımlanacak. §10 #10 açık soru.

---

## 7. Scale Test + KùzuDB Karar Kriterleri

### 7.1 Scale Benchmark Plan (reviewer 1 #7 — split: semantic vs storage)

**Semantic Benchmark** (adapter + SCIP dillerinde):
| Repo | Dil | ~Files | ~Nodes | Amaç |
|---|---|---|---|---|
| click | Python | 60 | 60 | baseline (known) |
| django | Python | 3k | 3k | medium (known) |
| tokio | Rust | 500 | 500 | Rust medium |
| gin | Go | 300 | 300 | Go medium |
| lodash | JS | 500 | 500 | JS medium |
| vue | TS | 2k | 2k | large TS |

**Storage/Graph Stress** (semantic analiz YOK — C adapter yok, file-level graph only):
| Repo | Dil | ~Files | ~Nodes | Amaç |
|---|---|---|---|---|
| Linux kernel (drivers/) | C | ~10k | ~10k | graph scale stress |
| Linux kernel (full) | C | ~60k | ~60k+ | **KùzuDB karar** |

> C/C++ SCIP alpha → Linux kernel sadece **graph-storage** benchmark. Semantic (LCOM4/A)
> bu repolarda çalışmaz. Etiket: "storage benchmark", "semantic benchmark" değil.

### 7.2 KùzuDB Decision Matrix (reviewer 1 #8 — multi-criteria, node-count tek başına değil)

**KùzuDB gerekli (herhangi biri):**
| Metric | Threshold |
|---|---|
| RAM usage | > 4 GB |
| Analysis time | > 10 min |
| `compute_reposition_set` | > 5 s |
| Snapshot save/load | kabul edilemez (> 30s) |

**Observation-only (> 50k node):** Node count tek başına DB migration trigger ETMEZ.
60k node Rust HashMap'de ~500MB — yeterli olabilir. Sadece yakın izle.

> Reviewer 2 #5: RAM ölçümü için `dhat` (heap profiler) kullan — `cargo bench` yetersiz.

### 7.3 KùzuDB Entegrasyon Planı (gerekirse)

Eğer scale test KùzuDB'yi gerektirirse:
- `Space` struct'unun altında bir KùzuDB graph store
- `compute_reposition_set` → Cypher query (`MATCH (n)-[*1]-(m) WHERE n IN $delta RETURN m`)
- Snapshot/milestone → KùzuDB checkpoint
- **inv #8 uyumu:** KùzuDB embedded (network-free) → osp-core bağımlılık olarak OK

---

## 8. Faz 3 Re-spike (15-repo, gerçek y/A)

### 8.1 Hedef

15-repo kalibrasyon korpusunu gerçek `y` (LCOM4) + `A` (abstractness) ile yeniden çalıştır.
D (Martin main-sequence) artık anlamlı → repos arası mimari kalite karşılaştırması.

### 8.2 Beklenen İyileşmeler

| Repo | Faz 2 (placeholder) | Faz 3 (gerçek) | Beklenen |
|---|---|---|---|
| click | y=0.5, A=0.5, D=? | y=LCOM4, A=real | D temiz (good DDD) |
| django | y=0.5, A=0.5, D=0.0 | y=LCOM4, A=real | D daha anlamlı (MTV) |
| worms | y=0.5, A=0.5, D=0.5 | y=LCOM4, A=? | D yüksek (foam → off main-seq) |
| fastapi | y=0.5, A=0.5, D=0.5 | y=LCOM4, A=real | D orta (modern arch) |

### 8.3 SCIP Index Üretimi

```bash
# Python reposu için
pip install scip-python
scip-python index ./repo --output index.scip

# TypeScript reposu için
npm install scip-typescript
scip-typescript index ./repo --output index.scip

# Rust reposu için (beta)
cargo install scip-rust
scip-rust ./repo --output index.scip
```

osp-analyzer bu index dosyalarını otomatik algılar veya `--scip <path>` ile explicit verilir.

---

## 9. Faz 3 Uygulama Planı (reviewer 1 #10 — contract-first sıra)

> **Contract önce:** `AnalysisResult` + `MetricValue` (§6) tanımlı → adapter'lar
> aynı hedefe yazılır. Pipeline contract oturmadan adapter implementasyonu = rework.

| Adım | İçerik | Çıktı |
|---|---|---|
| 3.1 | **`osp-analyzer` crate iskeleti** + `MetricValue`/`AnalysisResult`/`SemanticCoverage` contract (§6) | crate + tipler |
| 3.2 | **Python + TS/JS adapter migration** (osp-spike graph.rs'tan) | adapters |
| 3.3 | **Import resolver** + `ImportKind` internal/external/stdlib ayrımı (§3.1, reviewer 1 #4) | resolve_import |
| 3.4 | **Rust + Go adapter'ları** (tree-sitter-rust, tree-sitter-go) | multi-language |
| 3.5 | **Abstractness** (Tier 1 keyword check → A gerçek → D anlamlı) | A gerçek |
| 3.6 | **SCIP loader minimal projection** (partial protobuf, reviewer 2 #6) | index.scip parse |
| 3.7 | **LCOM4** (known fixtures + edge-case rules §4.5) | y gerçek (SCIP varsa) |
| 3.8 | **Metric provenance + coverage** (confidence, stale index, §13) | MetricValue complete |
| 3.9 | **Scale benchmark** (semantic + storage split, §7) → `docs/scale-bench-v3.md` | KùzuDB karar |
| 3.10 | **15-repo re-spike** → `docs/spike-results-v3.md` | D anlamlı, makale verisi |

**Kabul kriterleri:**
- AnalysisResult contract + MetricValue provenance her metric'te çalışıyor
- Import resolver external/stdlib'yi internal edge gibi saymıyor (coupling doğru)
- Rust + Go repoları analiz edilebiliyor
- LCOM4 hesabı SCIP index ile çalışıyor (known class fixture'lerle doğrulandı)
- Abstractness gerçek değer (placeholder 0.5 değil)
- D = |A + I − 1| anlamlı (repos arası ayrım gösteriyor)
- SCIP coverage/stale tespiti çalışıyor (MetricValue.confidence doğru)

---

## 10. Açık Sorular

1. ~~**SCIP protobuf parse:** partial mı full mü?~~ → **PARTIAL** (reviewer 2 #6).
   Sadece `occurrences` + `relationships` + `symbols` alanları → lightweight proto tanımı.
2. **LCOM4 aggregation:** Dosyada tek class vs çok class — weighted average (method-count ağırlıklı). **Kabul edildi.**
3. **Mixed-language repos:** Python + TypeScript same repo — separate analysis + merge.
   **Cross-language FFI edges** (Python→Rust) → `EdgeKind::FFIBinds` **Faz 4** (reviewer 2 #7).
4. **SCIP cache:** `.scip-cache/` + **commit-hash + branch name** ile invalidate (reviewer 2 #9).
5. **KùzuDB migration:** `SpaceStore` trait — in-memory ve KùzuDB iki implementasyon.
6. **Rust trait ≈ abstract:** `trait X` = abstract; `impl X for Y` = concrete (reviewer 2 #4). Test notu.
7. **SCIP edges in `compute_reposition_set`** (reviewer 2 #3): trait değişince implementors
   reposition'a girmeli mi? `Inheritance` + `FieldAccess` edge'leri → açık soru, Faz 3.8'de değerlendir.
8. **A graceful degradation** (reviewer 2 #2): Python `ABC` tespiti tree-sitter ile zor olabilir
   (inheritance chain gerek). `AbstractnessSource::Placeholder` fallback → A=0.5.
9. **Abstractness per-package** (reviewer 1 #6): repo-level A yanıltıcı olabilir.
   `abstractness_by_package` breakdown raporda. Faz 3 için note-level.
10. **`RepoContext` tanımı** (reviewer 2 #4): `resolve_import` parametresi — `repo_root: PathBuf`
    + `module_map: HashMap<PathBuf, NodeId>` içerir. Faz 3.3'te tanımlanacak.

---

## 11. Karar Özeti

| Karar | Seçim | Gerekçe |
|---|---|---|
| Analiz stratejisi | **Two-tier** (tree-sitter always + SCIP optional) | Hız (Tier 1) + derinlik (Tier 2) |
| LCOM4 verisi | **SCIP (Tier 2)** — field-access gerek | tree-sitter field-access'i güvenilir çıkaramaz |
| Abstractness verisi | **tree-sitter (Tier 1)** — keyword check yeterli | `abstract class` / `trait` / `interface` syntactic |
| Crate yapısı | **`osp-analyzer`** (osp-spike frozen) | Production vs spike ayrımı |
| Rust/Go desteği | **tree-sitter-rust + tree-sitter-go** | Language Adapter System ile |
| Scale test | **Linux-kernel-drivers → full** | 10k→60k node progression |
| KùzuDB eşiği | **RAM>4GB VEYA time>10min VEYA reposition>5s VEYA snapshot>30s** | Node-count>50k yalnızca observation (§7.2) |
| LCOM4 aggregation | **Weighted average** (method-count) | Büyük class'lar daha ağırlıklı |
| SCIP cache | **`.scip-cache/` + commit-hash + branch-name invalidate** | Language-server maliyeti amortize (reviewer 2 #9) |
| Rust trait | **abstract sayılır** (interface gibi) | A = (traits + abstract) / total_types |
| Confidence | `base_source × coverage × stale_penalty` | TreeSitter=0.75, SCIP=0.95, Placeholder=0.0 |
| Unknown import | **DiagnosticOnly** (no edge, no coupling inflation) | AnalysisConfig policy (reviewer 1 #6) |

---

## 12. Analysis Quality Rules (reviewer #8)

1. **Placeholder metric gerçek metric gibi yorumlanamaz.** `MetricValue.confidence = 0.0` → raporda "veri yok" olarak işaretle.
2. **`MetricValue.confidence` raporda gösterilir.** Faz 3 re-spike tablosunda her metrik için confidence sütunu.
3. **SCIP stale ise confidence düşürülür.** `stale_penalty = 0.5` → confidence yarıya iner.
4. **Unknown import internal edge üretmez.** `UnknownImportPolicy::DiagnosticOnly` default — coupling şişmesini önler.
5. **Generated/vendor/test exclude policy rapora yazılır.** Hangi dosyalar hariç tutuldu, kaç dosya etkilendi.
6. **D hesabı hangi scope'ta yapıldıysa raporda belirtilir.** `D_module = |A_repo + I_module − 1|` (A repo-level, I module-level).
7. **MetricValue finite invariant** (R1#2): `value`/`confidence`/`coverage` finite olmalı (NaN/Inf yasak); `confidence` ve `coverage` ∈ [0,1]. `MetricValue::new()` constructor validate eder.

---

## 13. Faz 3 → Faz 4 Köprüsü

Faz 3 gerçek cohesion + abstractness sağlar. Faz 4 (Comparison + Security):
- **"O mu? Bu mu?"** — iki repo-uzayını entity-align et + rezonans skoru
- **Malicious Witness Detection** — Sybil resistance (inv #4 → `osp-core::witness`)
- **Multi-repo sync** — birden fazla repo'nun uzayını tek Space'de birleştir

**SCIP Analyzer bu geçişe hazır:** gerçek `y`/`A` ile pozisyonlar doğru → entity alignment
(GitHub issue/literatür taraması §4) daha anlamlı.

---

*Sonraki: Faz 3.1 — `osp-analyzer` crate iskeleti + `MetricValue`/`AnalysisResult`/`SemanticCoverage` contract.*
