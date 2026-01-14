//! Generic Fund Programme Loader
//!
//! Loads fund structures from CSV files using a YAML config for column mapping.
//! Supports any fund programme (Allianz, BlackRock, Vanguard, etc.) with different
//! CSV schemas.
//!
//! # Usage
//!
//! ```bash
//! cargo xtask load-fund-programme --config data/configs/allianz.yaml --input data/funds.csv
//! ```
//!
//! # Config Format
//!
//! ```yaml
//! programme_name: "Allianz Global Investors"
//! defaults:
//!   holder_affiliation: INTRA_GROUP
//!   bo_data_available: false
//!   domicile_country: LU
//! column_mapping:
//!   lei: "LEI"                    # Column name in CSV
//!   entity_name: "Fund Name"
//!   vehicle_type: "Vehicle Type"
//!   umbrella_lei: "Umbrella LEI"  # Optional
//!   compartment_code: "Compartment"
//!   manager_lei: "Manager LEI"
//! vehicle_type_mapping:
//!   "SCSp": SCSP
//!   "SICAV-RAIF": SICAV_RAIF
//!   "SICAV-SIF": SICAV_SIF
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Configuration for a fund programme loader
#[derive(Debug, Deserialize, Serialize)]
pub struct FundProgrammeConfig {
    /// Name of the fund programme (for logging/display)
    pub programme_name: String,

    /// Default values for fields not in CSV
    #[serde(default)]
    pub defaults: FundDefaults,

    /// Column name mapping (config key -> CSV column name)
    pub column_mapping: ColumnMapping,

    /// Vehicle type value mapping (CSV value -> DB enum value)
    #[serde(default)]
    pub vehicle_type_mapping: HashMap<String, String>,

    /// Investor type value mapping (CSV value -> DB enum value)
    #[serde(default)]
    pub investor_type_mapping: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct FundDefaults {
    /// Default holder affiliation: INTRA_GROUP, EXTERNAL, MIXED, UNKNOWN
    pub holder_affiliation: Option<String>,
    /// Default for BO data availability
    pub bo_data_available: Option<bool>,
    /// Default domicile country (ISO 2-letter)
    pub domicile_country: Option<String>,
    /// Default role type for investors
    pub role_type: Option<String>,
    /// Default lookthrough policy
    pub lookthrough_policy: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ColumnMapping {
    /// LEI column (required for entity matching)
    pub lei: Option<String>,
    /// Entity name column (required)
    pub entity_name: String,
    /// Vehicle type column
    pub vehicle_type: Option<String>,
    /// Umbrella LEI column (for sub-fund relationships)
    pub umbrella_lei: Option<String>,
    /// Compartment code column
    pub compartment_code: Option<String>,
    /// Compartment name column
    pub compartment_name: Option<String>,
    /// Manager LEI column
    pub manager_lei: Option<String>,
    /// Domicile country column
    pub domicile_country: Option<String>,
    /// Is umbrella flag column
    pub is_umbrella: Option<String>,
    /// Investor type column (for investor records)
    pub investor_type: Option<String>,
    /// Holder affiliation column
    pub holder_affiliation: Option<String>,
    /// BO data available column
    pub bo_data_available: Option<String>,
}

/// A parsed fund record from CSV
#[derive(Debug, Clone)]
pub struct FundRecord {
    pub lei: Option<String>,
    pub entity_name: String,
    pub vehicle_type: Option<String>,
    pub umbrella_lei: Option<String>,
    pub compartment_code: Option<String>,
    pub compartment_name: Option<String>,
    pub manager_lei: Option<String>,
    pub domicile_country: Option<String>,
    pub is_umbrella: bool,
    pub investor_type: Option<String>,
    pub holder_affiliation: Option<String>,
    pub bo_data_available: Option<bool>,
}

/// Result of loading a fund programme
#[derive(Debug, Default)]
pub struct LoadResult {
    pub entities_created: usize,
    pub entities_updated: usize,
    pub fund_vehicles_created: usize,
    pub compartments_created: usize,
    pub role_profiles_created: usize,
    pub errors: Vec<String>,
}

impl FundProgrammeConfig {
    /// Load config from YAML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Self = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    /// Parse a CSV row into a FundRecord using this config
    pub fn parse_row(
        &self,
        row: &csv::StringRecord,
        headers: &csv::StringRecord,
    ) -> Result<FundRecord> {
        let get_field = |col_name: &str| -> Option<String> {
            headers
                .iter()
                .position(|h| h == col_name)
                .and_then(|idx| row.get(idx))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };

        let entity_name = get_field(&self.column_mapping.entity_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Missing required field: {}",
                self.column_mapping.entity_name
            )
        })?;

        let lei = self
            .column_mapping
            .lei
            .as_ref()
            .and_then(|col| get_field(col));

        let vehicle_type_raw = self
            .column_mapping
            .vehicle_type
            .as_ref()
            .and_then(|col| get_field(col));
        let vehicle_type =
            vehicle_type_raw.map(|v| self.vehicle_type_mapping.get(&v).cloned().unwrap_or(v));

        let investor_type_raw = self
            .column_mapping
            .investor_type
            .as_ref()
            .and_then(|col| get_field(col));
        let investor_type =
            investor_type_raw.map(|v| self.investor_type_mapping.get(&v).cloned().unwrap_or(v));

        let is_umbrella_str = self
            .column_mapping
            .is_umbrella
            .as_ref()
            .and_then(|col| get_field(col));
        let is_umbrella = is_umbrella_str
            .map(|s| s.eq_ignore_ascii_case("true") || s == "1" || s.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);

        let bo_data_str = self
            .column_mapping
            .bo_data_available
            .as_ref()
            .and_then(|col| get_field(col));
        let bo_data_available = bo_data_str
            .map(|s| s.eq_ignore_ascii_case("true") || s == "1" || s.eq_ignore_ascii_case("yes"))
            .or(self.defaults.bo_data_available);

        Ok(FundRecord {
            lei,
            entity_name,
            vehicle_type,
            umbrella_lei: self
                .column_mapping
                .umbrella_lei
                .as_ref()
                .and_then(|col| get_field(col)),
            compartment_code: self
                .column_mapping
                .compartment_code
                .as_ref()
                .and_then(|col| get_field(col)),
            compartment_name: self
                .column_mapping
                .compartment_name
                .as_ref()
                .and_then(|col| get_field(col)),
            manager_lei: self
                .column_mapping
                .manager_lei
                .as_ref()
                .and_then(|col| get_field(col)),
            domicile_country: self
                .column_mapping
                .domicile_country
                .as_ref()
                .and_then(|col| get_field(col))
                .or_else(|| self.defaults.domicile_country.clone()),
            is_umbrella,
            investor_type,
            holder_affiliation: self
                .column_mapping
                .holder_affiliation
                .as_ref()
                .and_then(|col| get_field(col))
                .or_else(|| self.defaults.holder_affiliation.clone()),
            bo_data_available,
        })
    }
}

