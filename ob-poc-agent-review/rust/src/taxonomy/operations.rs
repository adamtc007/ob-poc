//! DSL Operations for Taxonomy System
//!
//! Defines the operations that can be performed on the taxonomy system
//! and the results they produce.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// DSL operations for taxonomy management
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum DslOperation {
    CreateOnboarding {
        cbu_id: Uuid,
        initiated_by: String,
    },
    AddProducts {
        request_id: Uuid,
        product_codes: Vec<String>,
    },
    DiscoverServices {
        request_id: Uuid,
        product_id: Uuid,
    },
    ConfigureService {
        request_id: Uuid,
        service_code: String,
        options: HashMap<String, serde_json::Value>,
    },
    AllocateResources {
        request_id: Uuid,
        service_id: Uuid,
    },
    FinalizeOnboarding {
        request_id: Uuid,
    },
    GetStatus {
        request_id: Uuid,
    },
}

/// Result of a DSL operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub next_operations: Vec<String>,
    pub dsl_fragment: Option<String>,
    pub current_state: Option<String>,
}

impl DslResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
            next_operations: vec![],
            dsl_fragment: None,
            current_state: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            next_operations: vec![],
            dsl_fragment: None,
            current_state: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn with_next_operations(mut self, ops: Vec<String>) -> Self {
        self.next_operations = ops;
        self
    }

    pub fn with_dsl_fragment(mut self, dsl: String) -> Self {
        self.dsl_fragment = Some(dsl);
        self
    }

    pub fn with_state(mut self, state: String) -> Self {
        self.current_state = Some(state);
        self
    }
}
