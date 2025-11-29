# DSL Configuration-Driven Architecture

**Goal**: Replace hardcoded verb definitions and CSG rules with YAML configuration files. Adding new verbs/rules should require editing config, not Rust code.

**Prerequisites**:
- MCP server working ✅
- Database execution working ✅

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Configuration Files                         │
├─────────────────────────────────────────────────────────────────┤
│  verbs.yaml          │  csg_rules.yaml      │  (DB: entity_types,│
│  - Domain definitions│  - Validation rules  │   roles, doc_types)│
│  - Verb arguments    │  - Error messages    │                    │
│  - Table mappings    │  - Warnings          │                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Config Loader (Rust)                          │
│  - Parses YAML at startup                                        │
│  - Validates config schema                                       │
│  - Builds runtime registry                                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Runtime Registry                              │
│  - VerbRegistry (from YAML + DB entity_types)                   │
│  - CsgRuleSet (from YAML)                                       │
│  - GenericCrudExecutor (table mappings)                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Existing Pipeline (unchanged)                       │
│  Parser → Compiler → Executor → PostgreSQL                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## File Structure

```
rust/
├── config/
│   ├── verbs.yaml           # Verb definitions
│   └── csg_rules.yaml       # Validation rules
├── src/
│   └── dsl_v2/
│       ├── config/
│       │   ├── mod.rs
│       │   ├── loader.rs        # YAML loading
│       │   ├── verb_config.rs   # Verb config types
│       │   ├── csg_config.rs    # CSG rule config types
│       │   └── schema.rs        # Config validation
│       ├── verb_registry.rs     # UPDATE: Load from config
│       ├── csg_linter.rs        # UPDATE: Load rules from config
│       └── executor.rs          # UPDATE: Generic CRUD executor
```

---

## Part 1: Verb Configuration

### File: `rust/config/verbs.yaml`

```yaml
# DSL Verb Configuration
# This file defines all verbs available in the DSL.
# Adding a new verb here automatically makes it available - no code changes needed.

version: "1.0"

# =============================================================================
# DOMAIN: cbu (Client Business Unit)
# =============================================================================
cbu:
  description: "Client Business Unit operations"
  
  verbs:
    create:
      description: "Create a new Client Business Unit"
      behavior: crud
      crud:
        operation: insert
        table: cbus
        returning: cbu_id
        defaults:
          status: active
      args:
        - name: name
          type: string
          required: true
          maps_to: name
        - name: client-type
          type: string
          required: false
          maps_to: client_type
          valid_values: [individual, corporate, trust, fund]
        - name: jurisdiction
          type: string
          required: false
          maps_to: jurisdiction

    read:
      description: "Read a CBU by ID"
      behavior: crud
      crud:
        operation: select
        table: cbus
      args:
        - name: cbu-id
          type: reference
          required: true
          maps_to: cbu_id

    update:
      description: "Update a CBU"
      behavior: crud
      crud:
        operation: update
        table: cbus
        key: cbu_id
      args:
        - name: cbu-id
          type: reference
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

    delete:
      description: "Soft delete a CBU"
      behavior: crud
      crud:
        operation: soft_delete
        table: cbus
        key: cbu_id
        soft_delete_column: status
        soft_delete_value: deleted
      args:
        - name: cbu-id
          type: reference
          required: true
          maps_to: cbu_id

    assign-role:
      description: "Assign a role to an entity"
      behavior: crud
      crud:
        operation: insert
        table: entity_roles
        returning: entity_role_id
      args:
        - name: cbu-id
          type: reference
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: reference
          required: true
          maps_to: entity_id
        - name: target-entity-id
          type: reference
          required: true
          maps_to: target_entity_id
        - name: role
          type: lookup
          required: true
          lookup:
            table: roles
            code_column: role_code
            id_column: role_id
          maps_to: role_id
        - name: ownership-percentage
          type: decimal
          required: false
          maps_to: ownership_percentage

# =============================================================================
# DOMAIN: entity
# =============================================================================
entity:
  description: "Entity operations"
  
  # Dynamic verb generation from database
  # Queries entity_types table and creates verbs like:
  #   entity.create-limited-company
  #   entity.create-proper-person
  #   entity.create-trust
  dynamic_verbs:
    pattern: "create-{type_code}"  # type_code from entity_types table
    source:
      table: entity_types
      code_column: type_code
      transform: kebab-case  # LIMITED_COMPANY -> limited-company
    behavior: crud
    crud:
      operation: insert
      table: entities
      returning: entity_id
      defaults:
        status: active
      type_resolution:
        from_verb: true  # Extract type from verb name
        lookup_table: entity_types
        lookup_column: type_code
        maps_to: entity_type_id
    args:
      - name: cbu-id
        type: reference
        required: true
        maps_to: cbu_id
      - name: name
        type: string
        required: true
        maps_to: name
      # Additional args stored in JSONB attributes column
      extra_args_column: attributes

  verbs:
    read:
      description: "Read an entity by ID"
      behavior: crud
      crud:
        operation: select
        table: entities
        joins:
          - table: entity_types
            on: entity_type_id
            select: [type_code]
      args:
        - name: entity-id
          type: reference
          required: true
          maps_to: entity_id

    update:
      description: "Update an entity"
      behavior: crud
      crud:
        operation: update
        table: entities
        key: entity_id
      args:
        - name: entity-id
          type: reference
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

    delete:
      description: "Soft delete an entity"
      behavior: crud
      crud:
        operation: soft_delete
        table: entities
        key: entity_id
        soft_delete_column: status
        soft_delete_value: deleted
      args:
        - name: entity-id
          type: reference
          required: true
          maps_to: entity_id

# =============================================================================
# DOMAIN: document
# =============================================================================
document:
  description: "Document operations"
  
  verbs:
    catalog:
      description: "Catalog a document"
      behavior: crud
      crud:
        operation: insert
        table: documents
        returning: document_id
        defaults:
          status: pending
      args:
        - name: cbu-id
          type: reference
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: reference
          required: true
          maps_to: entity_id
        - name: document-type
          type: lookup
          required: true
          lookup:
            table: document_types
            code_column: type_code
            id_column: type_id
          maps_to: document_type_id

    extract:
      description: "Extract data from a document"
      behavior: crud
      crud:
        operation: update
        table: documents
        key: document_id
        set:
          status: extracting
      args:
        - name: document-id
          type: reference
          required: true
          maps_to: document_id

    request:
      description: "Request a document from client"
      behavior: crud
      crud:
        operation: insert
        table: documents
        returning: document_id
        defaults:
          status: requested
        metadata_column: metadata
      args:
        - name: cbu-id
          type: reference
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: reference
          required: true
          maps_to: entity_id
        - name: document-type
          type: lookup
          required: true
          lookup:
            table: document_types
            code_column: type_code
            id_column: type_id
          maps_to: document_type_id
        - name: due-date
          type: date
          required: false
          maps_to_metadata: due_date
        - name: priority
          type: string
          required: false
          maps_to_metadata: priority
          default: normal

# =============================================================================
# DOMAIN: screening
# =============================================================================
screening:
  description: "Screening operations"
  
  # Dynamic verbs for screening types
  dynamic_verbs:
    pattern: "{screening_type}"
    static_types:  # Not from DB, just a list
      - pep
      - sanctions
      - adverse-media
    behavior: crud
    crud:
      operation: insert
      table: screenings
      returning: screening_id
      defaults:
        status: pending
      type_resolution:
        from_verb: true
        transform: screaming-snake  # pep -> PEP, adverse-media -> ADVERSE_MEDIA
        maps_to: screening_type
      metadata_column: metadata
    args:
      - name: entity-id
        type: reference
        required: true
        maps_to: entity_id
      - name: lookback-months
        type: integer
        required: false
        maps_to_metadata: lookback_months

# =============================================================================
# DOMAIN: ubo
# =============================================================================
ubo:
  description: "Ultimate Beneficial Owner operations"
  
  verbs:
    calculate:
      description: "Calculate UBOs for an entity"
      behavior: plugin
      plugin: ubo_calculate  # Rust function name
      args:
        - name: cbu-id
          type: reference
          required: true
        - name: entity-id
          type: reference
          required: true
        - name: threshold
          type: decimal
          required: false
          default: 25.0

    validate:
      description: "Validate UBO documentation"
      behavior: plugin
      plugin: ubo_validate
      args:
        - name: cbu-id
          type: reference
          required: true

# =============================================================================
# DOMAIN: kyc
# =============================================================================
kyc:
  description: "KYC investigation operations"
  
  verbs:
    initiate:
      description: "Initiate a KYC investigation"
      behavior: crud
      crud:
        operation: insert
        table: investigations
        returning: investigation_id
        defaults:
          status: open
      args:
        - name: cbu-id
          type: reference
          required: true
          maps_to: cbu_id
        - name: investigation-type
          type: string
          required: true
          maps_to: investigation_type

    decide:
      description: "Record KYC decision"
      behavior: plugin
      plugin: kyc_decide  # Complex state machine logic
      args:
        - name: investigation-id
          type: reference
          required: true
        - name: decision
          type: string
          required: true
          valid_values: [approve, reject, escalate]
        - name: rationale
          type: string
          required: true
```

