# Ultimate Beneficial Ownership (UBO) Discovery

This document describes the comprehensive Ultimate Beneficial Ownership functionality added to the DSL-as-State onboarding system. The UBO module implements best practices for financial services compliance and regulatory requirements.

## 🎯 Overview

Ultimate Beneficial Ownership discovery is a critical component of Anti-Money Laundering (AML) and Counter-Terrorism Financing (CTF) programs. This implementation provides:

- **Recursive ownership structure analysis**
- **Threshold-based UBO identification** 
- **Control prong analysis**
- **Identity verification workflows**
- **Sanctions and PEP screening**
- **Risk-based assessment**
- **Ongoing monitoring setup**
- **Complete audit trail via DSL-as-State**

## 🏗️ Architecture

### Core Pattern: DSL-as-State + AttributeID-as-Type

The UBO system follows the fundamental architectural patterns:

- **DSL-as-State**: Each UBO operation appends to the accumulated DSL document
- **AttributeID-as-Type**: All data points reference UUID-based dictionary attributes
- **Event Sourcing**: Immutable versioning with complete audit trail
- **AI-Enabled**: Optional AI agent integration for complex structure analysis

### UBO Domain Structure

```
internal/domains/ubo/
├── domain.go              # Core UBO domain implementation
├── ubo_attributes.go      # UBO-specific dictionary attributes
└── ubo_attributes_test.go # Comprehensive attribute tests

internal/dictionary/seed/ubo/
├── ubo_attributes.go      # UBO attribute definitions
└── ubo_attributes_test.go # UBO attribute validation tests

examples/ubo/
└── complete_ubo_workflow.dsl # Comprehensive workflow example
```

## 🚀 Quick Start

### 1. Database Setup

```bash
# Initialize database with UBO tables
export DB_CONN_STRING="postgres://localhost:5432/postgres?sslmode=disable"
make init-db
./dsl-poc seed-catalog
```

### 2. Create Base Case

```bash
# Create a new onboarding case
./dsl-poc create --cbu="CBU-UBO-001"
```

### 3. Execute UBO Discovery

```bash
# Discover UBOs for a UK corporation
./dsl-poc discover-ubo \
  --cbu="CBU-UBO-001" \
  --entity="TechGlobal Holdings Ltd" \
  --jurisdiction="GB" \
  --threshold=25.0 \
  --framework="EU_5MLD" \
  --verbose

# With dry-run to see what would be executed
./dsl-poc discover-ubo \
  --cbu="CBU-UBO-001" \
  --entity="Acme Holdings S.à r.l." \
  --jurisdiction="LU" \
  --dry-run
```

### 4. View Results

```bash
# View complete DSL evolution including UBO discovery
./dsl-poc history --cbu="CBU-UBO-001"
```

## 📊 DSL Vocabulary

The UBO domain introduces **13 new verbs** organized into logical categories:

### Entity Structure Discovery (3 verbs)
- `ubo.collect-entity-data` - Collect comprehensive entity information
- `ubo.get-ownership-structure` - Retrieve ownership relationships
- `ubo.unroll-structure` - Recursively unroll complex structures

### UBO Identification (4 verbs)
- `ubo.resolve-ubos` - Core UBO identification with thresholds
- `ubo.calculate-indirect-ownership` - Calculate aggregated ownership
- `ubo.identify-control-prong` - Identify control relationships
- `ubo.apply-thresholds` - Apply regulatory thresholds

### UBO Verification (3 verbs)
- `ubo.verify-identity` - Verify UBO identity documents
- `ubo.screen-person` - Screen against sanctions/PEP/adverse media
- `ubo.assess-risk` - Risk assessment based on UBO profiles

### UBO Monitoring (3 verbs)
- `ubo.monitor-changes` - Set up ongoing monitoring
- `ubo.refresh-data` - Periodic data refresh
- `ubo.trigger-review` - Trigger manual compliance review

## 🔧 Command Line Usage

### Basic Command Structure

```bash
./dsl-poc discover-ubo [flags]
```

### Required Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--cbu` | CBU ID for the client case | `CBU-1234` |
| `--entity` | Legal name of the entity | `"Acme Holdings Ltd"` |
| `--jurisdiction` | ISO 3166-1 alpha-2 country code | `GB`, `US`, `LU` |

### Optional Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--entity-type` | `CORPORATION` | Entity type (CORPORATION, LLC, PARTNERSHIP, TRUST) |
| `--threshold` | `25.0` | Ownership threshold percentage for UBO identification |
| `--framework` | `EU_5MLD` | Regulatory framework (EU_5MLD, US_FINCEN, UK_PSC, FATF) |
| `--dry-run` | `false` | Show what would be executed without making changes |
| `--verbose` | `false` | Show detailed execution logs |

