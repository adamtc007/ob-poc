-- Seed test products for onboarding harness tests
INSERT INTO "ob-poc".products (product_id, product_code, name, product_category, is_active, created_at, updated_at)
VALUES
    ('22222222-2222-2222-2222-222222222222', 'GLOB_CUST', 'Global Custody', 'Custody', true, NOW(), NOW()),
    ('33333333-3333-3333-3333-333333333333', 'FUND_ADMIN', 'Fund Administration', 'Fund Services', true, NOW(), NOW()),
    ('44444444-4444-4444-4444-444444444444', 'PRIME_BROK', 'Prime Brokerage', 'Prime Services', true, NOW(), NOW())
ON CONFLICT (product_code) DO UPDATE SET
    name = EXCLUDED.name,
    product_category = EXCLUDED.product_category,
    is_active = EXCLUDED.is_active,
    updated_at = NOW();
