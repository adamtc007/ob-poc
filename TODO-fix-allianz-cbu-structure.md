# TODO: Fix Allianz Fund CBU Structure

**Created:** 2026-01-01
**Status:** READY FOR EXECUTION
**Priority:** HIGH
**Blocks:** Allianz data load completion

---

## Problem Statement

The current `gleif_load.rs` generates incorrect CBU structure:

### Current (WRONG)
```rust
// Creates fund entity, then assigns fund as its own SICAV - WRONG!
"(cbu.assign-role".to_string(),
format!("    :cbu-id {}", cbu_alias),
format!("    :entity-id {}", entity_alias),  // Fund is NOT its own SICAV!
"    :role \"SICAV\")".to_string(),
```

### Issues
1. **SICAV role wrong** - Assigns fund as its own SICAV (should be umbrella entity)
2. **Missing Ultimate Client** - No Allianz SE role assignment
3. **Only 10 funds** - `managed_funds_sample` only had 10, not all 417
4. **No umbrella entity creation** - SICAVs need to be created as entities first

---

## Correct CBU Structure

### For Sub-Fund (has umbrella_lei)
```
CBU: "Allianz EuropEquity Crescendo" (sub-fund)
│
├── Client/Asset Owner Entity
│   └── 529900A4... (the sub-fund itself)
│
├── SICAV Role
│   └── 4KT8DCRL... (umbrella SICAV entity - separate!)
│
├── Investment Manager Role
│   └── OJ2TIQSV... (AllianzGI GmbH)
│
├── ManCo Role
│   └── OJ2TIQSV... (AllianzGI - or separate ManCo if known)
│
└── Ultimate Client Role
    └── 529900K9... (Allianz SE)
```

### For Umbrella SICAV (is_umbrella = true, no umbrella_lei)
```
CBU: "Allianz Global Investors Fund" (umbrella SICAV itself)
│
├── Client/Asset Owner Entity
│   └── 4KT8DCRL... (the SICAV umbrella)
│
├── (NO SICAV Role - it IS the SICAV)
│
├── Investment Manager Role
│   └── OJ2TIQSV... (AllianzGI GmbH)
│
├── ManCo Role
│   └── OJ2TIQSV... (AllianzGI)
│
└── Ultimate Client Role
    └── 529900K9... (Allianz SE)
```

### For Standalone Fund (no umbrella, not itself an umbrella)
```
CBU: "Aktien Dividende Global" (standalone fund)
│
├── Client/Asset Owner Entity
│   └── 529900NS... (the fund)
│
├── (NO SICAV Role - standalone)
│
├── Investment Manager Role
│   └── OJ2TIQSV... (AllianzGI GmbH)
│
├── ManCo Role
│   └── OJ2TIQSV... (AllianzGI)
│
└── Ultimate Client Role
    └── 529900K9... (Allianz SE)
```

---

## Data Source

Use the new complete data file:
```
/Users/adamtc007/Developer/ob-poc/data/derived/gleif/allianzgi_funds_complete.json
```

Structure:
```json
{
  "investment_manager": { "lei": "OJ2TIQSV...", "name": "AllianzGI" },
  "ultimate_client": { "lei": "529900K9...", "name": "Allianz SE" },
  "total_funds": 417,
  "umbrella_count": 29,
  "unique_umbrella_leis": ["4KT8DCRL...", ...],
  "funds": [
    {
      "lei": "529900A4...",
      "name": "Allianz EuropEquity Crescendo",
      "jurisdiction": "LU",
      "umbrella_lei": "4KT8DCRL...",      // null if no umbrella
      "umbrella_name": "Allianz Global Investors Fund",
      "is_umbrella": false                 // true if this fund IS an umbrella
    }
  ]
}
```

---

## Implementation Changes

### 1. Update `gleif_load.rs` Types

