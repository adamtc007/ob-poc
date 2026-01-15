//! Test harness for batch stress-testing the lexicon tokenizer and intent parser.
//!
//! This module provides utilities to:
//! - Load prompts from YAML files
//! - Run them through tokenizer + parser
//! - Collect detailed results for analysis
//! - Generate reports on success/failure rates
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ob_agentic::lexicon::test_harness::{TestHarness, PromptTestCase};
//!
//! let harness = TestHarness::with_lexicon_path("config/agent/lexicon.yaml")?;
//! let results = harness.run_batch(&test_cases).await;
//! println!("{}", results.summary());
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::intent_parser::parse_tokens;
use super::loader::Lexicon;
use super::lowering::lower_tokens;
use super::tokenizer::{EntityResolver, MockEntityResolver, ResolvedEntity, Tokenizer};
use super::tokens::Token;
use super::IntentAst;

/// A single test case with input prompt and expected outcome.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PromptTestCase {
    /// Unique identifier for the test case.
    pub id: String,

    /// The input prompt to test.
    pub prompt: String,

    /// Expected intent type (e.g., "counterparty_create", "isda_establish").
    /// If None, we just check that parsing succeeds.
    #[serde(default)]
    pub expected_intent: Option<String>,

    /// Expected entities to be resolved.
    #[serde(default)]
    pub expected_entities: Vec<String>,

    /// Tags for categorization (e.g., ["otc", "counterparty", "happy_path"]).
    #[serde(default)]
    pub tags: Vec<String>,

    /// Optional description of what this test verifies.
    #[serde(default)]
    pub description: Option<String>,
}

/// Result of running a single test case.
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    /// The test case that was run.
    pub test_case: PromptTestCase,

    /// Whether the test passed.
    pub passed: bool,

    /// The actual intent type parsed (or "unknown" / "parse_error").
    pub actual_intent: String,

    /// Tokens produced by the tokenizer.
    pub tokens: Vec<TokenInfo>,

    /// Time taken to process.
    pub duration: Duration,

    /// Error message if parsing failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Detailed failure reason if test didn't pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

/// Simplified token info for reporting.
#[derive(Debug, Clone, Serialize)]
pub struct TokenInfo {
    pub text: String,
    pub token_type: String,
    pub source: String,
    pub resolved_id: Option<String>,
    pub confidence: f32,
}

impl From<&Token> for TokenInfo {
    fn from(token: &Token) -> Self {
        Self {
            text: token.text.clone(),
            token_type: format!("{:?}", token.token_type),
            source: format!("{:?}", token.source),
            resolved_id: token.resolved_id.clone(),
            confidence: token.confidence,
        }
    }
}

/// Batch test results with summary statistics.
#[derive(Debug, Clone, Serialize)]
pub struct BatchResults {
    /// Individual test results.
    pub results: Vec<TestResult>,

    /// Total tests run.
    pub total: usize,

    /// Tests that passed.
    pub passed: usize,

    /// Tests that failed.
    pub failed: usize,

    /// Total time for all tests.
    pub total_duration: Duration,

    /// Results grouped by tag.
    pub by_tag: HashMap<String, TagStats>,

    /// Results grouped by intent type.
    pub by_intent: HashMap<String, IntentStats>,
}

/// Statistics for a tag group.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TagStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Statistics for an intent type.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IntentStats {
    pub expected: usize,
    pub correctly_parsed: usize,
    pub incorrectly_parsed: usize,
}

impl BatchResults {
    /// Generate a human-readable summary.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out.push_str("                    LEXICON TEST HARNESS RESULTS                \n");
        out.push_str("═══════════════════════════════════════════════════════════════\n\n");

        let pass_rate = if self.total > 0 {
            (self.passed as f64 / self.total as f64) * 100.0
        } else {
            0.0
        };

        out.push_str(&format!(
            "OVERALL: {}/{} passed ({:.1}%) in {:?}\n\n",
            self.passed, self.total, pass_rate, self.total_duration
        ));

        // By tag
        if !self.by_tag.is_empty() {
            out.push_str("BY TAG:\n");
            let mut tags: Vec<_> = self.by_tag.iter().collect();
            tags.sort_by_key(|(k, _)| *k);
            for (tag, stats) in tags {
                let rate = if stats.total > 0 {
                    (stats.passed as f64 / stats.total as f64) * 100.0
                } else {
                    0.0
                };
                out.push_str(&format!(
                    "  {:<20} {}/{} ({:.1}%)\n",
                    tag, stats.passed, stats.total, rate
                ));
            }
            out.push('\n');
        }

