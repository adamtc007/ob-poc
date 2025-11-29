# Unified Verb Registry Implementation

**Problem**: Two parallel verb systems (CRUD verbs in `verbs.rs` and custom ops in `custom_ops/mod.rs`) are not integrated, causing DSL that validates to fail at compilation.

**Goal**: Create a unified verb registry that all pipeline stages (parser, CSG linter, compiler, executor) can query consistently.

---

## Current Architecture (Broken)

```
verbs.rs (53 CRUD verbs)          custom_ops/mod.rs (special ops)
├── cbu.create                     ├── document.catalog
├── cbu.read                       ├── document.extract  
├── entity.create                  ├── ubo.calculate
├── document.read                  ├── screening.pep
├── document.link-to-entity        ├── screening.sanctions
└── ...                            └── ...

CSG Linter: Hardcoded knowledge of document.catalog ✓
Compiler:   Only queries verbs.rs ✗
Executor:   Has custom_ops dispatch ✓
```

## Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    UnifiedVerbRegistry                       │
├─────────────────────────────────────────────────────────────┤
│  fn get_verb(domain, verb) -> Option<VerbDefinition>        │
│  fn list_verbs() -> Vec<VerbDefinition>                     │
│  fn verbs_for_domain(domain) -> Vec<VerbDefinition>         │
│  fn domains() -> Vec<&str>                                  │
├─────────────────────────────────────────────────────────────┤
│  Sources:                                                    │
│  ├── CRUD verbs (from verbs.rs)                             │
│  └── Custom ops (from custom_ops registry)                  │
├─────────────────────────────────────────────────────────────┤
│  VerbDefinition {                                           │
│    domain: String,                                          │
│    verb: String,                                            │
│    description: String,                                     │
│    args: Vec<ArgDef>,                                       │
│    behavior: VerbBehavior,  // CRUD, CustomOp, Composite    │
│    custom_op_id: Option<String>,  // For custom ops         │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
          │
          ├──▶ CSG Linter (validates verb exists + args)
          ├──▶ Compiler (builds execution plan)
          └──▶ Executor (dispatches to CRUD or custom handler)
```

---

## Implementation Plan

### Step 1: Define Unified Types

Create file: `rust/src/dsl_v2/verb_registry.rs`

```rust
//! Unified Verb Registry
//!
//! Single source of truth for all verbs in the DSL system.
//! Combines CRUD verbs from `verbs.rs` with custom operations from `custom_ops`.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::verbs::{VerbDef, STANDARD_VERBS, ArgDef};
use super::custom_ops::{CUSTOM_OPS_REGISTRY, CustomOpDef};

// =============================================================================
// TYPES
// =============================================================================

/// How a verb is executed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbBehavior {
    /// Standard CRUD operation (generic executor)
    Crud,
    /// Custom operation with specialized handler
    CustomOp,
    /// Composite operation (expands to multiple steps)
    Composite,
}

/// Unified verb definition combining CRUD and custom ops
#[derive(Debug, Clone)]
pub struct UnifiedVerbDef {
    pub domain: &'static str,
    pub verb: &'static str,
    pub description: &'static str,
    pub args: &'static [ArgDef],
    pub behavior: VerbBehavior,
    /// For custom ops, the handler ID
    pub custom_op_id: Option<&'static str>,
    /// Original CRUD verb def (if applicable)
    pub crud_def: Option<&'static VerbDef>,
}

impl UnifiedVerbDef {
    /// Full verb name: "domain.verb"
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.domain, self.verb)
    }

    /// Check if verb accepts a given argument key
    pub fn accepts_arg(&self, key: &str) -> bool {
        self.args.iter().any(|a| a.name == key)
    }

    /// Get required arguments
    pub fn required_args(&self) -> Vec<&ArgDef> {
        self.args.iter().filter(|a| a.required).collect()
    }
}

// =============================================================================
// REGISTRY
// =============================================================================

