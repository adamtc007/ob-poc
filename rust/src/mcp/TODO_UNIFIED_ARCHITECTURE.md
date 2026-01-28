# Unified Architecture: Operator Vocabulary + Intent Pipeline + Learning Loop

> **Status:** DRAFT - GPT reviewed, all feedback incorporated  
> **Date:** 2026-01-28  
> **Problem:** Intent matching at 80% (unacceptable). Implementation jargon leaks to ops. Learning loop broken.  
> **Solution:** Operator vocabulary layer (macro skin over DSL) + constraint cascade + DAG navigation + disambiguation feedback loop.

---

## Executive Summary

| Problem | Current | Target |
|---------|---------|--------|
| **Intent Matching** | Semantic search (80%) | Constraint cascade + phonetic (95%+) |
| **Confidently Wrong** | >0 | 0 (non-negotiable) |
| **Ops Vocabulary** | Implementation jargon (CBU, entity, trading-profile) | Business terms (structure, party, mandate) |
| **Learning Loop** | Broken (corrections not captured) | Disambiguation → selection → learned phrase |
| **Session State** | 5+ overlapping structs | Single `UnifiedSession` |
| **Navigation** | No history, no context flow | DAG nav + auto-fill + back/forward |

---

# Part 1: Core Insight

## Intent = Verb. Always.

```
"Open a KYC case for the Allianz PE structure"
   └── VERB ──┘         └────── NOUN ──────┘
   
Intent: case.open
Target: Allianz PE structure
```

Users are trained ops professionals. They say the verb. Our job:
1. **Match** their words to our verbs (fast, phonetic-tolerant)
2. **Show** what entities the verb needs (schema-driven)
3. **Scope** the entity search (context cascade)
4. **Execute**

**Not:** "Extract hidden intent with AI"  
**But:** "Match what they said to our vocabulary, fast"

---

# Part 2: Operator Vocabulary via Macros + Display Nouns

## The Problem: Implementation Jargon Leaks

DSL is 90% right. But some concepts are too technical:

| Implementation | Ops Has No Idea |
|----------------|-----------------|
| `cbu` | "What's a CBU?" |
| `cbu-role` | Graph edge abstraction |
| `entity` (generic) | Too abstract |
| `trading-profile` | Internal struct name |
| `entity_ref` | Type system artifact |

## The Solution: Generic Domains + Fund Type as Constraint

**Key insight:** Fund type as namespace (pe.*, sicav.*, hedge.*) explodes verb surface. Instead:

- **`structure.*`** - Generic structure operations
- **Fund type is a tag/constraint**, not a namespace

```
# WRONG: Duplicated verbs across fund types
pe.setup, sicav.setup, hedge.setup, etf.setup...
pe.assign-im, sicav.assign-im, hedge.assign-im...

# RIGHT: Single namespace, fund type as constraint
structure.setup :type pe
structure.setup :type sicav
structure.assign-role :role investment-manager
```

## Operator Domains (Collapsed)

```
structure.*  - All fund structure operations (wraps cbu.*)
party.*      - People/orgs in roles (wraps entity.* + cbu-role.*)
person.*     - Natural persons (wraps entity.* type=person)
company.*    - Corporates (wraps entity.* type=company)
case.*       - KYC cases (wraps kyc-case.*)
mandate.*    - Investment mandates (wraps trading-profile.*)
ownership.*  - UBO chains, control (wraps ownership.*, control.*)
document.*   - Document management
```

## Translation Table

| DSL (Implementation) | Operator (Macro) |
|---------------------|------------------|
| `cbu.create :kind private-equity` | `structure.setup :type pe` |
| `cbu.create :kind sicav` | `structure.setup :type sicav` |
| `cbu-role.assign :role general-partner` | `structure.assign-role :role gp` |
| `cbu-role.assign :role investment-manager` | `structure.assign-role :role im` |
| `trading-profile.create` | `mandate.create` |
| `kyc-case.create` | `case.open` |
| `entity.create :type person` | `person.add` |

**Critical:** Operator enum keys (`pe`, `gp`, `im`) never equal internal tokens (`private-equity`, `general-partner`, `investment-manager`). Macro expansions must always use `${arg.X.internal}` for enum args.

## Display Nouns (Mandatory)

**Rule: UI renders only operator labels, never internal IDs.**

| Internal Kind | Display Noun (UI) |
|---------------|-------------------|
| `cbu` | Structure / Client Unit |
| `entity` | Party |
| `entity_ref` | (hidden - never shown) |
| `trading-profile` | Mandate |
| `cbu-role` | Role |

---

# Part 3: Constraint Cascade + Scope Contract

## Client First - The Key Constraint

```
Without client context:
  "load the fund" → search ALL funds → thousands → hopeless

With client context (Allianz):
  "load the fund" → search Allianz funds → 12 matches → manageable
```

## The Cascade

```
┌─────────────────────────────────────────────────────────────────┐
│  1. CLIENT → Entities: 10,000 → 500                             │
│  2. STRUCTURE TYPE → Structures: 500 → 50                       │
│  3. VERB → Schema: needs entity of type [company]               │
│  4. ENTITY → Search ONLY companies within scope → 10 matches    │
└─────────────────────────────────────────────────────────────────┘
```

## Scope Contract (Explicit Rules)

| Scope Level | What It Constrains | Example |
|-------------|-------------------|---------|
| **Client** | All entities, structures, cases | Only Allianz entities visible |
| **Structure Type** | Structures (not parties) | Only PE structures in palette |
| **Current Structure** | Parties, cases, mandates | Only parties linked to this structure |
| **Verb Schema** | Entity kinds for args | `assign-role :role gp` → companies only |

---

# Part 4: Verb-First DAG Navigation

```
┌─────────────────────────────────────────────────────────────────┐
│  WORKFLOW: Onboarding PE Structure                              │
│  CLIENT: Allianz | TYPE: PE                                     │
│                                                                 │
│  ● structure.setup           ← START (ready)                    │
│    ├─► ○ structure.assign-role :role gp    (needs: setup)       │
│    ├─► ○ structure.assign-role :role im    (needs: setup)       │
│    └─► ○ case.open                         (needs: setup)       │
│          └─► ○ case.approve                (needs: submit)      │
│                └─► ○ mandate.create        (needs: KYC approved)│
│                                                                 │
│  ● = ready   ○ = blocked   ✓ = done                             │
└─────────────────────────────────────────────────────────────────┘
```

## DAG Readiness Rules

```rust
pub enum PrereqCondition {
    VerbCompleted(VerbFqn),
    AnyOf(Vec<VerbFqn>),
    StateExists { key: String },  // e.g., "structure.selected"
    FactExists { predicate: String },  // e.g., "case.documents.count >= 1"
}
```

**Rule:** `unlocks` is derived from (or validated against) `prereqs`. Don't let them drift.

## Canonical Prereq Keys

All prereq keys must use this exact naming convention (enforced by lint):

**State keys** (session state exists):
```
structure.selected      # A structure is currently selected
structure.exists        # At least one structure exists in scope
case.selected           # A case is currently selected  
case.exists             # At least one case exists for current structure
mandate.selected        # A mandate is currently selected
```

**Completion keys** (verb has been executed):
```
structure.created       # structure.setup completed
case.opened             # case.open completed
case.submitted          # case.submit completed
case.approved           # case.approve completed
mandate.created         # mandate.create completed
```

