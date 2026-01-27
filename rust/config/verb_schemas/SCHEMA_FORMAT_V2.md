# Verb Schema V2 Format

## Canonical Structure

```yaml
verb: view.drill
domain: view
action: drill

aliases:
  - drill
  - zoom-in
  - expand
  - dive
  - deeper

args:
  style: keyworded                              # keyworded | positional | hybrid
  required:
    entity: { type: entity_name }               # resolver fills in later
  optional:
    kind:  { type: enum, values: [fund, company, person, trust, partnership] }
    depth: { type: int, default: 1 }

positional_sugar:
  - entity
  - kind

invocation_phrases:
  - "drill into entity"
  - "zoom in on selected entity"
  - "open entity details"
  - "expand node"

examples:
  - "(view.drill :entity \"Allianz\")"
  - "(view.drill :entity \"Allianz\" :kind fund)"
  - "(drill \"Allianz\" fund)"                   # positional sugar

doc: "Drill down into entity structure"
tier: intent
tags: [navigation, view]
```

---

## Type Reference

```yaml
# Primitives
{ type: str }
{ type: int }
{ type: int, default: 10 }
{ type: bool }
{ type: bool, default: false }
{ type: decimal }
{ type: decimal, default: "25.0" }

# Date/Time
{ type: date }                                  # ISO 8601: 2024-01-15
{ type: datetime }                              # ISO 8601: 2024-01-15T10:30:00Z
{ type: duration }                              # ISO 8601: P1D, PT1H

# Identifiers
{ type: uuid }
{ type: lei }                                   # LEI format

# Enums
{ type: enum, values: [control, economic, both] }
{ type: enum, values: [RED, AMBER, GREEN], default: GREEN }

# Entity references
{ type: entity_name }                           # Free text, resolved later
{ type: entity_ref }                            # Resolved UUID
{ type: entity_ref, kinds: [fund, company] }    # Constrained to types

# Collections
{ type: list, of: str }
{ type: list, of: { type: enum, values: [a, b, c] } }

# Complex
{ type: json }                                  # Opaque JSON blob
{ type: nested, verb: ownership.add }           # Nested s-expr
```

---

## Args Style

```yaml
# Keyword-only (default for 2+ args)
args:
  style: keyworded
  required:
    entity: { type: entity_name }
    mode: { type: enum, values: [control, economic] }
  optional:
    depth: { type: int, default: 10 }

# Positional-only (rare, simple verbs)
args:
  style: positional
  required:
    - { name: entity, type: entity_name }
    - { name: mode, type: enum, values: [in, out] }

# Hybrid (positional sugar on top of keyworded)
args:
  style: keyworded
  required:
    entity: { type: entity_name }
  optional:
    kind: { type: enum, values: [...] }
positional_sugar:
  - entity
  - kind
```

---

## Complete Examples

### Navigation Verb (Simple)
```yaml
verb: view.surface
domain: view
action: surface

aliases:
  - surface
  - back
  - up
  - zoom-out
  - parent

args:
  style: keyworded
  optional:
    levels: { type: int, default: 1 }
    all:    { type: bool, default: false }

positional_sugar: []

invocation_phrases:
  - "go back"
  - "zoom out"
  - "surface up"
  - "parent level"

examples:
  - "(view.surface)"
  - "(view.surface :levels 2)"
  - "(view.surface :all true)"

doc: "Navigate back up the hierarchy"
tier: intent
tags: [navigation, view]
```

### Ownership Verb (Complex)
```yaml
verb: ownership.trace-chain
domain: ownership
action: trace-chain

aliases:
  - trace-chain
  - ownership-chain
  - show-chain
  - follow-ownership

args:
  style: keyworded
  required:
    entity: { type: entity_ref, kinds: [company, fund, person] }
  optional:
    depth:            { type: int, default: 10 }
    mode:             { type: enum, values: [control, economic, both], default: both }
    include-indirect: { type: bool, default: true }
    as-of:            { type: date }

positional_sugar:
  - entity

invocation_phrases:
  - "trace ownership chain for entity"
  - "show ownership structure"
  - "follow the ownership"
  - "who owns entity"

examples:
  - "(ownership.trace-chain :entity \"Allianz\")"
  - "(ownership.trace-chain :entity \"Allianz\" :mode control :depth 5)"

doc: "Trace ownership chain from entity to UBOs"
tier: intent
tags: [ownership, ubo, chain]
```

### CRUD Verb (Many Args)
```yaml
verb: fund.create-umbrella
domain: fund
action: create-umbrella

aliases:
  - create-umbrella
  - new-sicav
  - new-icav

args:
  style: keyworded
  required:
    name:         { type: str }
    jurisdiction: { type: str }
  optional:
    fund-type:    { type: enum, values: [SICAV, ICAV, OEIC, VCC, UCITS] }
    lei:          { type: lei }
    regulator:    { type: str }
    launch-date:  { type: date }

positional_sugar:
  - name
  - jurisdiction

invocation_phrases:
  - "create new umbrella fund"
  - "set up sicav"
  - "register new fund structure"

examples:
  - "(fund.create-umbrella :name \"Allianz Global\" :jurisdiction \"Luxembourg\")"
  - "(fund.create-umbrella \"Allianz Global\" \"Luxembourg\")"

doc: "Create umbrella fund structure (SICAV, ICAV, etc.)"
tier: crud
tags: [fund, create]
```

### Session Verb
```yaml
verb: session.load-client-group
domain: session
action: load-client-group

aliases:
  - load
  - open
  - switch
  - select-client

args:
  style: keyworded
  required:
    target: { type: entity_name }
  optional:
    kind: { type: enum, values: [client-group, cbu, entity] }

positional_sugar:
  - target

invocation_phrases:
  - "load client group"
  - "open entity"
  - "switch to client"
  - "select working set"

examples:
  - "(session.load-client-group :target \"Allianz\")"
  - "(load \"Allianz\")"

doc: "Load client group or entity into session"
tier: intent
tags: [session, navigation]
```

---

## Migration: V1 → V2

### V1 (Current)
```yaml
verbs:
  view:
    drill:
      description: "Drill into entity"
      args:
        - name: entity
          type: entity
          required: true
          description: "Entity to drill into"
        - name: depth
          type: integer
          required: false
          default: 1
      metadata:
        tier: intent
        tags: [navigation]
```

### V2 (New)
```yaml
verb: view.drill
domain: view
action: drill

aliases: [drill, zoom-in, expand]

args:
  style: keyworded
  required:
    entity: { type: entity_name }
  optional:
    depth: { type: int, default: 1 }

positional_sugar: [entity]

invocation_phrases:
  - "drill into entity"

examples:
  - "(view.drill :entity \"X\")"

doc: "Drill into entity"
tier: intent
tags: [navigation]
```

---

## Rules

1. **Keyword-only for 2+ required args** - No positional spaghetti
2. **Positional sugar max 2** - `[entity]` or `[entity, kind]`
3. **Aliases include action** - `drill` is alias for `view.drill`
4. **Invocation phrases 3-5** - For BGE/NL lane
5. **Examples show canonical + sugar** - Both forms
6. **Types inline** - `{ type: enum, values: [...] }` not separate definition
