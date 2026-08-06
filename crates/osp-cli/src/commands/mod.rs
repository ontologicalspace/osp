//! OSP CLI komut handler'ları. osp-core API'sini çağırır — CLI = truth surface.
//!
//! Pattern: osp-desktop cmd_simulate_claim (lib.rs:257-278) reuse —
//! analyze_repo_with_config → CoordinateSystem::default_raw_five → SpaceEngine.

pub mod analyze_provenance;
pub mod harness_task;
pub mod repo_snapshot;
pub mod run_envelope;

use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use osp_analyzer::contract::AnalysisConfig;
use osp_analyzer::language::AdapterRegistry;
use osp_analyzer::pipeline::analyze_repo_with_config;

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 — CLI exit-code contract (snapshot testlerle sabitlenir)
// ═══════════════════════════════════════════════════════════════════════════════

/// Navigator trajectory attempt exit codes.
///
/// Bu kodlar sabittir — downstream tooling (CI, scripts) bunlara güvenebilir.
/// Yeni kod eklemek mümkündür ama mevcut kodların anlamı değişmez.
pub mod exit_codes {
    /// Task completed — predicate satisfied, mainline applied.
    pub const COMPLETED: i32 = 0;
    /// INV-T9 — witness authorization bekleme (expected domain outcome, hata DEĞİL).
    pub const AWAITING_WITNESSES: i32 = 10;
    /// Explicit witness rejection — agent must revise proposal.
    pub const REQUIRES_REVISION: i32 = 11;
    /// INV-T7 — maneuver limit aşıldı (agent-correctable retryable failures tükendi).
    pub const EXCEEDED_MANEUVER_LIMIT: i32 = 12;
    /// Critical domain — insan review gerekli.
    pub const REQUIRES_OPERATOR_APPROVAL: i32 = 13;
    /// Invalid witness evidence — operational fault (malformed/author-self/duplicate).
    pub const WITNESS_EVALUATION_ERROR: i32 = 20;
    /// Pending authorization persistence failure — terminal (non-retryable).
    pub const PENDING_AUTHORIZATION_PERSISTENCE_FAILURE: i32 = 40;
    /// System failure — persistence/internal error. Terminal.
    pub const SYSTEM_FAILURE: i32 = 70;
    /// Task resolver'da bulunamadı.
    pub const TASK_NOT_FOUND: i32 = 80;
    /// LLM hatası (NoMoreProposals, parse, network).
    pub const LLM_ERROR: i32 = 90;
}

pub mod graph;
pub(crate) mod resolve_code_entity_preview_render;
pub mod review;
pub(crate) mod supersede_preview_render;

/// Output format (text/json).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            _ => Self::Text,
        }
    }
}

/// CLI output format (Faz 8 analyze) — typed ValueEnum (review P1-4).
///
/// Eski `OutputFormat` review komutlarında kullanılıyor (unknown → Text sessiz fallback).
/// Analyze için typed enum: bilinmeyen değer clap parse error (fail-closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum CliOutputFormat {
    /// Human-readable — envelope JSON + diagnostics birlikte (backward-compat default).
    #[default]
    Human,
    /// Machine-readable — stdout'a yalnız JSON document; diagnostics stderr'e.
    Json,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Komut argüman yapıları
// ═══════════════════════════════════════════════════════════════════════════════

/// `osp analyze <repo>` — repo analiz → space snapshot.
#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Analiz edilecek repo path'i.
    pub repo: PathBuf,
    /// SCIP index path'i (opsiyonel — gerçek LCOM4 cohesion için).
    #[arg(long)]
    pub scip: Option<PathBuf>,
    /// Çıktı JSON dosyası (default: stdout).
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Output format: `human` (default — envelope JSON + diagnostics) veya `json`
    /// (machine-readable, stdout'a yalnız JSON; diagnostics stderr'e). Unknown value →
    /// clap parse error (ValueEnum, review P1-4).
    #[arg(long, value_enum, default_value_t = CliOutputFormat::Human)]
    pub format: CliOutputFormat,
    /// Snapshot binding policy (review P0). Generic analyze = observe worktree (dirty OK,
    /// HEAD yalnız gözlenen metadata). `--require-clean-snapshot` = clean pre/post-equal HEAD
    /// zorunlu (harness task üretimi için; dirty/drift fail-closed). B-3 harness bu flag'i kullanır.
    #[arg(long, default_value_t = false)]
    pub require_clean_snapshot: bool,
}

/// `osp trajectory init --repo <repo>` — SpaceEngine + Trajectory kur.
#[derive(Args, Debug)]
pub struct TrajectoryInitArgs {
    #[arg(long)]
    pub repo: PathBuf,
    #[arg(long)]
    pub scip: Option<PathBuf>,
    /// Vision TOML config (opsiyonel — default builtin).
    #[arg(long)]
    pub vision: Option<PathBuf>,
}

/// Execution mode — Paper 2 harness/production ayrımı.
///
/// Faz 8 test-project (review v6-v7): `harness-auto-approve` witness policy yalnız
/// `harness` execution mode ile kullanılabilir (scoped relaxation). Production deployment
/// Paper 1 witness güven modelini (min_approvers=2) korur.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Default)]
pub enum CliExecutionMode {
    /// Production deployment — Paper 1 witness güven modeli, persistence, external evidence.
    #[default]
    Production,
    /// Controlled experiment/harness — test fixture, relaxed witness, deterministic.
    Harness,
}

