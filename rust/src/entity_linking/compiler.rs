//! Entity snapshot compiler
//!
//! Compiles entity data from the database into an in-memory snapshot
//! for fast resolution without database access in the hot path.

use super::normalize::{normalize_entity_text, tokenize};
use super::snapshot::{EntityId, EntityRow, EntitySnapshot, SNAPSHOT_VERSION};
use anyhow::Result;
use sha2::{Digest, Sha256};
use smallvec::SmallVec;
use sqlx::PgPool;
use std::collections::HashMap;

/// Compile entity snapshot from database
pub async fn compile_entity_snapshot(pool: &PgPool) -> Result<EntitySnapshot> {
    tracing::info!("Compiling entity snapshot...");

    tracing::debug!("Loading entities...");
    let entities = load_entities(pool).await?;
    tracing::info!("Loaded {} entities", entities.len());

    tracing::debug!("Building alias indexes...");
    let (alias_index, name_index) = build_alias_indexes(pool, &entities).await?;
    tracing::info!(
        "Built alias index ({} entries) and name index ({} entries)",
        alias_index.len(),
        name_index.len()
    );

    tracing::debug!("Loading concept links...");
    let concept_links = load_concept_links(pool).await?;
    tracing::info!("Loaded concept links for {} entities", concept_links.len());

    tracing::debug!("Building token index...");
    let token_index = build_token_index(pool, &entities, &alias_index).await?;
    tracing::info!("Built token index ({} tokens)", token_index.len());

    tracing::debug!("Building kind index...");
    let kind_index = build_kind_index(&entities);
    tracing::info!("Built kind index ({} kinds)", kind_index.len());

    tracing::debug!("Computing content hash...");
    let hash = compute_content_hash(&entities, &alias_index, &concept_links, &token_index);

    Ok(EntitySnapshot {
        version: SNAPSHOT_VERSION,
        hash,
        entities,
        alias_index,
        name_index,
        concept_links,
        token_index,
        kind_index,
    })
}

/// Load entities from database
async fn load_entities(pool: &PgPool) -> Result<Vec<EntityRow>> {
    let rows = sqlx::query!(
        r#"SELECT
            e.entity_id,
            et.name as entity_kind,
            e.name as canonical_name,
            COALESCE(e.name_norm, LOWER(e.name)) as "canonical_name_norm!"
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        ORDER BY e.entity_id"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EntityRow {
            entity_id: r.entity_id,
            entity_kind: r.entity_kind,
            canonical_name: r.canonical_name,
            canonical_name_norm: normalize_entity_text(&r.canonical_name_norm, false),
        })
        .collect())
}

/// Build alias and name indexes from entity_names and agent.entity_aliases
async fn build_alias_indexes(
    pool: &PgPool,
    entities: &[EntityRow],
) -> Result<(
    HashMap<String, SmallVec<[EntityId; 4]>>,
    HashMap<String, EntityId>,
)> {
    let mut alias_idx: HashMap<String, SmallVec<[EntityId; 4]>> = HashMap::new();
    let mut name_idx: HashMap<String, EntityId> = HashMap::new();

    // Add canonical names
    for e in entities {
        let norm = normalize_entity_text(&e.canonical_name, false);
        name_idx.insert(norm.clone(), e.entity_id);
        alias_idx.entry(norm).or_default().push(e.entity_id);
    }

    // Load from entity_names table
    let names = sqlx::query!(
        r#"SELECT
            entity_id,
            name,
            name_type
        FROM "ob-poc".entity_names
        WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities)
        ORDER BY entity_id"#
    )
    .fetch_all(pool)
    .await?;

    for n in names {
        let norm = normalize_entity_text(&n.name, false);
        alias_idx.entry(norm).or_default().push(n.entity_id);
    }

    // Load from agent.entity_aliases table
    let aliases = sqlx::query!(
        r#"SELECT
            entity_id,
            alias
        FROM agent.entity_aliases
        WHERE entity_id IS NOT NULL
        ORDER BY entity_id"#
    )
    .fetch_all(pool)
    .await?;

    for a in aliases {
        if let Some(entity_id) = a.entity_id {
            let norm = normalize_entity_text(&a.alias, false);
            alias_idx.entry(norm).or_default().push(entity_id);
        }
    }

    // Cap and dedupe alias entries
    for v in alias_idx.values_mut() {
        v.sort();
        v.dedup();
        v.truncate(20);
    }

    Ok((alias_idx, name_idx))
}

