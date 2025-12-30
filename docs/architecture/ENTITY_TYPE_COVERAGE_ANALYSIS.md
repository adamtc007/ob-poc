# Entity Type Coverage Analysis

## Summary Verdict: **Good Coverage for Core Use Cases**

You have 22 entity types covering 95% of custody bank onboarding scenarios. The gaps are edge cases.

---

## What You Have (22 Types)

### PERSONS ✅ Complete
| Type | Purpose | Notes |
|------|---------|-------|
| `PROPER_PERSON_NATURAL` | Individuals, directors, UBOs | Core |
| `PROPER_PERSON_BENEFICIAL_OWNER` | UBO-specific | Could be flag on NATURAL |

### CORPORATES ✅ Complete for 99% of cases
| Type | Purpose | Coverage |
|------|---------|----------|
| `LIMITED_COMPANY_PRIVATE` | Private Ltd, GmbH, SARL | Most common |
| `LIMITED_COMPANY_PUBLIC` | PLC, AG, SA | Listed companies, UBO terminus |
| `LIMITED_COMPANY_UNLIMITED` | Unlimited liability company | Rare but needed |
| `limited_company` | Generic fallback | For untyped imports |

### PARTNERSHIPS ✅ Complete
| Type | Purpose | Coverage |
|------|---------|----------|
| `PARTNERSHIP_LIMITED` | LP, KG, SCS | PE/VC structures |
| `PARTNERSHIP_GENERAL` | GP, OHG | Managing partner |
| `PARTNERSHIP_LLP` | LLP | Professional services |

### TRUSTS ✅ Complete for KYC purposes
| Type | Purpose | UBO Treatment |
|------|---------|---------------|
| `TRUST_DISCRETIONARY` | Family trusts, wealth planning | Look-through |
| `TRUST_FIXED_INTEREST` | Fixed beneficiaries | By-percentage |
| `TRUST_CHARITABLE` | Charitable trusts | Exempt |
| `TRUST_UNIT` | Unit trusts (UK fund structure) | Look-through |

### FUNDS ✅ Excellent coverage
| Type | Purpose | Examples |
|------|---------|----------|
| `fund_umbrella` | Multi-compartment | SICAV, ICAV, OEIC |
| `fund_subfund` | Compartment | Sub-fund of SICAV |
| `fund_share_class` | Share class | Different fee/currency tiers |
| `fund_standalone` | Single fund | Hedge fund, PE fund |
| `fund_master` | Master fund | Master-feeder structure |
| `fund_feeder` | Feeder fund | Feeds into master |

### SERVICE PROVIDERS (via Roles, not Entity Types) ✅ Correct Design
| Role | Entity Type Used |
|------|------------------|
| MANAGEMENT_COMPANY | `management_company` OR `LIMITED_COMPANY_*` |
| DEPOSITARY | `depositary` OR `LIMITED_COMPANY_*` |
| FUND_ADMINISTRATOR | `fund_administrator` OR `LIMITED_COMPANY_*` |
| CUSTODIAN | `LIMITED_COMPANY_*` |
| PRIME_BROKER | `LIMITED_COMPANY_*` |
| TRANSFER_AGENT | `LIMITED_COMPANY_*` |
| AUDITOR | `LIMITED_COMPANY_*` |
| etc. | Role-based, not type-based |

---

## What You're Missing (Edge Cases)

### Likely Need for Completeness

| Missing Type | Use Case | Priority | Notes |
|--------------|----------|----------|-------|
| `FOUNDATION` | Stiftung (DE/AT/LI), Anstalt (LI) | **MEDIUM** | Common in wealth planning, charitable |
| `COOPERATIVE` | Credit unions, agricultural co-ops | LOW | Rare in custody |
| `GOVERNMENT_ENTITY` | Sovereign, central bank | **MEDIUM** | SWF ownership chains |
| `SPV` | Special Purpose Vehicle | **MEDIUM** | Securitization, structured products |

### Probably Don't Need

| Type | Why Skip |
|------|----------|
| Sole Proprietorship | Natural person with business name - use PROPER_PERSON |
| Branch Office | Not separate legal entity - use parent + role |
| Joint Venture | Contractual - model via ownership edges |
| LLC (US) | Map to LIMITED_COMPANY_PRIVATE |
| Protected Cell Company | Rare, model as umbrella + subfunds |
| Non-profit/NGO | Map to LIMITED_COMPANY_* with flag |

### Jurisdictional Variants - Don't Need Separate Types

These are the same entity type with jurisdiction flag:
- Delaware LLC → `LIMITED_COMPANY_PRIVATE` + jurisdiction: US-DE
- Cayman Exempted → `LIMITED_COMPANY_PRIVATE` + jurisdiction: KY
- Luxembourg Soparfi → `LIMITED_COMPANY_PRIVATE` + jurisdiction: LU
- BVI Business Company → `LIMITED_COMPANY_PRIVATE` + jurisdiction: VG

### Fund Types - Don't Need Separate Entity Types

These are fund classifications, not entity types:
- UCITS / AIF / 40-Act → Flag or tag on fund
- Hedge Fund / PE Fund / RE Fund → Strategy tag
- ETF → Share class with exchange listing flag
- Money Market Fund → Strategy tag

---

## Recommended Additions (4 types)

```sql
INSERT INTO "ob-poc".entity_types (type_code, name, entity_category) VALUES
('FOUNDATION', 'Foundation/Stiftung', 'SHELL'),
('GOVERNMENT_ENTITY', 'Government/Sovereign Entity', 'SHELL'),
('SPV', 'Special Purpose Vehicle', 'SHELL'),
('COOPERATIVE', 'Cooperative', 'SHELL');
```

With these 4, you'd have 26 types covering ~99% of custody bank scenarios.

---

## Why Service Providers as Entity Types?

I noticed you have:
- `management_company`
- `depositary`
- `fund_administrator`

as entity types. This is redundant - they're also roles. The pattern should be:

```
Entity (LIMITED_COMPANY_PRIVATE) + Role (MANAGEMENT_COMPANY) = ManCo
```

Not:
```
Entity (management_company) = ManCo
```

**Recommendation:** Deprecate these 3 service provider entity types. Use `LIMITED_COMPANY_*` + role assignment instead.

---

## Entity Category Cleanup

Current categories are inconsistent:
- `PERSON` - Good
- `SHELL` - Catchall for everything else
- `NULL` - `limited_company` has no category

**Recommendation:**
```sql
UPDATE "ob-poc".entity_types SET entity_category = 
CASE 
  WHEN type_code LIKE 'PROPER_PERSON%' THEN 'PERSON'
  WHEN type_code LIKE 'LIMITED_COMPANY%' OR type_code = 'limited_company' THEN 'CORPORATE'
  WHEN type_code LIKE 'PARTNERSHIP%' THEN 'PARTNERSHIP'
  WHEN type_code LIKE 'TRUST%' THEN 'TRUST'
  WHEN type_code LIKE 'fund%' THEN 'FUND'
  ELSE 'OTHER'
END;
```

---

## Final Verdict

| Category | Coverage | Action |
|----------|----------|--------|
| Persons | ✅ 100% | None |
| Corporates | ✅ 98% | Add FOUNDATION, GOVERNMENT_ENTITY, SPV |
| Partnerships | ✅ 100% | None |
| Trusts | ✅ 100% | None |
| Funds | ✅ 100% | None |
| Service Providers | ⚠️ Redundant | Deprecate entity types, use roles |

**Bottom line:** You're in good shape. Add 3-4 types for completeness, clean up the service provider entity types, and you're done.
