-- Roles Seed Data
-- Run: psql $DATABASE_URL -f sql/seeds/roles.sql

INSERT INTO "ob-poc".roles (role_id, name, description, created_at)
VALUES
  -- Structural roles (account opening prong)
  (gen_random_uuid(), 'AssetOwner', 'Legal owner of assets', NOW()),
  (gen_random_uuid(), 'InvestmentManager', 'Manages investment decisions', NOW()),
  (gen_random_uuid(), 'ManagementCompany', 'UCITS/AIFM management company', NOW()),
  (gen_random_uuid(), 'Custodian', 'Holds assets in custody', NOW()),
  (gen_random_uuid(), 'Administrator', 'Fund administrator', NOW()),
  (gen_random_uuid(), 'Depositary', 'UCITS/AIFM depositary', NOW()),
  (gen_random_uuid(), 'PrimeBroker', 'Prime brokerage services', NOW()),
  (gen_random_uuid(), 'TransferAgent', 'Shareholder register', NOW()),

  -- Operational roles
  (gen_random_uuid(), 'AuditLead', 'Audit team lead', NOW()),
  (gen_random_uuid(), 'TradeCapture', 'Trade capture team', NOW()),
  (gen_random_uuid(), 'FundAccountant', 'Fund accounting team', NOW()),
  (gen_random_uuid(), 'ComplianceOfficer', 'Compliance oversight', NOW()),
  (gen_random_uuid(), 'RelationshipManager', 'Client relationship manager', NOW()),

  -- UBO/KYC roles (KYC prong)
  (gen_random_uuid(), 'BeneficialOwner', 'Ultimate beneficial owner (>10% or >25%)', NOW()),
  (gen_random_uuid(), 'ControllingPerson', 'Person with control (not ownership)', NOW()),
  (gen_random_uuid(), 'AuthorizedSignatory', 'Can sign on behalf of entity', NOW()),
  (gen_random_uuid(), 'MaterialInfluence', 'Material influence over activities', NOW()),
  (gen_random_uuid(), 'Director', 'Board director', NOW()),
  (gen_random_uuid(), 'Secretary', 'Company secretary', NOW()),

  -- Trust-specific roles
  (gen_random_uuid(), 'Settlor', 'Trust settlor', NOW()),
  (gen_random_uuid(), 'Trustee', 'Trust trustee', NOW()),
  (gen_random_uuid(), 'Beneficiary', 'Trust beneficiary', NOW()),
  (gen_random_uuid(), 'Protector', 'Trust protector', NOW()),

  -- Partnership-specific roles
  (gen_random_uuid(), 'GeneralPartner', 'General partner (unlimited liability)', NOW()),
  (gen_random_uuid(), 'LimitedPartner', 'Limited partner', NOW())
ON CONFLICT (name) DO NOTHING;
