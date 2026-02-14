# Policy Loom — Claude Code Implementation TODO

**Source**: `PENDING_Policy_Loom_v0_4.md`
**Target**: ob-poc (Rust / SQLx / PostgreSQL / Semantic OS)
**Constraint**: All phases are mandatory. After completing each phase, → IMMEDIATELY proceed to the next phase. Report progress % and E-invariant at every gate.

---

## Phase L0 — Core Types & Enums
**Depends on**: Semantic OS Phase 0 (registry)
**Files**: `src/loom/types.rs`, `src/loom/mod.rs`

- [ ] Create `src/loom/` module directory and wire into `src/lib.rs`
- [ ] Define `ObjectType` enum extensions:
  ```
  PolicyDocFamily, PolicyDocVersion, PolicyInterpretation, PolicyRuleSet, PolicyTestFixture
  ```
- [ ] Define `ActionRef` enum (substrate-aware union type):
  ```rust
  pub enum ActionRef {
      Dsl { fqn: String },
      Java { class_fqn: String, method: String, version: Option<String> },
      Bpmn { process_id: String, task_id: String },
      Custom { namespace: String, identifier: String },
  }
  ```
  - Implement `Serialize`/`Deserialize` for JSONB storage
  - Stability rule: `ActionRef` identity must not change across deployments
- [ ] Define `ActionRefVariant` discriminator for match arms
- [ ] Define `ComparisonOp` enum: `Gte | Gt | Lte | Lt | Eq | InSet`
- [ ] Define `TimeoutAction` enum: `Escalate | Cancel | Extend`
- [ ] Define `ExceptionEffect` enum: `Exclude | Override | Substitute`
- [ ] Define `LinkageType` enum:
  ```
  Precondition | EvidenceSource | ParameterSource | SecurityDirective | TimeoutSource | EscalationTrigger
  ```
- [ ] Define `InterpretationStatus` enum: `Draft | Reviewed | Approved | Disputed`
- [ ] Define `PolicyStatus` enum: `Draft | Active | Superseded | Retired`
- [ ] Define `GateVerdict` enum with severity ordering: `Deny > Escalate > PermitWithMasking > Permit`
  - Implement `Ord` trait so `max()` yields highest severity
- [ ] Define `MergeResult` enum: `Value(f64) | GovernanceFinding | UnionOfSets`
- [ ] Unit tests for `GateVerdict` ordering, `ComparisonOp` serde round-trip

→ IMMEDIATELY proceed to Phase L1. Progress: 10%

---

## Phase L1 — PolicySectionRef & Section Index
**Depends on**: Phase L0
**Files**: `src/loom/section_ref.rs`

- [ ] Define `PolicySectionRef` struct:
  ```rust
  pub struct PolicySectionRef {
      pub anchor_id: Option<String>,          // preferred: "sec-4.3", "def-ubo"
      pub positional: Option<PositionalRef>,   // fallback
      pub quote_excerpt_hash: Option<String>,  // SHA-256 of first 100 chars
      pub display_label: String,               // "§4.3", "Table 2"
  }
  ```
- [ ] Define `PositionalRef` struct:
  ```rust
  pub struct PositionalRef {
      pub doc_content_hash: String,
      pub page: u32,
      pub paragraph_index: u32,
  }
  ```
- [ ] Define `SectionIndexEntry` struct (stored as JSONB in `PolicyDocVersionBody`):
  ```rust
  pub struct SectionIndexEntry {
      pub anchor_id: String,
      pub display_label: String,
      pub page: Option<u32>,
      pub paragraph_index: Option<u32>,
      pub quote_excerpt_hash: String,  // SHA-256 of first 100 chars
  }
  ```
- [ ] Implement `compute_excerpt_hash(text: &str) -> String` — SHA-256 of first 100 chars
- [ ] Implement drift detection: `check_section_drift(old_ref: &PolicySectionRef, new_index: &[SectionIndexEntry]) -> Option<DriftFinding>`
- [ ] Implement `resolve_section_ref(ref: &PolicySectionRef, index: &[SectionIndexEntry]) -> Result<&SectionIndexEntry>`
  - Try anchor_id first, fall back to positional, error if both fail
