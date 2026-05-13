//! TaxonomyBuilder - Constructs taxonomy trees from database
//!
//! The builder takes membership rules and produces a TaxonomyNode tree.
//! It handles:
//! - Loading entities from database based on root filter
//! - Traversing relationships based on edge types
//! - Grouping children based on grouping strategy
//! - Computing metrics after tree construction

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::node::TaxonomyNode;
use super::rules::{
    Dimension, EdgeType, GroupingStrategy, MembershipRules, RootFilter, TerminusCondition,
    TraversalDirection,
};
use super::types::{DimensionValues, EntitySummary, NodeType, Status};

/// Builds taxonomy trees from database + rules
pub struct TaxonomyBuilder {
    rules: MembershipRules,
}

impl TaxonomyBuilder {
    /// Create a new builder with the given rules
    pub fn new(rules: MembershipRules) -> Self {
        Self { rules }
    }

    /// Build taxonomy from database
    #[cfg(feature = "database")]
    pub async fn build(&self, pool: &PgPool) -> Result<TaxonomyNode> {
        // 1. Load root entities based on rules.root_filter
        let root_entities = self.load_roots(pool).await?;

        // 2. Create root node
        let mut root = TaxonomyNode::root(self.root_label());

        // 3. Maybe group roots by dimension
        if let GroupingStrategy::ByDimension(dim) = &self.rules.grouping {
            let grouped = self.group_by_dimension(&root_entities, *dim);
            for (group_label, entities) in grouped {
                let mut cluster = TaxonomyNode::new(
                    Uuid::new_v4(),
                    NodeType::Cluster,
                    group_label,
                    DimensionValues::default(),
                );
                for entity in entities {
                    let node = self.entity_to_node(&entity);
                    cluster.add_child(node);
                }
                root.add_child(cluster);
            }
        } else {
            for entity in root_entities {
                let mut node = self.entity_to_node(&entity);

                // 4. Traverse children if needed
                if self.rules.max_depth > 1 {
                    self.load_children(pool, &mut node, 1).await?;
                }

                root.add_child(node);
            }
        }

        // 5. Compute metrics
        root.compute_metrics();

        Ok(root)
    }

    /// Build taxonomy from pre-loaded data (for testing or offline use)
    pub fn build_from_data(
        &self,
        entities: &[EntityData],
        edges: &[EdgeData],
    ) -> Result<TaxonomyNode> {
        // Build adjacency list
        let mut adjacency: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for edge in edges {
            if self.rules.edge_types.contains(&edge.edge_type)
                || self.rules.edge_types.contains(&EdgeType::Any)
            {
                adjacency.entry(edge.from_id).or_default().push(edge.to_id);
            }
        }

        // Entity lookup
        let entity_map: HashMap<Uuid, &EntityData> = entities.iter().map(|e| (e.id, e)).collect();

        // Find roots based on filter
        let roots = self.filter_roots(entities);

        // Build tree
        let mut root = TaxonomyNode::root(self.root_label());
        let mut visited = HashSet::new();

        for root_entity in roots {
            let mut node = self.entity_data_to_node(root_entity);
            self.build_subtree(&mut node, &adjacency, &entity_map, &mut visited, 1);
            root.add_child(node);
        }

        root.compute_metrics();
        Ok(root)
    }

    fn build_subtree(
        &self,
        node: &mut TaxonomyNode,
        adjacency: &HashMap<Uuid, Vec<Uuid>>,
        entity_map: &HashMap<Uuid, &EntityData>,
        visited: &mut HashSet<Uuid>,
        depth: u32,
    ) {
        if depth >= self.rules.max_depth {
            return;
        }

        if visited.contains(&node.id) {
            return;
        }
        visited.insert(node.id);

        if let Some(children) = adjacency.get(&node.id) {
            for child_id in children {
                if let Some(entity) = entity_map.get(child_id) {
                    // Check terminus condition
                    if self.is_terminus(entity) {
                        let child = self.entity_data_to_node(entity);
                        node.add_child(child);
                        continue;
                    }

                    let mut child = self.entity_data_to_node(entity);
                    self.build_subtree(&mut child, adjacency, entity_map, visited, depth + 1);
                    node.add_child(child);
                }
            }
        }
    }

