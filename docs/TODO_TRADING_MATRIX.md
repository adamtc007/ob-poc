# Trading Instrument Matrix - Implementation TODO

> **Priority**: CRITICAL - Core onboarding dependency
> **Status**: Gap Analysis Complete - Implementation Required
> **Last Updated**: 2024-12-31

## Overview

The Trading Instrument Matrix is the single source of truth for CBU trading configuration. It answers:
- **What** does the CBU trade? (Universe)
- **How** are trades instructed? (Instruction Profile)
- **Where** do trades flow? (Gateway Routing)
- **How** do trades settle? (SSI + Booking Rules)
- **What** lifecycle events apply? (Corporate Actions, Income, Tax)
- **How** are holdings valued? (Pricing Configuration)

The output is a comprehensive YAML/JSON document capturing the complete operational profile.

---

## Phase 1: Instruction Profile & Gateway Routing

### 1.1 Create `instruction-profile.yaml` Domain

**File**: `rust/config/verbs/custody/instruction-profile.yaml`

**Purpose**: Define HOW trades are instructed to external systems (message types, templates, field mappings)

```yaml
domains:
  instruction-profile:
    description: "Trade instruction message configuration - defines how trades are communicated to gateways"
    
    verbs:
      # =========================================================================
      # MESSAGE TYPE DEFINITIONS
      # =========================================================================
      
      define-message-type:
        description: "Define instruction message type for a lifecycle event"
        behavior: crud
        crud:
          operation: upsert
          table: instruction_message_types
          schema: custody
          conflict_keys:
            - lifecycle_event
            - message_standard
            - message_type
          returning: message_type_id
        args:
          - name: lifecycle-event
            type: string
            required: true
            maps_to: lifecycle_event
            valid_values:
              - TRADE_INSTRUCTION
              - SETTLEMENT_INSTRUCTION
              - CONFIRMATION
              - AFFIRMATION
              - ALLOCATION
              - CA_INSTRUCTION
              - CA_RESPONSE
              - COLLATERAL_CALL
              - COLLATERAL_RESPONSE
              - INCOME_INSTRUCTION
              - TAX_RECLAIM
          - name: message-standard
            type: string
            required: true
            maps_to: message_standard
            valid_values:
              - MT                    # SWIFT MT (legacy)
              - MX                    # SWIFT ISO 20022
              - FIX                   # FIX Protocol
              - FPML                  # FpML for derivatives
              - PROPRIETARY          # Proprietary format
          - name: message-type
            type: string
            required: true
            maps_to: message_type
            description: "e.g., MT540, MT542, sese.023.001.09, FIX NewOrderSingle"
          - name: direction
            type: string
            required: true
            maps_to: direction
            valid_values:
              - SEND
              - RECEIVE
              - BOTH
          - name: description
            type: string
            required: false
            maps_to: description
          - name: schema-version
            type: string
            required: false
            maps_to: schema_version
            description: "Message schema version (e.g., SR2023 for SWIFT)"
        returns:
          type: uuid
          name: message_type_id
          capture: true

      list-message-types:
        description: "List available message types"
        behavior: crud
        crud:
          operation: select
          table: instruction_message_types
          schema: custody
        args:
          - name: lifecycle-event
            type: string
            required: false
            maps_to: lifecycle_event
          - name: message-standard
            type: string
            required: false
            maps_to: message_standard
        returns:
          type: record_set

      # =========================================================================
      # TEMPLATE DEFINITIONS
      # =========================================================================

      create-template:
        description: "Create instruction message template"
        behavior: crud
        crud:
          operation: upsert
          table: instruction_templates
          schema: custody
          conflict_keys:
            - template_code
          returning: template_id
        args:
          - name: code
            type: string
            required: true
            maps_to: template_code
          - name: name
            type: string
            required: true
            maps_to: template_name
          - name: message-type-id
            type: uuid
            required: true
            maps_to: message_type_id
            lookup:
              table: instruction_message_types
              schema: custody
              search_key: message_type
              primary_key: message_type_id
          - name: base-template
            type: json
            required: true
            maps_to: base_template
            description: "JSON template with placeholders"
          - name: field-mappings
            type: json
            required: false
            maps_to: field_mappings
            description: "Map placeholders to data sources"
          - name: validation-rules
            type: json
            required: false
            maps_to: validation_rules
        returns:
          type: uuid
          name: template_id
          capture: true

      read-template:
        description: "Read instruction template"
        behavior: crud
        crud:
          operation: select
          table: instruction_templates
          schema: custody
        args:
          - name: template-id
            type: uuid
            required: false
            maps_to: template_id
          - name: code
            type: string
            required: false
            maps_to: template_code
        returns:
          type: record

      list-templates:
        description: "List instruction templates"
        behavior: crud
        crud:
          operation: select
          table: instruction_templates
          schema: custody
        args:
          - name: message-type-id
            type: uuid
            required: false
            maps_to: message_type_id
        returns:
          type: record_set

      # =========================================================================
      # CBU TEMPLATE ASSIGNMENTS
      # =========================================================================

      assign-template:
        description: "Assign instruction template to CBU for instrument/market/event"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_instruction_assignments
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
            - market_id
            - lifecycle_event
            - counterparty_entity_id
          returning: assignment_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: template-id
            type: uuid
            required: true
            maps_to: template_id
            lookup:
              table: instruction_templates
              schema: custody
              search_key: template_code
              primary_key: template_id
          - name: lifecycle-event
            type: string
            required: true
            maps_to: lifecycle_event
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: counterparty
            type: uuid
            required: false
            maps_to: counterparty_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: priority
            type: integer
            required: true
            maps_to: priority
            default: 50
          - name: effective-date
            type: date
            required: false
            maps_to: effective_date
        returns:
          type: uuid
          name: assignment_id
          capture: true

      list-assignments:
        description: "List instruction template assignments for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_instruction_assignments
          schema: custody
          fk_col: cbu_id
          order_by: priority
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: lifecycle-event
            type: string
            required: false
            maps_to: lifecycle_event
        returns:
          type: record_set

      remove-assignment:
        description: "Remove template assignment"
        behavior: crud
        crud:
          operation: delete
          table: cbu_instruction_assignments
          schema: custody
        args:
          - name: assignment-id
            type: uuid
            required: true
            maps_to: assignment_id
        returns:
          type: affected

      # =========================================================================
      # FIELD OVERRIDES
      # =========================================================================

      add-field-override:
        description: "Add field-level override to CBU's instruction profile"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_instruction_field_overrides
          schema: custody
          conflict_keys:
            - assignment_id
            - field_path
          returning: override_id
        args:
          - name: assignment-id
            type: uuid
            required: true
            maps_to: assignment_id
          - name: field-path
            type: string
            required: true
            maps_to: field_path
            description: "JSON path or SWIFT tag path (e.g., '95P/REAG/BIC')"
          - name: override-type
            type: string
            required: true
            maps_to: override_type
            valid_values:
              - STATIC            # Fixed value
              - DERIVED           # Computed from data
              - CONDITIONAL       # Based on conditions
              - SUPPRESS          # Remove field
          - name: override-value
            type: string
            required: false
            maps_to: override_value
          - name: derivation-rule
            type: json
            required: false
            maps_to: derivation_rule
            description: "Rule for DERIVED/CONDITIONAL types"
          - name: reason
            type: string
            required: false
            maps_to: reason
        returns:
          type: uuid
          name: override_id
          capture: true

      list-field-overrides:
        description: "List field overrides for assignment"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_instruction_field_overrides
          schema: custody
          fk_col: assignment_id
        args:
          - name: assignment-id
            type: uuid
            required: true
            maps_to: assignment_id
        returns:
          type: record_set

      remove-field-override:
        description: "Remove field override"
        behavior: crud
        crud:
          operation: delete
          table: cbu_instruction_field_overrides
          schema: custody
        args:
          - name: override-id
            type: uuid
            required: true
            maps_to: override_id
        returns:
          type: affected

      # =========================================================================
      # RESOLUTION & VALIDATION
      # =========================================================================

      find-template:
        description: "Find applicable template for trade characteristics"
        behavior: plugin
        handler: find_instruction_template
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: lifecycle-event
            type: string
            required: true
          - name: instrument-class
            type: string
            required: false
          - name: market
            type: string
            required: false
          - name: counterparty-bic
            type: string
            required: false
        returns:
          type: record
          description: "Matching template with resolved field overrides"

      validate-profile:
        description: "Validate instruction profile completeness"
        behavior: plugin
        handler: validate_instruction_profile
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          description: "{ complete: bool, missing_assignments: [...], warnings: [...] }"

      derive-required-templates:
        description: "Derive required templates from universe"
        behavior: plugin
        handler: derive_required_instruction_templates
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set
          description: "List of required template assignments based on universe"
```

