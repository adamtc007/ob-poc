//! sem_os_postgres — PostgreSQL implementations of sem_os_core port traits.
//!
//! Populated in Stage 1.3:
//! - PgSnapshotStore (from sem_reg/store.rs)
//! - PgObjectStore
//! - PgAuditStore
//! - PgOutboxStore (real SQL, not stub)
//! - PgEvidenceStore
//! - PgProjectionWriter (stub until S2.2)
//!
//! Phase 3 note (three-plane architecture v0.3 §13): `PgCrudExecutor`
//! moved to `dsl-runtime` because CRUD interpretation is a data-plane
//! concern. `sem_os_postgres` retains only metadata-loading and
//! store-implementation code.

pub mod authoring;
pub mod cleanup;
pub mod constellation_hydration;
pub mod ops;
pub(crate) mod sqlx_types;
pub mod store;

pub use authoring::{PgAuthoringStore, PgScratchSchemaRunner};
pub use cleanup::PgCleanupStore;
pub use ops::{SemOsVerbOp, SemOsVerbOpRegistry};
pub use store::{
    PgAuditStore, PgBootstrapAuditStore, PgChangesetStore, PgEvidenceStore, PgObjectStore,
    PgOutboxStore, PgProjectionWriter, PgSnapshotStore,
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
    pub authoring: PgAuthoringStore,
    pub scratch_runner: PgScratchSchemaRunner,
    pub cleanup: PgCleanupStore,
    pub bootstrap_audit: PgBootstrapAuditStore,
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
            projections: PgProjectionWriter::new(pool.clone()),
            authoring: PgAuthoringStore::new(pool.clone()),
            scratch_runner: PgScratchSchemaRunner::new(pool.clone()),
            cleanup: PgCleanupStore::new(pool.clone()),
            bootstrap_audit: PgBootstrapAuditStore::new(pool),
        }
    }
}
