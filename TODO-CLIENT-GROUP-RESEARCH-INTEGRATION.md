# TODO: Client Group as Research Discovery Buffer

> **Status:** Ready for Implementation  
> **Priority:** High  
> **Dependencies:** 048 (Client Group Resolver - ✅ Complete), Intent Pipeline Fixes  
> **Reviewed:** 2026-01-26 (ChatGPT peer review complete, refinements integrated)

---

## Implementation Checklist (for Claude Code)

### Phase 1: Schema
- [x] Create `client_group_entity_roles` junction table
- [x] Create `client_group_relationship` table (ownership edges)
- [x] Create `client_group_relationship_sources` table (multi-source lineage)
- [x] Extend `client_group` with discovery status columns
- [x] Seed missing roles in `roles` table (SICAV, UCITS, AIF, ULTIMATE_PARENT, etc.)
- [x] Create `v_client_group_entity_search` view
- [x] Create `v_cgr_canonical`, `v_cgr_discrepancies` views

### Phase 2: Verbs (YAML + Handlers)
- [x] `client-group.assign-role` / `remove-role` / `list-roles` / `parties`
- [x] `client-group.add-entity` / `remove-entity` / `list-entities` (existing: entity-add, entity-remove, entity-list)
- [x] `client-group.add-relationship` / `list-relationships`
- [x] `client-group.add-ownership-source` / `verify-ownership` / `set-canonical`
- [x] `client-group.list-unverified` / `list-discrepancies`
- [x] `client-group.start-discovery` / `complete-discovery`

### Phase 3: Intent Pipeline Integration
- [x] Slot type → role_id mapping in resolver
- [x] Scoped search via `v_client_group_entity_search`
- [x] Role-filtered search SQL
- [x] **Candle output format**: slot types + preferred_roles via YAML config
- [x] "Allianz rule": Context-dependent resolution based on expected slot type
- [x] Role inference from mention text (fallback when no preferred_roles)
- [ ] Picker gating for ambiguous matches (uses existing intent pipeline picker)

### Phase 4: GLEIF/BODS Integration
- [ ] Import to `client_group_entity` + `client_group_entity_roles`
- [ ] Create `client_group_relationship` edges from GLEIF relationships
- [ ] Store sources in `client_group_relationship_sources`

---

## Problem Statement

Current GLEIF/BODS research flows go straight to entity creation and attempt immediate UBO/shareholding discovery. This is brittle because:

1. **No staging area** — Entities created directly, hard to clean up on research failure
2. **No tagging** — Can't classify entities during discovery (MANCO, SICAV, SPV, etc.)
3. **Premature structure** — Tries to build CBUs and shareholdings before understanding the group
4. **Agent AI disconnected** — Client group resolver (048) built for intent but not wired to research
5. **No source provenance** — Can't reconcile conflicting ownership data from GLEIF vs BODS vs Companies House

---

## Solution: Client Group as Discovery Buffer

Use `client_group` + `client_group_alias` + `client_group_entity` junction as a **loose collection buffer** during research discovery:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RESEARCH DISCOVERY FLOW                                                     │
│                                                                              │
│  1. DISCOVER: User says "research Aviva"                                    │
│     └─► Create/find client_group "Aviva"                                    │
│     └─► Set group.discovery_status = 'in_progress'                          │
│     └─► Agent resolves scope context for subsequent commands                │
│                                                                              │
│  2. CRAWL: GLEIF/BODS import                                                │
│     └─► Create entities with LEI, name, jurisdiction                        │
│     └─► Link to client_group via client_group_entity junction               │
│     └─► Tag each entity: MANCO, SICAV, SPV, FUND, HOLDING_CO, etc.         │
│     └─► Store raw GLEIF/BODS payload in shareholding_sources for audit     │
│                                                                              │
│  3. CLASSIFY: Analyze the loose collection                                  │
│     └─► Identify apex/ultimate parent                                       │
│     └─► Identify management companies (MANCOs)                              │
│     └─► Identify fund structures (SICAV, UCITS, AIF)                        │
│     └─► Build provisional ownership edges (provisional_parent_entity_id)    │
│                                                                              │
│  4. RECONCILE: Multi-source ownership (NEW)                                 │
│     └─► Compare GLEIF vs BODS vs Companies House values                     │
│     └─► Flag discrepancies above threshold (e.g., 5%)                       │
│     └─► Human review for disputed values                                    │
│                                                                              │
│  5. STRUCTURE: Build formal taxonomies                                      │
│     └─► Create CBUs for onboardable entities                                │
│     └─► Promote provisional ownership to shareholding table                 │
│     └─► Discover UBOs via shareholding chain walk                           │
│     └─► Set group.discovery_status = 'complete'                             │
│                                                                              │
│  6. MAINTAIN: Ongoing refresh                                               │
│     └─► Re-crawl GLEIF periodically (user-initiated)                        │
│     └─► Diff against existing collection                                    │
│     └─► Flag new/changed entities with review_status = 'needs_update'       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Integration with Intent Pipeline

**This design connects directly to the Intent Pipeline Fixes (TODO-INTENT-PIPELINE-FIXES.md):**

1. **Scope Resolution** — "research Aviva" creates/finds client_group, sets session scope
2. **Typed Search** — Within scope, "tag Aviva Investors as MANCO" resolves against `client_group_entity` not global entities
3. **Staged Runbook** — Research commands stage in runbook, execute on confirm
4. **Discovery Status** — egui shows discovery_status chip: `in_progress` / `complete` / `stale`

```
User: "research Aviva"
       ↓
   Stage 0: Creates/finds client_group "Aviva", sets scope
       ↓
   Runbook stages: (client-group.start-discovery :group-id <Aviva>)
       ↓
   User: "run"
       ↓
   Discovery status: in_progress
       ↓
   Subsequent commands scoped to Aviva group
```

---

## Conceptual Model: Client Group as Role-Tagged Working Set

### What `client_group` IS

| Purpose | Description |
|---------|-------------|
| **Scoped working set** | A collection of entities relevant to a client onboarding context |
| **Role-tagged members** | Each member has role tags (ManCo, SICAV, SPV, IM, Custodian...) |
| **Includes externals** | Membership can include in-group AND external entities (e.g., BlackRock as IM) |
| **Agent intent bridge** | Optimized for selection + scoping, not truth of ownership |
| **Onboarding buffer** | Dump entities in fast → refine ownership/control/UBO later |

### What `client_group` is NOT

| Not this | That comes from |
|----------|-----------------|
| Canonical corporate group hierarchy | Formal shareholdings table |
| Final UBO/control graph | UBO discovery + verification |
| Share-class/economic registry | Product/fund registry |

### Why This Works for the Agent

Role tags give the **typed context** the intent pipeline needs:

| User says | Resolver action |
|-----------|-----------------|
| "apply custody to the SPV" | Search `client_group_entity` where `role_tags @> ['SPV']` |
| "get the IM docs" | Search `role_tags @> ['IM']`, even if IM is external (BlackRock) |
| "Allianz" at session start | Resolves to client anchor (scope-set) |
| "Allianz" inside verb slot | Resolves to role-scoped targets within current scope |

### Orthogonal Flags: Membership vs Role

Keep these **separate** on each member:

| Flag | Values | Purpose |
|------|--------|---------|
| `membership_type` | `in_group`, `external_partner`, `counterparty`, `service_provider` | Is this entity part of the corporate group or external? |
| `role_tags[]` | Multi-valued array | What roles does this entity play in this context? |

**Example: BlackRock as Investment Manager for Allianz funds**

```
client_group_entity:
  group_id: <Allianz>
  entity_id: <BlackRock>
  membership_type: 'external_partner'
  role_tags: ['IM', 'REGULATED']
```

This prevents external entities being treated as corporate group members while still being **resolvable in scope**.

---

## Role Taxonomy: Reuse Existing `roles` Table

### Existing Infrastructure

You already have a comprehensive `roles` table used by `cbu_entity_roles`:

```sql
-- EXISTING: ob-poc.roles
CREATE TABLE "ob-poc".roles (
    role_id uuid PRIMARY KEY,
    name VARCHAR(255) NOT NULL,           -- e.g., 'INVESTMENT_MANAGER', 'CUSTODIAN', 'UBO'
    description TEXT,
    role_category VARCHAR(30),            -- structural, service, compliance, ownership
    layout_category VARCHAR(30),          -- for visualization grouping
    ubo_treatment VARCHAR(30),            -- how this role affects UBO discovery
    requires_percentage BOOLEAN,          -- does this role need ownership %?
    natural_person_only BOOLEAN,
    legal_entity_only BOOLEAN,
    compatible_entity_categories JSONB,   -- which entity types can have this role
    kyc_obligation VARCHAR(30),           -- FULL_KYC, SIMPLIFIED, NONE
    display_priority INTEGER,
    sort_order INTEGER,
    is_active BOOLEAN
);

-- EXISTING: ob-poc.cbu_entity_roles (junction)
CREATE TABLE "ob-poc".cbu_entity_roles (
    cbu_entity_role_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL,
    entity_id uuid NOT NULL,
    role_id uuid NOT NULL REFERENCES roles(role_id),
    ownership_percentage NUMERIC(5,2),
    effective_from DATE,
    effective_to DATE,
    target_entity_id uuid,               -- for directed roles (e.g., IM for specific fund)
    authority_limit NUMERIC(18,2),
    requires_co_signatory BOOLEAN
);
```

### New: `client_group_entity_roles` (Same Pattern)

Use the **same `roles` table** for client group membership roles:

```sql
CREATE TABLE "ob-poc".client_group_entity_roles (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cge_id uuid NOT NULL REFERENCES "ob-poc".client_group_entity(id) ON DELETE CASCADE,
    role_id uuid NOT NULL REFERENCES "ob-poc".roles(role_id),
    
    -- Context: what is this role relative to?
    -- e.g., BlackRock is IM *for* a specific fund within the group
    target_entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    
    -- Effective period
    effective_from DATE,
    effective_to DATE,  -- NULL = current
    
    -- Discovery metadata
    assigned_by TEXT NOT NULL DEFAULT 'manual',
    -- manual, gleif, bods, auto_tag, agent
    source_record_id VARCHAR(255),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Same entity can have same role only once per target (or NULL target)
    UNIQUE (cge_id, role_id, COALESCE(target_entity_id, '00000000-0000-0000-0000-000000000000'))
);

-- Find all roles for an entity in a group
CREATE INDEX idx_cger_cge ON "ob-poc".client_group_entity_roles (cge_id);

-- Find all entities with a specific role in a group (THE HOT PATH for intent resolution)
CREATE INDEX idx_cger_role ON "ob-poc".client_group_entity_roles (role_id);

-- Find roles by target entity
CREATE INDEX idx_cger_target ON "ob-poc".client_group_entity_roles (target_entity_id)
    WHERE target_entity_id IS NOT NULL;
```

