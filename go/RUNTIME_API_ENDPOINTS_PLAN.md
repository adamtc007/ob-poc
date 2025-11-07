# Runtime API Endpoints & Resource Creation Plan

## Overview

This plan addresses the need to connect DSL verbs and attributes to runtime API endpoints that can execute actual resource creation workflows. The goal is to enable the DSL to trigger real-world actions like creating accounts, applications, or other business resources through external APIs and BPMN workflows.

## Current State Analysis

### What We Have
- **DSL-as-State**: Complete onboarding state represented as accumulated DSL documents
- **AttributeID-as-Type**: Semantic typing system using UUIDs referencing dictionary metadata
- **Verb Validation**: 70+ approved DSL verbs with validation system
- **Dictionary System**: Universal schema with source/sink metadata for attributes
- **State Machine**: Immutable, versioned progression through onboarding stages

### The Gap
Currently, the DSL is **declarative** but not **executable**. We can express "what should happen" but cannot "make it happen". We need a **Runtime Execution Engine** that bridges DSL declarations to actual API calls.

## Proposed Architecture: **DSL-to-API Runtime Execution**

### Core Concept: **Action Binding**

Each DSL verb can be bound to one or more **Action Definitions** that specify:
1. **When** to execute (trigger conditions)
2. **What** to execute (API endpoint, workflow, etc.)
3. **How** to execute (attribute mapping, authentication, etc.)
4. **Where** to execute (target system, environment)

```
DSL Verb → Action Definition → Runtime Execution → Resource Creation
```

## Detailed Design

### 0. Resource Data Model & Lifecycle Endpoints

#### Resource Types and Resource Dictionary
- A **Resource Type** defines a concrete runtime resource to be created (e.g., CustodyAccount, FundAccountingSetup).
- Each Resource Type owns a **Resource Dictionary** that is a curated subset of the main dictionary (by Attribute IDs). This subset declares:
  - Required attributes (must be populated prior to creation)
  - Optional attributes
  - Attribute-level constraints and transformations (referencing the global transformation registry)
- The Resource Dictionary does not duplicate attribute definitions; it references the main dictionary via Attribute IDs and adds Resource‑Type‑specific requirements.

#### Resource Lifecycle and Endpoints
- Every Resource Type declares lifecycle endpoints, at minimum a **Create** endpoint used to realize the resource:
  - `create` → HTTP URL (or workflow trigger) to create a new instance
  - Optionally: `activate`, `suspend`, `update`, `close` for future lifecycle needs
- Endpoints are environment‑scoped (dev/staging/prod) and may point to:
  - Direct HTTP API (REST/GraphQL)
  - Workflow start endpoint (BPMN/Zeebe/Camunda)
  - Service facade that internally runs BPM or other systems

#### Post‑Discovery Runtime Flow (Products → Services → Resources)
1. Products discovered → Services discovered → Resources discovered in the DSL onboarding lifecycle.
2. For each discovered Resource Type, the engine resolves and populates required attribute values from the DSL state ("attribute value population").
3. The DSL invokes `resources.create` for each resource instance; the runtime:
   - Looks up the Resource Type’s `create` endpoint URL (by environment)
   - Builds the request payload from the Resource Dictionary (required + present optional attributes)
   - Executes the endpoint call (may trigger BPM or direct service)
   - Processes the response and binds returned identifiers/URLs back into DSL state

This codifies the DSL creation, validation, combine, link, and run sequence, ending with final endpoint calls per Resource Type.

### 1. Action Definition Schema

#### Core Action Types
1. **HTTP API Call** - Direct REST/GraphQL API invocation
2. **BPMN Workflow** - Trigger external workflow engine (Camunda, Zeebe, etc.)
3. **Message Queue** - Publish to event bus (Kafka, RabbitMQ, etc.)
4. **Database Operation** - Direct database manipulation
5. **External Service** - Call to external system (CRM, Core Banking, etc.)

