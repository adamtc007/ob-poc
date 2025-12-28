# Allianz Global Investors - Full Ownership Chain to UBO
# Demonstrates: OWNERSHIP_CHAIN, FUND_STRUCTURE, FUND_MANAGEMENT, SERVICE_PROVIDER

# =============================================================================
# PHASE 1: Create the ownership chain (pyramid up to public shareholders)
# =============================================================================

# Level 4 (apex): Public shareholders (UBO terminus - dispersed ownership)
entity.create-person "BlackRock Inc" 
    --nationality "US"
    --is-institutional true
    => $blackrock

entity.create-person "Vanguard Group"
    --nationality "US" 
    --is-institutional true
    => $vanguard

# Level 3: Allianz SE (German SE - publicly traded)
entity.create-company "Allianz SE"
    --jurisdiction "DE"
    --legal-form "SE"
    --registration-number "HRB 164232"
    --is-publicly-traded true
    --stock-exchange "XETRA"
    => $allianz_se

# Level 2: Intermediate holding company
entity.create-company "Allianz Asset Management GmbH"
    --jurisdiction "DE"
    --legal-form "GmbH"
    --parent-entity-id $allianz_se
    => $allianz_am

# Level 1: The regulated ManCo/AIFM
entity.create-company "Allianz Global Investors GmbH"
    --jurisdiction "DE"
    --legal-form "GmbH"
    --regulatory-status "AIFM"
    --regulator "BaFin"
    => $allianz_gi

# =============================================================================
# PHASE 2: Create the CBU and assign ownership roles
# =============================================================================

cbu.create "Allianz Global Investors (Group)"
    --commercial-client-entity-id $allianz_gi
    --cbu-category "ASSET_MANAGER"
    --jurisdiction "DE"
    => $cbu_allianz

# Assign ownership chain with percentages
cbu.role.assign-ownership
    --cbu-id $cbu_allianz
    --owner-entity-id $blackrock
    --owned-entity-id $allianz_se
    --percentage 8.0
    --ownership-type "DIRECT"
    --role "SHAREHOLDER"

cbu.role.assign-ownership
    --cbu-id $cbu_allianz
    --owner-entity-id $vanguard
    --owned-entity-id $allianz_se
    --percentage 3.0
    --ownership-type "DIRECT"
    --role "SHAREHOLDER"

cbu.role.assign-ownership
    --cbu-id $cbu_allianz
    --owner-entity-id $allianz_se
    --owned-entity-id $allianz_am
    --percentage 100.0
    --role "PARENT_COMPANY"

cbu.role.assign-ownership
    --cbu-id $cbu_allianz
    --owner-entity-id $allianz_am
    --owned-entity-id $allianz_gi
    --percentage 100.0
    --role "HOLDING_COMPANY"

# Mark public shareholders as UBO (dispersed - no single ≥25%)
cbu.role.assign
    --cbu-id $cbu_allianz
    --entity-id $blackrock
    --role "BENEFICIAL_OWNER"
    --percentage 8.0

# =============================================================================
# PHASE 3: Create fund structure (UCITS SICAV)
# =============================================================================

# Umbrella fund (SICAV)
entity.create-fund "Allianz Global Investors Fund"
    --jurisdiction "LU"
    --fund-type "SICAV"
    --regulatory-status "UCITS"
    --regulator "CSSF"
    => $agi_sicav

# Sub-funds
entity.create-fund "Allianz Global Artificial Intelligence"
    --jurisdiction "LU"
    --fund-type "SUB_FUND"
    --parent-fund-id $agi_sicav
    => $subfund_ai

entity.create-fund "Allianz Emerging Markets Equity"
    --jurisdiction "LU"
    --fund-type "SUB_FUND"
    --parent-fund-id $agi_sicav
    => $subfund_em

# Assign fund structure roles
cbu.role.assign-fund-role
    --cbu-id $cbu_allianz
    --entity-id $agi_sicav
    --role "UMBRELLA_FUND"

cbu.role.assign-fund-role
    --cbu-id $cbu_allianz
    --entity-id $subfund_ai
    --fund-entity-id $agi_sicav
    --role "SUB_FUND"

cbu.role.assign-fund-role
    --cbu-id $cbu_allianz
    --entity-id $subfund_em
    --fund-entity-id $agi_sicav
    --role "SUB_FUND"

# ManCo manages the fund
cbu.role.assign-fund-role
    --cbu-id $cbu_allianz
    --entity-id $allianz_gi
    --fund-entity-id $agi_sicav
    --role "MANAGEMENT_COMPANY"
    --is-regulated true
    --regulatory-jurisdiction "DE-BaFin"