### Migration: Update `client_group_entity`

Remove the `role_tags TEXT[]` array, use junction instead:

```sql
-- Remove from client_group_entity:
-- role_tags TEXT[] NOT NULL DEFAULT '{}'

-- The junction table replaces it with proper FK relationships
```

### View: Denormalized for Search (Agent Intent Resolution)

```sql
CREATE OR REPLACE VIEW "ob-poc".v_client_group_entity_search AS
SELECT 
    cge.id as cge_id,
    cge.group_id,
    cge.entity_id,
    cge.membership_type,
    cge.review_status,
    
    -- Entity display fields
    e.name as entity_name,
    e.lei,
    e.jurisdiction,
    e.entity_type,
    
    -- Roles as array (for filtering/display)
    COALESCE(
        (SELECT array_agg(DISTINCT r.name ORDER BY r.name)
         FROM "ob-poc".client_group_entity_roles cer
         JOIN "ob-poc".roles r ON r.role_id = cer.role_id
         WHERE cer.cge_id = cge.id
           AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)),
        '{}'::VARCHAR[]
    ) as role_names,
    
    -- Role IDs for joins
    COALESCE(
        (SELECT array_agg(DISTINCT cer.role_id)
         FROM "ob-poc".client_group_entity_roles cer
         WHERE cer.cge_id = cge.id
           AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)),
        '{}'::UUID[]
    ) as role_ids,
    
    -- Group context
    cg.canonical_name as group_name,
    
    -- Is external?
    (cge.membership_type IN ('external_partner', 'counterparty', 'service_provider')) as is_external
    
FROM "ob-poc".client_group_entity cge
JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
JOIN "ob-poc".client_group cg ON cg.id = cge.group_id
WHERE cge.membership_type != 'historical';
```

### Resolver Rule: Same as CBU

Use the **same lookup pattern** that CBU verbs use:

```yaml
# In verb definitions
- name: entity-id
  type: uuid
  required: true
  lookup:
    table: v_client_group_entity_search
    entity_type: entity
    schema: ob-poc
    search_key: entity_name
    primary_key: entity_id
    scope_key: group_id           # Search within current scope
    role_filter: role_names       # Optional: filter by role
    resolution_mode: entity       # growing table - search modal

# Role lookup (same as CBU)
- name: role-id
  type: uuid
  required: true
  lookup:
    table: roles
    entity_type: role
    schema: ob-poc
    search_key: name
    primary_key: role_id
    resolution_mode: reference    # small static table - autocomplete
```

### Resolver SQL: Role-Filtered Scoped Search

```sql
-- Search within client_group by role
-- Used when slot expects specific role (e.g., "the IM", "the custodian")
SELECT 
    v.cge_id,
    v.entity_id,
    v.entity_name,
    v.role_names,
    v.membership_type,
    similarity(v.entity_name, $2) as score
FROM "ob-poc".v_client_group_entity_search v
WHERE v.group_id = $1
  -- Role filter (from slot type mapping)
  AND ($3::uuid IS NULL OR $3 = ANY(v.role_ids))
  -- Name match
  AND (
      v.entity_name ILIKE '%' || $2 || '%'
      OR similarity(v.entity_name, $2) > 0.3
  )
ORDER BY 
    -- Prefer in_group over external
    CASE v.membership_type WHEN 'in_group' THEN 0 ELSE 1 END,
    score DESC
LIMIT 10;
```

### Slot Type → Role ID Mapping

Map intent slot types to role IDs (not role names, for FK integrity):

```rust
/// Map verb slot types to role_id for scoped search
fn get_role_id_for_slot_type(slot_type: &str, db: &Pool) -> Option<Uuid> {
    let role_name = match slot_type {
        "im" | "investment_manager" => "INVESTMENT_MANAGER",
        "custodian" => "CUSTODIAN", 
        "ta" => "TRANSFER_AGENT",
        "manco" => "MANAGEMENT_COMPANY",
        "depositary" => "DEPOSITARY",
        "auditor" => "AUDITOR",
        "administrator" => "FUND_ADMINISTRATOR",
        // Structural roles
        "spv" => "SPV",
        "holding" => "HOLDING_COMPANY",
        // Fund types (may need multiple)
        "fund" => return get_fund_role_ids(db), // Returns array
        _ => return None,
    };
    
    // Lookup role_id from roles table (cached)
    lookup_role_id(role_name, db)
}
```

### Ensure Required Roles Exist in `roles` Table

If missing, seed these for client group context:

```sql
-- Check existing
SELECT name, role_category FROM "ob-poc".roles 
WHERE name IN (
    'INVESTMENT_MANAGER', 'CUSTODIAN', 'TRANSFER_AGENT', 
    'MANAGEMENT_COMPANY', 'DEPOSITARY', 'FUND_ADMINISTRATOR',
    'SPV', 'HOLDING_COMPANY', 'ULTIMATE_PARENT', 'SUBSIDIARY',
    'SICAV', 'UCITS', 'AIF', 'FUND', 'UMBRELLA'
);

-- Add any missing (idempotent)
INSERT INTO "ob-poc".roles (name, role_category, description, is_active)
SELECT * FROM (VALUES
    ('ULTIMATE_PARENT', 'structural', 'Top of corporate ownership chain', true),
    ('HOLDING_COMPANY', 'structural', 'Intermediate holding company', true),
    ('SUBSIDIARY', 'structural', 'Owned subsidiary', true),
    ('SPV', 'structural', 'Special purpose vehicle', true),
    ('SICAV', 'fund', 'Variable capital investment company', true),
    ('UCITS', 'fund', 'EU regulated retail fund', true),
    ('AIF', 'fund', 'Alternative investment fund', true),
    ('UMBRELLA', 'fund', 'Umbrella fund structure', true),
    ('SUBFUND', 'fund', 'Compartment of umbrella', true)
) AS v(name, role_category, description, is_active)
WHERE NOT EXISTS (SELECT 1 FROM "ob-poc".roles WHERE roles.name = v.name);
```

---

## Alignment: CBU vs Client Group Roles

| Aspect | CBU | Client Group |
|--------|-----|--------------|
| **Master table** | `roles` | `roles` (same) |
| **Junction** | `cbu_entity_roles` | `client_group_entity_roles` |
| **Lookup pattern** | `lookup: { table: roles, search_key: name }` | Same |
| **Target entity** | ✓ (e.g., IM for which fund) | ✓ Same |
| **Effective dates** | ✓ | ✓ |
| **Resolution mode** | `reference` (autocomplete) | Same |

---

## Summary: What Changed

| Before | After |
|--------|-------|
| `role_tags TEXT[]` on `client_group_entity` | Junction table `client_group_entity_roles` |
| Ad-hoc tag validation trigger | FK to existing `roles` table |
| New `master_role_tags` table | Reuse existing `roles` table |
| TEXT array search | UUID-based role filtering |

**Benefits:**
1. Single source of truth for role taxonomy
2. Same lookup pattern as CBU verbs
3. FK integrity (no invalid roles)
4. Target entity support (IM *for* specific fund)
5. Effective dates (role changes over time)
6. Role metadata (kyc_obligation, ubo_treatment) available

---

## Resolver Rule for Intent Pipeline

**Integration point:** This connects directly to `TODO-INTENT-PIPELINE-FIXES.md` Section 1.2 (Typed Search Dispatch).

### Slot Types (Typed Resolution)

Each verb slot declares what **type** of reference it expects:

| Slot Type | Resolves To | Table |
|-----------|-------------|-------|
| `ClientGroupRef` | Single client group | `client_group` |
| `EntityRef` | Single entity | `entities` via `client_group_entity` |
| `EntitySetRef` | Multiple entities | `entities` via `client_group_entity` |
| `CbuRef` | Single CBU | `cbus` |
| `CbuSetRef` | Multiple CBUs | `cbus` |
| `ProductRef` | Product | `products` |
| `RoleRef` | Role type | `roles` |

### Preferred Roles per Verb Slot

Each slot **optionally** declares preferred roles to filter the search:

```yaml
# Example: product.apply verb
product.apply:
  slots:
    target:
      type: CbuSetRef
      preferred_roles: [FUND_VEHICLE, SPV, SICAV]
    product:
      type: ProductRef

# Example: docs.request verb  
docs.request:
  slots:
    party:
      type: EntityRef
      preferred_roles: [INVESTMENT_MANAGER, CUSTODIAN]
```

This is the **missing glue**: verb → expected slot type → role filter.

### Candle Output Format (Required)

Candle must output not just a verb, but slot types + preferred roles:

```json
{
  "verb": "product.apply",
  "confidence": 0.92,
  "slots": {
    "target": { 
      "type": "CbuSetRef", 
      "preferred_roles": ["FUND_VEHICLE", "SPV"],
      "raw_text": "Allianz"
    },
    "product": { 
      "type": "ProductRef",
      "raw_text": "custody"
    }
  }
}
```

### The Resolver Algorithm (Deterministic)

**Input:**
- `mention_text` (e.g., "Allianz", "the IM", "BlackRock")
- `expected_slot_type` (e.g., `CbuSetRef`)
- `preferred_roles` (e.g., `["FUND_VEHICLE", "SPV"]`)
- `scope` (client_group_id, persona, current focus)

**Steps:**

