//! Trading profile document load / save.
//!
//! `trading-profile.ca.*` verbs (and the rest of the trading-profile
//! intent tier) treat the matrix JSONB document on
//! `"ob-poc".cbu_trading_profiles` as the source of truth — they
//! mutate the in-memory `TradingMatrixDocument`, then persist it back
//! whole. Operational table writes (corporate actions tables, SSI
//! tables, etc.) happen later during `trading-profile.materialize`.
//!
//! `TradingMatrixDocument` already lives in `ob_poc_types::trading_matrix`
//! (boundary crate), so the trait can use it directly without any
//! types-extraction work. The persistence layer (`crate::trading_profile::ast_db`)
//! stays in ob-poc — this trait projects only the `load_document` /
//! `save_document` pair that consumer ops actually need; the richer
//! `ast_db` surface (`load_active_document`, `ensure_draft`,
//! `create_draft`, `clone_to_draft`, `mark_validated`, etc.) is for
//! lifecycle ops in ob-poc and stays local.
//!
//! Introduced in Phase 5a composite-blocker #11 for
//! `trading_profile_ca_ops`. The ob-poc bridge
//! (`ObPocTradingProfileDocument`) delegates to
//! `crate::trading_profile::ast_db::{load_document, save_document}`
//! and converts the internal `AstDbError` to `anyhow::Error` at the
//! plane boundary. Consumers obtain the impl via
//! [`crate::VerbExecutionContext::service::<dyn TradingProfileDocument>`].

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_types::trading_matrix::TradingMatrixDocument;
use uuid::Uuid;

/// Document load + save for a trading profile (matrix JSONB blob on
/// `cbu_trading_profiles`). Used by the corporate-actions intent
/// handlers to read-modify-write the document atomically.
#[async_trait]
pub trait TradingProfileDocument: Send + Sync {
    /// Fetch the full `TradingMatrixDocument` for `profile_id`.
    /// Errors when the profile does not exist or the JSONB blob
    /// fails to deserialize.
    async fn load_document(&self, profile_id: Uuid) -> Result<TradingMatrixDocument>;

    /// Replace the document blob for `profile_id` and update the
    /// document hash. Caller is responsible for ensuring the profile
    /// is in a writable state (DRAFT) before calling.
    async fn save_document(&self, profile_id: Uuid, doc: &TradingMatrixDocument) -> Result<()>;
}