        // By intent
        if !self.by_intent.is_empty() {
            out.push_str("BY INTENT:\n");
            let mut intents: Vec<_> = self.by_intent.iter().collect();
            intents.sort_by_key(|(k, _)| *k);
            for (intent, stats) in intents {
                let rate = if stats.expected > 0 {
                    (stats.correctly_parsed as f64 / stats.expected as f64) * 100.0
                } else {
                    0.0
                };
                out.push_str(&format!(
                    "  {:<25} {}/{} ({:.1}%)\n",
                    intent, stats.correctly_parsed, stats.expected, rate
                ));
            }
            out.push('\n');
        }

        // Failed tests
        let failures: Vec<_> = self.results.iter().filter(|r| !r.passed).collect();
        if !failures.is_empty() {
            out.push_str("FAILURES:\n");
            out.push_str("───────────────────────────────────────────────────────────────\n");
            for result in failures.iter().take(20) {
                out.push_str(&format!(
                    "  [{}] {}\n",
                    result.test_case.id, result.test_case.prompt
                ));
                if let Some(ref reason) = result.failure_reason {
                    out.push_str(&format!("    Reason: {}\n", reason));
                }
                out.push_str(&format!(
                    "    Expected: {:?}, Got: {}\n",
                    result.test_case.expected_intent, result.actual_intent
                ));
                out.push_str(&format!(
                    "    Tokens: [{}]\n\n",
                    result
                        .tokens
                        .iter()
                        .map(|t| format!("{}:{}", t.text, t.token_type))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if failures.len() > 20 {
                out.push_str(&format!(
                    "  ... and {} more failures\n",
                    failures.len() - 20
                ));
            }
        }

        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out
    }

    /// Export results to JSON.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize results")
    }

    /// Get all failed tests.
    pub fn failures(&self) -> Vec<&TestResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }

    /// Get tests with a specific tag.
    pub fn with_tag(&self, tag: &str) -> Vec<&TestResult> {
        self.results
            .iter()
            .filter(|r| r.test_case.tags.contains(&tag.to_string()))
            .collect()
    }
}

/// Test harness configuration.
#[derive(Debug, Clone)]
pub struct TestHarnessConfig {
    /// Mock entity mappings for testing.
    pub mock_entities: HashMap<String, ResolvedEntity>,

    /// Whether to use the full pipeline or just tokenizer + parser.
    pub use_pipeline: bool,
}

impl Default for TestHarnessConfig {
    fn default() -> Self {
        Self {
            mock_entities: HashMap::new(),
            use_pipeline: true,
        }
    }
}

/// The main test harness for batch-testing prompts.
pub struct TestHarness {
    lexicon: Arc<Lexicon>,
    config: TestHarnessConfig,
}

impl TestHarness {
    /// Create a new test harness with the given lexicon.
    pub fn new(lexicon: Lexicon) -> Self {
        Self {
            lexicon: Arc::new(lexicon),
            config: TestHarnessConfig::default(),
        }
    }

    /// Create a test harness by loading lexicon from a file.
    pub fn with_lexicon_path(path: impl AsRef<Path>) -> Result<Self> {
        let lexicon = Lexicon::load_from_file(path)?;
        Ok(Self::new(lexicon))
    }

    /// Set the harness configuration.
    pub fn with_config(mut self, config: TestHarnessConfig) -> Self {
        self.config = config;
        self
    }

    /// Add mock entity mappings.
    pub fn with_mock_entities(mut self, entities: HashMap<String, ResolvedEntity>) -> Self {
        self.config.mock_entities = entities;
        self
    }

