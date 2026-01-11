# Luxembourg Source Loader

> **Status:** Planning
> **Priority:** High - Major fund domicile
> **Created:** 2026-01-10
> **Estimated Effort:** 16-20 hours
> **Dependencies:** 
>   - 021-pluggable-research-source-loaders.md (SourceLoader trait)
>   - Existing GLEIF loader (Lux entities with LEI)

---

## The Problem

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LUXEMBOURG DATA ACCESS REALITY                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  SOURCE              ACCESS          TESTABLE IN POC?                       │
│  ──────────────────────────────────────────────────────────────────────────│
│  CSSF Fund Lists     Free/Public     ✅ YES                                  │
│  GLEIF (LU filter)   Free/Public     ✅ YES                                  │
│  RCS API             Paid contract   ❌ NO - €0.50-2/query                   │
│  RBE (UBO)           FI registration ❌ NO - needs BNY registration          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Strategy:** Build what's testable, stub what's not, make stubs production-ready.

---

## What We CAN Test

### 1. CSSF Fund Lists (Public)

```
URL: https://www.cssf.lu/en/document-category/list-of-ucis/
Format: Excel/CSV downloads
Updated: Monthly

Contains:
• Fund name (Part I UCITS, Part II UCIs, SIFs, RAIFs)
• Sub-fund names
• Management company
• Depositary
• Launch date
• Status (active/liquidating)
```

**Sample CSSF data structure:**

```
Fund Name                          | ManCo                    | Type    | Status
───────────────────────────────────┼──────────────────────────┼─────────┼────────
Allianz Global Investors Fund      | Allianz Global Inv GmbH  | SICAV   | Active
├── Allianz Europe Equity Growth   |                          | Sub     | Active
├── Allianz Euro Bond              |                          | Sub     | Active
└── Allianz Dynamic Multi Asset    |                          | Sub     | Active
```

### 2. GLEIF Luxembourg Entities

```
Filter: jurisdiction = "LU" OR legal_address.country = "LU"
Coverage: ~4,000 Lux entities with LEI

Provides:
• Legal name
• LEI
• Direct parent LEI
• Ultimate parent LEI  
• Entity category (FUND, GENERAL)
• Registration number (RCS number)
```

---

## What We STUB (Production-Ready)

### 3. RCS API (Company Register)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RCS API - STUB WITH REALISTIC INTERFACE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Production endpoint: https://www.lbr.lu/mjrcs-api/                         │
│  Auth: API key + contract                                                   │
│  Cost: €0.50-2.00 per query                                                 │
│                                                                              │
│  Stub behavior:                                                             │
│  • Accept RCS number (e.g., "B123456")                                      │
│  • Return SourceUnavailable { reason: REQUIRES_CONTRACT }                   │
│  • Log: "RCS API requires LBR contract - see docs/lux-setup.md"            │
│  • Include sample response structure for production impl                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Stub response:**

```rust
// When RCS API is called without credentials
SourceResult::Unavailable {
    source: "luxembourg-rcs",
    reason: SourceUnavailableReason::RequiresContract,
    message: "RCS API requires contract with Luxembourg Business Registers",
    setup_url: "https://www.lbr.lu/mjrcs-api/",
    fallback_sources: vec!["gleif", "cssf-lists"],
    sample_response: Some(RCS_SAMPLE_RESPONSE), // For integration testing
}
```

### 4. RBE (Beneficial Ownership Register)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RBE - STUB WITH REALISTIC INTERFACE                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Production endpoint: https://www.lbr.lu/rbe (authenticated portal)         │
│  Auth: Entity registration as "obliged entity" under AML                    │
│  Access: Financial institutions qualify automatically                        │
│                                                                              │
│  Stub behavior:                                                             │
│  • Accept RCS number                                                        │
│  • Return SourceUnavailable { reason: REQUIRES_AUTHORIZATION }              │
│  • Generate outreach task if enabled                                        │
│  • Include sample BO structure for production impl                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Stub response:**

