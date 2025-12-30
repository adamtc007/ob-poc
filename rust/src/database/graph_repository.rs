//! Graph Repository
//!
//! Provides unified graph loading for EntityGraph with support for:
//! - Single CBU scope (existing functionality)
//! - Book scope (all CBUs under an ownership apex)
//! - Jurisdiction scope (all CBUs in a jurisdiction)
//! - Entity neighborhood scope (entity + N hops)
//!
//! This repository returns data compatible with the unified EntityGraph type.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::graph::{
    CbuNode, ControlEdge, ControlType, EntityGraph, FundEdge, GraphNode, GraphScope, OwnershipEdge,
    OwnershipType, RoleAssignment,
};

// =============================================================================
// DERIVED BOOK TYPE
// =============================================================================

/// A "book" is a collection of CBUs that share an ownership apex
#[derive(Debug, Clone)]
pub struct DerivedBook {
    /// The ultimate owner entity (terminus of ownership chain)
    pub apex_entity_id: Uuid,
    pub apex_name: String,
    pub apex_jurisdiction: Option<String>,
    /// CBUs in this book
    pub cbu_ids: Vec<Uuid>,
    pub cbu_count: usize,
}

// =============================================================================
// GRAPH REPOSITORY TRAIT
// =============================================================================

#[async_trait]
pub trait GraphRepository: Send + Sync {
    /// Load graph for a single CBU (existing functionality)
    async fn load_cbu_graph(&self, cbu_id: Uuid, as_of: NaiveDate) -> Result<EntityGraph>;

    /// Load graph for all CBUs under an ownership apex (book)
    async fn load_book_graph(&self, apex_entity_id: Uuid, as_of: NaiveDate) -> Result<EntityGraph>;

    /// Load graph for all CBUs in a jurisdiction
    async fn load_jurisdiction_graph(
        &self,
        jurisdiction: &str,
        as_of: NaiveDate,
    ) -> Result<EntityGraph>;

    /// Load graph for an entity and its neighborhood (N hops)
    async fn load_neighborhood_graph(
        &self,
        entity_id: Uuid,
        hops: u32,
        as_of: NaiveDate,
    ) -> Result<EntityGraph>;

    /// Find ownership apex (UBO terminus) for an entity
    async fn find_ownership_apex(&self, entity_id: Uuid, as_of: NaiveDate) -> Result<Option<Uuid>>;

    /// Derive book membership for all CBUs
    async fn derive_books(&self, as_of: NaiveDate) -> Result<Vec<DerivedBook>>;

    /// Search entities by name within a scope
    async fn search_entities(
        &self,
        name_pattern: &str,
        scope: &GraphScope,
        as_of: NaiveDate,
    ) -> Result<Vec<GraphNode>>;

    /// Find person's roles across a scope
    async fn find_person_roles(
        &self,
        person_name: &str,
        scope: &GraphScope,
        as_of: NaiveDate,
    ) -> Result<Vec<RoleAssignment>>;
}

