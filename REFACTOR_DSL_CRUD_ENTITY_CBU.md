# Refactor: DSL.CRUD for Entities and CBU Hub-Spoke Model

**Created:** 2025-11-25  
**Status:** SPECIFICATION  
**Priority:** P0 — Core Business Model  
**Scope:** Seed data, EntityService, DSL words, CrudExecutor, CBU attachment  

---

## Executive Summary

The CBU (Client Business Unit) is a **hub** that collects **spokes** (entities) via roles. Currently:
- Schema exists ✅
- Services partially exist ⚠️
- DSL words exist but don't link ❌
- CrudExecutor creates entities but never attaches to CBU ❌

This refactor delivers complete DSL.CRUD for the entity type tables and proper CBU attachment.

---

## Part 1: Seed Data

### 1.1 Entity Types

```sql
-- sql/seeds/entity_types.sql
INSERT INTO "ob-poc".entity_types (entity_type_id, type_code, type_name, description, is_active)
VALUES
  (gen_random_uuid(), 'PROPER_PERSON', 'Proper Person', 'Natural person / individual', true),
  (gen_random_uuid(), 'LIMITED_COMPANY', 'Limited Company', 'Limited liability company', true),
  (gen_random_uuid(), 'PARTNERSHIP', 'Partnership', 'Partnership (LP, LLP, GP)', true),
  (gen_random_uuid(), 'TRUST', 'Trust', 'Trust structure', true),
  (gen_random_uuid(), 'SICAV', 'SICAV', 'Société d''investissement à capital variable', true),
  (gen_random_uuid(), 'SPV', 'Special Purpose Vehicle', 'Special purpose vehicle', true),
  (gen_random_uuid(), 'FUND', 'Fund', 'Investment fund', true),
  (gen_random_uuid(), 'SOVEREIGN_WEALTH_FUND', 'Sovereign Wealth Fund', 'Government investment fund', true)
ON CONFLICT (type_code) DO NOTHING;
```

### 1.2 Roles

```sql
-- sql/seeds/roles.sql
INSERT INTO "ob-poc".roles (role_id, name, description, created_at)
VALUES
  -- Structural roles (account opening prong)
  (gen_random_uuid(), 'AssetOwner', 'Legal owner of assets', NOW()),
  (gen_random_uuid(), 'InvestmentManager', 'Manages investment decisions', NOW()),
  (gen_random_uuid(), 'ManagementCompany', 'UCITS/AIFM management company', NOW()),
  (gen_random_uuid(), 'Custodian', 'Holds assets in custody', NOW()),
  (gen_random_uuid(), 'Administrator', 'Fund administrator', NOW()),
  (gen_random_uuid(), 'Depositary', 'UCITS/AIFM depositary', NOW()),
  (gen_random_uuid(), 'PrimeBroker', 'Prime brokerage services', NOW()),
  (gen_random_uuid(), 'TransferAgent', 'Shareholder register', NOW()),
  
  -- Operational roles
  (gen_random_uuid(), 'AuditLead', 'Audit team lead', NOW()),
  (gen_random_uuid(), 'TradeCapture', 'Trade capture team', NOW()),
  (gen_random_uuid(), 'FundAccountant', 'Fund accounting team', NOW()),
  (gen_random_uuid(), 'ComplianceOfficer', 'Compliance oversight', NOW()),
  (gen_random_uuid(), 'RelationshipManager', 'Client relationship manager', NOW()),
  
  -- UBO/KYC roles (KYC prong)
  (gen_random_uuid(), 'BeneficialOwner', 'Ultimate beneficial owner (>10% or >25%)', NOW()),
  (gen_random_uuid(), 'ControllingPerson', 'Person with control (not ownership)', NOW()),
  (gen_random_uuid(), 'AuthorizedSignatory', 'Can sign on behalf of entity', NOW()),
  (gen_random_uuid(), 'MaterialInfluence', 'Material influence over activities', NOW()),
  (gen_random_uuid(), 'Director', 'Board director', NOW()),
  (gen_random_uuid(), 'Secretary', 'Company secretary', NOW()),
  
  -- Trust-specific roles
  (gen_random_uuid(), 'Settlor', 'Trust settlor', NOW()),
  (gen_random_uuid(), 'Trustee', 'Trust trustee', NOW()),
  (gen_random_uuid(), 'Beneficiary', 'Trust beneficiary', NOW()),
  (gen_random_uuid(), 'Protector', 'Trust protector', NOW()),
  
  -- Partnership-specific roles
  (gen_random_uuid(), 'GeneralPartner', 'General partner (unlimited liability)', NOW()),
  (gen_random_uuid(), 'LimitedPartner', 'Limited partner', NOW())
ON CONFLICT (name) DO NOTHING;
```