Add to fund struct:
```rust
#[derive(Debug, Deserialize)]
pub struct Fund {
    pub lei: String,
    pub name: String,
    pub jurisdiction: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub legal_form: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub umbrella_lei: Option<String>,    // NEW
    #[serde(default)]
    pub umbrella_name: Option<String>,   // NEW
    #[serde(default)]
    pub is_umbrella: bool,               // NEW
}

#[derive(Debug, Deserialize)]
pub struct FundsComplete {
    pub investment_manager: EntityRef,
    pub ultimate_client: EntityRef,
    pub total_funds: usize,
    pub umbrella_count: usize,
    pub unique_umbrella_leis: Vec<String>,
    pub funds: Vec<Fund>,
}

#[derive(Debug, Deserialize)]
pub struct EntityRef {
    pub lei: String,
    pub name: String,
}
```

### 2. Update DSL Generation Order

```rust
pub fn generate_full_dsl(...) -> Result<String> {
    // Phase 1: Parent entities (Allianz SE, AllianzGI)
    // ... existing code ...

    // Phase 2: Umbrella SICAV entities FIRST
    // (Must exist before sub-funds reference them)
    let umbrella_funds: Vec<_> = funds_data.funds.iter()
        .filter(|f| f.is_umbrella)
        .collect();
    
    for umbrella in &umbrella_funds {
        dsl_parts.push(generate_entity_dsl(umbrella));
    }

    // Phase 3: All fund entities (including sub-funds)
    for fund in &funds_data.funds {
        if !fund.is_umbrella {  // Skip umbrellas, already created
            dsl_parts.push(generate_entity_dsl(fund));
        }
    }

    // Phase 4: CBUs with correct role assignments
    for fund in &funds_data.funds {
        dsl_parts.push(generate_fund_cbu_dsl(
            fund,
            &funds_data.investment_manager,
            &funds_data.ultimate_client,
        ));
    }
}
```

### 3. Fix `generate_fund_cbu_dsl`

```rust
fn generate_fund_cbu_dsl(
    fund: &Fund,
    im: &EntityRef,
    ultimate_client: &EntityRef,
) -> String {
    let entity_alias = lei_to_alias(&fund.lei);
    let cbu_alias = format!("@cbu_{}", fund.lei.to_lowercase());
    let im_alias = lei_to_alias(&im.lei);
    let uc_alias = lei_to_alias(&ultimate_client.lei);

    let mut lines = vec![
        format!(";; CBU for: {}", fund.name),
        format!(""),
        // Create CBU
        "(cbu.ensure".to_string(),
        format!("    :name \"{}\"", escape_dsl_string(&fund.name)),
        "    :client-type \"FUND\"".to_string(),
        format!("    :jurisdiction \"{}\"", fund.jurisdiction),
        format!("    :as {})", cbu_alias),
        format!(""),
    ];

    // Investment Manager role
    lines.extend(vec![
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", im_alias),
        "    :role \"INVESTMENT_MANAGER\")".to_string(),
        format!(""),
    ]);

    // ManCo role (using IM as ManCo for AllianzGI self-managed funds)
    lines.extend(vec![
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", im_alias),
        "    :role \"MANAGEMENT_COMPANY\")".to_string(),
        format!(""),
    ]);

    // SICAV role - ONLY for sub-funds with umbrella, NOT for umbrellas themselves
    if let Some(ref umbrella_lei) = fund.umbrella_lei {
        if !fund.is_umbrella {
            let sicav_alias = lei_to_alias(umbrella_lei);
            lines.extend(vec![
                format!(";; SICAV: {} (umbrella)", fund.umbrella_name.as_deref().unwrap_or("Unknown")),
                "(cbu.assign-role".to_string(),
                format!("    :cbu-id {}", cbu_alias),
                format!("    :entity-id {}", sicav_alias),
                "    :role \"SICAV\")".to_string(),
                format!(""),
            ]);
        }
    }

    // Ultimate Client role - Allianz SE
    lines.extend(vec![
        format!(";; Ultimate Client: {}", ultimate_client.name),
        "(cbu.assign-role".to_string(),
        format!("    :cbu-id {}", cbu_alias),
        format!("    :entity-id {}", uc_alias),
        "    :role \"ULTIMATE_CLIENT\")".to_string(),
    ]);

    lines.join("\n")
}
```

