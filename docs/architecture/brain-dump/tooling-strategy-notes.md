# Tooling & Implementation Strategy — Working Notes

> Personal working notes. Not for circulation.
> Companion thinking to the Onboarding Approach Paper, deliberately kept separate.
> Last revised: April 2026

---

## Why this is a separate document

The Approach Paper makes the data-architecture case on its own merits. The tooling case — specifically, why class-oriented persistence (JPA/Hibernate/Spring Data) is the wrong tool for the canonical model — is correct but politically costly to lead with at BNY, where Java/Spring is the default stack and most of the architecture community has built their careers on it.

These notes are for me. They exist so that when the tooling question comes up in review — and it will — I have a clear, pre-thought answer that I can deploy at the level of detail the conversation warrants, rather than improvising under pressure or sounding doctrinaire.

The tactical posture in public discussion is: *the modelling stance excludes patterns that require the model to be reshaped to fit the tool; within that constraint, technology choice is open*. That sentence is true and it is what §2 of the Approach Paper says. These notes are the unstated reasoning underneath it.

---

## Operating summary

A navigation aid for the rest of the document. The four positions I should always have ready, in order of how they should be deployed.

**Private conclusion.** Rust is non-negotiable for the canonical model layer. Java/Spring is correct for the systems of execution. Go is acceptable for the integration layer. The split is not a language preference — it is a categorisation of three different kinds of software with three different tooling needs.

**Public posture.** The implementation stack must satisfy the canonical model's constraints (bitemporal records, governed identity, declarative gates, content-addressed self-versioning, multi-taxonomy support). Within those constraints, technology choice is open. JPA/Spring remains valid for systems of execution; the canonical model layer is a different kind of object and is treated differently. *Tooling fits the model, not the reverse.*

**Core proof.** JPA handles atomic entities well. JPA does not have a representation for *structured entities* (CBUs, UBO structures, operating arrangements — entities whose identity persists across compositional change). The gating DAG is not an entity model at all; it is a Terraform-shaped runtime constraint graph over state machines. The canonical model's interesting work lives in structured entities and the gating DAG. Therefore JPA is wrong for the canonical model layer, even though it remains right for systems of execution. *The exclusion is layer-specific, not blanket.*

**Decision rule.** The canonical model layer is a systems-style runtime, not an application service. Systems-style runtimes have different tooling requirements than application services (performance as correctness, predictability over peak speed, testability approaching its theoretical ceiling, explicit resource and failure semantics, configuration-over-code architecture). Rust meets these requirements by language design; Java 25 can meet them only by rejecting the Spring/JPA ecosystem entirely. *The choice between mandatory discipline (Rust) and voluntary discipline (Java 25 sans Spring) is a risk-management decision, not an engineering preference.*

The remaining sections expand each of these and provide pre-thought answers to the counter-arguments that follow.

---

## The core argument, expanded

Class-oriented persistence is a single-taxonomy, operations-first paradigm. It assumes one privileged decomposition (the class hierarchy), encodes attributes and relationships statically in code (requiring deployment to change), and locates behaviour inside aggregates (where cross-domain side effects are implicit and audit trails are reconstructed from call graphs). The canonical model is multi-taxonomy, data-first, dynamically extensible, and explicit-transition by design. Every property of the framework is at odds with every property of the model. This is not a tooling complaint; it is a category mismatch.

Everything below is illustration of that one paragraph.

---

## The category mismatch, expanded for my own clarity

### Stance — data-first vs operations-first

The canonical model treats identity and governed taxonomy structure (NOM, DAG) as primary, with operations as derived views. The class-oriented stance is the inverse — methods on classes are primary, data is what those methods hold and act on.

The consequence in a multi-taxonomy domain: the same data legitimately wears multiple personas across sub-domains. A CBU is operationally an entity for Operations, a subject for KYC, a node in the legal-entity hierarchy for Legal, a document context for Compliance, an instrument-matrix bearer for the trading and post-trade lines. A class structure cannot represent that without picking one persona as the "real" CBU and demoting the others to attributes attached to it. NOM doesn't pick — identity is primary, taxonomies are governed views over identity.

### Extensibility — dynamic vs hard-coded

The dynamically-built NOM admits new contexts, new attributes, and new relationships as governed registry changes. Class-defined structures admit the same changes only as code changes. Class definitions are statically-bound contracts: every attribute is hard-coded, every relationship is a hard-coded reference, every taxonomy participation is a hard-coded inheritance or composition decision.

Adding the legal-entity hierarchy slice (paper §13.6) in v2 is, in NOM, a registry change — new taxonomy, new relationships, identity layer untouched. In JPA it is a schema migration plus a class migration plus a deployment coordination across every sub-domain that touches the affected entities. The first ships when governance approves it. The second ships when seven teams have agreed on the schema, the migration order, and the cutover window.

### Side effects and audit

In a class-oriented model, multiple sub-domains mutate the same object through the object's methods. Two outcomes follow, both bad:

- **God-object** — the CBU class accretes methods serving every sub-domain. Cross-domain side effects become impossible to contain because every sub-domain's logic runs inside the same class. KYC's mutation can corrupt operational invariants; operational mutation can break document-context assertions. The class is the seam, and the seam holds everyone's hand.
- **Bypass-the-aggregate** — sub-domains tire of the god-class and mutate via repositories or native SQL. The aggregate's invariants are no longer enforced anywhere; they exist as comments on a class nobody reads.

Audit inherits this. When behaviour is distributed across class methods called from N entry points, "what changed and why" is reconstructed by reading method-call graphs across the codebase. When sub-domains bypass the aggregate, audit is reconstructed from database triggers and Envers tables that don't agree with each other.

The data-first stance solves both with one mechanism: every change is a typed transition recorded against the canonical model, with explicit source state, target state, evidence, and authority. Side effects become first-class data. The audit trail isn't reconstructed; it *is* the persistence layer.

This is the argument I keep nearly making and not landing. **Side-effect containment** and **audit reconstruction** are different problems with the same root cause. State them separately when I make the argument; mixing them makes each sound weaker than it is.

---

## The six specific frictions, ordered by severity

These are the concrete failure modes, all derived from the category mismatch above. Useful as ammunition in detail discussions; not useful as the lead argument.

### 1. `@Entity` is class-oriented by construction

JPA's foundational assumption is that data is a property of an object — `@Entity` is the unit of identity, persistence, and behaviour. This is the exact stance §2 of the paper excludes. Inheritance strategies (`SINGLE_TABLE`, `JOINED`, `TABLE_PER_CLASS`) are all single-tree mechanisms. A CBU appears in seven taxonomies; JPA can express one. The other six become foreign keys hanging off the chosen primary, and which one is primary is an arbitrary choice that corrupts the others.

The likely Spring-shop counter: *"we'll use `@SecondaryTable`, `@MappedSuperclass`, `@DiscriminatorColumn`, `@Inheritance` to express the taxonomies."* Counter-counter: those are mechanisms for expressing variation **within a single taxonomy**. They do not let you add a new taxonomy without modifying the primary `@Entity`. Multi-taxonomy requires the primary class to be **absent** — identity is primary, taxonomies are governed views — and no JPA mechanism gives you that.

### 2. Bitemporal data is a hard mismatch

JPA has no native bitemporal model. The two clocks (`valid_from/valid_to` and `recorded_at`) are not entities; they are properties of every fact in the model. Three problems:

- Hibernate Envers is an audit log, which is one clock. The model needs both clocks queryable as data, and corrections recorded as new tuples without overwriting history. Envers cannot answer the regulator's question: *"what did we believe to be true on March 14, given that on March 18 we discovered the KYC clearance was actually granted on March 12?"*
- Every read becomes a two-clock query. `entityManager.find(CBU.class, id)` returns "the current row." There is no "as-of valid_at=T1, recorded_at=T2" overload. JPQL predicates can shim it but the result bypasses Spring Data's repository abstractions.
- Optimistic locking via `@Version` is row-level concurrency, not temporal validity. Conflating them — which inexperienced teams do — is a class of subtle correctness bug I have seen in production.

The paper is explicit that bitemporal audit is non-negotiable. Hibernate gives a passable audit log and nothing closer. Anything beyond is custom — at which point most of the framework's value evaporates for the most safety-critical queries in the model.

### 3. Cross-layer gates are not state machines

Spring State Machine handles one state machine per aggregate. The model has gates *between* state machines: KYC clears → Deal can transition; Service active → consumption can provision; live binding on active resource → consumption can activate. These are predicates over state in *other* aggregates that govern transitions in *this* one.

Three implementation paths under Spring, all bad:

- **Transition listeners** — cross-layer dependency buried in `@OnTransition` methods scattered across services. The §8 property that "what blocks what" is queryable as data is lost. A new gate is a development cycle, not a registry change.
- **`@Service` methods called before `save()`** — same problem, distributed. Audit reconstruction now requires reading commit history.
- **Persist gates as data, write a runtime to evaluate them** — this is what the model requires, but the runtime sits outside JPA/Spring. The framework provides only CRUD plumbing. Most teams who go this route end up dropping JPA on the gate-evaluation paths and keeping it elsewhere — which produces friction #6.

### 4. Identity resolution doesn't fit `@Id`

JPA assumes `@Id` is the identity. The model needs identity that can be:

- **Provisional** pending KYC verification — rows that exist but whose identity link is not yet asserted.
- **Superseded** without losing the historical reference — a CBU merged into another during a fund restructure.
- **Disputed and corrected bitemporally** — we thought entity X was the same as entity Y; on March 18 evidence emerged they're different.

None of this fits the JPA `@Id` model. A separate `entity_link` table maintained outside Hibernate's session is the usual compromise — at which point Hibernate's notion of identity (the persistence-context entity) and the model's notion of identity (the governed link) diverge. Lazy-loading and `equals()` semantics break under that divergence in ways that take six months to surface and are agonising to debug.

### 5. Schema migration is not model self-versioning

