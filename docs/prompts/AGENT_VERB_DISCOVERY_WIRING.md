# Agent Verb Discovery Wiring Fix

## Context

The `dsl_verbs` table exists but is empty (0 rows). The `VerbSyncService` and `VerbDiscoveryService` are fully implemented but sync is never called on startup. This means the agent cannot discover its vocabulary via RAG.

## Task 1: Add Verb Sync to API Startup

File: `/Users/adamtc007/Developer/ob-poc/rust/src/bin/dsl_api.rs`

In the `main()` function, after the pool is created but before the router is built, add verb sync:

```rust
// Add these imports at the top of the file
use ob_poc::session::VerbSyncService;
use ob_poc::dsl_v2::RuntimeVerbRegistry;

// In main(), after pool creation:
async fn main() {
    // ... existing pool setup ...

    // Sync verbs to DB for agent RAG discovery
    println!("Syncing verb definitions to database...");
    let verb_config_path = std::env::var("VERB_CONFIG_PATH")
        .unwrap_or_else(|_| "config/verbs".to_string());
    
    match RuntimeVerbRegistry::from_config(&verb_config_path) {
        Ok(registry) => {
            let sync_service = VerbSyncService::new(pool.clone());
            match sync_service.sync_all(&registry).await {
                Ok(result) => {
                    println!(
                        "Verb sync complete: {} added, {} updated, {} unchanged, {} removed ({}ms)",
                        result.verbs_added,
                        result.verbs_updated,
                        result.verbs_unchanged,
                        result.verbs_removed,
                        result.duration_ms
                    );
                }
                Err(e) => {
                    eprintln!("Warning: Verb sync failed: {}. Agent discovery may be limited.", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to load verb registry: {}. Agent discovery disabled.", e);
        }
    }

    // ... rest of main (state, cors, router, etc.) ...
}
```

## Task 2: Populate RAG Metadata for Core Verbs

Create a new file: `/Users/adamtc007/Developer/ob-poc/rust/src/session/verb_rag_metadata.rs`

This provides intent patterns, workflow phases, and graph contexts for agent discovery:

```rust
//! Verb RAG Metadata
//!
//! Provides semantic metadata for verb discovery:
//! - intent_patterns: Natural language patterns that map to verbs
//! - workflow_phases: KYC lifecycle phases where verb is applicable
//! - graph_contexts: Graph UI contexts where verb is relevant
//! - typical_next: Common follow-up verbs in workflows

use std::collections::HashMap;

/// Intent patterns for natural language → verb matching
pub fn get_intent_patterns() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    
    // === CBU VERBS ===
    m.insert("cbu.create", vec![
        "create cbu", "new cbu", "add client", "onboard client", 
        "create client business unit", "new client"
    ]);
    m.insert("cbu.ensure", vec![
        "ensure cbu exists", "upsert cbu", "create or update cbu"
    ]);
    m.insert("cbu.assign-role", vec![
        "assign role", "add role", "give role", "set role",
        "make them", "appoint as"
    ]);
    m.insert("cbu.remove-role", vec![
        "remove role", "revoke role", "unassign role", "take away role"
    ]);
    m.insert("cbu.show", vec![
        "show cbu", "display cbu", "view cbu", "cbu details"
    ]);
    m.insert("cbu.parties", vec![
        "list parties", "show parties", "who is involved", "all entities"
    ]);
    
    // === ENTITY VERBS ===
    m.insert("entity.create-limited-company", vec![
        "create company", "new company", "add company", "create entity",
        "create ltd", "create limited company", "new legal entity"
    ]);
    m.insert("entity.ensure-limited-company", vec![
        "ensure company", "upsert company", "create or update company"
    ]);
    m.insert("entity.create-proper-person", vec![
        "create person", "add person", "new individual", "add individual",
        "create natural person"
    ]);
    m.insert("entity.ensure-proper-person", vec![
        "ensure person", "upsert person", "create or update person"
    ]);
    m.insert("entity.create-trust-discretionary", vec![
        "create trust", "new trust", "add trust", "discretionary trust"
    ]);
    m.insert("entity.create-partnership-limited", vec![
        "create partnership", "new lp", "add limited partnership", "create lp"
    ]);
    
    // === FUND VERBS ===
    m.insert("fund.create-umbrella", vec![
        "create umbrella", "new sicav", "create sicav", "new icav",
        "create fund umbrella", "umbrella fund"
    ]);
    m.insert("fund.ensure-umbrella", vec![
        "ensure umbrella", "upsert umbrella", "ensure sicav exists"
    ]);
    m.insert("fund.create-subfund", vec![
        "create subfund", "new subfund", "add compartment", "create compartment",
        "new sub-fund"
    ]);
    m.insert("fund.ensure-subfund", vec![
        "ensure subfund", "upsert subfund", "ensure compartment"
    ]);
    m.insert("fund.create-share-class", vec![
        "create share class", "new share class", "add share class",
        "create isin", "new isin"
    ]);
    m.insert("fund.ensure-share-class", vec![
        "ensure share class", "upsert share class"
    ]);
    m.insert("fund.link-feeder", vec![
        "link feeder", "connect feeder to master", "feeder master relationship"
    ]);
    m.insert("fund.list-subfunds", vec![
        "list subfunds", "show compartments", "subfunds under umbrella"
    ]);
    m.insert("fund.list-share-classes", vec![
        "list share classes", "show share classes", "isins for fund"
    ]);
    
    // === UBO/OWNERSHIP VERBS ===
    m.insert("ubo.add-ownership", vec![
        "add owner", "add ownership", "owns", "shareholder of",
        "add shareholder", "ownership stake", "equity stake",
        "parent company", "holding company"
    ]);
    m.insert("ubo.update-ownership", vec![
        "update ownership", "change percentage", "modify stake"
    ]);
    m.insert("ubo.end-ownership", vec![
        "end ownership", "remove owner", "sold stake", "divested"
    ]);
    m.insert("ubo.list-owners", vec![
        "list owners", "who owns", "shareholders", "ownership chain up"
    ]);
    m.insert("ubo.list-owned", vec![
        "list owned", "subsidiaries", "what do they own", "ownership chain down"
    ]);
    m.insert("ubo.register-ubo", vec![
        "register ubo", "add beneficial owner", "ubo registration"
    ]);
    m.insert("ubo.mark-terminus", vec![
        "mark terminus", "end of chain", "public company", "no known person",
        "ubo terminus", "dispersed ownership", "listed company"
    ]);
    m.insert("ubo.calculate", vec![
        "calculate ubo", "ubo calculation", "beneficial ownership calculation",
        "who are the ubos", "25% threshold"
    ]);
    
    // === CONTROL VERBS ===
    m.insert("control.add", vec![
        "add control", "controls", "controlling person", "significant control"
    ]);
    m.insert("control.list-controllers", vec![
        "list controllers", "who controls", "controlling parties"
    ]);
    
    // === ROLE ASSIGNMENT (V2) ===
    m.insert("cbu.role:assign", vec![
        "assign role", "add role to cbu", "entity role"
    ]);
    m.insert("cbu.role:assign-ownership", vec![
        "assign ownership role", "shareholder role", "owner role"
    ]);
    m.insert("cbu.role:assign-control", vec![
        "assign control role", "director role", "officer role"
    ]);
    m.insert("cbu.role:assign-trust-role", vec![
        "assign trust role", "trustee", "settlor", "beneficiary", "protector"
    ]);
    m.insert("cbu.role:assign-fund-role", vec![
        "assign fund role", "management company", "manco", "investment manager"
    ]);
    m.insert("cbu.role:assign-service-provider", vec![
        "assign service provider", "depositary", "custodian", "auditor",
        "administrator", "transfer agent"
    ]);
    m.insert("cbu.role:assign-signatory", vec![
        "assign signatory", "authorized signatory", "authorized trader",
        "power of attorney"
    ]);
    
    // === GRAPH/NAVIGATION VERBS ===
    m.insert("graph.view", vec![
        "view graph", "show graph", "visualize", "display structure"
    ]);
    m.insert("graph.focus", vec![
        "focus on", "zoom to", "center on"
    ]);
    m.insert("graph.ancestors", vec![
        "show ancestors", "ownership chain up", "who owns this"
    ]);
    m.insert("graph.descendants", vec![
        "show descendants", "ownership chain down", "what do they own"
    ]);
    m.insert("graph.path", vec![
        "path between", "connection between", "how are they related"
    ]);
    
    // === KYC VERBS ===
    m.insert("kyc.case:create", vec![
        "create kyc case", "new kyc case", "start kyc", "open case"
    ]);
    m.insert("kyc.case:submit", vec![
        "submit case", "submit for review", "ready for review"
    ]);
    m.insert("kyc.case:approve", vec![
        "approve case", "approve kyc", "case approved"
    ]);
    m.insert("kyc.screening:run", vec![
        "run screening", "screen entity", "sanctions check", "pep check"
    ]);
    
    // === DOCUMENT VERBS ===
    m.insert("document.attach", vec![
        "attach document", "upload document", "add document", "attach file"
    ]);
    m.insert("document.request", vec![
        "request document", "ask for document", "doc request"
    ]);
    
    // === SERVICE/PRODUCT VERBS ===
    m.insert("service.list", vec![
        "list services", "available services", "what services"
    ]);
    m.insert("product.list", vec![
        "list products", "available products", "what products"
    ]);
    
    m
}

/// Workflow phases for lifecycle-aware suggestions
pub fn get_workflow_phases() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    
    // Entity collection phase
    m.insert("entity_collection", vec![
        "cbu.create", "cbu.ensure",
        "entity.create-limited-company", "entity.ensure-limited-company",
        "entity.create-proper-person", "entity.ensure-proper-person",
        "entity.create-trust-discretionary", "entity.create-partnership-limited",
        "fund.create-umbrella", "fund.ensure-umbrella",
        "fund.create-subfund", "fund.ensure-subfund",
        "fund.create-share-class", "fund.ensure-share-class",
    ]);
    
    // Structure building phase
    m.insert("structure_building", vec![
        "cbu.assign-role", "cbu.role:assign",
        "cbu.role:assign-ownership", "cbu.role:assign-control",
        "cbu.role:assign-trust-role", "cbu.role:assign-fund-role",
        "cbu.role:assign-service-provider", "cbu.role:assign-signatory",
        "ubo.add-ownership", "control.add",
        "fund.link-feeder",
    ]);
    
    // UBO discovery phase
    m.insert("ubo_discovery", vec![
        "ubo.add-ownership", "ubo.list-owners", "ubo.list-owned",
        "ubo.calculate", "ubo.register-ubo", "ubo.mark-terminus",
        "graph.ancestors", "graph.descendants",
    ]);
    
    // Document collection phase
    m.insert("document_collection", vec![
        "document.attach", "document.request",
        "cbu.attach-evidence", "cbu.verify-evidence",
    ]);
    
    // Screening phase
    m.insert("screening", vec![
        "kyc.screening:run", "kyc.case:create",
    ]);
    
    // Review phase
    m.insert("review", vec![
        "kyc.case:submit", "kyc.case:approve",
        "cbu.decide",
    ]);
    
    m
}

/// Graph contexts for UI-aware suggestions
pub fn get_graph_contexts() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    
    // When cursor is on a CBU node
    m.insert("cursor_on_cbu", vec![
        "cbu.show", "cbu.parties", "cbu.assign-role",
        "cbu.add-product", "kyc.case:create",
    ]);
    
    // When cursor is on an entity node
    m.insert("cursor_on_entity", vec![
        "entity.update", "cbu.role:assign",
        "ubo.add-ownership", "ubo.list-owners", "ubo.list-owned",
        "control.add", "graph.ancestors", "graph.descendants",
    ]);
    
    // When cursor is on a fund entity
    m.insert("cursor_on_fund", vec![
        "fund.list-subfunds", "fund.list-share-classes",
        "fund.create-subfund", "fund.create-share-class",
        "cbu.role:assign-fund-role",
    ]);
    
    // When viewing UBO layer
    m.insert("layer_ubo", vec![
        "ubo.add-ownership", "ubo.list-owners", "ubo.calculate",
        "ubo.register-ubo", "ubo.mark-terminus",
        "graph.ancestors",
    ]);
    
    // When viewing trading layer
    m.insert("layer_trading", vec![
        "cbu.role:assign-signatory", "cbu.role:assign-service-provider",
        "fund.list-share-classes",
    ]);
    
    // When viewing control layer
    m.insert("layer_control", vec![
        "control.add", "control.list-controllers",
        "cbu.role:assign-control",
    ]);
    
    m
}

/// Typical next verbs for workflow suggestions
pub fn get_typical_next() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    
    // After creating CBU
    m.insert("cbu.create", vec![
        "entity.create-limited-company",
        "cbu.assign-role",
        "fund.create-umbrella",
    ]);
    m.insert("cbu.ensure", vec![
        "entity.ensure-limited-company",
        "cbu.assign-role",
        "fund.ensure-umbrella",
    ]);
    
    // After creating entity
    m.insert("entity.create-limited-company", vec![
        "cbu.assign-role",
        "ubo.add-ownership",
    ]);
    m.insert("entity.create-proper-person", vec![
        "cbu.role:assign-control",
        "ubo.register-ubo",
    ]);
    
    // After creating umbrella
    m.insert("fund.create-umbrella", vec![
        "fund.create-subfund",
        "cbu.role:assign-fund-role",
    ]);
    m.insert("fund.ensure-umbrella", vec![
        "fund.ensure-subfund",
        "cbu.role:assign-fund-role",
    ]);
    
    // After creating subfund
    m.insert("fund.create-subfund", vec![
        "fund.create-share-class",
    ]);
    m.insert("fund.ensure-subfund", vec![
        "fund.ensure-share-class",
    ]);
    
    // After adding ownership
    m.insert("ubo.add-ownership", vec![
        "ubo.add-ownership",  // chain continues
        "ubo.mark-terminus",
        "ubo.calculate",
    ]);
    
    // After assigning role
    m.insert("cbu.assign-role", vec![
        "cbu.assign-role",  // more roles
        "ubo.add-ownership",
        "document.attach",
    ]);
    
    // After UBO calculation
    m.insert("ubo.calculate", vec![
        "ubo.register-ubo",
        "kyc.screening:run",
    ]);
    
    // After screening
    m.insert("kyc.screening:run", vec![
        "kyc.case:create",
        "document.request",
    ]);
    
    m
}
```

