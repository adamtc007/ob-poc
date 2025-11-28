//! Semantic Intent Layer for Agent-Driven DSL Generation
//!
//! Pipeline:
//! ```
//! Agent JSON → Intent → Planner → DSL Source → Parser → AST → Compiler → Executor
//!     ↑           ↑         ↑          ↑          ↑        ↑        ↑
//!  Schema     Validated  Deterministic  Text    Syntax  Semantics  Refs
//!  constrained           pure fn                valid   valid      resolved
//! ```
//!
//! Each stage is independently testable.

use serde::{Deserialize, Serialize};

// =============================================================================
// INTENT SCHEMA - What the agent produces (JSON, schema-validated)
// =============================================================================

/// Top-level intent - agent classifies user request into ONE of these
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "intent", rename_all = "snake_case")]
pub enum KycIntent {
    OnboardClient(OnboardClientIntent),
    AddDocument(AddDocumentIntent),
    AddEntityRole(AddEntityRoleIntent),
    LinkEvidence(LinkEvidenceIntent),
    ExtractAttributes(ExtractAttributesIntent),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OnboardClientIntent {
    pub client_name: String,
    pub client_type: ClientType,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub documents: Vec<DocumentSpec>,
    #[serde(default)]
    pub entities: Vec<EntitySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Individual,
    Corporate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentSpec {
    pub document_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_true")]
    pub extract_attributes: bool,
    #[serde(default)]
    pub for_entity_index: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitySpec {
    pub entity_type: EntityType,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub ownership_percentage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    NaturalPerson,
    LimitedCompany,
    Partnership,
    Trust,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddDocumentIntent {
    pub cbu_ref: String,
    pub document: DocumentSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddEntityRoleIntent {
    pub cbu_ref: String,
    pub entity: EntitySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinkEvidenceIntent {
    pub document_ref: String,
    pub entity_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractAttributesIntent {
    pub document_ref: String,
}

// =============================================================================
// INTENT VALIDATION
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct IntentVocabulary {
    pub document_types: Vec<String>,
    pub entity_roles: Vec<String>,
    pub jurisdictions: Vec<String>,
}

impl Default for IntentVocabulary {
    fn default() -> Self {
        Self {
            document_types: vec![
                "PASSPORT".into(),
                "PASSPORT_GBR".into(),
                "PASSPORT_USA".into(),
                "PASSPORT_DEU".into(),
                "PASSPORT_FRA".into(),
                "DRIVERS_LICENSE".into(),
                "DRIVERS_LICENSE_GBR".into(),
                "DRIVERS_LICENSE_USA_CA".into(),
                "DRIVERS_LICENSE_USA_NY".into(),
                "CERT_OF_INCORPORATION".into(),
                "UTILITY_BILL".into(),
                "BANK_STATEMENT".into(),
            ],
            entity_roles: vec![
                "beneficial_owner".into(),
                "director".into(),
                "signatory".into(),
                "shareholder".into(),
                "ubo".into(),
                "authorized_person".into(),
            ],
            jurisdictions: vec![
                "UK".into(),
                "US".into(),
                "DE".into(),
                "FR".into(),
                "CH".into(),
                "IE".into(),
                "CA".into(),
                "AU".into(),
            ],
        }
    }
}

pub fn validate_intent(intent: &KycIntent, vocab: &IntentVocabulary) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match intent {
        KycIntent::OnboardClient(i) => {
            if i.client_name.trim().is_empty() {
                errors.push(ValidationError {
                    field: "client_name".into(),
                    message: "Client name cannot be empty".into(),
                });
            }

            if let Some(ref j) = i.jurisdiction {
                if !vocab.jurisdictions.contains(j) {
                    errors.push(ValidationError {
                        field: "jurisdiction".into(),
                        message: format!(
                            "Unknown jurisdiction: {}. Valid: {:?}",
                            j, vocab.jurisdictions
                        ),
                    });
                }
            }

            for (idx, doc) in i.documents.iter().enumerate() {
                if !vocab.document_types.contains(&doc.document_type) {
                    errors.push(ValidationError {
                        field: format!("documents[{}].document_type", idx),
                        message: format!("Unknown document type: {}", doc.document_type),
                    });
                }
                if let Some(ent_idx) = doc.for_entity_index {
                    if ent_idx >= i.entities.len() {
                        errors.push(ValidationError {
                            field: format!("documents[{}].for_entity_index", idx),
                            message: format!(
                                "Entity index {} out of bounds (max {})",
                                ent_idx,
                                i.entities.len()
                            ),
                        });
                    }
                }
            }

            for (idx, ent) in i.entities.iter().enumerate() {
                if !vocab.entity_roles.contains(&ent.role) {
                    errors.push(ValidationError {
                        field: format!("entities[{}].role", idx),
                        message: format!(
                            "Unknown role: {}. Valid: {:?}",
                            ent.role, vocab.entity_roles
                        ),
                    });
                }
                if let Some(pct) = ent.ownership_percentage {
                    if !(0.0..=100.0).contains(&pct) {
                        errors.push(ValidationError {
                            field: format!("entities[{}].ownership_percentage", idx),
                            message: format!("Ownership must be 0-100, got {}", pct),
                        });
                    }
                }
            }
        }

        KycIntent::AddDocument(i) => {
            if i.cbu_ref.trim().is_empty() {
                errors.push(ValidationError {
                    field: "cbu_ref".into(),
                    message: "CBU reference cannot be empty".into(),
                });
            }
            if !vocab.document_types.contains(&i.document.document_type) {
                errors.push(ValidationError {
                    field: "document.document_type".into(),
                    message: format!("Unknown document type: {}", i.document.document_type),
                });
            }
        }

        KycIntent::AddEntityRole(i) => {
            if i.cbu_ref.trim().is_empty() {
                errors.push(ValidationError {
                    field: "cbu_ref".into(),
                    message: "CBU reference cannot be empty".into(),
                });
            }
            if !vocab.entity_roles.contains(&i.entity.role) {
                errors.push(ValidationError {
                    field: "entity.role".into(),
                    message: format!("Unknown role: {}", i.entity.role),
                });
            }
        }

        KycIntent::LinkEvidence(i) => {
            if i.document_ref.trim().is_empty() {
                errors.push(ValidationError {
                    field: "document_ref".into(),
                    message: "Document reference cannot be empty".into(),
                });
            }
            if i.entity_ref.trim().is_empty() {
                errors.push(ValidationError {
                    field: "entity_ref".into(),
                    message: "Entity reference cannot be empty".into(),
                });
            }
        }

        KycIntent::ExtractAttributes(i) => {
            if i.document_ref.trim().is_empty() {
                errors.push(ValidationError {
                    field: "document_ref".into(),
                    message: "Document reference cannot be empty".into(),
                });
            }
        }
    }

    errors
}

// =============================================================================
// PLANNER - Deterministic Intent → DSL Source transformation
// =============================================================================

struct BindingGenerator {
    cbu_count: u32,
    doc_count: u32,
    ent_count: u32,
}

impl BindingGenerator {
    fn new() -> Self {
        Self {
            cbu_count: 0,
            doc_count: 0,
            ent_count: 0,
        }
    }

    fn next_cbu(&mut self) -> String {
        let b = format!("@cbu{}", self.cbu_count);
        self.cbu_count += 1;
        b
    }

    fn next_doc(&mut self) -> String {
        let b = format!("@doc{}", self.doc_count);
        self.doc_count += 1;
        b
    }

    fn next_entity(&mut self) -> String {
        let b = format!("@ent{}", self.ent_count);
        self.ent_count += 1;
        b
    }
}

#[derive(Debug, Clone)]
pub struct DslPlan {
    pub dsl_source: String,
    pub bindings: Vec<(String, String)>,
    pub operation_count: usize,
}

pub fn plan_intent(intent: &KycIntent) -> DslPlan {
    let mut gen = BindingGenerator::new();
    let mut lines = Vec::new();
    let mut bindings = Vec::new();

    match intent {
        KycIntent::OnboardClient(i) => {
            plan_onboard_client(i, &mut gen, &mut lines, &mut bindings);
        }
        KycIntent::AddDocument(i) => {
            plan_add_document(i, &mut gen, &mut lines, &mut bindings);
        }
        KycIntent::AddEntityRole(i) => {
            plan_add_entity_role(i, &mut gen, &mut lines, &mut bindings);
        }
        KycIntent::LinkEvidence(i) => {
            plan_link_evidence(i, &mut lines);
        }
        KycIntent::ExtractAttributes(i) => {
            plan_extract_attributes(i, &mut lines);
        }
    }

    let operation_count = lines.len();
    DslPlan {
        dsl_source: lines.join("\n"),
        bindings,
        operation_count,
    }
}

fn plan_onboard_client(
    intent: &OnboardClientIntent,
    gen: &mut BindingGenerator,
    lines: &mut Vec<String>,
    bindings: &mut Vec<(String, String)>,
) {
    // 1. Create CBU - IMPORTANT: :as binding must be LAST
    let cbu_binding = gen.next_cbu();
    let client_type_str = match intent.client_type {
        ClientType::Individual => "individual",
        ClientType::Corporate => "corporate",
    };

    // Build all arguments first, then add :as at the end
    let mut cbu_args = format!(
        r#":name "{}" :client-type "{}""#,
        escape_string(&intent.client_name),
        client_type_str
    );
    if let Some(ref j) = intent.jurisdiction {
        cbu_args.push_str(&format!(r#" :jurisdiction "{}""#, j));
    }
    // :as goes LAST
    lines.push(format!("(cbu.create {} :as {})", cbu_args, cbu_binding));
    bindings.push((
        cbu_binding.clone(),
        format!("CBU for {}", intent.client_name),
    ));

    // 2. Create entities first (so we can link documents to them)
    let mut entity_bindings = Vec::new();
    for ent in &intent.entities {
        let ent_binding = gen.next_entity();
        let ent_type_str = match ent.entity_type {
            EntityType::NaturalPerson => "natural-person",
            EntityType::LimitedCompany => "limited-company",
            EntityType::Partnership => "partnership",
            EntityType::Trust => "trust",
        };

        // Create entity - :as LAST, quote the type value
        lines.push(format!(
            r#"(entity.create :type "{}" :name "{}" :as {})"#,
            ent_type_str,
            escape_string(&ent.name),
            ent_binding
        ));

        // Link to CBU with role - use cbu.assign-role which exists
        let mut link_args = format!(
            r#":cbu-id {} :entity-id {} :role "{}""#,
            cbu_binding, ent_binding, ent.role
        );
        if let Some(pct) = ent.ownership_percentage {
            link_args.push_str(&format!(" :ownership-percentage {}", pct));
        }
        lines.push(format!("(cbu.assign-role {})", link_args));

        bindings.push((ent_binding.clone(), format!("{} ({})", ent.name, ent.role)));
        entity_bindings.push(ent_binding);
    }

    // 3. Catalog documents - :as LAST
    for doc in &intent.documents {
        let doc_binding = gen.next_doc();

        let mut doc_args = format!(
            r#":document-type "{}" :cbu-id {}"#,
            doc.document_type, cbu_binding
        );
        if let Some(ref title) = doc.title {
            doc_args.push_str(&format!(r#" :title "{}""#, escape_string(title)));
        }
        // :as goes LAST
        lines.push(format!(
            "(document.catalog {} :as {})",
            doc_args, doc_binding
        ));
        bindings.push((
            doc_binding.clone(),
            format!("Document: {}", doc.document_type),
        ));

        // Link to entity if specified (no :as needed)
        if let Some(ent_idx) = doc.for_entity_index {
            if ent_idx < entity_bindings.len() {
                lines.push(format!(
                    "(document.link-entity :document-id {} :entity-id {})",
                    doc_binding, entity_bindings[ent_idx]
                ));
            }
        }

        // Extract attributes if requested (no :as needed)
        if doc.extract_attributes {
            lines.push(format!("(document.extract :document-id {})", doc_binding));
        }
    }
}

fn plan_add_document(
    intent: &AddDocumentIntent,
    gen: &mut BindingGenerator,
    lines: &mut Vec<String>,
    bindings: &mut Vec<(String, String)>,
) {
    let doc_binding = gen.next_doc();

    // Build args first, :as LAST
    let mut doc_args = format!(
        r#":document-type "{}" :cbu-id {}"#,
        intent.document.document_type, intent.cbu_ref
    );
    if let Some(ref title) = intent.document.title {
        doc_args.push_str(&format!(r#" :title "{}""#, escape_string(title)));
    }
    lines.push(format!(
        "(document.catalog {} :as {})",
        doc_args, doc_binding
    ));
    bindings.push((
        doc_binding.clone(),
        format!("Document: {}", intent.document.document_type),
    ));

    if intent.document.extract_attributes {
        lines.push(format!("(document.extract :document-id {})", doc_binding));
    }
}

fn plan_add_entity_role(
    intent: &AddEntityRoleIntent,
    gen: &mut BindingGenerator,
    lines: &mut Vec<String>,
    bindings: &mut Vec<(String, String)>,
) {
    let ent_binding = gen.next_entity();
    let ent_type_str = match intent.entity.entity_type {
        EntityType::NaturalPerson => "natural-person",
        EntityType::LimitedCompany => "limited-company",
        EntityType::Partnership => "partnership",
        EntityType::Trust => "trust",
    };

    // Create entity - :as LAST, quote the type value
    lines.push(format!(
        r#"(entity.create :type "{}" :name "{}" :as {})"#,
        ent_type_str,
        escape_string(&intent.entity.name),
        ent_binding
    ));

    // Link to CBU
    let mut link_args = format!(
        r#":cbu-id {} :entity-id {} :role "{}""#,
        intent.cbu_ref, ent_binding, intent.entity.role
    );
    if let Some(pct) = intent.entity.ownership_percentage {
        link_args.push_str(&format!(" :ownership-percentage {}", pct));
    }
    lines.push(format!("(cbu.assign-role {})", link_args));

    bindings.push((
        ent_binding,
        format!("{} ({})", intent.entity.name, intent.entity.role),
    ));
}

fn plan_link_evidence(intent: &LinkEvidenceIntent, lines: &mut Vec<String>) {
    lines.push(format!(
        "(document.link-entity :document-id {} :entity-id {})",
        intent.document_ref, intent.entity_ref
    ));
}

fn plan_extract_attributes(intent: &ExtractAttributesIntent, lines: &mut Vec<String>) {
    lines.push(format!(
        "(document.extract :document-id {})",
        intent.document_ref
    ));
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// =============================================================================
// PIPELINE
// =============================================================================

#[derive(Debug)]
pub struct PipelineResult {
    pub intent: KycIntent,
    pub validation_errors: Vec<ValidationError>,
    pub dsl_plan: Option<DslPlan>,
    pub parse_result: Option<ParseResult>,
}

#[derive(Debug)]
pub struct ParseResult {
    pub success: bool,
    pub statement_count: usize,
    pub error: Option<String>,
}

pub fn run_pipeline(json_input: &str, vocab: &IntentVocabulary) -> Result<PipelineResult, String> {
    // Stage 1: Parse JSON into Intent
    let intent: KycIntent =
        serde_json::from_str(json_input).map_err(|e| format!("JSON parse error: {}", e))?;

    // Stage 2: Validate intent against vocabulary
    let validation_errors = validate_intent(&intent, vocab);

    if !validation_errors.is_empty() {
        return Ok(PipelineResult {
            intent,
            validation_errors,
            dsl_plan: None,
            parse_result: None,
        });
    }

    // Stage 3: Plan intent into DSL source
    let dsl_plan = plan_intent(&intent);

    // Stage 4: Parse DSL to verify syntax
    let parse_result = verify_dsl_syntax(&dsl_plan.dsl_source);

    Ok(PipelineResult {
        intent,
        validation_errors,
        dsl_plan: Some(dsl_plan),
        parse_result: Some(parse_result),
    })
}

fn verify_dsl_syntax(dsl_source: &str) -> ParseResult {
    match crate::dsl_v2::parser::parse_program(dsl_source) {
        Ok(program) => ParseResult {
            success: true,
            statement_count: program.statements.len(),
            error: None,
        },
        Err(e) => ParseResult {
            success: false,
            statement_count: 0,
            error: Some(e),
        },
    }
}

// =============================================================================
// PIPELINE WITH EXECUTION (Database Integration)
// =============================================================================

/// Extended pipeline result that includes execution results
#[cfg(feature = "database")]
#[derive(Debug)]
pub struct ExecutedPipelineResult {
    pub intent: KycIntent,
    pub validation_errors: Vec<ValidationError>,
    pub dsl_plan: Option<DslPlan>,
    pub parse_result: Option<ParseResult>,
    pub execution_results: Option<Vec<crate::dsl_v2::executor::ExecutionResult>>,
    /// Symbol table after execution - maps binding names to UUIDs
    pub symbols: std::collections::HashMap<String, uuid::Uuid>,
}

/// Run the full pipeline including database execution
///
/// This function extends `run_pipeline` to include the execution stage:
/// ```text
/// Agent JSON → Intent → Planner → DSL Source → Parser → Executor → DB
/// ```
///
/// # Arguments
/// * `json_input` - JSON string representing the agent intent
/// * `vocab` - Intent vocabulary for validation
/// * `executor` - DSL executor with database connection
///
/// # Returns
/// * `ExecutedPipelineResult` containing all pipeline stages and execution results
///
/// # Example
/// ```ignore
/// let pool = PgPool::connect(&database_url).await?;
/// let executor = DslExecutor::new(pool);
/// let vocab = IntentVocabulary::default();
///
/// let result = run_pipeline_with_execution(json, &vocab, &executor).await?;
///
/// // Access created CBU ID
/// if let Some(cbu_id) = result.symbols.get("cbu0") {
///     println!("Created CBU: {}", cbu_id);
/// }
/// ```
#[cfg(feature = "database")]
pub async fn run_pipeline_with_execution(
    json_input: &str,
    vocab: &IntentVocabulary,
    executor: &crate::dsl_v2::executor::DslExecutor,
) -> Result<ExecutedPipelineResult, String> {
    use crate::dsl_v2::executor::ExecutionContext;

    // Stage 1: Parse JSON into Intent
    let intent: KycIntent =
        serde_json::from_str(json_input).map_err(|e| format!("JSON parse error: {}", e))?;

    // Stage 2: Validate intent against vocabulary
    let validation_errors = validate_intent(&intent, vocab);

    if !validation_errors.is_empty() {
        return Ok(ExecutedPipelineResult {
            intent,
            validation_errors,
            dsl_plan: None,
            parse_result: None,
            execution_results: None,
            symbols: std::collections::HashMap::new(),
        });
    }

    // Stage 3: Plan intent into DSL source
    let dsl_plan = plan_intent(&intent);

    // Stage 4: Parse DSL to verify syntax
    let parse_result = verify_dsl_syntax(&dsl_plan.dsl_source);

    if !parse_result.success {
        return Ok(ExecutedPipelineResult {
            intent,
            validation_errors,
            dsl_plan: Some(dsl_plan),
            parse_result: Some(parse_result),
            execution_results: None,
            symbols: std::collections::HashMap::new(),
        });
    }

    // Stage 5: Execute DSL against database
    let mut ctx = ExecutionContext::new();
    let execution_results = executor
        .execute_dsl(&dsl_plan.dsl_source, &mut ctx)
        .await
        .map_err(|e| format!("Execution error: {}", e))?;

    Ok(ExecutedPipelineResult {
        intent,
        validation_errors,
        dsl_plan: Some(dsl_plan),
        parse_result: Some(parse_result),
        execution_results: Some(execution_results),
        symbols: ctx.symbols,
    })
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_intent_individual() {
        let json = r#"{
            "intent": "onboard_client",
            "client_name": "John Smith",
            "client_type": "individual",
            "jurisdiction": "UK",
            "documents": [
                {"document_type": "PASSPORT_GBR", "extract_attributes": true}
            ]
        }"#;

        let intent: KycIntent = serde_json::from_str(json).unwrap();

        match intent {
            KycIntent::OnboardClient(i) => {
                assert_eq!(i.client_name, "John Smith");
                assert_eq!(i.client_type, ClientType::Individual);
                assert_eq!(i.jurisdiction, Some("UK".into()));
                assert_eq!(i.documents.len(), 1);
            }
            _ => panic!("Wrong intent type"),
        }
    }

    #[test]
    fn test_validation_catches_invalid_document_type() {
        let intent = KycIntent::OnboardClient(OnboardClientIntent {
            client_name: "Test".into(),
            client_type: ClientType::Individual,
            jurisdiction: None,
            documents: vec![DocumentSpec {
                document_type: "INVALID_DOC_TYPE".into(),
                title: None,
                extract_attributes: true,
                for_entity_index: None,
            }],
            entities: vec![],
        });

        let vocab = IntentVocabulary::default();
        let errors = validate_intent(&intent, &vocab);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Unknown document type"));
    }

    #[test]
    fn test_validation_catches_invalid_entity_index() {
        let intent = KycIntent::OnboardClient(OnboardClientIntent {
            client_name: "Test".into(),
            client_type: ClientType::Individual,
            jurisdiction: None,
            documents: vec![DocumentSpec {
                document_type: "PASSPORT_GBR".into(),
                title: None,
                extract_attributes: true,
                for_entity_index: Some(5),
            }],
            entities: vec![],
        });

        let vocab = IntentVocabulary::default();
        let errors = validate_intent(&intent, &vocab);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("out of bounds"));
    }

    #[test]
    fn test_plan_individual_onboarding() {
        let intent = KycIntent::OnboardClient(OnboardClientIntent {
            client_name: "John Smith".into(),
            client_type: ClientType::Individual,
            jurisdiction: Some("UK".into()),
            documents: vec![DocumentSpec {
                document_type: "PASSPORT_GBR".into(),
                title: None,
                extract_attributes: true,
                for_entity_index: None,
            }],
            entities: vec![],
        });

        let plan = plan_intent(&intent);

        println!("Generated DSL:\n{}", plan.dsl_source);

        assert!(plan.dsl_source.contains("cbu.create"));
        assert!(plan.dsl_source.contains("John Smith"));
        assert!(plan.dsl_source.contains(":as @cbu0)")); // :as is LAST before )
    }

    #[test]
    fn test_deterministic_output() {
        let intent = KycIntent::OnboardClient(OnboardClientIntent {
            client_name: "Test Client".into(),
            client_type: ClientType::Individual,
            jurisdiction: None,
            documents: vec![DocumentSpec {
                document_type: "PASSPORT_USA".into(),
                title: None,
                extract_attributes: true,
                for_entity_index: None,
            }],
            entities: vec![],
        });

        let plan1 = plan_intent(&intent);
        let plan2 = plan_intent(&intent);

        assert_eq!(
            plan1.dsl_source, plan2.dsl_source,
            "Same intent must produce identical DSL"
        );
    }

    #[test]
    fn test_full_pipeline_success() {
        let json = r#"{
            "intent": "onboard_client",
            "client_name": "Pipeline Test",
            "client_type": "individual",
            "documents": [
                {"document_type": "PASSPORT_GBR"}
            ]
        }"#;

        let vocab = IntentVocabulary::default();
        let result = run_pipeline(json, &vocab).unwrap();

        assert!(result.validation_errors.is_empty());
        assert!(result.dsl_plan.is_some());

        let plan = result.dsl_plan.unwrap();
        println!("Pipeline DSL:\n{}", plan.dsl_source);

        let parse = result.parse_result.unwrap();
        assert!(
            parse.success,
            "DSL should parse successfully: {:?}",
            parse.error
        );
    }

    #[test]
    fn test_full_pipeline_validation_failure() {
        let json = r#"{
            "intent": "onboard_client",
            "client_name": "Test",
            "client_type": "individual",
            "documents": [
                {"document_type": "FAKE_DOCUMENT"}
            ]
        }"#;

        let vocab = IntentVocabulary::default();
        let result = run_pipeline(json, &vocab).unwrap();

        assert!(!result.validation_errors.is_empty());
        assert!(result.dsl_plan.is_none());
    }
}
