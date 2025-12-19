//! Adversarial Pattern Detection
//!
//! Detects suspicious patterns in ownership structures that may indicate
//! money laundering, tax evasion, or obfuscation of beneficial ownership.
//!
//! ## Pattern Types
//!
//! - **Circular Ownership**: A owns B owns C owns A
//! - **Layering**: 5+ entities in ownership chain
//! - **Nominee Usage**: Nominee directors, corporate trustees
//! - **Opacity Jurisdictions**: BVI, Cayman, Panama, etc.
//! - **Registry Mismatch**: GLEIF vs client claims don't match
//! - **Ownership Gaps**: Ownership doesn't sum to 100%
//! - **Recent Restructuring**: Structure changed during KYC

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Pattern Types
// ============================================================================

/// Types of suspicious patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PatternType {
    /// Circular ownership chain detected (A → B → C → A)
    CircularOwnership,
    /// Deep layering (5+ entities in chain)
    Layering,
    /// Nominee director or corporate trustee patterns
    NomineeUsage,
    /// Entity in opacity/secrecy jurisdiction
    OpacityJurisdiction,
    /// Mismatch between claimed and registry data
    RegistryMismatch,
    /// Ownership percentages don't sum to 100%
    OwnershipGaps,
    /// Structure changed during KYC process
    RecentRestructuring,
    /// Multiple roles suggesting control obfuscation
    RoleConcentration,
}

impl PatternType {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            PatternType::CircularOwnership => "CIRCULAR_OWNERSHIP",
            PatternType::Layering => "LAYERING",
            PatternType::NomineeUsage => "NOMINEE_USAGE",
            PatternType::OpacityJurisdiction => "OPACITY_JURISDICTION",
            PatternType::RegistryMismatch => "REGISTRY_MISMATCH",
            PatternType::OwnershipGaps => "OWNERSHIP_GAPS",
            PatternType::RecentRestructuring => "RECENT_RESTRUCTURING",
            PatternType::RoleConcentration => "ROLE_CONCENTRATION",
        }
    }
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Severity of detected pattern
/// Aligned with DB CHECK constraint: INFO, LOW, MEDIUM, HIGH, CRITICAL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PatternSeverity {
    /// Informational only, no action required
    Info,
    /// Minor concern, informational
    Low,
    /// Moderate concern, needs review
    Medium,
    /// Significant concern, requires investigation
    High,
    /// Critical concern, may block approval
    Critical,
}

impl PatternSeverity {
    /// Get severity from pattern type (default severity)
    pub fn from_pattern_type(pattern_type: PatternType) -> Self {
        match pattern_type {
            PatternType::CircularOwnership => PatternSeverity::Critical,
            PatternType::Layering => PatternSeverity::High,
            PatternType::NomineeUsage => PatternSeverity::Medium,
            PatternType::OpacityJurisdiction => PatternSeverity::High,
            PatternType::RegistryMismatch => PatternSeverity::High,
            PatternType::OwnershipGaps => PatternSeverity::Medium,
            PatternType::RecentRestructuring => PatternSeverity::Medium,
            PatternType::RoleConcentration => PatternSeverity::Low,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            PatternSeverity::Info => "INFO",
            PatternSeverity::Low => "LOW",
            PatternSeverity::Medium => "MEDIUM",
            PatternSeverity::High => "HIGH",
            PatternSeverity::Critical => "CRITICAL",
        }
    }
}

impl std::fmt::Display for PatternSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Detected Pattern
// ============================================================================

/// A detected pattern with evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    /// Pattern type
    pub pattern_type: PatternType,

    /// Severity
    pub severity: PatternSeverity,

    /// Human-readable description
    pub description: String,

    /// Entities involved in the pattern
    pub involved_entities: Vec<Uuid>,

    /// Pattern-specific evidence
    pub evidence: PatternEvidence,

    /// When detected
    pub detected_at: DateTime<Utc>,
}

