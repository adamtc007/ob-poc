-- =============================================================================
-- Custody Domain Seed Data
--
-- Industry-standard taxonomies and reference data:
-- - Instrument classes with CFI/SMPG/ISDA mappings
-- - Security types (ALERT codes)
-- - ISDA product taxonomy
-- - Currencies
-- - Markets
-- - Instruction types
-- =============================================================================

-- =============================================================================
-- INSTRUMENT CLASSES (with CFI, SMPG, ISDA mappings)
-- =============================================================================
INSERT INTO custody.instrument_classes
(code, name, default_settlement_cycle, swift_message_family, cfi_category, cfi_group, smpg_group, isda_asset_class, requires_isda)
VALUES
-- Cash Securities
('EQUITY', 'Equities', 'T+1', 'MT54x', 'E', 'ES', 'EQU', NULL, false),
('EQUITY_ADR', 'American Depositary Receipts', 'T+1', 'MT54x', 'E', 'ED', 'EQU', NULL, false),
('EQUITY_ETF', 'Exchange Traded Funds', 'T+1', 'MT54x', 'C', 'CI', 'EQU', NULL, false),
('FIXED_INCOME', 'Fixed Income', 'T+1', 'MT54x', 'D', 'DB', 'Corp FI', NULL, false),
('GOVT_BOND', 'Government Bonds', 'T+1', 'MT54x', 'D', 'DB', 'Govt FI', NULL, false),
('CORP_BOND', 'Corporate Bonds', 'T+2', 'MT54x', 'D', 'DB', 'Corp FI', NULL, false),
('MONEY_MARKET', 'Money Market', 'T+0', 'MT54x', 'D', 'DY', 'MM', NULL, false),
-- OTC Derivatives
('OTC_IRS', 'Interest Rate Swaps', 'T+0', NULL, 'S', 'SR', 'DERIV', 'InterestRate', true),
('OTC_CDS', 'Credit Default Swaps', 'T+0', NULL, 'S', 'SC', 'DERIV', 'Credit', true),
('OTC_EQD', 'Equity Derivatives', 'T+0', NULL, 'S', 'SE', 'DERIV', 'Equity', true),
('OTC_FX', 'FX Derivatives', 'T+0', NULL, 'S', 'SF', 'DERIV', 'ForeignExchange', true),
-- FX
('FX_SPOT', 'FX Spot', 'T+2', 'MT3xx', 'J', 'JF', 'FX/CSH', 'ForeignExchange', false),
('FX_FORWARD', 'FX Forward', 'T+2', 'MT3xx', 'K', 'KF', 'FX/CSH', 'ForeignExchange', false)
ON CONFLICT (code) DO NOTHING;

-- Set parent relationships
UPDATE custody.instrument_classes SET parent_class_id =
    (SELECT class_id FROM custody.instrument_classes WHERE code = 'EQUITY')
WHERE code IN ('EQUITY_ADR', 'EQUITY_ETF');

UPDATE custody.instrument_classes SET parent_class_id =
    (SELECT class_id FROM custody.instrument_classes WHERE code = 'FIXED_INCOME')
WHERE code IN ('GOVT_BOND', 'CORP_BOND', 'MONEY_MARKET');

