-- ============================================
-- Seed Core Vocabulary
-- ============================================
BEGIN;

-- KYC Domain Verbs
INSERT INTO "ob-poc".vocabulary_registry 
    (verb_name, domain, action, signature, description, operation_types, parameter_schema, examples)
VALUES
    ('kyc.declare-entity', 'kyc', 'declare-entity', 
     ':entity-type STRING :name STRING :data MAP', 
     'Declare a new entity for KYC processing',
     ARRAY['CREATE', 'DECLARE'],
     '{"entity-type": {"type": "string", "enum": ["PERSON", "COMPANY", "TRUST"]}, "name": {"type": "string", "minLength": 1}, "data": {"type": "object"}}'::jsonb,
     '[{"example": "(kyc.declare-entity :entity-type \"PERSON\" :name \"John Doe\" :data {})", "description": "Declare a natural person"}, {"example": "(kyc.declare-entity :entity-type \"COMPANY\" :name \"TechCorp Ltd\" :data {:jurisdiction \"UK\"})", "description": "Declare a company entity"}]'::jsonb),
     
    ('kyc.obtain-document', 'kyc', 'obtain-document',
     ':document-type STRING :from UUID', 
     'Obtain a document from an entity for KYC verification',
     ARRAY['READ', 'FETCH'],
     '{"document-type": {"type": "string", "enum": ["passport", "drivers-license", "utility-bill", "bank-statement"]}, "from": {"type": "uuid"}}'::jsonb,
     '[{"example": "(kyc.obtain-document :document-type \"passport\" :from @entity(12345678-...))", "description": "Request passport from entity"}]'::jsonb),
     
    ('kyc.verify-document', 'kyc', 'verify-document',
     ':document-id UUID :verification-method STRING',
     'Verify a KYC document using specified method',
     ARRAY['UPDATE', 'VERIFY'],
     '{"document-id": {"type": "uuid"}, "verification-method": {"type": "string", "enum": ["manual", "ocr", "third-party"]}}'::jsonb,
     '[{"example": "(kyc.verify-document :document-id @doc(id) :verification-method \"ocr\")", "description": "Verify document with OCR"}]'::jsonb)
ON CONFLICT (verb_name) DO UPDATE SET
    description = EXCLUDED.description,
    examples = EXCLUDED.examples,
    updated_at = NOW();

-- CBU Domain Verbs
INSERT INTO "ob-poc".vocabulary_registry 
    (verb_name, domain, action, signature, description, operation_types, parameter_schema, examples)
VALUES
    ('cbu.create', 'cbu', 'create',
     ':cbu-name STRING :client-type STRING :jurisdiction STRING :nature-purpose STRING :description STRING',
     'Create a new Client Business Unit',
     ARRAY['CREATE'],
     '{"cbu-name": {"type": "string"}, "client-type": {"type": "string"}, "jurisdiction": {"type": "string"}, "nature-purpose": {"type": "string"}, "description": {"type": "string"}}'::jsonb,
     '[{"example": "(cbu.create :cbu-name \"TechCorp\" :client-type \"HEDGE_FUND\" :jurisdiction \"GB\" :nature-purpose \"Investment\" :description \"Details\")", "description": "Create a hedge fund CBU"}]'::jsonb),

    ('cbu.read', 'cbu', 'read',
     ':cbu-id UUID',
     'Read a CBU by ID',
     ARRAY['READ'],
     '{"cbu-id": {"type": "uuid"}}'::jsonb,
     '[{"example": "(cbu.read :cbu-id \"uuid\")", "description": "Read CBU details"}]'::jsonb),

    ('cbu.update', 'cbu', 'update',
     ':cbu-id UUID :name STRING',
     'Update a CBU',
     ARRAY['UPDATE'],
     '{"cbu-id": {"type": "uuid"}, "name": {"type": "string"}}'::jsonb,
     '[{"example": "(cbu.update :cbu-id \"uuid\" :name \"New Name\")", "description": "Update CBU name"}]'::jsonb),

    ('cbu.delete', 'cbu', 'delete',
     ':cbu-id UUID',
     'Delete a CBU',
     ARRAY['DELETE'],
     '{"cbu-id": {"type": "uuid"}}'::jsonb,
     '[{"example": "(cbu.delete :cbu-id \"uuid\")", "description": "Delete a CBU"}]'::jsonb),

    ('cbu.submit', 'cbu', 'submit',
     ':cbu-id UUID :chunks ARRAY',
     'Submit CBU for approval with specified attribute chunks',
     ARRAY['UPDATE', 'TRANSITION'],
     '{"cbu-id": {"type": "uuid"}, "chunks": {"type": "array", "items": {"type": "string"}}}'::jsonb,
     '[{"example": "(cbu.submit :cbu-id @cbu(id) :chunks [\"core\" \"contact\"])", "description": "Submit CBU with core and contact chunks"}]'::jsonb),
     
    ('cbu.approve', 'cbu', 'approve',
     ':cbu-id UUID :approver STRING',
     'Approve a CBU for activation',
     ARRAY['UPDATE', 'TRANSITION'],
     '{"cbu-id": {"type": "uuid"}, "approver": {"type": "string"}}'::jsonb,
     '[{"example": "(cbu.approve :cbu-id @cbu(id) :approver \"compliance-officer\")", "description": "Approve CBU by compliance officer"}]'::jsonb),
     
    ('cbu.decline', 'cbu', 'decline',
     ':cbu-id UUID :reason STRING',
     'Decline a CBU application',
     ARRAY['UPDATE', 'TRANSITION'],
     '{"cbu-id": {"type": "uuid"}, "reason": {"type": "string", "minLength": 10}}'::jsonb,
     '[{"example": "(cbu.decline :cbu-id @cbu(id) :reason \"Incomplete KYC documentation\")", "description": "Decline CBU with reason"}]'::jsonb),
     
    ('cbu.suspend', 'cbu', 'suspend',
     ':cbu-id UUID :reason STRING',
     'Suspend an active CBU',
     ARRAY['UPDATE', 'TRANSITION'],
     '{"cbu-id": {"type": "uuid"}, "reason": {"type": "string", "minLength": 10}}'::jsonb,
     '[{"example": "(cbu.suspend :cbu-id @cbu(id) :reason \"Compliance review required\")", "description": "Suspend CBU for compliance review"}]'::jsonb)