**Fact predicates** (RunSheet-derived, use sparingly):
```
case.documents.count >= 1
case.risk_rating != null
structure.roles.count >= 2
```

**Rule:** YAML `requires: [X]` must reference keys from the above lists. Lint validates all prereq references exist in the canonical key registry.

---

# Part 5: Macro Schema Format v1

## Purpose and Invariants

Macros are **PUBLIC verbs** shown in the operator palette.
They expand into **INTERNAL primitive DSL verbs**.

**Hard invariants:**
1. UI renders only `ui.*` and display nouns. Never render internal kinds/ids/tokens.
2. Macro expansion output is **UNTRUSTED** - must pass gatekeeper validation.
3. No DB reads during expansion (unless snapshotted in RunSheet).
4. Expansion is structured (`verb` + `args`), not raw s-expr strings.

## Schema Structure (Lintable)

```yaml
<verb-fqn>:
  kind: macro | primitive                 # REQUIRED

  ui:                                     # REQUIRED
    label: <string>                       # REQUIRED: palette label
    description: <string>                 # REQUIRED: one-liner (operator language)
    target_label: <string>                # REQUIRED: noun shown to ops

  routing:                                # REQUIRED
    mode_tags: [<tag>, ...]               # REQUIRED: palette filtering
    operator_domain: <string>             # OPTIONAL: grouping

  target:                                 # REQUIRED
    operates_on: <operator_type>          # REQUIRED: which picker opens
    produces: <operator_type|null>        # OPTIONAL: what gets created (for DAG)
    allowed_structure_types: [...]        # OPTIONAL constraint

  args:                                   # REQUIRED
    style: keyworded                      # REQUIRED
    required:
      <arg-name>:
        type: <type>                      # REQUIRED
        ui_label: <string>                # REQUIRED
        autofill_from: [<context_path>]   # OPTIONAL (recommended)
        picker: <picker_id>               # OPTIONAL (recommended for *_ref)
        default: <value>                  # OPTIONAL
        internal:                         # OPTIONAL (NEVER render in UI)
          kinds: [...]                    # INTERNAL filter only
          map: {...}                      # INTERNAL mapping
          validate: {...}                 # INTERNAL constraints
    optional: { ... }

  prereqs:                                # REQUIRED (can be empty [])
    - requires: [<prereq_key>, ...]       # e.g. structure.exists, case.approved
    - any_of: [<verb-fqn>, ...]           # OR logic
    - state: <state_key>                  # e.g. structure.selected

  expands_to:                             # REQUIRED
    - verb: <internal-verb-fqn>
      args:
        <arg>: "${arg.<n>}"            # macro arg
        <arg>: "${arg.<n>.internal}"   # enum's internal token
        <arg>: "${scope.<n>}"          # scope substitution
        <arg>: "${session.<path>}"        # session substitution

  unlocks: [<verb-fqn>, ...]              # OPTIONAL (recommended)
```

## Operator Types (UI-Safe Ref Types)

| Operator Type | Replaces | UI Shows |
|---------------|----------|----------|
| `structure_ref` | `entity_ref kinds:[cbu]` | "Structure" |
| `party_ref` | `entity_ref kinds:[person,company,trust]` | "Party" |
| `client_ref` | `entity_ref kinds:[client]` | "Client" |
| `case_ref` | `entity_ref kinds:[kyc-case]` | "Case" |
| `mandate_ref` | `entity_ref kinds:[trading-profile]` | "Mandate" |

**Rule:** Do not use raw `entity_ref` in operator macros.

## Enum Internal Mapping (Critical)

Enums must NOT assume operator keys match internal tokens.

```yaml
role:
  type: enum
  ui_label: "Role"
  values:
    - key: gp
      label: "General Partner"
      internal: general-partner
      internal_validate:
        allowed_structure_types: [pe, hedge]
    - key: manco
      label: "Management Company"
      internal: manco
      internal_validate:
        allowed_structure_types: [sicav]
    - key: im
      label: "Investment Manager"
      internal: investment-manager
  default_key: im
```

Then expansion uses `${arg.role.internal}`.

## Examples (Canonical, Mapped, UI-Safe)

### structure.setup

```yaml
structure.setup:
  kind: macro

  ui:
    label: "Set up Structure"
    description: "Create a structure in the current client scope"
    target_label: "Structure"

  routing:
    mode_tags: [onboarding, kyc]
    operator_domain: structure

  target:
    operates_on: client_ref
    produces: structure_ref

  args:
    style: keyworded
    required:
      structure_type:
        type: enum
        ui_label: "Type"
        values:
          - key: pe
            label: "Private Equity"
            internal: private-equity
          - key: sicav
            label: "SICAV"
            internal: sicav
          - key: hedge
            label: "Hedge Fund"
            internal: hedge
        default_key: pe
      name:
        type: str
        ui_label: "Structure name"
    optional: {}

  prereqs: []

  expands_to:
    - verb: cbu.create
      args:
        client_id: "${scope.client_id}"
        kind: "${arg.structure_type.internal}"
        name: "${arg.name}"

  unlocks: [structure.assign-role, case.open]
```

### structure.assign-role

```yaml
structure.assign-role:
  kind: macro

  ui:
    label: "Assign Role"
    description: "Link a party to the structure with a role"
    target_label: "Structure"

  routing:
    mode_tags: [onboarding, structure]
    operator_domain: structure

  target:
    operates_on: structure_ref
    produces: null

  args:
    style: keyworded
    required:
      role:
        type: enum
        ui_label: "Role"
        values:
          - key: gp
            label: "General Partner"
            internal: general-partner
            internal_validate:
              allowed_structure_types: [pe, hedge]
          - key: manco
            label: "Management Company"
            internal: manco
            internal_validate:
              allowed_structure_types: [sicav]
          - key: im
            label: "Investment Manager"
            internal: investment-manager
        default_key: im
      party:
        type: party_ref
        ui_label: "Party"
        picker: party_in_scope
        internal:
          kinds: [company, person]
    optional:
      structure:
        type: structure_ref
        ui_label: "Structure"
        autofill_from: [session.current_structure]

  prereqs:
    - requires: [structure.exists]

  expands_to:
    - verb: cbu-role.assign
      args:
        cbu_id: "${arg.structure}"
        entity_id: "${arg.party}"
        role: "${arg.role.internal}"
```

### case.open

```yaml
case.open:
  kind: macro

  ui:
    label: "Open KYC Case"
    description: "Create a KYC case for the selected structure"
    target_label: "Case"

  routing:
    mode_tags: [kyc, onboarding]
    operator_domain: case

  target:
    operates_on: structure_ref
    produces: case_ref

  args:
    style: keyworded
    required:
      structure:
        type: structure_ref
        ui_label: "For structure"
        autofill_from: [session.current_structure]
    optional:
      priority:
        type: enum
        ui_label: "Priority"
        values:
          - key: normal
            label: "Normal"
            internal: normal
          - key: urgent
            label: "Urgent"
            internal: urgent
        default_key: normal

  prereqs:
    - requires: [structure.exists]

  expands_to:
    - verb: kyc-case.create
      args:
        cbu_id: "${arg.structure}"
        priority: "${arg.priority.internal}"

  unlocks: [case.add-document, case.request-info]
```

## Macro Lint Specification

### Diagnostic Model

```rust
pub enum Severity { Error, Warn, Info }

pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub path: String,      // YAML path, e.g. "structure.setup.args.required.role"
    pub message: String,
    pub hint: Option<String>,
}
```

