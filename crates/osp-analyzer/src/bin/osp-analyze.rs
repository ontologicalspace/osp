//! OSP Analyzer CLI — repo → AnalysisResult (real A/D + optional SCIP cohesion).
//!
//! Usage:
//!   osp-analyze [--scip <index.scip>] <repo-path> [repo2 repo3 ...]
//!
//! --scip: SCIP semantic index (Tier 2). Sağlanırsa gerçek LCOM4 cohesion.
//!         Yoksa cohesion placeholder (0.5, confidence=0.0).

use std::path::PathBuf;

use osp_analyzer::contract::AnalysisConfig;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?),
        )
        .init();

    let (scip_index, repos) = parse_args();

    if repos.is_empty() {
        eprintln!("Kullanım: osp-analyze [--scip <index.scip>] <repo-path> [repo2 ...]");
        std::process::exit(2);
    }

    let scip_note = match &scip_index {
        Some(p) => format!("SCIP: {} (real LCOM4 cohesion)", p.display()),
        None => "SCIP: yok (cohesion placeholder)".to_string(),
    };

    println!("\n=== OSP Analyzer v3 — Real Abstractness + SCIP Cohesion ===");
    println!("{}\n", scip_note);
    println!(
        "{:<16} {:>6} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "repo", "nodes", "edges", "κ", "A", "I", "D", "y"
    );
    println!("{}", "-".repeat(70));

    for repo_arg in &repos {
        let path = PathBuf::from(repo_arg);
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| repo_arg.clone());

        let config = AnalysisConfig {
            scip_index: scip_index.clone(),
            ..Default::default()
        };
        let registry = osp_analyzer::language::AdapterRegistry::default_all();

        match osp_analyzer::pipeline::analyze_repo_with_config(&path, &registry, &config) {
            Ok(result) => {
                let nodes = result.space.node_count();
                let edges = result.space.edge_count();
                let kappa = if nodes > 0 {
                    edges as f64 / nodes as f64
                } else {
                    0.0
                };
                let a = result.repo_metrics.abstractness.value;
                let d = result.repo_metrics.main_sequence_distance.value;
                let i = if nodes > 0 {
                    let total_i: f64 = result
                        .module_metrics
                        .values()
                        .map(|m| m.instability.value)
                        .sum();
                    total_i / nodes as f64
                } else {
                    0.5
                };
                let y = if nodes > 0 {
                    let total_y: f64 = result
                        .module_metrics
                        .values()
                        .map(|m| m.cohesion.value)
                        .sum();
                    total_y / nodes as f64
                } else {
                    0.5
                };
                let coverage = result.semantic_coverage.coverage_ratio;
                let y_display = if coverage > 0.0 {
                    format!("{:.2}", y)
                } else {
                    format!("{:.2}*", y) // * = placeholder
                };

                println!(
                    "{:<16} {:>6} {:>5} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6}",
                    name, nodes, edges, kappa, a, i, d, y_display
                );

                if coverage > 0.0 && coverage < 1.0 {
                    tracing::info!(
                        repo = %name,
                        scip_coverage = coverage,
                        "partial SCIP coverage — some modules use placeholder cohesion"
                    );
                }
            }
            Err(e) => {
                println!("{:<16} ERROR: {}", name, e);
            }
        }
    }
    println!();
    println!("A = abstractness (Tier 1). I = instability (Martin). D = |A+I-1|. y = cohesion.");
    println!("y* = placeholder (SCIP yok). y (no *) = gerçek LCOM4 (SCIP Tier 2).");
    println!();
    Ok(())
}

/// CLI arg parse: `--scip <path>` global flag + repo path list.
fn parse_args() -> (Option<PathBuf>, Vec<String>) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut scip_index: Option<PathBuf> = None;
    let mut repos: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scip" => {
                i += 1;
                if i < args.len() {
                    scip_index = Some(PathBuf::from(&args[i]));
                } else {
                    eprintln!("Hata: --scip bir dosya yolu gerektirir");
                    std::process::exit(2);
                }
            }
            "--help" | "-h" => {
                eprintln!("Kullanım: osp-analyze [--scip <index.scip>] <repo-path> [repo2 ...]");
                std::process::exit(0);
            }
            arg => {
                repos.push(arg.to_string());
            }
        }
        i += 1;
    }

    (scip_index, repos)
}
