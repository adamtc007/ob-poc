//! Evaluation harness for agent pipeline testing
//!
//! Runs golden test cases from evaluation_dataset.yaml and reports accuracy metrics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use super::dsl_generator::{DslGenerator, GenerationContext};
use super::entity_extractor::{EntityExtractor, ExtractedEntities};
use super::entity_types::EntityTypesConfig;
use super::instrument_hierarchy::InstrumentHierarchyConfig;
use super::intent_classifier::{ConversationContext, IntentClassifier};
use super::market_regions::MarketRegionsConfig;
use super::taxonomy::IntentTaxonomy;

/// Evaluation dataset loaded from YAML
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluationDataset {
    pub version: String,
    pub description: String,
    pub evaluation_cases: Vec<EvaluationCase>,
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub categories: HashMap<String, Vec<String>>,
}

/// A single evaluation test case
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluationCase {
    pub id: String,
    pub category: String,
    pub difficulty: String,
    pub input: String,
    #[serde(default)]
    pub expected_intents: Vec<String>,
    #[serde(default)]
    pub expected_entities: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub expected_dsl_contains: Vec<String>,
    #[serde(default)]
    pub expected_dsl_not_contains: Vec<String>,
    #[serde(default)]
    pub expected_dsl_statements: Option<usize>,
    #[serde(default)]
    pub expected_response_type: Option<String>,
    #[serde(default)]
    pub expected_expansion: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub context: Option<TestContext>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Test context for context-dependent cases
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TestContext {
    #[serde(default)]
    pub last_intent: Option<String>,
    #[serde(default)]
    pub session_entities: HashMap<String, String>,
    #[serde(default)]
    pub cbu_id: Option<String>,
}

/// Metrics configuration for thresholds
#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    pub intent_classification: ThresholdConfig,
    pub entity_extraction: ThresholdConfig,
    pub dsl_generation: DslThresholdConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdConfig {
    pub accuracy_threshold: f64,
    pub precision_threshold: f64,
    pub recall_threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DslThresholdConfig {
    pub validity_threshold: f64,
    pub completeness_threshold: f64,
}

/// Result for a single evaluation case
#[derive(Debug, Clone, Serialize)]
pub struct EvaluationResult {
    pub case_id: String,
    pub category: String,
    pub difficulty: String,
    pub intent_correct: bool,
    pub entities_correct: bool,
    pub dsl_valid: bool,
    pub dsl_contains_check: bool,
    pub passed: bool,
    pub errors: Vec<String>,
    pub latency_ms: u64,
    pub classified_intents: Vec<String>,
    pub extracted_entities: HashMap<String, String>,
    pub generated_dsl: Option<String>,
}

/// Aggregate evaluation report
#[derive(Debug, Clone, Serialize)]
pub struct EvaluationReport {
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub intent_accuracy: f64,
    pub entity_accuracy: f64,
    pub dsl_validity_rate: f64,
    pub avg_latency_ms: f64,
    pub by_category: HashMap<String, CategoryMetrics>,
    pub by_difficulty: HashMap<String, CategoryMetrics>,
    pub failures: Vec<FailureDetail>,
    pub results: Vec<EvaluationResult>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CategoryMetrics {
    pub total: usize,
    pub passed: usize,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureDetail {
    pub case_id: String,
    pub category: String,
    pub reason: String,
}

impl EvaluationDataset {
    /// Load dataset from YAML file
    pub fn load(path: &Path) -> Result<Self, EvaluationError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| EvaluationError::Io(e.to_string()))?;
        Self::load_from_str(&content)
    }

    /// Load dataset from YAML string
    pub fn load_from_str(yaml: &str) -> Result<Self, EvaluationError> {
        serde_yaml::from_str(yaml).map_err(|e| EvaluationError::Parse(e.to_string()))
    }

    /// Get cases by category name
    pub fn get_category(&self, category: &str) -> Vec<&EvaluationCase> {
        if let Some(case_ids) = self.categories.get(category) {
            self.evaluation_cases
                .iter()
                .filter(|c| case_ids.contains(&c.id))
                .collect()
        } else {
            self.evaluation_cases
                .iter()
                .filter(|c| c.category == category)
                .collect()
        }
    }

    /// Get case by ID
    pub fn get_case(&self, id: &str) -> Option<&EvaluationCase> {
        self.evaluation_cases.iter().find(|c| c.id == id)
    }
}

/// Evaluation runner that executes test cases against the pipeline
pub struct EvaluationRunner {
    classifier: IntentClassifier,
    extractor: EntityExtractor,
    generator: DslGenerator,
    dataset: EvaluationDataset,
}

impl EvaluationRunner {
    /// Create a new evaluation runner
    pub fn new(
        taxonomy: IntentTaxonomy,
        extractor: EntityExtractor,
        generator: DslGenerator,
        dataset: EvaluationDataset,
    ) -> Self {
        Self {
            classifier: IntentClassifier::new(taxonomy),
            extractor,
            generator,
            dataset,
        }
    }

    /// Create runner from config directory
    pub fn from_config_dir(config_dir: &Path) -> Result<Self, EvaluationError> {
        let taxonomy_path = config_dir.join("intent_taxonomy.yaml");
        let entity_types_path = config_dir.join("entity_types.yaml");
        let market_regions_path = config_dir.join("market_regions.yaml");
        let instrument_hierarchy_path = config_dir.join("instrument_hierarchy.yaml");
        let mappings_path = config_dir.join("parameter_mappings.yaml");
        let dataset_path = config_dir.join("evaluation_dataset.yaml");

        let taxonomy = IntentTaxonomy::load_from_file(&taxonomy_path)
            .map_err(|e| EvaluationError::Config(format!("Failed to load taxonomy: {}", e)))?;

        let entity_types = EntityTypesConfig::load_from_file(&entity_types_path)
            .map_err(|e| EvaluationError::Config(format!("Failed to load entity types: {}", e)))?;

        let market_regions =
            MarketRegionsConfig::load_from_file(&market_regions_path).map_err(|e| {
                EvaluationError::Config(format!("Failed to load market regions: {}", e))
            })?;

        let instrument_hierarchy =
            InstrumentHierarchyConfig::load_from_file(&instrument_hierarchy_path).map_err(|e| {
                EvaluationError::Config(format!("Failed to load instrument hierarchy: {}", e))
            })?;

        let extractor = EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);

        let generator = DslGenerator::from_file(&mappings_path).map_err(|e| {
            EvaluationError::Config(format!("Failed to load parameter mappings: {}", e))
        })?;

        let dataset = EvaluationDataset::load(&dataset_path)?;

        Ok(Self::new(taxonomy, extractor, generator, dataset))
    }

    /// Run all evaluation cases
    pub fn run_all(&mut self) -> EvaluationReport {
        // Clone cases to avoid borrow conflict with run_case(&mut self)
        let cases: Vec<EvaluationCase> = self.dataset.evaluation_cases.clone();
        let results: Vec<EvaluationResult> = cases.iter().map(|case| self.run_case(case)).collect();

        EvaluationReport::from_results(results, &self.dataset.metrics)
    }

    /// Run cases in a specific category
    pub fn run_category(&mut self, category: &str) -> EvaluationReport {
        // Clone cases to avoid borrow conflict with run_case(&mut self)
        let cases: Vec<EvaluationCase> = self
            .dataset
            .get_category(category)
            .into_iter()
            .cloned()
            .collect();
        let results: Vec<EvaluationResult> = cases.iter().map(|case| self.run_case(case)).collect();

        EvaluationReport::from_results(results, &self.dataset.metrics)
    }

    /// Run a single case by ID
    pub fn run_single(&mut self, case_id: &str) -> Option<EvaluationResult> {
        // Clone case to avoid borrow conflict with run_case(&mut self)
        let case = self.dataset.get_case(case_id).cloned();
        case.map(|c| self.run_case(&c))
    }

    /// Run a single evaluation case
    fn run_case(&mut self, case: &EvaluationCase) -> EvaluationResult {
        let start = Instant::now();
        let mut errors = Vec::new();

        // Build context
        let context = self.build_context(case);

        // Step 1: Classify intent
        let classification = self.classifier.classify(&case.input, &context);
        let classified_intents: Vec<String> = classification
            .intents
            .iter()
            .map(|i| i.intent_id.clone())
            .collect();

        // Step 2: Check intent correctness
        let intent_correct = self.check_intents(&classified_intents, &case.expected_intents);
        if !intent_correct {
            errors.push(format!(
                "Intent mismatch: expected {:?}, got {:?}",
                case.expected_intents, classified_intents
            ));
        }

        // Step 3: Extract entities
        let entities = self.extractor.extract(&case.input, &context);
        let extracted_map = self.entities_to_map(&entities);

        // Step 4: Check entity correctness
        let entities_correct = self.check_entities(&extracted_map, &case.expected_entities);
        if !entities_correct {
            errors.push(format!(
                "Entity mismatch: expected {:?}, got {:?}",
                case.expected_entities, extracted_map
            ));
        }

        // Step 5: Generate DSL (if we have intents)
        let generated_dsl = if !classification.intents.is_empty() {
            // Build generation context from test context
            let gen_context = self.build_generation_context(case);
            let dsl =
                self.generator
                    .generate(&classification.intents, &entities, &context, &gen_context);
            if dsl.is_complete() {
                Some(dsl.to_dsl_string())
            } else {
                errors.push(format!(
                    "DSL generation incomplete: missing params {:?}",
                    dsl.missing_params
                        .iter()
                        .map(|p| &p.name)
                        .collect::<Vec<_>>()
                ));
                // Still return the partial DSL for debugging
                Some(dsl.to_dsl_string())
            }
        } else {
            None
        };

        // Step 6: Check DSL validity and contents
        let (dsl_valid, dsl_contains_check) = if let Some(ref dsl) = generated_dsl {
            let valid = self.check_dsl_valid(dsl);
            let contains = self.check_dsl_contains(
                dsl,
                &case.expected_dsl_contains,
                &case.expected_dsl_not_contains,
            );
            if !contains {
                errors.push(format!(
                    "DSL content check failed: expected to contain {:?}, not contain {:?}",
                    case.expected_dsl_contains, case.expected_dsl_not_contains
                ));
            }
            (valid, contains)
        } else {
            // If we expect clarification, that's okay
            let is_clarification_expected =
                case.expected_response_type.as_deref() == Some("clarification");
            (is_clarification_expected, is_clarification_expected)
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        let passed = intent_correct && entities_correct && dsl_valid && dsl_contains_check;

        EvaluationResult {
            case_id: case.id.clone(),
            category: case.category.clone(),
            difficulty: case.difficulty.clone(),
            intent_correct,
            entities_correct,
            dsl_valid,
            dsl_contains_check,
            passed,
            errors,
            latency_ms,
            classified_intents,
            extracted_entities: extracted_map,
            generated_dsl,
        }
    }

    fn build_context(&self, case: &EvaluationCase) -> ConversationContext {
        let mut context = ConversationContext::default();

        if let Some(test_ctx) = &case.context {
            if let Some(last_intent) = &test_ctx.last_intent {
                context.last_intent = Some(last_intent.clone());
            }
            for (key, value) in &test_ctx.session_entities {
                context.session_entities.insert(key.clone(), value.clone());
            }
            // Add CBU to session_entities if present
            if let Some(cbu_id) = &test_ctx.cbu_id {
                context
                    .session_entities
                    .insert("cbu_reference".to_string(), cbu_id.clone());
            }
        }

        context
    }

    fn build_generation_context(&self, case: &EvaluationCase) -> GenerationContext {
        let mut gen_context = GenerationContext::default();

        if let Some(test_ctx) = &case.context {
            if let Some(cbu_id) = &test_ctx.cbu_id {
                gen_context.cbu_id = Some(cbu_id.clone());
            }
        }

        gen_context
    }

    fn check_intents(&self, classified: &[String], expected: &[String]) -> bool {
        if expected.is_empty() {
            return classified.is_empty();
        }

        // All expected intents should be present
        for exp in expected {
            if !classified.contains(exp) {
                return false;
            }
        }
        true
    }

    fn check_entities(
        &self,
        extracted: &HashMap<String, String>,
        expected: &HashMap<String, serde_json::Value>,
    ) -> bool {
        for (key, expected_value) in expected {
            match extracted.get(key) {
                Some(actual) => {
                    // Handle null expected value (entity should NOT be extracted)
                    if expected_value.is_null() {
                        return false;
                    }
                    // Handle string comparison
                    if let Some(exp_str) = expected_value.as_str() {
                        if actual.to_lowercase() != exp_str.to_lowercase() {
                            return false;
                        }
                    }
                    // Handle array of strings
                    if let Some(exp_arr) = expected_value.as_array() {
                        let exp_strs: Vec<String> = exp_arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                            .collect();
                        let actual_lower = actual.to_lowercase();
                        if !exp_strs.iter().any(|e| actual_lower.contains(e)) {
                            return false;
                        }
                    }
                }
                None => {
                    // If expected is null, that's correct (shouldn't be extracted)
                    if !expected_value.is_null() {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn entities_to_map(&self, entities: &ExtractedEntities) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for entity in entities.iter() {
            map.insert(entity.entity_type.clone(), entity.value.clone());
        }
        map
    }

    fn check_dsl_valid(&self, dsl: &str) -> bool {
        // Basic validity check - should start with ( and be balanced
        if dsl.is_empty() {
            return false;
        }

        let mut depth = 0;
        for c in dsl.chars() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if depth < 0 {
                return false;
            }
        }
        depth == 0
    }

    fn check_dsl_contains(
        &self,
        dsl: &str,
        must_contain: &[String],
        must_not_contain: &[String],
    ) -> bool {
        let dsl_lower = dsl.to_lowercase();

        for pattern in must_contain {
            if !dsl_lower.contains(&pattern.to_lowercase()) {
                return false;
            }
        }

        for pattern in must_not_contain {
            if dsl_lower.contains(&pattern.to_lowercase()) {
                return false;
            }
        }

        true
    }
}

impl EvaluationReport {
    /// Create report from evaluation results
    pub fn from_results(results: Vec<EvaluationResult>, _metrics: &MetricsConfig) -> Self {
        let total_cases = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total_cases - passed;

        let intent_correct_count = results.iter().filter(|r| r.intent_correct).count();
        let entity_correct_count = results.iter().filter(|r| r.entities_correct).count();
        let dsl_valid_count = results.iter().filter(|r| r.dsl_valid).count();

        let intent_accuracy = if total_cases > 0 {
            intent_correct_count as f64 / total_cases as f64
        } else {
            0.0
        };
        let entity_accuracy = if total_cases > 0 {
            entity_correct_count as f64 / total_cases as f64
        } else {
            0.0
        };
        let dsl_validity_rate = if total_cases > 0 {
            dsl_valid_count as f64 / total_cases as f64
        } else {
            0.0
        };

        let total_latency: u64 = results.iter().map(|r| r.latency_ms).sum();
        let avg_latency_ms = if total_cases > 0 {
            total_latency as f64 / total_cases as f64
        } else {
            0.0
        };

        // By category
        let mut by_category: HashMap<String, CategoryMetrics> = HashMap::new();
        for result in &results {
            let entry = by_category.entry(result.category.clone()).or_default();
            entry.total += 1;
            if result.passed {
                entry.passed += 1;
            }
        }
        for metrics in by_category.values_mut() {
            metrics.pass_rate = if metrics.total > 0 {
                metrics.passed as f64 / metrics.total as f64
            } else {
                0.0
            };
        }

        // By difficulty
        let mut by_difficulty: HashMap<String, CategoryMetrics> = HashMap::new();
        for result in &results {
            let entry = by_difficulty.entry(result.difficulty.clone()).or_default();
            entry.total += 1;
            if result.passed {
                entry.passed += 1;
            }
        }
        for metrics in by_difficulty.values_mut() {
            metrics.pass_rate = if metrics.total > 0 {
                metrics.passed as f64 / metrics.total as f64
            } else {
                0.0
            };
        }

        // Collect failures
        let failures: Vec<FailureDetail> = results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| FailureDetail {
                case_id: r.case_id.clone(),
                category: r.category.clone(),
                reason: r.errors.join("; "),
            })
            .collect();

        Self {
            total_cases,
            passed,
            failed,
            intent_accuracy,
            entity_accuracy,
            dsl_validity_rate,
            avg_latency_ms,
            by_category,
            by_difficulty,
            failures,
            results,
        }
    }

    /// Print summary to stdout
    pub fn print_summary(&self) {
        println!("=== Evaluation Report ===");
        println!(
            "Total: {} | Passed: {} | Failed: {}",
            self.total_cases, self.passed, self.failed
        );
        println!("Intent Accuracy: {:.1}%", self.intent_accuracy * 100.0);
        println!("Entity Accuracy: {:.1}%", self.entity_accuracy * 100.0);
        println!("DSL Validity: {:.1}%", self.dsl_validity_rate * 100.0);
        println!("Avg Latency: {:.1}ms", self.avg_latency_ms);

        println!("\nBy Category:");
        let mut categories: Vec<_> = self.by_category.iter().collect();
        categories.sort_by_key(|(k, _)| k.as_str());
        for (cat, metrics) in categories {
            println!(
                "  {}: {}/{} ({:.1}%)",
                cat,
                metrics.passed,
                metrics.total,
                metrics.pass_rate * 100.0
            );
        }

        println!("\nBy Difficulty:");
        for difficulty in ["easy", "medium", "hard"] {
            if let Some(metrics) = self.by_difficulty.get(difficulty) {
                println!(
                    "  {}: {}/{} ({:.1}%)",
                    difficulty,
                    metrics.passed,
                    metrics.total,
                    metrics.pass_rate * 100.0
                );
            }
        }

        if !self.failures.is_empty() {
            println!("\nFailures ({}):", self.failures.len());
            for f in &self.failures {
                println!("  [{}] {}: {}", f.category, f.case_id, f.reason);
            }
        }
    }

    /// Print CSV format
    pub fn print_csv(&self) {
        println!("case_id,category,difficulty,intent_correct,entities_correct,dsl_valid,passed,latency_ms");
        for r in &self.results {
            println!(
                "{},{},{},{},{},{},{},{}",
                r.case_id,
                r.category,
                r.difficulty,
                r.intent_correct,
                r.entities_correct,
                r.dsl_valid,
                r.passed,
                r.latency_ms
            );
        }
    }

    /// Check if report meets thresholds
    pub fn meets_thresholds(&self, intent_threshold: f64) -> bool {
        self.intent_accuracy >= intent_threshold
    }
}

/// Errors that can occur during evaluation
#[derive(Debug)]
pub enum EvaluationError {
    Io(String),
    Parse(String),
    Config(String),
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationError::Io(msg) => write!(f, "IO error: {}", msg),
            EvaluationError::Parse(msg) => write!(f, "Parse error: {}", msg),
            EvaluationError::Config(msg) => write!(f, "Config error: {}", msg),
        }
    }
}

impl std::error::Error for EvaluationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dataset() -> EvaluationDataset {
        let yaml = r#"
version: "1.0"
description: "Test dataset"
evaluation_cases:
  - id: test_1
    category: investment_manager
    difficulty: easy
    input: "Add BlackRock as investment manager"
    expected_intents:
      - im_assign
    expected_entities:
      manager_reference: "BlackRock"
    expected_dsl_contains:
      - "investment-manager.assign"
      - "BlackRock"
metrics:
  intent_classification:
    accuracy_threshold: 0.85
    precision_threshold: 0.80
    recall_threshold: 0.80
  entity_extraction:
    accuracy_threshold: 0.90
    precision_threshold: 0.85
    recall_threshold: 0.85
  dsl_generation:
    validity_threshold: 0.95
    completeness_threshold: 0.90
categories:
  quick_smoke:
    - test_1
"#;
        EvaluationDataset::load_from_str(yaml).unwrap()
    }

    #[test]
    fn test_load_dataset() {
        let dataset = sample_dataset();
        assert_eq!(dataset.version, "1.0");
        assert_eq!(dataset.evaluation_cases.len(), 1);
        assert_eq!(dataset.evaluation_cases[0].id, "test_1");
    }

    #[test]
    fn test_get_category() {
        let dataset = sample_dataset();
        let quick_smoke = dataset.get_category("quick_smoke");
        assert_eq!(quick_smoke.len(), 1);
        assert_eq!(quick_smoke[0].id, "test_1");
    }

    #[test]
    fn test_report_from_results() {
        let results = vec![
            EvaluationResult {
                case_id: "test_1".to_string(),
                category: "investment_manager".to_string(),
                difficulty: "easy".to_string(),
                intent_correct: true,
                entities_correct: true,
                dsl_valid: true,
                dsl_contains_check: true,
                passed: true,
                errors: vec![],
                latency_ms: 10,
                classified_intents: vec!["im_assign".to_string()],
                extracted_entities: HashMap::new(),
                generated_dsl: Some(
                    "(investment-manager.assign :manager \"BlackRock\")".to_string(),
                ),
            },
            EvaluationResult {
                case_id: "test_2".to_string(),
                category: "pricing".to_string(),
                difficulty: "medium".to_string(),
                intent_correct: false,
                entities_correct: true,
                dsl_valid: true,
                dsl_contains_check: true,
                passed: false,
                errors: vec!["Intent mismatch".to_string()],
                latency_ms: 15,
                classified_intents: vec![],
                extracted_entities: HashMap::new(),
                generated_dsl: None,
            },
        ];

        let metrics = MetricsConfig {
            intent_classification: ThresholdConfig {
                accuracy_threshold: 0.85,
                precision_threshold: 0.80,
                recall_threshold: 0.80,
            },
            entity_extraction: ThresholdConfig {
                accuracy_threshold: 0.90,
                precision_threshold: 0.85,
                recall_threshold: 0.85,
            },
            dsl_generation: DslThresholdConfig {
                validity_threshold: 0.95,
                completeness_threshold: 0.90,
            },
        };

        let report = EvaluationReport::from_results(results, &metrics);
        assert_eq!(report.total_cases, 2);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.intent_accuracy, 0.5);
        assert_eq!(report.entity_accuracy, 1.0);
    }
}
