//! Enhanced resource allocation strategies
//!
//! Provides multiple strategies for allocating production resources to services
//! based on different criteria (cost, performance, load balancing, etc.)

use crate::models::taxonomy::ProductionResource;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationStrategy {
    RoundRobin,
    LeastCost,
    HighestPerformance,
    PriorityBased,
    LoadBalanced,
}

#[derive(Debug, Clone)]
struct ResourceStats {
    current_load: f64,
    max_capacity: f64,
    average_response_time: f64,
    success_rate: f64,
}

impl Default for ResourceStats {
    fn default() -> Self {
        Self {
            current_load: 0.0,
            max_capacity: 100.0,
            average_response_time: 100.0,
            success_rate: 1.0,
        }
    }
}

pub struct ResourceAllocator {
    strategy: AllocationStrategy,
    resource_stats: HashMap<Uuid, ResourceStats>,
}

impl ResourceAllocator {
    pub fn new(strategy: AllocationStrategy) -> Self {
        Self {
            strategy,
            resource_stats: HashMap::new(),
        }
    }

    /// Allocate resources from a list of capable resources based on strategy
    pub async fn allocate_resources(
        &self,
        capable_resources: Vec<ProductionResource>,
        service_options: &Value,
        required_count: usize,
    ) -> Result<Vec<ProductionResource>> {
        if capable_resources.is_empty() {
            return Err(anyhow::anyhow!("No capable resources available"));
        }

        // Filter resources that match the options
        let filtered = self.filter_capable_resources(&capable_resources, service_options);

        if filtered.is_empty() {
            return Err(anyhow::anyhow!("No resources match the required options"));
        }

        // Apply allocation strategy
        let allocated = match self.strategy {
            AllocationStrategy::RoundRobin => self.allocate_round_robin(filtered, required_count),
            AllocationStrategy::LeastCost => self.allocate_least_cost(filtered, required_count),
            AllocationStrategy::HighestPerformance => {
                self.allocate_highest_performance(filtered, required_count)
            }
            AllocationStrategy::PriorityBased => {
                self.allocate_priority_based(filtered, required_count)
            }
            AllocationStrategy::LoadBalanced => {
                self.allocate_load_balanced(filtered, required_count)
            }
        };

        Ok(allocated)
    }

