//! Node-level witness diagnostic — repo'nun git history'sinden per-file
//! witness distribution'ı gösterir. "battle-tested vs speculative" dağılımı.
//!
//! Kullanım: `cargo run --example witness_breakdown -- <repo-path>`
//! Örn: `cargo run --example witness_breakdown -- P:/repos/osp-spike/svelte`

use std::path::Path;

use osp_analyzer::witness::extract_witness;

fn main() -> anyhow::Result<()> {
    let repo = std::env::args()
        .nth(1)
        .expect("usage: witness_breakdown <repo-path>");
    let repo = Path::new(&repo);

    println!("Extracting witness from {} ...", repo.display());
    let profile = extract_witness(repo);

    if !profile.repo_has_git {
        println!("\n⚠ Not a git repo (no .git) — WitnessProfile::no_git().");
        println!("  node_witnesses would be empty; inspector shows 'no witness data'.");
        return Ok(());
    }

    let head = profile.head_commit.as_deref().unwrap_or("?");
    let total_files = profile.by_file.len();
    println!(
        "\n=== Node-level witness: {} (HEAD {}, {} tracked files) ===",
        repo.display(),
        head,
        total_files
    );

    if total_files == 0 {
        println!("  (no tracked files matched source extensions — empty repo?)");
        return Ok(());
    }

    // Distribution metrics
    let commits: Vec<usize> = profile
        .by_file
        .values()
        .map(|w| w.commits_touching)
        .collect();
    let authors: Vec<usize> = profile
        .by_file
        .values()
        .map(|w| w.distinct_authors)
        .collect();
    let churn: Vec<u64> = profile.by_file.values().map(|w| w.churn).collect();
    let ownership: Vec<f64> = profile
        .by_file
        .values()
        .map(|w| w.ownership_concentration)
        .collect();
    let days: Vec<u32> = profile
        .by_file
        .values()
        .map(|w| w.last_modified_days_ago)
        .collect();

    let sum_commits: usize = commits.iter().sum();
    let sum_churn: u64 = churn.iter().sum();
    let max_commits = commits.iter().max().copied().unwrap_or(0);
    let mean_commits = sum_commits as f64 / total_files as f64;
    let mean_authors = authors.iter().sum::<usize>() as f64 / total_files as f64;
    let mean_ownership = ownership.iter().sum::<f64>() / total_files as f64;
    let mean_days = days.iter().sum::<u32>() as f64 / total_files as f64;

    println!("\n--- Distribution ---");
    println!(
        "  commits touching : mean {:.1}, max {}",
        mean_commits, max_commits
    );
    println!("  distinct authors : mean {:.2}", mean_authors);
    println!(
        "  ownership conc.  : mean {:.2} (1.0=solo, 0=shared)",
        mean_ownership
    );
    println!(
        "  churn (hist)     : total {}, mean {:.0}",
        sum_churn,
        sum_churn as f64 / total_files as f64
    );
    println!("  last modified    : mean {:.0} days ago", mean_days);

    // Stability classification (frontend renderWitness ile aynı eşikler)
    let battle_tested = profile
        .by_file
        .values()
        .filter(|w| {
            w.commits_touching >= 20
                && w.last_modified_days_ago >= 90
                && w.ownership_concentration < 0.6
        })
        .count();
    let speculative = profile
        .by_file
        .values()
        .filter(|w| w.commits_touching <= 2)
        .count();
    let established = total_files - battle_tested - speculative;

    println!("\n--- Stability classification ---");
    let pct = |n: usize| 100.0 * n as f64 / total_files as f64;
    println!(
        "  battle-tested : {:4} ({:.1}%) — >=20 commits, >=90 days old, shared ownership",
        battle_tested,
        pct(battle_tested)
    );
    println!(
        "  established   : {:4} ({:.1}%)",
        established,
        pct(established)
    );
    println!(
        "  speculative   : {:4} ({:.1}%) — <=2 commits (new/unfamiliar)",
        speculative,
        pct(speculative)
    );

    // Top-10 churn (en çok değişen dosyalar — volatility hotspot'ları)
    let mut by_churn: Vec<_> = profile.by_file.iter().collect();
    by_churn.sort_by(|a, b| b.1.churn.cmp(&a.1.churn));
    println!("\n--- Top-10 churn (volatility hotspots) ---");
    for (path, w) in by_churn.iter().take(10) {
        println!(
            "  {:>6} churn · {} commits · {} authors · {:.0}% solo · {}d ago · {}",
            w.churn,
            w.commits_touching,
            w.distinct_authors,
            w.ownership_concentration * 100.0,
            w.last_modified_days_ago,
            path
        );
    }

    // Top-10 solo-owned (bus-factor riski)
    let mut by_solo: Vec<_> = profile
        .by_file
        .iter()
        .filter(|(_, w)| w.commits_touching >= 5) // sadece meaningful history
        .collect();
    by_solo.sort_by(|a, b| {
        b.1.ownership_concentration
            .partial_cmp(&a.1.ownership_concentration)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!("\n--- Top-10 solo-owned (bus-factor risk, >=5 commits) ---");
    for (path, w) in by_solo.iter().take(10) {
        println!(
            "  {:.0}% solo · {} commits · 1 author · {}d ago · {}",
            w.ownership_concentration * 100.0,
            w.commits_touching,
            w.last_modified_days_ago,
            path
        );
    }

    Ok(())
}
