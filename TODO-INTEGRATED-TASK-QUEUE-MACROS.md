# TODO: Integrated Implementation — Workflow Task Queue + Proc Macros

> **Status:** Peer-reviewed (ChatGPT) — Round 3, implementation-ready
> **Date:** 2026-01-24  
> **Review:** 3 rounds with ChatGPT
> **Depends on:** TODO-WORKFLOW-TASK-QUEUE.md (5 review rounds completed)
> **Target:** Claude Code automated implementation

---

## Executive Summary

This TODO integrates two complementary designs:
1. **Workflow Task Queue** — async return path for external systems, document requirements, bundle payloads
2. **Proc Macros** — auto-registration for custom ops + ID newtypes

**Critical context:** Custom ops in ob-poc are **unit structs implementing `CustomOperation` trait**, NOT standalone functions. They're currently registered via a massive manual list in `CustomOperationRegistry::new()`. The macro system eliminates this manual wiring.

The task queue document ops (`DocumentSolicitOp`, `DocumentVerifyOp`, `DocumentRejectOp`) will be among the first ops using the new macro-driven registry.

---

## Recommended PR Split

Per ChatGPT review, implement in **two PR-sized chunks** to avoid "everything broken at once":

| PR | Phases | Content |
|----|--------|---------|
| **PR 1** | 0–3 + tests | Macros + auto-registry + IdType migration |
| **PR 2** | 4–8 | Task queue DB + ops + listener + endpoints |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  COMPILE-TIME LAYER (proc macros)                                           │
│                                                                             │
│  #[derive(IdType)]        #[register_custom_op]                             │
│       │                           │                                         │
│       ▼                           ▼                                         │
│  RequirementId              inventory::submit!                              │
│  VersionId                  CustomOpFactory                                 │
│  TaskId                     auto-registration                               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  RUNTIME LAYER                                                              │
│                                                                             │
│  CustomOperationRegistry ──► inventory::iter() ──► Arc<dyn CustomOperation> │
│       │                                                                     │
│       ▼                                                                     │
│  DSL Executor ──► lookup(domain, verb) ──► op.execute(...)                 │
│       │                                                                     │
│       ▼                                                                     │
│  workflow_pending_tasks, document_requirements, document_versions           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  EXTERNAL INTEGRATION                                                       │
│                                                                             │
│  POST /api/workflow/task-complete  (bundle payload)                        │
│  POST /api/documents/{id}/versions (version upload)                        │
│  Queue Listener ──► try_advance()                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 0: Macro Crate Setup

**Goal:** Create `ob-poc-macros` proc-macro crate.

### 0.1 Create Crate

```bash
# Directory structure
rust/crates/ob-poc-macros/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports
│   ├── register_op.rs      # #[register_custom_op]
│   └── id_type.rs          # #[derive(IdType)]
```

**Cargo.toml:**
```toml
[package]
name = "ob-poc-macros"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full", "parsing", "extra-traits"] }
quote = "1"
proc-macro2 = "1"
proc-macro-error = "1"

[dev-dependencies]
trybuild = "1"
```

### 0.2 Wire into Workspace

> **P0 FIX (Round 2):** Correct path is `rust/Cargo.toml`, not `rust/src/Cargo.toml`.

**Option A — If using workspace (recommended):**

**rust/Cargo.toml (workspace root + main crate):**
```toml
[workspace]
members = [
    "crates/ob-poc-macros",
]

[workspace.dependencies]
ob-poc-macros = { path = "crates/ob-poc-macros" }
inventory = "0.3"

[package]
name = "ob-poc"
# ... existing package config

[dependencies]
ob-poc-macros = { workspace = true }
inventory = { workspace = true }
```

**Option B — If NOT using workspace (simpler):**

**rust/Cargo.toml (main crate):**
```toml
[dependencies]
ob-poc-macros = { path = "crates/ob-poc-macros" }
inventory = "0.3"
```

> **Note:** Check if `rust/Cargo.toml` already has a `[workspace]` section. If yes, use Option A. If no, use Option B.

### Acceptance Criteria
- [ ] `cargo build -p ob-poc-macros` succeeds
- [ ] Main crate can `use ob_poc_macros::*`
- [ ] `inventory` is a direct dependency of the consuming crate

---

## Phase 1: Auto-Registry Infrastructure

**Goal:** Add `inventory`-based auto-registration to `CustomOperationRegistry`.

### 1.1 Define Factory Type

**File: `rust/src/domain_ops/mod.rs`** (or new `auto_registry.rs`)

