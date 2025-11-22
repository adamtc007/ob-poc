//! CBU CRUD Template Service
//!
//! Generates CRUD templates from CBU Model specifications and manages
//! template instantiation for specific CBU operations.

use crate::cbu_model_dsl::ast::{CbuModel, CbuTransition};
use crate::database::{DocumentService, DslRepository};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

// Note: All SQL operations have been moved to DocumentService.
// This module uses DocumentService for document_catalog operations
// and DslRepository for dsl_instances operations.

/// A CBU CRUD Template - parametrized recipe for CBU operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuCrudTemplate {
    /// Unique identifier for this template
    pub id: String,
    /// Reference to the CBU Model this template was generated from
    pub model_id: String,
    /// The transition verb this template implements (e.g., "cbu.submit")
    pub transition_verb: String,
    /// The chunks required for this transition
    pub chunks: Vec<String>,
    /// DSL content with placeholders (e.g., "{{CBU.LEGAL_NAME}}")
    pub content: String,
}

/// Result of template generation
#[derive(Debug)]
pub struct TemplateGenerationResult {
    /// Generated templates
    pub templates: Vec<CbuCrudTemplate>,
    /// DSL instance IDs for saved templates
    pub instance_ids: Vec<Uuid>,
    /// Document IDs for saved templates
    pub document_ids: Vec<Uuid>,
}

/// Source information for DSL CRUD documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslDocSource {
    /// DSL instance ID
    pub dsl_instance_id: Uuid,
    /// Document catalog ID
    pub document_id: Uuid,
    /// CBU Model ID
    pub model_id: String,
    /// Template ID (if instantiated from template)
    pub template_id: Option<String>,
    /// DSL version
    pub dsl_version: i32,
}

/// Service for generating and managing CBU CRUD templates
pub struct CbuCrudTemplateService {
    pool: PgPool,
    document_service: Arc<DocumentService>,
}

impl CbuCrudTemplateService {
    /// Create a new template service
    pub fn new(pool: PgPool) -> Self {
        let document_service = Arc::new(DocumentService::new(pool.clone()));
        Self {
            pool,
            document_service,
        }
    }

    /// Generate all templates for a CBU Model
    ///
    /// Creates one template per transition that has chunks defined.
    pub fn generate_templates(&self, model: &CbuModel) -> Vec<CbuCrudTemplate> {
        let mut templates = Vec::new();

        for transition in &model.states.transitions {
            // Only generate templates for transitions with chunks
            if !transition.chunks.is_empty() {
                let template = self.generate_template_for_transition(model, transition);
                templates.push(template);
            }
        }

        templates
    }

    /// Generate a single template for a transition
    fn generate_template_for_transition(
        &self,
        model: &CbuModel,
        transition: &CbuTransition,
    ) -> CbuCrudTemplate {
        let template_id = format!("{}.{}", model.id, transition.verb.replace('.', "_"));

        // Collect all attributes from the required chunks
        let mut all_attrs: Vec<(&str, bool)> = Vec::new(); // (attr_id, is_required)

        for chunk_name in &transition.chunks {
            if let Some(chunk) = model.get_chunk(chunk_name) {
                for attr in &chunk.required {
                    all_attrs.push((attr, true));
                }
                for attr in &chunk.optional {
                    all_attrs.push((attr, false));
                }
            }
        }

        // Generate DSL content with placeholders
        let content = self.generate_dsl_content(&transition.verb, &all_attrs);

        CbuCrudTemplate {
            id: template_id,
            model_id: model.id.clone(),
            transition_verb: transition.verb.clone(),
            chunks: transition.chunks.clone(),
            content,
        }
    }

    /// Generate DSL content with attribute placeholders
    fn generate_dsl_content(&self, verb: &str, attrs: &[(&str, bool)]) -> String {
        let mut lines = Vec::new();

        // Determine the DSL verb to use
        let dsl_verb = if verb.starts_with("cbu.") {
            verb.to_string()
        } else {
            format!("cbu.{}", verb)
        };

        lines.push(format!("({}", dsl_verb));

        for (attr_id, _is_required) in attrs {
            let keyword = map_attr_to_dsl_keyword(attr_id);
            let placeholder = format!("{{{{{}}}}}", attr_id);
            // Note: Comments removed as DSL parser doesn't support them
            lines.push(format!("  {} \"{}\"", keyword, placeholder));
        }

        lines.push(")".to_string());
        lines.join("\n")
    }

