# TODO: Load Allianz GLEIF Data via DSL

**Created:** 2026-01-01
**Status:** READY FOR EXECUTION
**Priority:** HIGH

---

## Overview

Load the refreshed GLEIF data for Allianz SE corporate tree into ob-poc using DSL commands. This requires proper dependency ordering and verb validation.

---

## Source Data Files

All in `/Users/adamtc007/Developer/ob-poc/data/derived/gleif/`:

| File | Records | Description |
|------|---------|-------------|
| `allianz_se_corporate_tree.json` | 237 subsidiaries | Direct children of Allianz SE |
| `allianzgi_ownership_chain.json` | 300 funds + chain | AllianzGI managed funds |
| `allianz_level2_data.json` | 2 entities | Level 2 relationship data with corroboration |

---

## Pre-Execution Checklist

### 1. Verify GLEIF Schema Migration Applied

```bash
# Check if new GLEIF columns exist on entity_limited_companies
psql -d ob-poc -c "SELECT column_name FROM information_schema.columns 
WHERE table_schema = 'ob-poc' AND table_name = 'entity_limited_companies' 
AND column_name IN ('lei', 'gleif_status', 'gleif_category', 'gleif_validation_level', 
'direct_parent_lei', 'ultimate_parent_lei', 'gleif_next_renewal');"
```

**Expected:** All 7 columns should exist. If not, run migration first:
```bash
psql -d ob-poc -f migrations/006_gleif_entity_enhancement.sql
```

### 2. Verify GLEIF Verb Handlers Exist

Check that these verbs are implemented in the DSL executor:

```
gleif.enrich          - Populate GLEIF fields on entity
entity.ensure-limited-company - Create/update limited company with LEI
cbu.create            - Create Client Business Unit
cbu.role:assign-investment-manager - Link IM to CBU
cbu.role:assign-sicav - Link SICAV to CBU  
cbu.role:assign-manco - Link ManCo to CBU
```

**Location:** `rust/src/session/verb_rag_metadata.rs` (lines 5481-5560)

### 3. Verify entity.ensure-limited-company Supports GLEIF Fields

The verb should accept these parameters:
- `:lei` - Legal Entity Identifier
- `:gleif-status` - ACTIVE/INACTIVE
- `:gleif-category` - GENERAL/FUND/BRANCH
- `:legal-form-code` - ELF code (2HBR, SGST, etc.)
- `:gleif-validation-level` - FULLY_CORROBORATED/ENTITY_SUPPLIED_ONLY
- `:direct-parent-lei` - Parent LEI
- `:ultimate-parent-lei` - Ultimate parent LEI
- `:gleif-next-renewal` - Renewal date

---

## Execution Plan

### Phase 1: Create Parent Entities (Dependency Root)

**Order matters!** Parents must exist before children can reference them.

```dsl
;; 1. Allianz SE - Ultimate Parent (UBO Terminus)
(entity.ensure-limited-company
    :name "Allianz SE"
    :lei "529900K9B0N5BT694847"
    :jurisdiction "DE"
    :registration-number "HRB 164232"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "SGST"
    :gleif-validation-level "FULLY_CORROBORATED"
    :gleif-next-renewal "2026-12-04"
    :parent-exception "NO_KNOWN_PERSON"
    :as @allianz_se)

;; 2. Allianz Global Investors GmbH - Investment Manager
(entity.ensure-limited-company
    :name "Allianz Global Investors GmbH"
    :lei "OJ2TIQSVQND4IZYYK658"
    :jurisdiction "DE"
    :registration-number "HRB 9340"
    :city "Frankfurt am Main"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :gleif-validation-level "FULLY_CORROBORATED"
    :gleif-next-renewal "2026-08-19"
    :direct-parent-lei "529900K9B0N5BT694847"
    :ultimate-parent-lei "529900K9B0N5BT694847"
    :as @allianzgi)

;; 3. Ownership: Allianz SE -> AllianzGI
(cbu.role:assign-ownership
    :owner-entity-id @allianz_se
    :owned-entity-id @allianzgi
    :percentage 100.0
    :ownership-type "ACCOUNTING_CONSOLIDATION"
    :source "GLEIF"
    :corroboration "FULLY_CORROBORATED")
```

### Phase 2: Create AllianzGI Subsidiaries