    fn is_terminus(&self, entity: &EntityData) -> bool {
        match &self.rules.terminus {
            TerminusCondition::MaxDepth => false, // Handled by depth check
            TerminusCondition::NaturalPerson => entity.entity_type == "proper_person",
            TerminusCondition::PublicCompany => entity.is_public.unwrap_or(false),
            TerminusCondition::NoMoreOwners => false, // Would check edges
            TerminusCondition::Custom(_) => false,    // Would evaluate predicate
        }
    }

    fn filter_roots<'a>(&self, entities: &'a [EntityData]) -> Vec<&'a EntityData> {
        match &self.rules.root_filter {
            RootFilter::AllCbus => entities.iter().filter(|e| e.entity_type == "cbu").collect(),
            RootFilter::Client { client_id } => entities
                .iter()
                .filter(|e| e.client_id == Some(*client_id))
                .collect(),
            RootFilter::SingleCbu { cbu_id } => {
                entities.iter().filter(|e| e.id == *cbu_id).collect()
            }
            RootFilter::Entities { filters } => entities
                .iter()
                .filter(|e| filters.iter().all(|f| f.matches(&e.dimensions)))
                .collect(),
        }
    }

    fn root_label(&self) -> String {
        match &self.rules.root_filter {
            RootFilter::AllCbus => "Universe".into(),
            RootFilter::Client { .. } => "Book".into(),
            RootFilter::SingleCbu { .. } => "CBU".into(),
            RootFilter::Entities { .. } => "Entities".into(),
        }
    }

    fn group_by_dimension(
        &self,
        entities: &[EntityData],
        dim: Dimension,
    ) -> HashMap<String, Vec<EntityData>> {
        let mut groups: HashMap<String, Vec<EntityData>> = HashMap::new();

        for entity in entities {
            let key = match dim {
                Dimension::Jurisdiction => entity
                    .dimensions
                    .jurisdiction
                    .clone()
                    .unwrap_or_else(|| "Other".into()),
                Dimension::FundType => entity
                    .dimensions
                    .fund_type
                    .clone()
                    .unwrap_or_else(|| "Other".into()),
                Dimension::ClientType => entity
                    .dimensions
                    .client_type
                    .clone()
                    .unwrap_or_else(|| "Other".into()),
                Dimension::EntityType => entity
                    .dimensions
                    .entity_type
                    .clone()
                    .unwrap_or_else(|| "Other".into()),
                Dimension::Status => entity
                    .dimensions
                    .status
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "Unknown".into()),
                Dimension::RoleCategory => entity
                    .dimensions
                    .role_category
                    .clone()
                    .unwrap_or_else(|| "Other".into()),
            };

            groups.entry(key).or_default().push(entity.clone());
        }

        groups
    }

    fn entity_to_node(&self, entity: &EntityData) -> TaxonomyNode {
        let node_type = match entity.entity_type.as_str() {
            "cbu" => NodeType::Cbu,
            "proper_person" | "limited_company" | "trust" | "partnership" => NodeType::Entity,
            "client" => NodeType::Client,
            _ => NodeType::Entity,
        };

        let mut node = TaxonomyNode::new(
            entity.id,
            node_type,
            &entity.name,
            entity.dimensions.clone(),
        );
        node.entity_data = Some(EntitySummary {
            name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            jurisdiction: entity.dimensions.jurisdiction.clone(),
            status: entity.dimensions.status.map(|s| format!("{:?}", s)),
            external_id: entity.external_id.clone(),
        });

        node
    }

    fn entity_data_to_node(&self, entity: &EntityData) -> TaxonomyNode {
        self.entity_to_node(entity)
    }

    // =========================================================================
    // DATABASE LOADING (feature-gated)
    // =========================================================================

    #[cfg(feature = "database")]
    async fn load_roots(&self, pool: &PgPool) -> Result<Vec<EntityData>> {
        match &self.rules.root_filter {
            RootFilter::AllCbus => self.load_all_cbus(pool).await,
            RootFilter::Client { client_id } => self.load_client_cbus(pool, *client_id).await,
            RootFilter::SingleCbu { cbu_id } => self.load_single_cbu(pool, *cbu_id).await,
            RootFilter::Entities { filters: _ } => {
                // Would apply filters to entities query
                self.load_all_cbus(pool).await
            }
        }
    }

    #[cfg(feature = "database")]
    async fn load_all_cbus(&self, pool: &PgPool) -> Result<Vec<EntityData>> {
        let rows = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT
                cbu_id as id,
                name,
                jurisdiction,
                client_type,
                risk_context->>'status' as status
            FROM "ob-poc".cbus
            ORDER BY name
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    #[cfg(feature = "database")]
    async fn load_client_cbus(&self, pool: &PgPool, client_id: Uuid) -> Result<Vec<EntityData>> {
        let rows = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT
                c.cbu_id as id,
                c.name,
                c.jurisdiction,
                c.client_type,
                c.risk_context->>'status' as status
            FROM "ob-poc".cbus c
            WHERE c.commercial_client_entity_id = $1
            ORDER BY c.name
            "#,
        )
        .bind(client_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    #[cfg(feature = "database")]
    async fn load_single_cbu(&self, pool: &PgPool, cbu_id: Uuid) -> Result<Vec<EntityData>> {
        let row = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT
                cbu_id as id,
                name,
                jurisdiction,
                client_type,
                risk_context->>'status' as status
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| vec![r.into()]).unwrap_or_default())
    }

    #[cfg(feature = "database")]
    async fn load_children(
        &self,
        pool: &PgPool,
        node: &mut TaxonomyNode,
        depth: u32,
    ) -> Result<()> {
        if depth >= self.rules.max_depth {
            return Ok(());
        }

        // Load entities with roles for this CBU
        if node.node_type == NodeType::Cbu {
            let entities = self.load_cbu_entities(pool, node.id).await?;

            // Group by role if needed
            if matches!(self.rules.grouping, GroupingStrategy::ByRole) {
                let by_role = self.group_entities_by_role(&entities);
                for (role, role_entities) in by_role {
                    let mut role_cluster = TaxonomyNode::new(
                        Uuid::new_v4(),
                        NodeType::Cluster,
                        role,
                        DimensionValues::default(),
                    );
                    for entity in role_entities {
                        let child = self.entity_to_node(&entity);
                        role_cluster.add_child(child);
                    }
                    node.add_child(role_cluster);
                }
            } else {
                for entity in entities {
                    let child = self.entity_to_node(&entity);
                    node.add_child(child);
                }
            }
        }

        // Load ownership chain if UBO view
        if self.rules.direction == TraversalDirection::Up {
            // Would trace ownership up to natural persons
            // This would be a separate query following entity_relationships
        }

        Ok(())
    }

    #[cfg(feature = "database")]
    async fn load_cbu_entities(&self, pool: &PgPool, cbu_id: Uuid) -> Result<Vec<EntityData>> {
        let rows = sqlx::query_as::<_, EntityRow>(
            r#"
            SELECT
                e.entity_id as id,
                e.name,
                et.type_code as entity_type,
                r.name as role_name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            ORDER BY r.name, e.name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    #[cfg(feature = "database")]
    fn group_entities_by_role(&self, entities: &[EntityData]) -> HashMap<String, Vec<EntityData>> {
        let mut groups: HashMap<String, Vec<EntityData>> = HashMap::new();
        for entity in entities {
            let role = entity
                .dimensions
                .role_category
                .clone()
                .unwrap_or_else(|| "Other".into());
            groups.entry(role).or_default().push(entity.clone());
        }
        groups
    }
}

// =============================================================================
// DATA TRANSFER TYPES
// =============================================================================

/// Entity data loaded from database or provided for testing
#[derive(Debug, Clone)]
pub struct EntityData {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub external_id: Option<String>,
    pub client_id: Option<Uuid>,
    pub is_public: Option<bool>,
    pub dimensions: DimensionValues,
}

/// Edge data for relationship traversal
#[derive(Debug, Clone)]
pub struct EdgeData {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub edge_type: EdgeType,
    pub percentage: Option<f64>,
}

// Database row types
#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct CbuRow {
    id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
    status: Option<String>,
}

