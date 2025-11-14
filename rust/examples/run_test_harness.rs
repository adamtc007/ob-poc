//! Run the complete test harness for the agentic DSL CRUD system
//!
//! Usage:
//!   cargo run --example run_test_harness --features database
//!
//! This will execute all canned test scenarios and display results.

use ob_poc::test_harness::{TestDetail, TestHarness, TestResults};
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 AGENTIC DSL CRUD - END-TO-END TEST HARNESS");
    println!("{}", "=".repeat(80));
    println!();

    // Get base URL from environment or use default
    let base_url =
        std::env::var("API_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    println!("📡 API Base URL: {}", base_url);
    println!();

    // Create HTTP client
    let client = reqwest::Client::new();

    // Check if API is accessible
    match client.get(format!("{}/api/health", base_url)).send().await {
        Ok(response) if response.status().is_success() => {
            println!("✅ API server is accessible");
        }
        Ok(response) => {
            eprintln!("❌ API server returned status: {}", response.status());
            eprintln!(
                "   Please start the server with: cargo run --bin agentic_server --features server"
            );
            return Ok(());
        }
        Err(e) => {
            eprintln!("❌ Cannot connect to API server: {}", e);
            eprintln!(
                "   Please start the server with: cargo run --bin agentic_server --features server"
            );
            return Ok(());
        }
    }
    println!();

    // Get all test scenarios
    let scenarios = TestHarness::get_test_scenarios();
    println!("📋 Running {} test scenarios\n", scenarios.len());

    let mut total_success = 0;
    let mut total_failed = 0;
    let start_time = Instant::now();

    // Run each scenario
    for (idx, scenario) in scenarios.iter().enumerate() {
        println!("{}. {} - {}", idx + 1, scenario.name, scenario.description);
        println!("   Prompts: {}", scenario.prompts.len());

        let results = run_scenario(&client, &base_url, scenario).await;

        total_success += results.successful;
        total_failed += results.failed;

        // Display results
        if results.failed == 0 {
            println!("   ✅ All prompts successful");
        } else {
            println!(
                "   ⚠️  {} successful, {} failed",
                results.successful, results.failed
            );
        }

        // Show details for failures
        for detail in &results.details {
            if !detail.success {
                println!("      ❌ {}", detail.prompt);
                println!("         {}", detail.message);
            }
        }

        println!();
    }

    let duration = start_time.elapsed();
    println!("{}", "=".repeat(80));
    println!("🏁 Test Harness Complete");
    println!(
        "   Total: {} successful, {} failed",
        total_success, total_failed
    );
    println!("   Duration: {:.2}s", duration.as_secs_f64());

    if total_failed == 0 {
        println!("   ✅ All tests passed!");
    } else {
        println!("   ⚠️  Some tests failed");
    }

    Ok(())
}

/// Run a single test scenario
async fn run_scenario(
    client: &reqwest::Client,
    base_url: &str,
    scenario: &ob_poc::test_harness::TestScenario,
) -> TestResults {
    let mut results = TestResults {
        scenario_name: scenario.name.clone(),
        total_prompts: scenario.prompts.len(),
        successful: 0,
        failed: 0,
        details: vec![],
    };

    for prompt in &scenario.prompts {
        match execute_prompt(client, base_url, prompt).await {
            Ok((entity_id, message)) => {
                results.successful += 1;
                results.details.push(TestDetail {
                    prompt: prompt.clone(),
                    success: true,
                    message,
                    entity_id: Some(entity_id),
                });
            }
            Err(e) => {
                results.failed += 1;
                results.details.push(TestDetail {
                    prompt: prompt.clone(),
                    success: false,
                    message: e,
                    entity_id: None,
                });
            }
        }
    }

    results
}

/// Execute a single prompt against the API
async fn execute_prompt(
    client: &reqwest::Client,
    base_url: &str,
    prompt: &str,
) -> Result<(Uuid, String), String> {
    let request = json!({
        "prompt": prompt
    });

    let response = client
        .post(format!("{}/api/agentic/execute", base_url))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP {}: {}",
            response.status(),
            response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string())
        ));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Extract entity_id from response
    let entity_id = result
        .get("entity_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| "No entity_id in response".to_string())?;

    let message = result
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Success")
        .to_string();

    Ok((entity_id, message))
}