```dsl
;; US Subsidiary
(entity.ensure-limited-company
    :name "ALLIANZ CAPITAL PARTNERS OF AMERICA LLC"
    :lei "5493005JTEV4OVDVNH32"
    :jurisdiction "US-DE"
    :direct-parent-lei "OJ2TIQSVQND4IZYYK658"
    :as @allianz_us)

;; Japan Subsidiary
(entity.ensure-limited-company
    :name "アリアンツ・グローバル・インベスターズ・ジャパン株式会社"
    :lei "353800NVWWGOB9JXQZ47"
    :jurisdiction "JP"
    :direct-parent-lei "OJ2TIQSVQND4IZYYK658"
    :as @allianz_jp)
```

### Phase 3: Create Funds as Entities + CBUs

For each of the 300 funds in `allianzgi_ownership_chain.json`:

**Pattern per fund:**

```dsl
;; Step 1: Create fund as legal entity
(entity.ensure-limited-company
    :name "{fund_name}"
    :lei "{fund_lei}"
    :jurisdiction "{jurisdiction}"
    :gleif-category "FUND"
    :as @fund_{lei_prefix})

;; Step 2: Create CBU for fund onboarding
(cbu.create
    :name "{fund_name}"
    :client-type "FUND"
    :jurisdiction "{jurisdiction}"
    :as @cbu_{lei_prefix})

;; Step 3: Assign Investment Manager role
(cbu.role:assign-investment-manager
    :cbu-id @cbu_{lei_prefix}
    :entity-id @allianzgi
    :role-type "INVESTMENT_MANAGER"
    :source "GLEIF_FUND_MANAGEMENT")

;; Step 4: Assign SICAV if Luxembourg umbrella
;; (Only for LU funds with umbrella structure)
(cbu.role:assign-sicav
    :cbu-id @cbu_{lei_prefix}
    :entity-id @sicav_entity
    :role-type "SICAV")

;; Step 5: Assign ManCo if known
;; (Many AllianzGI funds self-managed, so AllianzGI is also ManCo)
(cbu.role:assign-manco
    :cbu-id @cbu_{lei_prefix}
    :entity-id @allianzgi
    :role-type "MANAGEMENT_COMPANY")
```

### Phase 4: Create Allianz SE Direct Subsidiaries

For each of the 237 subsidiaries in `allianz_se_corporate_tree.json`:

```dsl
(entity.ensure-limited-company
    :name "{subsidiary_name}"
    :lei "{subsidiary_lei}"
    :jurisdiction "{jurisdiction}"
    :city "{city}"
    :direct-parent-lei "529900K9B0N5BT694847"
    :as @sub_{lei_prefix})
```

---

## DSL Generation Script

Create a Python script to generate the full DSL from JSON:

**File:** `scripts/generate_allianz_gleif_dsl.py`

