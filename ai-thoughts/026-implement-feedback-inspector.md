# 026: Implement Feedback Inspector (023b)

> **Status:** ✅ COMPLETE
> **Completed:** 2026-01-11
> **Design:** `ai-thoughts/023b-feedback-inspector.md`
> **Depends on:** 025 (Event Infrastructure) ✅
> **Tests:** 589 passing

---

## Implementation Summary

### MCP Tools Implemented (Rust)

| Tool | Description |
|------|-------------|
| `feedback_analyze` | Scan events, classify failures, deduplicate |
| `feedback_list` | List issues with filters (status, error_type, verb, source) |
| `feedback_get` | Get full issue detail with occurrences and audit trail |
| `feedback_repro` | Generate and verify repro test |
| `feedback_todo` | Generate TODO document (requires verified repro) |
| `feedback_audit` | Get chronological audit trail |

### Key Implementation Details

- All handlers use structs for args deserialization with `Option<T>` for optional fields
- Helper functions for enum parsing returning `Option<T>`
- Proper error handling with `Result<Value>`
- REPL commands: `:feedback`, `:fb <fingerprint>`, `:fb repro`, `:fb todo`, `:fb audit`

---

## Objective

Implement on-demand feedback analysis system. Reads events captured by 025, joins session context, classifies failures, generates repro tests, creates audit trail.

**Key principle:** Spins up when needed, not always running. Zero DSL pipeline impact.

---

## Implementation Tasks

### Phase 1: Database Schema (2h)

- [ ] Create `migrations/023b_feedback.sql`
- [ ] Create enums:
  ```sql
  CREATE TYPE feedback.error_type AS ENUM (
      'TIMEOUT', 'RATE_LIMITED', 'CONNECTION_RESET', 'SERVICE_UNAVAILABLE', 'POOL_EXHAUSTED',
      'ENUM_DRIFT', 'SCHEMA_DRIFT',
      'PARSE_ERROR', 'HANDLER_PANIC', 'HANDLER_ERROR', 'DSL_PARSE_ERROR',
      'API_ENDPOINT_MOVED', 'API_AUTH_CHANGED', 'VALIDATION_FAILED',
      'UNKNOWN'
  );
  
  CREATE TYPE feedback.remediation_path AS ENUM ('RUNTIME', 'CODE', 'LOG_ONLY');
  
  CREATE TYPE feedback.issue_status AS ENUM (
      'NEW', 'RUNTIME_RESOLVED', 'RUNTIME_ESCALATED',
      'REPRO_GENERATED', 'REPRO_VERIFIED', 'TODO_CREATED',
      'IN_PROGRESS', 'FIX_COMMITTED', 'FIX_VERIFIED',
      'DEPLOYED_STAGING', 'DEPLOYED_PROD', 'RESOLVED',
      'WONT_FIX', 'DUPLICATE', 'INVALID'
  );
  
  CREATE TYPE feedback.actor_type AS ENUM (
      'SYSTEM', 'MCP_AGENT', 'REPL_USER', 'EGUI_USER', 'CI_PIPELINE', 'CLAUDE_CODE', 'CRON_JOB'
  );
  
  CREATE TYPE feedback.audit_action AS ENUM (
      'CAPTURED', 'CLASSIFIED', 'DEDUPLICATED',
      'RUNTIME_ATTEMPT', 'RUNTIME_SUCCESS', 'RUNTIME_EXHAUSTED',
      'REPRO_GENERATED', 'REPRO_VERIFIED_FAILS', 'REPRO_VERIFICATION_FAILED',
      'TODO_CREATED', 'TODO_ASSIGNED', 'FIX_COMMITTED',
      'REPRO_VERIFIED_PASSES', 'DEPLOYED', 'SEMANTIC_REPLAY_PASSED', 'SEMANTIC_REPLAY_FAILED',
      'RESOLVED', 'MARKED_WONT_FIX', 'MARKED_DUPLICATE', 'REOPENED', 'COMMENT_ADDED'
  );
  ```
- [ ] Create `feedback.failures` table (deduplicated issues)
- [ ] Create `feedback.occurrences` table (each time fingerprint seen)
- [ ] Create `feedback.audit_log` table (full history)
- [ ] Add indexes for query patterns

### Phase 2: Core Types (2h)

- [ ] Create `rust/src/feedback/mod.rs`
- [ ] Create `rust/src/feedback/types.rs`:
  ```rust
  pub enum ErrorType { ... }  // Match SQL enum
  pub enum RemediationPath { Runtime, Code, LogOnly, Skip }
  pub enum IssueStatus { ... }  // Match SQL enum
  pub enum ActorType { ... }
  pub enum AuditAction { ... }
  
  pub struct IssueSummary { ... }
  pub struct IssueDetail { failure, occurrences, audit_trail }
  pub struct FailureRecord { ... }  // Full DB row
  pub struct OccurrenceRecord { ... }
  pub struct AuditRecord { ... }
  pub struct SessionContext { user_intent, command_sequence, entries }
  ```