**Policy:**
- `Error` → fails build/CI
- `Warn` → printed but allowed
- `Info` → optional

### Lint Pass Structure

**Pass 1 — Schema-only lint (single YAML file)**
Checks field presence/types, forbidden leakage, variable grammar.

**Pass 2 — Cross-registry lint (requires verb registry)**
Validates `expands_to` verbs exist, args match internal schema, `unlocks`/`prereqs` references exist.

### Rule Set

#### Structure Rules

| Code | Severity | Rule |
|------|----------|------|
| `MACRO000` | Error | YAML parse error |
| `MACRO001` | Error | Top-level value must be a mapping of `<verb-fqn>` → spec |
| `MACRO002` | Error | Top-level keys must be strings (verb FQN) |
| `MACRO003` | Error | Each verb spec must be a mapping |
| `MACRO010` | Error | `kind` required and must be `macro` or `primitive` |

#### UI Rules (Macro Only)

| Code | Severity | Rule |
|------|----------|------|
| `MACRO011` | Error | `ui.label`, `ui.description`, `ui.target_label` all required (non-empty strings) |
| `MACRO012` | Error | UI text must not contain forbidden tokens (case-insensitive scan) |

**MACRO012 forbidden tokens:**
```
cbu, entity_ref, trading-profile, cbu_id, entity_id, cbu-id, 
uboprong, resolver, kyc-case (use "Case"), :kind (internal DSL syntax)
```

**MACRO012 implementation:**
```rust
fn contains_forbidden(ui_str: &str) -> Option<&'static str> {
    const FORBIDDEN: &[&str] = &[
        "cbu", "entity_ref", "trading-profile", "cbu_id", "entity_id", 
        "cbu-id", "uboprong", "resolver", "kyc-case", ":kind"
    ];
    let lower = ui_str.to_lowercase();
    FORBIDDEN.iter().find(|&t| lower.contains(t)).copied()
}
```

**Hint:** Move internal terms into `internal.*` fields; keep UI copy operator-facing.

#### Routing Rules

| Code | Severity | Rule |
|------|----------|------|
| `MACRO020` | Error | `routing.mode_tags` required, must be non-empty list |

#### Target Rules

| Code | Severity | Rule |
|------|----------|------|
| `MACRO030` | Error | `target.operates_on` required, must be one of: `client_ref`, `structure_ref`, `party_ref`, `case_ref`, `mandate_ref`, `document_ref` |
| `MACRO031` | Error | `target.produces` if present must be valid operator type or `null` |
| `MACRO032` | Error | `allowed_structure_types` values must be in: `pe`, `sicav`, `hedge`, `etf`, `pension`, `trust`, `fof` |

#### Args Rules

| Code | Severity | Rule |
|------|----------|------|
| `MACRO040` | Error | `args.style` must be `keyworded` |
| `MACRO041` | Error | All required args must have `type` + `ui_label` |
| `MACRO042` | Error | **No `entity_ref` type allowed** — use operator types (`structure_ref`, `party_ref`, etc.) |
| `MACRO043` | Error | **`kinds:` must only appear under `internal:` block** (prevents UI leakage) |
| `MACRO044` | Error | **Enum used in expansion must have internal mapping** and be referenced as `${arg.<name>.internal}` |
| `MACRO045` | Error | Optional args must also have `type` + `ui_label` |

**MACRO043 implementation:**
```rust
fn walk_for_kinds(value: &Value, path: &str, diags: &mut Vec<Diagnostic>) {
    if let Some(map) = value.as_mapping() {
        for (k, v) in map {
            if let Some(key) = k.as_str() {
                let new_path = format!("{}.{}", path, key);
                if key == "kinds" && !path.ends_with(".internal") {
                    diags.push(err("MACRO043", &new_path, 
                        "kinds may only be declared under <arg>.internal.kinds"));
                }
                walk_for_kinds(v, &new_path, diags);
            }
        }
    }
}
```

**MACRO044 rationale:** Prevents `${arg.role}` passing operator key `gp` when internal expects `general-partner`.

**MACRO044 implementation:**
```rust
fn check_enum_internal_mapping(
    spec: &serde_yaml::Mapping, 
    verb: &str,
    diags: &mut Vec<Diagnostic>
) -> HashSet<String> {
    let mut enum_args = HashSet::new();
    
    // Collect all enum args
    if let Some(args) = get_map(spec, "args") {
        for section in ["required", "optional"] {
            if let Some(section_map) = get_map(args, section) {
                for (k, v) in section_map {
                    let arg_name = k.as_str().unwrap_or("?");
                    if let Some(arg_spec) = v.as_mapping() {
                        if get_str(arg_spec, "type") == Some("enum") {
                            // Check if enum has internal mapping
                            let has_internal = if let Some(values) = get_seq(arg_spec, "values") {
                                values.iter().all(|v| {
                                    v.as_mapping()
                                        .map(|m| m.contains_key("internal"))
                                        .unwrap_or(false)
                                })
                            } else {
                                false
                            };
                            
                            if has_internal {
                                enum_args.insert(arg_name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Now check expands_to for violations
    if let Some(steps) = get_seq(spec, "expands_to") {
        for (i, step) in steps.iter().enumerate() {
            if let Some(args) = step.as_mapping().and_then(|m| get_map(m, "args")) {
                for (k, v) in args {
                    if let Some(val) = v.as_str() {
                        // Check for ${arg.X} where X is an enum but not using .internal
                        let var_regex = regex::Regex::new(r"\$\{arg\.(\w+)\}").unwrap();
                        for cap in var_regex.captures_iter(val) {
                            let arg_name = &cap[1];
                            if enum_args.contains(arg_name) && !val.contains(&format!("${{arg.{}.internal}}", arg_name)) {
                                diags.push(err("MACRO044", 
                                    &format!("{}.expands_to[{}].args.{}", verb, i, k.as_str().unwrap_or("?")),
                                    &format!("Enum arg '{}' must use ${{arg.{}.internal}} in expansion, not ${{arg.{}}}", 
                                             arg_name, arg_name, arg_name)));
                            }
                        }
                    }
                }
            }
        }
    }
    
    enum_args
}
```

#### Prereqs Rules

| Code | Severity | Rule |
|------|----------|------|
| `MACRO050` | Error | `prereqs` must exist and be a list (can be empty `[]`) |
| `MACRO051` | Error | Each prereq item must be one of: `{requires: [...]}`, `{any_of: [...]}`, `{state: ...}` |

#### Expansion Rules

| Code | Severity | Rule |
|------|----------|------|
| `MACRO060` | Error | `expands_to` required for macros, must be non-empty list |
| `MACRO061` | Error | Each step must be `{ verb: <string>, args: <map> }` |
| `MACRO062` | Error | No raw s-expr strings (starts with `(`, ends with `)`) |
| `MACRO063` | Error | Variable grammar: only `${arg.*}`, `${arg.*.internal}`, `${scope.*}`, `${session.*}` allowed |

