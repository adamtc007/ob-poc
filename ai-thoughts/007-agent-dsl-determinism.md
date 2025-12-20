# The Agent-DSL Architecture: Determinism as Differentiator

*Captured: 2024-12-20*
*Context: Articulating the value of LLM + DSL vs smart forms, and why determinism is the key difference from exploratory AI*

---

## The Core Question

> "If the DSL is deterministic, why put a non-deterministic LLM in front of it?"

This is the question skeptics will ask. The answer is the entire value proposition.

---

## Two Modes of AI Interaction

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  MODE 1: EXPLORATORY AI                                                    │
│  ═══════════════════════                                                   │
│                                                                             │
│  "Research this topic"                                                     │
│  "Summarize this document"                                                 │
│  "What do you think about X?"                                              │
│  "Find information about Y"                                                │
│                                                                             │
│  Characteristics:                                                          │
│  • Open-ended output                                                       │
│  • Probabilistic / creative                                                │
│  • No single "right answer"                                                │
│  • Hallucination is a feature (creativity) and a bug                      │
│  • Output consumed by humans who judge quality                             │
│  • Non-deterministic by design                                             │
│                                                                             │
│  Use cases: Research, writing, brainstorming, analysis                     │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  MODE 2: OPERATIONAL AI (This DSL)                                         │
│  ══════════════════════════════════                                        │
│                                                                             │
│  "Onboard Apex Fund with BlackRock as ManCo"                              │
│  "Add John Smith as UBO with 25% ownership"                                │
│  "Set up custody services for all Luxembourg funds"                        │
│                                                                             │
│  Characteristics:                                                          │
│  • Constrained output (valid DSL or error)                                │
│  • Deterministic execution                                                 │
│  • Single correct interpretation (or explicit ambiguity)                   │
│  • Hallucination is ALWAYS a bug (caught by compiler)                     │
│  • Output executed by machines against real databases                      │
│  • Deterministic by design                                                 │
│                                                                             │
│  Use cases: Operations, transactions, compliance, audit                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**The key insight: These are fundamentally different modes. Most AI tools don't distinguish them. We do.**

---

## The Agent as Translator, Not Executor

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  HUMAN INTENT                                                              │
│  "Add John to the Apex fund as a director"                                │
│       │                                                                    │
│       │  ┌─────────────────────────────────────────────────────────────┐  │
│       │  │  LLM AGENT (Non-deterministic translation)                  │  │
│       │  │                                                             │  │
│       │  │  • Understands natural language                            │  │
│       │  │  • Resolves "John" → disambiguation or context             │  │
│       │  │  • Resolves "Apex fund" → exact entity                     │  │
│       │  │  • Maps intent to verb: entity.add-role                    │  │
│       │  │  • Generates structured VerbIntent                         │  │
│       │  │                                                             │  │
│       ▼  │  OUTPUT: Structured intent, NOT executable code            │  │
│       │  └─────────────────────────────────────────────────────────────┘  │
│       │                                                                    │
│       ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  DETERMINISTIC PIPELINE (No LLM involved)                          │  │
│  │                                                                     │  │
│  │  VerbIntent → DSL Builder → Parser → Enricher → Resolver → DAG    │  │
│  │                                                                     │  │
│  │  Every step: Deterministic, verifiable, auditable                  │  │
│  │  Any failure: Structured error, not hallucination                  │  │
│  │                                                                     │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│       │                                                                    │
│       ▼                                                                    │
│  VALID DSL (human-reviewable)                                             │
│  (entity.add-role                                                         │
│    :entity-id "John Smith"      ← Human sees name, not UUID              │
│    :role-type "DIRECTOR"                                                  │
│    :target-id "Apex Fund")                                                │
│       │                                                                    │
│       ▼                                                                    │
│  HUMAN REVIEW                                                              │
│  "Is this what you meant?" [Confirm] [Edit] [Cancel]                      │
│       │                                                                    │
│       ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  DETERMINISTIC EXECUTION                                            │  │
│  │                                                                     │  │
│  │  Resolved AST → Executor → Database                                │  │
│  │                                                                     │  │
│  │  • All entity references pre-resolved to UUIDs                     │  │
│  │  • Execution order fixed by DAG                                    │  │
│  │  • Same input = same output (always)                               │  │
│  │  • Full audit trail                                                │  │
│  │                                                                     │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│       │                                                                    │
│       ▼                                                                    │
│  DATABASE STATE (deterministic, audited, reversible)                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**The LLM is quarantined.** Its output goes through a deterministic validation pipeline before anything touches the database.

---

