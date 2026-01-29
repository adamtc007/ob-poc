# Zed Tree-sitter "Superpack" + DSL Golden Examples: Implementation TODO

## Overview

This TODO delivers a best-in-class DSL editing experience in Zed by shipping:
1. A real DSL tree-sitter grammar (not Clojure)
2. Full Zed Tree-sitter query pack (highlights, brackets, indents, outline, textobjects, runnables)
3. Golden DSL examples with annotation patterns that Zed Assistant uses for context
4. Supporting docs (style guide, agent rules, setup instructions)

**Dependency**: This TODO assumes the grammar fixes from `docs/LSP_ALIGNMENT_TODO.md` Phase 5.1 
are complete (specifically: `:as` parsed as dedicated `as_binding` node, not `keyword + symbol_ref`).

---

## Phase 0: Acceptance Criteria

### Zed Recognition
- [ ] `.dsl`, `.obl`, `.onboard` files open with DSL syntax highlighting (not Clojure)
- [ ] Language picker shows "DSL" or "OB-POC DSL"

### Editing Experience
- [ ] Rainbow brackets work for `()[]{}` 
- [ ] Bracket matching works (jump to matching paren)
- [ ] Auto-indent feels Lisp-like (indent inside forms)
- [ ] Comment toggle uses `;;`

### Outline & Navigation
- [ ] Outline panel lists each `domain.verb` call
- [ ] Binding shown: `cbu.create (@fund)` or similar
- [ ] Preceding `;;` comments appear as context in outline

### Agent/Assistant Integration
- [ ] Zed Assistant sees `@annotation` from preceding comments
- [ ] Edits preserve annotation blocks
- [ ] Golden examples parseable by both tree-sitter and NOM

### Runnables
- [ ] Run button appears beside top-level forms
- [ ] Tasks execute validate/expand/format commands

---

## Phase 1: Zed Extension Structure

### 1.1 Create Extension Directory Structure

**Path**: `rust/crates/dsl-lsp/zed-extension/`

```
zed-extension/
├── extension.toml              # Extension manifest
├── languages/
│   └── dsl/
│       ├── config.toml         # Language config
│       ├── highlights.scm      # Syntax highlighting
│       ├── brackets.scm        # Bracket matching + rainbow
│       ├── indents.scm         # Auto-indentation
│       ├── outline.scm         # Outline panel + @annotation
│       ├── textobjects.scm     # Vim-style text objects
│       ├── overrides.scm       # Scope-specific settings
│       ├── runnables.scm       # Run buttons
│       └── injections.scm      # (optional) embedded languages
└── snippets/
    └── dsl.json                # Code snippets
```

---

### 1.2 Create extension.toml

**File**: `rust/crates/dsl-lsp/zed-extension/extension.toml`

```toml
[package]
id = "ob-poc-dsl"
name = "OB-POC DSL"
description = "Language support for the OB-POC KYC/AML onboarding DSL"
version = "0.1.0"
schema_version = 1
authors = ["BNY Mellon Enterprise Onboarding Team"]
repository = "https://github.com/your-org/ob-poc"

[grammars.dsl]
# For local development, use file:// URL
# For publishing, use git repository + rev
repository = "https://github.com/your-org/ob-poc"
rev = "main"
path = "rust/crates/dsl-lsp/tree-sitter-dsl"

[language_servers.dsl-lsp]
name = "DSL Language Server"
languages = ["DSL"]
```

---

### 1.3 Create Language Config

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/config.toml`

```toml
name = "DSL"
grammar = "dsl"
path_suffixes = ["dsl", "obl", "onboard"]
line_comments = [";;"]
block_comment = [";; ", ""]
autoclose_before = ")]}\""
brackets = [
    { start = "(", end = ")", close = true, newline = true },
    { start = "[", end = "]", close = true, newline = true },
    { start = "{", end = "}", close = true, newline = true },
    { start = "\"", end = "\"", close = true, newline = false, not_in = ["string", "comment"] },
]
word_characters = ["@", ":", "-", "_", "."]
tab_size = 2
hard_tabs = false
```

---

## Phase 2: Tree-sitter Query Files

### 2.1 highlights.scm

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/highlights.scm`

