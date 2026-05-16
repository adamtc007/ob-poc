//! TaxonomyDomain trait and metadata structures
//!
//! Defines the contract for taxonomy domains and their metadata.

/// Metadata describing a taxonomy domain's schema
///
/// This struct captures all the table/column names that vary between domains
/// while following the same structural pattern.
#[derive(Debug, Clone)]
pub struct TaxonomyMetadata {
    /// Schema name (e.g., "ob-poc", "custody")
    pub schema: &'static str,

    // Type tier (e.g., Product, Instrument Class)
    pub type_table: &'static str,
    pub type_id_col: &'static str,
    pub type_code_col: &'static str,
    pub type_name_col: &'static str,

    // Operation tier (e.g., Service, Lifecycle)
    pub op_table: &'static str,
    pub op_id_col: &'static str,
    pub op_code_col: &'static str,
    pub op_name_col: &'static str,

    // Resource tier
    pub resource_table: &'static str,
    pub resource_id_col: &'static str,
    pub resource_code_col: &'static str,
    pub resource_name_col: &'static str,

    // Junction tables
    pub type_op_junction: &'static str,
    pub type_op_type_fk: &'static str,
    pub type_op_op_fk: &'static str,

    pub op_resource_junction: &'static str,
    pub op_resource_op_fk: &'static str,
    pub op_resource_resource_fk: &'static str,

    // Instance table
    pub instance_table: &'static str,
    pub instance_id_col: &'static str,

    // Gap view
    pub gap_view: &'static str,
}

impl TaxonomyMetadata {
    /// Returns fully qualified table name with schema
    pub fn fq_type_table(&self) -> String {
        format!("\"{}\".{}", self.schema, self.type_table)
    }

    pub fn fq_op_table(&self) -> String {
        format!("\"{}\".{}", self.schema, self.op_table)
    }

    pub fn fq_resource_table(&self) -> String {
        format!("\"{}\".{}", self.schema, self.resource_table)
    }

    pub fn fq_type_op_junction(&self) -> String {
        format!("\"{}\".{}", self.schema, self.type_op_junction)
    }

    pub fn fq_op_resource_junction(&self) -> String {
        format!("\"{}\".{}", self.schema, self.op_resource_junction)
    }

    pub fn fq_instance_table(&self) -> String {
        format!("\"{}\".{}", self.schema, self.instance_table)
    }

    pub fn fq_gap_view(&self) -> String {
        format!("\"{}\".{}", self.schema, self.gap_view)
    }
}

/// Trait for taxonomy domain definitions
///
/// Implement this trait to define a new taxonomy domain.
/// The trait provides metadata about the domain's schema mapping.
///
/// # Example
///
/// ```ignore
/// pub struct ProductDomain;
///
/// impl TaxonomyDomain for ProductDomain {
///     fn metadata() -> &'static TaxonomyMetadata {
///         &PRODUCT_METADATA
///     }
///
///     fn domain_name() -> &'static str {
///         "product"
///     }
/// }
/// ```
pub trait TaxonomyDomain: Send + Sync + 'static {
    /// Returns the metadata describing this domain's schema
    fn metadata() -> &'static TaxonomyMetadata;

    /// Returns the domain name (used in DSL verbs, logs)
    fn domain_name() -> &'static str;

    /// Returns the DSL verb prefix for this domain
    fn verb_prefix() -> &'static str {
        Self::domain_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test metadata for verification
    static TEST_METADATA: TaxonomyMetadata = TaxonomyMetadata {
        schema: "test",
        type_table: "types",
        type_id_col: "type_id",
        type_code_col: "code",
        type_name_col: "name",
        op_table: "ops",
        op_id_col: "op_id",
        op_code_col: "code",
        op_name_col: "name",
        resource_table: "resources",
        resource_id_col: "resource_id",
        resource_code_col: "code",
        resource_name_col: "name",
        type_op_junction: "type_ops",
        type_op_type_fk: "type_id",
        type_op_op_fk: "op_id",
        op_resource_junction: "op_resources",
        op_resource_op_fk: "op_id",
        op_resource_resource_fk: "resource_id",
        instance_table: "instances",
        instance_id_col: "instance_id",
        gap_view: "v_gaps",
    };

    #[test]
    fn test_fq_table_names() {
        assert_eq!(TEST_METADATA.fq_type_table(), "\"test\".types");
        assert_eq!(TEST_METADATA.fq_op_table(), "\"test\".ops");
        assert_eq!(TEST_METADATA.fq_gap_view(), "\"test\".v_gaps");
    }
}
