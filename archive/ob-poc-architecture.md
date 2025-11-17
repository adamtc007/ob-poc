# OB-POC Database Architecture

## Overview

The **ob-poc** system implements a production-ready Ultimate Beneficial Ownership (UBO) and comprehensive onboarding system using a **DSL-as-State** architecture with modern AI integration. The database schema supports multi-domain financial onboarding workflows with complete audit trails and regulatory compliance.

## Core Architecture Principles

### 1. DSL-as-State Pattern
- **State = Accumulated DSL Document**: Each onboarding case's current state is represented by its complete, accumulated DSL document
- **Immutable Event Sourcing**: Each operation appends to the DSL, creating new immutable versions
- **Executable Documentation**: DSL serves as human-readable documentation, machine-parseable data, audit trail, and workflow definition

### 2. AttributeID-as-Type Pattern
Variables in DSL are typed by AttributeID (UUID) referencing a universal dictionary, not primitive types:
```lisp
(verb @attr{uuid-001} @attr{uuid-002} ...)
```

### 3. AI-Enhanced Operations
- Multi-provider AI support (OpenAI, Gemini)
- Natural language to DSL conversion
- CBU generation and validation
- Context-aware business logic

## Database Schema Structure

### Core Tables (55+ total)

#### Primary Data Tables
- **`cbus`** - Client Business Unit definitions (primary entity containers)
- **`dictionary`** - Universal attribute dictionary (central pillar, AttributeID-as-Type)
- **`attribute_values`** - Runtime attribute values with JSONB metadata
- **`entities`** - Central entity registry with type-specific extensions

#### DSL Management
- **`dsl_ob`** - Immutable versioned DSL storage
- **`dsl_versions`** - DSL version management with compilation status
- **`dsl_domains`** - Domain organization (7 operational domains)
- **`domain_vocabularies`** - Verb registry (70+ approved verbs)
- **`parsed_asts`** - Compiled AST storage for performance

#### Entity Management System
- **`entity_types`** - Entity type definitions
- **`entity_limited_companies`** - Corporate entities
- **`entity_partnerships`** - Partnership structures
- **`entity_proper_persons`** - Natural persons
- **`entity_trusts`** - Trust structures
- **`cbu_entity_roles`** - Entity role assignments

#### Document Library (V3.1)
- **`document_catalog`** - Document storage and metadata
- **`document_types`** - Document classifications (24 types)
- **`document_metadata`** - EAV model for document attributes
- **`document_relationships`** - Document linking system

#### UBO & Ownership
- **`ubo_registry`** - Ultimate beneficial ownership tracking
- **`partnership_interests`** - Ownership percentages
- **`trust_beneficiary_classes`** - Trust beneficiary structures
- **`partnership_control_mechanisms`** - Control relationship modeling

#### AI & Operations
- **`crud_operations`** - AI-generated operation tracking
- **`dsl_examples`** - Training examples for AI systems
- **`dsl_execution_log`** - Operation execution history

#### Product & Services
- **`products`** - Product definitions
- **`services`** - Service offerings
- **`product_requirements`** - Product-specific requirements
- **`entity_product_mappings`** - Compatibility matrix

#### Regulatory & Compliance
- **`master_jurisdictions`** - Jurisdiction information
- **`entity_lifecycle_status`** - Entity status tracking
- **`vocabulary_audit`** - Change tracking for compliance

## Domain Architecture

### 7 Operational Domains

1. **Core Operations**
   - Verbs: `case.create`, `case.update`, `case.validate`, `case.approve`, `case.close`
   - Tables: `dsl_ob`, `cbus`, `attribute_values`

2. **Entity Management** 
   - Verbs: `entity.register`, `entity.classify`, `entity.link`, `identity.verify`, `identity.attest`
   - Tables: `entities`, `entity_*` family, `cbu_entity_roles`

3. **Product Operations**
   - Verbs: `products.add`, `products.configure`, `services.discover`, `services.provision`
   - Tables: `products`, `services`, `product_requirements`

4. **KYC Operations**
   - Verbs: `kyc.start`, `kyc.collect`, `kyc.verify`, `kyc.assess`, `compliance.screen`
   - Tables: `attribute_values` (with KYC group_id), `entities`