```scheme
;; =============================================================================
;; DSL Syntax Highlighting for Zed
;; =============================================================================

;; Comments
(comment) @comment

;; Verb names (domain.verb)
(verb_name) @function

;; Keywords (:name, :cbu-id, etc.)
(keyword) @property

;; The :as keyword specifically
(as_binding ":as" @keyword.special)

;; Symbol references (@fund, @entity)
(symbol_ref) @variable

;; Symbol in binding position (:as @fund) - special highlight
(as_binding (symbol_ref) @variable.special)

;; String literals
(string) @string

;; Escape sequences within strings
(string (escape_sequence) @string.escape)

;; Number literals
(number) @number

;; Boolean literals
(boolean) @constant.builtin

;; Null literal
(null_literal) @constant.builtin

;; Punctuation - brackets
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket

;; Punctuation - delimiters
"," @punctuation.delimiter
"." @punctuation.delimiter
```

---

### 2.2 brackets.scm

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/brackets.scm`

```scheme
;; =============================================================================
;; Bracket Matching + Rainbow Brackets for Zed
;; =============================================================================

;; Parentheses (S-expressions)
("(" @open)
(")" @close)

;; Square brackets (arrays/lists)
("[" @open)
("]" @close)

;; Curly braces (maps)
("{" @open)
("}" @close)
```

---

### 2.3 indents.scm

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/indents.scm`

```scheme
;; =============================================================================
;; Auto-indentation Rules for Zed (Lisp-style)
;; =============================================================================

;; Indent inside verb calls
(verb_call
  "(" @indent
  ")" @end)

;; Indent inside arrays
(array
  "[" @indent
  "]" @end)

;; Indent inside maps
(map
  "{" @indent
  "}" @end)

;; Nested verb calls also indent
(verb_call
  (verb_call) @indent)
```

---

### 2.4 outline.scm (Critical for Assistant integration)

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/outline.scm`

```scheme
;; =============================================================================
;; Outline Panel + @annotation for Zed Assistant
;; =============================================================================
;; 
;; IMPORTANT: Preceding comments become @annotation context that Zed Assistant
;; uses when generating/modifying code. This is how we ground the agent.
;;
;; Pattern in DSL files:
;;   ;; intent: Create the main custody banking unit
;;   ;; macro: operator.new-cbu
;;   (cbu.ensure :name "Apex Fund" :as @fund)
;;
;; The ;; comments above become annotation context for the form.
;; =============================================================================

;; Each top-level verb call is an outline item
(verb_call
  (verb_name) @name) @item

;; If the verb call has a binding, show it as context
(verb_call
  (verb_name) @name
  (as_binding
    (symbol_ref) @context.extra)) @item

;; Capture preceding comment block as annotation
;; This is what makes Zed Assistant edits stable
(comment)+ @annotation
  . (verb_call) @item
```

---

### 2.5 textobjects.scm

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/textobjects.scm`

```scheme
;; =============================================================================
;; Text Objects for Vim-style motions (vaf, vif, etc.)
;; =============================================================================

;; Verb calls are "functions" for text object purposes
(verb_call) @function.outer

;; Inside a verb call (excluding parens)
(verb_call
  "(" 
  (_)* @function.inner
  ")") 

;; Arrays as "list" objects
(array) @list.outer

(array
  "[" 
  (_)* @list.inner
  "]")

;; Maps as "block" objects  
(map) @block.outer

(map
  "{"
  (_)* @block.inner
  "}")

;; Comments
(comment) @comment.outer
```

---

### 2.6 overrides.scm

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/overrides.scm`

```scheme
;; =============================================================================
;; Scope-specific Settings Overrides
;; =============================================================================

;; Inside strings: disable certain completions, change autoclose
(string) @string

;; Inside comments: disable most features
(comment) @comment
```

---

### 2.7 runnables.scm

**File**: `rust/crates/dsl-lsp/zed-extension/languages/dsl/runnables.scm`

```scheme
;; =============================================================================
;; Runnables: Add run buttons beside forms in Zed
;; =============================================================================
;;
;; Zed shows a "Run" button beside nodes tagged with @run.
;; The captured fields become environment variables prefixed with ZED_CUSTOM_*.
;; 
;; Tasks in .zed/tasks.json can reference these to run validation, expansion, etc.
;; =============================================================================

