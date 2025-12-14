-- Add unique constraint for movement idempotency
-- Conflict key: (holding_id, trade_date, reference)
-- This enables retry-safe fund transactions

ALTER TABLE kyc.movements 
ADD CONSTRAINT movements_holding_trade_date_reference_key 
UNIQUE (holding_id, trade_date, reference);

-- Make reference required (NOT NULL) for idempotency
-- Existing NULL references will need to be backfilled first
-- ALTER TABLE kyc.movements ALTER COLUMN reference SET NOT NULL;

COMMENT ON CONSTRAINT movements_holding_trade_date_reference_key ON kyc.movements IS 
'Idempotency key for movement transactions. Same holding + trade_date + reference = same transaction.';
