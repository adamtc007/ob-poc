//! ACP session-input draft-mode selection.
//!
//! Owned by `ReplOrchestratorV2`. The orchestrator reads the env vars
//! `OB_ACP_SESSION_INPUT_DRAFT_SOURCE` (primary) and
//! `OB_ACP_SESSION_INPUT_DRAFT_MODE` (fallback) once at construction time
//! via [`AcpSessionInputDraftMode::from_env`]. Per-request env reads were
//! removed in R8 (single-path unification).

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcpSessionInputDraftMode {
    #[default]
    Deterministic,
    LiveLlm,
}

impl AcpSessionInputDraftMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::LiveLlm => "llm_tool_call",
        }
    }

    pub fn can_run_for_task(self, task: &str) -> bool {
        match self {
            Self::Deterministic => true,
            // The live draft adapter is currently implemented for the KYC
            // language loop only. Other providers stay on deterministic ACP.
            Self::LiveLlm => task == "kyc-case.update-status",
        }
    }

    /// Read the orchestrator's draft mode from env once at construction.
    pub fn from_env() -> Self {
        std::env::var("OB_ACP_SESSION_INPUT_DRAFT_SOURCE")
            .ok()
            .or_else(|| std::env::var("OB_ACP_SESSION_INPUT_DRAFT_MODE").ok())
            .and_then(|value| value.parse().ok())
            .unwrap_or(Self::Deterministic)
    }
}

impl std::str::FromStr for AcpSessionInputDraftMode {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "deterministic" | "deterministic_draft" => Ok(Self::Deterministic),
            "llm" | "llm_tool_call" | "live_llm" => Ok(Self::LiveLlm),
            _ => Err(()),
        }
    }
}
