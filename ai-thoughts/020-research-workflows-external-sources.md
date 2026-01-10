# Research Workflows - Bounded Non-Determinism Architecture

> **Status:** Planning
> **Priority:** High - Required for UBO gap resolution
> **Created:** 2026-01-10
> **Estimated Effort:** 70-85 hours
> **Dependencies:** 
>   - 019-group-taxonomy-intra-company-ownership.md (ownership graph, gaps)
>   - Existing GLEIF integration (refactor under this pattern)
>   - CLAUDE.md and annexes (review before implementation)

---

## Implementation Preamble

**Before implementing any phase of this TODO, Claude must:**

```
1. Review /CLAUDE.md for project conventions and patterns
2. Review /docs/entity-model-ascii.md for entity taxonomy
3. Review /docs/dsl-spec.md for verb definition patterns
4. Review /docs/repl-viewport.md for session/scope context
5. Review existing GLEIF implementation as reference pattern
6. Review /rust/config/verbs/*.yaml for verb YAML conventions
```

This ensures implementation aligns with established project architecture.

---

## Core Architecture

### The Two-Phase Pattern

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    BOUNDED NON-DETERMINISM                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Research requires TWO execution models:                                   │
│                                                                              │
│   PHASE 1: LLM EXPLORATION                                                  │
│   ════════════════════════                                                   │
│   • Fuzzy name matching                                                     │
│   • Context reasoning                                                       │
│   • Source selection                                                        │
│   • Disambiguation                                                          │
│   • Confidence scoring                                                      │
│                                                                              │
│   Executed via: PROMPT TEMPLATES + LLM reasoning                            │
│   Output: IDENTIFIER (LEI, company number, CIK, etc.)                       │
│                                                                              │
│   Non-deterministic but AUDITABLE                                           │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   PHASE 2: DSL EXECUTION                                                    │
│   ══════════════════════                                                     │
│   • Fetch exact record by identifier                                        │
│   • Normalize to schema                                                     │
│   • Create/update entities                                                  │
│   • Create relationships                                                    │
│   • Audit trail                                                             │
│                                                                              │
│   Executed via: DSL VERBS (key required)                                    │
│   Output: Entities, relationships in database                               │
│                                                                              │
│   Deterministic, reproducible, idempotent                                   │
│                                                                              │
│   ═══════════════════════════════════════════════════════════════════════   │
│                                                                              │
│   THE IDENTIFIER IS THE BRIDGE                                              │
│                                                                              │
│   ┌──────────────────┐         ┌──────────────────┐                        │
│   │  FUZZY WORLD     │         │ DETERMINISTIC    │                        │
│   │                  │   KEY   │ WORLD            │                        │
│   │  "AllianzGI"     │ ──────► │                  │                        │
│   │  "that ManCo"    │   LEI   │ import-hierarchy │                        │
│   │  reasoning...    │         │ create entities  │                        │
│   └──────────────────┘         └──────────────────┘                        │
│                                                                              │
│   Prompt templates find the key                                             │
│   DSL verbs use the key                                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Why This Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    MIRRORS HUMAN ANALYST WORKFLOW                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Human analyst:                                                            │
│   1. Gets request: "Add ownership for HoldCo"                               │
│   2. Searches GLEIF, Companies House, Google...          ← Fuzzy            │
│   3. Finds candidates, applies judgment                  ← Fuzzy            │
│   4. Picks the right one based on context                ← Fuzzy            │
│   5. Notes down LEI or company number                    ← KEY              │
│   6. Runs import in system                               ← Deterministic    │
│                                                                              │
│   LLM automates steps 1-5                                                   │
│   DSL executes step 6                                                       │
│   The KEY is the handoff point                                              │
│                                                                              │
│   ═══════════════════════════════════════════════════════════════════════   │
│                                                                              │
│   WHY NOT PURE APPROACHES:                                                  │
│                                                                              │
│   Pure Deterministic:                                                       │
│   • User must provide LEI, company number, CIK...                           │
│   • But users have: "AllianzGI" or "Fund Alpha's ManCo"                     │
│   • Pushes fuzzy search to user - defeats purpose                           │
│                                                                              │
│   Pure LLM:                                                                 │
│   • LLM searches AND imports data                                           │
│   • No audit trail, no reproducibility                                      │
│   • Can't answer: "Why did X get linked to Y?"                              │
│   • Can't replay or correct                                                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Component 1: Prompt Templates

### Directory Structure

```
/prompts/research/
│
├── sources/
│   ├── gleif/
│   │   ├── search.md                 # Find LEI for entity name
│   │   ├── disambiguate.md           # Pick best from candidates
│   │   └── validate-lei.md           # Check LEI status
│   │
│   ├── companies-house/
│   │   ├── search.md                 # Find company number
│   │   ├── search-officer.md         # Find person in officers
│   │   └── disambiguate.md
│   │
│   ├── sec-edgar/
│   │   ├── search.md                 # Find CIK
│   │   ├── find-filings.md           # Find relevant filings
│   │   └── parse-13f.md              # Extract holders from 13F
│   │
│   ├── orbis/
│   │   ├── search.md                 # Find BvD ID
│   │   └── disambiguate.md
│   │
│   └── open-corporates/
│       └── search.md
│
├── screening/
│   ├── interpret-sanctions.md        # Evaluate sanctions hits
│   ├── interpret-pep.md              # Evaluate PEP status
│   └── assess-risk.md                # Overall risk assessment
│
├── documents/
│   ├── extract-ownership.md          # Extract from annual report
│   ├── extract-directors.md          # Extract board composition
│   └── parse-declaration.md          # Parse UBO declaration
│
└── orchestration/
    ├── resolve-gap.md                # Strategy for single gap
    ├── chain-research.md             # Full chain strategy
    ├── select-source.md              # Pick best source for need
    └── reconcile-conflict.md         # Handle conflicting data
```

### Prompt Template Structure