    /// Save generated templates to database
    pub async fn save_templates(
        &self,
        templates: &[CbuCrudTemplate],
        model: &CbuModel,
    ) -> Result<TemplateGenerationResult> {
        let repo = DslRepository::new(self.pool.clone());
        let _doc_service = DocumentService::new(self.pool.clone());

        let mut instance_ids = Vec::new();
        let mut document_ids = Vec::new();

        // Ensure document type exists
        self.ensure_template_document_type().await?;

        for template in templates {
            // Save DSL instance
            let result = repo
                .save_dsl_instance(
                    &template.id,
                    "CBU-CRUD-TEMPLATE",
                    &template.content,
                    Some(&serde_json::to_value(template)?),
                    &template.transition_verb,
                )
                .await
                .map_err(|e| anyhow!("Failed to save template DSL: {}", e))?;

            instance_ids.push(result.instance_id);

            // Create document catalog entry
            let doc_id = Uuid::new_v4();
            self.document_service
                .create_document_with_metadata(
                    doc_id,
                    "DSL.CRUD.CBU.TEMPLATE",
                    &format!("{} v{}", template.id, model.version),
                    json!({
                        "template_id": template.id,
                        "model_id": model.id,
                        "model_version": model.version,
                        "transition_verb": template.transition_verb,
                        "chunks": template.chunks,
                        "dsl_instance_id": result.instance_id.to_string(),
                    }),
                )
                .await?;

            document_ids.push(doc_id);

            info!(
                "Saved template {} for verb {} (instance: {}, doc: {})",
                template.id, template.transition_verb, result.instance_id, doc_id
            );
        }

        Ok(TemplateGenerationResult {
            templates: templates.to_vec(),
            instance_ids,
            document_ids,
        })
    }

    /// Ensure the template document type exists
    async fn ensure_template_document_type(&self) -> Result<()> {
        self.document_service
            .ensure_document_type(
                "DSL.CRUD.CBU.TEMPLATE",
                "CBU CRUD Template",
                "DSL",
                "Parametrized CBU CRUD recipe",
            )
            .await?;
        Ok(())
    }

    /// Instantiate a CRUD sheet from a template
    ///
    /// Replaces placeholders with provided values and saves as DSL.CRUD.CBU
    pub async fn instantiate_crud_from_template(
        &self,
        template_doc_id: Uuid,
        initial_values: HashMap<String, String>,
    ) -> Result<(Uuid, Uuid)> {
        // Load template from document catalog
        let template_row = self
            .document_service
            .get_document_catalog_entry(template_doc_id)
            .await?
            .ok_or_else(|| anyhow!("Template document {} not found", template_doc_id))?;

        let metadata = template_row.1;
        let dsl_instance_id_str = metadata
            .get("dsl_instance_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing dsl_instance_id in template metadata"))?;
        let dsl_instance_id = Uuid::parse_str(dsl_instance_id_str)?;

        // Load template DSL content
        let repo = DslRepository::new(self.pool.clone());
        let template_content = repo
            .get_dsl_content(dsl_instance_id)
            .await?
            .ok_or_else(|| anyhow!("Template DSL content not found"))?;

        // Replace placeholders with values
        let mut crud_content = template_content;
        for (attr_id, value) in &initial_values {
            let placeholder = format!("{{{{{}}}}}", attr_id);
            crud_content = crud_content.replace(&placeholder, value);
        }

        // Generate CRUD instance ID
        let crud_id = format!("CRUD-{}", Uuid::new_v4().to_string()[..8].to_uppercase());

        // Ensure CRUD document type exists
        self.ensure_crud_document_type().await?;

        // Save CRUD instance
        let result = repo
            .save_dsl_instance(&crud_id, "CBU-CRUD", &crud_content, None, "instantiate")
            .await
            .map_err(|e| anyhow!("Failed to save CRUD instance: {}", e))?;

