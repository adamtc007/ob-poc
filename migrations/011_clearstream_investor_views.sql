-- =============================================================================
-- Migration 011: Clearstream Investor Register Views
-- =============================================================================
-- Purpose: Add views for Clearstream CASCADE-RS/Vestima data integration
--          and BODS-compliant investor register reporting
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. Clearstream Register View (Main investor register)
-- -----------------------------------------------------------------------------
-- Maps to Clearstream's "List of Registered Shares" report format

CREATE OR REPLACE VIEW kyc.v_clearstream_register AS
SELECT
    -- Share Class (Fund) Information
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,
    sc.fund_type,
    sc.fund_structure,
    sc.investor_eligibility,
    
    -- Fund CBU
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,
    
    -- Investor Information
    h.id AS holding_id,
    h.investor_entity_id,
    e.name AS investor_name,
    e.entity_type AS investor_entity_type,
    e.country_code AS investor_country,
    
    -- Clearstream Reference ID (if available)
    clr_id.id AS clearstream_reference,
    
    -- LEI (if available)
    lei.id AS investor_lei,
    lei.lei_status,
    
    -- Position Data
    h.units AS holding_quantity,
    h.cost_basis,
    h.acquisition_date AS registration_date,
    h.status AS holding_status,
    
    -- Computed: Market Value
    CASE 
        WHEN sc.nav_per_share IS NOT NULL 
        THEN h.units * sc.nav_per_share 
        ELSE NULL 
    END AS market_value,
    
    -- Computed: Ownership Percentage of Share Class
    CASE 
        WHEN total_units.total > 0 
        THEN ROUND((h.units / total_units.total) * 100, 4)
        ELSE NULL 
    END AS ownership_percentage,
    
    -- Timestamps
    h.created_at AS holding_created_at,
    h.updated_at AS holding_updated_at

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id

-- Clearstream KV reference (optional)
LEFT JOIN "ob-poc".entity_identifiers clr_id 
    ON e.entity_id = clr_id.entity_id 
    AND clr_id.scheme = 'CLEARSTREAM_KV'

-- LEI (optional)
LEFT JOIN "ob-poc".entity_identifiers lei 
    ON e.entity_id = lei.entity_id 
    AND lei.scheme = 'LEI'

-- Total units for percentage calculation
LEFT JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id
    AND h2.status = 'active'
) total_units ON true

WHERE h.status = 'active';

COMMENT ON VIEW kyc.v_clearstream_register IS 
'Clearstream-style investor register with holdings, identifiers, and ownership percentages';

-- -----------------------------------------------------------------------------
-- 2. Clearstream Movement Report View
-- -----------------------------------------------------------------------------
-- Maps to Clearstream's transaction/movement reporting format

CREATE OR REPLACE VIEW kyc.v_clearstream_movements AS
SELECT
    -- Movement Details
    m.id AS movement_id,
    m.reference AS trans_ref,
    m.movement_type,
    m.units,
    m.price_per_unit,
    m.amount,
    m.currency,
    m.trade_date,
    m.settlement_date,
    m.status AS movement_status,
    m.notes,
    
    -- Holding Context
    h.id AS holding_id,
    h.units AS current_holding_units,
    
    -- Share Class
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    
    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    
    -- Investor
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    clr_id.id AS clearstream_reference,
    lei.id AS investor_lei,
    
    -- Timestamps
    m.created_at,
    m.updated_at

FROM kyc.movements m
JOIN kyc.holdings h ON m.holding_id = h.id
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id

-- Clearstream KV reference
LEFT JOIN "ob-poc".entity_identifiers clr_id 
    ON e.entity_id = clr_id.entity_id 
    AND clr_id.scheme = 'CLEARSTREAM_KV'

-- LEI
LEFT JOIN "ob-poc".entity_identifiers lei 
    ON e.entity_id = lei.entity_id 
    AND lei.scheme = 'LEI';

COMMENT ON VIEW kyc.v_clearstream_movements IS 
'Clearstream-style movement/transaction log with investor and fund context';

-- -----------------------------------------------------------------------------
-- 3. BODS Ownership Statement View (for BODS export)
-- -----------------------------------------------------------------------------
-- Maps holdings to BODS 0.4 Ownership-or-Control Statement format

