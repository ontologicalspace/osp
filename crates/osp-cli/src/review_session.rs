//! Interactive review wizard — minimal operator session (stdio, yeni dependency yok).
//!
//! Argümansız `osp review` (veya `osp review session`) → operator oturumu açılır.
//! Generic `R: BufRead, W: Write` (Review 1#14) — production `stdin/stdout`, test `Cursor/Vec`.
//!
//! Her mutation `ReviewApplicationService::execute_mutation` çağırır (one-shot ile aynı
//! service — iki davranış oluşmaz). Gösterilen basis digest'ini taşır (Review 1#1):
//! reload sonrası yeni basis compile ETME; operator'ın gördüğü digest ile karar.
//!
//! v1 minimal: list/show/accept/reject/next/quit. v2: dialoguer/rustyline, fuzzy, renk.

use std::io::{BufRead, Write};

use osp_core::anchoring::review::{NodeDigest, OperatorId};
use osp_core::anchoring::types::ConceptNodeId;

use crate::application::repository::FileReviewStore;
use crate::application::review::{ReviewMutationCommand, ReviewQuery};
use crate::application::{ReviewApplicationService, ReviewReadOutput};
use crate::errors::ReviewError;

/// Interactive review session args.
#[derive(clap::Args, Debug)]
pub struct ReviewSessionArgs {
    #[arg(long, default_value = ".osp/anchor-store.json")]
    pub store: std::path::PathBuf,
    /// Operator kimliği (zorunlu). `$OSP_OPERATOR` fallback.
    #[arg(long)]
    pub operator: Option<String>,
}

