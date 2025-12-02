# Service Delivery Builder - Implementation

**Task**: Implement service_builder.rs for Service Delivery view  
**Created**: 2025-12-01  
**Status**: URGENT - DO NOW  
**Priority**: BLOCKING

---

## Overview

Create `rust/src/visualization/service_builder.rs` following the same pattern as `kyc_builder.rs`.

---

## Tree Structure

```
CBU (Apex Capital Partners)
├── Product (Global Custody)
│   ├── Service (Safekeeping)
│   │   └── Resource (Account-001)
│   ├── Service (Settlement)
│   │   └── Resource (SSI-US-EQ)
│   │       ├── Booking Rule (US Equities)
│   │       └── Booking Rule (US Fixed Income)
│   └── Service (Reporting)
│       └── Resource (Report-Config)
├── Product (Fund Administration)
│   └── Service (NAV Calculation)
│       └── Resource (NAV-System)
└── Product (Prime Brokerage)
    └── Service (Securities Lending)
```

---

## Implementation

Create `rust/src/visualization/service_builder.rs`:

```rust
use super::types::*;
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;
use std::collections::HashMap;

pub struct ServiceTreeBuilder {
    pool: PgPool,
}

impl ServiceTreeBuilder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    pub async fn build(&self, cbu_id: Uuid) -> Result<CbuVisualization> {
        // 1. Load CBU
        let cbu = self.load_cbu(cbu_id).await?;
        
        // 2. Load products for this CBU
        let products = self.load_cbu_products(cbu_id).await?;
        
        // 3. For each product, load services
        // 4. For each service, load resources
        // 5. For resources that are SSIs, load booking rules
        
        let mut product_nodes = Vec::new();
        
        for product in products {
            let services = self.load_product_services(cbu_id, product.product_id).await?;
            
            let mut service_nodes = Vec::new();
            for service in services {
                let resources = self.load_service_resources(cbu_id, service.service_id).await?;
                
                let mut resource_nodes = Vec::new();
                for resource in resources {
                    // Check if this is an SSI - if so, load booking rules
                    let booking_rules = if resource.resource_type == "SSI" {
                        self.load_booking_rules_for_ssi(resource.resource_id).await?
                    } else {
                        vec![]
                    };
                    
                    let rule_nodes: Vec<TreeNode> = booking_rules.into_iter().map(|rule| {
                        TreeNode {
                            id: rule.id,
                            node_type: TreeNodeType::BookingRule,
                            label: rule.name,
                            sublabel: Some(rule.description.unwrap_or_default()),
                            jurisdiction: None,
                            children: vec![],
                            metadata: HashMap::new(),
                        }
                    }).collect();
                    
                    resource_nodes.push(TreeNode {
                        id: resource.resource_id,
                        node_type: if resource.resource_type == "SSI" {
                            TreeNodeType::Ssi
                        } else {
                            TreeNodeType::Resource
                        },
                        label: resource.name,
                        sublabel: Some(resource.resource_type.clone()),
                        jurisdiction: None,
                        children: rule_nodes,
                        metadata: HashMap::new(),
                    });
                }
                
                service_nodes.push(TreeNode {
                    id: service.service_id,
                    node_type: TreeNodeType::Service,
                    label: service.name,
                    sublabel: service.description.clone(),
                    jurisdiction: None,
                    children: resource_nodes,
                    metadata: HashMap::new(),
                });
            }
            
            product_nodes.push(TreeNode {
                id: product.product_id,
                node_type: TreeNodeType::Product,
                label: product.name,
                sublabel: product.description.clone(),
                jurisdiction: None,
                children: service_nodes,
                metadata: HashMap::new(),
            });
        }
        
        let root = TreeNode {
            id: cbu_id,
            node_type: TreeNodeType::Cbu,
            label: cbu.name.clone(),
            sublabel: cbu.client_type.clone(),
            jurisdiction: Some(cbu.jurisdiction.clone()),
            children: product_nodes,
            metadata: HashMap::new(),
        };
        
        let stats = self.calculate_stats(&root);
        
        Ok(CbuVisualization {
            cbu_id,
            cbu_name: cbu.name,
            view_mode: ViewMode::ServiceDelivery,
            root,
            overlay_edges: vec![], // No overlay edges in service view
            stats,
        })
    }
    
    async fn load_cbu(&self, cbu_id: Uuid) -> Result<CbuRecord> {
        let row = sqlx::query_as!(
            CbuRecord,
            r#"
            SELECT 
                cbu_id,
                name,
                jurisdiction,
                client_type,
                commercial_client_entity_id
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(row)
    }
    
    async fn load_cbu_products(&self, cbu_id: Uuid) -> Result<Vec<ProductRecord>> {
        // Try loading from cbu_products join table first
        // If that doesn't exist, try product_subscriptions or similar
        
        let rows = sqlx::query_as!(
            ProductRecord,
            r#"
            SELECT 
                p.product_id,
                p.name,
                p.description
            FROM "ob-poc".products p
            INNER JOIN "ob-poc".cbu_products cp ON cp.product_id = p.product_id
            WHERE cp.cbu_id = $1
            ORDER BY p.name
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await;
        
        match rows {
            Ok(r) => Ok(r),
            Err(_) => {
                // Fallback: maybe products are linked differently
                // Or return empty if no products configured
                Ok(vec![])
            }
        }
    }
    
    async fn load_product_services(&self, cbu_id: Uuid, product_id: Uuid) -> Result<Vec<ServiceRecord>> {
        let rows = sqlx::query_as!(
            ServiceRecord,
            r#"
            SELECT 
                s.service_id,
                s.name,
                s.description
            FROM "ob-poc".services s
            INNER JOIN "ob-poc".product_services ps ON ps.service_id = s.service_id
            WHERE ps.product_id = $1
            ORDER BY s.name
            "#,
            product_id
        )
        .fetch_all(&self.pool)
        .await;
        
        match rows {
            Ok(r) => Ok(r),
            Err(_) => Ok(vec![])
        }
    }
    
    async fn load_service_resources(&self, cbu_id: Uuid, service_id: Uuid) -> Result<Vec<ResourceRecord>> {
        let rows = sqlx::query_as!(
            ResourceRecord,
            r#"
            SELECT 
                sr.service_resource_id as resource_id,
                sr.name,
                sr.resource_type
            FROM "ob-poc".service_resources sr
            WHERE sr.service_id = $1
              AND sr.cbu_id = $2
            ORDER BY sr.name
            "#,
            service_id,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await;
        
        match rows {
            Ok(r) => Ok(r),
            Err(_) => Ok(vec![])
        }
    }
    
    async fn load_booking_rules_for_ssi(&self, ssi_id: Uuid) -> Result<Vec<BookingRuleRecord>> {
        let rows = sqlx::query_as!(
            BookingRuleRecord,
            r#"
            SELECT 
                br.booking_rule_id as id,
                br.name,
                br.description
            FROM custody.booking_rules br
            WHERE br.ssi_id = $1
            ORDER BY br.name
            "#,
            ssi_id
        )
        .fetch_all(&self.pool)
        .await;
        
        match rows {
            Ok(r) => Ok(r),
            Err(_) => Ok(vec![])
        }
    }
    
    fn calculate_stats(&self, root: &TreeNode) -> VisualizationStats {
        fn count_nodes(node: &TreeNode, stats: &mut VisualizationStats, depth: usize) {
            stats.total_nodes += 1;
            stats.max_depth = stats.max_depth.max(depth);
            
            for child in &node.children {
                count_nodes(child, stats, depth + 1);
            }
        }
        
        let mut stats = VisualizationStats {
            total_nodes: 0,
            total_edges: 0,
            max_depth: 0,
            entity_count: 0,
            person_count: 0,
            share_class_count: 0,
        };
        
        count_nodes(root, &mut stats, 0);
        stats
    }
}

// Record types for queries
#[derive(Debug)]
struct CbuRecord {
    cbu_id: Uuid,
    name: String,
    jurisdiction: String,
    client_type: Option<String>,
    commercial_client_entity_id: Option<Uuid>,
}

#[derive(Debug)]
struct ProductRecord {
    product_id: Uuid,
    name: String,
    description: Option<String>,
}

#[derive(Debug)]
struct ServiceRecord {
    service_id: Uuid,
    name: String,
    description: Option<String>,
}

#[derive(Debug)]
struct ResourceRecord {
    resource_id: Uuid,
    name: String,
    resource_type: String,
}

#[derive(Debug)]
struct BookingRuleRecord {
    id: Uuid,
    name: String,
    description: Option<String>,
}
```

