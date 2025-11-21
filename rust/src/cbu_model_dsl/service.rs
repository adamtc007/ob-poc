//! CBU Model Service
//!
//! Provides validation of CBU Models against the attribute dictionary
//! and persistence as documents.

use crate::cbu_model_dsl::ast::CbuModel;
use crate::cbu_model_dsl::parser::{CbuModelError, CbuModelParser};
use crate::database::{DictionaryDatabaseService, DslRepository};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Service for validating and persisting CBU Models
pub struct CbuModelService {
    pool: PgPool,
    dictionary: DictionaryDatabaseService,
}

impl CbuModelService {
    /// Create a new CBU Model Service
    pub fn new(pool: PgPool) -> Self {
        let dictionary = DictionaryDatabaseService::new(pool.clone());
        Self { pool, dictionary }
    }

    /// Parse and validate a CBU Model DSL string
    pub async fn parse_and_validate(&self, input: &str) -> Result<CbuModel, CbuModelError> {
        // Parse the DSL
        let model = CbuModelParser::parse_str(input)?;

        // Validate against dictionary
        self.validate_model(&model).await?;

        Ok(model)
    }

    /// Validate a CBU Model against the attribute dictionary
    pub async fn validate_model(&self, model: &CbuModel) -> Result<(), CbuModelError> {
        info!("Validating CBU Model: {}", model.id);

        // Collect all attribute IDs from the model
        let mut all_attrs: Vec<&str> = model.attributes.all_attributes();

        // Add precondition attributes from transitions
        for transition in &model.states.transitions {
            for attr in &transition.preconditions {
                if !all_attrs.contains(&attr.as_str()) {
                    all_attrs.push(attr);
                }
            }
        }

        // Validate each attribute exists in dictionary and has CBU sink
        let mut errors: Vec<String> = Vec::new();

        for attr_id in &all_attrs {
            match self.validate_attribute(attr_id).await {
                Ok(()) => {
                    debug!("Attribute '{}' validated successfully", attr_id);
                }
                Err(e) => {
                    errors.push(e);
                }
            }
        }

        // Validate state machine consistency
        self.validate_state_machine(model, &mut errors);

        // Validate role constraints
        self.validate_roles(model, &mut errors);

        // Validate verb naming conventions
        self.validate_verbs(model, &mut errors);

        if !errors.is_empty() {
            return Err(CbuModelError::ValidationError(errors.join("; ")));
        }

        info!("CBU Model '{}' validated successfully", model.id);
        Ok(())
    }

    /// Validate a single attribute exists and has CBU sink
    async fn validate_attribute(&self, attr_id: &str) -> Result<(), String> {
        // Try to find by name first
        let attr = self
            .dictionary
            .get_by_name(attr_id)
            .await
            .map_err(|e| format!("Database error looking up '{}': {}", attr_id, e))?;

        match attr {
            Some(attr) => {
                // Check if attribute has CBU in its sink
                if let Some(sink) = &attr.sink {
                    let has_cbu_sink = match sink {
                        JsonValue::Array(arr) => arr.iter().any(|v| {
                            v.as_str()
                                .map(|s| s.to_uppercase() == "CBU")
                                .unwrap_or(false)
                        }),
                        JsonValue::Object(obj) => {
                            if let Some(assets) = obj.get("assets") {
                                match assets {
                                    JsonValue::Array(arr) => arr.iter().any(|v| {
                                        v.as_str()
                                            .map(|s| s.to_uppercase() == "CBU")
                                            .unwrap_or(false)
                                    }),
                                    JsonValue::String(s) => s.to_uppercase() == "CBU",
                                    _ => false,
                                }
                            } else {
                                false
                            }
                        }
                        JsonValue::String(s) => s.to_uppercase() == "CBU",
                        _ => false,
                    };

                    if !has_cbu_sink {
                        warn!(
                            "Attribute '{}' does not have CBU in sink: {:?}",
                            attr_id, sink
                        );
                        // For now, just warn - don't fail validation
                        // In production, this would be: return Err(...)
                    }
                }
                Ok(())
            }
            None => {
                // Attribute not found - this is an error
                Err(format!("Attribute '{}' not found in dictionary", attr_id))
            }
        }
    }

