# Design: DSL Language Server Protocol (LSP) Implementation

**Created:** 2025-11-25  
**Status:** DESIGN SPECIFICATION  
**Priority:** P2 — Developer Experience  
**Scope:** LSP server for IDE integration (Zed, VS Code)  

---

## Executive Summary

Build an LSP server for the s-expression DSL that provides:
- Syntax highlighting and error detection
- **Smart completions with human-readable picklists** (Option A)
- Go-to-definition for `@symbol` references
- Hover documentation for words
- Signature help while typing

**Option A Decision:** Display human-readable names, insert codes, runtime resolves to UUIDs.

```
User sees:  "Certificate of Incorporation"
DSL gets:   "CERT_OF_INCORP"
Runtime:    Looks up UUID from document_types table
```

---

## Part 1: Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           IDE (Zed / VS Code)                               │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ LSP Protocol (JSON-RPC over stdio)
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          dsl-language-server                                │
│                          (Rust binary)                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │   Parser     │  │   Analyzer   │  │  Vocabulary  │  │ Schema Cache │    │
│  │  (Layer 1)   │  │  (Layer 2-3) │  │  (Layer 2)   │  │  (Layer 4)   │    │
│  │              │  │              │  │              │  │              │    │
│  │ • Tokenize   │  │ • Symbol     │  │ • Word       │  │ • Doc types  │    │
│  │ • Parse      │  │   table      │  │   registry   │  │ • Attributes │    │
│  │ • AST        │  │ • Type check │  │ • Signatures │  │ • Roles      │    │
│  │ • Errors     │  │ • References │  │ • Param hints│  │ • Entities   │    │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘    │
│         │                 │                 │                 │             │
│         └─────────────────┴─────────────────┴─────────────────┘             │
│                                     │                                       │
│                                     ▼                                       │
│                    ┌─────────────────────────────┐                          │
│                    │    LSP Response Builder     │                          │
│                    │  • Completions              │                          │
│                    │  • Diagnostics              │                          │
│                    │  • Hover                    │                          │
│                    │  • Go-to-definition         │                          │
│                    └─────────────────────────────┘                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ (Optional, for Layer 4)
                                     ▼
                              ┌─────────────┐
                              │  PostgreSQL │
                              │  (metadata) │
                              └─────────────┘
```

---

## Part 2: Binding Layers

| Layer | Name | When | Data Source | LSP Features |
|-------|------|------|-------------|--------------|
| 1 | Lexical | On keystroke | Parser | Syntax errors, bracket matching |
| 2 | Vocabulary | On keystroke | In-memory registry | Word completions, signature help, hover |
| 3 | Session Symbols | On save/request | AST analysis | Go-to-definition, find references, `@symbol` completions |
| 4 | Reference Resolution | On request | Schema cache (from DB) | **Picklist completions for document types, attributes, roles** |
| 5 | Runtime | Execution only | Live DB | (Not in LSP — actual execution) |

---

## Part 3: Semantic Parameter Types

### 3.1 The Problem

Current signature:
```rust
signature: ":document-type STRING"  // Too weak — what STRING values are valid?
```

LSP can't provide meaningful completions without knowing the **semantic type**.

### 3.2 Solution: Semantic Type Annotations

**File:** `rust/src/dsl_runtime/vocabulary.rs`

```rust
/// Semantic types for parameter values
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticType {
    // Primitives
    String,
    Uuid,
    Integer,
    Decimal,
    Date,
    Boolean,
    
    // Reference types — trigger picklist from schema cache
    DocumentTypeRef,      // → document_types table
    AttributeRef,         // → attribute_dictionary table
    RoleRef,              // → roles table
    EntityTypeRef,        // → entity_types table
    JurisdictionRef,      // → jurisdictions table
    ScreeningListRef,     // → screening_lists table
    
    // Enum types — fixed set of values
    Enum(Vec<&'static str>),
    
    // Symbol reference — from session symbol table
    SymbolRef,
}

/// Parameter definition with semantic type
#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: &'static str,           // ":document-type"
    pub semantic_type: SemanticType,  // DocumentTypeRef
    pub required: bool,
    pub description: &'static str,
}

