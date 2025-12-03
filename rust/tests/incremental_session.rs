//! Comprehensive integration tests for incremental DSL session management
//!
//! Test categories:
//! 1. Happy path - normal session lifecycle
//! 2. Error handling - validation errors, execution failures
//! 3. Edge cases - abort, timeout, corruption, recovery
//! 4. Concurrency - parallel sessions, race conditions
//! 5. Domain detection - CBU, KYC, Onboarding context
//! 6. Idempotency - re-execution produces same results
//!
//! Run with: cargo test --features database --test incremental_session -- --test-threads=1

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

const API_URL: &str = "http://127.0.0.1:3000";

// =============================================================================
// TEST INFRASTRUCTURE
// =============================================================================

#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct ExecuteResponse {
    success: bool,
    results: Vec<Value>,
    errors: Vec<String>,
    bindings: Option<HashMap<String, Uuid>>,
}

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    valid: bool,
    errors: Vec<String>,
}

struct TestSession {
    client: Client,
    session_id: String,
    test_name: String,
}

impl TestSession {
    async fn new(test_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        let resp = client
            .post(format!("{}/api/session", API_URL))
            .json(&serde_json::json!({
                "client_type": "fund",
                "jurisdiction": "LU"
            }))
            .send()
            .await?;
        
        if !resp.status().is_success() {
            return Err(format!("Failed to create session: {}", resp.status()).into());
        }
        
        let session: CreateSessionResponse = resp.json().await?;
        
        Ok(Self {
            client,
            session_id: session.session_id,
            test_name: test_name.to_string(),
        })
    }
    
    async fn execute(&self, dsl: &str) -> Result<ExecuteResponse, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .post(format!("{}/api/session/{}/execute", API_URL, self.session_id))
            .json(&serde_json::json!({ "dsl": dsl }))
            .send()
            .await?;
        
        let result: ExecuteResponse = resp.json().await?;
        Ok(result)
    }
    
    async fn execute_with_timeout(&self, dsl: &str, timeout_secs: u64) -> Result<ExecuteResponse, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;
            
        let resp = client
            .post(format!("{}/api/session/{}/execute", API_URL, self.session_id))
            .json(&serde_json::json!({ "dsl": dsl }))
            .send()
            .await?;
        
        let result: ExecuteResponse = resp.json().await?;
        Ok(result)
    }
    
    fn unique_name(&self, base: &str) -> String {
        format!("{} {} {}", base, self.test_name, Uuid::new_v4().to_string()[..8].to_string())
    }
}

// =============================================================================
// 1. HAPPY PATH TESTS
// =============================================================================

#[tokio::test]
async fn test_01_session_create_and_execute() {
    let session = TestSession::new("test01").await.expect("Failed to create session");
    let name = session.unique_name("Basic Fund");
    
    let result = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await
        .expect("Execute failed");
    
    assert!(result.success, "Should succeed: {:?}", result.errors);
    assert!(result.bindings.is_some(), "Should return bindings");
    assert!(result.bindings.as_ref().unwrap().contains_key("cbu"), "Should have @cbu");
    
    println!("✓ test_01: Session create and execute works");
}

