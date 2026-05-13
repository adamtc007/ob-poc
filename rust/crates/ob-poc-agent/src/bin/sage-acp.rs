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
use ob_agentic::openai_client::OpenAiClient;
use ob_poc_agent::audit::{
    default_audit_path, default_otlp_endpoint, AuditPath, AuditSink, JsonlAuditSink,
    MultiAuditSink, NullAuditSink, OtlpAuditSink, OtlpEndpoint,
};
use ob_poc_agent::constellation::ConstellationHydrator;
use ob_poc_agent::goal_frame::GoalFrameStore;
use ob_poc_agent::goal_frame_handler::try_handle_goal_frame;
use ob_poc_agent::goal_proposal_trace::{GoalProposalTraceSink, LoggingTraceSink};
use ob_poc_agent::index::{DiskPackIndexLoader, IndexLoadRequest, IndexLoader};
use ob_poc_agent::knowledge::SemOsKnowledgeClient;
use ob_poc_agent::mcp_client::{
    InProcessTransport, McpConstellationHydrator, McpKnowledgeClient, McpTransport,
};
use ob_poc_agent::planning::PlanningLoop;
use ob_poc_agent::prompt_handler::try_handle_prompt;
use ob_poc_agent::repl_channel::LocalRunbookChannel;
use ob_poc_agent::runbook_handler::try_handle_runbook;
use ob_poc_boundary::acp_protocol::{AcpJsonRpcAgent, JsonRpcOutgoing, JsonRpcRequest};
use ob_poc_types::session::kinds::WorkspaceKind;
use sem_os_mcp::bridge::StubBridge;
use sem_os_mcp::server::McpServer;
use sem_os_mcp::tool_impls::build_registry;

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

    // BYOK provider selection — Phase 5.4 closure. Both Anthropic
    // and OpenAI are LlmClient impls in ob-agentic; the binary picks
    // whichever key is set. Anthropic wins when both are present
    // (matches the precedence the conformance harness uses).
    // `OBPOC_SAGE_LLM_PROVIDER` (`anthropic` | `openai`) overrides
    // the auto-pick so operators can force a provider when both
    // keys are exported. Empty values are treated as unset.
    let llm_client: Option<Arc<dyn LlmClient>> = pick_llm_client();

    // Phase 4.3 — knowledge + hydration now ride the sem_os_mcp
    // protocol surface. The MCP server is constructed in-process
    // with the StubBridge for the spike; production deployments
    // swap the bridge for a sem_os_client-backed impl without
    // touching this binary.
    let mcp_bridge = Arc::new(StubBridge::with_label("phase-4-spike"));
    let mcp_server = Arc::new(McpServer::new(build_registry(mcp_bridge.clone())));
    eprintln!(
        "[sage-acp] SemOS MCP server constructed (bridge: {}, {} tools)",
        sem_os_mcp::bridge::SemOsBridge::provider_label(mcp_bridge.as_ref()),
        mcp_server.registry().len(),
    );

    // §9 item 8 follow-up slice A — the in-process transport is the
    // CI-safe default; slice B introduces a subprocess transport
    // that swaps in here under env-var control.
    let transport: Arc<dyn McpTransport> = Arc::new(InProcessTransport::new(
        mcp_server.clone(),
        "sem_os_mcp@in-process",
    ));
    eprintln!(
        "[sage-acp] MCP transport wired (provider: {})",
        transport.provider_label()
    );

    let knowledge: Arc<dyn SemOsKnowledgeClient> = Arc::new(McpKnowledgeClient::new(
        transport.clone(),
        "sem_os_mcp@in-process",
    ));
    eprintln!(
        "[sage-acp] SemOS knowledge client wired (provider: {})",
        knowledge.provider_label()
    );

    let hydrator: Arc<dyn ConstellationHydrator> = Arc::new(McpConstellationHydrator::new(
        transport.clone(),
        "sem_os_mcp@in-process",
    ));
    eprintln!(
        "[sage-acp] Constellation hydrator wired (provider: {})",
        hydrator.provider_label()
    );

    let planning = PlanningLoop::new(index, llm_client, Some(knowledge), Some(hydrator));
    // Phase 4.4 — full LSP-shaped runbook lifecycle channel (per-URI
    // state, open/change/close/validateOnly/validateAndExecute).
    let channel = LocalRunbookChannel::new();

    let frames = GoalFrameStore::new();

    let traces: Arc<dyn GoalProposalTraceSink> =
        Arc::new(LoggingTraceSink::with_label("phase-3-spike"));
    eprintln!(
        "[sage-acp] GoalProposalTrace sink wired (provider: {})",
        traces.provider_label()
    );

    // Phase 5.3 — JSONL local sink + optional OTLP fan-out.
    // Operator switches:
    //   `OBPOC_SAGE_AUDIT=none`       — disable local JSONL sink
    //   `OBPOC_SAGE_AUDIT=<path>`     — override JSONL path
    //   `OBPOC_SAGE_OTLP_ENDPOINT=…`  — push to an OTLP collector
    let mut sinks: Vec<Box<dyn AuditSink>> = Vec::new();
    let mut labels: Vec<&'static str> = Vec::new();
    match default_audit_path() {
        AuditPath::Disabled => {
            eprintln!("[sage-acp] Local JSONL audit sink disabled (OBPOC_SAGE_AUDIT=none)");
        }
        AuditPath::File(path) => {
            eprintln!("[sage-acp] Local JSONL audit sink: {}", path.display());
            sinks.push(Box::new(JsonlAuditSink::new(path)));
            labels.push("jsonl");
        }
    }
    match default_otlp_endpoint() {
        OtlpEndpoint::Disabled => {
            eprintln!(
                "[sage-acp] OTLP audit exporter disabled (OBPOC_SAGE_OTLP_ENDPOINT unset)"
            );
        }
        OtlpEndpoint::Endpoint(url) => {
            eprintln!("[sage-acp] OTLP audit exporter: {url}");
            sinks.push(Box::new(OtlpAuditSink::new(url, "sage-acp")));
            labels.push("otlp");
        }
    }
    let audit: Arc<dyn AuditSink> = if sinks.is_empty() {
        eprintln!("[sage-acp] No audit sinks wired — audit records will be dropped");
        Arc::new(NullAuditSink)
    } else {
        let label = labels.join("+");
        eprintln!("[sage-acp] Audit fan-out: {label} ({} sinks)", sinks.len());
        Arc::new(MultiAuditSink::new(sinks).with_label(label))
    };

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
            Ok(request) => {
                // Dispatch order: prompt handler (planning loop) →
                // goal-frame lifecycle handlers → boundary fall-through
                // (discovery / projection / KYC dry-run surface).
                if let Some(messages) = try_handle_prompt(
                    &request,
                    &planning,
                    &channel,
                    audit.as_ref(),
                    &frames,
                    traces.as_ref(),
                )
                .await
                {
                    messages
                } else if let Some(messages) = try_handle_goal_frame(&request, &frames).await {
                    messages
                } else if let Some(messages) = try_handle_runbook(&request, &channel).await {
                    messages
                } else {
                    agent.handle_request(request)
                }
            }
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