/// The unified verb registry - singleton
static UNIFIED_REGISTRY: OnceLock<UnifiedVerbRegistry> = OnceLock::new();

pub struct UnifiedVerbRegistry {
    /// All verbs indexed by "domain.verb"
    verbs: HashMap<String, UnifiedVerbDef>,
    /// Verbs grouped by domain
    by_domain: HashMap<String, Vec<String>>,
    /// All domain names
    domains: Vec<String>,
}

impl UnifiedVerbRegistry {
    /// Get the global registry instance
    pub fn global() -> &'static UnifiedVerbRegistry {
        UNIFIED_REGISTRY.get_or_init(|| Self::build())
    }

    /// Build the registry from all sources
    fn build() -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<String, Vec<String>> = HashMap::new();

        // 1. Load CRUD verbs from verbs.rs
        for crud_verb in STANDARD_VERBS.iter() {
            let key = format!("{}.{}", crud_verb.domain, crud_verb.verb);
            let unified = UnifiedVerbDef {
                domain: crud_verb.domain,
                verb: crud_verb.verb,
                description: crud_verb.description,
                args: crud_verb.args,
                behavior: VerbBehavior::Crud,
                custom_op_id: None,
                crud_def: Some(crud_verb),
            };
            verbs.insert(key.clone(), unified);
            by_domain.entry(crud_verb.domain.to_string())
                .or_default()
                .push(key);
        }

        // 2. Load custom ops (may override CRUD verbs)
        for custom_op in custom_ops_definitions() {
            let key = format!("{}.{}", custom_op.domain, custom_op.verb);
            let unified = UnifiedVerbDef {
                domain: custom_op.domain,
                verb: custom_op.verb,
                description: custom_op.description,
                args: custom_op.args,
                behavior: VerbBehavior::CustomOp,
                custom_op_id: Some(custom_op.op_id),
                crud_def: None,
            };
            // Custom ops override CRUD if same name
            verbs.insert(key.clone(), unified);
            by_domain.entry(custom_op.domain.to_string())
                .or_default()
                .push(key);
        }

        // Deduplicate domain verb lists
        for list in by_domain.values_mut() {
            list.sort();
            list.dedup();
        }

        let domains: Vec<String> = by_domain.keys().cloned().collect();

        Self { verbs, by_domain, domains }
    }

    /// Look up a verb by domain and verb name
    pub fn get(&self, domain: &str, verb: &str) -> Option<&UnifiedVerbDef> {
        let key = format!("{}.{}", domain, verb);
        self.verbs.get(&key)
    }

    /// Look up by full name "domain.verb"
    pub fn get_by_name(&self, full_name: &str) -> Option<&UnifiedVerbDef> {
        self.verbs.get(full_name)
    }

    /// Get all verbs for a domain
    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&UnifiedVerbDef> {
        self.by_domain.get(domain)
            .map(|keys| keys.iter().filter_map(|k| self.verbs.get(k)).collect())
            .unwrap_or_default()
    }

    /// Get all domain names
    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    /// Get all verbs
    pub fn all_verbs(&self) -> impl Iterator<Item = &UnifiedVerbDef> {
        self.verbs.values()
    }

    /// Total verb count
    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if a verb exists
    pub fn contains(&self, domain: &str, verb: &str) -> bool {
        self.get(domain, verb).is_some()
    }
}

// =============================================================================
// CUSTOM OPS DEFINITIONS
// =============================================================================

/// Static definition for a custom operation
#[derive(Debug)]
pub struct CustomOpStaticDef {
    pub domain: &'static str,
    pub verb: &'static str,
    pub op_id: &'static str,
    pub description: &'static str,
    pub args: &'static [ArgDef],
}