/// Pattern-specific evidence data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatternEvidence {
    /// Circular ownership chain
    CircularChain {
        /// The cycle path: [A, B, C, A]
        chain: Vec<Uuid>,
        /// Entity names for display
        chain_names: Vec<String>,
    },

    /// Deep layering
    LayeringChain {
        /// Full ownership chain
        chain: Vec<Uuid>,
        /// Chain depth
        depth: usize,
        /// Threshold that was exceeded
        threshold: usize,
    },

    /// Nominee pattern
    NomineePattern {
        /// Nominee entity
        nominee_entity_id: Uuid,
        /// Role that triggered detection
        role: String,
        /// Indicator type (CORPORATE_DIRECTOR, TRUST_COMPANY, etc.)
        indicator: String,
    },

    /// Opacity jurisdiction
    OpacityJurisdiction {
        /// Entity in opacity jurisdiction
        entity_id: Uuid,
        /// Jurisdiction code
        jurisdiction: String,
        /// Risk tier
        risk_tier: String,
    },

    /// Registry mismatch
    RegistryMismatch {
        /// Entity with mismatch
        entity_id: Uuid,
        /// Attribute that mismatches
        attribute: String,
        /// Claimed value
        claimed_value: String,
        /// Registry value
        registry_value: String,
        /// Registry source
        registry_source: String,
    },

    /// Ownership gap
    OwnershipGap {
        /// Entity with incomplete ownership
        entity_id: Uuid,
        /// Total ownership percentage accounted for
        total_percentage: f64,
        /// Gap amount
        gap: f64,
    },

    /// Recent restructuring
    Restructuring {
        /// Entity that was restructured
        entity_id: Uuid,
        /// Type of change
        change_type: String,
        /// When the change occurred
        change_date: DateTime<Utc>,
    },

    /// Role concentration
    RoleConcentration {
        /// Person with multiple roles
        person_id: Uuid,
        /// Roles held
        roles: Vec<String>,
        /// Entities where roles are held
        entities: Vec<Uuid>,
    },
}

// ============================================================================
// Pattern Detector
// ============================================================================

/// Configuration for pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternDetectorConfig {
    /// Maximum chain depth before triggering layering alert
    pub layering_threshold: usize,

    /// Opacity jurisdictions (high secrecy)
    pub opacity_jurisdictions: HashSet<String>,

    /// High-risk jurisdictions (elevated concern)
    pub high_risk_jurisdictions: HashSet<String>,

    /// Role names that indicate nominee patterns
    pub nominee_role_indicators: HashSet<String>,

    /// Minimum ownership gap to flag
    pub ownership_gap_threshold: f64,

    /// Days before KYC that restructuring is suspicious
    pub restructuring_window_days: i64,
}

