//! Semantic stage map loader.
//!
//! Loads the semantic stage map from `config/ontology/semantic_stage_map.yaml`.
//! This provides the agent with a view of "where are we in the onboarding journey."
//!
//! Key distinction:
//! - SemanticStageMap: Configuration (this loader)
//! - SemanticState: Derived at runtime (see database/semantic_state_service.rs)

use ob_poc_types::semantic_stage::{SemanticStageMap, StageDefinition};
use std::collections::HashSet;
use std::path::Path;

/// Semantic stage map loaded from configuration.
#[derive(Debug, Clone)]
pub struct SemanticStageRegistry {
    /// The loaded stage map
    map: SemanticStageMap,
    /// Stage codes in topological order (dependencies first)
    topo_order: Vec<String>,
}

impl SemanticStageRegistry {
    /// Load stage map from a YAML file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref())?;
        Self::from_yaml(&content)
    }

    /// Load from default path (config/ontology/semantic_stage_map.yaml)
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        // Try multiple paths like ConfigLoader does
        let paths = [
            "config/ontology/semantic_stage_map.yaml",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/config/ontology/semantic_stage_map.yaml"
            ),
        ];

        for path in paths {
            if Path::new(path).exists() {
                return Self::load(path);
            }
        }

        Err("Could not find semantic_stage_map.yaml in any expected location".into())
    }

    /// Parse stage map from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let map: SemanticStageMap = serde_yaml::from_str(yaml)?;

        // Validate the map
        Self::validate(&map)?;

        // Compute topological order
        let topo_order = Self::topo_sort(&map.stages)?;

        Ok(Self { map, topo_order })
    }

    /// Get the underlying map
    pub fn map(&self) -> &SemanticStageMap {
        &self.map
    }

    /// Get stages in topological order (dependencies before dependents)
    pub fn stages_in_order(&self) -> impl Iterator<Item = &StageDefinition> {
        self.topo_order
            .iter()
            .filter_map(|code| self.map.stages.iter().find(|s| &s.code == code))
    }

    /// Get a stage by code
    pub fn get_stage(&self, code: &str) -> Option<&StageDefinition> {
        self.map.stages.iter().find(|s| s.code == code)
    }

    /// Get the stage for an entity type
    pub fn stage_for_entity(&self, entity_type: &str) -> Option<&StageDefinition> {
        self.map
            .entity_stage_mapping
            .get(entity_type)
            .and_then(|code| self.get_stage(code))
    }

    /// Get required stages for a product
    pub fn stages_for_product(&self, product_code: &str) -> Vec<&str> {
        self.map
            .product_stages
            .get(product_code)
            .map(|config| config.mandatory.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get required stages for a set of products (union)
    pub fn stages_for_products(&self, products: &[String]) -> Vec<&str> {
        let mut stages: HashSet<&str> = HashSet::new();

        for product in products {
            if let Some(config) = self.map.product_stages.get(product) {
                for stage in &config.mandatory {
                    stages.insert(stage.as_str());
                }
            }
        }

        // Return in topological order
        self.topo_order
            .iter()
            .filter(|code| stages.contains(code.as_str()))
            .map(|s| s.as_str())
            .collect()
    }

    /// Check if a stage is blocking
    pub fn is_blocking(&self, stage_code: &str) -> bool {
        self.get_stage(stage_code)
            .map(|s| s.blocking)
            .unwrap_or(false)
    }

    /// Get stages that depend on a given stage
    pub fn dependents_of(&self, stage_code: &str) -> Vec<&str> {
        self.map
            .stages
            .iter()
            .filter(|s| s.depends_on.iter().any(|d| d == stage_code))
            .map(|s| s.code.as_str())
            .collect()
    }

    /// Validate the stage map configuration
    fn validate(map: &SemanticStageMap) -> Result<(), Box<dyn std::error::Error>> {
        let stage_codes: HashSet<_> = map.stages.iter().map(|s| s.code.as_str()).collect();

        // Check depends_on references exist
        for stage in &map.stages {
            for dep in &stage.depends_on {
                if !stage_codes.contains(dep.as_str()) {
                    return Err(format!(
                        "Stage '{}' depends on unknown stage '{}'",
                        stage.code, dep
                    )
                    .into());
                }
            }
        }

        // Check product_stages reference valid stages
        for (product, config) in &map.product_stages {
            for stage in &config.mandatory {
                if !stage_codes.contains(stage.as_str()) {
                    return Err(format!(
                        "Product '{}' references unknown stage '{}'",
                        product, stage
                    )
                    .into());
                }
            }
            for cond in &config.conditional {
                if !stage_codes.contains(cond.stage.as_str()) {
                    return Err(format!(
                        "Product '{}' conditional references unknown stage '{}'",
                        product, cond.stage
                    )
                    .into());
                }
            }
        }

        // Check entity_stage_mapping references valid stages
        for (entity_type, stage) in &map.entity_stage_mapping {
            if !stage_codes.contains(stage.as_str()) {
                return Err(format!(
                    "Entity type '{}' maps to unknown stage '{}'",
                    entity_type, stage
                )
                .into());
            }
        }

        Ok(())
    }

    /// Topologically sort stages by dependencies
    fn topo_sort(stages: &[StageDefinition]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut result = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut in_progress: HashSet<String> = HashSet::new();

        let stage_map: std::collections::HashMap<_, _> =
            stages.iter().map(|s| (s.code.as_str(), s)).collect();

        fn visit(
            code: &str,
            stage_map: &std::collections::HashMap<&str, &StageDefinition>,
            visited: &mut HashSet<String>,
            in_progress: &mut HashSet<String>,
            result: &mut Vec<String>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if visited.contains(code) {
                return Ok(());
            }
            if in_progress.contains(code) {
                return Err(format!("Cycle detected involving stage '{}'", code).into());
            }

            in_progress.insert(code.to_string());

            if let Some(stage) = stage_map.get(code) {
                for dep in &stage.depends_on {
                    visit(dep, stage_map, visited, in_progress, result)?;
                }
            }

            in_progress.remove(code);
            visited.insert(code.to_string());
            result.push(code.to_string());

            Ok(())
        }

        for stage in stages {
            visit(
                &stage.code,
                &stage_map,
                &mut visited,
                &mut in_progress,
                &mut result,
            )?;
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_YAML: &str = r#"
stages:
  - code: CLIENT_SETUP
    name: "Client Setup"
    description: "Establish the client entity"
    required_entities:
      - cbu
    depends_on: []

  - code: PRODUCT_SELECTION
    name: "Product Selection"
    description: "Define what services they need"
    required_entities:
      - cbu_product_subscription
    depends_on: [CLIENT_SETUP]

  - code: KYC_REVIEW
    name: "KYC Review"
    description: "Know your customer"
    required_entities:
      - kyc_case
      - entity_workstream
    depends_on: [CLIENT_SETUP]
    blocking: true

product_stages:
  CUSTODY:
    mandatory:
      - CLIENT_SETUP
      - PRODUCT_SELECTION
      - KYC_REVIEW

entity_stage_mapping:
  cbu: CLIENT_SETUP
  cbu_product_subscription: PRODUCT_SELECTION
  kyc_case: KYC_REVIEW
  entity_workstream: KYC_REVIEW
"#;

    #[test]
    fn test_load_stage_map() {
        let registry = SemanticStageRegistry::from_yaml(TEST_YAML).unwrap();
        assert_eq!(registry.map.stages.len(), 3);
    }

    #[test]
    fn test_topo_order() {
        let registry = SemanticStageRegistry::from_yaml(TEST_YAML).unwrap();
        // CLIENT_SETUP should come before stages that depend on it
        let order = &registry.topo_order;
        let client_idx = order.iter().position(|s| s == "CLIENT_SETUP").unwrap();
        let product_idx = order.iter().position(|s| s == "PRODUCT_SELECTION").unwrap();
        let kyc_idx = order.iter().position(|s| s == "KYC_REVIEW").unwrap();

        assert!(client_idx < product_idx);
        assert!(client_idx < kyc_idx);
    }

    #[test]
    fn test_stage_for_entity() {
        let registry = SemanticStageRegistry::from_yaml(TEST_YAML).unwrap();
        let stage = registry.stage_for_entity("kyc_case").unwrap();
        assert_eq!(stage.code, "KYC_REVIEW");
    }

    #[test]
    fn test_stages_for_product() {
        let registry = SemanticStageRegistry::from_yaml(TEST_YAML).unwrap();
        let stages = registry.stages_for_product("CUSTODY");
        assert_eq!(stages.len(), 3);
        assert!(stages.contains(&"CLIENT_SETUP"));
        assert!(stages.contains(&"PRODUCT_SELECTION"));
        assert!(stages.contains(&"KYC_REVIEW"));
    }

    #[test]
    fn test_is_blocking() {
        let registry = SemanticStageRegistry::from_yaml(TEST_YAML).unwrap();
        assert!(registry.is_blocking("KYC_REVIEW"));
        assert!(!registry.is_blocking("CLIENT_SETUP"));
    }

    #[test]
    fn test_cycle_detection() {
        let yaml_with_cycle = r#"
stages:
  - code: A
    name: "A"
    description: "Stage A"
    required_entities: []
    depends_on: [B]
  - code: B
    name: "B"
    description: "Stage B"
    required_entities: []
    depends_on: [A]
product_stages: {}
entity_stage_mapping: {}
"#;
        let result = SemanticStageRegistry::from_yaml(yaml_with_cycle);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cycle"));
    }

    #[test]
    fn test_invalid_dependency() {
        let yaml_invalid_dep = r#"
stages:
  - code: A
    name: "A"
    description: "Stage A"
    required_entities: []
    depends_on: [NONEXISTENT]
product_stages: {}
entity_stage_mapping: {}
"#;
        let result = SemanticStageRegistry::from_yaml(yaml_invalid_dep);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown stage"));
    }
}
