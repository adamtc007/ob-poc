-- SAFE and CONVERTIBLE_NOTE instruments have no defined share count until
-- they convert in a priced round -- units_granted = 0 is the correct
-- "not yet determined" value for them, not a placeholder. The prior
-- CHECK (units_granted > 0) made it impossible to create either
-- instrument type at all (EOP-PLAN share-register board design pass,
-- Phase 3: capital.dilution.create-safe / .create-convertible-note).
ALTER TABLE "ob-poc".dilution_instruments
    DROP CONSTRAINT dilution_instruments_chk_units_positive;

ALTER TABLE "ob-poc".dilution_instruments
    ADD CONSTRAINT dilution_instruments_chk_units_positive
    CHECK (units_granted >= 0);
