-- ============================================================================
-- Migration 021: Corporate Actions Schema
-- ============================================================================
-- Part of trading matrix pivot (032-corporate-actions-integration.md)
-- Creates tables for CA preferences, instruction windows, and SSI mappings

-- Reference catalog: CA event types (global)
CREATE TABLE IF NOT EXISTS custody.ca_event_types (
    event_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_code TEXT NOT NULL UNIQUE,
    event_name TEXT NOT NULL,
    category TEXT NOT NULL CHECK (category IN (
        'INCOME', 'REORGANIZATION', 'VOLUNTARY', 'MANDATORY', 'INFORMATION'
    )),
    is_elective BOOLEAN NOT NULL DEFAULT false,
    default_election TEXT CHECK (default_election IN (
        'CASH', 'STOCK', 'ROLLOVER', 'LAPSE', 'DECLINE', 'NO_ACTION'
    )),
    iso_event_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- CBU-specific CA preferences
CREATE TABLE IF NOT EXISTS custody.cbu_ca_preferences (
    preference_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID NOT NULL REFERENCES custody.ca_event_types(event_type_id),
    instrument_class_id UUID REFERENCES custody.instrument_classes(class_id),
    processing_mode TEXT NOT NULL CHECK (processing_mode IN (
        'AUTO_INSTRUCT', 'MANUAL', 'DEFAULT_ONLY', 'THRESHOLD'
    )),
    default_election TEXT,
    threshold_value NUMERIC(18,4),
    threshold_currency TEXT,
    notification_email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, instrument_class_id)
);

-- Instruction windows (deadline rules)
CREATE TABLE IF NOT EXISTS custody.cbu_ca_instruction_windows (
    window_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID REFERENCES custody.ca_event_types(event_type_id),
    market_id UUID REFERENCES custody.markets(market_id),
    cutoff_days_before INTEGER NOT NULL,
    warning_days INTEGER DEFAULT 3,
    escalation_days INTEGER DEFAULT 1,
    escalation_contact TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, market_id)
);