5. **UBO Operations**
   - Verbs: `ubo.collect-entity-data`, `ubo.resolve-ubos`, `ubo.calculate-indirect-ownership`
   - Tables: `ubo_registry`, `partnership_interests`, `trust_beneficiary_classes`

6. **Document Library (V3.1)**
   - Verbs: `document.catalog`, `document.verify`, `document.extract`, `document.link`
   - Tables: `document_catalog`, `document_types`, `document_metadata`

7. **ISDA Derivatives (V3.1)**
   - Verbs: `isda.establish_master`, `isda.execute_trade`, `isda.margin_call`
   - Tables: Uses core tables with ISDA-specific attributes

## Key Data Patterns

### Universal Dictionary Pattern
```sql
-- AttributeID references provide stable, typed contracts
INSERT INTO "ob-poc".dictionary (
    attribute_id,           -- UUID - the "type" of the attribute
    name,                  -- Human readable name
    group_id,              -- Domain grouping
    mask,                  -- Data type (string, decimal, enum, etc.)
    domain,                -- Business domain
    source,                -- JSONB metadata for data sources
    sink                   -- JSONB metadata for data destinations
);
```

### DSL State Accumulation
```sql
-- Each DSL version represents complete system state
CREATE TABLE "ob-poc".dsl_ob (
    version_id UUID PRIMARY KEY,
    cbu_id UUID NOT NULL,           -- Links to business unit
    dsl_text TEXT NOT NULL,         -- Complete DSL document
    created_at TIMESTAMPTZ          -- Immutable timestamp
);
```

### Entity-Attribute-Value Pattern
```sql
-- Runtime values linked to universal dictionary
CREATE TABLE "ob-poc".attribute_values (
    av_id UUID PRIMARY KEY,
    cbu_id UUID NOT NULL,           -- Business unit context
    attribute_id UUID NOT NULL,     -- References dictionary
    value JSONB NOT NULL,           -- Flexible value storage
    source JSONB,                   -- Provenance metadata
    state TEXT DEFAULT 'resolved'   -- Value state tracking
);
```

## Business Use Cases

### Hedge Fund Onboarding
- **Entities**: General Partner (LP), Management Company (Corp), Fund (LP)
- **Relationships**: GP controls Fund, ManCo advises Fund
- **Requirements**: Series D eligibility, 1940 Act compliance
- **Documents**: LPA, Management Agreement, Offering Memorandum

### UCITS Fund Setup
- **Entities**: Fund (SICAV/FCP), Management Company, Depositary
- **Jurisdictions**: Luxembourg, Ireland, France domiciles
- **Requirements**: UCITS compliance, MiFID II requirements
- **Documents**: Constitutional documents, KIID, Prospectus

### Corporate Banking
- **Entities**: Corporate client with UBO chain
- **Services**: Cash management, trade finance, custody
- **Compliance**: Enhanced KYC, sanctions screening
- **Documents**: Articles of incorporation, board resolutions

### Ultimate Beneficial Ownership
- **Ownership Chains**: Multi-level corporate structures
- **Calculations**: Direct and indirect ownership percentages
- **Compliance**: 25% UBO thresholds, regulatory reporting
- **Documentation**: Ownership charts, control attestations

## AI Integration Architecture

### Natural Language Processing
```
Business Requirement → AI Service → DSL Generation → Database Operations
                         ↓              ↓               ↓
                   [OpenAI/Gemini] → [Validation] → [PostgreSQL]
```

### AI-Enhanced Tables
- **`crud_operations`** - Tracks AI-generated database operations
- **`dsl_examples`** - Training examples for AI prompt engineering
- **`dsl_execution_log`** - AI operation performance metrics

### Multi-Provider Support
- **OpenAI**: GPT-3.5-turbo, GPT-4 for complex reasoning
- **Google Gemini**: Alternative provider for redundancy
- **Unified Interface**: AiService trait abstracts provider differences

## Performance Considerations