### Phase 3: Classifier (2h)

- [ ] Create `rust/src/feedback/classifier.rs`
- [ ] Implement `FailureClassifier`:
  ```rust
  impl FailureClassifier {
      pub fn classify_snapshot(&self, verb: &str, error: &ErrorSnapshot) -> (ErrorType, RemediationPath);
      pub fn compute_fingerprint_snapshot(&self, verb: &str, error_type: ErrorType, error: &ErrorSnapshot) -> (String, String, u8);
      pub fn suggest_action(&self, error_type: ErrorType) -> Option<String>;
  }
  ```
- [ ] Classification rules:
  - Timeout/RateLimited/ConnectionReset → Runtime
  - EnumDrift/SchemaDrift → Code (provable drift)
  - ParseError/HandlerPanic → Code (might be our bug)
  - NotFound → LogOnly
- [ ] Fingerprint format: `v{version}:{error_type}:{verb}:{source}:{discriminator}`
- [ ] Version field for future migration

### Phase 4: Redactor (2h)

- [ ] Create `rust/src/feedback/redactor.rs`
- [ ] Implement policy-based redaction:
  ```rust
  pub enum RedactionMode {
      StructuralOnly,  // Keep keys/types, redact PII strings
      Full,            // Redact all strings
  }
  
  impl Redactor {
      pub fn redact_for_error(&self, value: &Value, error_type: ErrorType) -> Value;
  }
  ```
- [ ] Rules:
  - EnumDrift/SchemaDrift/ParseError → StructuralOnly (preserve schema for debugging)
  - Everything else → Full
- [ ] Preserve short alphanumeric values (likely enums/identifiers)
- [ ] Redact emails, phones, card numbers via regex

### Phase 5: Feedback Inspector Core (4h)

- [ ] Create `rust/src/feedback/inspector.rs`
- [ ] Implement `FeedbackInspector`:
  ```rust
  impl FeedbackInspector {
      pub fn new(pool: PgPool, event_store_path: PathBuf) -> Self;
      
      // Main analysis
      pub async fn analyze(&self, since: Option<DateTime<Utc>>) -> Result<AnalysisReport>;
      
      // Query
      pub async fn list_issues(&self, filter: IssueFilter) -> Result<Vec<IssueSummary>>;
      pub async fn get_issue(&self, fingerprint: &str) -> Result<Option<IssueDetail>>;
      
      // Session context
      async fn get_session_context(&self, session_id: Option<Uuid>, timestamp: DateTime<Utc>) -> Result<Option<SessionContext>>;
  }
  ```
- [ ] Implement event store reading (JSONL parser)
- [ ] Implement session context enrichment (join sessions.log)
- [ ] Extract user_intent from recent user_input/agent_thought entries

### Phase 6: Failure Store (2h)

- [ ] Create `rust/src/feedback/store.rs`
- [ ] Implement storage operations:
  ```rust
  impl FeedbackInspector {
      async fn create_failure(&self, ...) -> Result<Uuid>;
      async fn record_occurrence(&self, failure_id: Uuid, event: &DslEvent) -> Result<()>;
      pub async fn set_repro(&self, fingerprint: &str, repro_type: &str, repro_path: &str) -> Result<()>;
      pub async fn set_status(&self, fingerprint: &str, status: IssueStatus) -> Result<()>;
      pub async fn mark_fixed(&self, fingerprint: &str, commit: &str, notes: Option<&str>) -> Result<()>;
  }
  ```
- [ ] Implement upsert with occurrence counting
- [ ] Implement CTE pattern for occurrence updates

### Phase 7: Audit Trail (2h)

- [ ] Create `rust/src/feedback/audit.rs`
- [ ] Implement audit logging:
  ```rust
  impl FeedbackInspector {
      pub async fn audit(&self, failure_id: Uuid, action: AuditAction, actor_type: ActorType, details: Value) -> Result<()>;
      pub async fn audit_with_evidence(&self, ..., evidence: Option<&str>, evidence_hash: Option<&str>) -> Result<()>;
      pub async fn get_audit_trail(&self, fingerprint: &str) -> Result<Vec<AuditRecord>>;
  }
  ```
- [ ] Store evidence for verification actions (test output)
- [ ] Store evidence_hash for large outputs

### Phase 8: Repro Generator (4h)

- [ ] Create `rust/src/feedback/repro.rs`
- [ ] Implement `ReproGenerator`:
  ```rust
  impl ReproGenerator {
      pub fn new(tests_dir: PathBuf) -> Self;
      pub async fn generate_and_verify(&self, inspector: &FeedbackInspector, fingerprint: &str) -> Result<ReproResult>;
  }
  ```
- [ ] Generate golden JSON tests for EnumDrift/SchemaDrift/ParseError:
  - Write `rust/tests/golden/failures/{fingerprint}.json`
  - Write `rust/tests/golden/test_{fingerprint}.rs`