```markdown
# /prompts/research/sources/gleif/search.md

## Context
You are searching GLEIF (Global LEI Foundation) to find the LEI 
for an entity. GLEIF is the authoritative source for Legal Entity
Identifiers.

## Input
- entity_name: {{entity_name}}
- jurisdiction: {{jurisdiction}} (optional)
- entity_type: {{entity_type}} (optional)
- context: {{context}} (optional - why we're looking)

## GLEIF API
Endpoint: https://api.gleif.org/api/v1/fuzzycompletions
Parameters: field=fulltext, q={search_term}

Alternative: https://api.gleif.org/api/v1/lei-records
Filter: filter[entity.legalName]={exact_name}

## Search Strategy
1. Try exact name match first
2. If no results, remove legal suffixes (Ltd, GmbH, LLC, Inc, etc.)
3. If still no results, try key words from name
4. Filter by jurisdiction if provided
5. Only consider LEIs with status ISSUED (not LAPSED, RETIRED, etc.)

## Disambiguation Rules
When multiple candidates found:
- Prefer exact jurisdiction match
- Prefer active (not dormant) entities
- Prefer longer-established LEIs (earlier registration date)
- Check parent relationships for context clues

## Output Format
If confident match (score > 0.90):
```json
{
  "status": "found",
  "lei": "529900XXXXXXXXXXXXXX",
  "legal_name": "AllianzGI GmbH",
  "jurisdiction": "DE",
  "confidence": 0.95,
  "reasoning": "Exact name match, correct jurisdiction, active LEI"
}
```

If ambiguous (multiple candidates 0.70-0.90):
```json
{
  "status": "ambiguous",
  "candidates": [
    {"lei": "...", "name": "...", "score": 0.85, "jurisdiction": "DE"},
    {"lei": "...", "name": "...", "score": 0.82, "jurisdiction": "US"}
  ],
  "clarification_needed": "Multiple entities with similar names. Is this the German or US entity?"
}
```

If no match (all scores < 0.70):
```json
{
  "status": "not_found",
  "search_terms_tried": ["AllianzGI GmbH", "AllianzGI", "Allianz Global Investors"],
  "suggestion": "Entity may not have an LEI. Try Companies House or Orbis."
}
```

## Critical Rules
- NEVER fabricate an LEI
- NEVER assume - if unsure, return ambiguous
- Always include reasoning for selection
- Log all API calls made
```

### Orchestration Prompts

```markdown
# /prompts/research/orchestration/resolve-gap.md

## Context
You are resolving an ownership gap - a point in the ownership chain
where we don't have complete information.

## Input
- gap_type: {{gap_type}}
- entity_id: {{entity_id}}
- entity_name: {{entity_name}}
- jurisdiction: {{jurisdiction}}
- known_context: {{context}}

## Gap Types and Strategies

### BROKEN_CHAIN (non-terminal entity with no parent)
Priority sources:
1. GLEIF - if entity likely has LEI (large, regulated, international)
2. Companies House - if UK entity
3. SEC EDGAR - if US public company
4. Orbis - fallback for commercial data
5. OpenCorporates - broad coverage, less depth

### NOMINEE_HOLDING (legal owner known, beneficial unknown)
Strategy:
1. Check if nominee is known custodian (Clearstream, Euroclear, etc.)
2. If yes, outreach.request-nominee-disclosure
3. If no, check Orbis for nominee's own ownership

### UNKNOWN_PERSON (natural person not verified)
Strategy:
1. screening.sanctions - check sanctions lists
2. screening.pep - check PEP status
3. identity.verify - if high-risk or material holding

### UNACCOUNTED_SHARES (gap in cap table)
Strategy:
1. outreach.request-share-register
2. Check for recent corporate actions (splits, buybacks)
3. May need registrar reconciliation

## Source Selection Logic
```
jurisdiction == "GB" AND entity_type == "COMPANY" → companies-house
jurisdiction == "US" AND is_public → sec-edgar
has_lei OR is_large_corporate → gleif
else → orbis OR open-corporates
```

## Output Format
```json
{
  "strategy": [
    {"step": 1, "source": "gleif", "action": "search", "params": {...}},
    {"step": 2, "source": "companies-house", "action": "search", "if": "gleif.not_found"},
    {"step": 3, "action": "import", "source": "best_match", "verb": "research.{source}.import-hierarchy"}
  ],
  "confidence_threshold": 0.85,
  "human_checkpoint": false,
  "reasoning": "UK company likely in Companies House, may also have LEI"
}
```
```

---

## Component 2: Decision Logging Schema

### Research Decisions Table

```sql
-- =============================================================================
-- RESEARCH DECISIONS (audit trail for Phase 1)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What triggered this research
    trigger_id UUID REFERENCES kyc.ownership_research_triggers(trigger_id),
    target_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- Search context
    search_query TEXT NOT NULL,
    search_context JSONB,  -- jurisdiction, entity_type, etc.
    
    -- Source used
    source_provider VARCHAR(30) NOT NULL,
    
    -- Candidates found
    candidates_found JSONB NOT NULL,  -- [{key, name, score, metadata}]
    candidates_count INTEGER NOT NULL,
    
    -- Selection
    selected_key VARCHAR(100),  -- LEI, company number, CIK, etc.
    selected_key_type VARCHAR(20),  -- LEI, COMPANY_NUMBER, CIK, BVD_ID
    selection_confidence DECIMAL(3,2),  -- 0.00 - 1.00
    selection_reasoning TEXT NOT NULL,
    
    -- Decision type
    decision_type VARCHAR(20) NOT NULL,
    
    -- Verification
    auto_selected BOOLEAN NOT NULL DEFAULT false,
    verified_by UUID,  -- User who confirmed (if not auto)
    verified_at TIMESTAMPTZ,
    
    -- Link to resulting action
    resulting_action_id UUID,  -- Points to research_actions if executed
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    session_id UUID,
    
    CONSTRAINT chk_decision_type CHECK (
        decision_type IN (
            'AUTO_SELECTED',     -- High confidence, proceeded automatically
            'USER_SELECTED',     -- User picked from candidates
            'USER_CONFIRMED',    -- Auto-selected but user confirmed
            'NO_MATCH',          -- No suitable candidates found
            'AMBIGUOUS',         -- Multiple candidates, awaiting user input
            'REJECTED'           -- User rejected suggested match
        )
    )
);

CREATE INDEX idx_research_decisions_target ON kyc.research_decisions(target_entity_id);
CREATE INDEX idx_research_decisions_trigger ON kyc.research_decisions(trigger_id);
CREATE INDEX idx_research_decisions_type ON kyc.research_decisions(decision_type);

COMMENT ON TABLE kyc.research_decisions IS 
'Audit trail for Phase 1 (LLM exploration) decisions. Captures the non-deterministic 
search and selection process for later review and correction.';
```

### Research Actions Table