#[tokio::test]
async fn test_02_binding_persistence_across_executions() {
    let session = TestSession::new("test02").await.expect("Failed to create session");
    let name = session.unique_name("Binding Fund");
    
    // Step 1: Create CBU
    let r1 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await
        .expect("Step 1 failed");
    assert!(r1.success, "Step 1: {:?}", r1.errors);
    let cbu_id = r1.bindings.as_ref().unwrap().get("cbu").copied();
    
    // Step 2: Use @cbu from step 1
    let r2 = session
        .execute(r#"(entity.create-limited-company :name "Holdings Ltd" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")"#)
        .await
        .expect("Step 2 failed");
    assert!(r2.success, "Step 2 should use @cbu from step 1: {:?}", r2.errors);
    
    println!("✓ test_02: Binding persistence works");
}

#[tokio::test]
async fn test_03_multiple_bindings_accumulated() {
    let session = TestSession::new("test03").await.expect("Failed to create session");
    let name = session.unique_name("Multi Fund");
    
    // Create multiple entities across executions
    let r1 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    assert!(r1.success);
    
    let r2 = session
        .execute(r#"(entity.create-limited-company :name "Company A" :jurisdiction "LU" :as @companyA)"#)
        .await.unwrap();
    assert!(r2.success);
    
    let r3 = session
        .execute(r#"(entity.create-limited-company :name "Company B" :jurisdiction "LU" :as @companyB)"#)
        .await.unwrap();
    assert!(r3.success);
    
    // Now use all three bindings
    let r4 = session
        .execute(r#"(cbu.assign-role :cbu-id @cbu :entity-id @companyA :role "PRINCIPAL")
(cbu.assign-role :cbu-id @cbu :entity-id @companyB :role "SHAREHOLDER")"#)
        .await.unwrap();
    assert!(r4.success, "Should resolve @cbu, @companyA, @companyB: {:?}", r4.errors);
    
    println!("✓ test_03: Multiple bindings accumulated");
}

#[tokio::test]
async fn test_04_idempotent_cbu_ensure() {
    let session = TestSession::new("test04").await.expect("Failed to create session");
    let name = session.unique_name("Idempotent Fund");
    let dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name);
    
    let r1 = session.execute(&dsl).await.unwrap();
    let id1 = r1.bindings.as_ref().unwrap().get("cbu").copied();
    
    let r2 = session.execute(&dsl).await.unwrap();
    let id2 = r2.bindings.as_ref().unwrap().get("cbu").copied();
    
    let r3 = session.execute(&dsl).await.unwrap();
    let id3 = r3.bindings.as_ref().unwrap().get("cbu").copied();
    
    assert_eq!(id1, id2, "Same ID on re-execute");
    assert_eq!(id2, id3, "Same ID on third execute");
    
    println!("✓ test_04: Idempotent ensure returns same ID");
}

#[tokio::test]
async fn test_05_full_onboarding_flow() {
    let session = TestSession::new("test05").await.expect("Failed to create session");
    let name = session.unique_name("Full Flow Fund");
    
    // Step by step onboarding
    let steps = vec![
        format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name),
        r#"(entity.create-limited-company :name "HoldCo SARL" :jurisdiction "LU" :as @company)"#.to_string(),
        r#"(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)"#.to_string(),
        r#"(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")"#.to_string(),
        r#"(cbu.assign-role :cbu-id @cbu :entity-id @john :role "DIRECTOR")"#.to_string(),
        r#"(cbu.assign-role :cbu-id @cbu :entity-id @john :role "BENEFICIAL_OWNER" :ownership-percentage 100)"#.to_string(),
        r#"(kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)"#.to_string(),
        r#"(entity-workstream.create :case-id @case :entity-id @john :is-ubo true :as @ws)"#.to_string(),
    ];
    
    for (i, dsl) in steps.iter().enumerate() {
        let result = session.execute(dsl).await.expect(&format!("Step {} failed", i + 1));
        assert!(result.success, "Step {} should succeed: {:?}", i + 1, result.errors);
    }
    
    println!("✓ test_05: Full onboarding flow (8 steps) works");
}

// =============================================================================
// 2. ERROR HANDLING TESTS
// =============================================================================

#[tokio::test]
async fn test_10_invalid_dsl_syntax() {
    let session = TestSession::new("test10").await.expect("Failed to create session");
    
    let result = session.execute(r#"(this is not valid dsl"#).await.unwrap();
    
    assert!(!result.success, "Should fail on invalid syntax");
    assert!(!result.errors.is_empty(), "Should have error message");
    
    println!("✓ test_10: Invalid DSL syntax handled");
}

#[tokio::test]
async fn test_11_unknown_verb() {
    let session = TestSession::new("test11").await.expect("Failed to create session");
    
    let result = session.execute(r#"(nonexistent.verb :arg "value")"#).await.unwrap();
    
    assert!(!result.success, "Should fail on unknown verb");
    
    println!("✓ test_11: Unknown verb handled");
}

#[tokio::test]
async fn test_12_unresolved_reference() {
    let session = TestSession::new("test12").await.expect("Failed to create session");
    
    // Try to use @undefined without defining it
    let result = session
        .execute(r#"(cbu.assign-role :cbu-id @undefined :entity-id @also-undefined :role "DIRECTOR")"#)
        .await.unwrap();
    
    assert!(!result.success, "Should fail on unresolved reference");
    assert!(result.errors.iter().any(|e| e.contains("nresolved") || e.contains("undefined")), 
            "Error should mention unresolved: {:?}", result.errors);
    
    println!("✓ test_12: Unresolved reference handled");
}

#[tokio::test]
async fn test_13_missing_required_arg() {
    let session = TestSession::new("test13").await.expect("Failed to create session");
    
    // cbu.ensure requires :name
    let result = session.execute(r#"(cbu.ensure :jurisdiction "LU")"#).await.unwrap();
    
    assert!(!result.success, "Should fail on missing required arg");
    
    println!("✓ test_13: Missing required arg handled");
}

#[tokio::test]
async fn test_14_error_recovery_continues_session() {
    let session = TestSession::new("test14").await.expect("Failed to create session");
    let name = session.unique_name("Recovery Fund");
    
    // Step 1: Success
    let r1 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    assert!(r1.success);
    
    // Step 2: Fail with bad DSL
    let r2 = session.execute(r#"(invalid.verb)"#).await.unwrap();
    assert!(!r2.success, "Should fail");
    
    // Step 3: Session should still work, @cbu still available
    let r3 = session
        .execute(r#"(entity.create-limited-company :name "Recovery Co" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")"#)
        .await.unwrap();
    assert!(r3.success, "Session should recover after error: {:?}", r3.errors);
    
    println!("✓ test_14: Error recovery - session continues after failure");
}

#[tokio::test]
async fn test_15_multiple_errors_tracked() {
    let session = TestSession::new("test15").await.expect("Failed to create session");
    
    // Multiple failures
    for i in 0..5 {
        let result = session.execute(&format!(r#"(bad.verb{} :x "y")"#, i)).await.unwrap();
        assert!(!result.success);
    }
    
    // Session should still accept valid DSL
    let name = session.unique_name("After Errors Fund");
    let result = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    assert!(result.success, "Session survives multiple errors: {:?}", result.errors);
    
    println!("✓ test_15: Multiple errors tracked, session survives");
}

// =============================================================================
// 3. EDGE CASES
// =============================================================================

#[tokio::test]
async fn test_20_empty_dsl() {
    let session = TestSession::new("test20").await.expect("Failed to create session");
    
    let result = session.execute("").await.unwrap();
    
    // Empty DSL should either succeed with no-op or fail gracefully
    // Either is acceptable
    println!("✓ test_20: Empty DSL handled (success={})", result.success);
}

#[tokio::test]
async fn test_21_whitespace_only_dsl() {
    let session = TestSession::new("test21").await.expect("Failed to create session");
    
    let result = session.execute("   \n\t\n   ").await.unwrap();
    
    println!("✓ test_21: Whitespace-only DSL handled (success={})", result.success);
}

#[tokio::test]
async fn test_22_comment_only_dsl() {
    let session = TestSession::new("test22").await.expect("Failed to create session");
    
    let result = session.execute(";; This is just a comment\n;; Nothing to execute").await.unwrap();
    
    println!("✓ test_22: Comment-only DSL handled (success={})", result.success);
}

#[tokio::test]
async fn test_23_very_long_dsl() {
    let session = TestSession::new("test23").await.expect("Failed to create session");
    let name = session.unique_name("Long Fund");
    
    // Build a long DSL with many statements
    let mut dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name);
    
    for i in 0..20 {
        dsl.push_str(&format!(r#"
(entity.create-limited-company :name "Company {}" :jurisdiction "LU" :as @company{})"#, i, i));
    }
    
    let result = session.execute(&dsl).await.unwrap();
    assert!(result.success, "Long DSL should execute: {:?}", result.errors);
    
    println!("✓ test_23: Very long DSL (21 statements) handled");
}

#[tokio::test]
async fn test_24_special_characters_in_names() {
    let session = TestSession::new("test24").await.expect("Failed to create session");
    
    let result = session
        .execute(r#"(cbu.ensure :name "Test & Co. (Luxembourg) S.à r.l." :jurisdiction "LU" :client-type "fund" :as @cbu)"#)
        .await.unwrap();
    
    assert!(result.success, "Special chars should work: {:?}", result.errors);
    
    println!("✓ test_24: Special characters in names handled");
}

#[tokio::test]
async fn test_25_unicode_in_names() {
    let session = TestSession::new("test25").await.expect("Failed to create session");
    
    let result = session
        .execute(r#"(cbu.ensure :name "日本ファンド株式会社" :jurisdiction "LU" :client-type "fund" :as @cbu)"#)
        .await.unwrap();
    
    assert!(result.success, "Unicode should work: {:?}", result.errors);
    
    println!("✓ test_25: Unicode in names handled");
}

#[tokio::test]
async fn test_26_binding_name_reuse() {
    let session = TestSession::new("test26").await.expect("Failed to create session");
    let name1 = session.unique_name("Fund A");
    let name2 = session.unique_name("Fund B");
    
    // Create first CBU as @cbu
    let r1 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name1))
        .await.unwrap();
    assert!(r1.success);
    let id1 = r1.bindings.as_ref().unwrap().get("cbu").copied();
    
    // Create second CBU, also bound to @cbu (should overwrite)
    let r2 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "IE" :client-type "fund" :as @cbu)"#, name2))
        .await.unwrap();
    assert!(r2.success);
    let id2 = r2.bindings.as_ref().unwrap().get("cbu").copied();
    
    // The binding should now point to the second CBU
    assert_ne!(id1, id2, "Different CBUs should have different IDs");
    
    println!("✓ test_26: Binding name reuse (overwrite) works");
}

// =============================================================================
// 4. CONCURRENT SESSIONS
// =============================================================================

#[tokio::test]
async fn test_30_parallel_sessions_isolated() {
    // Create two sessions
    let session1 = TestSession::new("test30a").await.expect("Session 1");
    let session2 = TestSession::new("test30b").await.expect("Session 2");
    
    let name1 = session1.unique_name("Parallel Fund 1");
    let name2 = session2.unique_name("Parallel Fund 2");
    
    // Execute in parallel
    let dsl1 = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name1);
    let dsl2 = format!(r#"(cbu.ensure :name "{}" :jurisdiction "IE" :client-type "fund" :as @cbu)"#, name2);
    
    let (r1, r2) = tokio::join!(
        session1.execute(&dsl1),
        session2.execute(&dsl2)
    );
    
    let r1 = r1.unwrap();
    let r2 = r2.unwrap();
    
    assert!(r1.success && r2.success, "Both should succeed");
    
    let id1 = r1.bindings.as_ref().unwrap().get("cbu");
    let id2 = r2.bindings.as_ref().unwrap().get("cbu");
    
    assert_ne!(id1, id2, "Different sessions should create different CBUs");
    
    println!("✓ test_30: Parallel sessions are isolated");
}

#[tokio::test]
async fn test_31_session_isolation_bindings() {
    let session1 = TestSession::new("test31a").await.expect("Session 1");
    let session2 = TestSession::new("test31b").await.expect("Session 2");
    
    let name = session1.unique_name("Isolated Fund");
    
    // Session 1 creates @cbu
    let r1 = session1
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    assert!(r1.success);
    
    // Session 2 should NOT see @cbu from session 1
    let r2 = session2
        .execute(r#"(cbu.assign-role :cbu-id @cbu :entity-id @cbu :role "DIRECTOR")"#)
        .await.unwrap();
    
    assert!(!r2.success, "Session 2 should not see session 1's bindings");
    
    println!("✓ test_31: Session bindings are isolated");
}

#[tokio::test]
async fn test_32_many_concurrent_sessions() {
    let mut handles = vec![];
    
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            let session = TestSession::new(&format!("test32_{}", i)).await.expect("Session");
            let name = session.unique_name(&format!("Concurrent Fund {}", i));
            
            let result = session
                .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
                .await;
            
            result.map(|r| r.success).unwrap_or(false)
        });
        handles.push(handle);
    }
    
    let results: Vec<bool> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap_or(false))
        .collect();
    
    let successes = results.iter().filter(|&&x| x).count();
    assert_eq!(successes, 10, "All 10 concurrent sessions should succeed");
    
    println!("✓ test_32: 10 concurrent sessions all succeed");
}

// =============================================================================
// 5. DOMAIN DETECTION TESTS
// =============================================================================

#[tokio::test]
async fn test_40_domain_detection_cbu() {
    let session = TestSession::new("test40").await.expect("Session");
    let name = session.unique_name("Domain CBU Fund");
    
    let result = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @fund)"#, name))
        .await.unwrap();
    
    assert!(result.success);
    assert!(result.bindings.as_ref().unwrap().contains_key("fund"));
    
    println!("✓ test_40: Domain detection - CBU captured");
}

#[tokio::test]
async fn test_41_domain_detection_kyc_case() {
    let session = TestSession::new("test41").await.expect("Session");
    let name = session.unique_name("Domain KYC Fund");
    
    // Create CBU first
    session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    
    // Create KYC case
    let result = session
        .execute(r#"(kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)"#)
        .await.unwrap();
    
    assert!(result.success, "KYC case: {:?}", result.errors);
    assert!(result.bindings.as_ref().unwrap().contains_key("case"));
    
    println!("✓ test_41: Domain detection - KYC case captured");
}

#[tokio::test]
async fn test_42_cross_domain_flow() {
    let session = TestSession::new("test42").await.expect("Session");
    let name = session.unique_name("Cross Domain Fund");
    
    // CBU domain
    let r1 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    assert!(r1.success);
    
    // Entity domain
    let r2 = session
        .execute(r#"(entity.create-proper-person :first-name "Alice" :last-name "Wonder" :date-of-birth "1990-05-20" :as @alice)"#)
        .await.unwrap();
    assert!(r2.success);
    
    // Back to CBU domain (role assignment)
    let r3 = session
        .execute(r#"(cbu.assign-role :cbu-id @cbu :entity-id @alice :role "DIRECTOR")"#)
        .await.unwrap();
    assert!(r3.success);
    
    // KYC domain
    let r4 = session
        .execute(r#"(kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)"#)
        .await.unwrap();
    assert!(r4.success);
    
    // Entity workstream domain
    let r5 = session
        .execute(r#"(entity-workstream.create :case-id @case :entity-id @alice :as @ws)"#)
        .await.unwrap();
    assert!(r5.success);
    
    println!("✓ test_42: Cross-domain flow works (CBU → Entity → KYC → Workstream)");
}

// =============================================================================
// 6. STRESS AND ROBUSTNESS
// =============================================================================

#[tokio::test]
async fn test_50_rapid_fire_executions() {
    let session = TestSession::new("test50").await.expect("Session");
    let name = session.unique_name("Rapid Fund");
    
    // Create CBU
    session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    
    // Rapid fire 20 entity creations
    for i in 0..20 {
        let result = session
            .execute(&format!(r#"(entity.create-limited-company :name "Rapid Co {}" :jurisdiction "LU" :as @company{})"#, i, i))
            .await.unwrap();
        assert!(result.success, "Rapid fire {} failed: {:?}", i, result.errors);
    }
    
    println!("✓ test_50: Rapid fire 20 executions succeed");
}

#[tokio::test]
async fn test_51_session_after_long_pause() {
    let session = TestSession::new("test51").await.expect("Session");
    let name = session.unique_name("Pause Fund");
    
    // Create CBU
    let r1 = session
        .execute(&format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "fund" :as @cbu)"#, name))
        .await.unwrap();
    assert!(r1.success);
    
    // Simulate pause (in real scenario this would be longer)
    sleep(Duration::from_millis(500)).await;
    
    // Continue - @cbu should still work
    let r2 = session
        .execute(r#"(entity.create-limited-company :name "Post Pause Co" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")"#)
        .await.unwrap();
    assert!(r2.success, "Session should work after pause: {:?}", r2.errors);
    
    println!("✓ test_51: Session works after pause");
}

#[tokio::test]
async fn test_52_invalid_session_id() {
    let client = Client::new();
    let fake_session_id = Uuid::new_v4();
    
    let resp = client
        .post(format!("{}/api/session/{}/execute", API_URL, fake_session_id))
        .json(&serde_json::json!({ "dsl": "(cbu.ensure :name \"Test\" :jurisdiction \"LU\")" }))
        .send()
        .await
        .expect("Request failed");
    
    // Should return error, not crash
    assert!(!resp.status().is_success() || {
        let result: ExecuteResponse = resp.json().await.unwrap();
        !result.success
    }, "Invalid session should fail gracefully");
    
    println!("✓ test_52: Invalid session ID handled gracefully");
}

// =============================================================================
// MAIN - Run with test summary
// =============================================================================

fn main() {
    println!("Run with: cargo test --features database --test incremental_session -- --test-threads=1");
}