---

## Part 2: EntityService Extensions

### 2.1 New Field Structs

```rust
// rust/src/database/entity_service.rs

/// Fields for creating a limited company
#[derive(Debug, Clone)]
pub struct NewLimitedCompanyFields {
    // Base entity fields
    pub name: String,
    pub jurisdiction: Option<String>,
    pub registration_number: Option<String>,
    // Type-specific fields
    pub incorporation_date: Option<NaiveDate>,
    pub company_number: Option<String>,
    pub registered_office_address: Option<String>,
    pub share_capital: Option<f64>,
    pub currency: Option<String>,
}

/// Fields for creating a partnership
#[derive(Debug, Clone)]
pub struct NewPartnershipFields {
    // Base entity fields
    pub name: String,
    pub jurisdiction: Option<String>,
    pub registration_number: Option<String>,
    // Type-specific fields
    pub partnership_type: Option<String>,  // LP, LLP, GP
    pub formation_date: Option<NaiveDate>,
    pub partnership_agreement_date: Option<NaiveDate>,
}

/// Fields for creating a trust
#[derive(Debug, Clone)]
pub struct NewTrustFields {
    // Base entity fields
    pub name: String,
    pub jurisdiction: Option<String>,
    // Type-specific fields
    pub trust_type: Option<String>,  // Discretionary, Fixed, Unit
    pub formation_date: Option<NaiveDate>,
    pub governing_law: Option<String>,
    pub trust_deed_date: Option<NaiveDate>,
    pub is_revocable: Option<bool>,
}
```

### 2.2 New Service Methods

