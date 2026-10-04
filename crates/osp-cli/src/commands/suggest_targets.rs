//! #172: `osp suggest-targets` — c-sıralı hedef aday tablosu (ölçümden).
//!
//! Run 14-16'da elle yapılan py taramasının yerine geçer: baseline'ın ölçülmüş
//! coupling değerlerinden sıralı aday listesi + `--exclude-past` ile ledger'daki
//! geçmiş hedeflerin otomatik hariç tutulması. **Yorum üretmez** — sıralama
//! ölçülmüş c değeridir; hedef seçimi insan kalır (issue #172 Sınır).
//!
//! K6: `--exclude-past` task_ref'leri CWD-göreli çömler (ritüel: OSP kökünden
//! çalıştırma). Eksik task dosyası fail-closed — sessiz atlama hariç listesini
//! dürüstsızlaştırır (bir hedef "yapıldı" sanılıp atlanırsa ölçüm kaybıdır).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::commands::analyze_provenance::CliAnalyzeNode;
use crate::commands::baseline::{analyze_live, load_baseline_artifact};
use crate::commands::repo_snapshot::RepositorySnapshot;

/// `osp suggest-targets` — c-sıralı aday tablosu.
#[derive(Args, Debug)]
pub struct SuggestTargetsArgs {
    /// Analiz edilecek repo path'i.
    #[arg(long)]
    pub repo: PathBuf,
    /// Baseline ölçüm artifact'ı. Verilirse HEAD exact-match fence; verilmezse canlı analyze.
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    /// Minimum ölçülmüş coupling (bu değerin altındakiler listelenmez).
    #[arg(long, default_value_t = 0.0)]
    pub min_coupling: f64,
    /// Geçmiş hedefleri hariç tutan ledger (`dogfood/ledger.jsonl`) — task_ref'ler
    /// okunur, hedef path'leri tablodan düşer.
    #[arg(long)]
    pub exclude_past: Option<PathBuf>,
    /// Listelenecek aday sayısı.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// `human` (tablo) veya `json` (stdout yalnız JSON).
    #[arg(long, value_enum, default_value_t = crate::commands::CliOutputFormat::Human)]
    pub format: crate::commands::CliOutputFormat,
}