;; Every top-level verb call gets a run button
(verb_call
  (verb_name) @run
  (#set! tag "dsl-form"))

;; Capture verb name for task use
(verb_call
  (verb_name) @_verb_name
  (#set! tag "dsl-form")
  (#set! capture.verb "$_verb_name"))

;; If there's a binding, capture that too
(verb_call
  (verb_name) @_verb_name
  (as_binding (symbol_ref) @_binding)
  (#set! tag "dsl-form-with-binding")
  (#set! capture.verb "$_verb_name")
  (#set! capture.binding "$_binding"))
```

---

## Phase 3: Snippets

### 3.1 Create Snippets File

**File**: `rust/crates/dsl-lsp/zed-extension/snippets/dsl.json`

```json
{
  "CBU Create": {
    "prefix": ["cbu", "cbu.ensure"],
    "body": [
      ";; intent: ${1:Create custody banking unit}",
      "(cbu.ensure",
      "  :name \"${2:Fund Name}\"",
      "  :jurisdiction \"${3|LU,IE,US,GB,DE|}\"",
      "  :client-type \"${4|FUND,CORPORATE,INDIVIDUAL|}\"",
      "  :as @${5:cbu})"
    ],
    "description": "Create a new Custody Banking Unit"
  },
  "Entity Person": {
    "prefix": ["entity.person", "person"],
    "body": [
      ";; intent: ${1:Create natural person entity}",
      "(entity.create-proper-person",
      "  :first-name \"${2:First}\"",
      "  :last-name \"${3:Last}\"",
      "  :date-of-birth \"${4:1980-01-01}\"",
      "  :nationality \"${5|US,GB,DE,LU,IE|}\"",
      "  :as @${6:person})"
    ],
    "description": "Create a natural person entity"
  },
  "Entity Company": {
    "prefix": ["entity.company", "company", "corp"],
    "body": [
      ";; intent: ${1:Create corporate entity}",
      "(entity.create-limited-company",
      "  :name \"${2:Company Name}\"",
      "  :jurisdiction \"${3|US,GB,DE,LU,IE|}\"",
      "  :registration-number \"${4:REG123}\"",
      "  :as @${5:company})"
    ],
    "description": "Create a limited company entity"
  },
  "Assign Role": {
    "prefix": ["role", "assign-role"],
    "body": [
      ";; intent: ${1:Assign role to entity}",
      "(cbu.assign-role",
      "  :cbu-id @${2:cbu}",
      "  :entity-id @${3:entity}",
      "  :role \"${4|DIRECTOR,UBO,AUTHORIZED_SIGNATORY,MANAGER|}\")"
    ],
    "description": "Assign a role to an entity within a CBU"
  },
  "KYC Case": {
    "prefix": ["kyc", "case"],
    "body": [
      ";; intent: ${1:Create KYC case for onboarding}",
      "(kyc-case.create",
      "  :cbu-id @${2:cbu}",
      "  :case-type \"${3|NEW_CLIENT,PERIODIC_REVIEW,TRIGGER_EVENT|}\"",
      "  :entities [${4:@entity}]",
      "  :as @${5:case})"
    ],
    "description": "Create a KYC case"
  },
  "Intent Comment Block": {
    "prefix": [";;", "intent", "annotation"],
    "body": [
      ";; intent: ${1:What this form accomplishes}",
      ";; macro: ${2:operator.verb-name}",
      ";; constraints: ${3:Business rules or invariants}"
    ],
    "description": "Add an intent/annotation comment block (used by Zed Assistant)"
  }
}
```

---

## Phase 4: Zed Tasks (Runnables Integration)

### 4.1 Create Repository Tasks

**File**: `.zed/tasks.json` (at repository root)

```json
{
  "tasks": [
    {
      "label": "DSL: Validate Form",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "validate", "--file", "$ZED_FILE", "--offset", "$ZED_CUSTOM_offset"],
      "tags": ["dsl-form", "dsl-form-with-binding"],
      "reveal": "always"
    },
    {
      "label": "DSL: Expand Macro",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "expand", "--file", "$ZED_FILE", "--form", "$ZED_CUSTOM_verb"],
      "tags": ["dsl-form", "dsl-form-with-binding"],
      "reveal": "always"
    },
    {
      "label": "DSL: Show Expansion Diff",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "expand", "--file", "$ZED_FILE", "--form", "$ZED_CUSTOM_verb", "--diff"],
      "tags": ["dsl-form", "dsl-form-with-binding"],
      "reveal": "always"
    },
    {
      "label": "DSL: Format File",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "fmt", "$ZED_FILE"],
      "reveal": "never",
      "allow_concurrent_runs": false
    },
    {
      "label": "DSL: Lint File",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "lint", "--file", "$ZED_FILE", "--format", "json"],
      "tags": ["dsl-form"],
      "reveal": "always"
    },
    {
      "label": "DSL: Validate All Golden Examples",
      "command": "cargo",
      "args": ["run", "-p", "dsl-cli", "--", "validate", "--dir", "docs/dsl/golden/"],
      "reveal": "always"
    }
  ]
}
```

### 4.2 CLI Commands Required

The tasks above assume a `dsl-cli` binary with these subcommands:

```bash
# Validate a single file or form at offset
dsl-cli validate --file path.dsl [--offset N]

# Expand a macro form to primitive DSL
dsl-cli expand --file path.dsl --form "cbu.ensure" [--diff]

# Format a DSL file
dsl-cli fmt path.dsl [--check]

# Lint with machine-readable output
dsl-cli lint --file path.dsl --format json
```

**Note**: If these don't exist yet, create stubs or use existing REPL/validation infrastructure.

---

## Phase 5: Golden Examples Suite

### 5.1 Create Golden Examples Directory

**Path**: `docs/dsl/golden/`

### 5.2 Golden Example Files

#### 00-syntax-tour.dsl

```clojure
;; =============================================================================
;; DSL Syntax Tour
;; =============================================================================
;; This file demonstrates all DSL syntax constructs.
;; Use as a reference and a parse-test fixture.

;; -----------------------------------------------------------------------------
;; Comments: double semicolon required
;; -----------------------------------------------------------------------------

;; intent: Show basic verb call syntax
;; macro: test.echo
(test.echo :message "Hello, DSL!")

;; intent: Demonstrate string escapes
;; constraints: All standard escapes supported
(test.verb :text "Line 1\nLine 2\tTabbed\\Backslash\"Quoted\"")

;; intent: Show numeric types
(test.numbers 
  :integer 42 
  :negative -17 
  :decimal 3.14159
  :negative-decimal -0.5)

;; intent: Show boolean and null literals
(test.flags :active true :deleted false :optional nil)

;; intent: Demonstrate array syntax (both styles)
(test.arrays
  :with-spaces ["a" "b" "c"]
  :with-commas ["x", "y", "z"]
  :mixed ["one" "two", "three"])

;; intent: Demonstrate map syntax
(test.maps :config {:name "Test" :count 42 :enabled true})

;; intent: Nested verb calls
(test.nested :inner (test.echo :message "I'm nested!"))

;; intent: Symbol binding with :as
(test.binding :value "captured" :as @my-symbol)

;; intent: Symbol reference
(test.use-symbol :ref @my-symbol)

;; intent: Complex nesting - list of verb calls
(test.complex :items [
  (test.item :id 1)
  (test.item :id 2)
  (test.item :id 3)
])

;; intent: UUID auto-detection in strings
(test.uuid :id "550e8400-e29b-41d4-a716-446655440000")
```

#### 01-cbu-create.dsl

```clojure
;; =============================================================================
;; Hello World: Create a Custody Banking Unit
;; =============================================================================

;; intent: Create the primary fund structure for onboarding
;; macro: operator.new-cbu
;; constraints: Name must be unique within jurisdiction
(cbu.ensure
  :name "Apex Growth Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :as @fund)
```

#### 02-roles-and-links.dsl

```clojure
;; =============================================================================
;; Entity Creation and Role Assignment
;; =============================================================================

;; intent: Create the fund structure
;; macro: operator.new-cbu
(cbu.ensure :name "Apex Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)

;; intent: Create the fund manager (natural person)
;; macro: operator.new-person
;; constraints: Must have nationality for AML screening
(entity.create-proper-person
  :first-name "John"
  :last-name "Smith"
  :date-of-birth "1975-06-15"
  :nationality "GB"
  :as @john)

;; intent: Create the management company
;; macro: operator.new-company
(entity.create-limited-company
  :name "Apex Management Ltd"
  :jurisdiction "LU"
  :registration-number "B123456"
  :as @manco)

;; intent: Assign John as director of the fund
;; macro: operator.assign-role
;; constraints: Directors require enhanced due diligence
(cbu.assign-role
  :cbu-id @fund
  :entity-id @john
  :role "DIRECTOR")

;; intent: Link management company to fund
;; macro: operator.assign-role
(cbu.assign-role
  :cbu-id @fund
  :entity-id @manco
  :role "MANAGEMENT_COMPANY")
```

#### 03-kyc-case-sheet.dsl

```clojure
;; =============================================================================
;; KYC Case Creation with Entity List
;; =============================================================================

;; intent: Create fund and related entities for KYC
(cbu.ensure :name "Horizon Fund" :jurisdiction "IE" :client-type "FUND" :as @fund)

(entity.create-proper-person
  :first-name "Alice"
  :last-name "Director"
  :date-of-birth "1980-03-20"
  :nationality "IE"
  :as @alice)

(entity.create-proper-person
  :first-name "Bob"
  :last-name "UBO"
  :date-of-birth "1965-11-10"
  :nationality "US"
  :as @bob)

;; intent: Create KYC case for new client onboarding
;; macro: operator.open-kyc-case
;; constraints: All UBOs must be included in entities list
(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :priority "HIGH"
  :entities [@alice @bob]
  :config {:require-source-of-wealth true :aml-tier "ENHANCED"}
  :as @case)
```

#### 04-ubo-mini-graph.dsl

```clojure
;; =============================================================================
;; UBO Ownership Graph (Mini Example)
;; =============================================================================
;; Demonstrates ownership chain discovery for beneficial ownership.

;; intent: Target company being onboarded
(entity.create-limited-company
  :name "Target OpCo Ltd"
  :jurisdiction "GB"
  :registration-number "UK98765"
  :as @target)

;; intent: Intermediate holding company (75% owner of Target)
(entity.create-limited-company
  :name "Midco Holdings BV"
  :jurisdiction "NL"
  :registration-number "NL12345"
  :as @midco)

;; intent: Ultimate beneficial owner (100% of Midco)
;; constraints: UBO threshold is 25% - this person controls 75%
(entity.create-proper-person
  :first-name "Ultimate"
  :last-name "Owner"
  :date-of-birth "1960-01-01"
  :nationality "NL"
  :as @ubo)

;; intent: Record ownership: Midco owns 75% of Target
;; macro: operator.record-ownership
(ownership.record
  :owner-id @midco
  :owned-id @target
  :percentage 75.0
  :ownership-type "DIRECT")

;; intent: Record ownership: UBO owns 100% of Midco
(ownership.record
  :owner-id @ubo
  :owned-id @midco
  :percentage 100.0
  :ownership-type "DIRECT")

;; intent: Calculate effective UBO ownership (should show 75%)
;; macro: operator.calculate-ubo
(ubo.calculate :target-id @target :threshold 25.0)
```

#### 05-otc-isda-csa.dsl

```clojure
;; =============================================================================
;; OTC Derivatives Onboarding (ISDA/CSA)
;; =============================================================================
;; Realistic example: onboarding a hedge fund for OTC trading.

;; intent: Create the hedge fund client
;; macro: operator.new-cbu
(cbu.ensure
  :name "Quantum Alpha Fund LP"
  :jurisdiction "US"
  :client-type "FUND"
  :fund-type "HEDGE_FUND"
  :as @fund)

;; intent: General Partner entity
(entity.create-limited-company
  :name "Quantum Alpha GP LLC"
  :jurisdiction "US"
  :registration-number "DE-12345678"
  :as @gp)

;; intent: Link GP to fund
(cbu.assign-role :cbu-id @fund :entity-id @gp :role "GENERAL_PARTNER")

;; intent: Create KYC case for hedge fund onboarding
(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :priority "HIGH"
  :entities [@gp]
  :as @case)

;; intent: ISDA Master Agreement requirement
;; macro: operator.require-document
;; constraints: Must be 2002 ISDA for new relationships
(document.require
  :case-id @case
  :document-type "ISDA_MASTER"
  :config {:version "2002" :governing-law "NY"})

;; intent: Credit Support Annex requirement
;; macro: operator.require-document
(document.require
  :case-id @case
  :document-type "CSA"
  :config {:type "VM_CSA" :threshold-currency "USD" :threshold-amount 0})

;; intent: LEI verification task
;; macro: operator.require-verification
(verification.require
  :case-id @case
  :verification-type "LEI"
  :entity-id @fund)
```

#### 06-macro-v2-roundtrip.dsl

```clojure
;; =============================================================================
;; Macro v2 Expansion Example
;; =============================================================================
;; Shows the relationship between operator-level macros and primitive DSL.

;; -----------------------------------------------------------------------------
;; MACRO INTENT (what the operator writes in YAML playbook)
;; -----------------------------------------------------------------------------
;; operator.onboard-fund:
;;   name: "Test Fund"
;;   jurisdiction: LU
;;   manager:
;;     first-name: "Jane"
;;     last-name: "Manager"

;; -----------------------------------------------------------------------------
;; EXPANDED DSL (what the macro expands to)
;; -----------------------------------------------------------------------------

;; intent: [AUTO-EXPANDED] Create fund structure
;; macro: operator.onboard-fund (fragment 1/3)
(cbu.ensure
  :name "Test Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :as @fund)

;; intent: [AUTO-EXPANDED] Create fund manager
;; macro: operator.onboard-fund (fragment 2/3)
(entity.create-proper-person
  :first-name "Jane"
  :last-name "Manager"
  :as @manager)

;; intent: [AUTO-EXPANDED] Link manager to fund
;; macro: operator.onboard-fund (fragment 3/3)
(cbu.assign-role
  :cbu-id @fund
  :entity-id @manager
  :role "MANAGER")
```

#### 90-error-fixtures.dsl

```clojure
;; =============================================================================
;; Error Fixtures (Intentionally Invalid)
;; =============================================================================
;; These examples test error reporting. Each section has ONE error.
;; Run each section individually to test diagnostics.

;; --- ERROR: Unclosed parenthesis ---
;; Uncomment to test:
;; (cbu.ensure :name "Test"

;; --- ERROR: Unclosed string ---
;; Uncomment to test:
;; (cbu.ensure :name "Unclosed string)

;; --- ERROR: Missing value after keyword ---
;; Uncomment to test:
;; (cbu.ensure :name)

;; --- ERROR: Invalid token ---
;; Uncomment to test:
;; (cbu.ensure :name !!invalid!!)

;; --- ERROR: Unknown verb (semantic, not syntax) ---
;; Uncomment to test:
;; (cbu.nonexistent-verb :foo "bar")

;; --- VALID: For baseline comparison ---
(test.valid :message "This parses correctly")
```

---

## Phase 6: Documentation

### 6.1 DSL Style Guide

**File**: `docs/DSL_STYLE_GUIDE.md`

```markdown
# DSL Style Guide

## Formatting

### Indentation
- Use 2 spaces (no tabs)
- Indent arguments when verb call spans multiple lines
- Align arguments at same indentation level

### Single-line vs Multi-line
- Single line if ≤ 80 characters and ≤ 2 arguments
- Multi-line otherwise, one argument per line

```clojure
;; Single line (short)
(cbu.ensure :name "Test" :as @cbu)

;; Multi-line (longer or more args)
(entity.create-proper-person
  :first-name "John"
  :last-name "Smith"
  :date-of-birth "1980-01-01"
  :nationality "US"
  :as @person)
```

### Binding Position
- `:as @symbol` is ALWAYS the last argument
- Never place other arguments after `:as`

### Lists and Maps
- Short lists on one line: `["a" "b" "c"]`
- Long lists with one item per line
- Maps always multi-line if > 2 entries

## Naming Conventions

### Symbol Bindings (@name)
- Use descriptive kebab-case: `@fund`, `@main-cbu`, `@john-smith`
- Prefix with type hint when ambiguous: `@entity-john`, `@cbu-apex`
- Keep short for frequently referenced symbols

### Common Patterns
| Type | Pattern | Examples |
|------|---------|----------|
| CBU | `@fund`, `@cbu`, `@client` | `@apex-fund`, `@main-cbu` |
| Person | `@firstname` or `@role` | `@john`, `@director`, `@ubo` |
| Company | `@company` or `@abbrev` | `@manco`, `@holdco`, `@target` |
| Case | `@case` | `@kyc-case`, `@review-case` |

## Comments

### Annotation Block (Required for Reviewable Sheets)
Every top-level form in a production playbook MUST have:

```clojure
;; intent: <1 sentence describing business goal>
;; macro: <operator.verb-name or primitive verb>
;; constraints: <business rules, optional>
(verb.call ...)
```

This is NOT optional documentation—Zed Assistant uses these annotations
for context when generating or modifying code.

### Inline Comments
Use for clarifying complex logic within a form:

```clojure
(ownership.record
  :owner-id @holdco
  :owned-id @opco
  :percentage 100.0  ;; Full ownership, no minority
  :ownership-type "DIRECT")
```
```

---

### 6.2 Agent Rules

**File**: `docs/AGENT_RULES.md`

```markdown
# DSL Agent Rules

## Code Generation Loop

When generating or modifying DSL code, follow this loop:

1. **Select verb/macro** from top-K candidates based on intent
2. **Slot-fill required args only** - don't add optional args unless specified
3. **Generate minimal skeleton** - prefer less code over more
4. **Run validate/lint** via LSP or CLI
5. **Apply smallest fix** from diagnostics
6. **Stop when clean** and user has reviewed diff

## Invariants

### Never Violate
- `:as` binding must be last argument
- Symbol refs must reference defined symbols (no forward refs)
- Required args must be present
- String values must be properly escaped

### Prefer
- Fewer statements over more
- Explicit over implicit
- Primitive verbs over complex macros when simple

## Annotation Preservation

When editing existing DSL:
- PRESERVE existing `;;` annotation blocks
- UPDATE annotations if intent changes
- ADD annotations to forms that lack them

## Error Recovery

When validation fails:
1. Read the diagnostic message
2. Identify the exact span
3. Apply minimal fix (don't rewrite entire form)
4. Re-validate
5. Max 3 fix attempts before asking user

## Symbol Scope

- Symbols are scoped to the current sheet/file
- Cross-file references require explicit import (future feature)
- Don't assume symbols from other files exist
```

---

### 6.3 Zed Setup Guide

**File**: `docs/ZED_SETUP.md`

```markdown
# Zed Setup for DSL Development

## Prerequisites

- Zed editor (latest version)
- Rust toolchain (for LSP)
- Node.js (for tree-sitter grammar generation)

## Installing the Extension

### Option 1: Local Development

1. Build the tree-sitter grammar:
   ```bash
   cd rust/crates/dsl-lsp/tree-sitter-dsl
   npx tree-sitter generate
   ```

2. Link the extension to Zed:
   ```bash
   ln -s /path/to/ob-poc/rust/crates/dsl-lsp/zed-extension \
         ~/.config/zed/extensions/ob-poc-dsl
   ```

3. Restart Zed

### Option 2: Built Extension (Future)

```
Extensions > Search "OB-POC DSL" > Install
```

## Configuring the LSP

The extension auto-starts the LSP. To configure:

1. Open Zed settings (`Cmd+,`)
2. Add LSP configuration:

```json
{
  "lsp": {
    "dsl-lsp": {
      "binary": {
        "path": "/path/to/dsl-lsp"
      },
      "initialization_options": {
        "config_dir": "/path/to/ob-poc/rust/config"
      }
    }
  }
}
```

## File Associations

The extension registers these file types:
- `.dsl` - Primary DSL files
- `.obl` - Onboarding playbooks
- `.onboard` - Onboarding sheets

## Using Run Buttons

1. Open a `.dsl` file
2. Look for ▶ buttons beside verb calls
3. Click to run validation, expansion, or other tasks
4. Output appears in the terminal panel

## Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Format file | `Cmd+Shift+F` (if configured) |
| Go to definition | `Cmd+Click` or `F12` |
| Find references | `Shift+F12` |
| Rename symbol | `F2` |
| Show outline | `Cmd+Shift+O` |

## Troubleshooting

### LSP Not Starting
- Check `~/.config/zed/logs/` for errors
- Ensure `dsl-lsp` binary is in PATH or configured
- Verify DSL_CONFIG_DIR environment variable

### Highlighting Not Working
- Ensure tree-sitter grammar was generated
- Check extension is linked correctly
- Restart Zed after changes

### Run Buttons Missing
- Verify `.zed/tasks.json` exists at repo root
- Check runnables.scm syntax
- Ensure file is recognized as DSL (check language in status bar)
```

---

## Phase 7: Tests

### 7.1 Tree-sitter Parse Tests

**File**: `rust/crates/dsl-lsp/tree-sitter-dsl/test/corpus/golden.txt`

Tree-sitter uses a specific test format:

```
================================================================================
Basic verb call
================================================================================

(cbu.ensure :name "Test" :as @fund)

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg
      (keyword)
      (string))
    (as_binding
      (symbol_ref))))

================================================================================
Nested verb call in list
================================================================================

(test.verb :items [(inner.call :x 1)])

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg
      (keyword)
      (array
        (verb_call
          (verb_name)
          (keyword_arg
            (keyword)
            (number)))))))

================================================================================
Map literal
================================================================================

(test.verb :config {:name "Test" :count 42})

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg
      (keyword)
      (map
        (map_entry
          (keyword)
          (string))
        (map_entry
          (keyword)
          (number))))))

================================================================================
As binding must be separate node
================================================================================

(cbu.ensure :name "Fund" :as @cbu)

--------------------------------------------------------------------------------

(source_file
  (verb_call
    (verb_name)
    (keyword_arg
      (keyword)
      (string))
    (as_binding
      (symbol_ref))))
```

### 7.2 Golden Example Validation Test

**File**: `rust/crates/dsl-lsp/tests/golden_validation.rs`

```rust
//! Validate that all golden examples parse correctly in both
//! tree-sitter and NOM parser.

use std::fs;
use std::path::Path;
use ob_poc::dsl_v2::parse_program;

const GOLDEN_DIR: &str = "../../docs/dsl/golden";

#[test]
fn all_golden_examples_parse_with_nom() {
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_DIR);
    
    for entry in fs::read_dir(&golden_path).expect("golden dir exists") {
        let entry = entry.unwrap();
        let path = entry.path();
        
        if path.extension().map(|e| e == "dsl").unwrap_or(false) {
            let filename = path.file_name().unwrap().to_string_lossy();
            
            // Skip error fixtures (they're intentionally invalid)
            if filename.starts_with("90-") {
                continue;
            }
            
            let content = fs::read_to_string(&path)
                .expect(&format!("read {}", filename));
            
            let result = parse_program(&content);
            assert!(
                result.is_ok(),
                "Golden example {} failed to parse: {:?}",
                filename,
                result.err()
            );
        }
    }
}