```sql
-- =============================================================================
-- RESEARCH ACTIONS (audit trail for Phase 2)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What entity this affects
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Link to decision that triggered this
    decision_id UUID REFERENCES kyc.research_decisions(decision_id),
    trigger_id UUID REFERENCES kyc.ownership_research_triggers(trigger_id),
    
    -- Action details
    action_type VARCHAR(50) NOT NULL,  -- IMPORT_HIERARCHY, IMPORT_PSC, etc.
    source_provider VARCHAR(30) NOT NULL,
    source_key VARCHAR(100) NOT NULL,  -- The identifier used
    source_key_type VARCHAR(20) NOT NULL,
    
    -- DSL verb executed
    verb_domain VARCHAR(30) NOT NULL,
    verb_name VARCHAR(50) NOT NULL,
    verb_args JSONB NOT NULL,
    
    -- Outcome
    success BOOLEAN NOT NULL,
    
    -- Changes made (if successful)
    entities_created INTEGER DEFAULT 0,
    entities_updated INTEGER DEFAULT 0,
    relationships_created INTEGER DEFAULT 0,
    fields_updated JSONB,  -- [{entity_id, field, old_value, new_value}]
    
    -- Errors (if failed)
    error_code VARCHAR(50),
    error_message TEXT,
    
    -- Performance
    duration_ms INTEGER,
    api_calls_made INTEGER,
    
    -- Audit
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    executed_by UUID,
    session_id UUID,
    
    -- Rollback support
    is_rolled_back BOOLEAN DEFAULT false,
    rolled_back_at TIMESTAMPTZ,
    rolled_back_by UUID,
    rollback_reason TEXT
);

CREATE INDEX idx_research_actions_target ON kyc.research_actions(target_entity_id);
CREATE INDEX idx_research_actions_decision ON kyc.research_actions(decision_id);
CREATE INDEX idx_research_actions_verb ON kyc.research_actions(verb_domain, verb_name);

COMMENT ON TABLE kyc.research_actions IS 
'Audit trail for Phase 2 (DSL execution). Every import/update via research verbs 
is logged here with full details for reproducibility and rollback.';
```

### Correction Tracking

```sql
-- =============================================================================
-- RESEARCH CORRECTIONS (when wrong key was selected)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_corrections (
    correction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What's being corrected
    original_decision_id UUID NOT NULL REFERENCES kyc.research_decisions(decision_id),
    original_action_id UUID REFERENCES kyc.research_actions(action_id),
    
    -- Correction details
    correction_type VARCHAR(20) NOT NULL,
    
    -- Wrong selection
    wrong_key VARCHAR(100),
    wrong_key_type VARCHAR(20),
    
    -- Correct selection
    correct_key VARCHAR(100),
    correct_key_type VARCHAR(20),
    
    -- New action (if re-imported)
    new_action_id UUID REFERENCES kyc.research_actions(action_id),
    
    -- Why
    correction_reason TEXT NOT NULL,
    
    -- Who/when
    corrected_at TIMESTAMPTZ DEFAULT NOW(),
    corrected_by UUID NOT NULL,
    
    CONSTRAINT chk_correction_type CHECK (
        correction_type IN (
            'WRONG_ENTITY',      -- Selected wrong entity entirely
            'WRONG_JURISDICTION',-- Right name, wrong jurisdiction
            'STALE_DATA',        -- Data was outdated
            'MERGE_REQUIRED',    -- Need to merge with existing
            'UNLINK'             -- Remove incorrect link
        )
    )
);

COMMENT ON TABLE kyc.research_corrections IS 
'Tracks corrections when Phase 1 selected the wrong identifier. 
Supports learning and audit trail for regulatory inquiries.';
```

---

## Component 3: Confidence Thresholds

### Configuration

```sql
-- =============================================================================
-- RESEARCH CONFIDENCE CONFIGURATION
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_confidence_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Scope (global or per-source)
    source_provider VARCHAR(30),  -- NULL = global default
    
    -- Thresholds
    auto_proceed_threshold DECIMAL(3,2) DEFAULT 0.90,
    ambiguous_threshold DECIMAL(3,2) DEFAULT 0.70,
    reject_threshold DECIMAL(3,2) DEFAULT 0.50,
    
    -- Behavior
    require_human_checkpoint BOOLEAN DEFAULT false,
    checkpoint_contexts TEXT[],  -- ['NEW_CLIENT', 'MATERIAL_HOLDING', 'HIGH_RISK']
    
    -- Limits
    max_auto_imports_per_session INTEGER DEFAULT 50,
    max_chain_depth INTEGER DEFAULT 10,
    
    -- Active
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,
    
    CONSTRAINT uq_confidence_source UNIQUE (source_provider, effective_from)
);

-- Seed defaults
INSERT INTO kyc.research_confidence_config (
    source_provider, auto_proceed_threshold, ambiguous_threshold, require_human_checkpoint
) VALUES 
(NULL, 0.90, 0.70, false),      -- Global default
('gleif', 0.92, 0.75, false),   -- GLEIF is authoritative, high bar
('companies_house', 0.88, 0.70, false),
('orbis', 0.85, 0.65, false),   -- Commercial, slightly lower
('screening', 0.00, 0.00, true) -- Always human checkpoint for screening
ON CONFLICT DO NOTHING;
```

### Threshold Logic

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CONFIDENCE-BASED ROUTING                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   SCORE >= auto_proceed_threshold (e.g., 0.90)                              │
│   ════════════════════════════════════════════                               │
│   → AUTO_SELECTED                                                           │
│   → Proceed directly to Phase 2 (import)                                    │
│   → Log decision with reasoning                                             │
│   → No user interaction needed                                              │
│                                                                              │
│   SCORE >= ambiguous_threshold (e.g., 0.70)                                 │
│   ════════════════════════════════════════════                               │
│   → AMBIGUOUS                                                               │
│   → Present candidates to user                                              │
│   → "Did you mean X or Y?"                                                  │
│   → Wait for user selection                                                 │
│   → Then proceed to Phase 2                                                 │
│                                                                              │
│   SCORE < ambiguous_threshold                                               │
│   ═══════════════════════════                                                │
│   → NO_MATCH                                                                │
│   → "Could not find confident match"                                        │
│   → Suggest alternative sources                                             │
│   → May require manual research                                             │
│                                                                              │
│   CHECKPOINT CONTEXTS (always ask user regardless of score)                 │
│   ════════════════════════════════════════════════════════                   │
│   • NEW_CLIENT - First-time entity for new client                           │
│   • MATERIAL_HOLDING - >25% ownership stake                                 │
│   • HIGH_RISK_JURISDICTION - Sanctions-sensitive jurisdictions              │
│   • SCREENING_HIT - Any sanctions/PEP matches                               │
│   • CORRECTION - Re-doing after previous correction                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Component 4: DSL Verbs (Phase 2 Only)