pub fn run_suggest_targets(args: SuggestTargetsArgs) -> anyhow::Result<()> {
    let snapshot = RepositorySnapshot::capture(&args.repo).map_err(|e| anyhow::anyhow!(e))?;
    let live_head = snapshot.head.as_str().to_string();
    let (view, source) = match &args.baseline {
        Some(path) => (
            load_baseline_artifact(path, &live_head)?,
            "baseline-artifact",
        ),
        None => (analyze_live(&args.repo)?, "live-analysis"),
    };

    let past = match &args.exclude_past {
        Some(ledger) => past_targets(ledger)?,
        None => BTreeSet::new(),
    };

    // Ölçülmüş c'ye göre sırala (desc; eşitlikte path asc — deterministik).
    let mut ranked: Vec<&CliAnalyzeNode> = view
        .nodes
        .iter()
        .filter(|n| n.coupling.value >= args.min_coupling)
        .collect();
    ranked.sort_by(|a, b| {
        b.coupling
            .value
            .total_cmp(&a.coupling.value)
            .then_with(|| a.path.cmp(&b.path))
    });
    let excluded_now: Vec<&String> = ranked
        .iter()
        .map(|n| &n.path)
        .filter(|p| past.contains(p.as_str()))
        .collect();
    ranked.retain(|n| !past.contains(n.path.as_str()));
    let candidates: Vec<&&CliAnalyzeNode> = ranked.iter().take(args.limit).collect();

    match args.format {
        crate::commands::CliOutputFormat::Json => {
            let out = serde_json::json!({
                "repository_head": view.head,
                "source": source,
                "min_coupling": args.min_coupling,
                "total_nodes": view.nodes.len(),
                "past_targets_excluded": past.iter().collect::<Vec<_>>(),
                "candidates": candidates.iter().enumerate().map(|(i, n)| {
                    serde_json::json!({
                        "rank": i + 1,
                        "path": n.path,
                        "node_id": n.node_id,
                        "kind": kind_str(n),
                        "mass": n.mass,
                        "coupling": n.coupling.value,
                        "imports_out": view.imports_out_by_path.get(&n.path).copied().unwrap_or(0),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        crate::commands::CliOutputFormat::Human => {
            println!(
                "measured candidates at HEAD {} (source: {}, {} of {} nodes ≥ {}):",
                view.head,
                source,
                ranked.len(),
                view.nodes.len(),
                args.min_coupling
            );
            println!(
                "{:>4}  {:>9}  {:>4}  {:>7}  {:<10}  path",
                "#", "coupling", "out", "mass", "kind"
            );
            for (i, n) in candidates.iter().enumerate() {
                println!(
                    "{:>4}  {:>9.6}  {:>4}  {:>7.0}  {:<10}  {}",
                    i + 1,
                    n.coupling.value,
                    view.imports_out_by_path.get(&n.path).copied().unwrap_or(0),
                    n.mass,
                    kind_str(n),
                    n.path
                );
            }
            eprintln!(
                "{} past target(s) excluded via ledger ({} unique task refs seen)",
                excluded_now.len(),
                past.len()
            );
        }
    }
    Ok(())
}

/// Düğüm kind'i — wire adı (snake_case; `module`, `entity`, ...).
fn kind_str(node: &CliAnalyzeNode) -> String {
    serde_json::to_value(node.kind)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Ledger'dan geçmiş hedef path'leri (K6): her satırın task_ref'i CWD-göreli
/// yüklenir; v1 (expected_path) ve v2 (path) task zarflarının İKİSİ de okunur.
fn past_targets(ledger: &Path) -> anyhow::Result<BTreeSet<String>> {
    let raw = std::fs::read_to_string(ledger)
        .map_err(|e| anyhow::anyhow!("failed to read ledger {}: {e}", ledger.display()))?;
    let mut targets = BTreeSet::new();
    for (lineno, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: serde_json::Value = serde_json::from_str(line).map_err(|e| {
            anyhow::anyhow!(
                "ledger { }:{} does not parse: {e}",
                ledger.display(),
                lineno + 1
            )
        })?;
        let Some(task_ref) = row.get("task_ref").and_then(|v| v.as_str()) else {
            continue; // missing ≡ null (v1 geriye-dönük uyum)
        };
        targets.extend(task_scope_paths(Path::new(task_ref))?);
    }
    Ok(targets)
}

/// Task dosyasının scope path'leri — v1 `expected_path`, v2 `path`.
///
/// CWD-göreli çözümleme başarısızsa fail-closed: sessiz atlama, hariç listesini
/// eksik bırakır (aynı hedefin ikinci kez önerilmesi ölçüm kaybıdır).
fn task_scope_paths(task_path: &Path) -> anyhow::Result<Vec<String>> {
    let raw = std::fs::read_to_string(task_path).map_err(|_| {
        anyhow::anyhow!(
            "--exclude-past task_ref {:?} not found (resolved CWD-relative; run from \
             the OSP root where ledger refs are relative to)",
            task_path.display().to_string()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("task file {}: {e}", task_path.display()))?;
    let version = value.get("schema_version").and_then(|v| v.as_u64());
    let bindings = value
        .get("scope_bindings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let paths = match version {
        Some(1) => bindings
            .iter()
            .filter_map(|b| b.get("expected_path").and_then(|p| p.as_str()))
            .map(String::from)
            .collect(),
        Some(2) => bindings
            .iter()
            .filter_map(|b| b.get("path").and_then(|p| p.as_str()))
            .map(String::from)
            .collect(),
        other => {
            anyhow::bail!(
                "task file {} has unsupported schema_version {other:?}",
                task_path.display()
            )
        }
    };
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, content: &str) -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.keep().join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn ledger_v1_and_v2_task_refs_both_contribute_paths() {
        let v1 = write(
            "t1.json",
            r#"{"schema_version":1,"repository_head":"x","scope_bindings":[
                {"node_id":0,"expected_path":"src/old_v1.cs"}],"task":{}}"#,
        );
        let v2 = write(
            "t2.json",
            r#"{"schema_version":2,"repository_head":"x","scope_bindings":[
                {"path":"src/old_v2.cs"}],"task":{}}"#,
        );
        let ledger = write(
            "ledger.jsonl",
            &format!(
                "{{\"task_ref\": {v1:?}}}\n{{\"task_ref\": null}}\n{{\"task_ref\": {v2:?}}}\n",
                v1 = v1.display().to_string().replace('\\', "/"),
                v2 = v2.display().to_string().replace('\\', "/"),
            ),
        );
        let targets = past_targets(&ledger).unwrap();
        assert!(targets.contains("src/old_v1.cs"), "{targets:?}");
        assert!(targets.contains("src/old_v2.cs"), "{targets:?}");
        assert_eq!(targets.len(), 2, "null task_ref contributes nothing");
    }

    #[test]
    fn missing_task_ref_file_fails_closed() {
        let ledger = write("ledger.jsonl", r#"{"task_ref": "does/not/exist.json"}"#);
        let err = past_targets(&ledger).unwrap_err();
        assert!(format!("{err}").contains("CWD-relative"), "{err}");
    }

    #[test]
    fn malformed_ledger_line_fails_closed() {
        let ledger = write("ledger.jsonl", "not json\n");
        assert!(past_targets(&ledger).is_err());
    }
}