**MACRO063 implementation:**
```rust
use regex::Regex;
use std::collections::HashSet;

fn validate_expansion_vars(
    args_value: &serde_yaml::Value,
    arg_names: &HashSet<String>,
    enum_args: &HashSet<String>,
    path: &str,
    diags: &mut Vec<Diagnostic>
) {
    let var_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();
    
    fn walk_args(v: &serde_yaml::Value, path: &str, regex: &Regex, 
                 arg_names: &HashSet<String>, enum_args: &HashSet<String>,
                 diags: &mut Vec<Diagnostic>) {
        match v {
            serde_yaml::Value::String(s) => {
                for cap in regex.captures_iter(s) {
                    let var_content = &cap[1];
                    if let Err(e) = validate_single_var(var_content, arg_names, enum_args) {
                        diags.push(err("MACRO063", path, &format!("Invalid expansion variable ${{{}}}: {}", var_content, e)));
                    }
                }
            }
            serde_yaml::Value::Mapping(m) => {
                for (k, v) in m {
                    let key = k.as_str().unwrap_or("?");
                    walk_args(v, &format!("{}.{}", path, key), regex, arg_names, enum_args, diags);
                }
            }
            _ => {}
        }
    }
    
    walk_args(args_value, path, &var_regex, arg_names, enum_args, diags);
}

fn validate_single_var(var: &str, arg_names: &HashSet<String>, enum_args: &HashSet<String>) -> Result<(), String> {
    let parts: Vec<&str> = var.split('.').collect();
    match parts.first() {
        Some(&"arg") => {
            let name = parts.get(1).ok_or("Missing arg name after 'arg.'")?;
            if !arg_names.contains(*name) {
                return Err(format!("Unknown arg: {}", name));
            }
            if parts.get(2) == Some(&"internal") && !enum_args.contains(*name) {
                return Err(format!("Arg '{}' is not an enum, cannot use .internal", name));
            }
            if parts.len() > 3 || (parts.len() == 3 && parts[2] != "internal") {
                return Err(format!("Invalid arg path: only .internal suffix allowed"));
            }
            Ok(())
        }
        Some(&"scope") => {
            if parts.len() < 2 {
                return Err("scope.* requires a field name".into());
            }
            Ok(())
        }
        Some(&"session") => {
            if parts.len() < 2 {
                return Err("session.* requires a path".into());
            }
            Ok(()) // Allow any path depth for session
        }
        Some(other) => Err(format!("Unknown variable root '{}'. Allowed: arg, scope, session", other)),
        None => Err("Empty variable".into())
    }
}
```

#### Cross-Registry Rules (Pass 2)

| Code | Severity | Rule |
|------|----------|------|
| `MACRO070` | Error | `unlocks` references must exist in macro registry |
| `MACRO071` | Error | `expands_to[*].verb` must exist in primitive registry |
| `MACRO072` | Warn | `expands_to[*].args` should match internal verb schema |

#### UX Friction Warnings

| Code | Severity | Rule |
|------|----------|------|
| `MACRO080a` | Warn | Missing `autofill_from` for `structure_ref`, `case_ref` args |
| `MACRO080b` | Warn | Missing `picker` for `*_ref` types |
| `MACRO080c` | Warn | `ui.description` too short (<12 chars) |

### Implementation Skeleton

```rust
pub fn lint_macro_file(
    yaml_text: &str, 
    primitive_registry: Option<&PrimitiveRegistry>
) -> Vec<Diagnostic> {
    let mut diags = vec![];

    // Parse YAML
    let doc: serde_yaml::Value = match serde_yaml::from_str(yaml_text) {
        Ok(v) => v,
        Err(e) => return vec![err("MACRO000", "$", &format!("YAML parse error: {e}"))],
    };

    // MACRO001: Top-level must be mapping
    let top = match doc.as_mapping() {
        Some(m) => m,
        None => {
            diags.push(err("MACRO001", "$", "Schema must be mapping of <verb-fqn> -> spec"));
            return diags;
        }
    };

    // Build verb name set for unlock checks
    let verb_names: HashSet<String> = top.iter()
        .filter_map(|(k, _)| k.as_str().map(String::from))
        .collect();

    for (k, v) in top.iter() {
        let verb = k.as_str().unwrap_or("?");
        let spec = match v.as_mapping() {
            Some(m) => m,
            None => { diags.push(err("MACRO003", verb, "Spec must be mapping")); continue; }
        };

        // MACRO010: kind
        let kind = get_str(spec, "kind").unwrap_or("");
        if kind != "macro" && kind != "primitive" {
            diags.push(err("MACRO010", verb, "kind must be 'macro' or 'primitive'"));
        }

        if kind == "macro" {
            // MACRO011: UI fields
            check_required_ui_fields(&mut diags, spec, verb);
            
            // MACRO012: Forbidden tokens
            check_forbidden_ui_tokens(&mut diags, spec, verb);
            
            // MACRO020: routing.mode_tags
            check_routing(&mut diags, spec, verb);
            
            // MACRO030-032: target
            check_target(&mut diags, spec, verb);
            
            // MACRO040-045: args
            let (arg_names, enum_args) = check_args(&mut diags, spec, verb);
            
            // MACRO050-051: prereqs
            check_prereqs(&mut diags, spec, verb);
            
            // MACRO060-063: expands_to
            check_expansion(&mut diags, spec, verb, &arg_names, &enum_args);
            
            // MACRO070: unlocks (local check)
            check_unlocks(&mut diags, spec, verb, &verb_names);
            
            // Pass 2: cross-registry checks
            if let Some(registry) = primitive_registry {
                check_expansion_verbs(&mut diags, spec, verb, registry);
            }
        }

        // MACRO043: Global scan for kinds outside internal
        walk_for_kinds(v, verb, &mut diags);
    }

    diags
}
```

### Helper Functions

```rust
fn err(code: &'static str, path: &str, message: &str) -> Diagnostic {
    Diagnostic { code, severity: Severity::Error, path: path.into(), message: message.into(), hint: None }
}

fn warn(code: &'static str, path: &str, message: &str) -> Diagnostic {
    Diagnostic { code, severity: Severity::Warn, path: path.into(), message: message.into(), hint: None }
}

fn get_str<'a>(map: &'a serde_yaml::Mapping, key: &str) -> Option<&'a str> {
    map.get(&serde_yaml::Value::String(key.into()))?.as_str()
}

fn get_map<'a>(map: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Mapping> {
    map.get(&serde_yaml::Value::String(key.into()))?.as_mapping()
}

fn get_seq<'a>(map: &'a serde_yaml::Mapping, key: &str) -> Option<&'a serde_yaml::Sequence> {
    map.get(&serde_yaml::Value::String(key.into()))?.as_sequence()
}

/// Check if spec has nested path like "ui.label"
fn get_nested_str<'a>(map: &'a serde_yaml::Mapping, path: &str) -> Option<&'a str> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = map;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return get_str(current, part);
        } else {
            current = get_map(current, part)?;
        }
    }
    None
}
```

### Server-Side Role Validation

**Do not rely on UI filtering alone.** The macro compiler gatekeeper must validate:
- `structure.assign-role :role gp` MUST fail if `session.structure_type` is SICAV
- Each role's `internal_validate.allowed_structure_types` is enforced server-side

### Ratcheting Strategy

1. **Start strict** on errors: `MACRO01x`, `MACRO02x`, `MACRO03x`, `MACRO04x`, `MACRO06x`, `MACRO063`, `MACRO071`
2. **Keep as warnings** until repo is clean: `MACRO080*`, `MACRO072`
3. **Ratchet up** warnings to errors once all macros pass

---

# Part 6: Disambiguation Feedback Loop

## The Rules