/// Interactive wizard handler — production stdin/stdout.
pub fn run_review_session(args: ReviewSessionArgs) -> anyhow::Result<()> {
    let operator = resolve_operator(args.operator.clone())?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    run_interactive(
        &mut stdin.lock(),
        &mut stdout.lock(),
        &args.store,
        OperatorId::new(operator),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Operator kimliği: --operator > $OSP_OPERATOR > prompt (interactive'te sor).
fn resolve_operator(flag: Option<String>) -> Result<String, anyhow::Error> {
    if let Some(op) = flag {
        return Ok(op);
    }
    if let Ok(env_op) = std::env::var("OSP_OPERATOR") {
        if !env_op.trim().is_empty() {
            return Ok(env_op);
        }
    }
    Err(anyhow::anyhow!(
        "Operator identity is required. Provide --operator <id> or set OSP_OPERATOR env var."
    ))
}

/// Generic interactive loop. `R`/`W` test edilebilir I/O (Review 1#14).
pub fn run_interactive<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    store_path: &std::path::Path,
    operator: OperatorId,
) -> Result<(), ReviewError> {
    let repo = FileReviewStore::new(store_path);
    let service = ReviewApplicationService::new(repo);

    writeln!(
        output,
        "OSP review session — operator: {}",
        operator.as_str()
    )
    .ok();
    writeln!(
        output,
        "Commands: list, show <id>, accept <id>, reject <id>, quit"
    )
    .ok();
    writeln!(output).ok();

    loop {
        // Her döngüde candidate sayısını göster.
        match service.execute_query(ReviewQuery::List) {
            Ok(ReviewReadOutput::List { items, revision: _ }) => {
                writeln!(output, "{} candidates awaiting review.", items.len()).ok();
            }
            Ok(_) => {}
            Err(e) => {
                writeln!(output, "✗ cannot list candidates: {e}").ok();
            }
        }
        write!(output, "> ").ok();
        output.flush().ok();

        let mut line = String::new();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            break; // EOF
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

        match cmd {
            "quit" | "q" | "exit" => {
                writeln!(output, "Session ended.").ok();
                break;
            }
            "list" | "l" => match service.execute_query(ReviewQuery::List) {
                Ok(ReviewReadOutput::List { items, revision }) => {
                    for item in &items {
                        writeln!(output, "  {}  {}  [{}]", item.id, item.canonical, item.kind).ok();
                    }
                    writeln!(output, "  Revision: {revision}").ok();
                }
                _ => {}
            },
            "show" | "s" => {
                let id = match parts.next() {
                    Some(id) => id,
                    None => {
                        writeln!(output, "Usage: show <id>").ok();
                        continue;
                    }
                };
                match service.execute_query(ReviewQuery::Show(ConceptNodeId(id.into()))) {
                    Ok(ReviewReadOutput::Show { node: Some(n), .. }) => {
                        writeln!(
                            output,
                            "  {} — {} [{}] status={}",
                            n.id, n.canonical, n.kind, n.decision_status
                        )
                        .ok();
                        if let Some(hex) = &n.basis_digest_hex {
                            writeln!(output, "  digest: {hex}").ok();
                        }
                    }
                    Ok(_) => {
                        writeln!(output, "✗ node not found: {id}").ok();
                    }
                    Err(e) => {
                        writeln!(output, "✗ {e}").ok();
                    }
                }
            }
            "accept" | "a" | "reject" | "r" => {
                let accept = cmd == "accept" || cmd == "a";
                let id = match parts.next() {
                    Some(id) => id.to_string(),
                    None => {
                        writeln!(output, "Usage: {} <id>", cmd).ok();
                        continue;
                    }
                };
                // Reason prompt.
                write!(output, "Reason: ").ok();
                output.flush().ok();
                let mut reason = String::new();
                if input.read_line(&mut reason).unwrap_or(0) == 0 {
                    break;
                }
                let reason = reason.trim().to_string();
                if reason.is_empty() {
                    writeln!(output, "✗ reason cannot be empty").ok();
                    continue;
                }
                // Basis'i göster + digest al (lock öncesi show, sonra mutation).
                let digest = match get_basis_digest_for(&service, &id, output) {
                    Some(d) => d,
                    None => continue,
                };
                let command = if accept {
                    ReviewMutationCommand::Accept {
                        id: ConceptNodeId(id.clone()),
                        expected_basis_digest: digest,
                        reason,
                    }
                } else {
                    ReviewMutationCommand::Reject {
                        id: ConceptNodeId(id.clone()),
                        expected_basis_digest: digest,
                        reason,
                    }
                };
                match service.execute_mutation(command, operator.clone()) {
                    Ok(out) => {
                        writeln!(
                            output,
                            "✓ {} {} (record #{}, revision {})",
                            if accept { "Accepted" } else { "Rejected" },
                            out.mutation.node_id,
                            out.mutation.decision_sequence,
                            out.revision
                        )
                        .ok();
                    }
                    Err(ReviewError::StaleBasis) => {
                        writeln!(output, "✗ stale basis: node changed since you viewed it").ok();
                    }
                    Err(e) => {
                        writeln!(output, "✗ {e}").ok();
                    }
                }
            }
            other => {
                writeln!(
                    output,
                    "✗ unknown command: {other} (list/show/accept/reject/quit)"
                )
                .ok();
            }
        }
        writeln!(output).ok();
    }
    Ok(())
}

/// Node'un basis digest'ini göster ve döner (Candidate değilse None).
fn get_basis_digest_for<W: Write>(
    service: &ReviewApplicationService<FileReviewStore>,
    id: &str,
    output: &mut W,
) -> Option<NodeDigest> {
    match service.execute_query(ReviewQuery::Show(ConceptNodeId(id.into()))) {
        Ok(ReviewReadOutput::Show { node: Some(n), .. }) => {
            let hex = n.basis_digest_hex.as_deref()?;
            writeln!(output, "  Basis: {} (digest {hex})", n.canonical).ok();
            Some(NodeDigest::from_raw(
                u64::from_str_radix(hex, 16).expect("valid hex from show"),
            ))
        }
        _ => {
            writeln!(output, "✗ node not found or not Candidate: {id}").ok();
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    fn setup_store(dir: &std::path::Path, id: &str) -> std::path::PathBuf {
        use crate::store_io::{write_persisted_store, PersistedStore};
        use osp_core::anchoring::store::InMemoryAnchorStore;
        use osp_core::anchoring::types::{ConceptNode, ConceptNodeId, ConceptNodeKind, GraphSeed};
        use osp_core::anchoring::{DecisionStatus, PositionFamily};

        let node = ConceptNode {
            id: ConceptNodeId(id.into()),
            canonical: id.split(':').nth(1).unwrap_or(id).into(),
            aliases: vec![],
            node_kind: ConceptNodeKind::RuleCandidate,
            decision_status: DecisionStatus::Candidate,
            position_family: PositionFamily::ConceptualIntent,
        };
        let mut seed = GraphSeed::default();
        seed.rule_candidates.push(node);
        let store = InMemoryAnchorStore::with_seed(seed);
        let path = dir.join("store.json");
        write_persisted_store(
            &path,
            &PersistedStore::from_snapshot(store.export_snapshot()),
        )
        .unwrap();
        path
    }

    /// Interactive: accept candidate via piped input → Accepted.
    #[test]
    fn interactive_accept_candidate() {
        let dir = tempdir().unwrap();
        let path = setup_store(dir.path(), "RuleCandidate:X");
        let input = Cursor::new(b"accept RuleCandidate:X\ngood rule\nquit\n");
        let mut output = Vec::new();
        run_interactive(
            &mut std::io::BufReader::new(input),
            &mut output,
            &path,
            OperatorId::new("test-op"),
        )
        .unwrap();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("Accepted"), "output: {out}");
    }

    /// Interactive: unknown command handled gracefully.
    #[test]
    fn interactive_unknown_command_handled() {
        let dir = tempdir().unwrap();
        let path = setup_store(dir.path(), "RuleCandidate:X");
        let input = Cursor::new(b"bogus\nquit\n");
        let mut output = Vec::new();
        run_interactive(
            &mut std::io::BufReader::new(input),
            &mut output,
            &path,
            OperatorId::new("op"),
        )
        .unwrap();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("unknown command"), "output: {out}");
    }

    /// Interactive: empty reason rejected.
    #[test]
    fn interactive_empty_reason_rejected() {
        let dir = tempdir().unwrap();
        let path = setup_store(dir.path(), "RuleCandidate:X");
        let input = Cursor::new(b"accept RuleCandidate:X\n\nquit\n");
        let mut output = Vec::new();
        run_interactive(
            &mut std::io::BufReader::new(input),
            &mut output,
            &path,
            OperatorId::new("op"),
        )
        .unwrap();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("reason cannot be empty"), "output: {out}");
    }
}