- [ ] Unit tests: drift detection with matching/mismatching hashes, resolution priority

→ IMMEDIATELY proceed to Phase L2. Progress: 18%

---

## Phase L2 — Policy Document Family & Version Bodies
**Depends on**: Phase L1, Semantic OS Phase 1 (registry)
**Files**: `src/loom/policy_doc.rs`

- [ ] Define `PolicyDocFamilyBody` struct:
  ```rust
  pub struct PolicyDocFamilyBody {
      pub family_id: String,              // e.g. "bny.kyc.policy.ubo_discovery"
      pub display_name: String,
      pub description: Option<String>,
      pub issuer: String,                  // e.g. "BNY KYC Policy Team"
      pub domain_tags: Vec<String>,        // e.g. ["kyc.ubo", "kyc.proofs"]
  }
  ```
  - Validate: family_id must not contain volatile facts (no dates, no version numbers)
  - Validate: dot-notation format `^[a-z][a-z0-9]*(\.[a-z][a-z0-9_]*)+$`
- [ ] Define `PolicyDocVersionBody` struct:
  ```rust
  pub struct PolicyDocVersionBody {
      pub family_id: String,
      pub version_label: String,           // "@v12", "@2026-01"
      pub content_hash: String,            // SHA-256 of full document content
      pub storage_ref: String,             // pointer to immutable storage
      pub section_index: Vec<SectionIndexEntry>,
      pub jurisdiction_scope: Option<String>,  // activation scope key dimension
      pub effective_from: NaiveDate,
      pub effective_until: Option<NaiveDate>,
      pub approval_chain: Vec<ApprovalRecord>,
      pub status: PolicyStatus,
  }
  ```
- [ ] Define `ApprovalRecord` struct: `{ approver: String, approved_at: DateTime, role: String }`
- [ ] Implement active version constraint enforcement:
  - At most one Active version per `(family_id, jurisdiction_scope)` at a given time
  - When v13 activates for scope SA, auto-Supersede v12 for that scope
  - Allow different active versions across jurisdictions during transition
- [ ] Implement section index extraction stub: `extract_section_index(content: &str) -> Vec<SectionIndexEntry>`
  - Parse headings/anchors from document content
  - Compute `quote_excerpt_hash` for each section
- [ ] Database migration: `policy_doc_families` table, `policy_doc_versions` table
- [ ] SQLx CRUD: insert family, insert version, query active version by scope key, transition status
- [ ] Integration tests: version lifecycle (Draft → Active → Superseded), activation scope constraint

→ IMMEDIATELY proceed to Phase L3. Progress: 28%

---

## Phase L3 — CoverageSpec & Applicability Model
**Depends on**: Phase L2
**Files**: `src/loom/coverage.rs`

- [ ] Define `CoverageSpec` struct:
  ```rust
  pub struct CoverageSpec {
      pub domain_tags: Vec<String>,
      pub entity_type_scopes: Vec<String>,
      pub evidence_type_scopes: Vec<String>,
      pub action_categories: Vec<String>,
      pub jurisdiction_scopes: Vec<String>,   // empty = all
      pub risk_tier_scopes: Vec<String>,      // empty = all
      pub product_scopes: Vec<String>,        // empty = all
      pub exclusion_predicates: Vec<ExclusionPredicate>,
      pub is_default_for_domain: bool,
      pub specificity_rank: i32,              // higher = more specific, wins on conflict
  }
  ```
- [ ] Define `ExclusionPredicate` struct:
  ```rust
  pub struct ExclusionPredicate {
      pub predicate: String,          // evaluable expression
      pub rationale: String,
      pub superseded_by: Option<String>,  // family_id of replacement policy
  }
  ```
