# Pluggable Research Source Loaders

> **Status:** ✅ IMPLEMENTED (Phases 1-5)
> **Priority:** High - Required for 020 research workflows
> **Created:** 2026-01-10
> **Completed:** 2026-01-10
> **Estimated Effort:** 40-50 hours
> **Dependencies:** 
>   - 020-research-workflows-external-sources.md (orchestration layer)
>   - Existing GLEIF implementation (refactor as reference)
>   - CLAUDE.md and annexes (review before implementation)

---

## Implementation Preamble

**Before implementing any phase of this TODO, Claude must:**

```
1. Review /CLAUDE.md for project conventions and patterns
2. Review /docs/verb-definition-spec.md for verb YAML patterns
3. Review existing GLEIF implementation as reference (see below)
4. Review /docs/research-agent-annex.md for integration points
5. Review rust/src/dsl_v2/custom_ops/ for handler patterns
```

---

## Existing GLEIF Implementation (Reference)

**GLEIF is already implemented. This is the reference pattern for new loaders.**

```
rust/src/gleif/
├── mod.rs              # Re-exports
├── client.rs           # GleifClient - HTTP client, API calls
├── types.rs            # LeiRecord, ChainLink, OwnershipChain, etc.
├── enrichment.rs       # GleifEnrichmentService - orchestration
└── repository.rs       # DB operations (insert/update entities)

rust/src/dsl_v2/custom_ops/gleif_ops.rs    # 1,700+ lines
└── GleifEnrichOp, GleifImportTreeOp, GleifResolveLeiOp, etc.

rust/config/verbs/gleif.yaml               # Verb definitions
└── gleif.enrich, gleif.import-tree, gleif.resolve-lei, etc.
```

**The refactor will:**
1. Extract the common pattern into `SourceLoader` trait
2. Move GLEIF to `rust/src/research/gleif/`
3. Implement trait for GLEIF (minimal changes to existing logic)
4. Use same pattern for CH, SEC EDGAR

---

## ETL Flow: Source → DB

**All loaders follow this flow:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ETL FLOW: SOURCE API → DATABASE                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ┌─────────────┐                                                           │
│   │  EXTERNAL   │  GLEIF API, Companies House API, SEC EDGAR                │
│   │    API      │                                                           │
│   └──────┬──────┘                                                           │
│          │ HTTP response (JSON)                                             │
│          ▼                                                                  │
│   ┌─────────────┐                                                           │
│   │  SOURCE     │  GleifLeiRecord, ChCompanyProfile, SecSubmissions         │
│   │   TYPES     │  Serde structs matching exact API response                │
│   └──────┬──────┘                                                           │
│          │ normalize_*() functions                                          │
│          ▼                                                                  │
│   ┌─────────────┐                                                           │
│   │ NORMALIZED  │  NormalizedEntity, NormalizedControlHolder                │
│   │ STRUCTURES  │  Source-agnostic, maps to our model                       │
│   └──────┬──────┘                                                           │
│          │ repository functions                                             │
│          ▼                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    OUR DATABASE SCHEMA                               │   │
│   │                                                                      │   │
│   │   "ob-poc".entities              ← NormalizedEntity                  │   │
│   │   "ob-poc".entity_limited_companies  ← Company details              │   │
│   │   "ob-poc".entity_natural_persons    ← Individual details           │   │
│   │                                                                      │   │
│   │   kyc.control_relationships      ← NormalizedControlHolder           │   │
│   │   kyc.holdings                   ← Ownership stakes                  │   │
│   │   kyc.officers                   ← NormalizedOfficer                 │   │
│   │                                                                      │   │
│   │   kyc.research_decisions         ← Audit: why this key selected     │   │
│   │   kyc.research_actions           ← Audit: what was imported         │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Target Table Mappings

| Normalized Structure | Target Table(s) | Key Fields |
|---------------------|-----------------|------------|
| `NormalizedEntity` (company) | `entities`, `entity_limited_companies` | `entity_id`, `lei`, `registration_number` |
| `NormalizedEntity` (person) | `entities`, `entity_natural_persons` | `entity_id`, `nationality`, `date_of_birth` |
| `NormalizedControlHolder` | `kyc.control_relationships`, `kyc.holdings` | `holder_entity_id`, `subject_entity_id`, `ownership_pct` |
| `NormalizedOfficer` | `kyc.officers` | `entity_id`, `person_entity_id`, `role` |
| `NormalizedRelationship` | `kyc.ownership_edges` | `parent_entity_id`, `child_entity_id` |

**Entity Resolution:**
- Before creating, check if entity exists by: LEI, registration number, or name+jurisdiction
- Use entity-gateway for fuzzy matching if exact match fails
- Link to existing entity rather than create duplicate

---

## Core Concept

**Source loaders are pluggable API clients that normalize external data into our entity model.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PLUGGABLE SOURCE LOADER PATTERN                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Each source loader provides:                                              │
│                                                                              │
│   1. API CLIENT          - HTTP client, auth, rate limiting                 │
│   2. RESPONSE TYPES      - Serde structs matching API response              │
│   3. NORMALIZER          - Convert API types → our entity model             │
│   4. DSL HANDLERS        - CustomOp implementations                         │
│   5. VERB YAML           - DSL verb definitions with invocation phrases     │
│   6. PROMPT TEMPLATES    - For agent search/disambiguation                  │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   GLEIF (reference)     Companies House       SEC EDGAR                     │
│   ════════════════      ════════════════      ══════════                    │
│                                                                              │
│   GleifClient           ChClient              SecEdgarClient                │
│   GleifLeiRecord        ChCompany             SecFiling                     │
│   GleifRelationship     ChPscRecord           Sec13DGRecord                 │
│        ↓                     ↓                     ↓                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    NORMALIZED STRUCTURES                             │   │
│   │   NormalizedEntity, NormalizedControlHolder, NormalizedOfficer       │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│        ↓                     ↓                     ↓                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    OUR ENTITY MODEL                                  │   │
│   │   entities, control_relationships, holdings, officers                │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## External API Resilience Pattern

> **Reference:** `rust/src/gleif/mod.rs` - full documentation of this pattern

**External APIs change without notice. Never let unknown values crash the pipeline.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE RULE: Capture the raw, map what you know, flag what you don't.        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Anti-pattern (brittle - will break)

```rust
// ❌ DON'T DO THIS - will fail on unknown values
#[derive(Deserialize)]
pub enum ChCompanyStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "dissolved")]
    Dissolved,
    // Companies House sends "liquidation" → deserialize fails → verb crashes
}
```

### Pattern 1: Unknown(String) variant (preferred for enums)

```rust
// ✅ DO THIS - captures unknown values without failing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChCompanyStatus {
    Active,
    Dissolved,
    Liquidation,
    Administration,
    /// Unknown status from Companies House - captured verbatim
    Unknown(String),
}

impl ChCompanyStatus {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => Self::Active,
            "dissolved" => Self::Dissolved,
            "liquidation" => Self::Liquidation,
            "administration" | "voluntary-arrangement" => Self::Administration,
            other => {
                tracing::warn!(status = other, "Unknown Companies House status");
                Self::Unknown(s.to_string())
            }
        }
    }
    
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for ChCompanyStatus {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}
```

### Pattern 2: Store raw, map lazily