---

## Part 2: CSG Rules Configuration

### File: `rust/config/csg_rules.yaml`

```yaml
# CSG (Compliance and Security Gateway) Rules
# These rules validate DSL programs before execution.

version: "1.0"

# =============================================================================
# CONSTRAINT RULES (Errors - block execution)
# =============================================================================
constraints:

  # Document type / entity type compatibility
  - id: CSG-C001
    name: passport_requires_person
    description: "PASSPORT document can only be cataloged for natural persons"
    severity: error
    when:
      verb: document.catalog
      arg_equals:
        document-type: PASSPORT
    requires:
      arg: entity-id
      entity_type: PROPER_PERSON
    error: "Cannot catalog PASSPORT for {entity_type} - requires PROPER_PERSON"

  - id: CSG-C002
    name: certificate_requires_company
    description: "Certificate of Incorporation requires company entity"
    severity: error
    when:
      verb: document.catalog
      arg_equals:
        document-type: CERTIFICATE_OF_INCORPORATION
    requires:
      arg: entity-id
      entity_type_in: [LIMITED_COMPANY, PLC, LLP, LLC]
    error: "Cannot catalog CERTIFICATE_OF_INCORPORATION for {entity_type}"

  - id: CSG-C003
    name: trust_deed_requires_trust
    description: "Trust Deed requires trust entity"
    severity: error
    when:
      verb: document.catalog
      arg_equals:
        document-type: TRUST_DEED
    requires:
      arg: entity-id
      entity_type: TRUST
    error: "Cannot catalog TRUST_DEED for {entity_type} - requires TRUST"

  # Role assignment constraints
  - id: CSG-C010
    name: ubo_requires_person
    description: "Beneficial Owner role requires natural person"
    severity: error
    when:
      verb: cbu.assign-role
      arg_equals:
        role: BENEFICIAL_OWNER
    requires:
      arg: entity-id
      entity_type: PROPER_PERSON
    error: "BENEFICIAL_OWNER role requires PROPER_PERSON, got {entity_type}"

  - id: CSG-C011
    name: director_requires_person
    description: "Director role requires natural person"
    severity: error
    when:
      verb: cbu.assign-role
      arg_equals:
        role: DIRECTOR
    requires:
      arg: entity-id
      entity_type: PROPER_PERSON
    error: "DIRECTOR role requires PROPER_PERSON"

  - id: CSG-C012
    name: shareholder_target_must_be_company
    description: "Shareholder role target must be a company"
    severity: error
    when:
      verb: cbu.assign-role
      arg_equals:
        role: SHAREHOLDER
    requires:
      arg: target-entity-id
      entity_type_in: [LIMITED_COMPANY, PLC, LLP, LLC]
    error: "SHAREHOLDER target must be a company entity"

  # Symbol resolution
  - id: CSG-C020
    name: undefined_symbol
    description: "All referenced symbols must be defined"
    severity: error
    check: symbol_defined
    error: "Undefined symbol: @{symbol}"

  - id: CSG-C021
    name: symbol_type_mismatch
    description: "Symbol must reference correct entity type"
    severity: error
    check: symbol_type_compatible
    error: "Symbol @{symbol} is {actual_type}, expected {expected_type}"

# =============================================================================
# WARNING RULES (Warnings - allow execution but flag)
# =============================================================================
warnings:

  - id: CSG-W001
    name: high_ownership
    description: "Ownership percentage exceeds 100%"
    severity: warning
    when:
      verb: cbu.assign-role
      arg_greater_than:
        ownership-percentage: 100
    message: "Ownership percentage {value}% exceeds 100%"

  - id: CSG-W002
    name: low_ownership_ubo
    description: "UBO with very low ownership"
    severity: warning
    when:
      verb: cbu.assign-role
      arg_equals:
        role: BENEFICIAL_OWNER
      arg_less_than:
        ownership-percentage: 10
    message: "BENEFICIAL_OWNER with only {value}% ownership - verify this is intentional"

  - id: CSG-W003
    name: missing_screening
    description: "Person entity without screening"
    severity: warning
    check: person_has_screening
    message: "Person @{symbol} has no PEP/sanctions screening scheduled"

  - id: CSG-W004
    name: missing_identity_doc
    description: "Person without identity document"
    severity: warning
    check: person_has_identity_document
    message: "Person @{symbol} has no identity document cataloged"

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
    message: "US person may require W-9 for FATCA compliance"

  - id: CSG-J002
    name: uk_company_psc
    description: "UK companies require PSC register"
    severity: info
    when:
      entity_type_in: [LIMITED_COMPANY, PLC]
      jurisdiction: GB
    message: "UK company - consider requesting PSC register"

# =============================================================================
# COMPOSITE RULES (Complex multi-step validation)
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
```

