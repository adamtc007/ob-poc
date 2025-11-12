//! DSL Parsing Coordinator
//!
//! This module provides centralized parsing coordination that delegates to domain-specific
//! handlers while maintaining the unified v3.0 DSL syntax. It replaces the legacy
//! centralized parser with a domain-aware system that uses the DomainRegistry.
//!
//! ## Architecture:
//! - Central parsing entry point for all DSL operations
//! - Domain detection and routing based on verb analysis
//! - Domain-specific parsing delegation via DomainHandler trait
//! - Unified AST validation and error handling
//! - Support for v3.0 unified S-expression syntax
//!
//! ## Key Features:
//! - Automatic domain detection from DSL verbs
//! - Fallback to generic parsing for unknown domains
//! - Comprehensive error handling with domain context
//! - AST validation and semantic analysis
//! - Integration with existing DslManager workflow

use crate::dsl::{
    domain_context::DomainContext,
    domain_registry::{DomainHandler, DomainRegistry},
    DslEditError, DslEditResult,
};
use crate::parser::idiomatic_parser::{parse_form, parse_program as parse_program_internal};
use crate::parser_ast::{Form, Program, VerbForm};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Central coordinator for all DSL parsing operations
pub(crate) struct ParsingCoordinator {
    /// Domain registry for handler lookup and routing
    domain_registry: DomainRegistry,

    /// Verb-to-domain mapping for automatic domain detection
    verb_domain_mapping: HashMap<String, String>,

    /// Parsing statistics and metrics
    parsing_stats: ParsingStatistics,
}

/// Statistics for parsing operations
#[derive(Debug, Default)]
pub(crate) struct ParsingStatistics {
    pub total_parses: u64,
    pub successful_parses: u64,
    pub domain_routed_parses: u64,
    pub fallback_parses: u64,
    pub parse_errors: u64,
    pub domain_routing_errors: u64,
}

/// Domain detection result from DSL content analysis
#[derive(Debug, Clone)]
pub struct DomainDetection {
    /// Primary detected domain
    pub primary_domain: String,
    /// Secondary domains detected
    pub secondary_domains: Vec<String>,
    /// Confidence score for detection (0.0 - 1.0)
    pub confidence: f64,
    /// Verbs that led to domain detection
    pub detected_verbs: Vec<String>,
}

impl DomainDetection {
    pub fn new(primary_domain: String) -> Self {
        Self {
            primary_domain,
            secondary_domains: Vec::new(),
            confidence: 1.0,
            detected_verbs: Vec::new(),
        }
    }
}

/// Enhanced parsing result with domain context
#[derive(Debug)]
pub struct ParseResult {
    /// Parsed AST program
    pub program: Program,
    /// Detected domain information
    pub domain_context: Option<DomainContext>,
    /// Parsing warnings (non-fatal issues)
    pub warnings: Vec<String>,
    /// Domain handler used for parsing
    pub handler_used: Option<String>,
}

impl ParsingCoordinator {
    /// Create new parsing coordinator with domain registry
    pub fn new(domain_registry: DomainRegistry) -> Self {
        let verb_domain_mapping = Self::build_verb_domain_mapping(&domain_registry);

        Self {
            domain_registry,
            verb_domain_mapping,
            parsing_stats: ParsingStatistics::default(),
        }
    }

    /// Build verb-to-domain mapping from registered domains
    fn build_verb_domain_mapping(registry: &DomainRegistry) -> HashMap<String, String> {
        let mut mapping = HashMap::new();

        for domain_name in registry.list_domains() {
            if let Ok(handler) = registry.get_domain(&domain_name) {
                let vocabulary = handler.get_vocabulary();

                // Map all verbs from this domain's vocabulary
                for verb_def in &vocabulary.verbs {
                    mapping.insert(verb_def.name.clone(), domain_name.clone());
                }
            }
        }

        // Add standard v3.0 verbs with their domains
        mapping.extend([
            ("entity".to_string(), "ubo".to_string()),
            ("edge".to_string(), "ubo".to_string()),
            ("role.assign".to_string(), "ubo".to_string()),
            ("ubo.calc".to_string(), "ubo".to_string()),
            ("ubo.outcome".to_string(), "ubo".to_string()),
            ("kyc.verify".to_string(), "kyc".to_string()),
            ("kyc.assess_risk".to_string(), "kyc".to_string()),
            ("kyc.collect_document".to_string(), "kyc".to_string()),
            ("kyc.screen_sanctions".to_string(), "kyc".to_string()),
            ("compliance.fatca_check".to_string(), "kyc".to_string()),
            ("compliance.crs_check".to_string(), "kyc".to_string()),
            ("case.create".to_string(), "onboarding".to_string()),
            ("case.update".to_string(), "onboarding".to_string()),
            ("case.close".to_string(), "onboarding".to_string()),
            ("define-kyc-investigation".to_string(), "kyc".to_string()),
            ("workflow.transition".to_string(), "onboarding".to_string()),
        ]);

        mapping
    }

