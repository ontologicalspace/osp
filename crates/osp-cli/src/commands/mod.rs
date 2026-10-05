//! OSP CLI komut handler'ları. osp-core API'sini çağırır — CLI = truth surface.
//!
//! Pattern: osp-desktop cmd_simulate_claim (lib.rs:257-278) reuse —
//! analyze_repo_with_config → CoordinateSystem::default_raw_five → SpaceEngine.

pub mod analyze_provenance;
pub mod baseline;
pub mod draft_task;
pub mod finalize_run;
pub mod harness_task;
pub mod path_bindings;
pub mod path_keyed_proposals;
pub mod repo_snapshot;
pub mod run_envelope;
pub mod suggest_targets;

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
    /// **#97 MD-3:** cold-start operatör onayı (INV-T9 extension — witness'den
    /// ayrı otorite; expected domain outcome, hata DEĞİL).
    pub const AWAITING_COLD_START_APPROVAL: i32 = 14;
    /// **#164:** Resume reddedildi — artifact bu uzayda artık uygulanabilir değil
    /// (stale base revision: uzay askıdan beri değişti → remeasure / yeni attempt;
    /// veya artifact zaten uygulanmış — receipt var). Expected domain outcome.
    pub const RESUME_REFUSED_BASE: i32 = 15;
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
pub mod resume_envelope;
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
    /// #166: Kanonik attempt artifact'ın çağırıcıya ait kopyası (şemalı run
    /// envelope v1; atomic temp+rename yazım). Canonical KALICI kayıt her modda
    /// state-dir'e yazılır: `attempts/task-<task_id>-<unix_millis>-<pid>[-N].json`
    /// (no-clobber; final snapshot fence GEÇTİKTEN SONRA publish edilir — sıra:
    /// navigator → fence → canonical publish → emit → exit). `--out` analyzed
    /// repo'yu ve state-dir'in `.osp/` + `attempts/` canonical alanlarını
    /// hedefleyemez (preflight'te reddedilir).
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Runtime state directory (`.osp/` artifacts: pending-authorizations +
    /// persisted space identity). Default = CWD.
    /// Invariant (#152 R1 P1-2, her iki execution mode): mutlaka analyzed repo
    /// DIŞINDA — identity + Held artifacts repoya yazılırsa git status kirlenir
    /// → sonraki snapshot-bound run'lar reddedilir. Relative + henüz var olmayan
    /// path'ler dahil her durumda çözümlenir ve fence uygulanır.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
}

/// **#164 R2 P0:** Kanıt güven sınırı modu — resume'un kanıt kaynağına dair AÇIK
/// beyan. Zorunlu (default YOK): raw JSON'un sessizce production authorization
/// sağlaması engellenir; operator kefaleti kalıcı receipt'e yazılır.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum CliEvidenceTrustMode {
    /// Operator, kanıt dosyasındaki olayların gerçekliğine (actor kimliğinin
    /// doğru olduğu, olay tipinin/kaynağın gerçeği yansıttığı) kefildir —
    /// OSP dış doğrulama YAPMAZ (Paper 1 A1 provider-doğrulaması gelecek
    /// yüzey). Wire yalnız yapısal kusurları reddeder; `witness_kind`'ın
    /// ağırlığı belirlediği DİKKATE ALINIR — kefalet dolaylı ağırlık
    /// seçimini de kapsar. Beyan receipt'e `operator_asserted` olarak yazılır.
    OperatorAsserted,
}

impl CliEvidenceTrustMode {
    /// Core wire değerine projekte.
    pub fn to_core(self) -> osp_core::authorization::EvidenceTrustMode {
        match self {
            Self::OperatorAsserted => osp_core::authorization::EvidenceTrustMode::OperatorAsserted,
        }
    }
}