#### Action Definition Structure
```json
{
  "action_id": "uuid",
  "verb_pattern": "resources.create",
  "action_type": "BPMN_WORKFLOW",
  "resource_type": "CustodyAccount",
  "trigger_conditions": {
    "domain": "onboarding",
    "state": "DISCOVER_RESOURCES",
    "attribute_requirements": ["custody.account_type", "settlement.currency"]
  },
  "execution_config": {
    "endpoint_url": "LOOKUP:resource_type.create", 
    "endpoint_lookup_fallback": "https://workflow-engine.bank.com/api/v1/process-definitions/custody-account-creation/start",
    "authentication": {
      "type": "oauth2",
      "credentials_ref": "workflow_engine_oauth"
    },
    "method": "POST",
    "timeout_seconds": 300,
    "retry_config": {
      "max_retries": 3,
      "backoff_strategy": "exponential"
    },
    "idempotency": {
      "header": "Idempotency-Key",
      "key_template": "{{resource_type}}:{{environment}}:{{cbu_id}}:{{action_id}}:{{dsl_version_id}}",
      "dedupe_ttl_seconds": 86400
    },
    "telemetry": {
      "correlation_id_template": "{{cbu_id}}:{{action_id}}:{{resource_type}}",
      "propagate_trace": true
    }
  },
  "attribute_mapping": {
    "input_mapping": [
      {
        "dsl_attribute_id": "8a5d1a77-...",
        "api_parameter": "accountType",
        "transformation": "uppercase"
      },
      {
        "dsl_attribute_id": "9b6e2c88-...",
        "api_parameter": "baseCurrency",
        "transformation": "iso_currency_code"
      }
    ],
    "output_mapping": [
      {
        "api_response_path": "$.account.id",
        "dsl_attribute_id": "7f4d9a55-...",
        "attribute_name": "custody.account_id"
      },
      {
        "api_response_path": "$.account.url",
        "dsl_attribute_id": "6e3c8b44-...",
        "attribute_name": "custody.account_url"
      }
    ]
  },
  "success_criteria": {
    "http_status_codes": [200, 201, 202],
    "response_validation": "$.status == 'CREATED'",
    "required_outputs": ["custody.account_id", "custody.account_url"]
  },
  "failure_handling": {
    "retry_on_codes": [500, 502, 503, 504],
    "fallback_action": "manual_intervention",
    "notification_channels": ["ops_team_slack", "onboarding_manager_email"]
  }
}
```

### 2. Runtime Execution Engine

#### Components

**a. Action Registry**
- Stores action definitions
- Supports versioning and environment-specific configurations
- Provides action discovery by verb pattern matching

**b. Execution Orchestrator**
- Monitors DSL state changes
- Identifies triggered actions based on conditions
- Coordinates multi-step workflows
- Manages execution state and history
- Generates idempotency keys and enforces deduplication (exactly-once effect semantics per resource instance)
- Propagates correlation IDs and distributed tracing metadata

**c. API Client Factory**
- Creates appropriate HTTP clients with authentication
- Handles different protocols (REST, GraphQL, gRPC)
- Manages connection pooling and timeouts

**d. Attribute Resolver**
- Resolves attribute values from current DSL state
- Applies transformations (e.g., format conversions)
- Validates required attributes are available
- Supports Resource Type lookups to ensure required attributes for that Resource’s dictionary are populated before execution

**e. Response Processor**
- Parses API responses
- Maps response data to new attribute values
- Updates DSL state with execution results

#### Execution Flow
```
1. DSL State Change Detected
   ↓
2. Action Registry Lookup (match verb patterns)
   ↓
3. Trigger Condition Evaluation
   ↓
4. Attribute Resolution & Validation (Resource Dictionary requirements)
   ↓
5. Generate Idempotency Key & Correlation IDs
   ↓
6. API Request Construction (Resource Type → create endpoint URL lookup)
   ↓
7. Authentication & Request Execution (with Idempotency-Key and correlation propagation)
   ↓
8. Response Processing & Validation
   ↓
9. DSL State Update with Results
   ↓
10. Success/Failure Handling (retry with dedupe; record attempt)
```

### 3. Database Schema Extensions

#### New Tables

**actions_registry**
```sql
CREATE TABLE actions_registry (
    action_id UUID PRIMARY KEY,
    action_name VARCHAR(255) NOT NULL,
    verb_pattern VARCHAR(100) NOT NULL, -- e.g., "resources.create"
    action_type action_type_enum NOT NULL,
    domain VARCHAR(100), -- optional domain filter
    trigger_conditions JSONB,
    execution_config JSONB NOT NULL,
    attribute_mapping JSONB NOT NULL,
    success_criteria JSONB,
    failure_handling JSONB,
    active BOOLEAN DEFAULT true,
    version INTEGER DEFAULT 1,
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_actions_verb_pattern ON actions_registry(verb_pattern);
CREATE INDEX idx_actions_domain ON actions_registry(domain);
CREATE INDEX idx_actions_active ON actions_registry(active);
```

**resource_types**
```sql
CREATE TABLE resource_types (
    resource_type_id UUID PRIMARY KEY,
    resource_type_name VARCHAR(200) UNIQUE NOT NULL,
    description TEXT,
    active BOOLEAN DEFAULT true,
    version INTEGER DEFAULT 1,
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_resource_types_name_env ON resource_types(resource_type_name, environment, version);
```

**resource_type_attributes**
```sql
CREATE TABLE resource_type_attributes (
    resource_type_id UUID REFERENCES resource_types(resource_type_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL, -- references main dictionary attribute
    required BOOLEAN DEFAULT false,
    constraints JSONB, -- optional resource-specific constraints
    transformation VARCHAR(100), -- optional default transform key
    PRIMARY KEY (resource_type_id, attribute_id)
);
```