1. **Ambiguity threshold crossed → always show verb options as clickable buttons**
2. **Never ask open-ended "what do you mean?"**
3. **User selection = gold-standard label (confidence 0.95)**
4. **Record negative signals for rejected alternatives (soft, with decay)**
5. **Generate phrase variants (with guardrails)**

## Match Classification (Two-Axis: Score + Gap)

Ambiguity is about **gap between candidates**, not absolute score. Low score with clear winner ≠ ambiguous.

```
┌─────────────────────────────────────────────────────────────────┐
│  ACCEPT                                                         │
│  top >= 0.85 AND gap >= 0.05                                    │
│  → Execute immediately                                          │
├─────────────────────────────────────────────────────────────────┤
│  AMBIGUOUS                                                      │
│  top >= 0.60 AND gap < 0.05                                     │
│  → Show options (top candidates too close to call)              │
│  → Selection = learning signal                                  │
├─────────────────────────────────────────────────────────────────┤
│  SUGGEST                                                        │
│  top in [0.45..0.60)                                            │
│  → Show options ("best guess list", low confidence)             │
│  → Selection = learning signal                                  │
├─────────────────────────────────────────────────────────────────┤
│  NO_MATCH                                                       │
│  top < 0.45                                                     │
│  → Show palette filtered by mode + scope                        │
│  → Never open-ended "what do you mean?"                         │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** `gap >= 0.05` gates acceptance, not just top score. This keeps "confidently wrong = 0" aligned to gap gating.

```rust
pub enum MatchResult {
    Accept { verb: VerbFqn, score: f32 },
    Ambiguous { candidates: Vec<(VerbFqn, f32)> },  // gap < 0.05
    Suggest { candidates: Vec<(VerbFqn, f32)> },    // low confidence
    NoMatch { mode_filtered_verbs: Vec<VerbFqn> },
}

fn classify_match(candidates: &[(VerbFqn, f32)]) -> MatchResult {
    let top = candidates.first().map(|(_, s)| *s).unwrap_or(0.0);
    let second = candidates.get(1).map(|(_, s)| *s).unwrap_or(0.0);
    let gap = top - second;
    
    if top >= 0.85 && gap >= 0.05 {
        MatchResult::Accept { verb: candidates[0].0.clone(), score: top }
    } else if top >= 0.60 && gap < 0.05 {
        MatchResult::Ambiguous { candidates: candidates.to_vec() }
    } else if top >= 0.45 {
        MatchResult::Suggest { candidates: candidates.to_vec() }
    } else {
        MatchResult::NoMatch { mode_filtered_verbs: vec![] }  // filled by caller
    }
}
```

## Learning Guardrails

### Variant Generation Rules

```rust
fn generate_phrase_variants(phrase: &str) -> Vec<String> {
    // MAX 5 VARIANTS (prevent pollution)
    // Quality filters: Min 2 tokens, not generic alone
}
```

### Scope of Learning

| Level | When | Confidence Required |
|-------|------|---------------------|
| **User-scoped** | First confirmation | 0.85+ |
| **Team-scoped** | 3+ users confirm same mapping | 0.90+ |
| **Global** | 10+ confirmations across teams | 0.95+ |

### Negative Signal Decay

```rust
impl NegativeSignal {
    pub fn effective_weight(&self) -> f32 {
        let days_old = (Utc::now() - self.created_at).num_days();
        let decay = 0.95_f32.powi(days_old as i32);  // ~50% after 2 weeks
        self.weight * decay
    }
}
```

---

# Part 7: Unified Session State

```rust
pub struct UnifiedSession {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    
    // Context (constraint cascade)
    pub client: Option<ClientRef>,
    pub structure_type: Option<StructureType>,
    pub current_structure: Option<StructureRef>,
    pub current_case: Option<CaseRef>,
    
    // Persona (filters verbs)
    pub persona: Persona,
    
    // Navigation
    pub nav_path: Vec<NavNode>,
    pub state_stack: StateStack,
    
    // Execution
    pub run_sheet: RunSheet,
    pub dag_state: DagState,
}
```

---

# Part 8: Macro Expansion Pipeline

## Pipeline

```
1. Parse (macro heads allowed)
2. Expand macros → primitive-only DSL
3. Validate expanded plan (schema + scope + DAG) ← GATEKEEPER
4. Derive lock set from expanded steps (not macro heads)
5. Derive idempotency keys (execution_id + step_index + verb + normalized args)
6. Execute in transaction
7. Write RunSheet (audit: original + expanded + results)
```

**Critical invariants:**
- Lock set is derived from expanded primitive steps; macro heads don't lock.
- Every expanded primitive step gets an idempotency key; primitives must be ON CONFLICT safe.
- **No macro recursion:** Macros can only expand to primitives, never to other macros. This is an easy footgun once macros multiply.

## Robustness Rules

| Rule | Why |
|------|-----|
| Expansion is deterministic | Same input = same output, always |
| No DB reads during expansion | Snapshot inputs if unavoidable |
| Max expansion steps (e.g., 100) | Prevent runaway macros |
| No macro recursion | Primitives only in expansion |
| Validation on EXPANDED plan | DSL compiler is gatekeeper |
| Expansion output is UNTRUSTED | Validate like hostile input |

---

# Part 9: Matching Strategy

```
Input
  │
  ├─► 1. Context match (verb in current DAG ready set?) → Instant
  ├─► 2. Learned phrase exact match (score 1.0) → Instant
  ├─► 3. Phonetic match (within scope) → <50ms
  ├─► 4. Alias/invocation phrase match → <100ms
  ├─► 5. Semantic fallback (only if above fail) → <500ms
  └─► 6. Ambiguous/Suggest? Show concrete verb options → Learning