```rust
SourceResult::Unavailable {
    source: "luxembourg-rbe",
    reason: SourceUnavailableReason::RequiresAuthorization,
    message: "RBE requires registration as obliged entity under Luxembourg AML law",
    setup_url: "https://www.lbr.lu/rbe",
    legitimate_interest_categories: vec![
        "credit_institution",
        "investment_fund_manager", 
        "insurance_undertaking",
    ],
    fallback_sources: vec!["gleif"],
    sample_response: Some(RBE_SAMPLE_RESPONSE),
}
```

---

## Sample Response Structures (For Testing)

Even though we can't hit production APIs, we define exact response shapes:

```rust
// rust/src/research/sources/luxembourg/samples.rs

/// Sample RCS API response - based on LBR documentation
pub const RCS_SAMPLE_RESPONSE: &str = r#"{
    "rcs_number": "B123456",
    "name": "Example Fund SICAV",
    "legal_form": "Société anonyme",
    "legal_form_code": "SA",
    "registered_office": {
        "street": "2, Boulevard Konrad Adenauer",
        "postal_code": "1115",
        "city": "Luxembourg"
    },
    "incorporation_date": "2015-03-15",
    "share_capital": {
        "amount": 31000,
        "currency": "EUR"
    },
    "status": "ACTIVE",
    "managers": [
        {
            "name": "John Smith",
            "role": "Administrateur",
            "appointed_date": "2015-03-15"
        }
    ],
    "purpose": "Investment fund activities..."
}"#;

/// Sample RBE response - based on AMLD5 required fields
pub const RBE_SAMPLE_RESPONSE: &str = r#"{
    "entity_rcs": "B123456",
    "entity_name": "Example Fund SICAV",
    "beneficial_owners": [
        {
            "name": "Jane Doe",
            "date_of_birth": "1970-06",
            "nationality": "DE",
            "country_of_residence": "LU",
            "nature_of_control": "DIRECT_OWNERSHIP",
            "extent_of_interest": {
                "percentage_low": 25,
                "percentage_high": 50
            },
            "date_of_entry": "2020-01-15"
        }
    ],
    "last_updated": "2024-06-01"
}"#;
```

---

## Implementation

### Phase 1: CSSF Fund Lists Loader (6h) ✅ TESTABLE

```rust
// rust/src/research/sources/luxembourg/cssf.rs

pub struct CssfLoader {
    /// Cached fund list (refreshed on demand)
    fund_cache: RwLock<Option<CssfFundList>>,
    cache_url: String,
}

impl CssfLoader {
    pub fn new() -> Self {
        Self {
            fund_cache: RwLock::new(None),
            // Public Excel download
            cache_url: "https://www.cssf.lu/wp-content/uploads/UCI_full_list.xlsx".into(),
        }
    }
    
    /// Download and parse CSSF fund list
    pub async fn refresh_cache(&self) -> Result<()> {
        let bytes = reqwest::get(&self.cache_url).await?.bytes().await?;
        let funds = parse_cssf_excel(&bytes)?;
        *self.fund_cache.write().await = Some(funds);
        Ok(())
    }
    
    /// Search funds by name
    pub async fn search_funds(&self, query: &str) -> Result<Vec<CssfFund>> {
        let cache = self.fund_cache.read().await;
        let funds = cache.as_ref().ok_or(anyhow!("Cache not loaded"))?;
        
        Ok(funds.search(query))
    }
    
    /// Get fund by exact name or RCS number
    pub async fn get_fund(&self, key: &str) -> Result<Option<CssfFund>> {
        let cache = self.fund_cache.read().await;
        let funds = cache.as_ref().ok_or(anyhow!("Cache not loaded"))?;
        
        Ok(funds.get(key))
    }
}

#[derive(Debug, Clone)]
pub struct CssfFund {
    pub name: String,
    pub fund_type: CssfFundType,
    pub management_company: String,
    pub depositary: Option<String>,
    pub launch_date: Option<NaiveDate>,
    pub status: FundStatus,
    pub sub_funds: Vec<CssfSubFund>,
    pub rcs_number: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CssfFundType {
    UcitsPart1,      // UCITS
    UciPart2,        // Non-UCITS UCI
    Sif,             // Specialized Investment Fund
    Raif,            // Reserved AIF
    Sicar,           // Investment company in risk capital
    Unknown(String),
}
```

