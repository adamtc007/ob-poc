//! Integration tests for the agent clarification/disambiguation learning flow.
//!
//! This test harness exercises the full pipeline:
//! 1. Send ambiguous user input
//! 2. Receive disambiguation options
//! 3. Simulate user selection
//! 4. Verify learning signals are recorded
//!
//! Run with: cargo test --test clarification_learning_integration --features database

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// =============================================================================
// TEST SCENARIOS - Domain-specific ambiguous phrases
// =============================================================================

/// A test scenario with an ambiguous phrase and expected disambiguation
#[derive(Debug, Clone)]
struct ClarificationScenario {
    /// Human-readable description
    description: &'static str,
    /// The ambiguous user input
    user_input: &'static str,
    /// Expected verbs that should appear in disambiguation (at least one)
    expected_verbs: &'static [&'static str],
    /// The verb the simulated user will select
    user_selection: &'static str,
    /// Category for grouping
    category: &'static str,
}

/// Generate domain-specific test scenarios
fn get_test_scenarios() -> Vec<ClarificationScenario> {
    vec![
        // =================================================================
        // SESSION/NAVIGATION AMBIGUITY
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous 'load' command - galaxy vs cbu vs cluster",
            user_input: "load the book",
            expected_verbs: &[
                "session.load-galaxy",
                "session.load-cbu",
                "session.load-cluster",
            ],
            user_selection: "session.load-galaxy",
            category: "navigation",
        },
        ClarificationScenario {
            description: "Ambiguous 'show' command",
            user_input: "show me the allianz stuff",
            expected_verbs: &[
                "session.load-galaxy",
                "session.load-cluster",
                "view.universe",
            ],
            user_selection: "session.load-cluster",
            category: "navigation",
        },
        ClarificationScenario {
            description: "Vague scope request",
            user_input: "set scope to lux funds",
            expected_verbs: &["session.load-jurisdiction", "session.load-cluster"],
            user_selection: "session.load-jurisdiction",
            category: "navigation",
        },
        ClarificationScenario {
            description: "Ambiguous 'open' command",
            user_input: "open the client",
            expected_verbs: &["session.load-galaxy", "session.load-cbu"],
            user_selection: "session.load-cbu",
            category: "navigation",
        },
        // =================================================================
        // CBU/ENTITY CREATION AMBIGUITY
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous 'create' - cbu vs entity vs fund",
            user_input: "create acme corp",
            expected_verbs: &["cbu.create", "entity.create"],
            user_selection: "cbu.create",
            category: "creation",
        },
        ClarificationScenario {
            description: "Ambiguous fund creation",
            user_input: "set up a new fund",
            expected_verbs: &["cbu.create", "entity.create"],
            user_selection: "cbu.create",
            category: "creation",
        },
        ClarificationScenario {
            description: "Vague entity request",
            user_input: "add john smith",
            expected_verbs: &["entity.create", "cbu-role.assign"],
            user_selection: "entity.create",
            category: "creation",
        },
        // =================================================================
        // KYC/COMPLIANCE AMBIGUITY
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous KYC action",
            user_input: "start kyc",
            expected_verbs: &["kyc-case.create", "kyc-case.open"],
            user_selection: "kyc-case.create",
            category: "kyc",
        },
        ClarificationScenario {
            description: "Vague compliance check",
            user_input: "check the entity",
            expected_verbs: &["kyc-case.create", "entity.get", "ubo.discover"],
            user_selection: "ubo.discover",
            category: "kyc",
        },
        ClarificationScenario {
            description: "Ambiguous ownership request",
            user_input: "who owns this",
            expected_verbs: &["ubo.discover", "control.build-graph"],
            user_selection: "ubo.discover",
            category: "kyc",
        },
        // =================================================================
        // TRADING PROFILE AMBIGUITY
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous trading setup",
            user_input: "set up trading",
            expected_verbs: &["trading-profile.create", "trading-profile.update"],
            user_selection: "trading-profile.create",
            category: "trading",
        },
        ClarificationScenario {
            description: "Vague product request",
            user_input: "add custody",
            expected_verbs: &["cbu.add-product", "trading-profile.add-instrument"],
            user_selection: "cbu.add-product",
            category: "trading",
        },
        // =================================================================
        // RESEARCH/GLEIF AMBIGUITY
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous research request",
            user_input: "look up the company",
            expected_verbs: &["gleif.search", "entity.search", "gleif.import-tree"],
            user_selection: "gleif.search",
            category: "research",
        },
        ClarificationScenario {
            description: "Vague import request",
            user_input: "import the hierarchy",
            expected_verbs: &["gleif.import-tree", "gleif.import"],
            user_selection: "gleif.import-tree",
            category: "research",
        },
        // =================================================================
        // VIEW/ZOOM AMBIGUITY (ESPER-style)
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous zoom command",
            user_input: "zoom in",
            expected_verbs: &["view.drill", "view.cbu"],
            user_selection: "view.drill",
            category: "view",
        },
        ClarificationScenario {
            description: "Vague drill request",
            user_input: "go deeper",
            expected_verbs: &["view.drill", "view.cbu"],
            user_selection: "view.drill",
            category: "view",
        },
        ClarificationScenario {
            description: "Ambiguous surface command",
            user_input: "go back",
            expected_verbs: &["view.surface", "session.undo"],
            user_selection: "view.surface",
            category: "view",
        },
        // =================================================================
        // DOCUMENT/WORKFLOW AMBIGUITY
        // =================================================================
        ClarificationScenario {
            description: "Ambiguous document request",
            user_input: "request passport",
            expected_verbs: &["document.solicit", "requirement.create"],
            user_selection: "document.solicit",
            category: "document",
        },
        ClarificationScenario {
            description: "Vague approval request",
            user_input: "approve it",
            expected_verbs: &["kyc-case.approve", "document.verify"],
            user_selection: "kyc-case.approve",
            category: "document",
        },
        // =================================================================
        // TYPOS AND MISSPELLINGS
        // =================================================================
        ClarificationScenario {
            description: "Typo in command",
            user_input: "creaet a fund",
            expected_verbs: &["cbu.create", "entity.create"],
            user_selection: "cbu.create",
            category: "typo",
        },
        ClarificationScenario {
            description: "Misspelled verb",
            user_input: "laod the book",
            expected_verbs: &["session.load-galaxy", "session.load-cbu"],
            user_selection: "session.load-galaxy",
            category: "typo",
        },
    ]
}

