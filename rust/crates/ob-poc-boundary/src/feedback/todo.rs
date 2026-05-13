//! TODO Generator
//!
//! Generates structured TODO documents for verified failures.
//! Requires a verified repro test before TODO creation.

use anyhow::{anyhow, Result};
use chrono::Utc;
use std::path::PathBuf;

use super::inspector::FeedbackInspector;
use super::types::{ActorType, AuditAction, IssueStatus};

// =============================================================================
// TODO RESULT
// =============================================================================

/// Result of TODO generation
#[derive(Debug, Clone)]
pub struct TodoResult {
    /// TODO number (for tracking in ai-thoughts)
    pub todo_number: i32,
    /// Path to the generated TODO file
    pub todo_path: PathBuf,
    /// Content of the TODO
    pub content: String,
}

// =============================================================================
// TODO GENERATOR
// =============================================================================

/// Generates TODO documents for failures
pub struct TodoGenerator {
    /// Base directory for TODOs (e.g., ai-thoughts/)
    todos_dir: PathBuf,
}

impl TodoGenerator {
    pub fn new(todos_dir: PathBuf) -> Self {
        Self { todos_dir }
    }

    /// Generate a TODO document for a failure
    ///
    /// Requires:
    /// - Failure must have a verified repro test (repro_verified = true)
    /// - Status must be REPRO_VERIFIED
    pub async fn generate_todo(
        &self,
        inspector: &FeedbackInspector,
        fingerprint: &str,
        todo_number: i32,
    ) -> Result<TodoResult> {
        // Get the failure details
        let issue = inspector
            .get_issue(fingerprint)
            .await?
            .ok_or_else(|| anyhow!("Failure not found: {}", fingerprint))?;

        let failure = &issue.failure;

        // Verify repro requirement
        if !failure.repro_verified {
            return Err(anyhow!(
                "Cannot create TODO without verified repro. Run `:fb repro {}` first",
                fingerprint
            ));
        }

        if failure.status != IssueStatus::ReproVerified {
            return Err(anyhow!(
                "Failure status must be REPRO_VERIFIED, got: {}",
                failure.status
            ));
        }

        // Generate TODO content
        let content = self.generate_content(&issue, todo_number)?;

        // Write TODO file
        std::fs::create_dir_all(&self.todos_dir)?;
        let filename = format!(
            "{:03}-fix-{}.md",
            todo_number,
            self.sanitize_name(fingerprint)
        );
        let todo_path = self.todos_dir.join(&filename);
        std::fs::write(&todo_path, &content)?;

        // Update status
        inspector
            .set_status(fingerprint, IssueStatus::TodoCreated)
            .await?;

        // Audit: TODO_CREATED
        inspector
            .audit(
                super::types::AuditEntry::new(
                    failure.id,
                    AuditAction::TodoCreated,
                    ActorType::System,
                )
                .with_details(serde_json::json!({
                    "todo_number": todo_number,
                    "todo_path": todo_path.to_string_lossy(),
                })),
            )
            .await?;

        Ok(TodoResult {
            todo_number,
            todo_path,
            content,
        })
    }

    // =========================================================================
    // CONTENT GENERATION
    // =========================================================================