```rust
fn resolve_mention(
    mention_text: &str,
    expected_type: SlotType,
    preferred_roles: Option<Vec<&str>>,
    scope: &mut ScopeContext,
    role_cache: &RoleCache,
) -> ResolverResult {
    
    // === STEP 1: Scope Setting ===
    // If no scope and expected type is ClientGroupRef (or mention looks like group alias)
    if scope.group_id.is_none() && 
       (expected_type == SlotType::ClientGroupRef || looks_like_group_alias(mention_text)) {
        if let Some(group) = resolve_client_group_alias(mention_text) {
            scope.set_group(group.id);
            return ResolverResult::ScopeSet(group);
        }
    }
    
    // === STEP 2: Role-Constrained Search (First Pass) ===
    // Determine roles: from slot declaration OR inferred from text
    let role_ids = preferred_roles
        .map(|roles| role_cache.get_ids(&roles))
        .or_else(|| infer_roles_from_text(mention_text, role_cache));
    
    let results = match expected_type {
        SlotType::EntityRef | SlotType::EntitySetRef => 
            search_entities_in_scope(scope.group_id, mention_text, role_ids),
        SlotType::CbuRef | SlotType::CbuSetRef =>
            search_cbus_in_scope(scope.group_id, mention_text, role_ids),
        _ => vec![],
    };
    
    if !results.is_empty() {
        return rank_and_decide(results, expected_type);
    }
    
    // === STEP 3: Scoped General Search (Second Pass) ===
    // No role filter, but still typed (entity vs CBU)
    let results = match expected_type {
        SlotType::EntityRef | SlotType::EntitySetRef =>
            search_entities_in_scope(scope.group_id, mention_text, None),
        SlotType::CbuRef | SlotType::CbuSetRef =>
            search_cbus_in_scope(scope.group_id, mention_text, None),
        _ => vec![],
    };
    
    if !results.is_empty() {
        return rank_and_decide(results, expected_type);
    }
    
    // === STEP 4: Global Fallback (Only if Allowed) ===
    // Only if verb permits global search (e.g., IM might be external)
    if scope.allows_global_fallback {
        let results = search_global(mention_text, expected_type);
        if !results.is_empty() {
            return ResolverResult::GlobalMatch(results, "not in current scope");
        }
    }
    
    // === STEP 5: Not Found ===
    ResolverResult::NotFound {
        suggestions: vec!["add entity to group", "create new entity"],
    }
}

fn rank_and_decide(results: Vec<Match>, expected_type: SlotType) -> ResolverResult {
    // Ranking priority:
    // 1. Role match
    // 2. Tag match  
    // 3. Name similarity
    // 4. membership_type priority (in_group > external_partner > service_provider)
    // 5. Recency
    
    let ranked = rank_results(results);
    
    if ranked.len() == 1 || ranked[0].score >> ranked[1].score {
        // Single high-confidence match → resolve
        ResolverResult::Resolved(ranked[0].clone())
    } else {
        // Multiple close matches → picker
        ResolverResult::NeedsPicker(ranked)
    }
}
```

### The "Allianz" Rule (Context-Dependent Resolution)

When `mention_text` exactly matches the client group name/alias:

| Expected Slot Type | Resolution |
|--------------------|------------|
| `ClientGroupRef` | Set scope to this group |
| `CbuSetRef` | All CBUs in this group (footprint shows "28 CBUs") |
| `EntityRef` | The group's `CLIENT_ANCHOR` entity (or picker if not set) |
| `EntitySetRef` | All entities in this group |

```rust
fn resolve_group_name_mention(
    group: &ClientGroup,
    expected_type: SlotType,
) -> ResolverResult {
    match expected_type {
        SlotType::ClientGroupRef => 
            ResolverResult::ScopeSet(group),
        SlotType::CbuSetRef => 
            ResolverResult::Resolved(CbuSet::all_in_group(group.id)),
        SlotType::EntityRef => {
            if let Some(anchor) = group.client_anchor_entity_id {
                ResolverResult::Resolved(anchor)
            } else {
                ResolverResult::NeedsPicker(get_candidate_anchors(group.id))
            }
        },
        SlotType::EntitySetRef =>
            ResolverResult::Resolved(EntitySet::all_in_group(group.id)),
        _ => ResolverResult::Ambiguous,
    }
}
```

### Role Inference from Text

When no preferred roles declared, infer from mention text:

```rust
fn infer_roles_from_text(text: &str, cache: &RoleCache) -> Option<Vec<Uuid>> {
    let lower = text.to_lowercase();
    
    let role_names = if lower.contains("manco") || lower.contains("management company") {
        vec!["MANAGEMENT_COMPANY"]
    } else if lower.contains("im") || lower.contains("investment manager") {
        vec!["INVESTMENT_MANAGER"]
    } else if lower.contains("spv") || lower.contains("special purpose") {
        vec!["SPV"]
    } else if lower.contains("sicav") {
        vec!["SICAV"]
    } else if lower.contains("fund") {
        vec!["FUND", "SICAV", "UCITS", "AIF"]
    } else if lower.contains("custodian") {
        vec!["CUSTODIAN"]
    } else if lower.contains("ta") || lower.contains("transfer agent") {
        vec!["TRANSFER_AGENT"]
    } else if lower.contains("depositary") {
        vec!["DEPOSITARY"]
    } else if lower.contains("admin") {
        vec!["FUND_ADMINISTRATOR"]
    } else {
        return None;
    };
    
    cache.get_ids(&role_names)
}
```

### SQL: Role-Filtered Search via Junction

```sql
-- Search within client_group by role (using junction table)
SELECT 
    cge.id as cge_id,
    cge.entity_id,
    e.name as entity_name,
    cge.membership_type,
    array_agg(DISTINCT r.name) as role_names,
    similarity(e.name, $2) as score
FROM "ob-poc".client_group_entity cge
JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
LEFT JOIN "ob-poc".client_group_entity_roles cer ON cer.cge_id = cge.id
    AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
LEFT JOIN "ob-poc".roles r ON r.role_id = cer.role_id
WHERE cge.group_id = $1
  AND cge.membership_type != 'historical'
  -- Role filter (if provided)
  AND ($3::uuid[] IS NULL OR cer.role_id = ANY($3))
  -- Name match
  AND (
      e.name ILIKE '%' || $2 || '%'
      OR similarity(e.name, $2) > 0.3
  )
GROUP BY cge.id, cge.entity_id, e.name, cge.membership_type
ORDER BY 
    -- Role match first (if filtering)
    CASE WHEN $3 IS NOT NULL AND cer.role_id = ANY($3) THEN 0 ELSE 1 END,
    -- Then in_group over external
    CASE cge.membership_type WHEN 'in_group' THEN 0 ELSE 1 END,
    score DESC
LIMIT 10;
```

### Example Resolution Flow

```
Session start:
  User: "Allianz"
  → expected: ClientGroupRef (default for bare mention)
  → resolve_client_group_alias("Allianz") → found
  → scope.set_group(allianz_id)
  → chip shows "Client: Allianz"

Action:
  User: "add custody product to Allianz CBU"
  → Candle: verb=product.apply, target={type: CbuSetRef, text: "Allianz CBU"}
  → mention matches group name + expected CbuSetRef
  → resolve to ALL CBUs in group
  → runbook shows "Targets: 28 CBUs"

Role-specific:
  User: "send docs request to the IM"
  → Candle: verb=docs.request, party={type: EntityRef, preferred_roles: [INVESTMENT_MANAGER]}
  → search entities with role INVESTMENT_MANAGER
  → finds BlackRock (membership_type=external_partner, role=INVESTMENT_MANAGER)
  → resolved

Ambiguous:
  User: "the fund"
  → infer roles: [FUND, SICAV, UCITS, AIF]
  → search finds 3 matches with similar scores
  → show picker: "Which fund? [Allianz Lux SICAV] [Allianz Ireland AIF] [Allianz UK Fund]"
```

---

## Schema Changes

### Design Principle: Separate Membership from Ownership Edges

ChatGPT correctly identified that the original design conflated two distinct concepts:

| Concept | What it means | Table |
|---------|---------------|-------|
| **Membership** | "Entity X is part of group G" + tags + review | `client_group_entity` |
| **Ownership Edge** | "Entity A owns X% of Entity B" + lineage | `client_group_relationship` |

An entity can be a **member** of a group without having any ownership data yet (just discovered via GLEIF). And an entity can have **multiple parents** (shareholders), each with multiple sources.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  CLEAN SEPARATION                                                            │
│                                                                              │
│  client_group_entity          client_group_relationship                      │
│  ─────────────────────        ─────────────────────────                      │
│  "Who is in the group?"       "Who owns whom, by how much?"                  │
│  + tags (FUND, MANCO...)      + relationship_kind (ownership, control...)    │
│  + membership_type            + effective dates                              │
│  + review workflow                     │                                     │
│                                        ▼                                     │
│                               client_group_relationship_sources              │
│                               ─────────────────────────────────              │
│                               "What sources claim this edge?"                │
│                               + allegation → verification lineage            │
│                               + confidence, provenance, canonical flag       │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Table 1: `client_group_entity` (Membership + Role Tags)

Purpose: Track which entities belong to a client group's working set, with **role tags** for intent resolution and review workflow. **No ownership data here** — that goes in `client_group_relationship`.

```sql
CREATE TABLE "ob-poc".client_group_entity (
    id UUID DEFAULT gen_random_uuid() NOT NULL,
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- === ORTHOGONAL FLAG 1: Membership Type ===
    -- Is this entity part of the corporate group or external?
    membership_type TEXT NOT NULL DEFAULT 'in_group',
    -- in_group:          Part of the corporate ownership structure
    -- external_partner:  External party with ongoing relationship (IM, custodian)
    -- counterparty:      Transaction counterparty
    -- service_provider:  One-off or project-based service provider
    -- historical:        Was member, no longer active
    
    -- === ROLES: Via Junction Table ===
    -- Roles are stored in client_group_entity_roles junction table
    -- linking to the existing roles table (same as cbu_entity_roles)
    -- See "Role Taxonomy" section for details
    
    -- Discovery metadata
    added_by TEXT NOT NULL DEFAULT 'manual',
    -- manual, gleif, bods, scraper, agent
    source_record_id VARCHAR(255),  -- LEI, BODS statement ID, etc.
    
    -- Review workflow
    review_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    -- pending, confirmed, rejected, needs_update
    reviewed_by VARCHAR(100),
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    PRIMARY KEY (id),
    -- Same entity can be in group with different membership types
    -- (e.g., BlackRock as both in_group subsidiary AND external_partner IM)
    UNIQUE (group_id, entity_id, membership_type)
);

-- Find by membership type
CREATE INDEX idx_cge_membership ON "ob-poc".client_group_entity (group_id, membership_type)
    WHERE membership_type != 'historical';

-- Find pending reviews
CREATE INDEX idx_cge_review ON "ob-poc".client_group_entity (group_id, review_status) 
    WHERE review_status IN ('pending', 'needs_update');
```

