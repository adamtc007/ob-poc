# Architecture Proposal: Entity Data Lineage & Semantic Scope Resolution

## Document Metadata

| Field | Value |
|-------|-------|
| **Status** | DRAFT - For Peer Review |
| **Author** | Adam / Claude |
| **Created** | 2026-02-02 |
| **Domain** | ob-poc (Enterprise Onboarding Platform) |
| **Review Target** | ChatGPT / Architectural Peer Review |

---

## Executive Summary

This proposal addresses two **orthogonal concerns** that have become conflated in the current `client_group_entity` table design:

1. **Data Quality & Lineage** — Tracking the provenance of entity attributes (allegations vs. verified facts)
2. **Semantic Agent Assistance** — Resolving informal human language ("the Allianz Irish funds") to entity scope

The current implementation duplicates entity attributes in both staging and entity tables, creating data integrity issues and confusion about source of truth. This proposal separates these concerns into distinct architectural layers.

---

## Problem Statement

### Problem 1: The "Same Data, Two Places" Smell

The `client_group_entity` table has accumulated columns that duplicate data in `entity_limited_companies`:

```
client_group_entity              entity_limited_companies
─────────────────────            ────────────────────────
lei                      ←DUP→   lei
legal_name               ←DUP→   (via entities.name)
jurisdiction             ←DUP→   jurisdiction
is_fund                  ←DUP→   is_fund
related_via_lei          ←DUP→   manco_lei
relationship_category    ←DUP→   (should be entity_relationships)
```

**Consequence**: When an entity is "promoted" from staging to proper tables, we have two copies of the same facts with no clear authority.

### Problem 2: Allegations vs. Facts

Entity data comes from multiple sources with varying confidence:

| Source | Confidence | Example |
|--------|------------|---------|
| GLEIF | High (authoritative for LEI) | LEI, legal name, jurisdiction |
| Client allegation | Low (self-reported) | "We're 100% owned by HoldCo X" |
| LLM web scraper | Variable | "Found entity in Luxembourg" |
| Companies House | High (authoritative for UK) | Share register, officers |

**Current limitation**: The binary staging/promoted model forces us to treat data as either "unverified garbage" or "verified truth". Reality is a gradient with potential conflicts.

**Existing pattern**: The `shareholdings` + `shareholding_sources` tables already solve this for ownership data — client alleges 100% ownership, Companies House says 50/50, the conflict surfaces for review.

**Gap**: This pattern doesn't extend to other entity attributes (name, jurisdiction, fund status).

### Problem 3: Semantic Agent Scope Resolution

Users interact with the system using informal language:

```
User: "Load the Allianz Irish ETF funds"
```

This contains:
- **"Allianz"** — Not an entity! It's shorthand for a client group
- **"Irish ETF funds"** — Informal description of entity characteristics

The agent needs to:
1. Resolve "Allianz" to a session scope anchor
2. Find entities within that scope matching "Irish ETF funds"
3. Pass those entity_ids to the verb handler

**This is parallel to verb intent matching**:
- Verbs define WHAT operation ("show ownership chain" → `ubo.trace-chain`)
- Entity scope defines WHICH entities ("Allianz Irish funds" → `[entity_id, ...]`)

**Current confusion**: The `client_group_entity` table mixes semantic search helpers (role_tags) with entity facts (lei, jurisdiction), making it unclear what's authoritative.

---

## Current Architecture

### Entity Tables (Authoritative Facts)

```
entities
├── entity_id (PK)
├── name
├── entity_type_id
└── ... base fields

entity_limited_companies (1:1 extension)
├── company_id (PK)
├── entity_id (FK, UNIQUE)
├── lei (UNIQUE)
├── jurisdiction
├── is_fund
├── gleif_status, gleif_category
├── direct_parent_lei, ultimate_parent_lei
├── manco_lei, umbrella_lei
└── ... GLEIF fields

entity_relationships (graph edges)
├── from_entity_id
├── to_entity_id
├── relationship_type
└── ownership_pct
```

### Ownership Lineage (Existing Pattern)

```
shareholdings
├── subject_entity_id
├── owner_entity_id
├── ownership_pct
├── source                 ← 'client', 'companies_house', etc.
├── verification_status    ← 'alleged', 'verified', 'disputed'
└── ...

shareholding_sources (multi-source support)
├── shareholding_id
├── source
├── ownership_pct          ← May differ from other sources
├── verification_status
└── document_ref
```

