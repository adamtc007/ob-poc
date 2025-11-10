-- 11_document_verbs_basic.sql
-- Document Library DSL Verbs - Basic Implementation
--
-- This script adds the document library domain and verbs to enable
-- first-class document management in DSL workflows.

-- ============================================================================
-- DOCUMENT DOMAIN REGISTRATION
-- ============================================================================

-- Register the document domain
INSERT INTO "ob-poc".dsl_domains (domain_name, description, base_grammar_version, vocabulary_version)
VALUES ('Document', 'Document library and management workflows', '3.0.0', '1.0.0')
ON CONFLICT (domain_name) DO NOTHING;

-- ============================================================================
-- DOCUMENT LIBRARY VERBS
-- ============================================================================

-- Core document management verbs
INSERT INTO "ob-poc".domain_vocabularies (domain, verb, category, description, parameters, examples, active) VALUES

-- Document cataloging
('document', 'document.catalog', 'document_management', 'Add a document to the centralized library with rich metadata',
 '{
   ":doc-id": {"type": "string", "required": true, "description": "Unique document identifier"},
   ":doc-type": {"type": "string", "required": true, "description": "Document type code from document_types table"},
   ":title": {"type": "string", "required": false, "description": "Document title"},
   ":issuer": {"type": "string", "required": false, "description": "Issuing authority code"},
   ":issue-date": {"type": "date", "required": false, "description": "Date document was issued"},
   ":expiry-date": {"type": "date", "required": false, "description": "Date document expires"},
   ":file-path": {"type": "string", "required": false, "description": "Path to document file"},
   ":extracted-data": {"type": "map", "required": false, "description": "Key data points extracted from document"},
   ":tags": {"type": "array", "required": false, "description": "Searchable tags for document"},
   ":related-entities": {"type": "array", "required": false, "description": "Entity IDs this document relates to"},
   ":confidentiality": {"type": "string", "required": false, "description": "Confidentiality level: public, internal, restricted, confidential"},
   ":description": {"type": "string", "required": false, "description": "Document description"}
 }',
 '[{"usage": "(document.catalog :doc-id \"doc-cayman-registry-001\" :doc-type \"certificate_incorporation\" :issuer \"cayman_registry\" :title \"Zenith Capital Partners LP Certificate\" :issue-date \"2020-03-15\" :related-entities [\"company-zenith-spv-001\"] :tags [\"incorporation\", \"cayman\"])"}]',
 true),

-- Document verification
('document', 'document.verify', 'document_verification', 'Verify the authenticity and validity of a document',
 '{
   ":doc-id": {"type": "string", "required": true, "description": "Document identifier to verify"},
   ":verification-method": {"type": "string", "required": true, "description": "Method used for verification"},
   ":verifier": {"type": "string", "required": false, "description": "Person or system performing verification"},
   ":status": {"type": "string", "required": true, "description": "Verification result: verified, invalid, expired, pending"},
   ":confidence": {"type": "number", "required": false, "description": "Confidence score 0.0-1.0"},
   ":verified-at": {"type": "datetime", "required": true, "description": "When verification was performed"},
   ":notes": {"type": "string", "required": false, "description": "Verification notes"}
 }',
 '[{"usage": "(document.verify :doc-id \"doc-cayman-registry-001\" :verification-method \"issuer_api\" :status \"verified\" :confidence 0.95 :verified-at \"2025-11-10T09:15:00Z\")"}]',
 true),

-- Document linking/relationship
('document', 'document.link', 'document_relationship', 'Create relationships between documents',
 '{
   ":source-doc": {"type": "string", "required": true, "description": "Source document ID"},
   ":target-doc": {"type": "string", "required": true, "description": "Target document ID"},
   ":relationship": {"type": "string", "required": true, "description": "Relationship type: amends, supports, supersedes, references, annexes"},
   ":strength": {"type": "string", "required": false, "description": "Relationship strength: strong, weak, suggested"},
   ":description": {"type": "string", "required": false, "description": "Description of the relationship"},
   ":effective-date": {"type": "date", "required": false, "description": "When relationship becomes effective"}
 }',
 '[{"usage": "(document.link :source-doc \"doc-partnership-agreement-001\" :target-doc \"doc-partnership-amendment-001\" :relationship \"amends\" :description \"Amendment 1 to Partnership Agreement\")"}]',
 true),

-- Document extraction
('document', 'document.extract', 'content_extraction', 'Extract and structure key data from document content',
 '{
   ":doc-id": {"type": "string", "required": true, "description": "Document to extract data from"},
   ":extraction-method": {"type": "string", "required": true, "description": "Extraction method: ocr, ai, manual, api"},
   ":template": {"type": "string", "required": false, "description": "Template to use for extraction"},
   ":extracted-fields": {"type": "map", "required": true, "description": "Fields extracted from document with AttributeID keys"},
   ":confidence": {"type": "number", "required": false, "description": "Extraction confidence 0.0-1.0"},
   ":extracted-at": {"type": "datetime", "required": true, "description": "When extraction was performed"},
   ":extracted-by": {"type": "string", "required": false, "description": "System or person who performed extraction"}
 }',
 '[{"usage": "(document.extract :doc-id \"doc-cayman-registry-001\" :extraction-method \"ai\" :extracted-fields @attr{d0cf0002-0000-0000-0000-000000000001} \"Zenith Capital Partners LP\" :confidence 0.98 :extracted-at \"2025-11-10T08:30:00Z\")"}]',
 true),