// =============================================================================
// DATABASE ROW TYPES (for sqlx::FromRow)
// Fields may appear unused but are required for SQLx query binding
// =============================================================================

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct EntityRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    entity_category: Option<String>,
    jurisdiction: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct CbuRow {
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
    commercial_client_entity_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct RoleRow {
    cbu_id: Uuid,
    entity_id: Uuid,
    role_name: String,
    role_category: Option<String>,
    ownership_percentage: Option<rust_decimal::Decimal>,
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct OwnershipRow {
    relationship_id: Uuid,
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    percentage: Option<rust_decimal::Decimal>,
    ownership_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct ControlRow {
    relationship_id: Uuid,
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    control_type: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct FundStructureRow {
    structure_id: Uuid,
    parent_entity_id: Uuid,
    child_entity_id: Uuid,
    relationship_type: String,
}

#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
struct OwnershipChainRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    jurisdiction: Option<String>,
    depth: i32,
}

// =============================================================================
// POSTGRES IMPLEMENTATION
// =============================================================================

pub struct PgGraphRepository {
    pool: PgPool,
}

impl PgGraphRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Load all entities linked to a CBU via roles
    async fn load_cbu_entities(&self, cbu_id: Uuid) -> Result<Vec<EntityRow>> {
        let rows = sqlx::query_as::<_, EntityRow>(
            r#"
            SELECT DISTINCT
                e.entity_id,
                e.name,
                COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                et.entity_category,
                COALESCE(
                    ep.jurisdiction,
                    elc.jurisdiction,
                    ept.jurisdiction,
                    etr.jurisdiction,
                    ef.jurisdiction
                ) as jurisdiction
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_partnerships ept ON ept.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_trusts etr ON etr.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
            WHERE cer.cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Load role assignments for a CBU
    async fn load_cbu_roles(&self, cbu_id: Uuid) -> Result<Vec<RoleRow>> {
        let rows = sqlx::query_as::<_, RoleRow>(
            r#"
            SELECT
                cer.cbu_id,
                cer.entity_id,
                r.name as role_name,
                r.role_category,
                cer.ownership_percentage
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cer.cbu_id = $1
              AND (cer.effective_to IS NULL OR cer.effective_to >= CURRENT_DATE)
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Load ownership relationships for entities in a set
    async fn load_ownership_edges(
        &self,
        entity_ids: &HashSet<Uuid>,
        as_of: NaiveDate,
    ) -> Result<Vec<OwnershipRow>> {
        if entity_ids.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<Uuid> = entity_ids.iter().copied().collect();

        let rows = sqlx::query_as::<_, OwnershipRow>(
            r#"
            SELECT
                er.relationship_id,
                er.from_entity_id,
                er.to_entity_id,
                er.percentage,
                er.ownership_type
            FROM "ob-poc".entity_relationships er
            WHERE er.relationship_type = 'ownership'
              AND (er.from_entity_id = ANY($1) OR er.to_entity_id = ANY($1))
              AND (er.effective_from IS NULL OR er.effective_from <= $2)
              AND (er.effective_to IS NULL OR er.effective_to >= $2)
            "#,
        )
        .bind(&ids)
        .bind(as_of)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Load control relationships for entities in a set
    async fn load_control_edges(
        &self,
        entity_ids: &HashSet<Uuid>,
        as_of: NaiveDate,
    ) -> Result<Vec<ControlRow>> {
        if entity_ids.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<Uuid> = entity_ids.iter().copied().collect();

        let rows = sqlx::query_as::<_, ControlRow>(
            r#"
            SELECT
                er.relationship_id,
                er.from_entity_id,
                er.to_entity_id,
                er.control_type
            FROM "ob-poc".entity_relationships er
            WHERE er.relationship_type = 'control'
              AND (er.from_entity_id = ANY($1) OR er.to_entity_id = ANY($1))
              AND (er.effective_from IS NULL OR er.effective_from <= $2)
              AND (er.effective_to IS NULL OR er.effective_to >= $2)
            "#,
        )
        .bind(&ids)
        .bind(as_of)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Load fund structure relationships for entities in a set
    ///
    /// This queries BOTH:
    /// 1. fund_structure table (explicit structure relationships)
    /// 2. entity_funds.parent_fund_id (implicit parent links from fund verbs)
    ///
    /// The union ensures we capture fund hierarchy regardless of which mechanism was used.
    async fn load_fund_edges(&self, entity_ids: &HashSet<Uuid>) -> Result<Vec<FundStructureRow>> {
        if entity_ids.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<Uuid> = entity_ids.iter().copied().collect();

        let rows = sqlx::query_as::<_, FundStructureRow>(
            r#"
            -- Explicit fund_structure entries
            SELECT
                fs.structure_id,
                fs.parent_entity_id,
                fs.child_entity_id,
                fs.relationship_type
            FROM "ob-poc".fund_structure fs
            WHERE (fs.parent_entity_id = ANY($1) OR fs.child_entity_id = ANY($1))
              AND (fs.effective_to IS NULL OR fs.effective_to >= CURRENT_DATE)

            UNION ALL

            -- Implicit parent links from entity_funds.parent_fund_id
            -- These represent umbrella→subfund relationships
            SELECT
                ef.entity_id as structure_id,  -- Use child entity_id as synthetic structure_id
                ef.parent_fund_id as parent_entity_id,
                ef.entity_id as child_entity_id,
                'CONTAINS' as relationship_type
            FROM "ob-poc".entity_funds ef
            WHERE ef.parent_fund_id IS NOT NULL
              AND (ef.entity_id = ANY($1) OR ef.parent_fund_id = ANY($1))
              -- Exclude if already in fund_structure to avoid duplicates
              AND NOT EXISTS (
                  SELECT 1 FROM "ob-poc".fund_structure fs2
                  WHERE fs2.parent_entity_id = ef.parent_fund_id
                    AND fs2.child_entity_id = ef.entity_id
              )
            "#,
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get CBU info
    async fn get_cbu(&self, cbu_id: Uuid) -> Result<CbuRow> {
        let row = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, jurisdiction, client_type, commercial_client_entity_id
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        Ok(row)
    }

    /// Walk ownership chain upward using recursive CTE
    async fn walk_ownership_chain_up(
        &self,
        start_entity_id: Uuid,
        as_of: NaiveDate,
    ) -> Result<Vec<OwnershipChainRow>> {
        let rows = sqlx::query_as::<_, OwnershipChainRow>(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Base case: start entity
                SELECT
                    e.entity_id,
                    e.name,
                    COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                    COALESCE(
                        ep.jurisdiction,
                        elc.jurisdiction,
                        ept.jurisdiction,
                        etr.jurisdiction,
                        ef.jurisdiction
                    ) as jurisdiction,
                    0 as depth
                FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_partnerships ept ON ept.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_trusts etr ON etr.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
                WHERE e.entity_id = $1

                UNION ALL

                -- Recursive case: follow ownership upward
                SELECT
                    e.entity_id,
                    e.name,
                    COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                    COALESCE(
                        ep.jurisdiction,
                        elc.jurisdiction,
                        ept.jurisdiction,
                        etr.jurisdiction,
                        ef.jurisdiction
                    ) as jurisdiction,
                    oc.depth + 1 as depth
                FROM ownership_chain oc
                JOIN "ob-poc".entity_relationships er
                    ON er.to_entity_id = oc.entity_id
                    AND er.relationship_type = 'ownership'
                    AND (er.effective_from IS NULL OR er.effective_from <= $2)
                    AND (er.effective_to IS NULL OR er.effective_to >= $2)
                JOIN "ob-poc".entities e ON e.entity_id = er.from_entity_id
                JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_partnerships ept ON ept.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_trusts etr ON etr.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
                WHERE oc.depth < 20  -- Prevent infinite loops
            )
            SELECT entity_id, name, entity_type, jurisdiction, depth
            FROM ownership_chain
            ORDER BY depth ASC
            "#,
        )
        .bind(start_entity_id)
        .bind(as_of)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get all CBUs in a jurisdiction
    async fn get_cbus_by_jurisdiction(&self, jurisdiction: &str) -> Result<Vec<CbuRow>> {
        let rows = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, jurisdiction, client_type, commercial_client_entity_id
            FROM "ob-poc".cbus
            WHERE jurisdiction = $1
            "#,
        )
        .bind(jurisdiction)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Build EntityGraph from loaded data
    fn build_graph(
        &self,
        cbus: Vec<CbuRow>,
        entities: Vec<EntityRow>,
        roles: Vec<RoleRow>,
        ownership_rows: Vec<OwnershipRow>,
        control_rows: Vec<ControlRow>,
        fund_rows: Vec<FundStructureRow>,
    ) -> EntityGraph {
        // Build nodes
        let mut nodes: HashMap<Uuid, GraphNode> = HashMap::new();
        for e in entities {
            let mut node = GraphNode::new(
                e.entity_id,
                e.name,
                e.entity_type.parse().unwrap_or_default(),
            );
            node.jurisdiction = e.jurisdiction;
            nodes.insert(e.entity_id, node);
        }

        // Build CBU nodes
        let mut cbu_nodes: HashMap<Uuid, CbuNode> = HashMap::new();
        for c in cbus {
            let mut cbu = CbuNode::new(c.cbu_id, c.name);
            cbu.jurisdiction = c.jurisdiction;
            cbu.commercial_client_id = c.commercial_client_entity_id;
            cbu_nodes.insert(c.cbu_id, cbu);
        }

        // Build role assignments
        let role_assignments: Vec<RoleAssignment> = roles
            .into_iter()
            .map(|r| RoleAssignment {
                id: Uuid::new_v4(),
                cbu_id: r.cbu_id,
                entity_id: r.entity_id,
                role: r.role_name,
                role_category: r.role_category.as_deref().and_then(|s| s.parse().ok()),
                ownership_percentage: r.ownership_percentage,
                effective_from: None,
                effective_to: None,
                visible: true,
            })
            .collect();

        // Populate cbu_memberships and roles in nodes
        for ra in &role_assignments {
            if let Some(node) = nodes.get_mut(&ra.entity_id) {
                if !node.cbu_memberships.contains(&ra.cbu_id) {
                    node.cbu_memberships.push(ra.cbu_id);
                }
                if !node.roles.contains(&ra.role) {
                    node.roles.push(ra.role.clone());
                }
            }
            // Also add entity to CBU's member list
            if let Some(cbu) = cbu_nodes.get_mut(&ra.cbu_id) {
                if !cbu.member_entities.contains(&ra.entity_id) {
                    cbu.member_entities.push(ra.entity_id);
                }
            }
        }

        // Build ownership edges
        let ownership_edges: Vec<OwnershipEdge> = ownership_rows
            .into_iter()
            .map(|r| {
                let ownership_type = r
                    .ownership_type
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(OwnershipType::Direct);

                OwnershipEdge::new(
                    r.from_entity_id,
                    r.to_entity_id,
                    r.percentage.unwrap_or_default(),
                    ownership_type,
                )
            })
            .collect();

        // Populate adjacency lists for ownership
        for edge in &ownership_edges {
            if let Some(node) = nodes.get_mut(&edge.to_entity_id) {
                node.owners.push(edge.from_entity_id);
            }
            if let Some(node) = nodes.get_mut(&edge.from_entity_id) {
                node.owned.push(edge.to_entity_id);
            }
        }

        // Build control edges
        let control_edges: Vec<ControlEdge> = control_rows
            .into_iter()
            .map(|r| {
                let control_type = r
                    .control_type
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(ControlType::BoardMember);

                ControlEdge::new(r.from_entity_id, r.to_entity_id, control_type)
            })
            .collect();

        // Populate adjacency lists for control
        for edge in &control_edges {
            if let Some(node) = nodes.get_mut(&edge.controlled_id) {
                node.controlled_by.push(edge.controller_id);
            }
            if let Some(node) = nodes.get_mut(&edge.controller_id) {
                node.controls.push(edge.controlled_id);
            }
        }

        // Build fund edges
        let fund_edges: Vec<FundEdge> = fund_rows
            .into_iter()
            .map(|r| {
                let relationship_type = r.relationship_type.parse().unwrap_or_default();
                FundEdge::new(r.parent_entity_id, r.child_entity_id, relationship_type)
            })
            .collect();

        // Mark termini (nodes with no owners) - set depth_from_terminus = 0
        for node in nodes.values_mut() {
            if node.owners.is_empty() {
                // Nodes with no owners are termini (depth 0)
                node.depth_from_terminus = Some(0);
            }
        }

        // Create the graph using the constructor then populate
        let mut graph = EntityGraph::new();
        graph.nodes = nodes;
        graph.cbus = cbu_nodes;
        graph.ownership_edges = ownership_edges;
        graph.control_edges = control_edges;
        graph.fund_edges = fund_edges;
        graph.role_assignments = role_assignments;

        graph
    }
}

#[async_trait]
impl GraphRepository for PgGraphRepository {
    async fn load_cbu_graph(&self, cbu_id: Uuid, as_of: NaiveDate) -> Result<EntityGraph> {
        // Load CBU
        let cbu = self.get_cbu(cbu_id).await?;
        let cbus = vec![cbu];

        // Load entities linked to this CBU
        let entities = self.load_cbu_entities(cbu_id).await?;

        // Collect entity IDs for relationship queries
        let entity_ids: HashSet<Uuid> = entities.iter().map(|e| e.entity_id).collect();

        // Load roles
        let roles = self.load_cbu_roles(cbu_id).await?;

        // Load relationships
        let ownership_rows = self.load_ownership_edges(&entity_ids, as_of).await?;
        let control_rows = self.load_control_edges(&entity_ids, as_of).await?;
        let fund_rows = self.load_fund_edges(&entity_ids).await?;

        // Build graph
        let mut graph = self.build_graph(
            cbus,
            entities,
            roles,
            ownership_rows,
            control_rows,
            fund_rows,
        );

        // Compute depth from terminus
        graph.compute_depths();

        Ok(graph)
    }

    async fn load_book_graph(&self, apex_entity_id: Uuid, as_of: NaiveDate) -> Result<EntityGraph> {
        // Find all entities owned by this apex (walk down ownership chain)
        // Then find all CBUs whose commercial_client_entity_id is in that set

        // First, walk ownership chain DOWN from apex to find all owned entities
        let owned_entities = sqlx::query_as::<_, OwnershipChainRow>(
            r#"
            WITH RECURSIVE ownership_tree AS (
                -- Base case: apex entity
                SELECT
                    e.entity_id,
                    e.name,
                    COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                    NULL::text as jurisdiction,
                    0 as depth
                FROM "ob-poc".entities e
                JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                WHERE e.entity_id = $1

                UNION ALL

                -- Recursive case: follow ownership downward
                SELECT
                    e.entity_id,
                    e.name,
                    COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                    NULL::text as jurisdiction,
                    ot.depth + 1 as depth
                FROM ownership_tree ot
                JOIN "ob-poc".entity_relationships er
                    ON er.from_entity_id = ot.entity_id
                    AND er.relationship_type = 'ownership'
                    AND (er.effective_from IS NULL OR er.effective_from <= $2)
                    AND (er.effective_to IS NULL OR er.effective_to >= $2)
                JOIN "ob-poc".entities e ON e.entity_id = er.to_entity_id
                JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                WHERE ot.depth < 20
            )
            SELECT entity_id, name, entity_type, jurisdiction, depth
            FROM ownership_tree
            "#,
        )
        .bind(apex_entity_id)
        .bind(as_of)
        .fetch_all(&self.pool)
        .await?;

        let owned_entity_ids: HashSet<Uuid> = owned_entities.iter().map(|e| e.entity_id).collect();

        // Find CBUs whose commercial_client is in the owned set
        let cbus: Vec<CbuRow> = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, jurisdiction, client_type, commercial_client_entity_id
            FROM "ob-poc".cbus
            WHERE commercial_client_entity_id = ANY($1)
            "#,
        )
        .bind(owned_entity_ids.iter().copied().collect::<Vec<_>>())
        .fetch_all(&self.pool)
        .await?;

        if cbus.is_empty() {
            return Ok(EntityGraph::default());
        }

        // Load entities for all CBUs
        let cbu_ids: Vec<Uuid> = cbus.iter().map(|c| c.cbu_id).collect();
        let mut all_entities: Vec<EntityRow> = vec![];
        let mut all_roles: Vec<RoleRow> = vec![];

        for cbu_id in &cbu_ids {
            all_entities.extend(self.load_cbu_entities(*cbu_id).await?);
            all_roles.extend(self.load_cbu_roles(*cbu_id).await?);
        }

        // Dedupe entities
        let mut seen_entities: HashSet<Uuid> = HashSet::new();
        all_entities.retain(|e| seen_entities.insert(e.entity_id));

        let entity_ids: HashSet<Uuid> = all_entities.iter().map(|e| e.entity_id).collect();

        // Load relationships
        let ownership_rows = self.load_ownership_edges(&entity_ids, as_of).await?;
        let control_rows = self.load_control_edges(&entity_ids, as_of).await?;
        let fund_rows = self.load_fund_edges(&entity_ids).await?;

        // Build graph
        let mut graph = self.build_graph(
            cbus,
            all_entities,
            all_roles,
            ownership_rows,
            control_rows,
            fund_rows,
        );

        graph.compute_depths();

        Ok(graph)
    }

    async fn load_jurisdiction_graph(
        &self,
        jurisdiction: &str,
        as_of: NaiveDate,
    ) -> Result<EntityGraph> {
        // Get all CBUs in this jurisdiction
        let cbus = self.get_cbus_by_jurisdiction(jurisdiction).await?;

        if cbus.is_empty() {
            return Ok(EntityGraph::default());
        }

        // Load entities for all CBUs
        let cbu_ids: Vec<Uuid> = cbus.iter().map(|c| c.cbu_id).collect();
        let mut all_entities: Vec<EntityRow> = vec![];
        let mut all_roles: Vec<RoleRow> = vec![];

        for cbu_id in &cbu_ids {
            all_entities.extend(self.load_cbu_entities(*cbu_id).await?);
            all_roles.extend(self.load_cbu_roles(*cbu_id).await?);
        }

        // Dedupe entities
        let mut seen_entities: HashSet<Uuid> = HashSet::new();
        all_entities.retain(|e| seen_entities.insert(e.entity_id));

        let entity_ids: HashSet<Uuid> = all_entities.iter().map(|e| e.entity_id).collect();

        // Load relationships
        let ownership_rows = self.load_ownership_edges(&entity_ids, as_of).await?;
        let control_rows = self.load_control_edges(&entity_ids, as_of).await?;
        let fund_rows = self.load_fund_edges(&entity_ids).await?;

        // Build graph
        let mut graph = self.build_graph(
            cbus,
            all_entities,
            all_roles,
            ownership_rows,
            control_rows,
            fund_rows,
        );

        graph.compute_depths();

        Ok(graph)
    }

    async fn load_neighborhood_graph(
        &self,
        entity_id: Uuid,
        hops: u32,
        as_of: NaiveDate,
    ) -> Result<EntityGraph> {
        // BFS to collect entities within N hops
        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut frontier: Vec<Uuid> = vec![entity_id];
        visited.insert(entity_id);

        for _ in 0..hops {
            if frontier.is_empty() {
                break;
            }

            // Get neighbors for current frontier
            let neighbors: Vec<Uuid> = sqlx::query_scalar(
                r#"
                SELECT DISTINCT neighbor_id
                FROM (
                    -- Ownership edges (both directions)
                    SELECT er.from_entity_id as neighbor_id
                    FROM "ob-poc".entity_relationships er
                    WHERE er.to_entity_id = ANY($1)
                      AND er.relationship_type = 'ownership'
                      AND (er.effective_from IS NULL OR er.effective_from <= $2)
                      AND (er.effective_to IS NULL OR er.effective_to >= $2)

                    UNION

                    SELECT er.to_entity_id as neighbor_id
                    FROM "ob-poc".entity_relationships er
                    WHERE er.from_entity_id = ANY($1)
                      AND er.relationship_type = 'ownership'
                      AND (er.effective_from IS NULL OR er.effective_from <= $2)
                      AND (er.effective_to IS NULL OR er.effective_to >= $2)

                    UNION

                    -- Control edges (both directions)
                    SELECT er.from_entity_id as neighbor_id
                    FROM "ob-poc".entity_relationships er
                    WHERE er.to_entity_id = ANY($1)
                      AND er.relationship_type = 'control'
                      AND (er.effective_from IS NULL OR er.effective_from <= $2)
                      AND (er.effective_to IS NULL OR er.effective_to >= $2)

                    UNION

                    SELECT er.to_entity_id as neighbor_id
                    FROM "ob-poc".entity_relationships er
                    WHERE er.from_entity_id = ANY($1)
                      AND er.relationship_type = 'control'
                      AND (er.effective_from IS NULL OR er.effective_from <= $2)
                      AND (er.effective_to IS NULL OR er.effective_to >= $2)
                ) neighbors
                "#,
            )
            .bind(&frontier)
            .bind(as_of)
            .fetch_all(&self.pool)
            .await?;

            // Update frontier with unvisited neighbors
            frontier = neighbors
                .into_iter()
                .filter(|id| visited.insert(*id))
                .collect();
        }

        // Load entity details for all visited nodes
        let entity_ids: Vec<Uuid> = visited.iter().copied().collect();
        let entities: Vec<EntityRow> = sqlx::query_as(
            r#"
            SELECT
                e.entity_id,
                e.name,
                COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                et.entity_category,
                COALESCE(
                    ep.jurisdiction,
                    elc.jurisdiction,
                    ept.jurisdiction,
                    etr.jurisdiction,
                    ef.jurisdiction
                ) as jurisdiction
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            LEFT JOIN "ob-poc".entity_proper_persons ep ON ep.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_partnerships ept ON ept.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_trusts etr ON etr.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
            WHERE e.entity_id = ANY($1)
            "#,
        )
        .bind(&entity_ids)
        .fetch_all(&self.pool)
        .await?;

        // Find CBUs that contain any of these entities
        let cbus: Vec<CbuRow> = sqlx::query_as(
            r#"
            SELECT DISTINCT c.cbu_id, c.name, c.jurisdiction, c.client_type, c.commercial_client_entity_id
            FROM "ob-poc".cbus c
            JOIN "ob-poc".cbu_entity_roles cer ON cer.cbu_id = c.cbu_id
            WHERE cer.entity_id = ANY($1)
            "#,
        )
        .bind(&entity_ids)
        .fetch_all(&self.pool)
        .await?;

        // Load roles for these CBUs
        let mut all_roles: Vec<RoleRow> = vec![];
        for cbu in &cbus {
            all_roles.extend(self.load_cbu_roles(cbu.cbu_id).await?);
        }

        // Filter roles to only entities in our set
        all_roles.retain(|r| visited.contains(&r.entity_id));

        let entity_id_set: HashSet<Uuid> = visited;

        // Load relationships
        let ownership_rows = self.load_ownership_edges(&entity_id_set, as_of).await?;
        let control_rows = self.load_control_edges(&entity_id_set, as_of).await?;
        let fund_rows = self.load_fund_edges(&entity_id_set).await?;

        // Build graph
        let mut graph = self.build_graph(
            cbus,
            entities,
            all_roles,
            ownership_rows,
            control_rows,
            fund_rows,
        );

        graph.compute_depths();

        Ok(graph)
    }

    async fn find_ownership_apex(&self, entity_id: Uuid, as_of: NaiveDate) -> Result<Option<Uuid>> {
        let chain = self.walk_ownership_chain_up(entity_id, as_of).await?;

        // The apex is the entity with the highest depth (furthest up the chain)
        // that has no further owners
        if let Some(last) = chain.last() {
            Ok(Some(last.entity_id))
        } else {
            Ok(None)
        }
    }

    async fn derive_books(&self, as_of: NaiveDate) -> Result<Vec<DerivedBook>> {
        // For each CBU, find its apex, then group CBUs by apex
        let cbus: Vec<CbuRow> = sqlx::query_as(
            r#"
            SELECT cbu_id, name, jurisdiction, client_type, commercial_client_entity_id
            FROM "ob-poc".cbus
            WHERE commercial_client_entity_id IS NOT NULL
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut apex_to_cbus: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut apex_info: HashMap<Uuid, (String, Option<String>)> = HashMap::new();

        for cbu in &cbus {
            if let Some(commercial_id) = cbu.commercial_client_entity_id {
                if let Some(apex_id) = self.find_ownership_apex(commercial_id, as_of).await? {
                    apex_to_cbus.entry(apex_id).or_default().push(cbu.cbu_id);

                    // Get apex entity info if we haven't already
                    if let std::collections::hash_map::Entry::Vacant(e) = apex_info.entry(apex_id) {
                        if let Some(chain_row) = self
                            .walk_ownership_chain_up(apex_id, as_of)
                            .await?
                            .into_iter()
                            .find(|r| r.entity_id == apex_id)
                        {
                            e.insert((chain_row.name, chain_row.jurisdiction));
                        }
                    }
                }
            }
        }

        let books: Vec<DerivedBook> = apex_to_cbus
            .into_iter()
            .map(|(apex_id, cbu_ids)| {
                let (name, jurisdiction) = apex_info
                    .get(&apex_id)
                    .cloned()
                    .unwrap_or(("Unknown".to_string(), None));
                DerivedBook {
                    apex_entity_id: apex_id,
                    apex_name: name,
                    apex_jurisdiction: jurisdiction,
                    cbu_count: cbu_ids.len(),
                    cbu_ids,
                }
            })
            .collect();

        Ok(books)
    }

    async fn search_entities(
        &self,
        name_pattern: &str,
        scope: &GraphScope,
        _as_of: NaiveDate,
    ) -> Result<Vec<GraphNode>> {
        let pattern = format!("%{}%", name_pattern);

        let rows: Vec<EntityRow> = match scope {
            GraphScope::SingleCbu { cbu_id, .. } => {
                sqlx::query_as(
                    r#"
                    SELECT DISTINCT
                        e.entity_id,
                        e.name,
                        COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                        et.entity_category,
                        NULL::text as jurisdiction
                    FROM "ob-poc".entities e
                    JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                    JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = e.entity_id
                    WHERE cer.cbu_id = $1
                      AND e.name ILIKE $2
                    "#,
                )
                .bind(cbu_id)
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?
            }
            GraphScope::Jurisdiction { code } => {
                sqlx::query_as(
                    r#"
                    SELECT DISTINCT
                        e.entity_id,
                        e.name,
                        COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                        et.entity_category,
                        NULL::text as jurisdiction
                    FROM "ob-poc".entities e
                    JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                    JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = e.entity_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
                    WHERE c.jurisdiction = $1
                      AND e.name ILIKE $2
                    "#,
                )
                .bind(code)
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?
            }
            _ => {
                // Global search
                sqlx::query_as(
                    r#"
                    SELECT
                        e.entity_id,
                        e.name,
                        COALESCE(et.type_code, 'UNKNOWN') as entity_type,
                        et.entity_category,
                        NULL::text as jurisdiction
                    FROM "ob-poc".entities e
                    JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
                    WHERE e.name ILIKE $1
                    LIMIT 100
                    "#,
                )
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?
            }
        };

        let nodes: Vec<GraphNode> = rows
            .into_iter()
            .map(|e| {
                let mut node = GraphNode::new(
                    e.entity_id,
                    e.name,
                    e.entity_type.parse().unwrap_or_default(),
                );
                node.jurisdiction = e.jurisdiction;
                node
            })
            .collect();

        Ok(nodes)
    }

    async fn find_person_roles(
        &self,
        person_name: &str,
        scope: &GraphScope,
        _as_of: NaiveDate,
    ) -> Result<Vec<RoleAssignment>> {
        let pattern = format!("%{}%", person_name);

        let rows: Vec<RoleRow> = match scope {
            GraphScope::SingleCbu { cbu_id, .. } => {
                sqlx::query_as(
                    r#"
                    SELECT
                        cer.cbu_id,
                        cer.entity_id,
                        r.name as role_name,
                        r.role_category,
                        cer.ownership_percentage
                    FROM "ob-poc".cbu_entity_roles cer
                    JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                    JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
                    WHERE cer.cbu_id = $1
                      AND e.name ILIKE $2
                    "#,
                )
                .bind(cbu_id)
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?
            }
            GraphScope::Jurisdiction { code } => {
                sqlx::query_as(
                    r#"
                    SELECT
                        cer.cbu_id,
                        cer.entity_id,
                        r.name as role_name,
                        r.role_category,
                        cer.ownership_percentage
                    FROM "ob-poc".cbu_entity_roles cer
                    JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                    JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
                    JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
                    WHERE c.jurisdiction = $1
                      AND e.name ILIKE $2
                    "#,
                )
                .bind(code)
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?
            }
            _ => {
                sqlx::query_as(
                    r#"
                    SELECT
                        cer.cbu_id,
                        cer.entity_id,
                        r.name as role_name,
                        r.role_category,
                        cer.ownership_percentage
                    FROM "ob-poc".cbu_entity_roles cer
                    JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                    JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
                    WHERE e.name ILIKE $1
                    LIMIT 100
                    "#,
                )
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?
            }
        };

        let assignments: Vec<RoleAssignment> = rows
            .into_iter()
            .map(|r| RoleAssignment {
                id: Uuid::new_v4(),
                cbu_id: r.cbu_id,
                entity_id: r.entity_id,
                role: r.role_name,
                role_category: r.role_category.as_deref().and_then(|s| s.parse().ok()),
                ownership_percentage: r.ownership_percentage,
                effective_from: None,
                effective_to: None,
                visible: true,
            })
            .collect();

        Ok(assignments)
    }
}