## Task 3: Update VerbSyncService to Include RAG Metadata

File: `/Users/adamtc007/Developer/ob-poc/rust/src/session/verb_sync.rs`

Add a method to populate RAG metadata after sync:

```rust
use super::verb_rag_metadata::{
    get_intent_patterns, get_workflow_phases, get_graph_contexts, get_typical_next
};

impl VerbSyncService {
    // ... existing methods ...

    /// Populate RAG metadata for verbs (intent_patterns, workflow_phases, etc.)
    pub async fn populate_rag_metadata(&self) -> Result<i32, VerbSyncError> {
        let mut updated = 0i32;
        
        // Update intent patterns
        let intent_patterns = get_intent_patterns();
        for (verb, patterns) in intent_patterns {
            let result = sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_verbs
                SET intent_patterns = $1,
                    updated_at = NOW()
                WHERE full_name = $2
                "#
            )
            .bind(&patterns)
            .bind(verb)
            .execute(&self.pool)
            .await?;
            
            if result.rows_affected() > 0 {
                updated += 1;
            }
        }
        
        // Update workflow phases (reverse mapping: phase -> verbs)
        let workflow_phases = get_workflow_phases();
        for (phase, verbs) in workflow_phases {
            for verb in verbs {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".dsl_verbs
                    SET workflow_phases = array_append(
                        COALESCE(workflow_phases, ARRAY[]::text[]),
                        $1
                    ),
                    updated_at = NOW()
                    WHERE full_name = $2
                      AND NOT ($1 = ANY(COALESCE(workflow_phases, ARRAY[]::text[])))
                    "#
                )
                .bind(phase)
                .bind(verb)
                .execute(&self.pool)
                .await?;
            }
        }
        
        // Update graph contexts (reverse mapping)
        let graph_contexts = get_graph_contexts();
        for (context, verbs) in graph_contexts {
            for verb in verbs {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".dsl_verbs
                    SET graph_contexts = array_append(
                        COALESCE(graph_contexts, ARRAY[]::text[]),
                        $1
                    ),
                    updated_at = NOW()
                    WHERE full_name = $2
                      AND NOT ($1 = ANY(COALESCE(graph_contexts, ARRAY[]::text[])))
                    "#
                )
                .bind(context)
                .bind(verb)
                .execute(&self.pool)
                .await?;
            }
        }
        
        // Update typical_next
        let typical_next = get_typical_next();
        for (verb, next_verbs) in typical_next {
            sqlx::query(
                r#"
                UPDATE "ob-poc".dsl_verbs
                SET typical_next = $1,
                    updated_at = NOW()
                WHERE full_name = $2
                "#
            )
            .bind(&next_verbs)
            .bind(verb)
            .execute(&self.pool)
            .await?;
        }
        
        // Regenerate search_text from description + intent_patterns
        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_verbs
            SET search_text = CONCAT(
                COALESCE(full_name, ''), ' ',
                COALESCE(description, ''), ' ',
                COALESCE(array_to_string(intent_patterns, ' '), '')
            ),
            updated_at = NOW()
            "#
        )
        .execute(&self.pool)
        .await?;
        
        Ok(updated)
    }
}
```