### 1.2 Create `trade-gateway.yaml` Domain

**File**: `rust/config/verbs/custody/trade-gateway.yaml`

**Purpose**: Define WHERE trades flow (gateway definitions, routing rules, connectivity)

```yaml
domains:
  trade-gateway:
    description: "Trade gateway configuration - defines routing to external systems"
    
    verbs:
      # =========================================================================
      # GATEWAY DEFINITIONS (Reference Data)
      # =========================================================================
      
      define-gateway:
        description: "Define available trade gateway"
        behavior: crud
        crud:
          operation: upsert
          table: trade_gateways
          schema: custody
          conflict_keys:
            - gateway_code
          returning: gateway_id
        args:
          - name: code
            type: string
            required: true
            maps_to: gateway_code
          - name: name
            type: string
            required: true
            maps_to: gateway_name
          - name: gateway-type
            type: string
            required: true
            maps_to: gateway_type
            valid_values:
              - SWIFT_FIN           # SWIFT FIN (MT messages)
              - SWIFT_INTERACT      # SWIFT InterAct (MX messages)
              - FIX                  # FIX Protocol
              - OMGEO_CTM           # Omgeo CTM
              - OMGEO_ALERT         # Omgeo ALERT
              - BLOOMBERG_TOMS      # Bloomberg TOMS
              - TRADEWEB            # Tradeweb
              - MARKITWIRE          # MarkitWire
              - DTCC_GTR            # DTCC GTR (derivatives reporting)
              - PROPRIETARY         # Direct/proprietary connection
              - MANUAL              # Manual processing
          - name: protocol
            type: string
            required: true
            maps_to: protocol
            valid_values:
              - MT
              - MX
              - FIX_4_2
              - FIX_4_4
              - FIX_5_0
              - FPML
              - REST
              - SOAP
              - FILE
              - MANUAL
          - name: provider
            type: string
            required: false
            maps_to: provider
            description: "SWIFT, BLOOMBERG, REFINITIV, DIRECT"
          - name: supported-events
            type: string_list
            required: true
            maps_to: supported_events
            description: "Lifecycle events this gateway supports"
          - name: description
            type: string
            required: false
            maps_to: description
        returns:
          type: uuid
          name: gateway_id
          capture: true

      read-gateway:
        description: "Read gateway definition"
        behavior: crud
        crud:
          operation: select
          table: trade_gateways
          schema: custody
        args:
          - name: gateway-id
            type: uuid
            required: false
            maps_to: gateway_id
          - name: code
            type: string
            required: false
            maps_to: gateway_code
        returns:
          type: record

      list-gateways:
        description: "List available gateways"
        behavior: crud
        crud:
          operation: select
          table: trade_gateways
          schema: custody
        args:
          - name: gateway-type
            type: string
            required: false
            maps_to: gateway_type
          - name: protocol
            type: string
            required: false
            maps_to: protocol
        returns:
          type: record_set

      # =========================================================================
      # CBU GATEWAY CONNECTIVITY
      # =========================================================================

      enable-gateway:
        description: "Enable gateway connectivity for CBU"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_gateway_connectivity
          schema: custody
          conflict_keys:
            - cbu_id
            - gateway_id
          returning: connectivity_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: gateway-id
            type: uuid
            required: true
            maps_to: gateway_id
            lookup:
              table: trade_gateways
              schema: custody
              search_key: gateway_code
              primary_key: gateway_id
          - name: status
            type: string
            required: false
            maps_to: status
            default: PENDING
            valid_values:
              - PENDING
              - TESTING
              - ACTIVE
              - SUSPENDED
              - DECOMMISSIONED
          - name: connectivity-resource-id
            type: uuid
            required: false
            maps_to: connectivity_resource_id
            lookup:
              table: cbu_resource_instances
              entity_type: resource_instance
              schema: ob-poc
              search_key: instance_name
              primary_key: instance_id
            description: "Link to provisioned connectivity resource"
          - name: credentials-reference
            type: string
            required: false
            maps_to: credentials_reference
            description: "Reference to secure credentials store"
          - name: effective-date
            type: date
            required: false
            maps_to: effective_date
          - name: config
            type: json
            required: false
            maps_to: gateway_config
            description: "Gateway-specific configuration"
        returns:
          type: uuid
          name: connectivity_id
          capture: true

      activate-gateway:
        description: "Activate gateway connectivity"
        behavior: crud
        crud:
          operation: update
          table: cbu_gateway_connectivity
          schema: custody
          key: connectivity_id
          set_values:
            status: ACTIVE
            activated_at: now()
        args:
          - name: connectivity-id
            type: uuid
            required: true
            maps_to: connectivity_id
        returns:
          type: affected

      suspend-gateway:
        description: "Suspend gateway connectivity"
        behavior: crud
        crud:
          operation: update
          table: cbu_gateway_connectivity
          schema: custody
          key: connectivity_id
          set_values:
            status: SUSPENDED
            suspended_at: now()
        args:
          - name: connectivity-id
            type: uuid
            required: true
            maps_to: connectivity_id
          - name: reason
            type: string
            required: false
        returns:
          type: affected

      list-cbu-gateways:
        description: "List gateway connectivity for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_gateway_connectivity
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: status
            type: string
            required: false
            maps_to: status
        returns:
          type: record_set

      # =========================================================================
      # GATEWAY ROUTING RULES
      # =========================================================================

      add-routing-rule:
        description: "Add gateway routing rule for CBU"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_gateway_routing
          schema: custody
          conflict_keys:
            - cbu_id
            - gateway_id
            - lifecycle_event
            - instrument_class_id
            - market_id
            - counterparty_entity_id
          returning: routing_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: gateway-id
            type: uuid
            required: true
            maps_to: gateway_id
            lookup:
              table: trade_gateways
              schema: custody
              search_key: gateway_code
              primary_key: gateway_id
          - name: lifecycle-event
            type: string
            required: true
            maps_to: lifecycle_event
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: counterparty
            type: uuid
            required: false
            maps_to: counterparty_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: priority
            type: integer
            required: true
            maps_to: priority
            default: 50
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
            default: true
        returns:
          type: uuid
          name: routing_id
          capture: true

      list-routing-rules:
        description: "List gateway routing rules for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_gateway_routing
          schema: custody
          fk_col: cbu_id
          order_by: priority
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: lifecycle-event
            type: string
            required: false
            maps_to: lifecycle_event
          - name: is-active
            type: boolean
            required: false
            maps_to: is_active
        returns:
          type: record_set

      remove-routing-rule:
        description: "Remove routing rule"
        behavior: crud
        crud:
          operation: delete
          table: cbu_gateway_routing
          schema: custody
        args:
          - name: routing-id
            type: uuid
            required: true
            maps_to: routing_id
        returns:
          type: affected

      # =========================================================================
      # FALLBACK CONFIGURATION
      # =========================================================================

      set-fallback:
        description: "Configure fallback gateway"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_gateway_fallbacks
          schema: custody
          conflict_keys:
            - cbu_id
            - primary_gateway_id
            - lifecycle_event
          returning: fallback_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: primary-gateway-id
            type: uuid
            required: true
            maps_to: primary_gateway_id
            lookup:
              table: trade_gateways
              schema: custody
              search_key: gateway_code
              primary_key: gateway_id
          - name: fallback-gateway-id
            type: uuid
            required: true
            maps_to: fallback_gateway_id
            lookup:
              table: trade_gateways
              schema: custody
              search_key: gateway_code
              primary_key: gateway_id
          - name: lifecycle-event
            type: string
            required: false
            maps_to: lifecycle_event
            description: "NULL = all events"
          - name: trigger-conditions
            type: string_list
            required: true
            maps_to: trigger_conditions
            description: "TIMEOUT, ERROR, REJECTION, MANUAL"
          - name: priority
            type: integer
            required: true
            maps_to: priority
        returns:
          type: uuid
          name: fallback_id
          capture: true

      list-fallbacks:
        description: "List fallback configurations"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_gateway_fallbacks
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # RESOLUTION & VALIDATION
      # =========================================================================

      find-gateway:
        description: "Find applicable gateway for trade"
        behavior: plugin
        handler: find_trade_gateway
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: lifecycle-event
            type: string
            required: true
          - name: instrument-class
            type: string
            required: false
          - name: market
            type: string
            required: false
          - name: counterparty-bic
            type: string
            required: false
        returns:
          type: record
          description: "Matching gateway with fallback chain"

      validate-routing:
        description: "Validate gateway routing completeness"
        behavior: plugin
        handler: validate_gateway_routing
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          description: "{ complete: bool, missing_routes: [...], inactive_gateways: [...] }"

      derive-required-routes:
        description: "Derive required gateway routes from universe"
        behavior: plugin
        handler: derive_required_gateway_routes
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set
```