```rust
use std::sync::Arc;
use inventory;

/// Factory for auto-registration of custom ops
pub struct CustomOpFactory {
    pub create: fn() -> Arc<dyn CustomOperation>,
}

// Tell inventory to collect these
inventory::collect!(CustomOpFactory);
```

### 1.2 Update Registry Construction

> **P0 FIX (Round 2):** Match actual field name (`operations` not `ops`) and preserve existing `list()` signature.

**File: `rust/src/domain_ops/mod.rs`** — modify `CustomOperationRegistry`

First, check the actual field name and `list()` return type in the current codebase. Then:

```rust
impl CustomOperationRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            operations: HashMap::new(),  // Use actual field name from codebase
        };
        
        // Phase 1: Auto-register from inventory
        for factory in inventory::iter::<CustomOpFactory> {
            let op = (factory.create)();
            registry.register_internal(op);  // Use shared registration logic
        }
        
        // Phase 2: (TEMPORARY) Manual registrations for ops not yet migrated
        // Remove these as ops are annotated with #[register_custom_op]
        // registry.register(Arc::new(SomeOldOp));
        
        registry
    }
    
    /// Internal registration with duplicate detection
    /// Used by both inventory auto-registration and manual registration
    fn register_internal(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());
        
        if self.operations.contains_key(&key) {
            panic!(
                "Duplicate custom op registration: {}.{} — this is a bug",
                key.0, key.1
            );
        }
        self.operations.insert(key, op);
    }
    
    /// List all ops (deterministic order — always sorted)
    /// Preserves existing return type: Vec<(&str, &str, &str)> with rationale
    pub fn list(&self) -> Vec<(&str, &str, &str)> {
        let mut entries: Vec<_> = self.operations.iter()
            .map(|((d, v), op)| (d.as_str(), v.as_str(), op.rationale()))
            .collect();
        entries.sort_by_key(|(d, v, _)| (*d, *v));  // Sort by (domain, verb)
        entries
    }
}
```

> **IMPORTANT:** Before implementing, check the actual `CustomOperationRegistry` in the codebase:
> - What is the HashMap field name? (`ops`? `operations`?)
> - What does `list()` return? (pairs? triples with rationale?)
> - Does `CustomOperation` have a `rationale()` method?
> Adjust the code above to match reality.

### 1.3 Duplicate Detection

Duplicate detection is now in `register_internal()`, shared by both inventory and manual paths.

### Acceptance Criteria
- [ ] `CustomOpFactory` struct defined
- [ ] `inventory::collect!` set up
- [ ] Registry reads from inventory first
- [ ] Duplicate registration panics with clear message
- [ ] `list()` returns sorted results (deterministic)
- [ ] Existing `list()` return type preserved

---

## Phase 2: `#[register_custom_op]` Attribute Macro

**Goal:** Attribute macro that auto-registers unit struct ops.

### 2.1 Macro Design

```rust
// INPUT (what developer writes)
#[register_custom_op]
pub struct DocumentSolicitOp;

impl CustomOperation for DocumentSolicitOp {
    fn domain(&self) -> &'static str { "document" }
    fn verb(&self) -> &'static str { "solicit" }
    // ... execute method matching actual trait signature
}

// OUTPUT (what macro generates)
pub struct DocumentSolicitOp;  // Original struct preserved exactly

#[doc(hidden)]
fn __obpoc_factory_DocumentSolicitOp() -> ::std::sync::Arc<dyn crate::domain_ops::CustomOperation> {
    ::std::sync::Arc::new(DocumentSolicitOp)
}

::inventory::submit! {
    crate::domain_ops::CustomOpFactory {
        create: __obpoc_factory_DocumentSolicitOp
    }
}
```

### 2.2 Implementation

> **P0 FIX (Round 1):** Propagate `#[cfg(...)]` attributes to ALL generated items.
> **P1 FIX (Round 2):** Re-emit original parsed struct, don't reconstruct.

**File: `rust/crates/ob-poc-macros/src/register_op.rs`**