### Table 2: `client_group_relationship` (Ownership Edges)

Purpose: Track provisional ownership/control edges within a client group, **before promotion to formal shareholdings**. Each edge can have multiple sources.

```sql
CREATE TABLE "ob-poc".client_group_relationship (
    id UUID DEFAULT gen_random_uuid() NOT NULL,
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id),
    
    -- The edge: parent → child
    parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Relationship classification
    relationship_kind VARCHAR(30) NOT NULL DEFAULT 'ownership',
    -- ownership:     Direct share ownership
    -- control:       Control without ownership (voting agreements, etc.)
    -- beneficial:    Beneficial ownership (for UBO)
    -- management:    Management relationship (MANCO → fund)
    
    -- Effective period (from source documents)
    effective_from DATE,
    effective_to DATE,  -- NULL = current
    
    -- Review workflow for the edge itself
    review_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    reviewed_by VARCHAR(100),
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    
    -- Promotion tracking
    promoted_to_shareholding_id UUID REFERENCES "ob-poc".shareholdings(shareholding_id),
    promoted_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    PRIMARY KEY (id),
    -- Same edge can exist with different relationship_kinds
    UNIQUE (group_id, parent_entity_id, child_entity_id, relationship_kind)
);

-- Find all relationships for an entity (as parent or child)
CREATE INDEX idx_cgr_parent ON "ob-poc".client_group_relationship (group_id, parent_entity_id);
CREATE INDEX idx_cgr_child ON "ob-poc".client_group_relationship (group_id, child_entity_id);

-- Find unpromoted relationships
CREATE INDEX idx_cgr_unpromoted ON "ob-poc".client_group_relationship (group_id)
    WHERE promoted_to_shareholding_id IS NULL;

-- Find relationships by kind
CREATE INDEX idx_cgr_kind ON "ob-poc".client_group_relationship (group_id, relationship_kind);
```

### Table 3: `client_group_relationship_sources` (Multi-Source Lineage)

Purpose: Track multiple sources for each ownership edge. This is the **"trust but verify"** table.

```sql
CREATE TABLE "ob-poc".client_group_relationship_sources (
    id UUID DEFAULT gen_random_uuid() NOT NULL,
    relationship_id UUID NOT NULL REFERENCES "ob-poc".client_group_relationship(id) ON DELETE CASCADE,
    
    -- === SOURCE IDENTIFICATION ===
    source VARCHAR(50) NOT NULL,
    -- client_allegation, gleif, bods, companies_house, clearstream,
    -- annual_report, fund_prospectus, kyc_document, scraper, manual
    
    source_type VARCHAR(20) NOT NULL DEFAULT 'discovery',
    -- allegation:    Client-provided, trust but verify
    -- verification:  Authoritative source checking an allegation
    -- discovery:     Found during research (not verifying specific allegation)
    
    -- === OWNERSHIP VALUES FROM THIS SOURCE ===
    ownership_pct NUMERIC(5,2),
    voting_pct NUMERIC(5,2),
    control_pct NUMERIC(5,2),
    
    -- === PROVENANCE / LINEAGE ===
    source_document_ref VARCHAR(255),   -- LEI, BODS statement ID, CH filing ref
    source_document_type VARCHAR(100),  -- "share_register", "annual_return", "lei_record"
    source_document_date DATE,          -- Date ON the document
    source_effective_date DATE,         -- When ownership became effective
    source_retrieved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_retrieved_by VARCHAR(100),   -- User or system that captured this
    raw_payload JSONB,                  -- Original response for audit
    
    -- === ALLEGATION → VERIFICATION LINKAGE ===
    verifies_source_id UUID REFERENCES "ob-poc".client_group_relationship_sources(id),
    
    -- === VERIFICATION OUTCOME ===
    verification_outcome VARCHAR(20),
    -- NULL: Not a verification
    -- confirmed: Matches allegation (within threshold)
    -- disputed: Contradicts allegation
    -- partial: Some aspects confirmed
    
    discrepancy_pct NUMERIC(5,2),  -- Difference from alleged value
    
    -- === CANONICAL SELECTION ===
    -- After review, analyst can mark a source as canonical (overrides computed ranking)
    is_canonical BOOLEAN DEFAULT false,
    canonical_set_by VARCHAR(100),
    canonical_set_at TIMESTAMPTZ,
    canonical_notes TEXT,
    
    -- === KYC WORKFLOW STATUS ===
    verification_status VARCHAR(20) DEFAULT 'unverified',
    -- unverified, verified, disputed, superseded, rejected
    verified_by VARCHAR(100),
    verified_at TIMESTAMPTZ,
    verification_notes TEXT,
    
    -- === CONFIDENCE / QUALITY ===
    confidence_score NUMERIC(3,2),  -- 0.00-1.00 based on source authority
    is_direct_evidence BOOLEAN DEFAULT false,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    PRIMARY KEY (id),
    
    -- Only one canonical source per relationship
    UNIQUE (relationship_id, is_canonical) WHERE is_canonical = true,
    
    CONSTRAINT valid_source CHECK (source IN (
        'client_allegation', 'gleif', 'bods', 'companies_house', 'clearstream',
        'annual_report', 'fund_prospectus', 'kyc_document', 'scraper', 'manual'
    )),
    CONSTRAINT valid_source_type CHECK (source_type IN (
        'allegation', 'verification', 'discovery'
    )),
    CONSTRAINT verification_needs_target CHECK (
        (source_type = 'verification' AND verifies_source_id IS NOT NULL) OR
        (source_type != 'verification')
    )
);

-- Find all sources for a relationship
CREATE INDEX idx_cgrs_relationship ON "ob-poc".client_group_relationship_sources (relationship_id);

-- Find unverified allegations
CREATE INDEX idx_cgrs_unverified ON "ob-poc".client_group_relationship_sources (source_type, verification_status)
    WHERE source_type = 'allegation' AND verification_status = 'unverified';

-- Find canonical sources
CREATE INDEX idx_cgrs_canonical ON "ob-poc".client_group_relationship_sources (relationship_id)
    WHERE is_canonical = true;

-- Lineage traversal
CREATE INDEX idx_cgrs_verifies ON "ob-poc".client_group_relationship_sources (verifies_source_id)
    WHERE verifies_source_id IS NOT NULL;
```

### Table 4: `client_group` Extensions

```sql
-- Discovery status tracking
ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    discovery_status VARCHAR(20) NOT NULL DEFAULT 'not_started';
    -- not_started, in_progress, complete, stale, failed

ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    discovery_started_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    discovery_completed_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    discovery_source VARCHAR(50);  -- gleif, bods, manual, mixed

ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    discovery_root_lei VARCHAR(20);  -- Starting LEI for GLEIF crawl

-- Denormalized stats for quick UI display
ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    entity_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE "ob-poc".client_group ADD COLUMN IF NOT EXISTS
    pending_review_count INTEGER NOT NULL DEFAULT 0;

-- Trigger to maintain counts
CREATE OR REPLACE FUNCTION "ob-poc".update_client_group_counts()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE "ob-poc".client_group SET
        entity_count = (
            SELECT COUNT(*) FROM "ob-poc".client_group_entity 
            WHERE group_id = COALESCE(NEW.group_id, OLD.group_id)
            AND membership_type != 'historical'
        ),
        pending_review_count = (
            SELECT COUNT(*) FROM "ob-poc".client_group_entity 
            WHERE group_id = COALESCE(NEW.group_id, OLD.group_id)
            AND review_status IN ('pending', 'needs_update')
        ),
        updated_at = NOW()
    WHERE id = COALESCE(NEW.group_id, OLD.group_id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_cge_counts
AFTER INSERT OR UPDATE OR DELETE ON "ob-poc".client_group_entity
FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_client_group_counts();
```

### Source Authority Hierarchy

| Priority | Source | Default Confidence | Notes |
|----------|--------|-------------------|-------|
| 1 | `is_canonical = true` | — | Explicit analyst override |
| 2 | `verified` (any) | 1.00 | KYC analyst confirmed |
| 3 | `companies_house` | 0.95 | Regulatory filing, legal obligation |
| 4 | `clearstream` | 0.90 | Settlement system, high integrity |
| 5 | `bods` | 0.85 | Beneficial ownership register |
| 6 | `gleif` | 0.80 | LEI system, self-reported but validated |
| 7 | `annual_report` | 0.75 | Audited but point-in-time |
| 8 | `fund_prospectus` | 0.70 | Legal document but may be stale |
| 9 | `kyc_document` | 0.65 | Client-provided supporting doc |
| 10 | `client_allegation` | 0.50 | **Trust but verify** |
| 11 | `manual` | 0.40 | Analyst entry, needs sourcing |

**Note:** Priority can be jurisdiction-aware (e.g., Clearstream > Companies House for Luxembourg funds).

### Lineage Example

```
Client alleges: "Allianz SE owns 75% of Allianz Ireland"

┌─────────────────────────────────────────────────────────────────────────┐
│  RELATIONSHIP                                                            │
│  ────────────                                                            │
│  cgr[1]: parent = Allianz SE, child = Allianz Ireland, kind = ownership  │
└─────────────────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ SOURCE 1        │  │ SOURCE 2        │  │ SOURCE 3        │
│ ────────        │  │ ────────        │  │ ────────        │
│ allegation      │  │ verification    │  │ discovery       │
│ client_allege   │  │ companies_house │  │ gleif           │
│ 75.00%          │  │ 74.50%          │  │ 75.00%          │
│ conf: 0.50      │  │ conf: 0.95      │  │ conf: 0.80      │
│ unverified      │  │ verifies [1]    │  │                 │
│                 │  │ outcome: conf   │  │                 │
└─────────────────┘  └─────────────────┘  └─────────────────┘
                              │
                              ▼
                     CANONICAL: Source 2 (highest confidence, verified)
                     OR: Analyst sets is_canonical=true on any source
```

### View: Agent Scoped Entity Search

**Critical for intent pipeline**: This view provides the searchable fields needed for YAML lookups like `lookup: { table: v_client_group_entity_search, scope: group-id, search_key: entity_name }`.

