use thiserror::Error;

/// Errors specific to bpmn-controller operations.
///
/// Most callers use `anyhow::Result` and the `?` operator; this enum is for
/// cases where the caller needs to distinguish error kinds (e.g. "pool already
/// exists" vs "DB connectivity failure").
#[derive(Debug, Error)]
pub enum BpmnControllerError {
    #[error("pool '{0}' already exists")]
    PoolAlreadyExists(String),

    #[error("pool '{0}' not found")]
    PoolNotFound(String),

    #[error("cannot deprovision the default pool")]
    CannotDeprovisionDefaultPool,

    #[error("pool '{0}' still has tenants — reassign or move them first")]
    PoolHasTenants(String),

    #[error("tenant '{0}' not found")]
    TenantNotFound(String),

    #[error("process definition '{0}' not found")]
    ProcessDefinitionNotFound(String),

    #[error("instance '{0}' not found")]
    InstanceNotFound(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
