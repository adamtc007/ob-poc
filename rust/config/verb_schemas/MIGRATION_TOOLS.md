# Verb Schema V2 Migration Tools

## Strategy: Templates + xtask, Not Hand-Editing

You won't hand-author 935 schemas. You define ~15 domain templates and apply them by rule.

---

## xtask Commands

```bash
# Migrate all V1 YAMLs to V2 format
cargo xtask verb migrate-v2

# Lint all verb schemas (CI gate)
cargo xtask verb lint

# Build compiled VerbRegistry artifact
cargo xtask verb build-registry

# Format DSL files to canonical form
cargo xtask dsl fmt
```

---

## Domain Templates

### Template: `view.*` (navigation)
```yaml
_template: view
args:
  style: keyworded
  optional:
    entity: { type: entity_name }
    kind:   { type: enum, values: [fund, company, person, trust, partnership] }
    mode:   { type: enum, values: [ownership, services, documents] }
    depth:  { type: int, default: 1 }
positional_sugar: [entity]
tier: intent
tags: [navigation, view]
```

### Template: `session.*`
```yaml
_template: session
args:
  style: keyworded
  required:
    target: { type: entity_name }
  optional:
    kind:  { type: enum, values: [client-group, cbu, entity] }
    scope: { type: enum, values: [current, all] }
positional_sugar: [target]
tier: intent
tags: [session]
```

### Template: `ownership.*`
```yaml
_template: ownership
args:
  style: keyworded
  required:
    entity: { type: entity_ref }
  optional:
    mode:             { type: enum, values: [control, economic, both], default: both }
    depth:            { type: int, default: 10 }
    include-indirect: { type: bool, default: true }
    as-of:            { type: date }
positional_sugar: [entity]
tier: intent
tags: [ownership]
```

### Template: `ubo.*`
```yaml
_template: ubo
args:
  style: keyworded
  required:
    entity: { type: entity_ref }
  optional:
    threshold:       { type: decimal, default: "25.0" }
    include-control: { type: bool, default: true }
    max-depth:       { type: int, default: 10 }
positional_sugar: [entity]
tier: intent
tags: [ubo, ownership]
```

### Template: `fund.*`
```yaml
_template: fund
args:
  style: keyworded
  required:
    name: { type: str }
  optional:
    jurisdiction: { type: str }
    fund-type:    { type: enum, values: [SICAV, ICAV, OEIC, VCC, UCITS, hedge, pe, re, vc, fof] }
    lei:          { type: lei }
positional_sugar: [name, jurisdiction]
tier: crud
tags: [fund]
```

### Template: `entity.*` (CRUD)
```yaml
_template: entity
args:
  style: keyworded
  required:
    entity: { type: entity_ref }
  optional:
    name:         { type: str }
    jurisdiction: { type: str }
    entity-type:  { type: enum, values: [company, trust, partnership, person] }
positional_sugar: [entity]
tier: crud
tags: [entity]
```

### Template: `control.*`
```yaml
_template: control
args:
  style: keyworded
  required:
    controller: { type: entity_ref }
    controlled: { type: entity_ref }
  optional:
    control-type: { type: enum, values: [voting, board, management, veto, reserved-matters] }
    percentage:   { type: decimal }
positional_sugar: []
tier: crud
tags: [control, ownership]
```

### Template: `client.*`
```yaml
_template: client
args:
  style: keyworded
  required:
    client: { type: entity_ref, kinds: [client] }
  optional:
    status:  { type: enum, values: [active, prospect, dormant, terminated] }
    segment: { type: str }
positional_sugar: [client]
tier: crud
tags: [client]
```

### Template: `registry.*` (investor register)
```yaml
_template: registry
args:
  style: keyworded
  required:
    fund: { type: entity_ref, kinds: [fund] }
  optional:
    investor: { type: entity_ref }
    as-of:    { type: date }
    limit:    { type: int, default: 100 }
    offset:   { type: int, default: 0 }
positional_sugar: [fund]
tier: reference
tags: [registry, investor]
```

### Template: `kyc.*`
```yaml
_template: kyc
args:
  style: keyworded
  required:
    entity: { type: entity_ref }
  optional:
    case-id:   { type: uuid }
    status:    { type: enum, values: [pending, in-progress, approved, rejected] }
    due-date:  { type: date }
positional_sugar: [entity]
tier: intent
tags: [kyc]
```

### Template: `document.*`
```yaml
_template: document
args:
  style: keyworded
  required:
    entity: { type: entity_ref }
  optional:
    doc-type:  { type: str }
    doc-id:    { type: uuid }
    file-path: { type: str }
positional_sugar: [entity]
tier: crud
tags: [document]
```

### Template: `list-*` (list/query verbs)
```yaml
_template: list
args:
  style: keyworded
  optional:
    entity: { type: entity_ref }
    filter: { type: str }
    limit:  { type: int, default: 100 }
    offset: { type: int, default: 0 }
    sort:   { type: enum, values: [name, date, status] }
positional_sugar: [entity]
tier: reference
tags: [list, query]
```