### Client Group Tables (Current - Mixed Concerns)

```
client_group
├── id
├── canonical_name         ← "Allianz Group"
└── description

client_group_alias
├── group_id
├── alias                  ← "Allianz", "AGI", "the Germans"
├── alias_norm
└── embedding              ← For Candle semantic match

client_group_entity (PROBLEMATIC - MIXED CONCERNS)
├── group_id
├── entity_id
│
├── ─── SEMANTIC LAYER (OK) ───
├── role_tags[]            ← ['MANCO', 'LUX', 'FUND'] for search
├── membership_type        ← in_group, service_provider
├── added_by               ← gleif, manual, agent
│
├── ─── FACT LAYER (WRONG PLACE) ───
├── lei                    ← Should be entity_limited_companies
├── legal_name             ← Should be entities.name
├── jurisdiction           ← Should be entity_limited_companies
├── is_fund                ← Should be entity_limited_companies
├── relationship_category  ← Should be entity_relationships
└── related_via_lei        ← Should be entity_limited_companies.manco_lei
```

---

## Proposed Architecture

### Layer 1: Entity Facts + Source Lineage

**Principle**: All entity attributes live on entity tables, with optional source lineage for attributes that may be alleged/disputed.

#### 1.1 Entity Tables (Unchanged Structure)

```sql
-- Core entity (unchanged)
entities (
    entity_id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type_id UUID REFERENCES entity_types
);

-- Company extension (unchanged structure, facts are authoritative)
entity_limited_companies (
    company_id UUID PRIMARY KEY,
    entity_id UUID UNIQUE REFERENCES entities,
    lei VARCHAR(20) UNIQUE,
    jurisdiction VARCHAR(3),
    is_fund BOOLEAN,
    manco_lei VARCHAR(20),
    -- ... existing GLEIF fields
);
```

#### 1.2 NEW: Entity Attribute Sources (Lineage)

Extends the shareholding_sources pattern to ALL entity attributes that may have multiple sources or disputed values.

```sql
CREATE TABLE entity_attribute_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- What attribute
    attribute_name TEXT NOT NULL,      -- 'name', 'jurisdiction', 'lei', 'is_fund', 'manco_lei'
    attribute_value TEXT,              -- Serialized value
    
    -- Source provenance
    source TEXT NOT NULL,              -- 'gleif', 'client', 'companies_house', 'llm_scraper', 'manual'
    source_document_id UUID,           -- FK to documents if applicable
    source_record_id TEXT,             -- External ref (LEI that led to discovery)
    source_url TEXT,                   -- Web source if applicable
    
    -- Confidence & verification
    confidence DECIMAL(3,2),           -- 0.00 to 1.00
    verification_status TEXT NOT NULL DEFAULT 'alleged',
        -- 'alleged':      Source claims this, not independently verified
        -- 'corroborated': Multiple sources agree
        -- 'verified':     Authoritative source confirms (e.g., GLEIF for LEI)
        -- 'disputed':     Sources conflict, needs review
        -- 'superseded':   Replaced by newer information
    
    verified_at TIMESTAMPTZ,
    verified_by TEXT,                  -- User or system that verified
    
    -- Lifecycle
    is_current BOOLEAN DEFAULT true,
    superseded_by UUID REFERENCES entity_attribute_sources(id),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Prevent exact duplicates
    UNIQUE(entity_id, attribute_name, source, attribute_value)
);

-- Query: Current values for an entity
CREATE INDEX idx_eas_entity_current 
    ON entity_attribute_sources(entity_id, attribute_name) 
    WHERE is_current = true;

-- Query: Find disputed attributes needing review
CREATE INDEX idx_eas_disputed 
    ON entity_attribute_sources(entity_id) 
    WHERE verification_status = 'disputed';

-- Query: Find all allegations from a source
CREATE INDEX idx_eas_source 
    ON entity_attribute_sources(source, verification_status);
```

#### 1.3 Usage Pattern

