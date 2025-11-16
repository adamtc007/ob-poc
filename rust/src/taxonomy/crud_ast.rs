//! AST definitions for taxonomy CRUD operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Main CRUD statement types for taxonomy operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum TaxonomyCrudStatement {
    // Product operations
    CreateProduct(CreateProduct),
    ReadProduct(ReadProduct),
    UpdateProduct(UpdateProduct),
    DeleteProduct(DeleteProduct),
    ListProducts(ListProducts),

    // Service operations
    CreateService(CreateService),
    ReadService(ReadService),
    UpdateService(UpdateService),
    DeleteService(DeleteService),
    DiscoverServices(DiscoverServices),
    ConfigureService(ConfigureService),

    // Resource operations
    CreateResource(CreateResource),
    ReadResource(ReadResource),
    UpdateResource(UpdateResource),
    DeleteResource(DeleteResource),
    AllocateResource(AllocateResource),
    FindCapableResources(FindCapableResources),

    // Onboarding workflow operations
    CreateOnboarding(CreateOnboarding),
    ReadOnboarding(ReadOnboarding),
    UpdateOnboardingState(UpdateOnboardingState),
    AddProductsToOnboarding(AddProductsToOnboarding),
    FinalizeOnboarding(FinalizeOnboarding),

    // Complex queries
    QueryWorkflow(QueryWorkflow),
    GenerateCompleteDsl(GenerateCompleteDsl),
}

// Product CRUD structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProduct {
    pub product_code: String,
    pub name: String,
    pub category: Option<String>,
    pub regulatory_framework: Option<String>,
    pub min_asset_requirement: Option<f64>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadProduct {
    pub identifier: ProductIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProductIdentifier {
    Id(Uuid),
    Code(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProduct {
    pub identifier: ProductIdentifier,
    pub updates: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProduct {
    pub identifier: ProductIdentifier,
    pub soft_delete: bool, // If true, just mark as inactive
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListProducts {
    pub filter: Option<ProductFilter>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductFilter {
    pub active_only: bool,
    pub category: Option<String>,
    pub regulatory_framework: Option<String>,
}

// Service CRUD structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateService {
    pub service_code: String,
    pub name: String,
    pub category: Option<String>,
    pub sla_definition: Option<serde_json::Value>,
    pub options: Vec<ServiceOptionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceOptionDef {
    pub option_key: String,
    pub option_type: String, // single_select, multi_select, numeric, boolean, text
    pub is_required: bool,
    pub choices: Vec<String>, // For select types
    pub validation_rules: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadService {
    pub identifier: ServiceIdentifier,
    pub include_options: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceIdentifier {
    Id(Uuid),
    Code(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateService {
    pub identifier: ServiceIdentifier,
    pub updates: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteService {
    pub identifier: ServiceIdentifier,
    pub cascade: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverServices {
    pub product_id: Uuid,
    pub include_optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureService {
    pub onboarding_id: Uuid,
    pub service_code: String,
    pub options: HashMap<String, serde_json::Value>,
}

// Resource CRUD structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResource {
    pub resource_code: String,
    pub name: String,
    pub resource_type: String,
    pub vendor: Option<String>,
    pub api_endpoint: Option<String>,
    pub capabilities: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResource {
    pub identifier: ResourceIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceIdentifier {
    Id(Uuid),
    Code(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResource {
    pub identifier: ResourceIdentifier,
    pub updates: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResource {
    pub identifier: ResourceIdentifier,
    pub cascade: bool, // Delete dependent allocations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocateResource {
    pub onboarding_id: Uuid,
    pub service_id: Uuid,
    pub strategy: AllocationStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AllocationStrategy {
    Auto, // System decides
    LowestCost,
    HighestPerformance,
    RoundRobin,
    Specific(Uuid), // Specific resource ID
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindCapableResources {
    pub service_code: String,
    pub required_options: HashMap<String, serde_json::Value>,
}

// Onboarding workflow structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOnboarding {
    pub cbu_id: Uuid,
    pub initiated_by: String,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadOnboarding {
    pub onboarding_id: Uuid,
    pub include_details: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOnboardingState {
    pub onboarding_id: Uuid,
    pub new_state: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProductsToOnboarding {
    pub onboarding_id: Uuid,
    pub product_codes: Vec<String>,
    pub auto_discover_services: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeOnboarding {
    pub onboarding_id: Uuid,
    pub generate_artifacts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryWorkflow {
    pub onboarding_id: Uuid,
    pub include_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCompleteDsl {
    pub onboarding_id: Uuid,
    pub format: DslFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DslFormat {
    Lisp,    // S-expression format
    Json,    // JSON representation
    Yaml,    // YAML format
    Natural, // Natural language description
}
