//! Server mode: Agent vs Operator (`docs/mcp-design.md` §5).
//!
//! **INV-T2 — OperatorCapability startup'ta inject edilir, MCP request'ten ASLA.**
//!
//! ```text
//! osp-mcp --mode agent     -> observation + validation + policy-bound execution
//! osp-mcp --mode operator  -> + trajectory_init, task_add, milestone_decompose, ...
//! ```
//!
//! Agent mode MCP client operator tool çağıramaz — tool registry mode-filtered.
//! Bu mod runtime'da flag ile seçilir, compile-time değil (operator tool'lar aynı
//! binary'de, sadece mode flag'i ile disable). Operator tool çağrısı yapılırsa
//! `OperatorCapabilityRequired` error döner.

use clap::ValueEnum;

/// Server modu — agent mı operator mü?
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ServerMode {
    /// Agent mode — observation + validation + policy-bound execution only.
    /// Operator tools (trajectory_init, task_add, ...) DISABLED.
    Agent,
    /// Operator mode — tüm tools aktif (insan/trusted orchestrator).
    Operator,
}

impl ServerMode {
    /// Bu mode operator tool'larına izin veriyor mu? (INV-T2 gate)
    pub fn allows_operator_tools(self) -> bool {
        matches!(self, ServerMode::Operator)
    }

    /// Insan-okur string (CLI output için).
    pub fn as_str(self) -> &'static str {
        match self {
            ServerMode::Agent => "agent",
            ServerMode::Operator => "operator",
        }
    }
}

impl Default for ServerMode {
    fn default() -> Self {
        // Default: agent (en güvenli — operator açık opt-in).
        ServerMode::Agent
    }
}
