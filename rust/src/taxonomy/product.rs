//! Product Domain implementation
//!
//! Maps the Product → Service → Resource taxonomy to the generic pattern.

use super::domain::{TaxonomyDomain, TaxonomyMetadata};

/// Product domain metadata
///
/// Maps existing Product/Service/Resource schema to generic taxonomy pattern.
pub static PRODUCT_METADATA: TaxonomyMetadata = TaxonomyMetadata {
    schema: "ob-poc",

    // Type tier: Products
    type_table: "products",
    type_id_col: "product_id",
    type_code_col: "product_code",
    type_name_col: "name",

    // Operation tier: Services
    op_table: "services",
    op_id_col: "service_id",
    op_code_col: "service_code",
    op_name_col: "name",

    // Resource tier
    resource_table: "service_resource_types",
    resource_id_col: "resource_id",
    resource_code_col: "resource_code",
    resource_name_col: "name",

    // Junction tables
    type_op_junction: "product_services",
    type_op_type_fk: "product_id",
    type_op_op_fk: "service_id",

    op_resource_junction: "service_resource_capabilities",
    op_resource_op_fk: "service_id",
    op_resource_resource_fk: "resource_id",

    // Instance table
    instance_table: "cbu_resource_instances",
    instance_id_col: "instance_id",

    // Gap view
    gap_view: "v_cbu_service_gaps",
};

/// Product domain marker type
///
/// Use with `TaxonomyOps<ProductDomain>` for type-safe operations.
pub struct ProductDomain;

impl TaxonomyDomain for ProductDomain {
    fn metadata() -> &'static TaxonomyMetadata {
        &PRODUCT_METADATA
    }

    fn domain_name() -> &'static str {
        "product"
    }

    fn verb_prefix() -> &'static str {
        "service-resource"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_metadata() {
        let meta = ProductDomain::metadata();
        assert_eq!(meta.schema, "ob-poc");
        assert_eq!(meta.type_table, "products");
        assert_eq!(meta.op_table, "services");
        assert_eq!(meta.resource_table, "service_resource_types");
        assert_eq!(meta.gap_view, "v_cbu_service_gaps");
    }

    #[test]
    fn test_product_fq_tables() {
        let meta = ProductDomain::metadata();
        assert_eq!(meta.fq_type_table(), "\"ob-poc\".products");
        assert_eq!(meta.fq_op_table(), "\"ob-poc\".services");
        assert_eq!(meta.fq_gap_view(), "\"ob-poc\".v_cbu_service_gaps");
    }

    #[test]
    fn test_product_domain_name() {
        assert_eq!(ProductDomain::domain_name(), "product");
        assert_eq!(ProductDomain::verb_prefix(), "service-resource");
    }
}
