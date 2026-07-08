//! `osp graph` altkomutları — Candidate-only seed bootstrap + store durumu.
//!
//! - `init`: CandidateSeedFile → restore-validasyon → write (existing → fail, --force overwrite).
//! - `status`: node/edge/ledger counts + audit_seq.
//! - `validate`: restore + invariant-validasyon (read-only).

use std::path::PathBuf;

use clap::Args;
use osp_core::anchoring::store::InMemoryAnchorStore;

use crate::seed_file::CandidateSeedFile;
use crate::store_io::{read_persisted_store, write_persisted_store, PersistedStore, StoreLock};

/// `osp graph init --seed <path> --store <path>` — Candidate-only bootstrap.
#[derive(Args, Debug)]
pub struct GraphInitArgs {
    /// Candidate seed JSON dosyası (nodes-only; status/id alanları yok).
    #[arg(long)]
    pub seed: PathBuf,
    /// Canonical store JSON dosyası (persisted AnchorStoreSnapshot).
    #[arg(long)]
    pub store: PathBuf,
    /// Mevcut store dosyasını overwrite et (default: fail).
    #[arg(long)]
    pub force: bool,
}

/// `osp graph status --store <path>` — store durumu.
#[derive(Args, Debug)]
pub struct GraphStatusArgs {
    #[arg(long)]
    pub store: PathBuf,
    /// JSON çıktı (CI/script).
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// `osp graph validate --store <path>` — restore + invariant-validasyon.
#[derive(Args, Debug)]
pub struct GraphValidateArgs {
    #[arg(long)]
    pub store: PathBuf,
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// `osp graph init` — Candidate seed → trusted store.
///
/// Aynı StoreLock protokolü kullanır (review mutation'ları ile tek concurrency sözleşmesi):
/// lock → existence/force check → build + validate → atomic write → unlock (Review P2.3).
/// İki init process'i aynı anda `exists == false` göremez; `--force` aktif review
/// mutation'ı ile yarışıp canonical store'u overwrite edemez.
pub fn run_graph_init(args: GraphInitArgs) -> anyhow::Result<()> {
    // (1) Exclusive lock — sabit .lock dosyası (review mutation'ları ile aynı).
    let _lock = StoreLock::acquire(&args.store)
        .map_err(|e| anyhow::anyhow!("cannot acquire store lock: {e}"))?;

    // (2) Existence/force check (lock altında).
    if args.store.exists() && !args.force {
        anyhow::bail!(
            "store file already exists at {}: use --force to overwrite",
            args.store.display()
        );
    }

    // (3) Seed parse.
    let seed_json = std::fs::read_to_string(&args.seed)
        .map_err(|e| anyhow::anyhow!("cannot read seed file {}: {e}", args.seed.display()))?;
    let seed_file = CandidateSeedFile::from_json(&seed_json)
        .map_err(|e| anyhow::anyhow!("invalid seed file: {e}"))?;
    let graph_seed = seed_file
        .to_graph_seed()
        .map_err(|e| anyhow::anyhow!("seed validation failed: {e}"))?;

    // (4) with_seed → export → restore-validasyon (corrupt seed'i baştan yakala).
    let store = InMemoryAnchorStore::with_seed(graph_seed);
    let snapshot = store.export_snapshot();
    InMemoryAnchorStore::restore_snapshot(snapshot.clone())
        .map_err(|e| anyhow::anyhow!("post-init restore validation failed (corrupt seed): {e}"))?;

    // (5) Atomic write (lock altında).
    let persisted = PersistedStore::from_snapshot(snapshot);
    write_persisted_store(&args.store, &persisted)
        .map_err(|e| anyhow::anyhow!("cannot write store: {e}"))?;

    // lock drop → release.
    println!(
        "✓ Graph initialized ({} candidate nodes)",
        persisted.snapshot.graph.nodes.len()
    );
    println!("  Store: {}", args.store.display());
    println!("  Revision: 0");
    Ok(())
}

/// `osp graph status` — node/edge/ledger counts + audit_seq.
pub fn run_graph_status(args: GraphStatusArgs) -> anyhow::Result<()> {
    let persisted =
        read_persisted_store(&args.store).map_err(|e| anyhow::anyhow!("cannot read store: {e}"))?;
    let node_count = persisted.snapshot.graph.nodes.len();
    let edge_count = persisted.snapshot.graph.edges.len();
    let candidate_count = persisted
        .snapshot
        .graph
        .nodes
        .iter()
        .filter(|n| n.decision_status == osp_core::anchoring::DecisionStatus::Candidate)
        .count();
    let accepted_count = persisted
        .snapshot
        .graph
        .nodes
        .iter()
        .filter(|n| n.decision_status == osp_core::anchoring::DecisionStatus::Accepted)
        .count();
    let superseded_count = persisted
        .snapshot
        .graph
        .nodes
        .iter()
        .filter(|n| n.decision_status == osp_core::anchoring::DecisionStatus::SupersededAccepted)
        .count();

    if args.format == "json" {
        let json = serde_json::json!({
            "store_schema_version": persisted.store_schema_version,
            "revision": persisted.revision,
            "node_count": node_count,
            "edge_count": edge_count,
            "candidates": candidate_count,
            "accepted": accepted_count,
            "superseded_accepted": superseded_count,
            "decision_records": persisted.snapshot.decision_records.len(),
            "supersede_records": persisted.snapshot.supersede_records.len(),
            "audit_sequence": persisted.snapshot.audit_sequence,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("✓ Store status: {}", args.store.display());
        println!("  Revision: {}", persisted.revision);
        println!("  Nodes: {node_count} (candidates: {candidate_count}, accepted: {accepted_count}, superseded: {superseded_count})");
        println!("  Edges: {edge_count}");
        println!(
            "  Decision records: {}",
            persisted.snapshot.decision_records.len()
        );
        println!(
            "  Supersede records: {}",
            persisted.snapshot.supersede_records.len()
        );
        println!("  Audit sequence: {}", persisted.snapshot.audit_sequence);
    }
    Ok(())
}

/// `osp graph validate` — restore + invariant-validasyon (read-only).
pub fn run_graph_validate(args: GraphValidateArgs) -> anyhow::Result<()> {
    let persisted =
        read_persisted_store(&args.store).map_err(|e| anyhow::anyhow!("cannot read store: {e}"))?;
    match InMemoryAnchorStore::restore_snapshot(persisted.snapshot.clone()) {
        Ok(_) => {
            if args.format == "json" {
                let json = serde_json::json!({
                    "valid": true,
                    "revision": persisted.revision,
                    "node_count": persisted.snapshot.graph.nodes.len(),
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("✓ Store valid: {}", args.store.display());
                println!("  Revision: {}", persisted.revision);
                println!(
                    "  All invariants pass (node uniqueness, edge endpoints, ledger/status integrity, dense audit_seq, C15 triangulation)"
                );
            }
            Ok(())
        }
        Err(e) => {
            if args.format == "json" {
                let json = serde_json::json!({
                    "valid": false,
                    "error": e.to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("✗ Store invalid: {e}");
            }
            std::process::exit(1);
        }
    }
}