## Task 4: Update mod.rs to Export New Module

File: `/Users/adamtc007/Developer/ob-poc/rust/src/session/mod.rs`

Add:
```rust
pub mod verb_rag_metadata;
```

## Task 5: Update Startup to Include RAG Metadata Population

Back in `dsl_api.rs` main(), extend the sync block:

```rust
match sync_service.sync_all(&registry).await {
    Ok(result) => {
        println!(
            "Verb sync: {} added, {} updated, {} unchanged ({}ms)",
            result.verbs_added, result.verbs_updated, 
            result.verbs_unchanged, result.duration_ms
        );
        
        // Populate RAG metadata
        match sync_service.populate_rag_metadata().await {
            Ok(count) => println!("RAG metadata populated for {} verbs", count),
            Err(e) => eprintln!("Warning: RAG metadata population failed: {}", e),
        }
    }
    Err(e) => eprintln!("Warning: Verb sync failed: {}", e),
}
```

## Verification

After implementing, restart the API and verify:

```sql
-- Should return 150+ verbs
SELECT COUNT(*) FROM "ob-poc".dsl_verbs;

-- Check intent patterns populated
SELECT full_name, intent_patterns 
FROM "ob-poc".dsl_verbs 
WHERE intent_patterns IS NOT NULL 
LIMIT 10;

-- Test RAG search
SELECT full_name, description, 
       ts_rank(to_tsvector('english', search_text), plainto_tsquery('english', 'add owner')) as rank
FROM "ob-poc".dsl_verbs
WHERE to_tsvector('english', search_text) @@ plainto_tsquery('english', 'add owner')
ORDER BY rank DESC
LIMIT 5;

-- Check workflow phases
SELECT full_name, workflow_phases 
FROM "ob-poc".dsl_verbs 
WHERE 'ubo_discovery' = ANY(workflow_phases);

-- Check typical_next
SELECT full_name, typical_next 
FROM "ob-poc".dsl_verbs 
WHERE typical_next IS NOT NULL 
LIMIT 10;
```

## Summary

This implementation:
1. Syncs all verb definitions from YAML to `dsl_verbs` on API startup
2. Populates intent patterns for natural language → verb matching
3. Adds workflow phase metadata for lifecycle-aware suggestions
4. Adds graph context metadata for UI-aware suggestions  
5. Adds typical_next for workflow continuity
6. Generates search_text for full-text search

The agent can now RAG discover verbs based on:
- User intent: "add an owner" → `ubo.add-ownership`
- Workflow phase: "ubo_discovery" → ownership verbs
- Graph context: "cursor_on_fund" → fund management verbs
- Recent history: after `fund.create-umbrella` → suggest `fund.create-subfund`