- [ ] Generate DSL scenario tests for HandlerPanic/HandlerError:
  - Write `rust/tests/scenarios/failures/{fingerprint}.dsl`
- [ ] Implement verification (run cargo test, check exit code)
- [ ] Audit: REPRO_GENERATED, REPRO_VERIFIED_FAILS or REPRO_VERIFICATION_FAILED

### Phase 9: TODO Generator (2h)

- [ ] Create `rust/src/feedback/todo.rs`
- [ ] Implement TODO generation:
  ```rust
  impl FeedbackInspector {
      pub async fn generate_todo(&self, fingerprint: &str, todo_number: i32) -> Result<TodoResult>;
  }
  ```
- [ ] Require verified repro before TODO creation
- [ ] Include in TODO:
  - User context (what were they trying to do?)
  - Repro test path and verification status
  - Step 1: Verify test FAILS (pre-fix)
  - Step 2: Verify test PASSES (post-fix)
  - Audit trail
- [ ] Update status to TODO_CREATED
- [ ] Audit: TODO_CREATED

### Phase 10: MCP Server (4h)

- [ ] Create `mcp-servers/ob-feedback/package.json`
- [ ] Create `mcp-servers/ob-feedback/src/index.ts`
- [ ] Implement tools:
  ```typescript
  analyze_failures(since?)           // Run analysis, return report
  list_failures(status?, path?, verb?, limit?)  // Query issues
  get_failure(fingerprint)           // Get issue detail
  get_failure_context(fingerprint)   // Get issue + session context + audit
  generate_repro_test(fingerprint)   // Generate and verify repro
  verify_repro_passes(fingerprint)   // Post-fix verification
  generate_todo(fingerprint, todo_number)  // Create TODO doc
  mark_fixed(fingerprint, commit, notes?)  // Mark resolved
  get_audit_trail(fingerprint)       // Get full history
  ```
- [ ] Inspector created per-request, not long-running
- [ ] Format outputs for Claude readability

### Phase 11: REPL Integration (2h)

- [ ] Add `:feedback` / `:fb` command:
  ```
  :feedback              # Analyze last 24h, show summary
  :fb <fingerprint>      # Show issue detail
  :fb repro <fp>         # Generate and verify repro
  :fb todo <fp> <num>    # Generate TODO
  :fb audit <fp>         # Show audit trail
  ```
- [ ] Inspector created on-demand per command
- [ ] Format output for terminal readability

### Phase 12: Tests (4h)

- [ ] Unit test: Classifier correctly categorizes error types
- [ ] Unit test: Fingerprint is stable (same input → same output)
- [ ] Unit test: Redactor preserves structure for schema errors
- [ ] Unit test: Redactor removes PII
- [ ] Integration test: Event → analyze → failure created
- [ ] Integration test: Session context enrichment works
- [ ] Integration test: Repro generation creates valid test file
- [ ] Integration test: Audit trail captures all transitions
- [ ] Integration test: MCP tools return expected format

---

## Verification

After implementation:

```bash
# 1. Run migrations
sqlx migrate run

# 2. Run tests
cargo test feedback::

# 3. Manual test in REPL
# First, trigger a failure
research.fetch-entity source="gleif" lei="INVALID"

# Then analyze
:feedback
:fb <fingerprint>
:fb repro <fingerprint>
:fb todo <fingerprint> 27

# 4. Test MCP server
cd mcp-servers/ob-feedback && npm test
```

---

## Files to Create

```
rust/src/feedback/
├── mod.rs           # pub use
├── types.rs         # ErrorType, IssueStatus, etc.
├── classifier.rs    # FailureClassifier
├── redactor.rs      # Redactor
├── inspector.rs     # FeedbackInspector
├── store.rs         # DB operations
├── audit.rs         # Audit trail
├── repro.rs         # ReproGenerator
└── todo.rs          # TODO generator

mcp-servers/ob-feedback/
├── package.json
├── tsconfig.json
└── src/
    └── index.ts

migrations/
└── 023b_feedback.sql
```

---

## Flow Summary

```
1. User runs DSL, it fails
2. Event emitted (025 - already done)
3. User runs :feedback
4. Inspector reads events, classifies, creates feedback.failures record
5. User runs :fb repro <fp>
6. ReproGenerator creates test, runs it, verifies it FAILS
7. User runs :fb todo <fp> 27
8. TODO created (requires verified repro)
9. Claude Code implements fix
10. User runs :fb verify <fp>
11. ReproGenerator runs test, verifies it PASSES
12. User marks fixed: :fb fixed <fp> abc123
13. Full audit trail preserved
```

---

## Critical Constraints

1. **Inspector is on-demand** - Created per request, not always running
2. **Repro must be verified** - Test FAILS before fix, PASSES after
3. **TODO requires verified repro** - Enforced, not optional
4. **Full audit trail** - Every state transition logged with actor + timestamp
5. **Session context preserved** - What was user trying to do?
6. **Zero DSL impact** - This module never touches executor

---

## Next

After this TODO: Start implementation of 025 first (dependency)