```rust
// ✅ ALSO GOOD - especially for codes/IDs that may expand
#[derive(Debug, Deserialize)]
pub struct ChPscRecord {
    pub kind: String,  // Store raw: "individual-person-with-significant-control"
    pub natures_of_control: Vec<String>,  // Store raw array
}

impl ChPscRecord {
    /// Lazy mapping - only when needed
    pub fn holder_type(&self) -> HolderType {
        match self.kind.as_str() {
            "individual-person-with-significant-control" => HolderType::Individual,
            "corporate-entity-person-with-significant-control" => HolderType::Corporate,
            "legal-person-person-with-significant-control" => HolderType::Corporate,
            other => {
                tracing::warn!(kind = other, "Unknown PSC kind");
                HolderType::Unknown
            }
        }
    }
}
```

### Pattern 3: Optional fields with defaults

```rust
// ✅ Handle missing/null fields gracefully
#[derive(Debug, Deserialize)]
pub struct ChCompanyProfile {
    pub company_number: String,
    pub company_name: String,
    
    // These may be missing - use Option or default
    #[serde(default)]
    pub company_status: Option<String>,
    
    #[serde(default)]
    pub sic_codes: Vec<String>,  // Empty if missing
    
    // Flatten unknown fields for debugging
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

### Apply to all loaders

| Source | Enums needing Unknown variant |
|--------|------------------------------|
| **Companies House** | `ChCompanyStatus`, `ChCompanyType`, `ChPscKind`, `ChNatureOfControl`, `ChOfficerRole` |
| **SEC EDGAR** | `SecFilingType`, `SecFilerType`, `SecOwnershipType` |
| **Generic** | All - unknown sources have unpredictable schemas |

### Logging unknown values

```rust
// Log at WARN level so we can add new mappings later
tracing::warn!(
    source = "companies-house",
    field = "company_status", 
    value = raw_status,
    "Unknown enum value - storing as Unknown variant"
);
```

This creates a feedback loop: logs show what new values are appearing, we add mappings in next release.

---

## Source Loader Trait

```rust
// rust/src/research/traits.rs

/// Trait for pluggable research source loaders
#[async_trait]
pub trait SourceLoader: Send + Sync {
    /// Unique identifier for this source
    fn source_id(&self) -> &'static str;
    
    /// Human-readable name
    fn source_name(&self) -> &'static str;
    
