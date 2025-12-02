//! Service Delivery Tree Builder
//!
//! Builds hierarchical tree visualization for Service Delivery view.
//! Structure: CBU → Product → Service → Resource (Instance)
//!
//! NOTE: All database access goes through VisualizationRepository.
//! This builder only handles tree assembly logic.

use super::types::*;
use crate::database::{CbuSummaryView, ServiceDeliveryView, VisualizationRepository};
use anyhow::Result;
use std::collections::HashMap;
use uuid::Uuid;

/// Services grouped by ID with name and resource nodes
type ServiceMap = HashMap<Uuid, (String, Vec<TreeNode>)>;

/// Products grouped by ID with name and their services
type ProductMap = HashMap<Uuid, (String, ServiceMap)>;

pub struct ServiceTreeBuilder {
    repo: VisualizationRepository,
}

impl ServiceTreeBuilder {
    pub fn new(repo: VisualizationRepository) -> Self {
        Self { repo }
    }

    pub async fn build(&self, cbu_id: Uuid) -> Result<CbuVisualization> {
        let cbu = self
            .repo
            .get_cbu(cbu_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;
        let deliveries = self.repo.get_service_deliveries(cbu_id).await?;

        let product_nodes = self.build_product_tree(&deliveries);

        let root = self.build_root_node(cbu_id, &cbu, product_nodes);
        let stats = VisualizationStats::from_tree(&root, &[]);

        Ok(CbuVisualization {
            cbu_id,
            cbu_name: cbu.name,
            client_type: cbu.client_type,
            jurisdiction: cbu.jurisdiction,
            view_mode: ViewMode::ServiceDelivery,
            root,
            overlay_edges: vec![],
            stats,
        })
    }

    fn build_product_tree(&self, deliveries: &[ServiceDeliveryView]) -> Vec<TreeNode> {
        // Group by product -> service -> resources
        let mut products: ProductMap = HashMap::new();

        for d in deliveries {
            let product_entry = products
                .entry(d.product_id)
                .or_insert_with(|| (d.product_name.clone(), HashMap::new()));

            let service_entry = product_entry
                .1
                .entry(d.service_id)
                .or_insert_with(|| (d.service_name.clone(), Vec::new()));

            // Add resource if instance exists
            if let Some(instance_id) = d.instance_id {
                let resource_node = TreeNode {
                    id: instance_id,
                    node_type: TreeNodeType::Resource,
                    label: d
                        .instance_name
                        .clone()
                        .unwrap_or_else(|| "Instance".to_string()),
                    sublabel: d.resource_type_name.clone(),
                    jurisdiction: None,
                    children: vec![],
                    metadata: {
                        let mut m = HashMap::new();
                        if let Some(ref status) = d.delivery_status {
                            m.insert("status".to_string(), serde_json::json!(status));
                        }
                        m
                    },
                };
                service_entry.1.push(resource_node);
            }
        }

        // Build tree nodes
        let mut product_nodes: Vec<TreeNode> = products
            .into_iter()
            .map(|(product_id, (product_name, services))| {
                let service_nodes: Vec<TreeNode> = services
                    .into_iter()
                    .map(|(service_id, (service_name, resources))| TreeNode {
                        id: service_id,
                        node_type: TreeNodeType::Service,
                        label: service_name,
                        sublabel: Some(format!("{} resources", resources.len())),
                        jurisdiction: None,
                        children: resources,
                        metadata: HashMap::new(),
                    })
                    .collect();

                TreeNode {
                    id: product_id,
                    node_type: TreeNodeType::Product,
                    label: product_name,
                    sublabel: Some(format!("{} services", service_nodes.len())),
                    jurisdiction: None,
                    children: service_nodes,
                    metadata: HashMap::new(),
                }
            })
            .collect();

        product_nodes.sort_by(|a, b| a.label.cmp(&b.label));
        product_nodes
    }

    fn build_root_node(
        &self,
        cbu_id: Uuid,
        cbu: &CbuSummaryView,
        children: Vec<TreeNode>,
    ) -> TreeNode {
        TreeNode {
            id: cbu_id,
            node_type: TreeNodeType::Cbu,
            label: cbu.name.clone(),
            sublabel: cbu.client_type.clone(),
            jurisdiction: cbu.jurisdiction.clone(),
            children,
            metadata: HashMap::new(),
        }
    }
}
