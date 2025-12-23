# Architecture Rationale: Why Not BPMN/DMN/Spring?

## The Question That Will Be Asked

> "We have Camunda/jBPM. We have Spring. We have 15 years of Java talent. 
> Why are you building a custom DSL in Rust? This looks like NIH syndrome."

This document articulates why the traditional enterprise workflow approach 
fundamentally cannot address the problem space we're operating in.

---

## TL;DR

| Dimension | BPMN/DMN/Spring | ob-poc DSL |
|-----------|-----------------|------------|
| **Data semantics** | None - flow only | First-class citizen |
| **Path explosion** | O(n!) diagrams | O(1) grammar |
| **Configuration** | Code + XML + properties | YAML verbs + schema |
| **AI generation** | Cannot generate BPMN | Natural DSL target |
| **Long-running txn** | Process instance state | Event-sourced facts |
| **Multi-domain** | Separate processes | Unified execution |
| **Time to UAT** | 12-18 months | 3 months |

---

## 1. BPMN Has Nothing to Say About Data

BPMN models **control flow**. It answers: "What task comes next?"

It does NOT answer:
- What entities exist?
- What are their relationships?
- What attributes are required?
- How do we validate state transitions?
- How do we query current state?

**In BPMN, data is a second-class citizen.** You pass variables between tasks, 
but the schema, validation, and persistence are somebody else's problem.

**In ob-poc:**
```lisp
(cbu.create name:"Alpha Fund" jurisdiction:US) -> @fund
(entity.create-proper-person cbu-id:@fund first-name:"John" last-name:"Doe") -> @john
(cbu.assign-role cbu-id:@fund entity-id:@john role-type:ACCOUNT_HOLDER)
```

The DSL IS the data operation. Schema is in the verb definition. Validation is 
in the executor. Persistence is automatic. The operation and its data are atomic.

**BPMN equivalent:** Draw a task box, write Java service code, wire up JPA 
entities, configure validation, handle transactions manually, pray.

---

## 2. The Combinatorial Explosion Problem

### The Myth of the Happy Path

BPMN assumes you can draw "the process." But onboarding isn't one process.

Consider just KYC for a corporate client:
- 1-50 entities per CBU
- Each entity: Individual OR Corporate OR Trust OR Fund
- Each needs: Identity verification AND/OR Document collection AND/OR Screening
- Screening: PEP check AND Sanctions AND Adverse media
- Results: Pass OR Refer OR Fail
- Fail → Remediation → Re-screen
- Periodic review: 1yr OR 3yr OR 5yr cycles
- Trigger events: Material change, Risk upgrade, Regulatory request

**How many BPMN paths?** 

For a corporate with 10 entities, 3 UBOs, across 2 jurisdictions:
- Entity type combinations: 4^10 = 1,048,576
- Workflow variants per entity: ~20
- Cross-entity dependencies: n*(n-1)/2 = 45
- Total theoretical paths: **astronomical**

**You cannot draw this.** Any attempt produces either:
1. A single oversimplified diagram that lies
2. Hundreds of diagrams nobody maintains
3. A "dynamic subprocess" that's just code pretending to be BPMN

### The DSL Approach

Instead of drawing paths, we define:
1. **Entity types** with their attributes and valid states
2. **Operations** that transition states
3. **Dependencies** that constrain ordering
4. **A grammar** that composes operations

The paths emerge from execution, not from diagrams.

```yaml
# This YAML replaces hundreds of BPMN diagrams
stages:
  - code: KYC_REVIEW
    depends_on: [CLIENT_SETUP, PRODUCT_SELECTION]
    required_entities:
      - kyc_case
      - entity_workstream  # Per entity requiring KYC
    blocking: true
```

The system derives what's needed based on what exists. It doesn't follow a 
predetermined path—it navigates a state space.

---

## 3. Configuration Over Code

### The Spring/Java Pattern

```java
@Service
public class OnboardingService {
    @Autowired private CbuRepository cbuRepo;
    @Autowired private EntityRepository entityRepo;
    @Autowired private KycCaseRepository kycRepo;
    @Autowired private ProductService productService;
    @Autowired private ValidationService validator;
    @Autowired private AuditService audit;
    
    @Transactional
    public Cbu createCbu(CreateCbuRequest request) {
        validator.validate(request);
        Cbu cbu = new Cbu();
        cbu.setName(request.getName());
        cbu.setJurisdiction(request.getJurisdiction());
        // ... 50 more lines
        cbuRepo.save(cbu);
        audit.log("CBU_CREATED", cbu.getId());
        return cbu;
    }
    
    // Repeat for 200+ operations
}
```

