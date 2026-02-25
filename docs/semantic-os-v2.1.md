# Semantic Operating System
**Registry · Context · Control Plane · Security · Governance · Derived Semantics for Agentic Operations**

Version: **2.1** — February 2026  
Status: **Draft — Product Vision & Capability Scope (implementation-agnostic)**  
Audience: **Agile Product · Data Governance · Architecture · Engineering**

---

## 0. Executive Summary

Enterprise onboarding, KYC/AML and client lifecycle operations are increasingly delivered through **AI-assisted workflows**: chat-based journeys, agentic task execution, and automation that must be **safe, auditable, and policy-correct**.

A “traditional data dictionary” (a list of fields and descriptions) is not enough for that world.

The **Semantic Operating System (Semantic OS)** is the platform capability that makes AI usable at scale by providing a **single, queryable, versioned semantic substrate** that answers—deterministically—questions like:

- **What exists?** (entities, attributes, documents, evidence, states)
- **What does it mean?** (types, constraints, derived meanings, lineage)
- **What is allowed?** (policy, purpose limitation, residency/jurisdiction, export controls)
- **What can be executed?** (governed operation contracts / “verbs”)
- **How do we prove it?** (snapshot pinning, evidence manifests, negative evidence, decision records)

**Outcome:** AI can safely *discover, reason, plan, and act* inside clear boundaries—while the business gets governance, auditability, and repeatability.

---

## 1. Vision

The platform needs a **Semantic OS**: a living “semantic substrate” that sits beneath onboarding and compliance workflows and enables:

- **Discovery:** find entities, concepts, sources, documents, and required evidence
- **Understanding:** machine-readable definitions (types, constraints, meaning, derivations)
- **Security & governance by design:** classifications, permissions, and policy outcomes are computable
- **Deterministic automation:** governed operations with preconditions and postconditions
- **Explainability:** decisions are reconstructable at a point in time (what was known, what was missing, what rules applied)

This is foundational infrastructure—like an operating system—because it standardises how the platform *describes* and *controls* meaning.

---

## 2. Why this is foundational for “AI everywhere”

AI can only be reliably deployed across regulated operations if three conditions hold:

### 2.1 Agents require deterministic boundaries
An LLM can assist with research and drafting, but **execution must be constrained** by machine-checkable contracts:
- allowed operation surface (“verbs”)
- input types and constraints
- preconditions, policy gates, and postconditions
- audit trails and evidence requirements

Without this, AI either:
- cannot act (too risky), or
- acts inconsistently (high operational and regulatory risk).

### 2.2 AI needs typed semantics, not strings
If the platform’s semantics are “strings in PDFs”:
- agents hallucinate field meaning
- integrations drift
- forms and validations become brittle
- audit trails are weak

The Semantic OS makes semantics **typed, queryable, and versioned**.

### 2.3 Embeddings are useful—but must not become “truth”
Embeddings/vector search help with:
- discovery (“find the right attribute/verb/policy”)
- similarity matching and fuzzy lookup
- conversational assistance

But they are **derived projections**, not authoritative meaning.

Semantic OS defines the authoritative substrate; embeddings are safe, disposable read models built on top.

---

## 3. What this is (and isn’t)

### 3.1 What this IS
A platform capability that provides:
- a **semantic registry** (attributes, entities, relationships, verbs, policies, evidence)
- **context resolution** (what applies, what is permitted, what is relevant *right now*)
- **governance tiers** that allow progress without sacrificing proof
- **security labels** and policy enforcement that are computable
- **immutable snapshots** so decisions are provable at a point in time
- **derived projections** (including embeddings) that accelerate discovery and UX

### 3.2 What this is NOT
- Not “just a data dictionary” (descriptions only)
- Not a standalone catalog product running alongside the platform
- Not an LLM feature in itself

It **can integrate with** enterprise catalog/governance tooling where required, but its defining property is that it is **aligned to what the platform actually enforces at runtime**.

---

## 4. Foundational principle: Immutable snapshots

Semantic meaning must be reconstructable.

### 4.1 Principle
All governed semantic state (definitions, policies, contracts) is published as **immutable snapshots**. Every decision and execution can pin to a snapshot set.

### 4.2 Why it matters (product outcomes)
- **Auditability:** “what did we know and enforce on date X?”
- **Safe change:** new semantics don’t silently rewrite historical decisions
- **Deterministic AI:** the agent can cite the semantic basis of its plan

---

## 5. Foundational principle: Governed vs Operational semantics

We need speed *and* proof.

### 5.1 The problem this solves
Onboarding work starts messy: partial data, uncertain sources, missing documents. If governance requires full certainty before progress, work stalls.

### 5.2 The model
Two tiers of semantic content:

- **Governed semantics (“above the line”)**
  - reviewed, publishable, stable
  - used for deterministic automation and compliance-grade outputs

- **Operational semantics (“below the line”)**
  - useful, early, imperfect
  - supports progress, research, triage, and human-in-the-loop work

### 5.3 The Proof Rule
A governed outcome must be supported by:
- **positive evidence** (what proves a claim)
- and, where relevant, **negative evidence** (what we tried and did not find)

Operational content is still secured and audited; it is simply not treated as proof-grade.

### 5.4 Promotion path
Operational → Governed is an explicit workflow (review, validation, publish), not an informal overwrite.