### Examples

```bash
# EU 5th Money Laundering Directive (25% threshold)
./dsl-poc discover-ubo --cbu="CBU-1234" --entity="Global Investments S.A." --jurisdiction="LU"

# US FinCEN requirements (25% threshold)
./dsl-poc discover-ubo --cbu="CBU-5678" --entity="Tech Ventures LLC" --jurisdiction="US" --framework="US_FINCEN"

# UK PSC (Persons with Significant Control) requirements
./dsl-poc discover-ubo --cbu="CBU-9012" --entity="Innovation Partners Ltd" --jurisdiction="GB" --framework="UK_PSC"

# Custom threshold (30%)
./dsl-poc discover-ubo --cbu="CBU-3456" --entity="Investment Fund GP" --jurisdiction="DE" --threshold=30.0

# Complex entity with verbose logging
./dsl-poc discover-ubo --cbu="CBU-7890" --entity="Multi-Tier Holdings B.V." --jurisdiction="NL" --verbose

# Dry run to preview workflow
./dsl-poc discover-ubo --cbu="CBU-1111" --entity="Preview Corp" --jurisdiction="CH" --dry-run
```

## 🎨 DSL Workflow Example

Here's a complete UBO discovery workflow as represented in DSL:

```lisp
; Step 1: Collect Entity Data
(ubo.collect-entity-data
  (entity_name "TechGlobal Holdings S.à r.l.")
  (jurisdiction "LU")
  (entity_type "LLC"))

; Step 2: Get Ownership Structure
(ubo.get-ownership-structure
  (entity_id @attr{entity-uuid})
  (depth_limit 10))

; Step 3: Unroll Complex Structures
(ubo.unroll-structure
  (entity_id @attr{entity-uuid})
  (consolidation_method "ADDITIVE"))

; Step 4: Resolve UBOs with 25% threshold
(ubo.resolve-ubos
  (entity_id @attr{entity-uuid})
  (ownership_threshold 25.0)
  (jurisdiction_rules "EU_5MLD"))

; Step 5: Identify Control Prong
(ubo.identify-control-prong
  (entity_id @attr{entity-uuid})
  (control_types ["CEO", "BOARD_MAJORITY"]))

; Step 6: Verify UBO Identities
(ubo.verify-identity
  (ubo_id @attr{ubo-uuid-1})
  (document_list ["passport", "proof_of_address"])
  (verification_level "ENHANCED"))

; Step 7: Screen Against Watchlists
(ubo.screen-person
  (ubo_id @attr{ubo-uuid-1})
  (screening_lists ["OFAC", "EU_SANCTIONS", "PEP_DATABASE"])
  (screening_intensity "COMPREHENSIVE"))

; Step 8: Assess Risk
(ubo.assess-risk
  (entity_id @attr{entity-uuid})
  (ubo_list @attr{verified-ubos}))

; Step 9: Set Up Monitoring
(ubo.monitor-changes
  (entity_id @attr{entity-uuid})
  (monitoring_frequency "MONTHLY"))
```

## 📋 Attribute Dictionary

The UBO system introduces **25+ new attributes** organized into logical groups:

### Entity Identity
- `entity.legal_name` - Official legal name
- `entity.jurisdiction` - Country of incorporation
- `entity.type` - Legal form (CORPORATION, LLC, etc.)
- `entity.registration_number` - Official registration number

### Ownership Structure
- `ownership.percentage` - Ownership percentage (0.00-100.00)
- `ownership.link_type` - Type of ownership relationship
- `ownership.voting_rights` - Voting rights percentage
- `ownership.control_mechanism` - How control is exercised

### UBO Identification
- `ubo.natural_person_id` - Unique UBO identifier
- `ubo.relationship_type` - How person qualifies as UBO
- `ubo.total_ownership` - Calculated total ownership
- `ubo.ownership_threshold` - Applied threshold percentage

### UBO Verification
- `ubo.verification_status` - Identity verification status
- `ubo.identity_documents` - Documents collected
- `ubo.screening_result` - Screening outcome
- `ubo.pep_status` - Politically Exposed Person status

### Risk Assessment
- `ubo.risk_rating` - Overall risk rating
- `ubo.jurisdiction_risk` - Country risk rating
- `ubo.adverse_media` - Adverse media screening result

### Ongoing Monitoring
- `ubo.monitoring_frequency` - How often to review
- `ubo.last_review_date` - When last reviewed
- `ubo.next_review_due` - Next review date
- `ubo.change_detected` - Whether changes detected

### Compliance
- `ubo.compliance_status` - Overall compliance status
- `ubo.regulatory_threshold` - Applicable regulatory threshold
- `ubo.documentation_complete` - Documentation completeness

