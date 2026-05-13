//! Sage ACP server.
//!
//! Speaks newline-delimited JSON-RPC 2.0 over stdio. Stdout is reserved
//! for protocol messages; diagnostics go to stderr.
//!
//! Phase 2.4 (2026-05-13) of the Sage ACP capability plan: relocated
//! from `rust/src/bin/ob_poc_acp.rs` into this crate. The binary now
//! lives behind a charter-clean dep wall — no `ob-poc` edge in the
//! transitive tree (`cargo tree -p ob-poc-agent | grep ob-poc` is
//! empty). Engine wiring (LLM client, ValidVerbSetEngine impl,
//! SageEngine impl, runbook channel client, MCP client) will be
//! injected by future slices.

use std::io::{BufRead, Write};

use ob_poc_boundary::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing};

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut agent = AcpJsonRpcAgent::new();

    eprintln!("[sage-acp] Server started");

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        for outgoing in agent.handle_line(&line) {
            let serialized = match outgoing {
                JsonRpcOutgoing::Response(response) => serde_json::to_string(&response)?,
                JsonRpcOutgoing::Notification(notification) => {
                    serde_json::to_string(&notification)?
                }
            };
            writeln!(stdout, "{serialized}")?;
            stdout.flush()?;
        }
    }

    eprintln!("[sage-acp] Server stopped");
    Ok(())
}
