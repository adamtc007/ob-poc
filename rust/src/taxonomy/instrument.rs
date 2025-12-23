//! Instrument Domain implementation
//!
//! Maps the Instrument Class → Lifecycle → Resource taxonomy to the generic pattern.

use super::domain::{TaxonomyDomain, TaxonomyMetadata};

/// Instrument domain metadata
///
/// Maps Instrument/Lifecycle/Resource schema to generic taxonomy pattern.
/// Note: instrument_classes is in custody schema, rest is in ob-poc.
pub static INSTRUMENT_METADATA: TaxonomyMetadata = TaxonomyMetadata {
    // Primary schema for most tables
    schema: "ob-poc",

    // Type tier: Instrument Classes (in custody schema)
    // Note: queries will need to handle cross-schema join
    type_table: "instrument_classes", // Actually custody.instrument_classes
    type_id_col: "class_id",
    type_code_col: "code",
    type_name_col: "name",

    // Operation tier: Lifecycles
    op_table: "lifecycles",
    op_id_col: "lifecycle_id",
    op_code_col: "code",
    op_name_col: "name",

    // Resource tier
    resource_table: "lifecycle_resource_types",
    resource_id_col: "resource_type_id",
    resource_code_col: "code",
    resource_name_col: "name",

    // Junction tables
    type_op_junction: "instrument_lifecycles",
    type_op_type_fk: "instrument_class_id",
    type_op_op_fk: "lifecycle_id",

    op_resource_junction: "lifecycle_resource_capabilities",
    op_resource_op_fk: "lifecycle_id",
    op_resource_resource_fk: "resource_type_id",

    // Instance table
    instance_table: "cbu_lifecycle_instances",
    instance_id_col: "instance_id",

    // Gap view
    gap_view: "v_cbu_lifecycle_gaps",
};

/// Instrument domain marker type
///
/// Use with `TaxonomyOps<InstrumentDomain>` for type-safe operations.
pub struct InstrumentDomain;

impl TaxonomyDomain for InstrumentDomain {
    fn metadata() -> &'static TaxonomyMetadata {
        &INSTRUMENT_METADATA
    }

    fn domain_name() -> &'static str {
        "instrument"
    }

    fn verb_prefix() -> &'static str {
        "lifecycle"
    }
}

impl InstrumentDomain {
    /// Returns the fully qualified type table (cross-schema)
    ///
    /// Instrument classes are in custody schema, not ob-poc.
    pub fn fq_type_table() -> &'static str {
        "custody.instrument_classes"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_metadata() {
        let meta = InstrumentDomain::metadata();
        assert_eq!(meta.schema, "ob-poc");
        assert_eq!(meta.type_table, "instrument_classes");
        assert_eq!(meta.op_table, "lifecycles");
        assert_eq!(meta.resource_table, "lifecycle_resource_types");
        assert_eq!(meta.gap_view, "v_cbu_lifecycle_gaps");
    }

    #[test]
    fn test_instrument_fq_tables() {
        let meta = InstrumentDomain::metadata();
        // Note: type_table needs special handling for cross-schema
        assert_eq!(
            InstrumentDomain::fq_type_table(),
            "custody.instrument_classes"
        );
        assert_eq!(meta.fq_op_table(), "\"ob-poc\".lifecycles");
        assert_eq!(meta.fq_gap_view(), "\"ob-poc\".v_cbu_lifecycle_gaps");
    }

    #[test]
    fn test_instrument_domain_name() {
        assert_eq!(InstrumentDomain::domain_name(), "instrument");
        assert_eq!(InstrumentDomain::verb_prefix(), "lifecycle");
    }
}
