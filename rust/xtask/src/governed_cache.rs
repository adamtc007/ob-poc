//! xtask commands for GovernedQuery cache management.
//!
//! Usage:
//!   cargo x governed-cache refresh   — regenerate assets/governed_cache.bin
//!   cargo x governed-cache stats     — print cache statistics

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;

// ── Mirror types (must match governed_query_proc::registry_types) ───

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceTier {
    Governed,
    Operational,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustClass {
    Proof,
    DecisionSupport,
    Convenience,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    Draft,
    Active,
    Deprecated,
    Retired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    AttributeDef,
    EntityTypeDef,
    RelationshipTypeDef,
    VerbContract,
    TaxonomyDef,
    TaxonomyNode,
    MembershipRule,
    ViewDef,
    PolicyRule,
    EvidenceRequirement,
    DocumentTypeDef,
    ObservationDef,
    DerivationSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub fqn: String,
    pub object_type: ObjectType,
    pub status: SnapshotStatus,
    pub governance_tier: GovernanceTier,
    pub trust_class: TrustClass,
    pub pii: bool,
    pub classification: Classification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedCache {
    pub version: u32,
    pub generated_at: String,
    pub entries: HashMap<String, CacheEntry>,
}

// ── DB row type for sqlx ─────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct SnapshotCacheRow {
    fqn: String,
    object_type: String,
    status: String,
    governance_tier: String,
    trust_class: String,
    security_label: serde_json::Value,
}

// ── Commands ─────────────────────────────────────────────────────

pub async fn refresh(output: Option<PathBuf>) -> Result<()> {
    let pool = connect().await?;

    println!("Querying sem_reg.snapshots for all active entries...");

    let rows: Vec<SnapshotCacheRow> = sqlx::query_as(
        r#"
        SELECT
            definition ->> 'fqn' AS fqn,
            object_type::text,
            status::text,
            governance_tier::text,
            trust_class::text,
            COALESCE(security_label, '{}'::jsonb) as security_label
        FROM sem_reg.snapshots
        WHERE status = 'active'
          AND definition ->> 'fqn' IS NOT NULL
        ORDER BY definition ->> 'fqn'
        "#,
    )
    .fetch_all(&pool)
    .await
    .context("Failed to query sem_reg.snapshots")?;

    println!("Found {} active snapshots.", rows.len());

    let mut entries = HashMap::new();
    let mut parse_errors = 0u32;

    for row in &rows {
        let object_type = match parse_object_type(&row.object_type) {
            Some(t) => t,
            None => {
                eprintln!(
                    "  WARN: unknown object_type '{}' for {}",
                    row.object_type, row.fqn
                );
                parse_errors += 1;
                continue;
            }
        };
        let governance_tier = match row.governance_tier.as_str() {
            "governed" => GovernanceTier::Governed,
            "operational" => GovernanceTier::Operational,
            other => {
                eprintln!("  WARN: unknown governance_tier '{other}' for {}", row.fqn);
                parse_errors += 1;
                continue;
            }
        };
        let trust_class = match row.trust_class.as_str() {
            "proof" => TrustClass::Proof,
            "decision_support" => TrustClass::DecisionSupport,
            "convenience" => TrustClass::Convenience,
            other => {
                eprintln!("  WARN: unknown trust_class '{other}' for {}", row.fqn);
                parse_errors += 1;
                continue;
            }
        };
        let status = match row.status.as_str() {
            "active" => SnapshotStatus::Active,
            "draft" => SnapshotStatus::Draft,
            "deprecated" => SnapshotStatus::Deprecated,
            "retired" => SnapshotStatus::Retired,
            other => {
                eprintln!("  WARN: unknown status '{other}' for {}", row.fqn);
                parse_errors += 1;
                continue;
            }
        };

        // Extract PII and classification from security_label JSONB
        let pii = row
            .security_label
            .get("pii")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let classification = match row
            .security_label
            .get("classification")
            .and_then(|v| v.as_str())
        {
            Some("public") => Classification::Public,
            Some("confidential") => Classification::Confidential,
            Some("restricted") => Classification::Restricted,
            _ => Classification::Internal,
        };

        entries.insert(
            row.fqn.clone(),
            CacheEntry {
                fqn: row.fqn.clone(),
                object_type,
                status,
                governance_tier,
                trust_class,
                pii,
                classification,
            },
        );
    }

    if parse_errors > 0 {
        eprintln!("  {parse_errors} entries skipped due to parse errors.");
    }

    let cache = GovernedCache {
        version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        entries,
    };

    let output_path = output.unwrap_or_else(|| default_cache_path());
    let bytes = bincode::serialize(&cache).context("Failed to serialize cache")?;

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(&output_path, &bytes)
        .with_context(|| format!("Failed to write cache to {}", output_path.display()))?;

    println!(
        "Wrote {} entries ({} bytes) to {}",
        cache.entries.len(),
        bytes.len(),
        output_path.display()
    );

    Ok(())
}

pub async fn stats(path: Option<PathBuf>) -> Result<()> {
    let cache_path = path.unwrap_or_else(|| default_cache_path());

    let bytes = std::fs::read(&cache_path).with_context(|| {
        format!(
            "Cannot read cache at {}. Run `cargo x governed-cache refresh`.",
            cache_path.display()
        )
    })?;

    let cache: GovernedCache = bincode::deserialize(&bytes).context("Cannot deserialize cache")?;

    println!("GovernedQuery Cache Statistics");
    println!("==============================");
    println!("  Version:      {}", cache.version);
    println!("  Generated:    {}", cache.generated_at);
    println!("  Total entries: {}", cache.entries.len());
    println!();

    // Count by object type
    let mut by_type: HashMap<String, usize> = HashMap::new();
    let mut by_tier: HashMap<String, usize> = HashMap::new();
    let mut pii_count = 0usize;

    for entry in cache.entries.values() {
        *by_type
            .entry(format!("{:?}", entry.object_type))
            .or_default() += 1;
        *by_tier
            .entry(format!("{:?}", entry.governance_tier))
            .or_default() += 1;
        if entry.pii {
            pii_count += 1;
        }
    }

    println!("  By object type:");
    let mut types: Vec<_> = by_type.iter().collect();
    types.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (t, c) in &types {
        println!("    {:<30} {:>5}", t, c);
    }

    println!();
    println!("  By governance tier:");
    for (t, c) in &by_tier {
        println!("    {:<30} {:>5}", t, c);
    }

    println!();
    println!("  PII-labelled entries: {}", pii_count);

    Ok(())
}

fn default_cache_path() -> PathBuf {
    // From xtask directory, go up to rust/, then assets/
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let workspace_root = PathBuf::from(&manifest_dir)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    workspace_root.join("assets").join("governed_cache.bin")
}

fn parse_object_type(s: &str) -> Option<ObjectType> {
    match s {
        "attribute_def" => Some(ObjectType::AttributeDef),
        "entity_type_def" => Some(ObjectType::EntityTypeDef),
        "relationship_type_def" => Some(ObjectType::RelationshipTypeDef),
        "verb_contract" => Some(ObjectType::VerbContract),
        "taxonomy_def" => Some(ObjectType::TaxonomyDef),
        "taxonomy_node" => Some(ObjectType::TaxonomyNode),
        "membership_rule" => Some(ObjectType::MembershipRule),
        "view_def" => Some(ObjectType::ViewDef),
        "policy_rule" => Some(ObjectType::PolicyRule),
        "evidence_requirement" => Some(ObjectType::EvidenceRequirement),
        "document_type_def" => Some(ObjectType::DocumentTypeDef),
        "observation_def" => Some(ObjectType::ObservationDef),
        "derivation_spec" => Some(ObjectType::DerivationSpec),
        _ => None,
    }
}

async fn connect() -> Result<PgPool> {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    PgPool::connect(&url)
        .await
        .context("Failed to connect to database")
}