---

## Part 3: Config Type Definitions

### File: `rust/src/dsl_v2/config/verb_config.rs`

```rust
//! Verb configuration types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct VerbsConfig {
    pub version: String,
    #[serde(flatten)]
    pub domains: HashMap<String, DomainConfig>,
}

#[derive(Debug, Deserialize)]
pub struct DomainConfig {
    pub description: String,
    #[serde(default)]
    pub verbs: HashMap<String, VerbConfig>,
    #[serde(default)]
    pub dynamic_verbs: Option<DynamicVerbConfig>,
}

#[derive(Debug, Deserialize)]
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub plugin: Option<String>,
    pub args: Vec<ArgConfig>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VerbBehavior {
    Crud,
    Plugin,
    Composite,
}

#[derive(Debug, Deserialize)]
pub struct CrudConfig {
    pub operation: CrudOperation,
    pub table: String,
    #[serde(default)]
    pub returning: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub defaults: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub set: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub soft_delete_column: Option<String>,
    #[serde(default)]
    pub soft_delete_value: Option<String>,
    #[serde(default)]
    pub type_resolution: Option<TypeResolutionConfig>,
    #[serde(default)]
    pub metadata_column: Option<String>,
    #[serde(default)]
    pub joins: Vec<JoinConfig>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CrudOperation {
    Insert,
    Select,
    Update,
    SoftDelete,
    Delete,
}

#[derive(Debug, Deserialize)]
pub struct TypeResolutionConfig {
    pub from_verb: bool,
    #[serde(default)]
    pub lookup_table: Option<String>,
    #[serde(default)]
    pub lookup_column: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,  // kebab-case, screaming-snake, etc.
    pub maps_to: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinConfig {
    pub table: String,
    pub on: String,
    pub select: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DynamicVerbConfig {
    pub pattern: String,
    #[serde(default)]
    pub source: Option<DynamicSourceConfig>,
    #[serde(default)]
    pub static_types: Option<Vec<String>>,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    pub args: Vec<ArgConfig>,
}

#[derive(Debug, Deserialize)]
pub struct DynamicSourceConfig {
    pub table: String,
    pub code_column: String,
    #[serde(default)]
    pub transform: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArgConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub maps_to: Option<String>,
    #[serde(default)]
    pub maps_to_metadata: Option<String>,
    #[serde(default)]
    pub lookup: Option<LookupConfig>,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArgType {
    String,
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Reference,
    Lookup,
    Uuid,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LookupConfig {
    pub table: String,
    pub code_column: String,
    pub id_column: String,
}
```

### File: `rust/src/dsl_v2/config/csg_config.rs`

```rust
//! CSG rule configuration types

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct ConstraintRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: Severity,
    #[serde(default)]
    pub when: Option<WhenCondition>,
    #[serde(default)]
    pub requires: Option<RequiresCondition>,
    #[serde(default)]
    pub check: Option<String>,  // Built-in check function name
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct WarningRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: Severity,
    #[serde(default)]
    pub when: Option<WhenCondition>,
    #[serde(default)]
    pub check: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct JurisdictionRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: Severity,
    #[serde(default)]
    pub when: Option<JurisdictionWhen>,
    #[serde(default)]
    pub requires_document: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CompositeRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: Severity,
    #[serde(default)]
    pub applies_to: Option<AppliesTo>,
    pub checks: Vec<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Deserialize)]
pub struct WhenCondition {
    #[serde(default)]
    pub verb: Option<String>,
    #[serde(default)]
    pub arg_equals: Option<HashMap<String, String>>,
    #[serde(default)]
    pub arg_greater_than: Option<HashMap<String, f64>>,
    #[serde(default)]
    pub arg_less_than: Option<HashMap<String, f64>>,
}

#[derive(Debug, Deserialize)]
pub struct RequiresCondition {
    pub arg: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct JurisdictionWhen {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AppliesTo {
    #[serde(default)]
    pub client_type: Option<String>,
}
```

---

## Part 4: Config Loader

### File: `rust/src/dsl_v2/config/loader.rs`

