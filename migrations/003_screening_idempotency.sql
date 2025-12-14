-- Add unique constraint for screening idempotency
-- Conflict key: (workstream_id, screening_type)
-- One screening of each type per workstream

ALTER TABLE kyc.screenings 
ADD CONSTRAINT screenings_workstream_type_key 
UNIQUE (workstream_id, screening_type);

COMMENT ON CONSTRAINT screenings_workstream_type_key ON kyc.screenings IS 
'Idempotency key for screenings. One screening per type per workstream.';