**Problems:**
- Business logic buried in Java code
- Every change requires compile/deploy
- Testing requires Spring context
- Business users can't read it
- AI can't generate it reliably

### The DSL Pattern

```yaml
# config/verbs/cbu.yaml
create:
  description: "Create a new Client Business Unit"
  behavior: crud
  crud:
    operation: insert
    table: cbus
    schema: ob_cbu
  args:
    - name: name
      type: string
      required: true
      validation:
        min_length: 1
        max_length: 255
    - name: jurisdiction
      type: string
      required: true
      lookup:
        table: jurisdictions
        search_key: code
```

**Benefits:**
- Business logic is configuration
- Change YAML, restart, done
- Testable without framework
- Business users can review
- AI generates DSL naturally

---

## 4. AI Cannot Generate BPMN

Try this experiment:

> "Claude, generate a BPMN XML file for onboarding a corporate client 
> with KYC, product subscription, and trading setup."

You'll get:
- Syntactically invalid XML
- Missing namespace declarations  
- Incorrect element nesting
- No connection to actual system capabilities
- Hallucinated task names

Now try:

> "Generate DSL to onboard a corporate client called Acme Holdings"

```lisp
(cbu.create name:"Acme Holdings" jurisdiction:GB client-type:CORPORATE) -> @acme
(entity.create-limited-company cbu-id:@acme name:"Acme Holdings Ltd" 
  registration-number:"12345678") -> @acme-entity
(cbu.assign-role cbu-id:@acme entity-id:@acme-entity role-type:ACCOUNT_HOLDER)
(cbu.add-product cbu-id:@acme product:CUSTODY)
(kyc-case.create cbu-id:@acme case-type:NEW_CLIENT) -> @kyc
```

**Why the difference?**

1. **DSL is text** - LLMs are trained on text
2. **DSL has grammar** - Constraints guide generation
3. **DSL maps to domain** - Verbs match business operations
4. **DSL is composable** - Statements are independent units
5. **DSL is validatable** - Parser catches errors immediately

BPMN is a visual notation that happens to have XML serialization. 
It was designed for humans drawing boxes, not machines generating text.

---

## 5. Long-Running Transactions

### The BPMN Model

BPMN thinks in "process instances":
- Start event creates instance
- Instance has state (current task, variables)
- Instance persists in engine database
- Completion fires end event

**Problem:** Onboarding isn't a process instance. It's a **fact accumulation**.

A CBU isn't "in the KYC task." A CBU **has or doesn't have** a KYC case. 
The case **has or doesn't have** approved workstreams. These are facts, not states.

### The Event-Sourced Model

```
Time    Event                           State After
─────   ─────                           ───────────
T1      CBU_CREATED(Alpha Fund)         {cbu: exists}
T2      PRODUCT_ADDED(CUSTODY)          {cbu, products: [CUSTODY]}
T3      KYC_CASE_CREATED(NEW_CLIENT)    {cbu, products, kyc_case: INTAKE}
T4      WORKSTREAM_CREATED(entity-1)    {cbu, products, kyc_case, ws: 1}
T5      WORKSTREAM_APPROVED(entity-1)   {cbu, products, kyc_case, ws: 1✓}
T6      KYC_CASE_APPROVED               {cbu, products, kyc_case: APPROVED}
```

**The "process state" is derived from facts, not stored separately.**

Benefits:
- Full audit trail by construction
- Time-travel queries (state at T3?)
- No process instance corruption
- Resume from any point
- Multiple concurrent "processes" on same CBU

---

## 6. Multi-Domain Unification

### The BPMN Reality

In a BPMN shop, you end up with:
- `onboarding-process.bpmn`
- `kyc-process.bpmn`
- `trading-setup-process.bpmn`
- `document-collection-process.bpmn`
- `periodic-review-process.bpmn`

Each owned by different teams. Each with its own:
- Variable naming conventions
- Error handling patterns
- Integration points
- Testing approach

**Coordination becomes the problem.** You need:
- Process orchestration layer
- Message correlation
- Compensation handlers
- State synchronization

### The Unified DSL

One grammar. One executor. One transaction boundary.

```lisp
; This is ONE execution, not four process instances
(cbu.create name:"Fund" jurisdiction:US) -> @fund
(cbu.add-product cbu-id:@fund product:CUSTODY)
(kyc-case.create cbu-id:@fund case-type:NEW_CLIENT) -> @kyc
(trading-profile.import cbu-id:@fund profile-path:"default.yaml")
```