- [ ] Define `CaseProfile` struct (the evaluation-time input):
  ```rust
  pub struct CaseProfile {
      pub entity_type: String,
      pub jurisdiction: String,
      pub risk_tier: String,
      pub product: Option<String>,
      pub evidence_types_present: Vec<String>,
      // extensible with additional CoverageSpec dimensions
  }
  ```
- [ ] Implement `CoverageSpec::matches(&self, case: &CaseProfile) -> bool`
  - Check positive applicability (empty scope = matches all)
  - Check exclusion predicates
- [ ] Implement `select_applicable_policies(specs: &[(PolicyId, CoverageSpec)], case: &CaseProfile) -> Vec<PolicyId>`
  - Filter by `matches()`, sort by `specificity_rank` descending
- [ ] Implement default-for-domain fallback logic
- [ ] Unit tests: matching, exclusion, specificity ranking, default fallback

→ IMMEDIATELY proceed to Phase L4. Progress: 36%

---

## Phase L4 — Directive Types & Interpretation Body
**Depends on**: Phase L3, Semantic OS Phase 3
**Files**: `src/loom/directives.rs`, `src/loom/interpretation.rs`

- [ ] Define all directive types. Each carries `applicability_predicate: Option<String>`, `section_ref: PolicySectionRef`, `rationale: Option<String>`:

  - [ ] `EvidenceDirective`:
    ```rust
    pub struct EvidenceDirective {
        pub required_doc_types: Vec<String>,
        pub substitute_doc_types: Vec<String>,
        pub freshness_window_days: Option<u32>,
        pub confidence_minimum: Option<f64>,
        pub applicability_predicate: Option<String>,
        pub section_ref: PolicySectionRef,
        pub rationale: Option<String>,
    }
    ```
  - [ ] `ThresholdDirective`:
    ```rust
    pub struct ThresholdDirective {
        pub parameter_name: String,
        pub value: f64,
        pub unit: Option<String>,
        pub comparison_op: ComparisonOp,
        pub applicability_predicate: Option<String>,
        pub section_ref: PolicySectionRef,
        pub rationale: Option<String>,
    }
    ```
  - [ ] `DeadlineDirective`:
    ```rust
    pub struct DeadlineDirective {
        pub action_category: String,
        pub deadline_days: u32,
        pub reminder_days: Option<u32>,
        pub timeout_action: TimeoutAction,
        pub applicability_predicate: Option<String>,
        pub section_ref: PolicySectionRef,
        pub rationale: Option<String>,
    }
    ```
  - [ ] `HandlingDirective`:
    ```rust
    pub struct HandlingDirective {
        pub handling_controls: Vec<String>,  // e.g. ["MaskByDefault", "SecureViewerOnly", "NoExport"]
        pub applicability_predicate: Option<String>,
        pub section_ref: PolicySectionRef,
        pub rationale: Option<String>,
    }
    ```
  - [ ] `EscalationDirective`:
    ```rust
    pub struct EscalationDirective {
        pub trigger_condition: String,
        pub escalation_target: String,
        pub severity: String,
        pub required_context: Vec<String>,
        pub applicability_predicate: Option<String>,
        pub section_ref: PolicySectionRef,
        pub rationale: Option<String>,
    }
    ```
  - [ ] `ExceptionClause`:
    ```rust
    pub struct ExceptionClause {
        pub exception_predicate: String,
        pub effect: ExceptionEffect,
        pub substitute_directive: Option<Box<serde_json::Value>>,  // generic directive ref
        pub applicability_predicate: Option<String>,
        pub section_ref: PolicySectionRef,
        pub rationale: Option<String>,
    }
    ```
  - [ ] `PolicyDefinition`:
    ```rust
    pub struct PolicyDefinition {
        pub term: String,
        pub definition_text: String,
        pub semantic_mapping: Option<String>,  // e.g. "entity.ubo_threshold"
    }
    ```

