//! Runtime Environment for the DSL Forth Engine.
//!
//! Provides database-backed storage for attributes and documents during DSL execution.

use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use std::collections::HashMap;

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

    /// In-memory cache for attributes during execution
    pub attribute_cache: HashMap<AttributeId, Value>,

    /// In-memory cache for documents during execution
    pub document_cache: HashMap<DocumentId, DocumentMeta>,

    /// Extracted case_id from DSL execution
    pub case_id: Option<String>,
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
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
        }
    }

    /// Create a new RuntimeEnv with database connection
    #[cfg(feature = "database")]
    pub fn with_pool(request_id: OnboardingRequestId, pool: PgPool) -> Self {
        Self {
            request_id,
            pool: Some(pool),
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
        }
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
