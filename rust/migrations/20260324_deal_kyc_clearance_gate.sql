-- Migration: add pre-contract KYC clearance state to the deal lifecycle.

ALTER TABLE "ob-poc".deals
DROP CONSTRAINT IF EXISTS deals_status_check;

ALTER TABLE "ob-poc".deals
ADD CONSTRAINT deals_status_check CHECK (deal_status IN (
    'PROSPECT',
    'QUALIFYING',
    'NEGOTIATING',
    'KYC_CLEARANCE',
    'CONTRACTED',
    'ONBOARDING',
    'ACTIVE',
    'WINDING_DOWN',
    'OFFBOARDED',
    'CANCELLED'
));

COMMENT ON COLUMN "ob-poc".deals.deal_status IS
'PROSPECT | QUALIFYING | NEGOTIATING | KYC_CLEARANCE | CONTRACTED | ONBOARDING | ACTIVE | WINDING_DOWN | OFFBOARDED | CANCELLED';