```rust
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, format_ident};
use syn::{parse_macro_input, ItemStruct, Attribute};

pub fn register_custom_op_impl(input: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(input as ItemStruct);
    
    let struct_name = &input_struct.ident;
    
    // Validate: must be a unit struct (no fields)
    if !matches!(input_struct.fields, syn::Fields::Unit) {
        return syn::Error::new_spanned(
            &input_struct.fields,
            "#[register_custom_op] only works on unit structs"
        )
        .to_compile_error()
        .into();
    }
    
    // P0 FIX: Extract #[cfg(...)] and #[cfg_attr(...)] attributes
    // These must be applied to ALL generated items
    let cfg_attrs: Vec<&Attribute> = input_struct.attrs.iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect();
    
    // Generate deterministic factory function name
    let factory_fn_name = format_ident!("__obpoc_factory_{}", struct_name);
    
    // P1 FIX: Re-emit the ORIGINAL struct unchanged (preserves all attrs, generics, etc.)
    let original_struct = &input_struct;
    
    let expanded = quote! {
        // Emit the original struct EXACTLY as parsed (preserves doc attrs, derives, etc.)
        #original_struct
        
        // Hidden factory function — MUST have same cfg attrs
        #(#cfg_attrs)*
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #factory_fn_name() -> ::std::sync::Arc<dyn crate::domain_ops::CustomOperation> {
            ::std::sync::Arc::new(#struct_name)
        }
        
        // Auto-register with inventory — MUST have same cfg attrs
        #(#cfg_attrs)*
        ::inventory::submit! {
            crate::domain_ops::CustomOpFactory {
                create: #factory_fn_name
            }
        }
    };
    
    TokenStream::from(expanded)
}
```

**File: `rust/crates/ob-poc-macros/src/lib.rs`**

```rust
use proc_macro::TokenStream;

mod register_op;
mod id_type;

/// Auto-register a custom operation with the registry.
/// 
/// Apply to unit structs that implement `CustomOperation`.
/// 
/// **Important:** All ops must live in the main crate (uses `crate::domain_ops` path).
/// 
/// ```rust
/// #[register_custom_op]
/// pub struct MyOp;
/// 
/// impl CustomOperation for MyOp {
///     fn domain(&self) -> &'static str { "my" }
///     fn verb(&self) -> &'static str { "op" }
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn register_custom_op(_attr: TokenStream, input: TokenStream) -> TokenStream {
    register_op::register_custom_op_impl(input)
}

/// Derive macro for UUID-backed ID newtypes.
/// 
/// **Important:** Do NOT also derive Clone, Copy, Debug, PartialEq, Eq, Hash, 
/// Serialize, or Deserialize — IdType generates all of these.
#[proc_macro_derive(IdType, attributes(id))]
pub fn derive_id_type(input: TokenStream) -> TokenStream {
    id_type::derive_id_type_impl(input)
}
```

### 2.3 Migration Strategy

**Phase A — Prove it works (2-4 ops):**

1. Add `#[register_custom_op]` to:
   - `EntityGhostOp` (entity_ops.rs)
   - `EntityIdentifyOp` (entity_ops.rs)
   - `DocumentCatalogOp` (document_ops.rs)
   - `DocumentExtractOp` (document_ops.rs)

2. Remove their manual `registry.register(Arc::new(XOp))` lines

3. Run tests, verify ops still work

**Phase B — Bulk migrate:**

1. Add `#[register_custom_op]` to ALL `pub struct *Op;` in `rust/src/domain_ops/*.rs`

2. Delete the giant manual list in `CustomOperationRegistry::new()`

3. Remove helper functions:
   - `agent_ops::register_agent_ops(&mut registry)`
   - `source_loader_ops::register_source_loader_ops(&mut registry)`
   - `manco_ops::register_manco_ops(&mut registry)`
   - etc.

### 2.4 YAML ↔ Op Sanity Check (High Value)

Add startup validation:

```rust
// After YAML runtime verbs load
for verb in runtime_verbs.iter() {
    if let RuntimeBehavior::Plugin(plugin_ref) = &verb.behavior {
        if !custom_ops.has(&verb.domain, &verb.verb) {
            panic!(
                "YAML declares plugin verb {}.{} but no op is registered. \
                 Did you forget #[register_custom_op]?",
                verb.domain, verb.verb
            );
        }
    }
}
```

> **Note:** Match your actual YAML schema. Check how `RuntimeBehavior::Plugin` is structured in the codebase.

### 2.5 Design Constraints (Document These)

1. **Ops must live in main crate** — Generated code uses `crate::domain_ops::CustomOperation`. If ops move to sub-crates, this breaks. (Future: could add `#[register_custom_op(path = "...")]`)

2. **Determinism** — Factory fn name derived from struct name. No HashMap iteration leaks. `list()` always sorted.

### Acceptance Criteria
- [ ] `#[register_custom_op]` compiles and generates correct code
- [ ] `#[cfg(...)]` attrs propagate to factory + submit (P0 fix)
- [ ] Original struct emitted unchanged (P1 fix)
- [ ] Ops auto-register at startup via inventory
- [ ] All existing ops migrated (no manual registry list)
- [ ] YAML ↔ op sanity check catches missing implementations

---

