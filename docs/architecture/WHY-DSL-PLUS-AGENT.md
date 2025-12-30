# Why DSL + Agent? The Complexity Justification

**Date:** 2025-12-30  
**Context:** Response to "it's too complicated, Spring would do this"

---

## The Criticism

> "It's too complicated. Typical of you Adam - being flash for the sake of showing off. Java Spring and a bit of ORM magic will do all this. Once again you are making it complex to dis Java for your own ends."

This is a fair challenge. Let's address it honestly.

---

## The Honest Concession

**If all you need is "show me a tree, let me click nodes"** - yes, Spring Data JPA + recursive entity mapping + React tree component would do it in a fraction of the code:

```java
@Entity
public class LegalEntity {
    @ManyToOne
    private LegalEntity parent;
    
    @OneToMany(mappedBy = "parent")
    private List<LegalEntity> children;
}

// Done. Hibernate lazy-loads the tree. UI renders it.
```

**That criticism has merit in that narrow scope.**

---

## But That's Not What We're Building

The requirement is agent-navigable structures where the user says:

- "focus on Lux"
- "show me the control prong"
- "where is Hans a director"
- "go up"

**That's natural language parsing, not CRUD.**

Spring doesn't give you that. You'd end up writing a parser anyway - either:
- A shit one full of `if (input.contains("focus"))` spaghetti, or
- A proper one (which is what Nom gives us in ~200 lines)

---

## The Challenge to Critics

> "Show me how you'd implement 'where is Hans a director' across 47 CBUs with temporal filtering in Spring, and have it respond in under 100ms."

They'll either:
1. Write a custom query parser (proving our point), or
2. Say "just add a search box" (missing the agent use case entirely)

---

## The Screen Count Argument

This is underrated in enterprise software discussions.

### Traditional Approach

```
- CBU List screen
- CBU Detail screen
- Entity List screen  
- Entity Detail screen
- Ownership Tree screen
- UBO Report screen
- Control Prong screen
- Role Assignment screen
- KYC Case screen
- Document Upload screen
- Search Results screen
- ... 40 more
```

Each screen needs: design, build, test, maintain, train users on.

### Agent Approach

```
- One chat pane
- One visualization pane (dynamic)
```

User says what they want. System figures out how to show it.

**The reduction in screens/real estate alone is a massive saving.**

---

## The DSL as Contract

The DSL isn't just "fancy syntax". It's a **stable interface** between:

| Layer | What DSL Provides |
|-------|-------------------|
| Human intent | Parsed command |
| Agent reasoning | Executable operations |
| Audit trail | Replayable script |
| Testing | Deterministic scenarios |

Try getting that from Spring Data JPA.

---

## The Complexity Trade-Off

| Complexity You Accept | Complexity You Eliminate |
|-----------------------|--------------------------|
| Nom parser (~200 lines) | 50+ screen components |
| EntityGraph struct | Screen-to-screen navigation logic |
| Navigation executor | "How do I get to X?" user confusion |
| Role taxonomy | Hardcoded layout rules per screen |

**You pay once in the engine to avoid paying repeatedly in UI.**

---

## What Other Approaches Exist?

| Approach | What You Get | What You Lose |
|----------|--------------|---------------|
| **Traditional CRUD UI** | 50+ screens, forms, wizards | User needs to know where to click. Training. Documentation. |
| **Low-code / Form builder** | Faster screen dev, but still screens | Same navigation problem. |
| **GraphQL + React tree** | Flexible queries, nice tree viz | No natural language. Still click-to-navigate. |
| **Chatbot + intent matching** | "Natural" language | Fragile. Falls over on anything outside trained intents. |
| **Graph DB + LLM Cypher** | Natural queries | Trusting LLM to write correct Cypher. Terrifying for compliance. |
| **DSL + Agent** | One interface. Ask for what you want. | Complexity in the engine (but hidden from user). |

### The Graph Database Alternative

The only real alternative is Neo4j with Cypher queries exposed to an LLM:

```cypher
MATCH (p:Person)-[:DIRECTOR_OF]->(e:Entity)
WHERE p.name CONTAINS 'Hans'
RETURN p, e
```

But then you're trusting the LLM to write correct Cypher on every request - which is terrifying for a compliance system.

**The DSL constrains what's possible. That's a feature, not a bug.**

---

## The Requirement That Drove This

The decision to put everything behind an agent drove this architecture.

Without agent requirement:
- Build 50 screens
- Train users on navigation
- Document workflows
- Support "where is X?" questions manually

With agent requirement:
- Build one interface
- User asks, system responds
- Navigation is conversational
- "Where is X?" is just a command

**The complexity exists because the requirement exists.**

---

## Valid Criticism

If this were a traditional app with forms and buttons, you wouldn't need any of this.

The complexity is justified by the agentic interface requirement - conversational navigation, not click-through UI.

**If someone doesn't need that, they don't need this architecture. Simple as.**

---

## The Payback

| Investment | Return |
|------------|--------|
| Nom parser (200 lines) | Natural language navigation |
| EntityGraph struct (500 lines) | Unified CBU + UBO visualization |
| Navigation executor (300 lines) | "go up", "focus on Lux", "where is Hans" |
| Role taxonomy (DB + YAML) | Automatic layout by role category |
| DSL verb system (existing) | Agent can execute any operation |

**Total new code: ~1000 lines**

**Screens eliminated: 40+**

**User training eliminated: "just ask"**

---

## What Would Spring Actually Look Like?

To implement agent navigation in Spring, you'd need:

```java
// Still need a parser
@Service
public class NavigationCommandParser {
    public NavigationCommand parse(String input) {
        if (input.toLowerCase().contains("focus on")) {
            String target = extractTarget(input);
            if (isJurisdiction(target)) {
                return new FilterJurisdictionCommand(normalizeJurisdiction(target));
            }
            // ... 50 more conditions
        }
        if (input.toLowerCase().contains("go up")) {
            return new GoUpCommand();
        }
        // ... 25 more command types
    }
}

// Still need state management
@Component
@SessionScope
public class NavigationState {
    private UUID currentCursor;
    private ProngFilter prongFilter;
    private List<String> jurisdictionFilter;
    private LocalDate asOfDate;
    // ... getters, setters, history management
}

// Still need graph traversal
@Service
public class GraphNavigationService {
    public NavigationResult goUp(UUID currentEntity) {
        // Custom query for parent
        // Filter application
        // History update
    }
    
    public NavigationResult goDown(UUID currentEntity, String childName) {
        // Custom query for children
        // Name matching
        // Filter application
    }
    
    public List<RoleAssignment> whereIs(String personName, String role) {
        // Cross-CBU query
        // Temporal filtering
        // Result aggregation
    }
}
```

**You'd write the same logic, just in Java instead of Rust, with more boilerplate.**

The parser would be messier than Nom. The state management would fight Spring's request-scoped defaults. The queries would still need custom JPQL.

**Spring doesn't magically solve the agent navigation problem.**

---

## Summary

| Argument | Response |
|----------|----------|
| "It's too complicated" | The agent requirement creates the complexity. Simpler alternatives don't support natural language navigation. |
| "Spring would do this" | Spring would do CRUD. Agent navigation still needs a parser, state machine, and graph traversal - same work, more boilerplate. |
| "You're showing off" | 200 lines of Nom vs 50 screens. The payback is real. |
| "Just use a search box" | "Where is Hans a director across 47 CBUs with temporal filtering" isn't a search box query. |

---

## The Bottom Line

**The DSL + Agent approach isn't complex for complexity's sake.**

It's complex because:
1. Natural language navigation requires parsing
2. Stateful tree traversal requires a state machine
3. Cross-entity queries require graph operations
4. Temporal filtering requires date-aware queries

Any solution that meets the requirements will have this complexity. The question is where you put it:
- In 50 screens with navigation logic scattered everywhere, or
- In one engine that handles it all

We chose the engine.
