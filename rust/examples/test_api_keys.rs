//! API Key Test Script - Verify OpenAI and Gemini API Keys
//!
//! This example tests whether your API keys are properly set up and working.
//! It performs basic connectivity tests without making actual AI requests.
//!
//! ## Usage:
//! ```bash
//! # Set your API keys first:
//! export OPENAI_API_KEY="your-openai-api-key-here"
//! export GEMINI_API_KEY="your-gemini-api-key-here"
//!
//! # Run the test
//! cargo run --example test_api_keys
//! ```

use std::env;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🔑 API Key Test Script Starting");
    info!("🧪 Testing OpenAI and Gemini API key setup");

    let mut all_tests_passed = true;

    // Test 1: Check environment variables
    info!("\n📋 Step 1: Checking environment variables...");

    let openai_key = env::var("OPENAI_API_KEY");
    let gemini_key = env::var("GEMINI_API_KEY");

    match &openai_key {
        Ok(key) if !key.is_empty() => {
            let masked_key = mask_api_key(key);
            info!("✅ OPENAI_API_KEY found: {}", masked_key);
        }
        Ok(_) => {
            warn!("⚠️  OPENAI_API_KEY is set but empty");
            all_tests_passed = false;
        }
        Err(_) => {
            warn!("❌ OPENAI_API_KEY not found");
            all_tests_passed = false;
        }
    }

    match &gemini_key {
        Ok(key) if !key.is_empty() => {
            let masked_key = mask_api_key(key);
            info!("✅ GEMINI_API_KEY found: {}", masked_key);
        }
        Ok(_) => {
            warn!("⚠️  GEMINI_API_KEY is set but empty");
            all_tests_passed = false;
        }
        Err(_) => {
            warn!("❌ GEMINI_API_KEY not found");
            all_tests_passed = false;
        }
    }

    // Test 2: API Key format validation
    info!("\n🔍 Step 2: Validating API key formats...");

    if let Ok(key) = &openai_key {
        if validate_openai_key_format(key) {
            info!("✅ OpenAI API key format looks valid");
        } else {
            warn!("⚠️  OpenAI API key format may be invalid");
            all_tests_passed = false;
        }
    }

    if let Ok(key) = &gemini_key {
        if validate_gemini_key_format(key) {
            info!("✅ Gemini API key format looks valid");
        } else {
            warn!("⚠️  Gemini API key format may be invalid");
            all_tests_passed = false;
        }
    }

    // Test 3: Basic connectivity test (without making actual API calls)
    info!("\n🌐 Step 3: Basic connectivity preparation...");

    if openai_key.is_ok() {
        info!("🔗 OpenAI endpoint: https://api.openai.com/v1");
        info!("📝 Ready to test OpenAI GPT models");
    }

    if gemini_key.is_ok() {
        info!("🔗 Gemini endpoint: https://generativelanguage.googleapis.com");
        info!("📝 Ready to test Gemini Pro model");
    }

    // Test 4: Check for database URL (optional)
    info!("\n💾 Step 4: Checking database configuration (optional)...");

    match env::var("DATABASE_URL") {
        Ok(db_url) if !db_url.is_empty() => {
            info!(
                "✅ DATABASE_URL found: {}...",
                &db_url[..db_url.len().min(30)]
            );
            info!("📊 Full AI workflow with database will be available");
        }
        _ => {
            info!("ℹ️  DATABASE_URL not set (optional for AI demos)");
            info!("🎭 AI demos will run in mock mode without database");
        }
    }

    // Summary
    info!("\n📊 Test Summary:");
    if all_tests_passed {
        info!("🎉 All API key tests passed!");
        info!("🚀 You can now run the AI examples:");
        info!("   cargo run --example ai_dsl_onboarding_demo");
        info!("   cargo run --example simple_openai_dsl_demo");

        // Recommend which example to run based on available keys
        match (openai_key.is_ok(), gemini_key.is_ok()) {
            (true, true) => {
                info!("💡 Both APIs available - ai_dsl_onboarding_demo will use OpenAI first");
            }
            (true, false) => {
                info!("💡 OpenAI available - perfect for simple_openai_dsl_demo");
            }
            (false, true) => {
                info!("💡 Gemini available - ai_dsl_onboarding_demo will use Gemini");
            }
            (false, false) => {
                // This shouldn't happen if all_tests_passed is true, but just in case
                info!("🎭 No API keys - mock_openai_demo is perfect for you");
            }
        }
    } else {
        error!("❌ Some tests failed. Please check your API key setup.");
        info!("\n🔧 To fix API key issues:");
        info!("1. Get your OpenAI API key from: https://platform.openai.com/api-keys");
        info!("2. Get your Gemini API key from: https://makersuite.google.com/app/apikey");
        info!("3. Set environment variables:");
        info!("   export OPENAI_API_KEY=\"your-openai-key\"");
        info!("   export GEMINI_API_KEY=\"your-gemini-key\"");
        info!("4. Or run mock demo: cargo run --example mock_openai_demo");
    }

    Ok(())
}

/// Mask API key for safe display
fn mask_api_key(key: &str) -> String {
    if key.len() < 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Basic OpenAI API key format validation
fn validate_openai_key_format(key: &str) -> bool {
    // OpenAI keys typically start with "sk-" and are around 51 characters
    key.starts_with("sk-") && key.len() >= 20
}

/// Basic Gemini API key format validation
fn validate_gemini_key_format(key: &str) -> bool {
    // Gemini keys are typically 39 characters long and alphanumeric
    key.len() >= 20
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_masking() {
        assert_eq!(mask_api_key("short"), "*****");
        assert_eq!(
            mask_api_key("sk-1234567890abcdef1234567890abcdef"),
            "sk-1...cdef"
        );
        assert_eq!(mask_api_key("abcdefghijklmnop"), "abcd...mnop");
    }

    #[test]
    fn test_openai_key_validation() {
        assert!(validate_openai_key_format(
            "sk-1234567890abcdef1234567890abcdef"
        ));
        assert!(!validate_openai_key_format("invalid-key"));
        assert!(!validate_openai_key_format("sk-short"));
    }

    #[test]
    fn test_gemini_key_validation() {
        assert!(validate_gemini_key_format(
            "AIzaSyA1234567890abcdefghijklmnopqrstuv"
        ));
        assert!(validate_gemini_key_format(
            "1234567890abcdefghijklmnopqrstuv123456789"
        ));
        assert!(!validate_gemini_key_format("short"));
        assert!(!validate_gemini_key_format("key with spaces"));
    }
}