# =============================================================================
# PHASE 4: Create ManCo branch network
# =============================================================================

entity.create-company "Allianz Global Investors Luxembourg S.A."
    --jurisdiction "LU"
    --legal-form "SA"
    --regulatory-status "UCITS_MANCO"
    => $agi_lu

entity.create-company "Allianz Global Investors UK Limited"
    --jurisdiction "GB"
    --legal-form "LIMITED"
    --regulatory-status "FCA_AUTHORISED"
    => $agi_uk

entity.create-company "Allianz Global Investors Ireland Limited"
    --jurisdiction "IE"
    --legal-form "LIMITED"
    --regulatory-status "CBI_AUTHORISED"
    => $agi_ie

# Ownership from parent ManCo
cbu.role.assign-ownership
    --cbu-id $cbu_allianz
    --owner-entity-id $allianz_gi
    --owned-entity-id $agi_lu
    --percentage 100.0
    --role "PARENT_COMPANY"

cbu.role.assign-ownership
    --cbu-id $cbu_allianz
    --owner-entity-id $allianz_gi
    --owned-entity-id $agi_uk
    --percentage 100.0
    --role "PARENT_COMPANY"

# =============================================================================
# PHASE 5: Service providers (flat - no ownership)
# =============================================================================

entity.create-company "State Street Bank International GmbH, Luxembourg Branch"
    --jurisdiction "LU"
    --legal-form "BRANCH"
    => $state_street_lu

entity.create-company "State Street Bank International GmbH, Ireland Branch"
    --jurisdiction "IE"
    --legal-form "BRANCH"
    => $state_street_ie

entity.create-company "PricewaterhouseCoopers Luxembourg"
    --jurisdiction "LU"
    --legal-form "SARL"
    => $pwc_lu

entity.create-company "European Fund Services S.A."
    --jurisdiction "LU"
    --legal-form "SA"
    => $efs

# Assign service provider roles
cbu.role.assign-service-provider
    --cbu-id $cbu_allianz
    --provider-entity-id $state_street_lu
    --client-entity-id $agi_sicav
    --role "DEPOSITARY"
    --is-regulated true

cbu.role.assign-service-provider
    --cbu-id $cbu_allianz
    --provider-entity-id $pwc_lu
    --client-entity-id $agi_sicav
    --role "AUDITOR"

cbu.role.assign-service-provider
    --cbu-id $cbu_allianz
    --provider-entity-id $efs
    --client-entity-id $agi_sicav
    --role "TRANSFER_AGENT"

# =============================================================================
# PHASE 6: Control chain (directors/officers)
# =============================================================================

entity.create-person "Dr. Hans Müller"
    --nationality "DE"
    => $director_1

entity.create-person "Maria Schmidt"
    --nationality "DE"
    => $director_2

entity.create-person "James Wilson"
    --nationality "GB"
    => $conducting_officer

cbu.role.assign-control
    --cbu-id $cbu_allianz
    --controller-entity-id $director_1
    --controlled-entity-id $allianz_gi
    --role "CHAIRMAN"
    --control-type "BOARD_MEMBER"

cbu.role.assign-control
    --cbu-id $cbu_allianz
    --controller-entity-id $director_2
    --controlled-entity-id $allianz_gi
    --role "DIRECTOR"
    --control-type "BOARD_MEMBER"

cbu.role.assign-control
    --cbu-id $cbu_allianz
    --controller-entity-id $conducting_officer
    --controlled-entity-id $agi_lu
    --role "CONDUCTING_OFFICER"
    --control-type "EXECUTIVE"

# =============================================================================
# PHASE 7: Authorized signatories (trading/execution)
# =============================================================================

entity.create-person "Thomas Bauer"
    --nationality "DE"
    => $signatory_1

entity.create-person "Sophie Martin"
    --nationality "FR"
    => $signatory_2

cbu.role.assign-signatory
    --cbu-id $cbu_allianz
    --person-entity-id $signatory_1
    --for-entity-id $allianz_gi
    --role "AUTHORIZED_SIGNATORY"
    --authority-limit 50000000
    --authority-currency "EUR"

cbu.role.assign-signatory
    --cbu-id $cbu_allianz
    --person-entity-id $signatory_2
    --for-entity-id $allianz_gi
    --role "AUTHORIZED_TRADER"

# =============================================================================
# VALIDATION: Check all role requirements are satisfied
# =============================================================================

cbu.role.validate --cbu-id $cbu_allianz
