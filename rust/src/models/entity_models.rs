//! Entity Models for Agentic CRUD Operations
//!
//! This module defines the data models for entity CRUD operations that integrate
//! with the existing database schema and support the agentic DSL system.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// CORE ENTITY MODELS
// ============================================================================

/// Central entity registry entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Entity {
    pub entity_id: Uuid,
    pub entity_type_id: Uuid,
    pub external_id: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Entity type definition
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityType {
    pub entity_type_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub table_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// CBU to Entity role mapping
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuEntityRole {
    pub cbu_entity_role_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub role_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// ENTITY TYPE SPECIFIC MODELS
// ============================================================================

/// Limited Company entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LimitedCompany {
    pub limited_company_id: Uuid,
    pub company_name: String,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub registered_address: Option<String>,
    pub business_nature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Partnership entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Partnership {
    pub partnership_id: Uuid,
    pub partnership_name: String,
    pub partnership_type: Option<String>, // 'General', 'Limited', 'Limited Liability'
    pub jurisdiction: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub principal_place_business: Option<String>,
    pub partnership_agreement_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Proper Person (Natural Person) entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProperPerson {
    pub proper_person_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub middle_names: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub residence_address: Option<String>,
    pub id_document_type: Option<String>, // 'Passport', 'National ID', 'Driving License'
    pub id_document_number: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Trust entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Trust {
    pub trust_id: Uuid,
    pub trust_name: String,
    pub trust_type: Option<String>, // 'Discretionary', 'Fixed Interest', 'Unit Trust', 'Charitable'
    pub jurisdiction: String,
    pub establishment_date: Option<NaiveDate>,
    pub trust_deed_date: Option<NaiveDate>,
    pub trust_purpose: Option<String>,
    pub governing_law: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// TRUST RELATIONSHIP MODELS
// ============================================================================

/// Trust party relationships
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrustParty {
    pub trust_party_id: Uuid,
    pub trust_id: Uuid,
    pub entity_id: Uuid,
    pub party_role: String, // 'SETTLOR', 'TRUSTEE', 'BENEFICIARY', 'PROTECTOR'
    pub party_type: String, // 'PROPER_PERSON', 'CORPORATE_TRUSTEE', 'BENEFICIARY_CLASS'
    pub appointment_date: Option<NaiveDate>,
    pub resignation_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Trust beneficiary classes
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrustBeneficiaryClass {
    pub beneficiary_class_id: Uuid,
    pub trust_id: Uuid,
    pub class_name: String,
    pub class_definition: Option<String>,
    pub class_type: Option<String>, // 'DESCENDANTS', 'SPOUSE_FAMILY', 'CHARITABLE_CLASS'
    pub monitoring_required: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Trust protector powers
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrustProtectorPower {
    pub protector_power_id: Uuid,
    pub trust_party_id: Uuid,
    pub power_type: String, // 'TRUSTEE_APPOINTMENT', 'TRUSTEE_REMOVAL', 'DISTRIBUTION_VETO'
    pub power_description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// PARTNERSHIP RELATIONSHIP MODELS
// ============================================================================

/// Partnership interests and ownership
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnershipInterest {
    pub interest_id: Uuid,
    pub partnership_id: Uuid,
    pub entity_id: Uuid,
    pub partner_type: String, // 'GENERAL_PARTNER', 'LIMITED_PARTNER', 'MANAGING_PARTNER'
    pub capital_commitment: Option<rust_decimal::Decimal>,
    pub ownership_percentage: Option<rust_decimal::Decimal>,
    pub voting_rights: Option<rust_decimal::Decimal>,
    pub profit_sharing_percentage: Option<rust_decimal::Decimal>,
    pub admission_date: Option<NaiveDate>,
    pub withdrawal_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Partnership control mechanisms
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnershipControlMechanism {
    pub control_mechanism_id: Uuid,
    pub partnership_id: Uuid,
    pub entity_id: Uuid,
    pub control_type: String, // 'MANAGEMENT_AGREEMENT', 'GP_CONTROL', 'INVESTMENT_COMMITTEE'
    pub control_description: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// AGENTIC CRUD MODELS
// ============================================================================

/// CRUD operation tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CrudOperation {
    pub operation_id: Uuid,
    pub operation_type: String, // CREATE, READ, UPDATE, DELETE
    pub asset_type: String,     // CBU, ENTITY, PARTNERSHIP, etc.
    pub entity_table_name: Option<String>,
    pub generated_dsl: String,
    pub ai_instruction: String,
    pub affected_records: serde_json::Value, // JSONB
    pub execution_status: String,            // PENDING, EXECUTING, COMPLETED, FAILED, ROLLED_BACK
    pub ai_confidence: Option<rust_decimal::Decimal>,
    pub ai_provider: Option<String>,
    pub ai_model: Option<String>,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub rows_affected: i32,
    pub transaction_id: Option<Uuid>,
    pub parent_operation_id: Option<Uuid>,
}

/// RAG embeddings for context retrieval
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RagEmbedding {
    pub embedding_id: Uuid,
    pub content_type: String, // SCHEMA, EXAMPLE, ATTRIBUTE, RULE, GRAMMAR, VERB_PATTERN
    pub content_text: String,
    pub embedding_data: Option<serde_json::Value>, // JSONB
    pub metadata: serde_json::Value,               // JSONB
    pub source_table: Option<String>,
    pub asset_type: Option<String>,
    pub relevance_score: rust_decimal::Decimal,
    pub usage_count: i32,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// DSL examples library
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslExample {
    pub example_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub operation_type: String, // CREATE, READ, UPDATE, DELETE
    pub asset_type: String,     // CBU, ENTITY, PARTNERSHIP, etc.
    pub entity_table_name: Option<String>,
    pub natural_language_input: String,
    pub example_dsl: String,
    pub expected_outcome: Option<String>,
    pub tags: Vec<String>,        // TEXT[]
    pub complexity_level: String, // SIMPLE, MEDIUM, COMPLEX
    pub success_rate: rust_decimal::Decimal,
    pub usage_count: i32,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

/// Entity CRUD validation rules
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityCrudRule {
    pub rule_id: Uuid,
    pub entity_table_name: String,
    pub operation_type: String, // CREATE, READ, UPDATE, DELETE
    pub field_name: Option<String>,
    pub constraint_type: String, // REQUIRED, UNIQUE, FOREIGN_KEY, VALIDATION, BUSINESS_RULE
    pub constraint_description: String,
    pub validation_pattern: Option<String>,
    pub error_message: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// COMPOSITE MODELS FOR API RESPONSES
// ============================================================================

/// Entity with its type information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityWithType {
    pub entity: Entity,
    pub entity_type: EntityType,
}

/// Entity with its specific type data (polymorphic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityWithDetails {
    pub entity: Entity,
    pub entity_type: EntityType,
    pub details: EntityDetails,
}

/// Polymorphic entity details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "entity_type", content = "data")]
pub enum EntityDetails {
    LimitedCompany(LimitedCompany),
    Partnership(Partnership),
    ProperPerson(ProperPerson),
    Trust(Trust),
}

/// Trust with all its relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustWithRelationships {
    pub trust: Trust,
    pub parties: Vec<TrustPartyWithEntity>,
    pub beneficiary_classes: Vec<TrustBeneficiaryClass>,
    pub protector_powers: Vec<TrustProtectorPowerWithParty>,
}

/// Trust party with entity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPartyWithEntity {
    pub trust_party: TrustParty,
    pub entity: Entity,
    pub entity_details: Option<EntityDetails>,
}

/// Trust protector power with party information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustProtectorPowerWithParty {
    pub protector_power: TrustProtectorPower,
    pub trust_party: TrustParty,
    pub entity: Entity,
}

/// Partnership with all its relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartnershipWithRelationships {
    pub partnership: Partnership,
    pub interests: Vec<PartnershipInterestWithEntity>,
    pub control_mechanisms: Vec<PartnershipControlMechanismWithEntity>,
}

/// Partnership interest with entity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartnershipInterestWithEntity {
    pub interest: PartnershipInterest,
    pub entity: Entity,
    pub entity_details: Option<EntityDetails>,
}

/// Partnership control mechanism with entity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartnershipControlMechanismWithEntity {
    pub control_mechanism: PartnershipControlMechanism,
    pub entity: Entity,
    pub entity_details: Option<EntityDetails>,
}

// ============================================================================
// ENUMS FOR TYPE SAFETY
// ============================================================================

/// Supported entity asset types for CRUD operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityAssetType {
    Entity,
    LimitedCompany,
    Partnership,
    ProperPerson,
    Trust,
}

impl EntityAssetType {
    pub fn table_name(&self) -> &'static str {
        match self {
            Self::Entity => "entities",
            Self::LimitedCompany => "entity_limited_companies",
            Self::Partnership => "entity_partnerships",
            Self::ProperPerson => "entity_proper_persons",
            Self::Trust => "entity_trusts",
        }
    }

    pub fn asset_name(&self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::LimitedCompany => "limited_company",
            Self::Partnership => "partnership",
            Self::ProperPerson => "proper_person",
            Self::Trust => "trust",
        }
    }
}

impl std::fmt::Display for EntityAssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.asset_name())
    }
}