## Phase 3: `#[derive(IdType)]` — UUID Newtypes

**Goal:** Eliminate boilerplate for strongly-typed UUID IDs.

### 3.1 Target Types

| Type | Prefix | Table/Usage |
|------|--------|-------------|
| `AttributeId` | `attr` | Existing in `data_dictionary/attribute.rs` |
| `RequirementId` | `req` | `document_requirements` (new) |
| `VersionId` | `ver` | `document_versions` (new) |
| `TaskId` | `task` | `workflow_pending_tasks` (new) |
| `DocumentId` | `doc` | `documents` (new) |

### 3.2 Macro Design

> **P1 NOTE:** IdType generates Clone/Copy/Debug/Eq/Hash/Serialize/Deserialize.  
> **Do NOT also `#[derive(...)]` these traits** — you'll get conflicting impl errors.

```rust
// INPUT
#[derive(IdType)]
#[id(prefix = "req", new_v4)]  // new_v4 generates ::new() and Default
pub struct RequirementId(Uuid);

// DO NOT WRITE:
// #[derive(IdType, Clone, Debug)]  <-- WRONG, will conflict
```

### 3.3 Implementation

> **P0 FIX (Round 2):** 
> 1. Use fully-qualified `Deserialize` call to avoid trait-not-in-scope error
> 2. Return `Uuid` by value from `as_uuid()` for API compatibility (Uuid is Copy)

> **P1 FIX (Round 2):** Use `#[cfg(feature = "database")]` not `#[cfg(feature = "sqlx")]` to match repo feature name.

**File: `rust/crates/ob-poc-macros/src/id_type.rs`**

```rust
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields};

pub fn derive_id_type_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    
    // Parse attributes: #[id(prefix = "...", new_v4)]
    let (prefix, generate_new) = parse_id_attrs(&input.attrs);
    
    // Validate: must be tuple struct with single field
    let inner_type = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                &fields.unnamed.first().unwrap().ty
            }
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "IdType requires a tuple struct with exactly one field: struct MyId(Uuid)"
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input,
                "IdType only works on tuple structs"
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Generate new() + Default if requested
    let new_impl = if generate_new {
        quote! {
            impl #name {
                pub fn new() -> Self { Self(::uuid::Uuid::new_v4()) }
            }
            
            impl Default for #name {
                fn default() -> Self { Self::new() }
            }
        }
    } else {
        quote! {}
    };
    
    // Display format depends on prefix
    let display_impl = if let Some(ref pfx) = prefix {
        quote! {
            impl ::std::fmt::Display for #name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "{}_{}", #pfx, self.0)
                }
            }
        }
    } else {
        quote! {
            impl ::std::fmt::Display for #name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "{}", self.0)
                }
            }
        }
    };
    
    // FromStr handles prefix stripping
    let from_str_impl = if let Some(ref pfx) = prefix {
        let pfx_underscore = format!("{}_", pfx);
        quote! {
            impl ::std::str::FromStr for #name {
                type Err = ::uuid::Error;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    let uuid_str = s.strip_prefix(#pfx_underscore).unwrap_or(s);
                    Ok(Self(::uuid::Uuid::parse_str(uuid_str)?))
                }
            }
        }
    } else {
        quote! {
            impl ::std::str::FromStr for #name {
                type Err = ::uuid::Error;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Ok(Self(::uuid::Uuid::parse_str(s)?))
                }
            }
        }
    };
    
    let expanded = quote! {
        impl #name {
            pub fn from_uuid(id: #inner_type) -> Self { Self(id) }
            // P0 FIX: Return by value (Uuid is Copy) for API compatibility
            pub fn as_uuid(&self) -> #inner_type { self.0 }
        }
        
        #new_impl
        #display_impl
        #from_str_impl
        
        impl From<#inner_type> for #name {
            fn from(id: #inner_type) -> Self { Self(id) }
        }
        
        impl From<#name> for #inner_type {
            fn from(id: #name) -> Self { id.0 }
        }
        
        impl Clone for #name { fn clone(&self) -> Self { Self(self.0) } }
        impl Copy for #name {}
        impl PartialEq for #name { fn eq(&self, other: &Self) -> bool { self.0 == other.0 } }
        impl Eq for #name {}
        
        impl ::std::hash::Hash for #name {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
        
        impl ::std::fmt::Debug for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}({})", stringify!(#name), self.0)
            }
        }
        
        impl ::serde::Serialize for #name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.to_string())
            }
        }
        
        impl<'de> ::serde::Deserialize<'de> for #name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                // P0 FIX: Fully-qualified call to avoid trait-not-in-scope error
                let s = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                s.parse().map_err(::serde::de::Error::custom)
            }
        }
        
        // P1 FIX: Use "database" feature, not "sqlx" — match actual repo feature name
        // P0 FIX (Round 3): Use UFCS for trait method calls to avoid resolution issues
        #[cfg(feature = "database")]
        impl ::sqlx::Type<::sqlx::Postgres> for #name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <#inner_type as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }
        }
        
        #[cfg(feature = "database")]
        impl<'q> ::sqlx::Encode<'q, ::sqlx::Postgres> for #name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer
            ) -> ::sqlx::encode::IsNull {
                <#inner_type as ::sqlx::Encode<'q, ::sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            }
        }
        
        #[cfg(feature = "database")]
        impl<'r> ::sqlx::Decode<'r, ::sqlx::Postgres> for #name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'r>
            ) -> Result<Self, ::sqlx::error::BoxDynError> {
                Ok(Self(<#inner_type as ::sqlx::Decode<'r, ::sqlx::Postgres>>::decode(value)?))
            }
        }
    };
    
    TokenStream::from(expanded)
}

fn parse_id_attrs(attrs: &[syn::Attribute]) -> (Option<String>, bool) {
    let mut prefix = None;
    let mut new_v4 = false;
    
    for attr in attrs {
        if attr.path().is_ident("id") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("prefix") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    prefix = Some(value.value());
                } else if meta.path.is_ident("new_v4") {
                    new_v4 = true;
                }
                Ok(())
            }).ok();
        }
    }
    
    (prefix, new_v4)
}
```

