//! Domain metadata types, YAML loader, and reverse index computation.
//!
//! The domain metadata overlay provides:
//! - Business descriptions, governance tiers, security labels for tables
//! - Verb↔table data footprint (which verbs read/write which tables)
//! - Reverse index: table→verbs (computed from verb→tables forward mapping)
//!
//! Loaded from `config/sem_os_seeds/domain_metadata.yaml` and merged into
//! the seed bundle at `build_seed_bundle()` time.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Root of the domain metadata YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMetadataFile {
    pub domains: BTreeMap<String, DomainEntry>,
}

/// A single business domain (e.g., "deal", "kyc", "cbu").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEntry {
    pub description: String,
    #[serde(default)]
    pub tables: BTreeMap<String, TableMetadata>,
    #[serde(default)]
    pub verb_data_footprint: BTreeMap<String, VerbFootprint>,
}

/// Metadata overlay for a single database table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub description: String,
    pub governance_tier: GovernanceTierLabel,
    pub classification: ClassificationLabel,
    #[serde(default)]
    pub pii: bool,
}

/// Governance tier as specified in YAML (lowercase).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GovernanceTierLabel {
    Governed,
    Operational,
}

/// Security classification as specified in YAML (lowercase).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClassificationLabel {
    Public,
    Internal,
    Confidential,
    Restricted,
}

/// Which tables a verb reads from and writes to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbFootprint {
    #[serde(default)]
    pub reads: Vec<String>,
    #[serde(default)]
    pub writes: Vec<String>,
}

/// Computed reverse index: for each table, which verbs read/write it.
#[derive(Debug, Clone, Default)]
pub struct ReverseVerbIndex {
    /// table_name → set of verb FQNs that read from this table
    pub read_by: HashMap<String, HashSet<String>>,
    /// table_name → set of verb FQNs that write to this table
    pub written_by: HashMap<String, HashSet<String>>,
}

/// Fully loaded domain metadata with computed reverse index.
#[derive(Debug, Clone)]
pub struct DomainMetadata {
    pub domains: BTreeMap<String, DomainEntry>,
    pub reverse_index: ReverseVerbIndex,
}

impl DomainMetadata {
    /// Load domain metadata from YAML string content.
    pub fn from_yaml(yaml_str: &str) -> Result<Self, serde_yaml::Error> {
        let file: DomainMetadataFile = serde_yaml::from_str(yaml_str)?;
        let reverse_index = compute_reverse_index(&file.domains);
        Ok(Self {
            domains: file.domains,
            reverse_index,
        })
    }

