# Claude Code Task: COMPLETE Schema Cleanup

## Mandate

**FULL CLEANUP. ALL DOMAINS. ALL VERBS. NO EXCEPTIONS.**

We are not in production. Now is the time.

---

## Schema Format Reference

**USE THIS FORMAT FOR ALL GENERATED SCHEMAS:**

```
/Users/adamtc007/Developer/ob-poc/rust/config/verb_schemas/SCHEMA_FORMAT_V2.md
```

**USE THESE TOOLS FOR MIGRATION:**

```
/Users/adamtc007/Developer/ob-poc/rust/config/verb_schemas/MIGRATION_TOOLS.md
```

Key points:
- Don't hand-edit 935 schemas - use domain templates + xtask migration
- 15 domain templates cover all verb patterns
- Deterministic phrase generation (synonym dictionaries, not LLM)
- Lint rules as CI gate (hard fail on missing phrases, collisions, etc.)

Every verb schema MUST follow the V2 format. Read both docs before generating any schemas.

---

## Scope

**Total: 935 verbs across 110 YAML files**

| Category | Files | Verbs | Priority |
|----------|-------|-------|----------|
| Core domains | 15 | 380 | P0 |
| KYC subdomain | 13 | 90 | P1 |
| Registry subdomain | 7 | 80 | P1 |
| Custody subdomain | 8 | 75 | P1 |
| Research subdomain | 5 | 25 | P2 |
| Reference subdomain | 4 | 20 | P2 |
| Observation subdomain | 3 | 15 | P2 |
| Templates | 13 | 30 | P2 |
| Admin/Refdata | 12 | 40 | P2 |
| Other | 30 | 180 | P2 |

---

## Complete File List (All 110 Files)

### Core Domains (P0 - Do First)
```
./client.yaml                    68 verbs
./trading-profile.yaml           36 verbs
./view.yaml                      25 verbs
./client-group.yaml              23 verbs
./ubo.yaml                       22 verbs
./team.yaml                      22 verbs
./fund.yaml                      20 verbs
./cbu.yaml                       19 verbs
./sla.yaml                       17 verbs
./entity.yaml                    17 verbs
./lifecycle.yaml                 16 verbs
./gleif.yaml                     16 verbs
./control.yaml                   15 verbs
./agent.yaml                     15 verbs
./ownership.yaml                 16 verbs
```

### KYC Subdomain (P1)
```
./kyc/kyc-case.yaml              10 verbs
./kyc/tollgate.yaml               9 verbs
./kyc/request.yaml                9 verbs
./kyc/entity-workstream.yaml      9 verbs
./kyc/capital.yaml                9 verbs
./kyc/board.yaml                  9 verbs
./kyc/trust.yaml                  8 verbs
./kyc/partnership.yaml            7 verbs
./kyc/case-screening.yaml         7 verbs
./kyc/case-event.yaml             6 verbs
./kyc/doc-request.yaml            5 verbs
./kyc/red-flag.yaml               4 verbs
```

### Registry Subdomain (P1)
```
./registry/investor.yaml         20 verbs
./registry/movement.yaml         14 verbs
./registry/fund-vehicle.yaml     14 verbs
./registry/investor-role.yaml    10 verbs
./registry/holding.yaml          10 verbs
./registry/share-class.yaml       7 verbs
./registry/economic-exposure.yaml 5 verbs
```

### Custody Subdomain (P1)
```
./custody/settlement-chain.yaml  13 verbs
./custody/trade-gateway.yaml     12 verbs
./custody/tax-config.yaml        11 verbs
./custody/corporate-action.yaml   9 verbs
./custody/cbu-custody.yaml        8 verbs
./custody/isda.yaml               7 verbs
./custody/instruction-profile.yaml 6 verbs
./custody/entity-settlement.yaml  5 verbs
```

### Other Core Files (P1)
```
./service-pipeline.yaml          14 verbs
./pricing-config.yaml            14 verbs
./matrix-overlay.yaml            14 verbs
./contract.yaml                  14 verbs
./session.yaml                   13 verbs
./document.yaml                  13 verbs
./identifier.yaml                11 verbs
./attribute.yaml                 11 verbs
./service-resource.yaml          10 verbs
./requirement.yaml               10 verbs
./cbu-role-v2.yaml               10 verbs
./graph.yaml                      9 verbs
./cash-sweep.yaml                 9 verbs
./bods.yaml                       9 verbs
./temporal.yaml                   8 verbs
./manco-group.yaml                7 verbs
./batch.yaml                      7 verbs
./investment-manager.yaml         7 verbs
./runbook.yaml                    7 verbs
./semantic.yaml                   6 verbs
./regulatory.yaml                 5 verbs
./delegation.yaml                 4 verbs
./kyc-agreement.yaml              4 verbs
./screening.yaml                  3 verbs
./service.yaml                    3 verbs
./delivery.yaml                   3 verbs
./product.yaml                    2 verbs
./template.yaml                   2 verbs
./onboarding.yaml                 1 verb
```

