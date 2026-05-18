use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Pool types ────────────────────────────────────────────────────────────────

/// Which tier of worker pool a tenant runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolType {
    /// Shared default pool — all tenants unless explicitly dedicated.
    Default,
    /// Dedicated pool for a single tenant group.
    Dedicated,
}

/// Pool metadata row as returned by the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub pool_id: String,
    pub pool_type: PoolType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub paused: bool,
    pub created_at: DateTime<Utc>,
}

/// Configuration for provisioning a new pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Container image for bpmn-lite worker pods.
    pub image: String,
    /// Desired replica count.
    pub replicas: u32,
    /// Minimum replicas for HPA.
    pub min_replicas: u32,
    /// Maximum replicas for HPA.
    pub max_replicas: u32,
    /// CPU request (e.g. "250m").
    pub cpu_request: String,
    /// Memory request (e.g. "256Mi").
    pub memory_request: String,
    /// CPU limit (e.g. "1000m").
    pub cpu_limit: String,
    /// Memory limit (e.g. "512Mi").
    pub memory_limit: String,
    /// Kubernetes namespace for the pool's Deployment. Defaults to "default".
    #[serde(default = "default_k8s_namespace")]
    pub namespace: String,
}

fn default_k8s_namespace() -> String {
    "default".to_string()
}

/// Current status of a pool — DB metadata plus live K8s replica counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    pub pool_id: String,
    pub pool_type: PoolType,
    pub paused: bool,
    /// Number of tenants assigned to this pool.
    pub tenant_count: usize,
    /// Approximate number of queued instances for this pool's tenants.
    pub queue_depth: i64,
    /// Desired replicas from the K8s Deployment spec (None until L3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_replicas: Option<i32>,
    /// Currently ready pod count from K8s (None until L3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_replicas: Option<i32>,
}

// ── Instance types ────────────────────────────────────────────────────────────

/// Current execution state of a process instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    Running,
    Completed,
    Failed,
    Cancelled,
    Terminated,
}

/// Full status of a single process instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStatus {
    pub instance_id: Uuid,
    pub tenant_id: String,
    pub process_key: String,
    pub state: InstanceState,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine_state: Option<String>,
}

/// Lightweight summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSummary {
    pub instance_id: Uuid,
    pub process_key: String,
    pub state: InstanceState,
    pub created_at: DateTime<Utc>,
}