#[cfg(feature = "database")]
impl From<CbuRow> for EntityData {
    fn from(row: CbuRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            entity_type: "cbu".into(),
            external_id: None,
            client_id: None,
            is_public: None,
            dimensions: DimensionValues {
                jurisdiction: row.jurisdiction,
                client_type: row.client_type,
                status: row.status.as_ref().and_then(|s| Status::parse(s)),
                ..Default::default()
            },
        }
    }
}

#[cfg(feature = "database")]
#[derive(Debug, sqlx::FromRow)]
struct EntityRow {
    id: Uuid,
    name: String,
    entity_type: String,
    role_name: String,
}

#[cfg(feature = "database")]
impl From<EntityRow> for EntityData {
    fn from(row: EntityRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            entity_type: row.entity_type,
            external_id: None,
            client_id: None,
            is_public: None,
            dimensions: DimensionValues {
                role_category: Some(row.role_name),
                ..Default::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entities() -> Vec<EntityData> {
        vec![
            EntityData {
                id: Uuid::new_v4(),
                name: "Fund A".into(),
                entity_type: "cbu".into(),
                external_id: None,
                client_id: None,
                is_public: None,
                dimensions: DimensionValues {
                    jurisdiction: Some("LU".into()),
                    fund_type: Some("UCITS".into()),
                    ..Default::default()
                },
            },
            EntityData {
                id: Uuid::new_v4(),
                name: "Fund B".into(),
                entity_type: "cbu".into(),
                external_id: None,
                client_id: None,
                is_public: None,
                dimensions: DimensionValues {
                    jurisdiction: Some("IE".into()),
                    fund_type: Some("AIF".into()),
                    ..Default::default()
                },
            },
            EntityData {
                id: Uuid::new_v4(),
                name: "Fund C".into(),
                entity_type: "cbu".into(),
                external_id: None,
                client_id: None,
                is_public: None,
                dimensions: DimensionValues {
                    jurisdiction: Some("LU".into()),
                    fund_type: Some("UCITS".into()),
                    ..Default::default()
                },
            },
        ]
    }

    #[test]
    fn test_build_from_data_universe() {
        let entities = sample_entities();
        let builder = TaxonomyBuilder::new(MembershipRules::universe());

        let tree = builder.build_from_data(&entities, &[]).unwrap();

        assert_eq!(tree.label, "Universe");
        // build_from_data doesn't implement dimension grouping, so we get all 3 CBUs as direct children
        // (The database-backed build() method does implement grouping by jurisdiction)
        assert_eq!(tree.children.len(), 3);
    }

    #[test]
    fn test_group_by_jurisdiction() {
        let entities = sample_entities();
        let builder = TaxonomyBuilder::new(MembershipRules::universe());

        let groups = builder.group_by_dimension(&entities, Dimension::Jurisdiction);

        assert!(groups.contains_key("LU"));
        assert!(groups.contains_key("IE"));
        assert_eq!(groups.get("LU").unwrap().len(), 2);
        assert_eq!(groups.get("IE").unwrap().len(), 1);
    }
}