CREATE OR REPLACE VIEW kyc.v_bods_ownership_statements AS
SELECT
    -- Statement identifiers
    'ooc-' || h.id::text AS statement_id,
    'ownershipOrControlStatement' AS statement_type,
    
    -- Subject (the fund/share class being owned)
    'entityStatement' AS subject_type,
    sc.isin AS subject_isin,
    fund_lei.id AS subject_lei,
    c.name AS subject_name,
    
    -- Interested Party (the investor)
    CASE 
        WHEN e.entity_type IN ('proper_person', 'natural_person') 
        THEN 'personStatement'
        ELSE 'entityStatement'
    END AS interested_party_type,
    e.name AS interested_party_name,
    investor_lei.id AS interested_party_lei,
    
    -- Interest details
    'shareholding' AS interest_type,
    'direct' AS interest_directness,
    h.units AS share_exact,
    CASE 
        WHEN total_units.total > 0 
        THEN ROUND((h.units / total_units.total) * 100, 2)
        ELSE NULL 
    END AS share_percentage,
    
    -- Beneficial ownership flag
    CASE 
        WHEN total_units.total > 0 AND (h.units / total_units.total) >= 0.25
        THEN true
        ELSE false
    END AS beneficial_ownership_or_control,
    
    -- Dates
    h.acquisition_date AS interest_start_date,
    CASE WHEN h.status != 'active' THEN h.updated_at::date END AS interest_end_date,
    
    -- Source
    'Clearstream CASCADE-RS' AS source_type,
    clr_id.id AS source_reference,
    
    -- Statement date
    CURRENT_DATE AS statement_date,
    h.updated_at AS publication_date

FROM kyc.holdings h
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id
JOIN "ob-poc".entities e ON h.investor_entity_id = e.entity_id

-- Fund's LEI
LEFT JOIN "ob-poc".entity_identifiers fund_lei 
    ON sc.entity_id = fund_lei.entity_id 
    AND fund_lei.scheme = 'LEI'

-- Investor's LEI
LEFT JOIN "ob-poc".entity_identifiers investor_lei 
    ON e.entity_id = investor_lei.entity_id 
    AND investor_lei.scheme = 'LEI'

-- Clearstream reference
LEFT JOIN "ob-poc".entity_identifiers clr_id 
    ON e.entity_id = clr_id.entity_id 
    AND clr_id.scheme = 'CLEARSTREAM_KV'

-- Total units for percentage
LEFT JOIN LATERAL (
    SELECT COALESCE(SUM(h2.units), 0) AS total
    FROM kyc.holdings h2
    WHERE h2.share_class_id = sc.id
    AND h2.status = 'active'
) total_units ON true;

COMMENT ON VIEW kyc.v_bods_ownership_statements IS 
'BODS 0.4 Ownership-or-Control Statement format for regulatory reporting';

-- -----------------------------------------------------------------------------
-- 4. Share Class Summary View
-- -----------------------------------------------------------------------------
-- Aggregated view of each share class with investor counts and AUM

CREATE OR REPLACE VIEW kyc.v_share_class_summary AS
SELECT
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,
    sc.fund_type,
    sc.fund_structure,
    sc.investor_eligibility,
    sc.status,
    
    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,
    
    -- Aggregates
    COALESCE(stats.investor_count, 0) AS investor_count,
    COALESCE(stats.total_units, 0) AS total_units,
    CASE 
        WHEN sc.nav_per_share IS NOT NULL 
        THEN COALESCE(stats.total_units, 0) * sc.nav_per_share 
        ELSE NULL 
    END AS assets_under_management,
    
    -- Movement activity (last 30 days)
    COALESCE(activity.subscription_count, 0) AS subscriptions_30d,
    COALESCE(activity.redemption_count, 0) AS redemptions_30d,
    COALESCE(activity.net_flow_units, 0) AS net_flow_units_30d

FROM kyc.share_classes sc
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id

-- Holding statistics
LEFT JOIN LATERAL (
    SELECT 
        COUNT(DISTINCT h.investor_entity_id) AS investor_count,
        SUM(h.units) AS total_units
    FROM kyc.holdings h
    WHERE h.share_class_id = sc.id
    AND h.status = 'active'
) stats ON true

-- Recent activity
LEFT JOIN LATERAL (
    SELECT 
        COUNT(*) FILTER (WHERE m.movement_type = 'subscription') AS subscription_count,
        COUNT(*) FILTER (WHERE m.movement_type = 'redemption') AS redemption_count,
        COALESCE(SUM(CASE 
            WHEN m.movement_type IN ('subscription', 'transfer_in') THEN m.units
            WHEN m.movement_type IN ('redemption', 'transfer_out') THEN -m.units
            ELSE 0
        END), 0) AS net_flow_units
    FROM kyc.movements m
    JOIN kyc.holdings h ON m.holding_id = h.id
    WHERE h.share_class_id = sc.id
    AND m.trade_date >= CURRENT_DATE - INTERVAL '30 days'
    AND m.status IN ('confirmed', 'settled')
) activity ON true;

COMMENT ON VIEW kyc.v_share_class_summary IS 
'Share class summary with investor counts, AUM, and recent activity metrics';

-- -----------------------------------------------------------------------------
-- 5. Investor Portfolio View
-- -----------------------------------------------------------------------------
-- All holdings for a given investor across all funds

