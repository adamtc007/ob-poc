# Annex: Language and Framework Selection Rationale

## The Impedance Mismatch Problem

The term "impedance mismatch" originated in electrical engineering—the loss that occurs when connecting systems with different characteristics. In software, it describes the friction when a paradigm doesn't align with the problem domain.

Object-relational impedance mismatch is well-documented: objects have identity, encapsulation, and inheritance; relations have rows, nulls, and joins. Hibernate exists to bridge this gap.

But there's a deeper impedance mismatch that Hibernate doesn't solve: **the class-centric paradigm applied to operation-centric domains**.

---

## Class-Centric vs Operation-Centric

### The Java Mental Model

Java embodies a specific worldview:

- **Nouns are primary**: Design starts with classes (`Client`, `Account`, `Document`)
- **Verbs are secondary**: Behavior lives in methods attached to classes
- **Inheritance models variation**: `HedgeFundClient extends Client`
- **State is encapsulated**: Private fields, controlled access
- **Identity is object reference**: `client1 == client2`

This paradigm excels when the domain is naturally a graph of interacting objects with complex lifecycles and polymorphic behavior.

### The OB-POC Domain

Financial services onboarding is operation-centric:

- **Verbs are primary**: "Create case", "verify allegation", "run screening"
- **Nouns are records**: Entities are data, not behavior
- **Configuration models variation**: New entity types via YAML, not subclasses
- **State is explicit**: Database rows, audit logs, execution traces
- **Identity is reference**: `@entity-id`, not object pointers

When you apply a class-centric paradigm to an operation-centric domain:

| Domain Reality | Class-Centric Friction |
|---------------|----------------------|
| 80+ verbs as first-class operations | 80+ service classes with boilerplate |
| Entity types defined by configuration | Entity subclass proliferation or reflection gymnastics |
| Flat, wide schema (92 tables) | 92 entity classes, each with annotations |
| Audit trail = sequence of operations | Bolt-on event sourcing, AOP complexity |
| AI generates operation sequences | No natural representation of "program" |

The impedance mismatch isn't between objects and relations—it's between the paradigm and the problem.

---

## Composition Over Inheritance: The Structural Argument

### The Inheritance Tax

Java's class hierarchy encourages inheritance for variation:

```java
public abstract class BaseEntity { ... }
public class NaturalPerson extends BaseEntity { ... }
public class LegalEntity extends BaseEntity { ... }
public class Trust extends LegalEntity { ... }  // Is a trust a legal entity? Sometimes?
```

Problems compound:

- **Rigid hierarchies**: What if a Trust is sometimes treated as a NaturalPerson (grantor trust)?
- **Diamond inheritance**: Java's single inheritance forces awkward compositions
- **Fragile base class**: Changes to `BaseEntity` ripple through 50 subclasses
- **LSP violations**: Subtypes don't always substitute cleanly

### Composition via Traits and Interfaces

Rust and Go take a different approach:

**Rust** (traits as composable capabilities):
```rust
pub trait Identifiable {
    fn entity_id(&self) -> Uuid;
}

pub trait Auditable {
    fn created_at(&self) -> DateTime<Utc>;
    fn created_by(&self) -> &str;
}

pub trait Ownable {
    fn ownership_percentage(&self) -> Option<Decimal>;
}

// Compose capabilities without inheritance
impl Identifiable for NaturalPerson { ... }
impl Auditable for NaturalPerson { ... }
impl Ownable for NaturalPerson { ... }
```

**Go** (interfaces as structural contracts):
```go
type Identifiable interface {
    EntityID() uuid.UUID
}

type Auditable interface {
    CreatedAt() time.Time
    CreatedBy() string
}

// Any struct with the right methods satisfies the interface
// No explicit "implements" declaration
```

The difference is fundamental:

| Aspect | Inheritance (Java) | Composition (Rust/Go) |
|--------|-------------------|----------------------|
| Variation mechanism | Class hierarchy | Trait/interface composition |
| Adding capabilities | Modify hierarchy or use mixins | Implement additional traits |
| Type relationships | "Is-a" (rigid) | "Can-do" (flexible) |
| Breaking changes | Ripple through hierarchy | Isolated to trait impl |
| Compile-time safety | Partial (runtime ClassCastException) | Complete (Rust) / Structural (Go) |