/// `osp trajectory resume <artifact>` — #164: askılı (exit 10 / AwaitingWitnesses)
/// görevin kalıcı pending-authorization artifact'ından sürdürülmesi.
///
/// LLM'siz operatör akışı — karar zinciri attempt anında koşuldu ve artifact'a
/// bağlandı; resume yalnız fence teyidi (identity + staleness) + witness kanıtı
/// değerlendirmesi + kayıtlı delta'nın uygulanmasını yapar. Quorum parametreleri
/// artifact'tan gelir (operator düşüremez).
#[derive(Args, Debug)]
pub struct TrajectoryResumeArgs {
    /// Pending authorization artifact path'i (`<state-dir>/.osp/pending-authorizations/…`).
    pub artifact: PathBuf,
    #[arg(long)]
    pub repo: PathBuf,
    /// Witness kanıt dosyası — `EvidenceEvent` JSON dizisi (strict wire):
    /// `[{ "id", "source", "witness_kind", "actor", "claim" }, …]`.
    /// Ağırlık `witness_kind`'ın kalibre değerinden türetilir — ancak `witness_kind`
    /// caller tarafından SEÇİLDİĞİ için ağırlık dolaylı olarak seçilebilir; bu
    /// yüzden dosyanın gerçekliği `--evidence-trust` beyanına tabidir (OSP olayın
    /// gerçekle uyumunu DOĞRULAMAZ). Her olayın `claim`'i artifact'ın claim'idine
    /// bağlı olmalı. Kusurlu kanıt operational fault ile reddedilir (INV-T9):
    /// author'un kendi kanıtı, duplicate olay/actor/id → exit 20.
    /// KANIT BİRİKMESİ YOKTUR — her resume YALNIZ dosyadaki olayları değerlendirir;
    /// önceki resume'un kanıtı eklenmez (tam set yeniden verilir).
    #[arg(long)]
    pub witness_evidence: PathBuf,
    /// Kanıt güven sınırı beyanı (ZORUNLU — varsayılan yok). `operator-asserted`:
    /// dosyadaki olayların gerçekliğine operator kefildir; beyan receipt'e kalıcı
    /// yazılır. OSP-verified kanıt yüzeyi henüz yok (Paper 1 A1 follow-up).
    #[arg(long, value_enum)]
    pub evidence_trust: CliEvidenceTrustMode,
    /// Runtime state directory — attempt ile AYNI kök olmalı (identity + artifact
    /// orada yaşar). Farklı state-dir → farklı identity → fail-closed red.
    /// Attempt ile aynı fence ailesi: analyzed repo DIŞINDA.
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
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
    // #182: parent-dizin preflight her iki binding modunda (generic mod da analizi kaybeder).
    if let Some(out) = args.out.as_ref() {
        preflight_out_parent(out, "--out")?;
        if args.require_clean_snapshot {
            reject_output_inside_repo(&args.repo, out)?;
        }
        // #181: eligibility pre-check — dirty worktree'de TAM analizi boşa koşturma
        // (run-17 sürtünmesi: red, ~67 s'lik analizden sonra geliyordu). Post-analyze
        // kontrolü aşağıda TOCTOU drift-fence olarak aynen kalır (analiz SIRASINDA
        // kirlenen temiz ağaç yakalanmaya devam eder); bu erken kontrol onun yerini
        // almaz, önden eklenir. Mesajdaki "(rejected before analysis …)" işareti,
        // redin analizden önce geldiğinin dışarıdan gözlemlenebilir kanıtıdır.
        if let Err(e) = repo_snapshot::ensure_snapshot_eligible(&snapshot_before) {
            anyhow::bail!("{e} (rejected before analysis — eligibility pre-check, #181)");
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
    // #167: schema_version 1 → 2 — edge-kind kümesi genişledi (type_imports,
    // same_ns_type) ve node'a opsiyonel coupling_type eklendi; "edges yalnız
    // imports kinds" varsayımı yapan tüketiciler fail-visible uyarılmalı.
    let envelope = serde_json::json!({
        "schema_version": 2,
        "analysis": {
            "metric_representation": "axis_provenanced_v1",
            "provenance_model": "analyzer_axis_specific",
            "axis_specific_provenance": true
        },
        "repository": {
            "head": snapshot_after.head.as_str(),
            "clean": snapshot_after.clean(),
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

/// #182: `--out` parent-dizin preflight — yazım hatası en pahalı adımdan SONRA
/// çıkmasın (run-17 sürtünmesi: `analyze --out <yok-dizin>` tam analizden sonra
/// `os error 3` ile patladı, ~76 s kayıp). Paylaşımlı yardımcı; analyze /
/// trajectory attempt / draft-task / finalize-run taşıyıcıları uygular.
/// Preflight-red tercih edildi (mkdir -p sessiz yan-etki yaratırdı); hata
/// mesajı mkdir önerisi taşır.
pub(crate) fn preflight_out_parent(out: &Path, flag: &str) -> anyhow::Result<()> {
    if let Some(parent) = out.parent() {
        if parent.as_os_str().is_empty() {
            return Ok(());
        }
        if !parent.exists() {
            anyhow::bail!(
                "{flag} parent directory does not exist: {} — create it first \
                 (e.g. mkdir -p); refusing before expensive work",
                parent.display()
            );
        }
        // Review P2 (#185): parent VAR ama normal dosya ise exists() geçer, pahalı iş
        // sonrasında "Not a directory" sınıfı hata gelir — burada erken red.
        // is_dir hedefi izler: symlink→dizin GEÇER, symlink→dosya REDDEDİLİR.
        if !parent.is_dir() {
            anyhow::bail!(
                "{flag} parent path exists but is not a directory: {} — choose a \
                 directory parent for the output",
                parent.display()
            );
        }
    }
    Ok(())
}

/// Resolve runtime state directory for pending-authorizations (review B-3 P0).
///
/// **#152 R1 P1-2:** state-dir (default CWD, explicit dahil) analyzed repo
/// İÇİNDE olamaz — HER İKİ execution mode'da. Identity her attempt başında
/// `<state-dir>/.osp/space-identity`'ye yazılıyor; production default CWD repo
/// kökü olduğunda dosya repoya düşer ve `.osp/` gitignore'da YOK → "repo stays
/// clean" iddiası default production yolunda kırılırdı.
///
/// **#152 R2 P1 (canonicalization):** relative + henüz VAR OLMAYAN path'ler
/// (`--state-dir state/nested`) eskiden parent canonicalize başarısız olduğunda
/// raw relative kalıp absolute repo ile karşılaştırılıyordu → fence bypass.
/// Artık: CWD'ye göre mutlaklaştır → var olan en derin atayı canonicalize et,
/// var olmayan kuyruğu koru → `..`/`.` lexically normalize → karşılaştırma
/// daima mutlak ↔ mutlak. CWD edinilemezse fail-closed red (doğrulanamayan
/// state-dir repoya yazabilir).
///
/// Returns the caller-facing state-dir path (unchanged form) for
/// `FilesystemPendingAuthorizationStore::new`.
fn resolve_state_dir(
    explicit: Option<&std::path::Path>,
    _execution: CliExecutionMode,
    repo: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    let state_dir = match explicit {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    // R2 P1: mutlaklaştır — fence karşılaştırması daima mutlak↔mutlak.
    let abs_state = if state_dir.is_absolute() {
        state_dir.clone()
    } else {
        let cwd = std::env::current_dir().map_err(|e| {
            anyhow::anyhow!(
                "cannot resolve relative --state-dir {}: current dir unavailable: {e}",
                state_dir.display()
            )
        })?;
        cwd.join(&state_dir)
    };
    let canon_repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let canon_state = canonicalize_with_missing_tail(&abs_state);
    if canon_state.starts_with(&canon_repo) {
        anyhow::bail!(
            "--state-dir {} is inside the analyzed repository; state-dir must be outside \
             repo in every execution mode (persisted space identity + Held artifacts would \
             dirty git status → subsequent snapshot-bound runs rejected). Set --state-dir \
             to an external path.",
            state_dir.display()
        );
    }
    Ok(state_dir)
}

/// #166 review tur-2 (P0 + P1-1): `--out` hedefi preflight'te doğrulanır —
/// navigator ÇALIŞMADAN önce. İki bütünlük koruması:
///
/// 1. **Analyzed repo dışı:** `--out` repo içine yazarsa final snapshot fence'i
///    GEÇTİKTEN SONRA analyzed source'u değiştirebilir (fence'ten kaçış). State-dir
///    invariant'ı ve `reject_output_inside_repo` precedent'iyle aynı ilke.
/// 2. **Canonical state mağazaları dokunulmaz:** `<state-dir>/.osp/**` (space
///    identity + pending-authorizations) ve `<state-dir>/attempts/**` (no-clobber
///    canonical evidence store) hedeflenemez — `--out` kopyası `rename` ile REPLACE
///    eder; canonical store'un immutability'si `--out` arka kapısıyla kırılamaz.
///    State-dir KÖKÜNDEK caller-owned dosyalara (ör. attempt-out.json) izin verilir.
///
/// Karşılaştırma `canonicalize_with_missing_tail` ile (symlink/relative/`..`
/// kaçışlarına karşı component-wise; string-match değil).
fn validate_attempt_output_path(
    repo: &std::path::Path,
    state_dir: &std::path::Path,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    // #182: navigator ÇALIŞMADAN önce parent-dizin preflight (en pahalı adım öncesi).
    preflight_out_parent(out, "--out")?;

    // #166 review tur-3 (P2): CWD çözülemiyorsa fail-closed — relative path fence
    // doğrulaması bilinmeyen bir tabana düşmemeli (resolve_state_dir ile tutarlı).
    let cwd = |what: &str| -> anyhow::Result<PathBuf> {
        std::env::current_dir().map_err(|e| {
            anyhow::anyhow!(
                "cannot resolve relative {what} {}: current dir unavailable: {e}",
                out.display()
            )
        })
    };
    let abs_out = if out.is_absolute() {
        out.to_path_buf()
    } else {
        cwd("--out")?.join(out)
    };
    let canon_out = canonicalize_with_missing_tail(&abs_out);

    let canon_repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    if canon_out.starts_with(&canon_repo) {
        anyhow::bail!(
            "--out {} resolves inside the analyzed repository; --out must be outside the \
             repo (writing it after the final snapshot fence would modify analyzed source \
             post-validation). Set --out to an external path.",
            out.display()
        );
    }

    let abs_state = if state_dir.is_absolute() {
        state_dir.to_path_buf()
    } else {
        cwd("--state-dir")?.join(state_dir)
    };
    let canon_state = canonicalize_with_missing_tail(&abs_state);
    for reserved in [".osp", "attempts"] {
        // #166 review tur-3 (P0): reserved root'un KENDİSİ de canonicalize edilir.
        // `<state-dir>/attempts` bir symlink/junction ile `/external/attempts`'a
        // giderse `--out <state-dir>/attempts/x.json` tam yolu çözülür ama sabit
        // root eşleşmezdi → bypass. Gerçek (çözülmüş) store root'u ile karşılaştır.
        let reserved_root = canonicalize_with_missing_tail(&canon_state.join(reserved));
        if canon_out.starts_with(&reserved_root) {
            anyhow::bail!(
                "--out {} resolves inside the canonical state store ({}/); the no-clobber \
                 evidence store and space identity are immutable — --out cannot target them. \
                 Choose a caller-owned path (state-dir root is allowed).",
                out.display(),
                reserved
            );
        }
    }
    Ok(())
}

/// Mutlak path'i, var olmayan kuyruk bileşenlerini KORUYARAK canonicalize et.
///
/// En derin VAR OLAN atayı `canonicalize` eder (symlink/UNC çözümü), var olmayan
/// kuyruğu geri ekler ve `lexical_normalize` ile `..`/`.` temizler. Var olan
/// atası da çözülemiyorsa lexically normalize edilmiş ham path döner
/// (fail-closed karşılaştırma yine mutlak↔mutlak olur).
fn canonicalize_with_missing_tail(abs: &std::path::Path) -> PathBuf {
    debug_assert!(abs.is_absolute(), "caller must absolutize first");
    let mut existing = abs.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match existing.canonicalize() {
            Ok(canon) => {
                let mut out = canon;
                for part in tail.iter().rev() {
                    out.push(part);
                }
                return lexical_normalize(&out);
            }
            Err(_) => match (existing.parent(), existing.file_name()) {
                (Some(parent), Some(name)) if parent != existing => {
                    tail.push(name.to_os_string());
                    existing = parent.to_path_buf();
                }
                _ => {
                    let mut out = existing.clone();
                    for part in tail.iter().rev() {
                        out.push(part);
                    }
                    return lexical_normalize(&out);
                }
            },
        }
    }
}

/// Lexical path normalization — `.` düşer, `..` bir önceki Normal'ı söker
/// (kökte `..` no-op). Prefix (ör. `\\?\C:\`) ve RootDir korunur.
fn lexical_normalize(path: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// #172 tur-1 (P0-3/P2): girdi path'ini mutlaklaştırıp missing-tail canonicalize
/// eder — var-olmayan çıktı dosyaları da dahil tutarlı alias karşılaştırması.
pub(crate) fn canon_path(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    canonicalize_with_missing_tail(&abs)
}

/// #172 tur-1 (P0-3/P2): atomic artifact publish — unique same-dir temp
/// (`create_new`) → write+sync → rename-replace. Düz `fs::write` truncate
/// penceresini ve yarı-yazılmış artifact bırakma riskini kapatır
/// (`write_attempt_out_copy` çekirdeğinin genelleştirilmiş hâli).
pub(crate) fn atomic_write_replace(out: &Path, payload: &[u8]) -> anyhow::Result<()> {
    let tmp = stage_temp(out, payload)?;
    publish_rename(&tmp, out)
}

/// #172 tur-2 (P2) + tur-3 (P2): İKİ-dosya staged publish — hazırlık aşaması
/// TÜM temp dosyalar başarıyla oluşturulup `sync_all` olana kadar hiçbir hedefi
/// görünür yapmaz; ikinci dosyanın hazırlık hatasında (ör. eksik parent dizin)
/// ilkinin publish'i oluşmaz, temp'ler temizlenir. `rename` aşaması dosya-başına
/// atomiktir ve aynı-dizin temp'inden sonra fiilen infallible'dır; yine de
/// tam iki-dosya filesystem transaction'ı DEĞİLDİR — aradaki bir rename
/// başarısızsa öncekiler yayınlanmış kalabilir (hata mesajı söyler) ve
/// **publish edilmemiş kalan temp'ler temizlenir** (tur-3: leak kapatıldı).
pub(crate) fn atomic_write_replace_staged(pairs: &[(&Path, &[u8])]) -> anyhow::Result<()> {
    let mut staged: Vec<(PathBuf, &Path)> = Vec::with_capacity(pairs.len());
    let mut prep = || -> anyhow::Result<()> {
        for (out, payload) in pairs {
            staged.push((stage_temp(out, payload)?, out));
        }
        Ok(())
    };
    if let Err(e) = prep() {
        for (tmp, _) in &staged {
            let _ = std::fs::remove_file(tmp);
        }
        return Err(e);
    }
    // Tur-3 P2: rename hatasında index'ten sonrakiler (henüz publish edilmemiş
    // temp'ler) silinir — yalnız başarısız olanın temp'i değil.
    for (index, (tmp, out)) in staged.iter().enumerate() {
        if let Err(e) = std::fs::rename(tmp, out) {
            for (remaining, _) in &staged[index..] {
                let _ = std::fs::remove_file(remaining);
            }
            anyhow::bail!(
                "rename failed for {}: {e} (earlier renames in this call may \
                 have published)",
                out.display()
            );
        }
    }
    Ok(())
}

/// Unique same-dir temp oluştur, payload'ı yaz, sync et — publish ETMEDEN dön.
fn stage_temp(out: &Path, payload: &[u8]) -> anyhow::Result<PathBuf> {
    use std::io::Write as _;
    let dir = out
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output path {} has no parent directory", out.display()))?;
    let stem = out
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".to_string());
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();

    for suffix in 0..=64u32 {
        let name = match suffix {
            0 => format!(".{stem}.osp-tmp-{pid}-{millis}"),
            n => format!(".{stem}.osp-tmp-{pid}-{millis}-{n}"),
        };
        let candidate = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(payload).and_then(|_| file.sync_all()) {
                    let _ = std::fs::remove_file(&candidate);
                    anyhow::bail!("temp write failed for {}: {e}", out.display());
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => anyhow::bail!("temp open failed for {}: {e}", out.display()),
        }
    }
    anyhow::bail!("temp collision budget exhausted for {}", out.display())
}

/// Hazır temp'i hedefe taşı (rename-replace); hata durumunda temp'i temizle.
fn publish_rename(tmp: &Path, out: &Path) -> anyhow::Result<()> {
    if let Err(e) = std::fs::rename(tmp, out) {
        let _ = std::fs::remove_file(tmp);
        anyhow::bail!("rename failed for {}: {e}", out.display());
    }
    Ok(())
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

    // #166 review tur-2 (P0 + P1-1): --out hedefi navigator ÇALIŞMADAN doğrulanır —
    // analyzed repo ve canonical state mağazaları (.osp/, attempts/) dışını zorlar.
    if let Some(out) = &args.out {
        validate_attempt_output_path(&args.repo, &state_dir, out)?;
    }

    // Faz 8 test-project (review v6-v7): snapshot-bound controlled harness.
    // Pre-capture repository snapshot (HEAD + tracked paths + dirty-path set).
    // #155 (analyzed-scope clean semantics): global clean-worktree pre-fence KALDIRILDI —
    // fence artık analyze sonrası analyzed-scope'ta çalışır (ölçülen dosyalar
    // HEAD-tracked + değişmemiş olmalı; ilgisiz aktif iş/submodule run'ı bloklamaz).
    let snapshot_before =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;

    // 1. Analyze -> space.
    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig::default();
    let result = analyze_repo_with_config(&args.repo, &registry, &config)?;

    // P1-2: analyzed path ⊆ HEAD tracked-path invariant (fail-closed for untracked files).
    repo_snapshot::validate_analyzed_paths_tracked(&result.node_paths, &snapshot_before)
        .map_err(|e| anyhow::anyhow!(e))?;

    // #155: analyzed-scope pre-fence — ölçülen dosyalar dirty/untracked olamaz
    // (modified veya submodule-altı kapsam etkilenmişse educational red).
    repo_snapshot::validate_analyzed_paths_clean(
        &result.node_paths,
        &snapshot_before,
        "before attempt",
    )
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

    // #152: persisted space identity — load-or-create (state-dir kökü; pending-auths
    // ile aynı kök → D3 resume sözleşmesi tek kökte, repo kirlenmez). Identity
    // edinilemezse attempt BAŞLAMAZ: SystemFailure bucket (exit 70 — persistence/
    // internal, quickstart exit-code contract).
    let space_view_id =
        match osp_core::authorization::PersistedSpaceViewId::load_or_create(&state_dir) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("✗ System failure: persisted space identity unavailable: {e}");
                std::process::exit(exit_codes::SYSTEM_FAILURE);
            }
        };

    let mut engine = SpaceEngine::with_default_rules(
        result.space,
        cs,
        vision,
        EngineConfig::default_calibrated(),
    )?
    .with_persisted_view_id(space_view_id)?;

    // 3. Task resolution: harness task file (snapshot-bound) or hardcoded legacy fallback.
    let task_source: &'static str = if args.task.is_some() {
        "harness_task_file"
    } else {
        "legacy_hardcoded"
    };
    let task = resolve_task(&args, &snapshot_before, &result.node_paths)?;

    // 4. LLM seçimi: mock (FileMockLlm) veya real (RuntimeLlmClient, GPT-4o-mini).
    // #166 P1-1: navigator YAYIM YAPMAZ — AttemptExecution döndürür; fence +
    // canonical persist + emit + exit aşağıda, TÜM navigator sonuçları için.
    let execution = match args.llm.as_str() {
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
            )?
        }
        _ => {
            // mock (default)
            let proposals_path = args
                .proposals
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--proposals required for --llm mock"))?;
            // #161 (B5): v1 çıplak array (id-keyed) veya v2 object envelope
            // (path-keyed — HEAD fence + attempt anindeki baseline'a karşı re-bind).
            let proposals: Vec<osp_core::agent::DeltaProposal> =
                path_keyed_proposals::load_proposals_file(
                    proposals_path,
                    snapshot_before.head.as_str(),
                    &result.node_paths,
                )
                .map_err(|e| anyhow::anyhow!(e))?;
            let llm = crate::mock_llm::FileMockLlm::new(proposals);
            run_navigator(
                &llm,
                &mut engine,
                &args,
                task,
                &state_dir,
                &snapshot_before,
                task_source,
            )?
        }
    };

    // 5. Post-capture drift fence (review P0-3). #155: HEAD + tracked-set eşitliği
    //    global, içerik drift'i analyzed-scope'ta (saf fonksiyon — exact matrix testli).
    //    #166 P1-1: fence ARTIK HER navigator sonucu için koşar (Completed dışı
    //    sonuçlarda eskiden process::exit atlıyordu). Canonical artifact bu
    //    fence'İ GEÇTİKTEN SONRA yazılır — fence başarısızsa kanıt "canonical"
    //    değildir ve diskte canonical artifact YOKTUR.
    let snapshot_after =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    repo_snapshot::validate_post_attempt_snapshot(
        &snapshot_before,
        &snapshot_after,
        &result.node_paths,
    )
    .map_err(|e| anyhow::anyhow!(e))?;

    // 6. #166: canonical publish (no-clobber atomic) → emit (json envelope /
    //    human: progress stderr + stdout evidence[]) → exit.
    persist_canonical_attempt_artifact(&execution.envelope, &args, &state_dir)?;
    emit_attempt_output(&execution, &args)?;
    if execution.exit_code != exit_codes::COMPLETED {
        std::process::exit(execution.exit_code);
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// #164 — `osp trajectory resume`: askılı görevin artifact'tan sürdürülmesi
// ═══════════════════════════════════════════════════════════════════════════════

/// Witness kanıt wire'ı (strict) — operator ağırlık SEÇEMEZ: `weight`
/// `witness_kind`'ın kalibre ağırlığından türetilir (`EvidenceEvent::new`).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WitnessEvidenceWireV1 {
    id: u64,
    source: String,
    witness_kind: osp_core::witness::WitnessKind,
    actor: u64,
    claim: u64,
}

/// Kanıt dosyası yükle — strict wire + `EvidenceEvent::new` (weight = kind default).
fn load_witness_evidence(
    path: &std::path::Path,
) -> anyhow::Result<Vec<osp_core::witness::EvidenceEvent>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("witness evidence file {}: {e}", path.display()))?;
    let wire: Vec<WitnessEvidenceWireV1> = serde_json::from_str(&raw).map_err(|e| {
        anyhow::anyhow!(
            "witness evidence file {} is not a valid EvidenceEvent array (strict wire — \
             fields: id, source, witness_kind, actor, claim): {e}",
            path.display()
        )
    })?;
    Ok(wire
        .into_iter()
        .map(|w| {
            osp_core::witness::EvidenceEvent::new(w.id, w.source, w.witness_kind, w.actor, w.claim)
        })
        .collect())
}

/// `osp trajectory resume <artifact>` — #164 resume akışı (run_trajectory_attempt
/// mirror'ı: aynı state-dir fence ailesi + analyzed-scope clean fence'ler).
///
/// **Sıra sözleşmesi (R1 P0-2):** TÜM fallible doğruluk fence'leri (artifact
/// integrity, receipt idempotency, identity, kanıt doğrulaması, snapshot/analyze,
/// analyzed-scope tracked+clean, post-capture drift) kalıcı etkidEN ÖNCE
/// tamamlanır. Kalıcı etki = engine apply + receipt yazımı (commit/finalization);
/// sonrasında yalnız çıktı + exit kalır. receipt diske yazıldıktan sonra hiçbir
/// doğruluk kontrolü hata veremez — başarısız bir run "Applied" olarak
/// mühürlenmesi bu sırayla imkânsız.
pub fn run_trajectory_resume(args: TrajectoryResumeArgs) -> anyhow::Result<()> {
    use osp_core::axes::{CohesionAxis, EntropyAxis, WitnessDepthAxis};
    use osp_core::coords::{CoordinateSystem, MetricSource};
    use osp_core::engine::{EngineConfig, SpaceEngine};

    // State-dir — attempt ile aynı fence ailesi (repo dışı, her mode'da).
    let state_dir = resolve_state_dir(
        args.state_dir.as_deref(),
        CliExecutionMode::Production,
        &args.repo,
    )?;
    let store = osp_core::authorization::FilesystemPendingAuthorizationStore::new(&state_dir);

    // 1. Artifact yükle + verify (11-adım zincir) — integrity hatası → 70.
    //    Receipt identity-keyed olduğundan (R1 P0-1) önce record gerekir.
    let envelope = match osp_core::authorization::load_pending_authorization(&args.artifact) {
        Ok(envelope) => envelope,
        Err(e) => {
            eprintln!(
                "✗ System failure: pending authorization artifact {}: {e}",
                args.artifact.display()
            );
            std::process::exit(exit_codes::SYSTEM_FAILURE);
        }
    };
    let record = envelope.record();

    // 2. Applied-idempotency fence (R1 P0-1/P1-1): receipt EVIDENCE IDENTITY'den
    //    adreslenir (record'dan türetilir — caller path'i değil) ve strict parse +
    //    verify_against ile doğrulanır. Artifact'ı başka path'ten sunmak (copy/
    //    rename) receipt'i kaçıramaz; sahte/bozuk receipt → 70.
    match store.read_resume_receipt(record) {
        Ok(Some(receipt)) => {
            eprintln!(
                "✗ Resume refused: artifact already applied at unix {} \
                 (task {} / claim {} / attempt {} → sequence {}). \
                 A second authorization of the same suspension is not a new event.",
                receipt.applied_at,
                receipt.task_id,
                receipt.claim_id,
                receipt.attempt_num,
                receipt.resulting_sequence
            );
            std::process::exit(exit_codes::RESUME_REFUSED_BASE);
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!(
                "✗ System failure: resume receipt is unreadable/invalid — resolve \
                 manually before resuming (do not delete blindly; the receipt is the \
                 durable record of an apply): {e}"
            );
            std::process::exit(exit_codes::SYSTEM_FAILURE);
        }
    }

    // 3. Identity — LOAD-ONLY (resume yaratmaz; attempt'in yazdığı identity gerek).
    let space_view_id =
        match osp_core::authorization::PersistedSpaceViewId::load_existing(&state_dir) {
            Ok(id) => id,
            Err(e) => {
                eprintln!(
                    "✗ System failure: persisted space identity unavailable for resume: {e} \
                     (resume requires an identity created by a prior attempt in this \
                     state-dir — wrong --state-dir?)"
                );
                std::process::exit(exit_codes::SYSTEM_FAILURE);
            }
        };

    // 4. Witness kanıtı — strict wire parse; → 20 (operational fault).
    //    (Kanıt provenance'ı — digest + actors — motordan TEK değer olarak
    //    döner; CLI ayrı liste TÜRETMEZ — R3 P1.)
    let evidence = match load_witness_evidence(&args.witness_evidence) {
        Ok(events) => events,
        Err(e) => {
            eprintln!("✗ Witness evaluation error: {e}");
            std::process::exit(exit_codes::WITNESS_EVALUATION_ERROR);
        }
    };

    // 5. Snapshot + analyze + analyzed-scope fence'ler (attempt ile aynı).
    let snapshot_before =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig::default();
    let result = analyze_repo_with_config(&args.repo, &registry, &config)?;
    repo_snapshot::validate_analyzed_paths_tracked(&result.node_paths, &snapshot_before)
        .map_err(|e| anyhow::anyhow!(e))?;
    repo_snapshot::validate_analyzed_paths_clean(
        &result.node_paths,
        &snapshot_before,
        "before resume",
    )
    .map_err(|e| anyhow::anyhow!(e))?;

    // 6. Engine — attempt ile BİREBİR aynı kurulum (farklı kurulum farklı digest
    //    üretirdi → staleness fence yanlış tetiklenirdi).
    let cs = CoordinateSystem::default_raw_five(
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
    )?
    .with_persisted_view_id(space_view_id)?;

    // 7. Post-capture drift fence — SON doğruluk fence'i (R1 P0-2). Resume repo
    //    dokunmaz; pre/post eşitliği analiz penceresinde dış drift olmadığını
    //    kanıtlar. Bu noktadan sonra fallible doğruluk kontrolü YOKTUR — yalnız
    //    commit (apply + receipt) + raporlama kalır.
    let snapshot_after =
        repo_snapshot::RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    repo_snapshot::validate_post_attempt_snapshot(
        &snapshot_before,
        &snapshot_after,
        &result.node_paths,
    )
    .map_err(|e| anyhow::anyhow!(e))?;

    // 8. Commit — fence'ler + witness değerlendirme + apply (tek atomik motor adımı).
    let outcome = match engine.resume_held_authorization(&envelope, evidence) {
        Ok(outcome) => outcome,
        Err(osp_core::engine::ResumeHeldError::ClaimBindingMismatch {
            event_id,
            evidence_claim,
            artifact_claim,
        }) => {
            eprintln!(
                "✗ Witness evaluation error: evidence event {event_id} witnesses claim \
                 {evidence_claim} but the artifact suspends claim {artifact_claim} — \
                 foreign-claim evidence cannot authorize this suspension"
            );
            std::process::exit(exit_codes::WITNESS_EVALUATION_ERROR);
        }
        Err(osp_core::engine::ResumeHeldError::InvalidEvidence { detail }) => {
            eprintln!(
                "✗ Witness evaluation error: {detail} — fix the evidence file \
                 (one event per witness; the author cannot witness their own claim)"
            );
            std::process::exit(exit_codes::WITNESS_EVALUATION_ERROR);
        }
        Err(osp_core::engine::ResumeHeldError::StaleBaseRevision {
            base_seq,
            current_seq,
            base_digest,
            current_digest,
        }) => {
            eprintln!(
                "✗ Resume refused: stale base revision (artifact base sequence {base_seq} / \
                 digest {base_digest} vs current {current_seq} / {current_digest}) — the \
                 space changed since suspension; remeasure required (new attempt on fresh \
                 baseline)"
            );
            std::process::exit(exit_codes::RESUME_REFUSED_BASE);
        }
        Err(osp_core::engine::ResumeHeldError::IdentityMismatch { artifact, current }) => {
            eprintln!(
                "✗ System failure: space identity mismatch (artifact base view {artifact} ≠ \
                 current view {current}) — the artifact belongs to a different \
                 state-dir/space (wrong --state-dir?)"
            );
            std::process::exit(exit_codes::SYSTEM_FAILURE);
        }
    };

    // 9. Applied ise kalıcı receipt (idempotency + kanıt provenance) — yazılamazsa
    //    dürüst 70. Askı kimliği record'dan, kanıt bağlaması motorun TEK
    //    provenance değerinden (R3 P1: digest + actors yapışık — CLI ayrı actor
    //    listesi VERMEZ), güven beyanı operator'ın --evidence-trust flag'inden
    //    (R2 P0); adres identity-keyed (R1 P0-1).
    if let osp_core::engine::ResumeHeldOutcome::Applied {
        resulting_sequence,
        evidence_provenance,
        ..
    } = &outcome
    {
        let applied_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let receipt_input = osp_core::authorization::ResumeReceiptInput {
            record,
            applied_at,
            resulting_sequence: *resulting_sequence,
            evidence_trust: args.evidence_trust.to_core(),
            evidence_provenance: evidence_provenance.clone(),
        };
        if let Err(e) = store.write_resume_receipt(receipt_input) {
            eprintln!(
                "✗ System failure: mutation applied in-engine (sequence \
                 {resulting_sequence}) but resume receipt persistence failed: {e} — \
                 the apply is NOT durably recorded; do not re-run this artifact before \
                 resolving the receipt failure"
            );
            std::process::exit(exit_codes::SYSTEM_FAILURE);
        }
    }

    // 10. Rapor + exit (doğruluk fence'i YOK — yalnız çıktı).
    let evidence_trust_str = args.evidence_trust.to_core().as_str();
    let is_json = args.format.eq_ignore_ascii_case("json");
    if is_json {
        let envelope_json = resume_envelope::build_resume_envelope_v1(
            &outcome,
            &args.artifact,
            record.task_id,
            record.claim_id,
            record.attempt_num.get(),
            evidence_trust_str,
        );
        println!("{}", serde_json::to_string_pretty(&envelope_json)?);
    } else {
        print_human_resume_result(
            &outcome,
            record.task_id,
            record.claim_id,
            evidence_trust_str,
        );
    }

    let exit_code = match &outcome {
        osp_core::engine::ResumeHeldOutcome::Applied { .. } => exit_codes::COMPLETED,
        osp_core::engine::ResumeHeldOutcome::StillHeld { .. } => exit_codes::AWAITING_WITNESSES,
        osp_core::engine::ResumeHeldOutcome::Rejected { .. } => exit_codes::REQUIRES_REVISION,
    };
    if exit_code != exit_codes::COMPLETED {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// Human-readable resume sonucu.
fn print_human_resume_result(
    outcome: &osp_core::engine::ResumeHeldOutcome,
    task_id: u64,
    claim_id: u64,
    evidence_trust: &str,
) {
    match outcome {
        osp_core::engine::ResumeHeldOutcome::Applied {
            resulting_sequence,
            snapshot,
            ..
        } => {
            println!(
                "✓ Suspended authorization applied — task {task_id} / claim {claim_id} \
                 (space revision sequence → {resulting_sequence})"
            );
            println!(
                "  Witness quorum: {}/{} approvers, support {:.3}/{:.3}",
                snapshot.approvers,
                snapshot.required_approvers,
                snapshot.support,
                snapshot.required_support
            );
            println!("  Evidence trust: {evidence_trust} (recorded in the resume receipt)");
        }
        osp_core::engine::ResumeHeldOutcome::StillHeld { reason, snapshot } => {
            println!(
                "… Still awaiting witnesses — task {task_id} / claim {claim_id} \
                 ({}: {})",
                reason.as_reason_str(),
                hold_reason_detail(reason)
            );
            println!(
                "  Witness quorum: {}/{} approvers, support {:.3}/{:.3}",
                snapshot.approvers,
                snapshot.required_approvers,
                snapshot.support,
                snapshot.required_support
            );
            println!(
                "  Artifact unchanged — evidence does NOT accumulate across resume \
                 attempts: resume again with the COMPLETE evidence set (this file's \
                 events are the only ones evaluated)."
            );
        }
        osp_core::engine::ResumeHeldOutcome::Rejected { reasons, snapshot } => {
            println!(
                "✗ Witness rejection — task {task_id} / claim {claim_id} ({} reason(s))",
                reasons.as_slice().len()
            );
            println!(
                "  Witness quorum: {}/{} approvers, support {:.3}/{:.3}",
                snapshot.approvers,
                snapshot.required_approvers,
                snapshot.support,
                snapshot.required_support
            );
        }
    }
}

/// Hold reason kısa detay metni (human çıktı için).
fn hold_reason_detail(reason: &osp_core::witness::WitnessHoldReason) -> String {
    match reason {
        osp_core::witness::WitnessHoldReason::MinApproversNotMet { distinct, required } => {
            format!("{distinct}/{required} distinct non-author approvers")
        }
        osp_core::witness::WitnessHoldReason::QuorumInsufficient { support, threshold } => {
            format!("support {support:.3} < threshold {threshold:.3}")
        }
        osp_core::witness::WitnessHoldReason::EvidenceNotLocallyObservable { hint } => hint.clone(),
    }
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
    let task = match (args.execution_mode, args.task.as_ref()) {
        (CliExecutionMode::Harness, Some(task_path)) => {
            harness_task::load_and_validate_harness_task(
                task_path,
                args.task_id,
                snapshot,
                node_paths,
                args.maneuver_limit,
            )
            .map_err(|e| anyhow::anyhow!(e))?
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
            harness_task::load_and_validate_harness_task(
                task_path,
                args.task_id,
                snapshot,
                node_paths,
                args.maneuver_limit,
            )
            .map_err(|e| anyhow::anyhow!(e))?
        }
        (CliExecutionMode::Production, None) => {
            // Legacy hardcoded fallback (Node(0), coupling ≤ 0.55). D1 backward-compat.
            let policy = TaskPolicy {
                maneuver_limit: args.maneuver_limit.unwrap_or(5),
                predicate_failure_policy: PredicateFailurePolicy::StrictReject,
                ..Default::default()
            };
            osp_core::trajectory::Task {
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
                            // #144 (karar A, live-contract Faz 0): attempt pipeline'ın
                            // coupling authority'si TreeSitter'dır (INV-T9 #70 — topology
                            // source coupling + instability provenance'ını birlikte bağlar);
                            // legacy task bunu declare eder. Provenance-fail öğretici
                            // örneği ayrı, adlandırılmış fixture'tır (docs/fixtures/).
                            required_source: Some(osp_core::coords::MetricSource::TreeSitter),
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
            }
        }
    };
    // #144 (live-contract Faz 0): measurement-authority preflight — yapısal uyuşmazlığı
    // task-load anında educational hatayla bildir (INV-T4 runtime fail-closed korunur;
    // bu kontrol onun ÖNÜNE geçer, sessiz fallback yok).
    validate_attempt_measurement_authority(&task)?;
    Ok(task)
}

/// #144 / #151 (live-contract Faz 0): attempt pipeline authority-profile preflight.
///
/// INV-T4 ölçüm anında fail-closed red uygular (runtime savunma); bu preflight aynı
/// yapısal uyuşmazlıkları task yükleme anında, hatanın *sebebiyle* bildirir.
/// Preflight **exhaustive Live Contract v1 authority matrisidir** — sessiz geçiş
/// (`_ => {}`) YOKTUR; her axis × required_source kombinasyonu açıkça sınıflanır:
///
/// - **Coupling / Instability**: attempt pipeline'da ölçüm daima TreeSitter kaynaklıdır
///   (INV-T9 #70: topology source coupling + instability provenance'ını birlikte bağlar;
///   SCIP index bağlansa bile değişmez). Bu yüzden `required_source: TreeSitter`
///   **declare edilmesi zorunludur** — Scip istemek authority sözleşmesiyle çelişir,
///   None/Placeholder/Heuristic/Mixed ise pipeline authority'siyle yapısal uyuşmazdır.
/// - **Cohesion**: Scip-authoritative'dır ama attempt Tier-1 analizle çalışır ve Scip
///   ölçüm üretmez (placeholder cohesion) → declare edilse de edilmese de bu pipeline'da
///   asla tamamlanamaz (`ScipMeasurementUnavailable`).
/// - **Entropy / WitnessDepth**: commit-entropy / witness preset'lerinden pipeline
///   türetilir; v1 kontratta `required_source` declare EDİLMEMELİDİR (ölçüm INV-T4'te
///   değerlendirilir).
/// - **RiskScore / MainSequenceDistance / Custom**: derived/custom axis'tir;
///   `MeasuredRawPosition::axis()` bunlar için legacy coupling fallback'u yapar
///   (P2-2, ayrı takipte) — sessiz ontolojik fallback'i önlemek için yükleme anında
///   reddedilir (`UnsupportedPredicateAxis`).
fn validate_attempt_measurement_authority(task: &osp_core::trajectory::Task) -> anyhow::Result<()> {
    use osp_core::coords::MetricSource;
    use osp_core::trajectory::PredicateAxis;
    for wp in &task.target_predicate_set.predicates {
        let p = &wp.predicate;
        match p.metric {
            PredicateAxis::Coupling | PredicateAxis::Instability => match p.required_source {
                Some(MetricSource::TreeSitter) => {}
                Some(MetricSource::Scip) => anyhow::bail!(
                    "UnsupportedMeasurementAuthority: {:?} in the attempt pipeline is measured \
                     from tree_sitter (INV-T9 #70: the topology source binds coupling and \
                     instability provenance together — loading a SCIP index would not change \
                     this authority), but the task requires scip. Restructure the task \
                     predicate (Live Contract v1 authority profile: docs/design/live-contract.md; \
                     live-use program #151)",
                    p.metric
                ),
                other => anyhow::bail!(
                    "UnsupportedMeasurementAuthority: {:?} in the attempt pipeline is measured \
                     from tree_sitter — the task MUST declare `required_source: TreeSitter` \
                     (Live Contract v1 authority profile; no silent fallback), found {:?}",
                    p.metric,
                    other
                ),
            },
            PredicateAxis::Cohesion => anyhow::bail!(
                "ScipMeasurementUnavailable: cohesion is Scip-authoritative, but \
                 `trajectory attempt` runs Tier-1 analysis and accepts no SCIP index — \
                 a cohesion predicate can never complete here, with or without a declared \
                 source. Use `osp analyze --scip` for Scip cohesion measurements \
                 (live-use program #151 tracks attempt-side SCIP)"
            ),
            PredicateAxis::Entropy | PredicateAxis::WitnessDepth => {
                if let Some(src) = p.required_source {
                    anyhow::bail!(
                        "UnsupportedMeasurementAuthority: {:?} is pipeline-derived in attempt \
                         v1 (commit-entropy / witness presets) and must NOT declare \
                         required_source — the produced measurement is evaluated by INV-T4 at \
                         measurement time, found {:?}",
                        p.metric,
                        src
                    );
                }
            }
            PredicateAxis::RiskScore
            | PredicateAxis::MainSequenceDistance
            | PredicateAxis::Custom => anyhow::bail!(
                "UnsupportedPredicateAxis: {:?} is a derived/custom axis — the attempt \
                 pipeline would silently evaluate it against the coupling measurement \
                 (legacy fallback) and therefore refuses it up front. Attempt v1 supports \
                 coupling / cohesion / instability / entropy / witness_depth only \
                 (Live Contract v1)",
                p.metric
            ),
        }
    }
    Ok(())
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
) -> anyhow::Result<AttemptExecution> {
    use osp_core::navigator::AgentNavigator;
    use osp_core::trajectory::{
        InMemoryTaskRegistry, MilestoneId, OperatorCapability, TrajectoryId,
    };
    // CLI = operator mode (INV-T2) — trusted-boundary API (PR35 hardening).
    let _cap = OperatorCapability::issue_for_operator_session();
    let mut task_registry = InMemoryTaskRegistry::new();
    task_registry.insert(task);
    // 5. Navigator.
    // #100 (S5): synthetic bootstrap seed — uniform Scip damgası kalktı (source
    // laundering); dürüst etiket Placeholder (ölçüm DEĞİL, G1 bootstrap telemetry).
    let seed = osp_core::coords::RawPosition {
        x: 0.7,
        y: 0.5,
        z: 0.5,
        w: 0.5,
        v: 0.3,
    };
    let stamp = |v: f64| osp_core::trajectory::AxisMetric {
        value: v,
        source: osp_core::coords::MetricSource::Placeholder,
    };
    let current_measured = osp_core::trajectory::ProvenancedRawPosition {
        coupling: stamp(seed.x),
        cohesion: stamp(seed.y),
        instability: stamp(seed.z),
        entropy: stamp(seed.w),
        witness_depth: stamp(seed.v),
    };
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
    drop(nav); // evidence'ın mutable ödünç alanı (nav field'ı) biter — taşınabilir.
               // #166 P1-1 (review): navigator YAYIM YAPMAZ — persist/emit/exit dış katmanda,
               // final snapshot fence'inden SONRA koşar. Envelope burada yalnız HAZIRLANIR:
               // measured + subject-validity + persistence-validity birlikte "canonical"dir.
    let envelope = run_envelope::build_run_envelope_v1(
        &result,
        &evidence,
        args.execution_mode,
        args.witness,
        task_source,
        snapshot.head.as_str(),
        args.task_id,
    );
    let exit_code = navigator_exit_code(&result, args.task_id);
    Ok(AttemptExecution {
        result,
        evidence,
        envelope,
        exit_code,
    })
}

/// #166 P1-1: navigator yürütmesinin taşınabilir sonucu — yayım (canonical
/// persist + emit + exit) dış katmanda, final snapshot fence'inden sonra.
struct AttemptExecution {
    result: osp_core::navigator::NavigatorResult,
    evidence: Vec<osp_core::trajectory::TrajectoryEvidence>,
    envelope: run_envelope::CliRunEnvelopeV1,
    exit_code: i32,
}

/// #166 P1-1/P1-2: kanonik attempt artifact'ı YAYINLA — final snapshot fence
/// BAŞARILI olduktan sonra çağrılmalı (fence başarısızsa çağrılmaz: canonical
/// artifact YOKTUR; diagnostic-artifact bilinçli olarak eklenmedi — "canonical"
/// unvanı fence-geçmiş kanıta özeldir).
///
/// P1-2 persistence modeli — pending-authorization/space-identity precedent'i:
/// same-dir temp (`create_new`) → `write_all` + `sync_all` → **`hard_link`
/// no-clobber publish** (`fs::rename` hedefi REPLACE eder — no-clobber DEĞİL) →
/// parent-dir sync → temp temizliği. Crash penceresi: hedef ya YOKtur ya TAM
/// içeriktir; yarım canonical dosya üretilemez. Kimlik `task_id + unix_millis +
/// pid` (+ çakışmada `-N` soneki, 64 deneme bütçesi — zaman tek başına kimlik
/// değildir; `hard_link` AlreadyExists ayrımı zorlar, fail-closed).
fn persist_canonical_attempt_artifact(
    envelope: &run_envelope::CliRunEnvelopeV1,
    args: &TrajectoryAttemptArgs,
    state_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let payload = serde_json::to_vec_pretty(envelope)?;
    let attempts_dir = state_dir.join("attempts");
    std::fs::create_dir_all(&attempts_dir)?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    let canonical =
        publish_no_clobber_attempt_artifact(&attempts_dir, args.task_id, millis, pid, &payload)?;
    eprintln!("Canonical attempt artifact: {}", canonical.display());

    // --out kopyası: çağırıcıya ait hedef (validate_attempt_output_path ile
    // repo/canonical-store dışı guarantee'li) — unique temp + create_new ile
    // yazılır (tur-3 P1: temp adı caller verisine DEĞMEZ), rename final hedefi
    // replace eder (çağırıcının açık isteği).
    if let Some(out) = &args.out {
        write_attempt_out_copy(out, &payload)?;
        eprintln!("Attempt artifact (--out): {}", out.display());
    }
    Ok(())
}

/// #166 review tur-3 (P1): `--out` kopyası — unique same-dir temp + `create_new`
/// + write/sync + rename + hata durumunda temp temizliği.
///
/// Eski sabit `<target>.json.tmp-osp` adı, o adda duran ilgisiz bir caller
/// dosyasını `truncate` edebilir ve iki eşzamanlı çağrı aynı temp'i
/// paylaşabilirdi; canonical store'daki pattern ile aynı disiplin. `rename`
/// final hedefi replace eder — overwrite çağırıcının açık isteğidir (kasıtlı).
fn write_attempt_out_copy(out: &std::path::Path, payload: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;
    let dir = out
        .parent()
        .ok_or_else(|| anyhow::anyhow!("--out {} has no parent directory", out.display()))?;
    let stem = out
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "attempt".to_string());
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();

    let mut tmp: Option<PathBuf> = None;
    for suffix in 0..=64u32 {
        let name = match suffix {
            0 => format!(".{stem}.osp-tmp-{pid}-{millis}"),
            n => format!(".{stem}.osp-tmp-{pid}-{millis}-{n}"),
        };
        let candidate = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(payload).and_then(|_| file.sync_all()) {
                    let _ = std::fs::remove_file(&candidate);
                    anyhow::bail!("--out temp write failed: {e}");
                }
                tmp = Some(candidate);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => anyhow::bail!("--out temp open failed: {e}"),
        }
    }
    let tmp = match tmp {
        Some(p) => p,
        None => anyhow::bail!("--out temp collision budget exhausted"),
    };
    if let Err(e) = std::fs::rename(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        anyhow::bail!("--out rename failed: {e}");
    }
    Ok(())
}

/// #166 review tur-2 (P2-2): no-clobber publish çekirdeği — millis/pid PARAMETRE
/// (test edilebilirlik; gerçek değerler wrapper'dan). Deterministic kimlik +
/// collision yolu (`-N` soneği) unit testlerde pinli.
fn publish_no_clobber_attempt_artifact(
    attempts_dir: &std::path::Path,
    task_id: u64,
    millis: u128,
    pid: u32,
    payload: &[u8],
) -> anyhow::Result<PathBuf> {
    use std::io::Write as _;
    // Same-dir temp — ad per-attempt benzersiz (pid+millis): aynı process aynı
    // thread'in bir sonraki attempt'i stale temp'e takılmaz.
    let tmp = attempts_dir.join(format!("attempt.tmp.{pid}.{millis}"));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(payload).and_then(|_| file.sync_all())?;
    }