```sql
CREATE OR REPLACE VIEW "ob-poc".v_client_group_entity_search AS
SELECT 
    cge.id as cge_id,
    cge.group_id,
    cge.entity_id,
    cge.role_tags,
    cge.membership_type,
    cge.review_status,
    -- Entity display fields for search
    e.name as entity_name,
    e.lei,
    e.jurisdiction,
    e.entity_type,
    -- Group context
    cg.canonical_name as group_name,
    -- Computed: is this an external entity?
    (cge.membership_type IN ('external_partner', 'counterparty', 'service_provider')) as is_external
FROM "ob-poc".client_group_entity cge
JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
JOIN "ob-poc".client_group cg ON cg.id = cge.group_id
WHERE cge.membership_type != 'historical';

-- Fast name search within group (trigram)
CREATE INDEX idx_entities_name_trgm ON "ob-poc".entities USING GIN (name gin_trgm_ops);
```

### View: Canonical Relationship Ownership

Returns best-available ownership value for each relationship edge.

```sql
CREATE OR REPLACE VIEW "ob-poc".v_cgr_canonical AS
SELECT DISTINCT ON (r.id)
    r.id as relationship_id,
    r.group_id,
    r.parent_entity_id,
    r.child_entity_id,
    r.relationship_kind,
    pe.name as parent_name,
    ce.name as child_name,
    s.ownership_pct,
    s.voting_pct,
    s.control_pct,
    s.source as canonical_source,
    s.source_type,
    s.verification_status,
    s.is_canonical,
    s.confidence_score
FROM "ob-poc".client_group_relationship r
JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
LEFT JOIN "ob-poc".client_group_relationship_sources s ON s.relationship_id = r.id
    AND s.verification_status != 'rejected'
ORDER BY r.id,
    -- Explicit canonical wins
    s.is_canonical DESC,
    -- Then verified
    CASE s.verification_status WHEN 'verified' THEN 0 ELSE 1 END,
    -- Then by confidence
    s.confidence_score DESC NULLS LAST,
    -- Then by recency
    s.source_document_date DESC NULLS LAST;
```

### View: Unverified Allegations

```sql
CREATE OR REPLACE VIEW "ob-poc".v_cgr_unverified_allegations AS
SELECT 
    r.group_id,
    cg.canonical_name as group_name,
    pe.name as parent_name,
    ce.name as child_name,
    r.relationship_kind,
    s.id as source_id,
    s.ownership_pct as alleged_pct,
    s.source_document_ref,
    s.source_document_date,
    -- How many verifications exist?
    (SELECT COUNT(*) FROM "ob-poc".client_group_relationship_sources v 
     WHERE v.verifies_source_id = s.id) as verification_count
FROM "ob-poc".client_group_relationship_sources s
JOIN "ob-poc".client_group_relationship r ON r.id = s.relationship_id
JOIN "ob-poc".client_group cg ON cg.id = r.group_id
JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
WHERE s.source_type = 'allegation'
  AND s.verification_status = 'unverified';
```

### View: Relationship Discrepancies

```sql
CREATE OR REPLACE VIEW "ob-poc".v_cgr_discrepancies AS
SELECT 
    r.group_id,
    r.parent_entity_id,
    r.child_entity_id,
    r.relationship_kind,
    pe.name as parent_name,
    ce.name as child_name,
    array_agg(DISTINCT s.source ORDER BY s.source) as sources,
    array_agg(s.ownership_pct ORDER BY s.confidence_score DESC) as ownership_values,
    MAX(s.ownership_pct) - MIN(s.ownership_pct) as ownership_spread,
    MAX(s.ownership_pct) FILTER (WHERE s.source_type = 'allegation') as alleged_pct,
    MAX(s.ownership_pct) FILTER (WHERE s.source_type = 'verification') as verified_pct,
    COUNT(DISTINCT s.source) as source_count
FROM "ob-poc".client_group_relationship r
JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
JOIN "ob-poc".client_group_relationship_sources s ON s.relationship_id = r.id
WHERE s.ownership_pct IS NOT NULL
  AND s.verification_status != 'rejected'
GROUP BY r.group_id, r.parent_entity_id, r.child_entity_id, r.relationship_kind,
         pe.name, ce.name
HAVING COUNT(DISTINCT s.ownership_pct) > 1;
```

### Table 5: `shareholding_sources` (Formal Shareholding Provenance)

The core KYC pattern is **"trust but verify"**:
1. Client **alleges** ownership structure
2. We **verify** against authoritative sources (Companies House, Clearstream, GLEIF, BODS)
3. We **reconcile** discrepancies and establish canonical values

This requires tracking **multiple sources for the same truth** with explicit lineage.

```sql
CREATE TABLE "ob-poc".shareholding_sources (
    id UUID DEFAULT gen_random_uuid() NOT NULL,
    shareholding_id UUID NOT NULL REFERENCES "ob-poc".shareholdings(shareholding_id),
    
    -- === SOURCE IDENTIFICATION ===
    source VARCHAR(50) NOT NULL,
    -- Sources: gleif, bods, companies_house, clearstream, client_allegation, 
    --          annual_report, prospectus, manual, scraper
    
    source_type VARCHAR(20) NOT NULL DEFAULT 'discovery',
    -- allegation:    Client-provided, needs verification
    -- verification:  Authoritative source checking an allegation
    -- discovery:     Found during research (GLEIF crawl), not verifying specific allegation
    
    -- === OWNERSHIP VALUES FROM THIS SOURCE ===
    ownership_pct NUMERIC(5,2),
    voting_pct NUMERIC(5,2),
    control_pct NUMERIC(5,2),  -- For BODS control statements
    
    -- === PROVENANCE / LINEAGE ===
    source_document_ref VARCHAR(255),  -- LEI, BODS statement ID, CH filing ref, Clearstream ref
    source_date DATE,                   -- Date of source record (e.g., filing date)
    effective_date DATE,                -- Date ownership became effective (if different)
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    raw_payload JSONB,                  -- Store raw response for audit trail
    
    -- === ALLEGATION → VERIFICATION LINKAGE ===
    -- If this is a verification, what allegation is it verifying?
    verifies_source_id UUID REFERENCES "ob-poc".shareholding_sources(id),
    
    -- === VERIFICATION OUTCOME ===
    verification_outcome VARCHAR(20),
    -- NULL:       Not a verification (allegation or discovery)
    -- confirmed:  Verification matches allegation (within threshold)
    -- disputed:   Verification contradicts allegation
    -- partial:    Some aspects confirmed, others disputed
    -- superseded: Newer verification available
    
    discrepancy_pct NUMERIC(5,2),  -- Difference from alleged value (if verification)
    
    -- === KYC WORKFLOW STATUS ===
    review_status VARCHAR(20) DEFAULT 'pending',
    -- pending:     Awaiting review
    -- accepted:    Accepted as valid source
    -- rejected:    Source rejected (bad data, outdated, etc.)
    -- superseded:  Replaced by newer source
    
    reviewed_by VARCHAR(100),
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    PRIMARY KEY (id),
    
    CONSTRAINT valid_source CHECK (source IN (
        'gleif', 'bods', 'companies_house', 'clearstream', 'client_allegation', 
        'annual_report', 'prospectus', 'manual', 'scraper'
    )),
    CONSTRAINT valid_source_type CHECK (source_type IN (
        'allegation', 'verification', 'discovery'
    )),
    CONSTRAINT valid_verification_outcome CHECK (verification_outcome IS NULL OR verification_outcome IN (
        'confirmed', 'disputed', 'partial', 'superseded'
    )),
    -- Verification must reference an allegation
    CONSTRAINT verification_needs_target CHECK (
        (source_type = 'verification' AND verifies_source_id IS NOT NULL) OR
        (source_type != 'verification' AND verifies_source_id IS NULL)
    )
);

-- Find all sources for a shareholding
CREATE INDEX idx_ss_shareholding ON "ob-poc".shareholding_sources (shareholding_id);

-- Find allegations needing verification
CREATE INDEX idx_ss_allegations_pending ON "ob-poc".shareholding_sources (shareholding_id, source_type)
    WHERE source_type = 'allegation' AND review_status = 'pending';

-- Find verifications for an allegation
CREATE INDEX idx_ss_verifications ON "ob-poc".shareholding_sources (verifies_source_id)
    WHERE verifies_source_id IS NOT NULL;

-- Find disputed sources
CREATE INDEX idx_ss_disputed ON "ob-poc".shareholding_sources (shareholding_id, verification_outcome)
    WHERE verification_outcome = 'disputed';

-- Timeline queries (by source date)
CREATE INDEX idx_ss_timeline ON "ob-poc".shareholding_sources (shareholding_id, source_date DESC);
```

### Lineage Example

```
Client alleges: "Allianz SE owns 75% of Allianz Ireland"
   │
   ├── shareholding_sources[1]: source='client_allegation', source_type='allegation'
   │                            ownership_pct=75.00
   │
   ├── We verify against Companies House:
   │   shareholding_sources[2]: source='companies_house', source_type='verification'
   │                            ownership_pct=74.50
   │                            verifies_source_id=[1]
   │                            verification_outcome='confirmed' (within 1% threshold)
   │                            discrepancy_pct=0.50
   │
   └── We also check GLEIF (discovery, not verifying specific allegation):
       shareholding_sources[3]: source='gleif', source_type='discovery'
                                ownership_pct=75.00
                                verifies_source_id=NULL
```

### View: Allegations with Verification Status

```sql
CREATE OR REPLACE VIEW "ob-poc".v_shareholding_allegations AS
SELECT 
    a.id as allegation_id,
    a.shareholding_id,
    s.shareholder_entity_id,
    s.owned_entity_id,
    a.ownership_pct as alleged_ownership_pct,
    a.source_date as allegation_date,
    a.review_status as allegation_status,
    
    -- Verification summary
    COUNT(v.id) as verification_count,
    COUNT(v.id) FILTER (WHERE v.verification_outcome = 'confirmed') as confirmed_count,
    COUNT(v.id) FILTER (WHERE v.verification_outcome = 'disputed') as disputed_count,
    
    -- Latest verification
    (SELECT v2.source FROM "ob-poc".shareholding_sources v2 
     WHERE v2.verifies_source_id = a.id 
     ORDER BY v2.fetched_at DESC LIMIT 1) as latest_verification_source,
    (SELECT v2.verification_outcome FROM "ob-poc".shareholding_sources v2 
     WHERE v2.verifies_source_id = a.id 
     ORDER BY v2.fetched_at DESC LIMIT 1) as latest_verification_outcome,
    
    -- Is this allegation fully verified?
    CASE 
        WHEN COUNT(v.id) = 0 THEN 'unverified'
        WHEN COUNT(v.id) FILTER (WHERE v.verification_outcome = 'disputed') > 0 THEN 'disputed'
        WHEN COUNT(v.id) FILTER (WHERE v.verification_outcome = 'confirmed') > 0 THEN 'verified'
        ELSE 'partial'
    END as verification_status
    
FROM "ob-poc".shareholding_sources a
JOIN "ob-poc".shareholdings s ON s.shareholding_id = a.shareholding_id
LEFT JOIN "ob-poc".shareholding_sources v ON v.verifies_source_id = a.id
WHERE a.source_type = 'allegation'
GROUP BY a.id, a.shareholding_id, s.shareholder_entity_id, s.owned_entity_id,
         a.ownership_pct, a.source_date, a.review_status;
```