    /// Load domain metadata from a file path.
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read domain metadata from {}: {}",
                path.display(),
                e
            )
        })?;
        Self::from_yaml(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse domain metadata YAML: {e}"))
    }

    /// Look up table metadata by table name across all domains.
    /// Returns (domain_key, table_metadata) if found.
    pub fn find_table(&self, table_name: &str) -> Option<(&str, &TableMetadata)> {
        for (domain_key, domain) in &self.domains {
            if let Some(meta) = domain.tables.get(table_name) {
                return Some((domain_key, meta));
            }
        }
        None
    }

    /// Look up table metadata with schema-qualified name (e.g., "kyc.cases").
    /// Falls back to unqualified lookup if schema doesn't match a domain.
    pub fn find_table_qualified(
        &self,
        schema: Option<&str>,
        table_name: &str,
    ) -> Option<(&str, &TableMetadata)> {
        // For kyc schema tables, the YAML uses "kyc.table_name" keys
        if let Some(schema) = schema {
            if schema != "ob-poc" && schema != "public" {
                let qualified = format!("{}.{}", schema, table_name);
                for (domain_key, domain) in &self.domains {
                    if let Some(meta) = domain.tables.get(&qualified) {
                        return Some((domain_key, meta));
                    }
                }
            }
        }
        // Fall back to unqualified lookup
        self.find_table(table_name)
    }

    /// Get verb footprint for a specific verb FQN.
    pub fn find_verb_footprint(&self, verb_fqn: &str) -> Option<&VerbFootprint> {
        for domain in self.domains.values() {
            if let Some(footprint) = domain.verb_data_footprint.get(verb_fqn) {
                return Some(footprint);
            }
        }
        None
    }

    /// Get all verb FQNs that read from a table.
    pub fn verbs_reading(&self, table_name: &str) -> Vec<&str> {
        self.reverse_index
            .read_by
            .get(table_name)
            .map(|set| set.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get all verb FQNs that write to a table.
    pub fn verbs_writing(&self, table_name: &str) -> Vec<&str> {
        self.reverse_index
            .written_by
            .get(table_name)
            .map(|set| set.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}

/// Compute the reverse index (table → verbs) from the forward mapping (verb → tables).
fn compute_reverse_index(domains: &BTreeMap<String, DomainEntry>) -> ReverseVerbIndex {
    let mut index = ReverseVerbIndex::default();
    for domain in domains.values() {
        for (verb_fqn, footprint) in &domain.verb_data_footprint {
            for table in &footprint.reads {
                index
                    .read_by
                    .entry(table.clone())
                    .or_default()
                    .insert(verb_fqn.clone());
            }
            for table in &footprint.writes {
                index
                    .written_by
                    .entry(table.clone())
                    .or_default()
                    .insert(verb_fqn.clone());
            }
        }
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_YAML: &str = r#"
domains:
  deal:
    description: "Commercial origination and deal lifecycle management"
    tables:
      deals:
        description: "Commercial deal record"
        governance_tier: governed
        classification: confidential
        pii: false
      deal_events:
        description: "Immutable audit trail"
        governance_tier: operational
        classification: internal
        pii: false
    verb_data_footprint:
      deal.create:
        writes: [deals, deal_events]
        reads: [client_group]
      deal.summary:
        reads: [deals, deal_participants, deal_contracts]
  cbu:
    description: "Client Business Unit"
    tables:
      cbus:
        description: "Client Business Unit — operational container"
        governance_tier: governed
        classification: internal
        pii: false
    verb_data_footprint:
      cbu.create:
        writes: [cbus]
"#;

    #[test]
    fn parse_minimal_yaml() {
        let meta = DomainMetadata::from_yaml(MINIMAL_YAML).unwrap();
        assert_eq!(meta.domains.len(), 2);
        assert!(meta.domains.contains_key("deal"));
        assert!(meta.domains.contains_key("cbu"));

        let deal = &meta.domains["deal"];
        assert_eq!(deal.tables.len(), 2);
        assert_eq!(deal.verb_data_footprint.len(), 2);
    }

    #[test]
    fn find_table_works() {
        let meta = DomainMetadata::from_yaml(MINIMAL_YAML).unwrap();
        let (domain, table) = meta.find_table("deals").unwrap();
        assert_eq!(domain, "deal");
        assert_eq!(table.description, "Commercial deal record");
        assert_eq!(table.governance_tier, GovernanceTierLabel::Governed);
        assert_eq!(table.classification, ClassificationLabel::Confidential);
        assert!(!table.pii);
    }

    #[test]
    fn find_table_missing() {
        let meta = DomainMetadata::from_yaml(MINIMAL_YAML).unwrap();
        assert!(meta.find_table("nonexistent").is_none());
    }

    #[test]
    fn reverse_index_computed() {
        let meta = DomainMetadata::from_yaml(MINIMAL_YAML).unwrap();

        // deals table is written by deal.create
        let writers = meta.verbs_writing("deals");
        assert!(writers.contains(&"deal.create"));

        // deals table is read by deal.summary
        let readers = meta.verbs_reading("deals");
        assert!(readers.contains(&"deal.summary"));

        // deal_events is written by deal.create
        let writers = meta.verbs_writing("deal_events");
        assert!(writers.contains(&"deal.create"));

        // client_group is read by deal.create (cross-domain read)
        let readers = meta.verbs_reading("client_group");
        assert!(readers.contains(&"deal.create"));

        // cbus is written by cbu.create
        let writers = meta.verbs_writing("cbus");
        assert!(writers.contains(&"cbu.create"));
    }

    #[test]
    fn verb_footprint_lookup() {
        let meta = DomainMetadata::from_yaml(MINIMAL_YAML).unwrap();

        let fp = meta.find_verb_footprint("deal.create").unwrap();
        assert_eq!(fp.writes, vec!["deals", "deal_events"]);
        assert_eq!(fp.reads, vec!["client_group"]);

        assert!(meta.find_verb_footprint("nonexistent.verb").is_none());
    }

    #[test]
    fn serde_round_trip() {
        let meta = DomainMetadata::from_yaml(MINIMAL_YAML).unwrap();
        let file = DomainMetadataFile {
            domains: meta.domains.clone(),
        };
        let yaml = serde_yaml::to_string(&file).unwrap();
        let _restored = DomainMetadata::from_yaml(&yaml).unwrap();
    }

    #[test]
    fn load_real_domain_metadata_yaml() {
        let yaml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("config/sem_os_seeds/domain_metadata.yaml");
        if !yaml_path.exists() {
            // Skip if the file hasn't been created yet
            return;
        }
        let meta = DomainMetadata::from_file(&yaml_path)
            .unwrap_or_else(|e| panic!("Failed to load domain_metadata.yaml: {e}"));

        // Verify basic structure
        assert!(
            meta.domains.len() >= 10,
            "Expected at least 10 domains, got {}",
            meta.domains.len()
        );

        // Verify key domains are present
        for expected in &["deal", "cbu", "entity", "kyc", "billing"] {
            assert!(
                meta.domains.contains_key(*expected),
                "Missing expected domain: {expected}"
            );
        }

        // Verify tables exist in deal domain
        let deal = &meta.domains["deal"];
        assert!(!deal.tables.is_empty(), "deal domain should have tables");
        assert!(
            deal.tables.contains_key("deals"),
            "deal domain should have 'deals' table"
        );

        // Verify verb footprints exist
        assert!(
            !deal.verb_data_footprint.is_empty(),
            "deal domain should have verb_data_footprint entries"
        );

        // Verify reverse index is populated
        let writers = meta.verbs_writing("deals");
        assert!(!writers.is_empty(), "deals table should have writers");

        // Verify round-trip serialization
        let file = DomainMetadataFile {
            domains: meta.domains.clone(),
        };
        let yaml = serde_yaml::to_string(&file).unwrap();
        let restored = DomainMetadata::from_yaml(&yaml).unwrap();
        assert_eq!(meta.domains.len(), restored.domains.len());
    }
}
