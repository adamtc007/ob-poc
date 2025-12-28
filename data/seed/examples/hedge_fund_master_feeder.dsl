# Apex Capital - Cayman Master-Feeder Hedge Fund Structure
# Demonstrates: MASTER_FUND, FEEDER_FUND, GENERAL_PARTNER, LIMITED_PARTNER, PRIME_BROKER

# =============================================================================
# PHASE 1: UBOs (the principals)
# =============================================================================

entity.create-person "Jonathan Smith"
    --nationality "US"
    --is-pep false
    => $john_smith

entity.create-person "Michael Jones"
    --nationality "US"
    --is-pep false
    => $mike_jones

# =============================================================================
# PHASE 2: Investment Manager (the brains)
# =============================================================================

entity.create-company "Apex Capital Management LLC"
    --jurisdiction "US-DE"
    --legal-form "LLC"
    --regulatory-status "SEC_RIA"
    --regulator "SEC"
    => $apex_im

# =============================================================================
# PHASE 3: General Partner (carries the carry)
# =============================================================================

entity.create-company "Apex Capital GP LLC"
    --jurisdiction "US-DE"
    --legal-form "LLC"
    => $apex_gp

# =============================================================================
# PHASE 4: Master Fund (where the trading happens)
# =============================================================================

entity.create-fund "Apex Capital Master Fund Ltd"
    --jurisdiction "KY"
    --fund-type "EXEMPTED_COMPANY"
    --regulatory-status "CIMA_REGISTERED"
    => $master_fund

# =============================================================================
# PHASE 5: Feeder Funds
# =============================================================================

entity.create-fund "Apex Capital Offshore Fund Ltd"
    --jurisdiction "KY"
    --fund-type "EXEMPTED_COMPANY"
    => $offshore_feeder

entity.create-partnership "Apex Capital Onshore LP"
    --jurisdiction "US-DE"
    --partnership-type "LIMITED_PARTNERSHIP"
    => $onshore_feeder

# =============================================================================
# PHASE 6: Create the CBU
# =============================================================================

cbu.create "Apex Capital"
    --commercial-client-entity-id $master_fund
    --cbu-category "HEDGE_FUND"
    --jurisdiction "KY"
    => $cbu_apex

# =============================================================================
# PHASE 7: Assign ownership/control roles
# =============================================================================

# Principals own the IM (60/40 split)
cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $john_smith
    --owned-entity-id $apex_im
    --percentage 60.0
    --role "MEMBER"

cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $mike_jones
    --owned-entity-id $apex_im
    --percentage 40.0
    --role "MEMBER"

# Same principals own the GP
cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $john_smith
    --owned-entity-id $apex_gp
    --percentage 60.0
    --role "MEMBER"

cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $mike_jones
    --owned-entity-id $apex_gp
    --percentage 40.0
    --role "MEMBER"

# GP is general partner of master fund
cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $apex_gp
    --owned-entity-id $master_fund
    --percentage 0.01
    --role "GENERAL_PARTNER"

# Mark principals as UBOs
cbu.role.assign
    --cbu-id $cbu_apex
    --entity-id $john_smith
    --role "ULTIMATE_BENEFICIAL_OWNER"
    --percentage 60.0

cbu.role.assign
    --cbu-id $cbu_apex
    --entity-id $mike_jones
    --role "ULTIMATE_BENEFICIAL_OWNER"
    --percentage 40.0

# =============================================================================
# PHASE 8: Fund structure
# =============================================================================

cbu.role.assign-fund-role
    --cbu-id $cbu_apex
    --entity-id $master_fund
    --role "MASTER_FUND"

cbu.role.assign-fund-role
    --cbu-id $cbu_apex
    --entity-id $offshore_feeder
    --fund-entity-id $master_fund
    --role "FEEDER_FUND"
    --investment-percentage 100.0