```

**Critical:** Semantic fallback (step 5) NEVER auto-executes. It only produces candidates for Suggest/Ambiguous menus. This prevents reintroducing a "BGE decides and runs" path.

---

# Part 10: UX Rules (Non-Chatbot Console)

| Rule | Implementation |
|------|---------------|
| Mode selection is structured | `[KYC] [Onboarding] [Trading]` buttons |
| Ambiguity = concrete options | Clickable verb buttons, never open-ended |
| User selection = learning signal | Write to `user_learned_phrases` |
| Context auto-fills args | `autofill_from` in schema |
| DAG shows ready/blocked | Visual prereq status |
| UI renders only operator labels | Never internal IDs |

---

# Part 11: Implementation Phases

## Phase 1: Operator Noun/Verb Metadata Lint (FIRST)
- [x] Every macro must have `ui.label`, `ui.description`, `ui.target_label` ✅
- [x] `structure_ref`, `party_ref` etc. replace `entity_ref kinds:[x]` ✅
- [x] Lint fails build if internal kinds leak to UI layer ✅
- [x] Enums used in expansion MUST have internal mapping ✅
- [x] `cargo x verbs lint-macros` command added ✅

## Phase 2: Unified Session
- [x] Create `UnifiedSession` struct (already existed, extended) ✅
- [x] Add `StateStack` for history ✅
- [x] Add cascade context fields (client, structure_type, current_structure, current_case) ✅
- [x] Add `Persona` for verb filtering ✅
- [x] Add `DagState` for prereq tracking ✅
- [x] Extend `RunSheetEntry` with dag_depth, dependencies, validation_errors ✅
- [x] Add `PrereqCondition` enum (VerbCompleted, AnyOf, StateExists, FactExists) ✅
- [x] Add `SearchScope` for constraint cascade ✅
- [x] Delete `DslSheet` (superseded by `RunSheet`) ✅ DEPRECATED (kept for backward compat)
- [x] `UnifiedSessionContext` decision ✅ DEFERRED - kept separate (see UnifiedSessionContext Status section)
- [x] Merge `CbuSession` into `UnifiedSession` ✅ COMPLETE - functionality migrated, CbuSession deprecated

## Phase 3: Macro Schemas
- [x] `structure.setup`, `structure.assign-role` ✅ YAML schemas in `config/verb_schemas/macros/structure.yaml`
- [x] `case.open`, `case.submit`, `case.approve` ✅ YAML schemas in `config/verb_schemas/macros/case.yaml`
- [x] `mandate.create`, `mandate.set-instruments` ✅ YAML schemas in `config/verb_schemas/macros/mandate.yaml`
- [x] Build macro expander with `${arg.*.internal}` support ✅ `rust/src/dsl_v2/macros/` module

## Phase 4: Constraint Cascade
- [x] `SearchScope` type added ✅
- [x] `derive_search_scope()` method on session ✅
- [x] Client picker at session start (mandatory) ✅ (2026-01-28)
- [x] Entity search scoped by context ✅ (2026-01-28) - `create_scoped_entity_router()` in `entity_routes.rs`

## Phase 5: DAG Navigation
- [x] `DagState` tracks verb completions and state flags ✅
- [x] `PrereqCondition.is_satisfied()` checks prereqs ✅
- [x] `RunSheet.ready_for_execution()` computes ready entries ✅
- [x] `RunSheet.cascade_skip()` propagates failures ✅
- [x] Compute verb readiness from prereqs ✅ (2026-01-28) - `get_ready_verbs()`, `is_verb_ready()` in `macro_integration.rs`
- [x] Context flows down on execution ✅ (2026-01-28) - `update_dag_after_execution()` called in MCP handlers + agent_routes

## Phase 6: Learning Loop
- [x] Disambiguation UI complete (057) ✅
- [x] `/select-verb` captures selection ✅
- [x] Variant generation (max 5) ✅ (2026-01-28) - `generate_phrase_variants()` with MAX_VARIANTS=5, MIN_TOKENS=2
- [x] Negative signals with decay ✅ (2026-01-28) - Blocklist uses `POWER(0.95, days_old)` decay in SQL
- [ ] Promotion: user → team → global

## Phase 7: Testing
- [x] Unit tests for cascade context ✅
- [x] Unit tests for DAG operations ✅
- [x] Unit tests for prereq conditions ✅
- [ ] Golden expansion snapshot tests
- [ ] Determinism property tests
- [x] Role validation (GP fails on SICAV) ✅ (2026-01-28) - 5 tests in `expander.rs`, uses `StructureType::short_key()`
- [ ] Idempotency key collision tests
- [ ] Concurrency lock test: N parallel executions on same structure/case → no deadlocks, no duplicates
- [ ] Crash/retry test: fail mid-plan, rerun with same execution_id → safe resume/no-op

---

# Implementation Progress

## 2026-01-28: Session Layer Foundation

### Completed
1. **Lint Module** (`rust/src/lint/`)
   - `diagnostic.rs` - Severity, Diagnostic types
   - `macro_lint.rs` - MACRO000-MACRO080 rules (12 tests passing)
   - `cargo x verbs lint-macros` command

2. **UnifiedSession Extended** (`rust/src/session/unified.rs`)
   - Added cascade context: `client`, `structure_type`, `current_structure`, `current_case`
   - Added `Persona` enum (Ops, Kyc, Trading, Admin) with mode_tags
   - Added `DagState` for verb completion and state flag tracking
   - Added `PrereqCondition` enum for prereq checking
   - Added `SearchScope` for constraint cascade derivation
   - Extended `RunSheetEntry` with `dag_depth`, `dependencies`, `validation_errors`
   - Added `EntryStatus::Skipped` for cascade failures
   - Added `RunSheet` DAG helper methods:
     - `by_phase(depth)` - entries at DAG depth
     - `max_depth()` - deepest phase
     - `ready_for_execution()` - entries with satisfied deps
     - `phase_complete(depth)` - check phase completion
     - `cascade_skip(failed_id)` - propagate failure to dependents

### Key Types Added
```rust
// Constraint cascade (client → structure_type → structure → case)
pub struct ClientRef { client_id: Uuid, display_name: String }
pub enum StructureType { Pe, Sicav, Hedge, Etf, Pension, Trust, Fof }
pub struct StructureRef { structure_id: Uuid, display_name: String, structure_type: StructureType }
pub struct CaseRef { case_id: Uuid, display_name: String }

// Persona for verb filtering
pub enum Persona { Ops, Kyc, Trading, Admin }