/// Get all custom operation definitions
/// This bridges the gap between custom_ops/mod.rs and the registry
fn custom_ops_definitions() -> Vec<CustomOpStaticDef> {
    vec![
        // Document operations
        CustomOpStaticDef {
            domain: "document",
            verb: "catalog",
            op_id: "document.catalog",
            description: "Catalog a document for an entity within a CBU",
            args: &[
                ArgDef { name: "cbu-id", arg_type: "ref:cbu", required: true, description: "CBU reference" },
                ArgDef { name: "entity-id", arg_type: "ref:entity", required: true, description: "Entity reference" },
                ArgDef { name: "document-type", arg_type: "ref:document_type", required: true, description: "Document type code" },
                ArgDef { name: "file-path", arg_type: "string", required: false, description: "Path to document file" },
                ArgDef { name: "metadata", arg_type: "map", required: false, description: "Additional metadata" },
            ],
        },
        CustomOpStaticDef {
            domain: "document",
            verb: "extract",
            op_id: "document.extract",
            description: "Extract attributes from a cataloged document",
            args: &[
                ArgDef { name: "document-id", arg_type: "ref:document", required: true, description: "Document reference" },
                ArgDef { name: "attributes", arg_type: "list:string", required: false, description: "Specific attributes to extract" },
                ArgDef { name: "use-ocr", arg_type: "boolean", required: false, description: "Enable OCR extraction" },
            ],
        },
        CustomOpStaticDef {
            domain: "document",
            verb: "request",
            op_id: "document.request",
            description: "Request a document from client",
            args: &[
                ArgDef { name: "cbu-id", arg_type: "ref:cbu", required: true, description: "CBU reference" },
                ArgDef { name: "entity-id", arg_type: "ref:entity", required: true, description: "Entity reference" },
                ArgDef { name: "document-type", arg_type: "ref:document_type", required: true, description: "Document type code" },
                ArgDef { name: "due-date", arg_type: "date", required: false, description: "Request due date" },
                ArgDef { name: "priority", arg_type: "string", required: false, description: "Priority level" },
            ],
        },
        // UBO operations
        CustomOpStaticDef {
            domain: "ubo",
            verb: "calculate",
            op_id: "ubo.calculate",
            description: "Calculate ultimate beneficial ownership chain",
            args: &[
                ArgDef { name: "cbu-id", arg_type: "ref:cbu", required: true, description: "CBU reference" },
                ArgDef { name: "entity-id", arg_type: "ref:entity", required: true, description: "Entity to analyze" },
                ArgDef { name: "threshold", arg_type: "number", required: false, description: "Ownership threshold (default 25%)" },
            ],
        },
        CustomOpStaticDef {
            domain: "ubo",
            verb: "validate",
            op_id: "ubo.validate",
            description: "Validate UBO structure completeness",
            args: &[
                ArgDef { name: "cbu-id", arg_type: "ref:cbu", required: true, description: "CBU reference" },
            ],
        },
        // Screening operations
        CustomOpStaticDef {
            domain: "screening",
            verb: "pep",
            op_id: "screening.pep",
            description: "Run PEP (Politically Exposed Person) screening",
            args: &[
                ArgDef { name: "entity-id", arg_type: "ref:entity", required: true, description: "Entity to screen" },
                ArgDef { name: "provider", arg_type: "string", required: false, description: "Screening provider" },
            ],
        },
        CustomOpStaticDef {
            domain: "screening",
            verb: "sanctions",
            op_id: "screening.sanctions",
            description: "Run sanctions list screening",
            args: &[
                ArgDef { name: "entity-id", arg_type: "ref:entity", required: true, description: "Entity to screen" },
                ArgDef { name: "lists", arg_type: "list:string", required: false, description: "Specific sanction lists" },
            ],
        },
        CustomOpStaticDef {
            domain: "screening",
            verb: "adverse-media",
            op_id: "screening.adverse_media",
            description: "Run adverse media screening",
            args: &[
                ArgDef { name: "entity-id", arg_type: "ref:entity", required: true, description: "Entity to screen" },
                ArgDef { name: "lookback-months", arg_type: "number", required: false, description: "Months to search back" },
            ],
        },
        // KYC operations
        CustomOpStaticDef {
            domain: "kyc",
            verb: "initiate",
            op_id: "kyc.initiate",
            description: "Initiate KYC investigation",
            args: &[
                ArgDef { name: "cbu-id", arg_type: "ref:cbu", required: true, description: "CBU reference" },
                ArgDef { name: "investigation-type", arg_type: "string", required: true, description: "Type of investigation" },
            ],
        },
        CustomOpStaticDef {
            domain: "kyc",
            verb: "decide",
            op_id: "kyc.decide",
            description: "Record KYC decision",
            args: &[
                ArgDef { name: "investigation-id", arg_type: "ref:investigation", required: true, description: "Investigation reference" },
                ArgDef { name: "decision", arg_type: "string", required: true, description: "Decision: approve, reject, escalate" },
                ArgDef { name: "rationale", arg_type: "string", required: true, description: "Decision rationale" },
            ],
        },
    ]
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Get the global registry
pub fn registry() -> &'static UnifiedVerbRegistry {
    UnifiedVerbRegistry::global()
}

