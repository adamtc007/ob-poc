//! Generic taxonomy operations
//!
//! Provides `TaxonomyOps<D>` that works for any domain implementing `TaxonomyDomain`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use uuid::Uuid;

use super::domain::TaxonomyDomain;

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Discovery result for a type code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discovery {
    /// The type code that was queried
    pub type_code: String,
    /// Type name
    pub type_name: String,
    /// Whether this type requires special agreements (e.g., ISDA)
    pub requires_agreement: bool,
    /// Mandatory operations for this type
    pub mandatory_ops: Vec<OpInfo>,
    /// Optional operations for this type
    pub optional_ops: Vec<OpInfo>,
}

/// Operation info within a discovery result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpInfo {
    /// Operation code
    pub code: String,
    /// Operation name
    pub name: String,
    /// Resources required by this operation
    pub resources: Vec<ResourceInfo>,
}

/// Resource info within an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    /// Resource code
    pub code: String,
    /// Resource name
    pub name: String,
    /// Whether this resource is required
    pub required: bool,
}

/// Gap record from gap view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub type_code: String,
    pub op_code: String,
    pub op_name: String,
    pub is_mandatory: bool,
    pub missing_resource_code: String,
    pub missing_resource_name: String,
    pub provisioning_verb: Option<String>,
    pub location_type: Option<String>,
    pub per_market: bool,
    pub per_currency: bool,
    pub per_counterparty: bool,
}

/// Arguments for provisioning a resource instance
#[derive(Debug, Clone)]
pub struct ProvisionArgs {
    pub cbu_id: Uuid,
    pub resource_type_code: String,
    pub instance_url: Option<String>,
    pub market_id: Option<Uuid>,
    pub currency: Option<String>,
    pub counterparty_id: Option<Uuid>,
    pub provider_code: Option<String>,
    pub config: Option<serde_json::Value>,
}

/// Result of provisioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResult {
    pub instance_id: Uuid,
    pub instance_url: String,
    pub status: String,
    pub created: bool,
}

/// Generic taxonomy operations
///
/// All operations are parameterized by the domain type `D`.
/// This ensures type-safe operations across domains.
///
/// # Example
///
/// ```ignore
/// use taxonomy::{TaxonomyOps, ProductDomain, InstrumentDomain};
///
/// let product_ops = TaxonomyOps::<ProductDomain>::new(pool.clone());
/// let instrument_ops = TaxonomyOps::<InstrumentDomain>::new(pool.clone());
///
/// // Same method, different domains
/// let product_discovery = product_ops.discover("GLOBAL_CUSTODY").await?;
/// let instrument_discovery = instrument_ops.discover("IRS").await?;
/// ```
pub struct TaxonomyOps<D: TaxonomyDomain> {
    #[cfg(feature = "database")]
    pool: PgPool,
    _domain: PhantomData<D>,
}