## Why Not Just a Smart Form?

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SMART FORM APPROACH                                                       │
│  ═══════════════════                                                       │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │  Add Role                                                           │  │
│  │                                                                     │  │
│  │  Entity: [Dropdown: Search entities...    ▼]                       │  │
│  │  Role:   [Dropdown: DIRECTOR / UBO / ...  ▼]                       │  │
│  │  Target: [Dropdown: Search targets...     ▼]                       │  │
│  │                                                                     │  │
│  │  [Submit]                                                          │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  PROBLEMS:                                                                 │
│                                                                             │
│  1. User must know the form exists                                        │
│     "Where's the form for adding a UBO ownership chain?"                  │
│     "Is that under Entities? Relationships? Compliance?"                  │
│                                                                             │
│  2. User must know the vocabulary                                         │
│     "Is it 'Director' or 'Board Member' or 'Officer'?"                   │
│     "What's the difference between UBO and Beneficial Owner?"            │
│                                                                             │
│  3. Forms don't compose                                                   │
│     "Add John as director AND set up custody AND create the CBU"         │
│     = 3 different forms, 3 different workflows, user tracks state        │
│                                                                             │
│  4. Forms are pre-defined                                                 │
│     New requirement = new form = dev work = 6 months                     │
│                                                                             │
│  5. Forms don't understand context                                        │
│     "Add him to the other fund too" = ???                                │
│     User must re-enter everything                                         │
│                                                                             │
│  6. Forms scale linearly                                                  │
│     337 entities = 337 form submissions                                   │
│     Even with bulk upload: CSV template, column mapping, validation      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AGENT + DSL APPROACH                                                      │
│  ═══════════════════                                                       │
│                                                                             │
│  User: "Add John Smith as director of Apex Fund"                          │
│                                                                             │
│  Agent: ✓ Resolved John Smith                                             │
│         ✓ Resolved Apex Fund                                              │
│         Generated:                                                         │
│         (entity.add-role :entity-id "John Smith"                          │
│                          :role-type "DIRECTOR"                            │
│                          :target-id "Apex Fund")                          │
│         [Confirm] [Edit]                                                  │
│                                                                             │
│  ADVANTAGES:                                                               │
│                                                                             │
│  1. No form discovery                                                     │
│     User says what they want. Agent figures out the "form."              │
│                                                                             │
│  2. Natural vocabulary                                                    │
│     "director" / "board member" / "on the board" → DIRECTOR              │
│     Agent handles synonyms, user uses their words                         │
│                                                                             │
│  3. Composition is natural                                                │
│     "Add John as director, set up custody, and create the CBU"           │
│     = One conversation, multiple DSL statements, DAG-ordered execution   │
│                                                                             │
│  4. New capabilities via DSL extension                                    │
│     New verb in YAML → Agent can use it immediately                      │
│     No new forms, no UI changes                                           │
│                                                                             │
│  5. Context carries forward                                               │
│     "Add him to the other fund too"                                       │
│     Agent: ✓ "him" → John Smith, "other fund" → Beta Fund                │
│                                                                             │
│  6. Bulk is natural language                                              │
│     "Set up custody for all Luxembourg Allianz funds"                    │
│     Agent: Found 47 Luxembourg funds. Generate DSL for each? [Yes]       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Determinism Guarantee

