# Phase 3 Agent Intelligence - Lexicon/Grammar Architecture

**ARCHITECTURAL PIVOT: Regex → Formal Grammar**

**Problem:** Current regex pattern matching achieves 49% accuracy, 0% on OTC domain. Fundamentally wrong approach for NLU.

**Solution:** Lexicon-backed tokenizer + Nom grammar parser. Same technology as DSL parser, applied to natural language.

**Estimated Effort:** 5-6 weeks

---

## Table of Contents

1. [Why This Pivot](#why-this-pivot)
2. [Architecture Overview](#architecture-overview)
3. [Phase 3.1: Lexicon Infrastructure](#phase-31-lexicon-infrastructure)
4. [Phase 3.2: Tokenizer Implementation](#phase-32-tokenizer-implementation)
5. [Phase 3.3: Intent Grammar (Nom)](#phase-33-intent-grammar-nom)
6. [Phase 3.4: Pipeline Integration](#phase-34-pipeline-integration)
7. [Phase 3.5: OTC Derivatives Domain](#phase-35-otc-derivatives-domain)
8. [Phase 3.6: Integration Checkpoint](#phase-36-integration-checkpoint)
9. [Verification & Demo](#verification--demo)

---

## Why This Pivot

### Current Failure (Regex)

```
User: "Add Goldman Sachs as a counterparty"
Pattern: "add {counterparty} as counterparty"
Result: NO MATCH (article "a" breaks regex)
Accuracy: 49% overall, 0% OTC
```

### Problems with Regex for NLU

| Issue | Example |
|-------|---------|
| Combinatorial explosion | Every variation needs a pattern |
| Order sensitivity | "Add X as Y" vs "As Y, add X" |
| Slot boundary bleeding | Greedy capture conflicts |
| No semantic understanding | "Goldman" ≠ "Goldman Sachs" as strings |
| Maintenance nightmare | Synonyms = more regex |
| Silent failures | No match = no feedback |

### Solution: Tokenize First, Parse Second

```
User: "Add Goldman Sachs as a counterparty"
           │
           ▼
    ┌──────────────┐
    │  TOKENIZER   │  ← Lexicon-backed classification
    └──────────────┘
           │
           ▼
    [VERB:add] [ENTITY:Goldman Sachs] [PREP:as] [ART:a] [ROLE:counterparty]
           │
           ▼
    ┌──────────────┐
    │  NOM PARSER  │  ← Formal grammar over tokens
    └──────────────┘
           │
           ▼
    IntentAst::CounterpartyCreate { entity: "Goldman Sachs" }
```

**This is how real NLU systems work.**

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     LEXICON-BASED AGENT PIPELINE                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Natural Language Input                                                      │
│  "Add Goldman Sachs as counterparty for IRS"                                │
│                           │                                                  │
│                           ▼                                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  LAYER 1: TOKENIZER                                                    │  │
│  │                                                                        │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐     │  │
│  │  │  Static     │ │  DB Tables  │ │  Entity     │ │  Session    │     │  │
│  │  │  Lexicon    │ │  (cached)   │ │  Gateway    │ │  Context    │     │  │
│  │  │  (YAML)     │ │             │ │  (fuzzy)    │ │  (pronouns) │     │  │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘     │  │
│  │                                                                        │  │
│  │  Output: Typed Token Stream                                           │  │
│  │  [VERB:add] [ENTITY:Goldman Sachs] [PREP:as] [ROLE:counterparty]     │  │
│  │  [PREP:for] [PRODUCT:IRS]                                            │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                           │                                                  │
│                           ▼                                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  LAYER 2: NOM INTENT PARSER                                           │  │
│  │                                                                        │  │
│  │  Grammar (same technology as DSL parser):                             │  │
│  │    intent       = verb_phrase entity_phrase? role_phrase? scope?      │  │
│  │    verb_phrase  = VERB                                                │  │
│  │    entity_phrase = ENTITY                                             │  │
│  │    role_phrase  = PREP? ROLE                                          │  │
│  │    scope        = PREP (PRODUCT | INSTRUMENT | MARKET)+               │  │
│  │                                                                        │  │
│  │  Output: IntentAst                                                    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                           │                                                  │
│                           ▼                                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  DSL GENERATOR                                                         │  │
│  │  IntentAst → DSL (deterministic, no LLM)                              │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                           │                                                  │
│                           ▼                                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  EXISTING DSL PIPELINE                                                 │  │
│  │  Parse → Compile → Execute → Database                                 │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Two Grammars, Same Technology

| Layer | Grammar | Input | Output |
|-------|---------|-------|--------|
| **NLU Parser** | Intent grammar | Token stream | `IntentAst` |
| **DSL Parser** | DSL grammar | Character stream | `DslAst` |

Same nom parser combinators, different grammars.

---

## Phase 3.1: Lexicon Infrastructure

**Goal:** Create lexicon configuration and data structures.

**Duration:** 3-4 days

### Task 3.1.1: Token Types

**File:** `rust/src/agentic/lexicon/tokens.rs` (new)

```rust
//! Token types for lexicon-based NLU

use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Token {
    /// Token classification
    pub token_type: TokenType,
    /// Original text from input
    pub value: String,
    /// Normalized/canonical form
    pub canonical: String,
    /// Resolved entity ID (for ENTITY tokens)
    pub resolved_id: Option<Uuid>,
    /// Position in input (start, end)
    pub span: (usize, usize),
    /// Match confidence 0.0-1.0
    pub confidence: f32,
    /// How this token was resolved
    pub source: TokenSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // === Actions ===
    Verb(VerbClass),
    
    // === Domain Entities (resolved against sources) ===
    Entity(EntityClass),
    Product,              // IRS, CDS, SWAPTION (OTC derivatives)
    Instrument,           // EQUITY, GOVT_BOND (exchange-traded)
    Market,               // XNYS, XLON (MIC codes)
    Currency,             // USD, EUR, GBP
    
    // === Role/Relationship Markers ===
    Role,                 // counterparty, custodian, investment_manager
    
    // === OTC-Specific ===
    CsaType,              // VM, IM
    Law,                  // NY, ENGLISH
    IsdaVersion,          // 2002, 1992
    ConfirmationMethod,   // MARKITWIRE, DTCC_GTR
    
    // === Structural ===
    Prep,                 // for, with, as, to, under, via
    Conj,                 // and, or
    Article,              // a, the, an (usually absorbed)
    
    // === Modifiers ===
    Modifier(ModifierType),
    
    // === Literals ===
    Number(NumberType),   // 100000, 2%, T+1
    
    // === Coreference ===
    Pronoun,              // them, it, that, their (resolved by tokenizer)
    
    // === Unknown (triggers recovery) ===
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerbClass {
    Create,   // add, establish, create, onboard, set up
    Update,   // set, configure, update, change, modify
    Delete,   // remove, delete, cancel, terminate
    Query,    // show, list, find, who, what, where
    Link,     // connect, link, assign, use, via
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityClass {
    Counterparty,
    InvestmentManager,
    Custodian,
    Fund,
    Cbu,
    PricingSource,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModifierType {
    Negation,    // don't, not, never
    All,         // all, every, each
    Some,        // some, any
}

#[derive(Debug, Clone, PartialEq)]
pub enum NumberType {
    Integer,
    Percentage,
    Currency,
    Duration,    // T+1, T+2
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenSource {
    StaticLexicon,           // From YAML
    DatabaseLookup(String),  // From DB table
    EntityGateway,           // Fuzzy matched
    SessionContext,          // From session symbols
    Coreference(String),     // Resolved pronoun ("them" → entity)
    Inferred,                // Default/inferred value
}

impl Token {
    pub fn verb(value: &str, class: VerbClass) -> Self {
        Self {
            token_type: TokenType::Verb(class),
            value: value.to_string(),
            canonical: value.to_lowercase(),
            resolved_id: None,
            span: (0, 0),
            confidence: 1.0,
            source: TokenSource::StaticLexicon,
        }
    }
    
    pub fn entity(value: &str, class: EntityClass, id: Option<Uuid>) -> Self {
        Self {
            token_type: TokenType::Entity(class),
            value: value.to_string(),
            canonical: value.to_string(),
            resolved_id: id,
            span: (0, 0),
            confidence: if id.is_some() { 1.0 } else { 0.8 },
            source: if id.is_some() { TokenSource::EntityGateway } else { TokenSource::Inferred },
        }
    }
    
    pub fn unknown(value: &str, span: (usize, usize)) -> Self {
        Self {
            token_type: TokenType::Unknown,
            value: value.to_string(),
            canonical: value.to_string(),
            resolved_id: None,
            span,
            confidence: 0.0,
            source: TokenSource::Inferred,
        }
    }
}
```

### Task 3.1.2: Lexicon Configuration

**File:** `rust/config/agent/lexicon.yaml` (new)

```yaml
# Lexicon Configuration for Agent NLU
# 
# This file defines the vocabulary for tokenization.
# Tokens are matched longest-first against these definitions.

version: "1.0"

# =============================================================================
# VERBS - Action words that indicate intent
# =============================================================================
verbs:
  # Create/Add actions
  add:
    canonical: add
    class: Create
    aliases: [create, onboard, register]
  establish:
    canonical: establish
    class: Create
    aliases: [set up, setup]
  
  # Update/Configure actions
  set:
    canonical: set
    class: Update
    aliases: [configure, update, change, modify]
  
  # Delete/Remove actions
  remove:
    canonical: remove
    class: Delete
    aliases: [delete, cancel, terminate, drop]
  
  # Query actions
  show:
    canonical: show
    class: Query
    aliases: [list, find, display, get]
  who:
    canonical: who
    class: Query
  what:
    canonical: what
    class: Query
  
  # Link/Assign actions
  use:
    canonical: use
    class: Link
    aliases: [connect, link, assign, via]

# =============================================================================
# ROLES - Relationship types
# =============================================================================
roles:
  counterparty:
    canonical: counterparty
    domain: otc
    aliases:
      - swap counterparty
      - trading counterparty
      - derivatives counterparty
  
  investment_manager:
    canonical: investment_manager
    domain: trading_matrix
    aliases:
      - im
      - investment manager
      - asset manager
      - manager
  
  custodian:
    canonical: custodian
    domain: exchange
    aliases:
      - custody provider
      - sub-custodian
  
  pricing_source:
    canonical: pricing_source
    domain: trading_matrix
    aliases:
      - price source
      - pricing provider
      - data vendor

# =============================================================================
# PRODUCTS - OTC derivative products
# =============================================================================
products:
  source: static  # Could be: static, db.table_name
  values:
    IRS:
      aliases:
        - interest rate swap
        - interest rate swaps
        - rate swap
        - rate swaps
        - swaps
    XCCY:
      aliases:
        - cross currency
        - cross currency swap
        - xccy swap
        - cross-currency
    CDS:
      aliases:
        - credit default swap
        - credit default swaps
        - credit swap
        - credit swaps
    FX_FORWARD:
      aliases:
        - fx forward
        - fx forwards
        - currency forward
        - forward
    FX_OPTION:
      aliases:
        - fx option
        - fx options
        - currency option
    SWAPTION:
      aliases:
        - swaption
        - swaptions
        - swap option
    REPO:
      aliases:
        - repo
        - repurchase agreement
        - repurchase
  
  # Category expansions
  categories:
    rates:
      expands_to: [IRS, XCCY, SWAPTION]
    credit:
      expands_to: [CDS]
    fx:
      expands_to: [FX_FORWARD, FX_OPTION]
    all_otc:
      expands_to: [IRS, XCCY, CDS, FX_FORWARD, FX_OPTION, SWAPTION]

# =============================================================================
# INSTRUMENTS - Exchange-traded instrument classes
# =============================================================================
instruments:
  source: static
  values:
    EQUITY:
      aliases:
        - equities
        - stocks
        - shares
    GOVT_BOND:
      aliases:
        - government bonds
        - govvies
        - treasuries
        - gilts
        - bunds
        - sovereign bonds
    CORP_BOND:
      aliases:
        - corporate bonds
        - corporates
        - credit bonds
    AGENCY:
      aliases:
        - agency bonds
        - agencies
    ETF:
      aliases:
        - etf
        - exchange traded fund
        - exchange-traded fund
    FUND:
      aliases:
        - mutual fund
        - funds
  
  categories:
    fixed_income:
      expands_to: [GOVT_BOND, CORP_BOND, AGENCY]
    all_equity:
      expands_to: [EQUITY, ETF]

# =============================================================================
# MARKETS - MIC codes and aliases
# =============================================================================
markets:
  source: db.markets  # Load from database
  column_mapping:
    code: mic
    display: market_name
  
  # Static aliases (DB lookup is primary)
  static_aliases:
    nyse: XNYS
    nasdaq: XNAS
    london: XLON
    lse: XLON
    frankfurt: XFRA
    xetra: XETR
    tokyo: XTKS
    hong kong: XHKG
    paris: XPAR
    euronext: XPAR
  
  # Region expansions
  regions:
    european:
      expands_to: [XLON, XETR, XPAR, XAMS, XBRU, XMIL, XMAD, XSWX]
    north_american:
      expands_to: [XNYS, XNAS, XTSE, XMEX]
    asian:
      expands_to: [XTKS, XHKG, XSES, XKRX, XBOM, XNSE]
    global:
      expands_to: [] # All markets

# =============================================================================
# CURRENCIES
# =============================================================================
currencies:
  source: db.currencies
  static_aliases:
    dollars: USD
    dollar: USD
    euros: EUR
    euro: EUR
    pounds: GBP
    sterling: GBP
    yen: JPY
    swiss francs: CHF
    francs: CHF

# =============================================================================
# CSA TYPES - Credit Support Annex types
# =============================================================================
csa_types:
  VM:
    aliases:
      - variation margin
      - vm csa
      - var margin
  IM:
    aliases:
      - initial margin
      - im csa
      - simm

# =============================================================================
# GOVERNING LAW
# =============================================================================
laws:
  NY:
    aliases:
      - new york
      - ny law
      - new york law
  ENGLISH:
    aliases:
      - english law
      - english
      - uk law
  GERMAN:
    aliases:
      - german law
      - german

# =============================================================================
# ISDA VERSIONS
# =============================================================================
isda_versions:
  "2002":
    aliases:
      - 2002 isda
      - isda 2002
      - current
      - standard
  "1992":
    aliases:
      - 1992 isda
      - isda 1992
      - old

# =============================================================================
# CONFIRMATION METHODS
# =============================================================================
confirmation_methods:
  MARKITWIRE:
    aliases:
      - markitwire
      - markit
      - markit wire
  DTCC_GTR:
    aliases:
      - dtcc
      - gtr
      - trade repository
  SWIFT_MT360:
    aliases:
      - swift
      - mt360

# =============================================================================
# INSTRUCTION METHODS
# =============================================================================
instruction_methods:
  CTM:
    aliases:
      - ctm
      - omgeo
      - central trade manager
  SWIFT:
    aliases:
      - swift
      - mt messages
  FIX:
    aliases:
      - fix
      - fix protocol
  ALERT:
    aliases:
      - alert
      - omgeo alert

# =============================================================================
# PREPOSITIONS
# =============================================================================
prepositions:
  - for
  - with
  - as
  - to
  - under
  - via
  - through
  - using
  - in
  - at
  - on
  - by

# =============================================================================
# CONJUNCTIONS
# =============================================================================
conjunctions:
  - and
  - or
  - also
  - plus

# =============================================================================
# ARTICLES (absorbed during tokenization)
# =============================================================================
articles:
  - a
  - an
  - the

# =============================================================================
# PRONOUNS (resolved from session context)
# =============================================================================
pronouns:
  they:
    type: subject
    number: plural
  them:
    type: object
    number: plural
  their:
    type: possessive
    number: plural
  it:
    type: subject
    number: singular
  its:
    type: possessive
    number: singular
  that:
    type: demonstrative
    number: singular
  those:
    type: demonstrative
    number: plural
  this:
    type: demonstrative
    number: singular

# =============================================================================
# NEGATION
# =============================================================================
negation:
  - don't
  - dont
  - do not
  - not
  - never
  - without

# =============================================================================
# ENTITIES - Dynamic resolution
# =============================================================================
entities:
  counterparties:
    source: entity_gateway
    entity_type: counterparty
    fuzzy_threshold: 0.8
    # Well-known aliases for faster lookup
    static_aliases:
      gs: Goldman Sachs
      goldman: Goldman Sachs
      jpm: JP Morgan
      jpmorgan: JP Morgan
      ms: Morgan Stanley
      db: Deutsche Bank
      deutsche: Deutsche Bank
      barclays: Barclays
      bnpp: BNP Paribas
      bnp: BNP Paribas
      citi: Citibank
      bofa: Bank of America
      ubs: UBS
      cs: Credit Suisse
      nomura: Nomura
      hsbc: HSBC
  
  investment_managers:
    source: entity_gateway
    entity_type: investment_manager
    fuzzy_threshold: 0.8
    static_aliases:
      blackrock: BlackRock
      blk: BlackRock
      pimco: PIMCO
      vanguard: Vanguard
      fidelity: Fidelity
      state street: State Street
      ssga: State Street
  
  pricing_sources:
    source: static
    values:
      - Bloomberg
      - Refinitiv
      - Reuters
      - Markit
      - ICE
      - Exchange
      - Internal
```

### Task 3.1.3: Lexicon Loader

**File:** `rust/src/agentic/lexicon/loader.rs` (new)

```rust
//! Lexicon loader - loads and indexes lexicon configuration

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LexiconConfig {
    pub version: String,
    pub verbs: HashMap<String, VerbEntry>,
    pub roles: HashMap<String, RoleEntry>,
    pub products: ProductsConfig,
    pub instruments: InstrumentsConfig,
    pub markets: MarketsConfig,
    pub currencies: CurrenciesConfig,
    pub csa_types: HashMap<String, AliasEntry>,
    pub laws: HashMap<String, AliasEntry>,
    pub isda_versions: HashMap<String, AliasEntry>,
    pub confirmation_methods: HashMap<String, AliasEntry>,
    pub instruction_methods: HashMap<String, AliasEntry>,
    pub prepositions: Vec<String>,
    pub conjunctions: Vec<String>,
    pub articles: Vec<String>,
    pub pronouns: HashMap<String, PronounEntry>,
    pub negation: Vec<String>,
    pub entities: EntitiesConfig,
}

#[derive(Debug, Deserialize)]
pub struct VerbEntry {
    pub canonical: String,
    pub class: String,  // Create, Update, Delete, Query, Link
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RoleEntry {
    pub canonical: String,
    pub domain: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AliasEntry {
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProductsConfig {
    pub source: String,
    pub values: HashMap<String, AliasEntry>,
    #[serde(default)]
    pub categories: HashMap<String, CategoryExpansion>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryExpansion {
    pub expands_to: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InstrumentsConfig {
    pub source: String,
    pub values: HashMap<String, AliasEntry>,
    #[serde(default)]
    pub categories: HashMap<String, CategoryExpansion>,
}

#[derive(Debug, Deserialize)]
pub struct MarketsConfig {
    pub source: String,
    #[serde(default)]
    pub column_mapping: Option<ColumnMapping>,
    #[serde(default)]
    pub static_aliases: HashMap<String, String>,
    #[serde(default)]
    pub regions: HashMap<String, CategoryExpansion>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnMapping {
    pub code: String,
    pub display: String,
}

#[derive(Debug, Deserialize)]
pub struct CurrenciesConfig {
    pub source: String,
    #[serde(default)]
    pub static_aliases: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct PronounEntry {
    #[serde(rename = "type")]
    pub pronoun_type: String,
    pub number: String,
}

#[derive(Debug, Deserialize)]
pub struct EntitiesConfig {
    pub counterparties: EntitySourceConfig,
    pub investment_managers: EntitySourceConfig,
    pub pricing_sources: EntitySourceConfig,
}

#[derive(Debug, Deserialize)]
pub struct EntitySourceConfig {
    pub source: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub fuzzy_threshold: Option<f32>,
    #[serde(default)]
    pub static_aliases: HashMap<String, String>,
    #[serde(default)]
    pub values: Vec<String>,
}

/// Indexed lexicon for fast lookup
pub struct Lexicon {
    config: LexiconConfig,
    
    // Inverted indexes: phrase -> (token_type, canonical)
    verb_index: HashMap<String, (String, String)>,      // phrase -> (class, canonical)
    role_index: HashMap<String, (String, String)>,      // phrase -> (domain, canonical)
    product_index: HashMap<String, String>,             // phrase -> canonical
    instrument_index: HashMap<String, String>,          // phrase -> canonical
    market_index: HashMap<String, String>,              // phrase -> MIC
    currency_index: HashMap<String, String>,            // phrase -> ISO code
    csa_type_index: HashMap<String, String>,            // phrase -> VM/IM
    law_index: HashMap<String, String>,                 // phrase -> canonical
    isda_version_index: HashMap<String, String>,        // phrase -> version
    confirmation_index: HashMap<String, String>,        // phrase -> method
    instruction_index: HashMap<String, String>,         // phrase -> method
    entity_aliases: HashMap<String, (String, String)>,  // phrase -> (entity_type, canonical)
    
    // Sets for O(1) lookup
    prepositions: std::collections::HashSet<String>,
    conjunctions: std::collections::HashSet<String>,
    articles: std::collections::HashSet<String>,
    negations: std::collections::HashSet<String>,
    pronouns: HashMap<String, PronounEntry>,
    
    // Category expansions
    product_categories: HashMap<String, Vec<String>>,
    instrument_categories: HashMap<String, Vec<String>>,
    market_regions: HashMap<String, Vec<String>>,
}

impl Lexicon {
    pub fn from_config(config: LexiconConfig) -> Self {
        let mut lexicon = Self {
            verb_index: HashMap::new(),
            role_index: HashMap::new(),
            product_index: HashMap::new(),
            instrument_index: HashMap::new(),
            market_index: HashMap::new(),
            currency_index: HashMap::new(),
            csa_type_index: HashMap::new(),
            law_index: HashMap::new(),
            isda_version_index: HashMap::new(),
            confirmation_index: HashMap::new(),
            instruction_index: HashMap::new(),
            entity_aliases: HashMap::new(),
            prepositions: config.prepositions.iter().cloned().collect(),
            conjunctions: config.conjunctions.iter().cloned().collect(),
            articles: config.articles.iter().cloned().collect(),
            negations: config.negation.iter().cloned().collect(),
            pronouns: config.pronouns.clone(),
            product_categories: HashMap::new(),
            instrument_categories: HashMap::new(),
            market_regions: HashMap::new(),
            config,
        };
        
        lexicon.build_indexes();
        lexicon
    }
    
    pub fn load_from_file(path: &Path) -> Result<Self, LexiconError> {
        let file = std::fs::File::open(path)?;
        let config: LexiconConfig = serde_yaml::from_reader(file)?;
        Ok(Self::from_config(config))
    }
    
    fn build_indexes(&mut self) {
        // Build verb index
        for (key, entry) in &self.config.verbs {
            let normalized = key.to_lowercase();
            self.verb_index.insert(normalized.clone(), (entry.class.clone(), entry.canonical.clone()));
            for alias in &entry.aliases {
                self.verb_index.insert(alias.to_lowercase(), (entry.class.clone(), entry.canonical.clone()));
            }
        }
        
        // Build role index
        for (key, entry) in &self.config.roles {
            let normalized = key.to_lowercase();
            self.role_index.insert(normalized.clone(), (entry.domain.clone(), entry.canonical.clone()));
            for alias in &entry.aliases {
                self.role_index.insert(alias.to_lowercase(), (entry.domain.clone(), entry.canonical.clone()));
            }
        }
        
        // Build product index
        for (key, entry) in &self.config.products.values {
            self.product_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.product_index.insert(alias.to_lowercase(), key.clone());
            }
        }
        
        // Build product categories
        for (cat, expansion) in &self.config.products.categories {
            self.product_categories.insert(cat.to_lowercase(), expansion.expands_to.clone());
        }
        
        // Build instrument index
        for (key, entry) in &self.config.instruments.values {
            self.instrument_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.instrument_index.insert(alias.to_lowercase(), key.clone());
            }
        }
        
        // Build instrument categories
        for (cat, expansion) in &self.config.instruments.categories {
            self.instrument_categories.insert(cat.to_lowercase(), expansion.expands_to.clone());
        }
        
        // Build market static aliases
        for (alias, mic) in &self.config.markets.static_aliases {
            self.market_index.insert(alias.to_lowercase(), mic.clone());
        }
        
        // Build market regions
        for (region, expansion) in &self.config.markets.regions {
            self.market_regions.insert(region.to_lowercase(), expansion.expands_to.clone());
        }
        
        // Build entity aliases
        for (alias, canonical) in &self.config.entities.counterparties.static_aliases {
            self.entity_aliases.insert(alias.to_lowercase(), ("counterparty".into(), canonical.clone()));
        }
        for (alias, canonical) in &self.config.entities.investment_managers.static_aliases {
            self.entity_aliases.insert(alias.to_lowercase(), ("investment_manager".into(), canonical.clone()));
        }
        
        // Build CSA types, laws, etc.
        for (key, entry) in &self.config.csa_types {
            self.csa_type_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.csa_type_index.insert(alias.to_lowercase(), key.clone());
            }
        }
        
        for (key, entry) in &self.config.laws {
            self.law_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.law_index.insert(alias.to_lowercase(), key.clone());
            }
        }
        
        for (key, entry) in &self.config.isda_versions {
            self.isda_version_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.isda_version_index.insert(alias.to_lowercase(), key.clone());
            }
        }
        
        for (key, entry) in &self.config.confirmation_methods {
            self.confirmation_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.confirmation_index.insert(alias.to_lowercase(), key.clone());
            }
        }
        
        for (key, entry) in &self.config.instruction_methods {
            self.instruction_index.insert(key.to_lowercase(), key.clone());
            for alias in &entry.aliases {
                self.instruction_index.insert(alias.to_lowercase(), key.clone());
            }
        }
    }
    
    // Lookup methods
    pub fn lookup_verb(&self, phrase: &str) -> Option<(String, String)> {
        self.verb_index.get(&phrase.to_lowercase()).cloned()
    }
    
    pub fn lookup_role(&self, phrase: &str) -> Option<(String, String)> {
        self.role_index.get(&phrase.to_lowercase()).cloned()
    }
    
    pub fn lookup_product(&self, phrase: &str) -> Option<String> {
        self.product_index.get(&phrase.to_lowercase()).cloned()
    }
    
    pub fn lookup_instrument(&self, phrase: &str) -> Option<String> {
        self.instrument_index.get(&phrase.to_lowercase()).cloned()
    }
    
    pub fn lookup_market(&self, phrase: &str) -> Option<String> {
        self.market_index.get(&phrase.to_lowercase()).cloned()
    }
    
    pub fn is_preposition(&self, word: &str) -> bool {
        self.prepositions.contains(&word.to_lowercase())
    }
    
    pub fn is_conjunction(&self, word: &str) -> bool {
        self.conjunctions.contains(&word.to_lowercase())
    }
    
    pub fn is_article(&self, word: &str) -> bool {
        self.articles.contains(&word.to_lowercase())
    }
    
    pub fn is_negation(&self, word: &str) -> bool {
        self.negations.contains(&word.to_lowercase())
    }
    
    pub fn lookup_pronoun(&self, word: &str) -> Option<&PronounEntry> {
        self.pronouns.get(&word.to_lowercase())
    }
    
    pub fn expand_product_category(&self, category: &str) -> Option<&Vec<String>> {
        self.product_categories.get(&category.to_lowercase())
    }
    
    pub fn expand_instrument_category(&self, category: &str) -> Option<&Vec<String>> {
        self.instrument_categories.get(&category.to_lowercase())
    }
    
    pub fn expand_market_region(&self, region: &str) -> Option<&Vec<String>> {
        self.market_regions.get(&region.to_lowercase())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LexiconError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}
```

**Verification:**
```bash
cargo test --lib lexicon::loader
```

---

## Phase 3.2: Tokenizer Implementation

**Goal:** Build tokenizer that converts natural language to typed token stream.

**Duration:** 4-5 days

### Task 3.2.1: Tokenizer Core

**File:** `rust/src/agentic/lexicon/tokenizer.rs` (new)

```rust
//! Lexicon-backed tokenizer for NLU

use super::loader::Lexicon;
use super::tokens::{Token, TokenType, VerbClass, EntityClass, TokenSource, ModifierType};
use std::sync::Arc;

/// Entity resolver trait for async entity lookup
#[async_trait::async_trait]
pub trait EntityResolver: Send + Sync {
    async fn resolve(&self, phrase: &str, entity_type: Option<&str>) -> Option<ResolvedEntity>;
    async fn fuzzy_search(&self, phrase: &str, threshold: f32) -> Vec<ResolvedEntity>;
}

#[derive(Debug, Clone)]
pub struct ResolvedEntity {
    pub name: String,
    pub id: uuid::Uuid,
    pub entity_type: String,
    pub confidence: f32,
}

/// Session context for coreference resolution
pub struct SessionContext {
    /// Recently mentioned entities (most recent first)
    pub salient_entities: Vec<SalientEntity>,
    /// Active symbols from DSL execution
    pub symbols: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SalientEntity {
    pub entity_type: EntityClass,
    pub name: String,
    pub id: Option<uuid::Uuid>,
    pub mention_count: usize,
    pub last_turn: usize,
}

pub struct Tokenizer {
    lexicon: Lexicon,
    entity_resolver: Arc<dyn EntityResolver>,
}

impl Tokenizer {
    pub fn new(lexicon: Lexicon, entity_resolver: Arc<dyn EntityResolver>) -> Self {
        Self { lexicon, entity_resolver }
    }
    
    /// Tokenize input into typed token stream
    pub async fn tokenize(
        &self, 
        input: &str, 
        context: Option<&SessionContext>
    ) -> Vec<Token> {
        let words = self.segment(input);
        let mut tokens = Vec::new();
        let mut i = 0;
        let mut position = 0;
        
        while i < words.len() {
            // Try multi-word matches (longest match wins)
            let (token, consumed, length) = self.match_longest(&words, i, position, context).await;
            
            // Skip articles (absorbed)
            if token.token_type != TokenType::Article {
                tokens.push(token);
            }
            
            i += consumed;
            position += length;
        }
        
        tokens
    }
    
    /// Segment input into word boundaries
    fn segment(&self, input: &str) -> Vec<(String, usize, usize)> {
        let mut words = Vec::new();
        let mut current_word = String::new();
        let mut word_start = 0;
        
        for (i, c) in input.char_indices() {
            if c.is_whitespace() || c == ',' || c == ';' {
                if !current_word.is_empty() {
                    words.push((current_word.clone(), word_start, i));
                    current_word.clear();
                }
                word_start = i + 1;
            } else {
                if current_word.is_empty() {
                    word_start = i;
                }
                current_word.push(c);
            }
        }
        
        if !current_word.is_empty() {
            words.push((current_word, word_start, input.len()));
        }
        
        words
    }
    
    /// Match longest sequence against lexicon
    async fn match_longest(
        &self,
        words: &[(String, usize, usize)],
        start: usize,
        _position: usize,
        context: Option<&SessionContext>,
    ) -> (Token, usize, usize) {
        // Try 4-word, 3-word, 2-word, then 1-word matches
        for window in (1..=4).rev() {
            if start + window > words.len() {
                continue;
            }
            
            let phrase: String = words[start..start + window]
                .iter()
                .map(|(w, _, _)| w.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            
            let span_start = words[start].1;
            let span_end = words[start + window - 1].2;
            let span = (span_start, span_end);
            
            if let Some(token) = self.lookup(&phrase, span, context).await {
                let length = span_end - span_start;
                return (token, window, length);
            }
        }
        
        // No match - return UNKNOWN
        let (word, start, end) = &words[start];
        let token = Token::unknown(word, (*start, *end));
        (token, 1, end - start)
    }
    
    /// Look up phrase in lexicon (priority order)
    async fn lookup(
        &self, 
        phrase: &str, 
        span: (usize, usize),
        context: Option<&SessionContext>,
    ) -> Option<Token> {
        let lower = phrase.to_lowercase();
        
        // 1. Check for negation
        if self.lexicon.is_negation(&lower) {
            return Some(Token {
                token_type: TokenType::Modifier(ModifierType::Negation),
                value: phrase.to_string(),
                canonical: "not".to_string(),
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 2. Check for pronouns (resolve from context)
        if let Some(pronoun_entry) = self.lexicon.lookup_pronoun(&lower) {
            if let Some(ctx) = context {
                if let Some(entity) = self.resolve_pronoun(pronoun_entry, ctx) {
                    return Some(Token {
                        token_type: TokenType::Entity(entity.entity_type.clone()),
                        value: phrase.to_string(),
                        canonical: entity.name.clone(),
                        resolved_id: entity.id,
                        span,
                        confidence: 0.9,
                        source: TokenSource::Coreference(phrase.to_string()),
                    });
                }
            }
            // Unresolved pronoun - return as Unknown for clarification
            return Some(Token::unknown(phrase, span));
        }
        
        // 3. Check articles (will be absorbed)
        if self.lexicon.is_article(&lower) {
            return Some(Token {
                token_type: TokenType::Article,
                value: phrase.to_string(),
                canonical: lower,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 4. Check verbs
        if let Some((class, canonical)) = self.lexicon.lookup_verb(&lower) {
            let verb_class = match class.as_str() {
                "Create" => VerbClass::Create,
                "Update" => VerbClass::Update,
                "Delete" => VerbClass::Delete,
                "Query" => VerbClass::Query,
                "Link" => VerbClass::Link,
                _ => VerbClass::Create,
            };
            return Some(Token {
                token_type: TokenType::Verb(verb_class),
                value: phrase.to_string(),
                canonical,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 5. Check roles
        if let Some((_domain, canonical)) = self.lexicon.lookup_role(&lower) {
            return Some(Token {
                token_type: TokenType::Role,
                value: phrase.to_string(),
                canonical,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 6. Check products (OTC derivatives)
        if let Some(canonical) = self.lexicon.lookup_product(&lower) {
            return Some(Token {
                token_type: TokenType::Product,
                value: phrase.to_string(),
                canonical,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 7. Check instruments (exchange-traded)
        if let Some(canonical) = self.lexicon.lookup_instrument(&lower) {
            return Some(Token {
                token_type: TokenType::Instrument,
                value: phrase.to_string(),
                canonical,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 8. Check markets
        if let Some(mic) = self.lexicon.lookup_market(&lower) {
            return Some(Token {
                token_type: TokenType::Market,
                value: phrase.to_string(),
                canonical: mic,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 9. Check CSA types
        if let Some(csa_type) = self.lexicon.csa_type_index.get(&lower) {
            return Some(Token {
                token_type: TokenType::CsaType,
                value: phrase.to_string(),
                canonical: csa_type.clone(),
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 10. Check laws
        if let Some(law) = self.lexicon.law_index.get(&lower) {
            return Some(Token {
                token_type: TokenType::Law,
                value: phrase.to_string(),
                canonical: law.clone(),
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 11. Check ISDA versions
        if let Some(version) = self.lexicon.isda_version_index.get(&lower) {
            return Some(Token {
                token_type: TokenType::IsdaVersion,
                value: phrase.to_string(),
                canonical: version.clone(),
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 12. Check confirmation methods
        if let Some(method) = self.lexicon.confirmation_index.get(&lower) {
            return Some(Token {
                token_type: TokenType::ConfirmationMethod,
                value: phrase.to_string(),
                canonical: method.clone(),
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 13. Check prepositions
        if self.lexicon.is_preposition(&lower) {
            return Some(Token {
                token_type: TokenType::Prep,
                value: phrase.to_string(),
                canonical: lower,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 14. Check conjunctions
        if self.lexicon.is_conjunction(&lower) {
            return Some(Token {
                token_type: TokenType::Conj,
                value: phrase.to_string(),
                canonical: lower,
                resolved_id: None,
                span,
                confidence: 1.0,
                source: TokenSource::StaticLexicon,
            });
        }
        
        // 15. Try entity gateway (async)
        if let Some(resolved) = self.entity_resolver.resolve(phrase, None).await {
            let entity_class = match resolved.entity_type.as_str() {
                "counterparty" => EntityClass::Counterparty,
                "investment_manager" => EntityClass::InvestmentManager,
                "custodian" => EntityClass::Custodian,
                _ => EntityClass::Unknown,
            };
            return Some(Token {
                token_type: TokenType::Entity(entity_class),
                value: phrase.to_string(),
                canonical: resolved.name,
                resolved_id: Some(resolved.id),
                span,
                confidence: resolved.confidence,
                source: TokenSource::EntityGateway,
            });
        }
        
        // No match
        None
    }
    
    /// Resolve pronoun from session context
    fn resolve_pronoun(
        &self, 
        pronoun: &super::loader::PronounEntry, 
        context: &SessionContext
    ) -> Option<&SalientEntity> {
        // Find most salient entity matching pronoun number
        let is_plural = pronoun.number == "plural";
        
        context.salient_entities.iter().find(|e| {
            // TODO: More sophisticated matching
            // For now, just return most recent entity
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockEntityResolver;
    
    #[async_trait::async_trait]
    impl EntityResolver for MockEntityResolver {
        async fn resolve(&self, phrase: &str, _entity_type: Option<&str>) -> Option<ResolvedEntity> {
            match phrase.to_lowercase().as_str() {
                "goldman sachs" | "goldman" => Some(ResolvedEntity {
                    name: "Goldman Sachs".to_string(),
                    id: uuid::Uuid::new_v4(),
                    entity_type: "counterparty".to_string(),
                    confidence: 1.0,
                }),
                "blackrock" => Some(ResolvedEntity {
                    name: "BlackRock".to_string(),
                    id: uuid::Uuid::new_v4(),
                    entity_type: "investment_manager".to_string(),
                    confidence: 1.0,
                }),
                _ => None,
            }
        }
        
        async fn fuzzy_search(&self, _phrase: &str, _threshold: f32) -> Vec<ResolvedEntity> {
            vec![]
        }
    }
    
    #[tokio::test]
    async fn test_tokenize_counterparty_create() {
        let lexicon = Lexicon::load_from_file(
            std::path::Path::new("config/agent/lexicon.yaml")
        ).unwrap();
        let resolver = Arc::new(MockEntityResolver);
        let tokenizer = Tokenizer::new(lexicon, resolver);
        
        let tokens = tokenizer.tokenize("Add Goldman Sachs as a counterparty", None).await;
        
        assert_eq!(tokens.len(), 4); // add, Goldman Sachs, as, counterparty (article absorbed)
        assert!(matches!(tokens[0].token_type, TokenType::Verb(VerbClass::Create)));
        assert_eq!(tokens[0].canonical, "add");
        assert!(matches!(tokens[1].token_type, TokenType::Entity(EntityClass::Counterparty)));
        assert_eq!(tokens[1].canonical, "Goldman Sachs");
        assert!(matches!(tokens[2].token_type, TokenType::Prep));
        assert!(matches!(tokens[3].token_type, TokenType::Role));
        assert_eq!(tokens[3].canonical, "counterparty");
    }
    
    #[tokio::test]
    async fn test_tokenize_with_products() {
        let lexicon = Lexicon::load_from_file(
            std::path::Path::new("config/agent/lexicon.yaml")
        ).unwrap();
        let resolver = Arc::new(MockEntityResolver);
        let tokenizer = Tokenizer::new(lexicon, resolver);
        
        let tokens = tokenizer.tokenize("Add Goldman as counterparty for IRS and CDS", None).await;
        
        // Should have: add, Goldman, as, counterparty, for, IRS, and, CDS
        let product_tokens: Vec<_> = tokens.iter()
            .filter(|t| matches!(t.token_type, TokenType::Product))
            .collect();
        
        assert_eq!(product_tokens.len(), 2);
        assert_eq!(product_tokens[0].canonical, "IRS");
        assert_eq!(product_tokens[1].canonical, "CDS");
    }
    
    #[tokio::test]
    async fn test_tokenize_isda() {
        let lexicon = Lexicon::load_from_file(
            std::path::Path::new("config/agent/lexicon.yaml")
        ).unwrap();
        let resolver = Arc::new(MockEntityResolver);
        let tokenizer = Tokenizer::new(lexicon, resolver);
        
        let tokens = tokenizer.tokenize("Establish 2002 ISDA with Goldman under NY law", None).await;
        
        assert!(tokens.iter().any(|t| matches!(t.token_type, TokenType::Verb(VerbClass::Create))));
        assert!(tokens.iter().any(|t| matches!(t.token_type, TokenType::IsdaVersion)));
        assert!(tokens.iter().any(|t| matches!(t.token_type, TokenType::Entity(_))));
        assert!(tokens.iter().any(|t| matches!(t.token_type, TokenType::Law)));
    }
}
```

### Task 3.2.2: Database-Backed Lookups

**File:** `rust/src/agentic/lexicon/db_resolver.rs` (new)

```rust
//! Database-backed entity and reference data resolution

use super::tokenizer::{EntityResolver, ResolvedEntity};
use sqlx::PgPool;

pub struct DatabaseEntityResolver {
    pool: PgPool,
    /// Cached entities per session (refreshed on session start)
    cache: tokio::sync::RwLock<EntityCache>,
}

struct EntityCache {
    counterparties: Vec<CachedEntity>,
    investment_managers: Vec<CachedEntity>,
    markets: Vec<CachedMarket>,
    last_refresh: std::time::Instant,
}

#[derive(Clone)]
struct CachedEntity {
    id: uuid::Uuid,
    name: String,
    short_name: Option<String>,
    aliases: Vec<String>,
    entity_type: String,
}

#[derive(Clone)]
struct CachedMarket {
    mic: String,
    name: String,
    aliases: Vec<String>,
}

impl DatabaseEntityResolver {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: tokio::sync::RwLock::new(EntityCache {
                counterparties: vec![],
                investment_managers: vec![],
                markets: vec![],
                last_refresh: std::time::Instant::now(),
            }),
        }
    }
    
    pub async fn refresh_cache(&self) -> Result<(), sqlx::Error> {
        // Load counterparties
        let counterparties = sqlx::query_as!(
            CachedEntityRow,
            r#"SELECT id, name, short_name, NULL as aliases FROM counterparty"#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| CachedEntity {
            id: r.id,
            name: r.name,
            short_name: r.short_name,
            aliases: vec![],
            entity_type: "counterparty".to_string(),
        })
        .collect();
        
        // Load IMs
        let ims = sqlx::query_as!(
            CachedEntityRow,
            r#"SELECT id, manager_name as name, NULL as short_name, NULL as aliases 
               FROM cbu_im_assignment"#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| CachedEntity {
            id: r.id,
            name: r.name,
            short_name: None,
            aliases: vec![],
            entity_type: "investment_manager".to_string(),
        })
        .collect();
        
        // Load markets
        let markets = sqlx::query!(
            r#"SELECT mic, name FROM markets"#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| CachedMarket {
            mic: r.mic,
            name: r.name,
            aliases: vec![],
        })
        .collect();
        
        let mut cache = self.cache.write().await;
        cache.counterparties = counterparties;
        cache.investment_managers = ims;
        cache.markets = markets;
        cache.last_refresh = std::time::Instant::now();
        
        Ok(())
    }
    
    fn fuzzy_match(needle: &str, haystack: &str, threshold: f32) -> Option<f32> {
        let needle_lower = needle.to_lowercase();
        let haystack_lower = haystack.to_lowercase();
        
        // Exact match
        if needle_lower == haystack_lower {
            return Some(1.0);
        }
        
        // Starts with
        if haystack_lower.starts_with(&needle_lower) {
            return Some(0.95);
        }
        
        // Contains
        if haystack_lower.contains(&needle_lower) {
            return Some(0.85);
        }
        
        // Levenshtein distance
        let distance = strsim::levenshtein(&needle_lower, &haystack_lower);
        let max_len = needle_lower.len().max(haystack_lower.len());
        let similarity = 1.0 - (distance as f32 / max_len as f32);
        
        if similarity >= threshold {
            Some(similarity)
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl EntityResolver for DatabaseEntityResolver {
    async fn resolve(&self, phrase: &str, entity_type: Option<&str>) -> Option<ResolvedEntity> {
        let cache = self.cache.read().await;
        
        // Search counterparties
        if entity_type.is_none() || entity_type == Some("counterparty") {
            for cp in &cache.counterparties {
                if let Some(conf) = Self::fuzzy_match(phrase, &cp.name, 0.8) {
                    return Some(ResolvedEntity {
                        name: cp.name.clone(),
                        id: cp.id,
                        entity_type: "counterparty".to_string(),
                        confidence: conf,
                    });
                }
                if let Some(short) = &cp.short_name {
                    if let Some(conf) = Self::fuzzy_match(phrase, short, 0.9) {
                        return Some(ResolvedEntity {
                            name: cp.name.clone(),
                            id: cp.id,
                            entity_type: "counterparty".to_string(),
                            confidence: conf,
                        });
                    }
                }
            }
        }
        
        // Search IMs
        if entity_type.is_none() || entity_type == Some("investment_manager") {
            for im in &cache.investment_managers {
                if let Some(conf) = Self::fuzzy_match(phrase, &im.name, 0.8) {
                    return Some(ResolvedEntity {
                        name: im.name.clone(),
                        id: im.id,
                        entity_type: "investment_manager".to_string(),
                        confidence: conf,
                    });
                }
            }
        }
        
        None
    }
    
    async fn fuzzy_search(&self, phrase: &str, threshold: f32) -> Vec<ResolvedEntity> {
        let cache = self.cache.read().await;
        let mut results = Vec::new();
        
        for cp in &cache.counterparties {
            if let Some(conf) = Self::fuzzy_match(phrase, &cp.name, threshold) {
                results.push(ResolvedEntity {
                    name: cp.name.clone(),
                    id: cp.id,
                    entity_type: "counterparty".to_string(),
                    confidence: conf,
                });
            }
        }
        
        for im in &cache.investment_managers {
            if let Some(conf) = Self::fuzzy_match(phrase, &im.name, threshold) {
                results.push(ResolvedEntity {
                    name: im.name.clone(),
                    id: im.id,
                    entity_type: "investment_manager".to_string(),
                    confidence: conf,
                });
            }
        }
        
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }
}

struct CachedEntityRow {
    id: uuid::Uuid,
    name: String,
    short_name: Option<String>,
    aliases: Option<String>,
}
```

**Verification:**
```bash
cargo test --lib lexicon::tokenizer
cargo test --lib lexicon::db_resolver
```

---

## Phase 3.3: Intent Grammar (Nom)

**Goal:** Parse token stream into IntentAst using formal grammar.

**Duration:** 5-6 days

### Task 3.3.1: Intent AST Types

**File:** `rust/src/agentic/lexicon/intent_ast.rs` (new)

```rust
//! Intent AST - output of intent grammar parser

use uuid::Uuid;

/// Parsed intent from natural language
#[derive(Debug, Clone)]
pub enum IntentAst {
    // === OTC Domain ===
    CounterpartyCreate {
        entity: Option<EntityRef>,
        jurisdiction: Option<String>,
    },
    IsdaEstablish {
        counterparty: EntityRef,
        version: Option<String>,
        law: Option<String>,
        date: Option<String>,
    },
    IsdaAddProducts {
        isda_ref: EntityRef,
        products: Vec<String>,
    },
    CsaEstablish {
        isda_ref: EntityRef,
        csa_type: String,
        our_threshold: Option<f64>,
        their_threshold: Option<f64>,
    },
    CsaAddCollateral {
        csa_ref: EntityRef,
        asset_classes: Vec<String>,
        haircut: Option<f64>,
        currency: Option<String>,
    },
    CollateralSetupAccount {
        csa_ref: EntityRef,
        account_type: String,
        custodian: Option<String>,
    },
    ConfirmationConfigure {
        counterparty: Option<EntityRef>,
        method: String,
        products: Vec<String>,
    },
    
    // === Trading Matrix Domain ===
    ImAssign {
        entity: Option<EntityRef>,
        scope: ScopeAst,
        method: Option<String>,
    },
    ImUpdateScope {
        im_ref: EntityRef,
        add_markets: Vec<String>,
        add_instruments: Vec<String>,
        remove_markets: Vec<String>,
        remove_instruments: Vec<String>,
    },
    ImRemove {
        im_ref: EntityRef,
    },
    PricingSet {
        source: String,
        instruments: Vec<String>,
    },
    SweepConfigure {
        currency: String,
        threshold: Option<f64>,
        vehicle: Option<String>,
    },
    
    // === Query Domain ===
    Query {
        query_type: QueryType,
        filters: Vec<FilterAst>,
    },
    
    // === Compound ===
    Compound {
        intents: Vec<IntentAst>,
    },
    
    // === Negated ===
    Negated {
        intent: Box<IntentAst>,
    },
}

/// Reference to an entity
#[derive(Debug, Clone)]
pub enum EntityRef {
    /// Resolved entity with ID
    Resolved {
        name: String,
        id: Uuid,
        entity_type: String,
    },
    /// Symbol reference (@im-blackrock)
    Symbol(String),
    /// Resolve from session context
    Context,
    /// Unresolved - needs clarification
    Unresolved(String),
}

/// Scope specification (markets, instruments, products)
#[derive(Debug, Clone, Default)]
pub struct ScopeAst {
    pub products: Vec<String>,
    pub instruments: Vec<String>,
    pub markets: Vec<String>,
    pub regions: Vec<String>,
}

impl ScopeAst {
    pub fn is_empty(&self) -> bool {
        self.products.is_empty() 
            && self.instruments.is_empty() 
            && self.markets.is_empty()
            && self.regions.is_empty()
    }
}

/// Query type
#[derive(Debug, Clone)]
pub enum QueryType {
    WhoHandles,         // "who handles European equities"
    ListIms,            // "show investment managers"
    ListCounterparties, // "show counterparties"
    ShowMatrix,         // "show trading matrix"
    ValidateGaps,       // "any configuration gaps"
    ShowIsda,           // "show ISDA with Goldman"
    ShowCsa,            // "show CSA terms"
}

/// Filter for queries
#[derive(Debug, Clone)]
pub struct FilterAst {
    pub field: String,
    pub operator: FilterOp,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum FilterOp {
    Equals,
    Contains,
    In,
}

impl IntentAst {
    /// Get the domain for this intent
    pub fn domain(&self) -> &'static str {
        match self {
            Self::CounterpartyCreate { .. } |
            Self::IsdaEstablish { .. } |
            Self::IsdaAddProducts { .. } |
            Self::CsaEstablish { .. } |
            Self::CsaAddCollateral { .. } |
            Self::CollateralSetupAccount { .. } |
            Self::ConfirmationConfigure { .. } => "otc",
            
            Self::ImAssign { .. } |
            Self::ImUpdateScope { .. } |
            Self::ImRemove { .. } |
            Self::PricingSet { .. } |
            Self::SweepConfigure { .. } => "trading_matrix",
            
            Self::Query { .. } => "query",
            
            Self::Compound { intents } => {
                intents.first().map(|i| i.domain()).unwrap_or("unknown")
            }
            
            Self::Negated { intent } => intent.domain(),
        }
    }
    
    /// Check if this intent is complete (has required fields)
    pub fn is_complete(&self) -> bool {
        match self {
            Self::CounterpartyCreate { entity, .. } => entity.is_some(),
            Self::IsdaEstablish { counterparty, .. } => !matches!(counterparty, EntityRef::Unresolved(_)),
            Self::ImAssign { entity, .. } => entity.is_some(),
            Self::PricingSet { source, .. } => !source.is_empty(),
            _ => true,
        }
    }
    
    /// Get missing required fields
    pub fn missing_fields(&self) -> Vec<&'static str> {
        match self {
            Self::CounterpartyCreate { entity: None, .. } => vec!["counterparty name"],
            Self::IsdaEstablish { counterparty: EntityRef::Unresolved(_), .. } => vec!["counterparty"],
            Self::ImAssign { entity: None, .. } => vec!["investment manager name"],
            Self::CsaEstablish { isda_ref: EntityRef::Unresolved(_), .. } => vec!["ISDA reference"],
            _ => vec![],
        }
    }
}
```

### Task 3.3.2: Intent Grammar Parser

**File:** `rust/src/agentic/lexicon/intent_parser.rs` (new)

```rust
//! Nom-based intent grammar parser
//!
//! Parses token stream into IntentAst using formal grammar rules.

use nom::{
    IResult,
    branch::alt,
    combinator::{opt, map, value, eof},
    multi::{many0, many1, separated_list1},
    sequence::{tuple, preceded, terminated},
};
use super::tokens::{Token, TokenType, VerbClass, EntityClass};
use super::intent_ast::*;

/// Parser input type - slice of tokens
pub type Tokens<'a> = &'a [Token];

/// Parse error with context
#[derive(Debug, Clone)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub position: usize,
    pub context: String,
}

#[derive(Debug, Clone)]
pub enum ParseErrorKind {
    /// Missing required tokens
    Incomplete {
        partial: Option<IntentAst>,
        expected: Vec<String>,
    },
    /// Multiple valid interpretations
    Ambiguous {
        options: Vec<IntentAst>,
    },
    /// Tokens not in lexicon
    UnknownTokens {
        tokens: Vec<String>,
    },
    /// Grammar violation
    SyntaxError {
        expected: String,
        found: String,
    },
    /// Empty input
    Empty,
}

impl ParseError {
    pub fn to_clarification(&self) -> String {
        match &self.kind {
            ParseErrorKind::Incomplete { expected, .. } => {
                format!("I need more information: {}", expected.join(", "))
            }
            ParseErrorKind::Ambiguous { options } => {
                format!("That's ambiguous. Did you mean: {}?", 
                    options.iter().map(|o| format!("{:?}", o)).collect::<Vec<_>>().join(" or "))
            }
            ParseErrorKind::UnknownTokens { tokens } => {
                format!("I don't recognize: {}", tokens.join(", "))
            }
            ParseErrorKind::SyntaxError { expected, found } => {
                format!("Expected {} but found '{}'", expected, found)
            }
            ParseErrorKind::Empty => "I didn't understand that. Can you rephrase?".to_string(),
        }
    }
}

/// Match a specific token type
fn token_type<'a>(
    expected: TokenType
) -> impl Fn(Tokens<'a>) -> IResult<Tokens<'a>, &'a Token> {
    move |input: Tokens<'a>| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input, 
                nom::error::ErrorKind::Eof
            )));
        }
        
        if std::mem::discriminant(&input[0].token_type) == std::mem::discriminant(&expected) {
            Ok((&input[1..], &input[0]))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                input, 
                nom::error::ErrorKind::Tag
            )))
        }
    }
}

/// Match verb with specific class
fn verb_class<'a>(
    class: VerbClass
) -> impl Fn(Tokens<'a>) -> IResult<Tokens<'a>, &'a Token> {
    move |input: Tokens<'a>| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input, 
                nom::error::ErrorKind::Eof
            )));
        }
        
        if let TokenType::Verb(ref c) = input[0].token_type {
            if *c == class {
                return Ok((&input[1..], &input[0]));
            }
        }
        
        Err(nom::Err::Error(nom::error::Error::new(
            input, 
            nom::error::ErrorKind::Tag
        )))
    }
}

/// Match entity with specific class
fn entity_class<'a>(
    class: EntityClass
) -> impl Fn(Tokens<'a>) -> IResult<Tokens<'a>, &'a Token> {
    move |input: Tokens<'a>| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input, 
                nom::error::ErrorKind::Eof
            )));
        }
        
        if let TokenType::Entity(ref c) = input[0].token_type {
            if *c == class || class == EntityClass::Unknown {
                return Ok((&input[1..], &input[0]));
            }
        }
        
        Err(nom::Err::Error(nom::error::Error::new(
            input, 
            nom::error::ErrorKind::Tag
        )))
    }
}

/// Match role with specific canonical name
fn role_canonical<'a>(
    name: &'a str
) -> impl Fn(Tokens<'a>) -> IResult<Tokens<'a>, &'a Token> {
    move |input: Tokens<'a>| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input, 
                nom::error::ErrorKind::Eof
            )));
        }
        
        if let TokenType::Role = input[0].token_type {
            if input[0].canonical == name {
                return Ok((&input[1..], &input[0]));
            }
        }
        
        Err(nom::Err::Error(nom::error::Error::new(
            input, 
            nom::error::ErrorKind::Tag
        )))
    }
}

/// Optional preposition
fn opt_prep<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, Option<&'a Token>> {
    opt(token_type(TokenType::Prep))(input)
}

/// Skip prepositions
fn skip_prep<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, ()> {
    let (rest, _) = many0(token_type(TokenType::Prep))(input)?;
    Ok((rest, ()))
}

/// Skip conjunctions
fn skip_conj<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, ()> {
    let (rest, _) = many0(token_type(TokenType::Conj))(input)?;
    Ok((rest, ()))
}

// =============================================================================
// COUNTERPARTY GRAMMAR
// =============================================================================

/// Parse: VERB:Create ENTITY:Counterparty? PREP? ROLE:counterparty
fn counterparty_create<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, entity, _, _)) = tuple((
        verb_class(VerbClass::Create),
        opt(entity_class(EntityClass::Counterparty)),
        opt_prep,
        role_canonical("counterparty"),
    ))(input)?;
    
    Ok((rest, IntentAst::CounterpartyCreate {
        entity: entity.map(token_to_entity_ref),
        jurisdiction: None,
    }))
}

/// Alternative: VERB:Create ROLE:counterparty ENTITY?
fn counterparty_create_alt<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, _, entity)) = tuple((
        verb_class(VerbClass::Create),
        role_canonical("counterparty"),
        opt(entity_class(EntityClass::Counterparty)),
    ))(input)?;
    
    Ok((rest, IntentAst::CounterpartyCreate {
        entity: entity.map(token_to_entity_ref),
        jurisdiction: None,
    }))
}

// =============================================================================
// ISDA GRAMMAR
// =============================================================================

/// Parse: VERB:Create ISDA_VERSION? "ISDA" PREP ENTITY PREP? LAW?
fn isda_establish<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, version, _, entity, _, law)) = tuple((
        verb_class(VerbClass::Create),
        opt(token_type(TokenType::IsdaVersion)),
        skip_prep,  // Skip "ISDA" word - it's absorbed into the intent
        entity_class(EntityClass::Counterparty),
        skip_prep,
        opt(token_type(TokenType::Law)),
    ))(input)?;
    
    Ok((rest, IntentAst::IsdaEstablish {
        counterparty: token_to_entity_ref(entity),
        version: version.map(|t| t.canonical.clone()),
        law: law.map(|t| t.canonical.clone()),
        date: None,
    }))
}

/// Parse: VERB:Create PRODUCT+ PREP ISDA_REF
fn isda_add_products<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, products, _, _isda)) = tuple((
        verb_class(VerbClass::Create),
        many1(token_type(TokenType::Product)),
        opt_prep,
        // TODO: ISDA reference
    ))(input)?;
    
    Ok((rest, IntentAst::IsdaAddProducts {
        isda_ref: EntityRef::Context,
        products: products.iter().map(|t| t.canonical.clone()).collect(),
    }))
}

// =============================================================================
// CSA GRAMMAR
// =============================================================================

/// Parse: VERB:Create CSA_TYPE? "CSA" (PREP "zero"? "threshold")?
fn csa_establish<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, csa_type, _, _)) = tuple((
        verb_class(VerbClass::Create),
        opt(token_type(TokenType::CsaType)),
        skip_prep,
        // TODO: threshold parsing
        many0(token_type(TokenType::Unknown)), // Absorb remaining
    ))(input)?;
    
    Ok((rest, IntentAst::CsaEstablish {
        isda_ref: EntityRef::Context,
        csa_type: csa_type.map(|t| t.canonical.clone()).unwrap_or_else(|| "VM".to_string()),
        our_threshold: Some(0.0),
        their_threshold: Some(0.0),
    }))
}

// =============================================================================
// INVESTMENT MANAGER GRAMMAR
// =============================================================================

/// Parse: VERB:Create ENTITY:IM? PREP? ROLE:investment_manager SCOPE?
fn im_assign<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, entity, _, _, scope, method)) = tuple((
        verb_class(VerbClass::Create),
        opt(entity_class(EntityClass::InvestmentManager)),
        opt_prep,
        opt(role_canonical("investment_manager")),
        opt(scope_clause),
        opt(method_clause),
    ))(input)?;
    
    Ok((rest, IntentAst::ImAssign {
        entity: entity.map(token_to_entity_ref),
        scope: scope.unwrap_or_default(),
        method: method,
    }))
}

/// Parse scope: PREP (PRODUCT | INSTRUMENT | MARKET)+
fn scope_clause<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, ScopeAst> {
    let (rest, (_, items)) = tuple((
        token_type(TokenType::Prep),
        many1(alt((
            map(token_type(TokenType::Product), |t| ("product", t.canonical.clone())),
            map(token_type(TokenType::Instrument), |t| ("instrument", t.canonical.clone())),
            map(token_type(TokenType::Market), |t| ("market", t.canonical.clone())),
        ))),
    ))(input)?;
    
    let mut scope = ScopeAst::default();
    for (kind, value) in items {
        match kind {
            "product" => scope.products.push(value),
            "instrument" => scope.instruments.push(value),
            "market" => scope.markets.push(value),
            _ => {}
        }
    }
    
    Ok((rest, scope))
}

/// Parse method: PREP? "via"? INSTRUCTION_METHOD
fn method_clause<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, String> {
    // Look for instruction method token
    let (rest, (_, method)) = tuple((
        skip_prep,
        alt((
            map(token_type(TokenType::ConfirmationMethod), |t| t.canonical.clone()),
            // Also check for CTM/SWIFT/FIX in unknown tokens
            map(
                |i: Tokens<'a>| {
                    if !i.is_empty() {
                        let val = i[0].canonical.to_uppercase();
                        if val == "CTM" || val == "SWIFT" || val == "FIX" || val == "ALERT" {
                            return Ok((&i[1..], i[0].canonical.clone()));
                        }
                    }
                    Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Tag)))
                },
                |v| v
            ),
        )),
    ))(input)?;
    
    Ok((rest, method))
}

// =============================================================================
// PRICING GRAMMAR
// =============================================================================

/// Parse: VERB:Link ENTITY:PricingSource PREP? INSTRUMENT*
fn pricing_set<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, source, _, instruments)) = tuple((
        verb_class(VerbClass::Link),
        entity_class(EntityClass::Unknown), // Pricing source
        skip_prep,
        many0(token_type(TokenType::Instrument)),
    ))(input)?;
    
    Ok((rest, IntentAst::PricingSet {
        source: source.canonical.clone(),
        instruments: instruments.iter().map(|t| t.canonical.clone()).collect(),
    }))
}

// =============================================================================
// QUERY GRAMMAR
// =============================================================================

/// Parse: VERB:Query (ROLE | SCOPE)?
fn query_intent<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (verb, role, scope)) = tuple((
        verb_class(VerbClass::Query),
        opt(token_type(TokenType::Role)),
        opt(scope_clause),
    ))(input)?;
    
    let query_type = match verb.canonical.as_str() {
        "who" => QueryType::WhoHandles,
        "show" | "list" => {
            if let Some(r) = role {
                match r.canonical.as_str() {
                    "counterparty" => QueryType::ListCounterparties,
                    "investment_manager" => QueryType::ListIms,
                    _ => QueryType::ShowMatrix,
                }
            } else {
                QueryType::ShowMatrix
            }
        }
        _ => QueryType::ShowMatrix,
    };
    
    Ok((rest, IntentAst::Query {
        query_type,
        filters: vec![],
    }))
}

// =============================================================================
// CONFIRMATION GRAMMAR
// =============================================================================

/// Parse: VERB:Link CONFIRMATION_METHOD PREP PRODUCT*
fn confirmation_configure<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    let (rest, (_, method, _, products)) = tuple((
        verb_class(VerbClass::Link),
        token_type(TokenType::ConfirmationMethod),
        skip_prep,
        many0(token_type(TokenType::Product)),
    ))(input)?;
    
    Ok((rest, IntentAst::ConfirmationConfigure {
        counterparty: None,
        method: method.canonical.clone(),
        products: products.iter().map(|t| t.canonical.clone()).collect(),
    }))
}

// =============================================================================
// TOP-LEVEL PARSER
// =============================================================================

/// Parse tokens into IntentAst
pub fn parse_intent<'a>(input: Tokens<'a>) -> IResult<Tokens<'a>, IntentAst> {
    alt((
        // OTC domain
        counterparty_create,
        counterparty_create_alt,
        isda_establish,
        isda_add_products,
        csa_establish,
        confirmation_configure,
        
        // Trading matrix domain
        im_assign,
        pricing_set,
        
        // Query domain
        query_intent,
    ))(input)
}

/// Parse with error recovery and clarification
pub fn parse_with_recovery(tokens: &[Token]) -> Result<IntentAst, ParseError> {
    if tokens.is_empty() {
        return Err(ParseError {
            kind: ParseErrorKind::Empty,
            position: 0,
            context: String::new(),
        });
    }
    
    // Check for unknown tokens first
    let unknown: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.token_type, TokenType::Unknown))
        .map(|t| t.value.clone())
        .collect();
    
    if !unknown.is_empty() && unknown.len() > tokens.len() / 2 {
        return Err(ParseError {
            kind: ParseErrorKind::UnknownTokens { tokens: unknown },
            position: 0,
            context: String::new(),
        });
    }
    
    match parse_intent(tokens) {
        Ok((remaining, intent)) => {
            // Check if we consumed all tokens
            if !remaining.is_empty() && !remaining.iter().all(|t| 
                matches!(t.token_type, TokenType::Prep | TokenType::Conj | TokenType::Unknown)
            ) {
                // Partial parse - might be compound intent
                // For now, return what we got
            }
            
            // Check if intent is complete
            if !intent.is_complete() {
                return Err(ParseError {
                    kind: ParseErrorKind::Incomplete {
                        partial: Some(intent.clone()),
                        expected: intent.missing_fields().iter().map(|s| s.to_string()).collect(),
                    },
                    position: 0,
                    context: String::new(),
                });
            }
            
            Ok(intent)
        }
        Err(e) => {
            // Try to give helpful error
            Err(ParseError {
                kind: ParseErrorKind::SyntaxError {
                    expected: "valid intent pattern".to_string(),
                    found: tokens.first().map(|t| t.value.clone()).unwrap_or_default(),
                },
                position: 0,
                context: format!("{:?}", e),
            })
        }
    }
}

// =============================================================================
// HELPERS
// =============================================================================

fn token_to_entity_ref(token: &Token) -> EntityRef {
    if let Some(id) = token.resolved_id {
        EntityRef::Resolved {
            name: token.canonical.clone(),
            id,
            entity_type: match &token.token_type {
                TokenType::Entity(class) => format!("{:?}", class).to_lowercase(),
                _ => "unknown".to_string(),
            },
        }
    } else {
        EntityRef::Unresolved(token.canonical.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_token(token_type: TokenType, canonical: &str) -> Token {
        Token {
            token_type,
            value: canonical.to_string(),
            canonical: canonical.to_string(),
            resolved_id: None,
            span: (0, 0),
            confidence: 1.0,
            source: super::super::tokens::TokenSource::StaticLexicon,
        }
    }
    
    #[test]
    fn test_parse_counterparty_create() {
        let tokens = vec![
            make_token(TokenType::Verb(VerbClass::Create), "add"),
            make_token(TokenType::Entity(EntityClass::Counterparty), "Goldman Sachs"),
            make_token(TokenType::Prep, "as"),
            make_token(TokenType::Role, "counterparty"),
        ];
        
        let result = parse_with_recovery(&tokens);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), IntentAst::CounterpartyCreate { .. }));
    }
    
    #[test]
    fn test_parse_im_assign() {
        let tokens = vec![
            make_token(TokenType::Verb(VerbClass::Create), "add"),
            make_token(TokenType::Entity(EntityClass::InvestmentManager), "BlackRock"),
            make_token(TokenType::Prep, "for"),
            make_token(TokenType::Instrument, "EQUITY"),
        ];
        
        let result = parse_with_recovery(&tokens);
        assert!(result.is_ok());
        if let IntentAst::ImAssign { scope, .. } = result.unwrap() {
            assert!(scope.instruments.contains(&"EQUITY".to_string()));
        } else {
            panic!("Expected ImAssign");
        }
    }
}
```

**Verification:**
```bash
cargo test --lib lexicon::intent_parser
```

---

## Phase 3.4: Pipeline Integration

**Goal:** Replace regex-based classifier with tokenizer + grammar parser.

**Duration:** 3-4 days

### Task 3.4.1: New Pipeline Architecture

**File:** `rust/src/agentic/pipeline.rs` (rewrite)

```rust
//! Agent Pipeline - Lexicon/Grammar based
//!
//! Architecture:
//! 1. Tokenizer (lexicon-backed) → Token stream
//! 2. Intent Parser (nom grammar) → IntentAst
//! 3. DSL Generator → DSL source
//! 4. DSL Pipeline → Execute

use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use sqlx::PgPool;

use crate::agentic::lexicon::{
    Lexicon, Tokenizer, Token, TokenType,
    IntentAst, EntityRef, ParseError, ParseErrorKind,
    parse_with_recovery,
    DatabaseEntityResolver,
};

pub struct AgentPipeline {
    // Lexicon-based components
    tokenizer: Tokenizer,
    
    // DSL generation
    dsl_generator: DslGenerator,
    
    // Execution
    pool: Option<PgPool>,
    
    // Session management
    sessions: HashMap<Uuid, SessionState>,
}

pub struct SessionState {
    /// Salient entities for coreference
    pub salient_entities: Vec<SalientEntity>,
    /// Active symbols from execution
    pub symbols: HashMap<String, String>,
    /// Current CBU context
    pub active_cbu: Option<Uuid>,
    /// Turn counter
    pub turn: usize,
}

#[derive(Debug, Clone)]
pub struct SalientEntity {
    pub entity_type: String,
    pub name: String,
    pub id: Option<Uuid>,
    pub last_turn: usize,
}

pub struct AgentResponse {
    /// Generated DSL (if successful)
    pub dsl: Option<String>,
    /// Parsed intent (for debugging)
    pub intent: Option<IntentAst>,
    /// Natural language response
    pub response_text: Option<String>,
    /// Clarification needed
    pub clarification: Option<String>,
    /// Error (if any)
    pub error: Option<String>,
    /// Response type
    pub response_type: ResponseType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResponseType {
    Execution,
    Query,
    Clarification,
    Error,
}

impl AgentPipeline {
    pub async fn from_config_dir(config_dir: &std::path::Path, pool: Option<PgPool>) -> Result<Self, PipelineError> {
        // Load lexicon
        let lexicon = Lexicon::load_from_file(&config_dir.join("lexicon.yaml"))
            .map_err(|e| PipelineError::Config(e.to_string()))?;
        
        // Create entity resolver
        let resolver: Arc<dyn super::lexicon::tokenizer::EntityResolver> = if let Some(ref p) = pool {
            let db_resolver = DatabaseEntityResolver::new(p.clone());
            db_resolver.refresh_cache().await.ok();
            Arc::new(db_resolver)
        } else {
            Arc::new(NoOpEntityResolver)
        };
        
        let tokenizer = Tokenizer::new(lexicon, resolver);
        let dsl_generator = DslGenerator::new();
        
        Ok(Self {
            tokenizer,
            dsl_generator,
            pool,
            sessions: HashMap::new(),
        })
    }
    
    pub fn with_database(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }
    
    pub async fn process(&mut self, message: &str, session_id: Uuid) -> Result<AgentResponse, PipelineError> {
        // Get or create session
        let session = self.sessions.entry(session_id).or_insert_with(|| SessionState {
            salient_entities: vec![],
            symbols: HashMap::new(),
            active_cbu: None,
            turn: 0,
        });
        session.turn += 1;
        
        // Build session context for tokenizer
        let context = super::lexicon::tokenizer::SessionContext {
            salient_entities: session.salient_entities.iter().map(|e| {
                super::lexicon::tokenizer::SalientEntity {
                    entity_type: match e.entity_type.as_str() {
                        "counterparty" => super::lexicon::tokens::EntityClass::Counterparty,
                        "investment_manager" => super::lexicon::tokens::EntityClass::InvestmentManager,
                        _ => super::lexicon::tokens::EntityClass::Unknown,
                    },
                    name: e.name.clone(),
                    id: e.id,
                    mention_count: 1,
                    last_turn: e.last_turn,
                }
            }).collect(),
            symbols: session.symbols.clone(),
        };
        
        // Step 1: Tokenize
        let tokens = self.tokenizer.tokenize(message, Some(&context)).await;
        
        // Log tokens for debugging
        log::debug!("Tokens: {:?}", tokens.iter().map(|t| (&t.token_type, &t.canonical)).collect::<Vec<_>>());
        
        // Check for unknown tokens
        let unknown_tokens: Vec<_> = tokens.iter()
            .filter(|t| matches!(t.token_type, TokenType::Unknown))
            .collect();
        
        if !unknown_tokens.is_empty() && unknown_tokens.len() == tokens.len() {
            return Ok(AgentResponse {
                dsl: None,
                intent: None,
                response_text: None,
                clarification: Some(format!(
                    "I didn't understand: {}. Can you rephrase?",
                    unknown_tokens.iter().map(|t| t.value.as_str()).collect::<Vec<_>>().join(", ")
                )),
                error: None,
                response_type: ResponseType::Clarification,
            });
        }
        
        // Step 2: Parse tokens into IntentAst
        match parse_with_recovery(&tokens) {
            Ok(intent) => {
                // Update salient entities from parsed intent
                self.update_salience(&intent, session_id);
                
                // Step 3: Generate DSL
                let dsl = self.dsl_generator.from_ast(&intent, &session.symbols)?;
                
                // Step 4: Execute (if we have a pool)
                if let Some(ref pool) = self.pool {
                    match self.execute_dsl(&dsl, pool, session_id).await {
                        Ok(result) => {
                            // Update session with new symbols
                            if let Some(s) = self.sessions.get_mut(&session_id) {
                                for (k, v) in result.bindings {
                                    s.symbols.insert(k, v);
                                }
                            }
                            
                            Ok(AgentResponse {
                                dsl: Some(dsl),
                                intent: Some(intent),
                                response_text: Some(result.message),
                                clarification: None,
                                error: None,
                                response_type: ResponseType::Execution,
                            })
                        }
                        Err(e) => Ok(AgentResponse {
                            dsl: Some(dsl),
                            intent: Some(intent),
                            response_text: None,
                            clarification: None,
                            error: Some(e.to_string()),
                            response_type: ResponseType::Error,
                        }),
                    }
                } else {
                    // No database - return DSL only
                    Ok(AgentResponse {
                        dsl: Some(dsl),
                        intent: Some(intent),
                        response_text: Some("DSL generated (no database connected)".to_string()),
                        clarification: None,
                        error: None,
                        response_type: ResponseType::Execution,
                    })
                }
            }
            
            Err(ParseError { kind: ParseErrorKind::Incomplete { partial, expected }, .. }) => {
                Ok(AgentResponse {
                    dsl: None,
                    intent: partial,
                    response_text: None,
                    clarification: Some(format!("I need more information: {}", expected.join(", "))),
                    error: None,
                    response_type: ResponseType::Clarification,
                })
            }
            
            Err(ParseError { kind: ParseErrorKind::UnknownTokens { tokens }, .. }) => {
                // Try fuzzy matching
                Ok(AgentResponse {
                    dsl: None,
                    intent: None,
                    response_text: None,
                    clarification: Some(format!("I don't recognize: {}. Did you mean something else?", tokens.join(", "))),
                    error: None,
                    response_type: ResponseType::Clarification,
                })
            }
            
            Err(e) => {
                Ok(AgentResponse {
                    dsl: None,
                    intent: None,
                    response_text: None,
                    clarification: Some(e.to_clarification()),
                    error: None,
                    response_type: ResponseType::Clarification,
                })
            }
        }
    }
    
    fn update_salience(&mut self, intent: &IntentAst, session_id: Uuid) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            // Extract entities from intent and add to salience
            match intent {
                IntentAst::CounterpartyCreate { entity: Some(EntityRef::Resolved { name, id, .. }), .. } => {
                    session.salient_entities.insert(0, SalientEntity {
                        entity_type: "counterparty".to_string(),
                        name: name.clone(),
                        id: Some(*id),
                        last_turn: session.turn,
                    });
                }
                IntentAst::ImAssign { entity: Some(EntityRef::Resolved { name, id, .. }), .. } => {
                    session.salient_entities.insert(0, SalientEntity {
                        entity_type: "investment_manager".to_string(),
                        name: name.clone(),
                        id: Some(*id),
                        last_turn: session.turn,
                    });
                }
                // Add more cases...
                _ => {}
            }
            
            // Keep only last 10 salient entities
            session.salient_entities.truncate(10);
        }
    }
    
    async fn execute_dsl(&self, dsl: &str, pool: &PgPool, session_id: Uuid) -> Result<ExecutionResult, PipelineError> {
        // Parse DSL
        let program = crate::dsl_v2::parse_program(dsl)
            .map_err(|e| PipelineError::Execution(format!("Parse error: {}", e)))?;
        
        // Compile
        let plan = crate::dsl_v2::compile(&program)
            .map_err(|e| PipelineError::Execution(format!("Compile error: {}", e)))?;
        
        // Execute with transaction
        let mut tx = pool.begin().await
            .map_err(|e| PipelineError::Execution(e.to_string()))?;
        
        let executor = crate::dsl_v2::GenericCrudExecutor::new();
        let mut ctx = crate::dsl_v2::ExecutionContext::new();
        
        for step in &plan.steps {
            executor.execute_step(step, &mut ctx, &mut tx).await
                .map_err(|e| PipelineError::Execution(e.to_string()))?;
        }
        
        tx.commit().await
            .map_err(|e| PipelineError::Execution(e.to_string()))?;
        
        Ok(ExecutionResult {
            success: true,
            bindings: ctx.symbols.iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
            message: "Executed successfully".to_string(),
        })
    }
}

struct ExecutionResult {
    success: bool,
    bindings: HashMap<String, String>,
    message: String,
}

struct NoOpEntityResolver;

#[async_trait::async_trait]
impl super::lexicon::tokenizer::EntityResolver for NoOpEntityResolver {
    async fn resolve(&self, _phrase: &str, _entity_type: Option<&str>) -> Option<super::lexicon::tokenizer::ResolvedEntity> {
        None
    }
    async fn fuzzy_search(&self, _phrase: &str, _threshold: f32) -> Vec<super::lexicon::tokenizer::ResolvedEntity> {
        vec![]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Generation error: {0}")]
    Generation(String),
}
```

### Task 3.4.2: DSL Generator from AST

**File:** `rust/src/agentic/dsl_generator.rs` (rewrite)

```rust
//! DSL Generator - converts IntentAst to DSL source

use super::lexicon::intent_ast::{IntentAst, EntityRef, ScopeAst};
use std::collections::HashMap;

pub struct DslGenerator;

impl DslGenerator {
    pub fn new() -> Self {
        Self
    }
    
    pub fn from_ast(
        &self, 
        intent: &IntentAst, 
        symbols: &HashMap<String, String>
    ) -> Result<String, GeneratorError> {
        match intent {
            IntentAst::CounterpartyCreate { entity, jurisdiction } => {
                self.gen_counterparty_create(entity, jurisdiction)
            }
            IntentAst::IsdaEstablish { counterparty, version, law, date } => {
                self.gen_isda_establish(counterparty, version, law, date, symbols)
            }
            IntentAst::IsdaAddProducts { isda_ref, products } => {
                self.gen_isda_add_products(isda_ref, products, symbols)
            }
            IntentAst::CsaEstablish { isda_ref, csa_type, our_threshold, their_threshold } => {
                self.gen_csa_establish(isda_ref, csa_type, our_threshold, their_threshold, symbols)
            }
            IntentAst::ImAssign { entity, scope, method } => {
                self.gen_im_assign(entity, scope, method)
            }
            IntentAst::PricingSet { source, instruments } => {
                self.gen_pricing_set(source, instruments)
            }
            IntentAst::ConfirmationConfigure { counterparty, method, products } => {
                self.gen_confirmation_configure(counterparty, method, products, symbols)
            }
            IntentAst::Query { query_type, filters } => {
                // Queries don't generate DSL - handle separately
                Err(GeneratorError::QueryIntent)
            }
            IntentAst::Compound { intents } => {
                let dsls: Result<Vec<_>, _> = intents.iter()
                    .map(|i| self.from_ast(i, symbols))
                    .collect();
                Ok(dsls?.join("\n\n"))
            }
            IntentAst::Negated { intent } => {
                Err(GeneratorError::NegatedIntent)
            }
            _ => Err(GeneratorError::UnsupportedIntent),
        }
    }
    
    fn gen_counterparty_create(
        &self, 
        entity: &Option<EntityRef>, 
        jurisdiction: &Option<String>
    ) -> Result<String, GeneratorError> {
        let name = match entity {
            Some(EntityRef::Resolved { name, .. }) => name.clone(),
            Some(EntityRef::Unresolved(name)) => name.clone(),
            _ => return Err(GeneratorError::MissingField("counterparty name")),
        };
        
        let symbol = format!("@cp-{}", name.to_lowercase().replace(' ', "-"));
        let jurisdiction = jurisdiction.as_deref().unwrap_or("US");
        
        Ok(format!(
            r#"(counterparty.ensure
  :name "{}"
  :counterparty-type BANK
  :jurisdiction "{}"
  :as {})"#,
            name, jurisdiction, symbol
        ))
    }
    
    fn gen_isda_establish(
        &self,
        counterparty: &EntityRef,
        version: &Option<String>,
        law: &Option<String>,
        date: &Option<String>,
        symbols: &HashMap<String, String>,
    ) -> Result<String, GeneratorError> {
        let cp_ref = self.resolve_entity_ref(counterparty, symbols)?;
        let version = version.as_deref().unwrap_or("2002");
        let law = law.as_deref().unwrap_or("NY");
        
        let symbol = format!("@isda-{}", cp_ref.trim_start_matches("@cp-"));
        
        Ok(format!(
            r#"(isda.establish
  :cbu-id @cbu
  :counterparty-id {}
  :version "{}"
  :governing-law {}
  :as {})"#,
            cp_ref, version, law, symbol
        ))
    }
    
    fn gen_isda_add_products(
        &self,
        isda_ref: &EntityRef,
        products: &[String],
        symbols: &HashMap<String, String>,
    ) -> Result<String, GeneratorError> {
        let isda = self.resolve_entity_ref(isda_ref, symbols)?;
        
        let statements: Vec<String> = products.iter().map(|p| {
            format!(
                "(isda.add-product-scope :isda-id {} :product-type {})",
                isda, p
            )
        }).collect();
        
        Ok(statements.join("\n"))
    }
    
    fn gen_csa_establish(
        &self,
        isda_ref: &EntityRef,
        csa_type: &str,
        our_threshold: &Option<f64>,
        their_threshold: &Option<f64>,
        symbols: &HashMap<String, String>,
    ) -> Result<String, GeneratorError> {
        let isda = self.resolve_entity_ref(isda_ref, symbols)?;
        let symbol = format!("@csa-{}", isda.trim_start_matches("@isda-"));
        
        Ok(format!(
            r#"(csa.establish
  :isda-id {}
  :csa-type {}
  :our-threshold {}
  :their-threshold {}
  :mta 500000
  :as {})"#,
            isda, 
            csa_type,
            our_threshold.unwrap_or(0.0),
            their_threshold.unwrap_or(0.0),
            symbol
        ))
    }
    
    fn gen_im_assign(
        &self,
        entity: &Option<EntityRef>,
        scope: &ScopeAst,
        method: &Option<String>,
    ) -> Result<String, GeneratorError> {
        let name = match entity {
            Some(EntityRef::Resolved { name, .. }) => name.clone(),
            Some(EntityRef::Unresolved(name)) => name.clone(),
            _ => return Err(GeneratorError::MissingField("investment manager name")),
        };
        
        let symbol = format!("@im-{}", name.to_lowercase().replace(' ', "-"));
        
        let mut dsl = format!(
            r#"(investment-manager.assign
  :cbu-id @cbu
  :manager-name "{}"
  :manager-type INSTITUTIONAL"#,
            name
        );
        
        if !scope.markets.is_empty() {
            dsl.push_str(&format!("\n  :scope-markets [{}]", scope.markets.join(" ")));
        }
        
        if !scope.instruments.is_empty() {
            dsl.push_str(&format!("\n  :scope-instrument-classes [{}]", scope.instruments.join(" ")));
        }
        
        if let Some(m) = method {
            dsl.push_str(&format!("\n  :instruction-method {}", m));
        }
        
        dsl.push_str(&format!("\n  :as {})", symbol));
        
        Ok(dsl)
    }
    
    fn gen_pricing_set(
        &self,
        source: &str,
        instruments: &[String],
    ) -> Result<String, GeneratorError> {
        if instruments.is_empty() {
            Ok(format!(
                r#"(pricing-config.set
  :cbu-id @cbu
  :source {}
  :priority 1)"#,
                source.to_uppercase()
            ))
        } else {
            let statements: Vec<String> = instruments.iter().map(|inst| {
                format!(
                    r#"(pricing-config.set
  :cbu-id @cbu
  :instrument-class {}
  :source {}
  :priority 1)"#,
                    inst, source.to_uppercase()
                )
            }).collect();
            Ok(statements.join("\n\n"))
        }
    }
    
    fn gen_confirmation_configure(
        &self,
        counterparty: &Option<EntityRef>,
        method: &str,
        products: &[String],
        symbols: &HashMap<String, String>,
    ) -> Result<String, GeneratorError> {
        let cp_ref = if let Some(cp) = counterparty {
            Some(self.resolve_entity_ref(cp, symbols)?)
        } else {
            None
        };
        
        if products.is_empty() {
            let mut dsl = "(confirmation.configure\n  :cbu-id @cbu".to_string();
            if let Some(ref cp) = cp_ref {
                dsl.push_str(&format!("\n  :counterparty-id {}", cp));
            }
            dsl.push_str(&format!("\n  :method {}\n  :auto-match true)", method));
            Ok(dsl)
        } else {
            let statements: Vec<String> = products.iter().map(|p| {
                let mut dsl = "(confirmation.configure\n  :cbu-id @cbu".to_string();
                if let Some(ref cp) = cp_ref {
                    dsl.push_str(&format!("\n  :counterparty-id {}", cp));
                }
                dsl.push_str(&format!("\n  :product-type {}\n  :method {}\n  :auto-match true)", p, method));
                dsl
            }).collect();
            Ok(statements.join("\n\n"))
        }
    }
    
    fn resolve_entity_ref(
        &self, 
        entity_ref: &EntityRef, 
        symbols: &HashMap<String, String>
    ) -> Result<String, GeneratorError> {
        match entity_ref {
            EntityRef::Resolved { name, .. } => {
                // Look for existing symbol
                let expected_symbol = format!("@cp-{}", name.to_lowercase().replace(' ', "-"));
                if symbols.contains_key(&expected_symbol) {
                    Ok(expected_symbol)
                } else {
                    Ok(expected_symbol) // Will be created
                }
            }
            EntityRef::Symbol(s) => Ok(s.clone()),
            EntityRef::Context => {
                // Find most recent relevant symbol
                // For now, just error
                Err(GeneratorError::UnresolvedContext)
            }
            EntityRef::Unresolved(name) => {
                Ok(format!("@cp-{}", name.to_lowercase().replace(' ', "-")))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("Query intents don't generate DSL")]
    QueryIntent,
    #[error("Negated intents not supported")]
    NegatedIntent,
    #[error("Unsupported intent type")]
    UnsupportedIntent,
    #[error("Could not resolve context reference")]
    UnresolvedContext,
}
```

### Task 3.4.3: Module Organization

**File:** `rust/src/agentic/lexicon/mod.rs` (new)

```rust
//! Lexicon-based NLU components

mod tokens;
mod loader;
mod tokenizer;
mod db_resolver;
mod intent_ast;
mod intent_parser;

pub use tokens::*;
pub use loader::{Lexicon, LexiconError};
pub use tokenizer::{Tokenizer, EntityResolver, SessionContext, SalientEntity, ResolvedEntity};
pub use db_resolver::DatabaseEntityResolver;
pub use intent_ast::*;
pub use intent_parser::{parse_intent, parse_with_recovery, ParseError, ParseErrorKind};
```

**File:** `rust/src/agentic/mod.rs` (update)

```rust
//! Agent Intelligence Module
//!
//! Lexicon-based NLU with formal grammar parsing.

pub mod lexicon;
mod pipeline;
mod dsl_generator;

pub use pipeline::{AgentPipeline, AgentResponse, ResponseType, PipelineError};
pub use dsl_generator::{DslGenerator, GeneratorError};
```

**Verification:**
```bash
cargo build --lib
cargo test --lib agentic
```

---

## Phase 3.5: OTC Derivatives Domain

**Goal:** Add data model and DSL verbs for OTC derivatives.

**Duration:** 4-5 days

**Note:** This phase is unchanged from original TODO - the data model and verb definitions don't depend on the classification approach.

### Task 3.5.1: Database Migrations

See original TODO for full migration SQL. Key tables:
- `counterparty`
- `isda_master_agreement`
- `isda_product_scope`
- `credit_support_annex`
- `csa_eligible_collateral`
- `collateral_account`
- `confirmation_config`

### Task 3.5.2: DSL Verb Configs

Create YAML files in `rust/config/verbs/`:
- `counterparty.yaml`
- `isda.yaml`
- `csa.yaml`
- `collateral.yaml`
- `confirmation.yaml`

See original TODO for full YAML content.

**Verification:**
```bash
sqlx migrate run
cargo test --lib test_verb_configs_load
```

---

## Phase 3.6: Integration Checkpoint

**Goal:** Verify complete round-trip: NL → Tokens → IntentAst → DSL → Execute → DB

**Duration:** 3-4 days

### Task 3.6.1: Integration Tests

**File:** `rust/tests/lexicon_integration_test.rs` (new)

```rust
//! Integration tests for lexicon-based pipeline

use ob_poc::agentic::{AgentPipeline, ResponseType};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test]
async fn test_counterparty_create_round_trip(pool: PgPool) {
    let pipeline = AgentPipeline::from_config_dir(
        std::path::Path::new("config/agent"),
        Some(pool.clone())
    ).await.unwrap();
    
    let session_id = Uuid::new_v4();
    
    // Natural language input
    let response = pipeline.process(
        "Add Goldman Sachs as a counterparty",
        session_id
    ).await.unwrap();
    
    // Should generate DSL
    assert!(response.dsl.is_some());
    assert!(response.dsl.as_ref().unwrap().contains("counterparty.ensure"));
    assert!(response.dsl.as_ref().unwrap().contains("Goldman Sachs"));
    
    // Should execute
    assert_eq!(response.response_type, ResponseType::Execution);
    
    // Verify in database
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM counterparty WHERE name = 'Goldman Sachs'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count.0, 1);
}

#[sqlx::test]
async fn test_article_handling(pool: PgPool) {
    let pipeline = AgentPipeline::from_config_dir(
        std::path::Path::new("config/agent"),
        Some(pool.clone())
    ).await.unwrap();
    
    let session_id = Uuid::new_v4();
    
    // With article "a" - should work
    let response = pipeline.process(
        "Add Goldman Sachs as a counterparty",
        session_id
    ).await.unwrap();
    assert!(response.dsl.is_some());
    
    // With article "the" - should work
    let response = pipeline.process(
        "Add JP Morgan as the counterparty",
        session_id
    ).await.unwrap();
    assert!(response.dsl.is_some());
    
    // Without article - should work
    let response = pipeline.process(
        "Add Morgan Stanley as counterparty",
        session_id
    ).await.unwrap();
    assert!(response.dsl.is_some());
}

#[sqlx::test]
async fn test_word_order_flexibility(pool: PgPool) {
    let pipeline = AgentPipeline::from_config_dir(
        std::path::Path::new("config/agent"),
        Some(pool.clone())
    ).await.unwrap();
    
    let session_id = Uuid::new_v4();
    
    // "Add X as Y" order
    let r1 = pipeline.process("Add BlackRock as investment manager", session_id).await.unwrap();
    assert!(r1.dsl.is_some());
    
    // "Add Y X" order (less common but should work)
    let r2 = pipeline.process("Add investment manager PIMCO", session_id).await.unwrap();
    assert!(r2.dsl.is_some());
}

#[sqlx::test]
async fn test_isda_full_flow(pool: PgPool) {
    let mut pipeline = AgentPipeline::from_config_dir(
        std::path::Path::new("config/agent"),
        Some(pool.clone())
    ).await.unwrap();
    
    let session_id = Uuid::new_v4();
    
    // Step 1: Create counterparty
    pipeline.process("Add Goldman Sachs as counterparty", session_id).await.unwrap();
    
    // Step 2: Establish ISDA
    let r2 = pipeline.process(
        "Establish 2002 ISDA with Goldman under NY law",
        session_id
    ).await.unwrap();
    assert!(r2.dsl.is_some());
    assert!(r2.dsl.as_ref().unwrap().contains("isda.establish"));
    
    // Step 3: Add products
    let r3 = pipeline.process(
        "Add IRS and CDS to the ISDA",
        session_id
    ).await.unwrap();
    assert!(r3.dsl.is_some());
    
    // Step 4: Set up CSA - should resolve from context
    let r4 = pipeline.process(
        "Set up VM CSA with zero threshold",
        session_id
    ).await.unwrap();
    assert!(r4.dsl.is_some());
    assert!(r4.dsl.as_ref().unwrap().contains("csa.establish"));
}

#[sqlx::test]
async fn test_coreference_resolution(pool: PgPool) {
    let mut pipeline = AgentPipeline::from_config_dir(
        std::path::Path::new("config/agent"),
        Some(pool.clone())
    ).await.unwrap();
    
    let session_id = Uuid::new_v4();
    
    // Create entity
    pipeline.process("Add BlackRock as investment manager", session_id).await.unwrap();
    
    // Reference with pronoun
    let response = pipeline.process(
        "Set their scope to European equities",
        session_id
    ).await.unwrap();
    
    // Should resolve "their" to BlackRock
    assert!(response.dsl.is_some());
    assert!(response.dsl.as_ref().unwrap().contains("blackrock"));
}

#[sqlx::test]
async fn test_unknown_token_handling(pool: PgPool) {
    let pipeline = AgentPipeline::from_config_dir(
        std::path::Path::new("config/agent"),
        Some(pool.clone())
    ).await.unwrap();
    
    let session_id = Uuid::new_v4();
    
    // Complete gibberish should ask for clarification
    let response = pipeline.process(
        "xyzzy foobar baz",
        session_id
    ).await.unwrap();
    
    assert_eq!(response.response_type, ResponseType::Clarification);
    assert!(response.clarification.is_some());
}
```

### Task 3.6.2: Evaluation Runner

**File:** `rust/src/bin/evaluate_agent.rs` (new)

```rust
//! Evaluate agent accuracy on test cases

use ob_poc::agentic::AgentPipeline;
use std::path::Path;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct EvalCase {
    id: String,
    input: String,
    expected_intent: String,
    expected_dsl_contains: Vec<String>,
}

#[derive(serde::Deserialize)]
struct EvalDataset {
    cases: Vec<EvalCase>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pipeline = AgentPipeline::from_config_dir(
        Path::new("config/agent"),
        None
    ).await?;
    
    let dataset: EvalDataset = serde_yaml::from_reader(
        std::fs::File::open("config/agent/evaluation_dataset.yaml")?
    )?;
    
    let mut passed = 0;
    let mut failed = 0;
    
    for case in &dataset.cases {
        let session_id = Uuid::new_v4();
        let response = pipeline.process(&case.input, session_id).await?;
        
        let dsl_ok = if let Some(ref dsl) = response.dsl {
            case.expected_dsl_contains.iter().all(|s| dsl.contains(s))
        } else {
            false
        };
        
        if dsl_ok {
            passed += 1;
            println!("✓ {}", case.id);
        } else {
            failed += 1;
            println!("✗ {} - Expected DSL containing {:?}, got {:?}", 
                case.id, case.expected_dsl_contains, response.dsl);
        }
    }
    
    let accuracy = passed as f64 / (passed + failed) as f64 * 100.0;
    println!("\nAccuracy: {:.1}% ({}/{})", accuracy, passed, passed + failed);
    
    if accuracy < 85.0 {
        std::process::exit(1);
    }
    
    Ok(())
}
```

**Verification:**
```bash
cargo test --test lexicon_integration_test
cargo run --bin evaluate_agent
# Should show > 85% accuracy
```

---

## Verification & Demo

### Verification Commands

```bash
# Phase 3.1: Lexicon
cargo test --lib lexicon::loader
cargo test --lib lexicon::tokens

# Phase 3.2: Tokenizer
cargo test --lib lexicon::tokenizer

# Phase 3.3: Intent Parser
cargo test --lib lexicon::intent_parser

# Phase 3.4: Pipeline
cargo test --lib agentic::pipeline
cargo build --lib

# Phase 3.5: OTC
sqlx migrate run
cargo test --lib test_verb_configs_load

# Phase 3.6: Integration
cargo test --test lexicon_integration_test
cargo run --bin evaluate_agent
```

### Manual Demo

```
1. Start: cargo run --bin agentic_server

2. Test article handling (was broken):
   > "Add Goldman Sachs as a counterparty"
   → Should parse, generate DSL, execute

3. Test word variations:
   > "Establish ISDA with Goldman"
   > "Set up ISDA master with Goldman"
   → Both should work

4. Test OTC flow:
   > "Add Goldman as counterparty"
   > "Establish 2002 ISDA with Goldman under NY law"
   > "Add IRS and CDS to the ISDA"
   > "Set up VM CSA with zero threshold"
   > "Use MarkitWire for IRS confirmations"

5. Test coreference:
   > "Add BlackRock as investment manager"
   > "Set their scope to European equities"
   → "their" should resolve to BlackRock
```

### Sign-Off Checklist

- [ ] Lexicon YAML loads without errors
- [ ] Tokenizer handles articles (a, an, the)
- [ ] Tokenizer handles synonyms (add/create/onboard)
- [ ] Intent parser produces correct IntentAst
- [ ] DSL generator produces valid DSL
- [ ] Integration tests pass
- [ ] Evaluation accuracy > 85%
- [ ] Coreference resolution works
- [ ] OTC domain fully functional
- [ ] Unknown tokens produce clarification, not silent failure

---

## Files Summary

### Delete (Old Regex Approach)
```
rust/src/agentic/intent_classifier.rs  ← REMOVE
rust/src/agentic/entity_extractor.rs   ← REMOVE
rust/config/agent/intent_taxonomy.yaml ← REMOVE (trigger phrases obsolete)
rust/config/agent/parameter_mappings.yaml ← REMOVE (absorbed into DSL generator)
```

### Create (New Lexicon/Grammar Approach)
```
rust/config/agent/lexicon.yaml
rust/src/agentic/lexicon/mod.rs
rust/src/agentic/lexicon/tokens.rs
rust/src/agentic/lexicon/loader.rs
rust/src/agentic/lexicon/tokenizer.rs
rust/src/agentic/lexicon/db_resolver.rs
rust/src/agentic/lexicon/intent_ast.rs
rust/src/agentic/lexicon/intent_parser.rs
rust/tests/lexicon_integration_test.rs
rust/src/bin/evaluate_agent.rs
```

### Modify
```
rust/src/agentic/mod.rs               ← Update exports
rust/src/agentic/pipeline.rs          ← Rewrite for tokenizer+parser
rust/src/agentic/dsl_generator.rs     ← Rewrite for IntentAst input
rust/Cargo.toml                       ← Add dependencies (strsim, async-trait)
```

---

## Why This Is Better

| Aspect | Old (Regex) | New (Lexicon/Grammar) |
|--------|-------------|----------------------|
| "Add X as a counterparty" | ❌ Fails | ✅ Article absorbed |
| "Goldman" vs "Goldman Sachs" | ❌ Different strings | ✅ Same entity token |
| "Set up ISDA" | ❌ Need alias pattern | ✅ Alias in lexicon |
| Word order | ❌ Need multiple patterns | ✅ Grammar handles |
| Unknown entity | ❌ Silent fail | ✅ Clarification |
| Error messages | ❌ "No match" | ✅ "Expected ROLE after ENTITY" |
| Testability | ❌ Test regex strings | ✅ Test token stream + AST |
| Accuracy | 49% | Target: 85%+ |