    fn generate_content(
        &self,
        issue: &super::types::IssueDetail,
        todo_number: i32,
    ) -> Result<String> {
        let failure = &issue.failure;
        let audit_trail = &issue.audit_trail;

        let repro_path = failure.repro_path.as_deref().unwrap_or("(no repro path)");

        let user_intent = failure
            .user_intent
            .as_deref()
            .unwrap_or("Unknown - no session context captured");

        let source = failure.source.as_deref().unwrap_or("internal");

        let command_sequence = failure
            .command_sequence
            .as_ref()
            .map(|cmds| cmds.join("\n    "))
            .unwrap_or_else(|| "(no commands captured)".to_string());

        let suggested_action = super::classifier::FailureClassifier::new()
            .suggest_action(failure.error_type)
            .unwrap_or_else(|| "Investigate and fix the root cause".to_string());

        // Format audit trail
        let audit_summary: Vec<String> = audit_trail
            .iter()
            .map(|a| {
                format!(
                    "- {} {} by {} at {}",
                    a.action,
                    a.new_status
                        .map(|s| format!("-> {}", s))
                        .unwrap_or_default(),
                    a.actor_type,
                    a.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                )
            })
            .collect();

        let content = format!(
            r#"# {todo_number:03}: Fix {error_type} in {verb}

> **Status:** TODO
> **Priority:** {priority}
> **Fingerprint:** `{fingerprint}`
> **Created:** {created}
> **First seen:** {first_seen}
> **Occurrences:** {occurrences}

---

## Summary

**Error Type:** {error_type}
**Remediation Path:** {remediation_path}
**Source:** {source}
**Verb:** {verb}

### User Context

What the user was trying to do:
> {user_intent}

Commands leading to failure:
```
    {command_sequence}
```

### Error Message

```
{error_message}
```

---

## Repro Test

**Path:** `{repro_path}`
**Type:** {repro_type}
**Verified:** {repro_verified}

### Pre-fix Verification (Step 1)

Before making changes, verify the repro test FAILS:

```bash
cargo test --test {repro_test_name}
```

Expected: Test should FAIL, confirming the bug exists.

### Post-fix Verification (Step 2)

After implementing the fix, verify the repro test PASSES:

```bash
cargo test --test {repro_test_name}
```

Expected: Test should PASS, confirming the fix works.

---

## Suggested Fix

{suggested_action}

### Investigation Notes

<!-- Add your investigation notes here -->

### Implementation Plan

<!-- Add your implementation plan here -->

1. [ ] Investigate root cause
2. [ ] Implement fix
3. [ ] Verify repro test passes
4. [ ] Run full test suite
5. [ ] Mark as fixed: `:fb fixed {fingerprint} <commit>`

---

## Audit Trail

{audit_trail}

---

## Metadata

| Field | Value |
|-------|-------|
| Fingerprint | `{fingerprint}` |
| Error Type | {error_type} |
| Remediation Path | {remediation_path} |
| Verb | {verb} |
| Source | {source} |
| First Seen | {first_seen} |
| Last Seen | {last_seen} |
| Occurrence Count | {occurrences} |
| Repro Path | `{repro_path}` |
| Repro Verified | {repro_verified} |
"#,
            todo_number = todo_number,
            error_type = failure.error_type,
            verb = failure.verb,
            priority = self.determine_priority(failure),
            fingerprint = failure.fingerprint,
            created = Utc::now().format("%Y-%m-%d"),
            first_seen = failure.first_seen_at.format("%Y-%m-%d %H:%M UTC"),
            last_seen = failure.last_seen_at.format("%Y-%m-%d %H:%M UTC"),
            occurrences = failure.occurrence_count,
            remediation_path = failure.remediation_path,
            source = source,
            user_intent = user_intent,
            command_sequence = command_sequence,
            error_message = failure.error_message,
            repro_path = repro_path,
            repro_type = failure.repro_type.as_deref().unwrap_or("unknown"),
            repro_verified = if failure.repro_verified { "Yes" } else { "No" },
            repro_test_name = PathBuf::from(repro_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "repro".to_string()),
            suggested_action = suggested_action,
            audit_trail = audit_summary.join("\n"),
        );

        Ok(content)
    }

    fn determine_priority(&self, failure: &super::types::FailureRecord) -> &'static str {
        // Priority based on occurrence count and error type
        if failure.occurrence_count >= 10 {
            "HIGH"
        } else if failure.occurrence_count >= 5 || failure.error_type.requires_code_fix() {
            "MEDIUM"
        } else {
            "LOW"
        }
    }

    fn sanitize_name(&self, fingerprint: &str) -> String {
        // Extract just the key parts for a readable filename
        let parts: Vec<&str> = fingerprint.split(':').collect();
        if parts.len() >= 4 {
            format!("{}-{}", parts[1].to_lowercase(), parts[2].replace('.', "-"))
        } else {
            fingerprint
                .chars()
                .take(30)
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect()
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        let generator = TodoGenerator::new(PathBuf::from("/tmp/todos"));

        assert_eq!(
            generator.sanitize_name("v1:ENUM_DRIFT:gleif.parse:gleif:abc123"),
            "enum_drift-gleif-parse"
        );
    }

    #[test]
    fn test_determine_priority() {
        use super::super::types::{ErrorType, FailureRecord, IssueStatus, RemediationPath};
        use chrono::Utc;
        use uuid::Uuid;

        let generator = TodoGenerator::new(PathBuf::from("/tmp/todos"));

        let mut failure = FailureRecord {
            id: Uuid::new_v4(),
            fingerprint: "test".to_string(),
            fingerprint_version: 1,
            error_type: ErrorType::Timeout,
            remediation_path: RemediationPath::Runtime,
            status: IssueStatus::New,
            verb: "test.verb".to_string(),
            source: None,
            error_message: "test".to_string(),
            error_context: None,
            user_intent: None,
            command_sequence: None,
            repro_type: None,
            repro_path: None,
            repro_verified: false,
            fix_commit: None,
            fix_notes: None,
            occurrence_count: 1,
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
            resolved_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(generator.determine_priority(&failure), "LOW");

        failure.occurrence_count = 10;
        assert_eq!(generator.determine_priority(&failure), "HIGH");

        failure.occurrence_count = 5;
        assert_eq!(generator.determine_priority(&failure), "MEDIUM");

        failure.occurrence_count = 2;
        failure.error_type = ErrorType::EnumDrift;
        assert_eq!(generator.determine_priority(&failure), "MEDIUM");
    }
}