### Phase 2: GLEIF Luxembourg Filter (4h) ✅ TESTABLE

```rust
// Extend existing GLEIF loader with Lux-specific queries

impl GleifLoader {
    /// Search Luxembourg entities specifically
    pub async fn search_luxembourg(&self, query: &str) -> Result<Vec<SearchCandidate>> {
        // GLEIF API supports jurisdiction filter
        let url = format!(
            "{}/lei-records?filter[entity.legalAddress.country]=LU&filter[entity.legalName]=*{}*",
            self.base_url,
            urlencoding::encode(query)
        );
        
        self.search_with_url(&url).await
    }
    
    /// Get all Luxembourg funds (FUND category)
    pub async fn list_luxembourg_funds(&self, page: usize) -> Result<Vec<LeiRecord>> {
        let url = format!(
            "{}/lei-records?filter[entity.legalAddress.country]=LU&filter[entity.category]=FUND&page[number]={}",
            self.base_url,
            page
        );
        
        self.fetch_records(&url).await
    }
    
    /// Cross-reference CSSF fund with GLEIF
    pub async fn enrich_cssf_fund(&self, fund: &CssfFund) -> Result<Option<LeiRecord>> {
        // Try by name first
        let candidates = self.search_luxembourg(&fund.name).await?;
        
        if candidates.len() == 1 {
            return self.get_by_lei(&candidates[0].key).await.map(Some);
        }
        
        // Try by RCS number if available
        if let Some(rcs) = &fund.rcs_number {
            let by_reg = self.search_by_registration_number(rcs, "LU").await?;
            if let Some(lei) = by_reg.first() {
                return self.get_by_lei(&lei.key).await.map(Some);
            }
        }
        
        Ok(None)
    }
}
```

### Phase 3: RCS Stub (3h) 🔶 STUB ONLY

```rust
// rust/src/research/sources/luxembourg/rcs.rs

pub struct RcsLoader {
    /// API credentials (None = stub mode)
    credentials: Option<RcsCredentials>,
}

impl RcsLoader {
    pub fn new() -> Self {
        // Check for credentials in env
        let credentials = match (
            std::env::var("LUX_RCS_API_KEY"),
            std::env::var("LUX_RCS_CONTRACT_ID"),
        ) {
            (Ok(key), Ok(contract)) => Some(RcsCredentials { key, contract }),
            _ => None,
        };
        
        Self { credentials }
    }
    
    pub fn is_stub(&self) -> bool {
        self.credentials.is_none()
    }
}

#[async_trait]
impl SourceLoader for RcsLoader {
    fn source_id(&self) -> &'static str { "luxembourg-rcs" }
    fn source_name(&self) -> &'static str { "Luxembourg RCS (Company Register)" }
    fn jurisdictions(&self) -> &[&'static str] { &["LU"] }
    fn key_type(&self) -> &'static str { "RCS_NUMBER" }
    
    fn provides(&self) -> &[SourceDataType] {
        &[SourceDataType::Entity, SourceDataType::Officers]
    }
    
    async fn fetch_entity(&self, rcs_number: &str) -> Result<SourceResult<NormalizedEntity>> {
        match &self.credentials {
            Some(creds) => {
                // Production implementation
                self.fetch_from_api(rcs_number, creds).await
            }
            None => {
                // Stub mode
                Ok(SourceResult::Unavailable {
                    source: self.source_id(),
                    reason: SourceUnavailableReason::RequiresContract,
                    message: "RCS API requires contract with Luxembourg Business Registers (LBR)".into(),
                    setup_info: SetupInfo {
                        url: "https://www.lbr.lu/mjrcs-api/".into(),
                        env_vars: vec!["LUX_RCS_API_KEY", "LUX_RCS_CONTRACT_ID"],
                        cost: Some("€0.50-2.00 per query".into()),
                    },
                    fallback_sources: vec!["gleif", "cssf-lists"],
                })
            }
        }
    }
    
    // ... other trait methods return Unavailable in stub mode
}
```

