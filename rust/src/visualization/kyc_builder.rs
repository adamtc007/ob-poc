//! KYC/UBO Tree Builder
//!
//! Builds hierarchical tree visualization for KYC/UBO view.
//! Structure varies by client type:
//! - Fund: CommercialClient → ManCo → Fund → ShareClasses + Officers
//! - Trust: Trustee → Trust → Settlor + Beneficiaries
//! - Corporate: Parent → Subsidiary → ShareClasses + Officers

use super::types::*;
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Record types for database queries
#[derive(Debug, sqlx::FromRow)]
struct CbuRecord {
    #[allow(dead_code)]
    cbu_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
    commercial_client_entity_id: Option<Uuid>,
}

#[derive(Debug)]
struct EntityRecord {
    entity_id: Uuid,
    name: String,
    jurisdiction: Option<String>,
    #[allow(dead_code)]
    entity_type: String,
}

#[derive(Debug)]
struct OfficerRecord {
    entity_id: Uuid,
    name: String,
    nationality: Option<String>,
    roles: Vec<String>,
}

#[derive(Debug)]
struct ShareClassRecord {
    id: Uuid,
    name: String,
    currency: String,
    class_category: Option<String>,
    isin: Option<String>,
    nav_per_share: Option<String>, // Convert from Decimal to String for simplicity
    fund_type: Option<String>,
}

#[derive(Debug)]
struct HoldingRecord {
    investor_entity_id: Uuid,
    share_class_id: Uuid,
    units: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ControlRecord {
    controller_entity_id: Uuid,
    controlled_entity_id: Uuid,
    control_type: String,
}

/// Builder for KYC/UBO tree visualization
pub struct KycTreeBuilder {
    pool: PgPool,
}

impl KycTreeBuilder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn build(&self, cbu_id: Uuid) -> Result<CbuVisualization> {
        // Load CBU
        let cbu = self.load_cbu(cbu_id).await?;
        let client_type = cbu.client_type.as_deref().unwrap_or("UNKNOWN");

        // Build tree based on client type
        let (root, overlay_edges) = match client_type.to_uppercase().as_str() {
            "HEDGE_FUND" | "40_ACT" | "UCITS" | "PE_FUND" | "VC_FUND" | "FUND" => {
                self.build_fund_tree(cbu_id, &cbu).await?
            }
            "TRUST" => self.build_trust_tree(cbu_id, &cbu).await?,
            _ => self.build_corporate_tree(cbu_id, &cbu).await?,
        };

        let stats = VisualizationStats::from_tree(&root, &overlay_edges);

        Ok(CbuVisualization {
            cbu_id,
            cbu_name: cbu.name,
            client_type: cbu.client_type,
            jurisdiction: cbu.jurisdiction,
            view_mode: ViewMode::KycUbo,
            root,
            overlay_edges,
            stats,
        })
    }