```rust
//! Configuration loader

use anyhow::{anyhow, Result};
use std::path::Path;

use super::verb_config::VerbsConfig;
use super::csg_config::CsgRulesConfig;

pub struct ConfigLoader {
    config_dir: String,
}

impl ConfigLoader {
    pub fn new(config_dir: impl Into<String>) -> Self {
        Self { config_dir: config_dir.into() }
    }

    pub fn from_env() -> Self {
        let dir = std::env::var("DSL_CONFIG_DIR")
            .unwrap_or_else(|_| "config".to_string());
        Self::new(dir)
    }

    pub fn load_verbs(&self) -> Result<VerbsConfig> {
        let path = Path::new(&self.config_dir).join("verbs.yaml");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read {}: {}", path.display(), e))?;
        
        let config: VerbsConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse {}: {}", path.display(), e))?;
        
        self.validate_verbs(&config)?;
        Ok(config)
    }

    pub fn load_csg_rules(&self) -> Result<CsgRulesConfig> {
        let path = Path::new(&self.config_dir).join("csg_rules.yaml");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read {}: {}", path.display(), e))?;
        
        let config: CsgRulesConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse {}: {}", path.display(), e))?;
        
        self.validate_csg_rules(&config)?;
        Ok(config)
    }

    fn validate_verbs(&self, config: &VerbsConfig) -> Result<()> {
        for (domain, domain_config) in &config.domains {
            for (verb, verb_config) in &domain_config.verbs {
                // Validate CRUD config
                if verb_config.behavior == super::verb_config::VerbBehavior::Crud {
                    if verb_config.crud.is_none() {
                        return Err(anyhow!("{}.{}: crud behavior requires crud config", domain, verb));
                    }
                }
                // Validate plugin config
                if verb_config.behavior == super::verb_config::VerbBehavior::Plugin {
                    if verb_config.plugin.is_none() {
                        return Err(anyhow!("{}.{}: plugin behavior requires plugin name", domain, verb));
                    }
                }
                // Validate lookup args have lookup config
                for arg in &verb_config.args {
                    if arg.arg_type == super::verb_config::ArgType::Lookup && arg.lookup.is_none() {
                        return Err(anyhow!("{}.{} arg '{}': lookup type requires lookup config", 
                            domain, verb, arg.name));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_csg_rules(&self, config: &CsgRulesConfig) -> Result<()> {
        // Check for duplicate rule IDs
        let mut ids = std::collections::HashSet::new();
        
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
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_verbs() {
        let loader = ConfigLoader::new("config");
        let result = loader.load_verbs();
        assert!(result.is_ok(), "Failed to load verbs: {:?}", result.err());
    }

    #[test]
    fn test_load_csg_rules() {
        let loader = ConfigLoader::new("config");
        let result = loader.load_csg_rules();
        assert!(result.is_ok(), "Failed to load CSG rules: {:?}", result.err());
    }
}
```

### File: `rust/src/dsl_v2/config/mod.rs`

```rust
//! Configuration module

pub mod verb_config;
pub mod csg_config;
pub mod loader;

pub use verb_config::*;
pub use csg_config::*;
pub use loader::ConfigLoader;
```

---

## Part 5: Config-Driven Verb Registry

### File: `rust/src/dsl_v2/verb_registry_config.rs`

```rust
//! Config-driven verb registry

use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

use super::config::{
    VerbsConfig, DomainConfig, VerbConfig, DynamicVerbConfig,
    ArgConfig, VerbBehavior, ArgType,
};

/// Runtime verb definition (built from config)
#[derive(Debug, Clone)]
pub struct RuntimeVerb {
    pub domain: String,
    pub verb: String,
    pub description: String,
    pub behavior: VerbBehavior,
    pub crud_config: Option<Arc<super::config::CrudConfig>>,
    pub plugin_name: Option<String>,
    pub args: Vec<RuntimeArg>,
}

impl RuntimeVerb {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.domain, self.verb)
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeArg {
    pub name: String,
    pub canonical_name: String,  // kebab-case normalized
    pub arg_type: ArgType,
    pub required: bool,
    pub maps_to: Option<String>,
    pub maps_to_metadata: Option<String>,
    pub lookup: Option<super::config::LookupConfig>,
    pub valid_values: Option<Vec<String>>,
    pub default: Option<serde_yaml::Value>,
}

/// Config-driven verb registry
pub struct ConfigVerbRegistry {
    verbs: HashMap<String, HashMap<String, RuntimeVerb>>,
    domains: Vec<String>,
}

impl ConfigVerbRegistry {
    pub async fn from_config(config: VerbsConfig, pool: Option<&PgPool>) -> Result<Self> {
        let mut registry = Self {
            verbs: HashMap::new(),
            domains: Vec::new(),
        };

        for (domain_name, domain_config) in config.domains {
            registry.domains.push(domain_name.clone());
            let domain_verbs = registry.verbs.entry(domain_name.clone()).or_default();

            // Load static verbs
            for (verb_name, verb_config) in &domain_config.verbs {
                let runtime_verb = Self::build_runtime_verb(&domain_name, verb_name, verb_config);
                domain_verbs.insert(verb_name.clone(), runtime_verb);
            }

            // Load dynamic verbs
            if let Some(dynamic) = &domain_config.dynamic_verbs {
                let dynamic_verbs = Self::expand_dynamic_verbs(&domain_name, dynamic, pool).await?;
                for verb in dynamic_verbs {
                    domain_verbs.insert(verb.verb.clone(), verb);
                }
            }
        }

        Ok(registry)
    }

    fn build_runtime_verb(domain: &str, verb: &str, config: &VerbConfig) -> RuntimeVerb {
        RuntimeVerb {
            domain: domain.to_string(),
            verb: verb.to_string(),
            description: config.description.clone(),
            behavior: config.behavior,
            crud_config: config.crud.as_ref().map(|c| Arc::new(c.clone())),
            plugin_name: config.plugin.clone(),
            args: config.args.iter().map(Self::build_runtime_arg).collect(),
        }
    }

    fn build_runtime_arg(config: &ArgConfig) -> RuntimeArg {
        RuntimeArg {
            name: config.name.clone(),
            canonical_name: config.name.replace('_', "-"),
            arg_type: config.arg_type,
            required: config.required,
            maps_to: config.maps_to.clone(),
            maps_to_metadata: config.maps_to_metadata.clone(),
            lookup: config.lookup.clone(),
            valid_values: config.valid_values.clone(),
            default: config.default.clone(),
        }
    }

    async fn expand_dynamic_verbs(
        domain: &str,
        config: &DynamicVerbConfig,
        pool: Option<&PgPool>,
    ) -> Result<Vec<RuntimeVerb>> {
        let mut verbs = Vec::new();

        // Get type codes from DB or static list
        let type_codes: Vec<String> = if let Some(source) = &config.source {
            if let Some(pool) = pool {
                Self::load_types_from_db(pool, &source.table, &source.code_column).await?
            } else {
                Vec::new()
            }
        } else if let Some(static_types) = &config.static_types {
            static_types.clone()
        } else {
            Vec::new()
        };

        for type_code in type_codes {
            // Transform type code to verb name
            let verb_name = Self::transform_to_verb_name(&type_code, config.source.as_ref()
                .and_then(|s| s.transform.as_deref()));

            // Replace {pattern} in verb pattern
            let final_verb = config.pattern
                .replace("{type_code}", &verb_name)
                .replace("{screening_type}", &verb_name);

            let runtime_verb = RuntimeVerb {
                domain: domain.to_string(),
                verb: final_verb,
                description: format!("Create {} entity", type_code),
                behavior: config.behavior,
                crud_config: config.crud.as_ref().map(|c| Arc::new(c.clone())),
                plugin_name: None,
                args: config.args.iter().map(Self::build_runtime_arg).collect(),
            };

            verbs.push(runtime_verb);
        }

        Ok(verbs)
    }

    async fn load_types_from_db(pool: &PgPool, table: &str, column: &str) -> Result<Vec<String>> {
        let query = format!(
            r#"SELECT {} FROM "ob-poc".{} ORDER BY {}"#,
            column, table, column
        );
        
        let rows: Vec<(String,)> = sqlx::query_as(&query)
            .fetch_all(pool)
            .await?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    fn transform_to_verb_name(code: &str, transform: Option<&str>) -> String {
        match transform {
            Some("kebab-case") => code.to_lowercase().replace('_', "-"),
            Some("screaming-snake") => code.to_uppercase().replace('-', "_"),
            _ => code.to_lowercase(),
        }
    }

    // Registry access methods
    pub fn get(&self, domain: &str, verb: &str) -> Option<&RuntimeVerb> {
        self.verbs.get(domain).and_then(|v| v.get(verb))
    }

    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    pub fn all_verbs(&self) -> impl Iterator<Item = &RuntimeVerb> {
        self.verbs.values().flat_map(|v| v.values())
    }

    pub fn verbs_for_domain(&self, domain: &str) -> impl Iterator<Item = &RuntimeVerb> {
        self.verbs.get(domain).into_iter().flat_map(|v| v.values())
    }
}
```

