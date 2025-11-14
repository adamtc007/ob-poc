//! Parser-specific AST types
//!
//! This module contains the AST types used by the nom-based parser for the DSL.
//! These are simpler, flatter structures optimized for parsing performance
//! and are separate from the main AST types used by the rest of the system.
//!
//! ## PUBLIC FACADE
//! Only the essential types needed by external consumers are exposed.
//! All internal implementation details are kept private to this module.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Import types from dsl_types crate (Level 1 foundation)
pub use dsl_types::{RollbackStrategy, TransactionMode};

// ============================================================================
// PUBLIC FACADE - Core AST Types for External Consumers
// ============================================================================

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Number(f64),
    Boolean(bool),
    Date(String), // ISO 8601 string, e.g., "2025-11-10T10:30:00Z"
    Uuid(String), // UUID string, e.g., "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub parts: Vec<String>, // e.g., ["customer", "id"] for :customer.id
}

impl Key {
    pub fn new(s: &str) -> Self {
        Self {
            parts: s.split('.').map(|p| p.to_string()).collect(),
        }
    }

    pub fn as_str(&self) -> String {
        self.parts.join(".")
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Value {
    Literal(Literal),
    Identifier(String), // For unquoted symbols
    List(Vec<Value>),
    Map(HashMap<Key, Value>),
    AttrRef(String), // Semantic ID reference, e.g., "@attr.identity.first_name"
    AttrUuid(Uuid),  // UUID-based reference, e.g., "@attr{3020d46f-...}"
    AttrUuidWithSource(Uuid, String), // UUID with source hint, e.g., "@attr{uuid}:doc"
    AttrRefWithSource(String, String), // Semantic ID with source hint, e.g., "@attr.identity.name:doc"
    // Additional variants needed for CRUD operations
    String(String),
    Integer(i32),
    Double(f64),
    Boolean(bool),
    Array(Vec<Value>),
    Json(serde_json::Value),
}

// Helper for property maps, for consistency with old PropertyMap, but uses new Key/Value
pub type PropertyMap = HashMap<Key, Value>;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VerbForm {
    pub verb: String,
    pub pairs: PropertyMap, // Using PropertyMap for key-value pairs
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Form {
    Verb(VerbForm),
    Comment(String),
}

// Program is now a sequence of forms (workflow replaced by a specific verb form)
pub type Program = Vec<Form>;

// --- Agentic CRUD AST Structures ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CrudStatement {
    DataCreate(DataCreate),
    DataRead(DataRead),
    DataUpdate(DataUpdate),
    DataDelete(DataDelete),
    // Phase 3: Advanced operations
    ComplexQuery(ComplexQuery),
    ConditionalUpdate(ConditionalUpdate),
    BatchOperation(BatchOperation),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataCreate {
    pub asset: String,
    pub values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRead {
    pub asset: String,
    pub where_clause: HashMap<String, Value>,
    pub select: Vec<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataUpdate {
    pub asset: String,
    pub where_clause: HashMap<String, Value>,
    pub values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataDelete {
    pub asset: String,
    pub where_clause: HashMap<String, Value>,
}

// --- Phase 3: Advanced CRUD Structures ---

// ============================================================================
// INTERNAL CRUD STRUCTURES - Not exposed to external consumers
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComplexQuery {
    pub primary_asset: String,
    pub joins: Option<Vec<JoinClause>>,
    pub conditions: HashMap<String, Value>,
    pub aggregate: Option<AggregateClause>,
    pub select_fields: Vec<String>,
    pub order_by: Option<Vec<OrderClause>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub target_asset: String,
    pub on_condition: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateClause {
    pub operations: Vec<AggregateOperation>,
    pub group_by: Option<Vec<String>>,
    pub having: Option<PropertyMap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateOperation {
    pub function: AggregateFunction,
    pub field: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    CountDistinct,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderClause {
    pub field: String,
    pub direction: OrderDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalUpdate {
    pub asset: String,
    pub primary_condition: HashMap<String, Value>,
    pub if_exists: Option<HashMap<String, Value>>,
    pub if_not_exists: Option<HashMap<String, Value>>,
    pub values: HashMap<String, Value>,
    pub increment_values: Option<HashMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchOperation {
    pub operations: Vec<CrudStatement>,
    pub transaction_mode: TransactionMode,
    pub rollback_strategy: RollbackStrategy,
}

// TransactionMode and RollbackStrategy moved to dsl_types crate - import from there

// --- Transaction Management ---

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct CrudTransaction {
    pub operations: Vec<CrudStatement>,
    pub rollback_strategy: RollbackStrategy,
    pub atomic: bool,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TransactionResult {
    pub success: bool,
    pub completed_operations: Vec<usize>,
    pub failed_operations: Vec<(usize, String)>,
    pub rollback_performed: bool,
    pub total_duration_ms: u64,
}

// --- Validation Structures ---

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<ValidationWarning>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct IntegrityResult {
    pub referential_integrity_ok: bool,
    pub constraint_violations: Vec<ConstraintViolation>,
    pub dependency_issues: Vec<DependencyIssue>,
}

#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub constraint_name: String,
    pub violation_type: ConstraintType,
    pub affected_records: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintType {
    ForeignKey,
    Unique,
    NotNull,
    Check,
    Custom,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct DependencyIssue {
    pub dependent_table: String,
    pub dependency_type: DependencyType,
    pub affected_count: u32,
    pub resolution_hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum DependencyType {
    Cascade,
    Restrict,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct SimulationResult {
    pub would_succeed: bool,
    pub affected_records: u32,
    pub estimated_duration_ms: u64,
    pub resource_usage: ResourceUsage,
    pub potential_issues: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ResourceUsage {
    pub memory_kb: u64,
    pub disk_operations: u32,
    pub network_calls: u32,
    pub cpu_time_ms: u64,
}