```rust
impl EntityService {
    /// Create a limited company (entity + extension)
    pub async fn create_limited_company(
        &self,
        fields: &NewLimitedCompanyFields,
    ) -> Result<(Uuid, Uuid)> {  // Returns (entity_id, company_id)
        let entity_id = Uuid::new_v4();
        let company_id = Uuid::new_v4();
        let entity_type_id = self.resolve_entity_type_id("LIMITED_COMPANY").await?;

        // Insert base entity
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities 
                (entity_id, entity_type_id, name, jurisdiction, registration_number, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&fields.name)
        .bind(&fields.jurisdiction)
        .bind(&fields.registration_number)
        .execute(&self.pool)
        .await
        .context("Failed to create entity")?;

        // Insert type extension
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_limited_companies
                (company_id, entity_id, incorporation_date, company_number, 
                 registered_office_address, share_capital, currency, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            "#,
        )
        .bind(company_id)
        .bind(entity_id)
        .bind(fields.incorporation_date)
        .bind(&fields.company_number)
        .bind(&fields.registered_office_address)
        .bind(fields.share_capital)
        .bind(&fields.currency)
        .execute(&self.pool)
        .await
        .context("Failed to create limited company extension")?;

        info!("Created limited company: {} (entity_id: {}, company_id: {})", 
              fields.name, entity_id, company_id);

        Ok((entity_id, company_id))
    }

    /// Create a partnership (entity + extension)
    pub async fn create_partnership(
        &self,
        fields: &NewPartnershipFields,
    ) -> Result<(Uuid, Uuid)> {  // Returns (entity_id, partnership_id)
        let entity_id = Uuid::new_v4();
        let partnership_id = Uuid::new_v4();
        let entity_type_id = self.resolve_entity_type_id("PARTNERSHIP").await?;

        // Insert base entity
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities 
                (entity_id, entity_type_id, name, jurisdiction, registration_number, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&fields.name)
        .bind(&fields.jurisdiction)
        .bind(&fields.registration_number)
        .execute(&self.pool)
        .await
        .context("Failed to create entity")?;

        // Insert type extension
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_partnerships
                (partnership_id, entity_id, partnership_type, formation_date, 
                 partnership_agreement_date, created_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(partnership_id)
        .bind(entity_id)
        .bind(&fields.partnership_type)
        .bind(fields.formation_date)
        .bind(fields.partnership_agreement_date)
        .execute(&self.pool)
        .await
        .context("Failed to create partnership extension")?;

        info!("Created partnership: {} (entity_id: {}, partnership_id: {})", 
              fields.name, entity_id, partnership_id);

        Ok((entity_id, partnership_id))
    }

    /// Create a trust (entity + extension)
    pub async fn create_trust(
        &self,
        fields: &NewTrustFields,
    ) -> Result<(Uuid, Uuid)> {  // Returns (entity_id, trust_id)
        let entity_id = Uuid::new_v4();
        let trust_id = Uuid::new_v4();
        let entity_type_id = self.resolve_entity_type_id("TRUST").await?;

        // Insert base entity
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities 
                (entity_id, entity_type_id, name, jurisdiction, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&fields.name)
        .bind(&fields.jurisdiction)
        .execute(&self.pool)
        .await
        .context("Failed to create entity")?;

        // Insert type extension
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_trusts
                (trust_id, entity_id, trust_type, formation_date, 
                 governing_law, trust_deed_date, is_revocable, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            "#,
        )
        .bind(trust_id)
        .bind(entity_id)
        .bind(&fields.trust_type)
        .bind(fields.formation_date)
        .bind(&fields.governing_law)
        .bind(fields.trust_deed_date)
        .bind(fields.is_revocable)
        .execute(&self.pool)
        .await
        .context("Failed to create trust extension")?;

        info!("Created trust: {} (entity_id: {}, trust_id: {})", 
              fields.name, entity_id, trust_id);

        Ok((entity_id, trust_id))
    }

    /// Get limited company by entity_id
    pub async fn get_limited_company(&self, entity_id: Uuid) -> Result<Option<LimitedCompanyRow>> {
        sqlx::query_as::<_, LimitedCompanyRow>(
            r#"
            SELECT company_id, entity_id, incorporation_date, company_number,
                   registered_office_address, share_capital, currency, created_at
            FROM "ob-poc".entity_limited_companies
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get limited company")
    }

    /// Get partnership by entity_id
    pub async fn get_partnership(&self, entity_id: Uuid) -> Result<Option<PartnershipRow>> {
        sqlx::query_as::<_, PartnershipRow>(
            r#"
            SELECT partnership_id, entity_id, partnership_type, formation_date,
                   partnership_agreement_date, created_at
            FROM "ob-poc".entity_partnerships
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get partnership")
    }

    /// Get trust by entity_id
    pub async fn get_trust(&self, entity_id: Uuid) -> Result<Option<TrustRow>> {
        sqlx::query_as::<_, TrustRow>(
            r#"
            SELECT trust_id, entity_id, trust_type, formation_date,
                   governing_law, trust_deed_date, is_revocable, created_at
            FROM "ob-poc".entity_trusts
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get trust")
    }
}
```

### 2.3 New Row Structs

```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LimitedCompanyRow {
    pub company_id: Uuid,
    pub entity_id: Uuid,
    pub incorporation_date: Option<NaiveDate>,
    pub company_number: Option<String>,
    pub registered_office_address: Option<String>,
    pub share_capital: Option<Decimal>,
    pub currency: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnershipRow {
    pub partnership_id: Uuid,
    pub entity_id: Uuid,
    pub partnership_type: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub partnership_agreement_date: Option<NaiveDate>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrustRow {
    pub trust_id: Uuid,
    pub entity_id: Uuid,
    pub trust_type: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub governing_law: Option<String>,
    pub trust_deed_date: Option<NaiveDate>,
    pub is_revocable: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
}
```

