# Agent Architecture (ASCII)

> How agents generate DSL from natural language
> Reference doc for Claude Code when working on agentic pipeline

---

## 1. Agent System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AGENT SYSTEM OVERVIEW                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   USER INPUT                                                                 │
│   ──────────                                                                 │
│   "Add John Smith as director of Acme Fund"                                 │
│        │                                                                     │
│        │                                                                     │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                    AGENTIC PIPELINE                                   │  │
│   │  rust/crates/ob-agentic/                                             │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │                                                                       │  │
│   │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │  │
│   │   │  TOKENIZER  │───▶│   PARSER    │───▶│  GENERATOR  │              │  │
│   │   │  (lexicon)  │    │ (nom grammar│    │ (DSL emit)  │              │  │
│   │   └─────────────┘    └─────────────┘    └─────────────┘              │  │
│   │                                                                       │  │
│   │   OR (fallback):                                                      │  │
│   │                                                                       │  │
│   │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │  │
│   │   │  RESEARCH   │───▶│   CLAUDE    │───▶│   VALIDATOR │              │  │
│   │   │   MACRO     │    │    API      │    │  (parse DSL)│              │  │
│   │   └─────────────┘    └─────────────┘    └─────────────┘              │  │
│   │                                                                       │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│        │                                                                     │
│        │ Generated DSL                                                      │
│        ▼                                                                     │
│   (cbu.assign-role :cbu-id "Acme Fund" :entity-id "John Smith"              │
│                    :role "director")                                        │
│        │                                                                     │
│        │                                                                     │
│        ▼                                                                     │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │                      DSL PIPELINE                                     │  │
│   │  (Parser → Compiler → Executor → PostgreSQL)                         │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Two Agent Modes

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          TWO AGENT MODES                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   MODE 1: LEXICON PIPELINE (Deterministic)                                  │
│   ─────────────────────────────────────────                                  │
│   Fast, local, no API calls                                                  │
│   Best for: Known patterns, simple commands                                 │
│                                                                              │
│   ┌───────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐         │
│   │   Input   │───▶│ Tokenize  │───▶│   Parse   │───▶│  Generate │         │
│   │  (text)   │    │ (lexicon) │    │(nom grammar)│   │   (DSL)   │         │
│   └───────────┘    └───────────┘    └───────────┘    └───────────┘         │
│                                                                              │
│   Supported intents:                                                         │
│   - RoleAssign:       "Add X as director"                                   │
│   - CounterpartyCreate: "Add Goldman as counterparty"                       │
│   - IsdaEstablish:    "Set up ISDA with X under NY law"                    │
│   - CsaAdd:           "Add VM CSA"                                          │
│   - ProductAdd:       "Add custody product"                                 │
│   - UniverseDefine:   "Trade US equities"                                   │
│                                                                              │
│   ──────────────────────────────────────────────────────────────────────    │
│                                                                              │
│   MODE 2: LLM PIPELINE (Generative)                                         │
│   ──────────────────────────────────                                         │
│   Flexible, API-based, handles complex queries                              │
│   Best for: Fuzzy queries, complex scenarios, research                      │
│                                                                              │
│   ┌───────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐         │
│   │   Input   │───▶│  Context  │───▶│  Claude   │───▶│  Validate │         │
│   │  (text)   │    │  (schema, │    │   API     │    │  (parse)  │         │
│   │           │    │   verbs)  │    │           │    │           │         │
│   └───────────┘    └───────────┘    └───────────┘    └───────────┘         │
│                                                                              │
│   Features:                                                                  │
│   - Research macros (fuzzy discovery → deterministic DSL)                   │
│   - Full verb reference in context                                          │
│   - Schema awareness                                                         │
│   - Multi-step reasoning                                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Lexicon Pipeline Detail

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       LEXICON PIPELINE DETAIL                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Input: "Add John Smith as director of Acme Fund"                          │
│                                                                              │
│   STEP 1: TOKENIZATION                                                       │
│   ────────────────────                                                       │
│   rust/crates/ob-agentic/src/lexicon/tokenizer.rs                           │
│                                                                              │
│   Uses: rust/config/agent/lexicon.yaml                                      │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ Lexicon Categories:                                                  │   │
│   │                                                                      │   │
│   │ verbs:     [add, create, set, establish, remove, assign, ...]       │   │
│   │ roles:     [director, signatory, ubo, shareholder, ...]             │   │
│   │ products:  [custody, fund accounting, collateral, ...]              │   │
│   │ csa_types: [vm, im, variation margin, initial margin]               │   │
│   │ laws:      [ny, new york, english, irish, ...]                      │   │
│   │ articles:  [a, an, the, of, for, ...]                               │   │
│   │ preps:     [as, with, under, to, from, ...]                         │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Token Stream:                                                              │
│   ┌─────────┬─────────┬────────────┬──────────┬─────────┬──────────────┐   │
│   │  VERB   │  ENTITY │    PREP    │   ROLE   │  PREP   │    ENTITY    │   │
│   │  "add"  │ "John   │    "as"    │"director"│  "of"   │ "Acme Fund"  │   │
│   │         │  Smith" │            │          │         │              │   │
│   └─────────┴─────────┴────────────┴──────────┴─────────┴──────────────┘   │
│                                                                              │
│   STEP 2: ENTITY RESOLUTION                                                  │
│   ─────────────────────────                                                  │
│   rust/crates/ob-agentic/src/lexicon/db_resolver.rs                         │
│                                                                              │
│   "John Smith" → Check entities table → entity_id or CREATE marker          │
│   "Acme Fund"  → Check cbus table → cbu_id                                  │
│                                                                              │
│   STEP 3: GRAMMAR PARSING                                                    │
│   ───────────────────────                                                    │
│   rust/crates/ob-agentic/src/lexicon/intent_parser.rs                       │
│                                                                              │
│   Pattern match with nom combinators:                                       │
│                                                                              │
│   role_assign_pattern =                                                      │
│       VERB("add") >> ENTITY >> PREP("as") >> ROLE >> PREP("of") >> ENTITY  │
│                                                                              │
│   Produces IntentAst:                                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ IntentAst::RoleAssign {                                              │   │
│   │     entity: ResolvedEntity { name: "John Smith", id: None },        │   │
│   │     role: "director",                                                │   │
│   │     target_cbu: ResolvedEntity { name: "Acme Fund", id: Some(...) } │   │
│   │ }                                                                    │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   STEP 4: DSL GENERATION                                                     │
│   ──────────────────────                                                     │
│   rust/crates/ob-agentic/src/lexicon/pipeline.rs                            │
│                                                                              │
│   IntentAst → DSL string:                                                    │
│                                                                              │
│   (entity.create-proper-person :first-name "John" :last-name "Smith"        │
│    :as @john)                                                                │
│   (cbu.assign-role :cbu-id "Acme Fund" :entity-id @john :role "director")   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Research Macro System

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       RESEARCH MACRO SYSTEM                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Purpose: Bridge fuzzy LLM discovery with deterministic DSL execution      │
│                                                                              │
│   Pattern: @research { <natural language query> }                           │
│                                                                              │
│   Example:                                                                   │
│   ─────────                                                                  │
│   @research {                                                                │
│       Find all UBOs of Acme Fund who are PEPs                               │
│       and create screening tasks for each                                    │
│   }                                                                          │
│                                                                              │
│   Execution Flow:                                                            │
│   ───────────────                                                            │
│                                                                              │
│   ┌───────────────────────────────────────────────────────────────────────┐ │
│   │                         RESEARCH MACRO                                 │ │
│   │─────────────────────────────────────────────────────────────────────────│
│   │                                                                        │ │
│   │   ┌────────────┐     ┌────────────┐     ┌────────────┐                │ │
│   │   │   QUERY    │────▶│   CLAUDE   │────▶│   PARSE    │                │ │
│   │   │ + CONTEXT  │     │    API     │     │ RESPONSE   │                │ │
│   │   └────────────┘     └────────────┘     └────────────┘                │ │
│   │         │                                     │                        │ │
│   │         │                                     │                        │ │
│   │         ▼                                     ▼                        │ │
│   │   Context includes:                     Response format:               │ │
│   │   - Verb reference                      - DSL statements               │ │
│   │   - Schema summary                      - Validated syntax             │ │
│   │   - Current CBU context                 - Ready to execute             │ │
│   │   - Discovered entities                                                │ │
│   │                                                                        │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                      │                                       │
│                                      │ Generated DSL                        │
│                                      ▼                                       │
│   ┌───────────────────────────────────────────────────────────────────────┐ │
│   │   (entity.list-ubos :cbu-id "Acme Fund" :as @ubos)                    │ │
│   │   @foreach @ubos as @ubo {                                            │ │
│   │       (screening.check-pep :entity-id @ubo.entity_id :as @result)     │ │
│   │       @if @result.is_pep {                                            │ │
│   │           (task.create :entity-id @ubo.entity_id :type "PEP_REVIEW")  │ │
│   │       }                                                               │ │
│   │   }                                                                   │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                      │                                       │
│                                      │                                       │
│                                      ▼                                       │
│   ┌───────────────────────────────────────────────────────────────────────┐ │
│   │                    DSL PIPELINE (deterministic)                       │ │
│   │                    Parser → Compiler → Executor                       │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│   Key Principle:                                                             │
│   ──────────────                                                             │
│   LLM does DISCOVERY (what to do)                                           │
│   DSL does EXECUTION (how to do it deterministically)                       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Intent Types

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           INTENT TYPES                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   IntentAst (rust/crates/ob-agentic/src/lexicon/intent_ast.rs)              │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                                                                      │   │
│   │  RoleAssign                                                          │   │
│   │  ──────────                                                          │   │
│   │  Pattern: "Add <entity> as <role> [of <cbu>]"                       │   │
│   │  Example: "Add John as director of Acme"                            │   │
│   │  DSL:     (cbu.assign-role :cbu-id X :entity-id Y :role Z)         │   │
│   │                                                                      │   │
│   │  ─────────────────────────────────────────────────────────────────  │   │
│   │                                                                      │   │
│   │  CounterpartyCreate                                                  │   │
│   │  ──────────────────                                                  │   │
│   │  Pattern: "Add <entity> as counterparty"                            │   │
│   │  Example: "Add Goldman Sachs as counterparty"                       │   │
│   │  DSL:     (entity.ensure-limited-company :name X)                   │   │
│   │           (counterparty.create :entity-id ...)                      │   │
│   │                                                                      │   │
│   │  ─────────────────────────────────────────────────────────────────  │   │
│   │                                                                      │   │
│   │  IsdaEstablish                                                       │   │
│   │  ─────────────                                                       │   │
│   │  Pattern: "Set up ISDA with <entity> under <law>"                   │   │
│   │  Example: "Set up ISDA with Goldman under NY law"                   │   │
│   │  DSL:     (isda.create :counterparty-id X :governing-law Y)        │   │
│   │                                                                      │   │
│   │  ─────────────────────────────────────────────────────────────────  │   │
│   │                                                                      │   │
│   │  CsaAdd                                                              │   │
│   │  ──────                                                              │   │
│   │  Pattern: "Add <type> CSA [to ISDA]"                                │   │
│   │  Example: "Add VM CSA"                                              │   │
│   │  DSL:     (isda.add-csa :isda-id X :csa-type "VM")                 │   │
│   │                                                                      │   │
│   │  ─────────────────────────────────────────────────────────────────  │   │
│   │                                                                      │   │
│   │  ProductAdd                                                          │   │
│   │  ──────────                                                          │   │
│   │  Pattern: "Add <product> product"                                   │   │
│   │  Example: "Add custody product"                                     │   │
│   │  DSL:     (cbu.add-product :cbu-id X :product-type "custody")      │   │
│   │                                                                      │   │
│   │  ─────────────────────────────────────────────────────────────────  │   │
│   │                                                                      │   │
│   │  UniverseDefine                                                      │   │
│   │  ──────────────                                                      │   │
│   │  Pattern: "Trade <instruments> [on <markets>]"                      │   │
│   │  Example: "Trade US equities on NYSE"                               │   │
│   │  DSL:     (cbu-custody.add-universe :asset-class "equity"          │   │
│   │                                     :markets ["NYSE"])              │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. LLM Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          LLM INTEGRATION                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   API Endpoint: /api/agent/generate                                         │
│   Server: rust/src/bin/dsl_api.rs                                           │
│                                                                              │
│   Request Flow:                                                              │
│   ─────────────                                                              │
│                                                                              │
│   ┌────────────────────────────────────────────────────────────────────┐    │
│   │                       CLIENT REQUEST                                │    │
│   │────────────────────────────────────────────────────────────────────│    │
│   │  POST /api/agent/generate                                          │    │
│   │  {                                                                  │    │
│   │    "prompt": "Set up Acme Fund with custody and fund accounting", │    │
│   │    "cbu_context": "550e8400-...",                                  │    │
│   │    "mode": "llm"                                                    │    │
│   │  }                                                                  │    │
│   └────────────────────────────────────────────────────────────────────┘    │
│                         │                                                    │
│                         ▼                                                    │
│   ┌────────────────────────────────────────────────────────────────────┐    │
│   │                    CONTEXT ASSEMBLY                                 │    │
│   │────────────────────────────────────────────────────────────────────│    │
│   │                                                                     │    │
│   │  System Prompt:                                                     │    │
│   │  ┌──────────────────────────────────────────────────────────────┐  │    │
│   │  │ You are a DSL generation assistant. Generate valid DSL       │  │    │
│   │  │ statements using only the verbs defined below.               │  │    │
│   │  │                                                               │  │    │
│   │  │ Available domains: cbu, entity, kyc, isda, custody, ...      │  │    │
│   │  │                                                               │  │    │
│   │  │ Verb Reference:                                               │  │    │
│   │  │ - cbu.ensure :name :jurisdiction :as                         │  │    │
│   │  │ - cbu.add-product :cbu-id :product-type                      │  │    │
│   │  │ - entity.create-proper-person :first-name :last-name         │  │    │
│   │  │ - ... (720 verbs)                                            │  │    │
│   │  │                                                               │  │    │
│   │  │ Current context:                                              │  │    │
│   │  │ - CBU: Acme Fund (550e8400-...)                              │  │    │
│   │  │ - Existing entities: [John Smith, Jane Doe]                  │  │    │
│   │  └──────────────────────────────────────────────────────────────┘  │    │
│   │                                                                     │    │
│   └────────────────────────────────────────────────────────────────────┘    │
│                         │                                                    │
│                         ▼                                                    │
│   ┌────────────────────────────────────────────────────────────────────┐    │
│   │                      CLAUDE API CALL                                │    │
│   │────────────────────────────────────────────────────────────────────│    │
│   │  Model: claude-sonnet-4-20250514                                        │    │
│   │  Temperature: 0.0 (deterministic)                                  │    │
│   │  Max tokens: 4096                                                  │    │
│   └────────────────────────────────────────────────────────────────────┘    │
│                         │                                                    │
│                         ▼                                                    │
│   ┌────────────────────────────────────────────────────────────────────┐    │
│   │                      RESPONSE VALIDATION                            │    │
│   │────────────────────────────────────────────────────────────────────│    │
│   │                                                                     │    │
│   │  1. Extract DSL from response (```dsl ... ```)                     │    │
│   │  2. Parse with ob-dsl-parser                                       │    │
│   │  3. Lint with CSG linter                                           │    │
│   │  4. If invalid → retry with error context                          │    │
│   │  5. If valid → return to client                                    │    │
│   │                                                                     │    │
│   └────────────────────────────────────────────────────────────────────┘    │
│                         │                                                    │
│                         ▼                                                    │
│   ┌────────────────────────────────────────────────────────────────────┐    │
│   │                       CLIENT RESPONSE                               │    │
│   │────────────────────────────────────────────────────────────────────│    │
│   │  {                                                                  │    │
│   │    "success": true,                                                 │    │
│   │    "dsl": "(cbu.add-product :cbu-id @ctx :product-type \"custody\")\n│    │
│   │           (cbu.add-product :cbu-id @ctx :product-type \"fund_accounting\")",│
│   │    "explanation": "Added custody and fund accounting products"     │    │
│   │  }                                                                  │    │
│   └────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Conductor Mode (Agent Workflow)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     CONDUCTOR MODE (AGENT WORKFLOW)                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Conductor = Multi-turn agent with state                                   │
│                                                                              │
│   Session State:                                                             │
│   ──────────────                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ ConductorSession {                                                   │   │
│   │     session_id: UUID,                                                │   │
│   │     cbu_context: Option<UUID>,        // Current CBU focus          │   │
│   │     capture_map: HashMap<String, Value>, // @captured variables     │   │
│   │     history: Vec<Message>,            // Conversation history        │   │
│   │     pending_dsl: Option<String>,      // Awaiting confirmation       │   │
│   │ }                                                                    │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Workflow:                                                                  │
│   ─────────                                                                  │
│                                                                              │
│   ┌────────┐   ┌────────┐   ┌────────┐   ┌────────┐   ┌────────┐           │
│   │ START  │──▶│GENERATE│──▶│CONFIRM │──▶│EXECUTE │──▶│ RESULT │           │
│   │        │   │  DSL   │   │  Y/N?  │   │  DSL   │   │        │           │
│   └────────┘   └────────┘   └────────┘   └────────┘   └────────┘           │
│       │                         │                          │                │
│       │                         │ N (edit)                 │                │
│       │                         ▼                          │                │
│       │                    ┌────────┐                      │                │
│       │                    │ REFINE │──────────────────────┘                │
│       │                    │  DSL   │                                       │
│       │                    └────────┘                                       │
│       │                                                                     │
│       └─────────────────── (next command) ──────────────────────────────────│
│                                                                              │
│   Example Session:                                                           │
│   ─────────────────                                                          │
│                                                                              │
│   User: "Create Acme Fund in Luxembourg"                                    │
│   Agent: Generated DSL:                                                      │
│          (cbu.ensure :name "Acme Fund" :jurisdiction "LU" :as @fund)        │
│          Execute? [Y/n]                                                      │
│                                                                              │
│   User: "Y"                                                                  │
│   Agent: ✓ Created Acme Fund (cbu_id: 550e8400-...)                         │
│          Captured: @fund = 550e8400-...                                     │
│                                                                              │
│   User: "Add John Smith as director"                                        │
│   Agent: Generated DSL:                                                      │
│          (entity.create-proper-person :first-name "John"                    │
│                                       :last-name "Smith" :as @john)         │
│          (cbu.assign-role :cbu-id @fund :entity-id @john :role "director")  │
│          Execute? [Y/n]                                                      │
│                                                                              │
│   User: "Y"                                                                  │
│   Agent: ✓ Created John Smith, assigned as director                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Voice Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         VOICE INTEGRATION                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   rust/src/navigation/voice.rs                                              │
│                                                                              │
│   Flow:                                                                      │
│   ─────                                                                      │
│                                                                              │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐             │
│   │  SPEECH  │───▶│  WHISPER │───▶│ SEMANTIC │───▶│  AGENT   │             │
│   │  INPUT   │    │   ASR    │    │  MATCH   │    │ PIPELINE │             │
│   └──────────┘    └──────────┘    └──────────┘    └──────────┘             │
│                                                                              │
│   Semantic Matching:                                                         │
│   ──────────────────                                                         │
│   Handles variations in spoken input:                                       │
│                                                                              │
│   "Add John as director"      ─┐                                            │
│   "Make John a director"       ├──▶ RoleAssign intent                       │
│   "Assign director role to John"─┘                                          │
│                                                                              │
│   Voice Commands for Navigation:                                            │
│   ──────────────────────────────                                             │
│   "Show UBOs"           → Navigate to UBO view                              │
│   "Zoom into Goldman"   → Focus entity in graph                             │
│   "Drill down"          → Expand selected node                              │
│   "Go back"             → Navigation history pop                            │
│   "Run screening"       → Execute screening DSL                             │
│                                                                              │
│   Blade Runner "Esper Machine" Concept:                                     │
│   ─────────────────────────────────────                                      │
│   Voice-driven exploration of entity graph:                                 │
│   "Enhance... zoom... track right... stop..."                               │
│   Maps to graph navigation commands                                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Reference: Key Files

```
Lexicon Config:     rust/config/agent/lexicon.yaml
Tokenizer:          rust/crates/ob-agentic/src/lexicon/tokenizer.rs
Intent Parser:      rust/crates/ob-agentic/src/lexicon/intent_parser.rs
Intent AST:         rust/crates/ob-agentic/src/lexicon/intent_ast.rs
Pipeline:           rust/crates/ob-agentic/src/lexicon/pipeline.rs
DB Resolver:        rust/crates/ob-agentic/src/lexicon/db_resolver.rs
Lexicon Agent:      rust/crates/ob-agentic/src/lexicon_agent.rs

API Server:         rust/src/bin/dsl_api.rs
LLM Backend:        rust/src/llm/
Voice Module:       rust/src/navigation/voice.rs
Research Macros:    rust/src/dsl_v2/research.rs
```

---

Generated: 2026-01-09