### Template: `compute-*` / `calculate-*`
```yaml
_template: compute
args:
  style: keyworded
  required:
    entity: { type: entity_ref }
  optional:
    as-of:  { type: date }
    force:  { type: bool, default: false }
positional_sugar: [entity]
tier: intent
tags: [compute]
```

### Template: `default` (fallback)
```yaml
_template: default
args:
  style: keyworded
  optional:
    entity: { type: entity_ref }
positional_sugar: []
tier: crud
tags: []
```

---

## Template Selection Rules

```rust
fn select_template(verb_fqn: &str) -> &'static str {
    let domain = verb_fqn.split('.').next().unwrap_or("");
    let action = verb_fqn.split('.').last().unwrap_or("");
    
    // Domain-based
    match domain {
        "view" => return "view",
        "session" => return "session",
        "ownership" => return "ownership",
        "ubo" => return "ubo",
        "fund" => return "fund",
        "entity" => return "entity",
        "control" => return "control",
        "client" => return "client",
        "registry" => return "registry",
        "kyc" => return "kyc",
        "document" => return "document",
        _ => {}
    }
    
    // Action-based patterns
    if action.starts_with("list-") || action.starts_with("get-") {
        return "list";
    }
    if action.starts_with("compute-") || action.starts_with("calculate-") {
        return "compute";
    }
    if action.starts_with("create-") || action.starts_with("add-") {
        // Check if fund-related
        if action.contains("fund") || action.contains("umbrella") || action.contains("subfund") {
            return "fund";
        }
    }
    
    "default"
}
```

---

## Invocation Phrase Generation (Deterministic)

### Synonym Dictionaries

```yaml
# Verb synonyms
drill: [dive, expand, zoom-in, open, enter]
surface: [back, up, zoom-out, parent, exit]
trace: [follow, track, path, chain]
list: [show, display, view, get-all, enumerate]
create: [add, new, make, register]
update: [edit, modify, change, set]
delete: [remove, drop, terminate]
compute: [calculate, derive, run]
load: [open, switch, select]

# Domain nouns
ownership: [owners, stake, holding]
ubo: [beneficial owner, ultimate owner]
fund: [investment, vehicle, sicav]
entity: [company, person, trust]
```

### Phrase Templates

```rust
fn generate_phrases(verb_fqn: &str, domain: &str, action: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    
    // Pattern 1: "{action} {domain_noun}"
    // "drill entity", "list owners", "create fund"
    phrases.push(format!("{} {}", action, domain_noun(domain)));
    
    // Pattern 2: "{action} into/for/of {domain_noun}"
    // "drill into entity", "list of owners"
    phrases.push(format!("{} into {}", action, domain_noun(domain)));
    phrases.push(format!("{} for {}", action, domain_noun(domain)));
    
    // Pattern 3: "{verb_synonym} {domain_noun}"
    for syn in verb_synonyms(action) {
        phrases.push(format!("{} {}", syn, domain_noun(domain)));
    }
    
    // Pattern 4: "show/view {domain_noun} {action}"
    // "show ownership chain", "view fund structure"
    phrases.push(format!("show {} {}", domain_noun(domain), action));
    
    // Dedupe and enforce minimum
    phrases.sort();
    phrases.dedup();
    phrases.truncate(8);  // max 8 per verb
    
    assert!(phrases.len() >= 3, "Verb {} has < 3 phrases", verb_fqn);
    phrases
}
```

---

## Lint Rules (CI Gate)

```rust
// cargo xtask verb lint

fn lint_verb(spec: &VerbSpec) -> Vec<LintError> {
    let mut errors = Vec::new();
    
    // 1. Must have invocation_phrases
    if spec.invocation_phrases.is_empty() {
        errors.push(LintError::MissingPhrases(spec.verb.clone()));
    }
    
    // 2. Phrase count >= 3
    if spec.invocation_phrases.len() < 3 {
        errors.push(LintError::TooFewPhrases(spec.verb.clone(), spec.invocation_phrases.len()));
    }
    
    // 3. No 1-word phrases (unless whitelisted)
    const SINGLE_WORD_WHITELIST: &[&str] = &["back", "up", "out", "drill", "surface"];
    for phrase in &spec.invocation_phrases {
        let words: Vec<_> = phrase.split_whitespace().collect();
        if words.len() == 1 && !SINGLE_WORD_WHITELIST.contains(&words[0]) {
            errors.push(LintError::SingleWordPhrase(spec.verb.clone(), phrase.clone()));
        }
    }
    
    // 4. Must have at least one example
    if spec.examples.is_empty() {
        errors.push(LintError::MissingExamples(spec.verb.clone()));
    }
    
    // 5. Examples must parse
    for example in &spec.examples {
        if !is_valid_sexpr(example) {
            errors.push(LintError::InvalidExample(spec.verb.clone(), example.clone()));
        }
    }
    
    // 6. No alias collisions (global check)
    // Done at registry level, not per-verb
    
    // 7. Args required for CRUD verbs
    if is_crud_action(&spec.action) && spec.args.required.is_empty() {
        errors.push(LintError::CrudMissingRequired(spec.verb.clone()));
    }
    
    // 8. positional_sugar max 2
    if spec.positional_sugar.len() > 2 {
        errors.push(LintError::TooManyPositional(spec.verb.clone(), spec.positional_sugar.len()));
    }
    
    errors
}

fn lint_registry(registry: &VerbRegistry) -> Vec<LintError> {
    let mut errors = Vec::new();
    let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();
    
    for spec in registry.all() {
        // Collect aliases
        for alias in &spec.aliases {
            alias_map.entry(alias.clone())
                .or_default()
                .push(spec.verb.clone());
        }
    }
    
    // Check for collisions
    for (alias, verbs) in &alias_map {
        if verbs.len() > 1 {
            errors.push(LintError::AliasCollision(alias.clone(), verbs.clone()));
        }
    }
    
    errors
}
```