---

## 6. Cross-cutting: Data classification & security

Semantic OS must make security computable.

### 6.1 Security dimensions (examples)
- confidentiality (public/internal/restricted)
- privacy classification (PII, sensitive PII)
- residency / jurisdiction constraints
- purpose limitation
- export controls / no-external-LLM constraints
- masking / redaction requirements

### 6.2 Outcome
Every semantic object (attribute, document type, verb contract, derived view) carries a security posture that can be evaluated for:
- **who may see it**
- **who may act on it**
- **whether it may be exported**
- **whether it may be used in an embedding or external model**

---

## 7. Universal contract: Context Resolution

Agents and UIs need one reliable way to ask: “what applies now?”

### 7.1 What context resolution provides
Given a context (user role, case type, entity kind, jurisdiction, workflow phase), it returns:
- allowed verbs/operations (and why)
- relevant attributes (and their security constraints)
- required evidence/documents (and current gaps)
- applicable policies (and their effect)

### 7.2 Why this matters
This is the bridge between:
- semantic substrate (truth)
- runtime execution (deterministic action)
- agentic planning (safe autonomy)

---

## 8. Capability catalogue (product-facing)

This is the “what we ship” scope.

### 8.1 Attribute Dictionary
**What:** a versioned dictionary of attributes with:
- meaning, type, constraints, examples
- classifications/security labels
- derivation rules (where applicable)
- lineage to sources/evidence (where applicable)

**Why for AI:** prevents hallucinated meaning; enables typed extraction, validation, and form generation.

### 8.2 Entity & Relationship Model
**What:** canonical entity kinds and relationships (e.g., org/person/fund/account), including ownership/UBO edges and role semantics.

**Why for AI:** constrains linking and reasoning; enables safe graph navigation and gap detection.

### 8.3 Verb Dictionary (Executable Contracts)
**What:** a governed catalogue of operations the platform can perform, with contracts:
- inputs/outputs, constraints, preconditions/postconditions
- security/policy effects
- durable continuations (where human/external steps exist)

**Why for AI:** provides the deterministic “action surface” so the agent can execute safely.

### 8.4 Policy & Controls Registry
**What:** machine-evaluable rules that drive:
- access decisions (ABAC)
- workflow constraints (what must be true before progressing)
- evidence requirements (what counts as proof)
- export/embedding constraints

**Why for AI:** makes “compliance-aware planning” possible without embedding policy text in prompts.

### 8.5 Source & Evidence Registry
**What:** a structured view of sources (internal/external), evidence items, and their quality/trust posture.

**Why for AI:** lets the agent reason about trust, provenance, and “what’s missing” in a provable way.

### 8.6 Taxonomy Registry
**What:** controlled vocabularies and domain taxonomies (jurisdictions, document families, product/service/resource, etc.).

**Why for AI:** improves disambiguation, routing, and intent narrowing without overfitting to free-text.

### 8.7 View Definitions (read models)
**What:** reusable governed “views” that package semantics for:
- UIs
- APIs
- reports
- agent planning (“what to look at next”)

**Why for AI:** gives curated “what matters” representations (reduces prompt bloat and confusion).

### 8.8 Derived projections (including embeddings)
**What:** disposable derived indexes/projections built from the governed substrate:
- lineage graphs
- coverage metrics
- vector embeddings for discovery

**Why for AI:** enables fast semantic search and similarity matching—without promoting embeddings to truth.

**Principle:** derived projections are replaceable; the governed substrate remains authoritative.

---

## 9. Operating model (who owns what)

To keep this non-contentious and adoptable, responsibility is explicit:

- **Product**: owns scope, prioritisation, capability roadmap, adoption targets
- **Data Governance / Stewardship**: owns governed semantics, classifications, review cadence, proof rules
- **Engineering**: owns runtime enforcement, publish discipline, integration, performance
- **Risk/Compliance partners**: consult on policy semantics and audit requirements

This is not “governance slowing delivery”; it is **governance enabling safe automation**.

---

## 10. Adoption approach (incremental, low drama)

Start small, expand with value:

1) **Phase 1 — Foundation**
   - Attribute Dictionary + Security Labels + Context Resolution
2) **Phase 2 — Deterministic action**
   - Verb Dictionary (contracts) + basic policy gating
3) **Phase 3 — Proof-grade automation**
   - Evidence registry + proof rules + snapshot pinning in decisions
4) **Phase 4 — Scale-out AI enablement**
   - Derived projections, embeddings, coverage metrics, UX accelerators

---

## 11. Success measures (what “good” looks like)

- Agents can plan and act **only within governed verbs**
- Every action is explainable: “why allowed”, “what evidence”, “which snapshot”
- Data governance can answer: “what do we know”, “what is trusted”, “what is missing”
- Embeddings improve discovery **without** becoming a source of truth
- Change is safe: semantic updates don’t silently rewrite history

---

## 12. Summary

Semantic OS replaces the idea of a “static dictionary” with a **runtime-aligned semantic substrate** that enables:

- scalable AI assistance (discovery + planning)
- safe automation (deterministic execution surface)
- governance with velocity (operational vs governed tiers)
- defensible outcomes (snapshots + evidence)

It is foundational because **AI everywhere** requires semantics, security, and contracts that are **machine-readable and enforceable**, not just documented.

---