```sql
-- GLEIF provides authoritative LEI data
INSERT INTO entity_attribute_sources 
    (entity_id, attribute_name, attribute_value, source, confidence, verification_status)
VALUES 
    ('abc-123', 'lei', '529900K9B0N5BT694847', 'gleif', 1.0, 'verified'),
    ('abc-123', 'name', 'Allianz SE', 'gleif', 0.98, 'corroborated'),
    ('abc-123', 'jurisdiction', 'DE', 'gleif', 0.99, 'corroborated');

-- Client alleges different ownership structure
INSERT INTO entity_attribute_sources 
    (entity_id, attribute_name, attribute_value, source, confidence, verification_status)
VALUES 
    ('abc-123', 'ultimate_parent', 'HoldCo XYZ', 'client', 0.60, 'alleged');

-- Later: Companies House contradicts → mark as disputed
UPDATE entity_attribute_sources 
SET verification_status = 'disputed'
WHERE entity_id = 'abc-123' 
  AND attribute_name = 'ultimate_parent';
```

#### 1.4 View: Best Known Values with Lineage

```sql
CREATE VIEW entity_attributes_with_lineage AS
SELECT 
    e.entity_id,
    e.name,
    elc.lei,
    elc.jurisdiction,
    elc.is_fund,
    -- Aggregate sources for each key attribute
    (SELECT jsonb_agg(jsonb_build_object(
        'source', eas.source,
        'value', eas.attribute_value,
        'confidence', eas.confidence,
        'status', eas.verification_status
    )) FROM entity_attribute_sources eas 
    WHERE eas.entity_id = e.entity_id 
      AND eas.attribute_name = 'name' 
      AND eas.is_current) AS name_sources,
    -- Similar for other attributes...
    EXISTS (
        SELECT 1 FROM entity_attribute_sources eas
        WHERE eas.entity_id = e.entity_id 
          AND eas.verification_status = 'disputed'
    ) AS has_disputes
FROM entities e
LEFT JOIN entity_limited_companies elc ON elc.entity_id = e.entity_id;
```

---

### Layer 2: Semantic Agent Assistance (Client Group)

**Principle**: Client group tables exist ONLY for semantic resolution — mapping informal human language to entity scope. They contain NO authoritative facts about entities.

#### 2.1 Client Group (Brand/Informal Grouping)

```sql
-- Unchanged
CREATE TABLE client_group (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,      -- "Allianz Group" (display)
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### 2.2 Client Group Aliases (Semantic Anchoring)

```sql
-- For resolving "Allianz", "AGI", "the Germans" → group_id
CREATE TABLE client_group_alias (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    
    alias TEXT NOT NULL,               -- "Allianz", "AGI", "Allianz GI"
    alias_norm TEXT NOT NULL,          -- Normalized for exact match
    
    -- Candle semantic matching
    embedding VECTOR(384),
    embedder_id TEXT,                  -- 'bge-small-en-v1.5'
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(group_id, alias_norm)
);

CREATE INDEX idx_cga_alias_norm ON client_group_alias(alias_norm);
CREATE INDEX idx_cga_embedding ON client_group_alias 
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

#### 2.3 Client Group Entity (THIN - Membership + Search Tags Only)

```sql
CREATE TABLE client_group_entity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- ═══════════════════════════════════════════════════════════════════
    -- SEMANTIC SEARCH HELPERS (for agent phrase matching)
    -- These are NOT authoritative facts — they're search optimization
    -- ═══════════════════════════════════════════════════════════════════
    
    role_tags TEXT[] NOT NULL DEFAULT '{}',
    -- Informal tags for phrase matching:
    -- Structural: ULTIMATE_PARENT, HOLDING_CO, SUBSIDIARY, SPV
    -- Fund: MANCO, SICAV, UCITS, AIF, FUND, SUBFUND, UMBRELLA, ETF
    -- Geographic: LUX, IRISH, UK, US, DE
    -- Service: IM, CUSTODIAN, TA, AUDITOR
    -- 
    -- NOTE: These may overlap with entity facts but exist for SEARCH,
    -- not as source of truth. The entity tables are authoritative.
    
    -- ═══════════════════════════════════════════════════════════════════
    -- MEMBERSHIP CONTEXT
    -- ═══════════════════════════════════════════════════════════════════
    
    membership_type TEXT NOT NULL DEFAULT 'in_group',
    -- in_group:          Part of the corporate structure
    -- external_partner:  External party (e.g., third-party IM)
    -- service_provider:  Custodian, auditor, etc.
    -- counterparty:      Transaction counterparty
    -- historical:        No longer active in group
    
    -- ═══════════════════════════════════════════════════════════════════
    -- DISCOVERY PROVENANCE (how we found this membership, NOT entity facts)
    -- ═══════════════════════════════════════════════════════════════════
    
    added_by TEXT NOT NULL DEFAULT 'manual',
    -- manual:       Human added directly
    -- gleif:        Discovered via GLEIF ownership trace
    -- bods:         Discovered via BODS data
    -- agent:        LLM agent discovered during research
    -- import:       Bulk import
    
    -- Review workflow for membership (not entity facts)
    review_status TEXT NOT NULL DEFAULT 'pending',
    -- pending:   Membership not yet confirmed
    -- confirmed: Human confirmed entity belongs to this group
    -- rejected:  Human rejected membership
    
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(group_id, entity_id)
);

-- Fast role tag filtering (GIN for array containment)
CREATE INDEX idx_cge_role_tags 
    ON client_group_entity USING GIN (role_tags);

-- Membership type filtering
CREATE INDEX idx_cge_membership 
    ON client_group_entity(group_id, membership_type)
    WHERE membership_type != 'historical';

-- Pending reviews
CREATE INDEX idx_cge_review 
    ON client_group_entity(group_id, review_status)
    WHERE review_status = 'pending';
```