---

## Update mod.rs

Add to `rust/src/visualization/mod.rs`:

```rust
pub mod service_builder;
pub use service_builder::ServiceTreeBuilder;
```

---

## Update API Endpoint

In the API handler, replace the placeholder with actual builder:

```rust
// In the tree endpoint handler
ViewMode::ServiceDelivery => {
    let builder = ServiceTreeBuilder::new(pool.clone());
    builder.build(cbu_id).await?
}
```

---

## Tables Used

| Table | Purpose |
|-------|---------|
| `cbus` | CBU info |
| `products` | Product catalog |
| `cbu_products` | Which products a CBU subscribes to |
| `services` | Service catalog |
| `product_services` | Which services belong to a product |
| `service_resources` | Resource instances provisioned for CBU |
| `custody.booking_rules` | Booking rules linked to SSIs |

---

## If Tables Don't Exist

The builder has fallbacks that return empty vectors. So if:
- No `cbu_products` records → shows CBU with no products
- No `service_resources` → shows services with no resources

This is safe - it won't crash, just shows an empty tree.

---

## Seed Data Needed?

If the Service Delivery view is empty, we need to seed:

```sql
-- Link Apex Capital to Global Custody product
INSERT INTO "ob-poc".cbu_products (cbu_id, product_id)
SELECT 
    'a1000000-0000-0000-0000-000000001000'::uuid,
    product_id
FROM "ob-poc".products 
WHERE name = 'Global Custody';

-- Create service resources for the CBU
INSERT INTO "ob-poc".service_resources (service_resource_id, cbu_id, service_id, name, resource_type)
VALUES (
    gen_random_uuid(),
    'a1000000-0000-0000-0000-000000001000'::uuid,
    (SELECT service_id FROM "ob-poc".services WHERE name = 'Safekeeping'),
    'Apex-Account-001',
    'ACCOUNT'
);
```

---

## Summary

1. Create `service_builder.rs` with the code above
2. Add to `mod.rs`
3. Wire up in API endpoint
4. If empty, seed product/service data for CBUs

---

*Ship it.*