### View: Source Discrepancies

```sql
-- Find shareholdings where sources disagree beyond threshold
CREATE OR REPLACE VIEW "ob-poc".v_shareholding_discrepancies AS
WITH source_values AS (
    SELECT 
        shareholding_id,
        source,
        source_type,
        ownership_pct,
        ROW_NUMBER() OVER (PARTITION BY shareholding_id ORDER BY 
            CASE source_type WHEN 'allegation' THEN 0 ELSE 1 END,
            fetched_at DESC
        ) as rn
    FROM "ob-poc".shareholding_sources
    WHERE ownership_pct IS NOT NULL
      AND review_status != 'rejected'
)
SELECT 
    sv.shareholding_id,
    s.shareholder_entity_id,
    s.owned_entity_id,
    array_agg(sv.source ORDER BY sv.source) as sources,
    array_agg(sv.ownership_pct ORDER BY sv.source) as ownership_values,
    MAX(sv.ownership_pct) as max_pct,
    MIN(sv.ownership_pct) as min_pct,
    MAX(sv.ownership_pct) - MIN(sv.ownership_pct) as spread_pct,
    -- Alleged value (if any)
    MAX(sv.ownership_pct) FILTER (WHERE sv.source_type = 'allegation') as alleged_pct,
    -- Best verification value
    MAX(sv.ownership_pct) FILTER (WHERE sv.source_type = 'verification') as verified_pct
FROM source_values sv
JOIN "ob-poc".shareholdings s ON s.shareholding_id = sv.shareholding_id
GROUP BY sv.shareholding_id, s.shareholder_entity_id, s.owned_entity_id
HAVING COUNT(DISTINCT sv.ownership_pct) > 1;
```

### View: Canonical Shareholding (Reconciled)

```sql
-- Canonical shareholding: verified > companies_house > bods > gleif > manual
CREATE OR REPLACE VIEW "ob-poc".v_shareholding_canonical AS
SELECT DISTINCT ON (s.shareholding_id)
    s.*,
    ss.source as canonical_source,
    ss.ownership_pct as canonical_ownership_pct,
    ss.voting_pct as canonical_voting_pct,
    ss.verification_status,
    ss.source_date
FROM "ob-poc".shareholdings s
JOIN "ob-poc".shareholding_sources ss ON s.shareholding_id = ss.shareholding_id
ORDER BY s.shareholding_id, 
    CASE ss.verification_status WHEN 'verified' THEN 0 ELSE 1 END,
    CASE ss.source 
        WHEN 'companies_house' THEN 1
        WHEN 'bods' THEN 2
        WHEN 'gleif' THEN 3
        WHEN 'annual_report' THEN 4
        WHEN 'client_allegation' THEN 5
        ELSE 6 
    END,
    ss.source_date DESC NULLS LAST;

-- Find shareholdings with discrepancies
CREATE OR REPLACE VIEW "ob-poc".v_shareholding_discrepancies AS
SELECT 
    s.shareholding_id,
    s.shareholder_entity_id,
    s.owned_entity_id,
    array_agg(ss.source ORDER BY ss.source) as sources,
    array_agg(ss.ownership_pct ORDER BY ss.source) as ownership_values,
    MAX(ss.ownership_pct) - MIN(ss.ownership_pct) as ownership_spread
FROM "ob-poc".shareholdings s
JOIN "ob-poc".shareholding_sources ss ON s.shareholding_id = ss.shareholding_id
WHERE ss.ownership_pct IS NOT NULL
GROUP BY s.shareholding_id, s.shareholder_entity_id, s.owned_entity_id
HAVING COUNT(DISTINCT ss.ownership_pct) > 1;
```

---

## DSL Verbs

### Domain: `client-group`

```yaml
verbs:
  # === Discovery Lifecycle ===
  
  start-discovery:
    description: "Start research discovery for a client group"
    behavior: crud
    invocation_phrases:
      - "research {group}"
      - "start discovery for {group}"
      - "discover {group} structure"
    args:
      - name: group-id
        type: uuid
        required: true
        lookup: { table: client_group, search_key: canonical_name }
      - name: source
        type: string
        required: false
        default: "gleif"
        valid_values: [gleif, bods, manual, mixed]
      - name: root-lei
        type: string
        required: false
        description: "Starting LEI for GLEIF crawl (auto-detected if not provided)"

  complete-discovery:
    description: "Mark discovery complete and optionally trigger structuring"
    behavior: plugin
    invocation_phrases:
      - "complete {group} discovery"
      - "finish researching {group}"
      - "done with {group} research"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: create-cbus
        type: boolean
        required: false
        default: false
        description: "Create CBUs for FUND-tagged entities"
      - name: build-shareholdings
        type: boolean
        required: false
        default: false
        description: "Promote provisional ownership to shareholdings"
      - name: discover-ubos
        type: boolean
        required: false
        default: false
        description: "Run UBO discovery after structuring"

  # === Entity Collection Management ===

  add-entity:
    description: "Add an entity to a client group collection"
    behavior: crud
    invocation_phrases:
      - "add {entity} to {group}"
      - "{entity} is in {group}"
    args:
      - name: group-id
        type: uuid
        required: true
        lookup: { table: client_group, search_key: canonical_name }
      - name: entity-id
        type: uuid
        required: true
        lookup: { table: entities, search_key: name }
      - name: membership-type
        type: string
        required: false
        default: "in_group"
        valid_values: [in_group, external_partner, counterparty, service_provider]
    # Note: Roles assigned separately via assign-role verb

  remove-entity:
    description: "Remove an entity from a client group (marks as historical)"
    behavior: crud
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: hard-delete
        type: boolean
        required: false
        default: false
        description: "Actually delete vs mark historical"

  list-entities:
    description: "List all entities in a client group"
    behavior: crud
    invocation_phrases:
      - "show {group} entities"
      - "list {group} structure"
      - "what did we find for {group}"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: tags
        type: list
        required: false
        description: "Filter by tags (AND logic)"
      - name: review-status
        type: string
        required: false
        valid_values: [pending, confirmed, rejected, needs_update]
      - name: include-historical
        type: boolean
        required: false
        default: false

  # === Role Management (Same Pattern as CBU) ===

  assign-role:
    description: "Assign a role to an entity within a client group"
    behavior: crud
    invocation_phrases:
      - "assign {role} to {entity}"
      - "{entity} is the {role}"
      - "make {entity} the {role}"
    metadata:
      tier: intent
      source_of_truth: operational
      scope: scoped
      noun: client-group
      tags: [relationship, write, role]
    crud:
      operation: role_link
      junction: client_group_entity_roles
      schema: ob-poc
      from_col: cge_id
      to_col: role_id
      role_table: roles
      returning: id
    args:
      - name: group-id
        type: uuid
        required: true
        lookup: { table: client_group, search_key: canonical_name }
      - name: entity-id
        type: uuid
        required: true
        lookup: 
          table: v_client_group_entity_search
          scope: group-id
          search_key: entity_name
      - name: role
        type: lookup
        required: true
        lookup:
          schema: ob-poc
          table: roles
          entity_type: role
          code_column: name
          id_column: role_id
      - name: target-entity-id
        type: uuid
        required: false
        description: "For directed roles (e.g., IM for which fund)"
        lookup:
          table: v_client_group_entity_search
          scope: group-id
          search_key: entity_name
      - name: effective-from
        type: date
        required: false

  remove-role:
    description: "Remove a role from an entity within a client group"
    behavior: crud
    invocation_phrases:
      - "remove {role} from {entity}"
      - "{entity} is no longer the {role}"
    metadata:
      tier: intent
      source_of_truth: operational
      scope: scoped
      noun: client-group
      tags: [relationship, write, role]
    crud:
      operation: role_unlink
      junction: client_group_entity_roles
      schema: ob-poc
      from_col: cge_id
      role_table: roles
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
        lookup:
          table: v_client_group_entity_search
          scope: group-id
          search_key: entity_name
      - name: role
        type: lookup
        required: true
        lookup:
          schema: ob-poc
          table: roles
          entity_type: role
          code_column: name
          id_column: role_id

  list-roles:
    description: "List all roles for an entity or all entities with a role"
    behavior: crud
    invocation_phrases:
      - "show roles for {entity}"
      - "who is the {role}"
      - "list {role} entities"
    metadata:
      tier: diagnostics
      source_of_truth: operational
      scope: scoped
      noun: client-group
      tags: [read, query, role]
    crud:
      operation: list_parties
      junction: client_group_entity_roles
      schema: ob-poc
      fk_col: cge_id
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: false
        description: "Filter to roles for this entity"
        lookup:
          table: v_client_group_entity_search
          scope: group-id
          search_key: entity_name
      - name: role
        type: lookup
        required: false
        description: "Filter to entities with this role"
        lookup:
          schema: ob-poc
          table: roles
          entity_type: role
          code_column: name
          id_column: role_id
      - name: include-expired
        type: boolean
        required: false
        default: false
        description: "Include roles with effective_to in the past"

  parties:
    description: "List all parties (entities with their roles) for a client group"
    behavior: crud
    invocation_phrases:
      - "show parties for {group}"
      - "who are the parties"
      - "list all roles"
    metadata:
      tier: diagnostics
      source_of_truth: operational
      scope: scoped
      noun: client-group
      tags: [read, query, relationship]
    crud:
      operation: list_parties
      junction: client_group_entity_roles
      schema: ob-poc
      fk_col: cge_id
    args:
      - name: group-id
        type: uuid
        required: true
      - name: role-category
        type: string
        required: false
        valid_values: [structural, fund, service, compliance]
        description: "Filter by role category"

  # === Review Workflow ===

  confirm-entity:
    description: "Confirm a pending entity in the collection"
    behavior: crud
    invocation_phrases:
      - "confirm {entity}"
      - "approve {entity}"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: reviewer
        type: string
        required: false
      - name: notes
        type: string
        required: false

  reject-entity:
    description: "Reject a pending entity (mark for removal)"
    behavior: crud
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: reviewer
        type: string
        required: false
      - name: notes
        type: string
        required: true
        description: "Reason for rejection"

  list-pending:
    description: "List all entities pending review"
    behavior: crud
    invocation_phrases:
      - "show pending for {group}"
      - "what needs review in {group}"
    args:
      - name: group-id
        type: uuid
        required: true

  # === Relationship Management (Ownership Edges) ===

  add-relationship:
    description: "Create a provisional ownership edge between entities"
    behavior: crud
    invocation_phrases:
      - "{parent} owns {child}"
      - "add ownership from {parent} to {child}"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: parent-entity-id
        type: uuid
        required: true
        lookup: { table: v_client_group_entity_search, scope: group-id, search_key: entity_name }
      - name: child-entity-id
        type: uuid
        required: true
        lookup: { table: v_client_group_entity_search, scope: group-id, search_key: entity_name }
      - name: relationship-kind
        type: string
        required: false
        default: "ownership"
        valid_values: [ownership, control, beneficial, management]
      - name: effective-from
        type: date
        required: false

  remove-relationship:
    description: "Remove an ownership edge"
    behavior: crud
    args:
      - name: relationship-id
        type: uuid
        required: true

  list-relationships:
    description: "List all ownership edges in a group"
    behavior: crud
    invocation_phrases:
      - "show ownership structure"
      - "list relationships in {group}"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: false
        description: "Filter to relationships involving this entity"
      - name: relationship-kind
        type: string
        required: false

  # === Source Management (Trust But Verify) ===

  add-ownership-source:
    description: "Add a source claim for an ownership relationship"
    behavior: plugin  # Changed: has business logic (confidence scoring, verification linkage)
    invocation_phrases:
      - "client alleges {parent} owns {child}"
      - "add source for {parent} to {child} ownership"
    args:
      - name: relationship-id
        type: uuid
        required: true
        description: "The relationship edge to add source data to"
      - name: source
        type: string
        required: true
        valid_values: [client_allegation, gleif, bods, companies_house, clearstream, annual_report, fund_prospectus, kyc_document, manual]
      - name: source-type
        type: string
        required: false
        default: "discovery"
        valid_values: [allegation, verification, discovery]
      - name: ownership-pct
        type: decimal
        required: false
      - name: voting-pct
        type: decimal
        required: false
      - name: source-document-ref
        type: string
        required: false
      - name: source-document-date
        type: date
        required: false
      - name: verifies-source-id
        type: uuid
        required: false
        description: "If verification, which allegation does this verify?"

  verify-ownership:
    description: "Mark an ownership source as verified (KYC workflow)"
    behavior: plugin  # Changed: computes discrepancy, sets outcome, may supersede others
    invocation_phrases:
      - "verify ownership source"
      - "confirm {source} ownership"
    args:
      - name: source-id
        type: uuid
        required: true
      - name: verified-by
        type: string
        required: true
      - name: verification-notes
        type: string
        required: false

  set-canonical:
    description: "Explicitly set a source as canonical (analyst override)"
    behavior: plugin  # Changed: explicit business decision
    invocation_phrases:
      - "trust {source} for this ownership"
      - "use {source} as canonical"
    args:
      - name: source-id
        type: uuid
        required: true
      - name: canonical-set-by
        type: string
        required: true
      - name: canonical-notes
        type: string
        required: true
        description: "Reason for choosing this source"

  list-unverified:
    description: "List all unverified ownership allegations"
    behavior: crud
    invocation_phrases:
      - "show unverified allegations"
      - "what ownership needs verification"
    args:
      - name: group-id
        type: uuid
        required: true

  list-discrepancies:
    description: "Find relationships where sources disagree on ownership"
    behavior: plugin
    invocation_phrases:
      - "show ownership discrepancies"
      - "find conflicting ownership"
      - "where do sources disagree"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: threshold-pct
        type: decimal
        required: false
        default: 5.0
        description: "Flag if sources differ by more than this %"
      - name: relationship-kind
        type: string
        required: false
        description: "Filter by relationship kind"
```