#### 2.4 Client Group Entity Tags (Shorthand Phrases)

```sql
-- For resolving "the main Lux ManCo", "Irish funds" → entity_ids
CREATE TABLE client_group_entity_tag (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    tag TEXT NOT NULL,                 -- "main lux manco", "the Irish one"
    tag_norm TEXT NOT NULL,            -- Normalized
    
    -- Persona scoping (same tag can mean different things in different contexts)
    persona TEXT,                      -- NULL=universal, 'kyc', 'trading', 'ops'
    
    -- Candle semantic matching
    embedding VECTOR(384),
    embedder_id TEXT,
    
    -- Confidence (for disambiguation)
    confidence DECIMAL(3,2) DEFAULT 1.0,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(group_id, entity_id, tag_norm, persona)
);

CREATE INDEX idx_cget_tag_norm ON client_group_entity_tag(group_id, tag_norm);
CREATE INDEX idx_cget_embedding ON client_group_entity_tag 
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

---

### Layer 3: Agent Integration

#### 3.1 Resolution Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  USER INPUT                                                                 │
│  "Show the ownership chain for the Allianz Irish ETF funds"                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 1: VERB INTENT MATCHING (existing Candle pipeline)                   │
│                                                                             │
│  "Show the ownership chain" → ubo.trace-chain                               │
│                                                                             │
│  Output: verb = "ubo.trace-chain"                                           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 2: CLIENT GROUP RESOLUTION (NEW - parallel to verb matching)         │
│                                                                             │
│  "Allianz" → Candle search on client_group_alias.embedding                  │
│           → Exact match on client_group_alias.alias_norm                    │
│           → group_id                                                        │
│                                                                             │
│  Output: session.anchor_group_id = group_id                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 3: ENTITY SCOPE RESOLUTION (within group)                            │
│                                                                             │
│  "Irish ETF funds" → Search within client_group_entity WHERE group_id = X   │
│                                                                             │
│  Strategy (in order):                                                       │
│  1. Exact tag match: client_group_entity_tag.tag_norm                       │
│  2. Role tag filter: role_tags @> ARRAY['IRISH', 'ETF', 'FUND']            │
│  3. Semantic search: client_group_entity_tag.embedding                      │
│                                                                             │
│  Output: entity_ids = [uuid1, uuid2, uuid3, ...]                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 4: ENTITY FACTS RETRIEVAL (from authoritative tables)                │
│                                                                             │
│  SELECT e.*, elc.lei, elc.jurisdiction, elc.is_fund                        │
│  FROM entities e                                                            │
│  JOIN entity_limited_companies elc ON elc.entity_id = e.entity_id          │
│  WHERE e.entity_id = ANY(:entity_ids)                                       │
│                                                                             │
│  NOTE: Facts come from entity tables, NOT client_group_entity               │
│                                                                             │
│  Output: Full entity records with authoritative data                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 5: VERB EXECUTION                                                    │
│                                                                             │
│  Execute ubo.trace-chain for each entity in scope                           │
│  (or batch if verb supports it)                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2 Rust Integration Points

```rust
// In semantic intent pipeline (existing)
pub struct IntentResolution {
    pub verb: ResolvedVerb,
    pub entity_scope: Option<EntityScope>,  // NEW
}

