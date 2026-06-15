//! Phase 5.4 — BYOK conformance harness.
//!
//! V&S R7 commits the Sage ACP runtime to producing the same
//! constrained-composition pick across any BYOK provider. The
//! harness loads the corpus at `tools/sage-conformance-corpus.yaml`,
//! constructs a `PlanningLoop` for each fixture against the real
//! `DiskPackIndexLoader` (the same loader the `sage-acp` binary
//! uses), and asserts the planning outcome matches the fixture's
//! expected verb FQN + draft source.
//!
//! ## Providers
//!
//! - `stub` (default) — deterministic, CI-safe. Returns canned
//!   responses for each fixture so the harness exercises the
//!   planning-loop wiring + pack resolution + constrained-
//!   composition guard without any API call. Equivalent to the
//!   `--llm none` path when `use_llm: false` on the fixture.
//! - `anthropic` — gated behind `ANTHROPIC_API_KEY`. Calls the
//!   real Anthropic model; conformance asserted today.
//! - `openai` — placeholder; awaits the OpenAI provider in
//!   `ob-agentic`. Returns a hard error until that lands.
//!
//! The harness fails on the first non-conforming fixture and
//! reports the diff. Pass count + provider label are printed on
//! success so CI logs are self-describing.

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use ob_agentic::llm_client::{LlmClient, ToolCallResult, ToolDefinition};
use ob_poc_agent::index::{DiskPackIndexLoader, IndexLoadRequest, IndexLoader};
use ob_poc_agent::planning::{DraftSource, PlanningLoop};
use ob_poc_types::session::kinds::WorkspaceKind;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

const CORPUS_PATH: &str = "tools/sage-conformance-corpus.yaml";
const PACKS_DIR: &str = "config/packs";

/// One conformance fixture loaded from the YAML corpus.
#[derive(Debug, Clone, Deserialize)]
struct Fixture {
    id: String,
    #[serde(default)]
    description: String,
    pack_id: String,
    workspace: String,
    utterance: String,
    expected_verb_fqn: String,
    expected_draft_source: String,
    /// When `false`, the planning loop runs without an LLM client
    /// and falls back to the deterministic first-allowed-verb
    /// picker. Defaults to `true`.
    #[serde(default = "default_use_llm")]
    use_llm: bool,
    /// Optional refused-drafts list. Threaded onto the goal frame
    /// before the planning loop runs so the deterministic /
    /// effective-allowlist code paths see the exclusions.
    #[serde(default)]
    refused_drafts: Vec<String>,
}

fn default_use_llm() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
struct Corpus {
    fixtures: Vec<Fixture>,
}

/// Provider selector — picked by `--provider`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Provider {
    Stub,
    Anthropic,
    OpenAi,
}

impl Provider {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "stub" => Ok(Self::Stub),
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAi),
            other => Err(anyhow!(
                "unknown --provider '{other}' (expected stub|anthropic|openai)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Stub => "stub",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
        }
    }
}

/// Run the conformance harness against the named provider.
pub(crate) fn run(provider: &str) -> Result<()> {
    let provider = Provider::from_str(provider)?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_async(provider))
}

async fn run_async(provider: Provider) -> Result<()> {
    let corpus = load_corpus(Path::new(CORPUS_PATH))?;
    let loader = DiskPackIndexLoader::new(PACKS_DIR);

    let mut passed = 0usize;
    for fixture in &corpus.fixtures {
        check_fixture(&loader, fixture, provider)
            .await
            .with_context(|| {
                format!(
                    "conformance failure: fixture '{}' ({provider})",
                    fixture.id,
                    provider = provider.label()
                )
            })?;
        passed += 1;
    }
    println!(
        "Sage BYOK conformance check clean: {passed}/{total} fixtures passed against provider '{}'",
        provider.label(),
        total = corpus.fixtures.len()
    );
    Ok(())
}

async fn check_fixture(
    loader: &DiskPackIndexLoader,
    fixture: &Fixture,
    provider: Provider,
) -> Result<()> {
    let workspace = parse_workspace(&fixture.workspace)?;
    let request = IndexLoadRequest {
        workspace,
        pack_id: fixture.pack_id.clone(),
    };
    let index = loader
        .load(&request)
        .await
        .map_err(|e| anyhow!("loading pack '{}' from {PACKS_DIR}: {e}", fixture.pack_id))?;

    let llm_client = if fixture.use_llm {
        Some(build_llm(provider, fixture)?)
    } else {
        None
    };

    let planning = PlanningLoop::new(index, llm_client, None, None);

    // Seed an explicit GoalFrame so we can preload refused drafts
    // for fixtures that exercise the refused-drafts code path.
    let mut frame =
        ob_poc_agent::goal_frame::GoalFrame::seed_for_spike(&fixture.utterance, planning.index());
    for refused in &fixture.refused_drafts {
        frame.record_refused_draft(refused.clone());
    }

    let outcome = planning
        .propose_draft(&fixture.utterance, Some(frame))
        .await
        .map_err(|e| anyhow!("planning loop returned error: {e}"))?;

    if outcome.verb_fqn != fixture.expected_verb_fqn {
        bail!(
            "verb FQN mismatch: expected '{}', got '{}'",
            fixture.expected_verb_fqn,
            outcome.verb_fqn
        );
    }
    let actual_source = match outcome.source {
        DraftSource::LlmTool => "llm_tool",
        DraftSource::DeterministicFallback => "deterministic_fallback",
    };
    if actual_source != fixture.expected_draft_source {
        bail!(
            "draft source mismatch: expected '{}', got '{}'",
            fixture.expected_draft_source,
            actual_source
        );
    }
    if !fixture.description.is_empty() {
        eprintln!("  ✓ {} — {}", fixture.id, fixture.description);
    } else {
        eprintln!("  ✓ {}", fixture.id);
    }
    Ok(())
}