- [ ] Define `PolicyInterpretationBody` struct:
  ```rust
  pub struct PolicyInterpretationBody {
      pub interpretation_id: String,
      pub policy_doc_version_ref: Uuid,        // snapshot_id of linked PolicyDocVersion
      pub policy_section_refs: Vec<PolicySectionRef>,
      pub evidence_directives: Vec<EvidenceDirective>,
      pub threshold_directives: Vec<ThresholdDirective>,
      pub deadline_directives: Vec<DeadlineDirective>,
      pub handling_directives: Vec<HandlingDirective>,
      pub escalation_directives: Vec<EscalationDirective>,
      pub exception_clauses: Vec<ExceptionClause>,
      pub definitions: Vec<PolicyDefinition>,
      pub reviewed_by: Option<String>,
      pub reviewed_at: Option<DateTime<Utc>>,
      pub review_status: InterpretationStatus,
      pub dispute_reason: Option<String>,       // required when Disputed
      pub rationale_notes: Option<String>,
  }
  ```
- [ ] Implement lifecycle validation:
  - `Disputed` requires non-empty `dispute_reason`
  - Cannot transition to `Approved` from `Disputed` without clearing dispute
  - Only `Approved` interpretations can feed compilation (structural constraint)
- [ ] Implement structural diff: `diff_interpretations(a: &PolicyInterpretationBody, b: &PolicyInterpretationBody) -> InterpretationDiff`
  - Per-directive-type diff (added, removed, changed directives)
  - Report threshold changes, evidence set changes, deadline changes
- [ ] Database migration: `policy_interpretations` table with JSONB body
- [ ] SQLx CRUD: insert, update status, query by doc version ref, query Approved only
- [ ] Unit tests: lifecycle transitions, dispute blocking, structural diff

→ IMMEDIATELY proceed to Phase L5. Progress: 48%

---

## Phase L5 — Merge Semantics Engine
**Depends on**: Phase L4
**Files**: `src/loom/merge.rs`

- [ ] Implement `merge_thresholds(op: ComparisonOp, values: &[f64]) -> MergeResult`:
  ```rust
  fn merge_thresholds(op: ComparisonOp, values: &[f64]) -> MergeResult {
      match op {
          Gte | Gt => MergeResult::Value(values.iter().cloned().fold(f64::MAX, f64::min)),
          Lte | Lt => MergeResult::Value(values.iter().cloned().fold(f64::MIN, f64::max)),
          Eq => if values.windows(2).all(|w| w[0] == w[1]) {
              MergeResult::Value(values[0])
          } else { MergeResult::GovernanceFinding },
          InSet => MergeResult::UnionOfSets,
      }
  }
  ```
- [ ] Implement evidence merge: **union** of all evidence sets from applicable policies
- [ ] Implement deadline merge: **most stringent** (shortest deadline wins)
- [ ] Implement handling merge: **most restrictive** (union of all handling controls, highest classification)
- [ ] Implement gate verdict merge: **severity ordering** (Deny > Escalate > PermitWithMasking > Permit)
- [ ] Implement escalation merge: **union** of all triggered escalations
- [ ] Implement `ExceptionClause` override handling:
  - `Override` on more-specific policy can relax a merge result
  - `Substitute` can replace one evidence set with another
- [ ] Implement structural conflict detection:
  - Same parameter with different `ComparisonOp` → GovernanceFinding
  - Equal `specificity_rank` with conflicting values → GovernanceFinding + DisambiguationPrompt
  - Block automated execution on unresolved conflict
- [ ] Define `GovernanceFinding` struct:
  ```rust
  pub struct GovernanceFinding {
      pub policy_refs: Vec<Uuid>,
      pub conflicting_values: Vec<serde_json::Value>,
      pub output_class: String,
      pub description: String,
  }
  ```
- [ ] Define `DisambiguationPrompt` struct for agent/analyst escalation
- [ ] Define `PolicyEvaluationTrace` struct (full audit output of a merge evaluation):
  - All applicable policies, winning policy per output, merge rule applied, pre-merge values
  - Governance findings if any