**resource_type_endpoints**
```sql
CREATE TABLE resource_type_endpoints (
    endpoint_id UUID PRIMARY KEY,
    resource_type_id UUID REFERENCES resource_types(resource_type_id) ON DELETE CASCADE,
    lifecycle_action VARCHAR(50) NOT NULL, -- e.g., 'create'
    endpoint_url TEXT NOT NULL,
    method VARCHAR(10) DEFAULT 'POST',
    authentication JSONB, -- credentials_ref, type, etc.
    timeout_seconds INTEGER DEFAULT 300,
    retry_config JSONB,
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(resource_type_id, lifecycle_action, environment)
);
```

**action_executions**
```sql
CREATE TABLE action_executions (
    execution_id UUID PRIMARY KEY,
    action_id UUID REFERENCES actions_registry(action_id),
    cbu_id UUID REFERENCES cbus(cbu_id),
    dsl_version_id UUID REFERENCES dsl_ob(version_id),
    execution_status execution_status_enum NOT NULL, -- PENDING, RUNNING, COMPLETED, FAILED
    trigger_context JSONB, -- DSL state snapshot that triggered the action
    request_payload JSONB, -- API request that was sent
    response_payload JSONB, -- API response received
    result_attributes JSONB, -- Mapped attributes from response
    error_details JSONB, -- Error information if failed
    execution_duration_ms INTEGER,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    retry_count INTEGER DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    -- Idempotency and observability
    idempotency_key TEXT,
    correlation_id TEXT,
    trace_id TEXT,
    span_id TEXT,
    http_status INTEGER,
    endpoint TEXT,
    headers JSONB
);

CREATE INDEX idx_executions_status ON action_executions(execution_status);
CREATE INDEX idx_executions_cbu ON action_executions(cbu_id);
CREATE INDEX idx_executions_idempotency ON action_executions(idempotency_key);
CREATE UNIQUE INDEX uq_action_dedupe ON action_executions(action_id, cbu_id, idempotency_key);
```

**action_execution_attempts**
```sql
CREATE TABLE action_execution_attempts (
    attempt_id UUID PRIMARY KEY,
    execution_id UUID REFERENCES action_executions(execution_id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    status execution_status_enum NOT NULL, -- RUNNING, COMPLETED, FAILED
    request_payload JSONB,
    response_payload JSONB,
    error_details JSONB,
    http_status INTEGER,
    duration_ms INTEGER
);

CREATE UNIQUE INDEX uq_attempt_seq ON action_execution_attempts(execution_id, attempt_no);
CREATE INDEX idx_attempts_execution ON action_execution_attempts(execution_id);
```

**credentials_vault**
```sql
CREATE TABLE credentials_vault (
    credential_id UUID PRIMARY KEY,
    credential_name VARCHAR(255) UNIQUE NOT NULL,
    credential_type VARCHAR(50) NOT NULL, -- oauth2, api_key, basic_auth, etc.
    encrypted_data BYTEA NOT NULL, -- Encrypted credential data
    environment VARCHAR(50) DEFAULT 'production',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    active BOOLEAN DEFAULT true
);
```

### 4. Implementation Phases

#### Phase 1: Foundation (Weeks 1-2)
- [ ] Design and implement Action Registry database schema
- [ ] Model Resource Types, Resource Dictionary subset, and lifecycle endpoints
- [ ] Create basic Action Definition CRUD operations
- [ ] Implement simple HTTP API client with authentication
- [ ] Build attribute resolution engine
- [ ] Create execution tracking and logging
- [ ] Define idempotency key templates and correlation propagation strategy

#### Phase 2: Core Execution Engine (Weeks 3-4)
- [ ] Implement DSL state change detection
- [ ] Build trigger condition evaluation engine
- [ ] Create execution orchestrator
- [ ] Implement Resource endpoint lookup + request construction
- [ ] Implement response processing and attribute mapping
- [ ] Add basic error handling and retries
- [ ] Enforce idempotency with dedupe index and execution attempts table

#### Phase 3: Advanced Features (Weeks 5-6)
- [ ] Add BPMN workflow integration
- [ ] Implement message queue support
- [ ] Create credential management system
- [ ] Add execution monitoring and metrics
- [ ] Build admin UI for action management

#### Phase 4: Production Readiness (Weeks 7-8)
- [ ] Comprehensive testing and validation
- [ ] Security audit and penetration testing
- [ ] Performance optimization and load testing
- [ ] Documentation and training materials
- [ ] Deployment automation and monitoring

### 5. Example Use Cases