### 1.3 Database Schema for Phase 1

**File**: `schema/migrations/YYYYMMDD_instruction_gateway.sql`

```sql
-- =============================================================================
-- INSTRUCTION PROFILE TABLES
-- =============================================================================

-- Message type definitions (reference data)
CREATE TABLE custody.instruction_message_types (
    message_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lifecycle_event VARCHAR(50) NOT NULL,
    message_standard VARCHAR(20) NOT NULL,
    message_type VARCHAR(50) NOT NULL,
    direction VARCHAR(10) NOT NULL,
    description TEXT,
    schema_version VARCHAR(20),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(lifecycle_event, message_standard, message_type)
);

-- Instruction templates
CREATE TABLE custody.instruction_templates (
    template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_code VARCHAR(50) NOT NULL UNIQUE,
    template_name VARCHAR(255) NOT NULL,
    message_type_id UUID NOT NULL REFERENCES custody.instruction_message_types(message_type_id),
    base_template JSONB NOT NULL,
    field_mappings JSONB,
    validation_rules JSONB,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- CBU template assignments (which template for which instrument/market/event)
CREATE TABLE custody.cbu_instruction_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    template_id UUID NOT NULL REFERENCES custody.instruction_templates(template_id),
    lifecycle_event VARCHAR(50) NOT NULL,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    priority INTEGER NOT NULL DEFAULT 50,
    effective_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id, lifecycle_event, counterparty_entity_id)
);

-- Field-level overrides
CREATE TABLE custody.cbu_instruction_field_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    assignment_id UUID NOT NULL REFERENCES custody.cbu_instruction_assignments(assignment_id) ON DELETE CASCADE,
    field_path VARCHAR(255) NOT NULL,
    override_type VARCHAR(20) NOT NULL,
    override_value TEXT,
    derivation_rule JSONB,
    reason TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(assignment_id, field_path)
);

-- Indexes
CREATE INDEX idx_cbu_instruction_assignments_cbu ON custody.cbu_instruction_assignments(cbu_id);
CREATE INDEX idx_cbu_instruction_assignments_lookup ON custody.cbu_instruction_assignments(cbu_id, lifecycle_event, instrument_class_id, market_id);

-- =============================================================================
-- TRADE GATEWAY TABLES
-- =============================================================================

-- Gateway definitions (reference data)
CREATE TABLE custody.trade_gateways (
    gateway_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    gateway_code VARCHAR(50) NOT NULL UNIQUE,
    gateway_name VARCHAR(255) NOT NULL,
    gateway_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(20) NOT NULL,
    provider VARCHAR(50),
    supported_events TEXT[] NOT NULL,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- CBU gateway connectivity
CREATE TABLE custody.cbu_gateway_connectivity (
    connectivity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    connectivity_resource_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    credentials_reference VARCHAR(255),
    effective_date DATE,
    activated_at TIMESTAMPTZ,
    suspended_at TIMESTAMPTZ,
    gateway_config JSONB,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, gateway_id)
);

-- Gateway routing rules
CREATE TABLE custody.cbu_gateway_routing (
    routing_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    lifecycle_event VARCHAR(50) NOT NULL,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    priority INTEGER NOT NULL DEFAULT 50,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, gateway_id, lifecycle_event, instrument_class_id, market_id, counterparty_entity_id)
);

-- Gateway fallback configuration
CREATE TABLE custody.cbu_gateway_fallbacks (
    fallback_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    primary_gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    fallback_gateway_id UUID NOT NULL REFERENCES custody.trade_gateways(gateway_id),
    lifecycle_event VARCHAR(50),
    trigger_conditions TEXT[] NOT NULL,
    priority INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, primary_gateway_id, lifecycle_event)
);

-- Indexes
CREATE INDEX idx_cbu_gateway_connectivity_cbu ON custody.cbu_gateway_connectivity(cbu_id);
CREATE INDEX idx_cbu_gateway_routing_cbu ON custody.cbu_gateway_routing(cbu_id);
CREATE INDEX idx_cbu_gateway_routing_lookup ON custody.cbu_gateway_routing(cbu_id, lifecycle_event, instrument_class_id, market_id);
```

### 1.4 Plugin Handlers for Phase 1

**File**: `rust/src/dsl_v2/custom_ops/instruction_gateway_ops.rs`

```rust
// TODO: Implement plugin handlers:
// - find_instruction_template: Priority-based template lookup with field override resolution
// - validate_instruction_profile: Check all universe entries have instruction coverage
// - derive_required_instruction_templates: Generate required assignments from universe
// - find_trade_gateway: Priority-based gateway lookup with fallback chain
// - validate_gateway_routing: Check all universe entries have gateway routes
// - derive_required_gateway_routes: Generate required routes from universe
```

---

## Phase 2: Corporate Actions & Asset Servicing

### 2.1 Create `corporate-action.yaml` Domain

**File**: `rust/config/verbs/custody/corporate-action.yaml`

