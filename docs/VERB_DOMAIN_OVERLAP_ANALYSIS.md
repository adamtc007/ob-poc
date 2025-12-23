# Verb/Domain Overlap Analysis

## Current State: 53 Verb Files, ~40 Domains

### Domain Inventory

| Category | Domains | Verb Count (approx) |
|----------|---------|---------------------|
| **Core CBU** | cbu, onboarding | ~30 |
| **KYC Workflow** | kyc-case, entity-workstream, case-event, case-screening, red-flag, doc-request | ~40 |
| **Entity/UBO** | entity, ubo, control, delegation | ~35 |
| **Taxonomy: Product** | product, service, service-resource | ~15 |
| **Taxonomy: Instrument** | lifecycle, instrument-class | ~20 |
| **Trading Matrix** | cbu-custody, trading-profile, investment-manager, pricing-config, cash-sweep, matrix-overlay, product-subscription | ~50 |
| **ISDA/SLA** | isda, sla | ~25 |
| **Reference Data** | role, jurisdiction, currency, client-type, risk-rating, case-type, screening-type, settlement-type, ssi-type, market, subcustodian, security-type | ~40 |
| **Fund Registry** | fund, share-class, holding, movement | ~35 |
| **Verification** | verify, screening, observation, allegation, discrepancy | ~30 |
| **Utility** | graph, batch, template, document, delivery, refdata | ~20 |

**Total: ~340 verbs across ~40 domains**

---

## RED FLAG #1: Reference Data Duplication

**9 domains with IDENTICAL verb patterns:**

```yaml
# These all have the SAME verbs:
role:         ensure, read, list, delete
jurisdiction: ensure, read, list, delete
currency:     ensure, read, list, deactivate
client-type:  ensure, read, list, deactivate
risk-rating:  ensure, read, list, deactivate
case-type:    ensure, read, list, deactivate
screening-type: ensure, read, list, deactivate
settlement-type: ensure, read, list, deactivate
ssi-type:     ensure, read, list, deactivate
```

**Should be ONE domain:**
```yaml
refdata:
  ensure:    { table-param: type }
  read:      { table-param: type }
  list:      { table-param: type }
  deactivate: { table-param: type }

# Usage:
refdata.ensure type:jurisdiction code:US name:"United States"
refdata.ensure type:currency code:USD name:"US Dollar"
refdata.list type:risk-rating
```

**Savings:** 9 domains → 1 domain, ~36 verbs → 4 verbs

---

## RED FLAG #2: Taxonomy Duplication

**Two parallel hierarchies with similar verbs:**

| Product Taxonomy | Instrument Taxonomy | Same Pattern? |
|------------------|---------------------|---------------|
| `product.read` | `lifecycle.read` | ✓ |
| `product.list` | `lifecycle.list` | ✓ |
| `service.list-by-product` | `lifecycle.list-by-instrument` | ✓ |
| `service-resource.list-by-service` | `lifecycle.list-resources-for-lifecycle` | ✓ |
| `service-resource.provision` | `lifecycle.provision` | ✓ |
| `service-resource.activate` | `lifecycle.activate` | ✓ |
| `service-resource.suspend` | `lifecycle.suspend` | ✓ |
| — | `lifecycle.analyze-gaps` | (should exist for both) |
| — | `lifecycle.discover` | (should exist for both) |

**Should be ONE generic domain:**
```yaml
taxonomy:
  read:           { domain-param: domain }  # product, lifecycle
  list:           { domain-param: domain }
  list-ops:       { domain-param: domain }  # services for product, lifecycles for instrument
  list-resources: { domain-param: domain }
  provision:      { domain-param: domain }
  activate:       { domain-param: domain }
  suspend:        { domain-param: domain }
  analyze-gaps:   { domain-param: domain }
  discover:       { domain-param: domain }

# Usage:
taxonomy.discover domain:product type:GLOBAL_CUSTODY
taxonomy.discover domain:lifecycle type:EQUITY
taxonomy.analyze-gaps domain:lifecycle cbu-id:@fund
```

**Savings:** 3 domains → 1 domain, ~35 verbs → ~10 verbs

---

## RED FLAG #3: Status Transition Duplication

**Pattern appears across many domains:**

```
activate, suspend, deactivate, terminate, reactivate, decommission
```

Found in:
- `service-resource`: activate, suspend, decommission
- `lifecycle`: activate, suspend, decommission
- `investment-manager`: suspend, terminate
- `cash-sweep`: suspend, reactivate
- `product-subscription`: suspend, reactivate
- `matrix-overlay`: suspend, activate
- `sla`: suspend-commitment

**Could be generic:**
```yaml
status:
  transition:
    args:
      - entity-type: string   # service-resource, lifecycle, investment-manager
      - entity-id: uuid
      - new-status: string    # ACTIVE, SUSPENDED, TERMINATED
      
# Or even simpler - convention over configuration:
# Any entity with a `status` column gets these verbs automatically
```

