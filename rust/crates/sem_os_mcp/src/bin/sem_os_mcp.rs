//! sem_os_mcp server binary — Phase 4.2c.
//!
//! Speaks newline-delimited JSON-RPC 2.0 over stdio. Stdout is
//! reserved for protocol messages; diagnostics go to stderr.
//!
//! Today the binary wires the hermetic `StubBridge`. Phase 4.3
//! will swap in a `sem_os_client`-backed bridge so the tools
//! return real substrate data.

use std::io::{BufRead, Write};
use std::sync::Arc;

use sem_os_mcp::bridge::{SemOsBridge, StubBridge};
use sem_os_mcp::protocol::{JsonRpcRequest, JsonRpcResponse, PARSE_ERROR};
use sem_os_mcp::server::McpServer;
use sem_os_mcp::tool_impls::build_registry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let bridge: Arc<dyn SemOsBridge> = Arc::new(StubBridge::with_label("phase-4-spike"));
    eprintln!(
        "[sem_os_mcp] Bridge wired (provider: {})",
        bridge.provider_label()
    );

    let registry = build_registry(bridge);
    eprintln!("[sem_os_mcp] Tools registered: {}", registry.len());

    let server = McpServer::new(registry);

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    eprintln!("[sem_os_mcp] Server started");

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => server.handle_request(request).await,
            Err(error) => JsonRpcResponse::error(None, PARSE_ERROR, error.to_string()),
        };
        let serialised = serde_json::to_string(&response)?;
        writeln!(stdout, "{serialised}")?;
        stdout.flush()?;
    }

    eprintln!("[sem_os_mcp] Server stopped");
    Ok(())
}