## 🎯 Best Practices Implementation

### 1. Regulatory Framework Support

| Framework | Threshold | Key Features |
|-----------|-----------|--------------|
| **EU 5MLD** | 25% | Control prong, PEP screening, ongoing monitoring |
| **US FinCEN** | 25% | Senior managing official, enhanced screening |
| **UK PSC** | 25% | Persons with significant control register |
| **FATF** | Varies | Risk-based approach, ongoing monitoring |

### 2. Ownership Calculation Methods

- **ADDITIVE**: Sum all ownership paths (handles cross-holdings)
- **MULTIPLICATIVE**: Multiply ownership percentages along paths
- **MAX_PATH**: Take the maximum ownership path

### 3. Control Prong Identification

The system identifies control through multiple mechanisms:
- Board control (majority of directors)
- Voting control (voting agreements, proxies)
- Management contracts
- Trust arrangements
- Senior managing official designation

### 4. Verification Levels

- **BASIC**: Standard document verification
- **ENHANCED**: Additional source verification
- **SUPERIOR**: Biometric verification, source of wealth

### 5. Screening Intensity

- **BASIC**: Core sanctions lists
- **COMPREHENSIVE**: Sanctions + PEP + adverse media
- **DEEP**: Full background checks, professional sanctions

## 🔍 Error Handling and Validation

### Input Validation

```bash
# Invalid jurisdiction format
./dsl-poc discover-ubo --cbu="CBU-1234" --entity="Test Corp" --jurisdiction="USA"
# Error: jurisdiction must be 2-letter ISO 3166-1 alpha-2 code (e.g., US, GB, LU)

# Invalid threshold range
./dsl-poc discover-ubo --cbu="CBU-1234" --entity="Test Corp" --jurisdiction="US" --threshold=150.0
# Error: threshold must be between 0.01 and 100.0
```

### DSL Verb Validation

The system validates that only approved UBO verbs are used:

```lisp
; This will pass validation
(ubo.resolve-ubos (entity_id @attr{uuid}) (ownership_threshold 25.0))

; This will fail validation
(ubo.invalid-verb (entity_id @attr{uuid}))
; Error: unapproved DSL verbs detected: [ubo.invalid-verb]
```

## 📊 Output Examples

### Successful UBO Discovery

```bash
$ ./dsl-poc discover-ubo --cbu="CBU-1234" --entity="TechGlobal Holdings Ltd" --jurisdiction="GB" --verbose

🚀 Starting UBO Discovery for TechGlobal Holdings Ltd (GB)
📋 Parameters: Threshold=25.0%, Framework=EU_5MLD
📄 Current DSL length: 342 characters
🤖 Using AI agent for UBO discovery
💾 Storing UBO discovery DSL

🎯 UBO Discovery Results
========================
📊 Identified 2 Ultimate Beneficial Owner(s):

1. Maria Kowalski
   └─ ID: person-uuid-1
   └─ Relationship: DIRECT_OWNERSHIP
   └─ Ownership: 45.00%
   └─ Status: Verification pending, Screening pending

2. Dr. Hans Mueller
   └─ ID: person-uuid-2
   └─ Relationship: CONTROL_PRONG
   └─ Control: CEO
   └─ Status: Verification pending, Screening pending

📋 Compliance Status: pending_verification

💡 Recommendations:
1. Verify identity documents for all identified UBOs
2. Conduct sanctions and PEP screening for each UBO
3. Review local AML/CFT requirements for UBO identification
4. Set up ongoing monitoring for ownership changes
5. Schedule periodic UBO data refresh (recommended: quarterly)

⏱️  Total execution time: 2.345s
📊 New DSL version: 4
✅ UBO discovery completed for case CBU-1234
📋 Use 'dsl-poc history --cbu=CBU-1234' to view the complete DSL evolution
```

### Dry Run Output

```bash
$ ./dsl-poc discover-ubo --cbu="CBU-1234" --entity="Test Corp" --jurisdiction="US" --dry-run

🔍 DRY RUN - UBO Discovery Workflow:
=====================================
; Ultimate Beneficial Ownership Discovery Workflow
; Entity: Test Corp (Jurisdiction: US)

(ubo.collect-entity-data
  (entity_name "Test Corp")
  (jurisdiction "US")
  (entity_type "CORPORATION"))

(ubo.get-ownership-structure
  (entity_id @attr{entity-uuid})
  (depth_limit 5))
...
=====================================
✅ Dry run complete. This workflow would be added to case CBU-1234
```

## 🧪 Testing

### Run UBO Tests