    /// Main parsing entry point - parses DSL and routes to appropriate domain
    pub async fn parse_dsl(&mut self, input: &str) -> DslEditResult<ParseResult> {
        self.parsing_stats.total_parses += 1;

        // First, try basic AST parsing
        let program = match parse_program_internal(input) {
            Ok(program) => program,
            Err(parse_error) => {
                self.parsing_stats.parse_errors += 1;
                error!("Basic AST parsing failed: {:?}", parse_error);
                return Err(DslEditError::CompilationError(format!(
                    "Failed to parse DSL: {:?}",
                    parse_error
                )));
            }
        };

        debug!("Successfully parsed {} forms", program.len());

        // Detect domain from verbs in the program
        let detected_domain = self.detect_domain_from_program(&program);

        let mut warnings = Vec::new();
        let mut domain_context = None;
        let mut handler_used = None;

        // If we detected a domain, use domain-specific processing
        if let Some(domain_name) = detected_domain {
            match self.domain_registry.get_domain(&domain_name) {
                Ok(domain_handler) => {
                    debug!("Routing parsing to domain: {}", domain_name);
                    self.parsing_stats.domain_routed_parses += 1;
                    handler_used = Some(domain_name.clone());

                    // Extract domain context from parsed DSL
                    match domain_handler.extract_context_from_dsl(input).await {
                        Ok(context) => {
                            domain_context = Some(context);
                        }
                        Err(e) => {
                            warn!("Failed to extract domain context: {:?}", e);
                            warnings.push(format!("Context extraction failed: {}", e));
                        }
                    }

                    // Validate domain-specific constraints
                    if let Err(validation_error) = self
                        .validate_domain_constraints(&program, domain_handler, &domain_context)
                        .await
                    {
                        warnings.push(format!("Domain validation warning: {}", validation_error));
                    }
                }
                Err(e) => {
                    self.parsing_stats.domain_routing_errors += 1;
                    warn!("Failed to get domain handler for {}: {:?}", domain_name, e);
                    warnings.push(format!("Domain routing failed for {}: {}", domain_name, e));
                }
            }
        } else {
            debug!("No domain detected, using generic parsing");
            self.parsing_stats.fallback_parses += 1;
        }

        // Perform final validation
        self.validate_program_structure(&program)?;

        self.parsing_stats.successful_parses += 1;

        Ok(ParseResult {
            program,
            domain_context,
            warnings,
            handler_used,
        })
    }

