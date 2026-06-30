//! # OSP MCP Server — Agent Access Surface (Aşama G1)
//!
//! `docs/mcp-design.md` §1 tezi: **MCP authority DEĞİL, access layer'dır.**
//!
//! ```text
//! MCP is not an authority layer.
//! MCP never computes architectural truth.
//! MCP never bypasses osp-core.
//! MCP never exposes target coordinates to agent-facing tools.
//! MCP never grants OperatorCapability from request input.
//! MCP exposes typed access to osp-core command handlers.
//! ```
//!
//! ## INV-T1..T8 protection
//! - **Agent-facing tools** (`analyze_workspace`, `get_agent_task_view`, `check_predicate`,
//!   `submit_delta`) `preferred_vector` / `target_region` / `milestone_target_vector`
//!   SERBEST DEĞİL — serde-level enforce (`AgentTaskView` bu alanları taşımaz).
//! - **OperatorCapability** startup'ta inject edilir (`--mode operator`), request'ten ASLA.
//! - **Workspace** startup config'inden (`--workspace`) alınır — agent raw `repo_path` veremez.
//!
//! ## Modüller
//! - [`envelope`] — Standart output envelope (`McpResult`) + deterministic error codes
//! - [`workspace`] — Startup workspace (analyze-once, INV security)
//! - [`server`] — rmcp `ServerHandler` + 4 tools
//! - [`mode`] — Agent vs Operator mode (INV-T2)

pub mod envelope;
pub mod mode;
pub mod server;
pub mod workspace;

pub use envelope::{EnvelopeError, ErrorCode, McpEnvelope};
pub use mode::ServerMode;
pub use server::OspMcpServer;
pub use workspace::{Workspace, WorkspaceError};