---

## Part 3: DSL Vocabulary

### 3.1 Entity Domain Words

```rust
// rust/src/forth_engine/vocab_registry.rs - Entity domain additions

// ============ ENTITY.PROPER_PERSON ============
WordEntry {
    name: "entity.create-proper-person",
    domain: "entity",
    func: words::entity_create_proper_person,
    signature: ":first-name STRING :last-name STRING :middle-name STRING? :date-of-birth DATE? :nationality STRING? :tax-id STRING?",
    description: "Create a proper person (natural individual)",
    examples: &[
        r#"(entity.create-proper-person :first-name "John" :last-name "Smith" :nationality "GB")"#,
    ],
},

// ============ ENTITY.LIMITED_COMPANY ============
WordEntry {
    name: "entity.create-limited-company",
    domain: "entity",
    func: words::entity_create_limited_company,
    signature: ":name STRING :jurisdiction STRING? :company-number STRING? :incorporation-date DATE? :registered-office STRING? :share-capital DECIMAL? :currency STRING?",
    description: "Create a limited company entity",
    examples: &[
        r#"(entity.create-limited-company :name "Aviva Investors Ltd" :jurisdiction "GB" :company-number "12345678")"#,
    ],
},

// ============ ENTITY.PARTNERSHIP ============
WordEntry {
    name: "entity.create-partnership",
    domain: "entity",
    func: words::entity_create_partnership,
    signature: ":name STRING :jurisdiction STRING? :partnership-type STRING? :formation-date DATE? :agreement-date DATE?",
    description: "Create a partnership entity (LP, LLP, GP)",
    examples: &[
        r#"(entity.create-partnership :name "Alpine Capital Partners LLP" :jurisdiction "GB" :partnership-type "LLP")"#,
    ],
},

// ============ ENTITY.TRUST ============
WordEntry {
    name: "entity.create-trust",
    domain: "entity",
    func: words::entity_create_trust,
    signature: ":name STRING :jurisdiction STRING? :trust-type STRING? :governing-law STRING? :formation-date DATE? :is-revocable BOOL?",
    description: "Create a trust entity",
    examples: &[
        r#"(entity.create-trust :name "Pinnacle Discretionary Trust" :jurisdiction "KY" :trust-type "Discretionary")"#,
    ],
},

// ============ ENTITY READ/UPDATE/DELETE ============
WordEntry {
    name: "entity.read",
    domain: "entity",
    func: words::entity_read,
    signature: ":entity-id UUID",
    description: "Read an entity by ID (returns base + type extension)",
    examples: &[r#"(entity.read :entity-id "550e8400-...")"#],
},
WordEntry {
    name: "entity.update",
    domain: "entity",
    func: words::entity_update,
    signature: ":entity-id UUID :name STRING? :jurisdiction STRING?",
    description: "Update an entity's base fields",
    examples: &[r#"(entity.update :entity-id "..." :name "New Name")"#],
},
WordEntry {
    name: "entity.delete",
    domain: "entity",
    func: words::entity_delete,
    signature: ":entity-id UUID",
    description: "Delete an entity (cascades to type extension)",
    examples: &[r#"(entity.delete :entity-id "...")"#],
},
WordEntry {
    name: "entity.list",
    domain: "entity",
    func: words::entity_list,
    signature: ":entity-type STRING? :jurisdiction STRING?",
    description: "List entities with optional filters",
    examples: &[r#"(entity.list :entity-type "LIMITED_COMPANY" :jurisdiction "GB")"#],
},
```

### 3.2 CBU Attachment Words