cbu.role.assign-fund-role
    --cbu-id $cbu_apex
    --entity-id $onshore_feeder
    --fund-entity-id $master_fund
    --role "FEEDER_FUND"
    --investment-percentage 100.0

# IM manages the fund
cbu.role.assign-fund-role
    --cbu-id $cbu_apex
    --entity-id $apex_im
    --fund-entity-id $master_fund
    --role "INVESTMENT_MANAGER"
    --is-regulated true

# =============================================================================
# PHASE 9: Investors (LPs)
# =============================================================================

entity.create-company "California Public Employees' Retirement System"
    --jurisdiction "US-CA"
    --legal-form "PUBLIC_PENSION"
    => $calpers

entity.create-fund "GIC Private Limited"
    --jurisdiction "SG"
    --fund-type "SOVEREIGN_WEALTH_FUND"
    => $gic

entity.create-person "Ahmed Al-Rashid"
    --nationality "AE"
    --is-pep true
    => $hnwi_1

# LP investments
cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $calpers
    --owned-entity-id $onshore_feeder
    --percentage 15.0
    --role "LIMITED_PARTNER"

cbu.role.assign-fund-role
    --cbu-id $cbu_apex
    --entity-id $gic
    --fund-entity-id $offshore_feeder
    --role "SOVEREIGN_WEALTH_FUND"
    --investment-percentage 25.0

cbu.role.assign-ownership
    --cbu-id $cbu_apex
    --owner-entity-id $hnwi_1
    --owned-entity-id $offshore_feeder
    --percentage 5.0
    --role "LIMITED_PARTNER"

# =============================================================================
# PHASE 10: Service Providers (flat - no ownership hierarchy)
# =============================================================================

entity.create-company "Goldman Sachs & Co. LLC"
    --jurisdiction "US-NY"
    --legal-form "LLC"
    => $gs_pb

entity.create-company "Citco Fund Services (Cayman Islands) Limited"
    --jurisdiction "KY"
    --legal-form "LIMITED"
    => $citco

entity.create-company "Ernst & Young Cayman Islands"
    --jurisdiction "KY"
    --legal-form "PARTNERSHIP"
    => $ey_ky

cbu.role.assign-service-provider
    --cbu-id $cbu_apex
    --provider-entity-id $gs_pb
    --client-entity-id $master_fund
    --role "PRIME_BROKER"
    --is-regulated true

cbu.role.assign-service-provider
    --cbu-id $cbu_apex
    --provider-entity-id $citco
    --client-entity-id $master_fund
    --role "ADMINISTRATOR"

cbu.role.assign-service-provider
    --cbu-id $cbu_apex
    --provider-entity-id $ey_ky
    --client-entity-id $master_fund
    --role "AUDITOR"

# =============================================================================
# PHASE 11: Directors (control overlay)
# =============================================================================

entity.create-person "Sarah Johnson"
    --nationality "KY"
    => $cayman_director

cbu.role.assign-control
    --cbu-id $cbu_apex
    --controller-entity-id $cayman_director
    --controlled-entity-id $master_fund
    --role "DIRECTOR"
    --control-type "BOARD_MEMBER"

cbu.role.assign-control
    --cbu-id $cbu_apex
    --controller-entity-id $john_smith
    --controlled-entity-id $master_fund
    --role "DIRECTOR"
    --control-type "BOARD_MEMBER"

# =============================================================================
# PHASE 12: Trading Authority (flat right)
# =============================================================================

entity.create-person "Emily Chen"
    --nationality "US"
    => $trader_1

cbu.role.assign-signatory
    --cbu-id $cbu_apex
    --person-entity-id $john_smith
    --for-entity-id $master_fund
    --role "AUTHORIZED_SIGNATORY"

cbu.role.assign-signatory
    --cbu-id $cbu_apex
    --person-entity-id $trader_1
    --for-entity-id $master_fund
    --role "AUTHORIZED_TRADER"

# Validate structure
cbu.role.validate --cbu-id $cbu_apex