#### Use Case 1: Custody Account Creation
```
DSL State: (resources.plan (resource.create "CustodyAccount" ...))
↓
Trigger: Action "create_custody_account" matches "resources.create"
↓
Lookup: Resource Type "CustodyAccount" → lifecycle action 'create' URL
↓
API Call: POST /custody/accounts with Resource Dictionary attributes (account type, currency)
↓
Response: {"account_id": "CUST-123", "account_url": "https://portal.bank.com/accounts/CUST-123"}
↓
DSL Update: (values.bind (attr-id "custody.account_id") (value "CUST-123"))
```

#### Use Case 2: BPMN Workflow Trigger
```
DSL State: (kyc.start (documents (document "CertificateOfIncorporation")) ...)
↓
Trigger: Action "start_kyc_workflow" matches "kyc.start"
↓
BPMN Call: Start process "corporate-kyc-verification" with document metadata
↓
Response: {"process_instance_id": "proc-456", "task_url": "https://workflow.bank.com/tasks/789"}
↓
DSL Update: (workflow.track (process-id "proc-456") (status "RUNNING"))
```

#### Use Case 3: Multi-Step Resource Creation
```
DSL State: (products.add "CUSTODY" "FUND_ACCOUNTING")
↓
Triggers Multiple Actions:
1. Create custody account → Account ID returned
2. Create fund accounting setup → Setup ID returned
3. Link accounts together → Integration confirmed
↓
Final DSL State: Complete resource plan with all IDs and URLs
```

### 6. Security Considerations

#### Authentication & Authorization
- **Service-to-Service Authentication**: OAuth2, API keys, mTLS
- **Credential Management**: Encrypted storage with rotation policies
- **Environment Isolation**: Separate credentials for dev/staging/prod
- **Access Control**: Role-based permissions for action definitions

#### Data Protection
- **Encryption at Rest**: Sensitive data in credentials vault
- **Encryption in Transit**: HTTPS/TLS for all API calls
- **Data Masking**: PII handling in logs and audit trails
- **Compliance**: SOX, PCI DSS, GDPR compliance for financial data

#### Audit & Compliance
- **Complete Execution Logs**: Every API call tracked and logged
- **Immutable Audit Trail**: Execution history cannot be modified
- **Regulatory Reporting**: Compliance reports from execution data
- **Change Management**: Versioned action definitions with approval process

### 7. Monitoring & Observability

#### Metrics
- **Execution Success Rate**: % of successful action executions
- **Response Time Distribution**: P50, P95, P99 latencies for API calls
- **Error Rate by Action Type**: Track failure patterns
- **Retry Statistics**: Understanding of system reliability

#### Alerting
- **Failed Executions**: Immediate alerts for critical business processes
- **High Error Rates**: Proactive monitoring of degraded services
- **Timeout Warnings**: Early warning for performance issues
- **Credential Expiry**: Prevent authentication failures

#### Dashboards
- **Real-time Execution Status**: Live view of running actions
- **Historical Trends**: Execution patterns over time
- **Error Analysis**: Breakdown of failure causes
- **Performance Metrics**: System health and optimization opportunities

## Benefits of This Approach

### 1. **Seamless DSL-to-Action Bridge**
- Declarative DSL remains clean and business-focused
- Runtime execution happens transparently
- No changes needed to existing DSL verb validation

### 2. **Flexible Action Definitions**
- Support for multiple integration patterns (APIs, workflows, queues)
- Environment-specific configurations
- Versioning and rollback capabilities

### 3. **Complete Auditability**
- Every action execution tracked
- Full request/response logging
- Integration with existing DSL audit trail

### 4. **Business Process Alignment**
- Direct integration with BPMN workflows
- Support for complex multi-step processes
- Real-world resource creation (accounts, applications, etc.)

### 5. **Enterprise-Ready**
- Secure credential management
- Comprehensive monitoring and alerting
- Compliance with financial services regulations

## Next Steps

1. **Review and Feedback**: Get stakeholder input on architectural approach
2. **Proof of Concept**: Build minimal MVP with one action type (HTTP API)
3. **Integration Planning**: Identify first use cases and external systems
4. **Security Review**: Validate security approach with InfoSec team
5. **Implementation Planning**: Detailed sprint planning for Phase 1

## Questions for Review

1. **Scope**: Are there specific external systems or workflows to prioritize?
2. **Authentication**: What authentication mechanisms do target systems use?
3. **Environment Strategy**: How should dev/staging/prod environments be handled?
4. **Error Handling**: What should happen when external API calls fail?
5. **Performance Requirements**: Expected volume and response time SLAs?
6. **Compliance**: Any specific regulatory requirements for API integrations?

---

This plan transforms the DSL from a **state representation language** into a **executable workflow engine** while maintaining all existing benefits of auditability, immutability, and semantic typing.
