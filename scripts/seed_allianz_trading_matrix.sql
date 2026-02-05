-- Seed a sample trading matrix for ALLIANZ GLOBAL CREDIT (LU)
-- Run with: psql -d data_designer -f scripts/seed_allianz_trading_matrix.sql

INSERT INTO "ob-poc".cbu_trading_profiles (
    profile_id,
    cbu_id,
    version,
    status,
    document,
    created_by,
    created_at
) VALUES (
    gen_random_uuid(),
    'daa59bd3-a6a1-4030-a181-d0c256ee3e86', -- ALLIANZ GLOBAL CREDIT
    1,
    'ACTIVE',
    '{
        "cbu_id": "daa59bd3-a6a1-4030-a181-d0c256ee3e86",
        "cbu_name": "ALLIANZ GLOBAL CREDIT",
        "version": 1,
        "status": "ACTIVE",
        "children": [
            {
                "id": ["_UNIVERSE"],
                "node_type": {"type": "category", "name": "Trading Universe"},
                "label": "Trading Universe",
                "children": [
                    {
                        "id": ["_UNIVERSE", "EQUITY"],
                        "node_type": {"type": "instrument_class", "class_code": "EQUITY", "cfi_prefix": "ES", "is_otc": false},
                        "label": "Equity",
                        "sublabel": "Exchange-traded equities",
                        "children": [
                            {
                                "id": ["_UNIVERSE", "EQUITY", "XNYS"],
                                "node_type": {"type": "market", "mic": "XNYS", "market_name": "New York Stock Exchange", "country_code": "US"},
                                "label": "NYSE",
                                "sublabel": "XNYS",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            },
                            {
                                "id": ["_UNIVERSE", "EQUITY", "XLON"],
                                "node_type": {"type": "market", "mic": "XLON", "market_name": "London Stock Exchange", "country_code": "GB"},
                                "label": "LSE",
                                "sublabel": "XLON",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            },
                            {
                                "id": ["_UNIVERSE", "EQUITY", "XFRA"],
                                "node_type": {"type": "market", "mic": "XFRA", "market_name": "Frankfurt Stock Exchange", "country_code": "DE"},
                                "label": "Frankfurt",
                                "sublabel": "XFRA",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            }
                        ],
                        "is_loaded": true,
                        "leaf_count": 3
                    },
                    {
                        "id": ["_UNIVERSE", "GOVT_BOND"],
                        "node_type": {"type": "instrument_class", "class_code": "GOVT_BOND", "cfi_prefix": "DB", "is_otc": false},
                        "label": "Government Bonds",
                        "sublabel": "Sovereign debt",
                        "children": [
                            {
                                "id": ["_UNIVERSE", "GOVT_BOND", "US"],
                                "node_type": {"type": "market", "mic": "XNAS", "market_name": "US Treasuries", "country_code": "US"},
                                "label": "US Treasuries",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            },
                            {
                                "id": ["_UNIVERSE", "GOVT_BOND", "DE"],
                                "node_type": {"type": "market", "mic": "XFRA", "market_name": "German Bunds", "country_code": "DE"},
                                "label": "German Bunds",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            }
                        ],
                        "is_loaded": true,
                        "leaf_count": 2
                    },
                    {
                        "id": ["_UNIVERSE", "CORP_BOND"],
                        "node_type": {"type": "instrument_class", "class_code": "CORP_BOND", "cfi_prefix": "DC", "is_otc": false},
                        "label": "Corporate Bonds",
                        "sublabel": "Investment grade credit",
                        "children": [],
                        "is_loaded": true,
                        "leaf_count": 0
                    },
                    {
                        "id": ["_UNIVERSE", "OTC_IRS"],
                        "node_type": {"type": "instrument_class", "class_code": "OTC_IRS", "is_otc": true},
                        "label": "Interest Rate Swaps",
                        "sublabel": "OTC derivatives",
                        "children": [
                            {
                                "id": ["_UNIVERSE", "OTC_IRS", "GS"],
                                "node_type": {"type": "counterparty", "entity_id": "11111111-1111-1111-1111-111111111111", "entity_name": "Goldman Sachs", "lei": "FOR8UP27PHTHYVLBNG30"},
                                "label": "Goldman Sachs",
                                "sublabel": "FOR8UP27PHTHYVLBNG30",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            },
                            {
                                "id": ["_UNIVERSE", "OTC_IRS", "JPM"],
                                "node_type": {"type": "counterparty", "entity_id": "22222222-2222-2222-2222-222222222222", "entity_name": "JPMorgan Chase", "lei": "8I5DZWZKVSZI1NUHU748"},
                                "label": "JPMorgan Chase",
                                "sublabel": "8I5DZWZKVSZI1NUHU748",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            }
                        ],
                        "is_loaded": true,
                        "leaf_count": 2
                    }
                ],
                "is_loaded": true,
                "leaf_count": 7
            },
            {
                "id": ["_SSI"],
                "node_type": {"type": "category", "name": "Standing Settlement Instructions"},
                "label": "Standing Settlement Instructions",
                "children": [
                    {
                        "id": ["_SSI", "US_EQUITY"],
                        "node_type": {"type": "ssi", "ssi_id": "ssi-001", "ssi_name": "US Equities SSI", "ssi_type": "SECURITIES", "status": "ACTIVE", "safekeeping_account": "12345678", "safekeeping_bic": "BABORUSS", "pset_bic": "DTCYUS33"},
                        "label": "US Equities SSI",
                        "sublabel": "ACTIVE",
                        "status_color": "green",
                        "children": [
                            {
                                "id": ["_SSI", "US_EQUITY", "RULE1"],
                                "node_type": {"type": "booking_rule", "rule_id": "rule-001", "rule_name": "US Equity DVP", "priority": 10, "specificity_score": 100, "is_active": true, "match_criteria": {"instrument_class": "EQUITY", "mic": "XNYS", "settlement_type": "DVP"}},
                                "label": "US Equity DVP",
                                "sublabel": "Priority: 10",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            }
                        ],
                        "is_loaded": true,
                        "leaf_count": 1
                    },
                    {
                        "id": ["_SSI", "EUR_CASH"],
                        "node_type": {"type": "ssi", "ssi_id": "ssi-002", "ssi_name": "EUR Cash SSI", "ssi_type": "CASH", "status": "ACTIVE", "cash_account": "DE89370400440532013000", "cash_bic": "COBADEFF", "cash_currency": "EUR"},
                        "label": "EUR Cash SSI",
                        "sublabel": "ACTIVE",
                        "status_color": "green",
                        "children": [],
                        "is_loaded": true,
                        "leaf_count": 1
                    },
                    {
                        "id": ["_SSI", "COLLATERAL"],
                        "node_type": {"type": "ssi", "ssi_id": "ssi-003", "ssi_name": "Collateral SSI", "ssi_type": "COLLATERAL", "status": "PENDING", "safekeeping_account": "COL-001", "safekeeping_bic": "CITIUS33"},
                        "label": "Collateral SSI",
                        "sublabel": "PENDING",
                        "status_color": "yellow",
                        "children": [],
                        "is_loaded": true,
                        "leaf_count": 1
                    }
                ],
                "is_loaded": true,
                "leaf_count": 3
            },
            {
                "id": ["_ISDA"],
                "node_type": {"type": "category", "name": "ISDA Agreements"},
                "label": "ISDA Agreements",
                "children": [
                    {
                        "id": ["_ISDA", "GS"],
                        "node_type": {"type": "isda_agreement", "isda_id": "isda-001", "counterparty_name": "Goldman Sachs", "governing_law": "NY", "agreement_date": "2023-01-15", "counterparty_lei": "FOR8UP27PHTHYVLBNG30"},
                        "label": "Goldman Sachs",
                        "sublabel": "NY Law",
                        "status_color": "green",
                        "children": [
                            {
                                "id": ["_ISDA", "GS", "VM_CSA"],
                                "node_type": {"type": "csa_agreement", "csa_id": "csa-001", "csa_type": "VM", "threshold_currency": "USD", "threshold_amount": 250000, "minimum_transfer_amount": 500000},
                                "label": "VM CSA",
                                "sublabel": "USD 250K threshold",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            },
                            {
                                "id": ["_ISDA", "GS", "RATES"],
                                "node_type": {"type": "product_coverage", "coverage_id": "cov-001", "asset_class": "RATES", "base_products": ["IRS", "XCCY", "BASIS"]},
                                "label": "Rates Coverage",
                                "sublabel": "IRS, XCCY, BASIS",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            }
                        ],
                        "is_loaded": true,
                        "leaf_count": 2
                    },
                    {
                        "id": ["_ISDA", "JPM"],
                        "node_type": {"type": "isda_agreement", "isda_id": "isda-002", "counterparty_name": "JPMorgan Chase", "governing_law": "ENGLISH", "agreement_date": "2022-06-01", "counterparty_lei": "8I5DZWZKVSZI1NUHU748"},
                        "label": "JPMorgan Chase",
                        "sublabel": "English Law",
                        "status_color": "green",
                        "children": [
                            {
                                "id": ["_ISDA", "JPM", "VM_CSA"],
                                "node_type": {"type": "csa_agreement", "csa_id": "csa-002", "csa_type": "VM", "threshold_currency": "EUR", "threshold_amount": 0, "minimum_transfer_amount": 250000},
                                "label": "VM CSA",
                                "sublabel": "Zero threshold",
                                "status_color": "green",
                                "children": [],
                                "is_loaded": true,
                                "leaf_count": 1
                            }
                        ],
                        "is_loaded": true,
                        "leaf_count": 1
                    }
                ],
                "is_loaded": true,
                "leaf_count": 3
            },
            {
                "id": ["_TAX"],
                "node_type": {"type": "category", "name": "Tax Configuration"},
                "label": "Tax Configuration",
                "children": [
                    {
                        "id": ["_TAX", "DE"],
                        "node_type": {"type": "tax_jurisdiction", "jurisdiction_id": "tax-de", "jurisdiction_code": "DE", "jurisdiction_name": "Germany", "default_withholding_rate": 26.375, "reclaim_available": true},
                        "label": "Germany",
                        "sublabel": "WHT 26.375%",
                        "status_color": "green",
                        "children": [],
                        "is_loaded": true,
                        "leaf_count": 1
                    },
                    {
                        "id": ["_TAX", "US"],
                        "node_type": {"type": "tax_jurisdiction", "jurisdiction_id": "tax-us", "jurisdiction_code": "US", "jurisdiction_name": "United States", "default_withholding_rate": 30, "reclaim_available": true},
                        "label": "United States",
                        "sublabel": "WHT 30%",
                        "status_color": "yellow",
                        "children": [],
                        "is_loaded": true,
                        "leaf_count": 1
                    }
                ],
                "is_loaded": true,
                "leaf_count": 2
            },
            {
                "id": ["_MANAGERS"],
                "node_type": {"type": "category", "name": "Investment Managers"},
                "label": "Investment Managers",
                "children": [
                    {
                        "id": ["_MANAGERS", "AGI"],
                        "node_type": {"type": "investment_manager_mandate", "mandate_id": "im-001", "manager_entity_id": "33333333-3333-3333-3333-333333333333", "manager_name": "Allianz Global Investors", "manager_lei": "529900LN3S50JPU47S06", "priority": 1, "role": "DISCRETIONARY", "can_trade": true, "can_settle": true},
                        "label": "Allianz Global Investors",
                        "sublabel": "Discretionary",
                        "status_color": "green",
                        "children": [],
                        "is_loaded": true,
                        "leaf_count": 1
                    }
                ],
                "is_loaded": true,
                "leaf_count": 1
            }
        ],
        "total_leaf_count": 16,
        "metadata": {
            "source": "manual_seed",
            "notes": "Sample trading matrix for visualization testing"
        }
    }'::jsonb,
    'system',
    NOW()
)
ON CONFLICT (cbu_id, version) DO UPDATE SET
    document = EXCLUDED.document,
    status = 'ACTIVE';

-- Verify
SELECT c.name, c.jurisdiction,
       jsonb_array_length(tp.document->'children') as categories,
       tp.document->>'total_leaf_count' as items
FROM "ob-poc".cbus c
JOIN "ob-poc".cbu_trading_profiles tp ON c.cbu_id = tp.cbu_id
WHERE c.cbu_id = 'daa59bd3-a6a1-4030-a181-d0c256ee3e86';