/// Word entry with typed parameters
pub struct WordEntry {
    pub name: &'static str,
    pub domain: &'static str,
    pub func: WordFn,
    pub params: &'static [ParamDef],
    pub description: &'static str,
    pub examples: &'static [&'static str],
}
```

### 3.3 Example Word Definition

```rust
WordEntry {
    name: "document.request",
    domain: "document",
    func: words::document_request,
    params: &[
        ParamDef {
            name: ":investigation-id",
            semantic_type: SemanticType::SymbolRef,
            required: true,
            description: "Investigation this request belongs to",
        },
        ParamDef {
            name: ":entity-id",
            semantic_type: SemanticType::SymbolRef,
            required: true,
            description: "Entity to request document from",
        },
        ParamDef {
            name: ":document-type",
            semantic_type: SemanticType::DocumentTypeRef,  // ← TRIGGERS PICKLIST
            required: true,
            description: "Type of document to request",
        },
        ParamDef {
            name: ":source",
            semantic_type: SemanticType::Enum(&["REGISTRY", "CLIENT", "THIRD_PARTY"]),
            required: false,
            description: "Source to request from",
        },
        ParamDef {
            name: ":priority",
            semantic_type: SemanticType::Enum(&["LOW", "NORMAL", "HIGH", "URGENT"]),
            required: false,
            description: "Request priority",
        },
    ],
    description: "Request a document for KYC investigation",
    examples: &[
        r#"(document.request :investigation-id @inv :entity-id @company :document-type "CERT_OF_INCORP")"#,
    ],
}
```

---

## Part 4: Schema Cache

### 4.1 Structure

**File:** `rust/src/lsp/schema_cache.rs`

```rust
use std::collections::HashMap;

/// Entry in a lookup table for LSP completions
#[derive(Debug, Clone)]
pub struct LookupEntry {
    /// Human-readable name shown in picklist
    pub display_name: String,
    
    /// Code inserted into DSL (NOT UUID)
    pub insert_value: String,
    
    /// Optional description for hover/detail
    pub description: Option<String>,
    
    /// Category for grouping in completion list
    pub category: Option<String>,
    
    /// Filter tags for smart filtering
    pub tags: Vec<String>,
}

/// Cached schema metadata for LSP completions
pub struct SchemaCache {
    /// Document types: display_name → LookupEntry
    pub document_types: Vec<LookupEntry>,
    
    /// Attributes: grouped by domain
    pub attributes: Vec<LookupEntry>,
    
    /// Roles: all valid role names
    pub roles: Vec<LookupEntry>,
    
    /// Entity types
    pub entity_types: Vec<LookupEntry>,
    
    /// Jurisdictions (ISO codes)
    pub jurisdictions: Vec<LookupEntry>,
    
    /// Screening lists
    pub screening_lists: Vec<LookupEntry>,
    
    /// Last refresh timestamp
    pub last_refresh: std::time::Instant,
}

impl SchemaCache {
    /// Load from database
    pub async fn load(pool: &PgPool) -> Result<Self> {
        let document_types = Self::load_document_types(pool).await?;
        let attributes = Self::load_attributes(pool).await?;
        let roles = Self::load_roles(pool).await?;
        let entity_types = Self::load_entity_types(pool).await?;
        let jurisdictions = Self::load_jurisdictions(pool).await?;
        let screening_lists = Self::load_screening_lists(pool).await?;
        
        Ok(Self {
            document_types,
            attributes,
            roles,
            entity_types,
            jurisdictions,
            screening_lists,
            last_refresh: std::time::Instant::now(),
        })
    }
    