### Verb Design Principle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    VERB DESIGN RULES                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   1. ALL import verbs require an IDENTIFIER (key)                           │
│      ─────────────────────────────────────────────                           │
│      ✗ research.gleif.import(:name "AllianzGI")     ← NO fuzzy              │
│      ✓ research.gleif.import(:lei "529900XXXXXX")   ← YES exact key        │
│                                                                              │
│   2. Verbs are IDEMPOTENT                                                   │
│      ────────────────────────                                                │
│      Running same verb twice with same key = same result                    │
│      (may update existing rather than duplicate)                            │
│                                                                              │
│   3. Verbs CREATE AUDIT TRAIL                                               │
│      ────────────────────────────                                            │
│      Every verb execution logged to research_actions                        │
│      Links back to decision that provided the key                           │
│                                                                              │
│   4. Verbs VALIDATE post-import                                             │
│      ─────────────────────────────                                           │
│      After import, run sanity checks                                        │
│      Flag anomalies but don't block                                         │
│                                                                              │
│   5. Verbs support ROLLBACK                                                 │
│      ─────────────────────────                                               │
│      Can mark action as rolled back                                         │
│      Undo creates reverse relationships                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### research/gleif.yaml

```yaml
domains:
  research.gleif:
    description: "GLEIF - Global LEI Foundation (import verbs)"
    
    verbs:
      import-entity:
        description: "Import entity by LEI"
        behavior: plugin
        handler: GleifImportEntityOp
        args:
          - name: lei
            type: string
            required: true
            validation: "^[A-Z0-9]{20}$"
          - name: target-entity-id
            type: uuid
            description: "Link to existing entity (creates new if omitted)"
          - name: decision-id
            type: uuid
            description: "Link to research decision that found this LEI"
        returns:
          type: object
          fields:
            - entity_id: uuid
            - created: boolean
            - fields_updated: array

      import-hierarchy:
        description: "Import ownership hierarchy by LEI"
        behavior: plugin
        handler: GleifImportHierarchyOp
        args:
          - name: lei
            type: string
            required: true
            validation: "^[A-Z0-9]{20}$"
          - name: direction
            type: string
            default: "UP"
            valid_values: [UP, DOWN, BOTH]
          - name: max-depth
            type: integer
            default: 5
          - name: create-missing-entities
            type: boolean
            default: true
          - name: decision-id
            type: uuid
        returns:
          type: object
          fields:
            - entities_created: integer
            - entities_updated: integer
            - relationships_created: integer
            - chain_depth: integer
            - terminals_found: array

      validate-lei:
        description: "Validate LEI status against GLEIF"
        behavior: plugin
        handler: GleifValidateLeiOp
        args:
          - name: lei
            type: string
            required: true
          - name: entity-id
            type: uuid
            description: "Check against existing entity data"
        returns:
          type: object
          fields:
            - is_valid: boolean
            - status: string
            - discrepancies: array

      refresh-entity:
        description: "Refresh entity data from GLEIF"
        behavior: plugin
        handler: GleifRefreshEntityOp
        args:
          - name: entity-id
            type: uuid
            required: true
            description: "Must have LEI already"
        returns:
          type: object
          fields:
            - fields_updated: array
            - was_stale: boolean
```

### research/companies-house.yaml

```yaml
domains:
  research.companies-house:
    description: "UK Companies House (import verbs)"
    
    verbs:
      import-company:
        description: "Import company profile by company number"
        behavior: plugin
        handler: CompaniesHouseImportCompanyOp
        args:
          - name: company-number
            type: string
            required: true
            validation: "^[A-Z0-9]{8}$"
          - name: target-entity-id
            type: uuid
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-officers:
        description: "Import officers/directors"
        behavior: plugin
        handler: CompaniesHouseImportOfficersOp
        args:
          - name: company-number
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: include-resigned
            type: boolean
            default: false
          - name: decision-id
            type: uuid
        returns:
          type: object
          fields:
            - officers_created: integer
            - appointments_created: integer

      import-psc:
        description: "Import Persons with Significant Control"
        behavior: plugin
        handler: CompaniesHouseImportPscOp
        args:
          - name: company-number
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: decision-id
            type: uuid
        returns:
          type: object
          fields:
            - pscs_created: integer
            - ownership_edges_created: integer
            - control_edges_created: integer

      import-filing:
        description: "Import specific filing document"
        behavior: plugin
        handler: CompaniesHouseImportFilingOp
        args:
          - name: company-number
            type: string
            required: true
          - name: filing-id
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
```

### research/sec.yaml

```yaml
domains:
  research.sec:
    description: "US SEC EDGAR (import verbs)"
    
    verbs:
      import-company:
        description: "Import company by CIK"
        behavior: plugin
        handler: SecImportCompanyOp
        args:
          - name: cik
            type: string
            required: true
            validation: "^[0-9]{10}$"
          - name: target-entity-id
            type: uuid
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-13f-holders:
        description: "Import institutional holders from 13F filings"
        behavior: plugin
        handler: SecImport13FOp
        args:
          - name: cik
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: as-of-quarter
            type: string
            description: "YYYY-Q# format, defaults to latest"
          - name: threshold-pct
            type: decimal
            default: 0
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-13dg-owners:
        description: "Import beneficial owners from 13D/13G filings"
        behavior: plugin
        handler: SecImport13DGOp
        args:
          - name: cik
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-insiders:
        description: "Import insider holdings from Form 3/4/5"
        behavior: plugin
        handler: SecImportInsidersOp
        args:
          - name: cik
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
```

### research/orbis.yaml

```yaml
domains:
  research.orbis:
    description: "Bureau van Dijk Orbis (import verbs, requires subscription)"
    
    verbs:
      import-entity:
        description: "Import entity by BvD ID"
        behavior: plugin
        handler: OrbisImportEntityOp
        args:
          - name: bvd-id
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-hierarchy:
        description: "Import ownership hierarchy"
        behavior: plugin
        handler: OrbisImportHierarchyOp
        args:
          - name: bvd-id
            type: string
            required: true
          - name: direction
            type: string
            default: "UP"
            valid_values: [UP, DOWN, BOTH]
          - name: threshold-pct
            type: decimal
            default: 25
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-shareholders:
        description: "Import shareholders"
        behavior: plugin
        handler: OrbisImportShareholdersOp
        args:
          - name: bvd-id
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: threshold-pct
            type: decimal
            default: 0
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-ubo:
        description: "Import UBO data"
        behavior: plugin
        handler: OrbisImportUboOp
        args:
          - name: bvd-id
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: decision-id
            type: uuid
        returns:
          type: object
```