### Domain: `gleif` (Extended)

```yaml
verbs:
  import-to-group:
    description: "Import GLEIF entities into a client group buffer"
    behavior: plugin
    invocation_phrases:
      - "import gleif to {group}"
      - "crawl gleif for {group}"
      - "discover {group} from gleif"
    args:
      - name: group-id
        type: uuid
        required: true
        lookup: { table: client_group, search_key: canonical_name }
      - name: root-lei
        type: string
        required: true
        description: "Starting LEI for crawl"
      - name: direction
        type: string
        required: false
        default: "BOTH"
        valid_values: [UP, DOWN, BOTH]
      - name: max-depth
        type: integer
        required: false
        default: 5
      - name: auto-tag
        type: boolean
        required: false
        default: true
        description: "Auto-classify entities based on GLEIF data"

  refresh-group:
    description: "Refresh GLEIF data for all entities in a client group"
    behavior: plugin
    invocation_phrases:
      - "refresh {group} from gleif"
      - "update gleif for {group}"
    args:
      - name: group-id
        type: uuid
        required: true
      - name: flag-changes
        type: boolean
        required: false
        default: true
        description: "Set review_status = 'needs_update' for changed entities"
```

### Domain: `shareholding` (Extended)

```yaml
verbs:
  add-source:
    description: "Add a source record to an existing shareholding"
    behavior: crud
    args:
      - name: shareholding-id
        type: uuid
        required: true
      - name: source
        type: string
        required: true
        valid_values: [gleif, bods, companies_house, client_allegation, annual_report, manual]
      - name: ownership-pct
        type: decimal
        required: false
      - name: voting-pct
        type: decimal
        required: false
      - name: source-document-ref
        type: string
        required: false
      - name: source-date
        type: date
        required: false

  verify-source:
    description: "Mark a shareholding source as verified (KYC workflow)"
    behavior: crud
    invocation_phrases:
      - "verify {source} for shareholding"
      - "confirm {source} ownership"
    args:
      - name: shareholding-id
        type: uuid
        required: true
      - name: source
        type: string
        required: true
      - name: verified-by
        type: string
        required: true
      - name: verification-notes
        type: string
        required: false

  list-discrepancies:
    description: "Find shareholdings where sources disagree"
    behavior: plugin
    invocation_phrases:
      - "show ownership discrepancies"
      - "find conflicting ownership"
      - "compare ownership sources"
    args:
      - name: group-id
        type: uuid
        required: false
        description: "Limit to entities in this group"
      - name: threshold-pct
        type: decimal
        required: false
        default: 5.0
        description: "Flag if sources differ by more than this %"

  promote-provisional:
    description: "Promote provisional ownership from client_group_entity to shareholdings"
    behavior: plugin
    args:
      - name: group-id
        type: uuid
        required: true
      - name: source
        type: string
        required: false
        default: "gleif"
        description: "Source to tag the new shareholding records"
```

---

## Entity Tags Reference

| Tag | Meaning | Auto-detected from |
|-----|---------|-------------------|
| `ULTIMATE_PARENT` | Top of ownership chain | GLEIF ultimate parent relationship |
| `HOLDING_CO` | Intermediate holding company | GLEIF with subsidiaries, no operations |
| `MANCO` | Management company | GLEIF manages funds, LEI-ROC data |
| `SICAV` | Variable capital investment company | GLEIF entity category + LU/IE jurisdiction |
| `UCITS` | EU regulated fund | GLEIF + regulatory status |
| `AIF` | Alternative investment fund | GLEIF entity category |
| `SPV` | Special purpose vehicle | Name patterns ("SPV", "Holdings"), jurisdiction |
| `FUND` | Generic fund | GLEIF entity category = FUND |
| `SUBSIDIARY` | Owned by another group entity | GLEIF direct parent relationship |
| `REGULATED` | Under regulatory supervision | GLEIF registration authority present |
| `SERVICE_PROVIDER` | External service provider (IM, Custodian) | Role assignment, not ownership |
| `NEEDS_REVIEW` | Flagged for manual review | Conflicting data, low confidence match |

### Auto-Tagging Logic (GLEIF)

```rust
fn auto_tag_from_gleif(gleif_entity: &GleifEntity) -> Vec<String> {
    let mut tags = Vec::new();
    
    // Entity category
    match gleif_entity.entity_category.as_deref() {
        Some("FUND") => tags.push("FUND"),
        Some("BRANCH") => tags.push("SUBSIDIARY"),
        _ => {}
    }
    
    // Ultimate parent
    if gleif_entity.is_ultimate_parent() {
        tags.push("ULTIMATE_PARENT");
    }
    
    // Has subsidiaries but no fund category = holding company
    if gleif_entity.has_subsidiaries() && !tags.contains(&"FUND") {
        tags.push("HOLDING_CO");
    }
    
    // Manages funds = MANCO
    if gleif_entity.manages_funds() {
        tags.push("MANCO");
    }
    
    // Regulated
    if gleif_entity.registration_authority.is_some() {
        tags.push("REGULATED");
    }
    
    // Jurisdiction-based hints
    match gleif_entity.jurisdiction.as_deref() {
        Some("LU") | Some("IE") if tags.contains(&"FUND") => {
            tags.push("SICAV"); // Likely SICAV in Luxembourg/Ireland
        }
        Some("KY") | Some("JE") | Some("GG") => {
            // Offshore jurisdiction - might be SPV
            if gleif_entity.name.contains("Holdings") || gleif_entity.name.contains("SPV") {
                tags.push("SPV");
            }
        }
        _ => {}
    }
    
    tags
}
```

---

## Implementation Phases

### Phase 1: Schema & Core Verbs (Migration 055)
- [ ] Create `client_group_entity` junction table with indexes
- [ ] Extend `client_group` with discovery columns
- [ ] Create `shareholding_sources` table
- [ ] Create canonical shareholding view
- [ ] Implement count trigger
- [ ] Implement `client-group.add-entity`, `remove-entity`, `list-entities`
- [ ] Implement `client-group.tag-entity`, `untag-entity`
- [ ] Implement `client-group.start-discovery`, `complete-discovery`

