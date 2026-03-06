//! Sage — intent understanding layer for the utterance→DSL pipeline.
//!
//! The Sage understands WHAT the user wants (intent) without ever resolving
//! HOW to do it (verb FQNs, DSL assembly). That is the Coder's job.
//!
//! ## Architecture
//!
//! ```text
//! User utterance (raw text)
//!      │
//!      ▼  Stage 1.5 — BEFORE entity linking (E-SAGE-1)
//! ┌─────────────────────────────────────────────────────┐
//! │  SageEngine::classify(utterance, SageContext)        │
//! │  ┌───────────────────────────────────────────────┐  │
//! │  │ pre_classify() — deterministic, no LLM         │  │
//! │  │   1. ObservationPlane from session context    │  │
//! │  │   2. IntentPolarity from clue words           │  │
//! │  │   3. Domain hints from NounIndex              │  │
//! │  └───────────────────────────────────────────────┘  │
//! │  → OutcomeIntent (plane, polarity, domain, action)  │
//! └─────────────────────────────────────────────────────┘
//!      │
//!      ▼  Stage 3 — entity linking runs here (after Sage)
//! ```
//!
//! ## Invariants
//!
//! | ID | Invariant |
//! |----|-----------|
//! | E-SAGE-1 | Sage fires BEFORE entity linking (raw utterance, no UUID resolution) |
//! | E-SAGE-2 | Sage never sees verb FQNs (SageContext has no verb/fqn fields) |
//! | E-SAGE-3 | Coder never interprets NL (takes OutcomeIntent, not &str) |
//! | E-SAGE-4 | Shadow mode has zero production impact |
//! | E-SAGE-5 | `cargo check -p ob-poc` passes after every sub-phase |
//! | E-SAGE-6 | data_management_rewrite() unchanged until Sage accuracy exceeds it |

pub mod outcome;
pub mod plane;
pub mod polarity;

// Phase 1.2+
pub mod pre_classify;

// Phase 1.4
pub mod arg_assembly;
pub mod coder;
pub mod context;
pub mod deterministic;
pub mod llm_sage;
pub mod verb_index;
pub mod verb_resolve;

// Re-export core types for convenience
pub use arg_assembly::assemble_args_from_step;
pub use coder::{CoderEngine, CoderResolution, CoderResult};
pub use context::SageContext;
pub use deterministic::DeterministicSage;
pub use llm_sage::LlmSage;
pub use outcome::{
    Clarification, EntityRef, OutcomeAction, OutcomeIntent, OutcomeStep, SageConfidence,
};
pub use plane::ObservationPlane;
pub use polarity::IntentPolarity;
pub use pre_classify::SagePreClassification;
pub use verb_index::{runtime_registry_parity, VerbMeta, VerbMetadataIndex};
pub use verb_resolve::{ScoredVerbCandidate, StructuredVerbScorer};

// SageEngine trait
use anyhow::Result;

/// The Sage classifies user intent from raw utterance + session context.
///
/// ## Contract
/// - Never receives verb FQNs (E-SAGE-2)
/// - Always receives raw utterance text (not entity-resolved text) (E-SAGE-1)
/// - Always returns a valid OutcomeIntent (degrades to Low confidence stub on failure)
#[async_trait::async_trait]
pub trait SageEngine: Send + Sync {
    async fn classify(&self, utterance: &str, context: &SageContext) -> Result<OutcomeIntent>;
}