/// Look up a verb (convenience function)
pub fn find_verb(domain: &str, verb: &str) -> Option<&'static UnifiedVerbDef> {
    registry().get(domain, verb)
}

/// Check if verb exists (convenience function)
pub fn verb_exists(domain: &str, verb: &str) -> bool {
    registry().contains(domain, verb)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads() {
        let reg = UnifiedVerbRegistry::global();
        assert!(reg.len() > 0, "Registry should have verbs");
    }

    #[test]
    fn test_crud_verb_exists() {
        let reg = registry();
        let verb = reg.get("cbu", "create");
        assert!(verb.is_some(), "cbu.create should exist");
        assert_eq!(verb.unwrap().behavior, VerbBehavior::Crud);
    }

    #[test]
    fn test_custom_op_exists() {
        let reg = registry();
        let verb = reg.get("document", "catalog");
        assert!(verb.is_some(), "document.catalog should exist");
        assert_eq!(verb.unwrap().behavior, VerbBehavior::CustomOp);
    }

    #[test]
    fn test_domains_list() {
        let reg = registry();
        let domains = reg.domains();
        assert!(domains.contains(&"cbu".to_string()));
        assert!(domains.contains(&"entity".to_string()));
        assert!(domains.contains(&"document".to_string()));
    }

    #[test]
    fn test_verbs_for_domain() {
        let reg = registry();
        let doc_verbs = reg.verbs_for_domain("document");
        
        // Should have both CRUD and custom ops
        let verb_names: Vec<_> = doc_verbs.iter().map(|v| v.verb).collect();
        assert!(verb_names.contains(&"catalog"), "Should have document.catalog");
        assert!(verb_names.contains(&"extract"), "Should have document.extract");
    }

    #[test]
    fn test_custom_op_args() {
        let reg = registry();
        let catalog = reg.get("document", "catalog").unwrap();
        
        assert!(catalog.accepts_arg("cbu-id"));
        assert!(catalog.accepts_arg("entity-id"));
        assert!(catalog.accepts_arg("document-type"));
        assert!(!catalog.accepts_arg("nonexistent-arg"));
    }
}
```

---

### Step 2: Update mod.rs Exports

Edit file: `rust/src/dsl_v2/mod.rs`

Add the new module and re-exports:

```rust
// Add to module declarations (near top)
pub mod verb_registry;

// Add to re-exports (near bottom)
pub use verb_registry::{
    UnifiedVerbRegistry, UnifiedVerbDef, VerbBehavior,
    registry, find_verb as find_unified_verb, verb_exists,
};
```

---

### Step 3: Update Compiler to Use Unified Registry

Edit file: `rust/src/dsl_v2/execution_plan.rs`

Find the `compile` function and update verb lookup:

```rust
// Change this import at the top:
// use super::verbs::{find_verb, VerbDef};

// To this:
use super::verb_registry::{registry, VerbBehavior};