    /// Run a batch of test cases.
    pub async fn run_batch(&self, test_cases: &[PromptTestCase]) -> BatchResults {
        let start = Instant::now();
        let mut results = Vec::with_capacity(test_cases.len());

        // Create mock resolver from config
        let resolver: Option<Arc<dyn EntityResolver>> = if self.config.mock_entities.is_empty() {
            None
        } else {
            let mut mock = MockEntityResolver::new();
            for (name, entity) in &self.config.mock_entities {
                mock = mock.with_entity(name, &entity.id, &entity.name, &entity.entity_type);
            }
            Some(Arc::new(mock))
        };

        for test_case in test_cases {
            let result = self.run_single(test_case, resolver.clone()).await;
            results.push(result);
        }

        let total_duration = start.elapsed();

        // Compute statistics
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        // By tag
        let mut by_tag: HashMap<String, TagStats> = HashMap::new();
        for result in &results {
            for tag in &result.test_case.tags {
                let stats = by_tag.entry(tag.clone()).or_default();
                stats.total += 1;
                if result.passed {
                    stats.passed += 1;
                } else {
                    stats.failed += 1;
                }
            }
        }

        // By intent
        let mut by_intent: HashMap<String, IntentStats> = HashMap::new();
        for result in &results {
            if let Some(ref expected) = result.test_case.expected_intent {
                let stats = by_intent.entry(expected.clone()).or_default();
                stats.expected += 1;
                if result.passed {
                    stats.correctly_parsed += 1;
                } else {
                    stats.incorrectly_parsed += 1;
                }
            }
        }

        BatchResults {
            results,
            total,
            passed,
            failed,
            total_duration,
            by_tag,
            by_intent,
        }
    }

    /// Run a single test case.
    async fn run_single(
        &self,
        test_case: &PromptTestCase,
        resolver: Option<Arc<dyn EntityResolver>>,
    ) -> TestResult {
        let start = Instant::now();

        // Create tokenizer
        let tokenizer = if let Some(res) = resolver {
            Tokenizer::new(Arc::clone(&self.lexicon)).with_entity_resolver(res)
        } else {
            Tokenizer::new(Arc::clone(&self.lexicon))
        };

        // Tokenize
        let raw_tokens = tokenizer.tokenize(&test_case.prompt).await;

        // AST Lowering - normalize token order and fuse type+name pairs
        let tokens = lower_tokens(&raw_tokens);
        let token_infos: Vec<TokenInfo> = tokens.iter().map(TokenInfo::from).collect();

        // Parse
        let (actual_intent, error) = match parse_tokens(&tokens) {
            Ok(ast) => (intent_type_name(&ast), None),
            Err(e) => ("parse_error".to_string(), Some(e)),
        };

        let duration = start.elapsed();

        // Determine if test passed
        let (passed, failure_reason) = if let Some(ref expected) = test_case.expected_intent {
            if actual_intent == *expected {
                (true, None)
            } else {
                (
                    false,
                    Some(format!("Expected '{}', got '{}'", expected, actual_intent)),
                )
            }
        } else {
            // No expected intent - just check it parsed successfully
            if error.is_some() {
                (false, Some("Parse failed".to_string()))
            } else if actual_intent == "unknown" {
                (false, Some("Parsed as unknown".to_string()))
            } else {
                (true, None)
            }
        };

        TestResult {
            test_case: test_case.clone(),
            passed,
            actual_intent,
            tokens: token_infos,
            duration,
            error,
            failure_reason,
        }
    }

    /// Load test cases from a YAML file.
    pub fn load_test_cases(path: impl AsRef<Path>) -> Result<Vec<PromptTestCase>> {
        let content =
            std::fs::read_to_string(path.as_ref()).context("Failed to read test cases file")?;
        let cases: Vec<PromptTestCase> =
            serde_yaml::from_str(&content).context("Failed to parse test cases YAML")?;
        Ok(cases)
    }