Flyway/Liquibase do forward-only DDL. The model needs:

- Content-addressed identifiers for gates and states (so two definitions are equal iff their canonical serialisations hash identically).
- Replay-under-schema-evolution semantics (events whose `valid_at` predates a new gate are replayed with the *old* gate set).
- In-flight transaction handling under schema change.

This is fundamentally different from `ALTER TABLE ADD COLUMN`. JPA/Hibernate doesn't have a hook for it; it's not even a category of problem the framework recognises. You'd build a parallel system that records the model version each transition was recorded at and routes replay through the correct version's gate evaluator. Spring offers nothing here.

### 6. The compromise path — JPA for the easy parts, JDBC for the rest

The most common route teams take when they hit 1–5 is: keep JPA for the CRUD-shaped parts and drop to JDBC, jOOQ, or MyBatis for the parts JPA can't model. This produces a split codebase with two models of identity (the persistence-context entity and the SQL row), two models of state (the dirty-checked entity and the explicit transition record), and two models of correctness (constraint-validated entity and gate-evaluated transition).

They will drift. The boundary between them — when does an operation cross from the JPA half to the JDBC half? — is a constant source of bugs that surface as audit failures or projection inconsistencies, usually under regulatory inquiry. I have seen this pattern several times in custody and FA programmes. It does not stabilise.

---

## What I'm actually proposing as the implementation stack

The framing that justifies the choice: **the canonical model layer is systems programming. The integration layer is conventional service-tier work. The systems of execution are application programming.** Three different categories of software with three different tooling needs. The split below derives from that categorisation, not from language preference.

The canonical model layer needs:

- A typed, declarative state-transition runtime where gates are persisted data, not code.
- Bitemporal records as primary data, not audit log.
- Content-addressed serialisation for governed definitions (gates, states, taxonomies) with stable hashing across deployments.
- Identity as a governed object distinct from primary keys.
- Multi-taxonomy support without a privileged class hierarchy.
- A persistence layer that doesn't fight any of the above.
- Performance and predictability as correctness properties, not optimisation goals.
- Test coverage that approaches the limits of what testing can demonstrate.
- Configuration-over-code architecture: generic core, dense logic, new domains as configuration rather than new code.

These are systems-programming requirements, and Rust delivers all of them naturally. Strong types for transitions. Algebraic data types for state machines. Serde + bincode + BTreeMap for canonical serialisation. SQLx for compile-time-checked SQL without forcing an ORM stance. PostgreSQL as the primary store, with bitemporal records as a first-class table design rather than an audit-log addon. The compiler sees the whole codebase; test coverage corresponds to actual program coverage; property-based testing is well-integrated. The Linux/Cloudflare/Microsoft/AWS adoptions settle the maturity question.

Go is acceptable for the integration layer (write-contract endpoints, projection materialisation, the read API for downstream consumers) where the simpler concurrency model and faster build cycle matter more than the type-system precision the canonical model itself needs. Go is also a systems-programming language; for the integration tier, its tradeoffs (predictable GC, simpler type system, runtime-enforced concurrency safety) are acceptable.

Java is acceptable — and probably correct — for the systems of execution that consume the canonical model (custody, FA, TA, KYC casework). Those are application programming: CRUD-shaped, transactional, mostly atomic-entity work, where framework runtime convenience pays back in development speed. JPA is built for them. They should keep using it. *This is not a concession; it is the honest categorisation.*

The honest split is therefore:
- **Canonical model layer** — Rust. Justified on systems-programming grounds, not language preference. Non-negotiable for correctness reasons.
- **Integration layer** — Rust or Go. Either works; choose by team capability.
- **Systems of execution** — keep what they have. Java/Spring is the right tool for application-programming work, and these systems are application-programming work.

This is what I would say in a private architecture conversation. It is not what I would lead a public review with.

---

## AI-collaborative development — the productivity gap I keep seeing

This belongs in personal notes rather than the public paper because it is too easily mistaken for a language-war opinion. It isn't. It is an empirical pattern I have observed consistently over nine months of intensive AI-assisted development, and it bears directly on the implementation-stack choice for the canonical model.

But before the AI-productivity argument, the prior context that determined the language choice in the first place: **the canonical model layer is systems programming, not application programming**. The Rust choice was made on systems-programming grounds, before the AI productivity gap was visible to me. The AI benefits emerged later as confirming evidence — the same Rust properties that make it good for systems programming turn out to also be the properties that make AI-collaborative development work well. That sequencing matters because it means the technology choice was driven by the problem's actual nature, not by tooling that became available afterward.

### The canonical model is systems programming, and that determined the language choice

Systems programming, in its strict sense, is what you do when the software has to be performant, predictable, and trustworthy under load — operating systems, databases, runtime engines, schedulers, network stacks. The defining properties:

- **Performance is a correctness property, not an optimisation goal.** If the gate-evaluation runtime is too slow, it has not run *slower* — it has *failed*, because downstream consumers cannot wait. Performance is in the spec.
- **Predictability matters more than peak speed.** Worst-case tail latency, GC pauses, JIT warmup, framework startup costs — all are correctness concerns, not nice-to-haves.
- **Testability must approach the limits of what testing can demonstrate.** Provable correctness is mathematically too strong a claim for any real-world codebase, but the test surface for systems-programming code must be more meaningful and more complete than for application code, because the consequences of undetected defects are regulatory, financial, and reputational rather than merely operational.
- **Resource accounting is explicit.** Memory, file handles, connections, locks. The runtime is not allowed to surprise you.
- **Failure modes are designed, not discovered.** Every operation has defined behaviour under every failure scenario. Errors are values, not exceptions.

The canonical model layer has every one of these properties, and they are not optional. The gate-evaluation runtime is on the hot path of every transition in every state machine across every CBU at every booking entity. It is consulted on every operation. If it is slow, the platform is slow. If it has unpredictable tail latency, the platform has unpredictable tail latency. If it cannot be tested completely, the bank cannot certify it for production. **This is systems programming masquerading as enterprise data architecture. Building it in Spring/Hibernate is using application-programming tools for systems-programming work.**

**Label calibration — public versus private.** *"Systems programming"* is the right private vocabulary because it is what I actually think. For public review, some BNY architects will hear "systems programming" and pattern-match to kernel/device-driver/embedded work, then attack the label rather than the substance. The safer public formulation is **"systems-style runtime"** or **"the canonical model layer is a systems-style runtime, not an application service"**. Same substance; less surface area for pedantic objection. Use *"systems programming"* in private notes, in conversations with engineers who get it, and in this document. Use *"systems-style runtime"* in architecture reviews, written briefings, and any context where the label might be attacked.

The fuller public formulation if the conversation goes deeper: *"the canonical model layer has systems-programming properties — performance predictability, explicit resource and failure semantics, exhaustive testability, replay, deterministic state transitions, correctness under repeated refactoring."* That is unattackable because it lists the properties rather than naming the category.

Java/Spring is excellent at *application programming* — business systems where individual operations are ms-to-seconds, where GC pauses are acceptable, where reflection costs are amortised over long-running processes, where framework runtime convenience pays back in development speed. Most BNY systems of execution are application programming, and Java/Spring serves them well. It is not engineered for the systems-programming properties above. Java *can* be used for systems programming — the JDK itself, Cassandra, Kafka, Elasticsearch are existence proofs — but **the existence proofs for Java systems programming are not Spring/JPA applications**. Kafka is not enterprise Spring. Cassandra is not JPA aggregate persistence. Elasticsearch is not Spring Data with entities. They prove the language can do systems work; they do not prove the framework can. Doing systems work in Java requires *abandoning the Spring/Hibernate stack* and writing something closer to JNI-and-low-allocation Java that most enterprise Java teams have neither the experience nor the tooling for.

### Three claims about Rust's testability — defensible, not over-claimed

Provable correctness is not on offer. Anyone claiming Rust delivers provable correctness for real-world codebases is overstating it, and the claim collapses on examination. What Rust delivers is a *materially higher ceiling* on what testing can demonstrate. Three claims, each defensible:

**The compiler sees the whole compiled crate graph.** This is the deepest property, but it needs the precise statement. The Rust compiler validates all statically expressed call/type boundaries across the workspace — every call site of every function, every implementor of every trait, every consumer of every type — within the boundaries of the compiled artefact. There is still dynamic dispatch via `dyn Trait`, feature flags via `cfg`, macros, FFI, proc macros, runtime-loaded configuration. *What there is not* is a Spring-equivalent runtime object graph that materially changes the program after compilation. The compiler's view is the program's view; the framework does not assemble a different program at runtime.

In Java/Spring, this is the gap. Reflection means call sites can be invoked from configuration. Dependency injection means the container constructs object graphs the compiler never sees. AOP weaving means methods get behaviour added to them at runtime. Annotation processors generate code the original source never references. *The compiler sees fragments; the runtime sees the program.* That gap is what test coverage cannot close, because no test can exercise paths the compiler couldn't analyse statically.

This is not a small difference. It is the difference between *testing what the compiler validated* and *testing-plus-hoping-the-runtime-assembles-it-correctly*. For systems programming, the second is unacceptable.

**Test programs and production programs are the same program.** Rust narrows the gap between test and production because there is no framework container assembling a different runtime graph for tests. Tests still use mocks, `cfg(test)`, SQLx offline mode, test databases — the gap is not zero. But the structural gap that Spring introduces (Spring Boot Test booting a different application context than production, Mockito replacing real implementations, integration tests running with H2 instead of the real database) is not present. Coverage metrics correspond more closely to actual program coverage because there is less framework-introduced divergence between what the test exercises and what production runs.

**Property-based testing scales naturally.** `proptest` and `quickcheck` let you express invariants and have the test framework search for counterexamples. For a layer where the invariants *are* the spec — bitemporal record correctness, gate evaluation correctness, identity resolution correctness — property-based testing is the right tool, and it is well-supported in Rust. Java has property-based testing libraries too, but they're less integrated with the type system and less widely used because the typical Java codebase isn't structured around invariants in the same way.