### Phase 4: RBE Stub (3h) 🔶 STUB ONLY

```rust
// rust/src/research/sources/luxembourg/rbe.rs

pub struct RbeLoader {
    /// Auth token (None = stub mode)
    auth: Option<RbeAuth>,
}

#[async_trait]
impl SourceLoader for RbeLoader {
    fn source_id(&self) -> &'static str { "luxembourg-rbe" }
    fn source_name(&self) -> &'static str { "Luxembourg RBE (Beneficial Ownership)" }
    fn jurisdictions(&self) -> &[&'static str] { &["LU"] }
    fn key_type(&self) -> &'static str { "RCS_NUMBER" }
    
    fn provides(&self) -> &[SourceDataType] {
        &[SourceDataType::ControlHolders]
    }
    
    async fn fetch_control_holders(&self, rcs_number: &str) 
        -> Result<SourceResult<Vec<NormalizedControlHolder>>> 
    {
        match &self.auth {
            Some(auth) => {
                // Production implementation (authenticated portal)
                self.fetch_from_portal(rcs_number, auth).await
            }
            None => {
                Ok(SourceResult::Unavailable {
                    source: self.source_id(),
                    reason: SourceUnavailableReason::RequiresAuthorization,
                    message: "RBE access requires registration as obliged entity under AML law".into(),
                    setup_info: SetupInfo {
                        url: "https://www.lbr.lu/rbe".into(),
                        requirements: vec![
                            "Entity must be registered with CSSF or equivalent",
                            "Must demonstrate legitimate interest",
                            "Financial institutions qualify automatically",
                        ],
                        env_vars: vec!["LUX_RBE_AUTH_TOKEN"],
                    },
                    fallback_sources: vec!["gleif"],
                    // Trigger outreach workflow
                    suggest_outreach: true,
                })
            }
        }
    }
}
```

---

## Composite Luxembourg Loader

Combines all sources with intelligent fallback:

```rust
// rust/src/research/sources/luxembourg/mod.rs

pub struct LuxembourgLoader {
    cssf: CssfLoader,
    gleif: Arc<GleifLoader>,  // Shared with main registry
    rcs: RcsLoader,
    rbe: RbeLoader,
}

impl LuxembourgLoader {
    /// Best-effort entity fetch with fallback chain
    pub async fn fetch_entity(&self, key: &str) -> Result<LuxEntityResult> {
        let mut result = LuxEntityResult::default();
        
        // 1. Try CSSF (free, public)
        if let Some(fund) = self.cssf.get_fund(key).await? {
            result.cssf_data = Some(fund.clone());
            result.sources_used.push("cssf");
            
            // 2. Enrich with GLEIF if fund found
            if let Some(lei_record) = self.gleif.enrich_cssf_fund(&fund).await? {
                result.gleif_data = Some(lei_record);
                result.sources_used.push("gleif");
            }
        }
        
        // 3. Try RCS (may be stub)
        match self.rcs.fetch_entity(key).await? {
            SourceResult::Success(entity) => {
                result.rcs_data = Some(entity);
                result.sources_used.push("rcs");
            }
            SourceResult::Unavailable { reason, .. } => {
                result.unavailable_sources.push(("rcs", reason));
            }
        }
        
        // 4. Try RBE for beneficial owners (may be stub)
        match self.rbe.fetch_control_holders(key).await? {
            SourceResult::Success(holders) => {
                result.beneficial_owners = holders;
                result.sources_used.push("rbe");
            }
            SourceResult::Unavailable { reason, suggest_outreach, .. } => {
                result.unavailable_sources.push(("rbe", reason));
                if suggest_outreach {
                    result.suggested_actions.push(SuggestedAction::Outreach {
                        target: key.to_string(),
                        data_needed: vec!["beneficial_owners"],
                    });
                }
            }
        }
        
        Ok(result)
    }
}

#[derive(Debug, Default)]
pub struct LuxEntityResult {
    pub cssf_data: Option<CssfFund>,
    pub gleif_data: Option<LeiRecord>,
    pub rcs_data: Option<NormalizedEntity>,
    pub beneficial_owners: Vec<NormalizedControlHolder>,
    pub sources_used: Vec<&'static str>,
    pub unavailable_sources: Vec<(&'static str, SourceUnavailableReason)>,
    pub suggested_actions: Vec<SuggestedAction>,
}
```