        // Create document catalog entry
        let doc_id = Uuid::new_v4();
        let model_id = metadata
            .get("model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let template_id = metadata
            .get("template_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        self.document_service
            .create_document_with_metadata(
                doc_id,
                "DSL.CRUD.CBU",
                &crud_id,
                json!({
                    "crud_id": crud_id,
                    "model_id": model_id,
                    "template_id": template_id,
                    "template_doc_id": template_doc_id.to_string(),
                    "dsl_instance_id": result.instance_id.to_string(),
                    "values_provided": initial_values.keys().collect::<Vec<_>>(),
                }),
            )
            .await?;

        info!(
            "Instantiated CRUD {} from template {} (instance: {}, doc: {})",
            crud_id, template_id, result.instance_id, doc_id
        );

        Ok((result.instance_id, doc_id))
    }

    /// Ensure the CRUD document type exists
    async fn ensure_crud_document_type(&self) -> Result<()> {
        self.document_service
            .ensure_document_type(
                "DSL.CRUD.CBU",
                "CBU CRUD Sheet",
                "DSL",
                "Concrete CBU CRUD execution document",
            )
            .await?;
        Ok(())
    }

    /// Load a template by its ID
    pub async fn load_template(&self, template_id: &str) -> Result<Option<CbuCrudTemplate>> {
        let row = self
            .document_service
            .find_template_by_id(template_id)
            .await?;

        match row {
            Some((_doc_id, metadata)) => {
                let dsl_instance_id_str = metadata
                    .get("dsl_instance_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing dsl_instance_id"))?;
                let dsl_instance_id = Uuid::parse_str(dsl_instance_id_str)?;

                let repo = DslRepository::new(self.pool.clone());
                let content = repo
                    .get_dsl_content(dsl_instance_id)
                    .await?
                    .ok_or_else(|| anyhow!("Template content not found"))?;

                Ok(Some(CbuCrudTemplate {
                    id: template_id.to_string(),
                    model_id: metadata
                        .get("model_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    transition_verb: metadata
                        .get("transition_verb")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    chunks: metadata
                        .get("chunks")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    content,
                }))
            }
            None => Ok(None),
        }
    }
}

/// Map attribute ID to DSL keyword
///
/// Converts dictionary attribute names to DSL-friendly keywords.
fn map_attr_to_dsl_keyword(attr_id: &str) -> String {
    match attr_id {
        "CBU.LEGAL_NAME" => ":cbu-name".into(),
        "CBU.JURISDICTION" | "CBU.LEGAL_JURISDICTION" => ":jurisdiction".into(),
        "CBU.NATURE_PURPOSE" => ":nature-purpose".into(),
        "CBU.ENTITY_TYPE" => ":entity-type".into(),
        "CBU.REGISTERED_ADDRESS" => ":registered-address".into(),
        "CBU.PRIMARY_CONTACT_EMAIL" => ":primary-contact-email".into(),
        "CBU.PRIMARY_CONTACT_NAME" => ":primary-contact-name".into(),
        "CBU.PRIMARY_CONTACT_PHONE" => ":primary-contact-phone".into(),
        "CBU.TRADING_NAME" => ":trading-name".into(),
        "CBU.LEI" => ":lei".into(),
        "UBO.BENEFICIAL_OWNER_NAME" => ":beneficial-owner-name".into(),
        "UBO.OWNERSHIP_PERCENTAGE" => ":ownership-percentage".into(),
        "UBO.NATIONALITY" => ":nationality".into(),
        "UBO.TAX_RESIDENCY" => ":tax-residency".into(),
        _ => format!(":{}", attr_id.replace('.', "-").to_lowercase()),
    }
}

/// Map DSL keyword back to attribute ID (inverse of map_attr_to_dsl_keyword)
#[allow(dead_code)]
pub fn map_dsl_keyword_to_attr(keyword: &str) -> String {
    let kw = keyword.trim_start_matches(':');
    match kw {
        "cbu-name" => "CBU.LEGAL_NAME".into(),
        "jurisdiction" => "CBU.JURISDICTION".into(),
        "nature-purpose" => "CBU.NATURE_PURPOSE".into(),
        "entity-type" => "CBU.ENTITY_TYPE".into(),
        "registered-address" => "CBU.REGISTERED_ADDRESS".into(),
        "primary-contact-email" => "CBU.PRIMARY_CONTACT_EMAIL".into(),
        "primary-contact-name" => "CBU.PRIMARY_CONTACT_NAME".into(),
        "primary-contact-phone" => "CBU.PRIMARY_CONTACT_PHONE".into(),
        "trading-name" => "CBU.TRADING_NAME".into(),
        "lei" => "CBU.LEI".into(),
        "beneficial-owner-name" => "UBO.BENEFICIAL_OWNER_NAME".into(),
        "ownership-percentage" => "UBO.OWNERSHIP_PERCENTAGE".into(),
        "nationality" => "UBO.NATIONALITY".into(),
        "tax-residency" => "UBO.TAX_RESIDENCY".into(),
        _ => kw.replace('-', ".").to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cbu_model_dsl::CbuModelParser;

    #[test]
    fn test_map_attr_to_dsl_keyword() {
        assert_eq!(map_attr_to_dsl_keyword("CBU.LEGAL_NAME"), ":cbu-name");
        assert_eq!(map_attr_to_dsl_keyword("CBU.JURISDICTION"), ":jurisdiction");
        assert_eq!(map_attr_to_dsl_keyword("UNKNOWN.ATTR"), ":unknown-attr");
    }

    #[test]
    fn test_map_dsl_keyword_to_attr() {
        assert_eq!(map_dsl_keyword_to_attr(":cbu-name"), "CBU.LEGAL_NAME");
        assert_eq!(map_dsl_keyword_to_attr(":jurisdiction"), "CBU.JURISDICTION");
        assert_eq!(map_dsl_keyword_to_attr(":unknown-attr"), "UNKNOWN.ATTR");
    }

    #[test]
    fn test_generate_templates() {
        let model_dsl = r#"
        (cbu-model
          :id "CBU.TEST"
          :version "1.0"

          (attributes
            (group :name "core"
              :required [@attr("CBU.LEGAL_NAME"), @attr("CBU.JURISDICTION")]
              :optional [@attr("CBU.LEI")])
            (group :name "contact"
              :required [@attr("CBU.PRIMARY_CONTACT_EMAIL")]))

          (states
            :initial "Proposed"
            :final ["Active"]
            (state "Proposed" :description "Initial")
            (state "Active" :description "Active"))

          (transitions
            (-> "Proposed" "Active" :verb "cbu.submit" :chunks ["core", "contact"] :preconditions []))

          (roles
            (role "Owner" :min 1)))
        "#;

        let model = CbuModelParser::parse_str(model_dsl).unwrap();

        // Use standalone function to test template generation without requiring pool
        let templates = generate_templates_for_model(&model);
        assert_eq!(templates.len(), 1);

        let template = &templates[0];
        assert_eq!(template.id, "CBU.TEST.cbu_submit");
        assert_eq!(template.model_id, "CBU.TEST");
        assert_eq!(template.transition_verb, "cbu.submit");
        assert_eq!(template.chunks, vec!["core", "contact"]);

        // Check content contains placeholders
        assert!(template.content.contains("{{CBU.LEGAL_NAME}}"));
        assert!(template.content.contains("{{CBU.JURISDICTION}}"));
        assert!(template.content.contains("{{CBU.PRIMARY_CONTACT_EMAIL}}"));
        assert!(template.content.contains(":cbu-name"));
        assert!(template.content.contains(":jurisdiction"));
    }
}

/// Standalone function for generating templates (testable without pool)
#[allow(dead_code)]
fn generate_templates_for_model(model: &CbuModel) -> Vec<CbuCrudTemplate> {
    let mut templates = Vec::new();

    for transition in &model.states.transitions {
        if !transition.chunks.is_empty() {
            let template_id = format!("{}.{}", model.id, transition.verb.replace('.', "_"));

            let mut all_attrs: Vec<(&str, bool)> = Vec::new();

            for chunk_name in &transition.chunks {
                if let Some(chunk) = model.get_chunk(chunk_name) {
                    for attr in &chunk.required {
                        all_attrs.push((attr, true));
                    }
                    for attr in &chunk.optional {
                        all_attrs.push((attr, false));
                    }
                }
            }

            let mut lines = Vec::new();
            let dsl_verb = if transition.verb.starts_with("cbu.") {
                transition.verb.clone()
            } else {
                format!("cbu.{}", transition.verb)
            };

            lines.push(format!("({}", dsl_verb));
            for (attr_id, _is_required) in &all_attrs {
                let keyword = map_attr_to_dsl_keyword(attr_id);
                let placeholder = format!("{{{{{}}}}}", attr_id);
                lines.push(format!("  {} \"{}\"", keyword, placeholder));
            }
            lines.push(")".to_string());

            templates.push(CbuCrudTemplate {
                id: template_id,
                model_id: model.id.clone(),
                transition_verb: transition.verb.clone(),
                chunks: transition.chunks.clone(),
                content: lines.join("\n"),
            });
        }
    }

    templates
}
