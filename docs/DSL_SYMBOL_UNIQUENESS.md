# DSL Symbol Uniqueness: The LEI Alias Collision Problem

## Problem Summary

When generating DSL from GLEIF data, we encountered a validation error:

```
error[D003]: @lei_oj2tiqsvqnd4izyyk658 is already defined earlier in this program.
```

This occurred because the same entity (AllianzGI, LEI `OJ2TIQSVQND4IZYYK658`) appeared in multiple data sources and we tried to define the same `@lei_xxx` symbol binding twice.

## Two Levels of Uniqueness Checking

### Level 1: DSL Symbol Binding (D003 - Compile Time)

**Location:** `rust/src/dsl_v2/csg_linter.rs` lines 625-644

```rust
// Check for duplicate binding
if pending_context.contains(binding_name) {
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: DiagnosticCode::DataflowDuplicateBinding,  // D003
        message: format!(
            "@{} is already defined earlier in this program.",
            binding_name
        ),
        ...
    });
}
```

**When it fires:** Two statements in the same DSL program try to bind the same symbol name.

**Example that fails:**
```clojure
(entity.ensure-limited-company :name "AllianzGI" :lei "ABC" :as @allianzgi)
(entity.ensure-limited-company :name "AllianzGI" :lei "ABC" :as @allianzgi)  ;; D003!
```

### Level 2: Database Unique Constraint (Runtime)

**Location:** Database schema / `generic_executor.rs` entity creation

If the DSL passes validation but tries to insert duplicate LEIs at runtime:

```sql
-- entity_limited_companies has UNIQUE constraint on lei
ERROR: duplicate key value violates unique constraint "entity_limited_companies_lei_key"
```

**When it fires:** Two DSL commands create entities with the same LEI, even if they have different symbol names.

**Example that fails at runtime:**
```clojure
(entity.ensure-limited-company :name "AllianzGI" :lei "ABC" :as @foo)
(entity.ensure-limited-company :name "AllianzGI" :lei "ABC" :as @bar)  
;; Passes D003 (different symbols), but fails DB constraint!
```

## Root Cause

### 1. Initial Mistake: Truncated Aliases

The original implementation used only the first 8 characters of the LEI for aliases:

```rust
fn lei_to_alias(lei: &str) -> String {
    format!("@lei_{}", &lei[..8].to_lowercase())
}
```

This caused collisions when multiple LEIs shared the same 8-character prefix:
- `529900LS...` and `529900LX...` both became `@lei_529900ls`

**Fix:** Use the full 20-character LEI:

```rust
fn lei_to_alias(lei: &str) -> String {
    format!("@lei_{}", lei.to_lowercase())
}
```

### 2. Multi-Source Data Overlap

Even with full LEIs, we had a second collision. AllianzGI appeared in two data sources:

| Source | File | Purpose |
|--------|------|---------|
| Level 2 Data | `allianz_level2_data.json` | Parent entity hierarchy |
| Corporate Tree | `allianz_se_corporate_tree.json` | Direct subsidiaries of Allianz SE |

Both sources tried to create an entity with the same LEI, resulting in duplicate symbol definitions.

## DSL Symbol Semantics

In the DSL, `:as @symbol` creates a binding that can be referenced later:

```clojure
;; Define entity and bind to @fund
(entity.ensure-limited-company :name "Apex Fund" :lei "ABC123" :as @fund)

;; Reference the binding
(cbu.assign-role :entity-id @fund :role "DIRECTOR")
```

**Key rule:** Each symbol can only be defined ONCE per program. The DSL is parsed and validated as a single unit, so duplicate definitions are caught at validation time.

## Why Idempotent Verbs Don't Help Here

You might think: "The `entity.ensure-*` verbs are idempotent (upsert), so duplicates should be fine."

**The database operation is idempotent, but the DSL syntax is not.**

The error occurs at the **parsing/validation stage**, before any database operations:

```
Source → Parse → Validate (ERROR HERE) → Compile → Execute → Database
```

The validator sees two statements trying to bind the same symbol name, which is a syntax error regardless of what the database would do.

## Solution: Track and Skip Duplicates

The fix is to track which LEIs have been defined and skip duplicates in later phases:

```rust
// Track already-defined LEIs
let mut defined_leis: HashSet<String> = HashSet::new();

// Phase 1: Parent entities
if let Some(allianz_se) = level2.entities.get(allianz_se_lei) {
    dsl_parts.push(generate_parent_entity_dsl(allianz_se));
    defined_leis.insert(allianz_se_lei.to_string());  // Track it
}

// Phase 4: Corporate tree children
for child in children {
    if defined_leis.contains(&child.lei) {
        continue;  // Skip - already defined
    }
    dsl_parts.push(generate_corp_child_dsl(child, allianz_se_lei));
}
```

## Alternative Approaches

### 1. Generate Without Bindings

If you don't need to reference entities later, omit the `:as @symbol`:

```clojure
;; No binding - can appear multiple times (idempotent upsert)
(entity.ensure-limited-company :name "Apex Fund" :lei "ABC123")
(entity.ensure-limited-company :name "Apex Fund" :lei "ABC123")  ;; OK!
```

### 2. Use Different Symbol Names

If you need bindings but have overlapping data, use phase-specific prefixes:

```clojure
;; Phase 1
(entity.ensure-limited-company :name "AllianzGI" :as @phase1_allianzgi)

;; Phase 4 (if needed)
(entity.ensure-limited-company :name "AllianzGI" :as @phase4_allianzgi)
```

But this is usually unnecessary - just skip the duplicate.

### 3. Pre-Deduplicate Data

Clean the source data before generating DSL:

```rust
let all_entities: HashMap<String, Entity> = HashMap::new();
// Merge all sources, LEI as key (natural dedup)
for entity in level2_entities { all_entities.insert(entity.lei.clone(), entity); }
for entity in corp_tree_children { all_entities.entry(entity.lei.clone()).or_insert(entity); }
```

## Validation Pipeline Position

```
Source Text
    │
    ▼
┌─────────┐
│ Parser  │  Creates AST with symbol bindings
└────┬────┘
     │
     ▼
┌──────────────┐
│ CSG Linter   │  Pass 6: Check for duplicate bindings (D003)
└──────┬───────┘
       │
       ▼
┌──────────────────┐
│ Semantic Validator│  Additional context-aware checks
└──────────┬───────┘
           │
           ▼
┌──────────────┐
│ Compiler     │  Build execution plan
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ Executor     │  Run against database
└──────┬───────┘     (DB unique constraints apply here)
       │
       ▼
   Database
```

## Lessons Learned

1. **Symbol names must be unique** - DSL validation enforces this before execution
2. **LEIs are globally unique** - Use the full 20-character LEI, not truncated versions
3. **Multi-source data has overlaps** - Track what's been defined and skip duplicates
4. **Idempotency is database-level** - DSL syntax validation happens first
5. **Two validation layers** - D003 at compile time, DB constraints at runtime

## Related Files

- `rust/xtask/src/gleif_load.rs` - DSL generator with deduplication logic
- `rust/src/dsl_v2/csg_linter.rs` - CSG validation including symbol uniqueness check (line 625)
- `rust/src/dsl_v2/validation.rs` - DiagnosticCode definitions (D003 = DataflowDuplicateBinding)
- `rust/src/dsl_v2/parser.rs` - DSL parser that creates symbol bindings
