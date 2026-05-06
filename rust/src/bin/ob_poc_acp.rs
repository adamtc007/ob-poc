//! ob-poc ACP server.
//!
//! Speaks newline-delimited JSON-RPC 2.0 over stdio. Stdout is reserved for
//! protocol messages; diagnostics go to stderr.

use std::io::{BufRead, Write};

use ob_poc::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing};

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut agent = AcpJsonRpcAgent::new();

    eprintln!("[ob_poc_acp] Server started");

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

    eprintln!("[ob_poc_acp] Server stopped");
    Ok(())
}