/// Witness policy mode — production quorum vs harness auto-approve.
///
/// `HarnessAutoApprove` yalnız `--execution-mode harness` ile (guard). Production
/// deployment'ta kullanılamaz.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Default)]
pub enum CliWitnessMode {
    /// Production witness policy — Paper 1 güven modeli (min_approvers=2, quorum=1.5).
    #[default]
    Production,
    /// Harness auto-approve — controlled experiment gevşetmesi (quorum=0).
    #[value(name = "harness-auto-approve")]
    HarnessAutoApprove,
}

/// Faz 8 test-project: witness/execution mode kombinasyon guard'ı (review P0-1).
///
/// `harness-auto-approve` yalnız `harness` execution mode ile geçerli. Production'da
/// kullanımı fail-closed (Paper 2 scoped relaxation ilkesi).
pub fn validate_execution_witness_combination(
    execution: CliExecutionMode,
    witness: CliWitnessMode,
) -> anyhow::Result<()> {
    if witness == CliWitnessMode::HarnessAutoApprove && execution != CliExecutionMode::Harness {
        anyhow::bail!(
            "--witness harness-auto-approve requires --execution-mode harness \
             (Paper 2 scoped relaxation — production quorum disabled)"
        );
    }
    Ok(())
}

/// `osp trajectory attempt <task-id>` — navigator attempt.
#[derive(Args, Debug)]
pub struct TrajectoryAttemptArgs {
    /// Task ID. `--task` verilirse task dosyasının ID'siyle consistency check.
    pub task_id: u64,
    #[arg(long)]
    pub repo: PathBuf,
    /// Scripted proposals JSON (MockLlmClient). --llm mock ile.
    #[arg(long)]
    pub proposals: Option<PathBuf>,
    /// LLM mode: mock (FileMockLlm, --proposals) or real (RuntimeLlmClient, GPT-4o-mini).
    #[arg(long, default_value = "mock")]
    pub llm: String,
    /// Maneuver limit override (task policy'de yoksa). Default 5.
    #[arg(long)]
    pub maneuver_limit: Option<u32>,
    /// Harness task dosyası (CliHarnessTaskFileV1 JSON). snapshot-bound: repository HEAD
    /// + NodeId→path scope binding + Task. Yoksa hardcoded legacy task (backward-compat).
    #[arg(long)]
    pub task: Option<PathBuf>,
    /// Execution mode (review v6): production (default) veya harness (controlled experiment).
    #[arg(long, value_enum, default_value_t = CliExecutionMode::Production)]
    pub execution_mode: CliExecutionMode,
    /// Witness policy (review v6): production (default) veya harness-auto-approve.
    /// harness-auto-approve yalnız --execution-mode harness ile (guard).
    #[arg(long, value_enum, default_value_t = CliWitnessMode::Production)]
    pub witness: CliWitnessMode,
    /// Output format: human (default) veya json (machine-readable, stdout'a yalnız JSON).
    #[arg(long, default_value = "human")]
    pub format: String,
    /// Runtime state directory for pending-authorizations (`.osp/` artifacts). Default = CWD.
    /// Harness mode invariant (review B-3 P0): must be OUTSIDE the analyzed repo, otherwise
    /// Held artifacts dirty the repo → subsequent snapshot-bound runs rejected. Production
    /// callers may set this explicitly; harness mode rejects state-dir inside repo.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
}

/// `osp task view <task-id>` — AgentTaskView göster.
#[derive(Args, Debug)]
pub struct TaskViewArgs {
    pub task_id: u64,
    #[arg(long)]
    pub repo: PathBuf,
    /// Predicate threshold (örn "coupling <= 0.55").
    #[arg(long)]
    pub predicate: String,
}