### Research Subdomain (P2)
```
./research/workflow.yaml          9 verbs
./research/outreach.yaml          8 verbs
./research/sources.yaml           5 verbs
./research/sec-edgar.yaml         3 verbs
./research/companies-house.yaml   2 verbs
```

### Observation Subdomain (P2)
```
./observation/observation.yaml    8 verbs
./observation/discrepancy.yaml    5 verbs
./observation/allegation.yaml     3 verbs
```

### Reference Subdomain (P2)
```
./reference/instrument-class.yaml 6 verbs
./reference/market.yaml           5 verbs
./reference/subcustodian.yaml     5 verbs
./reference/security-type.yaml    4 verbs
```

### Verification Subdomain (P2)
```
./verification/verify.yaml       16 verbs
```

### Refdata (P2)
```
./refdata/bulk-load.yaml          5 verbs
./refdata/jurisdiction.yaml       4 verbs
./refdata/currency.yaml           4 verbs
./refdata/role.yaml               4 verbs
./refdata/client-type.yaml        3 verbs
./refdata/case-type.yaml          3 verbs
./refdata/risk-rating.yaml        3 verbs
./refdata/screening-type.yaml     3 verbs
./refdata/settlement-type.yaml    3 verbs
./refdata/ssi-type.yaml           3 verbs
```

### Admin (P2)
```
./admin/role-types.yaml           3 verbs
./admin/regulators.yaml           3 verbs
```

### Templates (P2)
```
./templates/fund/onboard-fund-cbu.yaml           4 verbs
./templates/signatory/onboard-signatory.yaml     4 verbs
./templates/director/onboard-director.yaml       4 verbs
./templates/ubo/add-ownership.yaml               3 verbs
./templates/screening/review-screening-hit.yaml  3 verbs
./templates/case/escalate-case.yaml              2 verbs
./templates/case/create-kyc-case.yaml            2 verbs
./templates/documents/catalog-document.yaml      2 verbs
./templates/documents/request-documents.yaml     2 verbs
./templates/ubo/register-ubo.yaml                2 verbs
./templates/case/approve-case.yaml               1 verb
./templates/screening/run-entity-screening.yaml  1 verb
./templates/ubo/trace-chains.yaml                1 verb
```

---

## Execution Strategy

### Step 1: Build Schema Infrastructure (Day 1)

Create `rust/src/mcp/schema/`:
- `mod.rs` - Module exports
- `types.rs` - VerbSpec, ArgSchema, ArgShape, etc.
- `registry.rs` - Load and resolve verbs
- `tokenizer.rs` - S-expr tokenizer with spans
- `parser.rs` - Schema-guided parsing
- `canonicalizer.rs` - Normalize to keyword form
- `validator.rs` - Type checking
- `feedback.rs` - Diagnostics and completions

### Step 2: Generate ALL Schema Files (Day 1-2)

For EACH of the 110 YAML files:
1. Parse existing verb definitions
2. Extract arg specs (name, type, valid_values, default, description)
3. Generate VerbSpec with:
   - `aliases` from invocation_phrases + common synonyms
   - `args.required` / `args.optional` 
   - `positional_sugar` (max 2 args)
   - `keyword_aliases` for shortcuts
   - `doc` from description
   - `tier` from metadata

Output: 110 schema files in `rust/config/verb_schemas/`

### Step 3: Write Tests (Day 2)

For EVERY domain:
- Parse test (valid input parses)
- Alias test (aliases resolve)
- Positional sugar test (positionals → keywords)
- Missing arg test (error on missing required)
- Invalid enum test (error on bad enum value)
- Round-trip test (parse → canonical → print → parse)

### Step 4: Integration (Day 3)

Wire into `intent_router.rs`:
- S-expr input → schema parser → canonical → execute
- Short command → command normalizer → canonical
- NL input → semantic fallback → canonical

---

## Schema Generation Script

Create a script that reads existing YAML and generates schema:

```rust
// rust/src/bin/generate_schemas.rs

fn main() {
    let verbs_dir = Path::new("config/verbs");
    let schemas_dir = Path::new("config/verb_schemas");
    
    for entry in WalkDir::new(verbs_dir) {
        let path = entry.path();
        if path.extension() == Some("yaml") && !path.file_name().starts_with("_") {
            let verbs = parse_verb_yaml(path);
            let schemas = generate_schemas(&verbs);
            let output_path = compute_schema_path(path, schemas_dir);
            write_schema_yaml(&schemas, output_path);
        }
    }
}

fn generate_schemas(verbs: &[VerbDef]) -> Vec<VerbSpec> {
    verbs.iter().map(|v| {
        VerbSpec {
            name: v.fqn.clone(),
            aliases: generate_aliases(&v),
            args: ArgSchema {
                required: v.args.iter()
                    .filter(|a| a.required)
                    .map(|a| to_arg_def(a))
                    .collect(),
                optional: v.args.iter()
                    .filter(|a| !a.required)
                    .map(|a| to_arg_def(a))
                    .collect(),
            },
            positional_sugar: compute_positional_sugar(&v.args),
            keyword_aliases: HashMap::new(),
            doc: v.description.clone(),
            tier: v.metadata.tier.clone(),
            tags: v.metadata.tags.clone(),
        }
    }).collect()
}

fn to_arg_def(arg: &ArgDef) -> schema::ArgDef {
    schema::ArgDef {
        name: arg.name.clone(),
        shape: match arg.arg_type.as_str() {
            "entity" => ArgShape::EntityRef { 
                allowed_kinds: arg.entity_type.clone().map(|t| vec![t]).unwrap_or_default() 
            },
            "enum" => ArgShape::Enum { 
                values: arg.valid_values.clone().unwrap_or_default() 
            },
            "string" => ArgShape::Str,
            "integer" => ArgShape::Int,
            "boolean" => ArgShape::Bool,
            "uuid" => ArgShape::Uuid,
            "decimal" => ArgShape::Decimal,
            "date" => ArgShape::Date,
            "json" => ArgShape::Json,
            _ => ArgShape::Str,
        },
        default: arg.default.clone(),
        doc: arg.description.clone(),
        maps_to: arg.maps_to.clone(),
    }
}

fn compute_positional_sugar(args: &[ArgDef]) -> Vec<String> {
    let required: Vec<_> = args.iter().filter(|a| a.required).collect();
    
    match required.len() {
        0 => vec![],
        1 => vec![required[0].name.clone()],
        2 => vec![required[0].name.clone(), required[1].name.clone()],
        _ => vec![], // Too many required args - keyword-only
    }
}

fn generate_aliases(verb: &VerbDef) -> Vec<String> {
    let mut aliases = Vec::new();
    
    // From invocation_phrases
    if let Some(phrases) = &verb.invocation_phrases {
        for phrase in phrases {
            let words: Vec<_> = phrase.split_whitespace().collect();
            if words.len() == 1 {
                aliases.push(words[0].to_string());
            }
        }
    }
    
    // Common synonyms for the action
    let action = verb.fqn.split('.').last().unwrap_or("");
    match action {
        "create" => aliases.extend(["add", "new", "make"].iter().map(|s| s.to_string())),
        "list" => aliases.extend(["show", "get-all", "display"].iter().map(|s| s.to_string())),
        "get" => aliases.extend(["show", "fetch", "retrieve"].iter().map(|s| s.to_string())),
        "update" => aliases.extend(["edit", "modify", "change"].iter().map(|s| s.to_string())),
        "delete" => aliases.extend(["remove", "drop"].iter().map(|s| s.to_string())),
        _ => {}
    }
    
    aliases.sort();
    aliases.dedup();
    aliases
}
```

---

## Test Framework

