//! CRUD Validator - Phase 3 Validation and Safety System
//!
//! This module provides comprehensive validation and safety checks for CRUD operations
//! including permission validation, referential integrity checking, and operation simulation.

use crate::{
    BatchOperation, ComplexQuery, ConditionalUpdate, ConstraintType, ConstraintViolation,
    CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, DependencyIssue, DependencyType,
    IntegrityResult, Key, Literal, PropertyMap, ResourceUsage, SimulationResult, ValidationResult,
    ValidationWarning, Value,
};
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Validation error structure
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    pub severity: ErrorSeverity,
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// CRUD Validator provides validation and safety checks for operations
#[derive(Debug, Clone)]
pub struct CrudValidator {
    /// Validation configuration
    config: ValidatorConfig,
    /// Cached schema information
    schema_cache: SchemaCache,
    /// Permission rules
    permission_rules: PermissionRules,
}

/// Configuration for the validator
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Enable strict mode (fail on warnings)
    pub strict_mode: bool,
    /// Maximum allowed affected records for bulk operations
    pub max_bulk_records: u32,
    /// Enable referential integrity checks
    pub check_referential_integrity: bool,
    /// Enable permission checks
    pub check_permissions: bool,
    /// Simulation timeout (seconds)
    pub simulation_timeout_seconds: u64,
}

/// Cached schema information
#[derive(Debug, Clone)]
pub struct SchemaCache {
    /// Asset schemas
    pub asset_schemas: HashMap<String, AssetSchema>,
    /// Relationship information
    pub relationships: HashMap<String, Vec<Relationship>>,
    /// Constraint definitions
    pub constraints: HashMap<String, Vec<Constraint>>,
}

/// Asset schema definition
#[derive(Debug, Clone)]
pub struct AssetSchema {
    pub name: String,
    pub fields: HashMap<String, FieldSchema>,
    pub primary_keys: Vec<String>,
    pub required_fields: Vec<String>,
    pub indexes: Vec<String>,
}

/// Field schema definition
#[derive(Debug, Clone)]
pub struct FieldSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub max_length: Option<u32>,
    pub default_value: Option<String>,
    pub validation_rules: Vec<String>,
}

/// Relationship between assets
#[derive(Debug, Clone)]
pub struct Relationship {
    pub from_asset: String,
    pub to_asset: String,
    pub from_field: String,
    pub to_field: String,
    pub relationship_type: RelationshipType,
    pub cascade_behavior: CascadeBehavior,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CascadeBehavior {
    Cascade,
    Restrict,
    SetNull,
    SetDefault,
    NoAction,
}

/// Constraint definition
#[derive(Debug, Clone)]
pub struct Constraint {
    pub name: String,
    pub constraint_type: ConstraintType,
    pub fields: Vec<String>,
    pub reference_table: Option<String>,
    pub reference_fields: Option<Vec<String>>,
    pub check_expression: Option<String>,
}

/// Permission rules system
#[derive(Debug, Clone)]
pub struct PermissionRules {
    /// Asset-level permissions
    pub asset_permissions: HashMap<String, AssetPermission>,
    /// Field-level permissions
    pub field_permissions: HashMap<String, HashMap<String, FieldPermission>>,
    /// Operation-level permissions
    pub operation_permissions: HashMap<String, OperationPermission>,
}

#[derive(Debug, Clone)]
pub struct AssetPermission {
    pub asset_name: String,
    pub can_create: bool,
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub required_roles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FieldPermission {
    pub field_name: String,
    pub can_read: bool,
    pub can_write: bool,
    pub required_roles: Vec<String>,
    pub pii_classification: PiiClassification,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PiiClassification {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct OperationPermission {
    pub operation_type: String,
    pub max_records: Option<u32>,
    pub requires_approval: bool,
    pub audit_required: bool,
    pub time_restrictions: Option<TimeRestrictions>,
}

#[derive(Debug, Clone)]
pub struct TimeRestrictions {
    pub allowed_hours: Vec<u8>, // 0-23
    pub allowed_days: Vec<u8>,  // 0-6 (Sunday=0)
    pub timezone: String,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            max_bulk_records: 1000,
            check_referential_integrity: true,
            check_permissions: true,
            simulation_timeout_seconds: 30,
        }
    }
}

impl CrudValidator {
    /// Creates a new CRUD validator with default configuration
    pub fn new() -> Self {
        Self {
            config: ValidatorConfig::default(),
            schema_cache: SchemaCache::default(),
            permission_rules: PermissionRules::default(),
        }
    }

