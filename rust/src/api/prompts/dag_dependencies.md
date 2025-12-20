# Multi-Intent Dependencies and DAG Execution

## How Multi-Intent Requests Work

When you generate multiple intents that depend on each other, the system automatically determines execution order using a Directed Acyclic Graph (DAG). You don't need to worry about ordering - just express the dependencies correctly.

### The Contract

**YOUR JOB (LLM):**
1. Generate intents with correct `@result_N` references (1-indexed)
2. Output intents in logical reading order (for human review)
3. Use `lookups` for existing entities, `refs` with `@result_N` for newly created ones
4. DO NOT try to order by execution - the DAG handles this

**SYSTEM'S JOB (DAG):**
1. Parse `@result_N` references to build dependency graph
2. Topological sort for execution order
3. Parallelize independent operations within each stage
4. Reject circular dependencies with clear error

---

## Reference Types

### `lookups` - For Existing Entities

Used when referencing entities that already exist in the database.
- Resolved by EntityGateway BEFORE any execution
- NOT dependency edges - don't affect execution order
- May trigger disambiguation if multiple matches

```json
{
  "verb": "cbu.assign-role",
  "params": {"role": "DIRECTOR"},
  "refs": {"cbu-id": "@cbu"},
  "lookups": {"entity-id": {"search_text": "John Smith", "entity_type": "person"}}
}
```
Here, "John Smith" is looked up in the database before execution.

### `refs` with `@result_N` - For Newly Created Entities

Used when referencing entities created by earlier intents in the same request.
- Creates a dependency edge in the execution DAG
- `@result_1` = output of intent 1, `@result_2` = output of intent 2, etc.
- System waits for referenced intent to complete before executing

```json
{
  "verb": "cbu.add-product",
  "params": {"product": "CUSTODY"},
  "refs": {"cbu-id": "@result_1"},
  "lookups": null
}
```
Here, `@result_1` means "use the entity created by intent 1".

### `refs` with `@cbu` - Session Context

Used when referencing the active CBU from session context.
- Already resolved - not a lookup or dependency
- Available when user has an active working context

```json
{
  "verb": "cbu.add-product",
  "params": {"product": "CUSTODY"},
  "refs": {"cbu-id": "@cbu"},
  "lookups": null
}
```

---

## Dependency Patterns

### Pattern 1: Fan-Out (One creates, many consume)

**User**: "Create Apex fund in Luxembourg, add John as director, set up custody and fund accounting"

```json
{
  "intents": [
    {
      "verb": "cbu.ensure",
      "params": {"name": "Apex", "jurisdiction": "LU", "client-type": "fund"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "cbu.assign-role",
      "params": {"role": "DIRECTOR"},
      "refs": {"cbu-id": "@result_1"},
      "lookups": {"entity-id": {"search_text": "John", "entity_type": "person"}}
    },
    {
      "verb": "cbu.add-product",
      "params": {"product": "CUSTODY"},
      "refs": {"cbu-id": "@result_1"},
      "lookups": null
    },
    {
      "verb": "cbu.add-product",
      "params": {"product": "FUND_ACCOUNTING"},
      "refs": {"cbu-id": "@result_1"},
      "lookups": null
    }
  ],
  "explanation": "Creating fund, then adding director and two products. Intents 2-4 all depend on intent 1.",
  "confidence": 0.92
}
```

**Execution DAG:**
```
         ┌─────────────┐
         │  Intent 1   │  Stage 1: Create fund
         │ cbu.ensure  │
         └──────┬──────┘
                │
       ┌────────┼────────┐
       ▼        ▼        ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ Intent 2 │ │ Intent 3 │ │ Intent 4 │  Stage 2: All parallel
│add-role  │ │add-prod  │ │add-prod  │
└──────────┘ └──────────┘ └──────────┘
```

### Pattern 2: Chain (Sequential dependencies)

**User**: "Create HoldCo in Jersey, then create a fund under it, then add custody to the fund"

```json
{
  "intents": [
    {
      "verb": "entity.create-limited-company",
      "params": {"name": "HoldCo Ltd", "jurisdiction": "JE"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "cbu.ensure",
      "params": {"name": "HoldCo Fund I", "jurisdiction": "JE", "client-type": "fund"},
      "refs": {"parent-entity-id": "@result_1"},
      "lookups": null
    },
    {
      "verb": "cbu.add-product",
      "params": {"product": "CUSTODY"},
      "refs": {"cbu-id": "@result_2"},
      "lookups": null
    }
  ],
  "explanation": "Sequential chain: company → fund (under company) → product (on fund).",
  "confidence": 0.90
}
```

