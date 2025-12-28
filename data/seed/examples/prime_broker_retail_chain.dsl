# Fidelity Investments - Retail Broker to End Investor Chain
# Demonstrates: ACCOUNT_HOLDER, NOMINEE, CUSTODIAN_FOR, FUND_INVESTOR, PRIME_BROKER chain

# =============================================================================
# PHASE 1: Fidelity Corporate Structure (UBO chain)
# =============================================================================

# Johnson family trusts (ultimate owners)
entity.create-trust "Abigail Johnson Family Trust"
    --jurisdiction "US-MA"
    --trust-type "IRREVOCABLE"
    => $johnson_trust

entity.create-person "Abigail Johnson"
    --nationality "US"
    --is-pep false
    => $abigail_johnson

# Parent company (privately held)
entity.create-company "FMR LLC"
    --jurisdiction "US-MA"
    --legal-form "LLC"
    => $fmr_llc

# Brokerage arm
entity.create-company "Fidelity Brokerage Services LLC"
    --jurisdiction "US-MA"
    --legal-form "LLC"
    --regulatory-status "FINRA_BD"
    --regulator "FINRA"
    => $fidelity_brokerage

# Custody arm
entity.create-company "National Financial Services LLC"
    --jurisdiction "US-MA"
    --legal-form "LLC"
    --regulatory-status "SEC_BD"
    => $nfs

# Fund management arm
entity.create-company "Fidelity Management & Research Company LLC"
    --jurisdiction "US-MA"
    --legal-form "LLC"
    --regulatory-status "SEC_RIA"
    => $fmr_co

# =============================================================================
# PHASE 2: Mutual Funds (product shelf)
# =============================================================================

entity.create-fund "Fidelity Contrafund"
    --jurisdiction "US-MA"
    --fund-type "MUTUAL_FUND"
    --regulatory-status "1940_ACT"
    => $contrafund

entity.create-fund "Fidelity 500 Index Fund"
    --jurisdiction "US-MA"
    --fund-type "MUTUAL_FUND"
    --regulatory-status "1940_ACT"
    => $sp500_fund

# =============================================================================
# PHASE 3: Retail Customers (account holders)
# =============================================================================

entity.create-person "John Smith"
    --nationality "US"
    => $retail_1

entity.create-person "Jane Doe"
    --nationality "US"
    => $retail_2

entity.create-company "Smith Family LLC"
    --jurisdiction "US-CA"
    --legal-form "LLC"
    => $family_llc

# 401k plan sponsor
entity.create-company "Acme Corporation"
    --jurisdiction "US-DE"
    --legal-form "CORPORATION"
    => $acme_corp

# =============================================================================
# PHASE 4: Institutional Prime Broker Clients
# =============================================================================

entity.create-fund "Quantum Alpha Master Fund Ltd"
    --jurisdiction "KY"
    --fund-type "EXEMPTED_COMPANY"
    => $hf_client

entity.create-company "Quantum Capital Management LLC"
    --jurisdiction "US-NY"
    --legal-form "LLC"
    --regulatory-status "SEC_RIA"
    => $hf_manager

# =============================================================================
# PHASE 5: Create the CBU (Fidelity as the client)
# =============================================================================

cbu.create "Fidelity Investments (Prime Services)"
    --commercial-client-entity-id $fidelity_brokerage
    --cbu-category "PRIME_BROKER"
    --jurisdiction "US"
    => $cbu_fidelity

# =============================================================================
# PHASE 6: Ownership Chain (Johnson → FMR → subsidiaries)
# =============================================================================

# Johnson family owns FMR
cbu.role.assign-ownership
    --cbu-id $cbu_fidelity
    --owner-entity-id $johnson_trust
    --owned-entity-id $fmr_llc
    --percentage 49.0
    --role "SHAREHOLDER"

cbu.role.assign-ownership
    --cbu-id $cbu_fidelity
    --owner-entity-id $abigail_johnson
    --owned-entity-id $fmr_llc
    --percentage 24.5
    --role "SHAREHOLDER"

# FMR owns subsidiaries
cbu.role.assign-ownership
    --cbu-id $cbu_fidelity
    --owner-entity-id $fmr_llc
    --owned-entity-id $fidelity_brokerage
    --percentage 100.0
    --role "PARENT_COMPANY"

cbu.role.assign-ownership
    --cbu-id $cbu_fidelity
    --owner-entity-id $fmr_llc
    --owned-entity-id $nfs
    --percentage 100.0
    --role "PARENT_COMPANY"

cbu.role.assign-ownership
    --cbu-id $cbu_fidelity
    --owner-entity-id $fmr_llc
    --owned-entity-id $fmr_co
    --percentage 100.0
    --role "PARENT_COMPANY"

# Mark Abigail as potential UBO (just under 25%)
cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $abigail_johnson
    --role "BENEFICIAL_OWNER"
    --percentage 24.5

# =============================================================================
# PHASE 7: Fund Management Relationships
# =============================================================================