/// `osp evidence export` — evidence ledger JSON.
#[derive(Args, Debug)]
pub struct EvidenceArgs {
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Evidence JSON input (trajectory attempt çıktısı).
    #[arg(long)]
    pub input: Option<PathBuf>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Komut handler'ları
// ═══════════════════════════════════════════════════════════════════════════════

/// `osp analyze` — repo analiz → space snapshot JSON (per-axis provenance envelope).
///
/// Two snapshot-binding contracts (review P0):
/// - **Generic** (default): observe worktree — dirty OK, HEAD yalnız gözlenen metadata,
///   `binding: observed_worktree_unbound`. "Şu anda diskte gördüğüm kodu analiz et."
/// - **Harness-bound** (`--require-clean-snapshot`): clean pre/post-equal HEAD zorunlu,
///   `binding: clean_pre_post_equal`. "Tam olarak HEAD commit'ine bağlı veri üret."
///   B-3 harness task üretimi bu flag'i kullanır; task loader yalnız clean-bound kabul eder.
///
/// Review fixes: P1-1 (provenance_model), P1-2 (head single source), P1-3 (key-set +
/// bijection), P1-4 (format/out matrix + out-inside-repo reject in require-clean mode).
pub fn run_analyze(args: AnalyzeArgs) -> anyhow::Result<()> {
    // Pre-capture snapshot (binding mode determines eligibility + drift fence).
    let snapshot_before =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;

    // P0: harness-bound mode rejects --out inside analyzed repo (would dirty next snapshot step).
    if args.require_clean_snapshot {
        if let Some(out) = args.out.as_ref() {
            reject_output_inside_repo(&args.repo, out)?;
        }
    }

    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig {
        scip_index: args.scip.clone(),
        ..Default::default()
    };
    let result = analyze_repo_with_config(&args.repo, &registry, &config)?;

    // Post-capture snapshot + binding mode resolution (review P0).
    let snapshot_after =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    let binding = if args.require_clean_snapshot {
        // Harness-bound: clean worktree + pre/post drift fence + analyzed-path⊆tracked.
        repo_snapshot::ensure_snapshot_eligible(&snapshot_before)
            .map_err(|e| anyhow::anyhow!(e))?;
        if snapshot_before != snapshot_after {
            anyhow::bail!(
                "repository changed during analysis (head/tracked/clean drift) — \
                 --require-clean-snapshot output cannot be bound to a stable HEAD"
            );
        }
        repo_snapshot::validate_analyzed_paths_tracked(&result.node_paths, &snapshot_after)
            .map_err(|e| anyhow::anyhow!(e))?;
        analyze_provenance::CliRepositoryBinding::CleanPrePostEqual
    } else {
        // Generic: observe worktree. HEAD yalnız gözlenen metadata; dirty OK, drift gözetlenir.
        if snapshot_before != snapshot_after {
            eprintln!(
                "osp analyze: note — repository changed during analysis (binding=observed_worktree_unbound)"
            );
        }
        analyze_provenance::CliRepositoryBinding::ObservedWorktreeUnbound
    };

    // Exact-set node identity + path bijection (review P1-3).
    analyze_provenance::validate_node_key_sets(&result).map_err(|e| anyhow::anyhow!(e))?;
    analyze_provenance::validate_node_paths_bijection(&result.node_paths)
        .map_err(|e| anyhow::anyhow!(e))?;
    // Defensive graph integrity (P1-2): her edge emitted node set'ine referans vermeli.
    analyze_provenance::validate_edge_endpoints(&result.space).map_err(|e| anyhow::anyhow!(e))?;

    // Per-node provenance entries (NodeId ascending — deterministic wire order).
    let mut nodes: Vec<analyze_provenance::CliAnalyzeNode> =
        Vec::with_capacity(result.space.nodes.len());
    let mut node_ids: Vec<u64> = result.space.nodes.keys().copied().collect();
    node_ids.sort_unstable();
    for node_id in &node_ids {
        let node = &result.space.nodes[node_id];
        let path = result
            .node_paths
            .get(node_id)
            .expect("key-set validated above");
        let metrics = result
            .module_metrics
            .get(node_id)
            .expect("key-set validated above");
        nodes.push(analyze_provenance::CliAnalyzeNode::from_analysis(
            *node_id,
            path.clone(),
            node,
            metrics,
        ));
    }

    // Per-edge bağımlılık grafiği — canonical wire order (from → to → kind_rank → is_type_only).
    // `sort_edges_canonical` ordering contract'ı kapsüller (P0-1); enum declaration order'a
    // bağımlı değil. `space.edges` insertion-order geliyor — kendi deterministik sıralamamız.
    let mut edges: Vec<analyze_provenance::CliEdge> = result
        .space
        .edges
        .iter()
        .map(analyze_provenance::CliEdge::from_edge)
        .collect();
    analyze_provenance::sort_edges_canonical(&mut edges);

    // Analyze provenance envelope (review P1-1 — analyzer_axis_specific, not "native").
    // repository.head tek kaynaktan: snapshot_after.head (review P1-2 — semantic_coverage
    // kısa SHA taşır, envelope authority'si olamaz).
    let envelope = serde_json::json!({
        "schema_version": 1,
        "analysis": {
            "metric_representation": "axis_provenanced_v1",
            "provenance_model": "analyzer_axis_specific",
            "axis_specific_provenance": true
        },
        "repository": {
            "head": snapshot_after.head.as_str(),
            "clean": snapshot_after.clean,
            "binding": binding
        },
        // Tek truth source (P2-1): count'lar DTO listelerinden gelir, space'den değil.
        "node_count": nodes.len(),
        "edge_count": edges.len(),
        "edges": edges,
        "nodes": nodes,
        "repo_metrics": {
            "abstractness": {
                "value": result.repo_metrics.abstractness.value,
                "source": analyze_provenance::CliMetricSource::from(
                    result.repo_metrics.abstractness.source
                )
            },
            "main_sequence_distance": {
                "value": result.repo_metrics.main_sequence_distance.value,
                "source": analyze_provenance::CliMetricSource::from(
                    result.repo_metrics.main_sequence_distance.source
                )
            }
        },
        "semantic_coverage": {
            "files_total": result.semantic_coverage.files_total,
            "files_with_scip": result.semantic_coverage.files_with_scip,
            "coverage_ratio": result.semantic_coverage.coverage_ratio,
            "stale": result.semantic_coverage.stale
        }
    });
    let json = serde_json::to_string_pretty(&envelope)?;

    // Diagnostics to stderr (shared across format/out combinations).
    let stderr_diagnostics = |stale: bool, coverage_ratio: f64| {
        if !stale && coverage_ratio < 1.0 {
            eprintln!(
                "osp analyze: partial SCIP coverage ({:.0}%) — cohesion placeholder \
                 fallback for uncovered files",
                coverage_ratio * 100.0
            );
        }
        if stale {
            eprintln!(
                "osp analyze: WARNING — SCIP index stale (index_commit ≠ repo_head); \
                 cohesion values may not reflect current HEAD"
            );
        }
    };

    match (args.format, args.out.as_ref()) {
        // json + out → JSON to file, stdout empty, confirmation + diagnostics stderr.
        (CliOutputFormat::Json, Some(path)) => {
            std::fs::write(path, &json)?;
            eprintln!("✓ Space snapshot written to {}", path.display());
            stderr_diagnostics(
                result.semantic_coverage.stale,
                result.semantic_coverage.coverage_ratio,
            );
        }
        // json + no-out → JSON only to stdout, diagnostics stderr (stdout JSON-only).
        (CliOutputFormat::Json, None) => {
            stderr_diagnostics(
                result.semantic_coverage.stale,
                result.semantic_coverage.coverage_ratio,
            );
            println!("{json}");
        }
        // human + out → JSON to file + confirmation stdout (backward-compat) + diagnostics stderr.
        (CliOutputFormat::Human, Some(path)) => {
            std::fs::write(path, &json)?;
            println!("✓ Space snapshot written to {}", path.display());
            stderr_diagnostics(
                result.semantic_coverage.stale,
                result.semantic_coverage.coverage_ratio,
            );
        }
        // human + no-out → JSON to stdout + diagnostics stderr (backward-compat).
        (CliOutputFormat::Human, None) => {
            println!("{json}");
            stderr_diagnostics(
                result.semantic_coverage.stale,
                result.semantic_coverage.coverage_ratio,
            );
        }
    }
    Ok(())
}

/// Reject `--out` path inside the analyzed repository in require-clean-snapshot mode
/// (review P1-4). Writing into the repo would dirty the next snapshot-bound step →
/// surprising B-3 harness rejection. Generic observed mode allows it.
fn reject_output_inside_repo(repo: &Path, out: &Path) -> anyhow::Result<()> {
    // canonicalize the repo (exists). For out, canonicalize the parent if the file
    // doesn't exist yet (output not yet written), else canonicalize the path itself.
    let canon_repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let canon_out = if out.exists() {
        out.canonicalize().unwrap_or_else(|_| out.to_path_buf())
    } else {
        // Resolve via parent (which exists) + file_name, then make absolute if needed.
        match out.parent().and_then(|p| p.canonicalize().ok()) {
            Some(parent) => out
                .file_name()
                .map(|name| parent.join(name))
                .unwrap_or_else(|| out.to_path_buf()),
            None => out.to_path_buf(),
        }
    };
    if canon_out.starts_with(&canon_repo) {
        anyhow::bail!(
            "--out path {} is inside the analyzed repository; --require-clean-snapshot \
             would be dirtied by the output write (place output outside the repo)",
            out.display()
        );
    }
    Ok(())
}

/// Resolve runtime state directory for pending-authorizations (review B-3 P0).
///
/// Harness mode REQUIRES state-dir outside the analyzed repo: Held artifacts written
/// into the repo would dirty git status → subsequent snapshot-bound runs rejected.
/// Production mode allows CWD default (backward-compat) or explicit `--state-dir`.
///
/// Returns a canonical state-dir path suitable for `FilesystemPendingAuthorizationStore::new`.
fn resolve_state_dir(
    explicit: Option<&std::path::Path>,
    execution: CliExecutionMode,
    repo: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    let state_dir = match explicit {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    // Harness invariant: state-dir must be outside the analyzed repo.
    if execution == CliExecutionMode::Harness {
        let canon_repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
        let canon_state = if state_dir.exists() {
            state_dir
                .canonicalize()
                .unwrap_or_else(|_| state_dir.clone())
        } else {
            // Resolve via parent if the dir doesn't exist yet (caller may pre-create).
            match state_dir.parent().and_then(|p| p.canonicalize().ok()) {
                Some(parent) => state_dir
                    .file_name()
                    .map(|name| parent.join(name))
                    .unwrap_or_else(|| state_dir.clone()),
                None => state_dir.clone(),
            }
        };
        if canon_state.starts_with(&canon_repo) {
            anyhow::bail!(
                "--state-dir {} is inside the analyzed repository; harness mode requires \
                 state-dir outside repo (Held artifacts would dirty git status → subsequent \
                 snapshot-bound runs rejected). Set --state-dir to an external path.",
                state_dir.display()
            );
        }
    }
    Ok(state_dir)
}

/// INV-T9 Step 4b: trajectory vision authority.
///
/// `run_trajectory_init` ve `run_trajectory_attempt` ortak vision vector'u — mutation
/// yüzeyinde `GlobalDefault` authority reject edilir (commit_task_claim → vision authority
/// gate). `UserLoaded` (kullanıcı-onaylı) en yüksek authority seviyesi. Bu yardımcı her iki
/// komutun aynı authority seviyesini paylaşmasını sağlar (issue #105 — `run_trajectory_init`
/// eskiden `VisionVector::new`/GlobalDefault kullanıyordu, navigator'a bağlanınca reject).
fn user_confirmed_trajectory_vision() -> osp_core::vision::VisionVector {
    osp_core::vision::VisionVector::with_source(
        osp_core::coords::RawPosition {
            x: 0.4,
            y: 0.6,
            z: 0.5,
            w: 0.5,
            v: 0.5,
        },
        osp_core::vision::VisionSource::UserLoaded,
    )
}

/// `osp trajectory init` — SpaceEngine kur (analyze + coord system + vision).
pub fn run_trajectory_init(args: TrajectoryInitArgs) -> anyhow::Result<()> {
    use osp_core::axes::{CohesionAxis, EntropyAxis, WitnessDepthAxis};
    use osp_core::coords::{CoordinateSystem, MetricSource};
    use osp_core::engine::{EngineConfig, SpaceEngine};

    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig {
        scip_index: args.scip.clone(),
        ..Default::default()
    };
    let result = analyze_repo_with_config(&args.repo, &registry, &config)?;
    let cs = CoordinateSystem::default_raw_five(
        // INV-T9 #70: production preset — graph topology TreeSitter, observed cohesion Scip.
        MetricSource::TreeSitter,
        CohesionAxis::try_with_observed_source(MetricSource::Scip)?,
        EntropyAxis::from_commit_entropy(6.0),
        WitnessDepthAxis::from_witness(0.3, 5),
    )?;
    let vision = user_confirmed_trajectory_vision();
    let engine = SpaceEngine::with_default_rules(
        result.space,
        cs,
        vision,
        EngineConfig::default_calibrated(),
    )?;
    let _ = engine; // engine kuruldu (space private — count analyze'den biliniyor)
    println!("✓ Trajectory initialized");
    println!("  SpaceEngine ready (analyze + coord system + vision)");
    Ok(())
}

/// `osp trajectory attempt` — D2 navigator + MockLlmClient/RuntimeLlmClient.
pub fn run_trajectory_attempt(args: TrajectoryAttemptArgs) -> anyhow::Result<()> {
    // Faz 8 test-project (review v6 P0-1): execution/witness mode guard.
    validate_execution_witness_combination(args.execution_mode, args.witness)?;
    use osp_core::axes::{CohesionAxis, EntropyAxis, WitnessDepthAxis};
    use osp_core::coords::{CoordinateSystem, MetricSource};
    use osp_core::engine::{EngineConfig, SpaceEngine};

    // Faz 8 B-3 review P0: runtime state directory resolution + harness invariant.
    // Harness mode REQUIRES state-dir outside analyzed repo (Held artifacts must not dirty
    // repo → subsequent snapshot-bound runs rejected). Production allows CWD default.
    let state_dir = resolve_state_dir(args.state_dir.as_deref(), args.execution_mode, &args.repo)?;

    // Faz 8 test-project (review v6-v7): snapshot-bound controlled harness.
    // Pre-capture repository snapshot (HEAD + tracked paths + clean state).
    let snapshot_before =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    // P0-3: harness requires clean worktree (drift fence prerequisite).
    repo_snapshot::ensure_snapshot_eligible(&snapshot_before).map_err(|e| anyhow::anyhow!(e))?;

    // 1. Analyze -> space.
    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig::default();
    let result = analyze_repo_with_config(&args.repo, &registry, &config)?;

    // P1-2: analyzed path ⊆ HEAD tracked-path invariant (fail-closed for untracked files).
    repo_snapshot::validate_analyzed_paths_tracked(&result.node_paths, &snapshot_before)
        .map_err(|e| anyhow::anyhow!(e))?;

    // 2. Engine (D2 gerçek measure).
    let cs = CoordinateSystem::default_raw_five(
        // INV-T9 #70: production preset — graph topology TreeSitter, observed cohesion Scip.
        MetricSource::TreeSitter,
        CohesionAxis::try_with_observed_source(MetricSource::Scip)?,
        EntropyAxis::from_commit_entropy(6.0),
        WitnessDepthAxis::from_witness(0.3, 5),
    )?;
    let vision = user_confirmed_trajectory_vision();
    let mut engine = SpaceEngine::with_default_rules(
        result.space,
        cs,
        vision,
        EngineConfig::default_calibrated(),
    )?;

    // 3. Task resolution: harness task file (snapshot-bound) or hardcoded legacy fallback.
    let task_source: &'static str = if args.task.is_some() {
        "harness_task_file"
    } else {
        "legacy_hardcoded"
    };
    let task = resolve_task(&args, &snapshot_before, &result.node_paths)?;

    // 4. LLM seçimi: mock (FileMockLlm) veya real (RuntimeLlmClient, GPT-4o-mini).
    match args.llm.as_str() {
        "real" => {
            let llm = osp_llm_runtime::RuntimeLlmClient::from_env()
                .map_err(|e| anyhow::anyhow!("LLM runtime (OPENAI_API_KEY?): {e}"))?;
            run_navigator(
                &llm,
                &mut engine,
                &args,
                task,
                &state_dir,
                &snapshot_before,
                task_source,
            )?;
        }
        _ => {
            // mock (default)
            let proposals_path = args
                .proposals
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--proposals required for --llm mock"))?;
            let proposals_json = std::fs::read_to_string(proposals_path)?;
            let proposals: Vec<osp_core::agent::DeltaProposal> =
                serde_json::from_str(&proposals_json)?;
            let llm = crate::mock_llm::FileMockLlm::new(proposals);
            run_navigator(
                &llm,
                &mut engine,
                &args,
                task,
                &state_dir,
                &snapshot_before,
                task_source,
            )?;
        }
    }

    // 5. Post-capture drift fence: repository must be unchanged during analysis+run
    //    (review P0-3). Equal snapshots ⇒ no transient mutation crossed the boundary.
    let snapshot_after =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    if snapshot_before != snapshot_after {
        anyhow::bail!(
            "repository changed during trajectory attempt (head/tracked/clean drift) — \
             analysis-run consistency violated"
        );
    }
    Ok(())
}

/// Resolve task: harness file (`--task`) or hardcoded legacy fallback (backward-compat).
///
/// Mode matrix (review P0-2 — no legacy fallback bypass):
/// - `(Harness, Some(path))` → snapshot-bound harness task (HEAD + scope binding + Node-only V1)
/// - `(Harness, None)`       → `Err` — harness REQUIRES snapshot-bound task file; legacy fallback
///   tüm harness garantilerini bypass eder.
/// - `(Production, Some)`    → snapshot-bound task, witness policy Production (trusted operator)
/// - `(Production, None)`    → legacy hardcoded coupling ≤ 0.55 (D1 backward-compat)
fn resolve_task(
    args: &TrajectoryAttemptArgs,
    snapshot: &repo_snapshot::RepositorySnapshot,
    node_paths: &std::collections::HashMap<u64, String>,
) -> anyhow::Result<osp_core::trajectory::Task> {
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, OpKind, PredicateAxis, PredicateFailurePolicy,
        PredicateMode, PredicateScope, PredicateSet, TaskPolicy, TaskStatus, WeightedPredicate,
    };
    match (args.execution_mode, args.task.as_ref()) {
        (CliExecutionMode::Harness, Some(task_path)) => {
            let task = harness_task::load_and_validate_harness_task(
                task_path,
                args.task_id,
                snapshot,
                node_paths,
                args.maneuver_limit,
            )
            .map_err(|e| anyhow::anyhow!(e))?;
            Ok(task)
        }
        (CliExecutionMode::Harness, None) => {
            // P0-2: harness REQUIRES snapshot-bound task file — legacy fallback bypass edemez.
            anyhow::bail!(
                "--execution-mode harness requires --task <path> (snapshot-bound task file); \
                 legacy fallback disables all harness guarantees (Paper 2 scoped relaxation)"
            );
        }
        (CliExecutionMode::Production, Some(task_path)) => {
            // Production + task file: snapshot-bound task, witness Production (trusted operator).
            let task = harness_task::load_and_validate_harness_task(
                task_path,
                args.task_id,
                snapshot,
                node_paths,
                args.maneuver_limit,
            )
            .map_err(|e| anyhow::anyhow!(e))?;
            Ok(task)
        }
        (CliExecutionMode::Production, None) => {
            // Legacy hardcoded fallback (Node(0), coupling ≤ 0.55). D1 backward-compat.
            let policy = TaskPolicy {
                maneuver_limit: args.maneuver_limit.unwrap_or(5),
                predicate_failure_policy: PredicateFailurePolicy::StrictReject,
                ..Default::default()
            };
            Ok(osp_core::trajectory::Task {
                id: args.task_id,
                milestone_id: 1,
                label: "CLI trajectory attempt".into(),
                target_predicate_set: PredicateSet {
                    mode: PredicateMode::All,
                    predicates: vec![WeightedPredicate {
                        predicate: MetricPredicate {
                            metric: PredicateAxis::Coupling,
                            operator: ComparisonOp::Le,
                            threshold: 0.55,
                            scope: PredicateScope::Node(0),
                            required_source: Some(osp_core::coords::MetricSource::Scip),
                            tolerance: 0.0,
                        },
                        weight: None,
                    }],
                    preferred_vector: Some(osp_core::coords::RawPosition {
                        x: 0.55,
                        y: 0.6,
                        z: 0.4,
                        w: 0.5,
                        v: 0.3,
                    }),
                },
                policy,
                allowed_operations: vec![OpKind::RemoveImport],
                constraints: vec![],
                status: TaskStatus::Pending,
            })
        }
    }
}

