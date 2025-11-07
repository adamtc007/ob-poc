//! DSL Manager Library
//!
//! This module provides a simple interface for creating and editing domain DSL definitions.
//! It focuses on just the core functionality needed without overwhelming complexity.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Domain DSL definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDsl {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub dsl_content: String,
    pub version: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

/// Result type for DSL operations
pub type DslResult<T> = Result<T, DslError>;

/// DSL manager errors
#[derive(Debug, thiserror::Error)]
pub enum DslError {
    #[error("DSL not found: {id}")]
    NotFound { id: String },

    #[error("Invalid DSL content: {reason}")]
    InvalidContent { reason: String },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },
}

/// DSL Manager - handles creation and editing of domain DSLs
pub struct DslManager {
    storage: HashMap<String, DomainDsl>,
}

impl DslManager {
    /// Create a new DSL manager
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    /// Create a new domain DSL
    ///
    /// # Arguments
    /// * `name` - The name of the DSL
    /// * `domain` - The domain this DSL belongs to (e.g., "finance.kyc")
    /// * `dsl_content` - The DSL content as a string
    ///
    /// # Returns
    /// * `Ok(String)` - The generated DSL ID
    /// * `Err(DslError)` - If creation fails
    pub fn create_domain_dsl(
        &mut self,
        name: String,
        domain: String,
        dsl_content: String,
    ) -> DslResult<String> {
        // Validate the DSL content
        self.validate_dsl_content(&dsl_content)?;

        // Generate a new ID
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        // Create the DSL definition
        let domain_dsl = DomainDsl {
            id: id.clone(),
            name: name.clone(),
            domain: domain.clone(),
            dsl_content,
            version: "1.0.0".to_string(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        };

        // Store it
        self.storage.insert(id.clone(), domain_dsl);

        println!(
            "✅ Created domain DSL '{}' in domain '{}' with ID: {}",
            name, domain, id
        );

        Ok(id)
    }

    /// Edit an existing domain DSL by ID
    ///
    /// # Arguments
    /// * `id` - The DSL ID to edit
    /// * `name` - Optional new name
    /// * `domain` - Optional new domain
    /// * `dsl_content` - Optional new DSL content
    ///
    /// # Returns
    /// * `Ok(DomainDsl)` - The updated DSL
    /// * `Err(DslError)` - If editing fails
    pub fn edit_domain_dsl(
        &mut self,
        id: String,
        name: Option<String>,
        domain: Option<String>,
        dsl_content: Option<String>,
    ) -> DslResult<DomainDsl> {
        // Find the existing DSL
        let domain_dsl = self
            .storage
            .get_mut(&id)
            .ok_or_else(|| DslError::NotFound { id: id.clone() })?;

        let mut updated = false;

        // Update name if provided
        if let Some(new_name) = name {
            domain_dsl.name = new_name;
            updated = true;
        }

        // Update domain if provided
        if let Some(new_domain) = domain {
            domain_dsl.domain = new_domain;
            updated = true;
        }

        // Update DSL content if provided
        if let Some(new_content) = dsl_content {
            self.validate_dsl_content(&new_content)?;
            domain_dsl.dsl_content = new_content;
            updated = true;
        }

        if updated {
            // Update timestamp and increment version
            domain_dsl.updated_at = chrono::Utc::now();
            domain_dsl.version = self.increment_version(&domain_dsl.version);

            println!(
                "🔄 Updated domain DSL '{}' (ID: {}) to version {}",
                domain_dsl.name, id, domain_dsl.version
            );
        }

        Ok(domain_dsl.clone())
    }

    /// Get a domain DSL by ID (helper function)
    pub fn get_domain_dsl(&self, id: &str) -> DslResult<&DomainDsl> {
        self.storage
            .get(id)
            .ok_or_else(|| DslError::NotFound { id: id.to_string() })
    }

    /// List all domain DSLs (helper function)
    pub fn list_domain_dsls(&self) -> Vec<&DomainDsl> {
        self.storage.values().collect()
    }

    /// Validate DSL content
    ///
    /// This is a basic validation - in a real implementation, you'd want to
    /// integrate with the actual DSL parser for proper syntax validation.
    fn validate_dsl_content(&self, content: &str) -> DslResult<()> {
        if content.trim().is_empty() {
            return Err(DslError::InvalidContent {
                reason: "DSL content cannot be empty".to_string(),
            });
        }

        // Basic syntax check - should start and end with parentheses for S-expression
        let trimmed = content.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return Err(DslError::InvalidContent {
                reason:
                    "DSL content should be a valid S-expression (start with '(' and end with ')')"
                        .to_string(),
            });
        }

        // Could add more validation here:
        // - Parse with nom to check syntax
        // - Validate against grammar rules
        // - Check for required elements

        Ok(())
    }