// In the compile function, change verb lookup from:
//
// let verb_def = find_verb(&vc.domain, &vc.verb)
//     .ok_or_else(|| CompileError::UnknownVerb {
//         domain: vc.domain.clone(),
//         verb: vc.verb.clone(),
//     })?;
//
// To:

let verb_def = registry().get(&vc.domain, &vc.verb)
    .ok_or_else(|| CompileError::UnknownVerb {
        domain: vc.domain.clone(),
        verb: vc.verb.clone(),
    })?;

// Update ExecutionStep to track behavior:
// Add field if not present:
pub struct ExecutionStep {
    // ... existing fields ...
    
    /// How this step should be executed
    pub behavior: VerbBehavior,
    
    /// For custom ops, the handler ID
    pub custom_op_id: Option<String>,
}

// When building ExecutionStep:
ExecutionStep {
    // ... existing fields ...
    behavior: verb_def.behavior,
    custom_op_id: verb_def.custom_op_id.map(|s| s.to_string()),
}
```

---

### Step 4: Update Executor to Dispatch Based on Behavior

Edit file: `rust/src/dsl_v2/executor.rs`

Update the execution dispatch logic:

```rust
use super::verb_registry::VerbBehavior;

impl DslExecutor {
    pub async fn execute_step(
        &self,
        step: &ExecutionStep,
        ctx: &mut ExecutionContext,
    ) -> Result<StepResult> {
        match step.behavior {
            VerbBehavior::Crud => {
                // Existing CRUD execution logic
                self.execute_crud_step(step, ctx).await
            }
            VerbBehavior::CustomOp => {
                // Dispatch to custom ops handler
                let op_id = step.custom_op_id.as_ref()
                    .ok_or_else(|| anyhow!("Custom op missing handler ID"))?;
                self.execute_custom_op(op_id, step, ctx).await
            }
            VerbBehavior::Composite => {
                // Composite ops expand to multiple steps
                // (Not yet implemented - placeholder)
                Err(anyhow!("Composite ops not yet implemented"))
            }
        }
    }

    async fn execute_crud_step(
        &self,
        step: &ExecutionStep,
        ctx: &mut ExecutionContext,
    ) -> Result<StepResult> {
        // ... existing CRUD logic ...
    }

    async fn execute_custom_op(
        &self,
        op_id: &str,
        step: &ExecutionStep,
        ctx: &mut ExecutionContext,
    ) -> Result<StepResult> {
        // Dispatch to custom_ops registry
        match op_id {
            "document.catalog" => {
                // Call document catalog handler
                let handler = super::custom_ops::DocumentCatalogOp;
                handler.execute(&self.pool, step, ctx).await
            }
            "document.extract" => {
                let handler = super::custom_ops::DocumentExtractOp;
                handler.execute(&self.pool, step, ctx).await
            }
            "ubo.calculate" => {
                let handler = super::custom_ops::UboCalculateOp;
                handler.execute(&self.pool, step, ctx).await
            }
            "screening.pep" => {
                let handler = super::custom_ops::ScreeningPepOp;
                handler.execute(&self.pool, step, ctx).await
            }
            "screening.sanctions" => {
                let handler = super::custom_ops::ScreeningSanctionsOp;
                handler.execute(&self.pool, step, ctx).await
            }
            _ => Err(anyhow!("Unknown custom op: {}", op_id)),
        }
    }
}
```

---

### Step 5: Update CSG Linter to Use Unified Registry

Edit file: `rust/src/dsl_v2/csg_linter.rs`

Replace hardcoded verb knowledge with registry lookup:

```rust
use super::verb_registry::{registry, verb_exists};

// In analyze_statement, replace hardcoded match:
//
// match (vc.domain.as_str(), vc.verb.as_str()) {
//     ("document", "catalog") | ("document", "request") => { ... }
// }
//
// With registry-based check:

fn analyze_statement(&self, vc: &VerbCall, source: &str, inferred: &mut InferredContext) {
    let span = self.span_to_source_span(&vc.span, source);

    // Verify verb exists in unified registry
    if !verb_exists(&vc.domain, &vc.verb) {
        // Will be caught by compiler, but could add diagnostic here
    }

    // ... rest of symbol binding logic ...

    // Track document operations (check verb definition for document domain)
    if vc.domain == "document" {
        if let Some(verb_def) = registry().get(&vc.domain, &vc.verb) {
            // Check if verb has document-type argument
            if verb_def.accepts_arg("document-type") {
                if let Some(doc_type) = self.extract_string_arg(vc, "document-type") {
                    inferred.document_catalogs.push(DocumentCatalog {
                        symbol: vc.as_binding.clone(),
                        document_type: doc_type,
                        cbu_ref: self.extract_ref_arg(vc, "cbu-id"),
                        entity_ref: self.extract_ref_arg(vc, "entity-id"),
                        span,
                    });
                }
            }
        }
    }
    
    // ... rest of analysis ...
}
```

---

### Step 6: Update CLI to Use Unified Registry

Edit file: `rust/src/bin/dsl_cli.rs`

Update the `cmd_verbs` function:

```rust
fn cmd_verbs(domain: Option<String>, verbose: bool, format: OutputFormat) -> Result<(), String> {
    use ob_poc::dsl_v2::verb_registry::{registry, VerbBehavior};

    let reg = registry();
    
    let domains_to_show: Vec<&str> = match &domain {
        Some(d) => vec![d.as_str()],
        None => reg.domains().iter().map(|s| s.as_str()).collect(),
    };

    match format {
        OutputFormat::Json => {
            let mut output = serde_json::Map::new();
            for d in &domains_to_show {
                let verbs: Vec<_> = reg.verbs_for_domain(d)
                    .iter()
                    .map(|v| {
                        serde_json::json!({
                            "verb": v.full_name(),
                            "description": v.description,
                            "behavior": format!("{:?}", v.behavior),
                            "args": v.args.iter().map(|a| {
                                serde_json::json!({
                                    "name": a.name,
                                    "type": a.arg_type,
                                    "required": a.required,
                                })
                            }).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                output.insert(d.to_string(), serde_json::Value::Array(verbs));
            }
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            for d in &domains_to_show {
                println!("{}", format!("Domain: {}", d).cyan().bold());
                println!();
                
                for v in reg.verbs_for_domain(d) {
                    let behavior_tag = match v.behavior {
                        VerbBehavior::Crud => "[CRUD]".dimmed(),
                        VerbBehavior::CustomOp => "[CUSTOM]".yellow(),
                        VerbBehavior::Composite => "[COMPOSITE]".blue(),
                    };
                    
                    println!("  {}.{} {}", v.domain.green(), v.verb.green().bold(), behavior_tag);
                    if verbose {
                        println!("    {}", v.description.dimmed());
                        for arg in v.args {
                            let req = if arg.required { "*" } else { "" };
                            println!("    :{}{} ({})", arg.name, req, arg.arg_type);
                        }
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}
```

---

### Step 7: Update Demo Scenarios

Now that `document.catalog` is recognized, restore the original demos in `dsl_cli.rs`:

```rust
fn cmd_demo(scenario: &str, ...) -> Result<(), String> {
    let (name, dsl) = match scenario {
        "onboard-individual" | "individual" => (
            "Onboard Individual Client",
            r#"
; Onboard an individual client with passport
(cbu.create 
    :name "John Smith" 
    :client-type "individual" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-proper-person 
    :cbu-id @cbu
    :name "John Smith"
    :first-name "John"
    :last-name "Smith"
    :as @person)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT"
    :as @passport)
"#,
        ),
        // ... rest of demos with document.catalog restored ...
    };
    // ...
}
```

---

## Execution Checklist

### Phase 1: Create Unified Registry
- [ ] Create `rust/src/dsl_v2/verb_registry.rs` with full implementation
- [ ] Add module declaration to `rust/src/dsl_v2/mod.rs`
- [ ] Add re-exports to `mod.rs`
- [ ] Run `cargo check` - fix any compile errors

### Phase 2: Verify Registry
- [ ] Run `cargo test verb_registry` - all tests should pass
- [ ] Verify `document.catalog` is in registry
- [ ] Verify CRUD verbs still present

### Phase 3: Update Compiler
- [ ] Update imports in `execution_plan.rs`
- [ ] Change verb lookup to use unified registry
- [ ] Add `behavior` and `custom_op_id` fields to `ExecutionStep`
- [ ] Run `cargo check` - fix any compile errors

### Phase 4: Update Executor
- [ ] Add behavior-based dispatch in `executor.rs`
- [ ] Implement `execute_custom_op` dispatch function
- [ ] Run `cargo check` - fix any compile errors

### Phase 5: Update CSG Linter
- [ ] Update `csg_linter.rs` to use registry
- [ ] Replace hardcoded verb matches with registry lookups
- [ ] Run `cargo check` - fix any compile errors

### Phase 6: Update CLI
- [ ] Update `cmd_verbs` to use unified registry
- [ ] Restore `document.catalog` in demo scenarios
- [ ] Run `cargo check --features cli` - fix any compile errors

### Phase 7: Integration Testing
- [ ] Run full test suite: `cargo test`
- [ ] Run CLI self-tests: `bash rust/tests/cli_self_test.sh`
- [ ] Test demo scenarios manually:
  ```bash
  cargo run --features cli --bin dsl_cli -- demo onboard-individual
  cargo run --features cli --bin dsl_cli -- demo onboard-corporate
  cargo run --features cli --bin dsl_cli -- demo invalid
  ```

### Phase 8: Verify Full Pipeline
- [ ] Parse → Validate → Plan should work for `document.catalog`
- [ ] `dsl_cli verbs --domain document` should show both CRUD and custom ops
- [ ] CSG validation should still catch passport-for-company error

---

## Verification Commands

```bash
# Check registry loads correctly
cargo test --lib verb_registry

# Check all tests pass
cargo test

# Test CLI with document.catalog
echo '(document.catalog :cbu-id @cbu :entity-id @person :document-type "PASSPORT")' | \
  cargo run --features cli --bin dsl_cli -- parse

# List all document verbs (should include catalog, extract)
cargo run --features cli --bin dsl_cli -- verbs --domain document --verbose

# Run full demo
cargo run --features cli --bin dsl_cli -- demo onboard-individual
```

---

## Architecture After Implementation

```
┌─────────────────────────────────────────────────────────────────┐
│                    UnifiedVerbRegistry                           │
│  ┌─────────────────┬─────────────────────────────────────────┐  │
│  │   CRUD Verbs    │         Custom Ops                      │  │
│  │   (verbs.rs)    │    (verb_registry.rs definitions)       │  │
│  ├─────────────────┼─────────────────────────────────────────┤  │
│  │ cbu.create      │ document.catalog                        │  │
│  │ cbu.read        │ document.extract                        │  │
│  │ entity.create   │ document.request                        │  │
│  │ document.read   │ ubo.calculate                           │  │
│  │ document.link-* │ screening.pep                           │  │
│  │ role.assign     │ screening.sanctions                     │  │
│  │ ...53 verbs     │ kyc.initiate, kyc.decide                │  │
│  └─────────────────┴─────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   ┌─────────┐          ┌──────────┐          ┌──────────┐
   │ Parser  │          │ Compiler │          │ Executor │
   └─────────┘          └──────────┘          └──────────┘
        │                     │                     │
        ▼                     ▼                     ▼
   ┌───────────┐        ┌───────────┐        ┌───────────────┐
   │CSG Linter │        │ Exec Plan │        │ CRUD Handler  │
   │(validates)│        │ (steps)   │        │ Custom Handler│
   └───────────┘        └───────────┘        └───────────────┘
```

All components now query the same unified registry - no more "validates but won't compile" scenarios.