CREATE OR REPLACE VIEW kyc.v_investor_portfolio AS
SELECT
    -- Investor
    e.entity_id AS investor_entity_id,
    e.name AS investor_name,
    e.entity_type AS investor_type,
    e.country_code AS investor_country,
    
    -- Identifiers
    lei.id AS investor_lei,
    clr_id.id AS clearstream_reference,
    
    -- Holding
    h.id AS holding_id,
    h.units,
    h.cost_basis,
    h.acquisition_date,
    h.status AS holding_status,
    
    -- Share Class
    sc.id AS share_class_id,
    sc.isin,
    sc.name AS share_class_name,
    sc.currency,
    sc.nav_per_share,
    sc.nav_date,
    
    -- Fund
    c.cbu_id AS fund_cbu_id,
    c.name AS fund_name,
    c.jurisdiction AS fund_jurisdiction,
    
    -- Computed values
    CASE 
        WHEN sc.nav_per_share IS NOT NULL 
        THEN h.units * sc.nav_per_share 
        ELSE NULL 
    END AS market_value,
    
    CASE 
        WHEN h.cost_basis IS NOT NULL AND sc.nav_per_share IS NOT NULL
        THEN (h.units * sc.nav_per_share) - h.cost_basis
        ELSE NULL 
    END AS unrealized_pnl

FROM "ob-poc".entities e
JOIN kyc.holdings h ON e.entity_id = h.investor_entity_id
JOIN kyc.share_classes sc ON h.share_class_id = sc.id
JOIN "ob-poc".cbus c ON sc.cbu_id = c.cbu_id

-- LEI
LEFT JOIN "ob-poc".entity_identifiers lei 
    ON e.entity_id = lei.entity_id 
    AND lei.scheme = 'LEI'

-- Clearstream reference
LEFT JOIN "ob-poc".entity_identifiers clr_id 
    ON e.entity_id = clr_id.entity_id 
    AND clr_id.scheme = 'CLEARSTREAM_KV'

WHERE h.status = 'active';

COMMENT ON VIEW kyc.v_investor_portfolio IS 
'Investor portfolio view showing all holdings across funds with market values';

-- -----------------------------------------------------------------------------
-- 6. Identifier Cross-Reference View
-- -----------------------------------------------------------------------------
-- All identifiers for entities (LEI, Clearstream, Tax ID, etc.)

CREATE OR REPLACE VIEW "ob-poc".v_entity_identifier_xref AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    e.entity_type,
    e.country_code,
    
    -- Pivot common identifier schemes
    MAX(CASE WHEN ei.scheme = 'LEI' THEN ei.id END) AS lei,
    MAX(CASE WHEN ei.scheme = 'LEI' THEN ei.lei_status END) AS lei_status,
    MAX(CASE WHEN ei.scheme = 'CLEARSTREAM_KV' THEN ei.id END) AS clearstream_kv,
    MAX(CASE WHEN ei.scheme = 'CLEARSTREAM_ACCT' THEN ei.id END) AS clearstream_account,
    MAX(CASE WHEN ei.scheme = 'company_register' THEN ei.id END) AS company_register_id,
    MAX(CASE WHEN ei.scheme = 'tax_id' THEN ei.id END) AS tax_id,
    MAX(CASE WHEN ei.scheme = 'ISIN' THEN ei.id END) AS isin,
    
    -- Count of all identifiers
    COUNT(ei.identifier_id) AS identifier_count,
    
    -- Validation status
    BOOL_OR(ei.is_validated) AS has_validated_identifier

FROM "ob-poc".entities e
LEFT JOIN "ob-poc".entity_identifiers ei ON e.entity_id = ei.entity_id
GROUP BY e.entity_id, e.name, e.entity_type, e.country_code;

COMMENT ON VIEW "ob-poc".v_entity_identifier_xref IS 
'Cross-reference view of all entity identifiers (LEI, Clearstream, Tax ID, etc.)';

-- -----------------------------------------------------------------------------
-- 7. Add indexes for view performance
-- -----------------------------------------------------------------------------

-- Index for Clearstream identifier lookups
CREATE INDEX IF NOT EXISTS idx_entity_identifiers_clearstream 
ON "ob-poc".entity_identifiers(scheme, id) 
WHERE scheme IN ('CLEARSTREAM_KV', 'CLEARSTREAM_ACCT');

-- Index for share class by ISIN
CREATE INDEX IF NOT EXISTS idx_share_classes_isin 
ON kyc.share_classes(isin) 
WHERE isin IS NOT NULL;

-- Index for active holdings
CREATE INDEX IF NOT EXISTS idx_holdings_active 
ON kyc.holdings(share_class_id, investor_entity_id) 
WHERE status = 'active';

-- Index for movement lookups by date
CREATE INDEX IF NOT EXISTS idx_movements_trade_date 
ON kyc.movements(trade_date, movement_type, status);

-- -----------------------------------------------------------------------------
-- Done
-- -----------------------------------------------------------------------------
