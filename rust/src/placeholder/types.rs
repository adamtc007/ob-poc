//! Placeholder entity types for deferred resolution.
//!
//! Placeholders allow macros to reference entities that don't exist yet,
//! creating stub records that must be resolved before the CBU becomes operational.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a placeholder entity in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PlaceholderStatus {
    /// Placeholder created, awaiting resolution
    Pending,
    /// Resolved to a real entity
    Resolved,
    /// Resolution verified by compliance/ops
    Verified,
}

impl std::fmt::Display for PlaceholderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaceholderStatus::Pending => write!(f, "pending"),
            PlaceholderStatus::Resolved => write!(f, "resolved"),
            PlaceholderStatus::Verified => write!(f, "verified"),
        }
    }
}

impl std::str::FromStr for PlaceholderStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(PlaceholderStatus::Pending),
            "resolved" => Ok(PlaceholderStatus::Resolved),
            "verified" => Ok(PlaceholderStatus::Verified),
            _ => Err(anyhow::anyhow!("Invalid placeholder status: {}", s)),
        }
    }
}

/// Kind of placeholder entity (maps to expected role/function).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlaceholderKind {
    /// Unique code for this kind (e.g., "depositary", "auditor")
    pub code: String,
    /// Human-readable label
    pub label: String,
    /// Description of what this placeholder represents
    pub description: Option<String>,
    /// Whether this kind requires verification after resolution
    pub requires_verification: bool,
}

/// A placeholder entity record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderEntity {
    /// The entity ID (from entities table)
    pub entity_id: Uuid,
    /// Placeholder status
    pub status: PlaceholderStatus,
    /// Kind of placeholder (depositary, auditor, etc.)
    pub kind: String,
    /// CBU this placeholder was created for
    pub created_for_cbu_id: Option<Uuid>,
    /// When the placeholder was created
    pub created_at: DateTime<Utc>,
    /// When the placeholder was resolved (if resolved)
    pub resolved_at: Option<DateTime<Utc>>,
    /// Entity ID this was resolved to (if resolved to different entity)
    pub resolved_to_entity_id: Option<Uuid>,
    /// Who resolved it
    pub resolved_by: Option<String>,
}

/// Request to create a placeholder entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlaceholderRequest {
    /// Kind of placeholder (depositary, auditor, etc.)
    pub kind: String,
    /// CBU this placeholder is for
    pub cbu_id: Uuid,
    /// Optional name hint for the placeholder
    pub name_hint: Option<String>,
    /// Optional description
    pub description: Option<String>,
}

/// Request to resolve a placeholder entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvePlaceholderRequest {
    /// The placeholder entity ID
    pub placeholder_entity_id: Uuid,
    /// The real entity ID to resolve to
    pub resolved_entity_id: Uuid,
    /// Who is performing the resolution
    pub resolved_by: String,
}

/// Result of placeholder resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderResolutionResult {
    /// Original placeholder entity ID
    pub placeholder_entity_id: Uuid,
    /// Entity ID it was resolved to
    pub resolved_to_entity_id: Uuid,
    /// New status
    pub status: PlaceholderStatus,
    /// Whether role assignments were transferred
    pub roles_transferred: i32,
}

/// Summary of pending placeholders for a CBU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderSummary {
    /// CBU ID
    pub cbu_id: Uuid,
    /// Total pending placeholders
    pub pending_count: i64,
    /// Placeholders by kind
    pub by_kind: Vec<PlaceholderKindCount>,
}

/// Count of placeholders by kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderKindCount {
    pub kind: String,
    pub count: i64,
}

/// Placeholder with entity details for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceholderWithDetails {
    /// The placeholder entity
    pub placeholder: PlaceholderEntity,
    /// Entity name
    pub entity_name: String,
    /// CBU name (if created for a CBU)
    pub cbu_name: Option<String>,
    /// Kind label
    pub kind_label: String,
}
