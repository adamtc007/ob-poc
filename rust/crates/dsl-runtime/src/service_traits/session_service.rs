//! Session lifecycle service.
//!
//! The 19 `session.*` verbs (`config/verbs/session.yaml`) manage the
//! `UnifiedSession` lifecycle: starting, loading entities at various
//! galaxy levels (universe, galaxy, cluster, system), filtering,
//! undo/redo, and binding contextual frames (client, persona,
//! structure, case, mandate, deal).
//!
//! `UnifiedSession` is the centerpiece of a 10934 LOC multi-consumer
//! session module that stays in ob-poc. This trait collapses dispatch
//! onto a single method — the consumer ops pass their YAML-bound
//! verb name + args as JSON, and the bridge does the actual work in
//! ob-poc against `crate::session::*`.
//!
//! Pending session state crosses turns through
//! `ctx.extensions["_pending_session"]` (mirroring the legacy
//! `ext_set_pending_session` helper).

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

#[async_trait]
pub trait SessionService: Send + Sync {
    async fn dispatch_session_verb(
        &self,
        pool: &PgPool,
        verb_name: &str,
        args: &serde_json::Value,
        extensions: &mut serde_json::Value,
    ) -> Result<serde_json::Value>;
}
