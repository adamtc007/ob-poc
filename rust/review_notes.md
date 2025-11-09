# DSL Manager and Engine Service Review Notes

## Session 1: Initial Analysis & API Discovery

This document tracks the progress and findings of the code review for the `dsl_manager.rs` and related components. The primary goal is to ensure the system is secure, robust, and that all state changes occur through well-defined, intentional gateways.

### Summary of Findings

1.  **Initial Target: `dsl_manager.rs`**:
    *   We began by analyzing `dsl_manager.rs`, which was described as the gateway for all DSL state changes.
    *   A review of its public API revealed several state-changing methods: `create_dsl_instance`, `edit_dsl_instance`, `create_kyc_case`, `create_onboarding_request`, and various methods for incremental onboarding edits (e.g., `add_products`).
    *   **Initial Concerns**:
        *   Public fields in core data structures (`DslInstance`, `DslInstanceVersion`) allow for direct modification, bypassing `DslManager`'s logic.
        *   Database operations are not wrapped in transactions, posing a risk of inconsistent state.
        *   A simplified, string-based template engine could be vulnerable to injection.
        *   Several database retrieval and storage methods are noted as `TODO` or are placeholders, indicating incomplete implementation.

2.  **API Gateway Discovery: `dsl_engine_service.proto`**:
    *   A hint led us to discover the gRPC service definition in `dsl_engine_service.proto`.
    *   This revealed that `DslManager` is **not** the primary external gateway. The `DSLEngineService` is the true public-facing API.
    *   The `DSLEngineService` API is abstract and focuses on processing workflows (`ProcessWorkflow`, `ParseAndExecute`) rather than direct lifecycle management of DSL instances.
    *   This is a strong architectural choice, as it hides implementation details and prevents direct, potentially unsafe, manipulation of DSL state from external clients.

### Next Steps: "Middle-Out" Analysis

Our next goal is to trace the logic from the middle layer—the specific business operations that incrementally change DSL state—outwards to the gRPC API and inwards to the database persistence layer.

#### 1. Trace Template-Driven Incremental Adds

We will focus on the suite of functions in `DslManager` that handle specific onboarding state changes. These appear to be the core of the "middle" layer.

*   **Target Functions**:
    *   `associate_cbu`
    *   `add_products`
    *   `discover_services`
    *   `discover_resources`
    *   `complete_onboarding`
    *   `archive_onboarding`
*   **Analysis Steps**:
    1.  How do these functions retrieve the previous DSL state? (They all call `get_latest_ob_version_dsl`).
    2.  How is the new DSL fragment constructed? (They use simple `format!` macros).
    3.  How is the new state persisted? (They all delegate to the `persist_ob_edit` method).

#### 2. Analyze Core State Change Logic

The `persist_ob_edit` method appears to be a critical choke point for all incremental edits. We need to analyze its implementation in detail.

*   **Target Function**: `persist_ob_edit`
*   **Analysis Steps**:
    1.  **Validation**: It starts by parsing the combined DSL. This is a key validation step.
    2.  **Versioning**: It creates a `NewDslVersion` and calls `domain_repository.create_new_version`. We need to trace this database interaction.
    3.  **Auditing/Journaling**: It calls `store_dsl_source_with_ob_index` to persist the source to the `dsl_ob` table.
    4.  **Compilation & AST Storage**: It calls `compile_and_store_ast`, which handles parsing, serializing the AST, and calling `store_ast_with_fk_links`.
    5.  **Instance Update**: It calls `update_instance_version` to update the compilation status.

#### 3. Investigate Database Interaction and Incomplete Logic

The final step is to trace the logic to the database and identify the impact of the incomplete functions.

*   **Target Functions (in `DslManager` and `DslDomainRepository`)**:
    *   `get_latest_ob_version_dsl` (DB query)
    *   `domain_repository.create_new_version` (DB insert)
    *   `store_dsl_source_with_ob_index` (DB insert)
    *   `store_ast_with_fk_links` (DB insert)
*   **Identify Incomplete Logic**:
    *   `get_latest_instance_version`
    *   `get_all_instance_versions`
    *   `get_version_by_id`
    *   `get_instance`
    *   `store_instance`
    *   `update_instance`
    *   `store_instance_version`
*   **Analysis Questions**:
    *   What is the exact database schema being used for `dsl_versions`, `dsl_ob`, and `parsed_asts`?
    *   How does the incomplete logic for `DslInstance` and `DslInstanceVersion` affect the overall system? It appears the system currently relies on the `dsl_versions` table from the repository and doesn't use the `DslInstance` concept for persistence yet. This is a major gap.

### Open Questions

*   Where is the server-side implementation of the `DSLEngineService` trait? We need to find this to trace the gRPC entry point inwards to the `DslManager`.