# TODO: Workflow Definition Persistence & Extended Requirements

**Purpose**: Persist workflow definitions to DB + add requirement types for new workflows  
**Priority**: HIGH - Needed for cbu_creation, kyc_case, periodic_review, ubo_determination  
**Effort**: ~8-10 hours

---

## Current State

**What exists:**
- `rust/src/workflow/` - Full engine, repository, requirements evaluator
- `rust/config/workflows/*.yaml` - 5 workflow definitions
- `WorkflowLoader` - Loads YAML from disk at startup
- Basic `RequirementDef` types implemented

**What's missing:**
1. **Workflow definitions not persisted to DB** - Can't query "list all workflows"
2. **New requirement types** needed by cbu_creation, kyc_case, etc.

---

## Part 1: Hybrid Persistence Model

### Design

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    YAML FILES (Source of Truth)                         │
│  rust/config/workflows/                                                 │
│  ├── cbu_creation.yaml         Git versioned, PR reviewable             │
│  ├── kyc_case.yaml             Deploy to update                         │
│  ├── kyc_onboarding.yaml                                                │
│  ├── periodic_review.yaml                                               │
│  └── ubo_determination.yaml                                             │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Load on startup
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    DB TABLE (Runtime Cache)                             │
│  workflow_definitions                                                   │
│  ├── workflow_id (PK)          Fast queries                             │
│  ├── version                   Version tracking                         │
│  ├── definition_json           Full workflow as JSONB                   │
│  ├── loaded_at                 When loaded from YAML                    │
│  └── hash                      Content hash for change detection        │
└─────────────────────────────────────────────────────────────────────────┘
```

### Database Schema

```sql
-- Add to migrations

-- Workflow definitions cache (loaded from YAML on startup)
CREATE TABLE "ob-poc".workflow_definitions (
    workflow_id VARCHAR(100) PRIMARY KEY,
    version INTEGER NOT NULL,
    description TEXT,
    definition_json JSONB NOT NULL,
    content_hash VARCHAR(64) NOT NULL,  -- SHA-256 of YAML content
    loaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT uq_workflow_version UNIQUE (workflow_id, version)
);

-- Index for listing workflows
CREATE INDEX idx_workflow_defs_loaded ON "ob-poc".workflow_definitions (loaded_at DESC);

-- View for easy querying
CREATE VIEW "ob-poc".workflow_summary AS
SELECT 
    workflow_id,
    version,
    description,
    definition_json->'states' as states,
    jsonb_array_length(definition_json->'transitions') as transition_count,
    loaded_at
FROM "ob-poc".workflow_definitions;
```

### Loader Update

```rust
// rust/src/workflow/definition.rs - Add to WorkflowLoader

impl WorkflowLoader {
    /// Load workflows from YAML and sync to database
    pub async fn load_and_sync(
        dir: &Path,
        pool: &PgPool,
    ) -> Result<HashMap<String, WorkflowDefinition>, WorkflowError> {
        let definitions = Self::load_from_dir(dir)?;
        
        for (workflow_id, def) in &definitions {
            Self::sync_to_db(pool, workflow_id, def).await?;
        }
        
        Ok(definitions)
    }
    
    /// Sync a single workflow definition to database
    async fn sync_to_db(
        pool: &PgPool,
        workflow_id: &str,
        def: &WorkflowDefinition,
    ) -> Result<(), WorkflowError> {
        let json = serde_json::to_value(def)?;
        let hash = Self::content_hash(&json);
        
        sqlx::query(r#"
            INSERT INTO "ob-poc".workflow_definitions 
            (workflow_id, version, description, definition_json, content_hash, loaded_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (workflow_id) DO UPDATE SET
                version = EXCLUDED.version,
                description = EXCLUDED.description,
                definition_json = EXCLUDED.definition_json,
                content_hash = EXCLUDED.content_hash,
                loaded_at = NOW()
            WHERE workflow_definitions.content_hash != EXCLUDED.content_hash
        "#)
        .bind(workflow_id)
        .bind(def.version as i32)
        .bind(&def.description)
        .bind(&json)
        .bind(&hash)
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    fn content_hash(json: &serde_json::Value) -> String {
        use sha2::{Sha256, Digest};
        let content = serde_json::to_string(json).unwrap();
        let hash = Sha256::digest(content.as_bytes());
        format!("{:x}", hash)
    }
}
```

### MCP Tool: list_workflows

```rust
// Add to tools.rs

Tool {
    name: "list_workflows".into(),
    description: "List all available workflow definitions".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "include_states": {
                "type": "boolean",
                "default": false,
                "description": "Include state definitions in response"
            }
        }
    }),
},

