-- Migration: Harden Status CHECK Constraints
-- Add missing CHECK constraints to tables that lack them

-- custody.cbu_ssi
ALTER TABLE custody.cbu_ssi 
DROP CONSTRAINT IF EXISTS chk_cbu_ssi_status;
ALTER TABLE custody.cbu_ssi 
ADD CONSTRAINT chk_cbu_ssi_status CHECK (status IN ('PENDING', 'ACTIVE', 'SUSPENDED', 'EXPIRED'));

-- custody.entity_ssi
ALTER TABLE custody.entity_ssi 
DROP CONSTRAINT IF EXISTS chk_entity_ssi_status;
ALTER TABLE custody.entity_ssi 
ADD CONSTRAINT chk_entity_ssi_status CHECK (status IN ('PENDING', 'ACTIVE', 'SUSPENDED', 'EXPIRED'));

-- kyc.holdings
ALTER TABLE kyc.holdings 
DROP CONSTRAINT IF EXISTS chk_holdings_status;
ALTER TABLE kyc.holdings 
ADD CONSTRAINT chk_holdings_status CHECK (status IN ('active', 'closed'));

-- kyc.share_classes
ALTER TABLE kyc.share_classes 
DROP CONSTRAINT IF EXISTS chk_share_classes_status;
ALTER TABLE kyc.share_classes 
ADD CONSTRAINT chk_share_classes_status CHECK (status IN ('active', 'closed', 'suspended'));

-- ob-poc.document_catalog
ALTER TABLE "ob-poc".document_catalog 
DROP CONSTRAINT IF EXISTS chk_document_catalog_status;
ALTER TABLE "ob-poc".document_catalog 
ADD CONSTRAINT chk_document_catalog_status CHECK (status IN ('active', 'archived', 'deleted'));

-- ob-poc.dsl_execution_log
ALTER TABLE "ob-poc".dsl_execution_log 
DROP CONSTRAINT IF EXISTS chk_dsl_execution_status;
ALTER TABLE "ob-poc".dsl_execution_log 
ADD CONSTRAINT chk_dsl_execution_status CHECK (status IN ('success', 'failed', 'partial'));

-- ob-poc.dsl_instances
ALTER TABLE "ob-poc".dsl_instances 
DROP CONSTRAINT IF EXISTS chk_dsl_instances_status;
ALTER TABLE "ob-poc".dsl_instances 
ADD CONSTRAINT chk_dsl_instances_status CHECK (status IN ('draft', 'active', 'deprecated'));

-- ob-poc.cbu_resource_instances
ALTER TABLE "ob-poc".cbu_resource_instances 
DROP CONSTRAINT IF EXISTS chk_resource_instance_status;
ALTER TABLE "ob-poc".cbu_resource_instances 
ADD CONSTRAINT chk_resource_instance_status CHECK (status IN ('PENDING', 'PROVISIONING', 'ACTIVE', 'SUSPENDED', 'DECOMMISSIONED'));
