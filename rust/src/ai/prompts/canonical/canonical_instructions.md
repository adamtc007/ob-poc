# Canonical DSL Generation Instructions

## CRITICAL: Use Only Canonical Forms

This document provides AI agents with strict guidelines for generating DSL that conforms to the canonical v3.1 specification. All generated DSL MUST use these canonical forms - no legacy aliases or variations are permitted.

## Verbs - Use These ONLY

### Case Management
- `case.create` - Create new investigation case
- `case.update` - Update case with findings or notes
- `case.approve` - Final case approval
- `case.close` - Close completed case

### Workflow Management
- `workflow.transition` - State transitions with reason

### Entity Operations
- `entity.register` - Register new entity
- `entity.link` - Create ownership/control relationships

### Document Operations
- `document.catalog` - Add document to catalog
- `document.use` - Link document as evidence

### KYC Operations
- `kyc.collect` - Collect KYC information
- `kyc.verify` - Verify collected information
- `kyc.assess` - Assess KYC status
- `kyc.screen_sanctions` - Screen against sanctions lists
- `kyc.check_pep` - Check for PEP status

### Compliance Operations
- `compliance.aml_check` - AML screening
- `compliance.fatca_check` - FATCA compliance check
- `compliance.screen` - General compliance screening
- `compliance.monitor` - Ongoing monitoring

### UBO Operations
- `ubo.calc` - Calculate beneficial ownership
- `ubo.outcome` - Record UBO determination

## Keys - Use These ONLY

### Case Keys
- `:case-id` - Unique case identifier
- `:case-type` - Type of case (KYC_CASE, UBO_ANALYSIS, etc.)
- `:business-reference` - Business reference number
- `:assigned-to` - Analyst assigned to case
- `:title` - Descriptive case title
- `:notes` - Case notes (append-only)
- `:approved-by` - Approving officer ID
- `:approval-summary` - Summary of approval decision

### Entity Keys
- `:entity-id` - Unique entity identifier
- `:entity-type` - Type of entity (LIMITED_COMPANY, PROPER_PERSON, etc.)
- `:props` - Entity properties map
- `:legal-name` - Legal entity name
- `:jurisdiction` - Legal jurisdiction code
- `:nationality` - Person nationality
- `:date-of-birth` - Person date of birth
- `:entity-status` - Entity status (ACTIVE, INACTIVE)

### Link Keys
- `:link-id` - Unique link identifier
- `:from-entity` - Source entity ID
- `:to-entity` - Target entity ID
- `:relationship-type` - Type of relationship (OWNERSHIP, CONTROL, etc.)
- `:relationship-props` - Relationship properties map
- `:ownership-percentage` - Ownership percentage (0.0-100.0)
- `:verification-status` - Status (ALLEGED, VERIFIED, REJECTED)
- `:description` - Human-readable description
- `:source` - Information source
- `:verified-by` - Verifying analyst ID
- `:verification-date` - Date of verification
- `:reason` - Reason for status or decision

### Document Keys
- `:document-id` - Unique document identifier
- `:document-type` - Type of document
- `:issuer` - Document issuing entity
- `:title` - Document title
- `:file-hash` - SHA256 hash of document file
- `:used-by-process` - Process using the document
- `:usage-type` - How document is used (EVIDENCE, etc.)
- `:evidence.of-link` - Link ID this document evidences
- `:user-id` - User accessing/using document

### Workflow Keys
- `:to-state` - Target workflow state
- `:reason` - Reason for transition

### UBO Keys
- `:entity` - Entity for UBO calculation
- `:method` - Calculation method
- `:threshold` - UBO threshold percentage
- `:target` - Target entity for UBO outcome
- `:ubos` - Array of UBO determinations
- `:effective-percent` - Calculated effective ownership
- `:prongs` - Map of qualification prongs (:ownership, :control)
- `:evidence` - Array of supporting document IDs
- `:confidence-score` - AI confidence score (0.0-100.0)

## FORBIDDEN Legacy Forms

### DO NOT USE These Verbs
- `kyc.start_case` → Use `case.create`
- `kyc.transition_state` → Use `workflow.transition`
- `kyc.add_finding` → Use `case.update`
- `kyc.approve_case` → Use `case.approve`
- `ubo.link_ownership` → Use `entity.link`
- `ubo.link_control` → Use `entity.link`
- `ubo.add_evidence` → Use `document.use`

### DO NOT USE These Keys
- `:new_state` → Use `:to-state`
- `:file_hash` → Use `:file-hash`
- `:approver_id` → Use `:approved-by`
- `:status` → Use `:verification-status`
- `:case_id` → Use `:case-id`
- `:entity_id` → Use `:entity-id`
- `:document_id` → Use `:document-id`

## Required Structure Patterns

### 1. Entity Links Must Use :relationship-props Map

```lisp
;; CORRECT - Canonical form
(entity.link
  :link-id "link-001"
  :from-entity "person-john"
  :to-entity "company-abc"
  :relationship-type "OWNERSHIP"
  :relationship-props {:ownership-percentage 50.0
                       :verification-status "VERIFIED"
                       :verified-by "analyst-1"})

;; WRONG - Flat structure
(entity.link
  :from-entity "person-john"
  :to-entity "company-abc"
  :ownership-percentage 50.0
  :verification-status "VERIFIED")
```

