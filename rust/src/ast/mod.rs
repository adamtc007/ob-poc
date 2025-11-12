//! Abstract Syntax Tree (AST) structures for the UBO/KYC DSL
//!
//! This module defines the in-memory representation of parsed DSL programs.
//! The AST preserves all semantic information from the source DSL for
//! validation, transformation, and execution.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod types;
pub(crate) mod visitors;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub workflows: Vec<Workflow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub properties: PropertyMap,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    DeclareEntity {
        id: String,
        entity_type: String,
        properties: PropertyMap,
    },
    ObtainDocument {
        document_type: String,
        source: String,
        properties: PropertyMap,
    },
    CreateEdge {
        from: String,
        to: String,
        edge_type: String,
        properties: PropertyMap,
    },
    CalculateUbo {
        entity_id: String,
        properties: PropertyMap,
    },
    // Legacy variants for compatibility
    SolicitAttribute(SolicitAttribute),
    ResolveConflict(ResolveConflict),
    GenerateReport(GenerateReport),
    ScheduleMonitoring(ScheduleMonitoring),
    ParallelObtain(ParallelObtain),
    Parallel(Vec<Statement>),
    Sequential(Vec<Statement>),
    // Placeholder for unknown statement types during parsing
    Placeholder {
        command: String,
        args: Vec<Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclareEntity {
    pub node_id: String,
    pub label: EntityLabel,
    pub properties: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityLabel {
    Company,
    Person,
    Trust,
    Address,
    Document,
    Officer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObtainDocument {
    pub doc_id: String,
    pub doc_type: String,
    pub issuer: String,
    pub issue_date: NaiveDate,
    pub confidence: f64,
    pub additional_props: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ParallelObtain {
    pub documents: Vec<ObtainDocument>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub properties: PropertyMap,
    pub evidenced_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    HasOwnership,
    HasControl,
    IsDirectorOf,
    IsSecretaryOf,
    HasShareholder,
    ResidesAt,
    HasRegisteredOffice,
    EvidencedBy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SolicitAttribute {
    pub attr_id: String,
    pub from: String,
    pub value_type: String,
    pub additional_props: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CalculateUbo {
    pub target: String,
    pub algorithm: String,
    pub max_depth: usize,
    pub threshold: f64,
    pub traversal_rules: PropertyMap,
    pub output: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ResolveConflict {
    pub node: String,
    pub property: String,
    pub strategy: WaterfallStrategy,
    pub resolution: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct WaterfallStrategy {
    pub priorities: Vec<SourcePriority>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SourcePriority {
    pub source_type: SourceType,
    pub name: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum SourceType {
    PrimarySource,
    GovernmentRegistry,
    ThirdPartyService,
    SelfDeclared,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct GenerateReport {
    pub target: String,
    pub status: String,
    pub identified_ubos: Vec<PropertyMap>,
    pub unresolved_prongs: Vec<PropertyMap>,
    pub additional_props: PropertyMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ScheduleMonitoring {
    pub target: String,
    pub frequency: String,
    pub triggers: Vec<PropertyMap>,
    pub additional_props: PropertyMap,
}

/// Property value with support for multi-source fragmented data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Date(NaiveDate),
    List(Vec<Value>),
    Map(PropertyMap),
    MultiValue(Vec<ValueWithSource>),
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ValueWithSource {
    pub value: Box<Value>,
    pub source: String,
    pub confidence: Option<f64>,
}

pub type PropertyMap = HashMap<String, Value>;

impl std::fmt::Display for EntityLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityLabel::Company => write!(f, "Company"),
            EntityLabel::Person => write!(f, "Person"),
            EntityLabel::Trust => write!(f, "Trust"),
            EntityLabel::Address => write!(f, "Address"),
            EntityLabel::Document => write!(f, "Document"),
            EntityLabel::Officer => write!(f, "Officer"),
        }
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::HasOwnership => write!(f, "HAS_OWNERSHIP"),
            EdgeType::HasControl => write!(f, "HAS_CONTROL"),
            EdgeType::IsDirectorOf => write!(f, "IS_DIRECTOR_OF"),
            EdgeType::IsSecretaryOf => write!(f, "IS_SECRETARY_OF"),
            EdgeType::HasShareholder => write!(f, "HAS_SHAREHOLDER"),
            EdgeType::ResidesAt => write!(f, "RESIDES_AT"),
            EdgeType::HasRegisteredOffice => write!(f, "HAS_REGISTERED_OFFICE"),
            EdgeType::EvidencedBy => write!(f, "EVIDENCED_BY"),
        }
    }
}
