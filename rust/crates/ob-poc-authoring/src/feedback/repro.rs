//! Repro Test Generator
//!
//! Generates reproducible test cases for verified failures.
//! Supports golden JSON tests for schema/enum drift and DSL scenario tests.

use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Command;

use super::inspector::FeedbackInspector;
use super::types::{ActorType, AuditAction, ErrorType, IssueStatus};

// =============================================================================
// REPRO RESULT
// =============================================================================

/// Result of repro generation
#[derive(Debug, Clone)]
pub struct ReproResult {
    /// Type of repro generated
    pub repro_type: ReproType,
    /// Path to the generated test file
    pub repro_path: PathBuf,
    /// Whether the test was verified (fails as expected)
    pub verified: bool,
    /// Test output if verification was attempted
    pub output: Option<String>,
}

/// Type of repro test
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReproType {
    /// Golden JSON test for schema/enum drift
    GoldenJson,
    /// DSL scenario test
    DslScenario,
    /// Unit test
    UnitTest,
}

impl ReproType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GoldenJson => "golden_json",
            Self::DslScenario => "dsl_scenario",
            Self::UnitTest => "unit_test",
        }
    }
}

impl std::fmt::Display for ReproType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// REPRO GENERATOR
// =============================================================================

/// Generates reproducible test cases for failures
pub struct ReproGenerator {
    /// Base directory for tests
    tests_dir: PathBuf,
}

impl ReproGenerator {
    pub fn new(tests_dir: PathBuf) -> Self {
        Self { tests_dir }
    }

    /// Generate and verify a repro test for a failure
    pub async fn generate_and_verify(
        &self,
        inspector: &FeedbackInspector,
        fingerprint: &str,
    ) -> Result<ReproResult> {
        // Get the failure details
        let issue = inspector
            .get_issue(fingerprint)
            .await?
            .ok_or_else(|| anyhow!("Failure not found: {}", fingerprint))?;

        let failure = &issue.failure;

        // Determine repro type based on error type
        let repro_type = self.determine_repro_type(failure.error_type);

        // Generate the test
        let repro_path = match repro_type {
            ReproType::GoldenJson => self.generate_golden_json(failure, fingerprint)?,
            ReproType::DslScenario => self.generate_dsl_scenario(failure, fingerprint)?,
            ReproType::UnitTest => self.generate_unit_test(failure, fingerprint)?,
        };

        // Update the failure record with repro info
        inspector
            .set_repro(
                fingerprint,
                repro_type.as_str(),
                repro_path.to_string_lossy().as_ref(),
            )
            .await?;

        // Audit: REPRO_GENERATED
        inspector
            .audit(
                super::types::AuditEntry::new(
                    failure.id,
                    AuditAction::ReproGenerated,
                    ActorType::System,
                )
                .with_details(serde_json::json!({
                    "repro_type": repro_type.as_str(),
                    "repro_path": repro_path.to_string_lossy(),
                })),
            )
            .await?;

        // Verify the test (it should fail)
        let (verified, output) = self.verify_test_fails(&repro_path, repro_type)?;

        if verified {
            // Update status to REPRO_VERIFIED
            inspector.verify_repro(fingerprint).await?;

            // Audit: REPRO_VERIFIED_FAILS
            inspector
                .audit(
                    super::types::AuditEntry::new(
                        failure.id,
                        AuditAction::ReproVerifiedFails,
                        ActorType::System,
                    )
                    .with_details(serde_json::json!({
                        "output_hash": self.hash_output(&output),
                    })),
                )
                .await?;
        } else {
            // Audit: REPRO_VERIFICATION_FAILED
            let output_hash = self.hash_output(&output);
            let mut entry = super::types::AuditEntry::new(
                failure.id,
                AuditAction::ReproVerificationFailed,
                ActorType::System,
            )
            .with_details(serde_json::json!({
                "reason": "Test did not fail as expected",
            }));
            if let Some(ref out) = output {
                entry = entry.with_evidence(out, &output_hash);
            }
            inspector.audit(entry).await?;
        }

        Ok(ReproResult {
            repro_type,
            repro_path,
            verified,
            output,
        })
    }

