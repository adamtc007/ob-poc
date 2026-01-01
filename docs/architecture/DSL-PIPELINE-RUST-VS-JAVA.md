# DSL Pipeline Implementation: Rust vs Java/Spring

**Date:** 2026-01-01  
**Context:** Evaluating whether the ob-poc DSL pipeline (parser → AST → validation → compilation → execution) could be implemented in Java/Spring, and the relative costs.

---

## Executive Summary

| Dimension | Rust | Java/Spring | Winner |
|-----------|------|-------------|--------|
| Initial dev (AI-assisted) | 11 days | 19 days | **Rust** |
| AI feedback loop | Tight (compiler) | Loose (runtime) | **Rust** |
| Prod stability | High | Medium | **Rust** |
| Memory footprint | 20-50MB | 200-500MB | **Rust** |
| Startup time | 50ms | 5-15s | **Rust** |
| Refactor safety | Compiler-enforced | Test-dependent | **Rust** |
| Refactor cost | 1x | 1.5-2x | **Rust** |
| Talent availability | Low | High | **Java** |
| Enterprise familiarity | Low | High | **Java** |

**Bottom line:** For a DSL in a financial compliance system, Rust wins decisively on correctness, maintainability, and AI-assisted development velocity.

---

## The Rust DSL Pipeline

```
Source Text
    │
    ▼
┌─────────────────┐
│ nom Combinators │  Functional parser composition
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ AST (enums)     │  Algebraic data types + pattern matching
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ CSG Linter      │  Multi-pass validation, exhaustive match
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Execution Plan  │  Topo sort, dependency DAG
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Executor        │  Trait dispatch to handlers
└─────────────────┘
```

---

## Component-by-Component Comparison

### 1. Parser: nom vs ANTLR

| Rust (nom) | Java Options | Mismatch |
|------------|--------------|----------|
| `fn verb_call(i: &str) -> IResult<&str, VerbCall>` | ANTLR (grammar files) | Different paradigm entirely |
| Combinator composition | JParsec | Less mature, awkward syntax |
| `alt((tag("cbu"), tag("entity")))` | Hand-rolled recursive descent | Tedious, error-prone |

**nom example:**
```rust
fn verb_call(input: &str) -> IResult<&str, VerbCall> {
    let (input, _) = char('(')(input)?;
    let (input, domain) = identifier(input)?;
    let (input, _) = char('.')(input)?;
    let (input, verb) = identifier(input)?;
    let (input, args) = many0(argument)(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, VerbCall { domain, verb, args }))
}
```

**Java ANTLR equivalent:**
```
// grammar Dsl.g4 (separate file)
verbCall : '(' IDENTIFIER '.' IDENTIFIER argument* ')' ;
argument : ':' IDENTIFIER value ;
value : STRING | NUMBER | symbol ;

// Then generate Java code, implement visitor:
public class DslVisitor extends DslBaseVisitor<AstNode> {
    @Override
    public AstNode visitVerbCall(DslParser.VerbCallContext ctx) {
        String domain = ctx.IDENTIFIER(0).getText();
        String verb = ctx.IDENTIFIER(1).getText();
        List<Argument> args = ctx.argument().stream()
            .map(this::visitArgument)
            .collect(Collectors.toList());
        return new VerbCall(domain, verb, args);
    }
}
```

**Verdict:** ANTLR works but it's a different mental model. Grammar files → generated code → visitor pattern. Not composable at runtime. **HIGH mismatch**.

---

### 2. AST: Rust Enums vs Java Sealed Classes

**Rust:**
```rust
pub enum AstNode {
    VerbCall(VerbCall),
    Literal(Literal),
    Symbol(String),
    List(Vec<AstNode>),
}

pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Uuid(Uuid),
}

// Pattern matching - exhaustive
match node {
    AstNode::VerbCall(vc) => execute_verb(vc),
    AstNode::Literal(Literal::String(s)) => handle_string(s),
    AstNode::Symbol(name) => resolve_symbol(name),
    AstNode::List(items) => handle_list(items),
    // Compile error if you miss one!
}
```

