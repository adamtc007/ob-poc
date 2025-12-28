# Chen Family Discretionary Trust - Jersey Trust Structure
# Demonstrates: SETTLOR, TRUSTEE, PROTECTOR, BENEFICIARY types, trust role radial layout

# =============================================================================
# PHASE 1: Family Members (natural persons)
# =============================================================================

entity.create-person "Chen Wei"
    --nationality "SG"
    --is-pep false
    => $settlor

entity.create-person "Mary Chen"
    --nationality "SG"
    --is-pep false
    => $protector

entity.create-person "Chen Wei Jr"
    --nationality "SG"
    --date-of-birth "1990-05-15"
    => $beneficiary_son

entity.create-person "Chen Mei"
    --nationality "SG"
    --date-of-birth "1995-11-20"
    => $beneficiary_daughter

# =============================================================================
# PHASE 2: Professional Trustee (regulated corporate trustee)
# =============================================================================

entity.create-company "ABC Trust Company (Jersey) Ltd"
    --jurisdiction "JE"
    --legal-form "LIMITED"
    --regulatory-status "JFSC_REGISTERED"
    --regulator "JFSC"
    => $corporate_trustee

# =============================================================================
# PHASE 3: The Trust Entity
# =============================================================================

entity.create-trust "The Chen Family Trust"
    --jurisdiction "JE"
    --trust-type "DISCRETIONARY"
    --governing-law "Jersey"
    --date-established "2015-03-10"
    => $chen_trust

# =============================================================================
# PHASE 4: Trust Assets (underlying holding company)
# =============================================================================

entity.create-company "Chen Holdings (BVI) Ltd"
    --jurisdiction "VG"
    --legal-form "BVI_BC"
    => $holding_company

entity.create-company "Chen Real Estate Pte Ltd"
    --jurisdiction "SG"
    --legal-form "PRIVATE_LIMITED"
    => $re_company

# Trust owns the holding structure
# Note: Trustee is legal owner, but trust is beneficial owner

# =============================================================================
# PHASE 5: Create the CBU
# =============================================================================

cbu.create "Chen Family Office"
    --commercial-client-entity-id $chen_trust
    --cbu-category "FAMILY_TRUST"
    --jurisdiction "JE"
    => $cbu_chen

# =============================================================================
# PHASE 6: Assign Trust Roles (radial layout around trust)
# =============================================================================

# Settlor - always UBO under 5MLD
cbu.role.assign-trust-role
    --cbu-id $cbu_chen
    --trust-entity-id $chen_trust
    --participant-entity-id $settlor
    --role "SETTLOR"

# Corporate Trustee - legal owner, control prong
cbu.role.assign-trust-role
    --cbu-id $cbu_chen
    --trust-entity-id $chen_trust
    --participant-entity-id $corporate_trustee
    --role "TRUSTEE"

# Protector - has veto and removal powers
cbu.role.assign-trust-role
    --cbu-id $cbu_chen
    --trust-entity-id $chen_trust
    --participant-entity-id $protector
    --role "PROTECTOR"

# Beneficiaries - discretionary (flagged, not percentage-based)
cbu.role.assign-trust-role
    --cbu-id $cbu_chen
    --trust-entity-id $chen_trust
    --participant-entity-id $beneficiary_son
    --role "BENEFICIARY_DISCRETIONARY"
    --interest-type "DISCRETIONARY"
    --class-description "Named beneficiary - child of settlor"

cbu.role.assign-trust-role
    --cbu-id $cbu_chen
    --trust-entity-id $chen_trust
    --participant-entity-id $beneficiary_daughter
    --role "BENEFICIARY_DISCRETIONARY"
    --interest-type "DISCRETIONARY"
    --class-description "Named beneficiary - child of settlor"

# =============================================================================
# PHASE 7: Ownership of Trust Assets
# =============================================================================

# Trust owns holding company (via trustee as legal owner)
cbu.role.assign-ownership
    --cbu-id $cbu_chen
    --owner-entity-id $chen_trust
    --owned-entity-id $holding_company
    --percentage 100.0
    --ownership-type "BENEFICIAL"
    --role "ASSET_OWNER"

# Holding company owns operating company
cbu.role.assign-ownership
    --cbu-id $cbu_chen
    --owner-entity-id $holding_company
    --owned-entity-id $re_company
    --percentage 100.0
    --ownership-type "DIRECT"
    --role "PARENT_COMPANY"

# =============================================================================
# PHASE 8: Service Providers
# =============================================================================

entity.create-company "Mourant Ozannes"
    --jurisdiction "JE"
    --legal-form "PARTNERSHIP"
    => $legal_counsel

entity.create-company "PwC Jersey"
    --jurisdiction "JE"
    --legal-form "LLP"
    => $auditor

cbu.role.assign-service-provider
    --cbu-id $cbu_chen
    --provider-entity-id $legal_counsel
    --client-entity-id $chen_trust
    --role "LEGAL_COUNSEL"

cbu.role.assign-service-provider
    --cbu-id $cbu_chen
    --provider-entity-id $auditor
    --client-entity-id $chen_trust
    --role "AUDITOR"

# =============================================================================
# PHASE 9: Authorized Signatories on underlying companies
# =============================================================================

entity.create-person "James Wong"
    --nationality "SG"
    => $director_1

cbu.role.assign-control
    --cbu-id $cbu_chen
    --controller-entity-id $director_1
    --controlled-entity-id $holding_company
    --role "DIRECTOR"

cbu.role.assign-signatory
    --cbu-id $cbu_chen
    --person-entity-id $director_1
    --for-entity-id $holding_company
    --role "AUTHORIZED_SIGNATORY"

# =============================================================================
# VALIDATION
# =============================================================================

cbu.role.validate --cbu-id $cbu_chen

# =============================================================================
# EXPECTED VISUALIZATION:
# 
#                    ┌─────────────┐
#                    │   SETTLOR   │  (Chen Wei - ALWAYS UBO)
#                    └──────┬──────┘
#                           │
#           ┌───────────────┼───────────────┐
#           ▼               ▼               ▼
#    ┌───────────┐  ┌─────────────┐  ┌───────────┐
#    │ PROTECTOR │  │   TRUST     │  │ TRUSTEE   │
#    │(Mary Chen)│  │(Chen Family)│  │(ABC Trust)│
#    └───────────┘  └─────────────┘  └───────────┘
#                          │
#           ┌──────────────┼──────────────┐
#           ▼              ▼              ▼
#    ┌────────────┐ ┌────────────┐ ┌────────────┐
#    │BENEFICIARY │ │BENEFICIARY │ │  ASSET     │
#    │(Chen Jr)   │ │(Chen Mei)  │ │(Holdings)  │
#    └────────────┘ └────────────┘ └────────────┘
#                                        │
#                                  ┌─────┴─────┐
#                                  │ Operating │
#                                  │ Company   │
#                                  └───────────┘
# =============================================================================