- [ ] Unit tests: every merge rule with known inputs from Appendix A example
  - Evidence union: {Passport, TaxReturn} ∪ {SourceOfFunds} = {Passport, TaxReturn, SourceOfFunds}
  - Deadline most-stringent: min(14d, 7d) = 7d
  - Threshold Gte: min(25%, 10%) = 10%
  - Equal-rank conflict → GovernanceFinding

→ IMMEDIATELY proceed to Phase L6. Progress: 58%

---

## Phase L6 — PolicyRuleSet, Compilation & Action Linkage
**Depends on**: Phase L5, Semantic OS Phase 6 (publish gates)
**Files**: `src/loom/ruleset.rs`, `src/loom/compilation.rs`, `src/loom/linkage.rs`

- [ ] Define `PolicyProvenance` struct:
  ```rust
  pub struct PolicyProvenance {
      pub policy_doc_family_id: String,
      pub policy_doc_version_ref: Uuid,
      pub interpretation_ref: Uuid,
      pub ruleset_ref: Uuid,
      pub source_directive_type: String,    // Evidence | Threshold | Deadline | etc.
      pub source_directive_index: usize,
      pub policy_section_ref: PolicySectionRef,
  }
  ```
- [ ] Define `ActionPolicyLinkage` struct:
  ```rust
  pub struct ActionPolicyLinkage {
      pub action_ref: ActionRef,
      pub linkage_type: LinkageType,
      pub affected_elements: Vec<String>,
      pub policy_section_ref: PolicySectionRef,
      pub interpretation_directive: String,  // e.g. "evidence_directives[0]"
  }
  ```
- [ ] Define `PolicyRuleSetBody` struct:
  ```rust
  pub struct PolicyRuleSetBody {
      pub ruleset_id: String,
      pub interpretation_ref: Uuid,
      pub policy_doc_version_ref: Uuid,
      pub rule_snapshot_ids: Vec<Uuid>,
      pub action_linkages: Vec<ActionPolicyLinkage>,
      pub fixture_snapshot_ids: Vec<Uuid>,
      pub compiled_at: DateTime<Utc>,
      pub compiled_by: String,
      pub compilation_warnings: Vec<String>,
      pub coverage_spec: CoverageSpec,
  }
  ```
- [ ] Implement deterministic compilation:
  `compile_ruleset(interpretation: &PolicyInterpretationBody) -> Result<PolicyRuleSetBody>`
  - **Guard**: interpretation must have `review_status == Approved`
  - Fail hard if interpretation is Draft, Reviewed, or Disputed
  - For each directive → produce a `PolicyRule` snapshot with `PolicyProvenance`
  - For each directive → produce `ActionPolicyLinkage` records mapping to known ActionRefs
  - Determinism: same Approved interpretation ALWAYS produces same ruleset (no randomness, no LLM)