-- Document usage tracking
('document', 'document.use', 'usage_tracking', 'Record usage of a document in a business process or workflow',
 '{
   ":doc-id": {"type": "string", "required": true, "description": "Document being used"},
   ":usage-type": {"type": "string", "required": true, "description": "Usage type: evidence, verification, compliance, reference"},
   ":workflow-stage": {"type": "string", "required": false, "description": "Workflow stage where document is used"},
   ":verb-context": {"type": "string", "required": false, "description": "DSL verb that is using this document"},
   ":cbu-id": {"type": "string", "required": false, "description": "CBU context for usage"},
   ":purpose": {"type": "string", "required": false, "description": "Purpose of document usage"},
   ":outcome": {"type": "string", "required": false, "description": "Outcome of document usage"},
   ":used-by": {"type": "string", "required": false, "description": "User or system using document"}
 }',
 '[{"usage": "(document.use :doc-id \"doc-cayman-registry-001\" :usage-type \"evidence\" :workflow-stage \"ubo_discovery\" :verb-context \"edge\" :purpose \"Supporting ownership relationship\")"}]',
 true),

-- Document amendment
('document', 'document.amend', 'document_lifecycle', 'Create amended or revised version of an existing document',
 '{
   ":parent-doc": {"type": "string", "required": true, "description": "Original document being amended"},
   ":new-doc-id": {"type": "string", "required": true, "description": "ID for the amended document"},
   ":amendment-type": {"type": "string", "required": true, "description": "Type of amendment: revision, supplement, correction, update"},
   ":changes": {"type": "array", "required": false, "description": "Description of changes made"},
   ":effective-date": {"type": "date", "required": false, "description": "When amendment becomes effective"},
   ":supersedes-parent": {"type": "boolean", "required": false, "description": "Whether this amendment supersedes the parent"},
   ":amended-by": {"type": "string", "required": false, "description": "Person or authority making amendment"}
 }',
 '[{"usage": "(document.amend :parent-doc \"doc-isda-master-001\" :new-doc-id \"doc-isda-master-001-v2\" :amendment-type \"revision\" :changes [\"Updated CSA terms\", \"Modified termination events\"] :effective-date \"2025-12-01\" :supersedes-parent true)"}]',
 true),

-- Document expiry and renewal
('document', 'document.expire', 'document_lifecycle', 'Mark document as expired or handle expiry process',
 '{
   ":doc-id": {"type": "string", "required": true, "description": "Document that is expiring"},
   ":expiry-date": {"type": "date", "required": true, "description": "Date document expired or will expire"},
   ":expiry-reason": {"type": "string", "required": false, "description": "Reason for expiry: natural_expiry, revocation, superseded, cancelled"},
   ":renewal-required": {"type": "boolean", "required": false, "description": "Whether document needs renewal"},
   ":renewal-deadline": {"type": "date", "required": false, "description": "Deadline for renewal"},
   ":notification-sent": {"type": "boolean", "required": false, "description": "Whether expiry notification was sent"}
 }',
 '[{"usage": "(document.expire :doc-id \"doc-passport-001\" :expiry-date \"2025-12-31\" :expiry-reason \"natural_expiry\" :renewal-required true :renewal-deadline \"2025-11-30\")"}]',
 true),

-- Document search and query
('document', 'document.query', 'document_search', 'Search and retrieve documents based on criteria',
 '{
   ":search-criteria": {"type": "map", "required": true, "description": "Search criteria including filters"},
   ":result-limit": {"type": "number", "required": false, "description": "Maximum number of results to return"},
   ":include-expired": {"type": "boolean", "required": false, "description": "Whether to include expired documents"},
   ":sort-by": {"type": "string", "required": false, "description": "Field to sort results by"},
   ":context": {"type": "string", "required": false, "description": "Context for the search (for audit purposes)"}
 }',
 '[{"usage": "(document.query :search-criteria {:doc-type \"certificate_incorporation\" :jurisdiction \"KY\"} :result-limit 10 :sort-by \"issue-date\")"}]',
 true);

-- ============================================================================
-- VERB REGISTRY ENTRIES
-- ============================================================================

-- Register document verbs in the global verb registry
INSERT INTO "ob-poc".verb_registry (verb, primary_domain, shared, description) VALUES
('document.catalog', 'document', false, 'Add document to centralized library with metadata'),
('document.verify', 'document', true, 'Verify document authenticity and validity'),
('document.link', 'document', false, 'Create relationships between documents'),
('document.extract', 'document', true, 'Extract structured data from document content'),
('document.use', 'document', true, 'Track document usage in workflows'),
('document.amend', 'document', false, 'Create amended versions of documents'),
('document.expire', 'document', false, 'Handle document expiry and renewal'),
('document.query', 'document', true, 'Search and retrieve documents')
ON CONFLICT (verb) DO NOTHING;

-- Document library verbs are now registered and ready for use