#[test]
fn golden_examples_have_annotation_comments() {
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_DIR);
    
    for entry in fs::read_dir(&golden_path).expect("golden dir exists") {
        let entry = entry.unwrap();
        let path = entry.path();
        
        if path.extension().map(|e| e == "dsl").unwrap_or(false) {
            let filename = path.file_name().unwrap().to_string_lossy();
            
            // Skip syntax tour and error fixtures
            if filename.starts_with("00-") || filename.starts_with("90-") {
                continue;
            }
            
            let content = fs::read_to_string(&path)
                .expect(&format!("read {}", filename));
            
            // Check for at least one ";; intent:" annotation
            assert!(
                content.contains(";; intent:"),
                "Golden example {} missing ;; intent: annotation",
                filename
            );
        }
    }
}
```

---

## Dependency Order Summary

```
Phase 1.1-1.3 (Extension structure)
    ↓
Phase 2.1-2.7 (Query files) ←── Depends on grammar from LSP_ALIGNMENT_TODO Phase 5.1
    ↓
Phase 3.1 (Snippets)
    ↓
Phase 4.1-4.2 (Tasks + CLI hooks)
    ↓
Phase 5.1-5.2 (Golden examples)
    ↓
Phase 6.1-6.3 (Documentation)
    ↓
Phase 7.1-7.2 (Tests)
```

---

## Files Created Summary

| Path | Description |
|------|-------------|
| `zed-extension/extension.toml` | Extension manifest |
| `zed-extension/languages/dsl/config.toml` | Language config |
| `zed-extension/languages/dsl/highlights.scm` | Syntax highlighting |
| `zed-extension/languages/dsl/brackets.scm` | Bracket matching |
| `zed-extension/languages/dsl/indents.scm` | Auto-indentation |
| `zed-extension/languages/dsl/outline.scm` | Outline + annotations |
| `zed-extension/languages/dsl/textobjects.scm` | Vim text objects |
| `zed-extension/languages/dsl/overrides.scm` | Scope overrides |
| `zed-extension/languages/dsl/runnables.scm` | Run buttons |
| `zed-extension/snippets/dsl.json` | Code snippets |
| `.zed/tasks.json` | Repository tasks |
| `docs/dsl/golden/*.dsl` | Golden examples (7 files) |
| `docs/DSL_STYLE_GUIDE.md` | Style guide |
| `docs/AGENT_RULES.md` | Agent behavior rules |
| `docs/ZED_SETUP.md` | Setup instructions |
| `tree-sitter-dsl/test/corpus/golden.txt` | Tree-sitter tests |
| `tests/golden_validation.rs` | NOM parse tests |

---

## Notes

### Integration with LSP TODO

This TODO complements `docs/LSP_ALIGNMENT_TODO.md`:
- LSP TODO fixes parser correctness (spans, encoding, errors)
- This TODO fixes editor experience (highlighting, outline, runnables)

The grammar changes in LSP TODO Phase 5.1 (`as_binding` as dedicated node) are 
**required** before the query files here will work correctly.

### Why @annotation Matters

Zed Assistant uses `@annotation` captures from `outline.scm` as context when 
generating or modifying code. By requiring `;;` intent comments before forms:

1. Assistant sees business intent, not just syntax
2. Edits preserve the intent blocks
3. Generated code follows the annotation pattern

This is the mechanism for "grounding" the agent without fine-tuning.
