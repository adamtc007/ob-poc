# DSL Style Guide

This guide defines formatting conventions and best practices for writing DSL files.

## File Organization

### Header Block

Every DSL file should start with a header comment block:

```clojure
;; ============================================================================
;; File Title
;; ============================================================================
;; intent: Brief description of what this file accomplishes
;;
;; Additional context or notes about the file contents.
```

### Section Headers

Use section headers to organize related operations:

```clojure
;; ----------------------------------------------------------------------------
;; Section Title
;; ----------------------------------------------------------------------------
```

## Formatting

### Indentation

- Use **2 spaces** for indentation (no tabs)
- Align continuation lines with the first argument after the verb

```clojure
;; Correct - 2-space indent
(trading-profile.create
  :cbu-id @fund
  :name "Growth Strategy"
  :as @profile)

;; Also correct - aligned with first arg
(cbu-role.assign :cbu-id @fund
                 :entity-id @person
                 :role "DIRECTOR")
```

### Line Length

- Maximum **80 characters** per line
- Break long argument lists across multiple lines

```clojure
;; Too long
(entity.create :name "Very Long Company Name Holdings Ltd" :type "LEGAL" :jurisdiction "LU" :lei "123456789012345678" :as @entity)

;; Correct - broken across lines
(entity.create
  :name "Very Long Company Name Holdings Ltd"
  :type "LEGAL"
  :jurisdiction "LU"
  :lei "123456789012345678"
  :as @entity)
```

### Whitespace

- One blank line between verb calls
- Two blank lines between sections
- No trailing whitespace

## Naming Conventions

### Bindings

- Use **snake_case** for binding names
- Use descriptive names that indicate purpose
- Prefix with entity type when ambiguous

```clojure
;; Good
(cbu.create :name "Fund" :as @fund)
(entity.create :name "Manager" :as @manager_entity)
(cbu-role.assign ... :as @director_role)

;; Avoid - unclear purpose
(cbu.create :name "Fund" :as @x)
(entity.create :name "Manager" :as @e1)
```

### Entity References

- Entity references use **angle brackets**: `<Entity Name>`
- Use the most specific unambiguous name

```clojure
;; Good - specific names
(session.load-galaxy :apex-name <Allianz Global Investors>)
(kyc-case.add-subject :entity-id <Goldman Sachs International>)

;; Avoid - ambiguous
(session.load-galaxy :apex-name <Allianz>)
```

## Comments

### Intent Comments

Every significant operation should have an intent comment:

```clojure
;; intent: Create management company for the fund structure
(entity.create :name "Alpine Capital S.a r.l." :as @manco)
```

### Macro Annotations

When using operator macros, annotate with the macro name:

```clojure
;; intent: Set up new PE fund
;; macro: structure.setup
(cbu.create :name "Alpine PE III" :type "FUND" :as @fund)
```

### Block Comments

For multi-line explanations, use consecutive comment lines:

```clojure
;; This ownership structure implements a typical PE fund hierarchy.
;; The GP entity holds a 1% stake while LPs contribute the remaining 99%.
;; Control flows through the GP despite minimal economic interest.
```

## Argument Order

### Recommended Order

1. **Identifier arguments** (`:id`, `:cbu-id`, `:entity-id`)
2. **Name/label arguments** (`:name`, `:description`)
3. **Type/category arguments** (`:type`, `:kind`, `:role`)
4. **Configuration arguments** (`:jurisdiction`, `:currency`, etc.)
5. **Date arguments** (`:effective-date`, `:expiry-date`)
6. **Binding** (`:as @name`) - always last

```clojure
;; Correct order
(cbu-role.assign
  :cbu-id @fund              ;; 1. Identifier
  :entity-id @person         ;; 1. Identifier
  :role "DIRECTOR"           ;; 3. Type
  :effective-date "2024-01-01"  ;; 5. Date
  :as @role)                 ;; 6. Binding (last)
```

## Collections

### Arrays

- Short arrays (3 items or less) can be single-line
- Longer arrays should be multi-line with one item per line

```clojure
;; Single line - short array
(trading-profile.set-instruments :instruments ["EQUITY" "BONDS"])

;; Multi-line - longer array
(trading-profile.set-instruments
  :instruments [
    "EQUITY"
    "FIXED_INCOME"
    "DERIVATIVES"
    "FX_FORWARDS"
    "STRUCTURED_PRODUCTS"
  ])
```

### Maps

- Always use multi-line format for maps
- Align keys and values

```clojure
(config.set
  :settings {
    "theme"    "dark"
    "locale"   "en-US"
    "debug"    false
    "timeout"  30000
  })
```

## Error Prevention

### Always Bind Results

Always capture results with `:as` when you'll reference them later:

```clojure
;; Good - captured for later use
(cbu.create :name "Fund" :as @fund)
(cbu-role.assign :cbu-id @fund ...)

;; Bad - can't reference the created CBU
(cbu.create :name "Fund")
(cbu-role.assign :cbu-id ??? ...)  ;; No way to reference
```

### Check Dependencies

Ensure bindings are defined before use:

```clojure
;; Correct order
(entity.create :name "Manager" :as @manager)
(cbu.create :name "Fund" :as @fund)
(cbu-role.assign :cbu-id @fund :entity-id @manager :role "MANCO")

;; Wrong - @fund used before defined
(cbu-role.assign :cbu-id @fund :entity-id @manager :role "MANCO")
(cbu.create :name "Fund" :as @fund)  ;; Too late!
```

## File Naming

- Use **kebab-case** for file names
- Include a numeric prefix for ordered sequences
- Use `.dsl` extension

```
00-syntax-tour.dsl
01-cbu-create.dsl
02-roles-and-links.dsl
fund-onboarding-template.dsl
kyc-case-workflow.dsl
```

## Examples

See the golden examples in `docs/dsl/golden/` for reference implementations:

| File | Demonstrates |
|------|--------------|
| `00-syntax-tour.dsl` | All syntax constructs |
| `01-cbu-create.dsl` | Basic CBU creation |
| `02-roles-and-links.dsl` | Entity creation and role assignment |
| `03-kyc-case-sheet.dsl` | KYC case with requirements |
| `04-ubo-mini-graph.dsl` | Ownership chain modeling |
| `05-otc-isda-csa.dsl` | OTC derivatives setup |
| `06-macro-v2-roundtrip.dsl` | Macro expansion examples |
| `90-error-fixtures.dsl` | Error handling test cases |