### research/screening.yaml

```yaml
domains:
  research.screening:
    description: "Screening results (record verbs)"
    
    verbs:
      record-sanctions-check:
        description: "Record sanctions screening result"
        behavior: plugin
        handler: ScreeningRecordSanctionsOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
            valid_values: [WORLD_CHECK, DOW_JONES, COMPLY_ADVANTAGE, ACCUITY, MANUAL]
          - name: lists-checked
            type: array
            required: true
          - name: result
            type: string
            required: true
            valid_values: [CLEAR, POTENTIAL_MATCH, CONFIRMED_MATCH]
          - name: matches
            type: array
            description: "Details of any matches found"
          - name: decision-id
            type: uuid
        returns:
          type: object

      record-pep-check:
        description: "Record PEP screening result"
        behavior: plugin
        handler: ScreeningRecordPepOp
        args:
          - name: person-entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
          - name: result
            type: string
            required: true
            valid_values: [NOT_PEP, PEP, RCA, FORMER_PEP]
          - name: pep-details
            type: object
            description: "Position, jurisdiction, dates if PEP"
          - name: decision-id
            type: uuid
        returns:
          type: object

      record-adverse-media:
        description: "Record adverse media screening result"
        behavior: plugin
        handler: ScreeningRecordAdverseMediaOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
          - name: result
            type: string
            required: true
            valid_values: [CLEAR, MENTIONS_FOUND, SIGNIFICANT_ADVERSE]
          - name: mentions
            type: array
          - name: decision-id
            type: uuid
        returns:
          type: object

      record-identity-verification:
        description: "Record identity verification result"
        behavior: plugin
        handler: ScreeningRecordIdentityOp
        args:
          - name: person-entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
            valid_values: [ONFIDO, JUMIO, TRULIOO, SUMSUB, MANUAL]
          - name: result
            type: string
            required: true
            valid_values: [VERIFIED, FAILED, PENDING_REVIEW, EXPIRED]
          - name: verification-details
            type: object
          - name: document-ids
            type: array
          - name: decision-id
            type: uuid
        returns:
          type: object
```

### research/outreach.yaml

```yaml
domains:
  research.outreach:
    description: "Counterparty outreach tracking"
    
    verbs:
      create-request:
        description: "Create outreach request"
        behavior: crud
        crud:
          operation: insert
          table: outreach_requests
          schema: kyc
          returning: request_id
        args:
          - name: target-entity-id
            type: uuid
            required: true
            maps_to: target_entity_id
          - name: request-type
            type: string
            required: true
            maps_to: request_type
            valid_values: [NOMINEE_DISCLOSURE, UBO_DECLARATION, SHARE_REGISTER, BOARD_COMPOSITION, GENERAL_INQUIRY]
          - name: recipient-entity-id
            type: uuid
            maps_to: recipient_entity_id
          - name: recipient-email
            type: string
            maps_to: recipient_email
          - name: deadline-days
            type: integer
            default: 30
          - name: trigger-id
            type: uuid
            maps_to: trigger_id
        returns:
          type: uuid
          capture: true

      mark-sent:
        description: "Mark request as sent"
        behavior: crud
        crud:
          operation: update
          table: outreach_requests
          schema: kyc
        args:
          - name: request-id
            type: uuid
            required: true
            maps_to: request_id

      record-response:
        description: "Record response to outreach"
        behavior: plugin
        handler: OutreachRecordResponseOp
        args:
          - name: request-id
            type: uuid
            required: true
          - name: response-type
            type: string
            required: true
            valid_values: [FULL_DISCLOSURE, PARTIAL_DISCLOSURE, DECLINED, NO_RESPONSE]
          - name: document-id
            type: uuid
          - name: notes
            type: string
        returns:
          type: object

      close-request:
        description: "Close outreach request"
        behavior: crud
        crud:
          operation: update
          table: outreach_requests
          schema: kyc
        args:
          - name: request-id
            type: uuid
            required: true
            maps_to: request_id
          - name: resolution-notes
            type: string
            maps_to: resolution_notes

      list-pending:
        description: "List pending outreach requests"
        behavior: crud
        crud:
          operation: select
          table: outreach_requests
          schema: kyc
        args:
          - name: target-entity-id
            type: uuid
            maps_to: target_entity_id
          - name: status
            type: string
            maps_to: status
            default: "PENDING"
```

### research/documents.yaml

```yaml
domains:
  research.documents:
    description: "Document import (after LLM extraction)"
    
    verbs:
      import-extracted-ownership:
        description: "Import ownership data extracted from document"
        behavior: plugin
        handler: DocumentsImportOwnershipOp
        args:
          - name: document-id
            type: uuid
            required: true
          - name: target-entity-id
            type: uuid
            required: true
          - name: extracted-data
            type: object
            required: true
            description: "Structured ownership data from LLM extraction"
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-extracted-directors:
        description: "Import directors extracted from document"
        behavior: plugin
        handler: DocumentsImportDirectorsOp
        args:
          - name: document-id
            type: uuid
            required: true
          - name: target-entity-id
            type: uuid
            required: true
          - name: extracted-data
            type: array
            required: true
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-ubo-declaration:
        description: "Import parsed UBO declaration"
        behavior: plugin
        handler: DocumentsImportUboDeclarationOp
        args:
          - name: document-id
            type: uuid
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: parsed-data
            type: object
            required: true
          - name: validate-against-existing
            type: boolean
            default: true
          - name: decision-id
            type: uuid
        returns:
          type: object

      record-reconciliation:
        description: "Record share register reconciliation result"
        behavior: plugin
        handler: DocumentsRecordReconciliationOp
        args:
          - name: document-id
            type: uuid
            required: true
          - name: issuer-entity-id
            type: uuid
            required: true
          - name: reconciliation-result
            type: object
            required: true
          - name: discrepancies
            type: array
        returns:
          type: object
```

### research/workflow.yaml