impl std::str::FromStr for EntityAssetType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "entity" => Ok(Self::Entity),
            "limited_company" => Ok(Self::LimitedCompany),
            "partnership" => Ok(Self::Partnership),
            "proper_person" => Ok(Self::ProperPerson),
            "trust" => Ok(Self::Trust),
            _ => Err(format!("Unknown entity asset type: {}", s)),
        }
    }
}

/// CRUD operation types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CrudOperationType {
    Create,
    Read,
    Update,
    Delete,
}

impl std::fmt::Display for CrudOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "CREATE"),
            Self::Read => write!(f, "READ"),
            Self::Update => write!(f, "UPDATE"),
            Self::Delete => write!(f, "DELETE"),
        }
    }
}

impl std::str::FromStr for CrudOperationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "CREATE" => Ok(Self::Create),
            "READ" => Ok(Self::Read),
            "UPDATE" => Ok(Self::Update),
            "DELETE" => Ok(Self::Delete),
            _ => Err(format!("Unknown CRUD operation type: {}", s)),
        }
    }
}

/// Execution status for CRUD operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    RolledBack,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Executing => write!(f, "EXECUTING"),
            Self::Completed => write!(f, "COMPLETED"),
            Self::Failed => write!(f, "FAILED"),
            Self::RolledBack => write!(f, "ROLLED_BACK"),
        }
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "EXECUTING" => Ok(Self::Executing),
            "COMPLETED" => Ok(Self::Completed),
            "FAILED" => Ok(Self::Failed),
            "ROLLED_BACK" => Ok(Self::RolledBack),
            _ => Err(format!("Unknown execution status: {}", s)),
        }
    }
}

