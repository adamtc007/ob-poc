//! Governed phrase authoring (Phases 1-3 + batch).
//!
//! 9 `phrase.*` verbs covering observation, coverage, collision checks,
//! and the propose → review → approve/reject/defer lifecycle. The bridge
//! lives in ob-poc because it touches `crate::sem_reg::store::SnapshotStore`
//! + `crate::sem_reg::types::*` + `crate::sem_reg::ids::object_id_for`
//! and the embedding-similarity SQL on `verb_pattern_embeddings` /
//! `phrase_bank` / `session_traces`.
//!
//! Snapshot writes record `created_by = principal.actor_id`, so the
//! dispatch signature carries `&Principal` (mirroring
//! `StewardshipDispatch::dispatch`). All verbs return Record-shaped
//! payloads; the trait returns `serde_json::Value` and the consumer
//! wraps in `VerbExecutionOutcome::Record`.

use anyhow::Result;
use async_trait::async_trait;
use sem_os_core::principal::Principal;
use sqlx::PgPool;

#[async_trait]
pub trait PhraseService: Send + Sync {
    async fn dispatch_phrase_verb(
        &self,
        pool: &PgPool,
        verb_name: &str,
        args: &serde_json::Value,
        principal: &Principal,
    ) -> Result<serde_json::Value>;
}
