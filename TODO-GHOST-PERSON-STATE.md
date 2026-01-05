# TODO: Ghost Person State

## Problem

Traditional systems require full identity attributes (DOB, nationality, etc.) before creating a person entity. But documents frequently reference people by name only:

- "Director: J Smith"
- "Beneficiary: Mrs. Chen"
- "UBO: Unknown individual - 25%"

Current workarounds are broken:
- Dummy data ("01/01/1900" DOB)
- Blocked workflows ("cannot assign role - person doesn't exist")
- Lost information (can't record what document said)

## Solution

Introduce `PersonState` to separate **role assignment** from **identity resolution**:

| State | Meaning | Attributes |
|-------|---------|------------|
| `Ghost` | Name placeholder - someone exists in this role | Name only (maybe partial) |
| `Identified` | Sufficient attributes to distinguish from others | Name + DOB + Nationality (or equivalent) |
| `Verified` | Identity proven by evidence | ID document linked, verification complete |

**Key principle:** Role assignment is independent of identity state. A Ghost can be a Director.

---

## Implementation

### Step 1: Add PersonState Enum

**File:** `rust/src/graph/types.rs` (or appropriate types module)

```rust
/// Identity resolution state for natural persons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersonState {
    /// Name placeholder - we know someone exists but can't uniquely identify
    /// "Document says there's a director called J Smith"
    #[default]
    Ghost,
    
    /// Sufficient attributes to distinguish identity
    /// Name + DOB + Nationality (or similar combination)
    Identified,
    
    /// Identity proven by evidence (ID document, verification service)
    Verified,
}

impl PersonState {
    /// Can this person be used for KYC completion?
    pub fn is_kyc_ready(&self) -> bool {
        matches!(self, Self::Identified | Self::Verified)
    }
    
    /// Does this person need identity work?
    pub fn needs_identification(&self) -> bool {
        matches!(self, Self::Ghost)
    }
    
    /// Is identity proven?
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }
}
```

### Step 2: Database Migration

**File:** `migrations/YYYYMMDD_XXXX_person_state_up.sql`

```sql
-- +goose Up

-- Add person_state enum type
CREATE TYPE person_state AS ENUM ('GHOST', 'IDENTIFIED', 'VERIFIED');

-- Add column to natural persons
ALTER TABLE "ob-poc".entity_natural_persons
ADD COLUMN person_state person_state NOT NULL DEFAULT 'GHOST';

-- Add column to track what triggered state transitions
ALTER TABLE "ob-poc".entity_natural_persons
ADD COLUMN state_changed_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".entity_natural_persons
ADD COLUMN state_changed_reason TEXT;

-- Index for querying ghosts needing work
CREATE INDEX ix_natural_persons_ghost 
ON "ob-poc".entity_natural_persons (person_state) 
WHERE person_state = 'GHOST';

-- Update existing persons based on attribute completeness
UPDATE "ob-poc".entity_natural_persons
SET person_state = CASE
    -- Has verification evidence = Verified
    WHEN EXISTS (
        SELECT 1 FROM "ob-poc".verifications v 
        WHERE v.entity_id = entity_natural_persons.entity_id
        AND v.status = 'PROVEN'
    ) THEN 'VERIFIED'::person_state
    -- Has DOB and nationality = Identified  
    WHEN date_of_birth IS NOT NULL 
         AND nationality IS NOT NULL 
    THEN 'IDENTIFIED'::person_state
    -- Otherwise Ghost
    ELSE 'GHOST'::person_state
END;
```

**File:** `migrations/YYYYMMDD_XXXX_person_state_down.sql`

```sql
-- +goose Down
ALTER TABLE "ob-poc".entity_natural_persons DROP COLUMN IF EXISTS state_changed_reason;
ALTER TABLE "ob-poc".entity_natural_persons DROP COLUMN IF EXISTS state_changed_at;
ALTER TABLE "ob-poc".entity_natural_persons DROP COLUMN IF EXISTS person_state;
DROP TYPE IF EXISTS person_state;
```

### Step 3: Update Entity Model

**File:** `rust/src/entities/natural_person.rs` (or equivalent)

```rust
pub struct NaturalPerson {
    pub entity_id: Uuid,
    pub name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub country_of_residence: Option<String>,
    
    // Identity state
    pub person_state: PersonState,
    pub state_changed_at: Option<DateTime<Utc>>,
    pub state_changed_reason: Option<String>,
}

impl NaturalPerson {
    /// Create a ghost person (name only)
    pub fn ghost(name: impl Into<String>) -> Self {
        Self {
            entity_id: Uuid::new_v4(),
            name: name.into(),
            date_of_birth: None,
            nationality: None,
            country_of_residence: None,
            person_state: PersonState::Ghost,
            state_changed_at: None,
            state_changed_reason: None,
        }
    }
    
    /// Check if person has sufficient attributes for identification
    pub fn can_be_identified(&self) -> bool {
        // Name + DOB + Nationality is minimum for unique identification
        self.date_of_birth.is_some() && self.nationality.is_some()
    }
    
    /// Attempt to transition from Ghost to Identified
    pub fn try_identify(&mut self, reason: &str) -> Result<(), String> {
        if self.person_state != PersonState::Ghost {
            return Err("Person is not in Ghost state".into());
        }
        if !self.can_be_identified() {
            return Err("Insufficient attributes for identification".into());
        }
        self.person_state = PersonState::Identified;
        self.state_changed_at = Some(Utc::now());
        self.state_changed_reason = Some(reason.into());
        Ok(())
    }
    
    /// Transition from Identified to Verified
    pub fn verify(&mut self, reason: &str) -> Result<(), String> {
        if self.person_state == PersonState::Ghost {
            return Err("Cannot verify Ghost - must be Identified first".into());
        }
        self.person_state = PersonState::Verified;
        self.state_changed_at = Some(Utc::now());
        self.state_changed_reason = Some(reason.into());
        Ok(())
    }
}
```

### Step 4: DSL Verbs

**File:** `config/verbs/entity.yaml` - Add ghost-specific operations

```yaml
# Create ghost person (name only placeholder)
ghost:
  description: "Create ghost person placeholder for role assignment"
  behavior: plugin
  plugin:
    handler: EntityGhostOp
  args:
    - name: name
      type: string
      required: true
      description: "Name as it appears (e.g., 'J Smith', 'Mrs. Chen')"
    - name: source
      type: string
      required: false
      description: "Where this name came from (document, verbal, etc.)"
  returns:
    type: uuid
    capture: true
    description: "Ghost person entity_id"

# Identify ghost (add attributes, transition state)
identify:
  description: "Add identifying attributes to ghost person"
  behavior: plugin
  plugin:
    handler: EntityIdentifyOp
  args:
    - name: entity-id
      type: uuid
      required: true
    - name: date-of-birth
      type: date
      required: false
    - name: nationality
      type: string
      required: false
    - name: country-of-residence
      type: string
      required: false
  returns:
    type: record
    description: "Updated person with new state"
```

### Step 5: Custom Operations

**File:** `rust/src/dsl_v2/custom_ops/entity_ops.rs` - Add handlers

```rust
/// Create ghost person placeholder
pub struct EntityGhostOp;

#[async_trait]
impl CustomOperation for EntityGhostOp {
    fn domain(&self) -> &'static str { "entity" }
    fn verb(&self) -> &'static str { "ghost" }
    fn rationale(&self) -> &'static str {
        "Creates person placeholder with Ghost state for role assignment before full identification"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let name = extract_string_required(verb_call, "name")?;
        let source = extract_string_opt(verb_call, "source");
        
        // Create base entity
        let entity_type_id: Uuid = sqlx::query_scalar(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'natural_person'"#
        )
        .fetch_one(pool)
        .await?;
        
        let entity_id = Uuid::new_v4();
        
        // Insert base entity
        sqlx::query(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name) 
               VALUES ($1, $2, $3)"#
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&name)
        .execute(pool)
        .await?;
        
        // Insert natural person with Ghost state
        sqlx::query(
            r#"INSERT INTO "ob-poc".entity_natural_persons 
               (entity_id, full_name, person_state, state_changed_reason)
               VALUES ($1, $2, 'GHOST', $3)"#
        )
        .bind(entity_id)
        .bind(&name)
        .bind(source.as_deref().unwrap_or("Created as ghost placeholder"))
        .execute(pool)
        .await?;
        
        // Bind result
        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, entity_id);
        }
        
        Ok(ExecutionResult::Uuid(entity_id))
    }
}
```

### Step 6: Graph Node Rendering

**File:** `rust/src/graph/types.rs` - Update GraphNode

```rust
pub struct GraphNode {
    // ... existing fields ...
    
    /// Person identity state (only for natural persons)
    pub person_state: Option<PersonState>,
}
```

**File:** `rust/src/graph/layout.rs` - Visual treatment

```rust
impl GraphNode {
    /// Get visual style based on person state
    pub fn person_visual_style(&self) -> PersonVisualStyle {
        match self.person_state {
            Some(PersonState::Ghost) => PersonVisualStyle {
                border: BorderStyle::Dashed,
                opacity: 0.6,
                badge: Some("👻"),
                color: Color::Gray,
            },
            Some(PersonState::Identified) => PersonVisualStyle {
                border: BorderStyle::Solid,
                opacity: 1.0,
                badge: None,
                color: Color::Normal,
            },
            Some(PersonState::Verified) => PersonVisualStyle {
                border: BorderStyle::Solid,
                opacity: 1.0,
                badge: Some("✓"),
                color: Color::Green,
            },
            None => PersonVisualStyle::default(), // Not a person
        }
    }
}
```

### Step 7: Workflow Surfacing

**File:** `rust/src/session/mod.rs` or equivalent

```rust
impl UnifiedSessionContext {
    /// Get all ghosts in current view needing identification
    pub fn ghosts_needing_work(&self) -> Vec<Uuid> {
        self.view
            .as_ref()
            .map(|v| {
                v.selection
                    .iter()
                    .filter(|id| {
                        // Check if entity is a ghost person
                        // This would need graph access
                    })
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }
}
```

Add DSL verb to query ghosts:

```yaml
# In view.yaml or query.yaml
ghosts:
  description: "List ghost persons needing identification"
  behavior: plugin
  plugin:
    handler: ViewGhostsOp
  args:
    - name: cbu-id
      type: uuid
      required: false
      description: "Filter to specific CBU"
  returns:
    type: record_list
    description: "Ghost persons with their role assignments"
```

---

## State Transitions

| From | To | Trigger | Validation |
|------|-----|---------|------------|
| Ghost | Identified | Attributes added | DOB + Nationality present |
| Ghost | Identified | Merge with known person | Target is Identified/Verified |
| Ghost | (deleted) | Prune - not real | Manual action |
| Identified | Verified | Evidence linked | ID document verified |
| Identified | Ghost | Attributes invalidated | Rare - data quality issue |
| Verified | Identified | Verification expired/revoked | Re-verification needed |

---

## Integration Points

### Role Assignment
- Ghost is valid target for role assignment
- Role assignment is an **allegation** - person state is separate
- Verification status on role vs person state are independent

### Observations
- Document extraction can create Ghosts
- Extracted attributes (DOB, nationality) trigger state transition
- Evidence observation triggers verification

### Graph/View
- Ghost nodes render with distinct visual treatment (dashed, faded, 👻)
- Filter option: "Show ghosts" / "Hide ghosts"
- Count badge: "3 persons need identification"

### KYC Workflow
- Ghost persons block KYC completion (intentionally)
- Tollgate: "All persons must be Identified" for KYC approval
- Tollgate: "All UBOs must be Verified" for high-risk

---

## Example Workflow

```clojure
;; 1. Document mentions director
@doc = (document.catalog :file "board-resolution.pdf" :cbu @cbu)

;; 2. Extraction finds "Director: J Smith" - create ghost
@ghost = (entity.ghost :name "J Smith" :source "Board Resolution 2024-01")

;; 3. Assign role (allegation)
(cbu-role.assign 
  :cbu @cbu 
  :entity @ghost 
  :role "DIRECTOR"
  :source @doc)

;; 4. Later - identify from passport
(entity.identify 
  :entity-id @ghost 
  :date-of-birth "1975-03-15"
  :nationality "DE")

;; 5. Verify with ID document
@passport = (document.catalog :file "passport.pdf" :entity @ghost)
(verification.prove 
  :entity @ghost 
  :attribute "identity"
  :evidence @passport)
```

---

## Visual Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    CBU: Allianz Luxembourg Fund                 │
│                                                                 │
│   Directors:                                                    │
│   ├── ✓ Hans Mueller (Verified)                                │
│   ├── 👤 Maria Schmidt (Identified)                            │
│   └── 👻 "J Smith" (Ghost) ← needs identification              │
│                                                                 │
│   UBOs:                                                         │
│   ├── ✓ Klaus Weber - 35% (Verified)                           │
│   └── 👻 "Beneficiary 2" - 25% (Ghost) ← who is this?          │
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  ⚠️  2 persons need identification                      │   │
│   └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Done Definition

- [ ] PersonState enum in Rust types
- [ ] Database migration adds person_state column
- [ ] Existing persons migrated based on attribute completeness
- [ ] entity.ghost verb creates Ghost placeholder
- [ ] entity.identify verb transitions Ghost → Identified
- [ ] Role assignment accepts Ghost persons
- [ ] GraphNode includes person_state for rendering
- [ ] Visual treatment: Ghost = dashed/faded/👻
- [ ] View filter: show/hide ghosts
- [ ] Tollgate integration: Ghost blocks KYC completion
- [ ] Tests for state transitions
