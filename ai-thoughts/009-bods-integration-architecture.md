# BODS 0.4 Integration Architecture

> **BODS:** Beneficial Ownership Data Standard (Open Ownership)
> **Version:** 0.4
> **Purpose:** Industry standard for representing ownership structures, perfect for OB-POC export/import

---

## BODS 0.4 Core Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BODS 0.4 DATA MODEL                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Statement (wrapper)                                                        │
│  ├── statementId (globally unique)                                         │
│  ├── statementDate                                                         │
│  ├── publicationDetails (bodsVersion, publisher, license)                  │
│  └── recordDetails → one of:                                               │
│                                                                             │
│      ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐ │
│      │  Entity Record  │  │  Person Record  │  │  Relationship Record    │ │
│      ├─────────────────┤  ├─────────────────┤  ├─────────────────────────┤ │
│      │ entityType      │  │ personType      │  │ subject (entity)        │ │
│      │ name            │  │ names[]         │  │ interestedParty         │ │
│      │ jurisdiction    │  │ nationalities[] │  │   (entity or person)    │ │
│      │ identifiers[]   │  │ birthDate       │  │ interests[]             │ │
│      │   (LEI, etc)    │  │ addresses[]     │  │   type (shareholding,   │ │
│      │ addresses[]     │  │ politicalExp    │  │     votingRights, etc)  │ │
│      │ publicListing   │  │   status (PEP)  │  │   share (exact/range)   │ │
│      │ foundingDate    │  │   details[]     │  │   directOrIndirect      │ │
│      │ dissolutionDate │  │ taxResidencies  │  │   beneficialOwnership   │ │
│      └─────────────────┘  └─────────────────┘  └─────────────────────────┘ │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Mapping: OB-POC → BODS 0.4

### Entity Mapping

| OB-POC Field | BODS 0.4 Field | Notes |
|--------------|----------------|-------|
| `concrete_entities.name` | `entityRecord.name` | Direct |
| `concrete_entities.legal_name` | `entityRecord.alternateNames[0]` | If different from name |
| `concrete_entities.entity_type` | `entityRecord.entityType.type` | Map enum |
| `concrete_entities.jurisdiction` | `entityRecord.jurisdiction.code` | ISO 3166 |
| `gleif_records.lei` | `entityRecord.identifiers[].id` with scheme=`lei` | |
| `gleif_records.registration_status` | Derive dissolution if RETIRED | |
| `concrete_entities.incorporation_date` | `entityRecord.foundingDate` | |
| `concrete_entities.addresses` | `entityRecord.addresses[]` | |
| `public_companies.*` | `entityRecord.publicListing` | Securities listings |

**Entity Type Mapping:**

| OB-POC | BODS 0.4 |
|--------|----------|
| `Corporation` | `registeredEntity` |
| `LLC` | `registeredEntity` |
| `Partnership` | `legalEntity` |
| `Trust` | `arrangement` with subtype `trust` |
| `Fund` | `arrangement` |
| `GovernmentEntity` | `stateBody` |
| `Unknown` | `unknownEntity` |

### Person Mapping

| OB-POC Field | BODS 0.4 Field | Notes |
|--------------|----------------|-------|
| `persons.full_name` | `personRecord.names[0].fullName` | |
| `persons.given_name` | `personRecord.names[0].givenName` | |
| `persons.family_name` | `personRecord.names[0].familyName` | |
| `persons.date_of_birth` | `personRecord.birthDate` | YYYY-MM-DD |
| `persons.nationalities` | `personRecord.nationalities[]` | ISO 3166 |
| `persons.addresses` | `personRecord.addresses[]` | |
| `pep_screenings.is_pep` | `personRecord.politicalExposure.status` | isPep/isNotPep |
| `pep_screenings.pep_details` | `personRecord.politicalExposure.details[]` | |

**Person Type Mapping:**

| OB-POC | BODS 0.4 |
|--------|----------|
| Person with full details | `knownPerson` |
| Person with name only | `knownPerson` (partial) |
| "Unknown UBO" placeholder | `unknownPerson` |
| Redacted/anonymous | `anonymousPerson` |

### Relationship Mapping