### 3.4 Migrate AttributeId

> **IMPORTANT:** Before migrating, check the existing `AttributeId::as_uuid()` signature. 
> If it returns `&Uuid`, update call sites. If it returns `Uuid`, the macro is already compatible.

**File: `rust/src/data_dictionary/attribute.rs`**

Before:
```rust
pub struct AttributeId(Uuid);

impl AttributeId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    // ... 50+ lines of boilerplate
}
```

After:
```rust
use ob_poc_macros::IdType;

#[derive(IdType)]
#[id(prefix = "attr", new_v4)]
pub struct AttributeId(Uuid);

// That's it. All impls generated.
```

### Acceptance Criteria
- [ ] `#[derive(IdType)]` compiles
- [ ] Prefix attribute works for Display/FromStr
- [ ] `new_v4` attribute generates new() + Default
- [ ] Serde round-trips correctly (P0 fix: Deserialize compiles)
- [ ] `as_uuid()` returns by value for API compatibility (P0 fix)
- [ ] `database` feature compiles with Postgres (P1 fix)
- [ ] `AttributeId` migrated and behaves identically
- [ ] Compile-fail test: non-tuple struct rejected

---

## Phase 4: Database Migrations

**Goal:** Create all tables for workflow task queue and document requirements.

### 4.1 Migration Order (FK dependencies)

```
00_extensions.sql                 -- pgcrypto for gen_random_uuid()
01_rejection_reason_codes.sql     -- no deps, reference data
02_workflow_pending_tasks.sql     -- refs workflow_instances
03_document_requirements.sql      -- refs pending_tasks, rejection_reason_codes
04_documents.sql                  -- refs requirements
05_document_versions.sql          -- refs documents, pending_tasks, rejection_reason_codes
06_document_events.sql            -- refs documents, versions
07_task_result_queue.sql          -- no FK to pending_tasks (soft ref)
08_workflow_task_events.sql       -- refs pending_tasks
```

### 4.2 Key Tables

**00_extensions.sql:**
```sql
-- Required for gen_random_uuid() used in PRIMARY KEY defaults
-- Skip if already enabled in your schema
CREATE EXTENSION IF NOT EXISTS pgcrypto;
```

**rejection_reason_codes:**
```sql
CREATE TABLE "ob-poc".rejection_reason_codes (
    code TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    client_message TEXT NOT NULL,
    ops_message TEXT NOT NULL,
    next_action TEXT NOT NULL,
    is_retryable BOOLEAN DEFAULT true
);
```

**document_requirements:**

> **P1 NOTE:** `UNIQUE NULLS NOT DISTINCT` requires Postgres 15+.  
> Using `NOT NULL` columns + normal UNIQUE instead.