Together: not provable correctness, but *test coverage that is meaningful and complete to a degree application-programming codebases never achieve*. That is the honest version.

### The credibility transfer — Rust at the most demanding tier

*[Note for self: the brand list below is fine for private notes as a quick reference. If any of this content ever migrates to a circulated document or senior briefing, every adoption claim needs a footnote with the source. Without citations, this paragraph reads as assertion rather than evidence and a hostile reviewer will treat it as such.]*

The Linux kernel has been adopting Rust for in-kernel drivers and increasingly for foundational components. The Linux kernel is the most performance-critical, most testability-demanding, most failure-intolerant systems-programming codebase in the world. If Rust meets the bar there, the bar is met. Cloudflare runs Rust in production network services. Microsoft is using Rust for Windows kernel components. AWS built Firecracker (the VM monitor underlying Lambda) in Rust. Discord uses it for performance-critical backends. Google adopted it for parts of Android.

Each is making the same evaluation: *for systems-programming work, Rust meets the bar that previously required C or C++, and does so with safety properties C and C++ cannot offer.* That evaluation has standing. It is a credibility transfer from communities whose judgment on systems-programming tooling is unimpeachable.

The argument to deploy if the "Rust is unproven" objection comes up: *Rust is not unproven. It has been evaluated at the most demanding tier of systems programming by communities whose judgment on these matters is settled. The question for the canonical model layer is not "is Rust ready for serious work" — that question is answered. The question is "is the canonical model layer the kind of work Rust is good for", and the answer is yes.*

The less brand-heavy formulation, for use if the brand list feels cheap in a specific room: *Rust is now proven in production systems-programming contexts including kernels, cloud infrastructure, network services, security-sensitive runtimes, and performance-critical backends.* Same claim; less name-dropping; safer if the audience reacts to brand-listing as marketing rather than evidence.

### Configuration over code — why the codebase is dense rather than large

The architectural property that ties the systems-programming framing to the specific shape of ob-poc: the canonical model layer is *configuration over code*. The compiler is a generic DSL evaluator. SemOS is a generic governance registry. The DAG runtime is a generic transition evaluator. None of these is hard-coded for KYC or Deal or CBU. Adding a new sub-domain DSL, a new gating DAG, a new entity type, a new taxonomy is *configuration* — registry entries, DSL definitions, schema metadata — not new code.

This is the *opposite* of how a Spring/Hibernate enterprise codebase scales. In Spring, adding a new domain means new `@Entity` classes, new `@Service` classes, new `@Repository` interfaces, new `@RestController` endpoints, new test classes, new configuration. The code grows linearly with the domain count. Each domain pays its own boilerplate tax.

In the configuration-over-code architecture, adding a new domain *primarily* requires new configuration: a DSL definition, gate records, entity-type metadata, schema migrations for entity tables. *New code is required only when the platform gains a genuinely new primitive form.* Most domain additions are configuration-only. The compiler/runtime is unchanged; the codebase grows only when the *generic machinery itself* needs to be extended. This is the qualifier that protects the claim — one new primitive form does not invalidate it; what invalidates it would be the platform requiring per-domain hand-coded scaffolding, which it does not.

This is why the codebase is small but dense. The smallness is real — there is genuinely less code than a Spring equivalent would have. The density is real — every line is in the generic machinery, doing work for every domain that runs through it. There is no boilerplate. There is no per-domain scaffolding. There is no framework-supplied configuration the codebase has to wrangle. *All the logic is collapsed into a couple of core crates rather than spread across forms, ORM, services, repositories, controllers, and configuration classes per-domain.*

The test surface partitions cleanly as a result: dense tests for the dense generic machinery (compiler, runtime, registry, bitemporal layer), boundary validation for configuration (which is checked by code that is itself thoroughly tested), and end-to-end integration tests for realistic flows. The total test count is moderate; coverage is high; the *meaningfulness* of coverage is materially higher than an equivalently-functional Spring codebase, because each test exercises load-bearing code rather than boilerplate.

A configuration-over-code architecture is achievable in Java 25 only by rejecting the Spring stack entirely — using sealed interfaces, records, and pattern matching to define DSLs, with a custom runtime interpreting them. The language allows it. The ecosystem doesn't help. This is the same conclusion the systems-programming framing reaches by a different route: *Java 25 has the primitives, but the ecosystem's gravity is hostile to using them this way.*

### The three-tier development experience (now grounded in the systems-programming framing)

With that prior context established, the empirical productivity observation:

The gap is not "Rust good, Java bad." The gap is between three distinct development experiences:

- **Modern Rust codebase** — algebraic types, exhaustive pattern matching, no implicit nulls, no runtime reflection, explicit lifetimes, compiler-enforced invariants. AI productivity is exceptional. Refactoring a 50k-line Rust codebase with AI assistance is a tractable conversation: the compiler is the source of truth, the AI proposes changes, the compiler validates them, and dead code is impossible to hide because the borrow checker and the type system surface it. The compiler is effectively a second reviewer that never gets tired.

- **Modern Java (21+, records, sealed interfaces, pattern matching)** — substantially better than legacy Java for AI work. Records eliminate boilerplate; sealed interfaces give the compiler the same exhaustiveness checks pattern matching needs; the type system is doing more of the work than it used to. Productivity is good. Not Rust-level, but defensible.

- **Legacy Java/Spring codebase** — this is the hard case, and the one most BNY codebases actually are. The reasons AI struggles here are not the AI's fault; they are properties of the codebase pattern:

  - **Reflection and runtime wiring obscure data flow.** `@Autowired`, `@Component`, `@Configuration`, AOP proxies, dependency-injection containers — the actual call graph is assembled at runtime from annotations and configuration files. Static analysis (which is what AI does) cannot reliably trace it. The AI can read the source; the runtime is where the program *actually* runs.
  - **Dead code is genuinely hard to identify, and this is structural, not accidental.** A method might be called only via reflection, only by a Spring proxy, only through a configuration file three modules away, only via a `@Component` autowire from a class the AI cannot see from this file. Removing it might be safe; might break production at 3 a.m. This is not a bug in the framework — it is the unavoidable cost of runtime discovery and injection as a design choice. The framework's value proposition is *"don't worry about wiring, we'll find the components and inject them at runtime"*. The cost of that proposition is that nobody — including the AI, including static analysis tools, including the IDE's call-graph view — can know with certainty what is actually live. Spring's dead code problem is a side effect of the feature that defines Spring.
  - **Inconsistent patterns across a single codebase.** Twenty years of accreted decisions — some classes use constructor injection, some field injection, some setter injection; some services are `@Transactional`, some manage transactions manually; some repositories use Spring Data, some JPA `EntityManager`, some JDBC templates. AI cannot infer "the way we do things here" because there is no single way. Each refactor needs human-supplied context about which pattern to follow, and humans frequently disagree.
  - **God-objects and Spring's natural tendency toward them.** The aggregate-as-class pattern (friction #1 above) plus Spring's encouragement of service classes that grow features over time produces files where a single class has 47 methods serving twelve sub-domains. AI can edit any one method; it cannot reliably reason about the cross-cutting consequences of the edit.
  - **Garbage in the form of accumulated abstraction layers.** `BaseEntityServiceImpl extends AbstractEntityService<T extends BaseEntity> implements EntityServiceContract<T>` — patterns where the abstraction was someone's career project rather than a response to a real requirement. AI can navigate it, but slowly, and the navigation cost compounds across every change.

### What the canonical model traffics in — three categories of object

Before the Java 25 equivalence exercise can produce useful verdicts, the categories of object the canonical model handles need to be named explicitly, because orthodox modelling doesn't have clean vocabulary for all of them. The reviewer who pattern-matches a CBU to "join table" or "aggregate" or "view" is missing the category, and the conversation goes nowhere until the category is named.

**Atomic entities.** Real things in the world with intrinsic attributes and no internal structure that the model needs to represent. Companies, persons, funds, accounts, instruments, jurisdictions, currencies. Identity is a fact about the world. The world hands them to you; you record them. Stable; change rarely; conventional schema design works perfectly for them.

**Structured entities.** Also real things in the world, but their substance includes how they are constituted from other entities and the roles/capacities of those constituents. CBUs, UBO structures, operating arrangements, client groups, booking-entity assignments. *These are not constructs the bank fabricates over real entities — they are real things the bank had not previously named or structured.* The CBU exists on the street whether or not the bank's data records it; the bank's historical lack of a representation for it is a *gap*, not evidence that the thing isn't real. The defining property of a structured entity is that *its identity persists across changes to its composition* — the CBU keeps its identity when the IM is rotated; the UBO structure keeps its identity when a beneficial owner is added. That continuity is what makes them first-class, not ephemeral.

**Cross-state-machine gating DAG.** The graph of dependency constraints between transitions in different state machines. KYC.APPROVED gates Deal.CONTRACTED. Deal.CONTRACTED gates CBU.PROPOSED → PROVISIONED. CBU.VALIDATED gates InstrumentMatrix.SUBMITTED. *This is not an entity.* It is governance metadata over the state machines that entities carry, evaluated by the runtime on every transition attempt. It tells you what is currently blocked by what.

These are three distinct kinds of object with three distinct schema treatments and three distinct tooling implications. Conflating them — or omitting any of them from the model, which is the today-state for structured entities and the gating DAG — is what produces stiffware and operational fragmentation.

### A useful analogy for senior engineers — the gating DAG is Terraform-shaped

When explaining the gating DAG to engineers who have built infrastructure-as-code, this analogy lands cleanly: *imagine Terraform's resource dependency graph, but where each node is a live state machine on an entity rather than a desired-state declaration on a resource.*

Terraform models infrastructure as a DAG of resources with dependency edges. Walking the DAG produces a plan; the plan governs order of operations and blast radius of change. Change a node, walk the graph, compute what must be re-evaluated, in what order, with what dependencies satisfied at each step. The DAG is consulted at plan-and-apply time.