/// Resolve which BYOK LLM client (if any) to wire into the
/// planning loop. Inspects three env vars:
///
/// - `OBPOC_SAGE_LLM_PROVIDER` (optional, `anthropic` | `openai`):
///   force a provider. The corresponding key MUST be set.
/// - `ANTHROPIC_API_KEY` / `OPENAI_API_KEY`: the keys themselves.
///   Empty values are treated as unset (matches sage-acp's existing
///   OTLP env-var discipline).
///
/// When `OBPOC_SAGE_LLM_PROVIDER` is unset, the picker prefers
/// Anthropic over OpenAI when both keys are set; returns `None`
/// (deterministic-fallback mode) when neither is set.
fn pick_llm_client() -> Option<Arc<dyn LlmClient>> {
    let forced = std::env::var("OBPOC_SAGE_LLM_PROVIDER")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty());

    let anthropic_key = nonempty_env("ANTHROPIC_API_KEY");
    let openai_key = nonempty_env("OPENAI_API_KEY");

    match forced.as_deref() {
        Some("anthropic") => match anthropic_key {
            Some(key) => Some(wire_anthropic(key)),
            None => {
                eprintln!(
                    "[sage-acp] OBPOC_SAGE_LLM_PROVIDER=anthropic but ANTHROPIC_API_KEY is not \
                     set or empty — falling back to deterministic mode"
                );
                None
            }
        },
        Some("openai") => match openai_key {
            Some(key) => Some(wire_openai(key)),
            None => {
                eprintln!(
                    "[sage-acp] OBPOC_SAGE_LLM_PROVIDER=openai but OPENAI_API_KEY is not set or \
                     empty — falling back to deterministic mode"
                );
                None
            }
        },
        Some(other) => {
            eprintln!(
                "[sage-acp] OBPOC_SAGE_LLM_PROVIDER='{other}' is not a known provider; falling \
                 back to deterministic mode"
            );
            None
        }
        None => match (anthropic_key, openai_key) {
            (Some(key), _) => Some(wire_anthropic(key)),
            (None, Some(key)) => Some(wire_openai(key)),
            (None, None) => {
                eprintln!(
                    "[sage-acp] Neither ANTHROPIC_API_KEY nor OPENAI_API_KEY set — planning loop \
                     will use deterministic fallback"
                );
                None
            }
        },
    }
}

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn wire_anthropic(key: String) -> Arc<dyn LlmClient> {
    let client = AnthropicClient::new(key);
    eprintln!(
        "[sage-acp] Anthropic client wired (model: {})",
        client.model_name()
    );
    Arc::new(client)
}

fn wire_openai(key: String) -> Arc<dyn LlmClient> {
    let client = OpenAiClient::new(key);
    eprintln!(
        "[sage-acp] OpenAI client wired (model: {})",
        client.model_name()
    );
    Arc::new(client)
}