```sql
CREATE TABLE "ob-poc".document_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_instance_id UUID NOT NULL REFERENCES workflow_instances(instance_id),
    subject_entity_id UUID NOT NULL REFERENCES entities(entity_id),
    doc_type TEXT NOT NULL,
    required_state TEXT NOT NULL DEFAULT 'verified',
    status TEXT NOT NULL DEFAULT 'missing',
    attempt_count INT DEFAULT 0,
    max_attempts INT DEFAULT 3,
    current_task_id UUID REFERENCES workflow_pending_tasks(task_id),
    last_rejection_code TEXT REFERENCES rejection_reason_codes(code),
    satisfied_at TIMESTAMPTZ,
    UNIQUE (workflow_instance_id, subject_entity_id, doc_type)
);
```

**task_result_queue (with claim pattern):**

```sql
CREATE TABLE "ob-poc".task_result_queue (
    id BIGSERIAL PRIMARY KEY,
    task_id UUID NOT NULL,
    status TEXT NOT NULL,
    payload JSONB,
    queued_at TIMESTAMPTZ DEFAULT now(),
    
    -- Claim tracking
    claimed_at TIMESTAMPTZ,
    claimed_by TEXT,
    
    -- Only set on successful processing
    processed_at TIMESTAMPTZ,
    
    retry_count INT DEFAULT 0,
    max_retries INT DEFAULT 3,
    last_error TEXT,
    idempotency_key TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_task_result_queue_idempotency
    ON task_result_queue(task_id, idempotency_key);
```

### Acceptance Criteria
- [ ] All migrations run without error
- [ ] FK constraints satisfied
- [ ] Seed data inserted for rejection_reason_codes
- [ ] task_result_queue has claim columns

---

## Phase 5: Task Queue Custom Ops

**Goal:** Implement document ops using `#[register_custom_op]`.

### 5.1 New Ops to Create

| Op Struct | Domain | Verb | Purpose |
|-----------|--------|------|---------|
| `DocumentSolicitOp` | document | solicit | Request document from external |
| `DocumentSolicitSetOp` | document | solicit-set | Request multiple docs in one task |
| `DocumentVerifyOp` | document | verify | Mark version as QA-passed |
| `DocumentRejectOp` | document | reject | Reject with code, maybe re-request |
| `RequirementCreateOp` | requirement | create | Create document requirement |
| `RequirementWaiveOp` | requirement | waive | Waive requirement (ops override) |

### 5.2 Example Implementation

> **P1 FIX (Round 2):** Match actual `CustomOperation::execute` signature from codebase.
> The example below is illustrative — adjust to match actual trait signature.

**File: `rust/src/domain_ops/document_solicit_op.rs`**

```rust
use ob_poc_macros::register_custom_op;
use crate::domain_ops::{CustomOperation, ExecutionContext, ExecutionResult};

#[register_custom_op]
pub struct DocumentSolicitOp;

impl CustomOperation for DocumentSolicitOp {
    fn domain(&self) -> &'static str { "document" }
    fn verb(&self) -> &'static str { "solicit" }
    fn rationale(&self) -> &'static str { "Request document from external system" }
    
    // IMPORTANT: Match the actual trait signature from the codebase
    // This example assumes: execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool)
    // Adjust as needed based on actual CustomOperation trait definition
    #[cfg(feature = "database")]
    fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> ExecutionResult {
        // Extract args from verb_call
        let entity_id = verb_call.get_required::<EntityId>("entity-id")?;
        let doc_type = verb_call.get_required::<String>("doc-type")?;
        let due_date = verb_call.get_optional::<NaiveDate>("due-date")?;
        
        // 1. Ensure requirement exists
        let requirement_id = ensure_requirement(pool, ctx.workflow_instance_id(), entity_id, &doc_type)?;
        
        // 2. Create pending task
        let task_id = TaskId::new();
        create_pending_task(pool, task_id, ctx.workflow_instance_id(), "document.solicit", 1)?;
        
        // 3. Link requirement to task
        update_requirement_task(pool, requirement_id, task_id)?;
        update_requirement_status(pool, requirement_id, "requested")?;
        
        Ok(DslValue::Uuid(task_id.into()))
    }
}
```

### Acceptance Criteria
- [ ] All ops use `#[register_custom_op]`
- [ ] Ops appear in registry automatically
- [ ] Op signatures match actual `CustomOperation` trait
- [ ] YAML plugin verbs reference ops
- [ ] YAML ↔ op sanity check passes

---

## Phase 6: Queue Listener

**Goal:** Background listener that drains `task_result_queue` and advances workflows.

### 6.1 Listener Implementation

> **Note:** Claim timeout hardcoded in SQL matches `CLAIM_TIMEOUT_SECS` constant.

