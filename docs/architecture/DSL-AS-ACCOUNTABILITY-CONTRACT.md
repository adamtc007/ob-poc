# DSL as Accountability Contract: Bridging Product and Engineering

**Document Type**: Architecture Philosophy  
**Status**: Working Draft  
**Last Updated**: 2024-12-30

---

## Executive Summary

This document argues that a Domain-Specific Language (DSL) aligned with business entity models serves as an **accountability contract** between Product and Engineering teams. Unlike traditional backlog management approaches (SAFe, Scrum user stories), a well-designed DSL eliminates the ambiguity that enables blame-shifting between teams.

The ob-poc DSL is not merely a technical convenience—it is an organizational mechanism that forces precision from Product and determinism from Engineering.

---

## The Problem: Structural Dysfunction in Agile at Scale

### The Impedance Mismatch

Modern scaled agile frameworks (SAFe, Platform Operating Model) create a structural separation between:

| Role | Supposed Responsibility | Actual Behavior |
|------|------------------------|-----------------|
| **Product** | "Own the backlog" | Write ambiguous requirements, avoid technical understanding |
| **Engineering** | "Bid for work, burn backlog" | Deliver technically correct but business-worthless features |

This separation creates an **impedance mismatch**:
- Product lacks technical depth to specify precisely
- Engineering lacks domain knowledge to interpret intent
- Both sides benefit from the resulting ambiguity

### Why the Dysfunction Persists

The current system is **stable in its dysfunction** because it serves multiple interests:

1. **Engineering benefits**: Can deliver code that "passes acceptance criteria" regardless of business value. When outcomes are poor: "We built what the story said."

2. **Product benefits**: Can write vague requirements without learning the domain deeply. When outcomes are poor: "Engineering didn't understand what we meant."

3. **Framework vendors benefit**: More process overhead means more certifications, consultants, and tool licenses.

4. **Middle management benefits**: More ceremonies create more visibility and justify more coordination roles.

### The Blame Firewall

Traditional user stories create a **blame firewall** between teams:

```
As a compliance officer
I want to onboard a Luxembourg SICAV
So that we can begin trading

Acceptance Criteria:
- Fund is created in system
- Required entities are linked
- KYC case is initiated
```

This specification is **useless** for accountability because:
- What entities are "required"?
- What roles should they have?
- What sequence of operations?
- What validation rules apply?

Product says "you should know this." Engineering says "you didn't specify." Both are right. Both are wrong. Nobody is accountable.

---

## The Solution: DSL as Executable Specification

### What a DSL Provides

A Domain-Specific Language transforms requirements from ambiguous prose into **executable specifications**:

```lisp
; This IS the specification AND the test AND the documentation
(cbu.create 
  :name "Alpha SICAV" 
  :jurisdiction "LU" 
  :entity-type "SICAV" 
  :as @sicav)

(entity.create-limited-company 
  :name "Alpha Management Company" 
  :jurisdiction "LU" 
  :as @manco)

(cbu.assign-role 
  :cbu-id @sicav 
  :entity-id @manco 
  :role "MANAGEMENT_COMPANY")

(kyc-case.create 
  :cbu-id @sicav 
  :case-type "INITIAL_ONBOARDING"
  :as @case)
```

This specification is:

| Property | Benefit |
|----------|---------|
| **Precise** | No ambiguity about what "onboard a fund" means |
| **Business-readable** | Uses domain vocabulary (CBU, jurisdiction, role) |
| **Deterministic** | Same script produces same outcome |
| **Executable** | Can be run, not just read |
| **Testable** | Scripts can be replayed for verification |
| **Auditable** | The "requirement" IS the implementation |
| **Versionable** | Changes are explicit and tracked |

### The Accountability Shift

With a DSL-based approach:

| Role | New Responsibility | Accountability |
|------|-------------------|----------------|
| **Product** | Express intent in DSL terms | Must understand entity model well enough to specify operations |
| **Engineering** | Execute DSL correctly | Must implement verbs that produce correct domain state |

**Neither side can hide behind ambiguity.**

