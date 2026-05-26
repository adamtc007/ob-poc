-- Form schema registry.
-- Stores Form.io schema JSON keyed by ref slug.
-- Served by GET /api/forms/:ref; the React FormioForm component fetches on mount.

CREATE TABLE form_schemas (
    ref         TEXT PRIMARY KEY,
    schema      JSONB NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed: fixture schemas shipped with the dsl.form transpiler integration.
INSERT INTO form_schemas (ref, schema) VALUES
(
    'kyc.review-summary',
    '{
        "title": "KYC Review Summary",
        "display": "form",
        "components": [
            {
                "type": "panel",
                "title": "Customer Information",
                "components": [
                    {"type": "textfield", "key": "entityName", "label": "Entity Name", "disabled": true},
                    {"type": "textfield", "key": "riskTier", "label": "Risk Tier", "disabled": true},
                    {"type": "textfield", "key": "kycStatus", "label": "KYC Status", "disabled": true}
                ]
            },
            {"type": "button", "action": "submit", "label": "Continue", "theme": "primary"}
        ]
    }'::jsonb
),
(
    'onboarding.document-checklist',
    '{
        "title": "Document Checklist",
        "display": "form",
        "components": [
            {
                "type": "panel",
                "title": "Required Documents",
                "components": [
                    {"type": "checkbox", "key": "certificateOfIncorporation", "label": "Certificate of Incorporation"},
                    {"type": "checkbox", "key": "memorandumOfAssociation", "label": "Memorandum and Articles of Association"},
                    {"type": "checkbox", "key": "proofOfAddress", "label": "Proof of Registered Address"},
                    {"type": "checkbox", "key": "beneficialOwnershipDeclaration", "label": "Beneficial Ownership Declaration"}
                ]
            },
            {"type": "textarea", "key": "notes", "label": "Additional Notes", "placeholder": "Any exceptions or observations..."},
            {"type": "button", "action": "submit", "label": "Submit Checklist", "theme": "primary"}
        ]
    }'::jsonb
);