Here's what makes this different from "just using ChatGPT":

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  CHATGPT / COPILOT APPROACH                                                │
│  ══════════════════════════                                                │
│                                                                             │
│  User: "Add John Smith as director of Apex Fund"                          │
│                                                                             │
│  LLM generates code:                                                       │
│    INSERT INTO roles (entity_id, role_type, target_id)                    │
│    VALUES ('???', 'DIRECTOR', '???');                                     │
│                                                                             │
│  Problems:                                                                 │
│  • What UUIDs? LLM doesn't know. Might hallucinate.                       │
│  • What if "John Smith" matches 3 people? LLM picks randomly.            │
│  • What if "Apex Fund" doesn't exist? LLM invents a UUID.                │
│  • What if the role requires a different table? LLM might not know.      │
│  • Code executes directly. Errors discovered at runtime.                  │
│                                                                             │
│  The LLM is GENERATING EXECUTABLE CODE.                                   │
│  There's no validation layer. You're trusting the LLM.                    │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  THIS ARCHITECTURE                                                         │
│  ═════════════════                                                         │
│                                                                             │
│  User: "Add John Smith as director of Apex Fund"                          │
│                                                                             │
│  LLM generates INTENT (not code):                                          │
│    {                                                                       │
│      "verb": "entity.add-role",                                           │
│      "params": { "role-type": "DIRECTOR" },                               │
│      "lookups": {                                                          │
│        "entity-id": { "search": "John Smith", "type": "person" },        │
│        "target-id": { "search": "Apex Fund", "type": "fund" }            │
│      }                                                                     │
│    }                                                                       │
│                                                                             │
│  Then DETERMINISTIC pipeline:                                              │
│                                                                             │
│  1. VALIDATE VERB: "entity.add-role" exists in registry? ✓               │
│     If not: Error. LLM doesn't get to invent verbs.                       │
│                                                                             │
│  2. VALIDATE PARAMS: "role-type" is valid arg? "DIRECTOR" is valid enum? │
│     If not: Error. LLM doesn't get to invent arguments.                   │
│                                                                             │
│  3. RESOLVE LOOKUPS: "John Smith" → query EntityGateway                   │
│     1 match: Auto-resolve with UUID                                       │
│     N matches: Disambiguation (human chooses)                             │
│     0 matches: Error (not silent hallucination)                           │
│                                                                             │
│  4. BUILD DSL: Deterministic code, not LLM-generated                      │
│     Pure Rust function: intent + resolved_ids → DSL string               │
│                                                                             │
│  5. PARSE & VALIDATE: Does the DSL parse? Semantic checks pass?           │
│     If not: Error back to LLM with structured feedback, retry            │
│                                                                             │
│  6. HUMAN REVIEW: Show human-readable DSL. Confirm?                       │
│                                                                             │
│  7. EXECUTE: Resolved AST → database operations                           │
│     All UUIDs known. All dependencies ordered. Deterministic.            │
│                                                                             │
│  THE LLM NEVER GENERATES EXECUTABLE CODE.                                 │
│  It generates structured intent that's validated at every step.           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Hallucination Firewall

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  WHERE HALLUCINATIONS GET CAUGHT                                           │
│                                                                             │
│  LLM hallucinates...        Caught by...           Result                  │
│  ══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  Invalid verb               Verb registry lookup   "Unknown verb: xyz"     │
│  "cbu.onbard" (typo)        (deterministic)        Retry with correction   │
│                                                                             │
│  Invalid argument           Verb param validation  "Unknown arg: xyz"      │
│  ":cbu-naem" (typo)         (deterministic)        Retry with correction   │
│                                                                             │
│  Invalid enum value         Enum validation        "Invalid: 'DIRECTR'"    │
│  "DIRECTR" (typo)           (deterministic)        Suggest: "DIRECTOR"     │
│                                                                             │
│  Non-existent entity        EntityGateway lookup   "Not found: 'XyzCorp'" │
│  "XyzCorp ManCo"            (deterministic)        Ask user to clarify     │
│                                                                             │
│  Wrong entity               Disambiguation UI      "3 matches for 'John'" │
│  "John" (ambiguous)         (human in loop)        User selects correct    │
│                                                                             │
│  Wrong UUID format          UUID parse             "Invalid UUID format"   │
│  "not-a-uuid"               (deterministic)        Never reaches DB        │
│                                                                             │
│  Circular dependency        DAG construction       "Circular: a→b→a"      │
│  @a needs @b needs @a       (deterministic)        Error before execution  │
│                                                                             │
│  Type mismatch              SQLx compile check     "Expected UUID, got     │
│  String where UUID needed   (compile time!)        String"                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘

NOTHING THE LLM PRODUCES REACHES THE DATABASE WITHOUT VALIDATION.