// NEW: Entity scope from client group resolution
pub struct EntityScope {
    pub group_id: Uuid,
    pub group_name: String,
    pub entity_ids: Vec<Uuid>,
    pub resolution_method: ScopeResolutionMethod,
}

pub enum ScopeResolutionMethod {
    ExactAliasMatch,
    SemanticAliasMatch { confidence: f32 },
    RoleTagFilter { tags: Vec<String> },
    SemanticTagMatch { confidence: f32 },
    Ambiguous { candidates: Vec<ScopeCandidate> },
}

// Resolution function
pub async fn resolve_entity_scope(
    input: &str,
    session_group_id: Option<Uuid>,  // If already anchored
    pool: &PgPool,
    embedder: &Embedder,
) -> Result<EntityScope, ScopeResolutionError> {
    // 1. If no group anchor, resolve group from input
    // 2. Extract entity descriptors from input
    // 3. Search within group using tags + embeddings
    // 4. Return entity_ids (or Ambiguous for user clarification)
}
```

#### 3.3 Session Context Enhancement

```rust
pub struct SessionContext {
    // Existing
    pub session_id: Uuid,
    pub active_domain: ActiveDomain,
    pub active_cbu_id: Option<Uuid>,
    
    // NEW: Client group anchoring
    pub anchor_group_id: Option<Uuid>,
    pub anchor_group_name: Option<String>,
    
    // NEW: Current entity scope (may be subset of group)
    pub scoped_entity_ids: Option<Vec<Uuid>>,
    pub scope_description: Option<String>,  // "Irish ETF funds"
}
```

#### 3.4 DSL Arg Enrichment

```yaml
# In verb definitions, entity args can specify client_group resolution
args:
  - name: entities
    type: entity_scope              # NEW type
    required: true
    description: "Entities to process"
    resolution:
      strategy: client_group        # Use client group semantic resolution
      fallback: direct_lookup       # Fall back to direct entity lookup
      allow_ambiguous: false        # Require clarification if ambiguous
```

---

## Migration Plan

### Phase 1: Add Entity Attribute Sources

**Migration: `060_entity_attribute_sources.sql`**

- Create `entity_attribute_sources` table
- Backfill from existing GLEIF data with source='gleif', status='verified'
- Create view for entities with lineage summary

**No breaking changes** — additive only.

### Phase 2: Slim Client Group Entity

**Migration: `061_slim_client_group_entity.sql`**

```sql
-- Remove fact columns that belong on entity tables
ALTER TABLE client_group_entity 
    DROP COLUMN IF EXISTS lei,
    DROP COLUMN IF EXISTS legal_name,
    DROP COLUMN IF EXISTS jurisdiction,
    DROP COLUMN IF EXISTS is_fund,
    DROP COLUMN IF EXISTS relationship_category,
    DROP COLUMN IF EXISTS related_via_lei,
    DROP COLUMN IF EXISTS fund_structure_type,
    DROP COLUMN IF EXISTS fund_type;

-- Ensure role_tags exists for semantic search
ALTER TABLE client_group_entity 
    ADD COLUMN IF NOT EXISTS role_tags TEXT[] NOT NULL DEFAULT '{}';

-- Add GIN index if not exists
CREATE INDEX IF NOT EXISTS idx_cge_role_tags 
    ON client_group_entity USING GIN (role_tags);