/// Generate DSL statements for a fund record
pub fn generate_dsl_for_fund(record: &FundRecord, config: &FundProgrammeConfig) -> Vec<String> {
    let mut statements = Vec::new();

    // 1. Create or ensure entity exists
    let entity_ref = if let Some(ref lei) = record.lei {
        format!(r#"(entity-ref legal-entity (k lei "{}"))"#, lei)
    } else {
        format!(
            r#"(entity-ref legal-entity (k name "{}"))"#,
            record.entity_name.replace('"', r#"\""#)
        )
    };

    // Entity upsert (if we have LEI)
    if record.lei.is_some() {
        let mut args = vec![format!(
            r#":name "{}""#,
            record.entity_name.replace('"', r#"\""#)
        )];
        if let Some(ref lei) = record.lei {
            args.push(format!(r#":lei "{}""#, lei));
        }
        statements.push(format!(
            "(entity.ensure-legal-entity {} :as @fund)",
            args.join(" ")
        ));
    }

    // 2. Create fund vehicle if vehicle_type is specified
    if let Some(ref vehicle_type) = record.vehicle_type {
        let mut vehicle_args = vec![
            format!(":fund-entity-id {}", entity_ref),
            format!(r#":vehicle-type "{}""#, vehicle_type),
        ];
        if record.is_umbrella {
            vehicle_args.push(":is-umbrella true".to_string());
        }
        if let Some(ref domicile) = record.domicile_country {
            vehicle_args.push(format!(r#":domicile-country "{}""#, domicile));
        }
        if let Some(ref umbrella_lei) = record.umbrella_lei {
            vehicle_args.push(format!(
                r#":umbrella-entity-id (entity-ref legal-entity (k lei "{}"))"#,
                umbrella_lei
            ));
        }
        if let Some(ref manager_lei) = record.manager_lei {
            vehicle_args.push(format!(
                r#":manager-entity-id (entity-ref legal-entity (k lei "{}"))"#,
                manager_lei
            ));
        }
        statements.push(format!("(fund-vehicle.upsert {})", vehicle_args.join(" ")));
    }

    // 3. Create compartment if specified
    if let Some(ref compartment_code) = record.compartment_code {
        if let Some(ref umbrella_lei) = record.umbrella_lei {
            let mut comp_args = vec![
                format!(
                    r#":umbrella-fund-entity-id (entity-ref legal-entity (k lei "{}"))"#,
                    umbrella_lei
                ),
                format!(r#":compartment-code "{}""#, compartment_code),
            ];
            if let Some(ref name) = record.compartment_name {
                comp_args.push(format!(
                    r#":compartment-name "{}""#,
                    name.replace('"', r#"\""#)
                ));
            }
            statements.push(format!("(fund-compartment.upsert {})", comp_args.join(" ")));
        }
    }

    // 4. Create role profile if holder affiliation specified
    if record.holder_affiliation.is_some() || record.bo_data_available.is_some() {
        let role_type = config
            .defaults
            .role_type
            .as_deref()
            .unwrap_or("END_INVESTOR");
        let lookthrough = config
            .defaults
            .lookthrough_policy
            .as_deref()
            .unwrap_or("NONE");

        let mut role_args = vec![
            format!(":holder {}", entity_ref),
            format!(r#":role-type "{}""#, role_type),
            format!(r#":lookthrough-policy "{}""#, lookthrough),
        ];
        if let Some(ref affiliation) = record.holder_affiliation {
            role_args.push(format!(r#":holder-affiliation "{}""#, affiliation));
        }
        if let Some(bo_available) = record.bo_data_available {
            role_args.push(format!(":bo-data-available {}", bo_available));
        }
        // Note: issuer-entity-id would need to be provided separately
        // This is a template that needs the issuer context
    }

    statements
}

/// Load funds from CSV using config
pub fn load_csv_with_config(
    csv_path: &Path,
    config: &FundProgrammeConfig,
    limit: Option<usize>,
) -> Result<Vec<FundRecord>> {
    let mut reader = csv::Reader::from_path(csv_path)
        .with_context(|| format!("Failed to open CSV file: {}", csv_path.display()))?;

    let headers = reader.headers()?.clone();
    let mut records = Vec::new();

    for (idx, result) in reader.records().enumerate() {
        if let Some(lim) = limit {
            if idx >= lim {
                break;
            }
        }

        let row = result.with_context(|| format!("Failed to read row {}", idx + 1))?;
        match config.parse_row(&row, &headers) {
            Ok(record) => records.push(record),
            Err(e) => {
                eprintln!("Warning: Skipping row {}: {}", idx + 1, e);
            }
        }
    }

    Ok(records)
}

/// Generate complete DSL file for a fund programme
pub fn generate_dsl_file(records: &[FundRecord], config: &FundProgrammeConfig) -> String {
    let mut lines = Vec::new();

    lines.push(format!(";; Fund Programme: {}", config.programme_name));
    lines.push(format!(
        ";; Generated: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    lines.push(format!(";; Records: {}", records.len()));
    lines.push(String::new());

    // First pass: create all entities
    lines.push(
        ";; ============================================================================="
            .to_string(),
    );
    lines.push(";; ENTITIES".to_string());
    lines.push(
        ";; ============================================================================="
            .to_string(),
    );
    lines.push(String::new());

    for record in records {
        let stmts = generate_dsl_for_fund(record, config);
        for stmt in stmts {
            lines.push(stmt);
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parse() {
        let yaml = r#"
programme_name: "Test Programme"
defaults:
  holder_affiliation: INTRA_GROUP
  bo_data_available: false
column_mapping:
  entity_name: "Fund Name"
  lei: "LEI"
  vehicle_type: "Type"
vehicle_type_mapping:
  "SCSp": SCSP
  "SICAV-RAIF": SICAV_RAIF
"#;
        let config: FundProgrammeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.programme_name, "Test Programme");
        assert_eq!(
            config.defaults.holder_affiliation,
            Some("INTRA_GROUP".to_string())
        );
        assert_eq!(
            config.vehicle_type_mapping.get("SCSp"),
            Some(&"SCSP".to_string())
        );
    }
}