---

## RED FLAG #4: Binding/Linking Duplication

**Pattern: "Link X to Y"**

```yaml
# SLA domain - 5 separate bind verbs
sla.bind-to-profile
sla.bind-to-service
sla.bind-to-resource
sla.bind-to-isda
sla.bind-to-csa

# Other domains
pricing-config.link-resource
cash-sweep.link-resource
investment-manager.link-connectivity
```

**Could be generic:**
```yaml
link:
  create:
    args:
      - source-type: string
      - source-id: uuid
      - target-type: string
      - target-id: uuid
      - link-type: string    # optional qualifier

# Usage:
link.create source-type:sla source-id:@sla target-type:profile target-id:@profile
link.create source-type:pricing-config source-id:@pc target-type:resource target-id:@bloomberg
```

**Savings:** ~10 specific verbs → 1 generic verb

---

## RED FLAG #5: CRUD Patterns

**Every domain reinvents CRUD:**

```
create, read, update, delete, list, ensure
```

These could be automatic for any registered entity type.

**Convention approach:**
```yaml
# Register entity type once:
entities:
  jurisdiction:
    table: jurisdictions
    pk: jurisdiction_id
    crud: [create, read, update, delete, list, ensure]

# Verbs auto-generated:
jurisdiction.create
jurisdiction.read
jurisdiction.list
# etc.
```

---

## Proposed Consolidation

### Tier 1: Generic Domains (replace many specific domains)

| Generic Domain | Replaces | Pattern |
|----------------|----------|---------|
| `refdata` | 9 reference domains | `refdata.{verb} type:{type}` |
| `taxonomy` | product, service, service-resource, lifecycle | `taxonomy.{verb} domain:{domain}` |
| `link` | All bind/link verbs | `link.{verb} source:{type} target:{type}` |
| `status` | All status transitions | `status.transition entity:{type} to:{status}` |

### Tier 2: Domain-Specific (keep, they have unique logic)

| Domain | Unique Verbs | Rationale |
|--------|--------------|-----------|
| `cbu` | create, assign-role, check-invariants | Core entity with special logic |
| `kyc-case` | escalate, close, reopen | Workflow state machine |
| `entity-workstream` | block, complete, set-ubo | KYC workflow |
| `ubo` | calculate, register-ubo, verify-ubo | Complex UBO logic |
| `isda` | add-coverage, add-csa | ISDA-specific structure |
| `trading-profile` | materialize, diff, validate | Profile-specific |
| `verify` | detect-patterns, challenge | Verification logic |
| `graph` | path, ancestors, descendants | Graph traversal |

### Tier 3: Template Patterns (not verbs)

Templates in `templates/` are not verbs - they're composite patterns. Keep separate.

---

## Before/After Verb Count

| Category | Before | After | Reduction |
|----------|--------|-------|-----------|
| Reference Data | ~36 | ~4 | 89% |
| Taxonomy (Product/Lifecycle) | ~35 | ~10 | 71% |
| Status Transitions | ~15 | ~2 | 87% |
| Binding/Linking | ~10 | ~2 | 80% |
| **Total Reduction** | **~96** | **~18** | **81%** |

**Overall:** ~340 verbs → ~260 verbs (24% reduction)

But more importantly: **fewer concepts to understand, fewer handlers to maintain**.

---

## Implementation Approach

### Step 1: Don't Break Existing

Keep old verbs working, add deprecation warnings:
```rust
// In verb handler
if domain == "jurisdiction" && verb == "ensure" {
    warn!("Deprecated: use refdata.ensure type:jurisdiction instead");
    // Delegate to generic handler
    return refdata_ensure(args.with("type", "jurisdiction"));
}
```

### Step 2: Create Generic Domains

1. `refdata` domain with type parameter
2. `taxonomy` domain with domain parameter  
3. `link` domain for all bindings
4. `status` domain for state transitions

### Step 3: Migrate Gradually

- New code uses generic domains
- Old verb files kept for compatibility
- Remove old domains after migration complete

### Step 4: Update Agent Prompts

Agent learns generic patterns:
```
"To manage reference data, use refdata.{verb} type:{type}"
"To work with taxonomies (products or lifecycles), use taxonomy.{verb} domain:{domain}"
```

---

## Decision Required

**Option A: Keep Current (40 domains, 340 verbs)**
- Pros: No refactoring risk
- Cons: Growing complexity, duplicate handlers

**Option B: Consolidate to Generic (fewer domains, fewer verbs)**
- Pros: Less code, one pattern to learn
- Cons: Refactoring effort, migration period

**Option C: Hybrid (generic for new, keep old)**
- Pros: No breaking changes
- Cons: Two ways to do things (temporary)

**Recommendation:** Option C for now, migrate to B over time.