impl<D: TaxonomyDomain> TaxonomyOps<D> {
    /// Create new taxonomy operations for a domain
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _domain: PhantomData,
        }
    }

    /// Create new taxonomy operations (no-database mode)
    #[cfg(not(feature = "database"))]
    pub fn new() -> Self {
        Self {
            _domain: PhantomData,
        }
    }

    /// Discover all operations and resources for a type code
    ///
    /// Returns mandatory and optional operations with their required resources.
    #[cfg(feature = "database")]
    pub async fn discover(&self, type_code: &str) -> Result<Discovery> {
        let meta = D::metadata();

        // Get type info
        let type_query = format!(
            "SELECT {}, {} FROM {} WHERE {} = $1",
            meta.type_id_col,
            meta.type_name_col,
            Self::fq_type_table(),
            meta.type_code_col
        );

        let type_row: Option<(Uuid, String)> = sqlx::query_as(&type_query)
            .bind(type_code)
            .fetch_optional(&self.pool)
            .await?;

        let (_type_id, type_name) = type_row
            .ok_or_else(|| anyhow::anyhow!("Unknown {} type: {}", D::domain_name(), type_code))?;

        // Get operations for this type
        let ops_query = format!(
            r#"SELECT o.{}, o.{}, j.is_mandatory
               FROM {} o
               JOIN {} j ON j.{} = o.{}
               JOIN {} t ON t.{} = j.{}
               WHERE t.{} = $1
                 AND COALESCE(j.is_active, true) = true
               ORDER BY COALESCE(j.display_order, 100)"#,
            meta.op_code_col,
            meta.op_name_col,
            meta.fq_op_table(),
            meta.fq_type_op_junction(),
            meta.type_op_op_fk,
            meta.op_id_col,
            Self::fq_type_table(),
            meta.type_id_col,
            meta.type_op_type_fk,
            meta.type_code_col
        );

        let ops: Vec<(String, String, bool)> = sqlx::query_as(&ops_query)
            .bind(type_code)
            .fetch_all(&self.pool)
            .await?;

        let mut mandatory_ops = Vec::new();
        let mut optional_ops = Vec::new();

        for (op_code, op_name, is_mandatory) in ops {
            // Get resources for this operation
            let resources_query = format!(
                r#"SELECT r.{}, r.{}, COALESCE(j.is_required, true)
                   FROM {} r
                   JOIN {} j ON j.{} = r.{}
                   JOIN {} o ON o.{} = j.{}
                   WHERE o.{} = $1
                     AND COALESCE(j.is_active, true) = true
                     AND COALESCE(r.is_active, true) = true
                   ORDER BY COALESCE(j.priority, 100)"#,
                meta.resource_code_col,
                meta.resource_name_col,
                meta.fq_resource_table(),
                meta.fq_op_resource_junction(),
                meta.op_resource_resource_fk,
                meta.resource_id_col,
                meta.fq_op_table(),
                meta.op_id_col,
                meta.op_resource_op_fk,
                meta.op_code_col
            );

            let resources: Vec<(String, String, bool)> = sqlx::query_as(&resources_query)
                .bind(&op_code)
                .fetch_all(&self.pool)
                .await?;

            let resources: Vec<ResourceInfo> = resources
                .into_iter()
                .map(|(code, name, required)| ResourceInfo {
                    code,
                    name,
                    required,
                })
                .collect();

            let op_info = OpInfo {
                code: op_code,
                name: op_name,
                resources,
            };

            if is_mandatory {
                mandatory_ops.push(op_info);
            } else {
                optional_ops.push(op_info);
            }
        }

        // Check if any mandatory op requires agreement (e.g., ISDA)
        let requires_agreement = mandatory_ops.iter().any(|op| {
            op.code.contains("ISDA") || op.code.contains("CSA") || op.code.contains("COLLATERAL")
        });

        Ok(Discovery {
            type_code: type_code.to_string(),
            type_name,
            requires_agreement,
            mandatory_ops,
            optional_ops,
        })
    }

    /// Analyze gaps for a CBU
    ///
    /// Returns missing required resources based on the CBU's type configuration.
    #[cfg(feature = "database")]
    pub async fn analyze_gaps(&self, cbu_id: Uuid) -> Result<Vec<Gap>> {
        let meta = D::metadata();

        // Query the gap view - column names vary by domain so we map them
        let query = format!(
            r#"SELECT
                cbu_id, cbu_name,
                COALESCE({}, '') as type_code,
                COALESCE({}, '') as op_code,
                COALESCE({}, '') as op_name,
                COALESCE(is_mandatory, true) as is_mandatory,
                missing_resource_code, missing_resource_name,
                provisioning_verb, location_type,
                COALESCE(per_market, false) as per_market,
                COALESCE(per_currency, false) as per_currency,
                COALESCE(per_counterparty, false) as per_counterparty
               FROM {}
               WHERE cbu_id = $1
               ORDER BY type_code, op_code, missing_resource_code"#,
            Self::gap_view_type_col(),
            Self::gap_view_op_col(),
            Self::gap_view_op_name_col(),
            meta.fq_gap_view()
        );

        let rows: Vec<(
            Uuid,
            String, // cbu_id, cbu_name
            String,
            String,
            String, // type_code, op_code, op_name
            bool,   // is_mandatory
            String,
            String, // missing_resource_code, missing_resource_name
            Option<String>,
            Option<String>, // provisioning_verb, location_type
            bool,
            bool,
            bool, // per_market, per_currency, per_counterparty
        )> = sqlx::query_as(&query)
            .bind(cbu_id)
            .fetch_all(&self.pool)
            .await?;

        let gaps = rows
            .into_iter()
            .map(|r| Gap {
                cbu_id: r.0,
                cbu_name: r.1,
                type_code: r.2,
                op_code: r.3,
                op_name: r.4,
                is_mandatory: r.5,
                missing_resource_code: r.6,
                missing_resource_name: r.7,
                provisioning_verb: r.8,
                location_type: r.9,
                per_market: r.10,
                per_currency: r.11,
                per_counterparty: r.12,
            })
            .collect();

        Ok(gaps)
    }

    /// Check readiness for a specific type
    ///
    /// Returns whether the CBU is ready (no blocking gaps) for the given type.
    #[cfg(feature = "database")]
    pub async fn check_readiness(&self, cbu_id: Uuid, type_code: &str) -> Result<ReadinessResult> {
        let gaps = self.analyze_gaps(cbu_id).await?;

        let blocking_gaps: Vec<Gap> = gaps
            .iter()
            .filter(|g| g.type_code == type_code && g.is_mandatory)
            .cloned()
            .collect();

        let warnings: Vec<Gap> = gaps
            .iter()
            .filter(|g| g.type_code == type_code && !g.is_mandatory)
            .cloned()
            .collect();

        Ok(ReadinessResult {
            ready: blocking_gaps.is_empty(),
            type_code: type_code.to_string(),
            blocking_gaps,
            warnings,
        })
    }

    // Helper methods for domain-specific column name mapping

    fn fq_type_table() -> String {
        let meta = D::metadata();
        // Handle cross-schema case for instrument domain
        if meta.type_table == "instrument_classes" {
            "custody.instrument_classes".to_string()
        } else {
            meta.fq_type_table()
        }
    }

    fn gap_view_type_col() -> &'static str {
        let meta = D::metadata();
        if meta.type_table == "instrument_classes" {
            "instrument_class"
        } else {
            "product_code"
        }
    }

    fn gap_view_op_col() -> &'static str {
        let meta = D::metadata();
        if meta.op_table == "lifecycles" {
            "lifecycle_code"
        } else {
            "service_code"
        }
    }

    fn gap_view_op_name_col() -> &'static str {
        let meta = D::metadata();
        if meta.op_table == "lifecycles" {
            "lifecycle_name"
        } else {
            "service_name"
        }
    }
}

/// Readiness check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResult {
    pub ready: bool,
    pub type_code: String,
    pub blocking_gaps: Vec<Gap>,
    pub warnings: Vec<Gap>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::{InstrumentDomain, ProductDomain};

    #[test]
    fn test_product_ops_type_table() {
        assert_eq!(
            TaxonomyOps::<ProductDomain>::fq_type_table(),
            "\"ob-poc\".products"
        );
    }

    #[test]
    fn test_instrument_ops_type_table() {
        assert_eq!(
            TaxonomyOps::<InstrumentDomain>::fq_type_table(),
            "custody.instrument_classes"
        );
    }

    #[test]
    fn test_gap_view_columns_product() {
        assert_eq!(
            TaxonomyOps::<ProductDomain>::gap_view_type_col(),
            "product_code"
        );
        assert_eq!(
            TaxonomyOps::<ProductDomain>::gap_view_op_col(),
            "service_code"
        );
    }

    #[test]
    fn test_gap_view_columns_instrument() {
        assert_eq!(
            TaxonomyOps::<InstrumentDomain>::gap_view_type_col(),
            "instrument_class"
        );
        assert_eq!(
            TaxonomyOps::<InstrumentDomain>::gap_view_op_col(),
            "lifecycle_code"
        );
    }
}
