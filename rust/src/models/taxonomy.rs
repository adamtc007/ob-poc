//! Product-Service-Resource Taxonomy Models

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================
// Product Models
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Product {
    pub product_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub product_code: Option<String>,
    pub product_category: Option<String>,
    pub regulatory_framework: Option<String>,
    pub min_asset_requirement: Option<BigDecimal>,
    pub is_active: Option<bool>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Service {
    pub service_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub service_code: Option<String>,
    pub service_category: Option<String>,
    pub sla_definition: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductService {
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub is_mandatory: Option<bool>,
    pub is_default: Option<bool>,
    pub display_order: Option<i32>,
    pub configuration: Option<serde_json::Value>,
}

// ============================================
// Service Options
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OptionType {
    SingleSelect,
    MultiSelect,
    Numeric,
    Boolean,
    Text,
}

impl From<String> for OptionType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "single_select" => Self::SingleSelect,
            "multi_select" => Self::MultiSelect,
            "numeric" => Self::Numeric,
            "boolean" => Self::Boolean,
            "text" => Self::Text,
            _ => Self::Text,
        }
    }
}

impl ToString for OptionType {
    fn to_string(&self) -> String {
        match self {
            Self::SingleSelect => "single_select".to_string(),
            Self::MultiSelect => "multi_select".to_string(),
            Self::Numeric => "numeric".to_string(),
            Self::Boolean => "boolean".to_string(),
            Self::Text => "text".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceOptionDefinition {
    pub option_def_id: Uuid,
    pub service_id: Uuid,
    pub option_key: String,
    pub option_label: Option<String>,
    pub option_type: String,
    pub validation_rules: Option<serde_json::Value>,
    pub is_required: Option<bool>,
    pub display_order: Option<i32>,
    pub help_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceOptionChoice {
    pub choice_id: Uuid,
    pub option_def_id: Uuid,
    pub choice_value: String,
    pub choice_label: Option<String>,
    pub choice_metadata: Option<serde_json::Value>,
    pub is_default: Option<bool>,
    pub is_active: Option<bool>,
    pub display_order: Option<i32>,
    pub requires_options: Option<serde_json::Value>,
    pub excludes_options: Option<serde_json::Value>,
}

// ============================================
// Production Resources
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductionResource {
    pub resource_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner: String,
    pub dictionary_group: Option<String>,
    pub resource_code: Option<String>,
    pub resource_type: Option<String>,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub api_endpoint: Option<String>,
    pub api_version: Option<String>,
    pub authentication_method: Option<String>,
    pub authentication_config: Option<serde_json::Value>,
    pub capabilities: Option<serde_json::Value>,
    pub capacity_limits: Option<serde_json::Value>,
    pub maintenance_windows: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceResourceCapability {
    pub capability_id: Uuid,
    pub service_id: Uuid,
    pub resource_id: Uuid,
    pub supported_options: serde_json::Value,
    pub priority: Option<i32>,
    pub cost_factor: Option<BigDecimal>,
    pub performance_rating: Option<i32>,
    pub resource_config: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ResourceAttributeRequirement {
    pub requirement_id: Uuid,
    pub resource_id: Uuid,
    pub attribute_id: Uuid,
    pub resource_field_name: Option<String>,
    pub is_mandatory: Option<bool>,
    pub transformation_rule: Option<serde_json::Value>,
    pub validation_override: Option<serde_json::Value>,
}

// ============================================
// Onboarding Models
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OnboardingState {
    Draft,
    ProductsSelected,
    ServicesDiscovered,
    ServicesConfigured,
    ResourcesAllocated,
    Complete,
}

impl From<String> for OnboardingState {
    fn from(s: String) -> Self {
        match s.as_str() {
            "draft" => Self::Draft,
            "products_selected" => Self::ProductsSelected,
            "services_discovered" => Self::ServicesDiscovered,
            "services_configured" => Self::ServicesConfigured,
            "resources_allocated" => Self::ResourcesAllocated,
            "complete" => Self::Complete,
            _ => Self::Draft,
        }
    }
}

impl ToString for OnboardingState {
    fn to_string(&self) -> String {
        match self {
            Self::Draft => "draft".to_string(),
            Self::ProductsSelected => "products_selected".to_string(),
            Self::ServicesDiscovered => "services_discovered".to_string(),
            Self::ServicesConfigured => "services_configured".to_string(),
            Self::ResourcesAllocated => "resources_allocated".to_string(),
            Self::Complete => "complete".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OnboardingRequest {
    pub request_id: Uuid,
    pub cbu_id: Uuid,
    pub request_state: String,
    pub dsl_draft: Option<String>,
    pub dsl_version: Option<i32>,
    pub current_phase: Option<String>,
    pub phase_metadata: Option<serde_json::Value>,
    pub validation_errors: Option<serde_json::Value>,
    pub created_by: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OnboardingProduct {
    pub onboarding_product_id: Uuid,
    pub request_id: Uuid,
    pub product_id: Uuid,
    pub selection_order: Option<i32>,
    pub selected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OnboardingServiceConfig {
    pub config_id: Uuid,
    pub request_id: Uuid,
    pub service_id: Uuid,
    pub option_selections: serde_json::Value,
    pub is_valid: Option<bool>,
    pub validation_messages: Option<serde_json::Value>,
    pub configured_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OnboardingResourceAllocation {
    pub allocation_id: Uuid,
    pub request_id: Uuid,
    pub service_id: Uuid,
    pub resource_id: Uuid,
    pub handles_options: Option<serde_json::Value>,
    pub required_attributes: Option<Vec<Uuid>>,
    pub allocation_status: Option<String>,
    pub allocated_at: Option<DateTime<Utc>>,
}

// ============================================
// DTOs for Service Discovery
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceWithOptions {
    pub service: Service,
    pub options: Vec<ServiceOptionWithChoices>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceOptionWithChoices {
    pub definition: ServiceOptionDefinition,
    pub choices: Vec<ServiceOptionChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocationRequest {
    pub service_id: Uuid,
    pub resource_id: Uuid,
    pub handles_options: HashMap<String, Vec<String>>,
    pub required_attributes: Vec<Uuid>,
}
