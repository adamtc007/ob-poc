# Research Workflows & Agent Integration

> **Reference TODOs:**
> - `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` (~78h)
> - `ai-thoughts/020-research-workflows-external-sources.md` (~93h)

This annex covers the GROUP ownership model, UBO computation, research workflows, and agent integration.

---

## Core Principle: UBO is COMPUTED not STORED

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    UBO IS COMPUTED, NOT STORED                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   We store FACTS:                    We COMPUTE on demand:                  │
│   • Ownership edges (A owns 30% B)   • UBO list for jurisdiction X          │
│   • Control edges (A appoints B)     • Coverage metrics                     │
│   • Source documents                 • Gap analysis                         │
│   • Verification status              • BODS export                          │
│                                                                              │
│   Same graph → different UBO list depending on jurisdiction rules           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why this matters:**
- UK threshold: 25% (PSC rules)
- US threshold: 10% (FinCEN) or 25% (CDD)
- EU threshold: 25% (AMLD)

Same ownership graph produces different UBO lists per jurisdiction.

---

## Five-Layer Ownership Model

| Layer | What it is | Stored? |
|-------|------------|---------|
| **Raw Data** | Ownership/control edges between entities | ✓ Yes |
| **Coverage** | Known vs unknown breakdown | ✓ Computed, cached |
| **Rules** | Jurisdiction thresholds (25% EU, 10% US) | ✓ Config table |
| **Computation** | `fn_compute_ubos(entity, jurisdiction)` | Computed |
| **Output** | BODS statements, reports | Generated |

---

## Coverage Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  COVERAGE CATEGORIES                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  KNOWN_BENEFICIAL (35%)    → Chain traced to natural person(s)              │
│  KNOWN_LEGAL_ONLY (25%)    → Nominee/custodian, needs look-through          │
│  KNOWN_AGGREGATE (18%)     → Public float, accepted unknown                 │
│  UNACCOUNTED (22%)         → Data gap, triggers research                    │
│                                                                              │
│  Incomplete data is a VALID STATE, not an error                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Synthetic holders** represent known unknowns:
- `PUBLIC_FLOAT` - Listed shares, no UBO required
- `NOMINEE_POOL` - Custodian holdings awaiting disclosure
- `UNACCOUNTED` - Data gap requiring research

---

## Bounded Non-Determinism Architecture

