//! TaxonomyService - Database-backed entity type hierarchy operations.
//!
//! Provides operations for:
//! - Building entity type hierarchy trees with counts
//! - Navigating subtrees by type
//! - Listing entities by type with search/pagination
//!
//! Used by MCP tools for taxonomy navigation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use super::node::TaxonomyNode;
use super::types::{DimensionValues, NodeType};

/// Service for database-backed taxonomy operations.
#[derive(Clone)]
pub struct TaxonomyService {
    pool: PgPool,
}

/// An entity summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityListItem {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Current taxonomy navigation position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyPosition {
    /// Current focused type (None = root)
    pub focused_type: Option<String>,
    /// Breadcrumb trail
    pub breadcrumbs: Vec<String>,
    /// Available children from current position
    pub children: Vec<ChildNode>,
    /// Entity count at current level
    pub entity_count: i64,
}

/// Simplified child node info for position response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildNode {
    pub type_code: String,
    pub name: String,
    pub entity_count: i64,
    pub has_children: bool,
}

impl TaxonomyService {
    /// Create a new taxonomy service with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Build the full entity type hierarchy tree as a TaxonomyNode.
    ///
    /// Structure:
    /// ```text
    /// ROOT (Entity Types)
    /// ├── SHELL (Legal Vehicles)
    /// │   ├── LIMITED_COMPANY (count)
    /// │   ├── PARTNERSHIP_LIMITED (count)
    /// │   ├── TRUST_DISCRETIONARY (count)
    /// │   └── ...
    /// └── PERSON (Natural Persons)
    ///     ├── PROPER_PERSON_NATURAL (count)
    ///     └── PROPER_PERSON_BENEFICIAL_OWNER (count)
    /// ```
    pub async fn build_taxonomy_tree(&self, include_counts: bool) -> Result<TaxonomyNode> {
        // Load entity types with optional counts
        let types_with_counts = if include_counts {
            self.load_types_with_counts().await?
        } else {
            self.load_types().await?
        };

        // Group by category
        let mut by_category: HashMap<String, Vec<TypeRow>> = HashMap::new();
        for t in types_with_counts {
            let category = t
                .entity_category
                .clone()
                .unwrap_or_else(|| "OTHER".to_string());
            by_category.entry(category).or_default().push(t);
        }

        // Create root node
        let mut root = TaxonomyNode::root("Entity Types");
        root.short_label = Some("Types".to_string());

        // SHELL category
        if let Some(shell_types) = by_category.remove("SHELL") {
            let mut shell_node = TaxonomyNode::new(
                Uuid::now_v7(),
                NodeType::Cluster,
                "SHELL",
                DimensionValues {
                    entity_type: Some("SHELL".to_string()),
                    ..Default::default()
                },
            );
            shell_node.short_label = Some("Legal Vehicles".to_string());

            for t in shell_types {
                let mut type_node = TaxonomyNode::new(
                    Uuid::now_v7(),
                    NodeType::Cluster, // Entity type is a cluster of entities
                    &t.type_code,
                    DimensionValues {
                        entity_type: Some(t.type_code.clone()),
                        ..Default::default()
                    },
                );
                type_node.short_label = Some(t.name.clone());
                type_node.descendant_count = t.entity_count as usize;
                shell_node.add_child(type_node);
            }

            shell_node.compute_metrics();
            root.add_child(shell_node);
        }

        // PERSON category
        if let Some(person_types) = by_category.remove("PERSON") {
            let mut person_node = TaxonomyNode::new(
                Uuid::now_v7(),
                NodeType::Cluster,
                "PERSON",
                DimensionValues {
                    entity_type: Some("PERSON".to_string()),
                    ..Default::default()
                },
            );
            person_node.short_label = Some("Natural Persons".to_string());

            for t in person_types {
                let mut type_node = TaxonomyNode::new(
                    Uuid::now_v7(),
                    NodeType::Cluster, // Entity type is a cluster of entities
                    &t.type_code,
                    DimensionValues {
                        entity_type: Some(t.type_code.clone()),
                        ..Default::default()
                    },
                );
                type_node.short_label = Some(t.name.clone());
                type_node.descendant_count = t.entity_count as usize;
                person_node.add_child(type_node);
            }

            person_node.compute_metrics();
            root.add_child(person_node);
        }

        // Any other categories
        for (category, types) in by_category {
            let mut category_node = TaxonomyNode::new(
                Uuid::now_v7(),
                NodeType::Cluster,
                &category,
                DimensionValues {
                    entity_type: Some(category.clone()),
                    ..Default::default()
                },
            );

            for t in types {
                let mut type_node = TaxonomyNode::new(
                    Uuid::now_v7(),
                    NodeType::Cluster, // Entity type is a cluster of entities
                    &t.type_code,
                    DimensionValues {
                        entity_type: Some(t.type_code.clone()),
                        ..Default::default()
                    },
                );
                type_node.short_label = Some(t.name.clone());
                type_node.descendant_count = t.entity_count as usize;
                category_node.add_child(type_node);
            }

            category_node.compute_metrics();
            root.add_child(category_node);
        }

        // Compute root metrics
        root.compute_metrics();

        Ok(root)
    }