```yaml
domains:
  corporate-action:
    description: "Corporate action processing configuration"
    
    verbs:
      # =========================================================================
      # CA EVENT TYPE REFERENCE DATA
      # =========================================================================
      
      define-event-type:
        description: "Define corporate action event type"
        behavior: crud
        crud:
          operation: upsert
          table: ca_event_types
          schema: custody
          conflict_keys:
            - event_code
          returning: event_type_id
        args:
          - name: code
            type: string
            required: true
            maps_to: event_code
          - name: name
            type: string
            required: true
            maps_to: event_name
          - name: category
            type: string
            required: true
            maps_to: category
            valid_values:
              - INCOME              # Dividends, interest, distributions
              - REORGANIZATION      # Mergers, splits, spinoffs
              - VOLUNTARY           # Tenders, rights, conversions
              - MANDATORY           # Name changes, stock dividends
              - INFORMATION         # AGM, proxy
          - name: is-elective
            type: boolean
            required: true
            maps_to: is_elective
          - name: default-election
            type: string
            required: false
            maps_to: default_election
            description: "CASH, STOCK, ROLLOVER, etc."
          - name: iso-event-code
            type: string
            required: false
            maps_to: iso_event_code
            description: "ISO 15022/20022 event code"
        returns:
          type: uuid
          name: event_type_id
          capture: true

      list-event-types:
        description: "List CA event types"
        behavior: crud
        crud:
          operation: select
          table: ca_event_types
          schema: custody
        args:
          - name: category
            type: string
            required: false
            maps_to: category
          - name: is-elective
            type: boolean
            required: false
            maps_to: is_elective
        returns:
          type: record_set

      # =========================================================================
      # CBU CA PREFERENCES
      # =========================================================================

      set-preferences:
        description: "Set corporate action processing preferences for CBU"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_ca_preferences
          schema: custody
          conflict_keys:
            - cbu_id
            - event_type_id
            - instrument_class_id
          returning: preference_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: event-type
            type: lookup
            required: true
            maps_to: event_type_id
            lookup:
              table: ca_event_types
              schema: custody
              code_column: event_code
              id_column: event_type_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: processing-mode
            type: string
            required: true
            maps_to: processing_mode
            valid_values:
              - AUTO_INSTRUCT       # Automatic instruction per default
              - MANUAL              # Always manual review
              - DEFAULT_ONLY        # Auto for default, manual for non-default
              - THRESHOLD           # Auto below threshold, manual above
          - name: default-election
            type: string
            required: false
            maps_to: default_election
            description: "Override event type default"
          - name: threshold-value
            type: decimal
            required: false
            maps_to: threshold_value
            description: "Value threshold for THRESHOLD mode"
          - name: threshold-currency
            type: string
            required: false
            maps_to: threshold_currency
          - name: notification-email
            type: string
            required: false
            maps_to: notification_email
        returns:
          type: uuid
          name: preference_id
          capture: true

      list-preferences:
        description: "List CA preferences for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_ca_preferences
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: event-type
            type: string
            required: false
        returns:
          type: record_set

      remove-preference:
        description: "Remove CA preference"
        behavior: crud
        crud:
          operation: delete
          table: cbu_ca_preferences
          schema: custody
        args:
          - name: preference-id
            type: uuid
            required: true
            maps_to: preference_id
        returns:
          type: affected

      # =========================================================================
      # INSTRUCTION WINDOWS
      # =========================================================================

      set-instruction-window:
        description: "Configure CA instruction deadline rules"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_ca_instruction_windows
          schema: custody
          conflict_keys:
            - cbu_id
            - event_type_id
            - market_id
          returning: window_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: event-type
            type: lookup
            required: true
            maps_to: event_type_id
            lookup:
              table: ca_event_types
              schema: custody
              code_column: event_code
              id_column: event_type_id
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: cutoff-days-before
            type: integer
            required: true
            maps_to: cutoff_days_before
            description: "Days before market deadline for internal cutoff"
          - name: warning-days
            type: integer
            required: false
            maps_to: warning_days
            default: 3
            description: "Days before internal cutoff for warning"
          - name: escalation-days
            type: integer
            required: false
            maps_to: escalation_days
            default: 1
            description: "Days before internal cutoff for escalation"
          - name: escalation-contact
            type: string
            required: false
            maps_to: escalation_contact
        returns:
          type: uuid
          name: window_id
          capture: true

      list-instruction-windows:
        description: "List instruction window configurations"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_ca_instruction_windows
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # CA SSI MAPPING
      # =========================================================================

      link-ca-ssi:
        description: "Link CA payment/delivery to specific SSI"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_ca_ssi_mappings
          schema: custody
          conflict_keys:
            - cbu_id
            - event_type_id
            - currency
          returning: mapping_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: event-type
            type: lookup
            required: true
            maps_to: event_type_id
            lookup:
              table: ca_event_types
              schema: custody
              code_column: event_code
              id_column: event_type_id
          - name: currency
            type: string
            required: true
            maps_to: currency
          - name: ssi-id
            type: uuid
            required: true
            maps_to: ssi_id
            lookup:
              table: cbu_ssi
              entity_type: ssi
              schema: custody
              search_key: ssi_name
              primary_key: ssi_id
        returns:
          type: uuid
          name: mapping_id
          capture: true

      list-ca-ssi-mappings:
        description: "List CA SSI mappings"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_ca_ssi_mappings
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # VALIDATION
      # =========================================================================

      validate-ca-config:
        description: "Validate corporate action configuration completeness"
        behavior: plugin
        handler: validate_ca_configuration
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          description: "{ complete: bool, missing_preferences: [...], missing_ssi_mappings: [...] }"

      derive-required-config:
        description: "Derive required CA config from universe"
        behavior: plugin
        handler: derive_required_ca_config
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set
```

### 2.2 Extend `pricing-config.yaml` Domain

**File**: `rust/config/verbs/pricing-config.yaml` (additions)

```yaml
# Add to existing pricing-config domain:

      # =========================================================================
      # VALUATION SCHEDULE
      # =========================================================================

      set-valuation-schedule:
        description: "Set position valuation frequency and timing"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_valuation_schedule
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
            - market_id
          returning: schedule_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: frequency
            type: string
            required: true
            maps_to: valuation_frequency
            valid_values:
              - REAL_TIME
              - INTRADAY
              - EOD
              - T_PLUS_1
              - WEEKLY
              - MONTHLY
          - name: valuation-time
            type: string
            required: false
            maps_to: valuation_time
            description: "HH:MM in market timezone"
          - name: timezone
            type: string
            required: false
            maps_to: timezone
          - name: business-days-only
            type: boolean
            required: false
            maps_to: business_days_only
            default: true
        returns:
          type: uuid
          name: schedule_id
          capture: true

      list-valuation-schedules:
        description: "List valuation schedules for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_valuation_schedule
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # FALLBACK CHAIN
      # =========================================================================

      set-fallback-chain:
        description: "Define multi-source pricing fallback chain"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_pricing_fallback_chains
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
            - market_id
          returning: chain_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: fallback-sources
            type: string_list
            required: true
            maps_to: fallback_sources
            description: "Ordered list of fallback sources"
          - name: fallback-trigger
            type: string
            required: true
            maps_to: fallback_trigger
            valid_values:
              - STALE
              - MISSING
              - THRESHOLD_BREACH
              - ANY_FAILURE
        returns:
          type: uuid
          name: chain_id
          capture: true

      # =========================================================================
      # STALE PRICE POLICY
      # =========================================================================

      set-stale-policy:
        description: "Configure stale price handling policy"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_stale_price_policies
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
          returning: policy_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: max-age-hours
            type: integer
            required: true
            maps_to: max_age_hours
          - name: stale-action
            type: string
            required: true
            maps_to: stale_action
            valid_values:
              - USE_LAST
              - USE_FALLBACK
              - ESCALATE
              - SUSPEND_NAV
              - MANUAL_OVERRIDE
          - name: escalation-contact
            type: string
            required: false
            maps_to: escalation_contact
        returns:
          type: uuid
          name: policy_id
          capture: true

      # =========================================================================
      # NAV IMPACT THRESHOLDS
      # =========================================================================

      set-nav-threshold:
        description: "Set NAV impact alert threshold"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_nav_impact_thresholds
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
          returning: threshold_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: threshold-pct
            type: decimal
            required: true
            maps_to: threshold_pct
            description: "Alert if price move exceeds this %"
          - name: action
            type: string
            required: true
            maps_to: threshold_action
            valid_values:
              - ALERT
              - MANUAL_REVIEW
              - SUSPEND_POSITION
              - ESCALATE
          - name: notification-email
            type: string
            required: false
            maps_to: notification_email
        returns:
          type: uuid
          name: threshold_id
          capture: true

      # =========================================================================
      # VALIDATION
      # =========================================================================

      validate-pricing-config:
        description: "Validate pricing configuration completeness"
        behavior: plugin
        handler: validate_pricing_configuration
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          description: "{ complete: bool, missing_sources: [...], missing_schedules: [...] }"
```