**Java 21+ (sealed classes):**
```java
public sealed interface AstNode 
    permits VerbCallNode, LiteralNode, SymbolNode, ListNode {}

public record VerbCallNode(String domain, String verb, List<Argument> args) 
    implements AstNode {}

public sealed interface LiteralNode extends AstNode 
    permits StringLiteral, IntLiteral, FloatLiteral, BoolLiteral, UuidLiteral {}

public record StringLiteral(String value) implements LiteralNode {}
// ... etc

// Pattern matching (Java 21+)
switch (node) {
    case VerbCallNode vc -> executeVerb(vc);
    case StringLiteral s -> handleString(s.value());
    case SymbolNode sym -> resolveSymbol(sym.name());
    default -> throw new IllegalArgumentException();  // Required!
}
```

**Verdict:** Java 21 sealed classes + pattern matching is *close*, but:
- More boilerplate (separate record for each variant)
- No nested pattern matching (`Literal::String(s)` in one arm)
- **Not exhaustive by default** (need `default` arm)

**MEDIUM mismatch**.

---

### 3. Validation Passes

**Rust:**
```rust
impl CsgLinter {
    pub fn lint(&self, ast: &Program) -> LintResult {
        let mut diagnostics = Vec::new();
        
        self.analyze_symbols(ast, &mut diagnostics);
        self.validate_references(ast, &mut diagnostics);
        self.validate_applicability(ast, &mut diagnostics);
        
        LintResult { ast, diagnostics }
    }
}
```

**Java Spring:**
```java
@Service
public class CsgLinter {
    
    @Autowired
    private SymbolAnalyzer symbolAnalyzer;
    
    @Autowired
    private ReferenceValidator referenceValidator;
    
    @Autowired
    private ApplicabilityValidator applicabilityValidator;
    
    public LintResult lint(Program ast) {
        List<Diagnostic> diagnostics = new ArrayList<>();
        
        symbolAnalyzer.analyze(ast, diagnostics);
        referenceValidator.validate(ast, diagnostics);
        applicabilityValidator.validate(ast, diagnostics);
        
        return new LintResult(ast, diagnostics);
    }
}
```

**Verdict:** This maps okay to Spring. But:
- Rust's `&mut diagnostics` is zero-copy
- Java passes mutable list (GC overhead)
- Spring wants `@Autowired` everything

**LOW mismatch**.

---

### 4. Execution Plan / DAG

**Rust:**
```rust
pub fn compile(ast: &Program) -> Result<ExecutionPlan, CompileError> {
    let verb_calls = extract_verb_calls(ast);
    let dependencies = build_dependency_graph(&verb_calls);
    let sorted = topo_sort(&dependencies)?;
    
    Ok(ExecutionPlan {
        steps: sorted.into_iter().map(|i| to_step(&verb_calls[i])).collect()
    })
}
```

**Java:**
```java
@Service
public class ExecutionPlanCompiler {
    
    public ExecutionPlan compile(Program ast) throws CompileException {
        List<VerbCall> verbCalls = extractVerbCalls(ast);
        Map<Integer, Set<Integer>> dependencies = buildDependencyGraph(verbCalls);
        List<Integer> sorted = topoSort(dependencies);
        
        return new ExecutionPlan(
            sorted.stream()
                .map(i -> toStep(verbCalls.get(i)))
                .collect(Collectors.toList())
        );
    }
}
```

**Verdict:** Graph algorithms don't care about the language. **NO mismatch**.

---

### 5. Custom Operations: Trait vs Interface

**Rust:**
```rust
#[async_trait]
pub trait CustomOperation: Send + Sync {
    fn domain(&self) -> &'static str;
    fn verb(&self) -> &'static str;
    
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult>;
}

// Registration - compile time
let op = registry.get(&format!("{}.{}", domain, verb));
op.execute(verb_call, ctx, pool).await?
```

