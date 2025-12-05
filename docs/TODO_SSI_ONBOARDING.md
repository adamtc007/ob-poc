# TODO: SSI (Standing Settlement Instructions) Onboarding Module

## Overview

This document defines the implementation plan for SSI onboarding within the ob-poc custody platform. The SSI system captures settlement routing chains equivalent to what Omgeo ALERT (now DTCC CTM/ITP) maintains - the industry standard for counterparty settlement instruction matching.

## Business Context

When a custody client onboards, they need settlement instructions configured for every market they trade in. Each market/currency/asset-class combination requires a settlement chain that routes from:

```
Client Account → Custodian → Sub-Custodian/Local Agent → CSD/Depository
```

The SSI Onboarding Document is a JSON structure that captures all these routing chains in a single payload. The `setup_ssi` verb consumes this document and hydrates the internal settlement routing tables.

---

## SSI Onboarding Document Schema (JSON)

```json
{
  "$schema": "https://ob-poc.example.com/schemas/ssi-onboarding/v1.json",
  "document_type": "SSI_ONBOARDING",
  "version": "1.0.0",
  "metadata": {
    "document_id": "uuid",
    "created_at": "ISO8601",
    "created_by": "operator_id",
    "client_reference": "string",
    "effective_date": "ISO8601",
    "expiry_date": "ISO8601 | null",
    "status": "DRAFT | PENDING_APPROVAL | ACTIVE | SUPERSEDED"
  },

  "client": {
    "client_id": "uuid",
    "legal_name": "string",
    "lei": "20-char LEI",
    "bic": "8 or 11 char SWIFT BIC",
    "account_reference": "string"
  },

  "custodian": {
    "custodian_id": "uuid",
    "legal_name": "string",
    "lei": "20-char LEI",
    "bic": "CUSTBICXXX",
    "role": "GLOBAL_CUSTODIAN"
  },

  "settlement_instructions": [
    {
      "_comment": "Maps directly to custody.cbu_ssi table",
      
      "ssi_name": "US Equity DTC",
      "ssi_type": "SECURITIES | CASH | COLLATERAL",
      "market_id": "uuid (FK to markets table)",
      
      "safekeeping_account": "account number (35 chars max)",
      "safekeeping_bic": "SWIFT BIC (11 chars)",
      "safekeeping_account_name": "display name",
      
      "cash_account": "cash account number",
      "cash_account_bic": "SWIFT BIC",
      "cash_currency": "ISO 4217 (3 chars)",
      
      "collateral_account": "optional collateral account",
      "collateral_account_bic": "optional SWIFT BIC",
      
      "pset_bic": "Place of Settlement BIC (DTCYUS33, etc)",
      "receiving_agent_bic": "REAG BIC",
      "delivering_agent_bic": "DEAG BIC",
      
      "effective_date": "YYYY-MM-DD",
      "expiry_date": "YYYY-MM-DD | null",
      "status": "PENDING | ACTIVE | SUSPENDED | CLOSED",
      "source": "MANUAL | OMGEO | ALERT | SWIFT",
      "source_reference": "external reference ID",

      "agent_overrides": [
        {
          "_comment": "Maps to custody.cbu_ssi_agent_override",
          "agent_role": "REAG | DEAG | INT1 | INT2 | RECU | DECU",
          "agent_bic": "SWIFT BIC",
          "agent_account": "account at agent",
          "agent_name": "display name",
          "sequence_order": 1,
          "reason": "Market requires intermediary"
        }
      ],

      "booking_rules": [
        {
          "_comment": "Maps to custody.ssi_booking_rules",
          "rule_name": "US Equity Default",
          "priority": 50,
          "instrument_class_id": "uuid | null (null = wildcard)",
          "security_type_id": "uuid | null",
          "currency": "USD | null",
          "settlement_type": "DVP | RVP | FOP | null",
          "counterparty_entity_id": "uuid | null",
          "isda_asset_class": "string | null",
          "isda_base_product": "string | null",
          "is_active": true,
          "effective_date": "YYYY-MM-DD",
          "expiry_date": "YYYY-MM-DD | null"
        }
      ]
    }
  ],

  "cash_sweep_config": {
    "enabled": true,
    "stif_fund_id": "uuid",
    "stif_fund_name": "Short Term Investment Fund",
    "sweep_threshold": {
      "currency": "USD",
      "amount": 10000
    },
    "sweep_frequency": "END_OF_DAY | INTRADAY",
    "sweep_account": {
      "bic": "STIFBICXXX",
      "account_number": "string"
    }
  },

  "corporate_actions_config": {
    "default_election": "CASH | STOCK | REINVEST",
    "notification_bic": "SWIFT BIC for CA notifications",
    "proxy_voting_enabled": true
  }
}
```