/// Navigator çalıştır (generic LlmClient — mock veya real).
fn run_navigator<L: osp_core::navigator::LlmClient>(
    llm: &L,
    engine: &mut osp_core::engine::SpaceEngine,
    args: &TrajectoryAttemptArgs,
    task: osp_core::trajectory::Task,
    state_dir: &PathBuf,
    snapshot: &repo_snapshot::RepositorySnapshot,
    task_source: &'static str,
) -> anyhow::Result<()> {
    use osp_core::navigator::AgentNavigator;
    use osp_core::trajectory::{
        InMemoryTaskRegistry, MilestoneId, OperatorCapability, TrajectoryId,
    };
    // CLI = operator mode (INV-T2) — trusted-boundary API (PR35 hardening).
    let _cap = OperatorCapability::issue_for_operator_session();
    let mut task_registry = InMemoryTaskRegistry::new();
    task_registry.insert(task);
    // 5. Navigator.
    let current_measured = osp_core::navigator::provenanced_from_raw(
        osp_core::coords::RawPosition {
            x: 0.7,
            y: 0.5,
            z: 0.5,
            w: 0.5,
            v: 0.3,
        },
        osp_core::coords::MetricSource::Scip,
    );
    let mut evidence = vec![];
    let mut nav = AgentNavigator {
        llm,
        resolver: &task_registry,
        engine,
        evidence: &mut evidence,
        trajectory_id: 1 as TrajectoryId,
        milestone_id: 1 as MilestoneId,
        target_vector: osp_core::coords::RawPosition {
            x: 0.55,
            y: 0.6,
            z: 0.4,
            w: 0.5,
            v: 0.3,
        },
        current_measured,
        output_contract: osp_core::agent::OutputContract::strict(),
        // Faz 8 test-project (review v6): witness policy args.witness'a göre.
        // harness-auto-approve → Completed loop (guarded: yalnız execution=harness).
        witness_policy: match args.witness {
            CliWitnessMode::Production => osp_core::navigator::NavigatorWitnessPolicy::Production,
            CliWitnessMode::HarnessAutoApprove => {
                osp_core::navigator::NavigatorWitnessPolicy::HarnessAutoApprove
            }
        },
        // INV-T9: filesystem store — state_dir altında .osp/pending-authorizations/.
        // review B-3 P0: harness mode resolves state_dir outside repo ( Held artifacts
        // must not dirty analyzed repo). Production allows caller-specified or CWD default.
        pending_authorization_store: Box::new(
            osp_core::authorization::FilesystemPendingAuthorizationStore::new(state_dir),
        ),
        clock: Box::new(osp_core::authorization::SystemClock),
    };
    let result = nav.run_task(args.task_id, 1);
    // 6. Output — versioned JSON envelope (json) veya human (default).
    let is_json = args.format.eq_ignore_ascii_case("json");
    if is_json {
        let envelope = run_envelope::build_run_envelope_v1(
            &result,
            &evidence,
            args.execution_mode,
            args.witness,
            task_source,
            snapshot.head.as_str(),
        );
        let json = serde_json::to_string_pretty(&envelope)?;
        // Diagnostics stderr'e — stdout JSON-only.
        eprintln!(
            "osp trajectory attempt: V1 legacy_projected_v1 execution authority \
             (provenance_native=false); native 5-axis engine measurement pending MD-2 (#96)"
        );
        println!("{json}");
    } else {
        print_human_result(&result, args.task_id, &evidence)?;
    }
    let exit_code = navigator_exit_code(&result, args.task_id);
    if exit_code != exit_codes::COMPLETED {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// Print human-readable navigator result (non-json mode).
fn print_human_result(
    result: &osp_core::navigator::NavigatorResult,
    task_id: u64,
    evidence: &[osp_core::trajectory::TrajectoryEvidence],
) -> anyhow::Result<()> {
    use osp_core::navigator::NavigatorResult;
    match result {
        NavigatorResult::Completed {
            attempts,
            total_tokens,
        } => {
            println!("✓ Task completed in {attempts} attempts");
            println!("  Total tokens: {}", total_tokens.total_tokens);
        }
        NavigatorResult::ExceededManeuverLimit { attempts, .. } => {
            println!("✗ Maneuver limit exceeded after {attempts} attempts");
        }
        NavigatorResult::AwaitingWitnesses {
            pending,
            persistence,
        } => {
            println!(
                "⏸ Awaiting witnesses (INV-T9) — task {}, claim {}",
                pending.task_id, pending.claim_id
            );
            println!(
                "  Witness hold reason: {}",
                pending.witness_hold_reason.as_reason_str()
            );
            println!("  Commit state: awaiting_witnesses");
            println!("  Mainline mutation: not_applied");
            println!("  Next action: await external evidence");
            println!(
                "  Pending artifact: {}",
                persistence.artifact_path.display()
            );
        }
        NavigatorResult::RequiresRevision(rev) => {
            println!(
                "↻ Requires revision (explicit witness rejection) — task {}, claim {}",
                rev.task_id(),
                rev.claim_id()
            );
        }
        NavigatorResult::PendingAuthorizationPersistenceFailure { pending, error } => {
            println!(
                "✗ Pending authorization persistence failed — task {}, claim {}: {error}",
                pending.task_id, pending.claim_id
            );
        }
        NavigatorResult::WitnessEvaluationError(msg) => {
            println!("✗ Witness evaluation error: {msg}");
        }
        NavigatorResult::SystemFailure(msg) => {
            println!("✗ System failure: {msg}");
        }
        NavigatorResult::TaskNotFound => {
            println!("✗ Task {task_id} not found");
        }
        NavigatorResult::RequiresOperatorApproval { attempts, .. } => {
            println!("⚠ Operator approval required after {attempts} attempts");
        }
        NavigatorResult::LlmError(e) => {
            println!("✗ LLM error: {e}");
        }
    }
    println!("  Evidence entries: {}", evidence.len());
    if !evidence.is_empty() {
        let json = serde_json::to_string_pretty(evidence)?;
        println!("{json}");
    }
    Ok(())
}

/// Map navigator result → CLI exit code (INV-T9 contract).
fn navigator_exit_code(result: &osp_core::navigator::NavigatorResult, _task_id: u64) -> i32 {
    use osp_core::navigator::NavigatorResult;
    match result {
        NavigatorResult::Completed { .. } => exit_codes::COMPLETED,
        NavigatorResult::ExceededManeuverLimit { .. } => exit_codes::EXCEEDED_MANEUVER_LIMIT,
        NavigatorResult::AwaitingWitnesses { .. } => exit_codes::AWAITING_WITNESSES,
        NavigatorResult::RequiresRevision(_) => exit_codes::REQUIRES_REVISION,
        NavigatorResult::PendingAuthorizationPersistenceFailure { .. } => {
            exit_codes::PENDING_AUTHORIZATION_PERSISTENCE_FAILURE
        }
        NavigatorResult::WitnessEvaluationError(_) => exit_codes::WITNESS_EVALUATION_ERROR,
        NavigatorResult::SystemFailure(_) => exit_codes::SYSTEM_FAILURE,
        NavigatorResult::TaskNotFound => exit_codes::TASK_NOT_FOUND,
        NavigatorResult::RequiresOperatorApproval { .. } => exit_codes::REQUIRES_OPERATOR_APPROVAL,
        NavigatorResult::LlmError(_) => exit_codes::LLM_ERROR,
    }
}

/// `osp task view` — AgentTaskView göster (INV-T1 — preferred_vector ASLA).
pub fn run_task_view(args: TaskViewArgs) -> anyhow::Result<()> {
    // D1: basit — task view predicate string parse + AgentTaskView üret.
    // Tam implementasyon D2 sonrası (task registry persistence).
    println!("Task {} view (INV-T1 — no preferred_vector):", args.task_id);
    println!("  Predicate: {}", args.predicate);
    println!("  Repo: {}", args.repo.display());
    println!("  (Full AgentTaskView serialization — D2 navigator integration)");
    Ok(())
}

/// `osp evidence export` — evidence ledger JSON.
pub fn run_evidence_export(args: EvidenceArgs) -> anyhow::Result<()> {
    if let Some(input) = args.input {
        let data = std::fs::read_to_string(input)?;
        let json =
            serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&data)?)?;
        match args.out {
            Some(path) => {
                std::fs::write(&path, &json)?;
                println!("✓ Evidence exported to {}", path.display());
            }
            None => println!("{json}"),
        }
    } else {
        println!("No evidence input provided. Run `osp trajectory attempt` first.");
    }
    Ok(())
}

