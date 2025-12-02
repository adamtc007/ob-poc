//! KYC/UBO Tree Builder
//!
//! Builds hierarchical tree visualization for KYC/UBO view.
//! Structure varies by client type:
//! - Fund: CommercialClient → ManCo → Fund → ShareClasses + Officers
//! - Trust: Trustee → Trust → Settlor + Beneficiaries
//! - Corporate: Parent → Subsidiary → ShareClasses + Officers
//!
//! NOTE: All database access goes through VisualizationRepository.
//! This builder only handles tree assembly logic.

use super::types::*;
use crate::database::{
    CbuView, ControlRelationshipView, EntityView, HoldingView, OfficerView, ShareClassView,
    VisualizationRepository,
};
use anyhow::Result;
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

    // ==========================================================================
    // FUND TREE
    // ==========================================================================

    async fn build_fund_tree(
        &self,
        cbu_id: Uuid,
        cbu: &CbuView,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load commercial client
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.repo.get_entity(cc_id).await?;
            Some(self.entity_to_node(&entity, TreeNodeType::CommercialClient, "Commercial Client"))
        } else {
            None
        };

        // Load ManCo (INVESTMENT_MANAGER role)
        let manco = self
            .repo
            .get_entity_by_role(cbu_id, "INVESTMENT_MANAGER")
            .await?;

        // Load fund entity (PRINCIPAL role)
        let principal = self.repo.get_entity_by_role(cbu_id, "PRINCIPAL").await?;

        // Load share classes
        let share_classes = self.repo.get_share_classes(cbu_id).await?;
        let share_class_nodes: Vec<TreeNode> = share_classes
            .iter()
            .filter(|sc| sc.class_category.as_deref() == Some("FUND"))
            .map(|sc| self.share_class_to_node(sc))
            .collect();

        // Load officers
        let officers = self.repo.get_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> =
            officers.iter().map(|o| self.officer_to_node(o)).collect();

        // Load holdings for overlay edges
        let holdings = self.repo.get_holdings(cbu_id).await?;
        self.add_holding_edges(&holdings, &mut overlay_edges);

        // Build fund node with share classes as children
        let fund_node = principal.map(|p| {
            let mut node = self.entity_to_node(&p, TreeNodeType::FundEntity, "Fund");
            node.children = share_class_nodes.clone();
            node
        });

        // Build ManCo node with fund + officers as children
        let manco_node = manco.map(|m| {
            let mut children = Vec::new();
            if let Some(fund) = fund_node.clone() {
                children.push(fund);
            }
            children.extend(officer_nodes.clone());

            let mut node = self.entity_to_node(&m, TreeNodeType::ManCo, "Management Company");
            node.children = children;
            node
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
        cbu: &CbuView,
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load trustee
        let trustee = self.repo.get_entity_by_role(cbu_id, "TRUSTEE").await?;

        // Load trust entity (PRINCIPAL)
        let principal = self.repo.get_entity_by_role(cbu_id, "PRINCIPAL").await?;

        // Load settlor
        let settlors = self.repo.get_entities_by_role(cbu_id, "SETTLOR").await?;

        // Load beneficiaries
        let beneficiaries = self
            .repo
            .get_entities_by_role(cbu_id, "BENEFICIARY")
            .await?;

        let beneficiary_nodes: Vec<TreeNode> = beneficiaries
            .iter()
            .map(|b| self.entity_to_node(b, TreeNodeType::Person, "Beneficiary"))
            .collect();

        let settlor_nodes: Vec<TreeNode> = settlors
            .iter()
            .map(|s| self.entity_to_node(s, TreeNodeType::Person, "Settlor"))
            .collect();

        // Build trust node with settlors and beneficiaries as children
        let mut trust_children = settlor_nodes;
        trust_children.extend(beneficiary_nodes);

        let trust_node = principal.map(|t| {
            let mut node = self.entity_to_node(&t, TreeNodeType::TrustEntity, "Trust");
            node.children = trust_children.clone();
            node
        });

        // Load control relationships for overlay
        let controls = self.repo.get_control_relationships(cbu_id).await?;
        self.add_control_edges(&controls, &mut overlay_edges);

        // Build tree: Trustee → Trust
        let root = if let Some(t) = trustee {
            let mut children = Vec::new();
            if let Some(trust) = trust_node {
                children.push(trust);
            }
            let mut node = self.entity_to_node(&t, TreeNodeType::ManCo, "Trustee");
            node.children = children;
            node
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
    ) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();

        // Load commercial client (parent company)
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.repo.get_entity(cc_id).await?;
            Some(self.entity_to_node(&entity, TreeNodeType::CommercialClient, "Parent Company"))
        } else {
            None
        };

        // Load principal (subsidiary)
        let principal = self.repo.get_entity_by_role(cbu_id, "PRINCIPAL").await?;

        // Load officers
        let officers = self.repo.get_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> =
            officers.iter().map(|o| self.officer_to_node(o)).collect();

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
        let principal_node = principal.map(|p| {
            let mut children = share_class_nodes.clone();
            children.extend(officer_nodes.clone());
            let mut node = self.entity_to_node(&p, TreeNodeType::FundEntity, "Subsidiary");
            node.children = children;
            node
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

    fn add_control_edges(&self, controls: &[ControlRelationshipView], edges: &mut Vec<TreeEdge>) {
        for ctrl in controls {
            edges.push(TreeEdge {
                from: ctrl.controller_entity_id,
                to: ctrl.controlled_entity_id,
                edge_type: TreeEdgeType::Controls,
                label: Some(ctrl.control_type.clone()),
                weight: None,
            });
        }
    }
}