    // No-clobber publish — candidate çakışırsa -N sonekiyle devam (fail-closed).
    let mut canonical: Option<PathBuf> = None;
    for suffix in 0..=64u32 {
        let name = match suffix {
            0 => format!("task-{task_id}-{millis}-{pid}.json"),
            n => format!("task-{task_id}-{millis}-{pid}-{n}.json"),
        };
        let candidate = attempts_dir.join(name);
        match std::fs::hard_link(&tmp, &candidate) {
            Ok(()) => {
                canonical = Some(candidate);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                anyhow::bail!("canonical attempt artifact publish failed: {e}");
            }
        }
    }
    let canonical = match canonical {
        Some(p) => p,
        None => {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("canonical attempt artifact collision budget exhausted");
        }
    };
    // Directory durability — best-effort (Windows'ta no-op).
    let _ = std::fs::File::open(attempts_dir).and_then(|d| d.sync_all());
    let _ = std::fs::remove_file(&tmp);
    Ok(canonical)
}

/// #166: attempt çıktısını yayınla — json modunda stdout'a tam envelope;
/// human modunda progress stderr'de, stdout'ta HER ZAMAN geçerli evidence JSON
/// dizisi (P1-3: boş evidence → `[]`; zero-evidence outcome'lar parser
/// istisnası üretmez).
fn emit_attempt_output(
    execution: &AttemptExecution,
    args: &TrajectoryAttemptArgs,
) -> anyhow::Result<()> {
    if args.format.eq_ignore_ascii_case("json") {
        let json = serde_json::to_string_pretty(&execution.envelope)?;
        // Diagnostics stderr'e — stdout JSON-only.
        // **#96 MD-2 + #95-A MD-1 cutover:** navigator ölçümü engine-native
        // per-axis (opaque NativeSubjectMeasurement token) + canonical
        // task-scope subject. Envelope iki-eksen vocabulary taşır.
        eprintln!(
            "osp trajectory attempt: #96 MD-2 engine_native_per_axis provenance authority \
             (provenance_native=true) + #95-A MD-1 task_scope subject authority"
        );
        println!("{json}");
    } else {
        print_human_result(&execution.result, args.task_id, &execution.evidence)?;
    }
    Ok(())
}

