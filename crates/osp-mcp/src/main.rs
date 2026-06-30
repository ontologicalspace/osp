//! OSP MCP Server — binary entrypoint (`docs/mcp-design.md` §3).
//!
//! stdio transport, tokio async runtime. MCP client (Claude/Cursor) JSON-RPC over
//! stdio ile tool çağırır, server `osp-core` API'sine delegate eder.
//!
//! ```bash
//! # Agent mode (default — operator tools disabled)
//! osp-mcp --workspace P:/repos/osp-spike/svelte
//!
//! # Operator mode (operator tools enabled)
//! osp-mcp --mode operator --workspace P:/repos/osp-spike/svelte
//!
//! # SCIP index ile (gerçek LCOM4 cohesion)
//! osp-mcp --workspace P:/repos/x --scip P:/repos/x/index.scip
//! ```

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use osp_mcp::mode::ServerMode;
use osp_mcp::workspace::{Workspace, WorkspaceError};
use osp_mcp::OspMcpServer;
use rmcp::transport::stdio;
use rmcp::ServiceExt;

/// OSP MCP Server — agent access surface (INV-T1..T8 enforced).
#[derive(Parser, Debug)]
#[command(
    name = "osp-mcp",
    version,
    about = "Ontological Space Protocol — MCP server (agent access surface)"
)]
struct Cli {
    /// Server mode: agent (default) veya operator.
    #[arg(long, value_enum, default_value_t = ServerModeArg::Agent)]
    mode: ServerModeArg,
    /// Workspace path (repo root). Agent raw path VEREMEZ — startup'ta alınır.
    #[arg(long)]
    workspace: PathBuf,
    /// SCIP index path (opsiyonel — gerçek LCOM4 cohesion için).
    #[arg(long)]
    scip: Option<PathBuf>,
}

/// clap value enum (ServerMode'u CLI'da kullanılabilir yap).
#[derive(Debug, Clone, Copy, ValueEnum)]
enum ServerModeArg {
    Agent,
    Operator,
}

impl From<ServerModeArg> for ServerMode {
    fn from(arg: ServerModeArg) -> Self {
        match arg {
            ServerModeArg::Agent => ServerMode::Agent,
            ServerModeArg::Operator => ServerMode::Operator,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging — tracing. MCP stdio için stderr'e log (stdout JSON-RPC için ayrılmış).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init();

    let cli = Cli::parse();
    let mode: ServerMode = cli.mode.into();

    tracing::info!(
        workspace = ?cli.workspace,
        mode = mode.as_str(),
        "osp-mcp starting"
    );

    // 1. Workspace analyze (startup — analyze-once).
    let workspace = match Workspace::analyze(&cli.workspace, cli.scip.as_deref()) {
        Ok(ws) => ws,
        Err(WorkspaceError::PathNotFound(p)) => {
            anyhow::bail!("workspace path does not exist: {}", p.display());
        }
        Err(WorkspaceError::NotADirectory(p)) => {
            anyhow::bail!("workspace path is not a directory: {}", p.display());
        }
        Err(WorkspaceError::Analyze(e)) => {
            anyhow::bail!("workspace analyze failed: {e}");
        }
        Err(e) => anyhow::bail!("workspace error: {e}"),
    };
    tracing::info!(
        nodes = workspace.node_count,
        edges = workspace.edge_count,
        coverage = workspace.semantic_coverage.coverage_ratio,
        "workspace analyzed"
    );

    // 2. Server kur.
    let server = OspMcpServer::new(workspace, mode);
    tracing::info!(mode = mode.as_str(), "osp-mcp server ready (stdio)");

    // 3. stdio transport ile serve (rmcp ServiceExt).
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
