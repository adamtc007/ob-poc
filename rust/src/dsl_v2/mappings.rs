//! Column mappings for DSL key → DB column translation
//!
//! This module provides the configuration for mapping DSL keyword arguments
//! to database column names. This decouples the DSL surface syntax from
//! the database schema.

/// Database column type (for proper value binding)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbType {
    Uuid,
    Text,
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Jsonb,
}

/// Single column mapping entry
#[derive(Debug, Clone)]
pub struct ColumnMapping {
    /// DSL key (canonical form, e.g., "company-number")
    pub dsl_key: &'static str,
    /// Database column name (e.g., "registration_number")
    pub db_column: &'static str,
    /// Database type for proper binding
    pub db_type: DbType,
    /// Aliases (alternative DSL keys that map to same column)
    pub aliases: &'static [&'static str],
}

/// Table-level mapping configuration
#[derive(Debug, Clone)]
pub struct TableMappings {
    /// Table name (without schema)
    pub table: &'static str,
    /// Primary key column name
    pub pk_column: &'static str,
    /// Column mappings for this table
    pub columns: &'static [ColumnMapping],
}

// ============================================================================
// Entity Mappings
// ============================================================================

pub static ENTITIES_MAPPINGS: TableMappings = TableMappings {
    table: "entities",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["entity-name", "legal-name"],
        },
        ColumnMapping {
            dsl_key: "entity-type",
            db_column: "entity_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "jurisdiction",
            db_column: "jurisdiction",
            db_type: DbType::Text,
            aliases: &["country", "domicile"],
        },
        ColumnMapping {
            dsl_key: "status",
            db_column: "status",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "external-id",
            db_column: "external_id",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static LIMITED_COMPANIES_MAPPINGS: TableMappings = TableMappings {
    table: "limited_companies",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["company-name", "legal-name"],
        },
        ColumnMapping {
            dsl_key: "jurisdiction",
            db_column: "jurisdiction",
            db_type: DbType::Text,
            aliases: &["country"],
        },
        ColumnMapping {
            dsl_key: "company-number",
            db_column: "registration_number",
            db_type: DbType::Text,
            aliases: &["registration-number", "reg-no"],
        },
        ColumnMapping {
            dsl_key: "incorporation-date",
            db_column: "incorporation_date",
            db_type: DbType::Date,
            aliases: &["formation-date"],
        },
        ColumnMapping {
            dsl_key: "registered-address",
            db_column: "registered_address",
            db_type: DbType::Text,
            aliases: &["registered-office"],
        },
        ColumnMapping {
            dsl_key: "business-nature",
            db_column: "business_nature",
            db_type: DbType::Text,
            aliases: &["nature-of-business"],
        },
    ],
};

pub static PROPER_PERSONS_MAPPINGS: TableMappings = TableMappings {
    table: "proper_persons",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "first-name",
            db_column: "first_name",
            db_type: DbType::Text,
            aliases: &["given-name"],
        },
        ColumnMapping {
            dsl_key: "last-name",
            db_column: "last_name",
            db_type: DbType::Text,
            aliases: &["family-name", "surname"],
        },
        ColumnMapping {
            dsl_key: "middle-names",
            db_column: "middle_names",
            db_type: DbType::Text,
            aliases: &["middle-name"],
        },
        ColumnMapping {
            dsl_key: "date-of-birth",
            db_column: "date_of_birth",
            db_type: DbType::Date,
            aliases: &["dob", "birth-date"],
        },
        ColumnMapping {
            dsl_key: "nationality",
            db_column: "nationality",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "tax-id",
            db_column: "tax_id",
            db_type: DbType::Text,
            aliases: &["tin", "ssn", "national-id"],
        },
        ColumnMapping {
            dsl_key: "residence-address",
            db_column: "residence_address",
            db_type: DbType::Text,
            aliases: &["address"],
        },
    ],
};