/// Print human-readable navigator result (non-json mode).
///
/// #166 akış ayrımı: progress satırları stderr'e yazılır; stdout yalnız
/// makine-okunur evidence JSON dizisi taşır (elle ayıklama gerektirmez).
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
            eprintln!("✓ Task completed in {attempts} attempts");
            eprintln!("  Total tokens: {}", total_tokens.total_tokens);
        }
        NavigatorResult::ExceededManeuverLimit { attempts, .. } => {
            eprintln!("✗ Maneuver limit exceeded after {attempts} attempts");
        }
        NavigatorResult::AwaitingWitnesses {
            pending,
            persistence,
        } => {
            eprintln!(
                "⏸ Awaiting witnesses (INV-T9) — task {}, claim {}",
                pending.task_id, pending.claim_id
            );
            eprintln!(
                "  Witness hold reason: {}",
                pending.witness_hold_reason.as_reason_str()
            );
            eprintln!("  Commit state: awaiting_witnesses");
            eprintln!("  Mainline mutation: not_applied");
            eprintln!("  Next action: await external evidence");
            eprintln!(
                "  Pending artifact: {}",
                persistence.artifact_path.display()
            );
        }
        NavigatorResult::RequiresRevision(rev) => {
            eprintln!(
                "↻ Requires revision (explicit witness rejection) — task {}, claim {}",
                rev.task_id(),
                rev.claim_id()
            );
        }
        NavigatorResult::AwaitingColdStartApproval {
            attempts,
            task_id,
            claim_id,
            ..
        } => {
            eprintln!(
                "❄ Awaiting cold-start operator approval (INV-T9 ext) — task {}, claim {}, attempt {}",
                task_id, claim_id, attempts
            );
        }
        NavigatorResult::PendingAuthorizationPersistenceFailure { pending, error } => {
            eprintln!(
                "✗ Pending authorization persistence failed — task {}, claim {}: {error}",
                pending.task_id, pending.claim_id
            );
        }
        NavigatorResult::WitnessEvaluationError(msg) => {
            eprintln!("✗ Witness evaluation error: {msg}");
        }
        NavigatorResult::SystemFailure(msg) => {
            eprintln!("✗ System failure: {msg}");
        }
        NavigatorResult::TaskNotFound => {
            eprintln!("✗ Task {task_id} not found");
        }
        NavigatorResult::RequiresOperatorApproval { attempts, .. } => {
            eprintln!("⚠ Operator approval required after {attempts} attempts");
        }
        NavigatorResult::LlmError(e) => {
            eprintln!("✗ LLM error: {e}");
        }
    }
    eprintln!("  Evidence entries: {}", evidence.len());
    // #166 P1-3: stdout HER ZAMAN geçerli JSON dizisi — boş evidence'da `[]`.
    // Zero-evidence outcome'lar (task_not_found, erken llm_error) parser istisnası
    // üretmez; `serde_json::from_reader(stdout)` her durumda çalışır.
    let json = serde_json::to_string_pretty(evidence)?;
    println!("{json}");
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
        NavigatorResult::AwaitingColdStartApproval { .. } => {
            exit_codes::AWAITING_COLD_START_APPROVAL
        }
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
            dirty_paths: std::collections::BTreeSet::new(),
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
            out: None,
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
        // #144 (karar A): legacy demo task, attempt pipeline'ın gerçek coupling
        // authority'sini declare eder — TreeSitter (INV-T9 #70).
        assert_eq!(
            task.target_predicate_set.predicates[0]
                .predicate
                .required_source,
            Some(osp_core::coords::MetricSource::TreeSitter)
        );
    }

    // ═══ #144 (live-contract Faz 0): measurement-authority preflight ═══

    fn authority_task(
        metric: osp_core::trajectory::PredicateAxis,
        required_source: Option<osp_core::coords::MetricSource>,
    ) -> osp_core::trajectory::Task {
        use osp_core::trajectory::{
            ComparisonOp, MetricPredicate, PredicateMode, PredicateScope, PredicateSet, TaskPolicy,
            TaskStatus, WeightedPredicate,
        };
        osp_core::trajectory::Task {
            id: 1,
            milestone_id: 1,
            label: "authority preflight test".into(),
            target_predicate_set: PredicateSet {
                mode: PredicateMode::All,
                predicates: vec![WeightedPredicate {
                    predicate: MetricPredicate {
                        metric,
                        operator: ComparisonOp::Le,
                        threshold: 0.5,
                        scope: PredicateScope::Node(0),
                        required_source,
                        tolerance: 0.0,
                    },
                    weight: None,
                }],
                preferred_vector: Some(osp_core::coords::RawPosition {
                    x: 0.5,
                    y: 0.5,
                    z: 0.5,
                    w: 0.5,
                    v: 0.3,
                }),
            },
            policy: TaskPolicy::default(),
            allowed_operations: vec![],
            constraints: vec![],
            status: TaskStatus::Pending,
        }
    }

    #[test]
    fn authority_preflight_rejects_coupling_required_scip() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Coupling,
            Some(osp_core::coords::MetricSource::Scip),
        ))
        .expect_err("coupling + required_source:Scip must fail the preflight");
        let msg = format!("{err}");
        assert!(
            msg.contains("UnsupportedMeasurementAuthority"),
            "got: {msg}"
        );
        assert!(
            msg.contains("tree_sitter") && msg.contains("would not change this authority"),
            "expected educational authority explanation, got: {msg}"
        );
    }

    #[test]
    fn authority_preflight_rejects_instability_required_scip() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Instability,
            Some(osp_core::coords::MetricSource::Scip),
        ))
        .expect_err("instability + required_source:Scip must fail the preflight");
        assert!(format!("{err}").contains("UnsupportedMeasurementAuthority"));
    }

    #[test]
    fn authority_preflight_rejects_cohesion_required_scip_with_unavailable() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Cohesion,
            Some(osp_core::coords::MetricSource::Scip),
        ))
        .expect_err("cohesion + required_source:Scip must fail the preflight (Tier-1 attempt)");
        let msg = format!("{err}");
        assert!(msg.contains("ScipMeasurementUnavailable"), "got: {msg}");
        assert!(
            msg.contains("analyze --scip"),
            "expected pointer to analyze --scip, got: {msg}"
        );
    }

    #[test]
    fn authority_preflight_accepts_coupling_required_treesitter() {
        validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Coupling,
            Some(osp_core::coords::MetricSource::TreeSitter),
        ))
        .expect("coupling + required_source:TreeSitter matches the attempt pipeline authority");
    }

    #[test]
    fn authority_preflight_rejects_coupling_without_declared_source() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Coupling,
            None,
        ))
        .expect_err("coupling must DECLARE required_source:TreeSitter (exhaustive matrix)");
        let msg = format!("{err}");
        assert!(
            msg.contains("UnsupportedMeasurementAuthority"),
            "got: {msg}"
        );
        assert!(
            msg.contains("MUST declare `required_source: TreeSitter`") && msg.contains("None"),
            "expected declaration-required explanation, got: {msg}"
        );
    }

    #[test]
    fn authority_preflight_rejects_coupling_placeholder_source() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Coupling,
            Some(osp_core::coords::MetricSource::Placeholder),
        ))
        .expect_err("coupling + required_source:Placeholder is structurally incompatible");
        assert!(format!("{err}").contains("UnsupportedMeasurementAuthority"));
    }

    #[test]
    fn authority_preflight_rejects_cohesion_without_declared_source() {
        // Live Contract v1: cohesion attempt pipeline'da KULLANILAMAZ — INV-T4'ün
        // "None → constraint yok" semantiği Tier-1 placeholder cohesion ölçümüyle
        // birleşince sessiz Completed kapısı açardı; preflight bunu kapatır.
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Cohesion,
            None,
        ))
        .expect_err("cohesion must fail preflight even without a declared source");
        let msg = format!("{err}");
        assert!(msg.contains("ScipMeasurementUnavailable"), "got: {msg}");
        assert!(
            msg.contains("with or without a declared source"),
            "expected None-covered explanation, got: {msg}"
        );
    }

    #[test]
    fn authority_preflight_accepts_entropy_without_declared_source() {
        validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Entropy,
            None,
        ))
        .expect("entropy is pipeline-derived in v1; unset required_source is the contract");
    }

    #[test]
    fn authority_preflight_accepts_witness_depth_without_declared_source() {
        validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::WitnessDepth,
            None,
        ))
        .expect("witness-depth is pipeline-derived in v1; unset required_source is the contract");
    }

    #[test]
    fn authority_preflight_rejects_entropy_with_declared_source() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::Entropy,
            Some(osp_core::coords::MetricSource::TreeSitter),
        ))
        .expect_err("entropy must NOT declare required_source in attempt v1");
        assert!(format!("{err}").contains("UnsupportedMeasurementAuthority"));
    }

    #[test]
    fn authority_preflight_rejects_derived_axis_risk_score() {
        let err = validate_attempt_measurement_authority(&authority_task(
            osp_core::trajectory::PredicateAxis::RiskScore,
            None,
        ))
        .expect_err("RiskScore would silently evaluate against coupling (legacy fallback)");
        let msg = format!("{err}");
        assert!(msg.contains("UnsupportedPredicateAxis"), "got: {msg}");
        assert!(
            msg.contains("coupling measurement") && msg.contains("refuses it up front"),
            "expected no-silent-fallback explanation, got: {msg}"
        );
    }

    #[test]
    fn authority_preflight_rejects_derived_and_custom_axes() {
        for axis in [
            osp_core::trajectory::PredicateAxis::MainSequenceDistance,
            osp_core::trajectory::PredicateAxis::Custom,
        ] {
            let err = validate_attempt_measurement_authority(&authority_task(axis, None))
                .expect_err("derived/custom axes must fail the preflight");
            assert!(format!("{err}").contains("UnsupportedPredicateAxis"));
        }
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

#[cfg(test)]
mod attempt_artifact_persistence_tests {
    //! #166 review tur-2: no-clobber publish çekirdeği + --out hedef doğrulaması.
    use super::*;

    #[test]
    fn publish_no_clobber_collision_takes_suffix_path() {
        // P2-2: aynı deterministic kimlik (task/millis/pid) ikinci publish'de
        // `-1` soneğine düşer; içerikler korunur; temp artığı kalmaz.
        let dir = tempfile::tempdir().expect("tempdir");
        let attempts = dir.path().join("attempts");
        std::fs::create_dir_all(&attempts).expect("mkdir");

        let first = publish_no_clobber_attempt_artifact(&attempts, 7, 123, 42, b"first")
            .expect("first publish");
        assert_eq!(
            first.file_name().unwrap().to_string_lossy(),
            "task-7-123-42.json"
        );

        let second = publish_no_clobber_attempt_artifact(&attempts, 7, 123, 42, b"second")
            .expect("second publish (collision -> -1)");
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            "task-7-123-42-1.json",
            "collision must take the -1 suffix, not clobber"
        );

        let third = publish_no_clobber_attempt_artifact(&attempts, 7, 123, 42, b"third")
            .expect("third publish (collision -> -2)");
        assert_eq!(
            third.file_name().unwrap().to_string_lossy(),
            "task-7-123-42-2.json"
        );

        assert_eq!(
            std::fs::read(&first).unwrap(),
            b"first",
            "first artifact content must be untouched"
        );
        assert_eq!(std::fs::read(&second).unwrap(), b"second");

        // Temp artığı kalmaz (başarılı publish temp'i temizler).
        let leftovers: Vec<_> = std::fs::read_dir(&attempts)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("attempt.tmp."))
            .collect();
        assert!(leftovers.is_empty(), "no temp leftovers: {leftovers:?}");
    }

    #[test]
    fn validate_attempt_output_path_rejects_repo_and_canonical_stores() {
        // P0 + P1-1: --out analyzed repo içine, state-dir/.osp/** ve
        // state-dir/attempts/** altına yazılamaz; state-dir kökü serbest.
        let repo = tempfile::tempdir().expect("repo tempdir");
        let state = tempfile::tempdir().expect("state tempdir");
        std::fs::create_dir_all(state.path().join(".osp")).expect("mkdir .osp");
        std::fs::create_dir_all(state.path().join("attempts")).expect("mkdir attempts");

        // Repo içi → red. (#182 sonrası parent-dizin preflight'i önce koştuğu için
        // bu fence'in kendisini görmek istiyoruz — fixture parent'ı önceden yaratır.)
        std::fs::create_dir_all(repo.path().join("src")).expect("mkdir repo/src");
        let err = validate_attempt_output_path(
            repo.path(),
            state.path(),
            &repo.path().join("src/foo.rs"),
        )
        .expect_err("out inside repo must be rejected");
        assert!(
            err.to_string().contains("inside the analyzed repository"),
            "message: {err}"
        );

        // Canonical store'lar → red.
        for reserved in ["space-identity", "attempts/task-7-1.json"] {
            let target = if reserved.starts_with("attempts") {
                state.path().join(reserved)
            } else {
                state.path().join(".osp").join(reserved)
            };
            let err = validate_attempt_output_path(repo.path(), state.path(), &target)
                .expect_err(&format!("must reject {reserved}"));
            assert!(
                err.to_string().contains("canonical state store"),
                "message for {reserved}: {err}"
            );
        }

        // State-dir kökü (caller-owned) → serbest; tamamen dış path → serbest.
        validate_attempt_output_path(
            repo.path(),
            state.path(),
            &state.path().join("attempt-out.json"),
        )
        .expect("state-dir root file is caller-owned");
        let external = tempfile::tempdir().expect("external tempdir");
        validate_attempt_output_path(repo.path(), state.path(), &external.path().join("out.json"))
            .expect("external path is fine");
    }
}