```rust
const CLAIM_TIMEOUT_SECS: i64 = 300; // 5 minutes — keep in sync with SQL interval

pub async fn task_result_listener(pool: PgPool, engine: Arc<WorkflowEngine>, instance_id: String) {
    loop {
        match pop_and_process(&pool, &engine, &instance_id).await {
            Ok(true) => continue,
            Ok(false) => tokio::time::sleep(Duration::from_millis(100)).await,
            Err(e) => {
                tracing::error!(?e, "Listener error");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn pop_and_process(pool: &PgPool, engine: &WorkflowEngine, instance_id: &str) -> Result<bool> {
    // Step 1: CLAIM a row (don't mark processed yet)
    let row = sqlx::query_as!(TaskResultRow, r#"
        WITH next AS (
            SELECT id FROM task_result_queue
            WHERE processed_at IS NULL
              AND (claimed_at IS NULL OR claimed_at < now() - interval '5 minutes')
              AND retry_count < max_retries
            ORDER BY id
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        UPDATE task_result_queue q
        SET claimed_at = now(),
            claimed_by = $1
        FROM next
        WHERE q.id = next.id
        RETURNING q.*
    "#, instance_id).fetch_optional(pool).await?;
    
    let Some(row) = row else { return Ok(false) };
    
    // Step 2: PROCESS
    match handle_bundle(pool, engine, &row).await {
        Ok(_) => {
            // SUCCESS: mark processed
            sqlx::query!(
                "UPDATE task_result_queue SET processed_at = now() WHERE id = $1",
                row.id
            ).execute(pool).await?;
        }
        Err(e) => {
            // FAILURE: release claim, increment retry
            let new_retry = row.retry_count + 1;
            if new_retry >= row.max_retries {
                move_to_dlq(pool, &row, &e.to_string()).await?;
            } else {
                sqlx::query!(
                    r#"
                    UPDATE task_result_queue 
                    SET claimed_at = NULL, 
                        claimed_by = NULL,
                        retry_count = $2,
                        last_error = $3
                    WHERE id = $1
                    "#,
                    row.id,
                    new_retry,
                    e.to_string()
                ).execute(pool).await?;
            }
            tracing::warn!(?e, task_id = %row.task_id, retry = new_retry, "Task processing failed");
        }
    }
    
    Ok(true)
}
```

### Acceptance Criteria
- [ ] Listener claims rows before processing
- [ ] Failed processing releases claim, increments retry
- [ ] Only successful processing sets `processed_at`
- [ ] DLQ after max retries
- [ ] Stale claims (>5 min) can be reclaimed

---

## Phase 7: API Endpoints

**Goal:** HTTP endpoints for external systems.

### 7.1 Routes

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/workflow/task-complete` | POST | Bundle callback from external |
| `/api/documents` | POST | Create logical document |
| `/api/documents/{id}/versions` | POST | Upload new version |
| `/api/requirements/{id}` | GET | Check requirement status |

### 7.2 Webhook Handler

```rust
pub async fn handle_task_complete(
    State(pool): State<PgPool>,
    Json(req): Json<TaskCompleteBundle>,
) -> Result<StatusCode, AppError> {
    validate_task_not_terminal(&pool, req.task_id).await?;
    
    for item in &req.items {
        validate_version_exists(&pool, &item.cargo_ref).await?;
    }
    
    // Use column list for ON CONFLICT, not constraint name
    let result = sqlx::query!(r#"
        INSERT INTO task_result_queue (task_id, status, payload, idempotency_key)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (task_id, idempotency_key) DO NOTHING
    "#,
        req.task_id.as_uuid(),
        req.status.as_str(),
        serde_json::to_value(&req)?,
        req.idempotency_key
    ).execute(&pool).await?;
    
    if result.rows_affected() == 0 {
        Ok(StatusCode::OK)  // Duplicate
    } else {
        Ok(StatusCode::ACCEPTED)
    }
}
```

### Acceptance Criteria
- [ ] Bundle webhook accepts payload
- [ ] ON CONFLICT uses column list
- [ ] Idempotency handles duplicates correctly

---

## Phase 8: Tests

### 8.1 Compile-Fail Tests (trybuild)

```rust
// tests/trybuild/register_op_non_unit.rs
#[register_custom_op]
pub struct BadOp { inner: String }  // Should fail: not unit struct

// tests/trybuild/id_type_non_tuple.rs
#[derive(IdType)]
pub struct BadId { inner: Uuid }  // Should fail: not tuple struct

// NOTE: cfg propagation is tested via unit tests + feature-gated CI, not trybuild
// (trybuild won't enable test features, so cfg-gated code won't compile at all)
```

### 8.2 Unit Tests

```rust
#[test]
fn test_registry_has_ops_from_inventory() {
    let registry = CustomOperationRegistry::new();
    assert!(registry.has("document", "solicit"));
    assert!(registry.has("entity", "ghost"));
}