impl Default for PatternDetectorConfig {
    fn default() -> Self {
        Self {
            layering_threshold: 5,
            opacity_jurisdictions: [
                "VG", // British Virgin Islands
                "KY", // Cayman Islands
                "PA", // Panama
                "SC", // Seychelles
                "BZ", // Belize
                "MH", // Marshall Islands
                "WS", // Samoa
                "VU", // Vanuatu
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            high_risk_jurisdictions: [
                "AE", // UAE
                "SG", // Singapore (for specific structures)
                "HK", // Hong Kong
                "LU", // Luxembourg (for certain vehicles)
                "MT", // Malta
                "CY", // Cyprus
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            nominee_role_indicators: [
                "NOMINEE",
                "NOMINEE_DIRECTOR",
                "NOMINEE_SHAREHOLDER",
                "CORPORATE_DIRECTOR",
                "PROFESSIONAL_TRUSTEE",
                "TRUST_COMPANY",
                "REGISTERED_AGENT",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            ownership_gap_threshold: 5.0,   // 5% gap triggers alert
            restructuring_window_days: 180, // 6 months
        }
    }
}

/// Detects adversarial patterns in ownership structures
pub struct PatternDetector {
    config: PatternDetectorConfig,
}

impl PatternDetector {
    /// Create a new detector with default configuration
    pub fn new() -> Self {
        Self {
            config: PatternDetectorConfig::default(),
        }
    }

    /// Create a detector with custom configuration
    pub fn with_config(config: PatternDetectorConfig) -> Self {
        Self { config }
    }

    /// Detect all patterns for a CBU
    #[cfg(feature = "database")]
    pub async fn detect_all(
        &self,
        pool: &PgPool,
        cbu_id: Uuid,
    ) -> Result<Vec<DetectedPattern>, sqlx::Error> {
        let mut patterns = vec![];

        // 1. Detect circular ownership
        patterns.extend(self.detect_circular_ownership(pool, cbu_id).await?);

        // 2. Detect layering
        patterns.extend(self.detect_layering(pool, cbu_id).await?);

        // 3. Detect nominee usage
        patterns.extend(self.detect_nominee_usage(pool, cbu_id).await?);

        // 4. Detect opacity jurisdictions
        patterns.extend(self.detect_opacity_jurisdictions(pool, cbu_id).await?);

        // 5. Detect ownership gaps
        patterns.extend(self.detect_ownership_gaps(pool, cbu_id).await?);

        Ok(patterns)
    }

    /// Detect circular ownership using DFS cycle detection
    #[cfg(feature = "database")]
    pub async fn detect_circular_ownership(
        &self,
        pool: &PgPool,
        cbu_id: Uuid,
    ) -> Result<Vec<DetectedPattern>, sqlx::Error> {
        // Get all ownership relationships for entities linked to this CBU
        let relationships: Vec<(Uuid, Uuid, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT DISTINCT
                r.from_entity_id,
                r.to_entity_id,
                e1.name as from_name,
                e2.name as to_name
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = r.from_entity_id OR cer.entity_id = r.to_entity_id
            JOIN "ob-poc".entities e1 ON e1.entity_id = r.from_entity_id
            JOIN "ob-poc".entities e2 ON e2.entity_id = r.to_entity_id
            WHERE cer.cbu_id = $1
            AND r.relationship_type = 'ownership'
            AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        // Build adjacency list
        let mut graph: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut names: HashMap<Uuid, String> = HashMap::new();

        for (from_id, to_id, from_name, to_name) in relationships {
            graph.entry(from_id).or_default().push(to_id);
            if let Some(name) = from_name {
                names.insert(from_id, name);
            }
            if let Some(name) = to_name {
                names.insert(to_id, name);
            }
        }

        // DFS cycle detection
        let mut patterns = vec![];
        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut rec_stack: HashSet<Uuid> = HashSet::new();
        let mut path: Vec<Uuid> = vec![];

        for &start in graph.keys() {
            if let Some(cycle) =
                self.find_cycle(&graph, start, &mut visited, &mut rec_stack, &mut path)
            {
                let chain_names: Vec<String> = cycle
                    .iter()
                    .map(|id| names.get(id).cloned().unwrap_or_else(|| id.to_string()))
                    .collect();

                patterns.push(DetectedPattern {
                    pattern_type: PatternType::CircularOwnership,
                    severity: PatternSeverity::Critical,
                    description: format!(
                        "Circular ownership detected: {}",
                        chain_names.join(" → ")
                    ),
                    involved_entities: cycle.clone(),
                    evidence: PatternEvidence::CircularChain {
                        chain: cycle,
                        chain_names,
                    },
                    detected_at: Utc::now(),
                });
            }
        }

        Ok(patterns)
    }

    /// DFS helper to find cycles
    #[allow(clippy::only_used_in_recursion)]
    fn find_cycle(
        &self,
        graph: &HashMap<Uuid, Vec<Uuid>>,
        node: Uuid,
        visited: &mut HashSet<Uuid>,
        rec_stack: &mut HashSet<Uuid>,
        path: &mut Vec<Uuid>,
    ) -> Option<Vec<Uuid>> {
        visited.insert(node);
        rec_stack.insert(node);
        path.push(node);

        if let Some(neighbors) = graph.get(&node) {
            for &neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    if let Some(cycle) = self.find_cycle(graph, neighbor, visited, rec_stack, path)
                    {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(&neighbor) {
                    // Found cycle - extract it from path
                    let cycle_start = path.iter().position(|&n| n == neighbor).unwrap();
                    let mut cycle: Vec<Uuid> = path[cycle_start..].to_vec();
                    cycle.push(neighbor); // Complete the cycle
                    return Some(cycle);
                }
            }
        }

        path.pop();
        rec_stack.remove(&node);
        None
    }

    /// Detect deep layering (chains exceeding threshold)
    #[cfg(feature = "database")]
    pub async fn detect_layering(
        &self,
        pool: &PgPool,
        cbu_id: Uuid,
    ) -> Result<Vec<DetectedPattern>, sqlx::Error> {
        // Use recursive CTE to find chain depths
        let deep_chains: Vec<(Uuid, i32)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Base: entities directly linked to CBU
                SELECT
                    r.from_entity_id as entity_id,
                    r.to_entity_id as root_entity_id,
                    1 as depth,
                    ARRAY[r.from_entity_id] as path
                FROM "ob-poc".entity_relationships r
                JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = r.to_entity_id
                WHERE cer.cbu_id = $1
                AND r.relationship_type = 'ownership'
                AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)

                UNION ALL

                -- Recursive: follow ownership chain upward
                SELECT
                    r.from_entity_id,
                    oc.root_entity_id,
                    oc.depth + 1,
                    oc.path || r.from_entity_id
                FROM ownership_chain oc
                JOIN "ob-poc".entity_relationships r ON r.to_entity_id = oc.entity_id
                WHERE oc.depth < 20
                AND NOT r.from_entity_id = ANY(oc.path)
                AND r.relationship_type = 'ownership'
                AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            )
            SELECT root_entity_id, MAX(depth) as max_depth
            FROM ownership_chain
            GROUP BY root_entity_id
            HAVING MAX(depth) >= $2
            "#,
        )
        .bind(cbu_id)
        .bind(self.config.layering_threshold as i32)
        .fetch_all(pool)
        .await?;

        let patterns: Vec<DetectedPattern> = deep_chains
            .into_iter()
            .map(|(entity_id, depth)| DetectedPattern {
                pattern_type: PatternType::Layering,
                severity: if depth >= 10 {
                    PatternSeverity::Critical
                } else if depth >= 7 {
                    PatternSeverity::High
                } else {
                    PatternSeverity::Medium
                },
                description: format!(
                    "Deep ownership layering detected: {} levels (threshold: {})",
                    depth, self.config.layering_threshold
                ),
                involved_entities: vec![entity_id],
                evidence: PatternEvidence::LayeringChain {
                    chain: vec![entity_id],
                    depth: depth as usize,
                    threshold: self.config.layering_threshold,
                },
                detected_at: Utc::now(),
            })
            .collect();

        Ok(patterns)
    }

    /// Detect nominee usage patterns
    #[cfg(feature = "database")]
    pub async fn detect_nominee_usage(
        &self,
        pool: &PgPool,
        cbu_id: Uuid,
    ) -> Result<Vec<DetectedPattern>, sqlx::Error> {
        // Look for roles that indicate nominee patterns
        let nominees: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT cer.entity_id, r.name as role_name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cer.cbu_id = $1
            AND UPPER(r.name) = ANY($2)
            "#,
        )
        .bind(cbu_id)
        .bind(
            self.config
                .nominee_role_indicators
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
        )
        .fetch_all(pool)
        .await?;

        let patterns: Vec<DetectedPattern> = nominees
            .into_iter()
            .map(|(entity_id, role)| DetectedPattern {
                pattern_type: PatternType::NomineeUsage,
                severity: PatternSeverity::Medium,
                description: format!("Nominee pattern detected: entity has '{}' role", role),
                involved_entities: vec![entity_id],
                evidence: PatternEvidence::NomineePattern {
                    nominee_entity_id: entity_id,
                    role: role.clone(),
                    indicator: role,
                },
                detected_at: Utc::now(),
            })
            .collect();

        Ok(patterns)
    }