---

## Part 6: Generic CRUD Executor

### File: `rust/src/dsl_v2/generic_crud.rs`

```rust
//! Generic CRUD executor driven by configuration

use anyhow::{anyhow, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use super::config::{CrudConfig, CrudOperation, ArgType, LookupConfig};
use super::execution_context::{StepResult, ResolvedValue};
use super::verb_registry_config::RuntimeVerb;

pub struct GenericCrudExecutor {
    pool: PgPool,
}

impl GenericCrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn execute(
        &self,
        verb: &RuntimeVerb,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<StepResult> {
        let crud = verb.crud_config.as_ref()
            .ok_or_else(|| anyhow!("Verb {} has no CRUD config", verb.full_name()))?;

        match crud.operation {
            CrudOperation::Insert => self.execute_insert(verb, crud, args).await,
            CrudOperation::Select => self.execute_select(verb, crud, args).await,
            CrudOperation::Update => self.execute_update(verb, crud, args).await,
            CrudOperation::SoftDelete => self.execute_soft_delete(verb, crud, args).await,
            CrudOperation::Delete => self.execute_delete(verb, crud, args).await,
        }
    }

    async fn execute_insert(
        &self,
        verb: &RuntimeVerb,
        crud: &CrudConfig,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<StepResult> {
        let table = &crud.table;
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> = Vec::new();
        let mut param_idx = 1;

        // Add columns from args
        for arg in &verb.args {
            if let Some(col) = &arg.maps_to {
                if let Some(value) = args.get(&arg.canonical_name) {
                    // Handle lookup args
                    let resolved = if arg.arg_type == ArgType::Lookup {
                        if let Some(lookup) = &arg.lookup {
                            let lookup_value = self.resolve_lookup(lookup, value).await?;
                            ResolvedValue::Uuid(lookup_value)
                        } else {
                            value.clone()
                        }
                    } else {
                        value.clone()
                    };

                    columns.push(col.clone());
                    placeholders.push(format!("${}", param_idx));
                    values.push(self.to_sql_value(&resolved)?);
                    param_idx += 1;
                } else if arg.required {
                    return Err(anyhow!("{} requires :{}", verb.full_name(), arg.name));
                }
            }
        }

        // Handle type resolution from verb name
        if let Some(type_res) = &crud.type_resolution {
            if type_res.from_verb {
                let type_code = self.extract_type_from_verb(&verb.verb, type_res.transform.as_deref());
                
                if let (Some(table), Some(col)) = (&type_res.lookup_table, &type_res.lookup_column) {
                    let type_id = self.lookup_type_id(table, col, &type_code).await?;
                    columns.push(type_res.maps_to.clone());
                    placeholders.push(format!("${}", param_idx));
                    values.push(Box::new(type_id));
                    param_idx += 1;
                }
            }
        }

        // Add defaults
        for (col, val) in &crud.defaults {
            if !columns.contains(col) {
                columns.push(col.clone());
                placeholders.push(format!("${}", param_idx));
                values.push(self.yaml_to_sql_value(val)?);
                param_idx += 1;
            }
        }

        // Add timestamps
        columns.push("created_at".to_string());
        placeholders.push("NOW()".to_string());
        columns.push("updated_at".to_string());
        placeholders.push("NOW()".to_string());

        // Generate ID
        let id = Uuid::new_v4();
        if let Some(returning) = &crud.returning {
            columns.insert(0, returning.clone());
            placeholders.insert(0, format!("${}", param_idx));
            values.insert(0, Box::new(id));
        }

        let sql = format!(
            r#"INSERT INTO "ob-poc".{} ({}) VALUES ({})"#,
            table,
            columns.join(", "),
            placeholders.join(", ")
        );

        // Execute (simplified - real implementation needs proper binding)
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!("Insert failed: {}", e))?;

        Ok(StepResult::success(0, verb.full_name())
            .with_id(id)
            .with_rows(1))
    }

    async fn execute_select(
        &self,
        verb: &RuntimeVerb,
        crud: &CrudConfig,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<StepResult> {
        let table = &crud.table;
        
        // Build SELECT columns
        let mut select_cols = vec!["*".to_string()];
        
        // Add join columns
        for join in &crud.joins {
            for col in &join.select {
                select_cols.push(format!("{}.{}", join.table, col));
            }
        }

        // Get key value
        let key_arg = verb.args.iter()
            .find(|a| a.required && a.arg_type == ArgType::Reference)
            .ok_or_else(|| anyhow!("No key argument found"))?;
        
        let key_value = args.get(&key_arg.canonical_name)
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("{} requires :{}", verb.full_name(), key_arg.name))?;

        let key_col = key_arg.maps_to.as_ref().unwrap_or(&key_arg.name);

        // Build query with joins
        let mut sql = format!(
            r#"SELECT {} FROM "ob-poc".{} t"#,
            select_cols.join(", "),
            table
        );

        for join in &crud.joins {
            sql.push_str(&format!(
                r#" JOIN "ob-poc".{} ON t.{} = {}.{}"#,
                join.table, join.on, join.table, join.on
            ));
        }

        sql.push_str(&format!(r#" WHERE t.{} = $1"#, key_col));

        let row = sqlx::query(&sql)
            .bind(key_value)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!("Select failed: {}", e))?
            .ok_or_else(|| anyhow!("Not found: {}", key_value))?;

        // Convert row to JSON (simplified)
        Ok(StepResult::success(0, verb.full_name())
            .with_data(serde_json::json!({"found": true})))
    }

    async fn execute_update(
        &self,
        verb: &RuntimeVerb,
        crud: &CrudConfig,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<StepResult> {
        let table = &crud.table;
        let key_col = crud.key.as_ref().ok_or_else(|| anyhow!("Update requires key column"))?;

        // Find key value
        let key_arg = verb.args.iter()
            .find(|a| a.maps_to.as_ref() == Some(key_col))
            .ok_or_else(|| anyhow!("No key argument found"))?;

        let key_value = args.get(&key_arg.canonical_name)
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("Key value required"))?;

        // Build SET clause
        let mut sets = Vec::new();
        let mut param_idx = 1;

        for arg in &verb.args {
            if arg.maps_to.as_ref() == Some(key_col) {
                continue;  // Skip key column
            }
            if let Some(col) = &arg.maps_to {
                if let Some(_value) = args.get(&arg.canonical_name) {
                    sets.push(format!("{} = COALESCE(${}, {})", col, param_idx, col));
                    param_idx += 1;
                }
            }
        }

        // Add static sets
        for (col, _val) in &crud.set {
            sets.push(format!("{} = ${}", col, param_idx));
            param_idx += 1;
        }

        sets.push("updated_at = NOW()".to_string());

        let sql = format!(
            r#"UPDATE "ob-poc".{} SET {} WHERE {} = ${} RETURNING {}"#,
            table, sets.join(", "), key_col, param_idx, key_col
        );

        sqlx::query(&sql)
            .bind(key_value)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Update failed: {}", e))?;

        Ok(StepResult::success(0, verb.full_name())
            .with_id(key_value)
            .with_rows(1))
    }

    async fn execute_soft_delete(
        &self,
        verb: &RuntimeVerb,
        crud: &CrudConfig,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<StepResult> {
        let table = &crud.table;
        let key_col = crud.key.as_ref().ok_or_else(|| anyhow!("Delete requires key"))?;
        let status_col = crud.soft_delete_column.as_ref()
            .ok_or_else(|| anyhow!("Soft delete requires status column"))?;
        let status_val = crud.soft_delete_value.as_ref()
            .ok_or_else(|| anyhow!("Soft delete requires status value"))?;

        let key_arg = verb.args.iter()
            .find(|a| a.maps_to.as_ref() == Some(key_col))
            .ok_or_else(|| anyhow!("No key argument"))?;

        let key_value = args.get(&key_arg.canonical_name)
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("Key required"))?;

        let sql = format!(
            r#"UPDATE "ob-poc".{} SET {} = $1, updated_at = NOW() WHERE {} = $2 RETURNING {}"#,
            table, status_col, key_col, key_col
        );

        sqlx::query(&sql)
            .bind(status_val)
            .bind(key_value)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Soft delete failed: {}", e))?;

        Ok(StepResult::success(0, verb.full_name())
            .with_id(key_value)
            .with_rows(1))
    }

    async fn execute_delete(
        &self,
        verb: &RuntimeVerb,
        crud: &CrudConfig,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<StepResult> {
        // Similar to soft_delete but actual DELETE
        let _ = (verb, crud, args);
        Err(anyhow!("Hard delete not implemented"))
    }

    // Helper methods

    async fn resolve_lookup(&self, lookup: &LookupConfig, value: &ResolvedValue) -> Result<Uuid> {
        let code = value.as_string()
            .ok_or_else(|| anyhow!("Lookup value must be string"))?;

        let sql = format!(
            r#"SELECT {} FROM "ob-poc".{} WHERE {} = $1"#,
            lookup.id_column, lookup.table, lookup.code_column
        );

        let id: Uuid = sqlx::query_scalar(&sql)
            .bind(code)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| anyhow!("Unknown {}: {}", lookup.code_column, code))?;

        Ok(id)
    }

    async fn lookup_type_id(&self, table: &str, column: &str, code: &str) -> Result<Uuid> {
        let sql = format!(
            r#"SELECT entity_type_id FROM "ob-poc".{} WHERE {} = $1"#,
            table, column
        );

        sqlx::query_scalar(&sql)
            .bind(code)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| anyhow!("Unknown type: {}", code))
    }

    fn extract_type_from_verb(&self, verb: &str, transform: Option<&str>) -> String {
        // Extract type from verb name like "create-limited-company" -> "LIMITED_COMPANY"
        let type_part = verb.strip_prefix("create-").unwrap_or(verb);
        
        match transform {
            Some("screaming-snake") | None => type_part.to_uppercase().replace('-', "_"),
            _ => type_part.to_string(),
        }
    }

    fn to_sql_value(&self, value: &ResolvedValue) -> Result<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> {
        match value {
            ResolvedValue::String(s) => Ok(Box::new(s.clone())),
            ResolvedValue::Uuid(u) => Ok(Box::new(*u)),
            ResolvedValue::Integer(i) => Ok(Box::new(*i)),
            ResolvedValue::Number(n) => Ok(Box::new(*n)),
            ResolvedValue::Boolean(b) => Ok(Box::new(*b)),
            _ => Err(anyhow!("Unsupported value type")),
        }
    }

    fn yaml_to_sql_value(&self, value: &serde_yaml::Value) -> Result<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> {
        match value {
            serde_yaml::Value::String(s) => Ok(Box::new(s.clone())),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Box::new(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(Box::new(f))
                } else {
                    Err(anyhow!("Invalid number"))
                }
            }
            serde_yaml::Value::Bool(b) => Ok(Box::new(*b)),
            _ => Err(anyhow!("Unsupported YAML value type")),
        }
    }
}
```