fn build_llm(provider: Provider, fixture: &Fixture) -> Result<Arc<dyn LlmClient>> {
    match provider {
        Provider::Stub => Ok(Arc::new(StubLlm {
            verb_fqn: fixture.expected_verb_fqn.clone(),
        })),
        Provider::Anthropic => {
            let key = std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "Anthropic conformance run requires ANTHROPIC_API_KEY (set or non-empty); \
                         set it or rerun with --provider stub"
                    )
                })?;
            Ok(Arc::new(
                ob_agentic::anthropic_client::AnthropicClient::new(key),
            ))
        }
        Provider::OpenAi => {
            let key = std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "OpenAI conformance run requires OPENAI_API_KEY (set or non-empty); \
                         set it or rerun with --provider stub"
                    )
                })?;
            Ok(Arc::new(ob_agentic::openai_client::OpenAiClient::new(key)))
        }
    }
}

/// Deterministic stub — returns the fixture-specified verb in the
/// tool call schema, regardless of prompt content. The constrained-
/// composition guard in `PlanningLoop` still verifies the verb sits
/// inside the pack allowlist + effective surface, so a buggy
/// fixture (e.g. expected verb absent from the pack) still trips
/// the planning-loop refusal path rather than silently passing.
struct StubLlm {
    verb_fqn: String,
}

#[async_trait]
impl LlmClient for StubLlm {
    async fn chat(&self, _s: &str, _u: &str) -> Result<String> {
        Ok(String::new())
    }
    async fn chat_json(&self, _s: &str, _u: &str) -> Result<String> {
        Ok(String::new())
    }
    async fn chat_with_tool(
        &self,
        _s: &str,
        _u: &str,
        _t: &ToolDefinition,
    ) -> Result<ToolCallResult> {
        Ok(ToolCallResult {
            tool_name: "propose_verb".to_string(),
            arguments: serde_json::json!({
                "verb_fqn": self.verb_fqn,
                "intent_summary": "stub-canonical response per BYOK conformance corpus"
            }),
        })
    }
    fn model_name(&self) -> &str {
        "stub-conformance"
    }
    fn provider_name(&self) -> &str {
        "stub"
    }
}

fn load_corpus(path: &Path) -> Result<Corpus> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading conformance corpus at {}", path.display()))?;
    serde_yaml::from_str(&source)
        .with_context(|| format!("parsing conformance corpus YAML at {}", path.display()))
}

fn parse_workspace(label: &str) -> Result<WorkspaceKind> {
    match label.to_lowercase().as_str() {
        "cbu" => Ok(WorkspaceKind::Cbu),
        "kyc" => Ok(WorkspaceKind::Kyc),
        "deal" => Ok(WorkspaceKind::Deal),
        "instrument_matrix" | "instrument-matrix" => Ok(WorkspaceKind::InstrumentMatrix),
        "lifecycle_resources" | "lifecycle-resources" => Ok(WorkspaceKind::LifecycleResources),
        "product_maintenance" | "product-maintenance" => Ok(WorkspaceKind::ProductMaintenance),
        "semos_maintenance" | "sem-os-maintenance" | "semos-maintenance" => {
            Ok(WorkspaceKind::SemOsMaintenance)
        }
        "onboarding" | "onboarding_request" => Ok(WorkspaceKind::OnBoarding),
        "catalogue" => Ok(WorkspaceKind::Catalogue),
        "bpmn" | "bpmn_maintenance" | "workflow" | "orchestration" => Ok(WorkspaceKind::Bpmn),
        other => Err(anyhow!("unknown workspace label '{other}' in corpus")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parses_canonical_names() {
        assert!(matches!(
            Provider::from_str("stub").unwrap(),
            Provider::Stub
        ));
        assert!(matches!(
            Provider::from_str("Anthropic").unwrap(),
            Provider::Anthropic
        ));
        assert!(matches!(
            Provider::from_str("OPENAI").unwrap(),
            Provider::OpenAi
        ));
        assert!(Provider::from_str("gemini").is_err());
    }

    #[test]
    fn workspace_labels_round_trip() {
        for label in [
            "cbu",
            "kyc",
            "deal",
            "instrument-matrix",
            "booking-principal",
            "lifecycle-resources",
            "product-maintenance",
            "semos-maintenance",
            "onboarding",
            "catalogue",
        ] {
            parse_workspace(label)
                .unwrap_or_else(|_| panic!("workspace label '{label}' should parse"));
        }
    }

    #[test]
    fn corpus_parses_clean() {
        // Inline corpus mirrors the real one's shape; the on-disk
        // fixture set is verified by the harness end-to-end at run
        // time. This test guards the YAML schema.
        let inline = "
fixtures:
  - id: x
    description: y
    pack_id: book-setup
    workspace: cbu
    utterance: hi
    expected_verb_fqn: cbu.create
    expected_draft_source: llm_tool
";
        let corpus: Corpus = serde_yaml::from_str(inline).unwrap();
        assert_eq!(corpus.fixtures.len(), 1);
        assert_eq!(corpus.fixtures[0].id, "x");
        assert!(corpus.fixtures[0].use_llm, "default must be true");
    }

    #[test]
    fn use_llm_default_is_true_when_field_absent() {
        let fixture: Fixture = serde_yaml::from_str(
            "
id: x
description: y
pack_id: book-setup
workspace: cbu
utterance: hi
expected_verb_fqn: cbu.create
expected_draft_source: llm_tool
",
        )
        .unwrap();
        assert!(fixture.use_llm);
        assert!(fixture.refused_drafts.is_empty());
    }
}