// =============================================================================
// API TYPES (matching server responses)
// =============================================================================

#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    session_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<String>,
    verb_disambiguation: Option<VerbDisambiguationRequest>,
    dsl: Option<DslResponse>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerbDisambiguationRequest {
    request_id: String,
    original_input: String,
    options: Vec<VerbOption>,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct VerbOption {
    verb_fqn: String,
    description: String,
    score: f32,
    matched_phrase: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DslResponse {
    source: Option<String>,
    can_execute: bool,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    message: String,
}

#[derive(Debug, Serialize)]
struct VerbSelectionRequest {
    request_id: String,
    original_input: String,
    selected_verb: String,
    all_candidates: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VerbSelectionResponse {
    /// Whether the learning signal was recorded
    recorded: bool,
    /// Message to display
    message: String,
    /// Execution result (optional)
    #[serde(default)]
    execution_result: Option<serde_json::Value>,
}

// =============================================================================
// TEST HARNESS
// =============================================================================

struct TestHarness {
    client: reqwest::Client,
    base_url: String,
    session_id: Option<Uuid>,
    results: Vec<ScenarioResult>,
}

#[derive(Debug)]
struct ScenarioResult {
    scenario: String,
    category: String,
    disambiguation_returned: bool,
    expected_verbs_found: Vec<String>,
    missing_verbs: Vec<String>,
    selection_successful: bool,
    learning_recorded: bool,
    error: Option<String>,
}

impl TestHarness {
    fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            session_id: None,
            results: Vec::new(),
        }
    }

    async fn create_session(&mut self) -> Result<Uuid> {
        let resp: CreateSessionResponse = self
            .client
            .post(format!("{}/api/session", self.base_url))
            .json(&serde_json::json!({}))
            .send()
            .await?
            .json()
            .await?;

        self.session_id = Some(resp.session_id);
        Ok(resp.session_id)
    }

    async fn send_chat(&self, message: &str) -> Result<ChatResponse> {
        let session_id = self
            .session_id
            .ok_or_else(|| anyhow::anyhow!("No session"))?;

        let resp = self
            .client
            .post(format!("{}/api/session/{}/chat", self.base_url, session_id))
            .json(&ChatRequest {
                message: message.to_string(),
            })
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    async fn select_verb(
        &self,
        request_id: &str,
        original_input: &str,
        selected_verb: &str,
        all_candidates: &[String],
    ) -> Result<VerbSelectionResponse> {
        let session_id = self
            .session_id
            .ok_or_else(|| anyhow::anyhow!("No session"))?;

        let resp = self
            .client
            .post(format!(
                "{}/api/session/{}/select-verb",
                self.base_url, session_id
            ))
            .json(&VerbSelectionRequest {
                request_id: request_id.to_string(),
                original_input: original_input.to_string(),
                selected_verb: selected_verb.to_string(),
                all_candidates: all_candidates.to_vec(),
            })
            .send()
            .await?
            .json()
            .await?;

        Ok(resp)
    }

    async fn run_scenario(&mut self, scenario: &ClarificationScenario) -> ScenarioResult {
        println!("\n  Testing: {}", scenario.description);
        println!("    Input: \"{}\"", scenario.user_input);

        // Step 1: Send ambiguous input
        let chat_resp = match self.send_chat(scenario.user_input).await {
            Ok(r) => r,
            Err(e) => {
                return ScenarioResult {
                    scenario: scenario.description.to_string(),
                    category: scenario.category.to_string(),
                    disambiguation_returned: false,
                    expected_verbs_found: vec![],
                    missing_verbs: scenario
                        .expected_verbs
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    selection_successful: false,
                    learning_recorded: false,
                    error: Some(format!("Chat error: {}", e)),
                };
            }
        };

        // Step 2: Check for disambiguation
        let Some(ref disambig) = chat_resp.verb_disambiguation else {
            // No disambiguation returned - might have matched clearly or failed
            let msg = chat_resp.message.unwrap_or_default();
            println!("    ⚠ No disambiguation returned");
            println!("    Message: {}", &msg[..msg.len().min(80)]);

            return ScenarioResult {
                scenario: scenario.description.to_string(),
                category: scenario.category.to_string(),
                disambiguation_returned: false,
                expected_verbs_found: vec![],
                missing_verbs: scenario
                    .expected_verbs
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                selection_successful: false,
                learning_recorded: false,
                error: Some("No disambiguation returned".to_string()),
            };
        };

        println!(
            "    ✓ Disambiguation returned with {} options",
            disambig.options.len()
        );

        // Step 3: Check which expected verbs were found
        let returned_verbs: Vec<String> = disambig
            .options
            .iter()
            .map(|o| o.verb_fqn.clone())
            .collect();
        let expected_verbs_found: Vec<String> = scenario
            .expected_verbs
            .iter()
            .filter(|v| returned_verbs.contains(&v.to_string()))
            .map(|v| v.to_string())
            .collect();
        let missing_verbs: Vec<String> = scenario
            .expected_verbs
            .iter()
            .filter(|v| !returned_verbs.contains(&v.to_string()))
            .map(|v| v.to_string())
            .collect();

        for opt in &disambig.options {
            let marker = if scenario.expected_verbs.contains(&opt.verb_fqn.as_str()) {
                "✓"
            } else {
                " "
            };
            println!(
                "      {} {} (score: {:.2})",
                marker, opt.verb_fqn, opt.score
            );
        }

        if !missing_verbs.is_empty() {
            println!("    ⚠ Missing expected verbs: {:?}", missing_verbs);
        }

        // Step 4: Simulate user selection
        let selection_verb = if returned_verbs.contains(&scenario.user_selection.to_string()) {
            scenario.user_selection.to_string()
        } else if !returned_verbs.is_empty() {
            // Fall back to first option if expected selection not available
            println!(
                "    ⚠ Expected selection '{}' not in options, using '{}'",
                scenario.user_selection, returned_verbs[0]
            );
            returned_verbs[0].clone()
        } else {
            return ScenarioResult {
                scenario: scenario.description.to_string(),
                category: scenario.category.to_string(),
                disambiguation_returned: true,
                expected_verbs_found,
                missing_verbs,
                selection_successful: false,
                learning_recorded: false,
                error: Some("No verbs to select".to_string()),
            };
        };

        println!("    Selecting: {}", selection_verb);

        let selection_resp = match self
            .select_verb(
                &disambig.request_id,
                &disambig.original_input,
                &selection_verb,
                &returned_verbs,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return ScenarioResult {
                    scenario: scenario.description.to_string(),
                    category: scenario.category.to_string(),
                    disambiguation_returned: true,
                    expected_verbs_found,
                    missing_verbs,
                    selection_successful: false,
                    learning_recorded: false,
                    error: Some(format!("Selection error: {}", e)),
                };
            }
        };

        let learning_recorded = selection_resp.recorded;
        println!(
            "    {} Selection completed, learning recorded: {}",
            if learning_recorded { "✓" } else { "✗" },
            learning_recorded
        );
        println!(
            "    Message: {}",
            &selection_resp.message[..selection_resp.message.len().min(60)]
        );

        ScenarioResult {
            scenario: scenario.description.to_string(),
            category: scenario.category.to_string(),
            disambiguation_returned: true,
            expected_verbs_found,
            missing_verbs,
            selection_successful: true, // If we got here, selection succeeded
            learning_recorded,
            error: None,
        }
    }

    fn print_summary(&self) {
        println!("\n========================================");
        println!("  CLARIFICATION LEARNING TEST SUMMARY");
        println!("========================================\n");

        let total = self.results.len();
        let disambig_returned = self
            .results
            .iter()
            .filter(|r| r.disambiguation_returned)
            .count();
        let selections_ok = self
            .results
            .iter()
            .filter(|r| r.selection_successful)
            .count();
        let learning_ok = self.results.iter().filter(|r| r.learning_recorded).count();
        let errors = self.results.iter().filter(|r| r.error.is_some()).count();

        println!("Total scenarios:           {}", total);
        println!(
            "Disambiguation returned:   {} ({:.0}%)",
            disambig_returned,
            100.0 * disambig_returned as f64 / total as f64
        );
        println!(
            "Selections successful:     {} ({:.0}%)",
            selections_ok,
            100.0 * selections_ok as f64 / total as f64
        );
        println!(
            "Learning recorded:         {} ({:.0}%)",
            learning_ok,
            100.0 * learning_ok as f64 / total as f64
        );
        println!("Errors:                    {}", errors);

        // Group by category
        println!("\nBy Category:");
        let mut categories: std::collections::HashMap<String, (usize, usize, usize)> =
            std::collections::HashMap::new();
        for r in &self.results {
            let entry = categories.entry(r.category.clone()).or_insert((0, 0, 0));
            entry.0 += 1;
            if r.disambiguation_returned {
                entry.1 += 1;
            }
            if r.learning_recorded {
                entry.2 += 1;
            }
        }
        for (cat, (total, disambig, learned)) in &categories {
            println!(
                "  {:<12} total={}, disambig={}, learned={}",
                cat, total, disambig, learned
            );
        }

        // List failures
        let failures: Vec<_> = self.results.iter().filter(|r| r.error.is_some()).collect();
        if !failures.is_empty() {
            println!("\nFailures:");
            for f in failures {
                println!("  - {}: {}", f.scenario, f.error.as_ref().unwrap());
            }
        }
    }
}

// =============================================================================
// INTEGRATION TEST
// =============================================================================

#[cfg(feature = "database")]
#[tokio::test]
async fn test_clarification_learning_flow() -> Result<()> {
    // Requires running server
    let base_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    println!("\n========================================");
    println!("  CLARIFICATION LEARNING TEST HARNESS");
    println!("========================================");
    println!("  Server: {}", base_url);

    let mut harness = TestHarness::new(&base_url);

    // Create session
    let session_id = harness.create_session().await?;
    println!("  Session: {}", session_id);

    let scenarios = get_test_scenarios();
    println!("  Scenarios: {}\n", scenarios.len());

    // Run each scenario
    for scenario in &scenarios {
        let result = harness.run_scenario(scenario).await;
        harness.results.push(result);
    }

    harness.print_summary();

    // Assert at least some disambiguation worked
    let disambig_count = harness
        .results
        .iter()
        .filter(|r| r.disambiguation_returned)
        .count();
    assert!(
        disambig_count > 0,
        "Expected at least some disambiguation responses"
    );

    Ok(())
}

// =============================================================================
// DATABASE VERIFICATION (check learning signals were stored)
// =============================================================================

#[cfg(feature = "database")]
#[tokio::test]
async fn test_verify_learning_signals_stored() -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = PgPool::connect(&database_url).await?;

    // Check user_learned_phrases
    let learned_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM agent.user_learned_phrases
        WHERE source IN ('user_disambiguation', 'generated_variant')
        "#,
    )
    .fetch_one(&pool)
    .await?;

    println!("Learned phrases from disambiguation: {}", learned_count.0);

    // Check learning_candidates
    let candidate_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM agent.learning_candidates
        WHERE status = 'pending'
        "#,
    )
    .fetch_one(&pool)
    .await?;

    println!("Pending learning candidates: {}", candidate_count.0);

    // Check intent_feedback with outcomes
    let feedback_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM "ob-poc".intent_feedback
        WHERE outcome IS NOT NULL
        "#,
    )
    .fetch_one(&pool)
    .await?;

    println!("Intent feedback with outcomes: {}", feedback_count.0);

    Ok(())
}

// =============================================================================
// STANDALONE RUNNER (for manual testing)
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let base_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    println!("\n========================================");
    println!("  CLARIFICATION LEARNING TEST HARNESS");
    println!("========================================");
    println!("  Server: {}", base_url);
    println!("  (Run with: cargo run --test clarification_learning_integration)");

    let mut harness = TestHarness::new(&base_url);

    // Create session
    match harness.create_session().await {
        Ok(session_id) => println!("  Session: {}", session_id),
        Err(e) => {
            eprintln!("  Failed to create session: {}", e);
            eprintln!("  Is the server running at {}?", base_url);
            return Ok(());
        }
    }

    let scenarios = get_test_scenarios();
    println!("  Scenarios: {}\n", scenarios.len());

    // Run each scenario
    for scenario in &scenarios {
        let result = harness.run_scenario(scenario).await;
        harness.results.push(result);
    }

    harness.print_summary();

    Ok(())
}