### 2.3 Database Schema for Phase 2

**File**: `schema/migrations/YYYYMMDD_corporate_actions_pricing.sql`

```sql
-- =============================================================================
-- CORPORATE ACTION TABLES
-- =============================================================================

-- CA event type definitions (reference data)
CREATE TABLE custody.ca_event_types (
    event_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_code VARCHAR(50) NOT NULL UNIQUE,
    event_name VARCHAR(255) NOT NULL,
    category VARCHAR(50) NOT NULL,
    is_elective BOOLEAN NOT NULL,
    default_election VARCHAR(50),
    iso_event_code VARCHAR(10),
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- CBU CA preferences
CREATE TABLE custody.cbu_ca_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    processing_mode VARCHAR(20) NOT NULL,
    default_election VARCHAR(50),
    threshold_value DECIMAL(18,2),
    threshold_currency VARCHAR(3),
    notification_email VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, event_type_id, instrument_class_id)
);

-- CA instruction windows
CREATE TABLE custody.cbu_ca_instruction_windows (
    window_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    cutoff_days_before INTEGER NOT NULL,
    warning_days INTEGER DEFAULT 3,
    escalation_days INTEGER DEFAULT 1,
    escalation_contact VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, event_type_id, market_id)
);

-- CA SSI mappings
CREATE TABLE custody.cbu_ca_ssi_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    currency VARCHAR(3) NOT NULL,
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, event_type_id, currency)
);

-- Indexes
CREATE INDEX idx_cbu_ca_preferences_cbu ON custody.cbu_ca_preferences(cbu_id);
CREATE INDEX idx_cbu_ca_instruction_windows_cbu ON custody.cbu_ca_instruction_windows(cbu_id);
CREATE INDEX idx_cbu_ca_ssi_mappings_cbu ON custody.cbu_ca_ssi_mappings(cbu_id);

-- =============================================================================
-- PRICING EXTENSION TABLES
-- =============================================================================

-- Valuation schedule
CREATE TABLE custody.cbu_valuation_schedule (
    schedule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    valuation_frequency VARCHAR(20) NOT NULL,
    valuation_time VARCHAR(10),
    timezone VARCHAR(50),
    business_days_only BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id)
);

-- Pricing fallback chains
CREATE TABLE custody.cbu_pricing_fallback_chains (
    chain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    fallback_sources TEXT[] NOT NULL,
    fallback_trigger VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id)
);

-- Stale price policies
CREATE TABLE custody.cbu_stale_price_policies (
    policy_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    max_age_hours INTEGER NOT NULL,
    stale_action VARCHAR(20) NOT NULL,
    escalation_contact VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id)
);

-- NAV impact thresholds
CREATE TABLE custody.cbu_nav_impact_thresholds (
    threshold_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    threshold_pct DECIMAL(5,2) NOT NULL,
    threshold_action VARCHAR(20) NOT NULL,
    notification_email VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id)
);
```

---

## Phase 3: Cross-Border Settlement & Tax

### 3.1 Extend `cbu-custody.yaml` for Settlement Chains

**Add to**: `rust/config/verbs/custody/cbu-custody.yaml`

```yaml
      # =========================================================================
      # SETTLEMENT CHAIN CONFIGURATION
      # =========================================================================

      define-settlement-chain:
        description: "Define multi-hop settlement chain for market"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_settlement_chains
          schema: custody
          conflict_keys:
            - cbu_id
            - market_id
            - settlement_type
            - currency
          returning: chain_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: market
            type: lookup
            required: true
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: settlement-type
            type: string
            required: true
            maps_to: settlement_type
            valid_values:
              - DVP
              - FOP
              - RVP
              - CROSS_BORDER
          - name: currency
            type: string
            required: false
            maps_to: currency
          - name: chain-steps
            type: json
            required: true
            maps_to: chain_steps
            description: "Array of {role, bic, account, name}"
        returns:
          type: uuid
          name: chain_id
          capture: true

      list-settlement-chains:
        description: "List settlement chains for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_settlement_chains
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: market
            type: string
            required: false
        returns:
          type: record_set

      # =========================================================================
      # FOP RULES
      # =========================================================================

      set-fop-rules:
        description: "Configure when FOP settlement is allowed"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_fop_rules
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
            - counterparty_entity_id
          returning: rule_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: counterparty
            type: uuid
            required: false
            maps_to: counterparty_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: fop-allowed
            type: boolean
            required: true
            maps_to: fop_allowed
          - name: fop-threshold
            type: decimal
            required: false
            maps_to: fop_threshold
            description: "Max value for FOP settlement"
          - name: threshold-currency
            type: string
            required: false
            maps_to: threshold_currency
          - name: approval-required
            type: boolean
            required: false
            maps_to: approval_required
            default: false
        returns:
          type: uuid
          name: rule_id
          capture: true

      list-fop-rules:
        description: "List FOP rules for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_fop_rules
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # CSD PREFERENCES
      # =========================================================================

      set-csd-preference:
        description: "Set CSD/ICSD preference for instrument/currency"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_csd_preferences
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
            - currency
          returning: preference_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: currency
            type: string
            required: true
            maps_to: currency
          - name: preferred-csd
            type: string
            required: true
            maps_to: preferred_csd
            description: "DTCC, EUROCLEAR, CLEARSTREAM, etc."
          - name: secondary-csd
            type: string
            required: false
            maps_to: secondary_csd
          - name: reason
            type: string
            required: false
            maps_to: reason
        returns:
          type: uuid
          name: preference_id
          capture: true

      list-csd-preferences:
        description: "List CSD preferences for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_csd_preferences
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # SETTLEMENT CYCLE OVERRIDES
      # =========================================================================

      set-settlement-cycle:
        description: "Override default settlement cycle"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_settlement_cycle_overrides
          schema: custody
          conflict_keys:
            - cbu_id
            - instrument_class_id
            - market_id
          returning: override_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: settlement-cycle
            type: string
            required: true
            maps_to: settlement_cycle
            valid_values:
              - T_0
              - T_1
              - T_2
              - T_3
              - T_5
          - name: effective-date
            type: date
            required: true
            maps_to: effective_date
          - name: reason
            type: string
            required: false
            maps_to: reason
        returns:
          type: uuid
          name: override_id
          capture: true

      list-settlement-cycle-overrides:
        description: "List settlement cycle overrides"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_settlement_cycle_overrides
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set
```

### 3.2 Create `tax-config.yaml` Domain

**File**: `rust/config/verbs/custody/tax-config.yaml`