pub static PARTNERSHIPS_MAPPINGS: TableMappings = TableMappings {
    table: "partnerships",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["partnership-name"],
        },
        ColumnMapping {
            dsl_key: "jurisdiction",
            db_column: "jurisdiction",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "partnership-type",
            db_column: "partnership_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "formation-date",
            db_column: "formation_date",
            db_type: DbType::Date,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "principal-place-business",
            db_column: "principal_place_of_business",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static TRUSTS_MAPPINGS: TableMappings = TableMappings {
    table: "trusts",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["trust-name"],
        },
        ColumnMapping {
            dsl_key: "jurisdiction",
            db_column: "jurisdiction",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "trust-type",
            db_column: "trust_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "establishment-date",
            db_column: "establishment_date",
            db_type: DbType::Date,
            aliases: &["formation-date"],
        },
        ColumnMapping {
            dsl_key: "governing-law",
            db_column: "governing_law",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "trust-purpose",
            db_column: "trust_purpose",
            db_type: DbType::Text,
            aliases: &["purpose"],
        },
    ],
};

// ============================================================================
// CBU Mappings
// ============================================================================

pub static CBUS_MAPPINGS: TableMappings = TableMappings {
    table: "cbus",
    pk_column: "cbu_id",
    columns: &[
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["cbu-name"],
        },
        ColumnMapping {
            dsl_key: "jurisdiction",
            db_column: "jurisdiction",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "client-type",
            db_column: "client_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "nature-purpose",
            db_column: "nature_purpose",
            db_type: DbType::Text,
            aliases: &["nature-and-purpose"],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "status",
            db_column: "status",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static CBU_ENTITY_ROLES_MAPPINGS: TableMappings = TableMappings {
    table: "cbu_entity_roles",
    pk_column: "cbu_entity_role_id",
    columns: &[
        ColumnMapping {
            dsl_key: "cbu-entity-role-id",
            db_column: "cbu_entity_role_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "role",
            db_column: "role",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "ownership-percent",
            db_column: "ownership_percent",
            db_type: DbType::Decimal,
            aliases: &["ownership-percentage", "ownership"],
        },
        ColumnMapping {
            dsl_key: "effective-date",
            db_column: "effective_date",
            db_type: DbType::Date,
            aliases: &["start-date"],
        },
        ColumnMapping {
            dsl_key: "notes",
            db_column: "notes",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

// ============================================================================
// Document Mappings
// ============================================================================

pub static DOCUMENT_CATALOG_MAPPINGS: TableMappings = TableMappings {
    table: "document_catalog",
    pk_column: "document_id",
    columns: &[
        ColumnMapping {
            dsl_key: "document-id",
            db_column: "document_id",
            db_type: DbType::Uuid,
            aliases: &["doc-id", "id"],
        },
        ColumnMapping {
            dsl_key: "document-type-id",
            db_column: "document_type_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "title",
            db_column: "title",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "file-path",
            db_column: "file_path",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "file-hash",
            db_column: "file_hash",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "mime-type",
            db_column: "mime_type",
            db_type: DbType::Text,
            aliases: &["content-type"],
        },
        ColumnMapping {
            dsl_key: "status",
            db_column: "status",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "verification-status",
            db_column: "verification_status",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "extraction-status",
            db_column: "extraction_status",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static DOCUMENT_CBU_LINKS_MAPPINGS: TableMappings = TableMappings {
    table: "document_cbu_links",
    pk_column: "link_id",
    columns: &[
        ColumnMapping {
            dsl_key: "link-id",
            db_column: "link_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "document-id",
            db_column: "document_id",
            db_type: DbType::Uuid,
            aliases: &["doc-id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "relationship-type",
            db_column: "relationship_type",
            db_type: DbType::Text,
            aliases: &["rel-type"],
        },
    ],
};

pub static DOCUMENT_ENTITY_LINKS_MAPPINGS: TableMappings = TableMappings {
    table: "document_entity_links",
    pk_column: "link_id",
    columns: &[
        ColumnMapping {
            dsl_key: "link-id",
            db_column: "link_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "document-id",
            db_column: "document_id",
            db_type: DbType::Uuid,
            aliases: &["doc-id"],
        },
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "relationship-type",
            db_column: "relationship_type",
            db_type: DbType::Text,
            aliases: &["rel-type"],
        },
    ],
};

// ============================================================================
// Product/Service Mappings
// ============================================================================

pub static PRODUCTS_MAPPINGS: TableMappings = TableMappings {
    table: "products",
    pk_column: "product_id",
    columns: &[
        ColumnMapping {
            dsl_key: "product-id",
            db_column: "product_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["product-name"],
        },
        ColumnMapping {
            dsl_key: "product-code",
            db_column: "product_code",
            db_type: DbType::Text,
            aliases: &["code"],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "product-category",
            db_column: "product_category",
            db_type: DbType::Text,
            aliases: &["category"],
        },
        ColumnMapping {
            dsl_key: "regulatory-framework",
            db_column: "regulatory_framework",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "is-active",
            db_column: "is_active",
            db_type: DbType::Boolean,
            aliases: &["active"],
        },
    ],
};

pub static SERVICES_MAPPINGS: TableMappings = TableMappings {
    table: "services",
    pk_column: "service_id",
    columns: &[
        ColumnMapping {
            dsl_key: "service-id",
            db_column: "service_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "name",
            db_column: "name",
            db_type: DbType::Text,
            aliases: &["service-name"],
        },
        ColumnMapping {
            dsl_key: "service-code",
            db_column: "service_code",
            db_type: DbType::Text,
            aliases: &["code"],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "service-category",
            db_column: "service_category",
            db_type: DbType::Text,
            aliases: &["category"],
        },
        ColumnMapping {
            dsl_key: "is-active",
            db_column: "is_active",
            db_type: DbType::Boolean,
            aliases: &["active"],
        },
    ],
};

pub static PRODUCT_SERVICES_MAPPINGS: TableMappings = TableMappings {
    table: "product_services",
    pk_column: "product_service_id",
    columns: &[
        ColumnMapping {
            dsl_key: "product-service-id",
            db_column: "product_service_id",
            db_type: DbType::Uuid,
            aliases: &["id", "link-id"],
        },
        ColumnMapping {
            dsl_key: "product-id",
            db_column: "product_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "service-id",
            db_column: "service_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "is-required",
            db_column: "is_required",
            db_type: DbType::Boolean,
            aliases: &["required"],
        },
    ],
};

// ============================================================================
// Investigation/Screening/Risk/Decision/Monitoring Mappings
// ============================================================================

pub static INVESTIGATIONS_MAPPINGS: TableMappings = TableMappings {
    table: "investigations",
    pk_column: "investigation_id",
    columns: &[
        ColumnMapping {
            dsl_key: "investigation-id",
            db_column: "investigation_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "investigation-type",
            db_column: "investigation_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "status",
            db_column: "status",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "outcome",
            db_column: "outcome",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "risk-rating",
            db_column: "risk_rating",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "ubo-threshold",
            db_column: "ubo_threshold",
            db_type: DbType::Decimal,
            aliases: &["threshold"],
        },
        ColumnMapping {
            dsl_key: "deadline",
            db_column: "deadline",
            db_type: DbType::Date,
            aliases: &["due-date"],
        },
        ColumnMapping {
            dsl_key: "assigned-to",
            db_column: "assigned_to",
            db_type: DbType::Text,
            aliases: &["assignee"],
        },
        ColumnMapping {
            dsl_key: "completed-by",
            db_column: "completed_by",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "notes",
            db_column: "notes",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static SCREENING_RESULTS_MAPPINGS: TableMappings = TableMappings {
    table: "screening_results",
    pk_column: "result_id",
    columns: &[
        ColumnMapping {
            dsl_key: "result-id",
            db_column: "result_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "screening-id",
            db_column: "screening_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "result",
            db_column: "result",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "resolution",
            db_column: "resolution",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "match-details",
            db_column: "match_details",
            db_type: DbType::Jsonb,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "rationale",
            db_column: "rationale",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "reviewed-by",
            db_column: "reviewed_by",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "resolved-by",
            db_column: "resolved_by",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static RISK_RATINGS_MAPPINGS: TableMappings = TableMappings {
    table: "risk_ratings",
    pk_column: "rating_id",
    columns: &[
        ColumnMapping {
            dsl_key: "rating-id",
            db_column: "rating_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "rating",
            db_column: "rating",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "factors",
            db_column: "factors",
            db_type: DbType::Jsonb,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "rationale",
            db_column: "rationale",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "assessed-by",
            db_column: "assessed_by",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static RISK_FLAGS_MAPPINGS: TableMappings = TableMappings {
    table: "risk_flags",
    pk_column: "flag_id",
    columns: &[
        ColumnMapping {
            dsl_key: "flag-id",
            db_column: "flag_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "entity-id",
            db_column: "entity_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "flag-type",
            db_column: "flag_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "severity",
            db_column: "severity",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "flagged-by",
            db_column: "flagged_by",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static DECISIONS_MAPPINGS: TableMappings = TableMappings {
    table: "decisions",
    pk_column: "decision_id",
    columns: &[
        ColumnMapping {
            dsl_key: "decision-id",
            db_column: "decision_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "investigation-id",
            db_column: "investigation_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "decision",
            db_column: "decision",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "decision-authority",
            db_column: "decision_authority",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "rationale",
            db_column: "rationale",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "decided-by",
            db_column: "decided_by",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static DECISION_CONDITIONS_MAPPINGS: TableMappings = TableMappings {
    table: "decision_conditions",
    pk_column: "condition_id",
    columns: &[
        ColumnMapping {
            dsl_key: "condition-id",
            db_column: "condition_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "decision-id",
            db_column: "decision_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "condition-type",
            db_column: "condition_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "frequency",
            db_column: "frequency",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "due-date",
            db_column: "due_date",
            db_type: DbType::Date,
            aliases: &["deadline"],
        },
        ColumnMapping {
            dsl_key: "threshold",
            db_column: "threshold",
            db_type: DbType::Decimal,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "satisfied-by",
            db_column: "satisfied_by",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "evidence",
            db_column: "evidence",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "satisfied-date",
            db_column: "satisfied_date",
            db_type: DbType::Timestamp,
            aliases: &[],
        },
    ],
};

pub static MONITORING_CONFIGS_MAPPINGS: TableMappings = TableMappings {
    table: "monitoring_configs",
    pk_column: "config_id",
    columns: &[
        ColumnMapping {
            dsl_key: "config-id",
            db_column: "config_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "monitoring-level",
            db_column: "monitoring_level",
            db_type: DbType::Text,
            aliases: &["level"],
        },
        ColumnMapping {
            dsl_key: "components",
            db_column: "components",
            db_type: DbType::Jsonb,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "frequency",
            db_column: "frequency",
            db_type: DbType::Text,
            aliases: &[],
        },
    ],
};

pub static MONITORING_EVENTS_MAPPINGS: TableMappings = TableMappings {
    table: "monitoring_events",
    pk_column: "event_id",
    columns: &[
        ColumnMapping {
            dsl_key: "event-id",
            db_column: "event_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "event-type",
            db_column: "event_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "description",
            db_column: "description",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "severity",
            db_column: "severity",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "requires-review",
            db_column: "requires_review",
            db_type: DbType::Boolean,
            aliases: &[],
        },
    ],
};

pub static SCHEDULED_REVIEWS_MAPPINGS: TableMappings = TableMappings {
    table: "scheduled_reviews",
    pk_column: "review_id",
    columns: &[
        ColumnMapping {
            dsl_key: "review-id",
            db_column: "review_id",
            db_type: DbType::Uuid,
            aliases: &["id"],
        },
        ColumnMapping {
            dsl_key: "cbu-id",
            db_column: "cbu_id",
            db_type: DbType::Uuid,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "review-type",
            db_column: "review_type",
            db_type: DbType::Text,
            aliases: &["type"],
        },
        ColumnMapping {
            dsl_key: "due-date",
            db_column: "due_date",
            db_type: DbType::Date,
            aliases: &["deadline"],
        },
        ColumnMapping {
            dsl_key: "assigned-to",
            db_column: "assigned_to",
            db_type: DbType::Text,
            aliases: &["assignee"],
        },
        ColumnMapping {
            dsl_key: "completed-by",
            db_column: "completed_by",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "notes",
            db_column: "notes",
            db_type: DbType::Text,
            aliases: &[],
        },
        ColumnMapping {
            dsl_key: "next-review-date",
            db_column: "next_review_date",
            db_type: DbType::Date,
            aliases: &[],
        },
    ],
};

// ============================================================================
// Lookup Functions
// ============================================================================

/// All table mappings for lookup
static ALL_TABLE_MAPPINGS: &[&TableMappings] = &[
    &ENTITIES_MAPPINGS,
    &LIMITED_COMPANIES_MAPPINGS,
    &PROPER_PERSONS_MAPPINGS,
    &PARTNERSHIPS_MAPPINGS,
    &TRUSTS_MAPPINGS,
    &CBUS_MAPPINGS,
    &CBU_ENTITY_ROLES_MAPPINGS,
    &DOCUMENT_CATALOG_MAPPINGS,
    &DOCUMENT_CBU_LINKS_MAPPINGS,
    &DOCUMENT_ENTITY_LINKS_MAPPINGS,
    &PRODUCTS_MAPPINGS,
    &SERVICES_MAPPINGS,
    &PRODUCT_SERVICES_MAPPINGS,
    &INVESTIGATIONS_MAPPINGS,
    &SCREENING_RESULTS_MAPPINGS,
    &RISK_RATINGS_MAPPINGS,
    &RISK_FLAGS_MAPPINGS,
    &DECISIONS_MAPPINGS,
    &DECISION_CONDITIONS_MAPPINGS,
    &MONITORING_CONFIGS_MAPPINGS,
    &MONITORING_EVENTS_MAPPINGS,
    &SCHEDULED_REVIEWS_MAPPINGS,
];

/// Get table mappings by table name
pub fn get_table_mappings(table: &str) -> Option<&'static TableMappings> {
    ALL_TABLE_MAPPINGS
        .iter()
        .find(|m| m.table == table)
        .copied()
}

/// Resolve a DSL key to a database column for a given table
/// Returns (db_column, db_type) if found
pub fn resolve_column(table: &str, dsl_key: &str) -> Option<(&'static str, DbType)> {
    let mappings = get_table_mappings(table)?;

    // First try direct match
    if let Some(col) = mappings.columns.iter().find(|c| c.dsl_key == dsl_key) {
        return Some((col.db_column, col.db_type));
    }

    // Then try aliases
    for col in mappings.columns {
        if col.aliases.contains(&dsl_key) {
            return Some((col.db_column, col.db_type));
        }
    }

    None
}

/// Get primary key column for a table
pub fn get_pk_column(table: &str) -> Option<&'static str> {
    get_table_mappings(table).map(|m| m.pk_column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_table_mappings() {
        let mappings = get_table_mappings("cbus");
        assert!(mappings.is_some());
        assert_eq!(mappings.unwrap().pk_column, "cbu_id");
    }

    #[test]
    fn test_resolve_column_direct() {
        let result = resolve_column("cbus", "name");
        assert!(result.is_some());
        let (col, db_type) = result.unwrap();
        assert_eq!(col, "name");
        assert_eq!(db_type, DbType::Text);
    }

    #[test]
    fn test_resolve_column_alias() {
        let result = resolve_column("cbus", "cbu-name");
        assert!(result.is_some());
        let (col, _) = result.unwrap();
        assert_eq!(col, "name");
    }

    #[test]
    fn test_resolve_column_not_found() {
        let result = resolve_column("cbus", "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_pk_column() {
        assert_eq!(get_pk_column("entities"), Some("entity_id"));
        assert_eq!(get_pk_column("cbus"), Some("cbu_id"));
        assert_eq!(get_pk_column("document_catalog"), Some("document_id"));
    }
}
