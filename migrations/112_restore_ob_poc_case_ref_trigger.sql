-- Migration 112: Restore case_ref auto-generation in consolidated "ob-poc" schema.
--
-- After legacy schema removal, `cases.case_ref` remains NOT NULL but the trigger
-- that generated case refs no longer exists. Recreate sequence/function/trigger
-- under "ob-poc".

CREATE SEQUENCE IF NOT EXISTS "ob-poc".case_ref_seq START WITH 200;

CREATE OR REPLACE FUNCTION "ob-poc".generate_case_ref()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.case_ref IS NULL THEN
        NEW.case_ref := 'KYC-'
            || EXTRACT(YEAR FROM COALESCE(NEW.opened_at, NOW()))::TEXT
            || '-'
            || LPAD(nextval('"ob-poc".case_ref_seq')::TEXT, 4, '0');
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_case_ref ON "ob-poc".cases;
CREATE TRIGGER trg_case_ref
    BEFORE INSERT ON "ob-poc".cases
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".generate_case_ref();