    fn filter_capable_resources(
        &self,
        resources: &[ProductionResource],
        options: &Value,
    ) -> Vec<ProductionResource> {
        resources
            .iter()
            .filter(|r| {
                if let Some(capabilities) = &r.capabilities {
                    self.options_match(capabilities, options)
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    fn options_match(&self, capabilities: &Value, required_options: &Value) -> bool {
        // If required options is null or empty, any capability matches
        if required_options.is_null() {
            return true;
        }

        // For JSON objects, check if all required keys/values are present in capabilities
        if let (Some(cap_obj), Some(req_obj)) =
            (capabilities.as_object(), required_options.as_object())
        {
            for (key, req_val) in req_obj {
                match cap_obj.get(key) {
                    None => return false,
                    Some(cap_val) => {
                        // For arrays (like markets), check if required values are subset
                        if let (Some(cap_arr), Some(req_arr)) =
                            (cap_val.as_array(), req_val.as_array())
                        {
                            for req_item in req_arr {
                                if !cap_arr.contains(req_item) {
                                    return false;
                                }
                            }
                        } else if cap_val != req_val {
                            return false;
                        }
                    }
                }
            }
            true
        } else {
            false
        }
    }

    fn allocate_priority_based(
        &self,
        resources: Vec<ProductionResource>,
        count: usize,
    ) -> Vec<ProductionResource> {
        // Priority is stored in ServiceResourceCapability, not ProductionResource
        // For now, use simple selection. TODO: Join with service_resource_capabilities
        resources.into_iter().take(count).collect()
    }

    fn allocate_least_cost(
        &self,
        resources: Vec<ProductionResource>,
        count: usize,
    ) -> Vec<ProductionResource> {
        // Cost factor is stored in ServiceResourceCapability, not ProductionResource
        // For now, use simple selection. TODO: Join with service_resource_capabilities
        resources.into_iter().take(count).collect()
    }

    fn allocate_highest_performance(
        &self,
        mut resources: Vec<ProductionResource>,
        count: usize,
    ) -> Vec<ProductionResource> {
        // Sort by average response time from stats (lower is better)
        resources.sort_by(|a, b| {
            let a_perf = self
                .resource_stats
                .get(&a.resource_id)
                .map(|s| s.average_response_time)
                .unwrap_or(1000.0);
            let b_perf = self
                .resource_stats
                .get(&b.resource_id)
                .map(|s| s.average_response_time)
                .unwrap_or(1000.0);
            a_perf
                .partial_cmp(&b_perf)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        resources.into_iter().take(count).collect()
    }

    fn allocate_round_robin(
        &self,
        resources: Vec<ProductionResource>,
        count: usize,
    ) -> Vec<ProductionResource> {
        // Simple round-robin: just take first N resources
        resources.into_iter().take(count).collect()
    }

    fn allocate_load_balanced(
        &self,
        mut resources: Vec<ProductionResource>,
        count: usize,
    ) -> Vec<ProductionResource> {
        // Sort by current load / max capacity ratio (lower is better)
        resources.sort_by(|a, b| {
            let a_ratio = self
                .resource_stats
                .get(&a.resource_id)
                .map(|s| s.current_load / s.max_capacity)
                .unwrap_or(0.0);
            let b_ratio = self
                .resource_stats
                .get(&b.resource_id)
                .map(|s| s.current_load / s.max_capacity)
                .unwrap_or(0.0);
            a_ratio
                .partial_cmp(&b_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        resources.into_iter().take(count).collect()
    }

    /// Update stats for a resource (would be called after operations)
    pub fn update_stats(&mut self, resource_id: Uuid, stats: ResourceStats) {
        self.resource_stats.insert(resource_id, stats);
    }

    /// Get current allocation strategy
    pub fn strategy(&self) -> AllocationStrategy {
        self.strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    fn create_test_resource(id: Uuid) -> ProductionResource {
        ProductionResource {
            resource_id: id,
            name: format!("Resource {}", id),
            description: None,
            owner: "test".to_string(),
            dictionary_group: None,
            resource_code: None,
            resource_type: Some("test".to_string()),
            vendor: None,
            version: None,
            api_endpoint: None,
            api_version: None,
            authentication_method: None,
            authentication_config: None,
            capabilities: Some(serde_json::json!({"markets": ["US"]})),
            capacity_limits: None,
            maintenance_windows: None,
            is_active: Some(true),
            created_at: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn test_priority_based_allocation() {
        let allocator = ResourceAllocator::new(AllocationStrategy::PriorityBased);

        let resources = vec![
            create_test_resource(Uuid::new_v4()),
            create_test_resource(Uuid::new_v4()),
            create_test_resource(Uuid::new_v4()),
        ];

        let result = allocator
            .allocate_resources(resources, &serde_json::json!({}), 2)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        // TODO: Priority-based allocation requires ServiceResourceCapability data
    }

    #[tokio::test]
    async fn test_least_cost_allocation() {
        let allocator = ResourceAllocator::new(AllocationStrategy::LeastCost);

        let resources = vec![
            create_test_resource(Uuid::new_v4()),
            create_test_resource(Uuid::new_v4()),
            create_test_resource(Uuid::new_v4()),
        ];

        let result = allocator
            .allocate_resources(resources, &serde_json::json!({}), 2)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        // TODO: Cost-based allocation requires ServiceResourceCapability data
    }

    #[test]
    fn test_options_matching() {
        let allocator = ResourceAllocator::new(AllocationStrategy::RoundRobin);

        let capabilities = serde_json::json!({
            "markets": ["US", "EU", "APAC"],
            "asset_classes": ["equity", "fixed_income"]
        });

        let required = serde_json::json!({
            "markets": ["US", "EU"]
        });

        assert!(allocator.options_match(&capabilities, &required));

        let incompatible = serde_json::json!({
            "markets": ["LATAM"]
        });

        assert!(!allocator.options_match(&capabilities, &incompatible));
    }
}
