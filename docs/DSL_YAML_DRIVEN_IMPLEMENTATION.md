# DSL YAML-Driven Configuration Implementation Plan

**Document**: Complete specification for Claude Code implementation  
**Goal**: Transform DSL from hardcoded Rust static arrays to YAML-driven configuration  
**Result**: Adding new verbs/domains requires editing YAML files, not Rust code

---

## Table of Contents

1. [Architecture Overview](#part-1-architecture-overview)
2. [YAML Configuration Formats](#part-2-yaml-configuration-formats)
3. [Rust Configuration Types](#part-3-rust-configuration-types)
4. [Runtime Verb Registry](#part-4-runtime-verb-registry)
5. [Generic CRUD Executor](#part-5-generic-crud-executor)
6. [Integration Points](#part-6-integration-points)
7. [Migration Strategy](#part-7-migration-strategy)
8. [Testing Checklist](#part-8-testing-checklist)

---

## Part 1: Architecture Overview

### Current Architecture (Hardcoded)

```
COMPILE TIME
     │
     ▼
┌─────────────────────────────────────────────────────────┐
│  verbs.rs          verb_schema.rs      verb_registry.rs │
│  STANDARD_VERBS    ALL_VERB_SCHEMAS    custom_ops_def() │
│  (static arrays)   (static arrays)     (static fn)      │
└─────────────────────────────────────────────────────────┘
     │
     ▼
  OnceLock<UnifiedVerbRegistry>
     │
     ▼
  Parser → Compiler → Executor
```

### Target Architecture (YAML-Driven)

```
RUNTIME (Server Startup)
     │
     ▼
┌─────────────────────────────────────────────────────────┐
│  config/verbs.yaml     config/csg_rules.yaml            │
│  (all verb defs)       (validation rules)               │
└─────────────────────────────────────────────────────────┘
     │
     ▼
  ConfigLoader (serde_yaml)
     │
     ▼
  RuntimeVerbRegistry (Arc<RwLock<...>>)
     │
     ▼
  Parser → Compiler → GenericExecutor
     │
     ▼
  (Optional) Hot-reload endpoint: POST /admin/reload-config
```

### File Structure

```
rust/
├── config/                          # NEW: Configuration files
│   ├── verbs.yaml                   # Verb definitions by domain
│   └── csg_rules.yaml               # CSG validation rules
│
├── src/
│   └── dsl_v2/
│       ├── config/                  # NEW: Config loading module
│       │   ├── mod.rs
│       │   ├── types.rs             # Config structs (serde)
│       │   ├── loader.rs            # YAML loading + validation
│       │   └── hot_reload.rs        # Optional hot-reload support
│       │
│       ├── runtime_registry.rs      # NEW: Runtime verb registry
│       ├── generic_executor.rs      # NEW: Config-driven CRUD executor
│       │
│       ├── verbs.rs                 # KEEP temporarily, then remove
│       ├── verb_registry.rs         # MODIFY: Delegate to runtime
│       └── executor.rs              # MODIFY: Use generic executor
```

---

## Part 2: YAML Configuration Formats

### File: `rust/config/verbs.yaml`

```yaml
# DSL Verb Configuration
# Version: 1.0
#
# Adding a new verb:
# 1. Find or create the domain section
# 2. Add verb definition with args and behavior
# 3. Restart server (or call reload endpoint)

version: "1.0"

# =============================================================================
# DOMAIN: cbu (Client Business Unit)
# =============================================================================
domains:
  cbu:
    description: "Client Business Unit operations"

    verbs:
      create:
        description: "Create a new Client Business Unit"
        behavior: crud
        crud:
          operation: insert
          table: cbus
          schema: ob-poc
          returning: cbu_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
          - name: client-type
            type: string
            required: false
            maps_to: client_type
          - name: nature-purpose
            type: string
            required: false
            maps_to: nature_purpose
          - name: description
            type: string
            required: false
            maps_to: description
        returns:
          type: uuid
          name: cbu_id
          capture: true

      read:
        description: "Read a CBU by ID"
        behavior: crud
        crud:
          operation: select
          table: cbus
          schema: ob-poc
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
        returns:
          type: record

      update:
        description: "Update a CBU"
        behavior: crud
        crud:
          operation: update
          table: cbus
          schema: ob-poc
          key: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: name
            type: string
            required: false
            maps_to: name
          - name: status
            type: string
            required: false
            maps_to: status
          - name: client-type
            type: string
            required: false
            maps_to: client_type
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
        returns:
          type: affected

      delete:
        description: "Delete a CBU"
        behavior: crud
        crud:
          operation: delete
          table: cbus
          schema: ob-poc
          key: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
        returns:
          type: affected

      list:
        description: "List CBUs with optional filters"
        behavior: crud
        crud:
          operation: select
          table: cbus
          schema: ob-poc
        args:
          - name: status
            type: string
            required: false
            maps_to: status
          - name: client-type
            type: string
            required: false
            maps_to: client_type
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
          - name: limit
            type: integer
            required: false
            default: 100
          - name: offset
            type: integer
            required: false
            default: 0
        returns:
          type: record_set

      ensure:
        description: "Create or update a CBU by natural key"
        behavior: crud
        crud:
          operation: upsert
          table: cbus
          schema: ob-poc
          conflict_keys: [name, jurisdiction]
          returning: cbu_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
          - name: client-type
            type: string
            required: false
            maps_to: client_type
          - name: nature-purpose
            type: string
            required: false
            maps_to: nature_purpose
        returns:
          type: uuid
          name: cbu_id
          capture: true

      assign-role:
        description: "Assign a role to an entity within a CBU"
        behavior: crud
        crud:
          operation: role_link
          junction: cbu_entity_roles
          schema: ob-poc
          from_col: cbu_id
          to_col: entity_id
          role_table: roles
          role_col: role_id
          returning: cbu_entity_role_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
          - name: role
            type: lookup
            required: true
            lookup:
              table: roles
              code_column: name
              id_column: role_id
          - name: ownership-percentage
            type: decimal
            required: false
            maps_to: ownership_percentage
        returns:
          type: uuid
          name: cbu_entity_role_id
          capture: false

      remove-role:
        description: "Remove a specific role from an entity within a CBU"
        behavior: crud
        crud:
          operation: role_unlink
          junction: cbu_entity_roles
          schema: ob-poc
          from_col: cbu_id
          to_col: entity_id
          role_table: roles
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: role
            type: lookup
            required: true
            lookup:
              table: roles
              code_column: name
              id_column: role_id
        returns:
          type: affected

      parties:
        description: "List all parties (entities with their roles) for a CBU"
        behavior: crud
        crud:
          operation: list_parties
          junction: cbu_entity_roles
          schema: ob-poc
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: record_set

  # ===========================================================================
  # DOMAIN: entity
  # ===========================================================================
  entity:
    description: "Entity management operations"

    # Dynamic verb generation from entity_types table
    dynamic_verbs:
      - pattern: "create-{type_code}"
        source:
          table: entity_types
          schema: ob-poc
          code_column: type_code
          name_column: name
          transform: kebab_case
        behavior: crud
        crud:
          operation: entity_create
          base_table: entities
          schema: ob-poc
          extension_table_column: table_name
          type_id_column: entity_type_id
          returning: entity_id
        base_args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction

    verbs:
      read:
        description: "Read an entity by ID"
        behavior: crud
        crud:
          operation: select
          table: entities
          schema: ob-poc
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
        returns:
          type: record

      update:
        description: "Update an entity's base fields"
        behavior: crud
        crud:
          operation: update
          table: entities
          schema: ob-poc
          key: entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
          - name: name
            type: string
            required: false
            maps_to: name
          - name: status
            type: string
            required: false
            maps_to: status
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
        returns:
          type: affected

      delete:
        description: "Delete an entity (cascades to type extension)"
        behavior: crud
        crud:
          operation: delete
          table: entities
          schema: ob-poc
          key: entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
        returns:
          type: affected

      list:
        description: "List entities with optional filters"
        behavior: crud
        crud:
          operation: select
          table: entities
          schema: ob-poc
        args:
          - name: entity-type
            type: string
            required: false
          - name: jurisdiction
            type: string
            required: false
            maps_to: jurisdiction
          - name: status
            type: string
            required: false
            maps_to: status
          - name: limit
            type: integer
            required: false
            default: 100
          - name: offset
            type: integer
            required: false
            default: 0
        returns:
          type: record_set

  # ===========================================================================
  # DOMAIN: product
  # ===========================================================================
  product:
    description: "Product catalog operations"

    verbs:
      create:
        description: "Create a new product in the catalog"
        behavior: crud
        crud:
          operation: insert
          table: products
          schema: ob-poc
          returning: product_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: product-code
            type: string
            required: true
            maps_to: product_code
          - name: description
            type: string
            required: false
            maps_to: description
          - name: product-category
            type: string
            required: false
            maps_to: product_category
          - name: regulatory-framework
            type: string
            required: false
            maps_to: regulatory_framework
          - name: min-asset-requirement
            type: decimal
            required: false
            maps_to: min_asset_requirement
          - name: metadata
            type: json
            required: false
            maps_to: metadata
        returns:
          type: uuid
          name: product_id
          capture: true

      read:
        description: "Read a product by ID or code"
        behavior: crud
        crud:
          operation: select
          table: products
          schema: ob-poc
        args:
          - name: product-id
            type: uuid
            required: false
            maps_to: product_id
          - name: product-code
            type: string
            required: false
            maps_to: product_code
        returns:
          type: record

      update:
        description: "Update a product"
        behavior: crud
        crud:
          operation: update
          table: products
          schema: ob-poc
          key: product_id
        args:
          - name: product-id
            type: uuid
            required: true
            maps_to: product_id
          - name: name
            type: string
            required: false
            maps_to: name
          - name: description
            type: string
            required: false
            maps_to: description
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
          - name: product-category
            type: string
            required: false
            maps_to: product_category
        returns:
          type: affected

      delete:
        description: "Delete a product"
        behavior: crud
        crud:
          operation: delete
          table: products
          schema: ob-poc
          key: product_id
        args:
          - name: product-id
            type: uuid
            required: true
            maps_to: product_id
        returns:
          type: affected

      list:
        description: "List products with optional filters"
        behavior: crud
        crud:
          operation: select
          table: products
          schema: ob-poc
        args:
          - name: product-category
            type: string
            required: false
            maps_to: product_category
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
          - name: limit
            type: integer
            required: false
            default: 100
          - name: offset
            type: integer
            required: false
            default: 0
        returns:
          type: record_set

      ensure:
        description: "Create or update a product by product-code"
        behavior: crud
        crud:
          operation: upsert
          table: products
          schema: ob-poc
          conflict_keys: [product_code]
          returning: product_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: product-code
            type: string
            required: true
            maps_to: product_code
          - name: description
            type: string
            required: false
            maps_to: description
          - name: product-category
            type: string
            required: false
            maps_to: product_category
          - name: regulatory-framework
            type: string
            required: false
            maps_to: regulatory_framework
        returns:
          type: uuid
          name: product_id
          capture: true

  # ===========================================================================
  # DOMAIN: service
  # ===========================================================================
  service:
    description: "Service catalog operations"

    verbs:
      create:
        description: "Create a new service in the catalog"
        behavior: crud
        crud:
          operation: insert
          table: services
          schema: ob-poc
          returning: service_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: service-code
            type: string
            required: true
            maps_to: service_code
          - name: description
            type: string
            required: false
            maps_to: description
          - name: service-type
            type: string
            required: false
            maps_to: service_type
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
            default: true
        returns:
          type: uuid
          name: service_id
          capture: true

      read:
        description: "Read a service by ID or code"
        behavior: crud
        crud:
          operation: select
          table: services
          schema: ob-poc
        args:
          - name: service-id
            type: uuid
            required: false
            maps_to: service_id
          - name: service-code
            type: string
            required: false
            maps_to: service_code
        returns:
          type: record

      update:
        description: "Update a service"
        behavior: crud
        crud:
          operation: update
          table: services
          schema: ob-poc
          key: service_id
        args:
          - name: service-id
            type: uuid
            required: true
            maps_to: service_id
          - name: name
            type: string
            required: false
            maps_to: name
          - name: description
            type: string
            required: false
            maps_to: description
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
          - name: service-type
            type: string
            required: false
            maps_to: service_type
        returns:
          type: affected

      delete:
        description: "Delete a service"
        behavior: crud
        crud:
          operation: delete
          table: services
          schema: ob-poc
          key: service_id
        args:
          - name: service-id
            type: uuid
            required: true
            maps_to: service_id
        returns:
          type: affected

      list:
        description: "List services with optional filters"
        behavior: crud
        crud:
          operation: select
          table: services
          schema: ob-poc
        args:
          - name: service-type
            type: string
            required: false
            maps_to: service_type
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
          - name: limit
            type: integer
            required: false
            default: 100
          - name: offset
            type: integer
            required: false
            default: 0
        returns:
          type: record_set

      ensure:
        description: "Create or update a service by service-code"
        behavior: crud
        crud:
          operation: upsert
          table: services
          schema: ob-poc
          conflict_keys: [service_code]
          returning: service_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: service-code
            type: string
            required: true
            maps_to: service_code
          - name: description
            type: string
            required: false
            maps_to: description
          - name: service-type
            type: string
            required: false
            maps_to: service_type
        returns:
          type: uuid
          name: service_id
          capture: true

      link-product:
        description: "Link a service to a product"
        behavior: crud
        crud:
          operation: link
          junction: product_services
          schema: ob-poc
          from_col: service_id
          to_col: product_id
        args:
          - name: service-id
            type: uuid
            required: true
            maps_to: service_id
          - name: product-id
            type: uuid
            required: true
            maps_to: product_id
          - name: is-mandatory
            type: boolean
            required: false
            maps_to: is_mandatory
          - name: is-default
            type: boolean
            required: false
            maps_to: is_default
          - name: display-order
            type: integer
            required: false
            maps_to: display_order
        returns:
          type: void

      unlink-product:
        description: "Unlink a service from a product"
        behavior: crud
        crud:
          operation: unlink
          junction: product_services
          schema: ob-poc
          from_col: service_id
          to_col: product_id
        args:
          - name: service-id
            type: uuid
            required: true
          - name: product-id
            type: uuid
            required: true
        returns:
          type: affected

      list-by-product:
        description: "List services for a product"
        behavior: crud
        crud:
          operation: select_with_join
          primary_table: services
          join_table: product_services
          join_col: service_id
          filter_col: product_id
          schema: ob-poc
        args:
          - name: product-id
            type: uuid
            required: true
          - name: is-mandatory
            type: boolean
            required: false
        returns:
          type: record_set

  # ===========================================================================
  # DOMAIN: lifecycle-resource
  # ===========================================================================
  lifecycle-resource:
    description: "Lifecycle resource type operations (taxonomy)"

    verbs:
      create:
        description: "Create a new lifecycle resource type"
        behavior: crud
        crud:
          operation: insert
          table: prod_resources
          schema: ob-poc
          returning: resource_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: resource-code
            type: string
            required: true
            maps_to: resource_code
          - name: owner
            type: string
            required: true
            maps_to: owner
          - name: description
            type: string
            required: false
            maps_to: description
          - name: resource-type
            type: string
            required: false
            maps_to: resource_type
          - name: vendor
            type: string
            required: false
            maps_to: vendor
          - name: api-endpoint
            type: string
            required: false
            maps_to: api_endpoint
          - name: capabilities
            type: json
            required: false
            maps_to: capabilities
        returns:
          type: uuid
          name: resource_id
          capture: true

      read:
        description: "Read a lifecycle resource type by ID or code"
        behavior: crud
        crud:
          operation: select
          table: prod_resources
          schema: ob-poc
        args:
          - name: resource-id
            type: uuid
            required: false
            maps_to: resource_id
          - name: resource-code
            type: string
            required: false
            maps_to: resource_code
        returns:
          type: record

      update:
        description: "Update a lifecycle resource type"
        behavior: crud
        crud:
          operation: update
          table: prod_resources
          schema: ob-poc
          key: resource_id
        args:
          - name: resource-id
            type: uuid
            required: true
            maps_to: resource_id
          - name: name
            type: string
            required: false
            maps_to: name
          - name: description
            type: string
            required: false
            maps_to: description
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
          - name: api-endpoint
            type: string
            required: false
            maps_to: api_endpoint
          - name: capabilities
            type: json
            required: false
            maps_to: capabilities
        returns:
          type: affected

      delete:
        description: "Delete a lifecycle resource type"
        behavior: crud
        crud:
          operation: delete
          table: prod_resources
          schema: ob-poc
          key: resource_id
        args:
          - name: resource-id
            type: uuid
            required: true
            maps_to: resource_id
        returns:
          type: affected

      list:
        description: "List lifecycle resource types with optional filters"
        behavior: crud
        crud:
          operation: select
          table: prod_resources
          schema: ob-poc
        args:
          - name: resource-type
            type: string
            required: false
            maps_to: resource_type
          - name: owner
            type: string
            required: false
            maps_to: owner
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
          - name: limit
            type: integer
            required: false
            default: 100
          - name: offset
            type: integer
            required: false
            default: 0
        returns:
          type: record_set

      ensure:
        description: "Create or update a lifecycle resource type by resource-code"
        behavior: crud
        crud:
          operation: upsert
          table: prod_resources
          schema: ob-poc
          conflict_keys: [resource_code]
          returning: resource_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: resource-code
            type: string
            required: true
            maps_to: resource_code
          - name: owner
            type: string
            required: true
            maps_to: owner
          - name: description
            type: string
            required: false
            maps_to: description
          - name: resource-type
            type: string
            required: false
            maps_to: resource_type
          - name: vendor
            type: string
            required: false
            maps_to: vendor
        returns:
          type: uuid
          name: resource_id
          capture: true

      link-service:
        description: "Link a lifecycle resource type to a service"
        behavior: crud
        crud:
          operation: link
          junction: service_resources
          schema: ob-poc
          from_col: resource_id
          to_col: service_id
        args:
          - name: resource-id
            type: uuid
            required: true
            maps_to: resource_id
          - name: service-id
            type: uuid
            required: true
            maps_to: service_id
          - name: is-primary
            type: boolean
            required: false
            maps_to: is_primary
        returns:
          type: void

      unlink-service:
        description: "Unlink a lifecycle resource type from a service"
        behavior: crud
        crud:
          operation: unlink
          junction: service_resources
          schema: ob-poc
          from_col: resource_id
          to_col: service_id
        args:
          - name: resource-id
            type: uuid
            required: true
          - name: service-id
            type: uuid
            required: true
        returns:
          type: affected

      list-by-service:
        description: "List lifecycle resource types for a service"
        behavior: crud
        crud:
          operation: select_with_join
          primary_table: prod_resources
          join_table: service_resources
          join_col: resource_id
          filter_col: service_id
          schema: ob-poc
        args:
          - name: service-id
            type: uuid
            required: true
        returns:
          type: record_set

      # Attribute dictionary linkage (5 new verbs)
      add-attribute:
        description: "Add an attribute requirement to a lifecycle resource type"
        behavior: crud
        crud:
          operation: insert
          table: resource_attribute_requirements
          schema: ob-poc
          returning: requirement_id
        args:
          - name: resource-id
            type: uuid
            required: true
            maps_to: resource_id
          - name: attribute-id
            type: uuid
            required: true
            maps_to: attribute_id
          - name: resource-field-name
            type: string
            required: false
            maps_to: resource_field_name
          - name: is-mandatory
            type: boolean
            required: false
            maps_to: is_mandatory
            default: true
          - name: transformation-rule
            type: json
            required: false
            maps_to: transformation_rule
          - name: validation-override
            type: json
            required: false
            maps_to: validation_override
          - name: default-value
            type: string
            required: false
            maps_to: default_value
          - name: display-order
            type: integer
            required: false
            maps_to: display_order
        returns:
          type: uuid
          name: requirement_id
          capture: false

      remove-attribute:
        description: "Remove an attribute requirement from a lifecycle resource type"
        behavior: crud
        crud:
          operation: delete
          table: resource_attribute_requirements
          schema: ob-poc
          key: requirement_id
        args:
          - name: requirement-id
            type: uuid
            required: true
            maps_to: requirement_id
        returns:
          type: affected

      update-attribute:
        description: "Update an attribute requirement configuration"
        behavior: crud
        crud:
          operation: update
          table: resource_attribute_requirements
          schema: ob-poc
          key: requirement_id
        args:
          - name: requirement-id
            type: uuid
            required: true
            maps_to: requirement_id
          - name: resource-field-name
            type: string
            required: false
            maps_to: resource_field_name
          - name: is-mandatory
            type: boolean
            required: false
            maps_to: is_mandatory
          - name: transformation-rule
            type: json
            required: false
            maps_to: transformation_rule
          - name: validation-override
            type: json
            required: false
            maps_to: validation_override
          - name: default-value
            type: string
            required: false
            maps_to: default_value
          - name: display-order
            type: integer
            required: false
            maps_to: display_order
        returns:
          type: affected

      list-attributes:
        description: "List attribute requirements for a lifecycle resource type"
        behavior: crud
        crud:
          operation: list_by_fk
          table: resource_attribute_requirements
          schema: ob-poc
          fk_col: resource_id
        args:
          - name: resource-id
            type: uuid
            required: true
          - name: is-mandatory
            type: boolean
            required: false
        returns:
          type: record_set

      ensure-attribute:
        description: "Ensure an attribute requirement exists (upsert)"
        behavior: crud
        crud:
          operation: upsert
          table: resource_attribute_requirements
          schema: ob-poc
          conflict_keys: [resource_id, attribute_id]
          returning: requirement_id
        args:
          - name: resource-id
            type: uuid
            required: true
            maps_to: resource_id
          - name: attribute-id
            type: uuid
            required: true
            maps_to: attribute_id
          - name: resource-field-name
            type: string
            required: false
            maps_to: resource_field_name
          - name: is-mandatory
            type: boolean
            required: false
            maps_to: is_mandatory
          - name: transformation-rule
            type: json
            required: false
            maps_to: transformation_rule
          - name: validation-override
            type: json
            required: false
            maps_to: validation_override
          - name: default-value
            type: string
            required: false
            maps_to: default_value
          - name: display-order
            type: integer
            required: false
            maps_to: display_order
        returns:
          type: uuid
          name: requirement_id
          capture: false

# =============================================================================
# PLUGINS (Custom operations requiring Rust code)
# =============================================================================
plugins:
  document.catalog:
    description: "Catalog a document for an entity within a CBU"
    handler: document_catalog
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: document-type
        type: lookup
        required: true
        lookup:
          table: document_types
          code_column: type_code
          id_column: type_id
      - name: file-path
        type: string
        required: false
      - name: metadata
        type: json
        required: false
    returns:
      type: uuid
      name: doc_id

  document.extract:
    description: "Extract attributes from a cataloged document"
    handler: document_extract
    args:
      - name: document-id
        type: uuid
        required: true
      - name: attributes
        type: string_list
        required: false
      - name: use-ocr
        type: boolean
        required: false
        default: false

  ubo.calculate:
    description: "Calculate ultimate beneficial ownership chain"
    handler: ubo_calculate
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
      - name: threshold
        type: decimal
        required: false
        default: 25.0

  ubo.validate:
    description: "Validate UBO structure completeness"
    handler: ubo_validate
    args:
      - name: cbu-id
        type: uuid
        required: true

  screening.pep:
    description: "Run PEP screening"
    handler: screening_pep
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: provider
        type: string
        required: false

  screening.sanctions:
    description: "Run sanctions list screening"
    handler: screening_sanctions
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: lists
        type: string_list
        required: false

  screening.adverse-media:
    description: "Run adverse media screening"
    handler: screening_adverse_media
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: lookback-months
        type: integer
        required: false
        default: 24

  kyc.initiate:
    description: "Initiate KYC investigation"
    handler: kyc_initiate
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: investigation-type
        type: string
        required: true

  kyc.decide:
    description: "Record KYC decision"
    handler: kyc_decide
    args:
      - name: investigation-id
        type: uuid
        required: true
      - name: decision
        type: string
        required: true
        valid_values: [approve, reject, escalate]
      - name: rationale
        type: string
        required: true

  resource.create:
    description: "Create a resource instance for a CBU"
    handler: resource_instance_create
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: resource-type
        type: string
        required: true
      - name: instance-url
        type: string
        required: true
      - name: instance-id
        type: string
        required: false
      - name: instance-name
        type: string
        required: false
      - name: product-id
        type: uuid
        required: false
      - name: service-id
        type: uuid
        required: false
      - name: config
        type: json
        required: false
    returns:
      type: uuid
      name: instance_id

  resource.set-attr:
    description: "Set an attribute value on a resource instance"
    handler: resource_set_attr
    args:
      - name: instance-id
        type: uuid
        required: true
      - name: attr
        type: string
        required: true
      - name: value
        type: string
        required: true
      - name: state
        type: string
        required: false
        valid_values: [proposed, confirmed, derived, system]
      - name: source
        type: json
        required: false

  resource.activate:
    description: "Activate a resource instance"
    handler: resource_activate
    args:
      - name: instance-id
        type: uuid
        required: true

  resource.suspend:
    description: "Suspend a resource instance"
    handler: resource_suspend
    args:
      - name: instance-id
        type: uuid
        required: true
      - name: reason
        type: string
        required: false

  resource.decommission:
    description: "Decommission a resource instance"
    handler: resource_decommission
    args:
      - name: instance-id
        type: uuid
        required: true
      - name: reason
        type: string
        required: false

  delivery.record:
    description: "Record a service delivery for a CBU"
    handler: delivery_record
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: product
        type: string
        required: true
      - name: service
        type: string
        required: true
      - name: instance-id
        type: uuid
        required: false
      - name: config
        type: json
        required: false

  delivery.complete:
    description: "Mark a service delivery as complete"
    handler: delivery_complete
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: product
        type: string
        required: true
      - name: service
        type: string
        required: true
      - name: instance-id
        type: uuid
        required: false

  delivery.fail:
    description: "Mark a service delivery as failed"
    handler: delivery_fail
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: product
        type: string
        required: true
      - name: service
        type: string
        required: true
      - name: reason
        type: string
        required: true
```



---

### File: `rust/config/csg_rules.yaml`

```yaml
# CSG (Compliance and Security Gateway) Rules
# These rules validate DSL programs before execution.
#
# Rule types:
# - constraints: Hard errors that block execution
# - warnings: Soft warnings that allow execution
# - jurisdiction_rules: Location-specific requirements
# - composite_rules: Multi-condition checks

version: "1.0"

# =============================================================================
# CONSTRAINT RULES (Block Execution)
# =============================================================================
constraints:

  - id: CSG-C001
    name: passport_requires_person
    description: "PASSPORT document requires PROPER_PERSON entity"
    when:
      verb: document.catalog
      arg: document-type
      value: PASSPORT
    requires:
      entity_type: PROPER_PERSON
      via_arg: entity-id
    error: "Cannot catalog PASSPORT for non-person entity. Entity must be PROPER_PERSON type."

  - id: CSG-C002
    name: certificate_requires_company
    description: "Certificate of incorporation requires company entity"
    when:
      verb: document.catalog
      arg: document-type
      value: CERTIFICATE_OF_INCORPORATION
    requires:
      entity_type_in: [LIMITED_COMPANY, PLC, LLP]
      via_arg: entity-id
    error: "Cannot catalog CERTIFICATE_OF_INCORPORATION for non-company entity."

  - id: CSG-C003
    name: ubo_requires_person
    description: "BENEFICIAL_OWNER role requires natural person"
    when:
      verb: cbu.assign-role
      arg: role
      value: BENEFICIAL_OWNER
    requires:
      entity_type: PROPER_PERSON
      via_arg: entity-id
    error: "BENEFICIAL_OWNER role can only be assigned to natural persons."

  - id: CSG-C004
    name: director_requires_person
    description: "DIRECTOR role requires natural person"
    when:
      verb: cbu.assign-role
      arg: role
      value: DIRECTOR
    requires:
      entity_type: PROPER_PERSON
      via_arg: entity-id
    error: "DIRECTOR role can only be assigned to natural persons."

  - id: CSG-C005
    name: trust_deed_requires_trust
    description: "Trust deed requires trust entity"
    when:
      verb: document.catalog
      arg: document-type
      value: TRUST_DEED
    requires:
      entity_type_in: [TRUST_DISCRETIONARY, TRUST_FIXED_INTEREST, TRUST_UNIT]
      via_arg: entity-id
    error: "TRUST_DEED can only be cataloged for trust entities."

# =============================================================================
# WARNING RULES (Allow Execution with Warning)
# =============================================================================
warnings:

  - id: CSG-W001
    name: high_ownership_percentage
    description: "Ownership percentage exceeds 100%"
    when:
      verb: cbu.assign-role
      arg: ownership-percentage
      greater_than: 100
    message: "Warning: Ownership percentage {value}% exceeds 100%. Please verify."

  - id: CSG-W002
    name: missing_jurisdiction
    description: "Entity created without jurisdiction"
    when:
      verb_pattern: "entity.create-*"
      missing_arg: jurisdiction
    message: "Entity created without jurisdiction. Consider adding jurisdiction for compliance."

  - id: CSG-W003
    name: missing_screening
    description: "Person entity without screening scheduled"
    check: person_has_screening
    message: "Person entity has no PEP/sanctions screening scheduled."

  - id: CSG-W004
    name: missing_identity_doc
    description: "Person entity without identity document"
    check: person_has_identity_document
    message: "Person entity has no identity document cataloged."

  - id: CSG-W005
    name: low_ubo_threshold
    description: "UBO threshold below standard"
    when:
      verb: ubo.calculate
      arg: threshold
      less_than: 25
    message: "UBO threshold {value}% is below standard 25%. Verify regulatory requirements."

# =============================================================================
# JURISDICTION RULES
# =============================================================================
jurisdiction_rules:

  - id: CSG-J001
    name: us_person_fatca
    description: "US persons require FATCA documentation"
    severity: warning
    when:
      entity_type: PROPER_PERSON
      jurisdiction: US
    requires_document: W9_FORM
    message: "US person may require W-9 for FATCA compliance."

  - id: CSG-J002
    name: uk_company_psc
    description: "UK companies require PSC register"
    severity: info
    when:
      entity_type_in: [LIMITED_COMPANY, PLC]
      jurisdiction: GB
    message: "UK company - consider requesting PSC register."

  - id: CSG-J003
    name: eu_entity_gdpr
    description: "EU entities subject to GDPR"
    severity: info
    when:
      jurisdiction_in: [DE, FR, IT, ES, NL, BE, AT, IE, LU, PT, GR, FI, SE, DK, PL, CZ, HU, RO, BG, HR, SK, SI, EE, LV, LT, CY, MT]
    message: "EU entity - ensure GDPR compliance for data handling."

  - id: CSG-J004
    name: cayman_aml
    description: "Cayman entities require enhanced AML"
    severity: warning
    when:
      jurisdiction: KY
    message: "Cayman entity - enhanced AML/KYC requirements apply."

# =============================================================================
# COMPOSITE RULES (Multi-condition Checks)
# =============================================================================
composite_rules:

  - id: CSG-X001
    name: complete_corporate_onboarding
    description: "Corporate onboarding completeness check"
    severity: info
    applies_to:
      client_type: corporate
    checks:
      - has_company_entity
      - has_at_least_one_ubo
      - all_ubos_have_identity_docs
      - all_ubos_screened
      - company_has_formation_docs
    message: "Corporate onboarding incomplete: {missing_items}"

  - id: CSG-X002
    name: complete_individual_onboarding
    description: "Individual onboarding completeness check"
    severity: info
    applies_to:
      client_type: individual
    checks:
      - has_person_entity
      - person_has_identity_doc
      - person_screened
      - person_has_address_proof
    message: "Individual onboarding incomplete: {missing_items}"
```

---

## Part 3: Rust Configuration Types

### File: `rust/src/dsl_v2/config/mod.rs`

```rust
//! Configuration module for YAML-driven DSL
//!
//! This module provides:
//! - Type definitions for YAML configuration
//! - Configuration loader with validation
//! - Runtime registry building from config

pub mod types;
pub mod loader;
pub mod hot_reload;

pub use types::*;
pub use loader::ConfigLoader;
pub use hot_reload::HotReloadHandle;
```

### File: `rust/src/dsl_v2/config/types.rs`

```rust
//! Configuration type definitions
//!
//! These structs map directly to the YAML configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// TOP-LEVEL CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbsConfig {
    pub version: String,
    pub domains: HashMap<String, DomainConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CsgRulesConfig {
    pub version: String,
    #[serde(default)]
    pub constraints: Vec<ConstraintRule>,
    #[serde(default)]
    pub warnings: Vec<WarningRule>,
    #[serde(default)]
    pub jurisdiction_rules: Vec<JurisdictionRule>,
    #[serde(default)]
    pub composite_rules: Vec<CompositeRule>,
}

// =============================================================================
// DOMAIN & VERB CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DomainConfig {
    pub description: String,
    #[serde(default)]
    pub verbs: HashMap<String, VerbConfig>,
    #[serde(default)]
    pub dynamic_verbs: Vec<DynamicVerbConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    pub args: Vec<ArgConfig>,
    #[serde(default)]
    pub returns: Option<ReturnsConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbBehavior {
    Crud,
    Plugin,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrudConfig {
    pub operation: CrudOperation,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub returning: Option<String>,
    #[serde(default)]
    pub conflict_keys: Option<Vec<String>>,
    // For junction operations
    #[serde(default)]
    pub junction: Option<String>,
    #[serde(default)]
    pub from_col: Option<String>,
    #[serde(default)]
    pub to_col: Option<String>,
    #[serde(default)]
    pub role_table: Option<String>,
    #[serde(default)]
    pub role_col: Option<String>,
    #[serde(default)]
    pub fk_col: Option<String>,
    #[serde(default)]
    pub filter_col: Option<String>,
    // For joins
    #[serde(default)]
    pub primary_table: Option<String>,
    #[serde(default)]
    pub join_table: Option<String>,
    #[serde(default)]
    pub join_col: Option<String>,
    // For entity creation
    #[serde(default)]
    pub base_table: Option<String>,
    #[serde(default)]
    pub extension_table_column: Option<String>,
    #[serde(default)]
    pub type_id_column: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrudOperation {
    Insert,
    Select,
    Update,
    Delete,
    Upsert,
    Link,
    Unlink,
    RoleLink,
    RoleUnlink,
    ListByFk,
    ListParties,
    SelectWithJoin,
    EntityCreate,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArgConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub maps_to: Option<String>,
    #[serde(default)]
    pub lookup: Option<LookupConfig>,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgType {
    String,
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Uuid,
    Json,
    Lookup,
    StringList,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LookupConfig {
    pub table: String,
    pub code_column: String,
    pub id_column: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReturnsConfig {
    #[serde(rename = "type")]
    pub return_type: ReturnTypeConfig,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub capture: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnTypeConfig {
    Uuid,
    Record,
    RecordSet,
    Affected,
    Void,
}

// =============================================================================
// DYNAMIC VERB CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicVerbConfig {
    pub pattern: String,
    #[serde(default)]
    pub source: Option<DynamicSourceConfig>,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub base_args: Vec<ArgConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicSourceConfig {
    pub table: String,
    pub schema: Option<String>,
    pub code_column: String,
    pub name_column: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}

// =============================================================================
// PLUGIN CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub description: String,
    pub handler: String,
    pub args: Vec<ArgConfig>,
    #[serde(default)]
    pub returns: Option<ReturnsConfig>,
}

// =============================================================================
// CSG RULE CONFIGS
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstraintRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub when: RuleCondition,
    pub requires: RuleRequirement,
    pub error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WarningRule {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub when: Option<RuleCondition>,
    #[serde(default)]
    pub check: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JurisdictionRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub when: JurisdictionCondition,
    #[serde(default)]
    pub requires_document: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompositeRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub applies_to: AppliesTo,
    pub checks: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RuleCondition {
    #[serde(default)]
    pub verb: Option<String>,
    #[serde(default)]
    pub verb_pattern: Option<String>,
    #[serde(default)]
    pub arg: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub missing_arg: Option<String>,
    #[serde(default)]
    pub greater_than: Option<f64>,
    #[serde(default)]
    pub less_than: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RuleRequirement {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub via_arg: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JurisdictionCondition {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub jurisdiction_in: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppliesTo {
    #[serde(default)]
    pub client_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
}
```

### File: `rust/src/dsl_v2/config/loader.rs`

```rust
//! Configuration loader
//!
//! Loads and validates YAML configuration files.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tracing::info;

use super::types::{VerbsConfig, CsgRulesConfig, VerbBehavior, ArgType};

pub struct ConfigLoader {
    config_dir: String,
}

impl ConfigLoader {
    pub fn new(config_dir: impl Into<String>) -> Self {
        Self { config_dir: config_dir.into() }
    }

    /// Create loader from DSL_CONFIG_DIR env var or default to "config"
    pub fn from_env() -> Self {
        let dir = std::env::var("DSL_CONFIG_DIR")
            .unwrap_or_else(|_| "config".to_string());
        Self::new(dir)
    }

    /// Load verb configuration
    pub fn load_verbs(&self) -> Result<VerbsConfig> {
        let path = Path::new(&self.config_dir).join("verbs.yaml");
        info!("Loading verb configuration from {}", path.display());

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let config: VerbsConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        self.validate_verbs(&config)?;

        info!("Loaded {} domains with {} total verbs",
            config.domains.len(),
            config.domains.values().map(|d| d.verbs.len()).sum::<usize>()
        );

        Ok(config)
    }

    /// Load CSG rules configuration
    pub fn load_csg_rules(&self) -> Result<CsgRulesConfig> {
        let path = Path::new(&self.config_dir).join("csg_rules.yaml");
        info!("Loading CSG rules from {}", path.display());

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let config: CsgRulesConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        self.validate_csg_rules(&config)?;

        info!("Loaded {} constraints, {} warnings, {} jurisdiction rules",
            config.constraints.len(),
            config.warnings.len(),
            config.jurisdiction_rules.len()
        );

        Ok(config)
    }

    fn validate_verbs(&self, config: &VerbsConfig) -> Result<()> {
        for (domain, domain_config) in &config.domains {
            for (verb, verb_config) in &domain_config.verbs {
                let full_name = format!("{}.{}", domain, verb);

                // Validate CRUD verbs have crud config
                if verb_config.behavior == VerbBehavior::Crud && verb_config.crud.is_none() {
                    return Err(anyhow!("{}: crud behavior requires crud config", full_name));
                }

                // Validate lookup args have lookup config
                for arg in &verb_config.args {
                    if arg.arg_type == ArgType::Lookup && arg.lookup.is_none() {
                        return Err(anyhow!(
                            "{} arg '{}': lookup type requires lookup config",
                            full_name, arg.name
                        ));
                    }
                }
            }
        }

        // Validate plugins
        for (plugin_name, plugin_config) in &config.plugins {
            for arg in &plugin_config.args {
                if arg.arg_type == ArgType::Lookup && arg.lookup.is_none() {
                    return Err(anyhow!(
                        "plugin {} arg '{}': lookup type requires lookup config",
                        plugin_name, arg.name
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_csg_rules(&self, config: &CsgRulesConfig) -> Result<()> {
        let mut ids = std::collections::HashSet::new();

        // Check for duplicate rule IDs
        for rule in &config.constraints {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }
        for rule in &config.warnings {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }
        for rule in &config.jurisdiction_rules {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }
        for rule in &config.composite_rules {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_creation() {
        let loader = ConfigLoader::new("config");
        assert_eq!(loader.config_dir, "config");
    }

    #[test]
    fn test_from_env_default() {
        std::env::remove_var("DSL_CONFIG_DIR");
        let loader = ConfigLoader::from_env();
        assert_eq!(loader.config_dir, "config");
    }
}
```

### File: `rust/src/dsl_v2/config/hot_reload.rs`

```rust
//! Hot reload support for configuration
//!
//! Allows reloading configuration without restarting the server.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::loader::ConfigLoader;
use super::types::{VerbsConfig, CsgRulesConfig};

/// Handle for hot-reloading configuration
pub struct HotReloadHandle {
    loader: ConfigLoader,
    verbs: Arc<RwLock<VerbsConfig>>,
    csg_rules: Arc<RwLock<CsgRulesConfig>>,
}

impl HotReloadHandle {
    pub fn new(
        loader: ConfigLoader,
        verbs: Arc<RwLock<VerbsConfig>>,
        csg_rules: Arc<RwLock<CsgRulesConfig>>,
    ) -> Self {
        Self { loader, verbs, csg_rules }
    }

    /// Reload all configuration files
    pub async fn reload(&self) -> Result<ReloadResult> {
        info!("Hot-reloading DSL configuration...");

        let mut result = ReloadResult::default();

        // Reload verbs
        match self.loader.load_verbs() {
            Ok(new_verbs) => {
                let mut guard = self.verbs.write().await;
                let old_count = guard.domains.values().map(|d| d.verbs.len()).sum::<usize>();
                let new_count = new_verbs.domains.values().map(|d| d.verbs.len()).sum::<usize>();
                *guard = new_verbs;
                result.verbs_reloaded = true;
                result.verb_count_before = old_count;
                result.verb_count_after = new_count;
                info!("Verbs reloaded: {} -> {}", old_count, new_count);
            }
            Err(e) => {
                result.verbs_error = Some(e.to_string());
            }
        }

        // Reload CSG rules
        match self.loader.load_csg_rules() {
            Ok(new_rules) => {
                let mut guard = self.csg_rules.write().await;
                let old_count = guard.constraints.len() + guard.warnings.len();
                let new_count = new_rules.constraints.len() + new_rules.warnings.len();
                *guard = new_rules;
                result.csg_reloaded = true;
                result.rule_count_before = old_count;
                result.rule_count_after = new_count;
                info!("CSG rules reloaded: {} -> {}", old_count, new_count);
            }
            Err(e) => {
                result.csg_error = Some(e.to_string());
            }
        }

        Ok(result)
    }

    /// Get current verb config (read-only)
    pub async fn verbs(&self) -> tokio::sync::RwLockReadGuard<'_, VerbsConfig> {
        self.verbs.read().await
    }

    /// Get current CSG rules (read-only)
    pub async fn csg_rules(&self) -> tokio::sync::RwLockReadGuard<'_, CsgRulesConfig> {
        self.csg_rules.read().await
    }
}

#[derive(Debug, Default)]
pub struct ReloadResult {
    pub verbs_reloaded: bool,
    pub verbs_error: Option<String>,
    pub verb_count_before: usize,
    pub verb_count_after: usize,
    pub csg_reloaded: bool,
    pub csg_error: Option<String>,
    pub rule_count_before: usize,
    pub rule_count_after: usize,
}

impl ReloadResult {
    pub fn is_success(&self) -> bool {
        self.verbs_error.is_none() && self.csg_error.is_none()
    }
}
```

---

## Part 4: Runtime Verb Registry

### File: `rust/src/dsl_v2/runtime_registry.rs`

```rust
//! Runtime verb registry built from YAML configuration
//!
//! This replaces the static STANDARD_VERBS array with a dynamic
//! registry that can be reloaded at runtime.

use anyhow::{anyhow, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::config::types::*;

// =============================================================================
// RUNTIME VERB DEFINITION
// =============================================================================

/// Runtime verb definition (built from YAML config)
#[derive(Debug, Clone)]
pub struct RuntimeVerb {
    pub domain: String,
    pub verb: String,
    pub full_name: String,
    pub description: String,
    pub behavior: RuntimeBehavior,
    pub args: Vec<RuntimeArg>,
    pub returns: RuntimeReturn,
}

#[derive(Debug, Clone)]
pub enum RuntimeBehavior {
    /// Standard CRUD operation
    Crud(RuntimeCrudConfig),
    /// Plugin handler (Rust function)
    Plugin(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeCrudConfig {
    pub operation: CrudOperation,
    pub table: String,
    pub schema: String,
    pub key: Option<String>,
    pub returning: Option<String>,
    pub conflict_keys: Vec<String>,
    // Junction config
    pub junction: Option<String>,
    pub from_col: Option<String>,
    pub to_col: Option<String>,
    pub role_table: Option<String>,
    pub role_col: Option<String>,
    pub fk_col: Option<String>,
    pub filter_col: Option<String>,
    // Join config
    pub primary_table: Option<String>,
    pub join_table: Option<String>,
    pub join_col: Option<String>,
    // Entity create config
    pub base_table: Option<String>,
    pub extension_table_column: Option<String>,
    pub type_id_column: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeArg {
    pub name: String,
    pub arg_type: ArgType,
    pub required: bool,
    pub maps_to: Option<String>,
    pub lookup: Option<LookupConfig>,
    pub valid_values: Option<Vec<String>>,
    pub default: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone)]
pub struct RuntimeReturn {
    pub return_type: ReturnTypeConfig,
    pub name: Option<String>,
    pub capture: bool,
}

// =============================================================================
// RUNTIME REGISTRY
// =============================================================================

/// Runtime verb registry - can be hot-reloaded
pub struct RuntimeVerbRegistry {
    verbs: HashMap<String, RuntimeVerb>,
    by_domain: HashMap<String, Vec<String>>,
    domains: Vec<String>,
}

impl RuntimeVerbRegistry {
    /// Build registry from configuration
    pub fn from_config(config: &VerbsConfig) -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<String, Vec<String>> = HashMap::new();

        // Process each domain
        for (domain_name, domain_config) in &config.domains {
            // Process static verbs
            for (verb_name, verb_config) in &domain_config.verbs {
                let full_name = format!("{}.{}", domain_name, verb_name);

                let runtime_verb = Self::build_verb(
                    domain_name,
                    verb_name,
                    verb_config,
                );

                verbs.insert(full_name.clone(), runtime_verb);
                by_domain.entry(domain_name.clone())
                    .or_default()
                    .push(full_name);
            }
        }

        // Process plugins
        for (plugin_name, plugin_config) in &config.plugins {
            let parts: Vec<&str> = plugin_name.split('.').collect();
            if parts.len() != 2 {
                continue;
            }
            let domain = parts[0];
            let verb = parts[1];

            let runtime_verb = Self::build_plugin_verb(
                domain,
                verb,
                plugin_config,
            );

            verbs.insert(plugin_name.clone(), runtime_verb);
            by_domain.entry(domain.to_string())
                .or_default()
                .push(plugin_name.clone());
        }

        // Sort domain lists
        for list in by_domain.values_mut() {
            list.sort();
            list.dedup();
        }

        let mut domains: Vec<String> = by_domain.keys().cloned().collect();
        domains.sort();

        Self { verbs, by_domain, domains }
    }

    /// Build registry with dynamic verbs from database
    pub async fn from_config_with_db(
        config: &VerbsConfig,
        pool: &PgPool,
    ) -> Result<Self> {
        let mut registry = Self::from_config(config);

        // Process dynamic verbs
        for (domain_name, domain_config) in &config.domains {
            for dynamic in &domain_config.dynamic_verbs {
                registry.expand_dynamic_verbs(
                    domain_name,
                    dynamic,
                    pool,
                ).await?;
            }
        }

        Ok(registry)
    }

    async fn expand_dynamic_verbs(
        &mut self,
        domain: &str,
        dynamic: &DynamicVerbConfig,
        pool: &PgPool,
    ) -> Result<()> {
        let source = dynamic.source.as_ref()
            .ok_or_else(|| anyhow!("Dynamic verb requires source config"))?;

        let schema = source.schema.as_deref().unwrap_or("ob-poc");

        // Query entity types from database
        let query = format!(
            r#"SELECT {} FROM "{}".{}"#,
            source.code_column,
            schema,
            source.table
        );

        let rows: Vec<(String,)> = sqlx::query_as(&query)
            .fetch_all(pool)
            .await?;

        for (type_code,) in rows {
            let verb_name = dynamic.pattern.replace("{type_code}", &type_code);
            let verb_name = Self::transform_name(&verb_name, source.transform.as_deref());
            let full_name = format!("{}.{}", domain, verb_name);

            // Build verb from dynamic config
            let runtime_verb = RuntimeVerb {
                domain: domain.to_string(),
                verb: verb_name.clone(),
                full_name: full_name.clone(),
                description: format!("Create {} entity", type_code),
                behavior: RuntimeBehavior::Crud(RuntimeCrudConfig {
                    operation: dynamic.crud.as_ref()
                        .map(|c| c.operation)
                        .unwrap_or(CrudOperation::EntityCreate),
                    table: dynamic.crud.as_ref()
                        .and_then(|c| c.base_table.clone())
                        .unwrap_or_else(|| "entities".to_string()),
                    schema: schema.to_string(),
                    key: None,
                    returning: Some("entity_id".to_string()),
                    conflict_keys: vec![],
                    junction: None,
                    from_col: None,
                    to_col: None,
                    role_table: None,
                    role_col: None,
                    fk_col: None,
                    filter_col: None,
                    primary_table: None,
                    join_table: None,
                    join_col: None,
                    base_table: Some("entities".to_string()),
                    extension_table_column: dynamic.crud.as_ref()
                        .and_then(|c| c.extension_table_column.clone()),
                    type_id_column: dynamic.crud.as_ref()
                        .and_then(|c| c.type_id_column.clone()),
                }),
                args: dynamic.base_args.iter()
                    .map(Self::convert_arg)
                    .collect(),
                returns: RuntimeReturn {
                    return_type: ReturnTypeConfig::Uuid,
                    name: Some("entity_id".to_string()),
                    capture: true,
                },
            };

            self.verbs.insert(full_name.clone(), runtime_verb);
            self.by_domain.entry(domain.to_string())
                .or_default()
                .push(full_name);
        }

        info!("Expanded {} dynamic verbs for domain {}", rows.len(), domain);
        Ok(())
    }

    fn transform_name(name: &str, transform: Option<&str>) -> String {
        match transform {
            Some("kebab_case") => name.to_lowercase().replace('_', "-"),
            Some("snake_case") => name.to_lowercase().replace('-', "_"),
            _ => name.to_string(),
        }
    }

    fn build_verb(
        domain: &str,
        verb: &str,
        config: &VerbConfig,
    ) -> RuntimeVerb {
        let behavior = match (&config.behavior, &config.crud) {
            (VerbBehavior::Crud, Some(crud)) => {
                RuntimeBehavior::Crud(RuntimeCrudConfig {
                    operation: crud.operation,
                    table: crud.table.clone().unwrap_or_default(),
                    schema: crud.schema.clone().unwrap_or_else(|| "ob-poc".to_string()),
                    key: crud.key.clone(),
                    returning: crud.returning.clone(),
                    conflict_keys: crud.conflict_keys.clone().unwrap_or_default(),
                    junction: crud.junction.clone(),
                    from_col: crud.from_col.clone(),
                    to_col: crud.to_col.clone(),
                    role_table: crud.role_table.clone(),
                    role_col: crud.role_col.clone(),
                    fk_col: crud.fk_col.clone(),
                    filter_col: crud.filter_col.clone(),
                    primary_table: crud.primary_table.clone(),
                    join_table: crud.join_table.clone(),
                    join_col: crud.join_col.clone(),
                    base_table: crud.base_table.clone(),
                    extension_table_column: crud.extension_table_column.clone(),
                    type_id_column: crud.type_id_column.clone(),
                })
            }
            _ => RuntimeBehavior::Crud(RuntimeCrudConfig {
                operation: CrudOperation::Select,
                table: String::new(),
                schema: "ob-poc".to_string(),
                key: None,
                returning: None,
                conflict_keys: vec![],
                junction: None,
                from_col: None,
                to_col: None,
                role_table: None,
                role_col: None,
                fk_col: None,
                filter_col: None,
                primary_table: None,
                join_table: None,
                join_col: None,
                base_table: None,
                extension_table_column: None,
                type_id_column: None,
            }),
        };

        RuntimeVerb {
            domain: domain.to_string(),
            verb: verb.to_string(),
            full_name: format!("{}.{}", domain, verb),
            description: config.description.clone(),
            behavior,
            args: config.args.iter().map(Self::convert_arg).collect(),
            returns: config.returns.as_ref()
                .map(|r| RuntimeReturn {
                    return_type: r.return_type,
                    name: r.name.clone(),
                    capture: r.capture.unwrap_or(false),
                })
                .unwrap_or(RuntimeReturn {
                    return_type: ReturnTypeConfig::Void,
                    name: None,
                    capture: false,
                }),
        }
    }

    fn build_plugin_verb(
        domain: &str,
        verb: &str,
        config: &PluginConfig,
    ) -> RuntimeVerb {
        RuntimeVerb {
            domain: domain.to_string(),
            verb: verb.to_string(),
            full_name: format!("{}.{}", domain, verb),
            description: config.description.clone(),
            behavior: RuntimeBehavior::Plugin(config.handler.clone()),
            args: config.args.iter().map(Self::convert_arg).collect(),
            returns: config.returns.as_ref()
                .map(|r| RuntimeReturn {
                    return_type: r.return_type,
                    name: r.name.clone(),
                    capture: r.capture.unwrap_or(false),
                })
                .unwrap_or(RuntimeReturn {
                    return_type: ReturnTypeConfig::Void,
                    name: None,
                    capture: false,
                }),
        }
    }

    fn convert_arg(arg: &ArgConfig) -> RuntimeArg {
        RuntimeArg {
            name: arg.name.clone(),
            arg_type: arg.arg_type,
            required: arg.required,
            maps_to: arg.maps_to.clone(),
            lookup: arg.lookup.clone(),
            valid_values: arg.valid_values.clone(),
            default: arg.default.clone(),
        }
    }

    // =========================================================================
    // LOOKUP METHODS
    // =========================================================================

    pub fn get(&self, domain: &str, verb: &str) -> Option<&RuntimeVerb> {
        let key = format!("{}.{}", domain, verb);
        self.verbs.get(&key)
    }

    pub fn get_by_name(&self, full_name: &str) -> Option<&RuntimeVerb> {
        self.verbs.get(full_name)
    }

    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&RuntimeVerb> {
        self.by_domain.get(domain)
            .map(|keys| keys.iter().filter_map(|k| self.verbs.get(k)).collect())
            .unwrap_or_default()
    }

    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    pub fn all_verbs(&self) -> impl Iterator<Item = &RuntimeVerb> {
        self.verbs.values()
    }

    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn contains(&self, domain: &str, verb: &str) -> bool {
        self.get(domain, verb).is_some()
    }
}

// =============================================================================
// THREAD-SAFE WRAPPER
// =============================================================================

/// Thread-safe wrapper for hot-reloadable registry
#[derive(Clone)]
pub struct SharedVerbRegistry {
    inner: Arc<RwLock<RuntimeVerbRegistry>>,
}

impl SharedVerbRegistry {
    pub fn new(registry: RuntimeVerbRegistry) -> Self {
        Self {
            inner: Arc::new(RwLock::new(registry)),
        }
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, RuntimeVerbRegistry> {
        self.inner.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, RuntimeVerbRegistry> {
        self.inner.write().await
    }

    pub fn clone_inner(&self) -> Arc<RwLock<RuntimeVerbRegistry>> {
        self.inner.clone()
    }
}
```



---

## Part 5: Generic CRUD Executor

### File: `rust/src/dsl_v2/generic_executor.rs`

```rust
//! Generic CRUD executor
//!
//! Executes CRUD operations based on runtime verb configuration.
//! No hardcoded table names or column mappings - everything comes from config.

use anyhow::{anyhow, Result};
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Row, postgres::PgRow};
use std::collections::HashMap;
use tracing::debug;
use uuid::Uuid;

use super::config::types::*;
use super::runtime_registry::{RuntimeVerb, RuntimeBehavior, RuntimeCrudConfig, RuntimeArg};

/// Result of executing a verb
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Single UUID returned (from INSERT/UPSERT with RETURNING)
    Uuid(Uuid),
    /// Single record (from SELECT by ID)
    Record(JsonValue),
    /// Multiple records (from SELECT list)
    RecordSet(Vec<JsonValue>),
    /// Number of rows affected (from UPDATE/DELETE)
    Affected(u64),
    /// No return value
    Void,
}

/// Generic CRUD executor - executes verbs based on config, not hardcoded logic
pub struct GenericCrudExecutor {
    pool: PgPool,
}

impl GenericCrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Execute a CRUD verb with given arguments
    pub async fn execute(
        &self,
        verb: &RuntimeVerb,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let crud = match &verb.behavior {
            RuntimeBehavior::Crud(crud) => crud,
            RuntimeBehavior::Plugin(handler) => {
                return Err(anyhow!(
                    "Verb {}.{} is a plugin ({}), use plugin executor",
                    verb.domain, verb.verb, handler
                ));
            }
        };

        match crud.operation {
            CrudOperation::Insert => self.execute_insert(verb, crud, args).await,
            CrudOperation::Select => self.execute_select(verb, crud, args).await,
            CrudOperation::Update => self.execute_update(verb, crud, args).await,
            CrudOperation::Delete => self.execute_delete(verb, crud, args).await,
            CrudOperation::Upsert => self.execute_upsert(verb, crud, args).await,
            CrudOperation::Link => self.execute_link(verb, crud, args).await,
            CrudOperation::Unlink => self.execute_unlink(verb, crud, args).await,
            CrudOperation::RoleLink => self.execute_role_link(verb, crud, args).await,
            CrudOperation::RoleUnlink => self.execute_role_unlink(verb, crud, args).await,
            CrudOperation::ListByFk => self.execute_list_by_fk(verb, crud, args).await,
            CrudOperation::ListParties => self.execute_list_parties(verb, crud, args).await,
            CrudOperation::SelectWithJoin => self.execute_select_with_join(verb, crud, args).await,
            CrudOperation::EntityCreate => self.execute_entity_create(verb, crud, args).await,
        }
    }

    // =========================================================================
    // INSERT
    // =========================================================================

    async fn execute_insert(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        let mut idx = 1;
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if let Some(col) = &arg.maps_to {
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));
                    bind_values.push(self.to_sql_value(value, arg)?);
                    idx += 1;
                }
            }
        }

        if columns.is_empty() {
            return Err(anyhow!("No columns to insert"));
        }

        let returning = crud.returning.as_deref().unwrap_or("*");
        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({}) RETURNING {}"#,
            crud.schema,
            crud.table,
            columns.join(", "),
            placeholders.join(", "),
            returning
        );

        debug!("Executing INSERT: {}", sql);

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }

        let row = query.fetch_one(&self.pool).await?;

        // Extract the returning value
        if returning != "*" {
            let uuid: Uuid = row.try_get(returning)?;
            Ok(ExecutionResult::Uuid(uuid))
        } else {
            Ok(ExecutionResult::Record(self.row_to_json(&row)?))
        }
    }

    // =========================================================================
    // SELECT
    // =========================================================================

    async fn execute_select(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let mut conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;
        let mut limit: Option<i64> = None;
        let mut offset: Option<i64> = None;

        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if arg.name == "limit" {
                    limit = value.as_i64();
                    continue;
                }
                if arg.name == "offset" {
                    offset = value.as_i64();
                    continue;
                }
                if let Some(col) = &arg.maps_to {
                    conditions.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(self.to_sql_value(value, arg)?);
                    idx += 1;
                }
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
        let offset_clause = offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default();

        let sql = format!(
            r#"SELECT * FROM "{}"."{}"{}{}{}""#,
            crud.schema,
            crud.table,
            where_clause,
            limit_clause,
            offset_clause
        );

        debug!("Executing SELECT: {}", sql);

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }

        let rows = query.fetch_all(&self.pool).await?;

        if rows.len() == 1 && limit.is_none() {
            Ok(ExecutionResult::Record(self.row_to_json(&rows[0])?))
        } else {
            let records: Result<Vec<JsonValue>> = rows.iter()
                .map(|r| self.row_to_json(r))
                .collect();
            Ok(ExecutionResult::RecordSet(records?))
        }
    }

    // =========================================================================
    // UPDATE
    // =========================================================================

    async fn execute_update(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let key = crud.key.as_deref()
            .ok_or_else(|| anyhow!("Update requires key column"))?;

        let mut sets = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut key_value: Option<SqlValue> = None;
        let mut idx = 1;

        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if let Some(col) = &arg.maps_to {
                    if col == key {
                        key_value = Some(self.to_sql_value(value, arg)?);
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        bind_values.push(self.to_sql_value(value, arg)?);
                        idx += 1;
                    }
                }
            }
        }

        let key_value = key_value.ok_or_else(|| anyhow!("Key argument required for update"))?;

        if sets.is_empty() {
            return Err(anyhow!("No columns to update"));
        }

        let sql = format!(
            r#"UPDATE "{}"."{}" SET {} WHERE "{}" = ${}"#,
            crud.schema,
            crud.table,
            sets.join(", "),
            key,
            idx
        );

        debug!("Executing UPDATE: {}", sql);

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }
        query = self.bind_value(query, &key_value);

        let result = query.execute(&self.pool).await?;
        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    // =========================================================================
    // DELETE
    // =========================================================================

    async fn execute_delete(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let key = crud.key.as_deref()
            .ok_or_else(|| anyhow!("Delete requires key column"))?;

        let key_arg = verb.args.iter()
            .find(|a| a.maps_to.as_deref() == Some(key))
            .ok_or_else(|| anyhow!("Key argument not found"))?;

        let key_value = args.get(&key_arg.name)
            .ok_or_else(|| anyhow!("Key argument value required"))?;

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1"#,
            crud.schema,
            crud.table,
            key
        );

        debug!("Executing DELETE: {}", sql);

        let sql_val = self.to_sql_value(key_value, key_arg)?;
        let mut query = sqlx::query(&sql);
        query = self.bind_value(query, &sql_val);

        let result = query.execute(&self.pool).await?;
        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    // =========================================================================
    // UPSERT
    // =========================================================================

    async fn execute_upsert(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        if crud.conflict_keys.is_empty() {
            return Err(anyhow!("Upsert requires conflict_keys"));
        }

        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut updates = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        let mut idx = 1;
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if let Some(col) = &arg.maps_to {
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    // Only update non-conflict columns
                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }

                    bind_values.push(self.to_sql_value(value, arg)?);
                    idx += 1;
                }
            }
        }

        let conflict_cols: Vec<String> = crud.conflict_keys.iter()
            .map(|c| format!("\"{}\"", c))
            .collect();

        let returning = crud.returning.as_deref().unwrap_or("*");

        let update_clause = if updates.is_empty() {
            "DO NOTHING".to_string()
        } else {
            format!("DO UPDATE SET {}", updates.join(", "))
        };

        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
               ON CONFLICT ({}) {}
               RETURNING {}"#,
            crud.schema,
            crud.table,
            columns.join(", "),
            placeholders.join(", "),
            conflict_cols.join(", "),
            update_clause,
            returning
        );

        debug!("Executing UPSERT: {}", sql);

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }

        let row = query.fetch_one(&self.pool).await?;

        if returning != "*" {
            let uuid: Uuid = row.try_get(returning)?;
            Ok(ExecutionResult::Uuid(uuid))
        } else {
            Ok(ExecutionResult::Record(self.row_to_json(&row)?))
        }
    }

    // =========================================================================
    // LINK / UNLINK (Junction table operations)
    // =========================================================================

    async fn execute_link(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("Link requires junction table"))?;
        let from_col = crud.from_col.as_deref()
            .ok_or_else(|| anyhow!("Link requires from_col"))?;
        let to_col = crud.to_col.as_deref()
            .ok_or_else(|| anyhow!("Link requires to_col"))?;

        // Collect all columns and values
        let mut columns = vec![format!("\"{}\"", from_col), format!("\"{}\"", to_col)];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string()];
        let mut bind_values: Vec<SqlValue> = Vec::new();

        // Find from and to values
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if arg.maps_to.as_deref() == Some(from_col) {
                    bind_values.insert(0, self.to_sql_value(value, arg)?);
                } else if arg.maps_to.as_deref() == Some(to_col) {
                    if bind_values.is_empty() {
                        bind_values.push(SqlValue::Null); // placeholder
                    }
                    bind_values.push(self.to_sql_value(value, arg)?);
                }
            }
        }

        // Add extra junction columns
        let mut idx = 3;
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if let Some(col) = &arg.maps_to {
                    if col != from_col && col != to_col {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.to_sql_value(value, arg)?);
                        idx += 1;
                    }
                }
            }
        }

        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({}) ON CONFLICT DO NOTHING"#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", ")
        );

        debug!("Executing LINK: {}", sql);

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }

        query.execute(&self.pool).await?;
        Ok(ExecutionResult::Void)
    }

    async fn execute_unlink(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("Unlink requires junction table"))?;
        let from_col = crud.from_col.as_deref()
            .ok_or_else(|| anyhow!("Unlink requires from_col"))?;
        let to_col = crud.to_col.as_deref()
            .ok_or_else(|| anyhow!("Unlink requires to_col"))?;

        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if arg.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.to_sql_value(value, arg)?);
                } else if arg.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.to_sql_value(value, arg)?);
                }
            }
        }

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1 AND "{}" = $2"#,
            crud.schema,
            junction,
            from_col,
            to_col
        );

        debug!("Executing UNLINK: {}", sql);

        let mut query = sqlx::query(&sql);
        query = self.bind_value(query, &from_value.ok_or_else(|| anyhow!("from value required"))?);
        query = self.bind_value(query, &to_value.ok_or_else(|| anyhow!("to value required"))?);

        let result = query.execute(&self.pool).await?;
        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    // =========================================================================
    // ROLE LINK / UNLINK (Junction with role lookup)
    // =========================================================================

    async fn execute_role_link(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires junction table"))?;
        let role_table = crud.role_table.as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires role_table"))?;

        // Find the role argument and look up the role_id
        let role_arg = verb.args.iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleLink requires lookup argument"))?;

        let role_value = args.get(&role_arg.name)
            .ok_or_else(|| anyhow!("Role argument required"))?;

        let lookup = role_arg.lookup.as_ref().unwrap();
        let role_code = role_value.as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        // Look up role_id
        let lookup_sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.id_column,
            crud.schema,
            lookup.table,
            lookup.code_column
        );

        let role_row = sqlx::query(&lookup_sql)
            .bind(role_code)
            .fetch_one(&self.pool)
            .await?;

        let role_id: Uuid = role_row.try_get(&lookup.id_column as &str)?;

        // Now insert into junction with role_id
        let from_col = crud.from_col.as_deref().unwrap();
        let to_col = crud.to_col.as_deref().unwrap();
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        let mut columns = vec![
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
            format!("\"{}\"", role_col),
        ];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string(), "$3".to_string()];
        let mut bind_values: Vec<SqlValue> = Vec::new();

        // Get from and to values
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if arg.maps_to.as_deref() == Some(from_col) {
                    bind_values.push(self.to_sql_value(value, arg)?);
                } else if arg.maps_to.as_deref() == Some(to_col) {
                    bind_values.push(self.to_sql_value(value, arg)?);
                }
            }
        }

        bind_values.push(SqlValue::Uuid(role_id));

        // Add extra columns
        let mut idx = 4;
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if let Some(col) = &arg.maps_to {
                    if col != from_col && col != to_col && arg.arg_type != ArgType::Lookup {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.to_sql_value(value, arg)?);
                        idx += 1;
                    }
                }
            }
        }

        let returning = crud.returning.as_deref();
        let returning_clause = returning.map(|r| format!(" RETURNING {}", r)).unwrap_or_default();

        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({}){}""#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            returning_clause
        );

        debug!("Executing ROLE_LINK: {}", sql);

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }

        if returning.is_some() {
            let row = query.fetch_one(&self.pool).await?;
            let uuid: Uuid = row.try_get(returning.unwrap())?;
            Ok(ExecutionResult::Uuid(uuid))
        } else {
            query.execute(&self.pool).await?;
            Ok(ExecutionResult::Void)
        }
    }

    async fn execute_role_unlink(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        // Similar to role_link but DELETE instead of INSERT
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires junction table"))?;

        let role_arg = verb.args.iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleUnlink requires lookup argument"))?;

        let role_value = args.get(&role_arg.name)
            .ok_or_else(|| anyhow!("Role argument required"))?;

        let lookup = role_arg.lookup.as_ref().unwrap();
        let role_code = role_value.as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        // Look up role_id
        let lookup_sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.id_column,
            crud.schema,
            lookup.table,
            lookup.code_column
        );

        let role_row = sqlx::query(&lookup_sql)
            .bind(role_code)
            .fetch_one(&self.pool)
            .await?;

        let role_id: Uuid = role_row.try_get(&lookup.id_column as &str)?;

        let from_col = crud.from_col.as_deref().unwrap();
        let to_col = crud.to_col.as_deref().unwrap();
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1 AND "{}" = $2 AND "{}" = $3"#,
            crud.schema,
            junction,
            from_col,
            to_col,
            role_col
        );

        debug!("Executing ROLE_UNLINK: {}", sql);

        let mut bind_values: Vec<SqlValue> = Vec::new();
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if arg.maps_to.as_deref() == Some(from_col) {
                    bind_values.push(self.to_sql_value(value, arg)?);
                } else if arg.maps_to.as_deref() == Some(to_col) {
                    bind_values.push(self.to_sql_value(value, arg)?);
                }
            }
        }
        bind_values.push(SqlValue::Uuid(role_id));

        let mut query = sqlx::query(&sql);
        for val in &bind_values {
            query = self.bind_value(query, val);
        }

        let result = query.execute(&self.pool).await?;
        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    // =========================================================================
    // LIST BY FK
    // =========================================================================

    async fn execute_list_by_fk(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let fk_col = crud.fk_col.as_deref()
            .ok_or_else(|| anyhow!("ListByFk requires fk_col"))?;

        let fk_arg = verb.args.iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListByFk requires FK argument"))?;

        let fk_value = args.get(&fk_arg.name)
            .ok_or_else(|| anyhow!("FK argument value required"))?;

        let sql = format!(
            r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1"#,
            crud.schema,
            crud.table,
            fk_col
        );

        debug!("Executing LIST_BY_FK: {}", sql);

        let sql_val = self.to_sql_value(fk_value, fk_arg)?;
        let mut query = sqlx::query(&sql);
        query = self.bind_value(query, &sql_val);

        let rows = query.fetch_all(&self.pool).await?;
        let records: Result<Vec<JsonValue>> = rows.iter()
            .map(|r| self.row_to_json(r))
            .collect();

        Ok(ExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // LIST PARTIES (CBU Entity Roles with joins)
    // =========================================================================

    async fn execute_list_parties(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("ListParties requires junction"))?;
        let fk_col = crud.fk_col.as_deref()
            .ok_or_else(|| anyhow!("ListParties requires fk_col"))?;

        let fk_arg = verb.args.iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListParties requires FK argument"))?;

        let fk_value = args.get(&fk_arg.name)
            .ok_or_else(|| anyhow!("FK argument value required"))?;

        // Join with entities and roles tables
        let sql = format!(
            r#"SELECT j.*, e.name as entity_name, e.status as entity_status, r.name as role_name
               FROM "{}"."{}" j
               JOIN "{}".entities e ON j.entity_id = e.entity_id
               JOIN "{}".roles r ON j.role_id = r.role_id
               WHERE j."{}" = $1"#,
            crud.schema, junction,
            crud.schema,
            crud.schema,
            fk_col
        );

        debug!("Executing LIST_PARTIES: {}", sql);

        let sql_val = self.to_sql_value(fk_value, fk_arg)?;
        let mut query = sqlx::query(&sql);
        query = self.bind_value(query, &sql_val);

        let rows = query.fetch_all(&self.pool).await?;
        let records: Result<Vec<JsonValue>> = rows.iter()
            .map(|r| self.row_to_json(r))
            .collect();

        Ok(ExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // SELECT WITH JOIN
    // =========================================================================

    async fn execute_select_with_join(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        let primary = crud.primary_table.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires primary_table"))?;
        let join_table = crud.join_table.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_table"))?;
        let join_col = crud.join_col.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_col"))?;
        let filter_col = crud.filter_col.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter_col"))?;

        let filter_arg = verb.args.iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter argument"))?;

        let filter_value = args.get(&filter_arg.name)
            .ok_or_else(|| anyhow!("Filter argument value required"))?;

        let sql = format!(
            r#"SELECT p.* FROM "{}"."{}" p
               JOIN "{}"."{}" j ON p."{}" = j."{}"
               WHERE j."{}" = $1"#,
            crud.schema, primary,
            crud.schema, join_table,
            join_col, join_col,
            filter_col
        );

        debug!("Executing SELECT_WITH_JOIN: {}", sql);

        let sql_val = self.to_sql_value(filter_value, filter_arg)?;
        let mut query = sqlx::query(&sql);
        query = self.bind_value(query, &sql_val);

        let rows = query.fetch_all(&self.pool).await?;
        let records: Result<Vec<JsonValue>> = rows.iter()
            .map(|r| self.row_to_json(r))
            .collect();

        Ok(ExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // ENTITY CREATE (Base + Extension table pattern)
    // =========================================================================

    async fn execute_entity_create(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<ExecutionResult> {
        // Extract type code from verb name (e.g., "create-limited-company" -> "LIMITED_COMPANY")
        let type_code = verb.verb
            .strip_prefix("create-")
            .map(|s| s.to_uppercase().replace('-', "_"))
            .ok_or_else(|| anyhow!("Invalid entity create verb name"))?;

        // Look up entity_type_id and table_name
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types WHERE type_code = $1"#,
            crud.schema
        );

        let type_row = sqlx::query(&type_sql)
            .bind(&type_code)
            .fetch_one(&self.pool)
            .await?;

        let entity_type_id: Uuid = type_row.try_get("entity_type_id")?;
        let table_name: String = type_row.try_get("table_name")?;

        // Insert into base entities table
        let mut base_columns = vec!["\"entity_type_id\"".to_string()];
        let mut base_placeholders = vec!["$1".to_string()];
        let mut base_values: Vec<SqlValue> = vec![SqlValue::Uuid(entity_type_id)];

        let mut idx = 2;
        for arg in &verb.args {
            if let Some(value) = args.get(&arg.name) {
                if let Some(col) = &arg.maps_to {
                    base_columns.push(format!("\"{}\"", col));
                    base_placeholders.push(format!("${}", idx));
                    base_values.push(self.to_sql_value(value, arg)?);
                    idx += 1;
                }
            }
        }

        let base_sql = format!(
            r#"INSERT INTO "{}".entities ({}) VALUES ({}) RETURNING entity_id"#,
            crud.schema,
            base_columns.join(", "),
            base_placeholders.join(", ")
        );

        debug!("Executing ENTITY_CREATE (base): {}", base_sql);

        let mut query = sqlx::query(&base_sql);
        for val in &base_values {
            query = self.bind_value(query, val);
        }

        let row = query.fetch_one(&self.pool).await?;
        let entity_id: Uuid = row.try_get("entity_id")?;

        // Insert into extension table
        let ext_sql = format!(
            r#"INSERT INTO "{}"."{}" (entity_id) VALUES ($1)"#,
            crud.schema,
            table_name
        );

        debug!("Executing ENTITY_CREATE (extension): {}", ext_sql);

        sqlx::query(&ext_sql)
            .bind(entity_id)
            .execute(&self.pool)
            .await?;

        Ok(ExecutionResult::Uuid(entity_id))
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    fn to_sql_value(&self, value: &JsonValue, arg: &RuntimeArg) -> Result<SqlValue> {
        match arg.arg_type {
            ArgType::String => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected string for {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
            ArgType::Uuid => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected UUID string for {}", arg.name))?;
                let uuid = Uuid::parse_str(s)?;
                Ok(SqlValue::Uuid(uuid))
            }
            ArgType::Integer => {
                let n = value.as_i64()
                    .ok_or_else(|| anyhow!("Expected integer for {}", arg.name))?;
                Ok(SqlValue::Int(n))
            }
            ArgType::Decimal => {
                let n = value.as_f64()
                    .ok_or_else(|| anyhow!("Expected decimal for {}", arg.name))?;
                Ok(SqlValue::Float(n))
            }
            ArgType::Boolean => {
                let b = value.as_bool()
                    .ok_or_else(|| anyhow!("Expected boolean for {}", arg.name))?;
                Ok(SqlValue::Bool(b))
            }
            ArgType::Json => {
                Ok(SqlValue::Json(value.clone()))
            }
            ArgType::Lookup => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected string for lookup {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
            ArgType::StringList => {
                let arr = value.as_array()
                    .ok_or_else(|| anyhow!("Expected array for {}", arg.name))?;
                let strings: Vec<String> = arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                Ok(SqlValue::StringArray(strings))
            }
            _ => {
                // Date, Timestamp - treat as string for now
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected string for {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
        }
    }

    fn bind_value<'q>(&self, query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>, value: &SqlValue) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match value {
            SqlValue::String(s) => query.bind(s.clone()),
            SqlValue::Uuid(u) => query.bind(*u),
            SqlValue::Int(n) => query.bind(*n),
            SqlValue::Float(f) => query.bind(*f),
            SqlValue::Bool(b) => query.bind(*b),
            SqlValue::Json(j) => query.bind(j.clone()),
            SqlValue::StringArray(arr) => query.bind(arr.clone()),
            SqlValue::Null => query.bind(None::<String>),
        }
    }

    fn row_to_json(&self, row: &PgRow) -> Result<JsonValue> {
        use sqlx::Column;
        let mut map = serde_json::Map::new();

        for col in row.columns() {
            let name = col.name();
            let value: JsonValue = match col.type_info().name() {
                "UUID" => {
                    if let Ok(v) = row.try_get::<Uuid, _>(name) {
                        json!(v.to_string())
                    } else {
                        JsonValue::Null
                    }
                }
                "TEXT" | "VARCHAR" | "CHAR" | "NAME" => {
                    if let Ok(v) = row.try_get::<String, _>(name) {
                        json!(v)
                    } else {
                        JsonValue::Null
                    }
                }
                "INT4" | "INT8" | "INT2" => {
                    if let Ok(v) = row.try_get::<i64, _>(name) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<i32, _>(name) {
                        json!(v)
                    } else {
                        JsonValue::Null
                    }
                }
                "FLOAT4" | "FLOAT8" | "NUMERIC" => {
                    if let Ok(v) = row.try_get::<f64, _>(name) {
                        json!(v)
                    } else {
                        JsonValue::Null
                    }
                }
                "BOOL" => {
                    if let Ok(v) = row.try_get::<bool, _>(name) {
                        json!(v)
                    } else {
                        JsonValue::Null
                    }
                }
                "JSONB" | "JSON" => {
                    if let Ok(v) = row.try_get::<JsonValue, _>(name) {
                        v
                    } else {
                        JsonValue::Null
                    }
                }
                "TIMESTAMPTZ" | "TIMESTAMP" => {
                    if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(name) {
                        json!(v.to_rfc3339())
                    } else {
                        JsonValue::Null
                    }
                }
                _ => JsonValue::Null,
            };
            map.insert(name.to_string(), value);
        }

        Ok(JsonValue::Object(map))
    }
}

/// Internal SQL value representation
#[derive(Debug, Clone)]
enum SqlValue {
    String(String),
    Uuid(Uuid),
    Int(i64),
    Float(f64),
    Bool(bool),
    Json(JsonValue),
    StringArray(Vec<String>),
    Null,
}
```

---

## Part 6: Integration Points

### Update: `rust/src/dsl_v2/mod.rs`

Add these new module exports:

```rust
// Existing modules
pub mod parser;
pub mod compiler;
pub mod executor;
pub mod verbs;
pub mod verb_registry;
pub mod verb_schema;
// ... other existing modules

// NEW: YAML-driven configuration modules
pub mod config;
pub mod runtime_registry;
pub mod generic_executor;

// Re-exports for convenience
pub use config::{ConfigLoader, VerbsConfig, CsgRulesConfig};
pub use runtime_registry::{RuntimeVerbRegistry, SharedVerbRegistry, RuntimeVerb};
pub use generic_executor::{GenericCrudExecutor, ExecutionResult};
```

### Update: `rust/Cargo.toml`

Ensure serde_yaml is in dependencies:

```toml
[dependencies]
serde_yaml = "0.9"
chrono = { version = "0.4", features = ["serde"] }
# ... other dependencies
```

### Integration with MCP Server

Update server initialization to load YAML config:

```rust
// In rust/src/mcp/mod.rs or wherever McpServer is created

use crate::dsl_v2::config::ConfigLoader;
use crate::dsl_v2::runtime_registry::{RuntimeVerbRegistry, SharedVerbRegistry};
use crate::dsl_v2::generic_executor::GenericCrudExecutor;

pub async fn create_mcp_server(pool: PgPool) -> Result<McpServer> {
    // Load YAML configuration
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs()?;
    let csg_config = loader.load_csg_rules()?;

    // Build runtime registry (with dynamic verbs from DB)
    let registry = RuntimeVerbRegistry::from_config_with_db(&verbs_config, &pool).await?;
    let shared_registry = SharedVerbRegistry::new(registry);

    // Create generic executor
    let executor = GenericCrudExecutor::new(pool.clone());

    Ok(McpServer {
        pool,
        registry: shared_registry,
        executor,
        csg_config,
        // ... other fields
    })
}
```

---

## Part 7: Migration Strategy

### Phase 1: Infrastructure (Days 1-2)

1. Create `rust/config/` directory
2. Create `rust/config/verbs.yaml` from this spec
3. Create `rust/config/csg_rules.yaml` from this spec
4. Create `rust/src/dsl_v2/config/` module:
   - `mod.rs`
   - `types.rs`
   - `loader.rs`
   - `hot_reload.rs`
5. Add serde_yaml to Cargo.toml
6. `cargo build` - verify compilation

### Phase 2: Runtime Components (Days 3-4)

1. Create `rust/src/dsl_v2/runtime_registry.rs`
2. Create `rust/src/dsl_v2/generic_executor.rs`
3. Update `rust/src/dsl_v2/mod.rs` with new exports
4. Add unit tests for config loading
5. Add unit tests for registry building

### Phase 3: Integration (Days 5-6)

1. Modify existing `verb_registry.rs` to optionally delegate to RuntimeVerbRegistry
2. Update MCP server to load YAML config at startup
3. Add feature flag: `DSL_USE_YAML_CONFIG=true`
4. Run existing tests with flag off (should pass)
5. Run existing tests with flag on (should pass)

### Phase 4: Parallel Validation (Days 7-8)

1. Keep static `STANDARD_VERBS` in verbs.rs
2. Add validation that compares static vs YAML-loaded verbs
3. Log warnings for any mismatches
4. Fix YAML until zero mismatches

### Phase 5: Cutover (Days 9-10)

1. Remove feature flag - YAML is now default
2. Remove static verb arrays from `verbs.rs`
3. Remove static custom ops from `verb_registry.rs`
4. Update all documentation
5. Final test run

---

## Part 8: Testing Checklist

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_verbs_config() {
        let loader = ConfigLoader::new("config");
        let config = loader.load_verbs().expect("Failed to load verbs");

        assert!(config.domains.contains_key("cbu"));
        assert!(config.domains.contains_key("entity"));
        assert!(config.domains.contains_key("product"));
        assert!(config.domains.contains_key("service"));
        assert!(config.domains.contains_key("lifecycle-resource"));
    }

    #[test]
    fn test_build_registry() {
        let loader = ConfigLoader::new("config");
        let config = loader.load_verbs().unwrap();
        let registry = RuntimeVerbRegistry::from_config(&config);

        assert!(registry.get("cbu", "create").is_some());
        assert!(registry.get("cbu", "assign-role").is_some());
        assert!(registry.get("product", "ensure").is_some());
        assert!(registry.get("lifecycle-resource", "add-attribute").is_some());
    }

    #[test]
    fn test_verb_arg_types() {
        let loader = ConfigLoader::new("config");
        let config = loader.load_verbs().unwrap();
        let registry = RuntimeVerbRegistry::from_config(&config);

        let verb = registry.get("cbu", "assign-role").unwrap();
        let role_arg = verb.args.iter().find(|a| a.name == "role").unwrap();
        assert_eq!(role_arg.arg_type, ArgType::Lookup);
        assert!(role_arg.lookup.is_some());
    }

    #[tokio::test]
    async fn test_execute_insert() {
        let pool = create_test_pool().await;
        let loader = ConfigLoader::new("config");
        let config = loader.load_verbs().unwrap();
        let registry = RuntimeVerbRegistry::from_config(&config);
        let executor = GenericCrudExecutor::new(pool);

        let verb = registry.get("cbu", "create").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), json!("Test CBU"));
        args.insert("jurisdiction".to_string(), json!("GB"));

        let result = executor.execute(verb, &args).await.unwrap();
        match result {
            ExecutionResult::Uuid(id) => assert!(!id.is_nil()),
            _ => panic!("Expected UUID"),
        }
    }
}
```

---

## Summary

### Files to Create

| File | Lines | Purpose |
|------|-------|---------|
| `config/verbs.yaml` | ~1200 | All verb definitions |
| `config/csg_rules.yaml` | ~150 | CSG validation rules |
| `src/dsl_v2/config/mod.rs` | ~20 | Module exports |
| `src/dsl_v2/config/types.rs` | ~300 | Serde structs |
| `src/dsl_v2/config/loader.rs` | ~120 | YAML loading |
| `src/dsl_v2/config/hot_reload.rs` | ~80 | Hot reload |
| `src/dsl_v2/runtime_registry.rs` | ~400 | Runtime registry |
| `src/dsl_v2/generic_executor.rs` | ~600 | Generic executor |

### Files to Modify

| File | Change |
|------|--------|
| `src/dsl_v2/mod.rs` | Add module exports |
| `src/mcp/mod.rs` | Load config at startup |
| `Cargo.toml` | Add serde_yaml |

### Files to Eventually Remove

| File | Content to Remove |
|------|-------------------|
| `src/dsl_v2/verbs.rs` | STANDARD_VERBS static array |
| `src/dsl_v2/verb_registry.rs` | custom_ops_definitions() static function |

### Benefits

| Before | After |
|--------|-------|
| Edit Rust + recompile | Edit YAML + restart |
| 10-30 min to add verb | 2 min to add verb |
| Build errors from typos | YAML validation errors |
| No hot-reload | Optional hot-reload |

### Command to Add New Verb (Post-Implementation)

```bash
# 1. Edit YAML
vim config/verbs.yaml

# 2. Restart server
cargo run --bin mcp-server

# OR (if hot-reload enabled)
curl -X POST http://localhost:8080/admin/reload-config
```

**No Rust code. No recompilation.**