Research uses a TWO-PHASE pattern separating LLM exploration from deterministic execution:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PHASE 1: LLM EXPLORATION              │  PHASE 2: DSL EXECUTION            │
│  ══════════════════════════            │  ════════════════════════          │
│                                        │                                    │
│  Prompt Templates                      │  DSL Verbs                         │
│  • /prompts/research/gleif/search.md   │  • research.gleif.import-hierarchy │
│  • /prompts/research/orchestration/*   │  • research.generic.import-entity  │
│                                        │                                    │
│  LLM searches, reasons, disambiguates  │  Fetch, normalize, create, audit   │
│                                        │                                    │
│  Output: IDENTIFIER (key)              │  Input: IDENTIFIER (key)           │
│                                        │                                    │
│  Non-deterministic but AUDITABLE       │  Deterministic, reproducible       │
└────────────────────┬───────────────────┴────────────────────────────────────┘
                     │
                     ▼
              THE IDENTIFIER IS THE BRIDGE
              (LEI, company_number, CIK)
```

**Why hybrid:**
- Pure deterministic: User must provide LEI/company number (they don't have it)
- Pure LLM: No audit trail, can't explain "why X linked to Y"
- Hybrid: LLM finds the key, DSL uses it deterministically

---

## Session Modes

| Mode | Description | User Role |
|------|-------------|-----------|
| `MANUAL` | User types DSL, REPL executes | Active |
| `AGENT` | LLM generates DSL, REPL executes | Supervisor |
| `HYBRID` | User and agent collaborate | Collaborative |

```rust
pub struct Session {
    // Existing
    pub scope: Scope,
    pub variables: HashMap<String, Value>,
    
    // Agent extension
    pub mode: SessionMode,
    pub agent_state: Option<AgentState>,
}

pub struct AgentState {
    pub task: AgentTask,
    pub status: AgentStatus,  // Running, Paused, Checkpoint, Complete
    pub pending_checkpoint: Option<Checkpoint>,
    pub decisions: Vec<DecisionRef>,
    pub actions: Vec<ActionRef>,
}
```

---

## Agent Invocation Phrases

The LLM uses these phrases to determine when to invoke agent/research verbs:

### Task Triggers
| Phrase | Verb |
|--------|------|
| "find the ownership" | `agent.resolve-gaps` |
| "complete the chain" | `agent.chain-research` |
| "who owns" | `agent.resolve-gaps` |
| "resolve the gaps" | `agent.resolve-gaps` |
| "enrich this entity" | `agent.enrich-entity` |
| "screen for sanctions" | `agent.screen-entities` |

### Source Triggers
| Phrase | Domain |
|--------|--------|
| "check GLEIF", "LEI" | `research.gleif.*` |
| "UK company", "Companies House" | `research.companies-house.*` |
| "SEC filing", "13F", "CIK" | `research.sec.*` |
| "sanctions", "PEP" | `research.screening.*` |

### Checkpoint Responses
| Phrase | Action |
|--------|--------|
| "select the first", "use that one" | Select candidate |
| "neither", "try again" | Reject, try next source |
| "the correct one is X" | Manual override |

---

## Confidence Thresholds

| Score | Action | Decision Type |
|-------|--------|---------------|
| ≥ 0.90 | Auto-proceed | `AUTO_SELECTED` |
| 0.70-0.90 | User checkpoint | `AMBIGUOUS` |
| < 0.70 | Try next source | `NO_MATCH` |

**Forced checkpoints** (regardless of score):
- Screening hits (sanctions, PEP)
- High-stakes context (`NEW_CLIENT`, `MATERIAL_HOLDING`)
- Corrections to previous decisions
- Multiple equally-scored candidates

---

## Pluggable Source Model

| Tier | Example | Handler | LLM Role |
|------|---------|---------|----------|
| **Built-in** | GLEIF, Companies House, SEC | Dedicated verb + handler | Search only |
| **Registered** | Singapore ACRA | `research.generic.import-*` | Search + adapt |
| **Discovered** | LLM finds API | `research.generic.import-*` | Everything |

**The LLM is the universal API adapter** - for Tier 2/3 sources, it discovers the API, makes calls, parses responses, and hands normalized data to deterministic import verbs.

### Normalized Data Contract

```yaml
extracted_entity:
  required:
    name: string
    source_key: string
    source_name: string
  
  optional:
    jurisdiction: string        # ISO country code
    entity_type: string         # Mapped to taxonomy
    status: string              # ACTIVE, DISSOLVED
    incorporated_date: date
    lei: string
    
  nested:
    officers:
      - name: string
        role: string            # DIRECTOR, SECRETARY
        appointed_date: date
        
    shareholders:
      - name: string
        percentage: decimal
        source_key: string
```

---

## Agent Loop Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AGENT LOOP                                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. IDENTIFY GAP                                                            │
│     ownership.identify-gaps(:entity-id @target)                             │
│     → "HoldCo Ltd has no parent"                                            │
│                                                                              │
│  2. LOAD ORCHESTRATION PROMPT                                               │
│     /prompts/research/orchestration/resolve-gap.md                          │
│                                                                              │
│  3. LLM REASONS                                                             │
│     "UK company, try GLEIF then Companies House"                            │
│                                                                              │
│  4. LOAD SOURCE PROMPT + SEARCH                                             │
│     /prompts/research/sources/gleif/search.md                               │
│     → 2 candidates found (scores: 0.85, 0.82)                               │
│                                                                              │
│  5. EVALUATE CONFIDENCE                                                     │
│     Score 0.85 < 0.90 → CHECKPOINT                                          │
│                                                                              │
│  6. USER SELECTS (or auto if ≥0.90)                                         │
│     > 1                                                                     │
│                                                                              │
│  7. RECORD DECISION                                                         │
│     research.workflow.record-decision(...)                                  │
│                                                                              │
│  8. EMIT IMPORT VERB                                                        │
│     research.gleif.import-hierarchy(:lei "213800..." :decision-id @dec)     │
│                                                                              │
│  9. CHECK FOR MORE GAPS                                                     │
│     → If gaps remain, loop to step 1                                        │
│     → If complete, exit                                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Viewport Checkpoint UI

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  REPL                                              [MODE: AGENT ▶ RUNNING]  │
├─────────────────────────────────────────────────────────────────────────────┤
│  > agent.resolve-gaps(:entity-id @fund-alpha)                               │
│                                                                              │
│  🤖 Agent started: RESOLVE_GAPS                                              │
│     Target: Fund Alpha | Scope: GROUP @allianzgi                            │
│                                                                              │
│  [1] ownership.identify-gaps(:entity-id @fund-alpha)                        │
│      → Found 2 gaps: HoldCo Ltd, Nominee X                                  │
│                                                                              │
│  [2] Searching GLEIF for "HoldCo Ltd"...                                    │
│      → 2 candidates (scores: 0.85, 0.82)                                    │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ ⚠️  CHECKPOINT: Select match for "HoldCo Ltd"                          │  │
│  │                                                                        │  │
│  │  [1] HOLDCO LIMITED (LEI: 213800ABC...)                               │  │
│  │      UK | Active | Score: 0.85                                        │  │
│  │                                                                        │  │
│  │  [2] HOLDCO LTD (LEI: 213800XYZ...)                                   │  │
│  │      UK | Active | Score: 0.82                                        │  │
│  │                                                                        │  │
│  │  > Enter 1, 2, N (neither), M (manual): _                             │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  Status: [Iteration 2/50] [Decisions: 0] [Actions: 0] [⏸ Pause] [⏹ Stop]    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Tables (kyc schema)

### Ownership Model (019)

| Table | Purpose |
|-------|---------|
| `ownership_groups` | Group registry linking CBUs |
| `synthetic_holders` | PUBLIC_FLOAT, NOMINEE_POOL, UNACCOUNTED |
| `control_relationships` | Board appointments, voting agreements |
| `ownership_coverage` | Computed coverage metrics |
| `ubo_jurisdiction_rules` | Configurable thresholds per jurisdiction |
| `ownership_research_triggers` | Gap resolution action items |

### Research Workflows (020)

| Table | Purpose |
|-------|---------|
| `research_decisions` | Phase 1 audit (search → selection → reasoning) |
| `research_actions` | Phase 2 audit (verb → outcome → entities created) |
| `research_corrections` | Tracks fixes when wrong key was selected |
| `discovered_sources` | Registry of Tier 2/3 sources LLM has used |
| `research_confidence_config` | Thresholds per source |
| `outreach_requests` | Counterparty disclosure request tracking |

---

## Domain Reference

### Agent Verbs (`agent.yaml`)

| Verb | Purpose | Invocation Phrases |
|------|---------|-------------------|
| `start` | Start agent mode | "start the agent", "automate this" |
| `pause` | Pause execution | "pause", "hold on" |
| `resume` | Resume execution | "continue", "carry on" |
| `stop` | Stop and return to manual | "stop", "cancel" |
| `status` | Get agent status | "what's the agent doing", "progress" |
| `respond-checkpoint` | Answer checkpoint | "select the first", "neither" |
| `resolve-gaps` | Task: resolve ownership gaps | "resolve the gaps", "who owns" |
| `chain-research` | Task: build full chain | "complete the chain" |
| `enrich-entity` | Task: enrich single entity | "enrich this entity" |
| `screen-entities` | Task: run screening | "screen for sanctions" |

### Research Verbs

| Domain | Verbs | Key Type |
|--------|-------|----------|
| `research.gleif` | import-entity, import-hierarchy, validate-lei, refresh | LEI |
| `research.companies-house` | import-company, import-officers, import-psc | COMPANY_NUMBER |
| `research.sec` | import-company, import-13f-holders, import-13dg-owners | CIK |
| `research.generic` | import-entity, import-hierarchy, import-officers | Any |
| `research.screening` | record-sanctions-check, record-pep-check, record-adverse-media | N/A |
| `research.workflow` | record-decision, confirm-decision, reject-decision, record-correction, audit-trail | N/A |

---

## Directory Structure

```
ob-poc/
├── rust/
│   ├── config/verbs/
│   │   ├── research/
│   │   │   ├── gleif.yaml
│   │   │   ├── companies-house.yaml
│   │   │   ├── sec.yaml
│   │   │   ├── generic.yaml
│   │   │   ├── screening.yaml
│   │   │   └── workflow.yaml
│   │   └── agent/
│   │       └── agent.yaml
│   └── src/
│       ├── research/
│       │   ├── mod.rs
│       │   ├── gleif/
│       │   ├── companies_house/
│       │   └── workflow/
│       └── agent/
│           ├── mod.rs
│           ├── controller.rs
│           └── checkpoint.rs
│
├── prompts/
│   └── research/
│       ├── sources/
│       │   ├── gleif/
│       │   │   ├── search.md
│       │   │   └── disambiguate.md
│       │   ├── companies-house/
│       │   │   └── search.md
│       │   └── discover-source.md
│       ├── screening/
│       │   ├── interpret-sanctions.md
│       │   └── interpret-pep.md
│       └── orchestration/
│           ├── resolve-gap.md
│           ├── chain-research.md
│           └── select-source.md
│
└── migrations/
    ├── 014_ownership_groups.sql
    ├── 015_coverage_model.sql
    └── 016_research_workflows.sql
```

---

## Implementation Status

| Component | Status | TODO |
|-----------|--------|------|
| GROUP taxonomy schema | Planning | 019 |
| UBO computation functions | Planning | 019 |
| Coverage model | Planning | 019 |
| Agent infrastructure | Planning | 020 Phase 1 |
| Agent verbs | Planning | 020 Phase 2 |
| Research audit schema | Planning | 020 Phase 3 |
| Prompt templates | Planning | 020 Phase 4 |
| GLEIF refactor | Planning | 020 Phase 5 |
| Companies House | Planning | 020 Phase 6 |
| Generic import | Planning | 020 Phase 7 |

---

*For full implementation details, see the TODO documents in ai-thoughts/*
