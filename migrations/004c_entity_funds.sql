-- Migration: Create entity_funds table
-- Originally missing from migrations baseline but required by 056_fund_relationship_categories.sql and graph_repository.rs

CREATE TABLE IF NOT EXISTS "ob-poc".entity_funds (
    entity_id uuid PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    lei character varying(20),
    isin_base character varying(12),
    registration_number character varying(100),
    fund_structure_type text,
    fund_type text,
    regulatory_status text,
    parent_fund_id uuid REFERENCES "ob-poc".entities(entity_id),
    master_fund_id uuid REFERENCES "ob-poc".entities(entity_id),
    jurisdiction character varying(10),
    regulator character varying(100),
    authorization_date date,
    investment_objective text,
    base_currency character varying(3),
    incorporation_date date,
    launch_date date,
    financial_year_end character varying(5),
    investor_type text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    gleif_legal_form_id character varying(10),
    gleif_registered_as character varying(100),
    gleif_registered_at character varying(20),
    gleif_category character varying(20),
    gleif_status character varying(20),
    gleif_corroboration_level character varying(30),
    gleif_managing_lou character varying(20),
    gleif_last_update timestamp with time zone,
    legal_address_city character varying(100),
    legal_address_country character varying(2),
    hq_address_city character varying(100),
    hq_address_country character varying(2),
    CONSTRAINT entity_funds_entity_id_uniq UNIQUE (entity_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_funds_jurisdiction ON "ob-poc".entity_funds USING btree (jurisdiction);
CREATE UNIQUE INDEX IF NOT EXISTS idx_entity_funds_lei ON "ob-poc".entity_funds USING btree (lei) WHERE (lei IS NOT NULL);
CREATE INDEX IF NOT EXISTS idx_entity_funds_master ON "ob-poc".entity_funds USING btree (master_fund_id);
CREATE INDEX IF NOT EXISTS idx_entity_funds_parent ON "ob-poc".entity_funds USING btree (parent_fund_id);
