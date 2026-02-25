//! sem_os_postgres — PostgreSQL implementations of sem_os_core port traits.
//!
//! Populated in Stage 1.3:
//! - PgSnapshotStore (from sem_reg/store.rs)
//! - PgObjectStore
//! - PgAuditStore
//! - PgOutboxStore (real SQL, not stub)
//! - PgEvidenceStore
//! - PgProjectionWriter (stub until S2.2)

pub mod sqlx_types;
pub mod store;

pub use store::{
    PgAuditStore, PgChangesetStore, PgEvidenceStore, PgObjectStore, PgOutboxStore,
    PgProjectionWriter, PgSnapshotStore,
};

use sqlx::PgPool;

/// Convenience struct that constructs all Postgres adapters from a single pool.
pub struct PgStores {
    pub snapshots: PgSnapshotStore,
    pub objects: PgObjectStore,
    pub changesets: PgChangesetStore,
    pub audit: PgAuditStore,
    pub outbox: PgOutboxStore,
    pub evidence: PgEvidenceStore,
    pub projections: PgProjectionWriter,
}

impl PgStores {
    pub fn new(pool: PgPool) -> Self {
        Self {
            snapshots: PgSnapshotStore::new(pool.clone()),
            objects: PgObjectStore::new(pool.clone()),
            changesets: PgChangesetStore::new(pool.clone()),
            audit: PgAuditStore::new(pool.clone()),
            outbox: PgOutboxStore::new(pool.clone()),
            evidence: PgEvidenceStore::new(pool.clone()),
            projections: PgProjectionWriter::new(pool),
        }
    }
}
