//! dsl-sage: Sage pack matcher + parameter extractor.
//!
//! # Overview
//!
//! Given a natural-language utterance and optional context, `match_packs`
//! returns a ranked list of [`RankedCandidate`] structs, each identifying
//! a decision pack and its confidence score.
//!
//! Once a pack is selected, [`extract_parameters`] proposes values for the
//! pack's declared parameters, and [`ConfirmationSession`] drives the
//! user through a confirm / edit / reject interaction before DSL emission.
//!
//! ## Two-layer matching pipeline
//!
//! 1. **Embedding retrieval** (always synchronous, no LLM):
//!    compute similarity between the utterance and each pack's example
//!    utterances; take the max per pack; keep top-K.
//!
//! 2. **LLM ranking** (optional async):
//!    pass the top-K pack summaries to an [`LlmClient`] for ranked
//!    re-ordering with rationale; combine with the embedding score.
//!
//! ## Accuracy baseline
//!
//! The [`BagOfWordsEmbedder`] (Jaccard similarity) achieves ~50–70% top-1
//! accuracy.  Plugging in the BGE-small-en-v1.5 model from `ob-semantic-matcher`
//! is expected to push that to ≥ 80%.  The evaluation harness in
//! `tests/pack_matching_eval.rs` targets 50% as the BoW baseline.
#![deny(unreachable_pub)]

pub mod audit;
pub mod confirmation;
pub mod context;
pub mod extractor;
pub mod instantiator;
pub mod matcher;
pub mod orchestrator;
pub mod repl;
pub mod types;

// Tranche 1
pub use context::context_from_session;
pub use matcher::{
    match_packs, match_packs_embedding_only, BagOfWordsEmbedder, LlmClient, LlmRankEntry,
    MockLlmClient, PackEmbedder, PackSummary,
};
// Tranche 2
pub use confirmation::{ConfirmationSession, ConfirmationState, ParameterEdit};
pub use extractor::{extract_parameters, HeuristicExtractor, LlmExtractor, PackSummaryWithParams};
pub use types::{
    ConfirmationRequest, ConfirmationResponse, ParameterProposal, RankedCandidate, SageContext,
};
// Tranche 3
pub use instantiator::{
    expand_template, instantiate, validate_instantiation, InstantiationResult, ValidationSummary,
};
// Tranche 4
pub use audit::{SageAuditEntry, SageAuditLog};
pub use orchestrator::{SageInput, SageOrchestrator, SageSession, SageState};
pub use repl::SageSessionStore;