The canonical model's gating DAG is structurally similar but state-machine-driven rather than resource-state-driven. Each node is a live state machine belonging to an entity, with persisted bitemporal state and typed transitions. The edges are typed gates that constrain when one machine's transitions can fire based on the current state of another. The DAG is consulted *every time a transition is attempted*, not just at apply time. It is always live.

This is the right pattern-match for the architecture. The wrong pattern-matches — "state machine library" (handles one machine, not their gating relationships), "workflow engine" (orchestrates processes, not gates between independent state machines), "saga pattern" (manages distributed transactions, not declarative gating constraints) — all miss what the gating DAG actually is. Terraform's dependency graph captures the architectural primitive correctly, with the substitution that nodes are stateful entities rather than declarative resource specs.

The closing sentence to deploy:

- *Private / engineer-to-engineer formulation:* *"This is conceptually closer to Terraform than to anything in the Spring/JPA world. State-machine-driven rather than resource-state-driven, but the architectural primitive is the same: a graph of dependency constraints, evaluated by a runtime, that governs the order and conditions of changes across a federation of independently-stateful objects."*
- *Public / review-room formulation:* *"The closest engineering analogy is Terraform's dependency graph: a runtime evaluates dependency constraints before allowing state changes. Our nodes are stateful entities rather than declarative resource specs, but the architectural primitive is the same."*

The public formulation is less provocative (no contrast against Spring/JPA, no language about "anything in" the Java world) and just as clear. Use the private one when explaining to engineers who already get the architectural argument; use the public one in any room where contrasting against Spring would distract from the substance.

### The schema strategy that excludes JPA, and the layer where it does so

The three-category vocabulary above produces a clean schema strategy as a derivation:

**Atomic entities get full schema treatment.** Typed columns, foreign keys to other atomic entities (a person's nationality, a fund's domicile, a security's instrument-type), NOT NULL, check constraints, unique constraints on natural identifiers (LEI, ISIN). The schema enforces what the DB engine is good at enforcing. *This is JPA's home turf. The conventional `@Entity` mapping is correct here, and there is no need to fight that battle.* A pure-Rust ob-poc could even use a thin ORM layer at this tier if it chose to (it doesn't, but it could).

**Structured entities are stored as thin identity rows plus governed metadata for their composition.** The CBU table holds the CBU's identity, lifecycle state, bitemporal validity, and audit fields. The composition — which atomic entities participate, in which roles, with which capacities — lives in metadata records (`cbu_participation` rows with FKs to the entity table, the role catalogue, and capacity declarations). The aggregation structure is *not* in DDL; it is in metadata that DDL describes the shape of. Schema is stable; composition is dynamic; identity persists across compositional change.

**The gating DAG is stored as governed gate definitions with bitemporal validity, evaluated by the transition runtime.** Not entities. Not relationships in the conventional sense. Governance metadata over the state machines that entities carry, with content-addressed identifiers so gate definitions can be versioned and replayed under schema evolution.

The architectural principle this delivers: ***foreign keys belong where the relationship they express is as stable as the entities they connect. Where the composition itself is soft, the composition lives in governed metadata, even though the metadata still references concrete entities through FKs.*** Or, more sharply: *FKs to atomic entities are fine and necessary; FKs that encode the structure of structured-entity compositions are stiffware.*

**The two rates of change.** The deeper observation behind this strategy: there are two completely different rates of change in any data model, and conflating them is what produces stiffware. The intrinsic attributes of an entity — what makes a company a company, what makes a fund a fund — are stable. The aggregations, hierarchies, and relationships between entities — fund restructures, M&A, ManCo rotations, custody-network changes, beneficial-ownership updates — change constantly. Conventional schema design conflates the two: entity tables hold attributes, but FKs and junction tables and inheritance hierarchies in DDL hold the aggregation structure. *Because the slow-changing thing and the fast-changing thing are encoded in the same schema, the schema's overall change rate is the change rate of the faster one — and the slower thing is dragged along for the ride.*

The fix is not "make the schema more flexible" — anyone who has tried that knows it doesn't work. The fix is to *separate the layers by their rate of change* and let each evolve at its natural cadence. Stable things go where stable things belong (DDL). Soft things go where soft things belong (governed metadata). The schema captures what the world contains; the metadata captures how the bank governs its business over what the world contains. The schema is stable because the world is stable. The metadata is dynamic because the bank's governance is dynamic.

The shorthand metaphor: *stars and galaxies*. Stars are atomic entities — stable, with intrinsic attributes. Galaxies are structured entities — real, persistent, but their composition (which stars belong, in what configuration) is soft. The DAG of which solar systems aggregate into which galaxies is taxonomy/gating metadata over those structured entities. Astronomy doesn't redefine what a star is when it reclassifies a galaxy. The schema for the canonical model should follow the same discipline: stable inventory of atomic entities in DDL, soft composition of structured entities in governed metadata, gating DAG as runtime-evaluated metadata over state machines.

**The JPA verdict, layer-specific.** JPA handles atomic entities well — that is its home turf. JPA cannot handle structured entities, because JPA conflates identity with composition (loading a structured entity means loading all its constituent rows, treating composition mutation as god-aggregate mutation, losing the identity-persists-across-composition property that defines structured entities). JPA is not the right tool for the gating DAG either, but for a different reason: the gating DAG is metadata over state machines, not an entity model at all. *The exclusion is layer-specific, not blanket.* JPA is fine for the atomic-entity layer; JPA is structurally wrong for the structured-entity layer; JPA is conceptually irrelevant to the gating DAG. Since the canonical model's interesting work is at the structured-entity and gating-DAG layers, JPA is wrong for the canonical model — even though it is right for the systems of execution that consume the canonical model and that work primarily with atomic entities.

This is a much more defensible JPA argument than "JPA is wrong" (too broad) or "JPA fights multi-taxonomy" (true but abstract). *"JPA does not have a representation for structured entities, and structured entities are where the canonical model's interesting work lives"* is concrete, layer-specific, and directly verifiable by anyone who tries to model a CBU in JPA.

### Concrete equivalence — what would the Java 25 version of ob-poc actually look like?

This is the section I want to write for myself, because the moment I claim "modern Java 25 is substantially better than legacy Java" I owe myself an honest answer to the obvious follow-up: *if Java 25 is so much better, why isn't ob-poc in Java 25?* The answer needs to be specific, not ideological. So let me work through the three architectural pieces of ob-poc — NOM taxonomy generation, DSL verbs as first-class objects, and DSL CRUD — and ask what the modern Java 25 version of each would actually look like.

The exercise tests the claim. It either confirms that Java 25 has the primitives needed (in which case my Rust preference becomes a productivity-and-discipline argument rather than a primitives argument) or surfaces the places where Java 25's primitives genuinely fall short. Both outcomes are useful.

**NOM taxonomy generation.** In Rust, taxonomies are typed structures derived from the type system itself, with `serde` for serialisation, content-addressed identifiers via canonical-form serialisation (`bincode` + `BTreeMap` for stable key ordering + SHA-256), and full participation in replay-under-schema-evolution. The taxonomy node *is* a Rust type or a structured value with derived `Serialize`/`Deserialize`/`Hash`/`Ord`. In Java 25, the closest analogue is a `record` with sealed interfaces for closed type hierarchies, plus a serialisation library, plus a custom canonical-form computation for content-addressing.

The primitives align. Records are immutable by design. Sealed interfaces enforce closed hierarchies. Pattern matching gives structural deconstruction. Verdict: *this works in Java 25*. The friction is in canonical serialisation — Java's standard JSON libraries (Jackson, Gson) don't guarantee key ordering or stable representations across versions, so the canonical-form function has to be written explicitly, with explicit field ordering, explicit handling of optional/null, explicit numeric format normalisation. In Rust this is bincode + BTreeMap off the shelf. In Java 25 it's bespoke infrastructure. The work is doable but it is *infrastructure I would have to build* rather than infrastructure I get from the language ecosystem. That is the difference, and it is not nothing.

**DSL verbs as first-class objects.** In Rust, DSL verbs are typed values — structs implementing a `Verb` trait, with strongly-typed arguments, evidence requirements, authority requirements. The verb is a compile-time-checked first-class object. Pattern matching over verb types is exhaustive. Adding a new verb produces compile errors in every `match` arm that doesn't handle it, which is exactly the discipline I want.

In Java 25, this maps cleanly to a `sealed interface Verb permits ...` with record types for each verb implementation, plus pattern-matching switch expressions. The exhaustiveness check works (`switch` over a sealed interface is required to be exhaustive in modern Java). The compile-time discipline works. Adding a new verb is a compile error in every non-exhaustive switch — same property as Rust. *This is the place where Java 25 is genuinely competitive.* The translation is direct and idiomatic. If a Java team wrote this layer in pure Java 25 — no annotations, no Spring, no runtime magic, just sealed interfaces and records and pattern matching — the architectural shape would be substantively similar to the Rust version. Different syntax, comparable semantics, comparable compile-time guarantees. *Verdict: this works in Java 25, and works well.*

**DSL CRUD — the part that would have been Spring/JPA.** This is where the equivalence breaks down hardest, and it is the most important piece of the exercise. In Rust, DSL verbs that mutate the canonical model are typed transitions written as plain functions over `sqlx` queries. Compile-time SQL verification. Explicit transaction boundaries. Bitemporal tuples written explicitly into rows with `valid_from`, `valid_to`, `recorded_at`. No ORM. No session. No persistence context. No dirty-checking. No autoflush. No `@Transactional` magic. Every read and write is a function call I can grep, with arguments I can see.

In Java 25, the equivalent is *plain JDBC plus a thin query layer*. The closest direct analogue to sqlx is jOOQ — type-safe SQL with code generation against the schema. With jOOQ plus Java 25's records and sealed interfaces, you can write the DSL CRUD layer in a style that is architecturally close to the Rust version: explicit queries, explicit transactions, explicit bitemporal handling, explicit verb-to-SQL translation, no ORM. **No JPA. No Hibernate. No Spring Data. No `@Entity` anywhere in the codebase.**

This is doable in Java 25, and the language is not the obstacle. *The ecosystem is the obstacle.* Every Java tutorial reaches for JPA. Every Spring guide reaches for Spring Data. Every senior Java engineer's instinct reaches for `@Entity`. Every IDE template, every Stack Overflow answer, every internal BNY architecture-review checklist assumes JPA. Choosing pure Java 25 with jOOQ for the canonical model layer is *swimming against the current of the entire Java ecosystem*. The language permits it. The ecosystem actively pushes against it. Maintaining the discipline across a multi-person team for the lifetime of the codebase requires constant active rejection of the path of least resistance.

In Rust, this discipline is enforced by the language. There is no JPA equivalent in Rust to drift toward; sqlx is the path of least resistance, and sqlx is what the canonical model needs. The team writes the right thing because the language does not offer the wrong thing as a tempting alternative.

**The summary table I would deploy if asked.**

| ob-poc layer | Rust idiom | Java 25 idiom | Verdict |
|---|---|---|---|
| Atomic entity tables | sqlx + plain functions | JPA `@Entity` (or jOOQ + records) | JPA fine here; either approach works |
| Structured entity composition (CBU, UBO) | thin row + metadata + sealed verbs | sealed interfaces + records + jOOQ over metadata | Works only with jOOQ — JPA conflates identity with composition and breaks |
| Gating DAG runtime | trait + struct per gate + bitemporal records | sealed interface + record per gate + bitemporal records | Works in either; comparable compile-time discipline |
| DSL verbs as first-class objects | trait + struct per verb + exhaustive match | sealed interface + record per verb + exhaustive switch | Works well in either; equivalent compile-time discipline |
| Content-addressed canonical-form serialisation | bincode + BTreeMap + SHA-256 | bespoke canonical-form serialiser | Works in Java 25; bespoke infrastructure cost |

**The honest conclusion this exercise produces.** Java 25 *has* the primitives the canonical model requires. A pure-Java-25 implementation of ob-poc — sealed interfaces, records, pattern matching, jOOQ, no Spring, no JPA, no annotation magic — would be architecturally close to the Rust version. The primitives argument does not, in Java 25, hold the way it did against Java 8 or Java 11.

The argument that *does* hold in Java 25 is the *discipline* argument. Rust enforces the data-first stance by language design. Java 25 *permits* the data-first stance, but the Java ecosystem — frameworks, training, instinct, tooling defaults, the legacy Java stance that compiles alongside the modern one — actively pulls the team back toward the operations-first/ORM/Spring stance. Maintaining modern-Java discipline across a multi-person team across years requires constant vigilance against the gravitational pull of the ecosystem. In Rust, the gravity is on the right side. In Java 25, the gravity is split, and the team has to choose, every day, every PR, every dependency, every framework decision.

For ob-poc specifically — small greenfield codebase, AI-collaborative development, correctness-critical — the gravity-on-the-right-side property of Rust is decisive. For a different project — large team, deep Java expertise, embedded in a Java-shop, with senior engineers willing to enforce the discipline — pure Java 25 with jOOQ would be a defensible choice. I would not pick it, but I could not honestly say it would not work.

This is the version of the argument I should have ready if asked *"why not modern Java instead of Rust?"*. The answer is not "Java can't do it." The answer is "Java can do it, but doing it requires constant team discipline against an ecosystem that does not help, whereas Rust enforces the same discipline by language design — and the difference shows up in how much cognitive budget the team spends on architectural vigilance versus on the actual problem."

### The decisive distinction — mandatory discipline versus voluntary discipline

The cleanest formulation of the entire Java-versus-Rust argument, distilled to one principle:

*The decisive distinction is not Java versus Rust at the syntax level. It is mandatory discipline versus voluntary discipline. Java 25 can express the architecture if the team voluntarily rejects Spring/JPA habits on every PR. Rust makes more of the required discipline the default shape of the language and ecosystem. For a small, correctness-critical, greenfield canonical runtime, choosing the stack where the architectural gravity points in the right direction is not preference; it is risk reduction.*

This is the version to deploy in any senior conversation where the question of language choice arises. It does several things at once. It frames the choice as risk management, which is the language compliance officers and senior architects respond to. It concedes that Java 25 *could* work, which disarms the listener expecting an absolutist position. It identifies the specific risk being managed — the cost of voluntary discipline at scale, which is what compounds into today-state codebases. And it ends on *risk reduction*, which is unambiguously the kind of consideration a custody-banking architecture review is supposed to take seriously.

If I lose every other specific argument in a review and remember only this paragraph, the case still stands. The mandatory-vs-voluntary cut is the one to internalise.

### Java's continuity is partly an illusion

There is a deeper observation behind the equivalence exercise that bears stating directly. Java's strongest marketing message has always been backward compatibility — "code written for Java 1.4 still runs on Java 25." This is technically true at the runtime level: bytecode runs, libraries link, deployment artefacts are valid. From a *runtime* perspective, the continuity is real.

But the *language* underneath that runtime has bifurcated. Modern Java (records, sealed interfaces, pattern matching, switch expressions, virtual threads, explicit data modelling) is a fundamentally different language *in stance* from legacy Java (mutable classes, inheritance hierarchies, getter/setter ceremony, reflection, container-managed everything). They share syntax fragments. They share the JVM. They share library ecosystems. They do not share an architectural philosophy.

Modern Java's direction of travel since Java 14 has been *toward the same data-first stance the canonical model requires*. The language has, quietly and without admitting it, been adding the primitives the data-first paradigm needs — implicitly acknowledging that the class paradigm was the wrong primary unit for data-centric work. Legacy Java is operations-first in stance, and Spring is the apex framework of that stance.

Both stances compile under the same `javac`. Both run on the same JVM. The continuity claim — that Java 25 is "still Java" — papers over the split. *The continuity is a deployment continuity, not an architectural one.* Choosing modern Java requires actively rejecting the legacy stance, on every line, against the gravity of the ecosystem.

The Spring-7-supports-Java-25 framing is the same problem at framework level. Spring continues to encourage the legacy stance — annotation-driven behaviour, runtime-discovered wiring, container-managed lifecycle, ORM-managed identity — *while running on a JVM whose language has moved away from those patterns*. The framework's gravitational pull is backward; the language's gravitational pull is forward. "Spring supports Java 25" is a deployment-compatibility statement, not an architectural-alignment statement. It is true and it is not a refutation of any of the arguments above.

What I should have ready, when someone says *"but Spring 7 is modern, it supports Java 25!"*: **"Spring running on Java 25 is a runtime compatibility fact. It does not change the framework's architectural stance, which is still annotation-driven, runtime-wired, container-managed, ORM-coupled — all of which Java 25's language direction is moving away from. Spring on Java 25 is two architectural eras bolted together by the JVM. The compatibility is real; the architectural alignment is not."**

The continuity-as-illusion framing is the broader principle. Java's claim that "your old code still runs" is true and useful, but it has produced an ecosystem in which two architecturally incompatible languages compile under the same compiler, and the team has to actively choose which language they're writing today. In Rust the language enforces the choice. In Go the language enforces the choice. In Java the choice is voluntary, and voluntary discipline at scale is what produces today-state codebases.

The compounding cost matters, but *where* it compounds matters more than a single multiplier. The honest picture is two-phase:

- **Initial clean-room construction** — vibe-coded greenfield, no legacy. The productivity gap here is real but modest. Both Rust and modern Java let AI move fast when there is nothing to fight against; the type system pays off but the absence of accumulated mess matters more. This is the phase where Java/Spring shops will tell you Rust has no advantage, and they are partly right.

- **Serious refactoring of an existing codebase** — this is where the gap explodes. My strong suspicion, based on nine months of intensive AI-assisted work across both stacks, is that the productivity differential here is large — possibly very large — but I will not put a number on it without instrumented evidence. What I can say with confidence is *why* it shows up: when the compiler and Clippy form a firewall against incorrect refactors, the AI can propose aggressive structural changes and trust the toolchain to surface every consequence. When the toolchain cannot give that guarantee — because of reflection, because of runtime wiring, because of inconsistent patterns the AI has to infer rather than verify — every proposed change requires human-supplied context about what *might* break. The cost of supplying that context, repeatedly, across every refactor cycle, is the productivity gap.

The compiler-as-firewall argument is the one I trust empirically. The multiplier estimate is the one I would hesitate to defend in a review without data.

### The honest qualification — Rust isn't immune

To stay intellectually consistent, I have to acknowledge that Rust has a milder version of the same anti-pattern, and the same productivity loss follows when it isn't disciplined. The mechanism is different but the effect is in the same family.

The mechanism: aggressive use of `pub` without thoughtful visibility scoping. When a crate exposes most of its internals as `pub` rather than `pub(crate)`, `pub(super)`, or module-private, the compiler still enforces correctness — types still check, lifetimes still validate, the borrow checker still does its job. But the *refactoring tractability* degrades, because the AI has to assume any `pub` symbol is used somewhere it cannot see — by another crate, by a downstream consumer, by integration tests in a workspace member it doesn't have visibility into. Aggressive structural change becomes harder to propose with confidence, for the same reason it does in Spring: the symbol's true blast radius is unknowable from the local view.

This is a much milder problem than Spring's. `pub` is a static visibility marker, not runtime injection — the AI can at least *find* every use by grep across the workspace, which it cannot do for a Spring autowire. And the discipline to fix it is straightforward: default to module-private, use `pub(crate)` for crate-internal API, reserve bare `pub` for the genuine public API. Crates that take this seriously feel completely different to refactor than crates that don't.

But it is the same family of problem, and that matters for the argument's honesty. The principle is **visibility minimisation as a refactoring affordance**, not "Rust good, Java bad". Spring's runtime wiring is the worst case of the principle being violated; Rust with everything `pub` is a milder case of the same violation. The full Rust productivity payoff that AI work depends on requires the language's visibility discipline to actually be applied — well-designed crates with tight `pub` boundaries, workspaces where API surfaces are deliberate rather than accreted.

For ob-poc this is something I have to bear down on actively. The temptation to slap `pub` on a struct or method to unblock a compile error is real, and every time I do it I am making the future refactor harder. The discipline is constant; the payoff is the codebase staying tractable as it grows.

### The deeper version of the same principle — sliding-window data and separate function namespaces

The `pub` discipline is the symbol-level version of a deeper architectural principle that runs all the way through my data strategy, and naming it explicitly is worth the space. The principle is **separation of governed data from the functions that manipulate it**, with sub-domain operations expressed as separate function namespaces over a shared window of governed data.

The data side: DAGs and taxonomies are the primary, persistent, governed structure. They are the *window* — the surface over which all sub-domain operations are defined. The window slides because taxonomies are dynamic and the DAG evolves, but the window is the thing that is persisted, governed, and audited. It is *what exists*.

The function side: many sub-domain DSLs — KYC operations, deal operations, CBU operations, instrument-matrix operations — each expressing its own typed transitions against the window. They are entirely separate from the data. Each DSL is its own namespace. None of them owns the data; all of them operate on it through typed contracts.

Two consequences follow, and both matter.

**Implementation flexibility.** Sub-domain operations can evolve independently because they are not embedded in the data definition. A new KYC DSL verb is a function-namespace addition; it does not require touching the data structure. A new sub-domain joining the platform — adding the legal-entity-hierarchy slice, say — defines its own DSL over the same window, without any other sub-domain having to know it exists. The window is shared; the namespaces are independent.

**Auditability as a structural property, not an instrumentation overhead.** This is the consequence I had not fully articulated to myself until now and which is the strongest single argument for the stance. When functions are separate from data, every operation against the window is an *external observable event* — a sub-domain DSL invokes a typed transition against the governed structure, and that invocation is recordable as primary data because it crosses a definitional boundary. When functions are methods inside classes (the Java/Spring stance), the call is internal to the aggregate; it is observable only via wrapping instrumentation that the class itself cannot enforce. Auditability in the data-first stance is *not* a feature you build; it is a property you cannot avoid, because every operation has to cross the boundary between the function namespace and the governed window, and every boundary crossing is recordable.

For a custody banking platform under CSDR, MiFID, AIFMD, and a regulator who can ask "what was the state at 10:00 UTC on March 14 and what did you believe at the time?" — auditability-as-structural-property is not a stylistic preference. It is the regulatory floor. And the data-first stance is the only one that delivers it without instrumentation overhead.

**Why class-based languages make this borderline impossible.** Java and the class-oriented tradition put behaviour *inside* the data definition. The methods belong to the class; the class is the unit of identity, persistence, and operation. When multiple sub-domains all need to manipulate the same data, you face the choice already documented: god-object (all sub-domain methods inside one class, side-effect containment impossible) or bypass-the-aggregate (sub-domains mutating via repositories or native SQL, invariants no longer enforced anywhere, auditability lost). There is no third option in the class paradigm because the paradigm itself defines data and behaviour as a single unit. Trying to do data-first multi-namespace operation against a Java domain model is fighting the language; you can do it, but you are constantly battling the paradigm rather than working with it.

**The fundamental difference, stated as cleanly as I can put it.** Class-based operation against shared data is necessarily a *two-step call*: the sub-domain code calls a method on the class (`cbu.updateKycStatus(...)`), the class mutates its own state. The audit question *"who changed this, when, and why?"* is answered only by knowing who called the method, which is information the class itself does not carry. To recover it, the class — or some wrapping infrastructure — has to maintain a separate call log, and someone has to read the log and correlate it to the state change. The audit is *reconstructive*: the *what* lives in the data, the *who* lives in a log, and the answer is a join that someone has to trust.

The data-first stance with separate function namespaces — Rust traits, Go interfaces, DSLs operating directly on the governed DAG — is a *one-step call*. The sub-domain emits a typed transition; the transition itself is the primary record; the *who*, the *what*, the *when valid*, the *when recorded*, the evidence, and the authority are the same tuple because the function namespace and its arguments *are* the operation. There is no method to call, no class to mutate, no second step. Audit is *constitutive*: the record is the audit.

This is not a stylistic difference. It is a difference in what the unit of work *is*. In the class paradigm the unit of work is a method invocation that produces a state change as a side effect. In the data-first paradigm the unit of work is the transition itself, recorded as primary data, with no side effect because there is nothing alongside the record that needs to stay consistent with it.

For the regulator question — *"what was the state at 10:00 UTC on March 14 and what did you believe at the time?"* — the consequence is direct. In the class paradigm, the answer requires joining the state table to the audit log and trusting the join. In the data-first paradigm, the answer is one row of one table, by construction. There is nothing to join because there is no separation between the operation and the record of the operation.

**Why Go and Rust make this natural.** The interface (Go) and trait (Rust) are first-class language constructs that are *external to the data definition*. A Rust struct does not know what traits implement it; a Go struct does not know what interfaces it satisfies. The data exists as a passive structure; the operations attach via traits or interfaces defined elsewhere — often in a different sub-domain, often in a different crate or package. This is the language-level expression of exactly the data-first stance. Sub-domain DSLs become trait implementations or interface satisfactions over the shared window, each in its own namespace, each independently versioned, each independently audited.

Java has interfaces too, but the implementing class must declare the contract — `class Foo implements Bar` — which couples the data definition back to the operation contract and partially defeats the separation. The class still owns its methods. Rust traits and Go interfaces are decoupled by language design; Java interfaces are decoupled by convention, and convention loses to the path of least resistance in any large codebase.

This is the *architectural* version of the `pub` discipline. The `pub` discipline says *minimise the visibility of internal symbols so that refactoring remains tractable*. The data-function separation says *minimise the coupling of data to behaviour so that sub-domains remain independent and operations remain auditable*. They are the same principle at different scales: define what is exposed, deliberately and minimally, and reap the tractability and auditability benefits that follow. Spring's runtime injection violates the principle at runtime; Java's class paradigm violates it at the language level; aggressive bare `pub` violates it at the symbol level. All three failures cost the same things — refactoring tractability and observable boundaries — for the same reason.

### Why this matters for the canonical model specifically

The canonical model is greenfield. It is also small relative to the systems of execution that consume it — a few tens of thousands of lines, not the hundreds-of-thousands that a custody platform involves. This is the *exact* profile where the Rust productivity advantage compounds most: small enough that the type-system tax doesn't dominate, greenfield enough that there is no legacy mess to fight, complex enough that the type-system payoff is real.

The same canonical model implemented in legacy Spring would be — based on my pattern-matching against similar BNY internal projects — three to five times the line count, with substantially worse correctness guarantees, and would still need a custom bitemporal layer, a custom gate runtime, a custom identity-resolution service, and a custom self-versioning protocol because Spring offers nothing for any of them. The framework would provide CRUD plumbing the model doesn't need and would actively hinder the parts that matter.

### What I will not say in public

That AI-assisted productivity is materially worse on legacy Java/Spring codebases. It is true and it is documented in my own work, but it sounds like a personal preference dressed as evidence and the audience that most needs to hear it is the audience least equipped to evaluate it. The Java/Spring developer who has not yet worked seriously with AI on a Rust codebase has no reference point for the gap, and pointing it out in a review is unhelpful regardless of accuracy.

### What I might say in private, if asked directly

That for greenfield work where AI-collaborative development is the operating model — which it now is, for me and increasingly for the industry — language and framework choice has a measurable effect on iteration speed and correctness. That effect is large enough to be a first-order consideration for new builds, alongside library ecosystem and team familiarity. The canonical model is greenfield, small, and correctness-critical; it is the case where the effect matters most.

This is the conversation I want to be ready for when it comes up — not to lecture, but to answer honestly if someone senior asks why ob-poc is in Rust rather than the BNY house language.

### Audience-aware framing — productivity for engineers, auditability for everyone else

A tactical point I need to internalise before any senior conversation: the productivity argument and the auditability argument are aimed at different audiences, and I should pick deliberately, not by accident.

The **productivity argument** — AI-collaborative development is faster on Rust, the compiler/Clippy firewall pays out at refactor time, the visibility discipline matters — is for engineers. It assumes the listener has worked with both stacks at scale and can evaluate the claim against their own experience. To anyone else it sounds like a personal preference dressed as evidence. Engineers will respond to it; compliance officers, risk officers, and senior business architects will not, and trying to use it on them is actively counterproductive.

The **auditability argument** — auditability as a structural property rather than an instrumentation overhead, the one-step-versus-two-step distinction, the regulator's state-at-time question answered by one row instead of a join — is for everyone else. Compliance officers, risk officers, audit teams, regulators themselves all respond to this argument because it speaks their primary concern: can the bank answer state-at-time questions deterministically, and how much effort does each answer take? The argument that the data-first stance answers regulatory inquiries with a single tuple by construction, while the class-based stance answers them by joining state to audit log and trusting the join, is the version that lands hardest with the audience that ultimately holds the most weight in a decision about a custody banking platform.

For any review where compliance is in the room — or any senior conversation where the regulatory-cost framing matters — I should lead with auditability, not productivity. The productivity argument is a follow-up if engineering peers want to understand the implementation rationale. The auditability argument is the one that justifies the architectural stance to the people who pay for it.

The fundamental sentence for that audience, the one I should have ready: *"In the class-based stack, when a regulator asks who changed the CBU's KYC status on March 14, we answer by joining the audit log to the state table and trusting the join. In the data-first stack, the answer is one row."* That sentence does the work that ten paragraphs of architectural prose cannot, because it makes the regulatory cost of the alternative concrete in a way a compliance officer can immediately evaluate.

---

## Tactical notes for handling this in review

### What to say in the public room

The tooling line is: *the modelling stance excludes patterns that require the model to be reshaped to fit the tool; within that constraint, technology choice is open*. This is true. It is what §2 says. It does not pick a fight.

When pressed: *the canonical model layer needs bitemporal-aware persistence, content-addressed schema evolution, and identity-as-governed-object — the implementation stack will be chosen to satisfy those constraints*. This is also true. It does not name names.

If pushed harder: *Hibernate/JPA is the right tool for the systems of execution that consume the canonical model — those are CRUD-shaped and benefit from the framework. The canonical model itself is a different category of object and will use tooling appropriate to it*. This concedes ground gracefully where the concession is true and holds ground where it isn't.

### What to keep out of the public room

- Anything that names class-oriented persistence as fundamentally wrong-paradigm. Even though it is, for this layer.
- The detailed six-friction list, unless someone explicitly asks for the implementation rationale.
- The Rust/Go preference, until the data-architecture endorsement is in hand. Tooling debate now will derail the substantive review.

### What to do if the data architecture endorsement is granted

Then the tooling conversation can be more direct, because the constraints (bitemporal, multi-taxonomy, governed identity, content-addressed self-versioning) are now agreed deliverables. Tooling that cannot deliver them is excluded by the spec, not by my preference. *That* is the conversation I want to be having — but only after the data architecture is settled, not before.

### What to do if a senior architect insists on JPA for the canonical model layer

Three options, in order of preference:

1. **Walk through the six frictions in detail**, framed as questions: *how do we represent a CBU that is in seven taxonomies? How do we answer the bitemporal regulator question? How do we add a new taxonomy without coordinating a deployment across seven teams?* Make them answer, not me. If the answers are honest, they will arrive at the same conclusion. If they are not honest, the dishonesty is on the record.

2. **Concede the systems-of-execution layer publicly.** Sometimes the political win is letting the JPA shop keep the layer they're good at. The canonical model is small; the consuming systems are large. Don't fight every battle.

3. **Pilot in Rust quietly.** Build the canonical model layer in the technology it needs, with a Java consumer alongside it that demonstrates the read/write contract. Empirical evidence is harder to argue with than architectural prose.

Option 3 is what ob-poc has been doing. The pilot is the argument.

---

## Counter-arguments and how to answer them

Three counters will come up in any serious architecture review, in roughly this order. Each exposes a deeper layer of the architectural difference, and each answer relies on the previous one having landed. By the time I am answering counter 3, the prerequisites established by counters 1 and 2 are doing the work.

I should know all three before walking into the room. Improvising under pressure on these is how endorsement gets lost.

### Counter 1 — "Use bounded contexts with per-domain aggregates. KYC class, Deal class, OB class. That's what DDD says."

This is the orthodox Java/Spring response and it has twenty years of advocacy behind it. Take it seriously and answer it carefully.

**Why it sounds like a complete answer.** Each domain has its own bounded context, its own aggregate root, its own data, its own lifecycle. Communication between contexts via events or APIs at the boundaries. Inside each bounded context, the class paradigm works fine. This is canonical DDD; it is what most BNY architects will reach for instinctively.

**Why it isn't.**

*The shared identity problem.* Each aggregate references the same client, the same legal entity, the same CBU — but each owns its own representation of the entity within its bounded context. Identity drift is inevitable: KYC's `subjectEntityId` and Legal Entity Hierarchy's `entityId` for the same SICAV are populated by different teams from different sources, and over months they diverge. Identity cannot be provisional, superseded, or corrected bitemporally because each aggregate's `@Id` is a primary key, not a governed identity. Cross-domain queries — *"for this Asset Owner, which CBUs trace back across all funds, all ManCos, all jurisdictions?"* — become four-system joins with manual reconciliation rather than traversals over a single identity layer.

*The bounded-context architecture is the today-state.* This is the bit that bites hardest. Per-domain bounded contexts with their own aggregates, communicating via events and APIs, *is exactly what BNY has today*. Custody has its own data model. KYC has its own. FA has its own. TA has its own. Each is well-designed within its bounded context. Each evolved independently. The proposed canonical model exists *precisely because* that architecture has produced — over twenty years — the operational costs the Approach Paper §1 documents. STP slips between bounded contexts because no governed substrate represents the chain end-to-end. Capability migration requires re-onboarding because the binding lives inside each context. Audit reconstruction is bespoke because each context has its own log. The Java/Spring counter-proposal is *the existing architecture, restated as a principle*. It is what produced the problem. Proposing it as the solution is begging the question.

*The architectural pattern itself is the wrong shape for what the canonical model is doing.* The canonical model is not another bounded context. It is a layer that sits *above* all the bounded contexts and provides them with shared governed state — the integration substrate they implicitly assume but never make explicit. DDD's textbook answer is "anti-corruption layers" — each context translating between its own model and a shared canonical model at its boundary. That works *if* the canonical model exists as a first-class governed object. In DDD-orthodox practice, it usually doesn't; it lives implicitly as event schemas and API contracts at boundaries. The Approach Paper proposes making it explicit.

**The closing sentence.** *"Per-domain class aggregates are exactly what we have today. The proposal is not to replace them — they should keep doing their work in their bounded contexts. The proposal is to add the governed layer above them that has been missing, and which their event schemas and API contracts have been implicitly approximating for twenty years without ever being made first-class. The canonical model is not in competition with KYC's aggregate or Custody's aggregate. It is the substrate they integrate with — and the case for paying for it is that no amount of refining the per-domain aggregates has produced cross-domain coherence, because the coherence problem doesn't live inside any of them."*

This sentence does three things at once. It concedes per-domain aggregates are fine within bounded contexts, which disarms the listener. It identifies the layer where the actual failure lives. It frames the canonical model as additive rather than replacing — which is true, and which is the version that has any chance of being endorsed politically.

### Counter 2 — "Your DSLs are just classes by another name. KYC DSL, Deal DSL, OB DSL — same as KYC class, Deal class, OB class."

This is the harder counter, because on the surface it looks symmetric. Same number of namespaces, same domain decomposition. *"You've just renamed classes as DSLs."*

The asymmetry is real but subtle, and articulating it precisely matters.

**What the response misses.** Both architectures partition operations by domain, but they partition different things. Class-based: data and operations bundled together inside each class — the KYC class owns KYC's data *and* KYC's methods. Data-first: the KYC DSL is *only* an operation namespace; it has no data. The data lives in the canonical model; the DSLs are external function namespaces that act on it. Data and operations are partitioned along *different axes* — data by governed taxonomy, operations by sub-domain — and the two partitions are independent.

That is the asymmetry. And it has direct consequences.

*Cross-domain reads are free in one model, expensive in the other.* KYC needs to read Deal state. Deal needs to read CBU state. Instrument Matrix needs to know KYC has cleared. In the class architecture, every cross-domain read is an integration with a contract, a versioning concern, and a failure mode — class-to-class API call, or event publication and subscription, or anti-corruption layer translating between models. In the data-first architecture, every DSL reads the same window directly; there is no integration because there is no domain boundary on the data, only on the operations. *"Can this CBU transact?"* requires reading deal state, KYC clearance, BP clearance, service consumption, capability bindings, and instrument matrix population — six aggregates with six contracts in the class architecture, one traversal in the data-first one.

*The audit asymmetry survives the response intact.* Multiple classes — KycCase, Deal, OB — doesn't change the reconstructive-audit problem; it just means the problem now exists per-class, with separate call logs, separate correlations, separate joins. The instrumentation overhead is multiplied across domains, not eliminated.

*Schema evolution behaves completely differently.* A new attribute that KYC, Deal, and Instrument Matrix all need to reason about — say a new ESG-classification field — requires modifying the class that owns the data, deploying that change, and propagating to every aggregate that reads it. Anti-corruption layers updated. Event schemas versioned. Three teams coordinated. In the data-first architecture, it's a registry change to the canonical model. The DSLs that need it are updated to read it; the others aren't. There is no class to modify because there is no class.

*The trait/interface point is more than syntactic.* Rust traits and Go interfaces define a contract over behaviour that any data shape can satisfy. Multiple sub-domain DSLs can define their own traits over the same canonical model — KYC's traits, Deal's traits, OB's traits — and the canonical model implements all of them, simultaneously, without any of the traits knowing about each other. A Java interface requires `class KycCase implements KycContract`, binding the class to KYC's contract at compile time. If Deal also wants operations over the same data, Deal needs its own class with its own copy of the data and a translation layer between them — which is the bounded-context architecture, with all its problems.

**The closing sentence.** *"The asymmetry is at the level of what each architecture partitions. Class-based: data and operations partitioned together, into the same units. Data-first: data partitioned by governed taxonomy, operations partitioned by sub-domain DSL, and the two partitions are independent. The KYC DSL and the KYC class look superficially similar — both group KYC operations together. But the KYC class owns KYC data; the KYC DSL owns no data. That difference is what makes cross-domain reads free in one model and expensive in the other, audit constitutive in one and reconstructive in the other, schema evolution local in one and coordinated in the other. The DSLs are not classes by another name. They are function namespaces over a shared governed window — which is a different architectural primitive entirely."*

### Counter 3 — "Shared data is a global access problem. Bounded contexts solve it naturally."

This is the highest-stakes counter because it touches access control, and access control is an *active* compliance concern (every minute the platform runs, who can see what) rather than a *reactive* one (after the fact, what was the state at time T).

**Why the concern is legitimate.** A platform-level shared data model creates an obvious objection: if everything reads the same data, every domain can see every other domain's data — security and compliance nightmare. The bounded-context architecture's implicit answer is that each context owns its own data, so access is naturally scoped to whoever can talk to that context. The data-first stance has to answer this differently.

**The answer — the DSL is the gate.** No sub-domain has direct read or write access to the canonical model. All access flows through DSLs and agents. The DSL is the *only* way a sub-domain operation reaches the data, and the DSL is itself a governed object — typed verbs, declared evidence requirements, declared authority requirements, declared scope of which parts of the data each verb can touch. The agent invoking the DSL operates under a typed identity with declared permissions over which DSL verbs it can execute and under what conditions.

The access model is therefore: *identity → authorised DSL verbs → typed transitions against the governed data*. Three checkpoints, all expressed as governed data themselves. Function-level gating on top of data-level governance. Every operation is constrained at two levels — what the verb can do, and who can invoke the verb — and both levels are themselves governed data, which means the access model is auditable by construction the same way the operations are.

**Why this is structurally different from class-based access control.** In Java/Spring, access control is `@PreAuthorize` annotations on service methods, plus method-level Spring Security checks, plus Hibernate filters limiting query results. This works, but access control is *behaviour attached to data containers*, not a separate governed layer over a shared substrate. The consequences:

- **Per-class access control.** Every class declares its own rules. Cross-cutting policies — *"no agent can read KYC data and Deal data in the same operation unless under explicit dual-control authorisation"* — cannot be expressed naturally because there is no place to put them.
- **Fragmentation across bounded contexts.** Each context implements its own access control. Three months later they're inconsistent. Six months later, five different ways the codebase expresses "who can read what" and a security audit finds a hole.
- **Access control cannot easily be governed as data.** The annotations are code; changing them requires deployment. A new compliance rule that takes effect at month-end means a release cycle. In the DSL-gated stance, the access policy is data — changeable under governance, with bitemporal records of when each policy was in effect.
- **Access control conflates with business logic.** The `@PreAuthorize` decoration sits on the same method as the logic, so the two are intermixed. Changing one requires touching the other.

**The deeper architectural truth.** What this is, fundamentally, is *least privilege through narrow interfaces* applied at the architectural layer rather than the OS or network layer. In class-based architecture, "least privilege" has to be reconstructed from a forest of annotations and aspect interceptors, none of which compose cleanly because they're all attached to different classes. In the DSL-gated data-first stance, "least privilege" is structural: an agent literally cannot reach data except through DSL verbs it is authorised to invoke, because the DSL is the *only* API the canonical model exposes.

**Why this matters specifically for custody.** Segregation of duties, dual control, audit of who-saw-what-and-when. Chinese walls between sub-business-lines. Fund-of-funds confidentiality. Beneficial-owner protection. Cross-CBU exposure data sensitivity. ManCo-level oversight access rules. A platform-level shared data model that *cannot* express access control as a first-class governed concern is unfit for purpose for custody banking. A platform-level shared data model that expresses access through narrow gated DSLs *exactly* matches what compliance requires. This argument lands harder with compliance and risk than auditability does, because it speaks to active daily concern rather than reactive inquiry.

**The closing sentence.** *"Shared data does create a global access problem if access is unmediated. In this model, access is never unmediated — every operation flows through a gated DSL verb invoked by an authorised agent, and the entire access policy is itself governed data with bitemporal records. Class-based architectures bolt access control on across N domain classes and N aspect interceptors, and the result is genuinely hard to design coherently across a multi-domain platform. In the DSL-gated stance, the design is one concept: access is permission to invoke verbs. There is no other access. The DSL design is the access design."*

### The diagnostic that ties all three counters together

Each counter exposes a different layer of the same architectural choice, and each answer relies on the same underlying diagnostic: **good architecture makes problems disappear; weak architecture makes problems easier**.

- Counter 1 — bounded contexts make cross-domain coherence *easier* (anti-corruption layers, event contracts). The data-first stance makes the cross-domain coherence problem *disappear* — there are no cross-domain boundaries on the data because the data is one governed substrate.
- Counter 2 — class-based per-domain aggregates make cross-domain reads *easier* than nothing (better than direct database access). DSL-gated function namespaces over shared data make cross-domain reads *disappear as a cost* — the Deal DSL reads CBU state directly because there is no integration to traverse.
- Counter 3 — class-based access control makes per-class access *easier* (annotations, AOP). DSL-gated access makes the access-control-as-separate-concern *disappear* — access is permission to invoke verbs, governed in the same registry as everything else.

Easier solutions accumulate cost over time as the problem recurs in new forms. Disappearance solutions pay the cost once at the architectural level and never again. Twenty years of "easier" compounds into the today-state. Disappearance compounds into a platform.

This is the diagnostic to deploy when any of the counters lands hard. The class advocate is offering an *easier* version of a problem the data-first stance makes *disappear*. That is not a tie. That is a structural difference that compounds across every operation the platform will ever do.

---

## Things I should remember when I'm tired and someone is shouting at me

- The category mismatch argument is correct. It is not personal. If someone takes it personally, that is information about them, not about the argument.
- The canonical model needs to exist whether or not the implementation stack is the one I prefer. Endorsing the data architecture is the priority. Tooling can be re-litigated later if I lose it now.
- *"Tooling fits the model, not the reverse"* is the line. Repeat it. It is true, it is short, and it does not name any vendor.
- The `pub` discipline on ob-poc is the *internal* equivalent of "tooling fits the model" — the same principle applied at the codebase level rather than the architecture level. Define what is exposed, deliberately and minimally. Every bare `pub` I add to silence a compile error is a violation of the same principle I am asking the bank to honour at architecture level. Maintaining the discipline internally is what makes the external argument credible. If I cannot do it on my own codebase I have no standing to ask the bank to do it on theirs.
- **Disappearance versus easier.** Good architecture makes problems disappear; weak architecture makes problems easier. Every counter-argument I face is offering an *easier* version of a problem the data-first stance makes *disappear*. The two are not equivalent; disappearance compounds into a platform, easier compounds into the today-state. This is the diagnostic that survives every specific debate, and it applies far beyond this argument — it is the test to apply to every architectural proposal I encounter for the rest of my career.
- **The canonical model layer is systems programming, not application programming.** This is the prior that determined the language choice, before the AI productivity gap was even visible. If asked why Rust, lead with: *"the canonical model layer is systems programming, and Rust is a systems-programming language. The AI productivity benefits are confirming evidence, not primary justification."* That framing moves the conversation off Rust-vs-Java preference (unwinnable) and onto category-of-problem-vs-category-of-tooling (winnable on facts).
- **Provable correctness is not on offer; meaningful test coverage to the limits of what testing can demonstrate is.** Don't over-claim. The real argument is that Rust's compiler sees the whole codebase, test coverage corresponds to actual program coverage, and property-based testing is well-integrated. That ceiling is materially higher than what application-programming codebases achieve, and for systems-programming work it is what matters.
- **The Linux/Cloudflare/Microsoft credibility transfer settles the "Rust is unproven" objection.** Don't argue Rust's maturity in the abstract; cite the adoptions. *"The question of whether Rust is ready for serious systems programming has been answered by communities whose judgment is settled. The remaining question is whether the canonical model layer is the kind of work Rust is good for, and the answer is yes."*
- **Configuration over code is the architectural property that ties everything together.** The codebase is small and dense because the compiler, runtime, and registry are generic; new domains are configuration, not new code. A Spring/Hibernate codebase grows linearly with the domain count because each domain pays its own boilerplate tax. The configuration-over-code architecture is the structural reason testability scales, the codebase stays tractable, and AI-assisted refactoring works.
- **Three categories of object: atomic entities, structured entities, gating DAG.** When someone pattern-matches a CBU to "join table" or "aggregate" or "view", redirect: *"a CBU is a structured entity — a real thing in the world whose substance includes how it is constituted from other entities, with identity that persists across compositional change. Orthodox modelling doesn't have a clean name for this category; that's why we're talking past each other."* The vocabulary is what stops the miscommunication.
- **The gating DAG is Terraform-shaped.** When explaining the architecture to senior engineers, this analogy lands cleanly. Imagine Terraform's resource dependency graph, but where each node is a live state machine on an entity rather than a desired-state declaration on a resource. Same architectural primitive; different content. Wrong pattern-matches: state machine library, workflow engine, saga pattern. Right pattern-match: Terraform.
- **Schemas become stiffware when fast-changing things are encoded into them.** Important and slow-changing belongs in DDL. Important and fast-changing belongs in governed metadata that DDL describes the shape of. *Importance is not the test; rate of change is the test.* The CBU is important and the bank's governance of it changes constantly — that combination means it belongs in metadata, not in the schema.
- **Java 25 has the primitives, but the ecosystem has the gravity.** When asked "why not modern Java?", the honest answer is that Java 25 *can* express the data-first stance, but doing so requires constant team discipline against an ecosystem that pulls toward the legacy stance. Rust enforces the discipline by language design. Java 25 makes it voluntary. Voluntary discipline at scale is what produces today-state codebases. This is the answer; have it ready.
- **Mandatory discipline versus voluntary discipline.** The decisive distinction is not Java versus Rust at the syntax level. It is which stack puts the architectural gravity on the right side. Java 25 can express the architecture if the team voluntarily rejects Spring/JPA habits on every PR. Rust makes more of the required discipline the default. For a small, correctness-critical, greenfield canonical runtime, *choosing the stack where the architectural gravity points in the right direction is not preference; it is risk reduction.* This is the framing that lands hardest in compliance/risk audiences and survives every other specific debate. If I remember nothing else, remember this.
- **"Spring supports Java 25" is a runtime compatibility fact, not an architectural one.** Two languages compile under the same javac. The continuity claim papers over a fundamental split. Don't be drawn into arguing that Java 25 is bad — argue that Java 25 is good, and Spring's gravitational pull is in the opposite direction from where Java 25 is going.
- Most resistance to non-Java tooling at BNY is sunk-cost framing dressed as technical preference. The honest response is not to argue technology preference back at it, but to keep the argument on the model's requirements. The model's requirements either can or cannot be met by a given stack. That is a factual conversation, not a preference one.
- I am not the first person to have this argument and I will not be the last. The fact that it is hard does not mean I am wrong.

---

*These are working notes. They are not a position paper. The position paper is the Approach Paper, which says less and is intended for circulation.*