```bash
# Run UBO attribute tests
go test ./internal/dictionary/seed/ubo -v

# Run UBO domain tests
go test ./internal/domains/ubo -v

# Run DSL agent UBO verb validation tests
go test ./internal/agent -run TestValidateDSLVerbs -v

# Run all tests
make test
```

### Test Coverage

The UBO implementation includes comprehensive tests:
- **Attribute generation and validation**
- **DSL verb approval and validation**
- **Domain logic execution**
- **CLI flag parsing**
- **Error handling scenarios**

## 📈 Integration with Main POC

### Seamless Integration

The UBO functionality integrates seamlessly with the existing DSL-as-State system:

1. **Uses existing infrastructure**: Database, store interfaces, agent system
2. **Follows established patterns**: DSL-as-State, AttributeID-as-Type
3. **Maintains compatibility**: Existing workflows continue to work
4. **Extends vocabulary**: Adds 13 new verbs to approved vocabulary

### State Machine Integration

UBO discovery integrates into the onboarding state machine:

```
CREATE → ADD_PRODUCTS → DISCOVER_KYC → DISCOVER_UBO → DISCOVER_SERVICES → ...
```

### DSL Evolution Example

```bash
# Version 1: Case creation
(case.create (cbu.id "CBU-1234") (nature-purpose "Investment holding company"))

# Version 2: Products added
(products.add "CUSTODY" "FUND_ACCOUNTING")

# Version 3: KYC initiated
(kyc.start (documents (document "CertificateOfIncorporation")) ...)

# Version 4: UBO discovery (NEW!)
(ubo.collect-entity-data (entity_name "Holdings Ltd") (jurisdiction "GB"))
(ubo.resolve-ubos (entity_id @attr{uuid}) (ownership_threshold 25.0))
...
```

## 🚀 Production Considerations

### Performance

- **Efficient attribute lookups**: UUID-based dictionary references
- **Lazy evaluation**: Only execute necessary UBO analysis steps
- **Caching**: Domain results cached between operations
- **Parallel processing**: Multiple UBOs processed concurrently

### Scalability

- **Horizontal scaling**: Stateless domain operations
- **Database optimization**: Indexed UBO-related tables
- **API integration**: Ready for external data sources
- **Monitoring integration**: Structured logging and metrics

### Security

- **PII protection**: Sensitive attributes marked with encryption flags
- **Audit compliance**: Complete immutable audit trail
- **Access control**: Role-based access to UBO data
- **Data retention**: Configurable retention policies

## 🔗 Related Documentation

- [CLAUDE.md](./CLAUDE.md) - Core architectural patterns
- [API_DOCUMENTATION.md](./API_DOCUMENTATION.md) - API reference
- [SCHEMA_DOCUMENTATION.md](./SCHEMA_DOCUMENTATION.md) - Database schema
- [examples/ubo/complete_ubo_workflow.dsl](./examples/ubo/complete_ubo_workflow.dsl) - Complete workflow example

## 💡 Future Enhancements

### Planned Features

1. **Real-time data integration** with beneficial ownership registries
2. **AI-powered structure analysis** for complex ownership chains
3. **Regulatory framework updates** as requirements evolve
4. **Enhanced visualization** of ownership structures
5. **Automated compliance reporting** generation

### Extensibility

The UBO domain is designed for easy extension:
- Add new regulatory frameworks
- Extend attribute dictionary
- Add new verification providers
- Integrate additional screening sources
- Customize risk assessment models

## 🆘 Troubleshooting

### Common Issues

**Issue**: `case CBU-1234 does not exist`
**Solution**: Create the case first with `./dsl-poc create --cbu="CBU-1234"`

**Issue**: `failed to initialize AI agent`
**Solution**: The command will work without AI agent (uses template fallback)

**Issue**: `jurisdiction must be 2-letter ISO code`
**Solution**: Use ISO 3166-1 alpha-2 codes (US, GB, LU, etc.)

**Issue**: `threshold must be between 0.01 and 100.0`
**Solution**: Use percentage values like 25.0 for 25%

### Debug Mode

```bash
# Enable verbose logging for detailed execution information
./dsl-poc discover-ubo --cbu="CBU-1234" --entity="Debug Corp" --jurisdiction="US" --verbose
```

## 📞 Support

For questions or issues with UBO functionality:

1. Check this documentation first
2. Review the complete example in `examples/ubo/complete_ubo_workflow.dsl`
3. Run tests to verify system health: `make test`
4. Use `--dry-run` flag to preview operations
5. Check logs with `--verbose` flag for detailed execution information

The UBO discovery system represents a comprehensive implementation of financial services best practices for Ultimate Beneficial Ownership identification and verification, fully integrated into the DSL-as-State architecture.