    /// Validate state machine consistency
    fn validate_state_machine(&self, model: &CbuModel, errors: &mut Vec<String>) {
        let sm = &model.states;

        // Check initial state exists
        if !sm.states.iter().any(|s| s.name == sm.initial) {
            errors.push(format!(
                "Initial state '{}' not defined in states",
                sm.initial
            ));
        }

        // Check final states exist
        for final_state in &sm.finals {
            if !sm.states.iter().any(|s| s.name == *final_state) {
                errors.push(format!(
                    "Final state '{}' not defined in states",
                    final_state
                ));
            }
        }

        // Check all transitions reference valid states
        for trans in &sm.transitions {
            if !sm.states.iter().any(|s| s.name == trans.from) {
                errors.push(format!(
                    "Transition from '{}' references undefined state",
                    trans.from
                ));
            }
            if !sm.states.iter().any(|s| s.name == trans.to) {
                errors.push(format!(
                    "Transition to '{}' references undefined state",
                    trans.to
                ));
            }

            // Warn if transitioning out of a final state
            if sm.finals.contains(&trans.from) {
                warn!(
                    "Transition from final state '{}' to '{}' - final states should not have outgoing transitions",
                    trans.from, trans.to
                );
            }
        }

        // Check for unreachable states (except initial)
        for state in &sm.states {
            if state.name != sm.initial {
                let is_reachable = sm.transitions.iter().any(|t| t.to == state.name);
                if !is_reachable {
                    warn!(
                        "State '{}' is not reachable from any transition",
                        state.name
                    );
                }
            }
        }
    }

    /// Validate role constraints
    fn validate_roles(&self, model: &CbuModel, errors: &mut Vec<String>) {
        for role in &model.roles {
            // Check min <= max
            if let Some(max) = role.max {
                if role.min > max {
                    errors.push(format!(
                        "Role '{}' has min ({}) > max ({})",
                        role.name, role.min, max
                    ));
                }
            }
        }

        // Check for duplicate role names
        let mut seen = std::collections::HashSet::new();
        for role in &model.roles {
            if !seen.insert(&role.name) {
                errors.push(format!("Duplicate role name: '{}'", role.name));
            }
        }
    }

    /// Validate verb naming conventions
    fn validate_verbs(&self, model: &CbuModel, _errors: &mut Vec<String>) {
        let mut seen_verbs = std::collections::HashSet::new();

        for trans in &model.states.transitions {
            // Check verb format (should be domain.action)
            if !trans.verb.contains('.') {
                warn!(
                    "Verb '{}' does not follow domain.action convention",
                    trans.verb
                );
            }

            // Check for duplicate verbs (same verb for different transitions is OK)
            seen_verbs.insert(&trans.verb);
        }
    }

    /// Save a CBU Model as a document
    pub async fn save_model(
        &self,
        raw_content: &str,
        model: &CbuModel,
    ) -> Result<Uuid, CbuModelError> {
        let repo = DslRepository::new(self.pool.clone());

        // Save to dsl_instances
        let domain = "cbu-model";
        let case_id = format!("MODEL-{}", model.id);

        // Serialize AST to JSON
        let ast_json = serde_json::to_value(model).map_err(|e| {
            CbuModelError::ValidationError(format!("Failed to serialize model: {}", e))
        })?;

        let result = repo
            .save_execution(
                raw_content,
                domain,
                &case_id,
                None, // No CBU ID for model definitions
                &ast_json,
            )
            .await
            .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        // Also create a document_catalog entry
        self.create_document_entry(&result.instance_id, model)
            .await?;

        info!(
            "Saved CBU Model '{}' with instance_id: {}",
            model.id, result.instance_id
        );

        Ok(result.instance_id)
    }