---

## Sub-Custodian Network Reference

The SSI document should reference known sub-custodian networks. These are the "Omgeo mappings" - pre-configured local agents per market:

```json
{
  "sub_custodian_network": [
    {
      "market": "US",
      "csd": "DTC",
      "csd_bic": "DTCYUS33",
      "sub_custodians": [
        {
          "bic": "BABOROB1",
          "name": "Bank of America",
          "account_range_prefix": "BA-"
        },
        {
          "bic": "CHASUS33",
          "name": "JP Morgan Chase",
          "account_range_prefix": "JPM-"
        }
      ]
    },
    {
      "market": "GB",
      "csd": "CREST",
      "csd_bic": "CABOROB1",
      "sub_custodians": [
        {
          "bic": "BABOROB1",
          "name": "Barclays",
          "account_range_prefix": "BAR-"
        }
      ]
    },
    {
      "market": "DE",
      "csd": "Clearstream Frankfurt",
      "csd_bic": "CABOROB1",
      "sub_custodians": []
    },
    {
      "market": "LU",
      "csd": "Clearstream Luxembourg",
      "csd_bic": "CABOROB1",
      "sub_custodians": []
    }
  ]
}
```

---

## DSL Verb: `setup_ssi`

### Verb Definition (YAML)

```yaml
verb: setup_ssi
category: custody_operations
description: "Consume SSI onboarding document and configure settlement routing"

inputs:
  - name: ssi_document
    type: json_document
    source: SSI_ONBOARDING
    required: true
  - name: client_id
    type: uuid
    source: context
    required: true
  - name: validation_mode
    type: enum
    values: [STRICT, PERMISSIVE]
    default: STRICT

outputs:
  - name: ssi_routing_records
    type: list<ssi_routing>
    destination: ssi_routing table
  - name: cash_correspondent_records
    type: list<cash_correspondent>
    destination: cash_correspondents table
  - name: sweep_config
    type: stif_sweep_config
    destination: sweep_configurations table

operations:
  - validate_document_schema
  - validate_bic_codes
  - validate_lei_format
  - check_sub_custodian_coverage
  - create_ssi_routing_records
  - create_cash_correspondent_links
  - configure_sweep_rules
  - activate_instructions

error_handling:
  - invalid_bic: REJECT_INSTRUCTION
  - missing_sub_custodian: WARN_AND_CONTINUE
  - duplicate_market_instruction: SUPERSEDE_PREVIOUS
```

---

## Database Schema - EXISTING (No Changes Required)

The `custody` schema already contains the SSI infrastructure:

### Layer 2: Account Data

**custody.cbu_ssi** - Core SSI records
```sql
- ssi_id (PK)
- cbu_id (FK to client business unit)
- ssi_name, ssi_type
- safekeeping_account, safekeeping_bic, safekeeping_account_name
- cash_account, cash_account_bic, cash_currency
- collateral_account, collateral_account_bic
- pset_bic (Place of Settlement)
- receiving_agent_bic, delivering_agent_bic
- status, effective_date, expiry_date
- market_id (FK)
- source, source_reference (for tracking Omgeo/ALERT imports)
```

**custody.cbu_ssi_agent_override** - Intermediary agent chains
```sql
- override_id (PK)
- ssi_id (FK)
- agent_role (REAG, DEAG, INT1, INT2, etc.)
- agent_bic, agent_account, agent_name
- sequence_order (for chain depth)
- reason, is_active
```

### Layer 3: Routing Rules

**custody.ssi_booking_rules** - ALERT-style priority matching
```sql
- rule_id (PK)
- cbu_id, ssi_id (FK)
- rule_name, priority
- instrument_class_id, security_type_id, market_id (matching criteria)
- currency, settlement_type, counterparty_entity_id
- isda_asset_class, isda_base_product
- specificity_score (GENERATED - computed from criteria presence)
- is_active, effective_date, expiry_date
```

### Existing Function

**custody.find_ssi_for_trade()** - Returns best-match SSI based on booking rules

### Key Relationships