ON CONFLICT (verb_name) DO UPDATE SET
    description = EXCLUDED.description,
    examples = EXCLUDED.examples,
    updated_at = NOW();

-- Document Domain Verbs
INSERT INTO "ob-poc".vocabulary_registry 
    (verb_name, domain, action, signature, description, operation_types, parameter_schema, examples)
VALUES
    ('document.catalog', 'document', 'catalog',
     ':doc-id STRING :doc-type STRING',
     'Catalog a document',
     ARRAY['CREATE'],
     '{"doc-id": {"type": "string"}, "doc-type": {"type": "string"}}'::jsonb,
     '[{"example": "(document.catalog :doc-id \"DOC-001\" :doc-type \"UK-PASSPORT\")", "description": "Catalog a UK passport"}]'::jsonb),

    ('document.verify', 'document', 'verify',
     ':doc-id STRING :status STRING',
     'Verify a document',
     ARRAY['UPDATE'],
     '{"doc-id": {"type": "string"}, "status": {"type": "string"}}'::jsonb,
     '[{"example": "(document.verify :doc-id \"DOC-001\" :status \"verified\")", "description": "Mark document as verified"}]'::jsonb),

    ('document.extract', 'document', 'extract',
     ':doc-id STRING :attr-id UUID',
     'Extract attribute from document',
     ARRAY['READ'],
     '{"doc-id": {"type": "string"}, "attr-id": {"type": "uuid"}}'::jsonb,
     '[{"example": "(document.extract :doc-id \"DOC-001\" :attr-id \"uuid\")", "description": "Extract attribute from document"}]'::jsonb),

    ('document.link', 'document', 'link',
     ':primary-doc UUID :related-doc UUID :type STRING',
     'Link two documents',
     ARRAY['CREATE'],
     '{"primary-doc": {"type": "uuid"}, "related-doc": {"type": "uuid"}, "type": {"type": "string"}}'::jsonb,
     '[{"example": "(document.link :primary-doc \"uuid1\" :related-doc \"uuid2\" :type \"SUPPORTS\")", "description": "Link supporting documents"}]'::jsonb)
ON CONFLICT (verb_name) DO UPDATE SET
    description = EXCLUDED.description,
    examples = EXCLUDED.examples,
    updated_at = NOW();

-- Attribute Domain Verbs
INSERT INTO "ob-poc".vocabulary_registry 
    (verb_name, domain, action, signature, description, operation_types, parameter_schema, examples)
VALUES
    ('attr.bind', 'attr', 'bind',
     ':attribute-id UUID :value ANY :source STRING',
     'Bind an attribute value to a CBU or entity',
     ARRAY['CREATE', 'UPDATE'],
     '{"attribute-id": {"type": "uuid"}, "value": {"type": ["string", "number", "boolean", "object"]}, "source": {"type": "string"}}'::jsonb,
     '[{"example": "(attr.bind :attribute-id @attr(\"CBU.LEGAL_NAME\") :value \"TechCorp Ltd\" :source \"user-input\")", "description": "Bind legal name attribute"}]'::jsonb),
     
    ('attr.validate', 'attr', 'validate',
     ':attribute-id UUID :value ANY',
     'Validate an attribute value against its schema',
     ARRAY['VALIDATE'],
     '{"attribute-id": {"type": "uuid"}, "value": {"type": ["string", "number", "boolean", "object"]}}'::jsonb,
     '[{"example": "(attr.validate :attribute-id @attr(\"CBU.EMAIL\") :value \"test@example.com\")", "description": "Validate email attribute"}]'::jsonb)
ON CONFLICT (verb_name) DO UPDATE SET
    description = EXCLUDED.description,
    examples = EXCLUDED.examples,
    updated_at = NOW();

COMMIT;