    async fn load_document_types(pool: &PgPool) -> Result<Vec<LookupEntry>> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
            r#"
            SELECT type_code, type_name, description, category
            FROM "ob-poc".document_types
            WHERE is_active = true
            ORDER BY category, type_name
            "#
        )
        .fetch_all(pool)
        .await?;
        
        Ok(rows.into_iter().map(|(code, name, desc, cat)| {
            LookupEntry {
                display_name: name,
                insert_value: code,
                description: desc,
                category: cat,
                tags: vec![],
            }
        }).collect())
    }
    
    async fn load_attributes(pool: &PgPool) -> Result<Vec<LookupEntry>> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
            r#"
            SELECT attr_id, attr_name, description, domain
            FROM "ob-poc".attribute_dictionary
            WHERE is_active = true
            ORDER BY domain, attr_name
            "#
        )
        .fetch_all(pool)
        .await?;
        
        Ok(rows.into_iter().map(|(id, name, desc, domain)| {
            LookupEntry {
                display_name: format!("{} ({})", name, id),
                insert_value: id,
                description: desc,
                category: domain,
                tags: vec![],
            }
        }).collect())
    }
    
    async fn load_roles(pool: &PgPool) -> Result<Vec<LookupEntry>> {
        let rows = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT name, description
            FROM "ob-poc".roles
            ORDER BY name
            "#
        )
        .fetch_all(pool)
        .await?;
        
        Ok(rows.into_iter().map(|(name, desc)| {
            LookupEntry {
                display_name: name.clone(),
                insert_value: name,
                description: desc,
                category: None,
                tags: vec![],
            }
        }).collect())
    }
    
    // Similar for entity_types, jurisdictions, screening_lists...
    
    /// Get completions for a semantic type
    pub fn get_completions(&self, semantic_type: &SemanticType) -> Vec<&LookupEntry> {
        match semantic_type {
            SemanticType::DocumentTypeRef => self.document_types.iter().collect(),
            SemanticType::AttributeRef => self.attributes.iter().collect(),
            SemanticType::RoleRef => self.roles.iter().collect(),
            SemanticType::EntityTypeRef => self.entity_types.iter().collect(),
            SemanticType::JurisdictionRef => self.jurisdictions.iter().collect(),
            SemanticType::ScreeningListRef => self.screening_lists.iter().collect(),
            _ => vec![],
        }
    }
    
    /// Filter completions by prefix
    pub fn filter_completions<'a>(
        entries: &'a [LookupEntry],
        prefix: &str,
    ) -> Vec<&'a LookupEntry> {
        let prefix_lower = prefix.to_lowercase();
        entries
            .iter()
            .filter(|e| {
                e.display_name.to_lowercase().contains(&prefix_lower)
                    || e.insert_value.to_lowercase().contains(&prefix_lower)
            })
            .collect()
    }
}
```

### 4.2 Database Tables Required

```sql
-- Document types lookup
CREATE TABLE IF NOT EXISTS "ob-poc".document_types (
    document_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type_code VARCHAR(100) UNIQUE NOT NULL,     -- "CERT_OF_INCORP" ← inserted into DSL
    type_name VARCHAR(255) NOT NULL,            -- "Certificate of Incorporation" ← shown in picker
    category VARCHAR(100),                      -- "Corporate", "Identity", "Financial"
    description TEXT,
    required_for JSONB,                         -- ["LIMITED_COMPANY", "PARTNERSHIP"]
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed data
INSERT INTO "ob-poc".document_types (type_code, type_name, category, description) VALUES
('CERT_OF_INCORP', 'Certificate of Incorporation', 'Corporate', 'Official incorporation document'),
('ARTICLES_OF_ASSOC', 'Articles of Association', 'Corporate', 'Company constitution document'),
('SHARE_REGISTER', 'Share Register', 'Corporate', 'Record of shareholders'),
('ANNUAL_RETURN', 'Annual Return', 'Corporate', 'Yearly company filing'),
('PASSPORT', 'Passport', 'Identity', 'Government-issued travel document'),
('NATIONAL_ID', 'National ID Card', 'Identity', 'Government-issued ID'),
('DRIVING_LICENSE', 'Driving License', 'Identity', 'Driver identification'),
('PROOF_OF_ADDRESS', 'Proof of Address', 'Identity', 'Utility bill or bank statement'),
('BANK_STATEMENT', 'Bank Statement', 'Financial', 'Account statement'),
('TAX_RETURN', 'Tax Return', 'Financial', 'Annual tax filing'),
('AUDITED_ACCOUNTS', 'Audited Financial Statements', 'Financial', 'Audited annual accounts'),
('TRUST_DEED', 'Trust Deed', 'Trust', 'Trust formation document'),
('PARTNERSHIP_AGREEMENT', 'Partnership Agreement', 'Partnership', 'Partnership formation document')
ON CONFLICT (type_code) DO NOTHING;

-- Attribute dictionary lookup
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_dictionary (
    attr_id VARCHAR(100) PRIMARY KEY,           -- "CBU.LEGAL_NAME" ← inserted into DSL
    attr_name VARCHAR(255) NOT NULL,            -- "Legal Name" ← shown in picker
    domain VARCHAR(50),                         -- "CBU", "PERSON", "COMPANY"
    data_type VARCHAR(50),                      -- "STRING", "DATE", "DECIMAL", "BOOLEAN"
    description TEXT,
    validation_pattern VARCHAR(255),
    is_required BOOLEAN DEFAULT false,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed data
INSERT INTO "ob-poc".attribute_dictionary (attr_id, attr_name, domain, data_type, description) VALUES
('CBU.LEGAL_NAME', 'Legal Name', 'CBU', 'STRING', 'Official registered name'),
('CBU.REGISTRATION_NUMBER', 'Registration Number', 'CBU', 'STRING', 'Company/fund registration'),
('CBU.INCORPORATION_DATE', 'Incorporation Date', 'CBU', 'DATE', 'Date of formation'),
('CBU.JURISDICTION', 'Jurisdiction', 'CBU', 'STRING', 'Country of registration'),
('CBU.NATURE_PURPOSE', 'Nature and Purpose', 'CBU', 'STRING', 'Business description'),
('PERSON.FULL_NAME', 'Full Name', 'PERSON', 'STRING', 'Complete legal name'),
('PERSON.DATE_OF_BIRTH', 'Date of Birth', 'PERSON', 'DATE', 'Birth date'),
('PERSON.NATIONALITY', 'Nationality', 'PERSON', 'STRING', 'Country of citizenship'),
('PERSON.TAX_ID', 'Tax ID', 'PERSON', 'STRING', 'Tax identification number'),
('PERSON.RESIDENTIAL_ADDRESS', 'Residential Address', 'PERSON', 'STRING', 'Home address'),
('COMPANY.COMPANY_NUMBER', 'Company Number', 'COMPANY', 'STRING', 'Official registration number'),
('COMPANY.REGISTERED_OFFICE', 'Registered Office', 'COMPANY', 'STRING', 'Official address'),
('COMPANY.SHARE_CAPITAL', 'Share Capital', 'COMPANY', 'DECIMAL', 'Authorized share capital')
ON CONFLICT (attr_id) DO NOTHING;
```

---

## Part 5: LSP Completion Flow

### 5.1 Trigger Detection

```rust
/// Detect what kind of completion to provide based on cursor position
pub fn detect_completion_context(
    document: &str,
    position: Position,
) -> CompletionContext {
    let line = get_line(document, position.line);
    let col = position.character as usize;
    let prefix = &line[..col];
    
    // Find enclosing s-expression
    let (word_name, current_param) = parse_context(prefix);
    
    match (word_name.as_deref(), current_param.as_deref()) {
        // At start of expression — complete word names
        (None, None) if prefix.trim_end().ends_with('(') => {
            CompletionContext::WordName { prefix: String::new() }
        }
        
        // After word, before any param — complete word names
        (None, None) => {
            let word_prefix = extract_word_prefix(prefix);
            CompletionContext::WordName { prefix: word_prefix }
        }
        
        // After a keyword — complete its value
        (Some(word), Some(param)) => {
            let param_def = lookup_param_def(word, param);
            let value_prefix = extract_value_prefix(prefix);
            CompletionContext::ParamValue {
                word: word.to_string(),
                param: param.to_string(),
                param_def,
                prefix: value_prefix,
            }
        }
        
        // After word, typing keyword — complete keywords
        (Some(word), None) if prefix.ends_with(':') || prefix.ends_with(" :") => {
            CompletionContext::ParamKeyword {
                word: word.to_string(),
                prefix: String::new(),
            }
        }
        
        // Symbol reference
        _ if prefix.ends_with('@') => {
            CompletionContext::SymbolRef { prefix: String::new() }
        }
        
        _ => CompletionContext::None,
    }
}
```

### 5.2 Completion Generation

```rust
pub fn generate_completions(
    context: &CompletionContext,
    vocabulary: &Vocabulary,
    schema_cache: &SchemaCache,
    symbol_table: &SymbolTable,
) -> Vec<CompletionItem> {
    match context {
        CompletionContext::WordName { prefix } => {
            vocabulary
                .words
                .iter()
                .filter(|w| w.name.starts_with(prefix))
                .map(|w| CompletionItem {
                    label: w.name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format_signature(w)),
                    documentation: Some(Documentation::String(w.description.to_string())),
                    insert_text: Some(w.name.to_string()),
                    ..Default::default()
                })
                .collect()
        }
        
        CompletionContext::ParamKeyword { word, prefix } => {
            let word_def = vocabulary.get_word(word).unwrap();
            word_def
                .params
                .iter()
                .filter(|p| p.name.starts_with(&format!(":{}", prefix)))
                .map(|p| CompletionItem {
                    label: p.name.to_string(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(format!("{:?}", p.semantic_type)),
                    documentation: Some(Documentation::String(p.description.to_string())),
                    insert_text: Some(format!("{} ", p.name)),
                    ..Default::default()
                })
                .collect()
        }
        
        CompletionContext::ParamValue { word, param, param_def, prefix } => {
            match &param_def.semantic_type {
                // Reference types — picklist from schema cache
                SemanticType::DocumentTypeRef => {
                    generate_lookup_completions(
                        &schema_cache.document_types,
                        prefix,
                        "📄",
                    )
                }
                
                SemanticType::AttributeRef => {
                    generate_lookup_completions(
                        &schema_cache.attributes,
                        prefix,
                        "📋",
                    )
                }
                
                SemanticType::RoleRef => {
                    generate_lookup_completions(
                        &schema_cache.roles,
                        prefix,
                        "👤",
                    )
                }
                
                SemanticType::EntityTypeRef => {
                    generate_lookup_completions(
                        &schema_cache.entity_types,
                        prefix,
                        "🏢",
                    )
                }
                
                // Enum — fixed values
                SemanticType::Enum(values) => {
                    values
                        .iter()
                        .filter(|v| v.to_lowercase().contains(&prefix.to_lowercase()))
                        .map(|v| CompletionItem {
                            label: v.to_string(),
                            kind: Some(CompletionItemKind::ENUM_MEMBER),
                            insert_text: Some(format!("\"{}\"", v)),
                            ..Default::default()
                        })
                        .collect()
                }
                
                // Symbol reference — from session
                SemanticType::SymbolRef => {
                    symbol_table
                        .symbols
                        .iter()
                        .filter(|(name, _)| name.contains(prefix))
                        .map(|(name, sym)| CompletionItem {
                            label: format!("@{}", name),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(format!("{:?} from line {}", sym.sym_type, sym.line)),
                            insert_text: Some(format!("@{}", name)),
                            ..Default::default()
                        })
                        .collect()
                }
                
                _ => vec![],
            }
        }
        
        CompletionContext::SymbolRef { prefix } => {
            symbol_table
                .symbols
                .iter()
                .filter(|(name, _)| name.starts_with(prefix))
                .map(|(name, sym)| CompletionItem {
                    label: format!("@{}", name),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(format!("{:?}", sym.sym_type)),
                    insert_text: Some(name.to_string()),
                    ..Default::default()
                })
                .collect()
        }
        
        CompletionContext::None => vec![],
    }
}

/// Generate completions from lookup entries (Option A)
fn generate_lookup_completions(
    entries: &[LookupEntry],
    prefix: &str,
    icon: &str,
) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    
    entries
        .iter()
        .filter(|e| {
            e.display_name.to_lowercase().contains(&prefix_lower)
                || e.insert_value.to_lowercase().contains(&prefix_lower)
        })
        .map(|e| {
            let mut item = CompletionItem {
                // Show human-readable name
                label: format!("{} {}", icon, e.display_name),
                kind: Some(CompletionItemKind::VALUE),
                
                // Show code in detail
                detail: Some(e.insert_value.clone()),
                
                // Show description in documentation
                documentation: e.description.as_ref().map(|d| {
                    Documentation::String(d.clone())
                }),
                
                // INSERT THE CODE (Option A)
                insert_text: Some(format!("\"{}\"", e.insert_value)),
                
                // Sort by category then name
                sort_text: Some(format!(
                    "{}-{}",
                    e.category.as_deref().unwrap_or("zzz"),
                    e.display_name
                )),
                
                // Filter matches both display and code
                filter_text: Some(format!("{} {}", e.display_name, e.insert_value)),
                
                ..Default::default()
            };
            
            // Group by category
            if let Some(cat) = &e.category {
                item.label_details = Some(CompletionItemLabelDetails {
                    description: Some(cat.clone()),
                    ..Default::default()
                });
            }
            
            item
        })
        .collect()
}
```

---

## Part 6: IDE Experience

### 6.1 Document Type Completion

User types:
```clojure
(document.request :document-type |
```

IDE shows:
```
┌─────────────────────────────────────────────────────────────────┐
│ 📄 Certificate of Incorporation          CERT_OF_INCORP        │
│    Corporate · Official incorporation document                  │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Articles of Association               ARTICLES_OF_ASSOC     │
│    Corporate · Company constitution document                    │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Share Register                        SHARE_REGISTER        │
│    Corporate · Record of shareholders                           │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Passport                              PASSPORT              │
│    Identity · Government-issued travel document                 │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Bank Statement                        BANK_STATEMENT        │
│    Financial · Account statement                                │
└─────────────────────────────────────────────────────────────────┘
```

User selects "Certificate of Incorporation", DSL becomes:
```clojure
(document.request :document-type "CERT_OF_INCORP"
```

### 6.2 Attribute Completion

User types:
```clojure
(document.extract-attributes :attributes [{:attr-id |
```

IDE shows:
```
┌─────────────────────────────────────────────────────────────────┐
│ 📋 Legal Name (CBU.LEGAL_NAME)                                  │
│    CBU · Official registered name                               │
├─────────────────────────────────────────────────────────────────┤
│ 📋 Registration Number (CBU.REGISTRATION_NUMBER)                │
│    CBU · Company/fund registration                              │
├─────────────────────────────────────────────────────────────────┤
│ 📋 Full Name (PERSON.FULL_NAME)                                 │
│    PERSON · Complete legal name                                 │
├─────────────────────────────────────────────────────────────────┤
│ 📋 Date of Birth (PERSON.DATE_OF_BIRTH)                         │
│    PERSON · Birth date                                          │
└─────────────────────────────────────────────────────────────────┘
```

User selects "Legal Name", DSL becomes:
```clojure
(document.extract-attributes :attributes [{:attr-id "CBU.LEGAL_NAME"
```

### 6.3 Role Completion

User types:
```clojure
(cbu.attach-entity :entity-id @company :role |
```

IDE shows:
```
┌─────────────────────────────────────────────────────────────────┐
│ 👤 AssetOwner                                                   │
│    Legal owner of assets                                        │
├─────────────────────────────────────────────────────────────────┤
│ 👤 InvestmentManager                                            │
│    Manages investment decisions                                 │
├─────────────────────────────────────────────────────────────────┤
│ 👤 ManagementCompany                                            │
│    UCITS/AIFM management company                                │
├─────────────────────────────────────────────────────────────────┤
│ 👤 BeneficialOwner                                              │
│    Ultimate beneficial owner (>10% or >25%)                     │
├─────────────────────────────────────────────────────────────────┤
│ 👤 Custodian                                                    │
│    Holds assets in custody                                      │
└─────────────────────────────────────────────────────────────────┘
```

### 6.4 Symbol Completion

User types:
```clojure
(cbu.ensure :cbu-name "Test Fund" :as @fund)
(entity.create-limited-company :name "TestCo" :as @testco)

(cbu.attach-entity :cbu-id @|
```

IDE shows:
```
┌─────────────────────────────────────────────────────────────────┐
│ @fund                        UUID from cbu.ensure (line 1)      │
├─────────────────────────────────────────────────────────────────┤
│ @testco                      UUID from entity.create (line 2)   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 7: Runtime Resolution

### 7.1 Code → UUID Lookup at Runtime

**File:** `rust/src/database/lookup_service.rs`

```rust
pub struct LookupService {
    pool: PgPool,
    // Cache for performance
    document_type_cache: HashMap<String, Uuid>,
    role_cache: HashMap<String, Uuid>,
}

impl LookupService {
    /// Resolve document type code to UUID
    pub async fn resolve_document_type(&self, type_code: &str) -> Result<Uuid> {
        // Check cache first
        if let Some(id) = self.document_type_cache.get(type_code) {
            return Ok(*id);
        }
        
        // Query DB
        let id: Uuid = sqlx::query_scalar(
            r#"
            SELECT document_type_id 
            FROM "ob-poc".document_types 
            WHERE type_code = $1 AND is_active = true
            "#
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Unknown document type: {}", type_code))?;
        
        Ok(id)
    }
    
    /// Resolve role name to UUID
    pub async fn resolve_role(&self, role_name: &str) -> Result<Uuid> {
        if let Some(id) = self.role_cache.get(role_name) {
            return Ok(*id);
        }
        
        let id: Uuid = sqlx::query_scalar(
            r#"SELECT role_id FROM "ob-poc".roles WHERE name = $1"#
        )
        .bind(role_name)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Unknown role: {}", role_name))?;
        
        Ok(id)
    }
    
    /// Resolve entity type code to UUID
    pub async fn resolve_entity_type(&self, type_code: &str) -> Result<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Unknown entity type: {}", type_code))?;
        
        Ok(id)
    }
}
```

### 7.2 CrudExecutor Uses LookupService

```rust
// In CrudExecutor
"DOCUMENT_REQUEST" => {
    let doc_type_code = self.get_string_value(&values, "document-type")?;
    
    // Resolve code to UUID at runtime
    let doc_type_id = self.lookup_service
        .resolve_document_type(&doc_type_code)
        .await?;
    
    // Use UUID in INSERT
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_requests (document_type_id, ...)
        VALUES ($1, ...)
        "#
    )
    .bind(doc_type_id)
    // ...
}
```

---

## Part 8: LSP Server Structure

### 8.1 Crate Structure

```
rust/
├── Cargo.toml
├── src/
│   └── ...                    # Main library
└── crates/
    └── dsl-lsp/
        ├── Cargo.toml
        ├── src/
        │   ├── main.rs        # LSP binary entry point
        │   ├── server.rs      # LSP server implementation
        │   ├── handlers/
        │   │   ├── mod.rs
        │   │   ├── completion.rs
        │   │   ├── hover.rs
        │   │   ├── goto_definition.rs
        │   │   ├── diagnostics.rs
        │   │   └── signature_help.rs
        │   ├── analysis/
        │   │   ├── mod.rs
        │   │   ├── parser.rs
        │   │   ├── symbol_table.rs
        │   │   └── type_check.rs
        │   └── schema_cache.rs
        └── tests/
```

### 8.2 Dependencies

```toml
# crates/dsl-lsp/Cargo.toml
[package]
name = "dsl-lsp"
version = "0.1.0"
edition = "2021"

[dependencies]
# LSP protocol
tower-lsp = "0.20"
lsp-types = "0.94"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Database (optional, for schema cache)
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres"], optional = true }

# Main library (for parser, vocabulary)
ob-poc = { path = "../.." }

# Utilities
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"

[features]
default = []
database = ["sqlx"]
```

### 8.3 Main Entry Point

```rust
// crates/dsl-lsp/src/main.rs
use tower_lsp::{LspService, Server};
use crate::server::DslLanguageServer;

#[tokio::main]
async fn main() {
    // Setup logging
    tracing_subscriber::fmt::init();
    
    // Create LSP service
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    
    let (service, socket) = LspService::new(|client| {
        DslLanguageServer::new(client)
    });
    
    // Run server
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

---

## Part 9: Zed Integration

### 9.1 Extension Configuration

```json
// zed-extension/extension.json
{
  "id": "dsl-onboard",
  "name": "Onboarding DSL",
  "version": "0.1.0",
  "description": "Language support for the onboarding DSL",
  "languages": ["dsl"],
  "language_servers": {
    "dsl-lsp": {
      "command": {
        "path": "dsl-lsp",
        "arguments": []
      },
      "languages": ["dsl"]
    }
  }
}
```

### 9.2 Language Definition

```json
// zed-extension/languages/dsl/config.toml
name = "DSL"
path_suffixes = ["dsl", "obl"]
line_comments = [";"]
```

### 9.3 Tree-sitter Grammar

```javascript
// tree-sitter-dsl/grammar.js
module.exports = grammar({
  name: 'dsl',
  
  rules: {
    source_file: $ => repeat($._expression),
    
    _expression: $ => choice(
      $.list,
      $.symbol,
      $.keyword,
      $.string,
      $.number,
      $.symbol_ref,
      $.comment,
    ),
    
    list: $ => seq(
      '(',
      optional($.symbol),  // word name
      repeat($._expression),
      ')'
    ),
    
    symbol: $ => /[a-zA-Z_][a-zA-Z0-9_\-\.]*/,
    
    keyword: $ => seq(':', /[a-zA-Z_][a-zA-Z0-9_\-]*/),
    
    string: $ => /"[^"]*"/,
    
    number: $ => /\-?[0-9]+(\.[0-9]+)?/,
    
    symbol_ref: $ => seq('@', /[a-zA-Z_][a-zA-Z0-9_\-]*/),
    
    comment: $ => /;.*/,
  }
});
```

---

## Part 10: Implementation Phases

### Phase 1: Core LSP (No DB)
- [ ] Parser integration
- [ ] Vocabulary completions (words, keywords)
- [ ] Symbol table for `@` references
- [ ] Hover documentation
- [ ] Signature help
- [ ] Basic diagnostics (syntax errors)

### Phase 2: Schema Cache (With DB)
- [ ] Schema cache loading
- [ ] Document type completions
- [ ] Attribute completions
- [ ] Role completions
- [ ] Entity type completions
- [ ] Cache refresh mechanism

### Phase 3: IDE Integration
- [ ] Zed extension packaging
- [ ] VS Code extension packaging
- [ ] Tree-sitter grammar
- [ ] Syntax highlighting

### Phase 4: Advanced Features
- [ ] Find all references
- [ ] Rename symbol
- [ ] Code actions (quick fixes)
- [ ] Workspace-wide analysis
- [ ] Incremental parsing

---

## Summary

| Component | Description |
|-----------|-------------|
| **Semantic Types** | `DocumentTypeRef`, `AttributeRef`, `RoleRef` in param definitions |
| **Schema Cache** | Loads lookup tables from DB, caches in memory |
| **Completion Flow** | Detect context → Get semantic type → Query cache → Build picklist |
| **Option A Pattern** | Display human name, insert code, runtime resolves to UUID |
| **LSP Protocol** | Standard tower-lsp implementation |
| **IDE Support** | Zed extension with tree-sitter grammar |

This delivers a full IDE experience where users see friendly names but the DSL remains portable with code identifiers.