```rust
// rust/src/forth_engine/vocab_registry.rs - CBU attachment

WordEntry {
    name: "cbu.attach-entity",
    domain: "cbu",
    func: words::cbu_attach_entity,
    signature: ":cbu-id UUID :entity-id UUID :role STRING :ownership-percent DECIMAL?",
    description: "Attach an existing entity to a CBU with a role",
    examples: &[
        r#"(cbu.attach-entity :cbu-id @cbu :entity-id @company :role "InvestmentManager")"#,
        r#"(cbu.attach-entity :cbu-id @cbu :entity-id @person :role "BeneficialOwner" :ownership-percent 25.0)"#,
    ],
},
WordEntry {
    name: "cbu.detach-entity",
    domain: "cbu",
    func: words::cbu_detach_entity,
    signature: ":cbu-id UUID :entity-id UUID :role STRING?",
    description: "Detach an entity from a CBU (optionally for specific role)",
    examples: &[
        r#"(cbu.detach-entity :cbu-id @cbu :entity-id @company)"#,
        r#"(cbu.detach-entity :cbu-id @cbu :entity-id @person :role "BeneficialOwner")"#,
    ],
},
WordEntry {
    name: "cbu.list-entities",
    domain: "cbu",
    func: words::cbu_list_entities,
    signature: ":cbu-id UUID :role STRING?",
    description: "List all entities attached to a CBU (optionally filtered by role)",
    examples: &[
        r#"(cbu.list-entities :cbu-id @cbu)"#,
        r#"(cbu.list-entities :cbu-id @cbu :role "BeneficialOwner")"#,
    ],
},
WordEntry {
    name: "cbu.update-entity-role",
    domain: "cbu",
    func: words::cbu_update_entity_role,
    signature: ":cbu-id UUID :entity-id UUID :old-role STRING :new-role STRING",
    description: "Change an entity's role within a CBU",
    examples: &[
        r#"(cbu.update-entity-role :cbu-id @cbu :entity-id @person :old-role "Director" :new-role "BeneficialOwner")"#,
    ],
},
```

---

## Part 4: Word Implementations

### 4.1 Entity Creation Words

```rust
// rust/src/forth_engine/words.rs

pub fn entity_create_proper_person(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "PROPER_PERSON".to_string(),
        values,
    }));
    
    Ok(())
}

pub fn entity_create_limited_company(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "LIMITED_COMPANY".to_string(),
        values,
    }));
    
    Ok(())
}

pub fn entity_create_partnership(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "PARTNERSHIP".to_string(),
        values,
    }));
    
    Ok(())
}

pub fn entity_create_trust(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "TRUST".to_string(),
        values,
    }));
    
    Ok(())
}

pub fn entity_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "ENTITY".to_string(),
        where_clause: values,
    }));
    
    Ok(())
}

pub fn entity_list(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "ENTITY_LIST".to_string(),
        where_clause: values,
    }));
    
    Ok(())
}
```

### 4.2 CBU Attachment Words

```rust
pub fn cbu_attach_entity(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_ENTITY_ROLE".to_string(),  // New asset type
        values,
    }));
    
    Ok(())
}

pub fn cbu_detach_entity(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "CBU_ENTITY_ROLE".to_string(),
        where_clause: values,
    }));
    
    Ok(())
}

pub fn cbu_list_entities(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU_ENTITY_ROLE".to_string(),
        where_clause: values,
    }));
    
    Ok(())
}
```

---

## Part 5: CrudExecutor Routing

### 5.1 Entity Type Routing

