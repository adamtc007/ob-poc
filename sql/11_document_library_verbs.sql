-- 11_document_library_verbs.sql
-- Document Library DSL Verbs and Domain Vocabulary
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
   ":extracted-fields": {"type": "map", "required": true, "description": "Fields extracted from document"},
   ":confidence": {"type": "number", "required": false, "description": "Extraction confidence 0.0-1.0"},
   ":extracted-at": {"type": "datetime", "required": true, "description": "When extraction was performed"},
   ":extracted-by": {"type": "string", "required": false, "description": "System or person who performed extraction"}
 }',
 '[{"usage": "(document.extract :doc-id \"doc-cayman-registry-001\" :extraction-method \"ai\" :extracted-fields {:company-name \"Zenith Capital Partners LP\" :registration-number \"KY-123456\" :incorporation-date \"2020-03-15\"} :confidence 0.98 :extracted-at \"2025-11-10T08:30:00Z\")"}]',
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

-- Document revision/amendment
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

-- ============================================================================
-- SEMANTIC VERB METADATA FOR AI AGENTS
-- ============================================================================

-- Add semantic metadata for document verbs to enable better AI understanding
INSERT INTO "ob-poc".verb_semantics (
    domain, verb, semantic_description, intent_category, business_purpose,
    side_effects, prerequisites, postconditions, agent_prompt,
    parameter_semantics, workflow_stage, compliance_implications
) VALUES

-- document.catalog semantic metadata
('document', 'document.catalog',
 'Registers a document in the centralized document library with comprehensive metadata including type, issuer, validity periods, and extracted content',
 'create',
 'Establish a single source of truth for document metadata to enable efficient document discovery, compliance tracking, and audit trails',
 ARRAY['Creates document catalog entry', 'May trigger content extraction', 'Updates document index'],
 ARRAY['Document type must exist in document_types table', 'Document ID must be unique'],
 ARRAY['Document is searchable in library', 'Document metadata is available for workflows', 'Usage tracking is enabled'],
 'Use this verb when you need to add a new document to the system. Always specify the document type, and include as much metadata as possible to help with future searches and compliance. Think of this as "registering" the document so the system knows it exists.',
 '{
   ":doc-id": {"business_meaning": "Unique identifier that will be used to reference this document throughout all workflows", "validation": "Must be unique across all documents"},
   ":doc-type": {"business_meaning": "Categorizes the document for processing rules and compliance requirements", "validation": "Must exist in document_types table"},
   ":issuer": {"business_meaning": "Authority that issued the document, critical for verification and trust assessment", "validation": "Should exist in document_issuers table"},
   ":related-entities": {"business_meaning": "Links document to specific entities in workflows, enabling entity-centric document queries", "validation": "Entity IDs should exist in workflow context"}
 }',
 'document_onboarding',
 ARRAY['Creates audit trail for document existence', 'May be required for regulatory reporting', 'Enables document retention policy enforcement']),

-- document.verify semantic metadata
('document', 'document.verify',
 'Performs authenticity and validity verification of a document through various methods including issuer APIs, manual review, or third-party services',
 'validate',
 'Establish trust and compliance by confirming document authenticity, which is critical for KYC/AML and regulatory requirements',
 ARRAY['Updates document verification status', 'Records verification audit trail', 'May trigger compliance workflows'],
 ARRAY['Document must exist in catalog', 'Verification method must be available'],
 ARRAY['Document has verified status', 'Verification audit trail exists', 'Compliance requirements may be satisfied'],
 'Use this verb when you need to verify that a document is authentic and valid. Choose the appropriate verification method based on the document type and available verification channels. Higher confidence scores indicate stronger verification.',
 '{
   ":verification-method": {"business_meaning": "How the document was verified - affects trust level and compliance value", "validation": "Should match available verification capabilities"},
   ":status": {"business_meaning": "Verification outcome that determines if document can be trusted for compliance purposes", "validation": "Must be: verified, invalid, expired, pending"},
   ":confidence": {"business_meaning": "Quantifies trust level in verification, affects downstream risk assessment", "validation": "Decimal between 0.0 and 1.0"}
 }',
 'verification',
 ARRAY['Critical for KYC/AML compliance', 'Required for regulatory audit trails', 'Affects customer risk assessment']),

-- document.extract semantic metadata
('document', 'document.extract',
 'Extracts structured data from document content using OCR, AI, or manual processes to make document information machine-readable and searchable',
 'transform',
 'Convert unstructured document content into structured data that can be used in automated workflows, compliance checks, and business rules',
 ARRAY['Populates extracted data fields', 'Enables document content search', 'Feeds structured data to downstream processes'],
 ARRAY['Document must be accessible', 'Extraction method must be available'],
 ARRAY['Document has structured data available', 'Content is searchable', 'Data can be used in business rules'],
 'Use this verb to extract key information from documents so it can be used in automated processes. Choose the extraction method based on document type and available technology. High confidence scores indicate reliable extraction.',
 '{
   ":extraction-method": {"business_meaning": "Technology used for extraction - affects accuracy and confidence", "validation": "Must be available extraction method"},
   ":extracted-fields": {"business_meaning": "Structured data that becomes available for business rules and compliance checks", "validation": "Should match document type expected fields"},
   ":confidence": {"business_meaning": "Reliability of extracted data, affects whether human review is needed", "validation": "Decimal between 0.0 and 1.0"}
 }',
 'content_processing',
 ARRAY['Enables automated compliance checking', 'Supports data quality requirements', 'May require human validation for high-stakes decisions']);

-- ============================================================================
-- VERB RELATIONSHIPS FOR WORKFLOW UNDERSTANDING
-- ============================================================================

-- Define relationships between document verbs and other domain verbs
INSERT INTO "ob-poc".verb_relationships (
    source_domain, source_verb, target_domain, target_verb,
    relationship_type, relationship_strength, business_rationale, sequence_type
) VALUES

-- Document cataloging typically comes before verification
('document', 'document.catalog', 'document', 'document.verify', 'enables', 0.9,
 'Must catalog document before it can be verified', 'before'),

-- Document extraction often follows cataloging
('document', 'document.catalog', 'document', 'document.extract', 'enables', 0.8,
 'Document must be cataloged before content can be extracted', 'before'),

-- Document verification often uses extracted data
('document', 'document.extract', 'document', 'document.verify', 'enables', 0.7,
 'Extracted data can support verification process', 'before'),

-- KYC workflows use document verification
('document', 'document.verify', 'kyc', 'kyc.verify', 'enables', 0.9,
 'Document verification supports KYC verification process', 'before'),

-- Document usage tracking follows verification
('document', 'document.verify', 'document', 'document.use', 'enables', 0.8,
 'Verified documents can be tracked for usage', 'before'),

-- Edge creation often references documents as evidence
('graph', 'edge', 'document', 'document.use', 'enables', 0.7,
 'Creating edges with document evidence triggers usage tracking', 'parallel');

-- Document library domain is now ready for integration with ISDA workflows