    /// Get a subtree for a specific type or category.
    ///
    /// - If `node_label` is "SHELL" or "PERSON", returns that category's subtree
    /// - If `node_label` is a specific type like "LIMITED_COMPANY", returns that type as a leaf
    pub async fn get_subtree(&self, node_label: &str) -> Result<TaxonomyNode> {
        let tree = self.build_taxonomy_tree(true).await?;

        // Normalize the label for comparison
        let normalized = node_label.to_uppercase().replace(['-', ' '], "_");

        // Check if it's a category (SHELL, PERSON)
        for category in &tree.children {
            if category.label.to_uppercase() == normalized {
                return Ok(category.clone());
            }

            // Check if it's a type within this category
            for child in &category.children {
                if child.label.to_uppercase() == normalized {
                    return Ok(child.clone());
                }
            }
        }

        // Not found - return empty node
        let mut empty = TaxonomyNode::root(node_label);
        empty.short_label = Some("Not Found".to_string());
        Ok(empty)
    }

    /// List entities of a specific type with optional search and pagination.
    pub async fn list_entities_by_type(
        &self,
        entity_type: &str,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EntityListItem>> {
        // Normalize type code
        let normalized_type = entity_type.to_uppercase().replace(['-', ' '], "_");

        // Check if this is a category (SHELL, PERSON) - if so, list all types in category
        let type_codes = self.resolve_type_codes(&normalized_type).await?;

        let entities = if let Some(search_term) = search {
            let search_pattern = format!("%{}%", search_term);
            sqlx::query_as::<_, EntityRow>(
                r#"
                SELECT
                    e.entity_id,
                    e.name,
                    et.type_code as entity_type,
                    COALESCE(
                        elc.jurisdiction,
                        ep.jurisdiction,
                        epp.nationality
                    ) as jurisdiction,
                    e.created_at
                FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                LEFT JOIN "ob-poc".entity_limited_companies elc ON e.entity_id = elc.entity_id
                LEFT JOIN "ob-poc".entity_partnerships ep ON e.entity_id = ep.entity_id
                LEFT JOIN "ob-poc".entity_proper_persons epp ON e.entity_id = epp.entity_id
                WHERE et.type_code = ANY($1)
                  AND e.name ILIKE $2
                ORDER BY e.name
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(&type_codes)
            .bind(&search_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, EntityRow>(
                r#"
                SELECT
                    e.entity_id,
                    e.name,
                    et.type_code as entity_type,
                    COALESCE(
                        elc.jurisdiction,
                        ep.jurisdiction,
                        epp.nationality
                    ) as jurisdiction,
                    e.created_at
                FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                LEFT JOIN "ob-poc".entity_limited_companies elc ON e.entity_id = elc.entity_id
                LEFT JOIN "ob-poc".entity_partnerships ep ON e.entity_id = ep.entity_id
                LEFT JOIN "ob-poc".entity_proper_persons epp ON e.entity_id = epp.entity_id
                WHERE et.type_code = ANY($1)
                ORDER BY e.name
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(&type_codes)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(entities.into_iter().map(|e| e.into()).collect())
    }

    /// Get the current taxonomy position (for navigation state).
    pub async fn get_position(&self, focused_type: Option<&str>) -> Result<TaxonomyPosition> {
        let tree = self.build_taxonomy_tree(true).await?;

        match focused_type {
            None => {
                // At root - show categories
                let children: Vec<ChildNode> = tree
                    .children
                    .iter()
                    .map(|c| ChildNode {
                        type_code: c.label.clone(),
                        name: c.short_label.clone().unwrap_or_else(|| c.label.clone()),
                        entity_count: c.descendant_count as i64,
                        has_children: !c.children.is_empty(),
                    })
                    .collect();

                let total_count: i64 = children.iter().map(|c| c.entity_count).sum();

                Ok(TaxonomyPosition {
                    focused_type: None,
                    breadcrumbs: vec!["Entity Types".to_string()],
                    children,
                    entity_count: total_count,
                })
            }
            Some(type_label) => {
                let normalized = type_label.to_uppercase().replace(['-', ' '], "_");

                // Find the node
                for category in &tree.children {
                    if category.label.to_uppercase() == normalized {
                        // Focused on a category
                        let children: Vec<ChildNode> = category
                            .children
                            .iter()
                            .map(|c| ChildNode {
                                type_code: c.label.clone(),
                                name: c.short_label.clone().unwrap_or_else(|| c.label.clone()),
                                entity_count: c.descendant_count as i64,
                                has_children: false, // Types are leaves
                            })
                            .collect();

                        return Ok(TaxonomyPosition {
                            focused_type: Some(category.label.clone()),
                            breadcrumbs: vec![
                                "Entity Types".to_string(),
                                category
                                    .short_label
                                    .clone()
                                    .unwrap_or_else(|| category.label.clone()),
                            ],
                            children,
                            entity_count: category.descendant_count as i64,
                        });
                    }

                    // Check types within category
                    for child in &category.children {
                        if child.label.to_uppercase() == normalized {
                            // Focused on a specific type (leaf)
                            return Ok(TaxonomyPosition {
                                focused_type: Some(child.label.clone()),
                                breadcrumbs: vec![
                                    "Entity Types".to_string(),
                                    category
                                        .short_label
                                        .clone()
                                        .unwrap_or_else(|| category.label.clone()),
                                    child
                                        .short_label
                                        .clone()
                                        .unwrap_or_else(|| child.label.clone()),
                                ],
                                children: vec![], // Leaf node
                                entity_count: child.descendant_count as i64,
                            });
                        }
                    }
                }

                // Type not found - return root position
                let children: Vec<ChildNode> = tree
                    .children
                    .iter()
                    .map(|c| ChildNode {
                        type_code: c.label.clone(),
                        name: c.short_label.clone().unwrap_or_else(|| c.label.clone()),
                        entity_count: c.descendant_count as i64,
                        has_children: !c.children.is_empty(),
                    })
                    .collect();

                Ok(TaxonomyPosition {
                    focused_type: None,
                    breadcrumbs: vec!["Entity Types".to_string()],
                    children,
                    entity_count: tree.descendant_count as i64,
                })
            }
        }
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    async fn load_types_with_counts(&self) -> Result<Vec<TypeRow>> {
        let rows = sqlx::query_as::<_, TypeRow>(
            r#"
            SELECT
                et.type_code,
                et.name,
                et.entity_category,
                COUNT(e.entity_id)::bigint as entity_count
            FROM "ob-poc".entity_types et
            LEFT JOIN "ob-poc".entities e ON e.entity_type_id = et.entity_type_id
            WHERE et.deprecated = false
            GROUP BY et.entity_type_id, et.type_code, et.name, et.entity_category
            ORDER BY et.entity_category, et.type_code
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn load_types(&self) -> Result<Vec<TypeRow>> {
        let rows = sqlx::query_as::<_, TypeRow>(
            r#"
            SELECT
                et.type_code,
                et.name,
                et.entity_category,
                0::bigint as entity_count
            FROM "ob-poc".entity_types et
            WHERE et.deprecated = false
            ORDER BY et.entity_category, et.type_code
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Resolve a type label to actual type codes.
    /// If it's a category (SHELL, PERSON), returns all types in that category.
    /// Otherwise returns the single type code.
    async fn resolve_type_codes(&self, type_label: &str) -> Result<Vec<String>> {
        // Check if it's a category
        let category_types = sqlx::query_scalar::<_, String>(
            r#"
            SELECT type_code
            FROM "ob-poc".entity_types
            WHERE entity_category = $1
              AND deprecated = false
            "#,
        )
        .bind(type_label)
        .fetch_all(&self.pool)
        .await?;

        if !category_types.is_empty() {
            return Ok(category_types);
        }

        // Otherwise it's a specific type
        Ok(vec![type_label.to_string()])
    }
}

// =============================================================================
// Database row types
// =============================================================================

#[derive(Debug, sqlx::FromRow)]
struct TypeRow {
    type_code: String,
    name: String,
    entity_category: Option<String>,
    entity_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct EntityRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    jurisdiction: Option<String>,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<EntityRow> for EntityListItem {
    fn from(row: EntityRow) -> Self {
        Self {
            entity_id: row.entity_id,
            name: row.name,
            entity_type: row.entity_type,
            jurisdiction: row.jurisdiction,
            created_at: row.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_list_item_structure() {
        let item = EntityListItem {
            entity_id: Uuid::now_v7(),
            name: "Test Entity".to_string(),
            entity_type: "LIMITED_COMPANY".to_string(),
            jurisdiction: Some("LU".to_string()),
            created_at: None,
        };

        assert_eq!(item.entity_type, "LIMITED_COMPANY");
    }
}