```rust
// rust/src/database/crud_executor.rs

async fn execute_create(&self, create: &DataCreate) -> Result<CrudExecutionResult> {
    match create.asset.as_str() {
        // ... existing CBU handling ...

        "PROPER_PERSON" => {
            let fields = self.extract_proper_person_fields(&create.values)?;
            let (entity_id, person_id) = self.entity_service.create_proper_person(&fields).await?;
            
            info!("Created proper person: {} {} (entity_id: {})", 
                  fields.first_name, fields.last_name, entity_id);
            
            Ok(CrudExecutionResult {
                operation: "CREATE".to_string(),
                asset: "PROPER_PERSON".to_string(),
                rows_affected: 1,
                generated_id: Some(entity_id),
                data: Some(json!({"entity_id": entity_id, "person_id": person_id})),
            })
        }

        "LIMITED_COMPANY" => {
            let fields = self.extract_limited_company_fields(&create.values)?;
            let (entity_id, company_id) = self.entity_service.create_limited_company(&fields).await?;
            
            info!("Created limited company: {} (entity_id: {})", fields.name, entity_id);
            
            Ok(CrudExecutionResult {
                operation: "CREATE".to_string(),
                asset: "LIMITED_COMPANY".to_string(),
                rows_affected: 1,
                generated_id: Some(entity_id),
                data: Some(json!({"entity_id": entity_id, "company_id": company_id})),
            })
        }

        "PARTNERSHIP" => {
            let fields = self.extract_partnership_fields(&create.values)?;
            let (entity_id, partnership_id) = self.entity_service.create_partnership(&fields).await?;
            
            info!("Created partnership: {} (entity_id: {})", fields.name, entity_id);
            
            Ok(CrudExecutionResult {
                operation: "CREATE".to_string(),
                asset: "PARTNERSHIP".to_string(),
                rows_affected: 1,
                generated_id: Some(entity_id),
                data: Some(json!({"entity_id": entity_id, "partnership_id": partnership_id})),
            })
        }

        "TRUST" => {
            let fields = self.extract_trust_fields(&create.values)?;
            let (entity_id, trust_id) = self.entity_service.create_trust(&fields).await?;
            
            info!("Created trust: {} (entity_id: {})", fields.name, entity_id);
            
            Ok(CrudExecutionResult {
                operation: "CREATE".to_string(),
                asset: "TRUST".to_string(),
                rows_affected: 1,
                generated_id: Some(entity_id),
                data: Some(json!({"entity_id": entity_id, "trust_id": trust_id})),
            })
        }

        "CBU_ENTITY_ROLE" => {
            // THE CRITICAL FIX: Actually link entity to CBU
            let cbu_id = self.get_uuid_value(&create.values, "cbu-id")
                .ok_or_else(|| anyhow!("cbu-id required for CBU_ENTITY_ROLE"))?;
            
            let entity_id = self.get_uuid_value(&create.values, "entity-id")
                .ok_or_else(|| anyhow!("entity-id required for CBU_ENTITY_ROLE"))?;
            
            let role = self.get_string_value(&create.values, "role")
                .ok_or_else(|| anyhow!("role required for CBU_ENTITY_ROLE"))?;
            
            let cbu_entity_role_id = self.cbu_entity_roles_service
                .attach_entity_to_cbu(cbu_id, entity_id, &role)
                .await?;
            
            info!("Attached entity {} to CBU {} with role '{}'", entity_id, cbu_id, role);
            
            Ok(CrudExecutionResult {
                operation: "CREATE".to_string(),
                asset: "CBU_ENTITY_ROLE".to_string(),
                rows_affected: 1,
                generated_id: Some(cbu_entity_role_id),
                data: Some(json!({
                    "cbu_id": cbu_id,
                    "entity_id": entity_id,
                    "role": role
                })),
            })
        }

        _ => Err(anyhow!("Unknown asset type for CREATE: {}", create.asset)),
    }
}
```

### 5.2 Field Extraction Helpers