| OB-POC Field | BODS 0.4 Field | Notes |
|--------------|----------------|-------|
| `ownership_edges.target_entity_id` | `relationshipRecord.subject` | Entity being owned |
| `ownership_edges.source_entity_id` | `relationshipRecord.interestedParty` | The owner |
| `ownership_edges.ownership_percentage` | `interests[].share.exact` | |
| `ownership_edges.ownership_type` | `interests[].type` | Map to codelist |
| `ownership_edges.is_direct` | `interests[].directOrIndirect` | direct/indirect |
| `ownership_edges.effective_date` | `interests[].startDate` | |
| `ownership_edges.end_date` | `interests[].endDate` | |
| `ubo_determinations.is_ubo` | `interests[].beneficialOwnershipOrControl` | Boolean |

**Interest Type Mapping:**

| OB-POC | BODS 0.4 interestType |
|--------|----------------------|
| `Equity` | `shareholding` |
| `VotingRights` | `votingRights` |
| `BoardAppointment` | `appointmentOfBoard` |
| `Control` | `otherInfluenceOrControl` |
| `Management` | `seniorManagingOfficial` |
| `Trustee` | `trustee` |
| `Settlor` | `settlor` |
| `Beneficiary` | `beneficiaryOfLegalArrangement` |
| `Nominee` | `nominee` |
| `Nominator` | `nominator` |

---

## BODS Export DSL Verbs

### `bods.export` - Export CBU as BODS Package

```clojure
(bods.export 
  :cbu @allianz 
  :format "json"
  :include-components true
  :as-of-date "2025-01-08")
```

**Parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `:cbu` | ref | yes | CBU to export |
| `:format` | string | no | "json" (default) or "jsonl" |
| `:include-components` | bool | no | Include intermediate entities in indirect chains |
| `:as-of-date` | date | no | Point-in-time export (default: latest) |
| `:publisher` | string | no | Publisher name for metadata |

**Returns:** BODS 0.4 compliant JSON array of Statements

### `bods.validate` - Validate Against BODS Schema

```clojure
(bods.validate :cbu @allianz)
```

**Returns:** Validation report with any schema violations

### `bods.import` - Import BODS Package

```clojure
(bods.import :file "/path/to/bods-package.json" :target-cbu @new-client)
```

**Parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `:file` | path | yes | BODS JSON file to import |
| `:target-cbu` | ref | no | CBU to import into (creates new if omitted) |
| `:merge-strategy` | string | no | "replace", "merge", "append" |

---

## Implementation Files

### 1. BODS Types

**File:** `rust/crates/ob-poc-types/src/bods.rs`

```rust
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// BODS 0.4 Statement wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodsStatement {
    pub statement_id: String,
    pub statement_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<Annotation>>,
    pub publication_details: PublicationDetails,
    pub record_type: RecordType,
    pub record_id: String,
    pub record_details: RecordDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordType {
    Entity,
    Person,
    Relationship,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RecordDetails {
    Entity(EntityRecord),
    Person(PersonRecord),
    Relationship(RelationshipRecord),
}

/// BODS Entity Record
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRecord {
    pub is_component: bool,
    pub entity_type: EntityType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternate_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<Jurisdiction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<Identifier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub founding_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dissolution_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<Address>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_listing: Option<PublicListing>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityType {
    #[serde(rename = "type")]
    pub type_: EntityTypeCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityTypeCode {
    RegisteredEntity,
    LegalEntity,
    Arrangement,
    AnonymousEntity,
    UnknownEntity,
    State,
    StateBody,
}

/// BODS Person Record
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRecord {
    pub is_component: bool,
    pub person_type: PersonType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<PersonName>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationalities: Option<Vec<Country>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub death_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<Address>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub political_exposure: Option<PoliticalExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_residencies: Option<Vec<Country>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PersonType {
    AnonymousPerson,
    UnknownPerson,
    KnownPerson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoliticalExposure {
    pub status: PepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<PepStatusDetails>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PepStatus {
    IsPep,
    IsNotPep,
    Unknown,
}

/// BODS Relationship Record
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipRecord {
    pub is_component: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_records: Option<Vec<String>>,
    pub subject: String,  // recordId of subject entity
    pub interested_party: InterestedParty,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interests: Option<Vec<Interest>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InterestedParty {
    RecordId(String),
    Unspecified(UnspecifiedRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interest {
    #[serde(rename = "type")]
    pub type_: InterestType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_or_indirect: Option<DirectOrIndirect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beneficial_ownership_or_control: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share: Option<Share>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InterestType {
    Shareholding,
    VotingRights,
    AppointmentOfBoard,
    OtherInfluenceOrControl,
    SeniorManagingOfficial,
    Settlor,
    Trustee,
    Protector,
    BeneficiaryOfLegalArrangement,
    RightsToSurplusAssetsOnDissolution,
    RightsToProfitOrIncome,
    RightsGrantedByContract,
    ConditionalRightsGrantedByContract,
    ControlViaCompanyRulesOrArticles,
    ControlByLegalFramework,
    BoardMember,
    BoardChair,
    UnknownInterest,
    UnpublishedInterest,
    EnjoymentAndUseOfAssets,
    RightToProfitOrIncomeFromAssets,
    Nominee,
    Nominator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DirectOrIndirect {
    Direct,
    Indirect,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<f64>,
}

// ... additional component types (Identifier, Address, etc.)
```