**Java Spring:**
```java
public interface CustomOperation {
    String domain();
    String verb();
    
    CompletableFuture<ExecutionResult> execute(
        VerbCall verbCall,
        ExecutionContext ctx,
        DataSource dataSource
    );
}

@Component
public class GleifEnrichOp implements CustomOperation {
    @Override public String domain() { return "gleif"; }
    @Override public String verb() { return "enrich"; }
    
    @Override
    public CompletableFuture<ExecutionResult> execute(...) {
        // implementation
    }
}

@Service
public class OperationRegistry {
    private final Map<String, CustomOperation> ops;
    
    @Autowired
    public OperationRegistry(List<CustomOperation> allOps) {
        this.ops = allOps.stream()
            .collect(Collectors.toMap(
                op -> op.domain() + "." + op.verb(),
                Function.identity()
            ));
    }
}
```

**Verdict:** Maps well to Spring. `@Component` discovery + injection. But:
- More annotation ceremony
- Runtime discovery vs compile-time registration
- Spring's proxy magic can hide behavior

**LOW mismatch**.

---

### 6. Error Handling: Result<T, E> vs Exceptions

**Rust:**
```rust
fn parse_and_validate(input: &str) -> Result<Program, DslError> {
    let ast = parse(input)?;           // Early return on error
    let validated = lint(&ast)?;       // Early return on error
    Ok(validated)
}
```

**Java options:**
```java
// Option A: Checked exceptions (traditional)
public Program parseAndValidate(String input) throws DslException {
    Program ast = parse(input);        // throws ParseException
    Program validated = lint(ast);     // throws ValidationException
    return validated;
}

// Option B: Result type (needs Vavr library)
public Result<Program, DslError> parseAndValidate(String input) {
    return parse(input)
        .flatMap(this::lint);
}

// Option C: Optional (loses error info)
public Optional<Program> parseAndValidate(String input) {
    try {
        return Optional.of(lint(parse(input)));
    } catch (Exception e) {
        return Optional.empty();
    }
}
```

**Verdict:** Java's error handling is messy:
- Checked exceptions require try-catch everywhere
- `Result<T, E>` isn't in standard library
- `Optional` loses error information
- Rust's `?` operator has no Java equivalent

**HIGH mismatch**.

---

## The Real Killers

### 1. Parser Combinators Don't Exist in Java (Ergonomically)

nom's power is composition:
```rust
let parser = delimited(
    char('('),
    tuple((domain, char('.'), verb, many0(arg))),
    char(')')
);
```

Java has nothing this clean. You either:
- Use ANTLR (grammar files, code generation, visitor pattern)
- Hand-write recursive descent (tedious)
- Use JParsec (obscure, not idiomatic)

### 2. Exhaustive Pattern Matching

Rust compiler **enforces** you handle all cases:
```rust
match node {
    AstNode::VerbCall(vc) => ...,
    AstNode::Literal(lit) => ...,
    AstNode::Symbol(s) => ...,
    AstNode::List(items) => ...,
    // Compile error if you miss one!
}
```

Java (even 21+) doesn't enforce exhaustiveness without explicit `default`.

### 3. Ownership / Mutability Control

Rust prevents accidental mutation:
```rust
fn analyze(&self, ast: &Program, diagnostics: &mut Vec<Diagnostic>)
//         ^^^^^ immutable         ^^^^ mutable - explicit
```

Java has no equivalent. Everything is mutable unless you use `final` (which only prevents reassignment).

### 4. Compiler Feedback Loop

**Rust errors are immediate and precise:**
```
error[E0308]: mismatched types
  --> src/dsl/parser.rs:45:12
   |
45 |     return "hello";
   |            ^^^^^^^ expected `VerbCall`, found `&str`
```

**Spring errors are often runtime:**
```
org.springframework.beans.factory.BeanCreationException: 
Error creating bean with name 'gleifEnrichOp': 
Injection of autowired dependencies failed...
```

---

## Quantified Mismatch