---

## Migration Script (`xtask/src/verb_migrate.rs`)

```rust
pub fn migrate_v2(verbs_dir: &Path, schemas_dir: &Path) -> Result<()> {
    // Load templates
    let templates = load_templates()?;
    
    // Load synonym dictionaries
    let verb_synonyms = load_verb_synonyms()?;
    let domain_nouns = load_domain_nouns()?;
    
    // Process each YAML file
    for entry in WalkDir::new(verbs_dir) {
        let path = entry?.path();
        if path.extension() != Some("yaml") || path.file_name().starts_with("_") {
            continue;
        }
        
        let v1_verbs = parse_v1_yaml(&path)?;
        let mut v2_specs = Vec::new();
        
        for v1 in v1_verbs {
            let domain = extract_domain(&v1.fqn);
            let action = extract_action(&v1.fqn);
            let template_name = select_template(&v1.fqn);
            let template = templates.get(template_name).unwrap();
            
            // Build V2 spec
            let v2 = VerbSpec {
                verb: v1.fqn.clone(),
                domain: domain.clone(),
                action: action.clone(),
                
                // Aliases from template + synonyms
                aliases: generate_aliases(&action, &verb_synonyms),
                
                // Args from template, with overrides from V1
                args: merge_args(&template.args, &v1.args),
                
                // Positional sugar from template
                positional_sugar: template.positional_sugar.clone(),
                
                // Generate invocation phrases
                invocation_phrases: generate_phrases(&v1.fqn, &domain, &action, &verb_synonyms, &domain_nouns),
                
                // Generate example
                examples: generate_examples(&v1.fqn, &v2.args),
                
                // Metadata
                doc: v1.description.unwrap_or_default(),
                tier: template.tier.clone(),
                tags: template.tags.clone(),
            };
            
            v2_specs.push(v2);
        }
        
        // Write V2 YAML
        let output_path = compute_output_path(&path, schemas_dir);
        write_v2_yaml(&v2_specs, &output_path)?;
    }
    
    Ok(())
}
```

---

## Build Registry (`xtask/src/verb_build.rs`)

```rust
pub fn build_registry(schemas_dir: &Path, output_path: &Path) -> Result<()> {
    let mut registry = VerbRegistry::new();
    
    // Load all V2 schemas
    for entry in WalkDir::new(schemas_dir) {
        let path = entry?.path();
        if path.extension() != Some("yaml") || path.file_name().starts_with("_") {
            continue;
        }
        
        let specs = parse_v2_yaml(&path)?;
        for spec in specs {
            registry.register(spec)?;
        }
    }
    
    // Run global lint
    let errors = lint_registry(&registry);
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("LINT ERROR: {:?}", e);
        }
        return Err(anyhow!("Registry lint failed with {} errors", errors.len()));
    }
    
    // Write compiled registry
    // Option A: JSON for runtime loading
    let json = serde_json::to_string_pretty(&registry)?;
    std::fs::write(output_path.with_extension("json"), &json)?;
    
    // Option B: Rust module for compile-time embedding
    let rust_code = generate_registry_module(&registry);
    std::fs::write(output_path.with_extension("rs"), &rust_code)?;
    
    println!("Built registry: {} verbs, {} aliases", 
        registry.verb_count(), 
        registry.alias_count()
    );
    
    Ok(())
}
```

---

## Usage

```bash
# Step 1: Migrate all V1 → V2
cargo xtask verb migrate-v2

# Step 2: Review diffs (git diff)
git diff config/verb_schemas/

# Step 3: Lint (should pass)
cargo xtask verb lint

# Step 4: Build registry
cargo xtask verb build-registry

# Step 5: Run tests
cargo test --package ob-poc schema
```