---

## Entity Model as Ubiquitous Language

### Domain-Driven Design Connection

The ob-poc entity model (CBU, Entity, Role, Relationship, Product, Service) serves as what Domain-Driven Design calls a **ubiquitous language**—a semi-formal vocabulary shared by all team members.

The DSL makes this language:
- **Executable** - not just documentation
- **Discoverable** - verb taxonomy reveals what's possible
- **Constrained** - only valid operations can be expressed

### The Taxonomy as Contract

The verb taxonomy defines the **contract surface** between Product and Engineering:

```
domains/
├── cbu/
│   ├── create          # Create Client Business Unit
│   ├── assign-role     # Link entity to CBU with role
│   └── update-status   # Change CBU lifecycle state
├── entity/
│   ├── create-proper-person
│   ├── create-limited-company
│   └── link-ownership
├── kyc-case/
│   ├── create
│   ├── assign-workstream
│   └── complete-task
└── product/
    ├── subscribe
    └── configure-service
```

Product can only request operations that exist. Engineering must implement operations that the taxonomy defines. The vocabulary is shared, precise, and bounded.

---

## Comparison: Traditional vs. DSL Approach

### Traditional Backlog Item

```markdown
**Story**: FUND-1234
**Title**: Onboard Luxembourg SICAV

As a compliance officer
I want to onboard a new Luxembourg SICAV fund
So that we can begin trading for this client

**Acceptance Criteria**:
- [ ] Fund record created
- [ ] Management company linked
- [ ] Directors identified
- [ ] KYC case opened
- [ ] Required documents requested

**Notes**: Similar to FUND-1198 but different structure.
Check with Jean about the umbrella requirements.
```

**Problems**:
- What fields on fund record?
- What role type for management company?
- How are directors "identified"?
- What case type?
- What documents are "required"?
- What was FUND-1198? Who is Jean?

### DSL Specification

```lisp
; FUND-1234: Onboard Luxembourg SICAV
; Specification Author: [Product Owner]
; Implementation: DSL Engine v2.3

; 1. Create the umbrella SICAV structure
(cbu.create 
  :name "Client Alpha SICAV" 
  :jurisdiction "LU"
  :fund-type "SICAV"
  :regulatory-status "UCITS"
  :as @umbrella)

; 2. Create and link management company
(entity.create-limited-company
  :name "Alpha Management S.A."
  :jurisdiction "LU"
  :lei "549300EXAMPLE000001"
  :as @manco)

(cbu.assign-role
  :cbu-id @umbrella
  :entity-id @manco
  :role "MANAGEMENT_COMPANY"
  :effective-from "2024-01-15")

; 3. Create director entities and assign control roles
(entity.create-proper-person
  :name "Jean Dupont"
  :nationality "LU"
  :as @director1)

(entity.assign-control
  :controller-id @director1
  :controlled-id @manco
  :control-type "BOARD_MEMBER"
  :role "DIRECTOR")

; 4. Initiate KYC process
(kyc-case.create
  :cbu-id @umbrella
  :case-type "INITIAL_ONBOARDING"
  :risk-rating "STANDARD"
  :as @case)

; 5. Request required documents per LU SICAV requirements
(kyc-case.request-documents
  :case-id @case
  :document-set "LU_SICAV_INITIAL"
  :due-date "2024-02-15")
```

**Benefits**:
- Every field specified
- Every relationship explicit
- Every operation auditable
- Replayable for testing
- Serves as documentation
- **No ambiguity to exploit**

---

## Why This Approach Is Rarely Adopted

### Threatens Existing Power Structures

1. **Product's existence justification**: If the DSL IS the requirement, what does the BA do all day? The answer is: they learn the domain model and express intent precisely. This is harder than writing vague stories.

2. **Engineering's escape hatch**: No more "that's not what the story said." The DSL specification is deterministic. Either you implemented the verb correctly or you didn't.

3. **The framework industry**: SAFe has ~500,000 certified practitioners. "Domain-Specific Language Design" has no certification to sell. The consulting industry has no revenue model for this approach.

