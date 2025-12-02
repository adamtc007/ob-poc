//! Service Delivery Tree Builder
//!
//! Builds hierarchical tree visualization for Service Delivery view.
//! Structure: CBU → Product → Service → Resource (Instance)

use super::types::*;
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Services grouped by ID with name and resource nodes
type ServiceMap = HashMap<Uuid, (String, Vec<TreeNode>)>;

/// Products grouped by ID with name and their services
type ProductMap = HashMap<Uuid, (String, ServiceMap)>;

pub struct ServiceTreeBuilder {
    pool: PgPool,
}

#[derive(Debug, sqlx::FromRow)]
struct CbuRecord {
    name: String,
    jurisdiction: Option<String>,
    client_type: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct DeliveryRecord {
    #[allow(dead_code)]
    delivery_id: Uuid,
    product_id: Uuid,
    product_name: String,
    service_id: Uuid,
    service_name: String,
    instance_id: Option<Uuid>,
    instance_name: Option<String>,
    resource_type_name: Option<String>,
    delivery_status: Option<String>,
}

impl ServiceTreeBuilder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn build(&self, cbu_id: Uuid) -> Result<CbuVisualization> {
        let cbu = self.load_cbu(cbu_id).await?;
        let deliveries = self.load_deliveries(cbu_id).await?;

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
                    label: d.instance_name.unwrap_or_else(|| "Instance".to_string()),
                    sublabel: d.resource_type_name,
                    jurisdiction: None,
                    children: vec![],
                    metadata: {
                        let mut m = HashMap::new();
                        if let Some(status) = d.delivery_status {
                            m.insert("status".to_string(), serde_json::json!(status));
                        }
                        m
                    },
                };
                service_entry.1.push(resource_node);
            }
        }

        // Build tree
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

        let root = TreeNode {
            id: cbu_id,
            node_type: TreeNodeType::Cbu,
            label: cbu.name.clone(),
            sublabel: cbu.client_type.clone(),
            jurisdiction: cbu.jurisdiction.clone(),
            children: product_nodes,
            metadata: HashMap::new(),
        };

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

    async fn load_cbu(&self, cbu_id: Uuid) -> Result<CbuRecord> {
        let row = sqlx::query_as!(
            CbuRecord,
            r#"SELECT name, jurisdiction, client_type
               FROM "ob-poc".cbus
               WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    async fn load_deliveries(&self, cbu_id: Uuid) -> Result<Vec<DeliveryRecord>> {
        let rows = sqlx::query_as!(
            DeliveryRecord,
            r#"SELECT
                sdm.delivery_id,
                sdm.product_id,
                p.name as "product_name!",
                sdm.service_id,
                s.name as "service_name!",
                sdm.instance_id,
                cri.instance_name as "instance_name?",
                srt.name as "resource_type_name?",
                sdm.delivery_status as "delivery_status?"
               FROM "ob-poc".service_delivery_map sdm
               JOIN "ob-poc".products p ON p.product_id = sdm.product_id
               JOIN "ob-poc".services s ON s.service_id = sdm.service_id
               LEFT JOIN "ob-poc".cbu_resource_instances cri ON cri.instance_id = sdm.instance_id
               LEFT JOIN "ob-poc".service_resource_types srt ON srt.resource_id = cri.resource_type_id
               WHERE sdm.cbu_id = $1
               ORDER BY p.name, s.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