---

## Part 7: Config-Driven CSG Linter

### File: `rust/src/dsl_v2/csg_linter_config.rs`

```rust
//! Config-driven CSG linter

use anyhow::Result;
use std::collections::HashMap;

use super::config::{CsgRulesConfig, ConstraintRule, WarningRule, Severity, WhenCondition, RequiresCondition};
use super::execution_plan::{ExecutionPlan, ExecutionStep};
use super::ast::Value;

#[derive(Debug)]
pub struct LintResult {
    pub errors: Vec<LintDiagnostic>,
    pub warnings: Vec<LintDiagnostic>,
    pub infos: Vec<LintDiagnostic>,
}

impl LintResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug)]
pub struct LintDiagnostic {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub step_index: Option<usize>,
}

pub struct ConfigCsgLinter {
    constraints: Vec<ConstraintRule>,
    warnings: Vec<WarningRule>,
    // Entity type tracking for validation
    entity_types: HashMap<String, String>,  // symbol -> entity_type
}

impl ConfigCsgLinter {
    pub fn from_config(config: CsgRulesConfig) -> Self {
        Self {
            constraints: config.constraints,
            warnings: config.warnings,
            entity_types: HashMap::new(),
        }
    }

    pub fn lint(&mut self, plan: &ExecutionPlan) -> LintResult {
        let mut result = LintResult {
            errors: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
        };

        // First pass: collect entity types from create verbs
        self.collect_entity_types(plan);

        // Second pass: validate each step
        for (i, step) in plan.steps.iter().enumerate() {
            self.lint_step(i, step, &mut result);
        }

        result
    }

    fn collect_entity_types(&mut self, plan: &ExecutionPlan) {
        for step in &plan.steps {
            if step.verb_call.domain == "entity" && step.verb_call.verb.starts_with("create-") {
                if let Some(binding) = &step.bind_as {
                    let entity_type = step.verb_call.verb
                        .strip_prefix("create-")
                        .map(|s| s.to_uppercase().replace('-', "_"))
                        .unwrap_or_default();
                    self.entity_types.insert(binding.clone(), entity_type);
                }
            }
        }
    }

    fn lint_step(&self, index: usize, step: &ExecutionStep, result: &mut LintResult) {
        let verb = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);

        // Check constraints
        for rule in &self.constraints {
            if let Some(diagnostic) = self.check_constraint(rule, index, step, &verb) {
                result.errors.push(diagnostic);
            }
        }

        // Check warnings
        for rule in &self.warnings {
            if let Some(diagnostic) = self.check_warning(rule, index, step, &verb) {
                result.warnings.push(diagnostic);
            }
        }
    }

    fn check_constraint(
        &self,
        rule: &ConstraintRule,
        index: usize,
        step: &ExecutionStep,
        verb: &str,
    ) -> Option<LintDiagnostic> {
        // Check if rule applies to this verb
        if let Some(when) = &rule.when {
            if !self.matches_when(when, step, verb) {
                return None;
            }
        }

        // Check built-in checks
        if let Some(check) = &rule.check {
            return self.run_builtin_check(check, rule, index, step);
        }

        // Check requires condition
        if let Some(requires) = &rule.requires {
            if !self.check_requires(requires, step) {
                let message = self.format_error_message(&rule.error, step);
                return Some(LintDiagnostic {
                    rule_id: rule.id.clone(),
                    severity: Severity::Error,
                    message,
                    step_index: Some(index),
                });
            }
        }

        None
    }

    fn check_warning(
        &self,
        rule: &WarningRule,
        index: usize,
        step: &ExecutionStep,
        verb: &str,
    ) -> Option<LintDiagnostic> {
        if let Some(when) = &rule.when {
            if !self.matches_when(when, step, verb) {
                return None;
            }
            
            // If when matches, emit warning
            let message = self.format_warning_message(&rule.message, step);
            return Some(LintDiagnostic {
                rule_id: rule.id.clone(),
                severity: Severity::Warning,
                message,
                step_index: Some(index),
            });
        }

        None
    }

    fn matches_when(&self, when: &WhenCondition, step: &ExecutionStep, verb: &str) -> bool {
        // Check verb match
        if let Some(expected_verb) = &when.verb {
            if verb != expected_verb {
                return false;
            }
        }

        // Check arg_equals
        if let Some(arg_equals) = &when.arg_equals {
            for (arg_name, expected_value) in arg_equals {
                let actual = self.get_arg_value(step, arg_name);
                if actual.as_deref() != Some(expected_value.as_str()) {
                    return false;
                }
            }
        }

        // Check arg_greater_than
        if let Some(arg_gt) = &when.arg_greater_than {
            for (arg_name, threshold) in arg_gt {
                if let Some(actual) = self.get_arg_number(step, arg_name) {
                    if actual <= *threshold {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        // Check arg_less_than
        if let Some(arg_lt) = &when.arg_less_than {
            for (arg_name, threshold) in arg_lt {
                if let Some(actual) = self.get_arg_number(step, arg_name) {
                    if actual >= *threshold {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        true
    }

    fn check_requires(&self, requires: &RequiresCondition, step: &ExecutionStep) -> bool {
        // Get the symbol referenced by the arg
        let symbol = match self.get_arg_reference(step, &requires.arg) {
            Some(s) => s,
            None => return true,  // No reference to check
        };

        // Get entity type for that symbol
        let entity_type = match self.entity_types.get(&symbol) {
            Some(t) => t,
            None => return true,  // Unknown symbol, let executor handle it
        };

        // Check single entity type
        if let Some(required_type) = &requires.entity_type {
            return entity_type == required_type;
        }

        // Check entity type in list
        if let Some(allowed_types) = &requires.entity_type_in {
            return allowed_types.contains(entity_type);
        }

        true
    }

    fn run_builtin_check(
        &self,
        check: &str,
        rule: &ConstraintRule,
        index: usize,
        step: &ExecutionStep,
    ) -> Option<LintDiagnostic> {
        match check {
            "symbol_defined" => {
                // Check all reference args are defined
                for arg in &step.verb_call.arguments {
                    if let Value::Reference(symbol) = &arg.value {
                        if !self.entity_types.contains_key(symbol) {
                            let message = rule.error.replace("{symbol}", symbol);
                            return Some(LintDiagnostic {
                                rule_id: rule.id.clone(),
                                severity: Severity::Error,
                                message,
                                step_index: Some(index),
                            });
                        }
                    }
                }
                None
            }
            _ => None,  // Unknown check
        }
    }

    fn get_arg_value(&self, step: &ExecutionStep, arg_name: &str) -> Option<String> {
        for arg in &step.verb_call.arguments {
            if arg.key.canonical() == arg_name.replace('_', "-") {
                if let Value::String(s) = &arg.value {
                    return Some(s.clone());
                }
            }
        }
        None
    }

    fn get_arg_number(&self, step: &ExecutionStep, arg_name: &str) -> Option<f64> {
        for arg in &step.verb_call.arguments {
            if arg.key.canonical() == arg_name.replace('_', "-") {
                if let Value::Number(n) = &arg.value {
                    return Some(*n);
                }
            }
        }
        None
    }

    fn get_arg_reference(&self, step: &ExecutionStep, arg_name: &str) -> Option<String> {
        for arg in &step.verb_call.arguments {
            if arg.key.canonical() == arg_name.replace('_', "-") {
                if let Value::Reference(s) = &arg.value {
                    return Some(s.clone());
                }
            }
        }
        None
    }

    fn format_error_message(&self, template: &str, step: &ExecutionStep) -> String {
        let mut msg = template.to_string();
        
        // Replace {entity_type} with actual type if we can find it
        if let Some(symbol) = self.get_arg_reference(step, "entity-id") {
            if let Some(entity_type) = self.entity_types.get(&symbol) {
                msg = msg.replace("{entity_type}", entity_type);
            }
        }

        msg
    }

    fn format_warning_message(&self, template: &str, step: &ExecutionStep) -> String {
        let mut msg = template.to_string();

        // Replace {value} with actual value
        if let Some(pct) = self.get_arg_number(step, "ownership-percentage") {
            msg = msg.replace("{value}", &pct.to_string());
        }

        msg
    }
}
```