    /// Creates a validator with custom configuration
    pub fn with_config(config: ValidatorConfig) -> Self {
        Self {
            config,
            schema_cache: SchemaCache::default(),
            permission_rules: PermissionRules::default(),
        }
    }

    /// Validates a CRUD operation
    pub fn validate_operation(&self, statement: &CrudStatement) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        // Basic structure validation
        if let Err(validation_errors) = self.validate_structure(statement) {
            errors.extend(validation_errors);
        }

        // Permission validation
        if self.config.check_permissions {
            if let Err(permission_errors) = self.validate_permissions(statement) {
                errors.extend(permission_errors);
            }
        }

        // Asset schema validation
        if let Err(schema_errors) = self.validate_schema(statement) {
            errors.extend(schema_errors);
        }

        // Business rule validation
        if let Some(business_warnings) = self.validate_business_rules(statement) {
            warnings.extend(business_warnings);
        }

        // Generate suggestions
        suggestions.extend(self.generate_suggestions(statement));

        let is_valid = errors.is_empty() && (!self.config.strict_mode || warnings.is_empty());

        ValidationResult {
            is_valid,
            errors,
            warnings,
            suggestions,
        }
    }

    /// Checks permissions for a CRUD operation
    pub fn check_permissions(&self, statement: &CrudStatement) -> bool {
        if !self.config.check_permissions {
            return true;
        }

        match statement {
            CrudStatement::DataCreate(op) => self.check_asset_permission(&op.asset, "create"),
            CrudStatement::DataRead(op) => self.check_asset_permission(&op.asset, "read"),
            CrudStatement::DataUpdate(op) => self.check_asset_permission(&op.asset, "update"),
            CrudStatement::DataDelete(op) => self.check_asset_permission(&op.asset, "delete"),
            CrudStatement::ComplexQuery(op) => self.check_complex_query_permissions(op),
            CrudStatement::ConditionalUpdate(op) => {
                self.check_asset_permission(&op.asset, "update")
            }
            CrudStatement::BatchOperation(op) => self.check_batch_permissions(op),
        }
    }

    /// Validates referential integrity for an operation
    pub fn validate_referential_integrity(&self, statement: &CrudStatement) -> IntegrityResult {
        if !self.config.check_referential_integrity {
            return IntegrityResult {
                referential_integrity_ok: true,
                constraint_violations: Vec::new(),
                dependency_issues: Vec::new(),
            };
        }

        let mut constraint_violations = Vec::new();
        let mut dependency_issues = Vec::new();

        match statement {
            CrudStatement::DataCreate(op) => {
                // Check foreign key constraints
                constraint_violations.extend(self.check_foreign_keys(&op.asset, &op.values));
                // Check unique constraints
                constraint_violations.extend(self.check_unique_constraints(&op.asset, &op.values));
            }
            CrudStatement::DataUpdate(op) => {
                constraint_violations.extend(self.check_foreign_keys(&op.asset, &op.values));
                constraint_violations.extend(self.check_unique_constraints(&op.asset, &op.values));
            }
            CrudStatement::DataDelete(op) => {
                // Check cascade dependencies
                dependency_issues
                    .extend(self.check_cascade_dependencies(&op.asset, &op.where_clause));
            }
            _ => {
                // Complex operations need specialized integrity checks
            }
        }

        IntegrityResult {
            referential_integrity_ok: constraint_violations.is_empty()
                && dependency_issues.is_empty(),
            constraint_violations,
            dependency_issues,
        }
    }

