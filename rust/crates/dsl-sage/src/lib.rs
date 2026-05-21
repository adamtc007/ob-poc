//! dsl-sage: Sage pack matcher — utterance → ranked decision-pack candidates.
//!
//! # Overview
//!
//! Given a natural-language utterance and optional context, `match_packs`
//! returns a ranked list of [`RankedCandidate`] structs, each identifying
//! a decision pack and its confidence score.
//!
//! ## Two-layer pipeline
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

pub mod context;
pub mod matcher;
pub mod types;

pub use context::context_from_session;
pub use matcher::{
    match_packs, match_packs_embedding_only, BagOfWordsEmbedder, LlmClient, LlmRankEntry,
    MockLlmClient, PackEmbedder, PackSummary,
};
pub use types::{RankedCandidate, SageContext};