```rust
impl CrudExecutor {
    fn extract_proper_person_fields(&self, values: &HashMap<String, Value>) -> Result<NewProperPersonFields> {
        Ok(NewProperPersonFields {
            first_name: self.get_string_value(values, "first-name")
                .ok_or_else(|| anyhow!("first-name required"))?,
            last_name: self.get_string_value(values, "last-name")
                .ok_or_else(|| anyhow!("last-name required"))?,
            middle_names: self.get_string_value(values, "middle-name"),
            date_of_birth: self.get_date_value(values, "date-of-birth"),
            nationality: self.get_string_value(values, "nationality"),
            residence_address: self.get_string_value(values, "residence-address"),
            id_document_type: self.get_string_value(values, "id-document-type"),
            id_document_number: self.get_string_value(values, "id-document-number"),
        })
    }

    fn extract_limited_company_fields(&self, values: &HashMap<String, Value>) -> Result<NewLimitedCompanyFields> {
        Ok(NewLimitedCompanyFields {
            name: self.get_string_value(values, "name")
                .ok_or_else(|| anyhow!("name required"))?,
            jurisdiction: self.get_string_value(values, "jurisdiction"),
            registration_number: self.get_string_value(values, "registration-number")
                .or_else(|| self.get_string_value(values, "company-number")),
            incorporation_date: self.get_date_value(values, "incorporation-date"),
            company_number: self.get_string_value(values, "company-number"),
            registered_office_address: self.get_string_value(values, "registered-office"),
            share_capital: self.get_decimal_value(values, "share-capital"),
            currency: self.get_string_value(values, "currency"),
        })
    }

    fn extract_partnership_fields(&self, values: &HashMap<String, Value>) -> Result<NewPartnershipFields> {
        Ok(NewPartnershipFields {
            name: self.get_string_value(values, "name")
                .ok_or_else(|| anyhow!("name required"))?,
            jurisdiction: self.get_string_value(values, "jurisdiction"),
            registration_number: self.get_string_value(values, "registration-number"),
            partnership_type: self.get_string_value(values, "partnership-type"),
            formation_date: self.get_date_value(values, "formation-date"),
            partnership_agreement_date: self.get_date_value(values, "agreement-date"),
        })
    }

    fn extract_trust_fields(&self, values: &HashMap<String, Value>) -> Result<NewTrustFields> {
        Ok(NewTrustFields {
            name: self.get_string_value(values, "name")
                .ok_or_else(|| anyhow!("name required"))?,
            jurisdiction: self.get_string_value(values, "jurisdiction"),
            trust_type: self.get_string_value(values, "trust-type"),
            formation_date: self.get_date_value(values, "formation-date"),
            governing_law: self.get_string_value(values, "governing-law"),
            trust_deed_date: self.get_date_value(values, "trust-deed-date"),
            is_revocable: self.get_bool_value(values, "is-revocable"),
        })
    }

    fn get_date_value(&self, values: &HashMap<String, Value>, key: &str) -> Option<NaiveDate> {
        self.get_string_value(values, key)
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
    }

    fn get_decimal_value(&self, values: &HashMap<String, Value>, key: &str) -> Option<f64> {
        match values.get(key) {
            Some(Value::Float(f)) => Some(*f),
            Some(Value::Integer(i)) => Some(*i as f64),
            Some(Value::String(s)) => s.parse().ok(),
            _ => None,
        }
    }

    fn get_bool_value(&self, values: &HashMap<String, Value>, key: &str) -> Option<bool> {
        match values.get(key) {
            Some(Value::Bool(b)) => Some(*b),
            Some(Value::String(s)) => s.to_lowercase().parse().ok(),
            _ => None,
        }
    }
}
```

---

## Part 6: Example DSL Session