---

## DSL Verb Requirements

Ensure these verbs/roles exist:

| Verb | Purpose |
|------|---------|
| `entity.ensure-limited-company` | Create fund entity with LEI |
| `cbu.ensure` | Create/upsert CBU |
| `cbu.assign-role :role "INVESTMENT_MANAGER"` | Link IM to CBU |
| `cbu.assign-role :role "MANAGEMENT_COMPANY"` | Link ManCo to CBU |
| `cbu.assign-role :role "SICAV"` | Link umbrella SICAV to sub-fund CBU |
| `cbu.assign-role :role "ULTIMATE_CLIENT"` | Link Allianz SE to CBU |

---

## Execution Order

```
1. Create Allianz SE entity
2. Create AllianzGI GmbH entity
3. Create umbrella SICAV entities (29)
4. Create sub-fund/standalone fund entities (388)
5. For each fund:
   a. Create CBU
   b. Assign Investment Manager role → AllianzGI
   c. Assign ManCo role → AllianzGI
   d. If has umbrella: Assign SICAV role → umbrella entity
   e. Assign Ultimate Client role → Allianz SE
```

---

## Expected Counts After Load

| Entity Type | Count |
|-------------|-------|
| Allianz SE | 1 |
| AllianzGI GmbH | 1 |
| AllianzGI Subsidiaries (US, JP) | 2 |
| Umbrella SICAV funds | 29 |
| Sub-funds / standalone funds | 388 |
| **Total fund entities** | **417** |
| **Total CBUs** | **417** |
| Investment Manager roles | 417 |
| ManCo roles | 417 |
| SICAV roles | ~300 (only sub-funds with umbrellas) |
| Ultimate Client roles | 417 |

---

## Verification Queries

```sql
-- Count fund entities
SELECT COUNT(*) FROM "ob-poc".entity_limited_companies 
WHERE gleif_category = 'FUND';
-- Expected: 417

-- Count CBUs
SELECT COUNT(*) FROM "ob-poc".client_business_units 
WHERE client_type = 'FUND';
-- Expected: 417

-- Count role assignments by type
SELECT role_type, COUNT(*) 
FROM "ob-poc".cbu_roles 
GROUP BY role_type;
-- Expected: INVESTMENT_MANAGER: 417, MANAGEMENT_COMPANY: 417, SICAV: ~300, ULTIMATE_CLIENT: 417

-- Verify SICAV role points to different entity than the fund itself
SELECT 
    c.name as cbu_name,
    e.lei as cbu_lei,
    r.role_type,
    re.lei as role_entity_lei,
    re.company_name as role_entity_name
FROM "ob-poc".cbu_roles r
JOIN "ob-poc".client_business_units c ON r.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON c.client_entity_id = e.entity_id
JOIN "ob-poc".entities re ON r.entity_id = re.entity_id
WHERE r.role_type = 'SICAV'
LIMIT 10;
-- CRITICAL: role_entity_lei should be DIFFERENT from cbu_lei (umbrella vs sub-fund)

-- Verify all CBUs have Ultimate Client role
SELECT c.name 
FROM "ob-poc".client_business_units c
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".cbu_roles r 
    WHERE r.cbu_id = c.cbu_id AND r.role_type = 'ULTIMATE_CLIENT'
);
-- Expected: empty (all CBUs should have Ultimate Client)
```

---

## Files to Update

1. `rust/xtask/src/gleif_load.rs` - Fix DSL generation
2. Load new data file: `data/derived/gleif/allianzgi_funds_complete.json`
3. Regenerate DSL: `cargo xtask gleif-load --execute`

---

## Summary

**Root Cause:** Original script only fetched 10 sample funds, and assigned fund as its own SICAV instead of linking to umbrella entity.

**Fix:** 
1. Use complete fund data (417 funds)
2. Create umbrella SICAV entities FIRST
3. SICAV role points to umbrella entity, not fund itself
4. Add Ultimate Client role (Allianz SE) to all CBUs
