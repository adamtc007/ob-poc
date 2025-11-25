-- Seed test CBU for onboarding harness tests
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, status, created_at, updated_at)
VALUES (
    '11111111-1111-1111-1111-111111111111',
    'Test Fund Ltd',
    'LU',
    'ACTIVE',
    NOW(),
    NOW()
)
ON CONFLICT (cbu_id) DO UPDATE SET
    name = EXCLUDED.name,
    jurisdiction = EXCLUDED.jurisdiction,
    status = EXCLUDED.status,
    updated_at = NOW();