```yaml
domains:
  tax-config:
    description: "Tax treatment and withholding configuration"
    
    verbs:
      # =========================================================================
      # WITHHOLDING TAX PROFILE
      # =========================================================================
      
      set-withholding-profile:
        description: "Set withholding tax profile for CBU/market"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_withholding_profiles
          schema: custody
          conflict_keys:
            - cbu_id
            - market_id
            - instrument_class_id
          returning: profile_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: market
            type: lookup
            required: true
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: statutory-rate
            type: decimal
            required: true
            maps_to: statutory_rate
            description: "Statutory withholding rate (e.g., 0.30 for 30%)"
          - name: treaty-rate
            type: decimal
            required: false
            maps_to: treaty_rate
            description: "Applicable treaty rate"
          - name: qi-status
            type: string
            required: false
            maps_to: qi_status
            valid_values:
              - QI                  # Qualified Intermediary
              - NQI                 # Non-Qualified Intermediary
              - QDD                 # Qualified Derivatives Dealer
              - NOT_APPLICABLE
          - name: documentation-status
            type: string
            required: false
            maps_to: documentation_status
            valid_values:
              - COMPLETE
              - PENDING
              - EXPIRED
              - NOT_REQUIRED
          - name: effective-date
            type: date
            required: true
            maps_to: effective_date
        returns:
          type: uuid
          name: profile_id
          capture: true

      list-withholding-profiles:
        description: "List withholding tax profiles"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_withholding_profiles
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # TAX RECLAIM PREFERENCES
      # =========================================================================

      set-reclaim-preferences:
        description: "Configure tax reclaim preferences"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_tax_reclaim_preferences
          schema: custody
          conflict_keys:
            - cbu_id
            - market_id
          returning: preference_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: market
            type: lookup
            required: true
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: reclaim-method
            type: string
            required: true
            maps_to: reclaim_method
            valid_values:
              - QUICK_REFUND        # Relief at source
              - STANDARD            # Standard reclaim process
              - LONG_FORM           # Long-form reclaim
              - NONE                # No reclaim
          - name: reclaim-threshold
            type: decimal
            required: false
            maps_to: reclaim_threshold
            description: "Minimum amount to initiate reclaim"
          - name: threshold-currency
            type: string
            required: false
            maps_to: threshold_currency
          - name: auto-claim
            type: boolean
            required: false
            maps_to: auto_claim
            default: false
        returns:
          type: uuid
          name: preference_id
          capture: true

      list-reclaim-preferences:
        description: "List tax reclaim preferences"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_tax_reclaim_preferences
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # TAX DOCUMENTATION
      # =========================================================================

      link-tax-documentation:
        description: "Link tax documentation to CBU profile"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_tax_documentation
          schema: custody
          conflict_keys:
            - cbu_id
            - document_type
            - market_id
          returning: doc_link_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: document-type
            type: string
            required: true
            maps_to: document_type
            valid_values:
              - W8_BEN
              - W8_BEN_E
              - W8_IMY
              - W8_ECI
              - W8_EXP
              - W9
              - CRS_SELF_CERT
              - FATCA_SELF_CERT
              - TAX_RESIDENCY_CERT
              - BENEFICIAL_OWNER_CERT
          - name: market
            type: lookup
            required: false
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
            description: "NULL = applies globally"
          - name: document-id
            type: uuid
            required: true
            maps_to: document_id
            lookup:
              table: document_catalog
              entity_type: document
              schema: ob-poc
              search_key: document_name
              primary_key: doc_id
          - name: effective-date
            type: date
            required: true
            maps_to: effective_date
          - name: expiry-date
            type: date
            required: false
            maps_to: expiry_date
        returns:
          type: uuid
          name: doc_link_id
          capture: true

      list-tax-documentation:
        description: "List tax documentation for CBU"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_tax_documentation
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: document-type
            type: string
            required: false
            maps_to: document_type
        returns:
          type: record_set

      # =========================================================================
      # RATE OVERRIDES
      # =========================================================================

      set-rate-override:
        description: "Override calculated withholding rate"
        behavior: crud
        crud:
          operation: upsert
          table: cbu_tax_rate_overrides
          schema: custody
          conflict_keys:
            - cbu_id
            - market_id
            - instrument_class_id
          returning: override_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: market
            type: lookup
            required: true
            maps_to: market_id
            lookup:
              table: markets
              entity_type: market
              schema: custody
              code_column: mic
              id_column: market_id
          - name: instrument-class
            type: lookup
            required: false
            maps_to: instrument_class_id
            lookup:
              table: instrument_classes
              entity_type: instrument_class
              schema: custody
              code_column: code
              id_column: class_id
          - name: override-rate
            type: decimal
            required: true
            maps_to: override_rate
          - name: override-reason
            type: string
            required: true
            maps_to: override_reason
          - name: approved-by
            type: string
            required: false
            maps_to: approved_by
          - name: effective-date
            type: date
            required: true
            maps_to: effective_date
          - name: expiry-date
            type: date
            required: false
            maps_to: expiry_date
        returns:
          type: uuid
          name: override_id
          capture: true

      list-rate-overrides:
        description: "List tax rate overrides"
        behavior: crud
        crud:
          operation: list_by_fk
          table: cbu_tax_rate_overrides
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record_set

      # =========================================================================
      # VALIDATION
      # =========================================================================

      validate-tax-config:
        description: "Validate tax configuration completeness"
        behavior: plugin
        handler: validate_tax_configuration
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          description: "{ complete: bool, missing_profiles: [...], expiring_docs: [...] }"

      find-withholding-rate:
        description: "Find applicable withholding rate for income event"
        behavior: plugin
        handler: find_withholding_rate
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: market
            type: string
            required: true
          - name: instrument-class
            type: string
            required: false
          - name: income-type
            type: string
            required: false
            description: "DIVIDEND, INTEREST, etc."
        returns:
          type: record
          description: "{ statutory_rate, treaty_rate, effective_rate, documentation_status }"
```

### 3.3 Database Schema for Phase 3

**File**: `schema/migrations/YYYYMMDD_settlement_tax.sql`

```sql
-- =============================================================================
-- SETTLEMENT EXTENSION TABLES
-- =============================================================================

-- Settlement chains (multi-hop)
CREATE TABLE custody.cbu_settlement_chains (
    chain_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    settlement_type VARCHAR(20) NOT NULL,
    currency VARCHAR(3),
    chain_steps JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, market_id, settlement_type, currency)
);

-- FOP rules
CREATE TABLE custody.cbu_fop_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    counterparty_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    fop_allowed BOOLEAN NOT NULL,
    fop_threshold DECIMAL(18,2),
    threshold_currency VARCHAR(3),
    approval_required BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, counterparty_entity_id)
);

-- CSD preferences
CREATE TABLE custody.cbu_csd_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    currency VARCHAR(3) NOT NULL,
    preferred_csd VARCHAR(50) NOT NULL,
    secondary_csd VARCHAR(50),
    reason TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, currency)
);

-- Settlement cycle overrides
CREATE TABLE custody.cbu_settlement_cycle_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    market_id UUID REFERENCES custody.markets(market_id),
    settlement_cycle VARCHAR(10) NOT NULL,
    effective_date DATE NOT NULL,
    reason TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, instrument_class_id, market_id)
);

-- =============================================================================
-- TAX CONFIGURATION TABLES
-- =============================================================================

-- Withholding tax profiles
CREATE TABLE custody.cbu_withholding_profiles (
    profile_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    statutory_rate DECIMAL(5,4) NOT NULL,
    treaty_rate DECIMAL(5,4),
    qi_status VARCHAR(20),
    documentation_status VARCHAR(20),
    effective_date DATE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, market_id, instrument_class_id)
);

-- Tax reclaim preferences
CREATE TABLE custody.cbu_tax_reclaim_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    reclaim_method VARCHAR(20) NOT NULL,
    reclaim_threshold DECIMAL(18,2),
    threshold_currency VARCHAR(3),
    auto_claim BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, market_id)
);

-- Tax documentation links
CREATE TABLE custody.cbu_tax_documentation (
    doc_link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    document_type VARCHAR(30) NOT NULL,
    market_id UUID REFERENCES custody.markets(market_id),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),
    effective_date DATE NOT NULL,
    expiry_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, document_type, market_id)
);

-- Tax rate overrides
CREATE TABLE custody.cbu_tax_rate_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    market_id UUID NOT NULL REFERENCES custody.markets(market_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    override_rate DECIMAL(5,4) NOT NULL,
    override_reason TEXT NOT NULL,
    approved_by VARCHAR(255),
    effective_date DATE NOT NULL,
    expiry_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, market_id, instrument_class_id)
);

-- Indexes
CREATE INDEX idx_cbu_withholding_profiles_cbu ON custody.cbu_withholding_profiles(cbu_id);
CREATE INDEX idx_cbu_tax_documentation_cbu ON custody.cbu_tax_documentation(cbu_id);
CREATE INDEX idx_cbu_tax_documentation_expiry ON custody.cbu_tax_documentation(expiry_date) WHERE expiry_date IS NOT NULL;
```