```
cbu (1) --> (N) cbu_ssi
cbu_ssi (1) --> (N) cbu_ssi_agent_override  
cbu (1) --> (N) ssi_booking_rules --> (1) cbu_ssi
```

---

## Implementation Tasks

### Phase 1: Document Parsing (Rust)
- [ ] Create JSON Schema for SSI Onboarding Document
- [ ] Implement Rust struct definitions matching schema
- [ ] Add serde deserialization with validation
- [ ] Create document_type `SSI_ONBOARDING` in document types
- [ ] Map JSON fields to existing `custody.cbu_ssi` columns
- [ ] Map intermediary chains to `custody.cbu_ssi_agent_override`

### Phase 2: Database - NO SCHEMA CHANGES
Tables already exist in `custody` schema:
- [x] `cbu_ssi` ✓
- [x] `cbu_ssi_agent_override` ✓
- [x] `ssi_booking_rules` ✓
- [x] `find_ssi_for_trade()` ✓

### Phase 3: Verb Implementation
- [ ] Add setup_ssi verb to verb registry (YAML)
- [ ] Implement verb handler in Go microservice
- [ ] Add BIC validation (SWIFT format check)
- [ ] Add LEI validation (checksum verification)
- [ ] Implement sub-custodian coverage check
- [ ] Create SSI routing record insertion logic
- [ ] Create cash correspondent linking logic
- [ ] Implement sweep configuration logic

### Phase 4: Query & Retrieval
- [ ] Add get_ssi verb for retrieving active instructions
- [ ] Add list_ssi_by_market verb
- [ ] Add validate_ssi verb for pre-flight checks
- [ ] Create SWIFT message field generator

### Phase 5: Integration
- [ ] Wire SSI setup into custody account onboarding flow
- [ ] Add SSI document upload to onboarding UI
- [ ] Create SSI visualization in egui (settlement chain diagram)
- [ ] Add SSI export to Omgeo/ALERT format (future)

---

## Example DSL Usage

```dsl
# Load SSI document and setup routing
DEFINE ssi_doc AS LOAD DOCUMENT "SSI_ONBOARDING" WHERE client_id = $client_id

EXECUTE setup_ssi 
  WITH ssi_document = $ssi_doc
  WITH validation_mode = "STRICT"

# Query active SSI for a market
GET ssi_routing 
  WHERE client_id = $client_id 
  AND market_code = "XNYS"
  AND status = "ACTIVE"
```

---

## Reference: SWIFT MT Settlement Fields

For settlement instruction matching, these SWIFT MT54x fields are populated from SSI data:

| Field | Tag | SSI Source |
|-------|-----|------------|
| Place of Settlement | :95P::PSET | place_of_settlement.pset_bic |
| Receiver's Agent | :95P::REAG | settlement_chain.receiver_chain.receivers_agent.bic |
| Receiver's Custodian | :95P::RECU | settlement_chain.receiver_chain.receivers_custodian.bic |
| Deliverer's Agent | :95P::DEAG | settlement_chain.deliverer_chain.deliverers_agent.bic |
| Deliverer's Custodian | :95P::DECU | settlement_chain.deliverer_chain.deliverers_custodian.bic |
| Safekeeping Account | :97A::SAFE | settlement_chain.*.account |
| Beneficiary | :95P::BUYR | settlement_chain.receiver_chain.beneficiary.bic |

---

## Notes

- **No schema changes required** - existing `custody` schema already supports full SSI lifecycle
- JSON document maps 1:1 to existing table columns for straightforward INSERT/UPDATE
- Booking rules use `specificity_score` (computed column) + `priority` for ALERT-style matching
- Multi-currency accounts may have multiple SSI records per market (one per settlement currency)
- `source` field tracks origin: MANUAL, OMGEO, ALERT, SWIFT for audit/reconciliation
- STIF/cash sweep configuration is included as it's typically set up alongside SSI during custody onboarding

---

## Dependencies

- Existing tables: `custody.cbu_ssi`, `custody.cbu_ssi_agent_override`, `custody.ssi_booking_rules`
- Existing function: `custody.find_ssi_for_trade()`
- Reference data: `custody.markets`, `custody.instrument_classes`, `custody.security_types`
- External (optional): BIC directory for validation, LEI-GLEIF for LEI verification

---

*Document Version: 1.1*  
*Created: December 2024*  
*Updated: Aligned with existing custody.cbu_ssi schema*  
*Status: TODO - Ready for Implementation (No Schema Changes)*