### 2. BODS Exporter Service

**File:** `rust/src/services/bods_exporter.rs`

```rust
use crate::database::*;
use ob_poc_types::bods::*;
use uuid::Uuid;
use chrono::Utc;

pub struct BodsExporter {
    db: DatabasePool,
}

impl BodsExporter {
    /// Export a CBU as a BODS 0.4 package
    pub async fn export_cbu(&self, cbu_id: Uuid, options: ExportOptions) -> Result<Vec<BodsStatement>> {
        let mut statements = Vec::new();
        
        // 1. Get all entities in CBU
        let entities = self.db.get_cbu_entities(cbu_id).await?;
        
        // 2. Export each entity as EntityRecord
        for entity in &entities {
            let stmt = self.entity_to_bods(entity, &options)?;
            statements.push(stmt);
        }
        
        // 3. Get all persons (UBOs) connected to CBU
        let persons = self.db.get_cbu_persons(cbu_id).await?;
        
        // 4. Export each person as PersonRecord
        for person in &persons {
            let stmt = self.person_to_bods(person, &options)?;
            statements.push(stmt);
        }
        
        // 5. Get all ownership relationships
        let relationships = self.db.get_cbu_ownership_edges(cbu_id).await?;
        
        // 6. Export each relationship as RelationshipRecord
        for rel in &relationships {
            let stmt = self.relationship_to_bods(rel, &options)?;
            statements.push(stmt);
        }
        
        Ok(statements)
    }
    
    fn entity_to_bods(&self, entity: &ConcreteEntity, options: &ExportOptions) -> Result<BodsStatement> {
        let record_id = format!("entity-{}", entity.id);
        
        let entity_record = EntityRecord {
            is_component: false,
            entity_type: EntityType {
                type_: self.map_entity_type(&entity.entity_type),
                subtype: None,
                details: None,
            },
            name: Some(entity.name.clone()),
            alternate_names: entity.legal_name.as_ref().map(|n| vec![n.clone()]),
            jurisdiction: entity.jurisdiction.as_ref().map(|j| Jurisdiction {
                code: j.clone(),
                name: None,
            }),
            identifiers: self.build_identifiers(entity),
            founding_date: entity.incorporation_date.map(|d| d.to_string()),
            dissolution_date: None,
            addresses: self.build_addresses(entity),
            public_listing: None, // TODO: add if public company
        };
        
        Ok(BodsStatement {
            statement_id: self.generate_statement_id(),
            statement_date: Utc::now().format("%Y-%m-%d").to_string(),
            annotations: None,
            publication_details: self.build_publication_details(options),
            record_type: RecordType::Entity,
            record_id,
            record_details: RecordDetails::Entity(entity_record),
        })
    }
    
    fn person_to_bods(&self, person: &Person, options: &ExportOptions) -> Result<BodsStatement> {
        let record_id = format!("person-{}", person.id);
        
        let person_record = PersonRecord {
            is_component: false,
            person_type: PersonType::KnownPerson,
            names: Some(vec![PersonName {
                type_: Some(NameType::Legal),
                full_name: person.full_name.clone(),
                family_name: person.family_name.clone(),
                given_name: person.given_name.clone(),
                patronymic_name: None,
            }]),
            nationalities: person.nationalities.as_ref().map(|ns| {
                ns.iter().map(|n| Country { code: n.clone(), name: None }).collect()
            }),
            birth_date: person.date_of_birth.map(|d| d.to_string()),
            death_date: None,
            addresses: None,
            political_exposure: self.build_pep_status(person),
            tax_residencies: None,
        };
        
        Ok(BodsStatement {
            statement_id: self.generate_statement_id(),
            statement_date: Utc::now().format("%Y-%m-%d").to_string(),
            annotations: None,
            publication_details: self.build_publication_details(options),
            record_type: RecordType::Person,
            record_id,
            record_details: RecordDetails::Person(person_record),
        })
    }
    
    fn relationship_to_bods(&self, edge: &OwnershipEdge, options: &ExportOptions) -> Result<BodsStatement> {
        let record_id = format!("relationship-{}", edge.id);
        
        // Subject is the entity being owned
        let subject = format!("entity-{}", edge.target_entity_id);
        
        // Interested party is the owner (could be entity or person)
        let interested_party = if let Some(entity_id) = edge.source_entity_id {
            InterestedParty::RecordId(format!("entity-{}", entity_id))
        } else if let Some(person_id) = edge.source_person_id {
            InterestedParty::RecordId(format!("person-{}", person_id))
        } else {
            return Err(anyhow::anyhow!("Ownership edge has no source"));
        };
        
        let interest = Interest {
            type_: self.map_interest_type(&edge.ownership_type),
            direct_or_indirect: Some(if edge.is_direct {
                DirectOrIndirect::Direct
            } else {
                DirectOrIndirect::Indirect
            }),
            beneficial_ownership_or_control: edge.is_ubo,
            share: edge.ownership_percentage.map(|pct| Share {
                exact: Some(pct as f64),
                minimum: None,
                maximum: None,
                exclusive_minimum: None,
                exclusive_maximum: None,
            }),
            start_date: edge.effective_date.map(|d| d.to_string()),
            end_date: edge.end_date.map(|d| d.to_string()),
            details: None,
        };
        
        let rel_record = RelationshipRecord {
            is_component: false,
            component_records: None,
            subject,
            interested_party,
            interests: Some(vec![interest]),
        };
        
        Ok(BodsStatement {
            statement_id: self.generate_statement_id(),
            statement_date: Utc::now().format("%Y-%m-%d").to_string(),
            annotations: None,
            publication_details: self.build_publication_details(options),
            record_type: RecordType::Relationship,
            record_id,
            record_details: RecordDetails::Relationship(rel_record),
        })
    }
    
    fn map_interest_type(&self, ownership_type: &str) -> InterestType {
        match ownership_type.to_lowercase().as_str() {
            "equity" | "shares" | "shareholding" => InterestType::Shareholding,
            "voting" | "voting_rights" => InterestType::VotingRights,
            "board" | "board_appointment" => InterestType::AppointmentOfBoard,
            "control" => InterestType::OtherInfluenceOrControl,
            "management" => InterestType::SeniorManagingOfficial,
            "trustee" => InterestType::Trustee,
            "settlor" => InterestType::Settlor,
            "beneficiary" => InterestType::BeneficiaryOfLegalArrangement,
            "nominee" => InterestType::Nominee,
            _ => InterestType::OtherInfluenceOrControl,
        }
    }
}
```