/// Build token index for fuzzy matching
async fn build_token_index(
    pool: &PgPool,
    entities: &[EntityRow],
    alias_index: &HashMap<String, SmallVec<[EntityId; 4]>>,
) -> Result<HashMap<String, SmallVec<[EntityId; 8]>>> {
    let mut token_idx: HashMap<String, SmallVec<[EntityId; 8]>> = HashMap::new();

    // Load from entity_feature table if populated
    let features = sqlx::query!(
        r#"SELECT entity_id, token_norm
        FROM "ob-poc".entity_feature
        WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities)
        ORDER BY entity_id"#
    )
    .fetch_all(pool)
    .await?;

    for f in features {
        token_idx.entry(f.token_norm).or_default().push(f.entity_id);
    }

    // Derive from canonical names
    for e in entities {
        for token in tokenize(&e.canonical_name) {
            token_idx.entry(token).or_default().push(e.entity_id);
        }
    }

    // Derive from aliases
    for (alias_norm, ids) in alias_index {
        for token in tokenize(alias_norm) {
            for id in ids {
                token_idx.entry(token.clone()).or_default().push(*id);
            }
        }
    }

    // Cap and dedupe
    for v in token_idx.values_mut() {
        v.sort();
        v.dedup();
        v.truncate(50);
    }

    Ok(token_idx)
}

/// Load concept links from entity_concept_link table
async fn load_concept_links(
    pool: &PgPool,
) -> Result<HashMap<EntityId, SmallVec<[(String, f32); 8]>>> {
    let links = sqlx::query!(
        r#"SELECT entity_id, concept_id, weight
        FROM "ob-poc".entity_concept_link
        WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities)
        ORDER BY entity_id, weight DESC"#
    )
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<EntityId, SmallVec<[(String, f32); 8]>> = HashMap::new();
    for l in links {
        map.entry(l.entity_id)
            .or_default()
            .push((l.concept_id, l.weight));
    }

    // Truncate to 8 per entity
    for v in map.values_mut() {
        v.truncate(8);
    }

    Ok(map)
}

/// Build kind index from entities
fn build_kind_index(entities: &[EntityRow]) -> HashMap<String, SmallVec<[EntityId; 16]>> {
    let mut kind_idx: HashMap<String, SmallVec<[EntityId; 16]>> = HashMap::new();

    for e in entities {
        kind_idx
            .entry(e.entity_kind.clone())
            .or_default()
            .push(e.entity_id);
    }

    kind_idx
}