### Phase 2: Review Workflow
- [ ] Implement `client-group.confirm-entity`, `reject-entity`, `list-pending`
- [ ] Add review_status transitions in domain_ops
- [ ] egui: Show pending review count badge on client group chip

### Phase 3: GLEIF Integration
- [ ] Implement `gleif.import-to-group` verb
- [ ] Modify existing GLEIF import to optionally use buffer
- [ ] Add auto-tagging logic
- [ ] Store raw GLEIF response in `shareholding_sources.raw_payload`
- [ ] Implement `gleif.refresh-group` with diff detection

### Phase 4: BODS Integration
- [ ] Implement `bods.import-ownership` → `shareholding_sources`
- [ ] Extract beneficial ownership into shareholdings
- [ ] Tag natural persons for UBO discovery

### Phase 5: Multi-Source Reconciliation
- [ ] Implement `shareholding.add-source`, `verify-source`
- [ ] Implement `shareholding.list-discrepancies`
- [ ] Implement `shareholding.promote-provisional`
- [ ] egui: Discrepancy highlighting in ownership view

### Phase 6: Agent Integration
- [ ] Wire "research X" intent to `start-discovery`
- [ ] Scoped entity search within `client_group_entity` (typed search integration)
- [ ] Discovery status in egui scope chip
- [ ] "what did we find" → `list-entities`

---

## Agent Intent Examples

### Discovery Flow
```
User: "research Aviva"
→ Stage 0: Create/find client_group "Aviva", set scope
→ Staged: (client-group.start-discovery :group-id <Aviva> :source "gleif")
→ Staged: (gleif.import-to-group :group-id <Aviva> :root-lei "..." :direction BOTH)

User: "show me what we found"
→ (client-group.list-entities :group-id <Aviva>)

User: "tag Aviva Investors as the manco"
→ (client-group.tag-entity :group-id <Aviva> :entity-id <Aviva Investors> :tags ["MANCO"])

User: "what needs review"
→ (client-group.list-pending :group-id <Aviva>)

User: "confirm all the Irish funds"
→ For each FUND+IE entity: (client-group.confirm-entity :group-id <Aviva> :entity-id <X>)

User: "we're done, build the structure"
→ (client-group.complete-discovery :group-id <Aviva> :create-cbus true :build-shareholdings true :discover-ubos true)
```

### Trust But Verify Flow (KYC Ownership Verification)
```
User: "client says Allianz SE owns 75% of Allianz Ireland"
→ First, create/find the relationship edge:
  (client-group.add-relationship 
    :group-id <Allianz> 
    :parent-entity-id <Allianz SE>
    :child-entity-id <Allianz Ireland>
    :relationship-kind "ownership")
  Returns: relationship-id = <rel-uuid>

→ Then add the allegation source:
  (client-group.add-ownership-source 
    :relationship-id <rel-uuid>
    :source "client_allegation"
    :source-type "allegation"
    :ownership-pct 75.00
    :source-document-ref "KYC-2025-001")

User: "verify that from companies house"
→ Agent fetches Companies House data
→ (client-group.add-ownership-source 
    :relationship-id <rel-uuid>
    :source "companies_house"
    :source-type "verification"
    :ownership-pct 74.50
    :verifies-source-id <allegation-source-uuid>
    :source-document-ref "CH-12345678")

User: "show ownership discrepancies"
→ (client-group.list-discrepancies :group-id <Allianz> :threshold-pct 5.0)
→ Returns: Allianz SE → Allianz Ireland - alleged 75%, verified 74.5% (spread: 0.5%)

User: "that's within tolerance, verify it"
→ (client-group.verify-ownership 
    :source-id <companies-house-source-uuid>
    :verified-by "analyst@bnymellon.com"
    :verification-notes "Within 1% threshold, accepted")

User: "what still needs verification"
→ (client-group.list-unverified :group-id <Allianz>)
```

### Multi-Source Reconciliation
```
User: "GLEIF says 60% but Clearstream says 58% for BlackRock → Fund A"
→ Relationship already exists from GLEIF discovery
→ (client-group.add-ownership-source :relationship-id <rel-uuid> :source "gleif" :ownership-pct 60.00 ...)
→ (client-group.add-ownership-source :relationship-id <rel-uuid> :source "clearstream" :ownership-pct 58.00 ...)

User: "which sources conflict?"
→ (client-group.list-discrepancies :group-id <X> :threshold-pct 1.0)
→ Returns: BlackRock → Fund A - gleif 60%, clearstream 58% (spread: 2%)

User: "trust clearstream for this one"
→ (client-group.set-canonical 
    :source-id <clearstream-source-uuid>
    :canonical-set-by "analyst@bnymellon.com"
    :canonical-notes "Clearstream is settlement source of truth for Lux funds")
```

---

## Resolved Design Decisions

### 1. BODS → Shareholding Table (UBO Taxonomy)

BODS JSON updates the **shareholding table** directly via `shareholding_sources`. BODS provides beneficial ownership statements that become source-tagged shareholding records.

```
BODS Statement → shareholding_sources (source='bods') → shareholdings → UBO discovery
```

### 2. Multi-Source Ownership: Source-Tagged Values

Both GLEIF and BODS provide ownership data. KYC verification uses additional sources (Companies House). We support **multiple values by source** for the same shareholding relationship.

**Source priority for canonical value:**
1. `verified` from any source (KYC confirmed)
2. `companies_house` (regulatory filing)
3. `bods` (beneficial ownership register)
4. `gleif` (LEI relationship data)
5. `annual_report` (public filings)
6. `client_allegation` (client-provided, needs verification)
7. `manual` (analyst entry)

### 3. Refresh Cadence

| Source | Refresh | Trigger |
|--------|---------|---------|
| GLEIF | On request only | User: "refresh group from gleif" |
| BODS | On request only | User: "refresh UBO data" |
| Companies House | On request only | KYC verification workflow |
| Client allegation | Never auto | Client provides, we store and verify |

**No automatic refresh** — all refreshes are user/workflow initiated. Staleness flagged via `discovery_status = 'stale'` after configurable period.

### 4. Multi-Group Entities: Service Providers

Entities like **BlackRock** (as Investment Manager) can belong to **multiple client groups** because they serve as IM for multiple clients' CBUs.

**Design:** `membership_type` distinguishes ownership vs service relationship:

| In Group | membership_type | Tags |
|----------|-----------------|------|
| "BlackRock" (own group) | `confirmed` | `ULTIMATE_PARENT, MANCO, REGULATED` |
| "Allianz" (as service provider) | `service_provider` | `SERVICE_PROVIDER, IM` |
| "Aviva" (as service provider) | `service_provider` | `SERVICE_PROVIDER, IM` |

### 5. Shareholding Identity

When GLEIF says A→B is 60% and BODS says 55%, this is the **same shareholding** (matched by parent+child entity pair) with **different source values**.

- `shareholdings` table: One row for the A→B relationship
- `shareholding_sources` table: Multiple rows (gleif=60%, bods=55%)
- Canonical view: Returns verified value, or highest-priority source

### 6. Historical Source Values

When refreshing, **keep old values** with `verification_status = 'superseded'` for audit trail. Never delete source records.

---

## Success Criteria

1. **Research → Buffer**: GLEIF crawl populates `client_group_entity`, not raw entity creation
2. **Tagged Collection**: All discovered entities have classification tags
3. **Multi-Source Ownership**: Multiple sources for same relationship stored in `client_group_entity_sources`
4. **Trust But Verify**: Client allegations can be verified against authoritative sources with lineage tracking
5. **Discrepancy Detection**: Conflicting source values automatically flagged for review
6. **Review Workflow**: Pending entities can be confirmed/rejected before structuring
7. **Clean Rollback**: Failed discovery can set `membership_type = 'historical'` without orphans
8. **Agent Integration**: "research X" creates group, sets scope, starts discovery
9. **Canonical Value**: Views provide best-available ownership using source authority hierarchy
10. **Incremental Refresh**: Re-crawl diffs against existing collection, flags changes
11. **Typed Search**: Within scope, entity references resolve against `client_group_entity`
12. **Audit Trail**: All source data preserved with provenance (who fetched, when, from what document)

---

## Files to Modify

| File | Changes |
|------|---------|
| `migrations/055_client_group_research.sql` | Schema + views + triggers |
| `rust/config/verbs/client-group.yaml` | Discovery + collection + source verbs |
| `rust/config/verbs/gleif.yaml` | Group-aware import verbs |
| `rust/config/verbs/shareholding.yaml` | Multi-source verbs |
| `rust/src/domain_ops/client_group_ops.rs` | Verb handlers + source management |
| `rust/src/domain_ops/gleif_ops.rs` | Buffer-aware import |
| `rust/src/domain_ops/shareholding_ops.rs` | Source management |
| `rust/src/mcp/scope_resolution.rs` | Search within `client_group_entity` |
| `rust/src/api/agent_service.rs` | Research intent handling |

---

## Resolved Questions (ChatGPT Peer Review)

| Question | Decision | Rationale |
|----------|----------|-----------|
| **1. Promotion to shareholdings** | Don't auto-promote on `complete-discovery`. Require explicit `promote-provisional` with dry-run preview. | Keeps user in control; avoids premature structuring |
| **2. Tag inheritance across groups** | No. Tags are group-contextual. | If global tags needed, create separate `entity_global_tags` table |
| **3. Discrepancy threshold** | 5% default, configurable per `relationship_kind` and entity category | Fund structures may need tighter thresholds |
| **4. Stale detection** | Configurable. Default 90 days for corporate, 30-60 for funds. | Staleness is UI prompt, not auto-refresh |
| **5. Verification before promotion** | Allow promotion but mark as `unverified`. Block downstream UBO finalization until verified. | Keeps flow moving without pretending it's proven |
| **6. Confidence per jurisdiction** | Yes, make table-driven and configurable | Different jurisdictions have different authoritative sources |
| **7. Clearstream vs Companies House** | Jurisdiction + relationship_kind aware | Clearstream for settlement/economic views (Lux funds); CH for UK filings |
| **8. CBU auto-creation tags** | `FUND` + optionally `SICAV`/`UCITS`/`AIF` | Avoid auto-creating for `MANCO`/`SPV` unless explicitly wanted |