For OB-POC's 15+ entity types with varying capability sets, composition is the natural fit.

---

## Single Stack: The Operational Argument

### The Java Runtime Tax

A typical Spring application requires:

```
┌─────────────────────────────────────────────────────────────────┐
│                         JVM                                      │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                   Spring Framework                        │   │
│   │   ┌─────────────────────────────────────────────────┐   │   │
│   │   │              Spring Boot                          │   │   │
│   │   │   ┌─────────────────────────────────────────┐   │   │   │
│   │   │   │           Hibernate/JPA                   │   │   │   │
│   │   │   │   ┌─────────────────────────────────┐   │   │   │   │
│   │   │   │   │      Your Application Code        │   │   │   │   │
│   │   │   │   └─────────────────────────────────┘   │   │   │   │
│   │   │   └─────────────────────────────────────────┘   │   │   │
│   │   └─────────────────────────────────────────────────┘   │   │
│   └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

Each layer adds:
- Startup latency (component scanning, DI container, connection pools)
- Memory overhead (metaspace, reflection caches, proxy objects)
- Stack trace depth (40+ frames for a simple DB call)
- Debugging complexity (which layer transformed my data?)
- Cognitive load (learn JVM + Spring + Boot + JPA + Hibernate)

### The Rust/Go Single Stack

```
┌─────────────────────────────────────────────────────────────────┐
│                    Compiled Binary                               │
│   Your code + dependencies, statically linked                   │
│   No runtime, no framework, no VM                               │
└─────────────────────────────────────────────────────────────────┘
```

| Metric | Spring Boot | Rust Binary | Go Binary |
|--------|-------------|-------------|-----------|
| Cold start | 3-10 seconds | <50ms | <50ms |
| Memory baseline | 200-500MB | 10-30MB | 10-30MB |
| Container image | 200-400MB | 10-30MB | 10-30MB |
| Stack trace depth | 40+ frames | 5-10 frames | 5-10 frames |
| Dependencies to understand | JVM + 4 frameworks | stdlib + crates | stdlib + modules |

For microservices, serverless, and CLI tools—all relevant to OB-POC—single stack wins.

---

## Agentic Alignment: Why Rust and Go Work Better with AI

This is the pivot point. AI coding assistants have changed the economics of language choice, and some languages are structurally better suited to agentic development.

### The Feedback Loop Problem

AI coding agents work through iteration:

```
Intent → Generate Code → Validate → Fix Errors → Validate → ... → Working Code
```

The quality of this loop depends on:

1. **Speed**: How fast can the agent get feedback?
2. **Signal quality**: How informative are the errors?
3. **Determinism**: Does the same code always produce the same result?
4. **Completeness**: Does passing validation mean the code works?

### Rust: If It Compiles, It Runs

Rust's compiler and Clippy linter provide the tightest feedback loop for AI agents:

**Compiler catches at compile time**:
- Null pointer dereference: Impossible (`Option<T>` must be handled)
- Use after free: Impossible (ownership system)
- Data races: Impossible (borrow checker)
- Unhandled cases: Impossible (exhaustive `match`)
- Type mismatches: Caught with precise error messages

**Clippy provides actionable feedback**:
```
warning: this `if let` can be collapsed into the outer `if let`
  --> src/main.rs:15:9
   |
