//! Vision breakdown diagnostic — #7 TypeSurface profile hardening doğrulaması.
//!
//! #1 fix sonrası TypeSurface reject'lerinin ne olduğunu ölçer. Review'in #5
//! "TypeSurface 8 reject hâlâ var, profile rafine edilmeli" şüphesi — bu şüphe
//! #1 bug'ından (tests/types/* → TypeSurface) kaynaklanıyordu. Fix sonrası
//! TypeSurface artık sadece production declaration dosyaları; reject'ler çok
//! azalmış olmalı.
//!
//! Frontend'in updateVisionPreview + evaluateNodeVerdict mantığının backend
//! mirror'ı. Her node için role-aware θ + VisionVerdict hesaplar, sonra:
//!   - verdict dağılımı (pass/warning/advisory/reject/inconclusive)
//!   - role × verdict kırılımı
//!   - axis × reject kırılımı
//!   - TypeSurface reject'lerinin örnek path'leri (eğer varsa)
//!
//! Usage: cargo run --release --example vision_breakdown -- <repo-path>

use std::collections::HashMap;
use std::path::Path;

use osp_analyzer::pipeline::analyze_repo;
use osp_core::space::NodeRole;

/// Backend builtin_role_override mirror (vision_config.rs ile birebir aynı).
fn builtin_role_vision(role: NodeRole) -> Option<(f64, f64, f64)> {
    match role {
        NodeRole::TypeSurface => Some((0.05, 0.80, 0.50)),
        NodeRole::Core => Some((0.60, 0.75, 0.20)),
        NodeRole::Adapter => Some((0.80, 0.50, 0.80)),
        NodeRole::Utility => Some((0.20, 0.60, 0.50)),
        NodeRole::Runtime => Some((0.40, 0.60, 0.35)),
        NodeRole::Support => None,
    }
}

/// Frontend evaluateNodeVerdict mirror — backend evaluate_node_vision ile aynı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Verdict {
    Pass,
    Warning,
    Advisory,
    Reject,
    Inconclusive,
}

