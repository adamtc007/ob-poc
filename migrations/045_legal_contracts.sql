-- Migration: 045_legal_contracts
-- Legal contracts with product-level rate cards
-- Join key: client_label (same as cbus.client_label, entities.client_label)

-- Master contract table
CREATE TABLE IF NOT EXISTS "ob-poc".legal_contracts (
    contract_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_label VARCHAR(100) NOT NULL,
    contract_reference VARCHAR(100),  -- External contract number
    effective_date DATE NOT NULL,
    termination_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('DRAFT', 'ACTIVE', 'TERMINATED', 'EXPIRED')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_legal_contracts_client_label
    ON "ob-poc".legal_contracts(client_label);

CREATE INDEX IF NOT EXISTS idx_legal_contracts_status
    ON "ob-poc".legal_contracts(status);

-- Contract products with rate cards
CREATE TABLE IF NOT EXISTS "ob-poc".contract_products (
    contract_id UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id) ON DELETE CASCADE,
    product_code VARCHAR(50) NOT NULL,
    rate_card_id UUID,
    effective_date DATE,
    termination_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (contract_id, product_code)
);

CREATE INDEX IF NOT EXISTS idx_contract_products_product_code
    ON "ob-poc".contract_products(product_code);

-- Rate cards (reference table)
CREATE TABLE IF NOT EXISTS "ob-poc".rate_cards (
    rate_card_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    currency VARCHAR(3) DEFAULT 'USD',
    effective_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- View: contracts with products
CREATE OR REPLACE VIEW "ob-poc".v_contract_summary AS
SELECT
    c.contract_id,
    c.client_label,
    c.contract_reference,
    c.effective_date,
    c.status,
    COUNT(cp.product_code) as product_count,
    ARRAY_AGG(cp.product_code ORDER BY cp.product_code) FILTER (WHERE cp.product_code IS NOT NULL) as products
FROM "ob-poc".legal_contracts c
LEFT JOIN "ob-poc".contract_products cp ON cp.contract_id = c.contract_id
GROUP BY c.contract_id, c.client_label, c.contract_reference, c.effective_date, c.status;

-- CBU subscriptions to contract+product (the onboarding gate)
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_subscriptions (
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    contract_id UUID NOT NULL,
    product_code VARCHAR(50) NOT NULL,
    subscribed_at TIMESTAMPTZ DEFAULT NOW(),
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('PENDING', 'ACTIVE', 'SUSPENDED', 'TERMINATED')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (cbu_id, contract_id, product_code),
    FOREIGN KEY (contract_id, product_code) REFERENCES "ob-poc".contract_products(contract_id, product_code) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_cbu_subscriptions_contract
    ON "ob-poc".cbu_subscriptions(contract_id, product_code);

-- View: CBU with subscriptions
CREATE OR REPLACE VIEW "ob-poc".v_cbu_subscriptions AS
SELECT
    s.cbu_id,
    c.name as cbu_name,
    c.client_label,
    lc.contract_id,
    lc.contract_reference,
    s.product_code,
    cp.rate_card_id,
    s.status as subscription_status,
    s.subscribed_at
FROM "ob-poc".cbu_subscriptions s
JOIN "ob-poc".cbus c ON c.cbu_id = s.cbu_id
JOIN "ob-poc".legal_contracts lc ON lc.contract_id = s.contract_id
JOIN "ob-poc".contract_products cp ON cp.contract_id = s.contract_id AND cp.product_code = s.product_code;

-- Seed sample data for Allianz
INSERT INTO "ob-poc".legal_contracts (client_label, contract_reference, effective_date, status)
VALUES ('allianz', 'MSA-ALZ-2020-001', '2020-01-01', 'ACTIVE')
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".legal_contracts (client_label, contract_reference, effective_date, status)
VALUES ('bridgewater', 'MSA-BW-2021-001', '2021-06-01', 'ACTIVE')
ON CONFLICT DO NOTHING;