    /// Detect entities in opacity jurisdictions
    #[cfg(feature = "database")]
    pub async fn detect_opacity_jurisdictions(
        &self,
        pool: &PgPool,
        cbu_id: Uuid,
    ) -> Result<Vec<DetectedPattern>, sqlx::Error> {
        // Check limited companies
        let opacity_entities: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, lc.jurisdiction
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            WHERE cer.cbu_id = $1
            AND lc.jurisdiction = ANY($2)
            "#,
        )
        .bind(cbu_id)
        .bind(
            self.config
                .opacity_jurisdictions
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
        )
        .fetch_all(pool)
        .await?;

        let patterns: Vec<DetectedPattern> = opacity_entities
            .into_iter()
            .map(|(entity_id, jurisdiction)| DetectedPattern {
                pattern_type: PatternType::OpacityJurisdiction,
                severity: PatternSeverity::High,
                description: format!(
                    "Entity registered in opacity jurisdiction: {}",
                    jurisdiction
                ),
                involved_entities: vec![entity_id],
                evidence: PatternEvidence::OpacityJurisdiction {
                    entity_id,
                    jurisdiction: jurisdiction.clone(),
                    risk_tier: "OPACITY".to_string(),
                },
                detected_at: Utc::now(),
            })
            .collect();

        Ok(patterns)
    }

    /// Detect ownership gaps (percentages don't sum to 100%)
    #[cfg(feature = "database")]
    pub async fn detect_ownership_gaps(
        &self,
        pool: &PgPool,
        cbu_id: Uuid,
    ) -> Result<Vec<DetectedPattern>, sqlx::Error> {
        // Sum ownership percentages per entity
        let ownership_sums: Vec<(Uuid, f64)> = sqlx::query_as(
            r#"
            SELECT
                r.to_entity_id,
                COALESCE(SUM(r.percentage), 0)::float8 as total_ownership
            FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = r.to_entity_id
            WHERE cer.cbu_id = $1
            AND r.relationship_type = 'ownership'
            AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
            GROUP BY r.to_entity_id
            HAVING ABS(100 - COALESCE(SUM(r.percentage), 0)) > $2
            "#,
        )
        .bind(cbu_id)
        .bind(self.config.ownership_gap_threshold)
        .fetch_all(pool)
        .await?;

        let patterns: Vec<DetectedPattern> = ownership_sums
            .into_iter()
            .map(|(entity_id, total)| {
                let gap = (100.0 - total).abs();
                DetectedPattern {
                    pattern_type: PatternType::OwnershipGaps,
                    severity: if gap > 25.0 {
                        PatternSeverity::High
                    } else if gap > 10.0 {
                        PatternSeverity::Medium
                    } else {
                        PatternSeverity::Low
                    },
                    description: format!(
                        "Ownership gap detected: {}% accounted for ({}% gap)",
                        total, gap
                    ),
                    involved_entities: vec![entity_id],
                    evidence: PatternEvidence::OwnershipGap {
                        entity_id,
                        total_percentage: total,
                        gap,
                    },
                    detected_at: Utc::now(),
                }
            })
            .collect();

        Ok(patterns)
    }
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Non-database implementations for testing
// ============================================================================