```rust
// rust/src/mcp/schema/tests/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    fn registry() -> VerbRegistry {
        VerbRegistry::load(Path::new("config/verb_schemas")).unwrap()
    }
    
    // ============================================
    // COVERAGE TESTS
    // ============================================
    
    #[test]
    fn test_all_verbs_have_schemas() {
        let registry = registry();
        let yaml_verbs = load_all_yaml_verbs("config/verbs");
        
        let mut missing = Vec::new();
        for verb in &yaml_verbs {
            if registry.get(&verb.fqn).is_none() {
                missing.push(verb.fqn.clone());
            }
        }
        
        assert!(
            missing.is_empty(),
            "Missing schemas for {} verbs:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }
    
    #[test]
    fn test_schema_count_matches_yaml() {
        let registry = registry();
        let yaml_count = count_all_yaml_verbs("config/verbs");
        let schema_count = registry.len();
        
        assert_eq!(
            yaml_count, schema_count,
            "YAML has {} verbs but schemas have {}",
            yaml_count, schema_count
        );
    }
    
    // ============================================
    // ROUND-TRIP TESTS (for every verb)
    // ============================================
    
    #[test]
    fn test_round_trip_all_verbs() {
        let registry = registry();
        
        for spec in registry.all() {
            let cmd = generate_minimal_command(&spec);
            
            // Parse
            let parsed = parse(&cmd, &registry)
                .expect(&format!("Parse failed for {}: {}", spec.name, cmd));
            
            // Canonicalize
            let canonical = canonicalize(&parsed, &registry)
                .expect(&format!("Canonicalize failed for {}", spec.name));
            
            // Print
            let printed = to_sexpr(&canonical);
            
            // Re-parse
            let reparsed = parse(&printed, &registry)
                .expect(&format!("Re-parse failed for {}: {}", spec.name, printed));
            
            // Compare
            assert_eq!(
                canonical.verb, reparsed.verb,
                "Round-trip verb mismatch for {}",
                spec.name
            );
            assert_eq!(
                canonical.args.len(), reparsed.args.len(),
                "Round-trip arg count mismatch for {}",
                spec.name
            );
        }
    }
    
    // ============================================
    // PER-DOMAIN TESTS (generated)
    // ============================================
    
    macro_rules! domain_tests {
        ($domain:ident, $verbs:expr) => {
            mod $domain {
                use super::*;
                
                #[test]
                fn test_parse_all() {
                    let registry = registry();
                    for verb_name in $verbs {
                        let spec = registry.get(verb_name)
                            .expect(&format!("Missing schema: {}", verb_name));
                        let cmd = generate_minimal_command(&spec);
                        assert!(
                            parse(&cmd, &registry).is_ok(),
                            "Parse failed for {}: {}",
                            verb_name, cmd
                        );
                    }
                }
                
                #[test]
                fn test_aliases_resolve() {
                    let registry = registry();
                    for verb_name in $verbs {
                        let spec = registry.get(verb_name).unwrap();
                        for alias in &spec.aliases {
                            let resolution = registry.resolve_head(alias);
                            assert!(
                                matches!(resolution, HeadResolution::Alias { .. } | HeadResolution::Exact(_)),
                                "Alias '{}' for {} doesn't resolve",
                                alias, verb_name
                            );
                        }
                    }
                }
            }
        };
    }
    
    domain_tests!(view, &[
        "view.universe", "view.book", "view.cbu", "view.drill", "view.surface",
        "view.trace", "view.xray", "view.refine", "view.clear", "view.layout",
        // ... all 25 view verbs
    ]);
    
    domain_tests!(ownership, &[
        "ownership.compute", "ownership.who-controls", "ownership.trace-chain",
        // ... all 16 ownership verbs
    ]);
    
    // ... generate for all 44 domains
}
```

---

## Success Criteria

| Metric | Target | Measured By |
|--------|--------|-------------|
| Schema coverage | 935/935 verbs (100%) | `test_all_verbs_have_schemas` |
| Parse success | 100% valid inputs | `test_parse_all` per domain |
| Alias resolution | 100% aliases | `test_aliases_resolve` per domain |
| Round-trip | 100% verbs | `test_round_trip_all_verbs` |
| Test count | ≥ 1000 tests | `cargo test -- --list \| wc -l` |

---

## Deliverables Checklist

### Infrastructure
- [ ] `rust/src/mcp/schema/mod.rs`
- [ ] `rust/src/mcp/schema/types.rs`
- [ ] `rust/src/mcp/schema/registry.rs`
- [ ] `rust/src/mcp/schema/tokenizer.rs`
- [ ] `rust/src/mcp/schema/parser.rs`
- [ ] `rust/src/mcp/schema/canonicalizer.rs`
- [ ] `rust/src/mcp/schema/validator.rs`
- [ ] `rust/src/mcp/schema/feedback.rs`

### Schema Files (110 files)
- [ ] Core domains (15 files, 380 verbs)
- [ ] KYC subdomain (13 files, 90 verbs)
- [ ] Registry subdomain (7 files, 80 verbs)
- [ ] Custody subdomain (8 files, 75 verbs)
- [ ] Research subdomain (5 files, 25 verbs)
- [ ] Reference subdomain (4 files, 20 verbs)
- [ ] Observation subdomain (3 files, 15 verbs)
- [ ] Templates (13 files, 30 verbs)
- [ ] Admin/Refdata (12 files, 40 verbs)
- [ ] Remaining (30 files, 180 verbs)

### Tests
- [ ] Coverage tests
- [ ] Round-trip tests
- [ ] Per-domain parse tests
- [ ] Per-domain alias tests
- [ ] Error case tests

### Integration
- [ ] Hook into `intent_router.rs`
- [ ] S-expr → schema → execute path
- [ ] Update `_index.yaml` manifest

---

## Timeline

| Day | Deliverable |
|-----|-------------|
| Day 1 | Infrastructure + schema generator script |
| Day 1 | Generate all 110 schema files |
| Day 2 | Write and run all tests |
| Day 2 | Fix any failing tests |
| Day 3 | Integration with intent router |
| Day 3 | Final verification |

---

## Notes

- **No partial cleanup** - every verb gets a schema
- **No legacy exceptions** - templates, refdata, admin all included
- **Tests must pass** - no moving forward without green
- **Round-trip is king** - parse → canonical → print → parse must match