```

**Breaking change**: Verbs/handlers that read these columns need updating.

### Phase 3: Backfill Role Tags

**Migration: `062_backfill_role_tags.sql`**

```sql
-- Populate role_tags from entity facts for semantic search
UPDATE client_group_entity cge
SET role_tags = ARRAY(
    SELECT DISTINCT tag FROM (
        SELECT CASE WHEN elc.is_fund THEN 'FUND' END AS tag
        FROM entity_limited_companies elc WHERE elc.entity_id = cge.entity_id
        UNION ALL
        SELECT UPPER(elc.jurisdiction) AS tag
        FROM entity_limited_companies elc WHERE elc.entity_id = cge.entity_id
        UNION ALL
        SELECT CASE WHEN elc.manco_lei IS NOT NULL THEN 'HAS_MANCO' END AS tag
        FROM entity_limited_companies elc WHERE elc.entity_id = cge.entity_id
        -- ... more derivations
    ) derived
    WHERE tag IS NOT NULL
);
```

### Phase 4: Update Verb Handlers

- Update handlers that read from `client_group_entity` to join entity tables for facts
- Update handlers to use `role_tags` for filtering, not fact columns

### Phase 5: Agent Integration

- Implement `resolve_entity_scope()` function
- Add `EntityScope` to intent resolution pipeline
- Update session context with group anchoring
- Add disambiguation flow for ambiguous scope

---

## Open Questions for Review

1. **Role Tags Derivation**: Should role_tags be manually curated or auto-derived from entity facts? Auto-derivation ensures consistency but loses nuance ("main lux manco" vs just "MANCO").

2. **Dispute Resolution Workflow**: When entity_attribute_sources shows a dispute, what's the UX? Flag in agent response? Separate review queue?

3. **Scope Caching**: Should resolved entity scope be cached in session, or re-resolved each turn? Caching is faster but may miss updates.

4. **Cross-Group Entities**: An entity could belong to multiple client groups (e.g., a custodian bank serves many clients). How should this affect scope resolution?

5. **Lineage Granularity**: Should we track lineage for ALL attributes, or only "sensitive" ones (ownership, jurisdiction)? Full lineage is comprehensive but verbose.

---

## Appendix A: Table Summary

| Table | Layer | Purpose | Contains Facts? |
|-------|-------|---------|-----------------|
| `entities` | Entity | Core entity record | YES (authoritative) |
| `entity_limited_companies` | Entity | Company attributes | YES (authoritative) |
| `entity_attribute_sources` | Entity | Source lineage | Tracks sources of facts |
| `shareholdings` | Entity | Ownership facts | YES (with multi-source) |
| `shareholding_sources` | Entity | Ownership lineage | Tracks sources |
| `entity_relationships` | Entity | Graph edges | YES (authoritative) |
| `client_group` | Semantic | Brand/grouping | NO (metadata only) |
| `client_group_alias` | Semantic | Name resolution | NO (search index) |
| `client_group_entity` | Semantic | Membership + search | NO (search tags only) |
| `client_group_entity_tag` | Semantic | Phrase resolution | NO (search index) |

---

## Appendix B: Comparison with Existing Patterns

### Shareholdings (Current Best Practice)

```
shareholdings                    shareholding_sources
─────────────                    ────────────────────
subject_entity_id                shareholding_id (FK)
owner_entity_id                  source
ownership_pct ◄── Best known     ownership_pct ◄── Per source
source                           verification_status
verification_status              document_ref
```

### Proposed Entity Attributes (Extends Pattern)

```
entity_limited_companies         entity_attribute_sources
────────────────────────         ────────────────────────
entity_id                        entity_id (FK)
lei ◄── Best known               attribute_name
jurisdiction ◄── Best known      attribute_value ◄── Per source
is_fund ◄── Best known           source
                                 confidence
                                 verification_status
```

The pattern is identical — main table holds "best known" value, sources table tracks provenance and enables dispute detection.

---

## Appendix C: Agent Conversation Example

```
User: "Work on Allianz"
Agent: [Resolves "Allianz" → client_group_id, sets session.anchor_group_id]
       "Session anchored to Allianz Group (47 entities). What would you like to do?"

User: "Show me the Irish funds"
Agent: [Searches client_group_entity WHERE group_id=X AND role_tags @> '{IRISH,FUND}']
       [Finds 12 entities]
       "Found 12 Irish funds in the Allianz group:
        - Allianz Ireland UCITS Platform (LEI: 549300...)
        - Allianz Dublin ETF Fund (LEI: 549300...)
        ..."

User: "Trace UBO for the main Dublin one"
Agent: [Searches client_group_entity_tag for "main Dublin" within group]
       [Resolves to single entity_id]
       [Executes ubo.trace-chain for that entity]
       [Retrieves entity facts from entity_limited_companies]
       "UBO chain for Allianz Dublin ETF Fund:
        └── Allianz Dublin ETF Fund (IE)
            └── 100% Allianz Ireland Holdings (IE)
                └── 100% Allianz Asset Management (DE)
                    └── 100% Allianz SE (DE) ← Ultimate Parent"

User: "Companies House says that middle one is only 50% owned"
Agent: [Records allegation in entity_attribute_sources]
       [Marks existing ownership as 'disputed']
       "I've noted the discrepancy. The ownership of Allianz Ireland Holdings 
        is now marked as disputed:
        - GLEIF says: 100% owned by Allianz Asset Management
        - Companies House says: 50%
        Would you like me to create a review task for this?"
```

---

*End of Architecture Proposal*