---

## Phase 4: Export & Document Generation

### 4.1 Extend `trading-profile.yaml` for Full Matrix Export

**Add to**: `rust/config/verbs/trading-profile.yaml`

```yaml
      # =========================================================================
      # FULL MATRIX EXPORT
      # =========================================================================

      export-full-matrix:
        description: "Export complete trading matrix document"
        behavior: plugin
        handler: export_full_trading_matrix
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: format
            type: string
            required: false
            default: YAML
            valid_values:
              - YAML
              - JSON
              - PDF
          - name: sections
            type: string_list
            required: false
            description: "Specific sections to include (default: ALL)"
            default:
              - universe
              - instruction_profile
              - gateway_routing
              - ssis
              - booking_rules
              - corporate_actions
              - pricing
              - tax
              - isda
              - subcustodians
          - name: include-gaps
            type: boolean
            required: false
            default: true
            description: "Include gap analysis results"
          - name: include-dependencies
            type: boolean
            required: false
            default: true
            description: "Include resource dependency graph"
          - name: as-of-date
            type: date
            required: false
            description: "Point-in-time export (default: now)"
        returns:
          type: record
          description: "{ document: string, format: string, sections: [...], gaps: [...] }"

      export-instruction-section:
        description: "Export instruction profile section only"
        behavior: plugin
        handler: export_instruction_profile_section
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: format
            type: string
            required: false
            default: YAML
        returns:
          type: record

      export-settlement-section:
        description: "Export settlement configuration section only"
        behavior: plugin
        handler: export_settlement_section
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: format
            type: string
            required: false
            default: YAML
        returns:
          type: record

      export-pricing-section:
        description: "Export pricing configuration section only"
        behavior: plugin
        handler: export_pricing_section
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: format
            type: string
            required: false
            default: YAML
        returns:
          type: record

      export-tax-section:
        description: "Export tax configuration section only"
        behavior: plugin
        handler: export_tax_section
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: format
            type: string
            required: false
            default: YAML
        returns:
          type: record

      # =========================================================================
      # VALIDATION & GAP ANALYSIS
      # =========================================================================

      validate-matrix-completeness:
        description: "Validate entire trading matrix is complete for go-live"
        behavior: plugin
        handler: validate_trading_matrix_completeness
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: validation-level
            type: string
            required: false
            default: STRICT
            valid_values:
              - STRICT            # All gaps are errors
              - STANDARD          # Critical gaps are errors, others warnings
              - PERMISSIVE        # All gaps are warnings
        returns:
          type: record
          description: |
            {
              ready_for_golive: bool,
              critical_gaps: [...],
              warnings: [...],
              coverage_summary: {
                universe_coverage: 0.95,
                instruction_coverage: 0.90,
                gateway_coverage: 1.0,
                ssi_coverage: 0.95,
                pricing_coverage: 1.0,
                tax_coverage: 0.80
              }
            }

      generate-gap-remediation-plan:
        description: "Generate DSL plan to remediate matrix gaps"
        behavior: plugin
        handler: generate_matrix_remediation_plan
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: priority
            type: string
            required: false
            default: CRITICAL_FIRST
            valid_values:
              - CRITICAL_FIRST
              - BY_SECTION
              - DEPENDENCY_ORDER
        returns:
          type: record
          description: |
            {
              dsl_statements: [...],
              pending_inputs: [...],   # Questions for user
              estimated_effort: "2 hours"
            }
```

### 4.2 Plugin Handlers for Phase 4

**File**: `rust/src/dsl_v2/custom_ops/trading_matrix_export.rs`

```rust
// TODO: Implement plugin handlers:
// - export_full_trading_matrix: Aggregate all sections into single document
// - export_instruction_profile_section: Extract instruction config
// - export_settlement_section: Extract SSI/booking/settlement config
// - export_pricing_section: Extract pricing/valuation config
// - export_tax_section: Extract tax/withholding config
// - validate_trading_matrix_completeness: Cross-domain validation
// - generate_matrix_remediation_plan: DSL generation for gaps
```

---

## Implementation Checklist

### Phase 1: Instruction & Gateway (Week 1-2)

- [ ] Create `rust/config/verbs/custody/instruction-profile.yaml`
- [ ] Create `rust/config/verbs/custody/trade-gateway.yaml`
- [ ] Create database migration for instruction/gateway tables
- [ ] Run migration: `psql -d data_designer -f migrations/YYYYMMDD_instruction_gateway.sql`
- [ ] Implement plugin handler: `find_instruction_template`
- [ ] Implement plugin handler: `validate_instruction_profile`
- [ ] Implement plugin handler: `derive_required_instruction_templates`
- [ ] Implement plugin handler: `find_trade_gateway`
- [ ] Implement plugin handler: `validate_gateway_routing`
- [ ] Implement plugin handler: `derive_required_gateway_routes`
- [ ] Add EntityGateway entries for new entity types
- [ ] Write unit tests for instruction profile resolution
- [ ] Write unit tests for gateway routing resolution
- [ ] Write integration tests for full flow

### Phase 2: Corporate Actions & Pricing (Week 2-3)

- [ ] Create `rust/config/verbs/custody/corporate-action.yaml`
- [ ] Extend `rust/config/verbs/pricing-config.yaml`
- [ ] Create database migration for CA/pricing tables
- [ ] Run migration
- [ ] Implement plugin handler: `validate_ca_configuration`
- [ ] Implement plugin handler: `derive_required_ca_config`
- [ ] Implement plugin handler: `validate_pricing_configuration`
- [ ] Seed CA event types reference data
- [ ] Write unit tests
- [ ] Write integration tests

### Phase 3: Settlement & Tax (Week 3-4)

- [ ] Extend `rust/config/verbs/custody/cbu-custody.yaml`
- [ ] Create `rust/config/verbs/custody/tax-config.yaml`
- [ ] Create database migration for settlement/tax tables
- [ ] Run migration
- [ ] Implement plugin handler: `validate_tax_configuration`
- [ ] Implement plugin handler: `find_withholding_rate`
- [ ] Write unit tests
- [ ] Write integration tests

### Phase 4: Export & Document Generation (Week 4-5)

- [ ] Extend `rust/config/verbs/trading-profile.yaml`
- [ ] Implement plugin handler: `export_full_trading_matrix`
- [ ] Implement plugin handler: `export_instruction_profile_section`
- [ ] Implement plugin handler: `export_settlement_section`
- [ ] Implement plugin handler: `export_pricing_section`
- [ ] Implement plugin handler: `export_tax_section`
- [ ] Implement plugin handler: `validate_trading_matrix_completeness`
- [ ] Implement plugin handler: `generate_matrix_remediation_plan`
- [ ] Create example trading matrix documents
- [ ] Write integration tests for full export cycle
- [ ] Update CLAUDE.md documentation

### Reference Data Seeding

- [ ] Seed `custody.instruction_message_types` with MT/MX message types
- [ ] Seed `custody.trade_gateways` with standard gateways
- [ ] Seed `custody.ca_event_types` with ISO CA event codes
- [ ] Create example instruction templates for common scenarios

---

## Trading Matrix Document Structure

The final exported document should have this structure:

```yaml
# Trading Matrix Document
# CBU: Luxembourg Growth Fund
# Generated: 2024-12-31T12:00:00Z
# Version: 1.0

metadata:
  cbu_id: "uuid..."
  cbu_name: "Luxembourg Growth Fund"
  jurisdiction: "LU"
  generated_at: "2024-12-31T12:00:00Z"
  version: 1
  status: ACTIVE

universe:
  base_currency: EUR
  allowed_currencies: [EUR, USD, GBP, CHF]
  entries:
    - instrument_class: EQUITY
      market: XETR
      currencies: [EUR]
      settlement_types: [DVP]
      is_held: true
      is_traded: true
    - instrument_class: GOVT_BOND
      market: XFRA
      currencies: [EUR]
      settlement_types: [DVP, FOP]
      is_held: true
      is_traded: true

instruction_profile:
  assignments:
    - lifecycle_event: SETTLEMENT_INSTRUCTION
      instrument_class: EQUITY
      market: XETR
      template: MT540_DELIVERY_XETR
      priority: 10
  field_overrides:
    - assignment: MT540_DELIVERY_XETR
      field_path: "95P/REAG/BIC"
      override_type: STATIC
      value: "DEUTDEFF"

gateway_routing:
  connectivity:
    - gateway: SWIFT_FIN
      status: ACTIVE
      connectivity_resource: "swift-conn-001"
  routes:
    - lifecycle_event: SETTLEMENT_INSTRUCTION
      instrument_class: EQUITY
      gateway: SWIFT_FIN
      priority: 10
  fallbacks:
    - primary: SWIFT_FIN
      fallback: MANUAL
      triggers: [TIMEOUT, ERROR]

standing_instructions:
  SECURITIES:
    - name: DE_EQUITY_SSI
      market: XETR
      currency: EUR
      safekeeping_account: "DE-DEPOT-001"
      safekeeping_bic: "DEUTDEFF"
      pset_bic: "CBLLDEFF"
      status: ACTIVE
  CASH:
    - name: EUR_CASH_SSI
      currency: EUR
      cash_account: "EUR-CASH-001"
      cash_bic: "DEUTDEFF"
      status: ACTIVE

booking_rules:
  - name: "German Equities"
    priority: 10
    match:
      instrument_class: EQUITY
      market: XETR
      currency: EUR
    ssi_ref: DE_EQUITY_SSI

corporate_actions:
  preferences:
    - event_type: DIVIDEND
      processing_mode: AUTO_INSTRUCT
      default_election: CASH
    - event_type: TENDER
      processing_mode: MANUAL
  instruction_windows:
    - event_type: TENDER
      market: XETR
      cutoff_days_before: 5
      warning_days: 3
  ssi_mappings:
    - event_type: DIVIDEND
      currency: EUR
      ssi_ref: EUR_CASH_SSI

pricing:
  sources:
    - instrument_class: EQUITY
      source: BLOOMBERG
      price_type: CLOSING
      priority: 1
    - instrument_class: EQUITY
      source: REUTERS
      price_type: CLOSING
      priority: 2
  valuation_schedule:
    - instrument_class: EQUITY
      frequency: EOD
      valuation_time: "18:00"
      timezone: "Europe/Berlin"
  stale_policies:
    - instrument_class: EQUITY
      max_age_hours: 24
      stale_action: USE_FALLBACK

tax:
  withholding_profiles:
    - market: XETR
      statutory_rate: 0.2638
      treaty_rate: 0.15
      qi_status: NQI
      documentation_status: COMPLETE
  reclaim_preferences:
    - market: XETR
      reclaim_method: QUICK_REFUND
      auto_claim: true
  documentation:
    - document_type: TAX_RESIDENCY_CERT
      market: XETR
      effective_date: "2024-01-01"
      expiry_date: "2024-12-31"

isda_agreements:
  - counterparty:
      type: LEI
      value: "W22LROWP2IHZNBB6K528"
    agreement_date: "2020-03-15"
    governing_law: ENGLISH
    coverage:
      - instrument_class: IRS
      - instrument_class: FX_FORWARD
    csa:
      csa_type: VM
      threshold_amount: 0
      threshold_currency: EUR
      collateral_ssi_ref: EUR_COLLATERAL_SSI

subcustodians:
  - market: XETR
    currency: EUR
    subcustodian_bic: "CBLLDEFF"
    pset_bic: "CBLLDEFF"
    is_primary: true

gaps:
  critical: []
  warnings:
    - section: tax
      issue: "Tax documentation expiring in 30 days"
      affected: [XETR]
    - section: corporate_actions
      issue: "No CA preferences for MERGER events"

coverage_summary:
  universe_entries: 15
  instruction_coverage: 100%
  gateway_coverage: 100%
  ssi_coverage: 100%
  booking_rule_coverage: 100%
  ca_coverage: 85%
  pricing_coverage: 100%
  tax_coverage: 90%
```

---

## Testing Strategy

### Unit Tests

```rust
// rust/src/dsl_v2/custom_ops/tests/instruction_profile_tests.rs
#[test]
fn test_find_instruction_template_priority_ordering() { ... }

#[test]
fn test_find_instruction_template_with_field_overrides() { ... }

#[test]
fn test_validate_instruction_profile_gaps() { ... }

// rust/src/dsl_v2/custom_ops/tests/gateway_routing_tests.rs
#[test]
fn test_find_gateway_with_fallback_chain() { ... }

#[test]
fn test_gateway_routing_null_wildcards() { ... }
```

### Integration Tests

```rust
// rust/tests/trading_matrix_integration.rs
#[tokio::test]
async fn test_full_matrix_export_cycle() {
    // 1. Create CBU
    // 2. Add universe entries
    // 3. Configure instruction profile
    // 4. Configure gateway routing
    // 5. Configure SSIs and booking rules
    // 6. Configure CA preferences
    // 7. Configure pricing
    // 8. Configure tax
    // 9. Export full matrix
    // 10. Validate completeness
}
```

### DSL Scenario Tests

**File**: `rust/tests/scenarios/trading_matrix_setup.dsl`

```clojure
;; Full trading matrix setup scenario
(cbu.ensure :name "Test Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)

;; Universe
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XETR" :currencies ["EUR"])

;; Instruction profile
(instruction-profile.assign-template :cbu-id @fund :template-id @mt540 
  :lifecycle-event "SETTLEMENT_INSTRUCTION" :instrument-class "EQUITY" :market "XETR" :priority 10)

;; Gateway routing
(trade-gateway.enable-gateway :cbu-id @fund :gateway-id @swift :status "ACTIVE")
(trade-gateway.add-routing-rule :cbu-id @fund :gateway-id @swift 
  :lifecycle-event "SETTLEMENT_INSTRUCTION" :priority 10)

;; SSI
(cbu-custody.create-ssi :cbu-id @fund :name "DE_EQUITY" :type "SECURITIES" 
  :safekeeping-account "DE-001" :safekeeping-bic "DEUTDEFF" :effective-date "2024-01-01" :as @ssi)

;; Booking rule
(cbu-custody.add-booking-rule :cbu-id @fund :ssi-id @ssi :name "German Equities" 
  :priority 10 :instrument-class "EQUITY" :market "XETR")

;; Validate
(trading-profile.validate-matrix-completeness :cbu-id @fund :validation-level "STRICT")

;; Export
(trading-profile.export-full-matrix :cbu-id @fund :format "YAML" :include-gaps true)
```

---

## Notes

1. **Priority-based Matching**: All lookup operations (templates, gateways, SSIs) use priority + specificity scoring. Lower priority number = higher preference. NULL values = wildcard match.

2. **Idempotency**: All upsert operations use conflict keys. Re-running the same DSL should be safe.

3. **Effective Dating**: Most configuration supports effective dates for audit trail and point-in-time reporting.

4. **Gap Analysis**: Each domain has `validate-*` and `derive-required-*` verbs to identify missing configuration.

5. **Export Format**: YAML is the default format, matching the trading profile import format for round-trip capability.

---

## Related Files

- `rust/config/verbs/trading-profile.yaml` - Existing trading profile verbs
- `rust/config/verbs/custody/cbu-custody.yaml` - Existing custody verbs
- `rust/config/verbs/custody/isda.yaml` - Existing ISDA verbs
- `rust/config/verbs/pricing-config.yaml` - Existing pricing verbs
- `rust/config/verbs/lifecycle.yaml` - Existing lifecycle verbs
- `rust/src/trading_profile/` - Existing trading profile implementation
- `CLAUDE.md` - Main project documentation (update after implementation)