// Add to handlers.rs

async fn list_workflows(&self, args: Value) -> Result<Value> {
    let include_states = args["include_states"].as_bool().unwrap_or(false);
    
    let workflows: Vec<WorkflowSummary> = sqlx::query_as(r#"
        SELECT workflow_id, version, description,
               CASE WHEN $1 THEN definition_json->'states' ELSE NULL END as states,
               jsonb_array_length(definition_json->'transitions') as transition_count
        FROM "ob-poc".workflow_definitions
        ORDER BY workflow_id
    "#)
    .bind(include_states)
    .fetch_all(&self.pool)
    .await?;
    
    Ok(serde_json::to_value(workflows)?)
}
```

---

## Part 2: Extended Requirement Types

The new workflows need these additional requirement types:

### 2.1 Add to `RequirementDef` enum

```rust
// rust/src/workflow/definition.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequirementDef {
    // --- Existing types ---
    RoleCount { role: String, min: u32, description: String },
    AllEntitiesScreened { description: String },
    DocumentSet { documents: Vec<String>, description: String },
    PerEntityDocument { entity_type: String, documents: Vec<String>, description: String },
    OwnershipComplete { threshold: f64, description: String },
    AllUbosVerified { description: String },
    NoOpenAlerts { description: String },
    CaseChecklistComplete { description: String },
    Custom { code: String, params: HashMap<String, serde_json::Value>, description: String },
    
    // --- NEW: Field validation ---
    /// Check that specified fields have non-null values
    FieldPresent {
        fields: Vec<String>,
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Product/Service ---
    /// At least N products assigned
    ProductAssigned {
        #[serde(default = "default_min_one")]
        min: u32,
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Relationships ---
    /// Check relationship exists (e.g., MANAGEMENT_COMPANY, UMBRELLA)
    RelationshipExists {
        relationship_type: String,
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Conditional requirements ---
    /// Apply requirement only when condition is met
    Conditional {
        condition: ConditionalCheck,
        requirement: Box<RequirementDef>,
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Case requirements ---
    /// KYC case exists for this subject
    CaseExists {
        #[serde(default)]
        case_type: Option<String>,
        #[serde(default)]
        description: String,
    },
    
    /// Analyst assigned to case
    AnalystAssigned {
        #[serde(default)]
        description: String,
    },
    
    /// Risk rating has been set
    RiskRatingSet {
        #[serde(default)]
        description: String,
    },
    
    /// Case approval/rejection recorded
    ApprovalRecorded {
        #[serde(default)]
        description: String,
    },
    
    RejectionRecorded {
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Data freshness ---
    /// Entity data refreshed within N days
    EntityDataCurrent {
        max_age_days: u32,
        #[serde(default)]
        description: String,
    },
    
    /// All screenings run within N days
    AllScreeningsCurrent {
        max_age_days: u32,
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: UBO-specific ---
    /// All ownership chains traced to natural persons
    ChainsResolvedToPersons {
        #[serde(default)]
        description: String,
    },
    
    /// UBO threshold applied and UBOs identified
    UboThresholdApplied {
        #[serde(default = "default_ubo_threshold")]
        threshold: f64,
        #[serde(default)]
        description: String,
    },
    
    /// UBO register complete
    UboRegisterComplete {
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Workstream requirements ---
    /// Entity workstreams created for all linked entities
    EntityWorkstreamsCreated {
        #[serde(default)]
        description: String,
    },
    
    /// All workstreams have required data
    AllWorkstreamsDataComplete {
        #[serde(default)]
        description: String,
    },
    
    /// No pending screening hits
    NoPendingHits {
        #[serde(default)]
        description: String,
    },
    
    // --- NEW: Sign-off ---
    /// Sign-off recorded
    SignOffRecorded {
        #[serde(default)]
        description: String,
    },
    
    /// Next review date scheduled
    NextReviewScheduled {
        #[serde(default)]
        description: String,
    },
}

/// Condition for conditional requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalCheck {
    pub field: String,
    #[serde(default)]
    pub equals: Option<String>,
    #[serde(rename = "in", default)]
    pub in_values: Vec<String>,
}

fn default_min_one() -> u32 { 1 }
fn default_ubo_threshold() -> f64 { 25.0 }
```

### 2.2 Implement Evaluators

```rust
// rust/src/workflow/requirements.rs - Add implementations

impl RequirementEvaluator {
    pub async fn evaluate(
        &self,
        req: &RequirementDef,
        subject_id: Uuid,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        match req {
            // ... existing cases ...
            
            RequirementDef::FieldPresent { fields, description } => {
                self.check_fields_present(subject_id, fields, description).await
            }
            
            RequirementDef::ProductAssigned { min, description } => {
                self.check_product_assigned(subject_id, *min, description).await
            }
            
            RequirementDef::RelationshipExists { relationship_type, description } => {
                self.check_relationship_exists(subject_id, relationship_type, description).await
            }
            
            RequirementDef::Conditional { condition, requirement, description } => {
                self.check_conditional(subject_id, condition, requirement, description).await
            }
            
            RequirementDef::CaseExists { case_type, description } => {
                self.check_case_exists(subject_id, case_type.as_deref(), description).await
            }
            
            RequirementDef::AnalystAssigned { description } => {
                self.check_analyst_assigned(subject_id, description).await
            }
            
            RequirementDef::RiskRatingSet { description } => {
                self.check_risk_rating_set(subject_id, description).await
            }
            
            RequirementDef::EntityDataCurrent { max_age_days, description } => {
                self.check_entity_data_current(subject_id, *max_age_days, description).await
            }
            
            RequirementDef::AllScreeningsCurrent { max_age_days, description } => {
                self.check_screenings_current(subject_id, *max_age_days, description).await
            }
            
            RequirementDef::ChainsResolvedToPersons { description } => {
                self.check_chains_resolved(subject_id, description).await
            }
            
            RequirementDef::UboThresholdApplied { threshold, description } => {
                self.check_ubo_threshold(subject_id, *threshold, description).await
            }
            
            RequirementDef::EntityWorkstreamsCreated { description } => {
                self.check_workstreams_created(subject_id, description).await
            }
            
            RequirementDef::NoPendingHits { description } => {
                self.check_no_pending_hits(subject_id, description).await
            }
            
            // ... etc for all new types
        }
    }
    
    // --- New evaluator implementations ---
    
    async fn check_fields_present(
        &self,
        cbu_id: Uuid,
        fields: &[String],
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let mut blockers = Vec::new();
        
        for field in fields {
            // Dynamic field check - query the column
            let is_null: bool = sqlx::query_scalar(&format!(
                r#"SELECT {} IS NULL FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                field
            ))
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?;
            
            if is_null {
                blockers.push(
                    Blocker::new(
                        BlockerType::FieldMissing { field: field.clone() },
                        format!("{} is required", field.replace('_', " ")),
                    )
                    .with_resolution("cbu.update")
                    .with_detail("field", serde_json::json!(field))
                );
            }
        }
        
        Ok(blockers)
    }
    
    async fn check_product_assigned(
        &self,
        cbu_id: Uuid,
        min: u32,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let count: i64 = sqlx::query_scalar(r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_products
            WHERE cbu_id = $1
        "#)
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await?;
        
        if (count as u32) < min {
            Ok(vec![
                Blocker::new(
                    BlockerType::MissingProduct { required: min, current: count as u32 },
                    if description.is_empty() {
                        format!("At least {} product(s) required", min)
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("cbu.add-product")
            ])
        } else {
            Ok(vec![])
        }
    }
    
    async fn check_relationship_exists(
        &self,
        cbu_id: Uuid,
        relationship_type: &str,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let exists: bool = sqlx::query_scalar(r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".fund_relationships
                WHERE fund_cbu_id = $1
                AND relationship_type = $2
            )
        "#)
        .bind(cbu_id)
        .bind(relationship_type)
        .fetch_one(&self.pool)
        .await?;
        
        if !exists {
            let verb = match relationship_type {
                "MANAGEMENT_COMPANY" => "fund.link-management-company",
                "UMBRELLA" => "fund.link-umbrella",
                _ => "cbu.set-relationship",
            };
            
            Ok(vec![
                Blocker::new(
                    BlockerType::MissingRelationship { relationship_type: relationship_type.to_string() },
                    if description.is_empty() {
                        format!("{} relationship required", relationship_type.replace('_', " ").to_lowercase())
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution(verb)
            ])
        } else {
            Ok(vec![])
        }
    }
    
    async fn check_conditional(
        &self,
        subject_id: Uuid,
        condition: &ConditionalCheck,
        requirement: &RequirementDef,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // First check if condition applies
        let field_value: Option<String> = sqlx::query_scalar(&format!(
            r#"SELECT {}::text FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            condition.field
        ))
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await?;
        
        let condition_met = match (&condition.equals, &condition.in_values, &field_value) {
            (Some(eq), _, Some(val)) => val == eq,
            (_, values, Some(val)) if !values.is_empty() => values.contains(val),
            _ => false,
        };
        
        if condition_met {
            // Condition met, evaluate the nested requirement
            self.evaluate(requirement, subject_id).await
        } else {
            // Condition not met, requirement doesn't apply
            Ok(vec![])
        }
    }
    
    async fn check_case_exists(
        &self,
        cbu_id: Uuid,
        case_type: Option<&str>,
        description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let exists: bool = if let Some(ct) = case_type {
            sqlx::query_scalar(r#"
                SELECT EXISTS(
                    SELECT 1 FROM kyc.cases
                    WHERE cbu_id = $1 AND case_type = $2
                )
            "#)
            .bind(cbu_id)
            .bind(ct)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_scalar(r#"
                SELECT EXISTS(SELECT 1 FROM kyc.cases WHERE cbu_id = $1)
            "#)
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?
        };
        
        if !exists {
            Ok(vec![
                Blocker::new(
                    BlockerType::NoCaseExists { case_type: case_type.map(String::from) },
                    if description.is_empty() {
                        "KYC case required".to_string()
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("kyc-case.create")
            ])
        } else {
            Ok(vec![])
        }
    }
    
    async fn check_screenings_current(
        &self,
        cbu_id: Uuid,
        max_age_days: u32,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let stale: Vec<(Uuid, String)> = sqlx::query_as(r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".screenings s
                WHERE s.entity_id = e.entity_id
                AND s.screened_at > NOW() - ($2 || ' days')::INTERVAL
            )
        "#)
        .bind(cbu_id)
        .bind(max_age_days as i32)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(stale.iter().map(|(id, name)| {
            Blocker::new(
                BlockerType::StaleScreening { 
                    entity_id: *id, 
                    max_age_days,
                },
                format!("Screening for {} is over {} days old", name, max_age_days),
            )
            .with_resolution("case-screening.run")
            .with_detail("entity_id", serde_json::json!(id))
        }).collect())
    }
    
    async fn check_chains_resolved(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Find ownership chains that don't terminate in natural persons or exempt entities
        let unresolved: Vec<(Uuid, String)> = sqlx::query_as(r#"
            WITH RECURSIVE chain AS (
                SELECT o.owner_entity_id, o.owned_entity_id, 1 as depth
                FROM "ob-poc".ownership_relationships o
                JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
                WHERE cer.cbu_id = $1
                
                UNION ALL
                
                SELECT o.owner_entity_id, c.owned_entity_id, c.depth + 1
                FROM "ob-poc".ownership_relationships o
                JOIN chain c ON o.owned_entity_id = c.owner_entity_id
                WHERE c.depth < 10
            )
            SELECT DISTINCT e.entity_id, e.name
            FROM chain c
            JOIN "ob-poc".entities e ON c.owner_entity_id = e.entity_id
            WHERE e.entity_type NOT IN ('PROPER_PERSON', 'NATURAL_PERSON')
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".ubo_exemptions ex
                WHERE ex.entity_id = e.entity_id
            )
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".ownership_relationships o2
                WHERE o2.owned_entity_id = e.entity_id
            )
        "#)
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(unresolved.iter().map(|(id, name)| {
            Blocker::new(
                BlockerType::UnresolvedOwnershipChain { entity_id: *id },
                format!("Ownership chain for {} not traced to natural person", name),
            )
            .with_resolution("ubo.add-ownership")
            .with_detail("entity_id", serde_json::json!(id))
        }).collect())
    }
    
    // ... additional implementations for remaining types ...
}
```

### 2.3 Add New BlockerTypes

```rust
// rust/src/workflow/state.rs - Add to BlockerType enum

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockerType {
    // ... existing types ...
    
    /// Field is missing/null
    FieldMissing {
        field: String,
    },
    
    /// Product not assigned
    MissingProduct {
        required: u32,
        current: u32,
    },
    
    /// Relationship doesn't exist
    MissingRelationship {
        relationship_type: String,
    },
    
    /// No KYC case exists
    NoCaseExists {
        case_type: Option<String>,
    },
    
    /// Analyst not assigned
    NoAnalystAssigned,
    
    /// Risk rating not set
    RiskRatingNotSet,
    
    /// Screening data is stale
    StaleScreening {
        entity_id: Uuid,
        max_age_days: u32,
    },
    
    /// Entity data is stale
    StaleEntityData {
        entity_id: Uuid,
        max_age_days: u32,
    },
    
    /// Ownership chain not resolved
    UnresolvedOwnershipChain {
        entity_id: Uuid,
    },
    
    /// Workstream not created
    WorkstreamMissing {
        entity_id: Uuid,
    },
    
    /// Pending screening hit
    PendingHit {
        screening_id: Uuid,
        entity_id: Uuid,
    },
    
    /// Sign-off not recorded
    SignOffMissing,
    
    /// Next review not scheduled
    NextReviewNotScheduled,
}
```

---

## Part 3: Workflow YAML Files

The following workflows are already created in `rust/config/workflows/`:

### cbu_creation.yaml
```yaml
# CBU Creation Workflow
# Initial setup of a Client Business Unit before KYC onboarding begins

workflow: cbu_creation
version: 1
description: Initial CBU setup and data capture

trigger:
  event: cbu.created
  conditions:
    - field: status
      equals: DRAFT

states:
  DRAFT:
    description: Initial creation, basic details only
    initial: true

  DATA_CAPTURE:
    description: Capturing core CBU information

  STRUCTURE_SETUP:
    description: Setting up fund structure, relationships, products

  READY_FOR_KYC:
    description: CBU setup complete, ready to start KYC
    terminal: true

  CANCELLED:
    description: CBU creation cancelled
    terminal: true

transitions:
  - from: DRAFT
    to: DATA_CAPTURE
    auto: true

  - from: DATA_CAPTURE
    to: STRUCTURE_SETUP
    guard: basic_data_complete

  - from: STRUCTURE_SETUP
    to: READY_FOR_KYC
    guard: structure_complete

  - from: DRAFT
    to: CANCELLED
    manual: true

  - from: DATA_CAPTURE
    to: CANCELLED
    manual: true

  - from: STRUCTURE_SETUP
    to: CANCELLED
    manual: true

requirements:
  DATA_CAPTURE:
    - type: field_present
      fields: [name, jurisdiction, client_type]
      description: Basic CBU details required

  STRUCTURE_SETUP:
    - type: field_present
      fields: [name, jurisdiction, client_type, domicile]
      description: Core CBU data required

    - type: role_count
      role: PRIMARY_CONTACT
      min: 1
      description: At least one primary contact

  READY_FOR_KYC:
    - type: product_assigned
      min: 1
      description: At least one product assigned

    - type: conditional
      condition:
        field: client_type
        in: [FUND, SUB_FUND]
      requirement:
        type: relationship_exists
        relationship_type: MANAGEMENT_COMPANY
      description: Funds must have management company linked

    - type: conditional
      condition:
        field: client_type
        equals: SUB_FUND
      requirement:
        type: relationship_exists
        relationship_type: UMBRELLA
      description: Sub-funds must be linked to umbrella

actions:
  DRAFT:
    - action: update_cbu
      verb: cbu.update

  DATA_CAPTURE:
    - action: update_cbu
      verb: cbu.update
    - action: set_domicile
      verb: cbu.set-domicile
    - action: add_contact
      verb: cbu.assign-role

  STRUCTURE_SETUP:
    - action: add_product
      verb: cbu.add-product
    - action: link_management_company
      verb: fund.link-management-company
    - action: link_umbrella
      verb: fund.link-umbrella
    - action: add_service
      verb: cbu.add-service
```

### kyc_case.yaml
```yaml
# KYC Case Workflow
# Manages the lifecycle of a KYC review case

workflow: kyc_case
version: 1
description: KYC case review lifecycle

trigger:
  event: kyc-case.created
  conditions:
    - field: case_type
      in: [INITIAL, PERIODIC_REVIEW, TRIGGER_EVENT, ENHANCED_DUE_DILIGENCE]

states:
  OPENED:
    description: Case opened, awaiting assignment
    initial: true

  ASSIGNED:
    description: Case assigned to analyst

  DATA_GATHERING:
    description: Collecting entity data and documents

  SCREENING:
    description: Running AML/PEP/sanctions screening

  ANALYSIS:
    description: Analyst reviewing findings

  ESCALATED:
    description: Escalated for senior review

  PENDING_APPROVAL:
    description: Awaiting final approval

  APPROVED:
    description: Case approved
    terminal: true

  REJECTED:
    description: Case rejected
    terminal: true

  ON_HOLD:
    description: Case on hold pending external input

  CLOSED_NO_ACTION:
    description: Case closed without decision
    terminal: true

transitions:
  - from: OPENED
    to: ASSIGNED
    guard: analyst_assigned

  - from: ASSIGNED
    to: DATA_GATHERING
    auto: true

  - from: DATA_GATHERING
    to: SCREENING
    guard: data_gathering_complete

  - from: SCREENING
    to: ANALYSIS
    guard: screening_complete

  - from: ANALYSIS
    to: PENDING_APPROVAL
    guard: analysis_complete

  - from: PENDING_APPROVAL
    to: APPROVED
    guard: approval_granted
    manual: true

  - from: PENDING_APPROVAL
    to: REJECTED
    guard: rejection_confirmed
    manual: true

  - from: ANALYSIS
    to: ESCALATED
    manual: true

  - from: ESCALATED
    to: ANALYSIS
    manual: true

  - from: ESCALATED
    to: PENDING_APPROVAL
    guard: escalation_resolved

  - from: DATA_GATHERING
    to: ON_HOLD
    manual: true

  - from: SCREENING
    to: ON_HOLD
    manual: true

  - from: ANALYSIS
    to: ON_HOLD
    manual: true

  - from: ON_HOLD
    to: DATA_GATHERING
    manual: true

  - from: ON_HOLD
    to: SCREENING
    manual: true

  - from: ON_HOLD
    to: ANALYSIS
    manual: true

  - from: OPENED
    to: CLOSED_NO_ACTION
    manual: true

  - from: ON_HOLD
    to: CLOSED_NO_ACTION
    manual: true

requirements:
  ASSIGNED:
    - type: analyst_assigned
      description: Analyst must be assigned

  DATA_GATHERING:
    - type: entity_workstreams_created
      description: Entity workstreams created

  SCREENING:
    - type: all_workstreams_data_complete
      description: All workstreams have required data

  ANALYSIS:
    - type: all_entities_screened
      description: All screening checks completed
    - type: no_pending_hits
      description: All screening hits reviewed

  PENDING_APPROVAL:
    - type: risk_rating_set
      description: Case risk rating determined
    - type: case_checklist_complete
      description: All checklist items completed
    - type: no_open_alerts
      description: All red flags resolved

  APPROVED:
    - type: approval_recorded
      description: Approval decision recorded

  REJECTED:
    - type: rejection_recorded
      description: Rejection decision recorded

actions:
  OPENED:
    - action: assign_analyst
      verb: kyc-case.assign
    - action: set_priority
      verb: kyc-case.set-priority

  ASSIGNED:
    - action: create_workstream
      verb: entity-workstream.create
    - action: add_checklist_item
      verb: kyc-case.add-checklist-item

  DATA_GATHERING:
    - action: catalog_document
      verb: document.catalog
    - action: extract_document
      verb: document.extract
    - action: update_workstream
      verb: entity-workstream.update

  SCREENING:
    - action: run_screening
      verb: case-screening.run
    - action: review_hit
      verb: case-screening.review-hit
    - action: clear_hit
      verb: case-screening.clear-hit
    - action: confirm_hit
      verb: case-screening.confirm-hit

  ANALYSIS:
    - action: raise_red_flag
      verb: red-flag.raise
    - action: mitigate_red_flag
      verb: red-flag.mitigate
    - action: set_risk_rating
      verb: kyc-case.set-risk-rating
    - action: complete_checklist
      verb: kyc-case.complete-checklist-item
    - action: escalate
      verb: kyc-case.escalate

  PENDING_APPROVAL:
    - action: approve
      verb: kyc-case.approve
    - action: reject
      verb: kyc-case.reject
    - action: request_changes
      verb: kyc-case.request-changes
```

### periodic_review.yaml and ubo_determination.yaml
(Already in rust/config/workflows/ - see files for full content)

---

## Implementation Checklist

### Part 1: DB Persistence
- [ ] Add migration for `workflow_definitions` table
- [ ] Update `WorkflowLoader` with `load_and_sync()` method
- [ ] Add content hashing for change detection
- [ ] Add `list_workflows` MCP tool
- [ ] Update engine startup to sync definitions to DB

### Part 2: Extended RequirementDef
- [ ] Add `FieldPresent` type + evaluator
- [ ] Add `ProductAssigned` type + evaluator
- [ ] Add `RelationshipExists` type + evaluator
- [ ] Add `Conditional` type + evaluator
- [ ] Add `CaseExists` type + evaluator
- [ ] Add `AnalystAssigned` type + evaluator
- [ ] Add `RiskRatingSet` type + evaluator
- [ ] Add `EntityDataCurrent` type + evaluator
- [ ] Add `AllScreeningsCurrent` type + evaluator
- [ ] Add `ChainsResolvedToPersons` type + evaluator
- [ ] Add `EntityWorkstreamsCreated` type + evaluator
- [ ] Add `NoPendingHits` type + evaluator
- [ ] Add `SignOffRecorded` type + evaluator
- [ ] Add `NextReviewScheduled` type + evaluator

### Part 3: BlockerTypes
- [ ] Add `FieldMissing` blocker type
- [ ] Add `MissingProduct` blocker type
- [ ] Add `MissingRelationship` blocker type
- [ ] Add `NoCaseExists` blocker type
- [ ] Add `StaleScreening` blocker type
- [ ] Add `UnresolvedOwnershipChain` blocker type
- [ ] Add remaining blocker types

### Part 4: Testing
- [ ] Test workflow definition loading and sync
- [ ] Test each new requirement type
- [ ] Test conditional requirements
- [ ] Test cbu_creation workflow end-to-end
- [ ] Test kyc_case workflow end-to-end

---

## Effort Estimate

| Task | Hours |
|------|-------|
| DB persistence (migration + loader) | 2 |
| list_workflows MCP tool | 0.5 |
| Extended RequirementDef types | 3 |
| Requirement evaluators | 3 |
| New BlockerTypes | 1 |
| Testing | 2 |
| **Total** | **~11-12 hours** |