```yaml
domains:
  research.workflow:
    description: "Research workflow management"
    
    verbs:
      # Trigger management
      list-triggers:
        description: "List research triggers"
        behavior: crud
        crud:
          operation: select
          table: ownership_research_triggers
          schema: kyc
        args:
          - name: entity-id
            type: uuid
            maps_to: target_entity_id
          - name: status
            type: string
            maps_to: status
          - name: priority
            type: string
            maps_to: priority

      create-trigger:
        description: "Create research trigger"
        behavior: crud
        crud:
          operation: insert
          table: ownership_research_triggers
          schema: kyc
          returning: trigger_id
        args:
          - name: target-entity-id
            type: uuid
            required: true
            maps_to: target_entity_id
          - name: research-type
            type: string
            required: true
            maps_to: research_type
          - name: description
            type: string
            maps_to: description
          - name: priority
            type: string
            default: "MEDIUM"
            maps_to: priority

      resolve-trigger:
        description: "Resolve research trigger"
        behavior: plugin
        handler: WorkflowResolveTriggerOp
        args:
          - name: trigger-id
            type: uuid
            required: true
          - name: resolution
            type: string
            required: true
            valid_values: [RESOLVED, PARTIALLY_RESOLVED, UNRESOLVABLE, DEFERRED]
          - name: resolution-notes
            type: string
          - name: action-ids
            type: array
            description: "Research actions that resolved this"
        returns:
          type: object

      # Decision management
      record-decision:
        description: "Record a research decision (Phase 1 outcome)"
        behavior: crud
        crud:
          operation: insert
          table: research_decisions
          schema: kyc
          returning: decision_id
        args:
          - name: trigger-id
            type: uuid
            maps_to: trigger_id
          - name: target-entity-id
            type: uuid
            maps_to: target_entity_id
          - name: search-query
            type: string
            required: true
            maps_to: search_query
          - name: source-provider
            type: string
            required: true
            maps_to: source_provider
          - name: candidates-found
            type: array
            required: true
            maps_to: candidates_found
          - name: selected-key
            type: string
            maps_to: selected_key
          - name: selected-key-type
            type: string
            maps_to: selected_key_type
          - name: confidence
            type: decimal
            maps_to: selection_confidence
          - name: reasoning
            type: string
            required: true
            maps_to: selection_reasoning
          - name: decision-type
            type: string
            required: true
            maps_to: decision_type
        returns:
          type: uuid
          capture: true

      confirm-decision:
        description: "User confirms ambiguous decision"
        behavior: plugin
        handler: WorkflowConfirmDecisionOp
        args:
          - name: decision-id
            type: uuid
            required: true
          - name: selected-key
            type: string
            required: true
          - name: selected-key-type
            type: string
            required: true
        returns:
          type: object

      reject-decision:
        description: "User rejects suggested decision"
        behavior: plugin
        handler: WorkflowRejectDecisionOp
        args:
          - name: decision-id
            type: uuid
            required: true
          - name: rejection-reason
            type: string
            required: true
        returns:
          type: object

      # Corrections
      record-correction:
        description: "Record a correction to previous decision"
        behavior: crud
        crud:
          operation: insert
          table: research_corrections
          schema: kyc
          returning: correction_id
        args:
          - name: original-decision-id
            type: uuid
            required: true
            maps_to: original_decision_id
          - name: correction-type
            type: string
            required: true
            maps_to: correction_type
          - name: wrong-key
            type: string
            maps_to: wrong_key
          - name: correct-key
            type: string
            maps_to: correct_key
          - name: correction-reason
            type: string
            required: true
            maps_to: correction_reason
        returns:
          type: uuid
          capture: true

      # Reporting
      gap-report:
        description: "Generate gap analysis report"
        behavior: plugin
        handler: WorkflowGapReportOp
        args:
          - name: entity-id
            type: uuid
          - name: group-id
            type: uuid
          - name: cbu-id
            type: uuid
        returns:
          type: object

      audit-trail:
        description: "Get research audit trail for entity"
        behavior: plugin
        handler: WorkflowAuditTrailOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: include-decisions
            type: boolean
            default: true
          - name: include-actions
            type: boolean
            default: true
          - name: include-corrections
            type: boolean
            default: true
        returns:
          type: object
```

---

## Component 5: Agent Patterns