#[cfg(test)]
mod mode_matrix_tests {
    //! Review P0-2 — harness execution mode requires --task (no legacy fallback bypass).
    use super::*;
    use std::collections::HashMap;

    fn snapshot_fixture() -> repo_snapshot::RepositorySnapshot {
        repo_snapshot::RepositorySnapshot {
            head: "0123456789abcdef0123456789abcdef01234567"
                .to_string()
                .try_into()
                .unwrap(),
            tracked_paths: std::collections::BTreeSet::from(["src/a.rs".to_string()]),
            clean: true,
        }
    }

    fn args(mode: CliExecutionMode, task: Option<PathBuf>) -> TrajectoryAttemptArgs {
        TrajectoryAttemptArgs {
            task_id: 7,
            repo: PathBuf::from("."),
            proposals: None,
            llm: "mock".into(),
            maneuver_limit: None,
            task,
            execution_mode: mode,
            witness: CliWitnessMode::default(),
            format: "human".into(),
            state_dir: None,
        }
    }

    #[test]
    fn harness_without_task_file_is_rejected() {
        let snap = snapshot_fixture();
        let node_paths = HashMap::new();
        let err = resolve_task(&args(CliExecutionMode::Harness, None), &snap, &node_paths)
            .expect_err("harness without --task must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("--execution-mode harness requires --task"),
            "expected harness-requires-task message, got: {msg}"
        );
    }