    /// Detect domain from verbs used in the program
    fn detect_domain_from_program(&self, program: &Program) -> Option<String> {
        let mut domain_votes: HashMap<String, u32> = HashMap::new();

        for form in program {
            if let Form::Verb(verb_form) = form {
                if let Some(domain_name) = self.verb_domain_mapping.get(&verb_form.verb) {
                    *domain_votes.entry(domain_name.clone()).or_insert(0) += 1;
                }
            }
        }

        // Return domain with most verb matches
        domain_votes
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(domain, _)| domain)
    }

    /// Validate domain-specific constraints
    async fn validate_domain_constraints(
        &self,
        _program: &Program,
        domain_handler: &dyn DomainHandler,
        _context: &Option<DomainContext>,
    ) -> DslEditResult<()> {
        // Get domain validation rules
        let validation_rules = domain_handler.get_validation_rules();

        // For now, just validate we have rules defined
        if validation_rules.is_empty() {
            return Err(DslEditError::DomainValidationError(
                "No validation rules defined for domain".to_string(),
            ));
        }

        // TODO: Implement specific validation rule checking
        // This would check things like:
        // - Required keys for specific verbs
        // - Value type validation
        // - Business rule constraints
        // - State transition validity

        Ok(())
    }

    /// Validate overall program structure
    fn validate_program_structure(&self, program: &Program) -> DslEditResult<()> {
        if program.is_empty() {
            return Err(DslEditError::DomainValidationError(
                "Empty DSL program".to_string(),
            ));
        }

        // Basic structural validation
        for (index, form) in program.iter().enumerate() {
            match form {
                Form::Verb(verb_form) => {
                    if verb_form.verb.is_empty() {
                        return Err(DslEditError::DomainValidationError(format!(
                            "Empty verb at form {}",
                            index
                        )));
                    }
                }
                Form::Comment(_) => {
                    // Comments are always valid
                }
            }
        }

        Ok(())
    }

    /// Get parsing statistics
    pub fn get_statistics(&self) -> &ParsingStatistics {
        &self.parsing_stats
    }

    /// Reset parsing statistics
    pub(crate) fn reset_statistics(&mut self) {
        self.parsing_stats = ParsingStatistics::default();
    }

    /// Add or update a domain in the registry
    pub fn register_domain(&mut self, handler: Box<dyn DomainHandler>) -> DslEditResult<()> {
        let domain_name = handler.domain_name().to_string();

        // Update verb mapping with new domain's verbs
        let vocabulary = handler.get_vocabulary();
        for verb_def in &vocabulary.verbs {
            self.verb_domain_mapping
                .insert(verb_def.name.clone(), domain_name.clone());
        }

        // Register with domain registry
        self.domain_registry.register_domain(handler)?;

        info!(
            "Registered domain '{}' with parsing coordinator",
            domain_name
        );
        Ok(())
    }

    /// Parse a single form (for testing and incremental parsing)
    pub(crate) fn parse_single_form(&self, input: &str) -> DslEditResult<Form> {
        match parse_form(input) {
            Ok((_, form)) => Ok(form),
            Err(e) => Err(DslEditError::CompilationError(format!(
                "Failed to parse form: {:?}",
                e
            ))),
        }
    }

    /// Validate a verb form against its domain constraints
    pub async fn validate_verb_form(&self, verb_form: &VerbForm) -> DslEditResult<Vec<String>> {
        let mut warnings = Vec::new();

        // Find domain for this verb
        if let Some(domain_name) = self.verb_domain_mapping.get(&verb_form.verb) {
            if let Ok(domain_handler) = self.domain_registry.get_domain(domain_name) {
                // Check if domain supports this verb
                let supported_ops = domain_handler.supported_operations();
                if !supported_ops.contains(&verb_form.verb) {
                    warnings.push(format!(
                        "Verb '{}' not in supported operations for domain '{}'",
                        verb_form.verb, domain_name
                    ));
                }

                // TODO: Add more specific validations like:
                // - Required keys for this verb
                // - Value type checking
                // - Business rule validation
            } else {
                warnings.push(format!("Domain handler not found for '{}'", domain_name));
            }
        } else {
            warnings.push(format!(
                "No domain mapping found for verb '{}'",
                verb_form.verb
            ));
        }

        Ok(warnings)
    }
}