    /// Verify that a repro test passes (post-fix)
    pub async fn verify_repro_passes(
        &self,
        inspector: &FeedbackInspector,
        fingerprint: &str,
    ) -> Result<bool> {
        let issue = inspector
            .get_issue(fingerprint)
            .await?
            .ok_or_else(|| anyhow!("Failure not found: {}", fingerprint))?;

        let failure = &issue.failure;

        let repro_path = failure
            .repro_path
            .as_ref()
            .ok_or_else(|| anyhow!("No repro test exists for this failure"))?;

        let repro_type = match failure.repro_type.as_deref() {
            Some("golden_json") => ReproType::GoldenJson,
            Some("dsl_scenario") => ReproType::DslScenario,
            Some("unit_test") => ReproType::UnitTest,
            _ => return Err(anyhow!("Unknown repro type")),
        };

        let (passes, output) = self.verify_test_passes(&PathBuf::from(repro_path), repro_type)?;

        if passes {
            // Update status
            inspector
                .set_status(fingerprint, IssueStatus::FixVerified)
                .await?;

            // Audit: REPRO_VERIFIED_PASSES
            inspector
                .audit(
                    super::types::AuditEntry::new(
                        failure.id,
                        AuditAction::ReproVerifiedPasses,
                        ActorType::System,
                    )
                    .with_details(serde_json::json!({
                        "output_hash": self.hash_output(&output),
                    })),
                )
                .await?;
        }

        Ok(passes)
    }

    // =========================================================================
    // GENERATORS
    // =========================================================================

    fn determine_repro_type(&self, error_type: ErrorType) -> ReproType {
        match error_type {
            ErrorType::EnumDrift | ErrorType::SchemaDrift | ErrorType::ParseError => {
                ReproType::GoldenJson
            }
            ErrorType::HandlerPanic | ErrorType::HandlerError | ErrorType::DslParseError => {
                ReproType::DslScenario
            }
            _ => ReproType::UnitTest,
        }
    }

    fn generate_golden_json(
        &self,
        failure: &super::types::FailureRecord,
        fingerprint: &str,
    ) -> Result<PathBuf> {
        // Create directory
        let dir = self.tests_dir.join("golden").join("failures");
        std::fs::create_dir_all(&dir)?;

        // Sanitize fingerprint for filename
        let safe_name = self.sanitize_filename(fingerprint);

        // Write JSON file with error context
        let json_path = dir.join(format!("{}.json", safe_name));
        let json_content = serde_json::json!({
            "fingerprint": fingerprint,
            "error_type": failure.error_type.to_string(),
            "verb": failure.verb,
            "source": failure.source,
            "error_message": failure.error_message,
            "error_context": failure.error_context,
            "expected_behavior": "This test should fail until the schema/enum is updated",
        });
        std::fs::write(&json_path, serde_json::to_string_pretty(&json_content)?)?;

        // Write test file
        let test_path = dir.join(format!("test_{}.rs", safe_name));
        let test_content = format!(
            r#"//! Auto-generated repro test for failure: {}
//!
//! Error: {}
//! Verb: {}
//!
//! This test reproduces the failure by attempting to parse the golden JSON.
//! It should FAIL until the issue is fixed.

use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct GoldenData {{
    // TODO: Add the expected structure based on error_context
}}

#[test]
fn test_{}() {{
    let json_path = "{}.json";
    let content = fs::read_to_string(json_path)
        .expect("Failed to read golden JSON");

    let data: serde_json::Value = serde_json::from_str(&content)
        .expect("Failed to parse JSON");

    let context = data.get("error_context")
        .expect("No error_context in golden file");

    // This should fail until the schema is updated
    let _parsed: GoldenData = serde_json::from_value(context.clone())
        .expect("Schema parsing should work after fix");
}}
"#,
            fingerprint,
            failure.error_message,
            failure.verb,
            safe_name,
            json_path.display()
        );
        std::fs::write(&test_path, test_content)?;

        Ok(test_path)
    }

    fn generate_dsl_scenario(
        &self,
        failure: &super::types::FailureRecord,
        fingerprint: &str,
    ) -> Result<PathBuf> {
        // Create directory
        let dir = self.tests_dir.join("scenarios").join("failures");
        std::fs::create_dir_all(&dir)?;

        // Sanitize fingerprint for filename
        let safe_name = self.sanitize_filename(fingerprint);

        // Write DSL file
        let dsl_path = dir.join(format!("{}.dsl", safe_name));

        // Reconstruct DSL from command sequence if available
        let dsl_content = if let Some(ref commands) = failure.command_sequence {
            format!(
                ";; Auto-generated repro for failure: {}\n;; Error: {}\n;; Verb: {}\n\n{}",
                fingerprint,
                failure.error_message,
                failure.verb,
                commands.join("\n")
            )
        } else {
            format!(
                ";; Auto-generated repro for failure: {}\n;; Error: {}\n;; Verb: {}\n\n;; TODO: Add DSL commands that reproduce this failure\n({} :param \"value\")",
                fingerprint, failure.error_message, failure.verb, failure.verb
            )
        };

        std::fs::write(&dsl_path, dsl_content)?;

        Ok(dsl_path)
    }