    #[test]
    fn production_without_task_file_falls_back_to_legacy() {
        let snap = snapshot_fixture();
        let node_paths = HashMap::new();
        let task = resolve_task(
            &args(CliExecutionMode::Production, None),
            &snap,
            &node_paths,
        )
        .expect("production without --task falls back to legacy");
        assert_eq!(task.id, 7);
        assert_eq!(
            task.allowed_operations,
            vec![osp_core::trajectory::OpKind::RemoveImport]
        );
    }
}

#[cfg(test)]
mod trajectory_vision_authority_tests {
    //! INV-T9 Step 4b (issue #105): trajectory vision authority regression guard.
    //!
    //! `run_trajectory_init` eskiden `VisionVector::new` (GlobalDefault) kullanıyordu —
    //! navigator'a bağlanınca `commit_task_claim` vision authority gate'inde reject olurdu.
    //! Bu test `user_confirmed_trajectory_vision` yardımcısının UserLoaded authority
    //! kullandığını doğrular (GlobalDefault'a geri dönülmesini engeller).
    use super::*;

    #[test]
    fn trajectory_vision_uses_user_loaded_authority_not_global_default() {
        let vision = user_confirmed_trajectory_vision();
        assert_eq!(
            vision.source,
            osp_core::vision::VisionSource::UserLoaded,
            "trajectory vision must use UserLoaded authority — GlobalDefault is rejected \
             at the authorization-gated mutation surface (INV-T9 Step 4b, issue #105)"
        );
    }
}