#[test]
fn test_registry_list_is_sorted() {
    let registry = CustomOperationRegistry::new();
    let list = registry.list();
    // Verify sorted by (domain, verb)
    for i in 1..list.len() {
        assert!((list[i-1].0, list[i-1].1) <= (list[i].0, list[i].1),
            "Registry list must be deterministically sorted");
    }
}

#[test]
fn test_attribute_id_roundtrip() {
    let id = AttributeId::new();
    let s = id.to_string();
    let parsed: AttributeId = s.parse().unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_attribute_id_as_uuid_returns_value() {
    let id = AttributeId::new();
    let uuid: Uuid = id.as_uuid();  // Should return by value, not reference
    assert_eq!(uuid, id.0);
}
```

### Acceptance Criteria
- [ ] Compile-fail tests pass
- [ ] Registry tests pass (including determinism)
- [ ] ID type round-trip tests pass
- [ ] `as_uuid()` returns by value (P0 fix verification)
- [ ] Integration tests pass

---

## Implementation Order

```
PR 1: Macros + Registry (Phases 0-3)
  Phase 0: Macro crate setup
    └─► Phase 1: inventory infrastructure
         └─► Phase 2: #[register_custom_op] + migrate ops
              └─► Phase 3: #[derive(IdType)] + migrate AttributeId
                   └─► Tests for macros

PR 2: Task Queue (Phases 4-8)  
  Phase 4: Database migrations
    └─► Phase 5: Task queue ops
         └─► Phase 6: Queue listener
              └─► Phase 7: API endpoints
                   └─► Phase 8: Integration tests
```

---

## P0 Fixes Applied (Summary)

### Round 1
| Issue | Fix Location |
|-------|--------------|
| `#[cfg]` not propagated to factory/submit | Phase 2.2 |
| Workspace deps at wrong level | Phase 0.2 |
| `ON CONFLICT ON CONSTRAINT` invalid | Phase 7.2 |
| Listener marks processed before processing | Phase 6.1 |

### Round 2
| Issue | Fix Location |
|-------|--------------|
| Wrong Cargo path (`rust/src/Cargo.toml`) | Phase 0.2 — corrected to `rust/Cargo.toml` |
| Registry field/list mismatch | Phase 1.2 — note to check actual field name, preserve `list()` return type |
| IdType Deserialize won't compile | Phase 3.3 — fully-qualified `String::deserialize` |
| IdType `as_uuid()` breaks API | Phase 3.3 — return `Uuid` by value, not `&Uuid` |

### Round 3
| Issue | Fix Location |
|-------|--------------|
| IdType SQLx encode/decode won't resolve without UFCS | Phase 3.3 — use `<T as Trait>::method()` syntax |
| `gen_random_uuid()` requires pgcrypto extension | Phase 4.1/4.2 — added `00_extensions.sql` |
| trybuild cfg test won't actually test cfg propagation | Phase 8.1 — removed, use unit tests + CI instead |

## P1 Improvements Applied

| Improvement | Applied |
|-------------|---------|
| Document "ops must live in main crate" | Phase 2.5 |
| Document "do not derive traits IdType generates" | Phase 3.2 |
| `UNIQUE NULLS NOT DISTINCT` PG15+ note | Phase 4.2 |
| Determinism: list() sorted | Phase 1.2 |
| Re-emit original struct (don't reconstruct) | Phase 2.2 |
| Use `database` feature not `sqlx` | Phase 3.3 |
| Match actual CustomOperation signature | Phase 5.2 |

---

## Pre-Implementation Checklist

Before Claude Code starts, verify these in the actual codebase:

- [ ] Check `rust/Cargo.toml` — is there already a `[workspace]` section?
- [ ] Check `CustomOperationRegistry` — what is the HashMap field name?
- [ ] Check `CustomOperationRegistry::list()` — what is the return type?
- [ ] Check `CustomOperation` trait — does it have `rationale()`?
- [ ] Check `CustomOperation::execute` — what is the actual signature?
- [ ] Check existing `AttributeId::as_uuid()` — returns `Uuid` or `&Uuid`?
- [ ] Check feature flag — is it `database` or `sqlx`?

---

## References

- TODO-WORKFLOW-TASK-QUEUE.md (5 peer review rounds)
- TODO (Claude Code) — Add Proc Macros (Document 3)
- ChatGPT peer review (2 rounds)
- inventory crate: https://docs.rs/inventory
- trybuild crate: https://docs.rs/trybuild