```clojure
;; =============================================================================
;; SESSION 1: Create entities (the spokes)
;; =============================================================================

;; Create the investment manager
(entity.create-limited-company
  :name "Aviva Investors Ltd"
  :jurisdiction "GB"
  :company-number "12345678"
  :incorporation-date "2010-03-15"
  :registered-office "1 Poultry, London EC2R 8EJ")

;; Create the management company
(entity.create-limited-company
  :name "Meridian ManCo S.à r.l."
  :jurisdiction "LU"
  :company-number "LU-B234568"
  :incorporation-date "2022-01-10")

;; Create a beneficial owner (proper person)
(entity.create-proper-person
  :first-name "Chen"
  :last-name "Wei"
  :date-of-birth "1968-04-12"
  :nationality "SG")

;; Create an LLP
(entity.create-partnership
  :name "Alpine Capital Partners LLP"
  :jurisdiction "GB"
  :partnership-type "LLP"
  :formation-date "2015-06-01")

;; Create a trust structure
(entity.create-trust
  :name "Pinnacle Discretionary Trust"
  :jurisdiction "KY"
  :trust-type "Discretionary"
  :governing-law "Cayman Islands"
  :is-revocable false)

;; =============================================================================
;; SESSION 2: Create the hub (CBU)
;; =============================================================================

(cbu.create
  :cbu-name "Aviva EU Bond Fund"
  :nature-purpose "Bond investment fund for European fixed income"
  :jurisdiction "LU"
  :client-type "UCITS")

;; =============================================================================
;; SESSION 3: Attach entities to CBU with roles (connect spokes to hub)
;; =============================================================================

;; Structural roles (account opening prong)
(cbu.attach-entity
  :cbu-id "550e8400-e29b-41d4-a716-446655440000"  ;; CBU UUID from session 2
  :entity-id "660e8400-e29b-41d4-a716-446655440001"  ;; Aviva Investors
  :role "InvestmentManager")

(cbu.attach-entity
  :cbu-id "550e8400-e29b-41d4-a716-446655440000"
  :entity-id "660e8400-e29b-41d4-a716-446655440002"  ;; Meridian ManCo
  :role "ManagementCompany")

;; UBO roles (KYC prong)
(cbu.attach-entity
  :cbu-id "550e8400-e29b-41d4-a716-446655440000"
  :entity-id "660e8400-e29b-41d4-a716-446655440003"  ;; Chen Wei
  :role "BeneficialOwner"
  :ownership-percent 25.0)

;; =============================================================================
;; SESSION 4: Query the structure
;; =============================================================================

(cbu.list-entities :cbu-id "550e8400-e29b-41d4-a716-446655440000")
;; Returns all entities with their roles

(cbu.list-entities :cbu-id "550e8400-..." :role "BeneficialOwner")
;; Returns only UBOs
```

---

## Part 7: Files to Create/Modify

### Create
| File | Purpose |
|------|---------|
| `sql/seeds/entity_types.sql` | Seed entity type codes |
| `sql/seeds/roles.sql` | Seed role names |

### Modify
| File | Changes |
|------|---------|
| `rust/src/database/entity_service.rs` | Add `NewLimitedCompanyFields`, `NewPartnershipFields`, `NewTrustFields`, `create_*` methods, row structs |
| `rust/src/database/crud_executor.rs` | Add routing for `LIMITED_COMPANY`, `PARTNERSHIP`, `TRUST`, `CBU_ENTITY_ROLE`; add field extraction helpers |
| `rust/src/forth_engine/words.rs` | Add `entity_create_limited_company`, `entity_create_partnership`, `entity_create_trust`, `cbu_attach_entity`, etc. |
| `rust/src/forth_engine/vocab_registry.rs` | Register new words with signatures and examples |
| `rust/src/database/mod.rs` | Export new structs |

---

## Part 8: Verification

After implementation:

```bash
# 1. Run seeds
psql $DATABASE_URL -f sql/seeds/entity_types.sql
psql $DATABASE_URL -f sql/seeds/roles.sql

# 2. Build and test
cd rust && cargo check --features database
cargo test --features database

# 3. Integration test - create entity via DSL
cargo run --features database -- execute "(entity.create-limited-company :name \"Test Co\" :jurisdiction \"GB\")"

# 4. Integration test - attach to CBU
cargo run --features database -- execute "(cbu.attach-entity :cbu-id \"...\" :entity-id \"...\" :role \"InvestmentManager\")"

# 5. Verify in DB
psql $DATABASE_URL -c "SELECT * FROM \"ob-poc\".cbu_entity_roles"
```

---

## Summary

| Layer | Before | After |
|-------|--------|-------|
| **Seed Data** | Empty | `entity_types` + `roles` populated |
| **EntityService** | `create_entity`, `create_proper_person` only | + `create_limited_company`, `create_partnership`, `create_trust` |
| **DSL Words** | Weak signatures, missing types | Full CRUD for all entity types |
| **CrudExecutor** | Creates entities, doesn't link | Routes all types, **links to CBU via `cbu_entity_roles`** |
| **CBU Hub-Spoke** | Broken | **Working** |

This delivers the complete DSL.CRUD for the entity type tables and proper CBU attachment mechanism.
