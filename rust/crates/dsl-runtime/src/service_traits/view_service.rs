//! Session view-state service.
//!
//! The 15 `view.*` verbs (`config/verbs/view.yaml`) build, refine, and
//! query the session's `ViewState` — a typed projection over the
//! taxonomy + selection + refinements that drives the UI viewport.
//!
//! In ob-poc the view state surface is large:
//!  - `crate::session::{ViewState, Refinement}` (lives in a 10934 LOC
//!    multi-consumer session module)
//!  - `crate::taxonomy::{Filter, Status, TaxonomyBuilder, TaxonomyContext}`
//!    (5345 LOC, multi-consumer)
//!
//! Rather than relocate either module or define a 15-method trait
//! that mirrors each verb, this trait collapses dispatch onto a
//! single method: the consumer ops pass their YAML-bound verb name +
//! args as JSON, and the bridge does the actual work in ob-poc. This
//! keeps the dsl-runtime side stateless and JSON-shaped (slice #9
//! lesson: when consumer ops wrap result as `Record(json)`, the trait
//! can return `Value` directly).
//!
//! Selection state crosses turns via `ctx.extensions["_selection"]`
//! (mirroring the legacy `ExecutionContext` selection API). The
//! bridge reads/writes that extension key directly.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

/// Single-method dispatch trait for the 15 `view.*` verbs. The
/// `verb_name` argument is the YAML-bound short name (e.g. `"universe"`,
/// `"book"`, `"refine"`, `"set-selection"`, `"zoom-in"`); the bridge
/// routes to the corresponding ob-poc handler.
///
/// Returns `Ok(Value)` shaped as `ViewOpResult` JSON. Selection state
/// is mutated in `extensions["_selection"]`.
#[async_trait]
pub trait ViewService: Send + Sync {
    async fn dispatch_view_verb(
        &self,
        pool: &PgPool,
        verb_name: &str,
        args: &serde_json::Value,
        extensions: &mut serde_json::Value,
    ) -> Result<serde_json::Value>;
}