    async fn load_cbu(&self, cbu_id: Uuid) -> Result<CbuRecord> {
        let cbu = sqlx::query_as!(
            CbuRecord,
            r#"SELECT cbu_id, name, jurisdiction, client_type, commercial_client_entity_id
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(cbu)
    }

    async fn load_entity(&self, entity_id: Uuid) -> Result<EntityRecord> {
        let entity = sqlx::query_as!(
            EntityRecord,
            r#"SELECT e.entity_id, e.name,
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
                      et.type_code as "entity_type!"
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               WHERE e.entity_id = $1"#,
            entity_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(entity)
    }

    async fn load_entity_by_role(&self, cbu_id: Uuid, role: &str) -> Result<Option<EntityRecord>> {
        let entity = sqlx::query_as!(
            EntityRecord,
            r#"SELECT e.entity_id, e.name,
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
                      et.type_code as "entity_type!"
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               WHERE cer.cbu_id = $1 AND r.name = $2
               LIMIT 1"#,
            cbu_id,
            role
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(entity)
    }

    async fn load_entities_by_role(&self, cbu_id: Uuid, role: &str) -> Result<Vec<EntityRecord>> {
        let entities = sqlx::query_as!(
            EntityRecord,
            r#"SELECT e.entity_id, e.name,
                      COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction, pp.nationality) as jurisdiction,
                      et.type_code as "entity_type!"
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
               LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
               LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               WHERE cer.cbu_id = $1 AND r.name = $2"#,
            cbu_id,
            role
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(entities)
    }

    async fn load_officers(&self, cbu_id: Uuid) -> Result<Vec<OfficerRecord>> {
        // Load persons with their roles
        let rows = sqlx::query!(
            r#"SELECT e.entity_id, e.name, pp.nationality, r.name as role_name
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               LEFT JOIN "ob-poc".entity_proper_persons pp ON e.entity_id = pp.entity_id
               WHERE cer.cbu_id = $1 AND et.type_code LIKE 'PROPER_PERSON%'
               ORDER BY e.name, r.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Group by person
        let mut officers: std::collections::HashMap<Uuid, OfficerRecord> =
            std::collections::HashMap::new();
        for row in rows {
            let entry = officers
                .entry(row.entity_id)
                .or_insert_with(|| OfficerRecord {
                    entity_id: row.entity_id,
                    name: row.name.clone(),
                    nationality: row.nationality.clone(),
                    roles: Vec::new(),
                });
            entry.roles.push(row.role_name);
        }

        Ok(officers.into_values().collect())
    }

    async fn load_share_classes(&self, cbu_id: Uuid) -> Result<Vec<ShareClassRecord>> {
        let classes = sqlx::query_as!(
            ShareClassRecord,
            r#"SELECT id, name, currency as "currency!", class_category, isin,
                      nav_per_share::text as nav_per_share, fund_type
               FROM kyc.share_classes
               WHERE cbu_id = $1
               ORDER BY class_category DESC, name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(classes)
    }

    async fn load_holdings(&self, cbu_id: Uuid) -> Result<Vec<HoldingRecord>> {
        let holdings = sqlx::query_as!(
            HoldingRecord,
            r#"SELECT h.investor_entity_id, h.share_class_id, h.units::text as "units!"
               FROM kyc.holdings h
               JOIN kyc.share_classes sc ON h.share_class_id = sc.id
               WHERE sc.cbu_id = $1 AND h.status = 'active'"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(holdings)
    }

    async fn load_control_relationships(&self, cbu_id: Uuid) -> Result<Vec<ControlRecord>> {
        // Get entities linked to this CBU, then find control relationships
        let controls = sqlx::query_as!(
            ControlRecord,
            r#"SELECT cr.controller_entity_id, cr.controlled_entity_id, cr.control_type
               FROM "ob-poc".control_relationships cr
               WHERE cr.is_active = true
               AND (cr.controller_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               ) OR cr.controlled_entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               ))"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(controls)
    }

    // ==========================================================================
    // FUND TREE
    // ==========================================================================

    async fn build_fund_tree(
        &self,
        cbu_id: Uuid,
        cbu: &CbuRecord,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load commercial client
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.load_entity(cc_id).await?;
            Some(TreeNode {
                id: cc_id,
                node_type: TreeNodeType::CommercialClient,
                label: entity.name,
                sublabel: Some("Commercial Client".to_string()),
                jurisdiction: entity.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            })
        } else {
            None
        };

        // Load ManCo (INVESTMENT_MANAGER role)
        let manco = self
            .load_entity_by_role(cbu_id, "INVESTMENT_MANAGER")
            .await?;

        // Load fund entity (PRINCIPAL role)
        let principal = self.load_entity_by_role(cbu_id, "PRINCIPAL").await?;

        // Load share classes
        let share_classes = self.load_share_classes(cbu_id).await?;
        let share_class_nodes: Vec<TreeNode> = share_classes
            .iter()
            .filter(|sc| sc.class_category.as_deref() == Some("FUND"))
            .map(|sc| TreeNode {
                id: sc.id,
                node_type: TreeNodeType::ShareClass,
                label: sc.name.clone(),
                sublabel: Some(format!(
                    "{} {}",
                    sc.currency,
                    sc.fund_type.as_deref().unwrap_or("")
                )),
                jurisdiction: None,
                children: vec![],
                metadata: [
                    ("isin".to_string(), serde_json::json!(sc.isin)),
                    ("nav".to_string(), serde_json::json!(sc.nav_per_share)),
                ]
                .into_iter()
                .collect(),
            })
            .collect();

        // Load officers
        let officers = self.load_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> = officers
            .iter()
            .map(|o| TreeNode {
                id: o.entity_id,
                node_type: TreeNodeType::Person,
                label: o.name.clone(),
                sublabel: Some(o.roles.join(", ")),
                jurisdiction: o.nationality.clone(),
                children: vec![],
                metadata: Default::default(),
            })
            .collect();

        // Load holdings for overlay edges
        let holdings = self.load_holdings(cbu_id).await?;
        for holding in holdings {
            overlay_edges.push(TreeEdge {
                from: holding.investor_entity_id,
                to: holding.share_class_id,
                edge_type: TreeEdgeType::Owns,
                label: Some(format!("{} units", holding.units)),
                weight: None,
            });
        }

        // Build fund node with share classes as children
        let fund_node = principal.map(|p| TreeNode {
            id: p.entity_id,
            node_type: TreeNodeType::FundEntity,
            label: p.name,
            sublabel: Some("Fund".to_string()),
            jurisdiction: p.jurisdiction,
            children: share_class_nodes.clone(),
            metadata: Default::default(),
        });

        // Build ManCo node with fund + officers as children
        let manco_node = manco.map(|m| {
            let mut children = Vec::new();
            if let Some(fund) = fund_node.clone() {
                children.push(fund);
            }
            children.extend(officer_nodes.clone());

            TreeNode {
                id: m.entity_id,
                node_type: TreeNodeType::ManCo,
                label: m.name,
                sublabel: Some("Management Company".to_string()),
                jurisdiction: m.jurisdiction,
                children,
                metadata: Default::default(),
            }
        });

        // Assemble final tree
        let root = if let Some(mut cc) = commercial_client {
            if let Some(manco) = manco_node {
                cc.children.push(manco);
            } else if let Some(fund) = fund_node {
                cc.children.push(fund);
            }
            cc
        } else if let Some(manco) = manco_node {
            manco
        } else if let Some(fund) = fund_node {
            fund
        } else {
            // Fallback: CBU as root with officers
            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: cbu.client_type.clone(),
                jurisdiction: cbu.jurisdiction.clone(),
                children: officer_nodes,
                metadata: Default::default(),
            }
        };

        Ok((root, overlay_edges))
    }

    // ==========================================================================
    // TRUST TREE
    // ==========================================================================

    async fn build_trust_tree(
        &self,
        cbu_id: Uuid,
        cbu: &CbuRecord,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load trustee
        let trustee = self.load_entity_by_role(cbu_id, "TRUSTEE").await?;

        // Load trust entity (PRINCIPAL)
        let principal = self.load_entity_by_role(cbu_id, "PRINCIPAL").await?;

        // Load settlor
        let settlors = self.load_entities_by_role(cbu_id, "SETTLOR").await?;

        // Load beneficiaries
        let beneficiaries = self.load_entities_by_role(cbu_id, "BENEFICIARY").await?;

        let beneficiary_nodes: Vec<TreeNode> = beneficiaries
            .into_iter()
            .map(|b| TreeNode {
                id: b.entity_id,
                node_type: TreeNodeType::Person,
                label: b.name,
                sublabel: Some("Beneficiary".to_string()),
                jurisdiction: b.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            })
            .collect();

        let settlor_nodes: Vec<TreeNode> = settlors
            .into_iter()
            .map(|s| TreeNode {
                id: s.entity_id,
                node_type: TreeNodeType::Person,
                label: s.name,
                sublabel: Some("Settlor".to_string()),
                jurisdiction: s.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            })
            .collect();

        // Build trust node with settlors and beneficiaries as children
        let mut trust_children = settlor_nodes;
        trust_children.extend(beneficiary_nodes);

        let trust_node = principal.map(|t| TreeNode {
            id: t.entity_id,
            node_type: TreeNodeType::TrustEntity,
            label: t.name,
            sublabel: Some("Trust".to_string()),
            jurisdiction: t.jurisdiction,
            children: trust_children.clone(),
            metadata: Default::default(),
        });

        // Load control relationships for overlay
        let controls = self.load_control_relationships(cbu_id).await?;
        for ctrl in controls {
            overlay_edges.push(TreeEdge {
                from: ctrl.controller_entity_id,
                to: ctrl.controlled_entity_id,
                edge_type: TreeEdgeType::Controls,
                label: Some(ctrl.control_type),
                weight: None,
            });
        }

        // Build tree: Trustee → Trust
        let root = if let Some(t) = trustee {
            let mut children = Vec::new();
            if let Some(trust) = trust_node {
                children.push(trust);
            }
            TreeNode {
                id: t.entity_id,
                node_type: TreeNodeType::ManCo, // Trustee acts like ManCo structurally
                label: t.name,
                sublabel: Some("Trustee".to_string()),
                jurisdiction: t.jurisdiction,
                children,
                metadata: Default::default(),
            }
        } else if let Some(trust) = trust_node {
            trust
        } else {
            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: Some("Trust".to_string()),
                jurisdiction: cbu.jurisdiction.clone(),
                children: trust_children,
                metadata: Default::default(),
            }
        };

        Ok((root, overlay_edges))
    }

    // ==========================================================================
    // CORPORATE TREE
    // ==========================================================================

    async fn build_corporate_tree(
        &self,
        cbu_id: Uuid,
        cbu: &CbuRecord,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load commercial client (parent company)
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.load_entity(cc_id).await?;
            Some(TreeNode {
                id: cc_id,
                node_type: TreeNodeType::CommercialClient,
                label: entity.name,
                sublabel: Some("Parent Company".to_string()),
                jurisdiction: entity.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            })
        } else {
            None
        };

        // Load principal (subsidiary)
        let principal = self.load_entity_by_role(cbu_id, "PRINCIPAL").await?;

        // Load officers
        let officers = self.load_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> = officers
            .iter()
            .map(|o| TreeNode {
                id: o.entity_id,
                node_type: TreeNodeType::Person,
                label: o.name.clone(),
                sublabel: Some(o.roles.join(", ")),
                jurisdiction: o.nationality.clone(),
                children: vec![],
                metadata: Default::default(),
            })
            .collect();

        // Load share classes
        let share_classes = self.load_share_classes(cbu_id).await?;
        let share_class_nodes: Vec<TreeNode> = share_classes
            .iter()
            .map(|sc| TreeNode {
                id: sc.id,
                node_type: TreeNodeType::ShareClass,
                label: sc.name.clone(),
                sublabel: Some(format!(
                    "{} - {}",
                    sc.currency,
                    sc.class_category.as_deref().unwrap_or("")
                )),
                jurisdiction: None,
                children: vec![],
                metadata: Default::default(),
            })
            .collect();

        // Holdings for overlay
        let holdings = self.load_holdings(cbu_id).await?;
        for holding in holdings {
            overlay_edges.push(TreeEdge {
                from: holding.investor_entity_id,
                to: holding.share_class_id,
                edge_type: TreeEdgeType::Owns,
                label: Some(format!("{} units", holding.units)),
                weight: None,
            });
        }

        // Build principal node
        let principal_node = principal.map(|p| {
            let mut children = share_class_nodes.clone();
            children.extend(officer_nodes.clone());
            TreeNode {
                id: p.entity_id,
                node_type: TreeNodeType::FundEntity,
                label: p.name,
                sublabel: Some("Subsidiary".to_string()),
                jurisdiction: p.jurisdiction,
                children,
                metadata: Default::default(),
            }
        });

        // Assemble tree
        let root = if let Some(mut cc) = commercial_client {
            if let Some(p) = principal_node {
                cc.children.push(p);
            }
            cc
        } else if let Some(p) = principal_node {
            p
        } else {
            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: Some("Corporate".to_string()),
                jurisdiction: cbu.jurisdiction.clone(),
                children: officer_nodes,
                metadata: Default::default(),
            }
        };

        Ok((root, overlay_edges))
    }
}