**Execution DAG:**
```
┌─────────────┐
│  Intent 1   │  Stage 1
│create-company│
└──────┬──────┘
       ▼
┌─────────────┐
│  Intent 2   │  Stage 2
│ cbu.ensure  │
└──────┬──────┘
       ▼
┌─────────────┐
│  Intent 3   │  Stage 3
│ add-product │
└─────────────┘
```

### Pattern 3: Diamond (Converging dependencies)

**User**: "Create a holding company and a management company, then create a fund owned by both"

```json
{
  "intents": [
    {
      "verb": "entity.create-limited-company",
      "params": {"name": "HoldCo Ltd", "jurisdiction": "LU"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "entity.create-limited-company",
      "params": {"name": "ManCo S.à r.l.", "jurisdiction": "LU"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "cbu.ensure",
      "params": {"name": "Alpha Fund", "jurisdiction": "LU", "client-type": "fund"},
      "refs": {"parent-entity-id": "@result_1", "manco-id": "@result_2"},
      "lookups": null
    }
  ],
  "explanation": "Two independent companies created first, then fund references both.",
  "confidence": 0.88
}
```

**Execution DAG:**
```
┌─────────────┐     ┌─────────────┐
│  Intent 1   │     │  Intent 2   │  Stage 1: Both parallel
│  HoldCo     │     │  ManCo      │
└──────┬──────┘     └──────┬──────┘
       │                   │
       └─────────┬─────────┘
                 ▼
          ┌─────────────┐
          │  Intent 3   │  Stage 2: Needs both
          │  Fund       │
          └─────────────┘
```

### Pattern 4: Mixed (Lookups + References)

**User**: "Create a new fund under BlackRock ManCo and add existing compliance officer Sarah Chen"

```json
{
  "intents": [
    {
      "verb": "cbu.ensure",
      "params": {"name": "BlackRock Alpha", "jurisdiction": "IE", "client-type": "fund"},
      "refs": {},
      "lookups": {"manco-id": {"search_text": "BlackRock ManCo", "entity_type": "entity"}}
    },
    {
      "verb": "cbu.assign-role",
      "params": {"role": "COMPLIANCE_OFFICER"},
      "refs": {"cbu-id": "@result_1"},
      "lookups": {"entity-id": {"search_text": "Sarah Chen", "entity_type": "person"}}
    }
  ],
  "explanation": "Fund creation looks up existing ManCo. Role assignment references new fund and looks up existing person.",
  "confidence": 0.88
}
```

**Key insight**: `lookups` are resolved before execution starts. `@result_1` creates execution dependency.

---

## What NOT To Do

### ❌ WRONG: Circular Dependencies

```json
{
  "intents": [
    {
      "verb": "cbu.ensure",
      "params": {"name": "Fund A"},
      "refs": {"related-to": "@result_2"},
      "lookups": null
    },
    {
      "verb": "cbu.ensure",
      "params": {"name": "Fund B"},
      "refs": {"related-to": "@result_1"},
      "lookups": null
    }
  ]
}
```

**Why it fails**: Intent 1 needs Intent 2, but Intent 2 needs Intent 1. Impossible to order.

**Error**: `CircularDependencyError: @result_1 → @result_2 → @result_1`

**How to fix**: Break the cycle. Create both independently, then link them with a third operation.

### ❌ WRONG: Forward References

```json
{
  "intents": [
    {
      "verb": "cbu.add-product",
      "params": {"product": "CUSTODY"},
      "refs": {"cbu-id": "@result_2"},
      "lookups": null
    },
    {
      "verb": "cbu.ensure",
      "params": {"name": "Apex"},
      "refs": {},
      "lookups": null
    }
  ]
}
```

**Why it's confusing**: Intent 1 references `@result_2`, but that's actually fine - the DAG will reorder. However, it's harder for humans to review.

**Better approach**: Output intents in logical order (create first, use later). The DAG handles execution order, but human review is easier with logical order.