| Component | Rust | Java/Spring | Code Ratio | Mismatch |
|-----------|------|-------------|------------|----------|
| Parser | nom combinators | ANTLR + visitor | 1:3 | **HIGH** |
| AST types | enums + match | sealed + switch | 1:2 | MEDIUM |
| Validation | multi-pass | @Service injection | 1:1.5 | LOW |
| DAG/Topo sort | same algorithm | same algorithm | 1:1 | NONE |
| Custom ops | traits | interfaces + @Component | 1:1.5 | LOW |
| Error handling | Result + ? | exceptions/Optional | 1:2 | **HIGH** |
| Async | tokio | CompletableFuture | 1:1.5 | MEDIUM |
| **Overall** | | | **1:2+** | |

---

## Initial Implementation Effort (AI-Assisted)

| Phase | Rust | Java/Spring | Notes |
|-------|------|-------------|-------|
| Parser | **2 days** | 5 days | nom composes naturally; ANTLR needs grammar + visitor + wiring |
| AST types | **1 day** | 2 days | Rust enums are terse; Java needs record per variant |
| Validation passes | 2 days | 2 days | Similar - both iterate AST |
| Execution plan/DAG | 2 days | 2 days | Algorithm is language-agnostic |
| Custom op handlers | 3 days | 3 days | Similar interface/trait pattern |
| Error handling | **1 day** | 3 days | Result + ? vs exception plumbing |
| Wiring/DI | **0.5 days** | 2 days | Direct construction vs annotations/config |
| **Total** | **~11 days** | **~19 days** | ~1.7x more effort for Java |

---

## AI-Assisted Development Factor

| Aspect | Rust + AI | Java/Spring + AI |
|--------|-----------|------------------|
| Compiler feedback quality | Excellent - AI reads error, fixes it | Poor - runtime errors, stack traces |
| Pattern ambiguity | Low - one way to do things | High - @Service or @Component? Constructor or field injection? |
| Boilerplate generation | Minimal | Heavy annotations, config |
| Refactor confidence | High - compiler catches breakage | Low - tests must catch it |

**Real example from ob-poc:**
- Claude built the nom parser + AST in one session
- Compiler errors guided each fix
- No "it compiles but fails at runtime" surprises

**Java/Spring with AI would:**
1. Generate ANTLR grammar
2. Generate visitor
3. Wire visitor as @Component
4. Debug Spring context loading errors
5. Debug runtime ClassCastExceptions
6. Debug Jackson serialization issues
7. Debug async context propagation

**The Rust compiler is a co-pilot for the AI.** It catches what the AI misses. Java's compiler is permissive - it lets mistakes through to runtime.

---

## Production Stability

| Dimension | Rust | Java/Spring |
|-----------|------|-------------|
| Memory safety | **Guaranteed at compile time** | GC handles it (usually) |
| Null pointer | **Impossible** (Option<T>) | NPE is #1 runtime error |
| Thread safety | **Compile-time enforced** | Hope you got synchronization right |
| Startup time | **50ms** | 5-15 seconds |
| Memory footprint | **20-50MB** | 200-500MB |
| Crash behavior | Process dies cleanly | JVM can hang on OOM |
| Dependency hell | Cargo.lock deterministic | Maven/Gradle conflicts |
| CVE surface area | Small (fewer deps) | Large (Spring + transitives) |

### Failure Modes

| Scenario | Rust | Java/Spring |
|----------|------|-------------|
| Unhandled enum variant | **Won't compile** | Runtime ClassCastException |
| Forgotten null check | **Won't compile** (Option) | Runtime NPE |
| Race condition | **Won't compile** (Send/Sync) | Intermittent prod failures |
| Missing config | **Won't compile** (if designed right) | Runtime "bean not found" |
| Memory leak | Rare (ownership) | Common (accidental references) |

### Production Gut Feel

```
Rust:  If it compiles, it probably works. Crashes are rare and loud.

Spring: It compiles. Tests pass. Works on my machine. 
        Prod fails at 3am with cryptic stack trace.
```

---

## Refactoring Relative Costs

| Refactor Type | Rust | Java/Spring |
|---------------|------|-------------|
| Rename field | **Compiler finds all usages** | IDE helps, but runtime reflection breaks |
| Add AST variant | **Compiler shows every match to update** | Find usages, hope you got them all |
| Change function signature | **Compiler guides you** | IDE helps, but tests must validate |
| Split/merge modules | Straightforward | @ComponentScan, package boundaries, context issues |
| Change DB schema | Same | Same |