### Requires Actual Domain Knowledge

| Traditional Approach | DSL Approach |
|---------------------|--------------|
| Product can remain domain-shallow | Product MUST understand entity model |
| Engineering can ignore business context | Engineering MUST understand domain semantics |
| Ambiguity absorbs incompetence | Precision exposes gaps |

### Higher Initial Investment

Designing a good DSL requires:
- Deep domain analysis
- Entity model formalization
- Verb taxonomy design
- Parser implementation
- Execution engine
- Agent integration (for discovery)

This is genuinely harder than "write stories, groom backlog, run PI Planning."

The payoff comes from:
- Eliminated ambiguity
- Reduced rework
- Executable specifications
- Audit trail
- Agent-navigable operations

---

## Integration with Agent Architecture

### The RAG-Indexed Verb Taxonomy

The DSL becomes even more powerful when integrated with an AI agent that can:

1. **Discover available operations** via semantic search
2. **Understand preconditions** for each verb
3. **Suggest next steps** based on current state
4. **Generate valid DSL** from natural language intent

```yaml
# verb_index.yaml - Agent-discoverable operations
entries:
  - verb: cbu.create
    domain: cbu
    search_text: |
      create fund client business unit onboard new sicav
    intent_patterns:
      - "onboard a new fund"
      - "create a client"
      - "set up {name} for trading"
    preconditions: []
    produces: cbu
    typical_next: [entity.create-*, cbu.assign-role]
```

### Natural Language to DSL

User (Product): "I need to onboard a Luxembourg SICAV called Alpha Fund with a ManCo in Luxembourg"

Agent generates:
```lisp
(cbu.create :name "Alpha Fund" :jurisdiction "LU" :fund-type "SICAV" :as @fund)
(entity.create-limited-company :name "[ManCo Name]" :jurisdiction "LU" :as @manco)
(cbu.assign-role :cbu-id @fund :entity-id @manco :role "MANAGEMENT_COMPANY")
```

The agent provides **scaffolding for precision** without requiring Product to learn DSL syntax directly.

---

## Organizational Implications

### The Backlog Transformation

| Traditional Backlog | DSL-Based Backlog |
|--------------------|-------------------|
| Prioritized list of wishes | Ordered sequence of executable operations |
| Ambiguous acceptance criteria | Deterministic verification |
| Documentation separate from code | Specification IS the code |
| Blame-absorbing buffer | Accountability contract |

### Role Evolution

**Product Owner becomes**: Domain modeler who expresses business intent in structured, executable form. Must understand entity relationships, valid operations, and domain constraints.

**Engineering becomes**: DSL implementers who guarantee verbs produce correct domain state. Must understand business semantics, not just technical execution.

**The gap closes** because both sides must understand and work with the same formal model.

---

## Conclusion

The ob-poc DSL is not a technical convenience—it is an **organizational intervention** that:

1. Eliminates the ambiguity that enables blame-shifting
2. Forces domain knowledge into both Product and Engineering
3. Creates executable specifications that serve as contracts
4. Provides an audit trail of business intent
5. Enables agent-assisted discovery and generation

The reason this approach is "almost never discussed" in agile literature is that **both armed camps prefer the current truce**. The DSL approach breaks the truce by requiring competence and accountability from both sides.

This is harder. It's also the only approach that actually works.

---

## References

- Fowler, M. (2010). *Domain Specific Languages*. Addison-Wesley.
- Evans, E. (2003). *Domain-Driven Design*. Addison-Wesley.
- Schwaber, K. (2013). "unSAFe at any speed" - https://kenschwaber.wordpress.com
- Cagan, M. *SVPG* - "I don't know a single strong product company using SAFe"

## Related Documents

- [WHY-DSL-PLUS-AGENT.md](./WHY-DSL-PLUS-AGENT.md) - Technical justification for DSL + Agent architecture
- [Entity Model](../entity-model/) - CBU, Entity, Role taxonomy definitions
- [Verb Taxonomy](../../rust/config/verbs/) - DSL operation definitions
