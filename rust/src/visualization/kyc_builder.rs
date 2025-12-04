//! KYC/UBO Tree Builder
//!
//! Builds hierarchical tree visualization for KYC/UBO view.
//! Queries ALL entities linked to a CBU via roles and organizes them
//! into a hierarchical structure.
//!
//! NOTE: All database access goes through VisualizationRepository.
//! This builder only handles tree assembly logic.

use super::types::*;
use crate::database::{
    CbuView, EntityView, EntityWithRoleView, HoldingView, OfficerView, ShareClassView,
    VisualizationRepository,
};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Builder for KYC/UBO tree visualization
pub struct KycTreeBuilder {
    repo: VisualizationRepository,
}

impl KycTreeBuilder {
    pub fn new(repo: VisualizationRepository) -> Self {
        Self { repo }
    }

    pub async fn build(&self, cbu_id: Uuid) -> Result<CbuVisualization> {
        // Load CBU
        let cbu = self.repo.get_cbu_for_tree(cbu_id).await?;

        // Load ALL entities linked to this CBU via any role
        let all_linked = self.repo.get_all_linked_entities(cbu_id).await?;

        // Group entities by their roles
        let entities_by_role = group_by_role(&all_linked);

        // Build tree based on client type
        let client_type = cbu.client_type.as_deref().unwrap_or("UNKNOWN");
        let (root, overlay_edges) = match client_type.to_uppercase().as_str() {
            "HEDGE_FUND" | "40_ACT" | "UCITS" | "PE_FUND" | "VC_FUND" | "FUND" => {
                self.build_fund_tree(cbu_id, &cbu, &entities_by_role)
                    .await?
            }
            "TRUST" => {
                self.build_trust_tree(cbu_id, &cbu, &entities_by_role)
                    .await?
            }
            _ => {
                self.build_corporate_tree(cbu_id, &cbu, &entities_by_role)
                    .await?
            }
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

    // ==========================================================================
    // FUND TREE
    // ==========================================================================

    async fn build_fund_tree(
        &self,
        cbu_id: Uuid,
        cbu: &CbuView,
        entities_by_role: &HashMap<String, Vec<&EntityWithRoleView>>,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load commercial client
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.repo.get_entity(cc_id).await?;
            Some(self.entity_to_node(&entity, TreeNodeType::CommercialClient, "Commercial Client"))
        } else {
            // Check if COMMERCIAL_CLIENT role exists
            entities_by_role.get("COMMERCIAL_CLIENT").and_then(|v| {
                v.first().map(|e| {
                    self.entity_with_role_to_node(
                        e,
                        TreeNodeType::CommercialClient,
                        "Commercial Client",
                    )
                })
            })
        };

        // Load ManCo (INVESTMENT_MANAGER role)
        let manco = entities_by_role.get("INVESTMENT_MANAGER").and_then(|v| {
            v.first().map(|e| {
                self.entity_with_role_to_node(e, TreeNodeType::ManCo, "Management Company")
            })
        });

        // Load fund entity (PRINCIPAL role)
        let principal = entities_by_role.get("PRINCIPAL").and_then(|v| {
            v.first()
                .map(|e| self.entity_with_role_to_node(e, TreeNodeType::FundEntity, "Fund"))
        });

        // Load share classes
        let share_classes = self.repo.get_share_classes(cbu_id).await?;
        let share_class_nodes: Vec<TreeNode> = share_classes
            .iter()
            .filter(|sc| sc.class_category.as_deref() == Some("FUND"))
            .map(|sc| self.share_class_to_node(sc))
            .collect();

        // Load officers (DIRECTOR, OFFICER, AUTHORIZED_SIGNATORY, etc.)
        let officers = self.repo.get_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> =
            officers.iter().map(|o| self.officer_to_node(o)).collect();

        // Load shareholders
        let shareholder_nodes: Vec<TreeNode> = entities_by_role
            .get("SHAREHOLDER")
            .map(|v| {
                v.iter()
                    .map(|e| {
                        self.entity_with_role_to_node(
                            e,
                            self.node_type_for_entity(e),
                            "Shareholder",
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Load beneficial owners
        let ubo_nodes: Vec<TreeNode> = entities_by_role
            .get("BENEFICIAL_OWNER")
            .map(|v| {
                v.iter()
                    .map(|e| {
                        self.entity_with_role_to_node(e, TreeNodeType::Person, "Beneficial Owner")
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Load holdings for overlay edges
        let holdings = self.repo.get_holdings(cbu_id).await?;
        self.add_holding_edges(&holdings, &mut overlay_edges);

        // Build fund node with share classes + shareholders + UBOs as children
        let fund_node = principal.map(|mut p| {
            p.children = share_class_nodes.clone();
            p.children.extend(shareholder_nodes.clone());
            p.children.extend(ubo_nodes.clone());
            p
        });

        // Build ManCo node with fund + officers as children
        let manco_node = manco.map(|mut m| {
            let mut children = Vec::new();
            if let Some(fund) = fund_node.clone() {
                children.push(fund);
            }
            children.extend(officer_nodes.clone());
            m.children = children;
            m
        });

        // Assemble final tree
        let root = if let Some(mut cc) = commercial_client {
            if let Some(manco) = manco_node {
                cc.children.push(manco);
            } else if let Some(fund) = fund_node {
                cc.children.push(fund);
            }
            // Add any remaining entities not yet in tree
            self.add_remaining_entities(
                &mut cc,
                entities_by_role,
                &officer_nodes,
                &shareholder_nodes,
                &ubo_nodes,
            );
            cc
        } else if let Some(mut manco) = manco_node {
            self.add_remaining_entities(
                &mut manco,
                entities_by_role,
                &officer_nodes,
                &shareholder_nodes,
                &ubo_nodes,
            );
            manco
        } else if let Some(mut fund) = fund_node {
            self.add_remaining_entities(
                &mut fund,
                entities_by_role,
                &officer_nodes,
                &shareholder_nodes,
                &ubo_nodes,
            );
            fund
        } else {
            // Fallback: CBU as root with all entities
            let mut all_children = officer_nodes;
            all_children.extend(shareholder_nodes);
            all_children.extend(ubo_nodes);
            self.add_all_entities_as_children(&mut all_children, entities_by_role);

            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: cbu.client_type.clone(),
                jurisdiction: cbu.jurisdiction.clone(),
                children: all_children,
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
        cbu: &CbuView,
        entities_by_role: &HashMap<String, Vec<&EntityWithRoleView>>,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let overlay_edges = Vec::new();

        // Load trustee
        let trustee = entities_by_role.get("TRUSTEE").and_then(|v| {
            v.first()
                .map(|e| self.entity_with_role_to_node(e, TreeNodeType::ManCo, "Trustee"))
        });

        // Load trust entity (PRINCIPAL)
        let principal = entities_by_role.get("PRINCIPAL").and_then(|v| {
            v.first()
                .map(|e| self.entity_with_role_to_node(e, TreeNodeType::TrustEntity, "Trust"))
        });

        // Load protector
        let protector_nodes: Vec<TreeNode> = entities_by_role
            .get("PROTECTOR")
            .map(|v| {
                v.iter()
                    .map(|e| self.entity_with_role_to_node(e, TreeNodeType::Person, "Protector"))
                    .collect()
            })
            .unwrap_or_default();

        // Load settlors
        let settlor_nodes: Vec<TreeNode> = entities_by_role
            .get("SETTLOR")
            .map(|v| {
                v.iter()
                    .map(|e| self.entity_with_role_to_node(e, TreeNodeType::Person, "Settlor"))
                    .collect()
            })
            .unwrap_or_default();

        // Load beneficiaries
        let beneficiary_nodes: Vec<TreeNode> = entities_by_role
            .get("BENEFICIARY")
            .map(|v| {
                v.iter()
                    .map(|e| self.entity_with_role_to_node(e, TreeNodeType::Person, "Beneficiary"))
                    .collect()
            })
            .unwrap_or_default();

        // Build trust node with settlors and beneficiaries as children
        let mut trust_children = protector_nodes;
        trust_children.extend(settlor_nodes);
        trust_children.extend(beneficiary_nodes);

        let trust_node = principal.map(|mut t| {
            t.children = trust_children.clone();
            t
        });

        // Build tree: Trustee → Trust
        let root = if let Some(mut t) = trustee {
            let mut children = Vec::new();
            if let Some(trust) = trust_node {
                children.push(trust);
            }
            t.children = children;
            t
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
        cbu: &CbuView,
        entities_by_role: &HashMap<String, Vec<&EntityWithRoleView>>,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load commercial client (parent company)
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.repo.get_entity(cc_id).await?;
            Some(self.entity_to_node(&entity, TreeNodeType::CommercialClient, "Parent Company"))
        } else {
            entities_by_role.get("COMMERCIAL_CLIENT").and_then(|v| {
                v.first().map(|e| {
                    self.entity_with_role_to_node(
                        e,
                        TreeNodeType::CommercialClient,
                        "Parent Company",
                    )
                })
            })
        };

        // Load principal (subsidiary)
        let principal = entities_by_role.get("PRINCIPAL").and_then(|v| {
            v.first()
                .map(|e| self.entity_with_role_to_node(e, TreeNodeType::FundEntity, "Subsidiary"))
        });

        // Load officers
        let officers = self.repo.get_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> =
            officers.iter().map(|o| self.officer_to_node(o)).collect();

        // Load shareholders
        let shareholder_nodes: Vec<TreeNode> = entities_by_role
            .get("SHAREHOLDER")
            .map(|v| {
                v.iter()
                    .map(|e| {
                        self.entity_with_role_to_node(
                            e,
                            self.node_type_for_entity(e),
                            "Shareholder",
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Load beneficial owners
        let ubo_nodes: Vec<TreeNode> = entities_by_role
            .get("BENEFICIAL_OWNER")
            .map(|v| {
                v.iter()
                    .map(|e| {
                        self.entity_with_role_to_node(e, TreeNodeType::Person, "Beneficial Owner")
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Load share classes
        let share_classes = self.repo.get_share_classes(cbu_id).await?;
        let share_class_nodes: Vec<TreeNode> = share_classes
            .iter()
            .map(|sc| self.share_class_to_node(sc))
            .collect();

        // Holdings for overlay
        let holdings = self.repo.get_holdings(cbu_id).await?;
        self.add_holding_edges(&holdings, &mut overlay_edges);

        // Build principal node
        let principal_node = principal.map(|mut p| {
            let mut children = share_class_nodes.clone();
            children.extend(officer_nodes.clone());
            children.extend(shareholder_nodes.clone());
            children.extend(ubo_nodes.clone());
            p.children = children;
            p
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
            let mut all_children = officer_nodes;
            all_children.extend(shareholder_nodes);
            all_children.extend(ubo_nodes);

            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: Some("Corporate".to_string()),
                jurisdiction: cbu.jurisdiction.clone(),
                children: all_children,
                metadata: Default::default(),
            }
        };

        Ok((root, overlay_edges))
    }

    // ==========================================================================
    // HELPER METHODS - Convert repository views to tree nodes
    // ==========================================================================

    fn entity_to_node(
        &self,
        entity: &EntityView,
        node_type: TreeNodeType,
        sublabel: &str,
    ) -> TreeNode {
        TreeNode {
            id: entity.entity_id,
            node_type,
            label: entity.name.clone(),
            sublabel: Some(sublabel.to_string()),
            jurisdiction: entity.jurisdiction.clone(),
            children: vec![],
            metadata: Default::default(),
        }
    }

    fn entity_with_role_to_node(
        &self,
        entity: &EntityWithRoleView,
        node_type: TreeNodeType,
        sublabel: &str,
    ) -> TreeNode {
        TreeNode {
            id: entity.entity_id,
            node_type,
            label: entity.name.clone(),
            sublabel: Some(sublabel.to_string()),
            jurisdiction: entity.jurisdiction.clone(),
            children: vec![],
            metadata: Default::default(),
        }
    }

    fn node_type_for_entity(&self, entity: &EntityWithRoleView) -> TreeNodeType {
        match entity.entity_type.as_str() {
            t if t.contains("PROPER_PERSON") => TreeNodeType::Person,
            t if t.contains("TRUST") => TreeNodeType::TrustEntity,
            _ => TreeNodeType::FundEntity, // Companies, partnerships, etc.
        }
    }

    fn officer_to_node(&self, officer: &OfficerView) -> TreeNode {
        TreeNode {
            id: officer.entity_id,
            node_type: TreeNodeType::Person,
            label: officer.name.clone(),
            sublabel: Some(officer.roles.join(", ")),
            jurisdiction: officer.nationality.clone(),
            children: vec![],
            metadata: Default::default(),
        }
    }

    fn share_class_to_node(&self, sc: &ShareClassView) -> TreeNode {
        TreeNode {
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
        }
    }

    fn add_holding_edges(&self, holdings: &[HoldingView], edges: &mut Vec<TreeEdge>) {
        for holding in holdings {
            edges.push(TreeEdge {
                from: holding.investor_entity_id,
                to: holding.share_class_id,
                edge_type: TreeEdgeType::Owns,
                label: Some(format!("{} units", holding.units)),
                weight: None,
            });
        }
    }

    /// Add any entities not yet included in the tree
    fn add_remaining_entities(
        &self,
        root: &mut TreeNode,
        entities_by_role: &HashMap<String, Vec<&EntityWithRoleView>>,
        officers: &[TreeNode],
        shareholders: &[TreeNode],
        ubos: &[TreeNode],
    ) {
        // Collect IDs already in tree
        let mut existing_ids: HashSet<Uuid> = HashSet::new();
        collect_node_ids(root, &mut existing_ids);
        for o in officers {
            existing_ids.insert(o.id);
        }
        for s in shareholders {
            existing_ids.insert(s.id);
        }
        for u in ubos {
            existing_ids.insert(u.id);
        }

        // Add any entities not yet in tree
        for (role, entities) in entities_by_role {
            // Skip roles we've already handled
            if matches!(
                role.as_str(),
                "PRINCIPAL"
                    | "INVESTMENT_MANAGER"
                    | "COMMERCIAL_CLIENT"
                    | "DIRECTOR"
                    | "OFFICER"
                    | "AUTHORIZED_SIGNATORY"
                    | "SHAREHOLDER"
                    | "BENEFICIAL_OWNER"
                    | "TRUSTEE"
                    | "SETTLOR"
                    | "BENEFICIARY"
                    | "PROTECTOR"
            ) {
                continue;
            }

            for entity in entities {
                if !existing_ids.contains(&entity.entity_id) {
                    existing_ids.insert(entity.entity_id);
                    root.children.push(self.entity_with_role_to_node(
                        entity,
                        self.node_type_for_entity(entity),
                        role,
                    ));
                }
            }
        }
    }

    /// Add all entities as children (for fallback case)
    fn add_all_entities_as_children(
        &self,
        children: &mut Vec<TreeNode>,
        entities_by_role: &HashMap<String, Vec<&EntityWithRoleView>>,
    ) {
        let mut existing_ids: HashSet<Uuid> = children.iter().map(|c| c.id).collect();

        for (role, entities) in entities_by_role {
            for entity in entities {
                if !existing_ids.contains(&entity.entity_id) {
                    existing_ids.insert(entity.entity_id);
                    children.push(self.entity_with_role_to_node(
                        entity,
                        self.node_type_for_entity(entity),
                        role,
                    ));
                }
            }
        }
    }
}

/// Group entities by their role name
fn group_by_role<'a>(
    entities: &'a [EntityWithRoleView],
) -> HashMap<String, Vec<&'a EntityWithRoleView>> {
    let mut map: HashMap<String, Vec<&'a EntityWithRoleView>> = HashMap::new();
    for entity in entities {
        map.entry(entity.role_name.clone())
            .or_default()
            .push(entity);
    }
    map
}

/// Recursively collect all node IDs in a tree
fn collect_node_ids(node: &TreeNode, ids: &mut HashSet<Uuid>) {
    ids.insert(node.id);
    for child in &node.children {
        collect_node_ids(child, ids);
    }
}
