# Compile-Time Instance Binding: Why It Matters for Agentic Systems

*Captured: 2024-12-20*
*Context: Deep dive on why the ob-poc DSL's compile-time entity resolution creates deterministic agentic outcomes, contrasted with Java/Spring/Hibernate runtime resolution*

---

## The Core Innovation

Most programming languages stop at **type resolution** at compile time. Instance binding happens at runtime.

This DSL goes further: **instance resolution at compile time**.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TRADITIONAL: Type-checked, Instance-bound-at-runtime                      │
│                                                                             │
│  Compiler: "This variable is of type Person" ✓                            │
│  Runtime:  "Does person 'John Smith' exist? Let me query... maybe... 💥"  │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  THIS DSL: Type-checked AND Instance-bound at compile time                 │
│                                                                             │
│  Compiler: "This references entity type 'manco'" ✓                        │
│  Compiler: "'BlackRock ManCo' exists, UUID is 550e8400..." ✓              │
│  Runtime:  "Executing with known-good UUID" ✓                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Four-Phase Pipeline

### Phase 1: Parse (Syntax)

```
Input:  (cbu.ensure :name "Apex" :manco-id "BlackRock ManCo")

Output: VerbCall {
          domain: "cbu", verb: "ensure",
          args: [
            {key: "name", value: Literal::String("Apex")},
            {key: "manco-id", value: Literal::String("BlackRock ManCo")}
          ]           ↑
        }             Just a string - parser doesn't know it's a reference

Status: Well-formed syntax ✓
Errors: Syntax only (missing parens, malformed tokens)
```

### Phase 2: Enrich (Semantic Classification)

```
YAML config says: manco-id has lookup: {entity_type: "entity"}

Output: VerbCall {
          args: [
            {key: "name", value: Literal::String("Apex")},
            {key: "manco-id", value: EntityRef {
              entity_type: "entity",
              search_column: "name",
              value: "BlackRock ManCo",    ← Human input preserved
              resolved_key: None           ← UNRESOLVED (valid intermediate state)
            }}
          ]
        }

Status: Knows WHAT needs resolving
Errors: Unknown verbs, unknown arguments, type mismatches
```

### Phase 3: Resolve (Instance Binding)

```
EntityGateway query: "Find entity where name ≈ 'BlackRock ManCo'"

POSSIBLE OUTCOMES:
┌─────────────────────────────────────────────────────────────────────────────┐
│  Exactly 1 match    →  Auto-resolve, continue                              │
│  Multiple matches   →  Disambiguation required (STOP, ask user)            │
│  No matches         →  Error or "create new?" prompt (STOP, ask user)      │
└─────────────────────────────────────────────────────────────────────────────┘

Output (success): EntityRef {
                    value: "BlackRock ManCo",        ← STILL PRESERVED
                    resolved_key: Some("550e8400...")  ← NOW BOUND
                  }

Status: Instance bound at compile time ✓
Errors: Unresolved references, ambiguous references
```

### Phase 4: DAG (Execution Planning)

```
Input: Multiple statements with @binding dependencies

(entity.create-limited-company :name "HoldCo" :as @holdco)
(cbu.ensure :name "Fund" :manco-id @holdco :as @fund)
(ubo.add-ownership :owner @holdco :owned @fund)

Output: ExecutionPlan {
          stages: [
            Stage 1: [@holdco]           ← No deps, execute first
            Stage 2: [@fund]             ← Needs @holdco
            Stage 3: [@ubo_ownership]    ← Needs both
          ]
        }

Status: Deterministic execution order ✓
Errors: Circular dependencies, undefined bindings
```

---

## Why This Creates Deterministic Agentic Outcomes

### The Agent Loop Problem