### Agent Loop Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    RESEARCH AGENT LOOP                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   TRIGGER: User request or ownership gap identified                         │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  LOOP:                                                               │   │
│   │                                                                      │   │
│   │  1. IDENTIFY GAP                                                    │   │
│   │     ───────────────                                                  │   │
│   │     ownership.identify-gaps(:entity-id @target)                     │   │
│   │     → Gap: HoldCo Ltd has no parent                                 │   │
│   │                                                                      │   │
│   │  2. LOAD ORCHESTRATION PROMPT                                       │   │
│   │     ─────────────────────────────                                    │   │
│   │     Load: /prompts/research/orchestration/resolve-gap.md            │   │
│   │     Insert: entity_name, jurisdiction, context                      │   │
│   │                                                                      │   │
│   │  3. LLM REASONS (prompt execution)                                  │   │
│   │     ─────────────────────────────────                                │   │
│   │     "UK company, try GLEIF then Companies House"                    │   │
│   │                                                                      │   │
│   │  4. LOAD SOURCE PROMPT                                              │   │
│   │     ──────────────────────                                           │   │
│   │     Load: /prompts/research/sources/gleif/search.md                 │   │
│   │     Execute: Call GLEIF API, evaluate results                       │   │
│   │                                                                      │   │
│   │  5. EVALUATE CONFIDENCE                                             │   │
│   │     ─────────────────────────                                        │   │
│   │     If score >= 0.90: AUTO_SELECTED → continue                      │   │
│   │     If score 0.70-0.90: AMBIGUOUS → present to user                 │   │
│   │     If score < 0.70: Try next source or NO_MATCH                    │   │
│   │                                                                      │   │
│   │  6. RECORD DECISION (Phase 1 audit)                                 │   │
│   │     ───────────────────────────────────                              │   │
│   │     research.workflow.record-decision(                              │   │
│   │         :search-query "HoldCo Ltd"                                  │   │
│   │         :source-provider "gleif"                                    │   │
│   │         :candidates-found [...]                                     │   │
│   │         :selected-key "213800ABC..."                                │   │
│   │         :confidence 0.92                                            │   │
│   │         :reasoning "Exact match, correct jurisdiction"              │   │
│   │         :decision-type "AUTO_SELECTED")                             │   │
│   │     → decision_id captured                                          │   │
│   │                                                                      │   │
│   │  7. EMIT IMPORT VERB (Phase 2)                                      │   │
│   │     ──────────────────────────────                                   │   │
│   │     research.gleif.import-hierarchy(                                │   │
│   │         :lei "213800ABC..."                                         │   │
│   │         :direction "UP"                                             │   │
│   │         :decision-id @decision_id)                                  │   │
│   │     → entities_created: 1, relationships_created: 1                 │   │
│   │                                                                      │   │
│   │  8. VALIDATE RESULT                                                 │   │
│   │     ─────────────────                                                │   │
│   │     Check: jurisdiction matches? Entity type sensible?              │   │
│   │     Flag anomalies if found                                         │   │
│   │                                                                      │   │
│   │  9. CHECK IF MORE GAPS                                              │   │
│   │     ─────────────────────                                            │   │
│   │     ownership.identify-gaps(:entity-id @target)                     │   │
│   │     If gaps remain → LOOP back to step 2                            │   │
│   │     If no gaps → EXIT                                               │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   EXIT CONDITIONS:                                                          │
│   • No more gaps (coverage sufficient)                                      │
│   • Max depth reached                                                       │
│   • All sources exhausted for a gap                                         │
│   • User intervention required (ambiguous, checkpoint)                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Checkpoint Logic

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    HUMAN CHECKPOINT TRIGGERS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ALWAYS CHECKPOINT:                                                        │
│   ══════════════════                                                         │
│   • Screening hits (sanctions, PEP, adverse media)                          │
│   • Confidence score in ambiguous range (0.70-0.90)                         │
│   • Multiple equally-scored candidates                                      │
│   • Correction to previous decision                                         │
│                                                                              │
│   CONTEXT-BASED CHECKPOINT:                                                 │
│   ═════════════════════════                                                  │
│   • NEW_CLIENT flag set                                                     │
│   • Material holding (>25% ownership)                                       │
│   • High-risk jurisdiction                                                  │
│   • Entity type mismatch (found company, expected person)                   │
│                                                                              │
│   CHECKPOINT UI PATTERN:                                                    │
│   ══════════════════════                                                     │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  RESEARCH CHECKPOINT                                                 │   │
│   │                                                                      │   │
│   │  Searching for: "HoldCo Ltd" (UK company)                           │   │
│   │                                                                      │   │
│   │  Found 2 candidates:                                                │   │
│   │                                                                      │   │
│   │  ○ HOLDCO LIMITED (12345678)                                        │   │
│   │    Score: 0.85 | UK | Active                                        │   │
│   │    Registered: 2015 | SIC: 64209                                    │   │
│   │                                                                      │   │
│   │  ○ HOLDCO LTD (87654321)                                            │   │
│   │    Score: 0.82 | UK | Active                                        │   │
│   │    Registered: 2019 | SIC: 70100                                    │   │
│   │                                                                      │   │
│   │  [Select first] [Select second] [Neither - manual research]         │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Component 6: Validation Rules

### Post-Import Validation

```yaml
# /config/research/validation-rules.yaml

validation_rules:
  
  # Jurisdiction consistency
  jurisdiction_match:
    description: "Imported entity jurisdiction should match expected"
    severity: WARNING
    check: |
      imported.jurisdiction == expected.jurisdiction 
      OR expected.jurisdiction IS NULL

  # Entity type sanity
  entity_type_sensible:
    description: "Entity type should make sense in context"
    severity: WARNING
    check: |
      IF context.expecting == 'NATURAL_PERSON'
      THEN imported.entity_type IN ('INDIVIDUAL', 'PERSON')
      
      IF context.expecting == 'CORPORATE'
      THEN imported.entity_type IN ('COMPANY', 'FUND', 'PARTNERSHIP', etc.)

  # Circular reference
  no_circular_ownership:
    description: "Import should not create circular ownership"
    severity: ERROR
    check: |
      NOT EXISTS cycle in ownership_graph starting from imported.entity_id

  # Duplicate entity
  no_duplicate_entity:
    description: "Should not create duplicate of existing entity"
    severity: WARNING
    check: |
      NOT EXISTS entity WHERE 
        (lei = imported.lei AND lei IS NOT NULL)
        OR (company_number = imported.company_number AND jurisdiction = imported.jurisdiction)
        OR (similarity(name, imported.name) > 0.95 AND jurisdiction = imported.jurisdiction)

  # LEI status
  lei_is_active:
    description: "LEI should be in ISSUED status"
    severity: WARNING
    check: |
      imported.lei_status IN ('ISSUED', 'PENDING_TRANSFER')
      OR imported.lei IS NULL

  # Relationship consistency
  ownership_totals:
    description: "Imported ownership should not exceed 100%"
    severity: WARNING
    check: |
      SUM(ownership_pct) for target_entity <= 105  # Allow small rounding

  # Temporal consistency
  dates_sensible:
    description: "Dates should be sensible"
    severity: WARNING
    check: |
      incorporated_date < TODAY
      AND (ceased_date IS NULL OR ceased_date > incorporated_date)
```

### Anomaly Flagging

```sql
-- =============================================================================
-- RESEARCH ANOMALIES
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_anomalies (
    anomaly_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- What action triggered this
    action_id UUID NOT NULL REFERENCES kyc.research_actions(action_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Anomaly details
    rule_code VARCHAR(50) NOT NULL,
    severity VARCHAR(10) NOT NULL,
    description TEXT NOT NULL,
    
    -- Context
    expected_value TEXT,
    actual_value TEXT,
    
    -- Resolution
    status VARCHAR(20) DEFAULT 'OPEN',
    resolution TEXT,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    
    -- Audit
    detected_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_anomaly_severity CHECK (severity IN ('ERROR', 'WARNING', 'INFO')),
    CONSTRAINT chk_anomaly_status CHECK (status IN ('OPEN', 'ACKNOWLEDGED', 'RESOLVED', 'FALSE_POSITIVE'))
);

CREATE INDEX idx_anomalies_action ON kyc.research_anomalies(action_id);
CREATE INDEX idx_anomalies_entity ON kyc.research_anomalies(entity_id);
CREATE INDEX idx_anomalies_status ON kyc.research_anomalies(status);
```

---

## Key Files