fn evaluate(theta: f64, theta_bound: f64, theta_warn: f64, is_support: bool, inconclusive: bool) -> Verdict {
    if inconclusive {
        return Verdict::Inconclusive;
    }
    if theta <= theta_bound {
        return Verdict::Pass;
    }
    if theta <= theta_warn {
        return Verdict::Warning;
    }
    if is_support {
        Verdict::Advisory
    } else {
        Verdict::Reject
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: vision_breakdown <repo-path> [--bound 0.30]");
        std::process::exit(2);
    }
    let repo = Path::new(&args[1]);
    let theta_bound: f64 = args
        .iter()
        .position(|a| a == "--bound")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.30);
    let theta_warn = theta_bound + 0.10;

    let name = repo
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "?".to_string());

    let result = analyze_repo(repo)?;
    let total = result.space.nodes.len();
    println!("=== Vision breakdown: {} ({} nodes, θ_bound={}, θ_warn={}) ===\n", name, total, theta_bound, theta_warn);

    // Global vision (frontend slider default ≈ nötr 0.5; builtin override kazanır)
    let gx = 0.5_f64;
    let gy = 0.5;
    let gz = 0.5;

    let mut verdict_counts: HashMap<Verdict, usize> = HashMap::new();
    let mut role_verdict: HashMap<(NodeRole, Verdict), Vec<u64>> = HashMap::new();
    let mut axis_reject: HashMap<&str, Vec<u64>> = HashMap::from([
        ("coupling", vec![]),
        ("cohesion", vec![]),
        ("instability", vec![]),
    ]);
    let mut typesurface_rejects: Vec<(u64, String, f64, f64, f64, f64)> = vec![];

    for (id, node) in &result.space.nodes {
        let role = node.role;
        let (rvx, rvy, rvz) = builtin_role_vision(role).unwrap_or((gx, gy, gz));
        let coupling = node.position.raw.x;
        let cohesion = node.cohesion.unwrap_or(0.5);
        let instability = node.position.raw.z;
        let dx = coupling - rvx;
        let dy = cohesion - rvy;
        let dz = instability - rvz;
        let theta = (dx * dx + dy * dy + dz * dz).sqrt() / 3.0_f64.sqrt();

        // cohesion placeholder → inconclusive (frontend ile aynı)
        let inconclusive = node.cohesion.is_none();
        let is_support = role == NodeRole::Support;
        let v = evaluate(theta, theta_bound, theta_warn, is_support, inconclusive);

        *verdict_counts.entry(v).or_insert(0) += 1;
        role_verdict.entry((role, v)).or_default().push(*id);

        if v == Verdict::Reject {
            let abs_dx = dx.abs();
            let abs_dy = dy.abs();
            let abs_dz = dz.abs();
            let worst = if abs_dx >= abs_dy && abs_dx >= abs_dz {
                "coupling"
            } else if abs_dy >= abs_dz {
                "cohesion"
            } else {
                "instability"
            };
            axis_reject.entry(worst).or_default().push(*id);
            if role == NodeRole::TypeSurface {
                let path = result.node_paths.get(id).cloned().unwrap_or_default();
                typesurface_rejects.push((*id, path, theta, dx, dy, dz));
            }
        }
    }

    println!("── Verdict distribution ──");
    for v in [Verdict::Pass, Verdict::Warning, Verdict::Advisory, Verdict::Reject, Verdict::Inconclusive] {
        let c = *verdict_counts.get(&v).or(Some(&0)).unwrap();
        let pct = pct(c, total);
        let label = match v {
            Verdict::Pass => "pass",
            Verdict::Warning => "warning",
            Verdict::Advisory => "advisory",
            Verdict::Reject => "reject",
            Verdict::Inconclusive => "inconclusive",
        };
        println!("  {:<14} {:>5}  ({:>5}%)", label, c, pct);
    }

    println!("\n── Role × verdict breakdown ──");
    let roles = [
        NodeRole::TypeSurface,
        NodeRole::Core,
        NodeRole::Adapter,
        NodeRole::Utility,
        NodeRole::Runtime,
        NodeRole::Support,
    ];
    let verdicts = [Verdict::Pass, Verdict::Warning, Verdict::Advisory, Verdict::Reject, Verdict::Inconclusive];
    print!("  {:<12}", "");
    for v in verdicts {
        let s = match v {
            Verdict::Pass => "pass",
            Verdict::Warning => "warn",
            Verdict::Advisory => "adv",
            Verdict::Reject => "rej",
            Verdict::Inconclusive => "inc",
        };
        print!(" {:>6}", s);
    }
    println!();
    for role in roles {
        print!("  {:<12}", format!("{:?}", role));
        for v in verdicts {
            let c = role_verdict.get(&(role, v)).map(|x| x.len()).unwrap_or(0);
            print!(" {:>6}", c);
        }
        println!();
    }

    println!("\n── Reject by worst axis ──");
    for axis in ["coupling", "cohesion", "instability"] {
        let c = axis_reject.get(axis).map(|x| x.len()).unwrap_or(0);
        println!("  {:<12} {}", axis, c);
    }

    // #7 ana çıktı: TypeSurface reject'leri var mı?
    println!("\n── #7 TypeSurface reject diagnosis ──");
    if typesurface_rejects.is_empty() {
        println!("  ✓ TypeSurface reject YOK — #1 fix (tests/types/* → Support) işe yaradı.");
        println!("    Review'in 'TypeSurface 8 reject' şüphesi çürütüldü: o 8 node tests/types/*");
        println!("    dosyalarıydı, artık Support (advisory) olarak değerlendiriliyor.");
    } else {
        println!("  ⚠ {} TypeSurface reject var — production declaration dosyaları threshold'a takılıyor.", typesurface_rejects.len());
        println!("    Bu gerçek reject'ler (test değil). Profile rafine etmek için detaylar:");
        for (id, path, theta, dx, dy, dz) in typesurface_rejects.iter().take(15) {
            println!("    Node {}: θ={:.3} Δx={:+.2} Δy={:+.2} Δz={:+.2} {}", id, theta, dx, dy, dz, path);
        }
        if typesurface_rejects.len() > 15 {
            println!("    ... ({} more)", typesurface_rejects.len() - 15);
        }
    }

    Ok(())
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 / total as f64) * 100.0
    }
}