Traditional agent systems:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TRADITIONAL AGENT (Runtime Resolution)                                    │
│                                                                             │
│  1. Agent generates code/SQL/API calls                                     │
│  2. Code executes                                                          │
│  3. Runtime error: "Entity not found"                                      │
│  4. Agent sees error, tries to recover                                     │
│  5. But: 3 other operations already committed! 💥                          │
│  6. State is now inconsistent                                              │
│  7. Agent hallucinates recovery strategy                                   │
│  8. Makes it worse                                                         │
│                                                                             │
│  The agent is debugging at runtime with partial information.               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

This DSL's approach:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THIS DSL (Compile-Time Resolution)                                        │
│                                                                             │
│  1. Agent generates DSL with human-readable names                          │
│  2. DSL is parsed and enriched (Phase 1-2) ✓                               │
│  3. Resolution phase (Phase 3) runs BEFORE any execution                   │
│                                                                             │
│     "BlackRock ManCo" → FAIL: 3 matches found                              │
│                                                                             │
│  4. Agent receives structured disambiguation request                       │
│  5. Agent can ask user OR make informed choice                             │
│  6. Resolution retries with clarified input                                │
│  7. ALL references resolved ✓                                              │
│  8. DAG computed, execution plan ready ✓                                   │
│  9. NOW execution happens - with known-good references                     │
│                                                                             │
│  No partial commits. No inconsistent state. No hallucinated recovery.      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Determinism Guarantee

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DETERMINISM PROPERTIES                                                    │
│                                                                             │
│  1. PARSE is deterministic                                                 │
│     Same input → same AST (always)                                         │
│                                                                             │
│  2. ENRICH is deterministic                                                │
│     Same AST + same YAML config → same enriched AST (always)               │
│                                                                             │
│  3. RESOLVE is deterministic given database state                          │
│     Same enriched AST + same DB → same resolved AST (always)               │
│     Ambiguity → structured error, not random choice                        │
│                                                                             │
│  4. DAG is deterministic                                                   │
│     Same resolved AST → same execution plan (always)                       │
│     Topological sort is stable                                             │
│                                                                             │
│  5. EXECUTE is deterministic                                               │
│     Same execution plan → same DB operations (always)                      │
│     Order is fixed by DAG, not runtime discovery                           │
│                                                                             │
│  The ONLY non-determinism is user choice during disambiguation.            │
│  And that's EXPLICIT and LOGGED.                                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Agent Error Handling Comparison

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ERROR: "John Smith" matches 3 entities                                    │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  TRADITIONAL AGENT                                                         │
│                                                                             │
│  Runtime: SQLException: duplicate key or foreign key violation             │
│  Agent: "Hmm, something went wrong. Let me try SELECT * FROM..."          │
│  Agent: *queries wrong table*                                              │
│  Agent: "I don't see the problem, let me retry the insert"                │
│  Agent: *makes it worse*                                                   │
│                                                                             │
│  The agent is pattern-matching on error strings. Garbage in, garbage out. │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  THIS DSL                                                                  │
│                                                                             │
│  Compile: DisambiguationRequired {                                         │
│             param: "entity-id",                                            │
│             search_text: "John Smith",                                     │
│             matches: [                                                     │
│               {id: "...", name: "John Smith", dob: "1980-01-15"},         │
│               {id: "...", name: "John Smith", dob: "1975-03-22"},         │
│               {id: "...", name: "John A. Smith", dob: "1990-07-08"}       │
│             ]                                                              │
│           }                                                                │
│                                                                             │
│  Agent: "I found 3 people named John Smith. Based on the context          │
│          (we're setting up a Luxembourg fund), I'll ask the user to       │
│          clarify, or use the DOB hint from the document."                 │
│                                                                             │
│  The agent has STRUCTURED information. It can make informed decisions.    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Java/Spring/Hibernate Contrast

### The Seductive Promise

```java
// "Look how clean this is!"
@Service
public class OnboardingService {
    
    @Autowired
    private EntityRepository entityRepo;
    
    @Autowired  
    private CbuRepository cbuRepo;
    
    @Transactional
    public Cbu createCbu(String name, String mancoName) {
        Entity manco = entityRepo.findByName(mancoName);  // 💥 Runtime
        Cbu cbu = new Cbu(name, manco);
        return cbuRepo.save(cbu);
    }
}
```

Looks clean. What could go wrong?

### Everything. Everything Can Go Wrong.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PROBLEM 1: @Autowired - Runtime Dependency Injection                      │
│                                                                             │
│  @Autowired                                                                │
│  private EntityRepository entityRepo;                                      │
│                                                                             │
│  Compiler says: ✓ (it's a field with an annotation, looks fine)           │
│                                                                             │
│  What actually happens:                                                    │
│  • Spring boots up                                                         │
│  • Scans 847 @Component classes                                            │
│  • Builds dependency graph at RUNTIME                                      │
│  • Hopes EntityRepository has exactly one implementation                   │
│  • If two implementations exist: NoUniqueBeanDefinitionException          │
│  • If zero implementations exist: NoSuchBeanDefinitionException           │
│                                                                             │
│  When do you find out? When the container starts. In production. At 3am.  │
│                                                                             │
│  Can the compiler help? No. It's just an annotation on a field.           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PROBLEM 2: Repository.findByName() - Runtime Query Execution              │
│                                                                             │
│  Entity manco = entityRepo.findByName(mancoName);                          │
│                                                                             │
│  Compiler says: ✓ (method exists, returns Entity, looks fine)             │
│                                                                             │
│  What actually happens:                                                    │
│  • Spring Data generates SQL at RUNTIME from method name                   │
│  • Query executes against database                                         │
│  • Returns null if not found (no exception!)                               │
│  • Or returns one of multiple matches (which one? undefined!)              │
│  • Or throws if multiple and you used findOne()                            │
│                                                                             │
│  When do you find out "BlackRock ManCo" doesn't exist?                    │
│  At runtime. After the transaction started. Maybe after other writes.     │
│                                                                             │
│  What does the compiler know about "BlackRock ManCo"?                     │
│  Nothing. It's a string. Strings are strings.                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PROBLEM 3: Hibernate Entity Mapping - Runtime Schema Discovery            │
│                                                                             │
│  @Entity                                                                   │
│  @Table(name = "entities")                                                 │
│  public class Entity {                                                     │
│      @Column(name = "nmae")  // ← Typo: "nmae" instead of "name"          │
│      private String name;                                                  │
│  }                                                                         │
│                                                                             │
│  Compiler says: ✓ (it's a string annotation, looks fine)                  │
│                                                                             │
│  When do you find out "nmae" column doesn't exist?                        │
│  • If hibernate.hbm2ddl.auto=validate: at boot time (in prod, 3am)        │
│  • If hibernate.hbm2ddl.auto=update: Hibernate CREATES the typo column!  │
│  • If hibernate.hbm2ddl.auto=none: first query fails at runtime           │
│                                                                             │
│  The compiler cannot help. Column names are strings. Strings are strings. │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PROBLEM 4: @Transactional - Runtime Transaction Boundaries                │
│                                                                             │
│  @Transactional                                                            │
│  public Cbu createCbu(String name, String mancoName) {                     │
│      Entity manco = entityRepo.findByName(mancoName);                      │
│      Cbu cbu = new Cbu(name, manco);                                       │
│      cbuRepo.save(cbu);                                                    │
│      auditRepo.save(new AuditEntry(...));  // If this fails...            │
│      return cbu;                            // ...does cbu rollback?       │
│  }                                                                         │
│                                                                             │
│  Compiler says: ✓ (annotation, looks fine)                                │
│                                                                             │
│  What actually happens:                                                    │
│  • Spring wraps method in proxy at RUNTIME                                 │
│  • Transaction started before method                                       │
│  • If unchecked exception: rollback                                        │
│  • If checked exception: NO rollback (unless configured)                   │
│  • If called from same class: proxy bypassed, NO transaction!             │
│                                                                             │
│  The behavior depends on runtime proxy magic, exception types, and         │
│  whether you called the method correctly. Compiler knows nothing.          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Testing Illusion

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  "But we have tests!"                                                      │
│                                                                             │
│  @Test                                                                     │
│  void testCreateCbu() {                                                    │
│      when(entityRepo.findByName("BlackRock")).thenReturn(mockEntity);     │
│      when(cbuRepo.save(any())).thenReturn(mockCbu);                       │
│                                                                             │
│      Cbu result = service.createCbu("Fund", "BlackRock");                 │
│                                                                             │
│      assertNotNull(result);  // ✓ Passes!                                 │
│  }                                                                         │
│                                                                             │
│  What does this test prove?                                                │
│  • That mocks return what you told them to return                         │
│  • That your code works with perfect inputs                                │
│  • NOTHING about database schema                                           │
│  • NOTHING about actual query behavior                                     │
│  • NOTHING about transaction boundaries                                    │
│  • NOTHING about whether "BlackRock" exists in production                 │
│                                                                             │
│  The test is checking that your fantasy world is internally consistent.   │
│  Production is not your fantasy world.                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Integration Test Escape Hatch

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  "Fine, we'll use @SpringBootTest with a real database!"                   │
│                                                                             │
│  @SpringBootTest                                                           │
│  @Testcontainers                                                           │
│  class OnboardingServiceIT {                                               │
│      @Container                                                            │
│      static PostgreSQLContainer<?> postgres = new PostgreSQLContainer<>();│
│                                                                             │
│      @Test                                                                 │
│      void testCreateCbu() {                                                │
│          // Set up test data...                                            │
│          // Run test...                                                    │
│      }                                                                     │
│  }                                                                         │
│                                                                             │
│  Problems:                                                                 │
│  • Takes 30+ seconds to start (so devs run it rarely)                     │
│  • Test database schema may drift from production                          │
│  • Test data is synthetic, not production entity names                     │
│  • You're testing findByName("Test Entity 1"), not "BlackRock ManCo"      │
│  • CI passes, production fails on data that doesn't exist in tests        │
│                                                                             │
│  You've moved the problem, not solved it.                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The DSL Alternative: Compile-Time Instance Binding

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THIS DSL                                                                  │
│                                                                             │
│  (cbu.ensure                                                               │
│    :name "Apex Fund"                                                       │
│    :manco-id "BlackRock ManCo"                                             │
│    :as @apex)                                                              │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  PHASE 1: Parse                                                            │
│                                                                             │
│  Compiler says: ✓ Syntax valid                                            │
│  Can fail: Malformed s-expression                                          │
│  Equivalent Java failure: Won't compile (syntax error)                     │
│  → SAME as Java                                                            │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  PHASE 2: Enrich                                                           │
│                                                                             │
│  Compiler says: ✓ cbu.ensure is valid verb, :manco-id is valid arg        │
│  Can fail: Unknown verb, unknown argument, type mismatch                   │
│  Equivalent Java failure: @Autowired NoSuchBeanDefinitionException        │
│  → BETTER: Caught at compile time, not Spring boot time                   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  PHASE 3: Resolve                                                          │
│                                                                             │
│  Compiler says: ✓ "BlackRock ManCo" exists, UUID is 550e8400...           │
│  Can fail: Not found, ambiguous match                                      │
│  Equivalent Java failure: findByName() returns null at runtime            │
│  → MUCH BETTER: Caught at compile time, before any execution              │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  PHASE 4: DAG                                                              │
│                                                                             │
│  Compiler says: ✓ Execution order: stage 1, stage 2...                    │
│  Can fail: Circular dependency, undefined binding                          │
│  Equivalent Java failure: StackOverflow or NullPointerException           │
│  → MUCH BETTER: Caught at compile time with clear error message           │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  EXECUTION                                                                 │
│                                                                             │
│  All references are resolved. All dependencies are ordered.                │
│  Execution is deterministic. No runtime surprises.                         │
│                                                                             │
│  Equivalent Java: @Transactional with runtime exceptions                   │
│  → MUCH BETTER: Known-good references, predictable execution              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## SQLx: The Same Philosophy for SQL

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  HIBERNATE                                                                 │
│                                                                             │
│  @Query("SELECT e FROM Entity e WHERE e.nmae = :name")  // Typo           │
│  Entity findByName(String name);                                           │
│                                                                             │
│  Compiler says: ✓ (it's a string, strings are valid)                      │
│  Runtime says: QuerySyntaxException (if you're lucky)                      │
│                or silently returns nothing (if column exists but empty)   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  SQLX                                                                      │
│                                                                             │
│  let entity = sqlx::query_as!(                                             │
│      Entity,                                                               │
│      "SELECT * FROM entities WHERE nmae = $1",  // Typo                   │
│      name                                                                  │
│  ).fetch_one(&pool).await?;                                                │
│                                                                             │
│  Compiler says: ❌ error: column "nmae" does not exist                     │
│                                                                             │
│  The compiler ACTUALLY RUNS THE QUERY against Postgres to validate.       │
│  Not at runtime. At compile time.                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Agentic Implications

### Why This Matters for AI Agents

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TRADITIONAL AGENT (Java-style runtime resolution)                         │
│                                                                             │
│  Agent generates: service.createCbu("Fund", "BlackRock ManCo")             │
│                                                                             │
│  1. Code compiles ✓ (of course, it's just strings)                        │
│  2. Code runs...                                                           │
│  3. findByName() executes...                                               │
│  4. Oops: Returns null (not found) or wrong entity (multiple matches)     │
│  5. NullPointerException or wrong data propagates                          │
│  6. Agent sees: "java.lang.NullPointerException at line 47"               │
│  7. Agent has NO IDEA what "BlackRock ManCo" resolved to                  │
│  8. Agent guesses at recovery strategy                                     │
│  9. Agent makes it worse                                                   │
│                                                                             │
│  The agent is debugging with error messages, not structured feedback.      │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  THIS DSL (compile-time resolution)                                        │
│                                                                             │
│  Agent generates: (cbu.ensure :name "Fund" :manco-id "BlackRock ManCo")   │
│                                                                             │
│  1. DSL parses ✓                                                          │
│  2. DSL enriches ✓                                                        │
│  3. Resolution runs BEFORE execution...                                    │
│  4. Result:                                                                │
│                                                                             │
│     ResolutionResult::Ambiguous {                                          │
│       param: "manco-id",                                                   │
│       search_text: "BlackRock ManCo",                                      │
│       matches: [                                                           │
│         {id: "uuid1", name: "BlackRock ManCo S.à r.l.", jurisdiction: LU},│
│         {id: "uuid2", name: "BlackRock ManCo GmbH", jurisdiction: DE},    │
│       ]                                                                    │
│     }                                                                      │
│                                                                             │
│  5. Agent receives STRUCTURED disambiguation request                       │
│  6. Agent can: ask user, use context hints, or choose based on rules      │
│  7. Resolution retries with clarified input                                │
│  8. All resolved ✓                                                        │
│  9. Execution proceeds with known-good references                          │
│                                                                             │
│  The agent operates with structured data, not error string parsing.        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Determinism Chain

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AGENT DETERMINISM PROPERTIES                                              │
│                                                                             │
│  1. AGENT OUTPUT → DSL                                                     │
│     Agent produces text in a constrained grammar                           │
│     Grammar is defined by YAML verb registry                               │
│     Invalid syntax is caught immediately                                   │
│     → DETERMINISTIC                                                        │
│                                                                             │
│  2. DSL → ENRICHED AST                                                     │
│     YAML config determines which args need resolution                      │
│     Same DSL + same config = same enriched AST                             │
│     → DETERMINISTIC                                                        │
│                                                                             │
│  3. ENRICHED AST → RESOLVED AST                                            │
│     EntityGateway queries are deterministic                                │
│     Same query + same DB state = same results                              │
│     Ambiguity produces structured request, not random choice               │
│     → DETERMINISTIC (or structured user interaction)                       │
│                                                                             │
│  4. RESOLVED AST → EXECUTION PLAN                                          │
│     DAG construction is deterministic                                      │
│     Topological sort is stable                                             │
│     Same AST = same execution order                                        │
│     → DETERMINISTIC                                                        │
│                                                                             │
│  5. EXECUTION PLAN → DB OPERATIONS                                         │
│     Each operation uses resolved UUIDs                                     │
│     No runtime lookups, no surprises                                       │
│     → DETERMINISTIC                                                        │
│                                                                             │
│  END-TO-END: Agent output → DB state                                       │
│  The ONLY non-determinism is explicit user choice during disambiguation.  │
│  Everything else is predictable, reproducible, auditable.                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The User Visibility Model

### What the User Sees at Each Phase

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DSL INPUT                                                                 │
│                                                                             │
│  (cbu.ensure                                                               │
│    :name "Apex Fund"                           ← Literal (no decoration)  │
│    :manco-id "BlackRock ManCo"                 ← EntityRef (decorated)    │
│    :as @apex)                                                              │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  VISUAL STATES                                                             │
│                                                                             │
│  Unresolved:   ⚠ "BlackRock ManCo"      (yellow, pending resolution)      │
│  Resolving:    ⏳ "BlackRock ManCo"      (spinner, query in progress)      │
│  Resolved:     ✓ "BlackRock ManCo"      (green, hover shows UUID)         │
│  Ambiguous:    ⚡ "John Smith" (3)       (orange, click to pick)           │
│  Not found:    ✗ "Xyz Corp"             (red, error)                      │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  THE KEY INSIGHT                                                           │
│                                                                             │
│  EntityRef carries BOTH views:                                             │
│                                                                             │
│  EntityRef {                                                               │
│    value: "BlackRock ManCo",        ← USER sees this (always)             │
│    resolved_key: Some("550e8400"),  ← EXECUTOR uses this                  │
│  }                                                                         │
│                                                                             │
│  User reviews INTENT: "BlackRock ManCo"                                   │
│  System executes with UUID: "550e8400..."                                 │
│  Both are true. Both are preserved. Audit trail is complete.              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Summary: Why This Approach Wins

| Aspect | Java/Spring/Hibernate | This DSL |
|--------|----------------------|----------|
| Type checking | Compile time | Compile time |
| Instance binding | Runtime | **Compile time** |
| Schema validation | Boot time (if configured) | **Compile time (SQLx)** |
| Query validation | Runtime | **Compile time** |
| Dependency resolution | Runtime (Spring DI) | **Compile time (DAG)** |
| Error discovery | Production | **Development** |
| Agent error handling | Parse error strings | **Structured responses** |
| Disambiguation | Random/undefined | **Explicit user choice** |
| Audit trail | Log files | **AST with both views** |
| Determinism | Pray | **Guaranteed** |

---

## Key Quotes

> "Most programming languages stop at variable type == my entity type. My DSL needs to resolve entity instance - at compile time."

> "The trick is how to show the user what's going on."

> "EntityRef is the escape hatch. It carries both views."

---

## Implications for Production

1. **Agent reliability** - Errors caught before execution, not during
2. **User trust** - They see what they asked for, not UUIDs
3. **Audit compliance** - Full trail from intent to execution
4. **Debugging** - Know exactly what resolved to what
5. **Testing** - Test against real data, not mocks
6. **Reproducibility** - Same input = same output (given same DB state)

---

*The best runtime is one that never surprises you, because the compiler already caught it.*