#[cfg(not(feature = "database"))]
impl PatternDetector {
    /// Detect all patterns (stub for non-database builds)
    pub fn detect_all_sync(&self, _entities: &[Uuid]) -> Vec<DetectedPattern> {
        vec![]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_severity_from_type() {
        assert_eq!(
            PatternSeverity::from_pattern_type(PatternType::CircularOwnership),
            PatternSeverity::Critical
        );
        assert_eq!(
            PatternSeverity::from_pattern_type(PatternType::NomineeUsage),
            PatternSeverity::Medium
        );
    }

    #[test]
    fn test_default_config() {
        let config = PatternDetectorConfig::default();
        assert!(config.opacity_jurisdictions.contains("VG"));
        assert!(config.opacity_jurisdictions.contains("KY"));
        assert_eq!(config.layering_threshold, 5);
    }

    #[test]
    fn test_cycle_detection() {
        let detector = PatternDetector::new();
        let mut graph: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        // Create cycle: A → B → C → A
        graph.insert(a, vec![b]);
        graph.insert(b, vec![c]);
        graph.insert(c, vec![a]);

        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = vec![];

        let cycle = detector.find_cycle(&graph, a, &mut visited, &mut rec_stack, &mut path);

        assert!(cycle.is_some());
        let cycle = cycle.unwrap();
        assert!(cycle.len() >= 3);
    }

    #[test]
    fn test_no_cycle() {
        let detector = PatternDetector::new();
        let mut graph: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        // Linear chain: A → B → C (no cycle)
        graph.insert(a, vec![b]);
        graph.insert(b, vec![c]);
        graph.insert(c, vec![]);

        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = vec![];

        let cycle = detector.find_cycle(&graph, a, &mut visited, &mut rec_stack, &mut path);

        assert!(cycle.is_none());
    }
}