---

## Verb YAML

```yaml
# rust/config/verbs/research/luxembourg.yaml

domains:
  research.luxembourg:
    description: "Luxembourg fund and company data"
    
    invocation_hints:
      - "Luxembourg"
      - "Lux"
      - "SICAV"
      - "RAIF"
      - "SIF"
      - "CSSF"
      - "RCS"
    
    verbs:
      search-funds:
        description: "Search CSSF fund lists"
        invocation_phrases:
          - "search Luxembourg funds"
          - "find SICAV"
          - "CSSF lookup"
        behavior: plugin
        handler: LuxSearchFundsOp
        args:
          - name: query
            type: string
            required: true
          - name: fund-type
            type: string
            description: "UCITS, SIF, RAIF, etc."
        returns:
          type: array

      fetch-entity:
        description: "Fetch Luxembourg entity with fallback chain"
        invocation_phrases:
          - "get Luxembourg company"
          - "fetch RCS"
          - "Lux entity"
        behavior: plugin
        handler: LuxFetchEntityOp
        args:
          - name: key
            type: string
            required: true
            description: "Fund name or RCS number (B123456)"
          - name: include-bo
            type: boolean
            default: true
            description: "Attempt RBE lookup (may be unavailable)"
        returns:
          type: object

      refresh-cssf-cache:
        description: "Refresh CSSF fund list cache"
        invocation_phrases:
          - "refresh CSSF data"
          - "update Lux fund list"
        behavior: plugin
        handler: LuxRefreshCssfOp
        returns:
          type: object
```

---

## Directory Structure

```
rust/src/research/sources/luxembourg/
├── mod.rs              # LuxembourgLoader composite
├── cssf.rs             # CSSF fund list parser ✅ TESTABLE
├── gleif_lux.rs        # GLEIF Luxembourg extensions ✅ TESTABLE
├── rcs.rs              # RCS API client (stub if no creds) 🔶 STUB
├── rbe.rs              # RBE portal client (stub if no auth) 🔶 STUB
├── types.rs            # CssfFund, LuxEntityResult, etc.
├── normalize.rs        # Normalize Lux data to common structs
└── samples.rs          # Sample responses for testing

rust/config/verbs/research/
└── luxembourg.yaml     # Verb definitions
```

---

## Testing Strategy

### What's Fully Testable

```rust
#[tokio::test]
async fn test_cssf_fund_search() {
    let loader = CssfLoader::new();
    loader.refresh_cache().await.unwrap();
    
    let results = loader.search_funds("Allianz").await.unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|f| f.fund_type == CssfFundType::UcitsPart1));
}

#[tokio::test]
async fn test_gleif_luxembourg_filter() {
    let loader = GleifLoader::new();
    
    let results = loader.search_luxembourg("BlackRock").await.unwrap();
    assert!(results.iter().all(|r| r.jurisdiction == Some("LU".into())));
}
```

### Stub Testing