| Category | File | Purpose |
|----------|------|---------|
| **Prompts** | `/prompts/research/sources/gleif/*.md` | GLEIF search/disambiguate |
| | `/prompts/research/sources/companies-house/*.md` | CH search |
| | `/prompts/research/orchestration/*.md` | Strategy prompts |
| **Verbs** | `/rust/config/verbs/research/gleif.yaml` | GLEIF import verbs |
| | `/rust/config/verbs/research/companies-house.yaml` | CH import verbs |
| | `/rust/config/verbs/research/sec.yaml` | SEC import verbs |
| | `/rust/config/verbs/research/screening.yaml` | Screening record verbs |
| | `/rust/config/verbs/research/workflow.yaml` | Workflow verbs |
| **Handlers** | `/rust/src/research/gleif/handler.rs` | GLEIF verb handlers |
| | `/rust/src/research/companies_house/handler.rs` | CH handlers |
| | `/rust/src/research/workflow/handler.rs` | Workflow handlers |
| **Schema** | `/migrations/016_research_workflows.sql` | All research tables |
| **Config** | `/config/research/validation-rules.yaml` | Validation rules |
| | `/config/research/confidence-thresholds.yaml` | Threshold config |

---

## Implementation Phases

### Phase 1: Schema & Framework (12h)
- [ ] 1.1 Review CLAUDE.md and all annexes
- [ ] 1.2 Create research_decisions table
- [ ] 1.3 Create research_actions table
- [ ] 1.4 Create research_corrections table
- [ ] 1.5 Create research_anomalies table
- [ ] 1.6 Create research_confidence_config table
- [ ] 1.7 Create outreach_requests table
- [ ] 1.8 Seed default confidence thresholds

### Phase 2: Prompt Templates (10h)
- [ ] 2.1 Create prompt directory structure
- [ ] 2.2 Write GLEIF search prompt
- [ ] 2.3 Write GLEIF disambiguate prompt
- [ ] 2.4 Write Companies House search prompt
- [ ] 2.5 Write SEC search prompt
- [ ] 2.6 Write resolve-gap orchestration prompt
- [ ] 2.7 Write chain-research orchestration prompt
- [ ] 2.8 Write screening interpretation prompts

### Phase 3: GLEIF Refactor (8h)
- [ ] 3.1 Refactor existing GLEIF under research module
- [ ] 3.2 Add decision_id parameter to import verbs
- [ ] 3.3 Implement audit trail logging
- [ ] 3.4 Add validation post-import
- [ ] 3.5 Update verb YAML definitions

### Phase 4: Companies House Integration (10h)
- [ ] 4.1 Implement CH API client
- [ ] 4.2 Implement import-company verb
- [ ] 4.3 Implement import-officers verb
- [ ] 4.4 Implement import-psc verb
- [ ] 4.5 Add CH prompt templates
- [ ] 4.6 Test with real UK companies

### Phase 5: SEC EDGAR Integration (8h)
- [ ] 5.1 Implement EDGAR API client
- [ ] 5.2 Implement 13F parser
- [ ] 5.3 Implement 13D/G parser
- [ ] 5.4 Implement import verbs
- [ ] 5.5 Add SEC prompt templates

### Phase 6: Screening Framework (8h)
- [ ] 6.1 Define screening provider interface
- [ ] 6.2 Implement record-sanctions-check verb
- [ ] 6.3 Implement record-pep-check verb
- [ ] 6.4 Implement interpretation prompts
- [ ] 6.5 Always-checkpoint logic for hits

### Phase 7: Workflow Verbs (8h)
- [ ] 7.1 Implement record-decision verb
- [ ] 7.2 Implement confirm-decision verb
- [ ] 7.3 Implement reject-decision verb
- [ ] 7.4 Implement record-correction verb
- [ ] 7.5 Implement resolve-trigger verb
- [ ] 7.6 Implement audit-trail verb

### Phase 8: Validation Framework (6h)
- [ ] 8.1 Define validation rule structure
- [ ] 8.2 Implement post-import validation hook
- [ ] 8.3 Implement anomaly recording
- [ ] 8.4 Implement anomaly resolution workflow

### Phase 9: Agent Integration (10h)
- [ ] 9.1 Implement agent loop structure
- [ ] 9.2 Implement checkpoint UI pattern
- [ ] 9.3 Implement confidence routing
- [ ] 9.4 Implement multi-source fallback
- [ ] 9.5 Implement chain-research orchestration
- [ ] 9.6 Test end-to-end agent flow

### Phase 10: Testing & Documentation (8h)
- [ ] 10.1 Test GLEIF flow end-to-end
- [ ] 10.2 Test Companies House flow
- [ ] 10.3 Test ambiguous case handling
- [ ] 10.4 Test correction workflow
- [ ] 10.5 Test validation rules
- [ ] 10.6 Update CLAUDE.md with research patterns
- [ ] 10.7 Document prompt template conventions

---

## Estimated Effort

| Phase | Effort |
|-------|--------|
| 1. Schema & Framework | 12h |
| 2. Prompt Templates | 10h |
| 3. GLEIF Refactor | 8h |
| 4. Companies House | 10h |
| 5. SEC EDGAR | 8h |
| 6. Screening | 8h |
| 7. Workflow Verbs | 8h |
| 8. Validation | 6h |
| 9. Agent Integration | 10h |
| 10. Testing & Docs | 8h |
| **Total** | **~88h** |

---

## Verb Summary

| Domain | Verbs | Type |
|--------|-------|------|
| `research.gleif` | 4 | Import (Phase 2) |
| `research.companies-house` | 4 | Import (Phase 2) |
| `research.sec` | 4 | Import (Phase 2) |
| `research.orbis` | 4 | Import (Phase 2) |
| `research.screening` | 4 | Record (Phase 2) |
| `research.outreach` | 5 | Workflow |
| `research.documents` | 4 | Import (Phase 2) |
| `research.workflow` | 10 | Decision/audit |
| **Total** | **~39** | |

Note: Search/disambiguation is handled by **prompt templates**, not verbs.

---

## Success Criteria

1. **Phase separation clear** - Prompts for exploration, verbs for import
2. **All decisions audited** - research_decisions captures every selection
3. **All actions audited** - research_actions captures every import
4. **Confidence routing works** - Auto/ambiguous/no-match correctly routed
5. **Checkpoints enforced** - High-stakes decisions require human confirmation
6. **Corrections tracked** - Wrong selections can be corrected with audit trail
7. **Validation catches anomalies** - Post-import sanity checks work
8. **Agent loop functional** - End-to-end gap resolution works

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| LLM picks wrong entity | Confidence thresholds, human checkpoints, corrections |
| API rate limits | Caching, backoff, rate limit tracking |
| Source data conflicts | Reconciliation prompts, discrepancy flagging |
| Audit trail too verbose | Configurable detail levels |
| Agent loops forever | Max depth, max iterations, timeout |

---

Generated: 2026-01-10
