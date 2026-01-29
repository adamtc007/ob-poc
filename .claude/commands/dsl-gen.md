# DSL Generation Mode

You are generating OB-POC DSL. Follow these rules strictly.

## Non-Negotiable Rules

1. **Never freehand syntax** - Follow golden examples and style guide exactly
2. **Annotation blocks required** - Every top-level form must be preceded by:
   ```clojure
   ;; intent: <what this accomplishes in business terms>
   ;; macro: <operator.verb-name or "primitive">
   ;; constraints: <any preconditions or dependencies>
   ```
3. **Small increments** - Generate 1-3 forms at a time, then wait for validation
4. **Diagnostics-driven fixes** - After validation, fix only what diagnostics report
5. **`:as @binding` is always last** - Never place `:as` before other arguments
6. **Prefer existing verbs** - Do not introduce new verbs unless explicitly asked

## Generation Process

1. **Propose skeleton first** - List the minimal forms needed without writing DSL
2. **Ask for missing details** - Required args, entity names, jurisdictions, etc.
3. **Generate incrementally** - Write 1-3 forms, then pause for validation
4. **Fix on feedback** - Only change what validation errors require

## Reference Files

Before generating, review these files:
- `docs/dsl/golden/00-syntax-tour.dsl` - All syntax patterns
- `docs/DSL_STYLE_GUIDE.md` - Formatting rules
- Domain-specific golden examples as needed

## Syntax Quick Reference

```clojure
;; intent: Create a Luxembourg SICAV fund
;; macro: structure.setup
;; constraints: requires contract.create first
(cbu.create
  :name "Acme Global Equity Fund"
  :type "FUND"
  :jurisdiction "LU"
  :legal-form "SICAV"
  :as @fund)
```

### Argument Order
1. Identifier args (`:id`, `:cbu-id`, `:entity-id`)
2. Name/label args (`:name`, `:description`)
3. Type/category args (`:type`, `:kind`, `:role`)
4. Configuration args (`:jurisdiction`, `:currency`)
5. Date args (`:effective-date`, `:expiry-date`)
6. **Binding (`:as @name`) - ALWAYS LAST**

### Data Types
- Strings: `"quoted"`
- Numbers: `42`, `3.14`, `-17`
- Booleans: `true`, `false`
- Null: `nil`
- Symbol refs: `@name` (reference previous binding)
- Entity refs: `<Entity Name>` (resolved by lookup)
- Arrays: `["a" "b" "c"]`
- Maps: `{:key "value" :count 42}`

## Common Patterns

### CBU + Roles
```clojure
;; intent: Create fund structure
;; macro: structure.setup
(cbu.create :name "Fund" :type "FUND" :jurisdiction "LU" :as @fund)

;; intent: Create management company
;; macro: party.create
(entity.create :name "ManCo S.a r.l." :type "LEGAL" :jurisdiction "LU" :as @manco)

;; intent: Assign ManCo role
;; macro: structure.assign-role
(cbu-role.assign :cbu-id @fund :entity-id @manco :role "MANAGEMENT_COMPANY" :effective-date "2024-01-01")
```

### KYC Case
```clojure
;; intent: Open KYC case for investor
;; macro: case.open
(kyc-case.create :name "Investor Onboarding" :type "INVESTOR_ONBOARDING" :cbu-id @fund :as @case)

;; intent: Add subject to case
;; macro: case.add-subject
(kyc-case.add-subject :case-id @case :entity-id @investor :role "PRIMARY")
```

## Validation Loop

After generating DSL:
1. User runs `DSL: Validate Form` task in Zed
2. User pastes any diagnostics back
3. You fix ONLY what diagnostics report - no speculative changes
4. Repeat until validation passes

## Start Here

Tell me what you want to create (e.g., "Lux SICAV with ManCo and 2 directors") and I will:
1. Propose the skeleton forms needed
2. Ask for any missing required details
3. Generate the DSL incrementally with proper annotations