-- =============================================================================
-- SECURITY TYPES (ALERT/SMPG codes)
-- =============================================================================
INSERT INTO custody.security_types (class_id, code, name, cfi_pattern)
SELECT ic.class_id, t.code, t.name, t.cfi_pattern
FROM (VALUES
    ('EQUITY', 'EQU', 'Equities', 'ES****'),
    ('EQUITY', 'ADR', 'American Depositary Receipt', 'ED****'),
    ('EQUITY', 'GDR', 'Global Depositary Receipt', 'ED****'),
    ('EQUITY_ETF', 'ETF', 'Exchange Traded Fund', 'CI****'),
    ('EQUITY', 'PRS', 'Preference Shares', 'EP****'),
    ('EQUITY', 'RTS', 'Rights', 'RA****'),
    ('EQUITY', 'UIT', 'Unit Investment Trust', 'CI****'),
    ('FIXED_INCOME', 'COB', 'Corporate Bond', 'DB****'),
    ('FIXED_INCOME', 'ABS', 'Asset Backed Security', 'DA****'),
    ('FIXED_INCOME', 'MBS', 'Mortgage Backed Security', 'DM****'),
    ('FIXED_INCOME', 'CMO', 'Collateralized Mortgage Obligation', 'DM****'),
    ('FIXED_INCOME', 'CON', 'Convertible Bond', 'DC****'),
    ('GOVT_BOND', 'TRY', 'Treasuries', 'DB****'),
    ('GOVT_BOND', 'AGS', 'Agencies', 'DB****'),
    ('GOVT_BOND', 'MNB', 'Municipal Bond', 'DB****'),
    ('MONEY_MARKET', 'MMT', 'Money Market', 'DY****'),
    ('MONEY_MARKET', 'COD', 'Certificate of Deposit', 'DY****'),
    ('MONEY_MARKET', 'COM', 'Commercial Paper', 'DY****'),
    ('MONEY_MARKET', 'REP', 'Repurchase Agreement', 'LR****'),
    ('OTC_IRS', 'IRS', 'Interest Rate Swap', 'SR****'),
    ('OTC_CDS', 'CDS', 'Credit Default Swap', 'SC****'),
    ('OTC_EQD', 'TRS', 'Total Return Swap', 'SE****'),
    ('FX_SPOT', 'CSH', 'Cash', 'JF****'),
    ('FX_SPOT', 'F/X', 'Foreign Exchange', 'JF****')
) AS t(class_code, code, name, cfi_pattern)
JOIN custody.instrument_classes ic ON ic.code = t.class_code
ON CONFLICT (code) DO NOTHING;

-- =============================================================================
-- ISDA PRODUCT TAXONOMY
-- =============================================================================
INSERT INTO custody.isda_product_taxonomy
(asset_class, base_product, sub_product, taxonomy_code, class_id)
SELECT t.asset_class, t.base_product, t.sub_product, t.taxonomy_code, ic.class_id
FROM (VALUES
    ('InterestRate', 'IRSwap', 'FixedFloat', 'InterestRate:IRSwap:FixedFloat', 'OTC_IRS'),
    ('InterestRate', 'IRSwap', 'Basis', 'InterestRate:IRSwap:Basis', 'OTC_IRS'),
    ('InterestRate', 'IRSwap', 'OIS', 'InterestRate:IRSwap:OIS', 'OTC_IRS'),
    ('InterestRate', 'IRSwap', 'CrossCurrency', 'InterestRate:IRSwap:CrossCurrency', 'OTC_IRS'),
    ('InterestRate', 'Swaption', NULL, 'InterestRate:Swaption', 'OTC_IRS'),
    ('InterestRate', 'Cap-Floor', NULL, 'InterestRate:Cap-Floor', 'OTC_IRS'),
    ('InterestRate', 'FRA', NULL, 'InterestRate:FRA', 'OTC_IRS'),
    ('Credit', 'CreditDefaultSwap', 'SingleName', 'Credit:CDS:SingleName', 'OTC_CDS'),
    ('Credit', 'CreditDefaultSwap', 'Index', 'Credit:CDS:Index', 'OTC_CDS'),
    ('Credit', 'TotalReturnSwap', NULL, 'Credit:TRS', 'OTC_CDS'),
    ('Equity', 'EquitySwap', 'PriceReturn', 'Equity:Swap:PriceReturn', 'OTC_EQD'),
    ('Equity', 'EquitySwap', 'TotalReturn', 'Equity:Swap:TotalReturn', 'OTC_EQD'),
    ('Equity', 'EquityOption', 'Vanilla', 'Equity:Option:Vanilla', 'OTC_EQD'),
    ('ForeignExchange', 'FXSpot', NULL, 'FX:Spot', 'FX_SPOT'),
    ('ForeignExchange', 'FXForward', 'Deliverable', 'FX:Forward:Deliverable', 'FX_FORWARD'),
    ('ForeignExchange', 'FXForward', 'NDF', 'FX:Forward:NDF', 'FX_FORWARD'),
    ('ForeignExchange', 'FXSwap', NULL, 'FX:Swap', 'OTC_FX'),
    ('ForeignExchange', 'FXOption', 'Vanilla', 'FX:Option:Vanilla', 'OTC_FX')
) AS t(asset_class, base_product, sub_product, taxonomy_code, class_code)
JOIN custody.instrument_classes ic ON ic.code = t.class_code
ON CONFLICT (taxonomy_code) DO NOTHING;