```python
#!/usr/bin/env python3
"""
Generate DSL commands to load Allianz GLEIF data.

Outputs: data/derived/dsl/allianz_gleif_full.dsl
"""

import json
from datetime import datetime

def lei_to_alias(lei: str) -> str:
    """Convert LEI to short alias."""
    return lei[:8].lower()

def generate_entity_dsl(entity: dict, alias_override: str = None) -> str:
    """Generate entity.ensure-limited-company DSL."""
    lei = entity['lei']
    alias = alias_override or f"@{lei_to_alias(lei)}"
    
    lines = [
        f"(entity.ensure-limited-company",
        f'    :name "{entity["name"]}"',
        f'    :lei "{lei}"',
        f'    :jurisdiction "{entity.get("jurisdiction", "")}"',
    ]
    
    if entity.get('registration_number'):
        lines.append(f'    :registration-number "{entity["registration_number"]}"')
    if entity.get('city'):
        lines.append(f'    :city "{entity["city"]}"')
    if entity.get('status'):
        lines.append(f'    :gleif-status "{entity["status"]}"')
    if entity.get('category'):
        lines.append(f'    :gleif-category "{entity["category"]}"')
    if entity.get('legal_form'):
        lines.append(f'    :legal-form-code "{entity["legal_form"]}"')
    if entity.get('validation_level'):
        lines.append(f'    :gleif-validation-level "{entity["validation_level"]}"')
    if entity.get('direct_parent', {}).get('parent_lei'):
        lines.append(f'    :direct-parent-lei "{entity["direct_parent"]["parent_lei"]}"')
    if entity.get('ultimate_parent', {}).get('parent_lei'):
        lines.append(f'    :ultimate-parent-lei "{entity["ultimate_parent"]["parent_lei"]}"')
    
    lines.append(f'    :as {alias})')
    return '\n'.join(lines)

def generate_fund_cbu_dsl(fund: dict, im_alias: str = "@allianzgi") -> str:
    """Generate CBU creation and role assignment for a fund."""
    lei = fund['lei']
    alias = f"@fund_{lei_to_alias(lei)}"
    cbu_alias = f"@cbu_{lei_to_alias(lei)}"
    
    lines = [
        f";; Fund: {fund['name'][:50]}...",
        generate_entity_dsl({
            'lei': lei,
            'name': fund['name'],
            'jurisdiction': fund.get('jurisdiction', ''),
            'category': 'FUND',
        }, alias),
        "",
        f"(cbu.create",
        f'    :name "{fund["name"][:100]}"',
        f'    :client-type "FUND"',
        f'    :jurisdiction "{fund.get("jurisdiction", "")}"',
        f'    :as {cbu_alias})',
        "",
        f"(cbu.role:assign-investment-manager",
        f'    :cbu-id {cbu_alias}',
        f'    :entity-id {im_alias}',
        f'    :role-type "INVESTMENT_MANAGER"',
        f'    :source "GLEIF_FUND_MANAGEMENT")',
        "",
        f";; ManCo = AllianzGI (self-managed)",
        f"(cbu.role:assign-manco",
        f'    :cbu-id {cbu_alias}',
        f'    :entity-id {im_alias}',
        f'    :role-type "MANAGEMENT_COMPANY")',
    ]
    
    # Add SICAV for Luxembourg funds
    if fund.get('jurisdiction') == 'LU':
        lines.extend([
            "",
            f";; Luxembourg SICAV structure",
            f"(cbu.role:assign-sicav",
            f'    :cbu-id {cbu_alias}',
            f'    :entity-id {alias}',
            f'    :role-type "SICAV")',
        ])
    
    return '\n'.join(lines)

def main():
    base_path = '/Users/adamtc007/Developer/ob-poc/data/derived/gleif'
    
    # Load data
    with open(f'{base_path}/allianz_level2_data.json') as f:
        level2 = json.load(f)
    
    with open(f'{base_path}/allianz_se_corporate_tree.json') as f:
        corp_tree = json.load(f)
    
    with open(f'{base_path}/allianzgi_ownership_chain.json') as f:
        ownership = json.load(f)
    
    dsl_lines = [
        f";; ============================================================================",
        f";; ALLIANZ GLEIF DATA LOAD",
        f";; Generated: {datetime.now().isoformat()}",
        f";; Source: GLEIF API (api.gleif.org)",
        f";; ============================================================================",
        "",
        ";; PHASE 1: Parent Entities",
        "",
    ]
    
    # Phase 1: Parents
    for lei, entity in level2['entities'].items():
        dsl_lines.append(generate_entity_dsl(entity))
        dsl_lines.append("")
    
    # Ownership relationship
    dsl_lines.extend([
        ";; Ownership: Allianz SE -> AllianzGI (100% accounting consolidation)",
        "(cbu.role:assign-ownership",
        '    :owner-entity-id @529900k9',
        '    :owned-entity-id @oj2tiqsv',
        '    :percentage 100.0',
        '    :ownership-type "ACCOUNTING_CONSOLIDATION"',
        '    :source "GLEIF")',
        "",
        ";; PHASE 2: AllianzGI Subsidiaries",
        "",
    ])
    
    # Phase 2: AllianzGI subsidiaries (from ownership chain)
    for sub in ownership.get('subsidiaries', []):
        dsl_lines.append(generate_entity_dsl(sub))
        dsl_lines.append("")
    
    # Phase 3: Funds as CBUs
    dsl_lines.extend([
        ";; PHASE 3: Funds -> CBUs with Investment Manager roles",
        f";; Total funds: {len(ownership.get('funds', []))}",
        "",
    ])
    
    for fund in ownership.get('funds', [])[:50]:  # Start with first 50
        dsl_lines.append(generate_fund_cbu_dsl(fund))
        dsl_lines.append("")
    
    # Phase 4: Allianz SE direct subsidiaries
    dsl_lines.extend([
        ";; PHASE 4: Allianz SE Direct Subsidiaries",
        f";; Total: {corp_tree['direct_children_count']}",
        "",
    ])
    
    for child in corp_tree['direct_children'][:50]:  # Start with first 50
        dsl_lines.append(generate_entity_dsl(child))
        dsl_lines.append("")
    
    # Write output
    output_path = '/Users/adamtc007/Developer/ob-poc/data/derived/dsl/allianz_gleif_full.dsl'
    with open(output_path, 'w') as f:
        f.write('\n'.join(dsl_lines))
    
    print(f"Generated: {output_path}")
    print(f"  Parents: {len(level2['entities'])}")
    print(f"  Subsidiaries: {len(ownership.get('subsidiaries', []))}")
    print(f"  Funds (first 50): 50")
    print(f"  Corp Tree (first 50): 50")

if __name__ == '__main__':
    main()
```