// ============================================================================
// REQUEST/RESPONSE MODELS FOR API
// ============================================================================

/// Request for creating an entity via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEntityCreateRequest {
    pub instruction: String,
    pub asset_type: EntityAssetType,
    pub context: HashMap<String, serde_json::Value>,
    pub constraints: Vec<String>,
    pub link_to_cbu: Option<Uuid>,
    pub role_in_cbu: Option<String>,
}

/// Request for reading entities via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEntityReadRequest {
    pub instruction: String,
    pub asset_types: Vec<EntityAssetType>,
    pub filters: HashMap<String, serde_json::Value>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Request for updating entities via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEntityUpdateRequest {
    pub instruction: String,
    pub asset_type: EntityAssetType,
    pub identifier: HashMap<String, serde_json::Value>,
    pub updates: HashMap<String, serde_json::Value>,
}

/// Request for deleting entities via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEntityDeleteRequest {
    pub instruction: String,
    pub asset_type: EntityAssetType,
    pub identifier: HashMap<String, serde_json::Value>,
    pub cascade: bool,
}

/// Response from agentic entity CRUD operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEntityCrudResponse {
    pub operation_id: Uuid,
    pub generated_dsl: String,
    pub execution_status: ExecutionStatus,
    pub affected_records: Vec<Uuid>,
    pub ai_explanation: String,
    pub ai_confidence: Option<f64>,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub rag_context_used: Vec<String>,
}