cbu.role.assign-fund-role
    --cbu-id $cbu_fidelity
    --entity-id $fmr_co
    --fund-entity-id $contrafund
    --role "INVESTMENT_MANAGER"
    --is-regulated true

cbu.role.assign-fund-role
    --cbu-id $cbu_fidelity
    --entity-id $fmr_co
    --fund-entity-id $sp500_fund
    --role "INVESTMENT_MANAGER"
    --is-regulated true

# =============================================================================
# PHASE 8: Investor Chain (retail → brokerage → funds)
# =============================================================================

# Retail account holders at brokerage
cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $retail_1
    --role "ACCOUNT_HOLDER"
    --target-entity-id $fidelity_brokerage

cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $retail_2
    --role "ACCOUNT_HOLDER"
    --target-entity-id $fidelity_brokerage

cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $family_llc
    --role "ACCOUNT_HOLDER"
    --target-entity-id $fidelity_brokerage

# NFS holds securities as nominee for brokerage clients
cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $nfs
    --role "NOMINEE"
    --target-entity-id $fidelity_brokerage

cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $nfs
    --role "CUSTODIAN"
    --target-entity-id $fidelity_brokerage

# Retail investors are fund shareholders
cbu.role.assign-fund-role
    --cbu-id $cbu_fidelity
    --entity-id $retail_1
    --fund-entity-id $contrafund
    --role "FUND_INVESTOR"
    --investment-percentage 0.001  # tiny % of $100B fund

cbu.role.assign-fund-role
    --cbu-id $cbu_fidelity
    --entity-id $retail_2
    --fund-entity-id $sp500_fund
    --role "FUND_INVESTOR"
    --investment-percentage 0.0005

# =============================================================================
# PHASE 9: Prime Brokerage Clients (hedge fund)
# =============================================================================

cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $hf_client
    --role "ACCOUNT_HOLDER"
    --target-entity-id $fidelity_brokerage

# Fidelity provides prime broker services
cbu.role.assign-service-provider
    --cbu-id $cbu_fidelity
    --provider-entity-id $fidelity_brokerage
    --client-entity-id $hf_client
    --role "PRIME_BROKER"
    --is-regulated true

cbu.role.assign-service-provider
    --cbu-id $cbu_fidelity
    --provider-entity-id $nfs
    --client-entity-id $hf_client
    --role "CUSTODIAN"

# HF manager controls the HF (look-through)
cbu.role.assign-fund-role
    --cbu-id $cbu_fidelity
    --entity-id $hf_manager
    --fund-entity-id $hf_client
    --role "INVESTMENT_MANAGER"

# =============================================================================
# PHASE 10: 401k Relationship (omnibus)
# =============================================================================

cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $acme_corp
    --role "ACCOUNT_HOLDER"
    --target-entity-id $fidelity_brokerage

# 401k is omnibus - individual participants not tracked at broker level
cbu.role.assign
    --cbu-id $cbu_fidelity
    --entity-id $acme_corp
    --role "OMNIBUS_ACCOUNT"
    --target-entity-id $fidelity_brokerage

# =============================================================================
# PHASE 11: Control (officers)
# =============================================================================

cbu.role.assign-control
    --cbu-id $cbu_fidelity
    --controller-entity-id $abigail_johnson
    --controlled-entity-id $fmr_llc
    --role "CHIEF_EXECUTIVE"
    --control-type "EXECUTIVE"

cbu.role.assign-control
    --cbu-id $cbu_fidelity
    --controller-entity-id $abigail_johnson
    --controlled-entity-id $fmr_llc
    --role "CHAIRMAN"
    --control-type "BOARD_MEMBER"

# Validate
cbu.role.validate --cbu-id $cbu_fidelity

# =============================================================================
# INVESTOR CHAIN VISUALIZATION:
#
#    ┌─────────────────┐
#    │ Johnson Trust   │ ← UBO apex
#    │ (49%)           │
#    └────────┬────────┘
#             │
#    ┌────────┴────────┐
#    │    FMR LLC      │ ← Parent holding
#    └────────┬────────┘
#     ________|________
#    │        │        │
#    ▼        ▼        ▼
# ┌─────┐ ┌─────┐ ┌─────┐
# │Brok │ │ NFS │ │FMRCo│ ← Subsidiaries
# │     │ │(Cust)│ │(IM) │
# └──┬──┘ └─────┘ └──┬──┘
#    │               │
#    │ accounts      │ manages
#    ▼               ▼
# ┌─────────────────────┐
# │   Mutual Funds      │
# │ (Contrafund, etc)   │
# └──────────┬──────────┘
#            │
#     ┌──────┴──────┐
#     ▼             ▼
# ┌───────┐   ┌───────┐
# │Retail │   │Retail │  ← INVESTOR_CHAIN (pyramid down)
# │Smith  │   │Doe    │
# └───────┘   └───────┘
#
# SIDE: Prime broker clients (HF) → flat/satellite
# BOTTOM: Service roles (custody, clearing) → flat
# =============================================================================