Every possible hallucination has a deterministic check that catches it.
```

---

## The Value Stack

What does the agent add that forms can't do?

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AGENT VALUE LAYER 1: Natural Language Understanding                      │
│  ════════════════════════════════════════════════════                      │
│                                                                             │
│  User vocabulary → System vocabulary                                       │
│                                                                             │
│  "Put John on the board"          → entity.add-role :role-type DIRECTOR   │
│  "Make John a director"           → entity.add-role :role-type DIRECTOR   │
│  "Add John as board member"       → entity.add-role :role-type DIRECTOR   │
│  "John should be a director"      → entity.add-role :role-type DIRECTOR   │
│                                                                             │
│  Form approach: User must know "entity.add-role" and "DIRECTOR"           │
│  Agent approach: User speaks naturally, agent translates                   │
│                                                                             │
│  DETERMINISM PRESERVED: Output is always the same valid DSL               │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  AGENT VALUE LAYER 2: Context and Memory                                  │
│  ══════════════════════════════════════                                    │
│                                                                             │
│  Session context → Pronoun and reference resolution                       │
│                                                                             │
│  "Add him to the other fund too"                                          │
│     ↓                                                                      │
│  "him" → John Smith (from prior context)                                  │
│  "the other fund" → Beta Fund (from mention history)                     │
│     ↓                                                                      │
│  (entity.add-role :entity-id "John Smith" :target-id "Beta Fund" ...)    │
│                                                                             │
│  Form approach: Re-enter everything from scratch                          │
│  Agent approach: Remembers context, resolves references                    │
│                                                                             │
│  DETERMINISM PRESERVED: Pronoun resolution is deterministic given context │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  AGENT VALUE LAYER 3: Composition and Sequencing                          │
│  ════════════════════════════════════════════════                          │
│                                                                             │
│  Multi-step request → Ordered DSL statements                              │
│                                                                             │
│  "Create a CBU for Apex, add John as director, set up custody"            │
│     ↓                                                                      │
│  (cbu.ensure :name "Apex" :as @apex)                                      │
│  (entity.add-role :entity-id "John" :target-id @apex :role "DIRECTOR")   │
│  (service.add-product :cbu-id @apex :product "CUSTODY")                   │
│     ↓                                                                      │
│  DAG orders execution: @apex first, then role, then service              │
│                                                                             │
│  Form approach: 3 forms, user tracks order, manual sequencing            │
│  Agent approach: One statement, automatic dependency ordering             │
│                                                                             │
│  DETERMINISM PRESERVED: DAG construction is deterministic                 │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  AGENT VALUE LAYER 4: Bulk Operations from Intent                         │
│  ═══════════════════════════════════════════════                           │
│                                                                             │
│  High-level intent → Expanded DSL for each entity                         │
│                                                                             │
│  "Set up custody for all Luxembourg Allianz funds"                        │
│     ↓                                                                      │
│  Query: Find funds where jurisdiction=LU and scope=Allianz (47 results)  │
│  Confirm: "Found 47 funds. Generate custody setup for each?"             │
│     ↓                                                                      │
│  (service.add-product :cbu-id "Allianz Lux Fund 1" :product "CUSTODY")   │
│  (service.add-product :cbu-id "Allianz Lux Fund 2" :product "CUSTODY")   │
│  ... (45 more)                                                            │
│                                                                             │
│  Form approach: 47 form submissions, or CSV upload with template          │
│  Agent approach: One sentence, confirm count, execute batch              │
│                                                                             │
│  DETERMINISM PRESERVED: Same query → same 47 funds → same DSL            │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  AGENT VALUE LAYER 5: Error Recovery with Understanding                   │
│  ═══════════════════════════════════════════════════════                   │
│                                                                             │
│  Error → Explanation → Suggestion                                          │
│                                                                             │
│  User: "Add John to Apex as CFO"                                          │
│  Error: "CFO is not a valid role type"                                    │
│  Agent: "Did you mean 'OFFICER'? Or would you like to see available       │
│          role types?"                                                     │
│                                                                             │
│  Form approach: Red validation error, user guesses                        │
│  Agent approach: Contextual help, suggestions, recovery                   │
│                                                                             │
│  DETERMINISM PRESERVED: Error detection is deterministic, help is LLM    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Audit Story

For compliance, the determinism guarantee is critical:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AUDIT TRAIL                                                               │
│                                                                             │
│  Regulator asks: "How did John Smith become a director of Apex Fund?"     │
│                                                                             │
│  CHATGPT APPROACH:                                                         │
│  "An AI generated some SQL that we executed."                             │
│  "We don't have the exact prompt."                                        │
│  "The AI might have made different choices on a different day."          │
│  😬                                                                        │
│                                                                             │
│  THIS ARCHITECTURE:                                                        │
│  1. User request: "Add John Smith as director of Apex Fund"              │
│     [Logged: timestamp, user_id, session_id, raw text]                    │
│                                                                             │
│  2. Agent interpretation: VerbIntent { verb: "entity.add-role", ... }    │
│     [Logged: structured intent, LLM model, confidence]                    │
│                                                                             │
│  3. Entity resolution:                                                    │
│     - "John Smith" → 3 matches → user selected UUID abc123               │
│     - "Apex Fund" → 1 match → auto-resolved to UUID def456              │
│     [Logged: resolution path, alternatives shown, user choice]            │
│                                                                             │
│  4. DSL generated (deterministic):                                        │
│     (entity.add-role :entity-id "abc123" :target-id "def456" ...)        │
│     [Logged: exact DSL, AST, resolved references]                        │
│                                                                             │
│  5. Human review: User clicked [Confirm]                                  │
│     [Logged: confirmation timestamp, user_id]                             │
│                                                                             │
│  6. Execution: INSERT INTO roles ...                                      │
│     [Logged: SQL executed, rows affected, execution time]                 │
│                                                                             │
│  COMPLETE CHAIN: Intent → Resolution → DSL → Confirmation → Execution     │
│  REPRODUCIBLE: Same inputs → same outputs (deterministic)                 │
│  HUMAN VERIFIED: User confirmed before execution                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Research vs Operations Distinction

This is the key differentiator to communicate:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  EXPLORATORY AI                        OPERATIONAL AI (This DSL)           │
│  ══════════════                        ══════════════════════════          │
│                                                                             │
│  "What do you think about X?"          "Do X"                              │
│                                                                             │
│  Output: Text, opinions, analysis      Output: Database state change       │
│                                                                             │
│  Hallucination: Annoying but okay      Hallucination: Catastrophic         │
│                                                                             │
│  Verification: Human reads and judges  Verification: Compiler validates    │
│                                                                             │
│  Determinism: Not expected             Determinism: REQUIRED               │
│                                                                             │
│  Audit: "AI said this"                 Audit: "AI translated, human        │
│                                                confirmed, system executed" │
│                                                                             │
│  Rollback: Not applicable              Rollback: Transaction reversal      │
│                                                                             │
│  Risk: Wrong information               Risk: Wrong data in production      │
│                                                                             │
│  Examples:                             Examples:                           │
│  - Research assistant                  - KYC onboarding                    │
│  - Document summarization              - Trade execution                   │
│  - Brainstorming                       - Compliance reporting              │
│  - Writing help                        - Entity management                 │
│                                                                             │
│  MOST AI TOOLS DON'T DISTINGUISH THESE MODES.                             │
│  WE DO. THAT'S THE DIFFERENTIATOR.                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Sales Pitch

For different audiences:

### To Business Leaders

> "Your analysts can say what they want in plain English. The system translates it to precise operations. Everything is verified before it touches your database. Full audit trail. AI that's actually safe for regulated operations."

### To Compliance/Risk

> "The AI doesn't execute anything. It translates. Everything goes through deterministic validation. Human confirms before execution. Complete audit trail from intent to action. Same input always produces same output."

### To Technology Leaders

> "We've solved the hallucination problem for operational AI. The LLM generates structured intents, not code. Intents are validated against a grammar. Entity references are resolved against real data. Only valid, verified operations execute. It's AI with guardrails that actually work."

### To End Users

> "Just say what you need. The system figures out the rest. It'll show you what it's going to do before it does it. If it's not right, fix it. If it is right, confirm and it's done."

---

## Key Quotes to Remember

> "The LLM is a TRANSLATOR, not an EXECUTOR."

> "Translation is validated. Execution is deterministic."

> "Nothing the LLM produces reaches the database without validation."

> "Exploratory AI: hallucination is annoying. Operational AI: hallucination is catastrophic. We prevent catastrophe."

> "Same input → same output. Always. That's the guarantee."

---

## Summary: The Determinism Stack

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  LAYER          DETERMINISM GUARANTEE                                      │
│  ══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  LLM Output     Constrained to structured VerbIntent                       │
│       ↓         (Not arbitrary code)                                       │
│                                                                             │
│  Verb Valid?    Registry lookup - verb exists or error                     │
│       ↓         (LLM can't invent verbs)                                   │
│                                                                             │
│  Args Valid?    Schema validation - args match verb signature              │
│       ↓         (LLM can't invent arguments)                               │
│                                                                             │
│  Entities?      EntityGateway resolution - real data or error              │
│       ↓         (LLM can't hallucinate entities)                           │
│                                                                             │
│  Ambiguity?     Human disambiguation - user chooses                        │
│       ↓         (LLM doesn't guess)                                        │
│                                                                             │
│  DSL Valid?     Parser + semantic validator                                │
│       ↓         (Syntax and semantics checked)                             │
│                                                                             │
│  Order?         DAG construction - dependencies resolved                   │
│       ↓         (Execution order is deterministic)                         │
│                                                                             │
│  Confirmed?     Human review - user approves                               │
│       ↓         (Nothing executes without confirmation)                    │
│                                                                             │
│  Execution      Resolved AST → SQL                                         │
│                 (All UUIDs known, all types checked)                       │
│                                                                             │
│  EVERY LAYER: Deterministic. Verifiable. Auditable.                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*Determinism isn't a limitation. It's the entire point. AI that you can actually trust for operations.*