### 2. Case Updates Use :notes for Append-Only Findings

```lisp
;; CORRECT - Notes field for findings
(case.update
  :case-id "kyc-001"
  :notes "Received partnership agreement. Document hash: abc123...")

;; WRONG - Separate finding verb
(case.add_finding
  :case-id "kyc-001"
  :finding "Partnership agreement received")
```

### 3. Evidence Must Use document.use with :evidence.of-link

```lisp
;; CORRECT - Link document as evidence
(document.use
  :document-id "doc-123"
  :used-by-process "UBO_ANALYSIS"
  :usage-type "EVIDENCE"
  :evidence.of-link "link-001"
  :user-id "analyst-1")

;; WRONG - Direct evidence attachment
(ubo.add_evidence
  :link-id "link-001"
  :document-id "doc-123")
```

### 4. Entity Properties Use :props Map

```lisp
;; CORRECT - Properties in :props map
(entity.register
  :entity-id "company-123"
  :entity-type "LIMITED_COMPANY"
  :props {:legal-name "ABC Corp Ltd"
          :jurisdiction "GB"
          :entity-status "ACTIVE"})

;; WRONG - Flat properties
(entity.register
  :entity-id "company-123"
  :entity-type "LIMITED_COMPANY"
  :legal-name "ABC Corp Ltd"
  :jurisdiction "GB")
```

## Naming Conventions

### Kebab-Case for All Keys
- Use hyphens, not underscores: `:case-id` not `:case_id`
- Use hyphens, not camelCase: `:entity-type` not `:entityType`
- Use dots for namespaced keys: `:evidence.of-link`

### Entity Types (Use These Exact Values)
- `KYC_CASE` - KYC investigation case
- `UBO_ANALYSIS` - UBO analysis case
- `LIMITED_COMPANY` - Limited liability company
- `HEDGE_FUND` - Hedge fund entity
- `INVESTMENT_FUND` - Investment fund entity
- `PROPER_PERSON` - Natural person
- `TRUST` - Trust entity
- `PARTNERSHIP` - Partnership entity

### Relationship Types (Use These Exact Values)
- `OWNERSHIP` - Ownership relationship
- `CONTROL` - Control relationship
- `GENERAL_PARTNER` - General partner relationship
- `LIMITED_PARTNER` - Limited partner relationship
- `TRUSTEE` - Trustee relationship
- `BENEFICIARY` - Beneficiary relationship

### Document Types (Use These Exact Values)
- `ARTICLES_OF_ASSOCIATION` - Articles of association
- `LIMITED_PARTNERSHIP_AGREEMENT` - Partnership agreement
- `SHARE_REGISTER` - Share register
- `SHAREHOLDING_STRUCTURE` - Shareholding structure document
- `PASSPORT` - Passport document
- `IDENTITY_DOCUMENT` - Generic identity document
- `TRUST_DEED` - Trust deed
- `BOARD_RESOLUTION` - Board resolution

## AI Generation Best Practices

### 1. Always Use Templates
- Base generation on canonical templates (kyc_investigation.template, ubo_analysis.template)
- Replace template variables with actual values
- Maintain the canonical structure and flow

### 2. Generate Complete Workflows
- Include all phases: creation, documentation, verification, calculation, approval
- Use proper workflow transitions between states
- Ensure all links have supporting evidence

### 3. Maintain Referential Integrity
- All entity-ids referenced in links must be registered first
- All document-ids in evidence must be cataloged first
- All link-ids in evidence must reference valid links

### 4. Use Consistent Identifiers
- Generate meaningful IDs: `kyc-case-abc-001` not `case-123`
- Use entity names in IDs: `person-john-doe` not `entity-456`
- Include business context: `ubo-analysis-hedge-fund-xyz`

### 5. Include Appropriate Metadata
- Always include verification status and dates
- Add confidence scores for AI-generated content
- Include analyst IDs and approval chains
- Provide descriptive reasons and summaries

## Quality Checklist

Before outputting any generated DSL, verify:

- [ ] All verbs are from the canonical list (no legacy aliases)
- [ ] All keys use kebab-case format (no underscores)
- [ ] Entity links use `:relationship-props` map structure
- [ ] Case updates use `:notes` field for findings
- [ ] Evidence uses `document.use` with `:evidence.of-link`
- [ ] Entity properties are in `:props` map
- [ ] All referenced entities are registered first
- [ ] All referenced documents are cataloged first
- [ ] Workflow transitions include `:reason`
- [ ] UBO calculations include confidence scores
- [ ] All IDs are meaningful and consistent
- [ ] Complete workflow from creation to approval

## Error Recovery

If you catch yourself using legacy forms:

1. **Stop immediately** - Do not continue with legacy syntax
2. **Identify the canonical equivalent** using this guide
3. **Rewrite the problematic section** using canonical forms
4. **Verify the entire workflow** against the checklist above

Remember: The normalization layer will catch and convert legacy forms, but AI agents MUST generate canonical forms directly to ensure consistency and maintainability.