-- =============================================================================
-- CURRENCIES
-- =============================================================================
INSERT INTO custody.currencies (iso_code, name, decimal_places, is_cls_eligible) VALUES
('USD', 'US Dollar', 2, true),
('EUR', 'Euro', 2, true),
('GBP', 'British Pound', 2, true),
('JPY', 'Japanese Yen', 0, true),
('CHF', 'Swiss Franc', 2, true),
('CAD', 'Canadian Dollar', 2, true),
('AUD', 'Australian Dollar', 2, true),
('HKD', 'Hong Kong Dollar', 2, true),
('SGD', 'Singapore Dollar', 2, true),
('MXN', 'Mexican Peso', 2, true),
('NZD', 'New Zealand Dollar', 2, true),
('SEK', 'Swedish Krona', 2, true),
('NOK', 'Norwegian Krone', 2, true),
('DKK', 'Danish Krone', 2, true),
('ZAR', 'South African Rand', 2, true),
('ILS', 'Israeli Shekel', 2, true),
('KRW', 'South Korean Won', 0, true)
ON CONFLICT (iso_code) DO NOTHING;

-- =============================================================================
-- MARKETS
-- =============================================================================
INSERT INTO custody.markets (mic, name, country_code, primary_currency, supported_currencies, csd_bic, timezone) VALUES
('XNYS', 'New York Stock Exchange', 'US', 'USD', '{}', 'DTCYUS33', 'America/New_York'),
('XNAS', 'NASDAQ', 'US', 'USD', '{}', 'DTCYUS33', 'America/New_York'),
('XLON', 'London Stock Exchange', 'GB', 'GBP', '{USD,EUR}', 'CABOROCP', 'Europe/London'),
('XPAR', 'Euronext Paris', 'FR', 'EUR', '{}', 'SICABOROCP', 'Europe/Paris'),
('XETR', 'Deutsche Boerse Xetra', 'DE', 'EUR', '{}', 'DAKVDEFF', 'Europe/Berlin'),
('XAMS', 'Euronext Amsterdam', 'NL', 'EUR', '{}', 'ECABOROCP', 'Europe/Amsterdam'),
('XSWX', 'SIX Swiss Exchange', 'CH', 'CHF', '{EUR}', 'SABOROCP', 'Europe/Zurich'),
('XTKS', 'Tokyo Stock Exchange', 'JP', 'JPY', '{}', 'JASDECTK', 'Asia/Tokyo'),
('XHKG', 'Hong Kong Stock Exchange', 'HK', 'HKD', '{USD}', 'CCABOROCP', 'Asia/Hong_Kong'),
('XSES', 'Singapore Exchange', 'SG', 'SGD', '{USD}', 'CDABOROCP', 'Asia/Singapore'),
('XASX', 'Australian Securities Exchange', 'AU', 'AUD', '{}', 'CHESAU2S', 'Australia/Sydney'),
('XTSE', 'Toronto Stock Exchange', 'CA', 'CAD', '{USD}', 'CDSLCA2O', 'America/Toronto')
ON CONFLICT (mic) DO NOTHING;

-- =============================================================================
-- INSTRUCTION TYPES
-- =============================================================================
INSERT INTO custody.instruction_types (type_code, name, direction, payment_type, swift_mt_code, iso20022_msg_type) VALUES
('RECEIVE_FOP', 'Receive Free of Payment', 'RECEIVE', 'FOP', 'MT540', 'sese.023'),
('RECEIVE_DVP', 'Receive vs Payment', 'RECEIVE', 'DVP', 'MT541', 'sese.023'),
('DELIVER_FOP', 'Deliver Free of Payment', 'DELIVER', 'FOP', 'MT542', 'sese.023'),
('DELIVER_DVP', 'Deliver vs Payment', 'DELIVER', 'DVP', 'MT543', 'sese.023'),
('RECEIVE_RVP', 'Receive vs Payment (Repo)', 'RECEIVE', 'RVP', 'MT541', 'sese.023'),
('DELIVER_DFP', 'Deliver Free of Payment (Repo)', 'DELIVER', 'DFP', 'MT542', 'sese.023')
ON CONFLICT (type_code) DO NOTHING;