### Indexing Strategy
```sql
-- CBU-centric queries
CREATE INDEX idx_dsl_ob_cbu_id_created_at ON dsl_ob (cbu_id, created_at DESC);
CREATE INDEX idx_attribute_values_cbu_attribute ON attribute_values (cbu_id, attribute_id);

-- Dictionary lookups
CREATE INDEX idx_dictionary_name ON dictionary (name);
CREATE INDEX idx_dictionary_group_id ON dictionary (group_id);

-- Entity relationships
CREATE INDEX idx_entities_type ON entities (entity_type_id);
CREATE INDEX idx_cbu_entity_roles_cbu ON cbu_entity_roles (cbu_id);
```

### JSONB Usage
- **Flexible Metadata**: Source/sink information in dictionary
- **Performance**: GIN indexes on JSONB columns for fast queries
- **Schema Evolution**: No migrations required for attribute changes

### Query Patterns
- **State Reconstruction**: Latest DSL version per CBU
- **Attribute Lookups**: Join dictionary for type information
- **Relationship Traversal**: Entity graphs via junction tables

## Security & Compliance

### Data Classification
- **PII Handling**: Proper person attributes marked in dictionary
- **Audit Trails**: Complete operation history in execution logs
- **Access Control**: Role-based permissions via entity roles

### Regulatory Compliance
- **Immutable Records**: DSL versions preserve complete history
- **Data Lineage**: Source/sink metadata tracks data provenance
- **Change Tracking**: Vocabulary audit for schema evolution

### Privacy Considerations
- **GDPR Compliance**: Right to erasure via soft deletion flags
- **Data Minimization**: Only required attributes per product type
- **Consent Management**: Tracked via attribute value metadata

## Integration Points

### External Systems
- **Document Storage**: S3-compatible storage for document catalog
- **Identity Providers**: OIDC integration for user authentication
- **Regulatory APIs**: KYC/AML provider integrations
- **Market Data**: Financial data feeds for validation

### API Patterns
- **REST Endpoints**: CRUD operations on entities and attributes
- **GraphQL**: Complex relationship queries and mutations
- **Event Streaming**: Real-time updates via message queues
- **Batch Processing**: Bulk data operations and migrations

## Development Guidelines

### Schema Evolution
1. **Dictionary First**: Add new attributes to dictionary table
2. **Vocabulary Updates**: Register new verbs in domain_vocabularies
3. **Backward Compatibility**: Maintain existing AttributeID contracts
4. **Migration Strategy**: Use soft rollouts with feature flags

### Data Integrity
1. **Foreign Keys**: Enforce referential integrity
2. **Check Constraints**: Validate enum values and ranges
3. **Triggers**: Automatic timestamp updates and audit logging
4. **Transactions**: ACID compliance for multi-table operations

### Testing Strategy
1. **Unit Tests**: Individual table operations and constraints
2. **Integration Tests**: Cross-table relationship validation
3. **Performance Tests**: Query optimization and indexing
4. **Compliance Tests**: Regulatory requirement validation

## Monitoring & Operations

### Key Metrics
- **DSL Compilation Success Rate**: Track parsing and execution
- **AI Operation Accuracy**: Monitor AI-generated DSL quality
- **Query Performance**: Index usage and slow query identification
- **Data Quality**: Attribute value validation and completeness

### Alerting
- **Failed Operations**: DSL compilation or execution failures
- **Data Anomalies**: Unexpected attribute value patterns
- **Performance Degradation**: Query timeout or resource exhaustion
- **Compliance Violations**: Missing required attributes or documents

## Future Enhancements

### Planned Features
- **Graph Database**: Neo4j integration for complex relationship queries
- **Time-Series Data**: Historical attribute value trends and analytics
- **ML Pipeline**: Automated pattern recognition and anomaly detection
- **API Gateway**: Centralized authentication and rate limiting

### Architecture Evolution
- **Microservices**: Domain-specific service decomposition
- **Event Sourcing**: Complete event stream with projections
- **CQRS**: Separate read and write models for performance
- **Multi-Tenancy**: Isolated schemas per client organization

---

**Architecture Status**: Production-ready with 55+ tables, 70+ approved verbs, and comprehensive AI integration.
**Last Updated**: 2025-01-14
**Schema Version**: V3.1 with Document Library and ISDA Derivatives support