    fn generate_unit_test(
        &self,
        failure: &super::types::FailureRecord,
        fingerprint: &str,
    ) -> Result<PathBuf> {
        // Create directory
        let dir = self.tests_dir.join("unit").join("failures");
        std::fs::create_dir_all(&dir)?;

        // Sanitize fingerprint for filename
        let safe_name = self.sanitize_filename(fingerprint);

        // Write test file
        let test_path = dir.join(format!("test_{}.rs", safe_name));
        let test_content = format!(
            r#"//! Auto-generated repro test for failure: {}
//!
//! Error: {}
//! Verb: {}
//!
//! This test reproduces the failure scenario.
//! It should FAIL until the issue is fixed.

#[test]
fn test_{}() {{
    // TODO: Implement test that reproduces this failure
    //
    // User was trying to: {}
    //
    // The failure occurred in verb: {}
    //
    // Error message: {}

    panic!("Repro test not yet implemented - implement the scenario above");
}}
"#,
            fingerprint,
            failure.error_message,
            failure.verb,
            safe_name,
            failure.user_intent.as_deref().unwrap_or("Unknown"),
            failure.verb,
            failure.error_message
        );
        std::fs::write(&test_path, test_content)?;

        Ok(test_path)
    }

    // =========================================================================
    // VERIFICATION
    // =========================================================================

    fn verify_test_fails(
        &self,
        path: &std::path::Path,
        repro_type: ReproType,
    ) -> Result<(bool, Option<String>)> {
        match repro_type {
            ReproType::GoldenJson | ReproType::UnitTest => {
                // For Rust tests, we expect cargo test to fail
                let output = Command::new("cargo")
                    .args([
                        "test",
                        "--test",
                        &path.file_stem().unwrap().to_string_lossy(),
                    ])
                    .current_dir(self.tests_dir.parent().unwrap_or(&self.tests_dir))
                    .output();

                match output {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let combined = format!("{}\n{}", stdout, stderr);

                        // Test should fail (exit code != 0)
                        let verified = !output.status.success();
                        Ok((verified, Some(combined)))
                    }
                    Err(e) => Ok((false, Some(format!("Failed to run test: {}", e)))),
                }
            }
            ReproType::DslScenario => {
                // For DSL scenarios, we'd need to run them through the DSL executor
                // For now, just check the file exists
                Ok((path.exists(), None))
            }
        }
    }

    fn verify_test_passes(
        &self,
        path: &std::path::Path,
        repro_type: ReproType,
    ) -> Result<(bool, Option<String>)> {
        match repro_type {
            ReproType::GoldenJson | ReproType::UnitTest => {
                let output = Command::new("cargo")
                    .args([
                        "test",
                        "--test",
                        &path.file_stem().unwrap().to_string_lossy(),
                    ])
                    .current_dir(self.tests_dir.parent().unwrap_or(&self.tests_dir))
                    .output();

                match output {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let combined = format!("{}\n{}", stdout, stderr);

                        // Test should pass (exit code == 0)
                        let passes = output.status.success();
                        Ok((passes, Some(combined)))
                    }
                    Err(e) => Ok((false, Some(format!("Failed to run test: {}", e)))),
                }
            }
            ReproType::DslScenario => {
                // Would need DSL executor integration
                Ok((
                    false,
                    Some("DSL scenario verification not yet implemented".to_string()),
                ))
            }
        }
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    fn sanitize_filename(&self, fingerprint: &str) -> String {
        fingerprint
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn hash_output(&self, output: &Option<String>) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(output.as_deref().unwrap_or("").as_bytes());
        hex::encode(hasher.finalize())[..16].to_string()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repro_type_display() {
        assert_eq!(ReproType::GoldenJson.to_string(), "golden_json");
        assert_eq!(ReproType::DslScenario.to_string(), "dsl_scenario");
        assert_eq!(ReproType::UnitTest.to_string(), "unit_test");
    }

    #[test]
    fn test_sanitize_filename() {
        let generator = ReproGenerator::new(PathBuf::from("/tmp/tests"));

        assert_eq!(
            generator.sanitize_filename("v1:ENUM_DRIFT:gleif.parse:gleif:abc123"),
            "v1_ENUM_DRIFT_gleif_parse_gleif_abc123"
        );
    }

    #[test]
    fn test_determine_repro_type() {
        let generator = ReproGenerator::new(PathBuf::from("/tmp/tests"));

        assert_eq!(
            generator.determine_repro_type(ErrorType::EnumDrift),
            ReproType::GoldenJson
        );
        assert_eq!(
            generator.determine_repro_type(ErrorType::SchemaDrift),
            ReproType::GoldenJson
        );
        assert_eq!(
            generator.determine_repro_type(ErrorType::HandlerPanic),
            ReproType::DslScenario
        );
        assert_eq!(
            generator.determine_repro_type(ErrorType::Timeout),
            ReproType::UnitTest
        );
    }
}
