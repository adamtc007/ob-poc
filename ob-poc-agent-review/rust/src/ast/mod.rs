//! Abstract Syntax Tree (AST) structures for the UBO/KYC DSL
//!
//! This module defines core type definitions used across the DSL system.
//! For the primary V3.1 AST implementation, see `parser::ast` module.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod types;

/// Property value - basic type definitions for AST types module
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

/// Value with source metadata for multi-source data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueWithSource {
    pub value: Box<Value>,
    pub source: String,
    pub confidence: Option<f64>,
}

/// Property map type alias
pub type PropertyMap = HashMap<String, Value>;

/// Entity labels for graph node classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityLabel {
    Company,
    Person,
    Trust,
    Address,
    Document,
    Officer,
}

/// Edge types for graph relationships
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