-- CA proceeds SSI mapping
CREATE TABLE IF NOT EXISTS custody.cbu_ca_ssi_mappings (
    mapping_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type_id UUID REFERENCES custody.ca_event_types(event_type_id),
    currency TEXT NOT NULL,
    proceeds_type TEXT NOT NULL CHECK (proceeds_type IN ('CASH', 'STOCK')),
    ssi_id UUID NOT NULL REFERENCES custody.cbu_ssi(ssi_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cbu_id, event_type_id, currency, proceeds_type)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_ca_preferences_cbu ON custody.cbu_ca_preferences(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ca_windows_cbu ON custody.cbu_ca_instruction_windows(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ca_ssi_cbu ON custody.cbu_ca_ssi_mappings(cbu_id);

-- ============================================================================
-- ISO 15022 Corporate Action Event Types (CAEV)
-- Complete reference catalog per SWIFT/DTCC/SMPG standards (53 codes)
-- ============================================================================

-- INCOME EVENTS (8)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('DVCA', 'Cash Dividend', 'INCOME', false, 'CASH', 'DVCA'),
    ('DVSE', 'Stock Dividend', 'INCOME', false, 'STOCK', 'DVSE'),
    ('DVOP', 'Dividend Option', 'INCOME', true, 'CASH', 'DVOP'),
    ('INTR', 'Interest Payment', 'INCOME', false, 'CASH', 'INTR'),
    ('CAPD', 'Capital Distribution', 'INCOME', false, 'CASH', 'CAPD'),
    ('CAPG', 'Capital Gains Distribution', 'INCOME', false, 'CASH', 'CAPG'),
    ('DRIP', 'Dividend Reinvestment Plan', 'INCOME', true, 'STOCK', 'DRIP'),
    ('PINK', 'Interest Payment in Kind', 'INCOME', false, 'STOCK', 'PINK')
ON CONFLICT (event_code) DO NOTHING;

-- REORGANIZATION EVENTS (11)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('MRGR', 'Merger', 'REORGANIZATION', false, NULL, 'MRGR'),
    ('SPLF', 'Stock Split (Forward)', 'REORGANIZATION', false, NULL, 'SPLF'),
    ('SPLR', 'Reverse Stock Split', 'REORGANIZATION', false, NULL, 'SPLR'),
    ('BONU', 'Bonus Issue/Capitalisation Issue', 'REORGANIZATION', false, 'STOCK', 'BONU'),
    ('EXOF', 'Exchange Offer', 'REORGANIZATION', true, 'DECLINE', 'EXOF'),
    ('CONS', 'Consent', 'REORGANIZATION', true, NULL, 'CONS'),
    ('CONV', 'Conversion', 'REORGANIZATION', true, 'STOCK', 'CONV'),
    ('PARI', 'Pari-Passu', 'REORGANIZATION', false, NULL, 'PARI'),
    ('REDO', 'Redenomination', 'REORGANIZATION', false, NULL, 'REDO'),
    ('DECR', 'Decrease in Value', 'REORGANIZATION', false, NULL, 'DECR'),
    ('SOFF', 'Spin-Off', 'REORGANIZATION', false, 'STOCK', 'SOFF')
ON CONFLICT (event_code) DO NOTHING;

-- VOLUNTARY EVENTS (7)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('RHTS', 'Rights Issue', 'VOLUNTARY', true, 'LAPSE', 'RHTS'),
    ('RHDI', 'Rights Distribution', 'VOLUNTARY', false, NULL, 'RHDI'),
    ('TEND', 'Tender/Takeover Offer', 'VOLUNTARY', true, 'DECLINE', 'TEND'),
    ('BIDS', 'Repurchase Offer/Issuer Bid', 'VOLUNTARY', true, 'CASH', 'BIDS'),
    ('BPUT', 'Put Redemption', 'VOLUNTARY', true, 'CASH', 'BPUT'),
    ('EXWA', 'Exercise of Warrants', 'VOLUNTARY', true, NULL, 'EXWA'),
    ('NOOF', 'Non-Official Offer', 'VOLUNTARY', true, 'DECLINE', 'NOOF')
ON CONFLICT (event_code) DO NOTHING;

-- REDEMPTION/MANDATORY EVENTS (6)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('REDM', 'Final Maturity/Redemption', 'MANDATORY', false, 'CASH', 'REDM'),
    ('MCAL', 'Full Call/Early Redemption', 'MANDATORY', false, 'CASH', 'MCAL'),
    ('PCAL', 'Partial Redemption (Nominal Reduction)', 'MANDATORY', false, 'CASH', 'PCAL'),
    ('PRED', 'Partial Redemption (No Nominal Change)', 'MANDATORY', false, 'CASH', 'PRED'),
    ('DRAW', 'Drawing', 'MANDATORY', false, 'CASH', 'DRAW'),
    ('PDEF', 'Prerefunding', 'MANDATORY', false, NULL, 'PDEF')
ON CONFLICT (event_code) DO NOTHING;

-- MEETINGS & INFORMATION EVENTS (6)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('OMET', 'Ordinary General Meeting', 'INFORMATION', false, NULL, 'OMET'),
    ('XMET', 'Extraordinary General Meeting', 'INFORMATION', false, NULL, 'XMET'),
    ('BMET', 'Bondholder Meeting', 'INFORMATION', false, NULL, 'BMET'),
    ('CMET', 'Court Meeting', 'INFORMATION', false, NULL, 'CMET'),
    ('INFO', 'Information Only', 'INFORMATION', false, NULL, 'INFO'),
    ('DSCL', 'Disclosure', 'INFORMATION', false, NULL, 'DSCL')
ON CONFLICT (event_code) DO NOTHING;

-- CREDIT/DEFAULT EVENTS (4)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('DFLT', 'Bond Default', 'MANDATORY', false, NULL, 'DFLT'),
    ('CREV', 'Credit Event', 'MANDATORY', false, NULL, 'CREV'),
    ('BRUP', 'Bankruptcy', 'MANDATORY', false, NULL, 'BRUP'),
    ('LIQU', 'Liquidation', 'MANDATORY', false, 'CASH', 'LIQU')
ON CONFLICT (event_code) DO NOTHING;

-- OTHER EVENTS (11)
INSERT INTO custody.ca_event_types (event_code, event_name, category, is_elective, default_election, iso_event_code) VALUES
    ('ATTI', 'Attachment', 'MANDATORY', false, NULL, 'ATTI'),
    ('CERT', 'Certification of Beneficial Ownership', 'VOLUNTARY', true, NULL, 'CERT'),
    ('CHAN', 'Change (Name/Domicile/etc)', 'MANDATORY', false, NULL, 'CHAN'),
    ('DETI', 'Detachment of Warrants', 'MANDATORY', false, NULL, 'DETI'),
    ('DRCA', 'Non-Eligible Securities Cash Distribution', 'MANDATORY', false, 'CASH', 'DRCA'),
    ('PPMT', 'Installment Call', 'MANDATORY', false, 'CASH', 'PPMT'),
    ('REMK', 'Remarketing Agreement', 'VOLUNTARY', true, NULL, 'REMK'),
    ('TREC', 'Tax Reclaim', 'VOLUNTARY', true, 'CASH', 'TREC'),
    ('WTRC', 'Withholding Tax Relief Certification', 'VOLUNTARY', true, NULL, 'WTRC'),
    ('ACCU', 'Accumulation', 'MANDATORY', false, NULL, 'ACCU'),
    ('CAPI', 'Capitalisation', 'MANDATORY', false, NULL, 'CAPI'),
    ('OTHR', 'Other (Unclassified)', 'INFORMATION', false, NULL, 'OTHR')
ON CONFLICT (event_code) DO NOTHING;

COMMENT ON TABLE custody.ca_event_types IS 'Reference catalog of corporate action event types';
COMMENT ON TABLE custody.cbu_ca_preferences IS 'CBU-specific CA processing preferences (written by materialize)';
COMMENT ON TABLE custody.cbu_ca_instruction_windows IS 'CBU deadline/cutoff rules for CA instructions';
COMMENT ON TABLE custody.cbu_ca_ssi_mappings IS 'Which SSI receives CA proceeds (cash/stock) per currency';