Cross-domain operations compose naturally:
- Symbols flow across domains (@fund used everywhere)
- Transaction is atomic
- Rollback is coherent
- Audit is unified

---

## 7. The 3-Month Claim

### What We Have (Today)

| Component | Status | LOC |
|-----------|--------|-----|
| DSL Parser (nom) | ✅ Complete | ~2,000 |
| Verb Registry | ✅ Complete | ~1,500 |
| Generic CRUD Executor | ✅ Complete | ~3,000 |
| Custom Operations | ✅ Complete | ~5,000 |
| Entity Gateway (gRPC) | ✅ Complete | ~2,000 |
| Semantic Stage System | ✅ Complete | ~2,500 |
| Agent Integration | ✅ Complete | ~4,000 |
| UI (egui/WASM) | ✅ Working | ~6,000 |
| Database Schema | ✅ 90+ tables | ~3,000 |

**Total: ~29,000 LOC Rust, working end-to-end**

### What UAT Needs

1. **Production hardening** - Error handling, logging, monitoring
2. **Security** - AuthN/AuthZ integration
3. **Performance** - Connection pooling, caching
4. **Deployment** - Docker, K8s manifests
5. **Testing** - Integration test suite
6. **Documentation** - Runbooks (in progress)

### Why 3 Months Is Realistic

| Task | Effort | Parallel? |
|------|--------|-----------|
| Security integration | 3 weeks | Yes |
| Performance tuning | 2 weeks | Yes |
| Error handling cleanup | 2 weeks | Yes |
| Integration tests | 4 weeks | Yes |
| Deployment automation | 2 weeks | No |
| Documentation | 3 weeks | Yes |
| UAT support buffer | 2 weeks | - |

With 3 devs working in parallel: **10-12 weeks calendar time**

### Why BPMN Would Take 12-18 Months

1. **Process modeling** - 2-3 months just drawing diagrams
2. **Service layer** - 3-4 months coding Java services
3. **Integration** - 2-3 months wiring everything together
4. **Testing** - 2-3 months (Spring context startup alone...)
5. **Process debugging** - 2-3 months fixing the inevitable issues
6. **Performance** - 1-2 months because Camunda isn't fast

And you'd STILL have:
- No AI integration
- No unified data model
- No configuration-driven verbs
- No semantic journey tracking

---

## 8. The Real Risk Discussion

### Risks of the DSL Approach

| Risk | Mitigation |
|------|------------|
| Rust talent scarcity | Go fallback for services; Rust core is stable |
| Custom = maintenance burden | YAML config reduces code churn |
| No vendor support | Anthropic partnership; internal expertise |
| "Not invented here" perception | This document; working demo |

### Risks of BPMN/Spring Approach

| Risk | Mitigation |
|------|------------|
| 12-18 month timeline | None - it's structural |
| Combinatorial explosion | None - fundamental limitation |
| No AI integration path | Major rework required |
| Data model fragmentation | Heroic integration effort |
| Vendor lock-in (Camunda) | Expensive migration |

---

## 9. What We're NOT Saying

We're not saying:
- ❌ BPMN is bad (it's good for simple, linear processes)
- ❌ Spring is bad (it's excellent for web services)
- ❌ Java is bad (it's a fine language)
- ❌ DMN is bad (it's good for isolated decision tables)

We ARE saying:
- ✅ This problem space doesn't fit BPMN's model
- ✅ Configuration-driven > code-driven for this domain
- ✅ AI-native design is a strategic advantage
- ✅ The DSL approach is working, today, with evidence

---

## 10. The Demo

The best argument is a working system.

```
"Show me onboarding a corporate client with 3 UBOs, 
custody product, full KYC, and trading setup."

> 45 seconds with ob-poc DSL
> Draw me the BPMN diagram for that... [silence]
```

---

## Appendix: Common Objections

### "We have existing BPMN expertise"

Expertise in a tool that doesn't fit the problem is not an asset.
We have DSL expertise now. It took 3 months to build.

### "Camunda has AI features now"

Camunda's AI helps you DRAW diagrams faster.
It doesn't help you EXECUTE operations or DERIVE state.

### "What about regulatory compliance?"

The DSL produces complete audit trails. Every operation is logged.
BPMN process instances are actually harder to audit across boundaries.

### "This is too risky for production"

The core is ~29K LOC of Rust with strong typing.
A Spring equivalent would be 100K+ LOC of Java with runtime reflection.
Which is riskier?

### "What if you leave?"

The YAML configs are self-documenting.
The Rust code is straightforward CRUD + graph traversal.
Any competent team can maintain it.
