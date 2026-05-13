//! Sage ACP server.
//!
//! Speaks newline-delimited JSON-RPC 2.0 over stdio. Stdout is reserved
//! for protocol messages; diagnostics go to stderr.
//!
//! Phase 2.4 (2026-05-13) of the Sage ACP capability plan: relocated
//! from `rust/src/bin/ob_poc_acp.rs` into this crate. Phase 2.6
//! (2026-05-13) wires the planning loop — `session/prompt` requests
//! route through `ob_poc_agent::prompt_handler::try_handle_prompt`,
//! falling through to the boundary `AcpJsonRpcAgent` for every other
//! method (initialize, session/new, discovery / projection / KYC
//! dry-run surface).
//!
//! Configuration (all optional — sensible defaults so the spike
//! launches without any setup):
//! - `OBPOC_PACKS_DIR` — directory holding pack YAML manifests.
//!   Default: `rust/config/packs/` relative to the current working
//!   directory (the canonical location in the ob-poc repo).
//! - `SAGE_PACK_ID` — pack to anchor the session to. Default:
//!   `book-setup`.
//! - `ANTHROPIC_API_KEY` — if set, the planning loop calls Anthropic
//!   for one constrained-tool round-trip per prompt. If unset, the
//!   loop falls back to deterministic "first allowed verb" picking.
//!
//! The binary holds the planning loop across requests — index is
//! loaded once at startup. Phase 4 wires dirty-flag refresh.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use ob_agentic::anthropic_client::AnthropicClient;
use ob_agentic::llm_client::LlmClient;
use ob_poc_agent::index::{DiskPackIndexLoader, IndexLoadRequest, IndexLoader};
use ob_poc_agent::knowledge::{SemOsKnowledgeClient, StubKnowledgeClient};
use ob_poc_agent::planning::PlanningLoop;
use ob_poc_agent::prompt_handler::try_handle_prompt;
use ob_poc_agent::repl_channel::LocalParseChannel;
use ob_poc_boundary::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing, JsonRpcRequest};
use ob_poc_types::session::kinds::WorkspaceKind;

const DEFAULT_PACKS_DIR: &str = "rust/config/packs";
const DEFAULT_PACK_ID: &str = "book-setup";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let packs_dir: PathBuf = std::env::var("OBPOC_PACKS_DIR")
        .unwrap_or_else(|_| DEFAULT_PACKS_DIR.to_string())
        .into();
    let pack_id =
        std::env::var("SAGE_PACK_ID").unwrap_or_else(|_| DEFAULT_PACK_ID.to_string());

    let loader = DiskPackIndexLoader::new(&packs_dir);
    let request = IndexLoadRequest {
        workspace: WorkspaceKind::Cbu,
        pack_id: pack_id.clone(),
    };
    let index = loader.load(&request).await.map_err(|error| {
        anyhow::anyhow!(
            "failed to load pack '{pack_id}' from {}: {error}",
            packs_dir.display()
        )
    })?;
    eprintln!(
        "[sage-acp] Loaded pack '{}' ({} allowed verbs, {} forbidden) hash={}",
        index.pack.id,
        index.allowed_verbs().len(),
        index.forbidden_verbs().len(),
        index.pack_hash,
    );

    // Treat a missing-or-empty ANTHROPIC_API_KEY identically — an
    // empty env var is no API key. `AnthropicClient::from_env` only
    // checks for unset, so we filter empty values here so the planning
    // loop's deterministic fallback runs instead of 401-failing on
    // every prompt.
    let llm_client: Option<Arc<dyn LlmClient>> = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            let client = AnthropicClient::new(key);
            eprintln!(
                "[sage-acp] Anthropic client wired (model: {})",
                client.model_name()
            );
            Some(Arc::new(client))
        }
        _ => {
            eprintln!(
                "[sage-acp] ANTHROPIC_API_KEY not set or empty — planning loop will use \
                 deterministic fallback"
            );
            None
        }
    };

    let knowledge: Arc<dyn SemOsKnowledgeClient> =
        Arc::new(StubKnowledgeClient::with_label("phase-2-spike"));
    eprintln!(
        "[sage-acp] SemOS knowledge client wired (provider: {})",
        knowledge.provider_label()
    );

    let planning = PlanningLoop::new(index, llm_client, Some(knowledge));
    let channel = LocalParseChannel::new();
    let mut agent = AcpJsonRpcAgent::new();

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    eprintln!("[sage-acp] Server started");

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let outgoing = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => match try_handle_prompt(&request, &planning, &channel).await {
                Some(messages) => messages,
                None => agent.handle_request(request),
            },
            // Parse failures bubble through the boundary's
            // `handle_line` which emits a standard ParseError.
            Err(_) => agent.handle_line(&line),
        };

        for message in outgoing {
            let serialized = match message {
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
