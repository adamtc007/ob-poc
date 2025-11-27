//! Slot type definitions for template forms

use crate::services::EntityType;
use serde::{Deserialize, Serialize};

/// Types of form slots
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlotType {
    /// Free text input
    Text {
        #[serde(default)]
        max_length: Option<u32>,
        #[serde(default)]
        multiline: bool,
    },

    /// Date picker (YYYY-MM-DD)
    Date,

    /// Country selector (ISO 2-letter)
    Country,

    /// Currency selector (ISO 3-letter)
    Currency,

    /// Money amount (links to currency slot)
    Money { currency_slot: String },

    /// Percentage (0-100)
    Percentage,

    /// Integer input with optional range
    Integer {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },

    /// Decimal number
    Decimal {
        #[serde(default)]
        precision: Option<u32>,
    },

    /// Boolean toggle
    Boolean,

    /// Dropdown/select from fixed options
    Enum { options: Vec<EnumOption> },

    /// Entity reference with search
    EntityRef {
        /// Which entity types are allowed
        allowed_types: Vec<EntityType>,
        /// Search scope
        #[serde(default)]
        scope: RefScope,
        /// Allow creating new entity inline
        #[serde(default)]
        allow_create: bool,
    },

    /// UUID (auto-generated or manual)
    Uuid {
        #[serde(default)]
        auto_generate: bool,
    },
}

/// Options for enum slot type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Scope for entity reference search
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefScope {
    /// Search all entities in database
    #[default]
    Global,
    /// Search only entities attached to current CBU
    WithinCbu,
    /// Search only entities created in this session
    WithinSession,
}

/// Definition of a single slot in a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDefinition {
    /// Slot identifier (maps to DSL param name)
    pub name: String,

    /// Human-readable label
    pub label: String,

    /// The slot type
    pub slot_type: SlotType,

    /// Is this slot required?
    #[serde(default)]
    pub required: bool,

    /// Default value (JSON)
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,

    /// Help text shown below input
    #[serde(default)]
    pub help_text: Option<String>,

    /// Placeholder text
    #[serde(default)]
    pub placeholder: Option<String>,

    /// DSL param name (if different from slot name)
    #[serde(default)]
    pub dsl_param: Option<String>,
}

impl SlotDefinition {
    /// Get the DSL parameter name for this slot
    pub fn dsl_param_name(&self) -> &str {
        self.dsl_param.as_deref().unwrap_or(&self.name)
    }
}

impl Default for SlotDefinition {
    fn default() -> Self {
        SlotDefinition {
            name: String::new(),
            label: String::new(),
            slot_type: SlotType::Text {
                max_length: None,
                multiline: false,
            },
            required: false,
            default_value: None,
            help_text: None,
            placeholder: None,
            dsl_param: None,
        }
    }
}

/// A complete form template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormTemplate {
    /// Unique template identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of what this template does
    pub description: String,

    /// DSL verb this template generates
    pub verb: String,

    /// DSL domain
    pub domain: String,

    /// Slot definitions
    pub slots: Vec<SlotDefinition>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

impl FormTemplate {
    /// Get a slot by name
    pub fn get_slot(&self, name: &str) -> Option<&SlotDefinition> {
        self.slots.iter().find(|s| s.name == name)
    }

    /// Get all required slots
    pub fn required_slots(&self) -> impl Iterator<Item = &SlotDefinition> {
        self.slots.iter().filter(|s| s.required)
    }

    /// Get all EntityRef slots
    pub fn entity_ref_slots(&self) -> impl Iterator<Item = &SlotDefinition> {
        self.slots
            .iter()
            .filter(|s| matches!(s.slot_type, SlotType::EntityRef { .. }))
    }
}