    /// Increment version string (simple semantic versioning)
    fn increment_version(&self, current_version: &str) -> String {
        let parts: Vec<&str> = current_version.split('.').collect();
        match parts.as_slice() {
            [major, minor, patch] => {
                if let Ok(patch_num) = patch.parse::<u32>() {
                    format!("{}.{}.{}", major, minor, patch_num + 1)
                } else {
                    "1.0.1".to_string()
                }
            }
            _ => "1.0.1".to_string(),
        }
    }
}

impl Default for DslManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_domain_dsl() {
        let mut manager = DslManager::new();

        let result = manager.create_domain_dsl(
            "onboarding-workflow".to_string(),
            "finance.kyc".to_string(),
            "(workflow \"onboard\" (declare-entity \"customer\" \"person\"))".to_string(),
        );

        assert!(result.is_ok());
        let id = result.unwrap();
        assert!(!id.is_empty());

        // Verify it was stored
        let dsl = manager.get_domain_dsl(&id).unwrap();
        assert_eq!(dsl.name, "onboarding-workflow");
        assert_eq!(dsl.domain, "finance.kyc");
        assert_eq!(dsl.version, "1.0.0");
    }

    #[test]
    fn test_edit_domain_dsl() {
        let mut manager = DslManager::new();

        // Create first
        let id = manager
            .create_domain_dsl(
                "test-workflow".to_string(),
                "test.domain".to_string(),
                "(workflow \"test\")".to_string(),
            )
            .unwrap();

        // Edit it
        let result = manager.edit_domain_dsl(
            id.clone(),
            Some("updated-workflow".to_string()),
            None,
            Some("(workflow \"updated\" (declare-entity \"entity1\" \"person\"))".to_string()),
        );

        assert!(result.is_ok());
        let updated_dsl = result.unwrap();
        assert_eq!(updated_dsl.name, "updated-workflow");
        assert_eq!(updated_dsl.version, "1.0.1");
        assert!(updated_dsl.dsl_content.contains("updated"));
    }

    #[test]
    fn test_invalid_dsl_content() {
        let mut manager = DslManager::new();

        // Empty content should fail
        let result =
            manager.create_domain_dsl("test".to_string(), "test".to_string(), "".to_string());
        assert!(result.is_err());

        // Invalid syntax should fail
        let result = manager.create_domain_dsl(
            "test".to_string(),
            "test".to_string(),
            "not a valid s-expression".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_edit_nonexistent_dsl() {
        let mut manager = DslManager::new();

        let result = manager.edit_domain_dsl(
            "nonexistent-id".to_string(),
            Some("new-name".to_string()),
            None,
            None,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            DslError::NotFound { id } => assert_eq!(id, "nonexistent-id"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_version_increment() {
        let manager = DslManager::new();

        assert_eq!(manager.increment_version("1.0.0"), "1.0.1");
        assert_eq!(manager.increment_version("2.5.10"), "2.5.11");
        assert_eq!(manager.increment_version("invalid"), "1.0.1");
    }
}