#[cfg(test)]
mod attempt_output_path_hardening_tests {
    //! #166 review tur-3: symlink'li reserved store bypass'ı + --out temp clobber.
    use super::*;

    /// Platform symlink oluşturucu — Windows'ta ayrıcalık gerektirebilir;
    /// bu durumda test skip (CI ubuntu-latest'te tam koşar).
    fn try_symlink(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(src, dst)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(src, dst)
        }
    }

    #[test]
    fn validate_attempt_output_path_missing_parent_rejects_early() {
        // #182: navigator ÇALIŞMADAN önce parent-dizin preflight — canonicalizasyon
        // (symlink vb.) çalışmadan hızlı red; attempt'in pahalı adımı boşa girmez.
        let base = tempfile::tempdir().expect("base tempdir");
        let state = base.path().join("state");
        let repo = base.path().join("repo");
        std::fs::create_dir_all(&state).expect("mkdir state");
        std::fs::create_dir_all(&repo).expect("mkdir repo");

        let out = base.path().join("missing-dir").join("attempt-out.json");
        let err = validate_attempt_output_path(&repo, &state, &out)
            .expect_err("missing --out parent must reject early");
        assert!(
            err.to_string().contains("parent directory does not exist"),
            "message: {err}"
        );
    }

    #[test]
    fn preflight_out_parent_rejects_file_as_parent() {
        // Review P2 (#185): parent VAR ama normal dosya — exists() geçerdi, pahalı iş
        // sonrası "Not a directory" gelirdi; erken red.
        let base = tempfile::tempdir().expect("tempdir");
        let parent_as_file = base.path().join("parent-is-a-file");
        std::fs::write(&parent_as_file, b"not a directory").expect("file");

        let out = parent_as_file.join("result.json");
        let err = preflight_out_parent(&out, "--out").expect_err("file-as-parent must reject");
        assert!(
            err.to_string().contains("is not a directory"),
            "message: {err}"
        );

        // Negatif kontrol: gerçek dizin parent geçer.
        preflight_out_parent(&base.path().join("ok.json"), "--out")
            .expect("directory parent passes");
    }

    #[test]
    fn validate_attempt_output_path_resolves_symlinked_reserved_stores() {
        // P0 (tur-3): <state>/attempts veya <state>/.osp bir symlink ile başka
        // yere yönlendirilirse --out o GERÇEK hedefe düşse bile reddedilmelidir —
        // reserved root'un kendisi canonicalize edilerek karşılaştırılır.
        let base = tempfile::tempdir().expect("base tempdir");
        let state = base.path().join("state");
        let repo = base.path().join("repo");
        std::fs::create_dir_all(&state).expect("mkdir state");
        std::fs::create_dir_all(&repo).expect("mkdir repo");

        for reserved in ["attempts", ".osp"] {
            let real = base.path().join(format!("real-{reserved}"));
            std::fs::create_dir_all(&real).expect("mkdir real store");
            let link = state.join(reserved);
            if let Err(e) = try_symlink(&real, &link) {
                eprintln!(
                    "skipped (symlink unsupported here: {e}) — CI ubuntu-latest covers this path"
                );
                return;
            }
            let target = link.join("task-7-1.json");
            let err = validate_attempt_output_path(&repo, &state, &target)
                .expect_err("symlinked reserved store must be rejected");
            assert!(
                err.to_string().contains("canonical state store"),
                "message for {reserved}: {err}"
            );
        }

        // Negatif kontrol: symlink'li store DIŞINDAKI bir path serbest kalmalı.
        let external = base.path().join("caller-out.json");
        validate_attempt_output_path(&repo, &state, &external)
            .expect("external caller-owned path stays allowed");
    }

    #[test]
    fn out_copy_temp_never_touches_caller_files() {
        // P1 (tur-3): unique temp — eski sabit ad şemasına (`report.json.tmp-osp`)
        // uyan caller dosyası DOKUNULMAMALI; hedef doğru yazılmalı; temp artığı
        // kalmamalı; ikinci yazım overwrite etmek kasıtlı.
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("report.json");
        let decoy = dir.path().join("report.json.tmp-osp");
        std::fs::write(&decoy, b"caller data").expect("decoy");

        write_attempt_out_copy(&out, b"payload-one").expect("first copy");
        assert_eq!(std::fs::read(&out).unwrap(), b"payload-one");
        assert_eq!(
            std::fs::read(&decoy).unwrap(),
            b"caller data",
            "same-named caller file must NOT be truncated"
        );

        write_attempt_out_copy(&out, b"payload-two").expect("second copy (overwrite intended)");
        assert_eq!(std::fs::read(&out).unwrap(), b"payload-two");
        assert_eq!(std::fs::read(&decoy).unwrap(), b"caller data");

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".osp-tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "no temp leftovers: {leftovers:?}");
    }
}