// DAG navigation
pub struct DagState { completed: HashSet<String>, state_flags: HashMap<String, bool>, facts: HashMap<String, Value> }
pub enum PrereqCondition { VerbCompleted, AnyOf, StateExists, FactExists }
pub struct SearchScope { client_id, structure_type, structure_id }
```

### Next Steps
1. ~~Delete `DslSheet`, migrate `SheetExecutor` to use `RunSheet`~~ ✅ DONE
2. ~~Update `cbu_session_routes.rs` to use RunSheet types~~ ✅ DONE
3. ~~Migrate `CbuSession` to use `RunSheet`~~ ✅ DONE
4. ~~Migrate `SheetExecutor.persist_audit()` to accept `RunSheet` directly~~ ✅ DONE
5. ~~Delete `DslSheet` module~~ ✅ DEPRECATED (kept for backward compat)
6. ~~`UnifiedSessionContext` decision~~ ✅ DEFERRED - kept separate (documented above)
7. ~~Merge `CbuSession` functionality into `UnifiedSession`~~ ✅ DONE
8. Create `SessionResponse` HTTP boundary type
9. ~~Phase 2: Create `operator_types.rs` and `display_nouns.rs`~~ ✅ DONE
10. ~~Migrate `ExecutionContext.pending_cbu_session` to use `UnifiedSession`~~ ✅ DONE
11. ~~Migrate `cbu_session_routes.rs` endpoints to use `UnifiedSession`~~ ✅ DONE
12. ~~Migrate `mcp/handlers/core.rs` to use `UnifiedSession`~~ ✅ DONE

## Summary: CbuSession → UnifiedSession Migration Complete

All production code now uses `UnifiedSession` instead of the deprecated `CbuSession`:

### Files Migrated:
1. **`dsl_v2/executor.rs`** - `ExecutionContext.pending_session` (was `pending_cbu_session`)
2. **`domain_ops/session_ops.rs`** - All session operation handlers
3. **`domain_ops/template_ops.rs`** - Batch operation context creation
4. **`api/agent_routes.rs`** - DSL execution scope propagation
5. **`api/agent_service.rs`** - Chat execution scope propagation
6. **`api/cbu_session_routes.rs`** - All REST endpoints now use `UnifiedSession`
7. **`mcp/handlers/core.rs`** - All MCP tool handlers

### Method Mappings:
| CbuSession | UnifiedSession |
|------------|----------------|
| `id()` | `id` (field) |
| `name()` | `name` (field) |
| `count()` | `cbu_count()` |
| `cbu_ids()` | `cbu_ids_vec()` |
| `load_many()` | `load_cbus()` |
| `clear()` | `clear_cbus_with_history()` |
| `undo()` | `undo_cbu()` |
| `redo()` | `redo_cbu()` |
| `history_depth()` | `cbu_history_depth()` |
| `future_depth()` | `cbu_future_depth()` |
| `maybe_save()` | `save().await` |
| `force_save()` | `save().await` |
| `is_dirty()` | `dirty` (field) |
| `set_scope()` | `set_repl_scope()` |
| `set_template()` | `set_repl_template()` |
| `confirm_intent()` | `confirm_repl_intent()` |
| `set_generated()` | `set_repl_generated()` |
| `mark_executed()` | `mark_repl_executed()` |
| `reset_to_scoped()` | `reset_repl_to_scoped()` |
| `list_all()` | `list_recent()` |

### CbuSession Deletion Complete (2026-01-28):
- ✅ Deleted `rust/src/session/cbu_session.rs`
- ✅ Removed module declaration from `session/mod.rs`
- ✅ Removed re-exports of deprecated types
- ✅ Moved result types (`CbuSummary`, `ClearResult`, `HistoryResult`, `JurisdictionCount`, `SessionInfo`) to `domain_ops/session_ops.rs` where they're used
- ✅ Fixed all callers to use correct UnifiedSession API signatures

## 2026-01-28: cbu_session_routes.rs Migration

### Completed
1. **Updated `generate_sheet` endpoint** to return `RunSheet`
   - Creates `RunSheetEntry` instances with unified `EntryStatus`
   - Maintains backward compatibility via `convert_run_sheet_to_dsl_sheet()` bridge
   - CbuSession still stores legacy DslSheet (until CbuSession migration)

2. **Updated `submit_sheet` endpoint** to use unified executor
   - Converts legacy `DslSheet` to `RunSheet` via `convert_dsl_sheet_to_run_sheet()`
   - Builds `ExecutionPhase` list from DAG depths
   - Calls `execute_run_sheet()` (new unified API)
   - Returns `SheetExecutionResult` (unified type)
   - Bridge function `convert_unified_result_to_legacy()` for audit persistence

3. **Added conversion functions** (temporary bridges)
   - `convert_run_sheet_to_dsl_sheet()` - RunSheet → legacy DslSheet
   - `convert_dsl_sheet_to_run_sheet()` - legacy DslSheet → RunSheet
   - `build_execution_phases()` - Build phases from RunSheet entries
   - `convert_unified_result_to_legacy()` - unified result → legacy for audit

### Remaining DslSheet Consumers (After CbuSession Migration)
- ~~`CbuSession.sheet: Option<DslSheet>`~~ ✅ Migrated to `RunSheet`
- `SheetExecutor.execute_phased()` - legacy API kept for compatibility
- `SheetExecutor.persist_audit()` - still takes legacy types (needs migration)
- `cbu_session_routes.rs` conversion functions - for audit persistence only

## 2026-01-28: CbuSession Migration to RunSheet

### Completed
1. **Updated `CbuSession` struct** (`rust/src/session/cbu_session.rs`)
   - Changed `sheet: Option<DslSheet>` to `sheet: Option<RunSheet>`
   - Updated `set_generated()` to take `RunSheet`
   - Updated `sheet()` and `sheet_mut()` getters to return `RunSheet` references
   - Import changed from `dsl_sheet::DslSheet` to `unified::RunSheet`

2. **Simplified `cbu_session_routes.rs`**
   - Removed `convert_run_sheet_to_dsl_sheet()` bridge (no longer needed)
   - `generate_sheet` now directly stores `RunSheet` in session
   - `submit_sheet` now uses session's `RunSheet` directly
   - Added `convert_run_sheet_to_legacy_for_audit()` - temporary bridge for audit persistence
   - Updated `get_repl_state` to use RunSheet fields (`entries.len()`, `max_depth()`, `cursor`)

### UnifiedSessionContext Status
⚠️ **DEFERRED** - `UnifiedSessionContext` retained (deprecated, not deleted):

The original plan was to delete `UnifiedSessionContext` and consolidate into `UnifiedSession`. After analysis, we're keeping both types because they serve distinct purposes:

| Type | Purpose | Key Fields |
|------|---------|------------|
| `UnifiedSession` | DSL execution, run sheet, constraint cascade | `run_sheet`, `dag_state`, `bindings`, cascade context |
| `UnifiedSessionContext` | Visualization, agent mode, graph navigation | `graph`, `viewport`, `agent`, `view`, `command_history` |

**Why keep separate:**
1. `UnifiedSession` is serializable, `UnifiedSessionContext` has non-serializable fields (`ExecutionContext`)
2. `AgentController` depends on `UnifiedSessionContext.agent` and `UnifiedSessionContext.mode`
3. Visualization state (graph, viewport) is complex and well-isolated
4. A full merge would require significant refactoring with limited benefit

**Current approach:**
- Mark `UnifiedSessionContext` as deprecated in code comments
- New code should prefer `UnifiedSession` for DSL/REPL state
- Future consolidation can happen incrementally as visualization features migrate

**Files using `UnifiedSessionContext`:**
- `session/enhanced_context.rs` - `EnhancedContextBuilder::from_session_context()`
- `session/agent_context.rs` - `AgentGraphContext::from_session()`
- `research/agent_controller.rs` - Holds `Arc<RwLock<UnifiedSessionContext>>`

### CbuSession Merge Status
✅ **COMPLETE** - `CbuSession` functionality merged into `UnifiedSession`:

**What was migrated:**
1. **CBU undo/redo history** → `UnifiedSession.cbu_history`, `cbu_future`
   - `CbuSnapshot` type for capturing CBU set state
   - `load_cbu()`, `load_cbus()`, `unload_cbu()`, `clear_cbus_with_history()`
   - `undo_cbu()`, `redo_cbu()`, `can_undo_cbu()`, `can_redo_cbu()`

2. **REPL state machine** → `UnifiedSession.repl_state: ReplState`
   - `ReplState` enum (Empty, Scoped, Templated, Generated, Parsed, Resolving, Ready, Executing, Executed)
   - `set_repl_scope()`, `set_repl_template()`, `confirm_repl_intent()`
   - `set_repl_generated()`, `set_repl_parsed()`, `resolve_repl_ref()`
   - `set_repl_executing()`, `update_repl_progress()`, `mark_repl_executed()`
   - `reset_repl_to_scoped()`, `reset_repl_to_empty()`

3. **Persistence** → `UnifiedSession.save()`, `load()`, `load_or_new()`, `delete()`, `list_recent()`
   - `dirty` flag for change tracking
   - `name` field for session naming

4. **Query helpers** → `cbu_count()`, `contains_cbu()`, `cbu_ids_vec()`, `is_dirty()`, `mark_clean()`

**Migration guide (in cbu_session.rs module docs):**
- `CbuSession.state.cbu_ids` → `UnifiedSession.entity_scope.cbu_ids`
- `CbuSession.sheet` → `UnifiedSession.run_sheet`
- `ReplSessionState` → `ReplState`

**Deprecated (kept for backward compat):**
- `CbuSession` struct - marked `#[deprecated]`
- `ReplSessionState` enum - marked `#[deprecated]`

**Files with deprecation warnings (to migrate later):**
- `dsl_v2/executor.rs` - `ExecutionContext.pending_cbu_session`
- `api/cbu_session_routes.rs` - endpoint handlers