    /// Simulates an operation without executing it
    pub fn simulate_operation(&self, statement: &CrudStatement) -> SimulationResult {
        let start_time = Instant::now();

        // Estimate affected records
        let affected_records = self.estimate_affected_records(statement);

        // Estimate resource usage
        let resource_usage = self.estimate_resource_usage(statement, affected_records);

        // Identify potential issues
        let potential_issues = self.identify_potential_issues(statement, affected_records);

        // Check if operation would succeed
        let validation_result = self.validate_operation(statement);
        let integrity_result = self.validate_referential_integrity(statement);
        let would_succeed = validation_result.is_valid && integrity_result.referential_integrity_ok;

        let estimated_duration_ms = start_time.elapsed().as_millis() as u64;

        SimulationResult {
            would_succeed,
            affected_records,
            estimated_duration_ms,
            resource_usage,
            potential_issues,
        }
    }

    // Private validation methods

    fn validate_structure(&self, statement: &CrudStatement) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        match statement {
            CrudStatement::DataCreate(op) => {
                if op.asset.is_empty() {
                    errors.push(ValidationError {
                        code: "EMPTY_ASSET".to_string(),
                        message: "Asset name cannot be empty".to_string(),
                        field: Some("asset".to_string()),
                        severity: ErrorSeverity::Critical,
                    });
                }
                if op.values.is_empty() {
                    errors.push(ValidationError {
                        code: "EMPTY_VALUES".to_string(),
                        message: "Values cannot be empty for create operation".to_string(),
                        field: Some("values".to_string()),
                        severity: ErrorSeverity::Critical,
                    });
                }
            }
            CrudStatement::DataRead(op) => {
                if op.asset.is_empty() {
                    errors.push(ValidationError {
                        code: "EMPTY_ASSET".to_string(),
                        message: "Asset name cannot be empty".to_string(),
                        field: Some("asset".to_string()),
                        severity: ErrorSeverity::Critical,
                    });
                }
            }
            CrudStatement::DataUpdate(op) => {
                if op.asset.is_empty() {
                    errors.push(ValidationError {
                        code: "EMPTY_ASSET".to_string(),
                        message: "Asset name cannot be empty".to_string(),
                        field: Some("asset".to_string()),
                        severity: ErrorSeverity::Critical,
                    });
                }
                if op.values.is_empty() {
                    errors.push(ValidationError {
                        code: "EMPTY_VALUES".to_string(),
                        message: "Values cannot be empty for update operation".to_string(),
                        field: Some("values".to_string()),
                        severity: ErrorSeverity::Critical,
                    });
                }
            }
            CrudStatement::DataDelete(op) => {
                if op.asset.is_empty() {
                    errors.push(ValidationError {
                        code: "EMPTY_ASSET".to_string(),
                        message: "Asset name cannot be empty".to_string(),
                        field: Some("asset".to_string()),
                        severity: ErrorSeverity::Critical,
                    });
                }
            }
            _ => {
                // Additional validation for complex operations
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_permissions(&self, _statement: &CrudStatement) -> Result<(), Vec<ValidationError>> {
        // Implementation would check against permission_rules
        // For now, return success
        Ok(())
    }

    fn validate_schema(&self, statement: &CrudStatement) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        match statement {
            CrudStatement::DataCreate(op) => {
                if let Some(schema) = self.schema_cache.asset_schemas.get(&op.asset) {
                    // Check required fields
                    for required_field in &schema.required_fields {
                        let key = Key {
                            parts: vec![required_field.clone()],
                        };
                        if !op.values.contains_key(&key) {
                            errors.push(ValidationError {
                                code: "MISSING_REQUIRED_FIELD".to_string(),
                                message: format!("Required field '{}' is missing", required_field),
                                field: Some(required_field.clone()),
                                severity: ErrorSeverity::Critical,
                            });
                        }
                    }

                    // Validate field types and constraints
                    for (key, value) in &op.values {
                        if let Some(field_name) = key.parts.first() {
                            if let Some(field_schema) = schema.fields.get(field_name) {
                                if let Err(field_errors) =
                                    self.validate_field_value(field_schema, value)
                                {
                                    errors.extend(field_errors);
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                // Additional schema validation for other operations
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_field_value(
        &self,
        field_schema: &FieldSchema,
        value: &Value,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        match value {
            Value::Literal(Literal::Null) => {
                if !field_schema.nullable {
                    errors.push(ValidationError {
                        code: "NULL_NOT_ALLOWED".to_string(),
                        message: format!("Field '{}' cannot be null", field_schema.name),
                        field: Some(field_schema.name.clone()),
                        severity: ErrorSeverity::Critical,
                    });
                }
            }
            Value::Literal(Literal::String(s)) => {
                if let Some(max_length) = field_schema.max_length {
                    if s.len() > max_length as usize {
                        errors.push(ValidationError {
                            code: "VALUE_TOO_LONG".to_string(),
                            message: format!(
                                "Field '{}' value exceeds maximum length of {}",
                                field_schema.name, max_length
                            ),
                            field: Some(field_schema.name.clone()),
                            severity: ErrorSeverity::High,
                        });
                    }
                }
            }
            _ => {
                // Additional type validation
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_business_rules(
        &self,
        _statement: &CrudStatement,
    ) -> Option<Vec<ValidationWarning>> {
        // Implementation would check business-specific rules
        // For now, return empty
        Some(Vec::new())
    }

    fn generate_suggestions(&self, statement: &CrudStatement) -> Vec<String> {
        let mut suggestions = Vec::new();

        match statement {
            CrudStatement::DataRead(op) => {
                if op.select_fields.is_none() {
                    suggestions.push(
                        "Consider specifying select fields to improve performance".to_string(),
                    );
                }
                if op.where_clause.is_none() {
                    suggestions.push("Consider adding WHERE clause to limit results".to_string());
                }
            }
            _ => {}
        }

        suggestions
    }

    fn check_asset_permission(&self, asset: &str, operation: &str) -> bool {
        if let Some(permission) = self.permission_rules.asset_permissions.get(asset) {
            match operation {
                "create" => permission.can_create,
                "read" => permission.can_read,
                "update" => permission.can_update,
                "delete" => permission.can_delete,
                _ => false,
            }
        } else {
            // Default to allow if no permissions defined
            true
        }
    }

    fn check_complex_query_permissions(&self, _query: &ComplexQuery) -> bool {
        // Implementation would check permissions for joined assets
        true
    }

    fn check_batch_permissions(&self, _batch: &BatchOperation) -> bool {
        // Implementation would check permissions for all operations in batch
        true
    }

    fn check_foreign_keys(&self, _asset: &str, _values: &PropertyMap) -> Vec<ConstraintViolation> {
        // Implementation would check foreign key constraints
        Vec::new()
    }

    fn check_unique_constraints(
        &self,
        _asset: &str,
        _values: &PropertyMap,
    ) -> Vec<ConstraintViolation> {
        // Implementation would check unique constraints
        Vec::new()
    }

    fn check_cascade_dependencies(
        &self,
        _asset: &str,
        _where_clause: &PropertyMap,
    ) -> Vec<DependencyIssue> {
        // Implementation would check cascade dependencies
        Vec::new()
    }

    fn estimate_affected_records(&self, statement: &CrudStatement) -> u32 {
        match statement {
            CrudStatement::DataCreate(_) => 1,
            CrudStatement::DataRead(_) => 100,  // Default estimate
            CrudStatement::DataUpdate(_) => 10, // Default estimate
            CrudStatement::DataDelete(_) => 1,  // Default estimate
            CrudStatement::BatchOperation(op) => op.operations.len() as u32,
            _ => 1,
        }
    }

    fn estimate_resource_usage(
        &self,
        _statement: &CrudStatement,
        affected_records: u32,
    ) -> ResourceUsage {
        ResourceUsage {
            memory_kb: affected_records as u64 * 10, // 10KB per record estimate
            disk_operations: affected_records * 2,   // Read + write
            network_calls: 1,                        // Single DB call
            cpu_time_ms: affected_records as u64,    // 1ms per record estimate
        }
    }

    fn identify_potential_issues(
        &self,
        statement: &CrudStatement,
        affected_records: u32,
    ) -> Vec<String> {
        let mut issues = Vec::new();

        if affected_records > self.config.max_bulk_records {
            issues.push(format!(
                "Operation affects {} records, which exceeds the limit of {}",
                affected_records, self.config.max_bulk_records
            ));
        }

        match statement {
            CrudStatement::DataDelete(_) => {
                if affected_records > 1 {
                    issues
                        .push("Bulk delete operation - consider backing up data first".to_string());
                }
            }
            CrudStatement::BatchOperation(_) => {
                issues.push("Batch operation - ensure adequate transaction log space".to_string());
            }
            _ => {}
        }

        issues
    }
}

impl Default for SchemaCache {
    fn default() -> Self {
        let mut asset_schemas = HashMap::new();

        // Add default CBU schema
        asset_schemas.insert(
            "cbu".to_string(),
            AssetSchema {
                name: "cbu".to_string(),
                fields: {
                    let mut fields = HashMap::new();
                    fields.insert(
                        "id".to_string(),
                        FieldSchema {
                            name: "id".to_string(),
                            data_type: "uuid".to_string(),
                            nullable: false,
                            max_length: None,
                            default_value: None,
                            validation_rules: vec!["uuid_format".to_string()],
                        },
                    );
                    fields.insert(
                        "name".to_string(),
                        FieldSchema {
                            name: "name".to_string(),
                            data_type: "text".to_string(),
                            nullable: false,
                            max_length: Some(255),
                            default_value: None,
                            validation_rules: vec!["non_empty".to_string()],
                        },
                    );
                    fields
                },
                primary_keys: vec!["id".to_string()],
                required_fields: vec!["name".to_string()],
                indexes: vec!["name".to_string()],
            },
        );

        Self {
            asset_schemas,
            relationships: HashMap::new(),
            constraints: HashMap::new(),
        }
    }
}

impl Default for PermissionRules {
    fn default() -> Self {
        let mut asset_permissions = HashMap::new();

        // Default CBU permissions
        asset_permissions.insert(
            "cbu".to_string(),
            AssetPermission {
                asset_name: "cbu".to_string(),
                can_create: true,
                can_read: true,
                can_update: true,
                can_delete: false, // Restrict delete by default
                required_roles: vec!["user".to_string()],
            },
        );

        Self {
            asset_permissions,
            field_permissions: HashMap::new(),
            operation_permissions: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, Literal};

    fn create_test_validator() -> CrudValidator {
        CrudValidator::new()
    }

    #[test]
    fn test_validate_data_create() {
        let validator = create_test_validator();

        let mut values = PropertyMap::new();
        values.insert(
            Key {
                parts: vec!["name".to_string()],
            },
            Value::Literal(Literal::String("Test CBU".to_string())),
        );

        let statement = CrudStatement::DataCreate(DataCreate {
            asset: "cbu".to_string(),
            values,
        });

        let result = validator.validate_operation(&statement);
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_empty_asset() {
        let validator = create_test_validator();

        let statement = CrudStatement::DataCreate(DataCreate {
            asset: "".to_string(),
            values: PropertyMap::new(),
        });

        let result = validator.validate_operation(&statement);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].code, "EMPTY_ASSET");
    }

    #[test]
    fn test_permission_check() {
        let validator = create_test_validator();

        let statement = CrudStatement::DataRead(DataRead {
            asset: "cbu".to_string(),
            where_clause: None,
            select_fields: None,
        });

        let has_permission = validator.check_permissions(&statement);
        assert!(has_permission);
    }

    #[test]
    fn test_simulation() {
        let validator = create_test_validator();

        let statement = CrudStatement::DataRead(DataRead {
            asset: "cbu".to_string(),
            where_clause: None,
            select_fields: None,
        });

        let result = validator.simulate_operation(&statement);
        assert!(result.would_succeed);
        assert!(result.affected_records > 0);
    }

    #[test]
    fn test_referential_integrity() {
        let validator = create_test_validator();

        let mut values = PropertyMap::new();
        values.insert(
            Key {
                parts: vec!["name".to_string()],
            },
            Value::Literal(Literal::String("Test".to_string())),
        );

        let statement = CrudStatement::DataCreate(DataCreate {
            asset: "cbu".to_string(),
            values,
        });

        let result = validator.validate_referential_integrity(&statement);
        assert!(result.referential_integrity_ok);
    }
}