---

## Part 8: Module Updates

### Update `rust/src/dsl_v2/mod.rs`

```rust
// Add config module
pub mod config;
pub mod verb_registry_config;
pub mod generic_crud;
pub mod csg_linter_config;

// Re-exports
pub use config::ConfigLoader;
pub use verb_registry_config::ConfigVerbRegistry;
pub use generic_crud::GenericCrudExecutor;
pub use csg_linter_config::{ConfigCsgLinter, LintResult};
```

### Update Cargo.toml

```toml
[dependencies]
serde_yaml = "0.9"
```

---

## Execution Checklist

### Phase 1: Create Config Files
- [ ] Create `rust/config/` directory
- [ ] Create `rust/config/verbs.yaml`
- [ ] Create `rust/config/csg_rules.yaml`

### Phase 2: Create Config Types
- [ ] Create `rust/src/dsl_v2/config/mod.rs`
- [ ] Create `rust/src/dsl_v2/config/verb_config.rs`
- [ ] Create `rust/src/dsl_v2/config/csg_config.rs`
- [ ] Create `rust/src/dsl_v2/config/loader.rs`

### Phase 3: Create Runtime Components
- [ ] Create `rust/src/dsl_v2/verb_registry_config.rs`
- [ ] Create `rust/src/dsl_v2/generic_crud.rs`
- [ ] Create `rust/src/dsl_v2/csg_linter_config.rs`