15 |         if let Some(x) = maybe_x {
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if
   = note: `#[warn(clippy::collapsible_if)]` on by default
help: collapse nested if block
   |
15 |         if let Some(inner) = outer && let Some(x) = maybe_x {
   |
```

**The agentic implication**: An AI agent can generate Rust code, run `cargo clippy`, parse the structured errors, and fix them—often in a single iteration. The compiler is a co-pilot.

**"If it compiles, it runs"** is nearly true for Rust. The class of runtime errors that survive compilation is tiny:
- Logic errors (algorithm wrong)
- Panics (explicit `panic!()` or `unwrap()` on `None`)
- Resource exhaustion (OOM, stack overflow)

Compare to Java, where compilation guarantees almost nothing about runtime behavior (null pointers, class cast exceptions, concurrent modification, resource leaks—all runtime).

### Go: Simplicity as Agentic Advantage

Go takes a different approach: **radical simplicity**.

**One way to do it**:
- No generics (until recently, and limited)
- No inheritance
- No exceptions (explicit error returns)
- No operator overloading
- No implicit conversions
- `gofmt` enforces one code style

**Agentic implications**:

1. **Smaller pattern space**: AI doesn't choose between 5 ways to iterate; there's one way
2. **Predictable structure**: All Go code looks the same; easier to generate correctly
3. **Explicit errors**: `if err != nil` is verbose but unambiguous
4. **Fast feedback**: `go build` is near-instant; `go vet` catches common mistakes

**Fast REPL turnaround**:
```bash
$ time go build ./...
real    0m0.247s   # Sub-second builds for large projects

$ time go test ./...
real    0m1.102s   # Fast test execution
```

An AI agent can iterate rapidly: generate → build → test → fix → repeat. The cycle time is seconds, not minutes.

**Language simplicity aids generation**:
```go
// There's basically one way to write this
func (s *Service) CreateEntity(ctx context.Context, req CreateEntityRequest) (*Entity, error) {
    if err := s.validate(req); err != nil {
        return nil, fmt.Errorf("validation failed: %w", err)
    }
    
    entity := &Entity{
        ID:        uuid.New(),
        Name:      req.Name,
        CreatedAt: time.Now(),
    }
    
    if err := s.repo.Insert(ctx, entity); err != nil {
        return nil, fmt.Errorf("insert failed: %w", err)
    }
    
    return entity, nil
}
```

No annotations, no inheritance decisions, no framework magic. AI generates this correctly because there are few degrees of freedom.

### Java: The Agentic Friction

Java with Spring creates friction for AI agents:

**Multiple valid patterns**:
```java
// Constructor injection or field injection?
@Autowired
private Repository repo;

// vs
private final Repository repo;
public Service(Repository repo) { this.repo = repo; }

// Which exception handling style?
try { ... } catch (Exception e) { throw new ServiceException(e); }
// vs
@ExceptionHandler annotations
// vs
ControllerAdvice
```

AI must choose among patterns. Different choices create inconsistent codebases.

**Runtime behavior invisible to static analysis**:
```java
// This compiles fine but fails at runtime
Object obj = someMap.get("key");  // Could be null
String str = (String) obj;         // ClassCastException if wrong type
str.length();                      // NullPointerException if null
```

The compiler provides weak signal. Tests (which AI must also generate) are required to catch errors.

**Framework magic obscures behavior**:
```java
@Transactional
@Cacheable("entities")
@Retryable(maxAttempts = 3)
public Entity getEntity(UUID id) {
    return repository.findById(id).orElseThrow();
}
```

What does this method actually do? Transaction boundary, cache check, retry logic—all invisible. AI must understand the framework's runtime behavior, not just the code.

**Slow feedback**:
```bash
$ time mvn compile
real    0m12.847s   # Seconds for compilation

$ time mvn test
real    0m45.231s   # Longer for tests
```

Each iteration takes longer. AI agents burn tokens waiting for feedback.

### The Comparison Matrix

| Factor | Rust | Go | Java/Spring |
|--------|------|----|-----------| 
| Compile-time error detection | Exceptional | Good | Weak |
| Error message quality | Excellent + suggestions | Good | Variable |
| "If it compiles, it runs" | Nearly true | Mostly true | Rarely true |
| Build speed | Good (incremental) | Excellent | Poor |
| Pattern consistency | High (idioms enforced) | Very high (gofmt) | Low (many valid patterns) |
| Runtime surprises | Rare | Uncommon | Common |
| Framework magic | None | Minimal | Extensive |
| Agent iteration speed | Fast | Very fast | Slow |

**For agentic development, the ranking is clear**: Go > Rust > Java.

Go's radical simplicity makes it the easiest target for AI generation. Rust's strict compiler makes it the safest. Java's flexibility and framework complexity make it the hardest to generate correctly.

---

## The OB-POC Language Choices

Given this analysis, OB-POC uses:

### Rust for the DSL Core

- **Parser and linter**: Correctness is paramount; Rust's type system prevents bugs
- **Executor**: Performance matters for batch processing; zero-cost abstractions
- **WASM compilation**: Single codebase runs in browser and server
- **AI alignment**: Compiler feedback enables tight generation loop

### Go for Orchestration Services

- **Microservices**: Fast startup, small footprint, simple deployment
- **API layer**: Quick iteration, straightforward HTTP handling
- **AI alignment**: Simple patterns, fast builds, predictable generation

### Not Java Because

- **Impedance mismatch**: Class-centric paradigm vs operation-centric domain
- **Framework overhead**: Spring complexity without corresponding benefit
- **Agentic friction**: Slow feedback, weak compile-time guarantees, pattern ambiguity
- **Operational cost**: JVM resources, startup time, container size

---

## The Honest Counter-Arguments

### Team Reality

If the team has deep Java expertise and no Rust/Go experience, retraining has real cost. AI assistance flattens the learning curve but doesn't eliminate it.

### Integration Ecosystem

If extensive integration with existing Java services is required, staying on JVM reduces friction. Crossing runtime boundaries has overhead.

### Enterprise Governance

Some organisations mandate Java. Fighting that mandate may cost more than the technical benefits justify.

### Maturity

The Java ecosystem is more mature. Library stability, long-term support, and tooling depth favor Java for risk-averse organisations.

---

## Conclusion: Structural Alignment

The argument is not that Java is bad. It's that:

1. **The class-centric paradigm creates impedance mismatch** with operation-centric domains
2. **Composition (traits/interfaces) fits better than inheritance** for capability-based entity models
3. **Single stack deployment** eliminates framework overhead
4. **Rust's compiler and Go's simplicity align with agentic development**—the feedback loop is tighter, the error signal is stronger, the iteration is faster

For a greenfield system in 2024+ that will be extended and maintained with AI assistance, language choice should optimise for:

- **Compile-time correctness** (Rust wins)
- **Generation simplicity** (Go wins)
- **Paradigm fit** (both beat Java for this domain)
- **Operational efficiency** (both beat JVM)

The choice of Rust and Go isn't preference—it's structural alignment between tools and problem.

---

## Appendix: The Feedback Loop in Practice

### Rust Agent Iteration

```
Agent: Generate function to parse entity type from string

Attempt 1:
fn parse_entity_type(s: &str) -> EntityType {
    match s {
        "NATURAL_PERSON" => EntityType::NaturalPerson,
        "LEGAL_ENTITY" => EntityType::LegalEntity,
        _ => EntityType::Unknown,
    }
}

Compiler:
error[E0004]: non-exhaustive patterns: `"TRUST"` not covered
  --> src/lib.rs:15:11
   |
15 |     match s {
   |           ^ pattern `"TRUST"` not covered

Agent fixes, recompiles: ✓

Clippy:
warning: this function could return `Option<EntityType>` instead of using a catch-all
help: consider using `Option` for the unknown case

Agent fixes, recompiles: ✓

Result: Correct, idiomatic code in 2 iterations
```

### Go Agent Iteration

```
Agent: Generate function to create entity

Attempt 1:
func CreateEntity(name string) *Entity {
    return &Entity{
        ID:   uuid.New(),
        Name: name,
    }
}

go vet: no issues
go build: success (0.2s)

Reviewer (human or AI): Should validate name, return error

Attempt 2:
func CreateEntity(name string) (*Entity, error) {
    if name == "" {
        return nil, errors.New("name is required")
    }
    return &Entity{
        ID:   uuid.New(),
        Name: name,
    }, nil
}

Result: Correct, idiomatic code in 2 iterations, sub-second builds
```

### Java Agent Iteration

```
Agent: Generate service to create entity

Attempt 1:
@Service
public class EntityService {
    @Autowired
    private EntityRepository repository;
    
    public Entity createEntity(String name) {
        Entity entity = new Entity();
        entity.setName(name);
        return repository.save(entity);
    }
}

Compiles: ✓ (12 seconds)

Runtime: NullPointerException - repository not injected in test context

Attempt 2: Add @SpringBootTest, @MockBean...

Compiles: ✓
Tests: TransactionRequiredException - no transaction

Attempt 3: Add @Transactional...

Compiles: ✓
Tests: ConstraintViolationException - name validation

Attempt 4: Add @Valid, validation annotations...

Result: Working code after 4+ iterations, minutes of build time
```

The feedback quality and iteration speed compound over a project's lifetime.
