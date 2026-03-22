-- ============================================================================
-- Seed Allianz Onboarding Pipeline Data
-- ============================================================================
-- Creates diverse lifecycle states for composite state testing:
--   CBU 1 (Dynamic Commodities): VALIDATED + case APPROVED + screenings COMPLETED
--   CBU 2 (Thematica): DISCOVERED + case INTAKE + workstreams PENDING
--   CBU 3 (AADB Fonds): DISCOVERED + no case (fresh)
-- ============================================================================

BEGIN;

-- ── CBU 1: Advance to VALIDATED ──────────────────────────────────
UPDATE "ob-poc".cbus SET status = 'VALIDATED'
WHERE cbu_id = 'd8e53359-7a75-47de-9ff5-50c2a9fc00bc';

-- Create KYC case for CBU 1 (APPROVED — fully reviewed)
INSERT INTO "ob-poc".cases (case_id, cbu_id, status, case_type, notes)
VALUES (
  'a1000001-0001-0001-0001-000000000001',
  'd8e53359-7a75-47de-9ff5-50c2a9fc00bc',
  'APPROVED',
  'NEW_CLIENT',
  'Allianz Dynamic Commodities — full onboarding complete'
) ON CONFLICT (case_id) DO NOTHING;

-- Create entity workstreams for CBU 1 (COMPLETED)
INSERT INTO "ob-poc".entity_workstreams (workstream_id, case_id, entity_id, status, completed_at)
SELECT
  gen_random_uuid(),
  'a1000001-0001-0001-0001-000000000001',
  sub.entity_id,
  'COMPLETE',
  NOW()
FROM (
  SELECT DISTINCT r.entity_id
  FROM "ob-poc".cbu_entity_roles r
  WHERE r.cbu_id = 'd8e53359-7a75-47de-9ff5-50c2a9fc00bc'
) sub
ON CONFLICT DO NOTHING;

-- Create screenings for CBU 1 (via workstream_id)
INSERT INTO "ob-poc".screenings (screening_id, workstream_id, screening_type, status, completed_at, result_summary)
SELECT
  gen_random_uuid(),
  ew.workstream_id,
  'SANCTIONS',
  'CLEAR',
  NOW(),
  'CLEAR'
FROM "ob-poc".entity_workstreams ew
WHERE ew.case_id = 'a1000001-0001-0001-0001-000000000001'
ON CONFLICT DO NOTHING;

INSERT INTO "ob-poc".screenings (screening_id, workstream_id, screening_type, status, completed_at, result_summary)
SELECT
  gen_random_uuid(),
  ew.workstream_id,
  'PEP',
  'CLEAR',
  NOW(),
  'CLEAR'
FROM "ob-poc".entity_workstreams ew
WHERE ew.case_id = 'a1000001-0001-0001-0001-000000000001'
ON CONFLICT DO NOTHING;

-- ── CBU 2: Case open, no screening ───────────────────────────────
INSERT INTO "ob-poc".cases (case_id, cbu_id, status, case_type, notes)
VALUES (
  'a1000002-0002-0002-0002-000000000002',
  'aba01260-fffe-448d-8077-5381de864ee4',
  'INTAKE',
  'NEW_CLIENT',
  'Allianz Thematica — case opened, pending screening'
) ON CONFLICT (case_id) DO NOTHING;

-- Create entity workstreams for CBU 2 (PENDING)
INSERT INTO "ob-poc".entity_workstreams (workstream_id, case_id, entity_id, status)
SELECT
  gen_random_uuid(),
  'a1000002-0002-0002-0002-000000000002',
  sub.entity_id,
  'PENDING'
FROM (
  SELECT DISTINCT r.entity_id
  FROM "ob-poc".cbu_entity_roles r
  WHERE r.cbu_id = 'aba01260-fffe-448d-8077-5381de864ee4'
) sub
ON CONFLICT DO NOTHING;

-- ── CBU 3: Fresh — no case, no screening ─────────────────────────
-- Allianz AADB Fonds stays as-is: DISCOVERED, no case

COMMIT;

-- ── Verify ───────────────────────────────────────────────────────
SELECT c.name, c.status,
  (SELECT COUNT(*) FROM "ob-poc".cases k WHERE k.cbu_id = c.cbu_id) as cases,
  (SELECT COALESCE(k.status, 'none') FROM "ob-poc".cases k WHERE k.cbu_id = c.cbu_id LIMIT 1) as case_status,
  (SELECT COUNT(*) FROM "ob-poc".entity_workstreams ew
   JOIN "ob-poc".cases k ON k.case_id = ew.case_id
   WHERE k.cbu_id = c.cbu_id) as workstreams,
  (SELECT COUNT(*) FROM "ob-poc".screenings s
   JOIN "ob-poc".entity_workstreams ew ON ew.workstream_id = s.workstream_id
   JOIN "ob-poc".cases k ON k.case_id = ew.case_id
   WHERE k.cbu_id = c.cbu_id) as screenings
FROM "ob-poc".cbus c
WHERE c.cbu_id IN (
  'd8e53359-7a75-47de-9ff5-50c2a9fc00bc',
  'aba01260-fffe-448d-8077-5381de864ee4',
  '5429f3cb-7442-4b27-acf9-ef6cdf6a13b6'
)
ORDER BY c.name;