### ❌ WRONG: Using @result_N for Existing Entities

```json
{
  "intents": [
    {
      "verb": "cbu.assign-role",
      "params": {"role": "DIRECTOR"},
      "refs": {"cbu-id": "@result_1", "entity-id": "@result_2"},
      "lookups": null
    }
  ]
}
```

**Why it fails**: `@result_1` and `@result_2` don't exist - there are no prior intents.

**Correct approach**: Use `lookups` for existing entities:

```json
{
  "intents": [
    {
      "verb": "cbu.assign-role",
      "params": {"role": "DIRECTOR"},
      "refs": {},
      "lookups": {
        "cbu-id": {"search_text": "Apex Fund", "entity_type": "cbu"},
        "entity-id": {"search_text": "John Smith", "entity_type": "person"}
      }
    }
  ]
}
```

---

## Decision Guide: Lookup vs Reference

| Situation | Use | Example |
|-----------|-----|---------|
| Entity exists in database | `lookups` | "Add John Smith..." → lookup John Smith |
| Entity created in earlier intent | `refs` with `@result_N` | "Create fund... add custody to it" → @result_1 |
| Entity in session context | `refs` with `@cbu` | "Add product" (active CBU) → @cbu |
| Entity might or might not exist | `lookups` | System will find or fail gracefully |
| Entity definitely being created now | `refs` with `@result_N` | Current multi-intent request |

---

## Complex Example: Full Onboarding Scenario

**User**: "Create a Luxembourg fund called Apex Capital, create the management company Apex ManCo, link them together, add John Smith as director of both, and set up custody and fund accounting for the fund"

```json
{
  "intents": [
    {
      "verb": "entity.create-limited-company",
      "params": {"name": "Apex ManCo S.à r.l.", "jurisdiction": "LU"},
      "refs": {},
      "lookups": null
    },
    {
      "verb": "cbu.ensure",
      "params": {"name": "Apex Capital Fund", "jurisdiction": "LU", "client-type": "fund"},
      "refs": {"manco-id": "@result_1"},
      "lookups": null
    },
    {
      "verb": "cbu.assign-role",
      "params": {"role": "DIRECTOR"},
      "refs": {"cbu-id": "@result_1"},
      "lookups": {"entity-id": {"search_text": "John Smith", "entity_type": "person"}}
    },
    {
      "verb": "cbu.assign-role",
      "params": {"role": "DIRECTOR"},
      "refs": {"cbu-id": "@result_2"},
      "lookups": {"entity-id": {"search_text": "John Smith", "entity_type": "person"}}
    },
    {
      "verb": "cbu.add-product",
      "params": {"product": "CUSTODY"},
      "refs": {"cbu-id": "@result_2"},
      "lookups": null
    },
    {
      "verb": "cbu.add-product",
      "params": {"product": "FUND_ACCOUNTING"},
      "refs": {"cbu-id": "@result_2"},
      "lookups": null
    }
  ],
  "explanation": "Creating ManCo and Fund with relationship, adding director to both, adding products to fund. John Smith lookup will be resolved once and reused.",
  "confidence": 0.85
}
```

**Execution DAG:**
```
         ┌─────────────┐
         │  Intent 1   │  Stage 1: Create ManCo
         │  ManCo      │
         └──────┬──────┘
                │
       ┌────────┴────────┐
       ▼                 ▼
┌─────────────┐   ┌─────────────┐
│  Intent 2   │   │  Intent 3   │  Stage 2: Fund (needs ManCo) + Director on ManCo
│  Fund       │   │  Dir→ManCo  │
└──────┬──────┘   └─────────────┘
       │
┌──────┴──────────────────┐
▼           ▼             ▼
┌──────┐ ┌──────┐ ┌──────────┐
│ I-4  │ │ I-5  │ │   I-6    │  Stage 3: All depend on Fund
│Dir→F │ │Cust  │ │Fund Acct │
└──────┘ └──────┘ └──────────┘
```

---

## Summary

1. **@result_N** = "I need the output of intent N" (creates DAG edge)
2. **lookups** = "Find this existing entity" (resolved before DAG execution)
3. **Output order** = logical/readable (for humans)
4. **Execution order** = determined by DAG (automatic)
5. **No circular dependencies** - if A needs B, B cannot need A
6. **Parallel when possible** - independent operations run together