- [ ] Add `PolicyProvenance` field to existing `PolicyRule` struct (extend, don't replace)
- [ ] Implement `ActionRef` resolution: validate that referenced actions exist in the verb registry
- [ ] Database migration: `policy_rulesets` table, `action_policy_linkages` table
- [ ] SQLx CRUD: insert ruleset, query by interpretation ref, query action linkages by action_ref
- [ ] Integration tests: compile from Appendix A example interpretation, verify provenance chain integrity

→ IMMEDIATELY proceed to Phase L7. Progress: 68%

---

## Phase L7 — Test Fixtures & Publish Gates
**Depends on**: Phase L6, Semantic OS Phase 6 (publish gates)
**Files**: `src/loom/fixtures.rs`, `src/loom/gates.rs`

- [ ] Define `PolicyTestFixtureBody` struct:
  ```rust
  pub struct PolicyTestFixtureBody {
      pub fixture_id: String,
      pub ruleset_ref: Uuid,
      pub scenario_name: String,
      pub case_profile: CaseProfile,
      pub evidence_present: Vec<String>,
      pub entity_state: serde_json::Value,
      pub expected_gate_verdict: GateVerdict,
      pub expected_evidence_reqs: Vec<String>,
      pub expected_deadlines: HashMap<String, u32>,  // action_category → days
      pub expected_handling: Vec<String>,
      pub expected_escalation: Option<String>,
      pub expected_parameters: serde_json::Value,
      pub intentional_change: bool,
      pub change_rationale: Option<String>,          // required when intentional_change = true
      pub superseded_by_fixture_id: Option<Uuid>,
      pub deprecated: bool,
      pub deprecation_rationale: Option<String>,
  }
  ```
- [ ] Implement fixture evaluation:
  `evaluate_fixture(fixture: &PolicyTestFixtureBody, ruleset: &PolicyRuleSetBody, merge_engine: &MergeEngine) -> FixtureResult`
  - Compare actual gate verdict, evidence reqs, deadlines, handling against expected
  - Return pass/fail with structured diff on failure
- [ ] Implement fixture inheritance across versions:
  - Load all non-superseded, non-deprecated fixtures from predecessor ruleset
  - Evaluate predecessor fixtures against new rules
  - Changed outcomes must have `intentional_change` fixture or superseding fixture
  - Fixtures not relevant to new version can be `deprecated = true` with rationale
- [ ] Implement coverage matrix computation:
  `compute_coverage_matrix(ruleset: &PolicyRuleSetBody, fixtures: &[PolicyTestFixtureBody]) -> CoverageMatrix`
  - jurisdiction × risk_tier × entity_type × product combinations with/without fixtures
  - Untested combinations → governance signals
- [ ] Implement publish gates (integrate with existing Phase 6 gate framework):
  - [ ] **Fixture gate** (Hard): all non-superseded, non-deprecated fixtures must pass
  - [ ] **Interpretation coverage gate** (Hard): must reference valid active PolicyDocVersion
  - [ ] **Interpretation status gate** (Hard): linked interpretation must be Approved, not Disputed
  - [ ] **Action linkage consistency gate** (Hard): all ActionPolicyLinkages must reference active contracts
  - [ ] **Provenance completeness gate** (Hard): every compiled PolicyRule must have non-empty PolicyProvenance with valid PolicySectionRef
  - [ ] **Coverage gap warning** (ReportOnly → Hard): <80% fixture coverage triggers governance warning
  - [ ] **Section drift warning** (ReportOnly): stale `quote_excerpt_hash` triggers governance warning
- [ ] Database migration: `policy_test_fixtures` table
- [ ] SQLx CRUD: insert fixture, query by ruleset, query non-superseded/non-deprecated
- [ ] Integration tests: fixture pass/fail, inheritance from v12→v13 per Appendix A (F-1 through F-4), gate blocking

→ IMMEDIATELY proceed to Phase L8. Progress: 80%

---

## Phase L8 — Policy Evaluation & Traceability
**Depends on**: Phase L7, Semantic OS Phase 7 (Context Resolution)
**Files**: `src/loom/evaluation.rs`, `src/loom/trace.rs`

- [ ] Define `PolicyEvaluationTrace` struct (full version from §6.5):
  ```rust
  pub struct PolicyEvaluationTrace {
      pub verdict: GateVerdict,
      pub parameter_outputs: serde_json::Value,
      pub merge_applied: Vec<MergeRecord>,
      pub applicable_policies: Vec<Uuid>,
      pub winning_policy_per_output: Vec<OutputWinner>,
      pub policy_doc_version_refs: Vec<Uuid>,
      pub policy_section_refs: Vec<PolicySectionRef>,
      pub interpretation_refs: Vec<Uuid>,
      pub ruleset_refs: Vec<Uuid>,
      pub rule_snapshot_ids: Vec<Uuid>,
      pub action_linkages: Vec<ActionPolicyLinkage>,
      pub evidence_used: Vec<String>,
      pub evidence_missing: Vec<String>,
      pub negative_evidence: Vec<String>,
      pub escalation_reasons: Vec<String>,
      pub masking_plan: Option<serde_json::Value>,
      pub governance_findings: Vec<GovernanceFinding>,
      pub evaluated_at: DateTime<Utc>,
      pub as_of_time: DateTime<Utc>,
  }
  ```
- [ ] Implement full policy evaluation pipeline:
  `evaluate_policies(case: &CaseProfile, as_of: DateTime<Utc>) -> Result<PolicyEvaluationTrace>`
  1. Select applicable policies via CoverageSpec
  2. Load active PolicyRuleSets for selected policies
  3. Evaluate rules against case state
  4. Apply merge semantics per output class (§6.3.1)
  5. Detect governance findings (equal-rank conflicts)
  6. Compute evidence gaps (required − available)
  7. Build full PolicyEvaluationTrace with snapshot-pinned refs
- [ ] Implement point-in-time reconstruction:
  - Given a DecisionRecord, reconstruct the full chain: decision → rule → ruleset → interpretation → legal text + section
- [ ] Integrate with Context Resolution API (Phase 7):
  - CoverageSpec matching during case resolution
  - Merge semantics on overlapping policy outputs
  - Policy verdicts in `ContextResolutionResponse`
  - Policy-driven action ranking
- [ ] Integrate with DecisionRecord / snapshot_manifest:
  - Policy provenance chain carried on every decision
- [ ] Integration tests: end-to-end evaluation from Appendix A example, point-in-time reconstruction

→ IMMEDIATELY proceed to Phase L9. Progress: 88%

---

## Phase L9 — Runbook Compilation
**Depends on**: Phase L8, Semantic OS Phase 8 (MCP tools)
**Files**: `src/loom/runbook.rs`

- [ ] Implement runbook compilation pipeline (§6.7):
  `compile_runbook(case: &CaseProfile, eval_trace: &PolicyEvaluationTrace) -> Result<AgentPlan>`
  1. Take evidence gaps from evaluation trace
  2. For each gap, identify closing action via ActionPolicyLinkage
  3. Assemble `AgentPlan` with steps, each carrying:
     - `ActionRef` + contract snapshot_id (pinned)
     - Parameters from PolicyRuleSet (deadlines, thresholds, handling — post-merge)
     - `PolicyProvenance` reference (which directive generated this step)
     - `expected_postconditions` (the evidence gap this step closes)
     - `fallback_steps` (from ExceptionClause substitute directives)
  4. Validate plan: proof rule compliance, ABAC clearance, contract compatibility
- [ ] Implement BPMN handoff metadata generation:
  - For durable wait steps, produce `ContinuationContract` parameters sourced from policy
- [ ] Integration tests: runbook from Appendix A scenario, verify all steps have provenance

→ IMMEDIATELY proceed to Phase L10. Progress: 93%

---

## Phase L10 — MCP Tools & Impact Analysis
**Depends on**: Phase L9, Semantic OS Phase 8-9
**Files**: `src/loom/mcp_tools.rs`, `src/loom/impact.rs`

- [ ] Implement MCP tool handlers (Appendix B.8):
  - [ ] `sem_reg_describe_policy_doc` — Load PolicyDocVersion with section_index
  - [ ] `sem_reg_describe_interpretation` — Load structured interpretation with directives
  - [ ] `sem_reg_policy_coverage` — "Which policies apply to this case?"
  - [ ] `sem_reg_policy_evaluate` — Evaluate rules against case with merge semantics
  - [ ] `sem_reg_policy_evidence_gaps` — Policy-driven gap analysis
  - [ ] `sem_reg_compile_runbook` — Full runbook compilation → AgentPlan
  - [ ] `sem_reg_policy_impact` — Impact analysis via graph traversal
  - [ ] `sem_reg_policy_test_run` — Execute all fixtures (with inheritance), return pass/fail/coverage
  - [ ] `sem_reg_policy_diff` — Structural diff of two interpretations
  - [ ] `sem_reg_policy_drift_check` — Check PolicySectionRef excerpt hashes against current doc version
- [ ] Implement impact analysis (§4.6):
  `analyze_policy_impact(old_version: Uuid, new_version: Uuid) -> PolicyImpactReport`
  - Traverse ActionPolicyLinkage records (which actions are linked)
  - Traverse PolicyProvenance fields on PolicyRule snapshots (which rules derive from policy)
  - Traverse CoverageSpec.domain_tags (which domain areas affected)
  - No codebase search required — pure graph traversal over Loom artifacts
- [ ] Implement Policy Traceability taxonomy materialization:
  - Classify actions by the policies that drive them (from ActionPolicyLinkage)
  - Materialize during PolicyRuleSet compilation
- [ ] Implement policy coverage metrics:
  - Per-ruleset fixture coverage percentage
  - Cross-jurisdiction coverage gaps
  - Drift detection summary across all active policies
- [ ] Integration tests: MCP tool round-trips, impact analysis on v12→v13 change

→ Progress: 100%. E-invariant: all phases complete, all publish gates wired, full provenance chain from legal text to decision record.

---

## Database Migration Summary

All migrations go in `migrations/` directory, numbered sequentially after existing migrations.

| Table | Key Columns | Notes |
|---|---|---|
| `policy_doc_families` | `id (PK)`, `family_id (UNIQUE)`, `display_name`, `issuer`, `domain_tags (JSONB)` | Immutable family_id |
| `policy_doc_versions` | `id (PK)`, `family_id (FK)`, `snapshot_id (UNIQUE)`, `version_label`, `content_hash`, `section_index (JSONB)`, `jurisdiction_scope`, `effective_from`, `effective_until`, `status` | Active version constraint via unique partial index on `(family_id, jurisdiction_scope) WHERE status = 'Active'` |
| `policy_interpretations` | `id (PK)`, `snapshot_id (UNIQUE)`, `policy_doc_version_ref (FK)`, `body (JSONB)`, `review_status`, `dispute_reason` | JSONB body contains all directives |
| `policy_rulesets` | `id (PK)`, `snapshot_id (UNIQUE)`, `interpretation_ref (FK)`, `policy_doc_version_ref (FK)`, `body (JSONB)`, `status` | Compilation artifact |
| `action_policy_linkages` | `id (PK)`, `ruleset_id (FK)`, `action_ref (JSONB)`, `linkage_type`, `affected_elements (JSONB)`, `policy_section_ref (JSONB)` | Materialized for impact analysis |
| `policy_test_fixtures` | `id (PK)`, `snapshot_id (UNIQUE)`, `ruleset_ref (FK)`, `body (JSONB)`, `superseded_by`, `deprecated` | Fixture gate input |
| `policy_provenance` | `policy_rule_id (FK)`, `provenance (JSONB)` | Extends existing PolicyRule table |
| `governance_findings` | `id (PK)`, `evaluation_id`, `policy_refs (JSONB)`, `output_class`, `description` | Audit trail for merge conflicts |

---

## Cross-Cutting Concerns

- [ ] **Serde**: all Loom types must round-trip through JSONB cleanly. Use `#[serde(tag = "type")]` for enum variants.
- [ ] **Snapshot integration**: all Loom objects participate in Semantic OS snapshot store. Use existing `snapshot_id` / `governance_tier` / `trust_class` / `security_label` infrastructure.
- [ ] **ABAC**: policy objects inherit security handling from the Semantic OS ABAC framework. Handling directives compile to SecurityLabel fields.
- [ ] **Determinism**: NO randomness, NO LLM inference in the compilation chain. The path from Approved interpretation to Active RuleSet is pure function.
- [ ] **Dot-notation discovery**: implement prefix-based query `SELECT * FROM policy_doc_families WHERE family_id LIKE $1` for agent discovery patterns like `bny.kyc.policy.ubo.*`.
- [ ] **Error types**: define `LoomError` enum covering `CompilationBlocked`, `InterpretationDisputed`, `DriftDetected`, `FixtureFailure`, `MergeConflict`, `ProvenanceIncomplete`, `ActionRefNotFound`.