### Phase 4: Integration
- [ ] Update `rust/src/dsl_v2/mod.rs`
- [ ] Update `Cargo.toml` with serde_yaml
- [ ] Add config loading to MCP server startup
- [ ] Add config loading to CLI startup

### Phase 5: Testing
- [ ] `cargo build --features mcp`
- [ ] Test config loading
- [ ] Test dynamic verb generation from DB
- [ ] Test CSG rules from config
- [ ] Verify existing tests still pass

### Phase 6: Migration
- [ ] Gradually move verbs from hardcoded to config
- [ ] Run parallel validation during migration
- [ ] Remove old hardcoded verb definitions
- [ ] Remove old hardcoded CSG rules

---

## Adding a New Verb (After Implementation)

**Before (hardcoded)**:
1. Edit `verbs.rs`
2. Edit `custom_ops/mod.rs`
3. Edit `executor.rs`
4. Recompile
5. Redeploy

**After (config-driven)**:
1. Edit `verbs.yaml`
2. Restart server (or hot-reload if implemented)

Example - adding `document.verify`:

```yaml
# Just add to verbs.yaml:
document:
  verbs:
    verify:
      description: "Mark document as verified"
      behavior: crud
      crud:
        operation: update
        table: documents
        key: document_id
        set:
          status: verified
          verified_at: NOW()
      args:
        - name: document-id
          type: reference
          required: true
          maps_to: document_id
        - name: verified-by
          type: string
          required: true
          maps_to_metadata: verified_by
```

Done. No Rust code needed.