---

## Execution Commands

```bash
# 1. Generate DSL from JSON
cd /Users/adamtc007/Developer/ob-poc
python3 scripts/generate_allianz_gleif_dsl.py

# 2. Validate DSL syntax
cargo run --bin dsl_parser -- data/derived/dsl/allianz_gleif_full.dsl --validate

# 3. Execute DSL (dry run)
cargo run --bin dsl_executor -- data/derived/dsl/allianz_gleif_full.dsl --dry-run

# 4. Execute DSL (live)
cargo run --bin dsl_executor -- data/derived/dsl/allianz_gleif_full.dsl
```

---

## Verification Queries

After execution, verify data loaded correctly:

```sql
-- Check entities with LEI
SELECT lei, company_name, jurisdiction, gleif_status, gleif_category
FROM "ob-poc".entity_limited_companies
WHERE lei IS NOT NULL
ORDER BY company_name
LIMIT 20;

-- Check parent relationships
SELECT 
    e.company_name,
    e.lei,
    e.direct_parent_lei,
    p.company_name as parent_name
FROM "ob-poc".entity_limited_companies e
LEFT JOIN "ob-poc".entity_limited_companies p ON e.direct_parent_lei = p.lei
WHERE e.direct_parent_lei IS NOT NULL
LIMIT 20;

-- Check CBUs created for funds
SELECT c.name, c.client_type, c.jurisdiction
FROM "ob-poc".client_business_units c
WHERE c.client_type = 'FUND'
LIMIT 20;

-- Check Investment Manager role assignments
SELECT 
    c.name as cbu_name,
    e.company_name as investment_manager,
    r.role_type
FROM "ob-poc".cbu_roles r
JOIN "ob-poc".client_business_units c ON r.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON r.entity_id = e.entity_id
WHERE r.role_type = 'INVESTMENT_MANAGER'
LIMIT 20;
```

---

## Known Issues / Edge Cases

1. **Fund names with special characters** - JSON may have Unicode escapes, ensure DSL parser handles UTF-8

2. **Japanese entity names** - `アリアンツ・グローバル・インベスターズ・ジャパン株式会社` - verify encoding

3. **Duplicate LEIs** - Use `entity.ensure-limited-company` for idempotent upsert

4. **Missing ManCo data** - Not all funds have separate ManCo; many are self-managed by AllianzGI

5. **SICAV vs Sub-fund** - Luxembourg funds may be sub-funds of umbrella SICAV; need to identify umbrella relationships

---

## Dependency Graph

```
Allianz SE (529900K9B0N5BT694847)
│   └── NO_KNOWN_PERSON (UBO Terminus)
│
├── AllianzGI GmbH (OJ2TIQSVQND4IZYYK658)
│   │
│   ├── Allianz Capital Partners USA (5493005JTEV4OVDVNH32)
│   ├── AllianzGI Japan (353800NVWWGOB9JXQZ47)
│   │
│   └── [300 Managed Funds]
│       └── Each fund:
│           1. entity.ensure-limited-company (create fund entity)
│           2. cbu.create (create CBU for onboarding)
│           3. cbu.role:assign-investment-manager (link AllianzGI)
│           4. cbu.role:assign-manco (link ManCo, often AllianzGI)
│           5. cbu.role:assign-sicav (if LU umbrella)
│
└── [237 Direct Subsidiaries]
    └── entity.ensure-limited-company (with direct-parent-lei)
```

---

## Estimated Effort

| Task | Time |
|------|------|
| Verify schema migration | 5 min |
| Create DSL generator script | 30 min |
| Generate full DSL | 2 min |
| Validate DSL syntax | 5 min |
| Execute dry run | 5 min |
| Execute live | 10 min |
| Verify data | 10 min |

**Total: ~1 hour**

---

## Success Criteria

- [ ] 2 parent entities created (Allianz SE, AllianzGI)
- [ ] 237 Allianz SE subsidiaries created with `direct_parent_lei`
- [ ] 2 AllianzGI subsidiaries created (US, JP)
- [ ] 300 fund entities created with `gleif_category = 'FUND'`
- [ ] 300 CBUs created with `client_type = 'FUND'`
- [ ] 300 Investment Manager role assignments to AllianzGI
- [ ] 300 ManCo role assignments
- [ ] ~56 SICAV role assignments (LU funds)