```rust
#[tokio::test]
async fn test_rcs_stub_returns_unavailable() {
    // No credentials set
    let loader = RcsLoader::new();
    assert!(loader.is_stub());
    
    let result = loader.fetch_entity("B123456").await.unwrap();
    
    match result {
        SourceResult::Unavailable { reason, setup_info, .. } => {
            assert_eq!(reason, SourceUnavailableReason::RequiresContract);
            assert!(setup_info.env_vars.contains(&"LUX_RCS_API_KEY"));
        }
        _ => panic!("Expected Unavailable"),
    }
}

#[tokio::test]
async fn test_rbe_stub_suggests_outreach() {
    let loader = RbeLoader::new();
    
    let result = loader.fetch_control_holders("B123456").await.unwrap();
    
    match result {
        SourceResult::Unavailable { suggest_outreach, .. } => {
            assert!(suggest_outreach);
        }
        _ => panic!("Expected Unavailable"),
    }
}
```

### Integration Test with Mocked Production Response

```rust
#[tokio::test]
async fn test_rcs_with_mocked_response() {
    // Use sample response for integration testing
    let entity = serde_json::from_str::<RcsCompany>(samples::RCS_SAMPLE_RESPONSE).unwrap();
    
    let normalized = normalize_rcs_entity(&entity);
    
    assert_eq!(normalized.name, "Example Fund SICAV");
    assert_eq!(normalized.jurisdiction, Some("LU".into()));
}
```

---

## Implementation Phases

### Phase 1: CSSF Loader (6h) ✅ TESTABLE
- [ ] 1.1 Excel/CSV parser for CSSF fund lists
- [ ] 1.2 Cache management (refresh on demand)
- [ ] 1.3 Search by name, filter by type
- [ ] 1.4 Extract sub-fund relationships
- [ ] 1.5 Tests with real CSSF data

### Phase 2: GLEIF Luxembourg (4h) ✅ TESTABLE
- [ ] 2.1 Jurisdiction filter helpers
- [ ] 2.2 CSSF → GLEIF cross-reference
- [ ] 2.3 ManCo → Fund hierarchy from parent chain
- [ ] 2.4 Tests with real GLEIF data

### Phase 3: RCS Stub (3h) 🔶 STUB
- [ ] 3.1 Define response types from LBR documentation
- [ ] 3.2 Implement stub with Unavailable response
- [ ] 3.3 Prepare production impl (commented out)
- [ ] 3.4 Sample response for testing
- [ ] 3.5 Environment variable detection

### Phase 4: RBE Stub (3h) 🔶 STUB  
- [ ] 4.1 Define BO response types from AMLD5 spec
- [ ] 4.2 Implement stub with Unavailable + outreach suggestion
- [ ] 4.3 Sample response for testing
- [ ] 4.4 Outreach task generation

### Phase 5: Composite Loader & Verbs (4h)
- [ ] 5.1 LuxembourgLoader with fallback chain
- [ ] 5.2 Verb YAML definitions
- [ ] 5.3 Handler implementations
- [ ] 5.4 Integration tests

---

## Environment Variables

```bash
# Optional - enables production RCS API
LUX_RCS_API_KEY="your-api-key"
LUX_RCS_CONTRACT_ID="your-contract-id"

# Optional - enables production RBE access
LUX_RBE_AUTH_TOKEN="your-auth-token"
```

Without these, loaders run in stub mode and return `SourceUnavailable`.

---

## Success Criteria

1. **CSSF loader works** - Can search/fetch from public fund lists
2. **GLEIF Lux filter works** - Can find Luxembourg entities by name
3. **RCS stub is production-ready** - Just needs credentials to go live
4. **RBE stub suggests outreach** - Integrates with research workflow
5. **Composite loader** - Tries all sources, reports what's unavailable
6. **Tests pass** - Real data for CSSF/GLEIF, mocked for RCS/RBE

---

Generated: 2026-01-10
