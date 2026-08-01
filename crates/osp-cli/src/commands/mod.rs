//! OSP CLI komut handler'ları. osp-core API'sini çağırır — CLI = truth surface.
//!
//! Pattern: osp-desktop cmd_simulate_claim (lib.rs:257-278) reuse —
//! analyze_repo_with_config → CoordinateSystem::default_raw_five → SpaceEngine.

pub mod harness_task;
pub mod repo_snapshot;

use std::path::PathBuf;

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

/// `osp analyze` — repo analiz → space snapshot JSON.
pub fn run_analyze(args: AnalyzeArgs) -> anyhow::Result<()> {
    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig {
        scip_index: args.scip.clone(),
        ..Default::default()
    };
    let result = analyze_repo_with_config(&args.repo, &registry, &config)?;
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "node_count": result.space.nodes.len(),
        "edge_count": result.space.edges.len(),
        "module_metrics_count": result.module_metrics.len(),
        "repo_metrics": {
            "abstractness": result.repo_metrics.abstractness.value,
            "main_sequence_distance": result.repo_metrics.main_sequence_distance.value,
        },
        "semantic_coverage": {
            "files_total": result.semantic_coverage.files_total,
            "files_with_scip": result.semantic_coverage.files_with_scip,
            "coverage_ratio": result.semantic_coverage.coverage_ratio,
        },
    }))?;
    match args.out {
        Some(path) => {
            std::fs::write(&path, &json)?;
            println!("✓ Space snapshot written to {}", path.display());
        }
        None => println!("{json}"),
    }
    Ok(())
}

/// `osp trajectory init` — SpaceEngine kur (analyze + coord system + vision).
pub fn run_trajectory_init(args: TrajectoryInitArgs) -> anyhow::Result<()> {
    use osp_core::axes::{CohesionAxis, EntropyAxis, WitnessDepthAxis};
    use osp_core::coords::{CoordinateSystem, MetricSource};
    use osp_core::engine::{EngineConfig, SpaceEngine};
    use osp_core::vision::VisionVector;

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
    let vision = VisionVector::new(osp_core::coords::RawPosition {
        x: 0.4,
        y: 0.6,
        z: 0.5,
        w: 0.5,
        v: 0.5,
    });
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
    use osp_core::vision::VisionVector;

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
    let vision = VisionVector::new(osp_core::coords::RawPosition {
        x: 0.4,
        y: 0.6,
        z: 0.5,
        w: 0.5,
        v: 0.5,
    });
    let mut engine = SpaceEngine::with_default_rules(
        result.space,
        cs,
        vision,
        EngineConfig::default_calibrated(),
    )?;

    // 3. Task resolution: harness task file (snapshot-bound) or hardcoded legacy fallback.
    let task = resolve_task(&args, &snapshot_before, &result.node_paths)?;

    // 4. LLM seçimi: mock (FileMockLlm) veya real (RuntimeLlmClient, GPT-4o-mini).
    match args.llm.as_str() {
        "real" => {
            let llm = osp_llm_runtime::RuntimeLlmClient::from_env()
                .map_err(|e| anyhow::anyhow!("LLM runtime (OPENAI_API_KEY?): {e}"))?;
            run_navigator(&llm, &mut engine, &args, task)?;
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
            run_navigator(&llm, &mut engine, &args, task)?;
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
///                              tüm harness garantilerini bypass eder.
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
            let mut policy = TaskPolicy::default();
            policy.maneuver_limit = args.maneuver_limit.unwrap_or(5);
            policy.predicate_failure_policy = PredicateFailurePolicy::StrictReject;
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
) -> anyhow::Result<()> {
    use osp_core::navigator::{AgentNavigator, NavigatorResult};
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
        // INV-T9: production filesystem store — cwd altında .osp/pending-authorizations/.
        pending_authorization_store: Box::new(
            osp_core::authorization::FilesystemPendingAuthorizationStore::new("."),
        ),
        clock: Box::new(osp_core::authorization::SystemClock),
    };
    let result = nav.run_task(args.task_id, 1);
    // 6. Sonuç yazdır + exit code.
    let exit_code = match result {
        NavigatorResult::Completed {
            attempts,
            total_tokens,
        } => {
            println!("✓ Task completed in {attempts} attempts");
            println!("  Total tokens: {}", total_tokens.total_tokens);
            exit_codes::COMPLETED
        }
        NavigatorResult::ExceededManeuverLimit { attempts, .. } => {
            println!("✗ Maneuver limit exceeded after {attempts} attempts");
            exit_codes::EXCEEDED_MANEUVER_LIMIT
        }
        NavigatorResult::AwaitingWitnesses {
            pending,
            persistence,
        } => {
            // **INV-T9** — expected authorization bekleme. Domain outcome, hata DEĞİL.
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
            exit_codes::AWAITING_WITNESSES
        }
        NavigatorResult::RequiresRevision(rev) => {
            println!(
                "↻ Requires revision (explicit witness rejection) — task {}, claim {}",
                rev.task_id(),
                rev.claim_id()
            );
            exit_codes::REQUIRES_REVISION
        }
        NavigatorResult::PendingAuthorizationPersistenceFailure { pending, error } => {
            println!(
                "✗ Pending authorization persistence failed — task {}, claim {}: {error}",
                pending.task_id, pending.claim_id
            );
            exit_codes::PENDING_AUTHORIZATION_PERSISTENCE_FAILURE
        }
        NavigatorResult::WitnessEvaluationError(msg) => {
            println!("✗ Witness evaluation error: {msg}");
            exit_codes::WITNESS_EVALUATION_ERROR
        }
        NavigatorResult::SystemFailure(msg) => {
            println!("✗ System failure: {msg}");
            exit_codes::SYSTEM_FAILURE
        }
        NavigatorResult::TaskNotFound => {
            println!("✗ Task {} not found", args.task_id);
            exit_codes::TASK_NOT_FOUND
        }
        NavigatorResult::RequiresOperatorApproval { attempts, .. } => {
            println!("⚠ Operator approval required after {attempts} attempts");
            exit_codes::REQUIRES_OPERATOR_APPROVAL
        }
        NavigatorResult::LlmError(e) => {
            println!("✗ LLM error: {e}");
            exit_codes::LLM_ERROR
        }
    };
    println!("  Evidence entries: {}", evidence.len());
    if !evidence.is_empty() {
        let json = serde_json::to_string_pretty(&evidence)?;
        println!("{json}");
    }
    if exit_code != exit_codes::COMPLETED {
        std::process::exit(exit_code);
    }
    Ok(())
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
        let task =
            resolve_task(&args(CliExecutionMode::Production, None), &snap, &node_paths)
                .expect("production without --task falls back to legacy");
        assert_eq!(task.id, 7);
        assert_eq!(
            task.allowed_operations,
            vec![osp_core::trajectory::OpKind::RemoveImport]
        );
    }
}
