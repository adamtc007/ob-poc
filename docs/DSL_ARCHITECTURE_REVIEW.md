# OB-POC DSL: Architectural Review & Agentic Capabilities

## 1. Executive Summary

The `ob-poc` Domain Specific Language (DSL) is a formal, declarative system designed to orchestrate complex financial onboarding (KYC/AML) workflows. Unlike traditional imperative scripting, it serves as a **data-driven configuration layer** that separates business intent from technical execution.

For AI Agents, this DSL provides a **safe, deterministic sandbox**. Instead of generating raw SQL or Rust code, agents generate high-level DSL instructions that are validated against a strict grammar and context-sensitive rules before execution.

## 2. DSL Syntax and Semantics

The DSL utilizes a Lisp-like **S-expression** syntax, chosen for its homoiconicity (code is data), which simplifies parsing and AST manipulation by AI models.

### 2.1 Syntax Structure
The general form of a command is:
```clojure
(domain.verb :arg1 value1 :arg2 value2 ... :as @binding)
```
*   **Domain**: Namespaces the operation (e.g., `cbu`, `entity`, `ubo`).
*   **Verb**: The specific action (e.g., `ensure`, `assign-role`, `catalog`).
*   **Arguments**: Key-value pairs mapped to domain logic or database columns.
*   **Bindings (`:as @var`)**: Captures the result (e.g., a UUID) for use in subsequent steps, enabling dependency chaining within a single script.

### 2.2 Semantic Architecture
The DSL is not hardcoded; it is **configuration-defined**:
*   **`verbs.yaml`**: The "standard library" of the language. It defines every valid operation, its required arguments, type validation, and mapping to the underlying PostgreSQL schema.
*   **`csg_rules.yaml`**: Defines the "laws" of the universe (Context Sensitive Grammar). It prevents invalid business states (e.g., "You cannot assign a DIRECTOR role to a Trust").

## 3. Agentic Integration & Deterministic Outcomes

The DSL is specifically architected to enable **Agentic AI** to operate safely within a regulated environment.

### 3.1 Formal Language Constraint
*   **Restricted Action Space**: An agent cannot "hallucinate" arbitrary code. It can only construct sentences using the vocabulary defined in `verbs.yaml`.
*   **Syntactic Correctness**: The EBNF grammar ensures that all generated scripts are structurally sound before they are even considered for execution.

### 3.2 Context-Sensitive Guardrails (CSG)
The system implements a **CSG Linter** that runs *before* execution. This allows an agent to "dry run" its plan.
*   **Pre-conditions**: "To create a `share-class`, the CBU must be a `FUND`."
*   **Type Safety**: "A `PASSPORT` document can only be attached to a `PROPER_PERSON`."
*   **Deterministic Error Handling**: If an agent proposes an invalid action, the Linter returns a structured error explaining exactly *why* (e.g., `CSG-C001: passport_requires_person`). The agent can then self-correct and retry.

### 3.3 State Machine Awareness
The DSL explicitly models state transitions (defined in `csg_rules.yaml`). An agent does not need to guess the next step; it can query the allowed transitions for a `kyc_case` (e.g., `INTAKE` -> `DISCOVERY`). This makes the agent's behavior predictable and auditable.

## 4. Attribute & Document Integration

A key advantage of this DSL is that **Evidence** and **Data** are treated as first-class syntactic elements, not external attachments.

### 4.1 Document-to-Data Lineage
In traditional systems, a document is just a blob. In `ob-poc`, the DSL captures the *extraction* process:
1.  `(document.catalog ...)`: Registers the physical file.
2.  `(document.extract-to-observations ...)`: Runs OCR/AI extraction.
3.  `(observation.record ...)`: structured data points derived from the doc.

### 4.2 The "Evidence" Advantage
*   **Traceability**: Every piece of data in the KYC profile can be traced back to the specific DSL command that created it, and the document source it came from.
*   **Conflict Resolution**: The DSL supports `allegations` (what the client said) vs. `observations` (what the document says). Agents can generate DSL scripts to explicitly `resolve-conflict` between these two, creating a verified "Golden Record."

## 5. Verb Catalog Overview

The verbs are organized by **Domain**, representing different facets of the onboarding lifecycle.

| Domain | Focus | Key Verbs |
| :--- | :--- | :--- |
| **CBU** | Client Business Unit (The "Case") | `ensure`, `update`, `assign-role`, `set-status` |
| **Entity** | Legal/Natural Persons | `create-limited-company`, `create-proper-person`, `read` |
| **KYC Case** | Workflow Management | `create`, `update-status`, `escalate` |
| **UBO** | Ownership & Control | `discover-owner`, `register-ubo`, `infer-chain`, `snapshot-cbu` |
| **Document** | Evidence Handling | `catalog`, `extract`, `extract-to-observations` |
| **Screening** | Risk Checks | `pep`, `sanctions`, `adverse-media` |
| **Custody** | Settlement Instructions | `create-ssi`, `add-booking-rule`, `add-universe` |

## 6. KYC DSL Deep Dive

The KYC logic is the most sophisticated part of the system, centering on the **Client Business Unit (CBU)**.

### 6.1 CBU as Data State
The `CBU` is the root aggregate. It is not just a "customer ID"; it is a container for:
*   **Entity Graph**: The legal entities involved (Fund, HoldCo, Directors, UBOs).
*   **Role Map**: How those entities relate to the client (`BENEFICIAL_OWNER`, `CUSTODIAN`, `INVESTMENT_MANAGER`).
*   **Evidence Store**: All documents and verifications linked to the CBU.

### 6.2 Incremental Editing & Evolution
The DSL is designed for **incremental enrichment**. A KYC case is not built in one shot; it evolves:

1.  **Phase 1: Intake (Skeleton)**
    *   Agent receives an email.
    *   Generates: `(cbu.ensure ...)` and `(kyc-case.create ...)`.
    *   Result: A hollow CBU container.

2.  **Phase 2: Discovery (Graph Expansion)**
    *   Agent reads a corporate registry.
    *   Generates: `(entity.create-limited-company ...)` for a parent company.
    *   Generates: `(cbu.assign-role ...)` to link it to the CBU.
    *   Generates: `(ubo.discover-owner ...)` to record a potential UBO.

3.  **Phase 3: Evidence & Validation (Hardening)**
    *   Agent requests documents.
    *   Generates: `(document.catalog ...)` when files arrive.
    *   Generates: `(ubo.verify-ubo ...)` once evidence confirms ownership.

4.  **Phase 4: Approval (Snapshot)**
    *   Agent finalizes the case.
    *   Generates: `(ubo.snapshot-cbu ...)` to freeze the state for regulatory audit.
    *   Generates: `(kyc-case.update-status :status "APPROVED")`.

### 6.3 The "Diff" Capability
Because the entire state is built via DSL commands, the system can easily handle **re-onboarding**. If a client returns a year later, the agent can:
1.  Load the previous "Snapshot".
2.  Compare it with current registry data.
3.  Generate a *delta script* containing only the necessary updates (e.g., `(cbu.update-role ...)`), ensuring minimal work and exact auditability of what changed.