/// Compute content-based hash for cache invalidation
fn compute_content_hash(
    entities: &[EntityRow],
    alias_idx: &HashMap<String, SmallVec<[EntityId; 4]>>,
    concept_links: &HashMap<EntityId, SmallVec<[(String, f32); 8]>>,
    token_idx: &HashMap<String, SmallVec<[EntityId; 8]>>,
) -> String {
    let mut h = Sha256::new();

    // Version
    h.update(SNAPSHOT_VERSION.to_le_bytes());

    // Entities (sorted by id)
    for e in entities {
        h.update(e.entity_id.as_bytes());
        h.update(e.entity_kind.as_bytes());
        h.update(e.canonical_name_norm.as_bytes());
    }

    // Alias index (sorted keys)
    let mut alias_keys: Vec<_> = alias_idx.keys().collect();
    alias_keys.sort();
    for k in alias_keys {
        h.update(k.as_bytes());
        let mut ids: Vec<_> = alias_idx[k].iter().collect();
        ids.sort();
        for id in ids {
            h.update(id.as_bytes());
        }
    }

    // Concept links (sorted by entity id)
    let mut entity_ids: Vec<_> = concept_links.keys().collect();
    entity_ids.sort();
    for eid in entity_ids {
        h.update(eid.as_bytes());
        for (cid, w) in &concept_links[eid] {
            h.update(cid.as_bytes());
            h.update(w.to_le_bytes());
        }
    }

    // Token index (sorted keys, only count to avoid huge hash)
    let mut token_keys: Vec<_> = token_idx.keys().collect();
    token_keys.sort();
    for k in token_keys {
        h.update(k.as_bytes());
        h.update((token_idx[k].len() as u32).to_le_bytes());
    }

    format!("{:x}", h.finalize())
}

/// Lint entity data for quality issues
pub async fn lint_entity_data(pool: &PgPool) -> Result<Vec<LintWarning>> {
    let mut warnings = Vec::new();

    // Check entities with empty name_norm
    let empty_norm = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM "ob-poc".entities WHERE name_norm IS NULL OR name_norm = ''"#
    )
    .fetch_one(pool)
    .await?;

    if empty_norm > 0 {
        warnings.push(LintWarning {
            severity: LintSeverity::Warning,
            message: format!("{} entities have empty name_norm", empty_norm),
            suggestion: Some("Run UPDATE entities SET name_norm = ... to populate".to_string()),
        });
    }

    // Check duplicate aliases pointing to different entities
    let dup_aliases = sqlx::query!(
        r#"SELECT alias_norm, COUNT(DISTINCT entity_id) as cnt
        FROM (
            SELECT LOWER(TRIM(REGEXP_REPLACE(name, '[^a-zA-Z0-9 ]', ' ', 'g'))) as alias_norm, entity_id
            FROM "ob-poc".entity_names
            UNION ALL
            SELECT LOWER(TRIM(REGEXP_REPLACE(alias, '[^a-zA-Z0-9 ]', ' ', 'g'))) as alias_norm, entity_id
            FROM agent.entity_aliases WHERE entity_id IS NOT NULL
        ) combined
        GROUP BY alias_norm
        HAVING COUNT(DISTINCT entity_id) > 1
        ORDER BY cnt DESC
        LIMIT 20"#
    )
    .fetch_all(pool)
    .await?;

    for d in &dup_aliases {
        warnings.push(LintWarning {
            severity: LintSeverity::Info,
            message: format!(
                "Alias '{}' maps to {} different entities (ambiguous)",
                d.alias_norm.as_deref().unwrap_or("?"),
                d.cnt.unwrap_or(0)
            ),
            suggestion: Some("Consider adding concept links for disambiguation".to_string()),
        });
    }

    // Check entities without any names/aliases
    let no_names = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!"
        FROM "ob-poc".entities e
        WHERE NOT EXISTS (
            SELECT 1 FROM "ob-poc".entity_names en WHERE en.entity_id = e.entity_id
        )"#
    )
    .fetch_one(pool)
    .await?;

    if no_names > 0 {
        warnings.push(LintWarning {
            severity: LintSeverity::Info,
            message: format!(
                "{} entities have no entries in entity_names (canonical name only)",
                no_names
            ),
            suggestion: None,
        });
    }

    Ok(warnings)
}

/// A lint warning
#[derive(Debug, Clone)]
pub struct LintWarning {
    pub severity: LintSeverity,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Lint severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for LintWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let icon = match self.severity {
            LintSeverity::Error => "✗",
            LintSeverity::Warning => "⚠",
            LintSeverity::Info => "ℹ",
        };
        write!(f, "{} {}", icon, self.message)?;
        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  → {}", suggestion)?;
        }
        Ok(())
    }
}