#[cfg(test)]
mod staged_publish_tests {
    //! #172 tur-3 (P2): staged publish lifecycle — rename hatasında henüz
    //! publish edilmemiş temp'ler temizlenir (yalnız başarısız olanınki değil).
    use super::*;

    #[test]
    fn staged_publish_cleans_remaining_temps_on_rename_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = dir.path().join("first.json");
        // İkinci hedef: dolu bir DİZİN — rename(file → dir) her platformda başarısız.
        let target_is_dir = dir.path().join("target-is-dir");
        std::fs::create_dir_all(&target_is_dir).expect("mkdir");
        std::fs::write(target_is_dir.join("occupied"), b"x").expect("occupy");

        let err = atomic_write_replace_staged(&[(&first, b"first"), (&target_is_dir, b"second")])
            .expect_err("rename onto a directory must fail");
        assert!(format!("{err}").contains("rename failed"), "{err}");
        // Dokümante edilen davranış: önceki rename yayınlanmış kalabilir.
        assert_eq!(std::fs::read(&first).unwrap(), b"first");
        // Tur-3 P2: hiçbir staged temp artığı kalmaz.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".osp-tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no staged temp leftovers: {leftovers:?}"
        );
    }

    #[test]
    fn staged_publish_no_partial_set_on_second_prep_failure() {
        // #172 tur-2 (P2) garantiyi unit düzeyine taşır (#182 sonrası CLI'dan yok-dizin
        // PREP hatasına preflight'te red edilinerek erişilemez): İKİNCİ hedefin
        // HAZIRLIK hatasında ilkinin HİÇBİR baytı görünmez olur — publish aşamasına
        // hiç girilmez.
        let dir = tempfile::tempdir().expect("tempdir");
        let first = dir.path().join("task.json");
        let missing_parent = dir.path().join("no-such-dir").join("proposals.json");

        let err = atomic_write_replace_staged(&[(&first, b"task"), (&missing_parent, b"props")])
            .expect_err("second prep (temp create in missing parent) must fail");
        assert!(
            format!("{err}").contains("no-such-dir") || format!("{err}").contains("os error"),
            "prep hatası yol bilgisi taşımalı: {err}"
        );
        assert!(
            !first.exists(),
            "first output must NOT be visible when the second prep failed — \
             staged publish leaves no partial artifact set"
        );
    }
}