    /// Create a standard set of test cases for common intents.
    pub fn standard_test_cases() -> Vec<PromptTestCase> {
        vec![
            // Counterparty creation
            PromptTestCase {
                id: "cp_create_01".into(),
                prompt: "add Goldman Sachs as counterparty".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["Goldman Sachs".into()],
                tags: vec!["counterparty".into(), "create".into(), "happy_path".into()],
                description: Some("Basic counterparty creation".into()),
            },
            PromptTestCase {
                id: "cp_create_02".into(),
                prompt: "add JP Morgan as counterparty for IRS".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["JP Morgan".into()],
                tags: vec![
                    "counterparty".into(),
                    "create".into(),
                    "with_instrument".into(),
                ],
                description: Some("Counterparty with instrument".into()),
            },
            PromptTestCase {
                id: "cp_create_03".into(),
                prompt: "create counterparty Barclays under NY law".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["Barclays".into()],
                tags: vec!["counterparty".into(), "create".into(), "with_law".into()],
                description: Some("Counterparty with governing law".into()),
            },
            // ISDA establishment
            PromptTestCase {
                id: "isda_01".into(),
                prompt: "establish ISDA with Morgan Stanley under NY law".into(),
                expected_intent: Some("isda_establish".into()),
                expected_entities: vec!["Morgan Stanley".into()],
                tags: vec!["isda".into(), "create".into(), "happy_path".into()],
                description: Some("Basic ISDA establishment".into()),
            },
            PromptTestCase {
                id: "isda_02".into(),
                prompt: "create ISDA agreement with Citi under English law for CDS".into(),
                expected_intent: Some("isda_establish".into()),
                expected_entities: vec!["Citi".into()],
                tags: vec!["isda".into(), "create".into(), "with_instrument".into()],
                description: Some("ISDA with instrument".into()),
            },
            // CSA addition
            PromptTestCase {
                id: "csa_01".into(),
                prompt: "add VM CSA to Goldman Sachs".into(),
                expected_intent: Some("csa_add".into()),
                expected_entities: vec!["Goldman Sachs".into()],
                tags: vec!["csa".into(), "create".into(), "happy_path".into()],
                description: Some("Basic CSA addition".into()),
            },
            // Universe/portfolio operations
            PromptTestCase {
                id: "universe_01".into(),
                prompt: "add equities and bonds to universe".into(),
                expected_intent: Some("universe_add".into()),
                expected_entities: vec![],
                tags: vec!["universe".into(), "instruments".into()],
                description: Some("Add instruments to universe".into()),
            },
            PromptTestCase {
                id: "universe_02".into(),
                prompt: "enable trading in NYSE and LSE".into(),
                expected_intent: Some("universe_add".into()),
                expected_entities: vec![],
                tags: vec!["universe".into(), "markets".into()],
                description: Some("Add markets to universe".into()),
            },
            // Queries
            PromptTestCase {
                id: "query_01".into(),
                prompt: "show all counterparties".into(),
                expected_intent: Some("counterparty_list".into()),
                expected_entities: vec![],
                tags: vec!["query".into(), "list".into()],
                description: Some("List counterparties".into()),
            },
            PromptTestCase {
                id: "query_02".into(),
                prompt: "what ISDAs do we have with Goldman".into(),
                expected_intent: Some("isda_show".into()),
                expected_entities: vec!["Goldman".into()],
                tags: vec!["query".into(), "isda".into()],
                description: Some("Query ISDAs for entity".into()),
            },
            // Role assignment
            PromptTestCase {
                id: "role_01".into(),
                prompt: "assign John Smith as director".into(),
                expected_intent: Some("role_assign".into()),
                expected_entities: vec!["John Smith".into()],
                tags: vec!["role".into(), "assign".into()],
                description: Some("Assign director role".into()),
            },
            // Edge cases and variations
            PromptTestCase {
                id: "edge_01".into(),
                prompt: "Add a new counterparty called Deutsche Bank AG".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["Deutsche Bank AG".into()],
                tags: vec!["counterparty".into(), "edge_case".into(), "verbose".into()],
                description: Some("Verbose counterparty creation".into()),
            },
            PromptTestCase {
                id: "edge_02".into(),
                prompt: "I want to add BNP Paribas as a counterparty please".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["BNP Paribas".into()],
                tags: vec![
                    "counterparty".into(),
                    "edge_case".into(),
                    "conversational".into(),
                ],
                description: Some("Conversational style".into()),
            },
            PromptTestCase {
                id: "edge_03".into(),
                prompt: "counterparty add: HSBC".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["HSBC".into()],
                tags: vec!["counterparty".into(), "edge_case".into(), "terse".into()],
                description: Some("Terse/command style".into()),
            },
            // Expected failures (should parse as unknown)
            PromptTestCase {
                id: "unknown_01".into(),
                prompt: "hello world".into(),
                expected_intent: Some("unknown".into()),
                expected_entities: vec![],
                tags: vec!["unknown".into(), "negative".into()],
                description: Some("Irrelevant input".into()),
            },
            PromptTestCase {
                id: "unknown_02".into(),
                prompt: "what is the weather today".into(),
                expected_intent: Some("unknown".into()),
                expected_entities: vec![],
                tags: vec!["unknown".into(), "negative".into()],
                description: Some("Off-topic query".into()),
            },
        ]
    }
}

