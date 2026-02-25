//! Storage port traits — implemented by sem_os_postgres.
//! Core logic depends only on these traits, never on sqlx directly.

use async_trait::async_trait;

use crate::{error::SemOsError, principal::Principal, types::*};

pub type Result<T> = std::result::Result<T, SemOsError>;

#[async_trait]
pub trait SnapshotStore: Send + Sync {
    async fn resolve(&self, fqn: &Fqn, as_of: Option<&SnapshotSetId>) -> Result<SnapshotRow>;
    async fn publish(&self, principal: &Principal, req: PublishInput) -> Result<SnapshotSetId>;
    async fn list_as_of(&self, as_of: &SnapshotSetId) -> Result<Vec<SnapshotSummary>>;
    async fn get_manifest(&self, id: &SnapshotSetId) -> Result<Manifest>;
    async fn export(&self, id: &SnapshotSetId) -> Result<Vec<SnapshotExport>>;

    /// Publish a snapshot into an existing snapshot set with full metadata.
    /// Used by changeset promotion to insert multiple snapshots atomically.
    /// The `correlation_id` ties the outbox event back to the promoting changeset.
    async fn publish_into_set(
        &self,
        meta: &SnapshotMeta,
        definition: &serde_json::Value,
        snapshot_set_id: uuid::Uuid,
        correlation_id: uuid::Uuid,
    ) -> Result<uuid::Uuid>;

    /// Publish a batch of snapshots into a snapshot set atomically.
    ///
    /// **v3.3 invariant:** All snapshot inserts + exactly ONE outbox event
    /// happen in a single database transaction. This ensures:
    /// - One publish ⇒ one outbox event ⇒ one projection job.
    /// - No partial writes (all-or-nothing).
    ///
    /// Returns the list of created snapshot_ids.
    async fn publish_batch_into_set(
        &self,
        items: Vec<(SnapshotMeta, serde_json::Value)>,
        snapshot_set_id: uuid::Uuid,
        correlation_id: uuid::Uuid,
    ) -> Result<Vec<uuid::Uuid>>;

    /// Find active snapshots whose JSONB definition references the given FQN.
    /// Used for impact analysis — finds downstream dependents of a changed object.
    /// Returns (snapshot_id, object_type, fqn) tuples.
    async fn find_dependents(&self, fqn: &str, limit: i64) -> Result<Vec<DependentSnapshot>>;
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn load_typed(&self, snapshot_id: &SnapshotId, fqn: &Fqn) -> Result<TypedObject>;
}

#[async_trait]
pub trait ChangesetStore: Send + Sync {
    /// Create a new changeset in 'draft' status.
    async fn create_changeset(&self, input: CreateChangesetInput) -> Result<Changeset>;

    /// Load a changeset by ID.
    async fn get_changeset(&self, changeset_id: uuid::Uuid) -> Result<Changeset>;

    /// List changesets with optional filters.
    async fn list_changesets(
        &self,
        status: Option<&str>,
        owner: Option<&str>,
        scope: Option<&str>,
    ) -> Result<Vec<Changeset>>;

    /// Update a changeset's status.
    async fn update_status(
        &self,
        changeset_id: uuid::Uuid,
        new_status: ChangesetStatus,
    ) -> Result<()>;

    /// Add an entry to a changeset (only if changeset is 'draft').
    async fn add_entry(
        &self,
        changeset_id: uuid::Uuid,
        input: AddChangesetEntryInput,
    ) -> Result<ChangesetEntry>;

    /// List all entries in a changeset.
    async fn list_entries(&self, changeset_id: uuid::Uuid) -> Result<Vec<ChangesetEntry>>;

    /// Submit a review on a changeset.
    async fn submit_review(
        &self,
        changeset_id: uuid::Uuid,
        input: SubmitReviewInput,
    ) -> Result<ChangesetReview>;

    /// List all reviews for a changeset.
    async fn list_reviews(&self, changeset_id: uuid::Uuid) -> Result<Vec<ChangesetReview>>;
}

#[async_trait]
pub trait AuditStore: Send + Sync {
    async fn append(&self, principal: &Principal, entry: AuditEntry) -> Result<()>;
}

#[async_trait]
pub trait OutboxStore: Send + Sync {
    /// Must be called inside the publish transaction.
    /// Atomicity is the caller's responsibility.
    async fn enqueue(&self, event: OutboxEvent) -> Result<()>;
    async fn claim_next(&self, claimer_id: &str) -> Result<Option<OutboxEvent>>;
    async fn mark_processed(&self, event_id: &EventId) -> Result<()>;

    /// Record a retryable failure — clears the claim so the event can be re-claimed.
    /// The event stays eligible for `claim_next()`.
    async fn record_failure(&self, event_id: &EventId, error: &str) -> Result<()>;

    /// Permanently dead-letter an event — sets `failed_at` so `claim_next()` skips it.
    async fn mark_dead_letter(&self, event_id: &EventId, error: &str) -> Result<()>;
}

#[async_trait]
pub trait EvidenceInstanceStore: Send + Sync {
    async fn record(&self, principal: &Principal, instance: EvidenceInstance) -> Result<()>;
}

#[async_trait]
pub trait ProjectionWriter: Send + Sync {
    /// Called by the outbox dispatcher ONLY. Never called by publish directly.
    /// SC-3 applied: takes full OutboxEvent (includes outbox_seq) so the writer
    /// can advance projection_watermark.last_outbox_seq atomically.
    async fn write_active_snapshot_set(&self, event: &OutboxEvent) -> Result<()>;
}