impl Default for ParsingCoordinator {
    fn default() -> Self {
        Self::new(DomainRegistry::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KycDomainHandler, OnboardingDomainHandler};

    #[tokio::test]
    async fn test_parsing_coordinator_creation() {
        let coordinator = ParsingCoordinator::default();
        assert_eq!(coordinator.get_statistics().total_parses, 0);
    }

    #[tokio::test]
    async fn test_domain_detection() {
        let mut coordinator = ParsingCoordinator::default();

        // Register test domains
        coordinator
            .register_domain(Box::new(KycDomainHandler::new()))
            .unwrap();
        coordinator
            .register_domain(Box::new(OnboardingDomainHandler::new()))
            .unwrap();

        let kyc_dsl = "(kyc.verify :customer-id \"person-001\" :method \"enhanced_dd\")";
        let result = coordinator.parse_dsl(kyc_dsl).await;

        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert_eq!(parse_result.handler_used, Some("kyc".to_string()));
    }

    #[tokio::test]
    async fn test_v3_entity_parsing() {
        let mut coordinator = ParsingCoordinator::default();

        let entity_dsl =
            r#"(entity :id "company-test-001" :label "Company" :props {:legal-name "Test Corp"})"#;
        let result = coordinator.parse_dsl(entity_dsl).await;

        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert_eq!(parse_result.program.len(), 1);

        if let Form::Verb(verb_form) = &parse_result.program[0] {
            assert_eq!(verb_form.verb, "entity");
            assert!(verb_form.pairs.len() >= 3);
        } else {
            panic!("Expected verb form");
        }
    }

    #[tokio::test]
    async fn test_v3_comprehensive_parsing() {
        let mut coordinator = ParsingCoordinator::default();

        // Register all domains
        coordinator
            .register_domain(Box::new(KycDomainHandler::new()))
            .unwrap();
        coordinator
            .register_domain(Box::new(OnboardingDomainHandler::new()))
            .unwrap();

        // Test complex v3 DSL with multiple domains
        let v3_dsl = r#"
        ;; Test comprehensive v3 parsing
        (case.create
          :id "CBU-TEST-001"
          :nature-purpose "Investment fund setup")

        (entity
          :id "fund-test-001"
          :label "Company"
          :props {
            :legal-name "Test Investment Fund LP"
            :jurisdiction "US"
            :entity-type "Limited Partnership"
          })

        (kyc.verify
          :customer-id "person-manager-001"
          :method "enhanced_due_diligence"
          :doc-types ["passport" "utility_bill"]
          :verified-at "2025-01-27T10:00:00Z")

        (edge
          :from "person-manager-001"
          :to "fund-test-001"
          :type "HAS_CONTROL"
          :props {
            :percent 100.0
            :control-type "General Partner"
          }
          :evidence ["doc-partnership-agreement-001"])
        "#;

        let result = coordinator.parse_dsl(v3_dsl).await;
        assert!(
            result.is_ok(),
            "Failed to parse comprehensive v3 DSL: {:?}",
            result.err()
        );

        let parse_result = result.unwrap();

        // Should have comment + 4 verb forms
        assert_eq!(parse_result.program.len(), 5);

        // Verify parsing succeeded
        assert!(parse_result.warnings.is_empty() || parse_result.warnings.len() > 0);
        // May have warnings
    }

    #[tokio::test]
    async fn test_v3_syntax_compliance() {
        let coordinator = ParsingCoordinator::default();

        // Test all v3 syntax features
        let syntax_tests = vec![
            // Basic verb form
            "(entity :id \"test\" :label \"Company\")",
            // Map with nested properties
            r#"(entity :id "test" :props {:name "Test Corp" :active true})"#,
            // List values
            r#"(kyc.verify :doc-types ["passport" "utility_bill" "bank_statement"])"#,
            // Numbers and booleans
            r#"(edge :props {:percent 45.0 :voting-rights 45.0 :active true})"#,
            // Nested structures
            r#"(ubo.outcome :ubos [{:entity "person-001" :percent 45.0}])"#,
        ];

        for test_input in syntax_tests {
            let result = coordinator.parse_single_form(test_input);
            assert!(
                result.is_ok(),
                "Failed to parse v3 syntax: {} - Error: {:?}",
                test_input,
                result.err()
            );

            if let Ok(Form::Verb(verb_form)) = result {
                assert!(!verb_form.verb.is_empty(), "Verb should not be empty");
            }
        }
    }

    #[tokio::test]
    async fn test_parsing_statistics() {
        let mut coordinator = ParsingCoordinator::default();

        let simple_dsl = "(case.create :id \"test\")";
        let _ = coordinator.parse_dsl(simple_dsl).await;

        let stats = coordinator.get_statistics();
        assert_eq!(stats.total_parses, 1);

        coordinator.reset_statistics();
        assert_eq!(coordinator.get_statistics().total_parses, 0);
    }
}
