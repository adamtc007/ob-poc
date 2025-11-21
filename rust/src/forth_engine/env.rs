//! Runtime Environment for the DSL Forth Engine.
//!
//! Provides database-backed storage for attributes and documents during DSL execution.

use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnboardingRequestId(pub String);

/// The RuntimeEnv holds state for a single DSL execution session.
/// It provides access to the database for reading/writing attributes and documents.
pub struct RuntimeEnv {
    /// The case/request ID for this execution
    pub request_id: OnboardingRequestId,

    /// Database connection pool (when database feature enabled)
    #[cfg(feature = "database")]
    pub pool: Option<PgPool>,

    /// Current CBU ID for this execution context
    pub cbu_id: Option<Uuid>,

    /// Current entity ID for this execution context
    pub entity_id: Option<Uuid>,

    /// In-memory cache for attributes during execution
    pub attribute_cache: HashMap<AttributeId, Value>,

    /// In-memory cache for documents during execution
    pub document_cache: HashMap<DocumentId, DocumentMeta>,

    /// Extracted case_id from DSL execution
    pub case_id: Option<String>,

    /// Sink attributes - attributes that should be populated for this context
    pub sink_attributes: HashSet<Uuid>,

    /// Source attributes - attributes that produce data in this context
    pub source_attributes: HashSet<Uuid>,
}

/// Document metadata
#[derive(Debug, Clone)]
pub struct DocumentMeta {
    pub id: DocumentId,
    pub name: String,
    pub doc_type: String,
    pub location: Option<String>,
}

impl RuntimeEnv {
    /// Create a new RuntimeEnv without database connection
    pub fn new(request_id: OnboardingRequestId) -> Self {
        Self {
            request_id,
            #[cfg(feature = "database")]
            pool: None,
            cbu_id: None,
            entity_id: None,
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
            sink_attributes: HashSet::new(),
            source_attributes: HashSet::new(),
        }
    }

    /// Create a new RuntimeEnv with database connection
    #[cfg(feature = "database")]
    pub fn with_pool(request_id: OnboardingRequestId, pool: PgPool) -> Self {
        Self {
            request_id,
            pool: Some(pool),
            cbu_id: None,
            entity_id: None,
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
            sink_attributes: HashSet::new(),
            source_attributes: HashSet::new(),
        }
    }

    /// Set the CBU ID for this execution context
    pub fn set_cbu_id(&mut self, id: Uuid) {
        self.cbu_id = Some(id);
    }

    /// Get the CBU ID, returning error if not set
    pub fn ensure_cbu_id(&self) -> Result<Uuid, &'static str> {
        self.cbu_id.ok_or("CBU ID not set in runtime environment")
    }

    /// Set the entity ID for this execution context
    pub fn set_entity_id(&mut self, id: Uuid) {
        self.entity_id = Some(id);
    }

    /// Get the entity ID, returning error if not set
    pub fn ensure_entity_id(&self) -> Result<Uuid, &'static str> {
        self.entity_id
            .ok_or("Entity ID not set in runtime environment")
    }

    /// Check if an attribute is a sink for this context
    pub fn is_sink(&self, attr_id: &Uuid) -> bool {
        self.sink_attributes.contains(attr_id)
    }

    /// Check if an attribute is a source for this context
    pub fn is_source(&self, attr_id: &Uuid) -> bool {
        self.source_attributes.contains(attr_id)
    }

    /// Add a sink attribute
    pub fn add_sink_attribute(&mut self, attr_id: Uuid) {
        self.sink_attributes.insert(attr_id);
    }

    /// Add a source attribute
    pub fn add_source_attribute(&mut self, attr_id: Uuid) {
        self.source_attributes.insert(attr_id);
    }

    /// Set sink attributes from a list
    pub fn set_sink_attributes(&mut self, attrs: Vec<Uuid>) {
        self.sink_attributes = attrs.into_iter().collect();
    }

    /// Set source attributes from a list
    pub fn set_source_attributes(&mut self, attrs: Vec<Uuid>) {
        self.source_attributes = attrs.into_iter().collect();
    }

    /// Check if database is available
    #[cfg(feature = "database")]
    pub fn has_database(&self) -> bool {
        self.pool.is_some()
    }

    #[cfg(not(feature = "database"))]
    pub fn has_database(&self) -> bool {
        false
    }

    /// Get attribute from cache (sync - for VM execution)
    pub fn get_attribute(&self, id: &AttributeId) -> Option<&Value> {
        self.attribute_cache.get(id)
    }

    /// Set attribute in cache (will be persisted at end of execution)
    pub fn set_attribute(&mut self, id: AttributeId, value: Value) {
        self.attribute_cache.insert(id, value);
    }

    /// Set the case_id extracted during execution
    pub fn set_case_id(&mut self, case_id: String) {
        self.case_id = Some(case_id);
    }

    /// Get the case_id
    pub fn get_case_id(&self) -> Option<&String> {
        self.case_id.as_ref()
    }

    /// Load attribute from database into cache
    #[cfg(feature = "database")]
    pub async fn load_attribute(&mut self, id: &AttributeId) -> Result<Option<Value>, sqlx::Error> {
        if let Some(pool) = &self.pool {
            let case_id = self.case_id.as_deref().unwrap_or("");

            let row = sqlx::query_as::<_, (String,)>(
                r#"
                SELECT attribute_value
                FROM "ob-poc".attribute_values
                WHERE attribute_id = $1::uuid AND entity_id = $2
                "#,
            )
            .bind(&id.0)
            .bind(case_id)
            .fetch_optional(pool)
            .await?;

            if let Some((value_text,)) = row {
                let value = Value::Str(value_text);
                self.attribute_cache.insert(id.clone(), value.clone());
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    // Note: DB operations for DSL/AST persistence, CBU creation, and attribute saving
    // have been moved to the central database facade (DslRepository).
    // RuntimeEnv now only handles in-memory caching and attribute loading during execution.
    // See crate::database::DslRepository for transactional DSL/AST saves.
}

/// Generate a new OB Request ID
pub fn mint_ob_request_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    format!("OB-{}", &uuid.to_string()[..8].to_uppercase())
}

/// Generate DSL onboarding template with minted ID
pub fn generate_onboarding_template(
    ob_request_id: &str,
    client_name: &str,
    client_type: &str,
) -> String {
    format!(
        r#"(case.create :case-id "{}" :case-type "ONBOARDING" :client-name "{}" :client-type "{}")"#,
        ob_request_id, client_name, client_type
    )
}