---

## RAG Metadata

```rust
m.insert("bods.export", vec![
    "export bods", "bods export", "export ownership data",
    "export to bods", "generate bods", "bods package",
    "beneficial ownership export", "export ubo data",
    "open ownership format", "export standard format",
]);

m.insert("bods.validate", vec![
    "validate bods", "bods validation", "check bods compliance",
    "verify bods format", "bods schema check",
]);

m.insert("bods.import", vec![
    "import bods", "bods import", "load bods file",
    "import ownership data", "load beneficial ownership",
    "import from bods", "ingest bods",
]);
```

---

## Benefits of BODS Integration

1. **Industry Standard** - Open Ownership is widely recognized
2. **Regulatory Alignment** - BODS designed for BO disclosure requirements
3. **Interoperability** - Exchange data with other BODS-compliant systems
4. **Audit Trail** - Statement-based model with temporal versioning
5. **PEP Support** - politicalExposure built into person records
6. **Flexible Interests** - Rich codelist for ownership/control types
7. **Indirect Chains** - isComponent + componentRecords for complex structures

---

## Files to Create

| File | Purpose |
|------|---------|
| `rust/crates/ob-poc-types/src/bods.rs` | BODS 0.4 type definitions |
| `rust/src/services/bods_exporter.rs` | Export CBU to BODS |
| `rust/src/services/bods_importer.rs` | Import BODS to CBU |
| `rust/src/services/bods_validator.rs` | Validate against schema |
| `rust/config/verbs/bods.yaml` | DSL verb definitions |
| `data/schemas/bods-0.4/` | Local copy of schemas |
