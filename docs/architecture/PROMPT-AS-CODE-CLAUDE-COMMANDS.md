# Prompt-as-Code: Claude Code Custom Commands & Agentic Development Patterns

**Document Type:** Architecture Brain Dump  
**Created:** 2026-01-02  
**Status:** Reference Documentation

---

## Executive Summary

Claude Code provides an under-documented but powerful feature: **custom slash commands** via the `.claude/commands/` directory. Combined with `CLAUDE.md` (persistent context) and YAML-driven configuration, this creates a version-controlled "expertise injection" system for AI-assisted development.

This document details the mechanics, patterns, and best practices discovered through building ob-poc with agentic AI development.

---

## Table of Contents

1. [The Three Pillars of Agentic Context](#the-three-pillars-of-agentic-context)
2. [Claude Code Custom Commands Deep Dive](#claude-code-custom-commands-deep-dive)
3. [CLAUDE.md - Persistent Project Context](#claudemd---persistent-project-context)
4. [YAML-Driven Configuration](#yaml-driven-configuration)
5. [ob-poc Implementation Patterns](#ob-poc-implementation-patterns)
6. [Anti-Patterns and Failure Modes](#anti-patterns-and-failure-modes)
7. [Advanced Patterns](#advanced-patterns)
8. [Comparison to Other Approaches](#comparison-to-other-approaches)

---

## The Three Pillars of Agentic Context

Effective AI-assisted development requires three types of context injection:

| Pillar | Mechanism | When Loaded | Purpose |
|--------|-----------|-------------|---------|
| **Always-On** | `CLAUDE.md` | Every conversation | Core rules, architecture, non-negotiables |
| **On-Demand** | `.claude/commands/*.md` | User invokes `/command` | Domain expertise, workflows, checklists |
| **Data-Driven** | YAML/JSON configs | Runtime, tool use | Dynamic vocabulary, schemas, templates |

```
┌─────────────────────────────────────────────────────────────────┐
│                     Claude Code Context                         │
├─────────────────────────────────────────────────────────────────┤
│  CLAUDE.md (always loaded)                                      │
│  ├── Project structure                                          │
│  ├── Core architectural rules                                   │
│  ├── Non-negotiable constraints                                 │
│  └── Quick reference                                            │
├─────────────────────────────────────────────────────────────────┤
│  .claude/commands/ (on-demand via /slash)                       │
│  ├── /build     → Build & deploy runbook                        │
│  ├── /egui      → UI development rules                          │
│  ├── /kyc       → Domain expertise injection                    │
│  ├── /verify    → Completion checklist                          │
│  └── /dsl       → DSL syntax & patterns                         │
├─────────────────────────────────────────────────────────────────┤
│  config/*.yaml (loaded by tools)                                │
│  ├── verbs/     → DSL vocabulary                                │
│  ├── macros/    → Template definitions                          │
│  ├── schemas/   → Validation rules                              │
│  └── prompts/   → LLM prompt templates                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Claude Code Custom Commands Deep Dive

### Discovery & Mechanics

Claude Code looks for markdown files in `.claude/commands/` at the project root. Each file becomes a slash command:

```
.claude/commands/
├── build.md          →  /build
├── egui.md           →  /egui
├── kyc.md            →  /kyc
├── verify-complete.md →  /verify-complete
└── types.md          →  /types
```

**File naming:**
- Filename becomes command name (minus `.md`)
- Hyphens preserved: `verify-complete.md` → `/verify-complete`
- Case insensitive: `BUILD.md` → `/build`

### What Happens When Invoked

When you type `/egui` in Claude Code:

1. Claude Code reads `.claude/commands/egui.md`
2. The **entire file contents** are injected into the conversation context
3. This happens **before** Claude processes your actual message
4. The command content acts as a "system prompt extension" for that message

```
User types: /egui How do I add a new panel?

Claude sees:
┌──────────────────────────────────────────────────────┐
│ [Contents of .claude/commands/egui.md]               │
│                                                      │
│ # egui UI Development                                │
│ Before writing any egui/WASM UI code, review...      │
│ ...                                                  │
├──────────────────────────────────────────────────────┤
│ [User's actual question]                             │
│ How do I add a new panel?                            │
└──────────────────────────────────────────────────────┘
```

### Command File Structure (Best Practices)

```markdown
# Command Name - Human Readable Title

Brief description of when to use this command.

## Quick Reference
- Bullet points for fast scanning
- Most common patterns
- Key files/paths

## Rules (if applicable)
1. Numbered non-negotiables
2. Things Claude MUST do
3. Things Claude MUST NOT do

## Examples
```code
Concrete examples Claude can pattern-match against
```

## Files to Read
- path/to/relevant/file.rs - Why it matters
- path/to/another.rs - What to learn from it

## Common Mistakes
- Anti-pattern 1: Why it's wrong
- Anti-pattern 2: What to do instead
```

### Why This Is Powerful

1. **Version Controlled Expertise** - Your AI instructions live in git, not scattered chat histories
2. **Team Knowledge Sharing** - New team members get your hard-won patterns
3. **Context Window Efficiency** - Load domain knowledge only when needed
4. **Iterative Refinement** - Fix bad AI behavior by editing a file, not re-explaining
5. **Reproducibility** - Same command, same context, consistent behavior

---

## CLAUDE.md - Persistent Project Context

### Purpose

`CLAUDE.md` at project root is **always loaded** by Claude Code. It's your "constitution" - rules that apply to every interaction.

### Effective Structure

```markdown
# CLAUDE.md - Project Intelligence

## Project Overview
One paragraph: what is this, what does it do, who is it for.

## Tech Stack
- Language: Rust 2021 edition
- Database: PostgreSQL 15 with sqlx
- UI: egui + WASM
- etc.

## Directory Structure
```
project/
├── src/           # What's here
├── config/        # What's here
└── tests/         # What's here
```

## Architecture Invariants
Non-negotiable rules that should NEVER be violated:
1. Rule one - why it exists
2. Rule two - why it exists

## Development Workflow
How to build, test, deploy.

## Domain Glossary
Key terms with precise definitions.

## Common Patterns
Code patterns to follow with examples.

## Anti-Patterns
Things that seem right but are wrong.
```

### Size Considerations

CLAUDE.md is loaded every time, so:
- Keep it focused (500-1500 lines max)
- Put detailed domain knowledge in `/commands`
- Reference other docs rather than duplicating

### ob-poc CLAUDE.md Sections (Example)

```markdown
## egui State Management & Best Practices

### 5 Non-Negotiable Rules

1. **NO local state mirroring server data**
   - BAD: `struct MyPanel { messages: Vec<Message> }`
   - GOOD: Read from `AppState.session` which is fetched from server

2. **Actions return values, no callbacks**
   - BAD: `if clicked { self.save_data() }`
   - GOOD: `if clicked { return Some(Action::SaveData(id)) }`

3. **Short lock, then render**
   - BAD: `let state = app_state.lock(); /* render 100 widgets */`
   - GOOD: `let data = { app_state.lock().clone_what_i_need() }; /* render */`

[etc.]
```

---

## YAML-Driven Configuration

### The Third Pillar

While CLAUDE.md and commands inject static context, YAML configs provide **dynamic, data-driven** context:

```
config/
├── verbs/
│   ├── core.yaml      # (entity.create :type "..." :name "...")
│   ├── gleif.yaml     # (gleif.enrich :lei "..." :as @binding)
│   └── templates/     # Macro definitions
├── macros/
│   └── research/      # Research macro definitions
└── schemas/
    └── entities.yaml  # Entity type definitions
```

### How It Integrates

```rust
// Tool reads YAML at runtime
let registry = VerbRegistry::load_from_dir("config/verbs")?;

// MCP tool exposes to Claude
fn verbs_list() -> Vec<VerbInfo> {
    registry.all_verbs()
        .map(|v| VerbInfo {
            name: v.full_name(),        // "gleif.enrich"
            description: v.description,  // "Enrich entity from GLEIF..."
            parameters: v.params,        // [{name: "lei", required: true}, ...]
        })
        .collect()
}
```

When Claude calls `verbs_list`, it receives the **current** vocabulary - no code changes needed to teach Claude new verbs.

### YAML as Prompt Engineering

The YAML files themselves are prompt engineering:

```yaml
verb:
  name: enrich
  domain: gleif
  description: |
    Fetch entity data from GLEIF API by LEI and store in database.
    Creates or updates the entity record with official GLEIF data.
    Use :as @binding to capture the entity ID for subsequent operations.
  
  parameters:
    - name: lei
      type: string
      required: true
      description: "20-character Legal Entity Identifier"
      pattern: "^[A-Z0-9]{20}$"
    
    - name: as
      type: binding
      required: false
      description: "Symbol to bind result entity ID"
  
  examples:
    - "(gleif.enrich :lei \"529900K9B0N5BT694847\" :as @allianz)"
    - "(gleif.enrich :lei \"213800ABCD1234567890\")"
  
  see_also:
    - gleif.search
    - gleif.trace-ownership
```

The `description`, `examples`, and `see_also` fields exist primarily for Claude's benefit - they're prompt engineering embedded in configuration.

---

## ob-poc Implementation Patterns

### Pattern 1: Domain Expertise Commands

**`.claude/commands/kyc.md`** - Inject KYC/AML domain knowledge:

```markdown
# KYC Domain Expertise

## Key Concepts

### Client Business Unit (CBU)
The atomic unit of onboarding. Represents a client's business relationship.
- Has exactly one apex entity (the legal client)
- Contains all related entities, roles, documents
- Tracks workflow state

### Ultimate Beneficial Owner (UBO)
Natural person(s) who ultimately own or control the client.
- Ownership threshold: typically 25% (varies by jurisdiction)
- Must trace through all intermediate entities
- Terminus types: PUBLIC_FLOAT, STATE_OWNED, NATURAL_PERSONS

### Legal Entity Identifier (LEI)
20-character alphanumeric code for legal entities.
- Issued by Local Operating Units (LOUs)
- GLEIF is the global aggregator
- Format: 4-char LOU prefix + 14-char entity ID + 2-char checksum

[etc.]
```

### Pattern 2: Workflow Commands

**`.claude/commands/build.md`** - Runbook injection:

```markdown
# Build & Development Commands

## Quick Reference
```bash
cargo x pre-commit    # Before committing
cargo x deploy        # Full deploy
cargo x check --db    # With database tests
```

## Full CI Pipeline
```bash
cargo x ci
```

## Common Issues

### "Database connection refused"
```bash
docker-compose up -d postgres
```

### "WASM build failed"
```bash
rustup target add wasm32-unknown-unknown
```
```

### Pattern 3: Enforcement Commands

**`.claude/commands/verify-complete.md`** - Anti-laziness enforcement:

```markdown
# Verify Implementation Complete

## Mandatory Grep Patterns

Run these before declaring "complete":

```bash
rg -n "TODO|FIXME" --glob "*.rs" rust/src/
rg -n "unimplemented!|todo!" --glob "*.rs" rust/src/
rg -n "placeholder|stub" -i --glob "*.rs" rust/src/
```

## Never Say "Complete" If:
- Any `todo!()` in the code path
- Placeholder data returned as real results
- Comments containing "pending" or "stub"

## What To Say Instead:
"Implementation compiles. Known gaps: [explicit list]"
```

### Pattern 4: Architectural Constraint Commands

**`.claude/commands/egui.md`** - UI rules:

```markdown
# egui UI Development

## 5 Non-Negotiable Rules

1. **NO local state mirroring server data**
2. **Actions return values, no callbacks**
3. **Short lock, then render**
4. **Process async first, render second**
5. **Server round-trip for mutations**

## Files to Understand
- `app.rs` - Main update loop
- `state.rs` - AppState, AsyncState
- `panels/` - Panel implementations

## Anti-Patterns

### BAD: Local message cache
```rust
struct ChatPanel {
    messages: Vec<Message>,  // NO! Use AppState.session
}
```

### GOOD: Read from shared state
```rust
fn show(&mut self, state: &AppState) {
    let messages = &state.session.messages;  // Server is source of truth
}
```
```

---

## Anti-Patterns and Failure Modes

### Anti-Pattern 1: Command Bloat

**Problem:** Commands become 2000-line documents that overflow context.

**Solution:** Keep commands focused. If it's getting long, split into multiple commands or reference external docs.

```markdown
# BAD - Everything in one command
/kyc contains: domain model + API reference + examples + history + regulations...

# GOOD - Focused commands
/kyc-concepts    - Core domain concepts
/kyc-api         - API reference
/kyc-regulations - Regulatory context
```

### Anti-Pattern 2: Stale Commands

**Problem:** Code evolves but commands don't. Claude gets wrong patterns.

**Solution:** 
- Review commands during code review
- Add commands to PR checklist
- Date/version your commands

```markdown
# Entity API Patterns
**Last Updated:** 2026-01-02
**Valid For:** ob-poc v2.x
```

### Anti-Pattern 3: Duplicate Truth

**Problem:** Same information in CLAUDE.md, commands, and code comments.

**Solution:** Single source of truth with references.

```markdown
# In CLAUDE.md
See `.claude/commands/egui.md` for UI development rules.

# In egui.md
Core patterns defined here. Implementation in `src/ui/patterns.rs`.
```

### Anti-Pattern 4: Missing Negative Examples

**Problem:** Commands show what TO do but not what NOT to do. Claude still makes mistakes.

**Solution:** Always include anti-patterns with explanations.

```markdown
## Anti-Patterns

### DON'T: Nested async in render
```rust
// This will deadlock
ui.button("Save").clicked() {
    runtime.block_on(save_async());  // DEADLOCK
}
```

### DO: Return action for async handling
```rust
if ui.button("Save").clicked() {
    return Some(Action::Save(data));  // Handled in update loop
}
```
```

### Anti-Pattern 5: No Verification Hook

**Problem:** Claude "completes" work with hidden stubs.

**Solution:** Explicit verification command that Claude is trained to run.

```markdown
# In CLAUDE.md
## Completion Protocol
Before declaring any task complete, run /verify-complete and report findings.
```

---

## Advanced Patterns

### Pattern: Contextual Command Chaining

Combine commands for complex tasks:

```
User: /kyc /egui Create a UBO visualization panel

Claude receives:
1. KYC domain knowledge (what is UBO, ownership chains, thresholds)
2. egui constraints (state management, action patterns)
3. The actual request
```

### Pattern: Command Templates with Variables

Use markdown structure that Claude can pattern-match:

```markdown
# Database Migration Command

## Template
```sql
-- Migration: {{DESCRIPTION}}
-- Created: {{DATE}}
-- Author: {{AUTHOR}}

BEGIN;

-- Forward migration
{{FORWARD_SQL}}

-- Rollback
-- {{ROLLBACK_SQL}}

COMMIT;
```

## Example
```sql
-- Migration: Add LEI validation column
-- Created: 2026-01-02
...
```
```

### Pattern: Self-Documenting Commands

Commands that explain their own usage:

```markdown
# /verify-complete

## What This Command Does
Injects a verification checklist into context to prevent incomplete implementations.

## When To Use
- Before saying "done" or "complete"
- After implementing a new feature
- Before creating a PR

## What Claude Will Do
1. Run grep patterns for TODO/FIXME/stub
2. List any findings
3. Provide accurate completion status

## Example Interaction
```
User: /verify-complete Check the research module

Claude: Running verification...
Found 2 items:
- src/research/llm_client.rs:108 - "// Web search API integration pending"
- src/research/executor.rs:42 - "todo!()" 

Status: NOT COMPLETE
Blocking items: Web search is stubbed, todo!() in executor
```
```

### Pattern: Progressive Disclosure

Start simple, reference depth:

```markdown
# /dsl

## Quick Syntax
```
(domain.verb :key "value" :key2 value2 :as @binding)
```

## Common Verbs
- `entity.create` - Create new entity
- `gleif.enrich` - Fetch from GLEIF
- `workflow.advance` - Progress workflow

## Deep Dive
For full verb reference: `rust/config/verbs/`
For DSL architecture: `docs/architecture/WHY-DSL-PLUS-AGENT.md`
For parser implementation: `rust/src/dsl_v2/parser.rs`
```

---

## Comparison to Other Approaches

### vs. System Prompts (API)

| Aspect | Custom Commands | System Prompts |
|--------|-----------------|----------------|
| Version Control | Git ✓ | External management |
| Team Sharing | Automatic via repo | Manual distribution |
| Contextual Loading | On-demand | Always loaded |
| Size Limits | Practical (context window) | Explicit token limits |
| Iteration Speed | Edit file, immediate | Redeploy/reconfigure |

### vs. RAG (Retrieval Augmented Generation)

| Aspect | Custom Commands | RAG |
|--------|-----------------|-----|
| Precision | Exact content injected | Similarity search |
| Latency | Instant (local file) | Vector DB query |
| Curation | Human-curated | Algorithm-selected |
| Maintenance | Manual updates | Index maintenance |
| Best For | Known patterns | Large doc corpus |

### vs. Fine-Tuning

| Aspect | Custom Commands | Fine-Tuning |
|--------|-----------------|-------------|
| Update Speed | Instant | Hours/days |
| Cost | Free | Compute costs |
| Specificity | Very high | Generalized |
| Reversibility | Edit file | Retrain |
| Best For | Project-specific | Org-wide patterns |

### The Sweet Spot

Custom commands excel for:
- **Project-specific patterns** that don't generalize
- **Rapidly evolving** codebases
- **Team knowledge** that needs version control
- **Guardrails** against known AI failure modes
- **Domain expertise** injection on demand

---

## Implementation Checklist

### Starting Fresh

```bash
# Create command directory
mkdir -p .claude/commands

# Create initial commands
touch .claude/commands/build.md      # How to build/test/deploy
touch .claude/commands/verify.md     # Completion verification
touch .claude/commands/patterns.md   # Code patterns to follow

# Add to git
git add .claude/
git commit -m "Add Claude Code custom commands"
```

### Minimum Viable Commands

1. **build.md** - Build, test, deploy commands
2. **verify.md** - Completion verification checklist
3. **patterns.md** - Code patterns and anti-patterns

### Growing Your Command Library

As you discover repeated explanations to Claude:
1. Note the pattern
2. Create a command
3. Use it next time
4. Refine based on results

### Team Onboarding

```markdown
# In README.md

## AI-Assisted Development

This project uses Claude Code with custom commands:

- `/build` - Build and deployment commands
- `/verify` - Verify implementation complete
- `/domain` - Domain expertise injection

See `.claude/commands/` for full list.
```

---

## Conclusion

The combination of:
- `CLAUDE.md` for always-on context
- `.claude/commands/` for on-demand expertise
- YAML configs for dynamic vocabulary

Creates a **version-controlled, team-shareable, iteratively-refinable** system for AI-assisted development.

This is "Prompt-as-Code" - treating AI instructions with the same rigor as application code:
- Version controlled
- Code reviewed
- Tested through use
- Iterated based on results

The ob-poc project demonstrates this pattern at scale: 8 custom commands, comprehensive CLAUDE.md, and YAML-driven DSL configuration working together to enable rapid, consistent, AI-assisted development.

---

## Appendix: ob-poc Command Reference

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `/build` | Build, test, deploy runbook | Starting work, CI issues |
| `/egui` | UI development constraints | Any egui/WASM work |
| `/kyc` | KYC/AML domain knowledge | Domain modeling, business logic |
| `/dsl` | DSL syntax and patterns | Parser, compiler, verb work |
| `/types` | Type system patterns | Rust type design |
| `/custody` | Custody domain knowledge | Settlement, safekeeping logic |
| `/verification` | Verification patterns | Document, screening checks |
| `/verify-complete` | Anti-stub enforcement | Before declaring "done" |

---

*Document generated from ob-poc development experience. Patterns validated through 4 months of agentic AI development.*