/// Get the intent type name from an IntentAst.
fn intent_type_name(ast: &IntentAst) -> String {
    match ast {
        IntentAst::CounterpartyCreate { .. } => "counterparty_create".into(),
        IntentAst::IsdaEstablish { .. } => "isda_establish".into(),
        IntentAst::CsaAdd { .. } => "csa_add".into(),
        IntentAst::IsdaAddCoverage { .. } => "isda_add_coverage".into(),
        IntentAst::UniverseAdd { .. } => "universe_add".into(),
        IntentAst::SsiCreate { .. } => "ssi_create".into(),
        IntentAst::BookingRuleAdd { .. } => "booking_rule_add".into(),
        IntentAst::RoleAssign { .. } => "role_assign".into(),
        IntentAst::RoleRemove { .. } => "role_remove".into(),
        IntentAst::EntityCreate { .. } => "entity_create".into(),
        IntentAst::ProductAdd { .. } => "product_add".into(),
        IntentAst::ServiceProvision { .. } => "service_provision".into(),
        IntentAst::EntityList { .. } => "entity_list".into(),
        IntentAst::EntityShow { .. } => "entity_show".into(),
        IntentAst::CounterpartyList { .. } => "counterparty_list".into(),
        IntentAst::IsdaShow { .. } => "isda_show".into(),
        IntentAst::Unknown { .. } => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lexicon() -> Lexicon {
        use super::super::loader::{
            EntitiesConfig, InstrumentsConfig, LexiconConfig, PrepositionsConfig, VerbsConfig,
        };

        Lexicon::from_config(LexiconConfig {
            verbs: VerbsConfig {
                create: vec!["add".into(), "create".into(), "establish".into()],
                query: vec!["show".into(), "list".into(), "what".into()],
                link: vec!["assign".into()],
                ..Default::default()
            },
            entities: EntitiesConfig {
                counterparty: vec!["counterparty".into()],
                isda: vec!["isda".into()],
                csa: vec!["csa".into()],
                ..Default::default()
            },
            instruments: InstrumentsConfig {
                otc: vec!["irs".into(), "cds".into()],
                exchange_traded: vec!["equities".into(), "bonds".into()],
            },
            prepositions: PrepositionsConfig {
                as_: vec!["as".into()],
                for_: vec!["for".into()],
                with: vec!["with".into()],
                under: vec!["under".into()],
                to: vec!["to".into()],
                ..Default::default()
            },
            roles: vec!["director".into(), "ubo".into()],
            markets: vec!["nyse".into(), "lse".into()],
            articles: vec!["a".into(), "an".into(), "the".into()],
            ..Default::default()
        })
        .unwrap()
    }

    #[tokio::test]
    async fn test_harness_basic() {
        let harness = TestHarness::new(test_lexicon());

        let test_cases = vec![PromptTestCase {
            id: "test_01".into(),
            prompt: "add Goldman as counterparty".into(),
            expected_intent: Some("counterparty_create".into()),
            expected_entities: vec![],
            tags: vec!["counterparty".into()],
            description: None,
        }];

        let results = harness.run_batch(&test_cases).await;

        assert_eq!(results.total, 1);
        println!("{}", results.summary());
    }

    #[tokio::test]
    async fn test_harness_standard_cases() {
        let harness = TestHarness::new(test_lexicon());
        let test_cases = TestHarness::standard_test_cases();

        let results = harness.run_batch(&test_cases).await;

        println!("{}", results.summary());

        // We expect some failures due to missing entity resolution
        // but the harness should complete without panicking
        assert!(results.total > 0);
    }

    #[tokio::test]
    async fn test_harness_with_mock_entities() {
        let harness = TestHarness::new(test_lexicon()).with_mock_entities(
            [
                (
                    "goldman".into(),
                    ResolvedEntity {
                        id: "uuid-goldman".into(),
                        name: "Goldman Sachs".into(),
                        entity_type: "counterparty".into(),
                        confidence: 0.95,
                    },
                ),
                (
                    "barclays".into(),
                    ResolvedEntity {
                        id: "uuid-barclays".into(),
                        name: "Barclays".into(),
                        entity_type: "counterparty".into(),
                        confidence: 0.95,
                    },
                ),
            ]
            .into(),
        );

        let test_cases = vec![
            PromptTestCase {
                id: "mock_01".into(),
                prompt: "add Goldman as counterparty".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["Goldman".into()],
                tags: vec!["mock".into()],
                description: None,
            },
            PromptTestCase {
                id: "mock_02".into(),
                prompt: "add Barclays as counterparty for IRS".into(),
                expected_intent: Some("counterparty_create".into()),
                expected_entities: vec!["Barclays".into()],
                tags: vec!["mock".into()],
                description: None,
            },
        ];

        let results = harness.run_batch(&test_cases).await;
        println!("{}", results.summary());

        // With mock entities, these should pass
        assert_eq!(results.passed, 2);
    }
}