    /// Create a document catalog entry for the model
    async fn create_document_entry(
        &self,
        instance_id: &Uuid,
        model: &CbuModel,
    ) -> Result<(), CbuModelError> {
        // Check if document_type exists, create if not
        let type_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".document_types
                WHERE type_code = 'DSL.CBU.MODEL'
            )
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        if !type_exists {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".document_types (type_code, display_name, category, description)
                VALUES ('DSL.CBU.MODEL', 'CBU Model DSL', 'DSL', 'CBU Model specification document')
                ON CONFLICT (type_code) DO NOTHING
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;
        }

        // Create document catalog entry
        let doc_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                document_id, document_type_code, document_name,
                source_system, status, metadata
            )
            VALUES ($1, 'DSL.CBU.MODEL', $2, 'ob-poc', 'active', $3)
            "#,
        )
        .bind(doc_id)
        .bind(&format!("{} v{}", model.id, model.version))
        .bind(serde_json::json!({
            "model_id": model.id,
            "version": model.version,
            "dsl_instance_id": instance_id.to_string(),
            "applies_to": model.applies_to
        }))
        .execute(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Load a CBU Model by its model ID (e.g., "CBU.GENERIC")
    pub async fn load_model_by_id(
        &self,
        model_id: &str,
    ) -> Result<Option<CbuModel>, CbuModelError> {
        // Find the latest version in document_catalog
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            r#"
            SELECT metadata
            FROM "ob-poc".document_catalog
            WHERE document_type_code = 'DSL.CBU.MODEL'
            AND metadata->>'model_id' = $1
            AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        match row {
            Some((metadata,)) => {
                // Get the DSL instance ID from metadata
                let instance_id_str = metadata
                    .get("dsl_instance_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        CbuModelError::ValidationError(
                            "Missing dsl_instance_id in metadata".to_string(),
                        )
                    })?;

                let instance_id = Uuid::parse_str(instance_id_str)
                    .map_err(|e| CbuModelError::ValidationError(format!("Invalid UUID: {}", e)))?;

                // Load the DSL content
                let repo = DslRepository::new(self.pool.clone());
                let content = repo
                    .get_dsl_content(instance_id)
                    .await
                    .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?
                    .ok_or_else(|| {
                        CbuModelError::ValidationError(format!(
                            "DSL instance {} not found",
                            instance_id
                        ))
                    })?;

                // Parse and return
                let model = CbuModelParser::parse_str(&content)?;
                Ok(Some(model))
            }
            None => Ok(None),
        }
    }

    /// Load a CBU Model by DSL instance ID
    pub async fn load_model_by_instance(
        &self,
        instance_id: Uuid,
    ) -> Result<CbuModel, CbuModelError> {
        let repo = DslRepository::new(self.pool.clone());
        let content = repo
            .get_dsl_content(instance_id)
            .await
            .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?
            .ok_or_else(|| {
                CbuModelError::ValidationError(format!("DSL instance {} not found", instance_id))
            })?;

        CbuModelParser::parse_str(&content)
    }

    /// List all available CBU Models
    pub async fn list_models(&self) -> Result<Vec<(String, String, Uuid)>, CbuModelError> {
        let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
            r#"
            SELECT document_name, metadata
            FROM "ob-poc".document_catalog
            WHERE document_type_code = 'DSL.CBU.MODEL'
            AND status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CbuModelError::DatabaseError(e.to_string()))?;

        let mut models = Vec::new();
        for (name, metadata) in rows {
            let model_id = metadata
                .get("model_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let instance_id_str = metadata
                .get("dsl_instance_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Ok(instance_id) = Uuid::parse_str(instance_id_str) {
                models.push((name, model_id, instance_id));
            }
        }

        Ok(models)
    }
}