### DslSheet Deletion Status
✅ **COMPLETE** - The `DslSheet` module is now fully deprecated:
- Added `persist_audit_unified()` method that accepts `RunSheet` directly
- Removed all legacy conversion functions from `cbu_session_routes.rs`
- Marked legacy `execute_phased()` and `persist_audit()` as `#[deprecated]`
- Marked `dsl_sheet` re-exports in `session/mod.rs` as `#[deprecated]`
- The `dsl_sheet.rs` module is kept for backward compatibility but all new code uses unified types

## 2026-01-28: persist_audit Migration

### Completed
1. **Added `persist_audit_unified()` method** (`rust/src/dsl_v2/sheet_executor.rs`)
   - Accepts `RunSheet` and unified `SheetExecutionResult`
   - Builds DAG analysis JSON from RunSheet entries
   - Maps unified status types to audit table

2. **Removed legacy conversion functions** (`rust/src/api/cbu_session_routes.rs`)
   - Deleted `convert_run_sheet_to_legacy_for_audit()`
   - Deleted `convert_unified_result_to_legacy()`
   - `submit_sheet` now calls `persist_audit_unified()` directly

3. **Added deprecation attributes**
   - `execute_phased()` marked `#[deprecated]`
   - `persist_audit()` marked `#[deprecated]`
   - `dsl_sheet` re-exports marked `#[deprecated]` in `session/mod.rs`

## 2026-01-28: SheetExecutor Migration (Continued)

### Completed
1. **Sheet Execution Types** (`rust/src/session/unified.rs`)
   - Added `ExecutionPhase` - group of entries at same DAG depth
   - Added `ErrorCode` - categorized execution error codes
   - Added `SheetStatus` - Success/Failed/RolledBack
   - Added `EntryError` - detailed error for single entry
   - Added `EntryResult` - execution result per entry
   - Added `SheetExecutionResult` - full sheet execution result
   - Added `Display` impl for `StructureType`

2. **SheetExecutor Updated** (`rust/src/dsl_v2/sheet_executor.rs`)
   - Added `execute_run_sheet()` - new unified API using `RunSheet`
   - Added `mark_downstream_skipped_unified()` - cascade skip for RunSheet
   - Added `classify_unified_error()` - error classification for UnifiedErrorCode
   - Added `extract_symbol_from_dsl()` - extract `:as @symbol` bindings
   - Legacy `execute_phased()` preserved for backward compatibility

3. **Module Exports** (`rust/src/session/mod.rs`)
   - Exported unified sheet execution types with `Unified*` prefix
   - Added deprecation notice to `dsl_sheet` re-exports
   - Type mapping documented in comments

### Key New API
```rust
// New unified API (preferred)
impl SheetExecutor {
    pub async fn execute_run_sheet(
        &self,
        session_id: Uuid,
        run_sheet: &mut RunSheet,
        phases: &[UnifiedExecutionPhase],
    ) -> Result<UnifiedSheetExecutionResult>;
}

// Legacy API (deprecated, still works)
impl SheetExecutor {
    pub async fn execute_phased(
        &self,
        sheet: &mut DslSheet,
        phases: &[ExecutionPhase],
    ) -> Result<SheetExecutionResult>;
}
```

---

## 2026-01-28: Phase 3 - Macro Expander Complete

### Completed
1. **Macro YAML Schemas** (`rust/config/verb_schemas/macros/`)
   - `structure.yaml` - 5 macros: setup, assign-role, list, select, roles
   - `case.yaml` - 9 macros: open, add-party, solicit-document, submit, approve, reject, list, select
   - `mandate.yaml` - 7 macros: create, add-product, set-instruments, set-markets, list, select, details

2. **Macro Expander Module** (`rust/src/dsl_v2/macros/`)
   - `mod.rs` - Module exports and documentation
   - `schema.rs` - MacroSchema, MacroArg, MacroEnumValue, MacroPrereq types
   - `variable.rs` - Variable substitution (`${arg.*}`, `${scope.*}`, `${session.*}`)
   - `registry.rs` - MacroRegistry with domain/mode_tag indexing
   - `expander.rs` - expand_macro() with prereq checking and constraint validation

3. **Intent Pipeline Integration** (`rust/src/mcp/macro_integration.rs`)
   - Global macro registry singleton
   - `is_macro()`, `try_expand_macro()` for pipeline integration
   - `intent_args_to_macro_args()` for argument conversion
   - `list_macros()`, `macros_by_mode()` for UI

4. **Server Startup** (`rust/crates/ob-poc-web/src/main.rs`)
   - Macro registry initialization on startup
   - Logging of macro count and domain count

### Key Types
```rust
// Variable substitution context
pub struct VariableContext {
    pub args: HashMap<String, ArgValue>,   // ${arg.name}, ${arg.name.internal}
    pub scope: HashMap<String, String>,    // ${scope.client_id}
    pub session: HashMap<String, Value>,   // ${session.current_structure}
}

// Expansion output
pub struct MacroExpansionOutput {
    pub statements: Vec<String>,           // Primitive DSL statements
    pub sets_state: Vec<(String, Value)>,  // State flags to set
    pub unlocks: Vec<String>,              // Verbs that become available
    pub audit: MacroExpansionAudit,        // Audit trail
}

// Pipeline integration
pub enum MacroAttemptResult {
    NotAMacro,                             // Continue normal processing
    Expanded(MacroExpansionOutput),        // Use expanded statements
    Failed(MacroExpansionError),           // Expansion failed
}
```

### Tests
- 16 unit tests covering schema parsing, variable substitution, expansion, and error cases
- All tests passing

---

# Part 12: Files to Create/Modify

| File | Change |
|------|--------|
| `config/verb_schemas/macros/structure.yaml` | ✅ CREATED |
| `config/verb_schemas/macros/case.yaml` | ✅ CREATED |
| `config/verb_schemas/macros/mandate.yaml` | ✅ CREATED |
| `rust/src/session/unified.rs` | ✅ EXISTS (extended) |
| `rust/src/dsl_v2/macros/mod.rs` | ✅ CREATED |
| `rust/src/dsl_v2/macros/schema.rs` | ✅ CREATED |
| `rust/src/dsl_v2/macros/variable.rs` | ✅ CREATED |
| `rust/src/dsl_v2/macros/registry.rs` | ✅ CREATED |
| `rust/src/dsl_v2/macros/expander.rs` | ✅ CREATED |
| `rust/src/mcp/macro_integration.rs` | ✅ CREATED |
| `rust/src/lint/macro_lint.rs` | ✅ EXISTS |

---

# Part 13: Success Criteria

| Metric | Current | Target |
|--------|---------|--------|
| Intent match (overall) | 80% | 95%+ |
| Confidently wrong | >0 | 0 |
| Latency (learned/phonetic hit) | N/A | <50ms |
| Ops sees "CBU" | Yes | Never |
| Ops sees "entity_ref" | Yes | Never |
| Disambiguation captures learning | No | Yes |
| Context auto-fill | None | Args pre-filled |

---

# Part 14: Open Questions

1. **Role filtering** - GP only for PE/Hedge. Server-side validation: ✅ Required
2. **Power user mode** - DSL direct input for SMEs?
3. **Multi-structure ops** - Batch mode?
4. **Voice commands** - "Go back", "Start over" → nav methods?

---

# References

- `docs/architecture/DISAMBIGUATION-FEEDBACK-LOOP-RATIONALE.md`
- `config/verb_schemas/SCHEMA_FORMAT_V2.md`
- `rust/src/mcp/scope_resolution.rs`