### Real Refactoring Scenario

"Add a new AstNode variant for GLEIF discovery result"

**Rust:**
```rust
// 1. Add variant
pub enum AstNode {
    // ... existing
    GleifDiscovery(GroupDiscoveryResult),  // NEW
}

// 2. Compile
error[E0004]: non-exhaustive patterns: `GleifDiscovery(_)` not covered
  --> src/dsl/executor.rs:45:11
   |
45 |     match node {
   |           ^^^^ pattern `GleifDiscovery(_)` not covered

// 3. Fix each location compiler tells you
// 4. Compile again - done
```

**Java:**
```java
// 1. Add class
public record GleifDiscoveryNode(GroupDiscoveryResult result) 
    implements AstNode {}

// 2. Add to sealed interface permits
public sealed interface AstNode 
    permits VerbCallNode, LiteralNode, SymbolNode, ListNode, 
            GleifDiscoveryNode {}

// 3. Compile - no errors! (switch has default)

// 4. Run tests - some fail (good)

// 5. Run in prod - ClassCastException in edge case 
//    you didn't have a test for

// 6. Debug at 3am
```

### Refactoring Cost Multiplier

| Scope | Rust | Java/Spring |
|-------|------|-------------|
| Local (one file) | 1x | 1x |
| Module (one crate) | 1x | 1.5x |
| Cross-module | 1x | 2x |
| With reflection/annotations | N/A | 3x |
| With Spring proxies | N/A | 4x |

---

## When Java/Spring Makes Sense

Despite the above, Java/Spring is the right choice if:

1. **Team only knows Java** - Learning Rust has a 2-3 month ramp
2. **Must integrate with existing Spring monolith** - Fighting the architecture is worse
3. **Enterprise politics require "standard" stack** - Sometimes you can't choose
4. **Hiring constraints** - Rust devs are 10x harder to find

---

## When Rust Makes Sense

1. **Correctness matters** - Financial systems, compliance ✓
2. **Small team, high leverage** - 2 devs doing what 6 Java devs would
3. **AI-assisted development** - Compiler feedback loop is gold
4. **Performance/footprint matter** - 10x memory savings, instant startup
5. **Long-term maintenance** - Compiler is your documentation

---

## Gut Numbers

| Metric | Rust | Java/Spring |
|--------|------|-------------|
| Initial build | 2 weeks | 3.5 weeks |
| First prod incident | Month 3+ | Week 2 |
| Refactor confidence | 95% | 70% |
| Sleep quality on deploy | Good | Check phone at 2am |
| 12-month maintenance cost | 1x | 2x |

---

## Conclusion

For the ob-poc DSL pipeline specifically:

| Aspect | Verdict |
|--------|---------|
| Build speed | Rust 1.7x faster |
| Prod reliability | Rust significantly better |
| Refactoring | Rust 1.5-2x cheaper |
| Long-term maintenance | Rust wins (compiler as documentation) |

**The AI development angle is underrated:**

With Rust, the workflow is:
```
Human: "Add GLEIF discovery to AST"
AI: *adds enum variant*
Compiler: "Here are the 12 places you need to handle it"
AI: *fixes all 12*
Human: "Ship it"
```

With Java/Spring:
```
Human: "Add GLEIF discovery to AST"
AI: *adds record + sealed permits*
Compiler: "Looks good!"
Human: "Ship it"
*Prod fails on edge case*
Human: "What happened?"
AI: "The switch default case silently swallowed it"
```

For a DSL in a financial compliance system? **Rust. Not close.**

---

## Related Documents

- [WHY-NOT-SPRING-JPA.md](./WHY-NOT-SPRING-JPA.md) - Broader Spring/JPA comparison
- [WHY-DSL-PLUS-AGENT.md](./WHY-DSL-PLUS-AGENT.md) - Why DSL + AI agent architecture
- [WHY-NOT-BPMN.md](./WHY-NOT-BPMN.md) - Why not workflow engines