    /// Jurisdictions this source covers
    fn jurisdictions(&self) -> &[&'static str];
    
    /// What data types this source provides
    fn provides(&self) -> &[SourceDataType];
    
    /// Search for entities by name (fuzzy)
    async fn search(&self, query: &str, jurisdiction: Option<&str>) 
        -> Result<Vec<SearchCandidate>>;
    
    /// Fetch entity by source-specific key
    async fn fetch_entity(&self, key: &str) -> Result<NormalizedEntity>;
    
    /// Fetch control holders (>threshold% ownership/voting)
    async fn fetch_control_holders(&self, key: &str) 
        -> Result<Vec<NormalizedControlHolder>>;
    
    /// Fetch officers/directors
    async fn fetch_officers(&self, key: &str) -> Result<Vec<NormalizedOfficer>>;
    
    /// Fetch parent chain (if available)
    async fn fetch_parent_chain(&self, key: &str) 
        -> Result<Vec<NormalizedRelationship>>;
    
    /// Validate a key format
    fn validate_key(&self, key: &str) -> bool;
    
    /// Key type name (LEI, COMPANY_NUMBER, CIK)
    fn key_type(&self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceDataType {
    Entity,
    ControlHolders,
    Officers,
    ParentChain,
    Subsidiaries,
    Filings,
}

#[derive(Debug, Clone)]
pub struct SearchCandidate {
    pub key: String,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub status: Option<String>,
    pub score: f64,
    pub metadata: serde_json::Value,
}
```

---

## Normalized Structures

```rust
// rust/src/research/normalized.rs

/// Normalized entity from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEntity {
    // Required
    pub source_key: String,
    pub source_name: String,
    pub name: String,
    
    // Optional identifiers
    pub lei: Option<String>,
    pub registration_number: Option<String>,
    pub tax_id: Option<String>,
    
    // Classification
    pub entity_type: Option<EntityType>,
    pub jurisdiction: Option<String>,
    pub status: Option<EntityStatus>,
    
    // Dates
    pub incorporated_date: Option<NaiveDate>,
    pub dissolved_date: Option<NaiveDate>,
    
    // Address
    pub registered_address: Option<NormalizedAddress>,
    pub business_address: Option<NormalizedAddress>,
    
    // Raw for audit
    pub raw_response: Option<serde_json::Value>,
}

/// Normalized control holder (PSC, 13D/G filer, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedControlHolder {
    // Identity
    pub holder_name: String,
    pub holder_type: HolderType,
    
    // For corporate holders
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub lei: Option<String>,
    
    // For individual holders
    pub nationality: Option<String>,
    pub country_of_residence: Option<String>,
    pub date_of_birth_partial: Option<String>,  // "YYYY-MM" or "YYYY"
    
    // Control details
    pub ownership_pct_low: Option<Decimal>,
    pub ownership_pct_high: Option<Decimal>,
    pub ownership_pct_exact: Option<Decimal>,
    pub voting_pct: Option<Decimal>,
    
    // Control rights
    pub has_voting_rights: bool,
    pub has_appointment_rights: bool,
    pub has_veto_rights: bool,
    pub natures_of_control: Vec<String>,
    
    // Timing
    pub notified_on: Option<NaiveDate>,
    pub ceased_on: Option<NaiveDate>,
    
    // Source
    pub source_document: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HolderType {
    Individual,
    Corporate,
    Trust,
    Partnership,
    Government,
    Nominee,
    Unknown,
}

/// Normalized officer/director
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedOfficer {
    pub name: String,
    pub role: OfficerRole,
    pub appointed_date: Option<NaiveDate>,
    pub resigned_date: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub country_of_residence: Option<String>,
    pub date_of_birth_partial: Option<String>,
    pub occupation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OfficerRole {
    Director,
    Secretary,
    Chairman,
    CEO,
    CFO,
    NonExecutiveDirector,
    AlternateDirector,
    Other(String),
}

/// Normalized parent/subsidiary relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedRelationship {
    pub parent_key: String,
    pub parent_name: String,
    pub child_key: String,
    pub child_name: String,
    pub relationship_type: RelationshipType,
    pub ownership_pct: Option<Decimal>,
    pub is_direct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    DirectParent,
    UltimateParent,
    Subsidiary,
    BranchOf,
}
```

---

## Repository Pattern (ETL to Database)

**Shared repository functions write normalized data to our schema:**

```rust
// rust/src/research/repository.rs

use crate::research::normalized::*;
use sqlx::PgPool;
use uuid::Uuid;

/// Upsert entity from normalized data, returns entity_id
pub async fn upsert_entity(
    pool: &PgPool,
    entity: &NormalizedEntity,
    decision_id: Option<Uuid>,
) -> Result<Uuid> {
    // 1. Check for existing entity by LEI or registration number
    let existing = find_existing_entity(pool, entity).await?;
    
    if let Some(entity_id) = existing {
        // Update existing
        update_entity(pool, entity_id, entity).await?;
        return Ok(entity_id);
    }
    
    // 2. Create new entity
    let entity_id = Uuid::new_v4();
    
    // Base entity
    sqlx::query(r#"
        INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
        VALUES ($1, $2, $3)
    "#)
    .bind(entity_id)
    .bind(entity_type_id_for(&entity.entity_type))
    .bind(&entity.name)
    .execute(pool)
    .await?;
    
    // Type-specific table
    match &entity.entity_type {
        Some(EntityType::LimitedCompany) | Some(EntityType::Corporation) => {
            sqlx::query(r#"
                INSERT INTO "ob-poc".entity_limited_companies 
                (entity_id, lei, registration_number, jurisdiction, status, incorporated_date)
                VALUES ($1, $2, $3, $4, $5, $6)
            "#)
            .bind(entity_id)
            .bind(&entity.lei)
            .bind(&entity.registration_number)
            .bind(&entity.jurisdiction)
            .bind(&entity.status.as_ref().map(|s| s.to_string()))
            .bind(&entity.incorporated_date)
            .execute(pool)
            .await?;
        }
        Some(EntityType::NaturalPerson) => {
            // Insert into entity_natural_persons
        }
        _ => {}
    }
    
    // 3. Record source provenance
    sqlx::query(r#"
        INSERT INTO "ob-poc".entity_source_provenance 
        (entity_id, source_name, source_key, source_key_type, fetched_at, decision_id)
        VALUES ($1, $2, $3, $4, NOW(), $5)
    "#)
    .bind(entity_id)
    .bind(&entity.source_name)
    .bind(&entity.source_key)
    .bind(source_key_type_for(&entity.source_name))
    .bind(decision_id)
    .execute(pool)
    .await?;
    
    Ok(entity_id)
}

/// Find existing entity by identifiers
async fn find_existing_entity(pool: &PgPool, entity: &NormalizedEntity) -> Result<Option<Uuid>> {
    // Try LEI first (globally unique)
    if let Some(lei) = &entity.lei {
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT entity_id FROM "ob-poc".entity_limited_companies WHERE lei = $1"#
        )
        .bind(lei)
        .fetch_optional(pool)
        .await?;
        
        if existing.is_some() {
            return Ok(existing);
        }
    }
    
    // Try registration number + jurisdiction
    if let (Some(reg), Some(jur)) = (&entity.registration_number, &entity.jurisdiction) {
        let existing: Option<Uuid> = sqlx::query_scalar(r#"
            SELECT entity_id FROM "ob-poc".entity_limited_companies 
            WHERE registration_number = $1 AND jurisdiction = $2
        "#)
        .bind(reg)
        .bind(jur)
        .fetch_optional(pool)
        .await?;
        
        if existing.is_some() {
            return Ok(existing);
        }
    }
    
    Ok(None)
}

/// Upsert control holder relationship
pub async fn upsert_control_holder(
    pool: &PgPool,
    subject_entity_id: Uuid,
    holder: &NormalizedControlHolder,
    decision_id: Option<Uuid>,
) -> Result<Uuid> {
    // 1. Find or create holder entity
    let holder_entity_id = find_or_create_holder_entity(pool, holder).await?;
    
    // 2. Create control relationship
    let relationship_id = Uuid::new_v4();
    
    sqlx::query(r#"
        INSERT INTO kyc.control_relationships (
            relationship_id, holder_entity_id, subject_entity_id,
            ownership_pct_low, ownership_pct_high, ownership_pct_exact,
            voting_pct, has_voting_rights, has_appointment_rights, has_veto_rights,
            natures_of_control, notified_on, source_name, decision_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        ON CONFLICT (holder_entity_id, subject_entity_id) DO UPDATE SET
            ownership_pct_low = EXCLUDED.ownership_pct_low,
            ownership_pct_high = EXCLUDED.ownership_pct_high,
            updated_at = NOW()
    "#)
    .bind(relationship_id)
    .bind(holder_entity_id)
    .bind(subject_entity_id)
    .bind(&holder.ownership_pct_low)
    .bind(&holder.ownership_pct_high)
    .bind(&holder.ownership_pct_exact)
    .bind(&holder.voting_pct)
    .bind(holder.has_voting_rights)
    .bind(holder.has_appointment_rights)
    .bind(holder.has_veto_rights)
    .bind(&holder.natures_of_control)
    .bind(&holder.notified_on)
    .bind(source_name_for_holder(holder))
    .bind(decision_id)
    .execute(pool)
    .await?;
    
    Ok(relationship_id)
}

/// Upsert officer
pub async fn upsert_officer(
    pool: &PgPool,
    company_entity_id: Uuid,
    officer: &NormalizedOfficer,
    decision_id: Option<Uuid>,
) -> Result<Uuid> {
    // 1. Find or create person entity for officer
    let person_entity_id = find_or_create_person_entity(pool, officer).await?;
    
    // 2. Create officer record
    let officer_id = Uuid::new_v4();
    
    sqlx::query(r#"
        INSERT INTO kyc.officers (
            officer_id, company_entity_id, person_entity_id,
            role, appointed_date, resigned_date, decision_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (company_entity_id, person_entity_id, role) DO UPDATE SET
            resigned_date = EXCLUDED.resigned_date,
            updated_at = NOW()
    "#)
    .bind(officer_id)
    .bind(company_entity_id)
    .bind(person_entity_id)
    .bind(officer.role.to_string())
    .bind(&officer.appointed_date)
    .bind(&officer.resigned_date)
    .bind(decision_id)
    .execute(pool)
    .await?;
    
    Ok(officer_id)
}
```

---

## Source Registry

```rust
// rust/src/research/registry.rs

/// Registry of available source loaders
pub struct SourceRegistry {
    loaders: HashMap<String, Arc<dyn SourceLoader>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            loaders: HashMap::new(),
        };
        
        // Register built-in sources
        registry.register(Arc::new(GleifLoader::new()));
        registry.register(Arc::new(CompaniesHouseLoader::new()));
        registry.register(Arc::new(SecEdgarLoader::new()));
        
        registry
    }
    
    pub fn register(&mut self, loader: Arc<dyn SourceLoader>) {
        self.loaders.insert(loader.source_id().to_string(), loader);
    }
    
    pub fn get(&self, source_id: &str) -> Option<Arc<dyn SourceLoader>> {
        self.loaders.get(source_id).cloned()
    }
    
    /// Find best source for jurisdiction + data type
    pub fn find_for_jurisdiction(
        &self, 
        jurisdiction: &str, 
        data_type: SourceDataType
    ) -> Vec<Arc<dyn SourceLoader>> {
        self.loaders.values()
            .filter(|l| {
                l.jurisdictions().contains(&jurisdiction) &&
                l.provides().contains(&data_type)
            })
            .cloned()
            .collect()
    }
    
    /// List all registered sources
    pub fn list(&self) -> Vec<SourceInfo> {
        self.loaders.values()
            .map(|l| SourceInfo {
                id: l.source_id().to_string(),
                name: l.source_name().to_string(),
                jurisdictions: l.jurisdictions().iter().map(|s| s.to_string()).collect(),
                provides: l.provides().to_vec(),
                key_type: l.key_type().to_string(),
            })
            .collect()
    }
}
```

---

## GLEIF Loader (Reference Implementation)

```rust
// rust/src/research/gleif/loader.rs

pub struct GleifLoader {
    client: GleifClient,
}

impl GleifLoader {
    pub fn new() -> Self {
        Self {
            client: GleifClient::new(),
        }
    }
}

#[async_trait]
impl SourceLoader for GleifLoader {
    fn source_id(&self) -> &'static str { "gleif" }
    fn source_name(&self) -> &'static str { "GLEIF - Global LEI Foundation" }
    fn jurisdictions(&self) -> &[&'static str] { &["*"] }  // Global
    fn key_type(&self) -> &'static str { "LEI" }
    
    fn provides(&self) -> &[SourceDataType] {
        &[SourceDataType::Entity, SourceDataType::ParentChain]
    }
    
    fn validate_key(&self, key: &str) -> bool {
        // LEI is 20 alphanumeric characters
        key.len() == 20 && key.chars().all(|c| c.is_ascii_alphanumeric())
    }
    
    async fn search(&self, query: &str, jurisdiction: Option<&str>) 
        -> Result<Vec<SearchCandidate>> 
    {
        let results = self.client.fuzzy_search(query).await?;
        
        Ok(results.into_iter()
            .filter(|r| {
                jurisdiction.map_or(true, |j| {
                    r.entity.jurisdiction.as_deref() == Some(j)
                })
            })
            .map(|r| SearchCandidate {
                key: r.lei.clone(),
                name: r.entity.legal_name.clone(),
                jurisdiction: r.entity.jurisdiction.clone(),
                status: Some(r.registration.status.clone()),
                score: calculate_match_score(&r, query),
                metadata: serde_json::to_value(&r).unwrap_or_default(),
            })
            .collect())
    }
    
    async fn fetch_entity(&self, lei: &str) -> Result<NormalizedEntity> {
        let record = self.client.get_by_lei(lei).await?;
        Ok(normalize_gleif_entity(&record))
    }
    
    async fn fetch_control_holders(&self, _lei: &str) 
        -> Result<Vec<NormalizedControlHolder>> 
    {
        // GLEIF doesn't provide shareholder data
        Ok(vec![])
    }
    
    async fn fetch_officers(&self, _lei: &str) -> Result<Vec<NormalizedOfficer>> {
        // GLEIF doesn't provide officer data
        Ok(vec![])
    }
    
    async fn fetch_parent_chain(&self, lei: &str) 
        -> Result<Vec<NormalizedRelationship>> 
    {
        let hierarchy = self.client.get_hierarchy(lei).await?;
        Ok(normalize_gleif_hierarchy(&hierarchy))
    }
}

fn normalize_gleif_entity(record: &GleifLeiRecord) -> NormalizedEntity {
    NormalizedEntity {
        source_key: record.lei.clone(),
        source_name: "GLEIF".into(),
        name: record.entity.legal_name.clone(),
        lei: Some(record.lei.clone()),
        registration_number: record.entity.registration_number.clone(),
        tax_id: None,
        entity_type: map_gleif_entity_type(&record.entity.entity_category),
        jurisdiction: record.entity.jurisdiction.clone(),
        status: map_gleif_status(&record.registration.status),
        incorporated_date: record.entity.creation_date,
        dissolved_date: None,
        registered_address: record.entity.legal_address.as_ref().map(normalize_gleif_address),
        business_address: record.entity.headquarters_address.as_ref().map(normalize_gleif_address),
        raw_response: Some(serde_json::to_value(record).unwrap_or_default()),
    }
}
```

---

## Companies House Loader

### API Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  COMPANIES HOUSE API                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Base URL: https://api.company-information.service.gov.uk                   │
│  Auth: HTTP Basic (API key as username, no password)                        │
│  Rate Limit: 600 requests per 5 minutes                                     │
│  Key Type: Company Number (8 chars, e.g., "12345678" or "SC123456")         │
│                                                                              │
│  Endpoints:                                                                 │
│  ──────────────────────────────────────────────────────────────────────────│
│                                                                              │
│  GET /company/{number}                                                      │
│      → Company profile (name, status, type, addresses)                      │
│                                                                              │
│  GET /company/{number}/persons-with-significant-control                     │
│      → PSC list (>25% ownership/voting/control)                             │
│                                                                              │
│  GET /company/{number}/officers                                             │
│      → Directors, secretaries                                               │
│                                                                              │
│  GET /search/companies?q={query}                                            │
│      → Fuzzy company search                                                 │
│                                                                              │
│  GET /company/{number}/filing-history                                       │
│      → Historical filings (confirmation statements, etc.)                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Implementation

```rust
// rust/src/research/companies_house/client.rs

pub struct CompaniesHouseClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl CompaniesHouseClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            api_key,
            base_url: "https://api.company-information.service.gov.uk".into(),
        }
    }
    
    pub async fn get_company(&self, number: &str) -> Result<ChCompanyProfile> {
        self.get(&format!("/company/{}", number)).await
    }
    
    pub async fn get_psc(&self, number: &str) -> Result<ChPscList> {
        self.get(&format!("/company/{}/persons-with-significant-control", number)).await
    }
    
    pub async fn get_officers(&self, number: &str) -> Result<ChOfficerList> {
        self.get(&format!("/company/{}/officers", number)).await
    }
    
    pub async fn search(&self, query: &str, limit: usize) -> Result<ChSearchResult> {
        self.get(&format!("/search/companies?q={}&items_per_page={}", 
            urlencoding::encode(query), limit)).await
    }
    
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        
        let resp = self.http
            .get(&url)
            .basic_auth(&self.api_key, Option::<&str>::None)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Companies House API error {}: {}", status, body));
        }
        
        Ok(resp.json().await?)
    }
}
```

```rust
// rust/src/research/companies_house/types.rs
//
// RESILIENCE: Uses "store raw, map lazily" pattern.
// All enum-like fields stored as String, mapped via helper methods.
// See "External API Resilience Pattern" section above.

#[derive(Debug, Deserialize)]
pub struct ChCompanyProfile {
    pub company_number: String,
    pub company_name: String,
    pub company_status: String,  // Raw - map via company_status()
    #[serde(rename = "type")]
    pub company_type: String,    // Raw - map via company_type()
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub date_of_creation: Option<String>,
    #[serde(default)]
    pub date_of_cessation: Option<String>,
    #[serde(default)]
    pub registered_office_address: Option<ChAddress>,
    #[serde(default)]
    pub sic_codes: Vec<String>,
    
    // Capture unknown fields for debugging
    #[serde(flatten)]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

impl ChCompanyProfile {
    /// Map raw status to enum (resilient)
    pub fn status(&self) -> CompanyStatus {
        match self.company_status.to_lowercase().as_str() {
            "active" => CompanyStatus::Active,
            "dissolved" => CompanyStatus::Dissolved,
            "liquidation" => CompanyStatus::Liquidation,
            "administration" => CompanyStatus::Administration,
            "voluntary-arrangement" => CompanyStatus::Administration,
            "converted-closed" => CompanyStatus::Dissolved,
            "insolvency-proceedings" => CompanyStatus::Liquidation,
            other => {
                tracing::warn!(status = other, "Unknown CH company_status");
                CompanyStatus::Unknown(self.company_status.clone())
            }
        }
    }
}

/// Resilient company status enum
#[derive(Debug, Clone, PartialEq)]
pub enum CompanyStatus {
    Active,
    Dissolved,
    Liquidation,
    Administration,
    Unknown(String),
}

#[derive(Debug, Deserialize)]
pub struct ChPscList {
    #[serde(default)]
    pub items: Vec<ChPscRecord>,
    #[serde(default)]
    pub active_count: i32,
    #[serde(default)]
    pub ceased_count: i32,
    #[serde(default)]
    pub total_results: i32,
}

#[derive(Debug, Deserialize)]
pub struct ChPscRecord {
    pub name: String,
    pub kind: String,  // Raw - map via holder_type()
    #[serde(default)]
    pub natures_of_control: Vec<String>,  // Raw array - map via extract_percentages()
    
    // Individual - all optional
    #[serde(default)]
    pub nationality: Option<String>,
    #[serde(default)]
    pub country_of_residence: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<ChPartialDate>,
    
    // Corporate - optional
    #[serde(default)]
    pub identification: Option<ChIdentification>,
    
    #[serde(default)]
    pub address: Option<ChAddress>,
    #[serde(default)]
    pub notified_on: Option<String>,
    #[serde(default)]
    pub ceased_on: Option<String>,
}

impl ChPscRecord {
    /// Map PSC kind to holder type (resilient)
    pub fn holder_type(&self) -> HolderType {
        match self.kind.as_str() {
            "individual-person-with-significant-control" => HolderType::Individual,
            "corporate-entity-person-with-significant-control" => HolderType::Corporate,
            "legal-person-person-with-significant-control" => HolderType::Corporate,
            "super-secure-person-with-significant-control" => HolderType::Individual,
            other => {
                tracing::warn!(kind = other, "Unknown PSC kind");
                HolderType::Unknown
            }
        }
    }
    
    /// Extract ownership percentages from natures_of_control (resilient)
    pub fn ownership_range(&self) -> (Option<Decimal>, Option<Decimal>) {
        for nature in &self.natures_of_control {
            match nature.as_str() {
                "ownership-of-shares-25-to-50-percent" => return (Some(dec!(25)), Some(dec!(50))),
                "ownership-of-shares-50-to-75-percent" => return (Some(dec!(50)), Some(dec!(75))),
                "ownership-of-shares-75-to-100-percent" => return (Some(dec!(75)), Some(dec!(100))),
                "ownership-of-shares-25-to-50-percent-as-trust" |
                "ownership-of-shares-25-to-50-percent-as-firm" => return (Some(dec!(25)), Some(dec!(50))),
                _ => continue,  // Unknown nature - skip, don't fail
            }
        }
        (None, None)  // No ownership found - may be voting/control only
    }
}

#[derive(Debug, Deserialize)]
pub struct ChIdentification {
    #[serde(default)]
    pub legal_form: Option<String>,
    #[serde(default)]
    pub legal_authority: Option<String>,
    #[serde(default)]
    pub place_registered: Option<String>,
    #[serde(default)]
    pub registration_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChPartialDate {
    #[serde(default)]
    pub month: Option<i32>,
    pub year: i32,
}

#[derive(Debug, Deserialize)]
pub struct ChOfficerList {
    pub items: Vec<ChOfficer>,
    pub active_count: i32,
    pub resigned_count: i32,
    pub total_results: i32,
}

#[derive(Debug, Deserialize)]
pub struct ChOfficer {
    pub name: String,
    pub officer_role: String,
    pub appointed_on: Option<String>,
    pub resigned_on: Option<String>,
    pub nationality: Option<String>,
    pub country_of_residence: Option<String>,
    pub date_of_birth: Option<ChPartialDate>,
    pub occupation: Option<String>,
    pub address: Option<ChAddress>,
}

#[derive(Debug, Deserialize)]
pub struct ChAddress {
    pub address_line_1: Option<String>,
    pub address_line_2: Option<String>,
    pub locality: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChSearchResult {
    pub items: Vec<ChSearchItem>,
    pub total_results: i32,
}

#[derive(Debug, Deserialize)]
pub struct ChSearchItem {
    pub company_number: String,
    pub title: String,
    pub company_status: String,
    pub company_type: String,
    pub address_snippet: Option<String>,
    pub date_of_creation: Option<String>,
}
```

```rust
// rust/src/research/companies_house/loader.rs

pub struct CompaniesHouseLoader {
    client: CompaniesHouseClient,
}

impl CompaniesHouseLoader {
    pub fn new() -> Self {
        let api_key = std::env::var("COMPANIES_HOUSE_API_KEY")
            .expect("COMPANIES_HOUSE_API_KEY required");
        Self {
            client: CompaniesHouseClient::new(api_key),
        }
    }
}

#[async_trait]
impl SourceLoader for CompaniesHouseLoader {
    fn source_id(&self) -> &'static str { "companies-house" }
    fn source_name(&self) -> &'static str { "UK Companies House" }
    fn jurisdictions(&self) -> &[&'static str] { &["GB", "UK"] }
    fn key_type(&self) -> &'static str { "COMPANY_NUMBER" }
    
    fn provides(&self) -> &[SourceDataType] {
        &[
            SourceDataType::Entity,
            SourceDataType::ControlHolders,
            SourceDataType::Officers,
        ]
    }
    
    fn validate_key(&self, key: &str) -> bool {
        // UK company numbers: 8 chars, may start with letters (SC, NI, etc.)
        let key = key.to_uppercase();
        key.len() == 8 && key.chars().all(|c| c.is_ascii_alphanumeric())
    }
    
    async fn search(&self, query: &str, _jurisdiction: Option<&str>) 
        -> Result<Vec<SearchCandidate>> 
    {
        let results = self.client.search(query, 20).await?;
        
        Ok(results.items.into_iter()
            .map(|r| SearchCandidate {
                key: r.company_number.clone(),
                name: r.title.clone(),
                jurisdiction: Some("GB".into()),
                status: Some(r.company_status.clone()),
                score: calculate_name_similarity(&r.title, query),
                metadata: serde_json::to_value(&r).unwrap_or_default(),
            })
            .collect())
    }
    
    async fn fetch_entity(&self, number: &str) -> Result<NormalizedEntity> {
        let company = self.client.get_company(number).await?;
        Ok(normalize_ch_company(&company))
    }
    
    async fn fetch_control_holders(&self, number: &str) 
        -> Result<Vec<NormalizedControlHolder>> 
    {
        let psc_list = self.client.get_psc(number).await?;
        
        Ok(psc_list.items.into_iter()
            .filter(|p| p.ceased_on.is_none())
            .map(|p| normalize_ch_psc(&p))
            .collect())
    }
    
    async fn fetch_officers(&self, number: &str) -> Result<Vec<NormalizedOfficer>> {
        let officers = self.client.get_officers(number).await?;
        
        Ok(officers.items.into_iter()
            .filter(|o| o.resigned_on.is_none())
            .map(|o| normalize_ch_officer(&o))
            .collect())
    }
    
    async fn fetch_parent_chain(&self, _number: &str) 
        -> Result<Vec<NormalizedRelationship>> 
    {
        // CH doesn't provide parent chain - use PSC for corporate owners
        Ok(vec![])
    }
}

fn normalize_ch_psc(psc: &ChPscRecord) -> NormalizedControlHolder {
    let (pct_low, pct_high) = extract_psc_percentages(&psc.natures_of_control);
    
    NormalizedControlHolder {
        holder_name: psc.name.clone(),
        holder_type: match psc.kind.as_str() {
            "individual-person-with-significant-control" => HolderType::Individual,
            "corporate-entity-person-with-significant-control" => HolderType::Corporate,
            "legal-person-person-with-significant-control" => HolderType::Corporate,
            _ => HolderType::Unknown,
        },
        registration_number: psc.identification.as_ref()
            .and_then(|i| i.registration_number.clone()),
        jurisdiction: psc.identification.as_ref()
            .and_then(|i| i.place_registered.clone()),
        lei: None,
        nationality: psc.nationality.clone(),
        country_of_residence: psc.country_of_residence.clone(),
        date_of_birth_partial: psc.date_of_birth.as_ref()
            .map(|d| format!("{}-{:02}", d.year, d.month.unwrap_or(1))),
        ownership_pct_low: pct_low,
        ownership_pct_high: pct_high,
        ownership_pct_exact: None,
        voting_pct: None,
        has_voting_rights: psc.natures_of_control.iter()
            .any(|n| n.contains("voting")),
        has_appointment_rights: psc.natures_of_control.iter()
            .any(|n| n.contains("appoint")),
        has_veto_rights: false,
        natures_of_control: psc.natures_of_control.clone(),
        notified_on: parse_date(&psc.notified_on),
        ceased_on: psc.ceased_on.as_ref().and_then(|d| parse_date(d)),
        source_document: None,
    }
}

fn extract_psc_percentages(natures: &[String]) -> (Option<Decimal>, Option<Decimal>) {
    for nature in natures {
        match nature.as_str() {
            "ownership-of-shares-25-to-50-percent" => return (Some(dec!(25)), Some(dec!(50))),
            "ownership-of-shares-50-to-75-percent" => return (Some(dec!(50)), Some(dec!(75))),
            "ownership-of-shares-75-to-100-percent" => return (Some(dec!(75)), Some(dec!(100))),
            "ownership-of-shares-more-than-25-percent" => return (Some(dec!(25)), None),
            _ => continue,
        }
    }
    (None, None)
}
```

---

## SEC EDGAR Loader

### API Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SEC EDGAR API                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Base URL: https://data.sec.gov                                             │
│  Auth: None (but User-Agent header REQUIRED)                                │
│  Rate Limit: 10 requests per second                                         │
│  Key Type: CIK (Central Index Key, 10 digits, zero-padded)                  │
│                                                                              │
│  Endpoints:                                                                 │
│  ──────────────────────────────────────────────────────────────────────────│
│                                                                              │
│  GET /submissions/CIK{cik}.json                                             │
│      → Company info + all filings                                           │
│                                                                              │
│  GET /cgi-bin/browse-edgar?action=getcompany&CIK={cik}&type=SC%2013         │
│      → 13D/13G filings for a company                                        │
│                                                                              │
│  POST /cgi-bin/srch-ia                                                      │
│      → Full-text search                                                     │
│                                                                              │
│  ──────────────────────────────────────────────────────────────────────────│
│                                                                              │
│  For 13D/13G beneficial ownership:                                          │
│  → Need to fetch the filing, then parse XML primary document                │
│  → Or use full-text search to find filings mentioning company               │
│                                                                              │
│  13D: Filed within 10 days of acquiring >5%                                 │
│  13G: Passive investors (no intent to control)                              │
│                                                                              │
│  Key fields in 13D/13G:                                                     │
│  • CUSIP (security identifier)                                              │
│  • Percent of class                                                         │
│  • Sole voting power                                                        │
│  • Shared voting power                                                      │
│  • Sole dispositive power                                                   │
│  • Shared dispositive power                                                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Implementation

```rust
// rust/src/research/sec_edgar/client.rs

pub struct SecEdgarClient {
    http: reqwest::Client,
    base_url: String,
}

impl SecEdgarClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("OB-POC/1.0 (compliance@example.com)")  // REQUIRED
                .build()
                .unwrap(),
            base_url: "https://data.sec.gov".into(),
        }
    }
    
    pub async fn get_company(&self, cik: &str) -> Result<SecCompanySubmissions> {
        let cik_padded = format!("{:0>10}", cik.trim_start_matches('0'));
        let url = format!("{}/submissions/CIK{}.json", self.base_url, cik_padded);
        self.get(&url).await
    }
    
    pub async fn get_13dg_filings(&self, cik: &str) -> Result<Vec<Sec13DGFiling>> {
        let submissions = self.get_company(cik).await?;
        
        // Filter to 13D and 13G filings
        let filings: Vec<_> = submissions.filings.recent.form.iter()
            .enumerate()
            .filter(|(_, form)| form.starts_with("SC 13"))
            .map(|(i, _)| Sec13DGFiling {
                accession_number: submissions.filings.recent.accession_number[i].clone(),
                form: submissions.filings.recent.form[i].clone(),
                filing_date: submissions.filings.recent.filing_date[i].clone(),
                primary_document: submissions.filings.recent.primary_document[i].clone(),
            })
            .collect();
        
        Ok(filings)
    }
    
    pub async fn fetch_filing_document(&self, cik: &str, accession: &str, document: &str) 
        -> Result<String> 
    {
        let cik_padded = format!("{:0>10}", cik.trim_start_matches('0'));
        let accession_clean = accession.replace("-", "");
        let url = format!(
            "https://www.sec.gov/Archives/edgar/data/{}/{}/{}", 
            cik_padded, accession_clean, document
        );
        
        let resp = self.http.get(&url).send().await?;
        Ok(resp.text().await?)
    }
    
    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        // Rate limiting - 10 req/sec
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let resp = self.http.get(url).send().await?;
        
        if !resp.status().is_success() {
            return Err(anyhow!("SEC EDGAR error: {}", resp.status()));
        }
        
        Ok(resp.json().await?)
    }
}
```

```rust
// rust/src/research/sec_edgar/types.rs
//
// RESILIENCE: Uses "store raw, map lazily" pattern.
// SEC EDGAR is particularly variable - new form types, field changes common.
// See "External API Resilience Pattern" section above.

#[derive(Debug, Deserialize)]
pub struct SecCompanySubmissions {
    pub cik: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sic: Option<String>,
    #[serde(rename = "sicDescription", default)]
    pub sic_description: Option<String>,
    #[serde(default)]
    pub tickers: Vec<String>,
    #[serde(default)]
    pub exchanges: Vec<String>,
    pub filings: SecFilings,
    
    // Capture unknown fields
    #[serde(flatten)]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct SecFilings {
    pub recent: SecRecentFilings,
}

#[derive(Debug, Deserialize)]
pub struct SecRecentFilings {
    #[serde(rename = "accessionNumber", default)]
    pub accession_number: Vec<String>,
    #[serde(default)]
    pub form: Vec<String>,
    #[serde(rename = "filingDate", default)]
    pub filing_date: Vec<String>,
    #[serde(rename = "primaryDocument", default)]
    pub primary_document: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Sec13DGFiling {
    pub accession_number: String,
    pub form: String,
    pub filing_date: String,
    pub primary_document: String,
}

impl Sec13DGFiling {
    /// Is this an amendment?
    pub fn is_amendment(&self) -> bool {
        self.form.contains("/A")
    }
    
    /// Filing type (resilient)
    pub fn filing_type(&self) -> FilingType {
        match self.form.as_str() {
            "SC 13D" => FilingType::Schedule13D,
            "SC 13D/A" => FilingType::Schedule13DAmendment,
            "SC 13G" => FilingType::Schedule13G,
            "SC 13G/A" => FilingType::Schedule13GAmendment,
            other => {
                tracing::warn!(form = other, "Unknown SEC filing form");
                FilingType::Unknown(self.form.clone())
            }
        }
    }
}

/// Resilient filing type enum
#[derive(Debug, Clone, PartialEq)]
pub enum FilingType {
    Schedule13D,
    Schedule13DAmendment,
    Schedule13G,
    Schedule13GAmendment,
    Unknown(String),
}

/// Parsed 13D/13G beneficial ownership data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecBeneficialOwner {
    pub filer_name: String,
    pub filer_type: String,  // Raw - map via filer_type()
    #[serde(default)]
    pub filer_address: Option<String>,
    
    pub issuer_name: String,
    pub issuer_cusip: String,
    
    #[serde(default)]
    pub percent_of_class: Decimal,
    #[serde(default)]
    pub shares_beneficially_owned: i64,
    
    #[serde(default)]
    pub sole_voting_power: Option<i64>,
    #[serde(default)]
    pub shared_voting_power: Option<i64>,
    #[serde(default)]
    pub sole_dispositive_power: Option<i64>,
    #[serde(default)]
    pub shared_dispositive_power: Option<i64>,
    
    pub filing_date: String,
    pub form_type: String,  // SC 13D, SC 13G, SC 13D/A, etc.
    pub accession_number: String,
}

impl SecBeneficialOwner {
    /// Map filer type code to holder type (resilient)
    /// SEC codes: IN=individual, CO=corporation, PN=partnership, etc.
    pub fn holder_type(&self) -> HolderType {
        match self.filer_type.to_uppercase().as_str() {
            "IN" => HolderType::Individual,
            "CO" | "CP" => HolderType::Corporate,
            "PN" | "LP" | "GP" => HolderType::Partnership,
            "HC" => HolderType::Corporate,  // Holding company
            "IA" | "BD" | "BK" => HolderType::Corporate,  // Investment advisor, broker, bank
            "OO" => HolderType::Unknown,  // Other
            other => {
                tracing::warn!(filer_type = other, "Unknown SEC filer type code");
                HolderType::Unknown
            }
        }
    }
}
```

```rust
// rust/src/research/sec_edgar/loader.rs

pub struct SecEdgarLoader {
    client: SecEdgarClient,
}

#[async_trait]
impl SourceLoader for SecEdgarLoader {
    fn source_id(&self) -> &'static str { "sec-edgar" }
    fn source_name(&self) -> &'static str { "US SEC EDGAR" }
    fn jurisdictions(&self) -> &[&'static str] { &["US"] }
    fn key_type(&self) -> &'static str { "CIK" }
    
    fn provides(&self) -> &[SourceDataType] {
        &[
            SourceDataType::Entity,
            SourceDataType::ControlHolders,
            SourceDataType::Filings,
        ]
    }
    
    fn validate_key(&self, key: &str) -> bool {
        // CIK is up to 10 digits
        let digits_only = key.trim_start_matches('0');
        digits_only.len() <= 10 && digits_only.chars().all(|c| c.is_ascii_digit())
    }
    
    async fn search(&self, query: &str, _jurisdiction: Option<&str>) 
        -> Result<Vec<SearchCandidate>> 
    {
        // SEC doesn't have a good company search API
        // Use company tickers endpoint or full-text search
        // For now, simplified implementation
        Err(anyhow!("SEC search requires ticker or CIK"))
    }
    
    async fn fetch_entity(&self, cik: &str) -> Result<NormalizedEntity> {
        let company = self.client.get_company(cik).await?;
        Ok(normalize_sec_company(&company))
    }
    
    async fn fetch_control_holders(&self, cik: &str) 
        -> Result<Vec<NormalizedControlHolder>> 
    {
        let filings = self.client.get_13dg_filings(cik).await?;
        
        // Parse each filing for beneficial owner data
        // This is complex - 13D/13G are semi-structured XML/SGML
        let mut holders = vec![];
        
        for filing in filings.iter().take(20) {  // Limit to recent
            if let Ok(doc) = self.client.fetch_filing_document(
                cik, &filing.accession_number, &filing.primary_document
            ).await {
                if let Some(owner) = parse_13dg_document(&doc, filing) {
                    holders.push(normalize_sec_beneficial_owner(&owner));
                }
            }
        }
        
        // Dedupe by filer name, keep most recent
        dedupe_beneficial_owners(&mut holders);
        
        Ok(holders)
    }
    
    async fn fetch_officers(&self, _cik: &str) -> Result<Vec<NormalizedOfficer>> {
        // Would need to parse DEF 14A proxy statements
        Ok(vec![])
    }
    
    async fn fetch_parent_chain(&self, _cik: &str) 
        -> Result<Vec<NormalizedRelationship>> 
    {
        Ok(vec![])
    }
}
```

---

## Verb YAML Definitions

```yaml
# rust/config/verbs/research/sources.yaml

domains:
  research.sources:
    description: "Research source management"
    
    verbs:
      list:
        description: "List available research sources"
        invocation_phrases:
          - "what sources are available"
          - "list research sources"
          - "which registries can you access"
        behavior: plugin
        handler: SourceListOp
        returns:
          type: array
          
      info:
        description: "Get info about a specific source"
        invocation_phrases:
          - "tell me about GLEIF"
          - "what can Companies House provide"
        behavior: plugin
        handler: SourceInfoOp
        args:
          - name: source-id
            type: string
            required: true

      search:
        description: "Search any source for an entity"
        invocation_phrases:
          - "search for"
          - "find company"
          - "look up"
        behavior: plugin
        handler: SourceSearchOp
        args:
          - name: source-id
            type: string
            required: true
          - name: query
            type: string
            required: true
          - name: jurisdiction
            type: string

      fetch:
        description: "Fetch entity from source by key"
        invocation_phrases:
          - "fetch from"
          - "get record"
          - "import from"
        behavior: plugin
        handler: SourceFetchOp
        args:
          - name: source-id
            type: string
            required: true
          - name: key
            type: string
            required: true
          - name: include
            type: array
            description: "Data to include: entity, control-holders, officers, parent-chain"
```

```yaml
# rust/config/verbs/research/companies-house.yaml

domains:
  research.companies-house:
    description: "UK Companies House registry"
    
    invocation_hints:
      - "Companies House"
      - "UK company"
      - "British company"
      - "company number"
      - "UK directors"
      - "PSC"
      - "persons with significant control"
    
    verbs:
      import-company:
        description: "Import company by company number"
        invocation_phrases:
          - "import from Companies House"
          - "get UK company"
          - "fetch company"
        behavior: plugin
        handler: ChImportCompanyOp
        args:
          - name: company-number
            type: string
            required: true
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-psc:
        description: "Import Persons with Significant Control"
        invocation_phrases:
          - "get the PSCs"
          - "who controls this UK company"
          - "import UBOs from Companies House"
          - "significant control"
          - "beneficial owners UK"
        behavior: plugin
        handler: ChImportPscOp
        args:
          - name: company-number
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: create-holder-entities
            type: boolean
            default: true
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-officers:
        description: "Import officers (directors, secretaries)"
        invocation_phrases:
          - "get the directors"
          - "import officers"
          - "who runs this company"
          - "board composition"
        behavior: plugin
        handler: ChImportOfficersOp
        args:
          - name: company-number
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: include-resigned
            type: boolean
            default: false
          - name: decision-id
            type: uuid
        returns:
          type: object
```

```yaml
# rust/config/verbs/research/sec-edgar.yaml

domains:
  research.sec:
    description: "US SEC EDGAR filings"
    
    invocation_hints:
      - "SEC"
      - "EDGAR"
      - "US company"
      - "American company"
      - "CIK"
      - "13F"
      - "13D"
      - "13G"
      - "beneficial owner"
    
    verbs:
      import-company:
        description: "Import company by CIK"
        invocation_phrases:
          - "import from SEC"
          - "get SEC company"
          - "fetch from EDGAR"
        behavior: plugin
        handler: SecImportCompanyOp
        args:
          - name: cik
            type: string
            required: true
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-beneficial-owners:
        description: "Import >5% beneficial owners from 13D/13G"
        invocation_phrases:
          - "get beneficial owners"
          - "who owns more than 5%"
          - "13D holders"
          - "13G filers"
          - "significant shareholders US"
        behavior: plugin
        handler: SecImportBeneficialOwnersOp
        args:
          - name: cik
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: as-of-date
            type: date
          - name: decision-id
            type: uuid
        returns:
          type: object

      import-13f-holders:
        description: "Import institutional holders from 13F"
        invocation_phrases:
          - "get 13F holders"
          - "institutional holders"
          - "who are the institutional investors"
        behavior: plugin
        handler: SecImport13FOp
        args:
          - name: cik
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: threshold-pct
            type: decimal
            default: 0
          - name: decision-id
            type: uuid
        returns:
          type: object
```

---

## Directory Structure

```
rust/src/research/
├── mod.rs                          # Re-exports
├── traits.rs                       # SourceLoader trait
├── normalized.rs                   # Normalized structures
├── registry.rs                     # SourceRegistry
├── util.rs                         # Shared utilities (scoring, parsing)
│
├── gleif/
│   ├── mod.rs
│   ├── client.rs                   # GleifClient (existing, refactored)
│   ├── types.rs                    # GleifLeiRecord, etc.
│   ├── loader.rs                   # impl SourceLoader for GleifLoader
│   └── handlers.rs                 # GleifImportEntityOp, etc.
│
├── companies_house/
│   ├── mod.rs
│   ├── client.rs                   # CompaniesHouseClient
│   ├── types.rs                    # ChCompanyProfile, ChPscRecord, etc.
│   ├── loader.rs                   # impl SourceLoader for CompaniesHouseLoader
│   └── handlers.rs                 # ChImportCompanyOp, ChImportPscOp, etc.
│
├── sec_edgar/
│   ├── mod.rs
│   ├── client.rs                   # SecEdgarClient
│   ├── types.rs                    # SecCompanySubmissions, Sec13DGFiling, etc.
│   ├── loader.rs                   # impl SourceLoader for SecEdgarLoader
│   ├── parser.rs                   # 13D/13G document parsing
│   └── handlers.rs                 # SecImportCompanyOp, SecImportBeneficialOwnersOp
│
└── generic/
    ├── mod.rs
    ├── handlers.rs                 # GenericImportEntityOp (for Tier 2/3)
    └── validator.rs                # Validate LLM-extracted data

rust/config/verbs/research/
├── sources.yaml                    # research.sources.* verbs
├── gleif.yaml                      # research.gleif.* verbs (existing, moved)
├── companies-house.yaml            # research.companies-house.* verbs
├── sec-edgar.yaml                  # research.sec.* verbs
└── generic.yaml                    # research.generic.* verbs
```

---

## Environment Variables

```bash
# Source API keys
COMPANIES_HOUSE_API_KEY="your-api-key"     # Required for UK data

# Optional - rate limit tuning
GLEIF_RATE_LIMIT_MS=100
SEC_EDGAR_RATE_LIMIT_MS=100
CH_RATE_LIMIT_MS=100
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (8h) ✅
- [x] 1.1 Create `SourceLoader` trait (`rust/src/research/sources/traits.rs`)
- [x] 1.2 Create normalized structures (`rust/src/research/sources/normalized.rs`)
- [x] 1.3 Create `SourceRegistry` (`rust/src/research/sources/registry.rs`)
- [x] 1.4 Add shared utilities (HTTP client, rate limiting, scoring)

### Phase 2: GLEIF Refactor (6h) ✅
- [x] 2.1 Move existing GLEIF code to new structure (`rust/src/research/sources/gleif/`)
- [x] 2.2 Implement `SourceLoader` for GLEIF
- [x] 2.3 Update verb YAML with invocation phrases
- [x] 2.4 Test against existing functionality

### Phase 3: Companies House (12h) ✅
- [x] 3.1 Implement `CompaniesHouseClient` (`rust/src/research/sources/companies_house/client.rs`)
- [x] 3.2 Define CH response types with resilience pattern
- [x] 3.3 Implement `CompaniesHouseLoader`
- [x] 3.4 Create normalizers (company, PSC, officers)
- [x] 3.5 Implement DSL handlers (`source_loader_ops.rs`)
- [x] 3.6 Create verb YAML (`rust/config/verbs/research/companies-house.yaml`)
- [x] 3.7 Test with real data

### Phase 4: SEC EDGAR (14h) ✅
- [x] 4.1 Implement `SecEdgarClient` (`rust/src/research/sources/sec_edgar/client.rs`)
- [x] 4.2 Define SEC response types with resilience pattern
- [x] 4.3 Implement 13D/13G parser (semi-structured XML)
- [x] 4.4 Implement `SecEdgarLoader`
- [x] 4.5 Create normalizers
- [x] 4.6 Implement DSL handlers
- [x] 4.7 Create verb YAML (`rust/config/verbs/research/sec-edgar.yaml`)
- [x] 4.8 Test with real data

### Phase 5: Generic Import (6h) ✅
- [x] 5.1 Create validator for LLM-extracted data
- [x] 5.2 Implement `GenericImportEntityOp`
- [x] 5.3 Create verb YAML (`rust/config/verbs/research/sources.yaml`)
- [x] 5.4 Test with Tier 2/3 flow from 020

### Phase 6: Integration & Testing (6h) ✅
- [x] 6.1 Integration tests for each source (35 tests passing)
- [x] 6.2 Test source selection logic
- [x] 6.3 Update CLAUDE.md
- [x] 6.4 Update docs/research-agent-annex.md

---

## Estimated Effort

| Phase | Effort |
|-------|--------|
| 1. Core Infrastructure | 8h |
| 2. GLEIF Refactor | 6h |
| 3. Companies House | 12h |
| 4. SEC EDGAR | 14h |
| 5. Generic Import | 6h |
| 6. Integration & Testing | 6h |
| **Total** | **~52h** |

---

## Success Criteria

1. **SourceLoader trait** implemented with search, fetch, normalize
2. **Three Tier 1 loaders** (GLEIF, CH, SEC) all implement trait
3. **Normalized structures** map cleanly to our entity model
4. **Source registry** can select best source for jurisdiction
5. **Verbs work** - can import PSC, import 13D/G, import GLEIF
6. **Invocation phrases** trigger correct verbs from agent
7. **Generic import** works for Tier 2/3 sources
8. **GLEIF unchanged** - refactor doesn't break existing functionality

---

Generated: 2026-01-10
