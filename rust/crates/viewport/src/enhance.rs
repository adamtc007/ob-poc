//! Enhanceable trait implementations for different entity types
//!
//! Each entity type has different maximum enhance levels and available operations
//! at each level. This module provides concrete implementations.

use ob_poc_types::viewport::{EnhanceOp, Enhanceable};

/// CBU container enhanceable implementation
///
/// | Level | Description |
/// |-------|-------------|
/// | L0 | Collapsed badge with jurisdiction flag |
/// | L1 | Category counts visible |
/// | L2 | Entity nodes and relationships visible |
#[derive(Debug, Clone)]
pub struct CbuEnhanceable {
    enhance_level: u8,
}

impl CbuEnhanceable {
    pub fn new(level: u8) -> Self {
        Self {
            enhance_level: level.min(2),
        }
    }
}

impl Enhanceable for CbuEnhanceable {
    fn enhance_level(&self) -> u8 {
        self.enhance_level
    }

    fn max_enhance_level(&self) -> u8 {
        2
    }

    fn available_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![],
            1 => vec![EnhanceOp::ShowAttributes {
                keys: vec![
                    "entity_count".to_string(),
                    "product_count".to_string(),
                    "service_count".to_string(),
                ],
            }],
            2 => vec![
                EnhanceOp::ShowAttributes {
                    keys: vec![
                        "entity_count".to_string(),
                        "product_count".to_string(),
                        "service_count".to_string(),
                    ],
                },
                EnhanceOp::ExpandRelationships {
                    depth: 1,
                    rel_types: None,
                },
                EnhanceOp::ShowConfidenceScores,
            ],
            _ => vec![],
        }
    }

    fn next_level_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![EnhanceOp::ShowAttributes {
                keys: vec!["entity_count".to_string(), "product_count".to_string()],
            }],
            1 => vec![
                EnhanceOp::ExpandRelationships {
                    depth: 1,
                    rel_types: None,
                },
                EnhanceOp::ShowConfidenceScores,
            ],
            _ => vec![],
        }
    }

    fn level_description(&self) -> &'static str {
        match self.enhance_level {
            0 => "Collapsed - Badge with jurisdiction flag",
            1 => "Summary - Category counts visible",
            2 => "Expanded - Entity nodes and relationships",
            _ => "Unknown",
        }
    }
}

/// Concrete entity (Company, Partnership, Trust, Person) enhanceable
///
/// | Level | Description |
/// |-------|-------------|
/// | L0 | Name and type badge |
/// | L1 | Jurisdiction, status |
/// | L2 | 1-hop relationships |
/// | L3 | Key attributes |
/// | L4 | Full attributes with evidence |
#[derive(Debug, Clone)]
pub struct ConcreteEntityEnhanceable {
    enhance_level: u8,
}

impl ConcreteEntityEnhanceable {
    pub fn new(level: u8) -> Self {
        Self {
            enhance_level: level.min(4),
        }
    }
}

impl Enhanceable for ConcreteEntityEnhanceable {
    fn enhance_level(&self) -> u8 {
        self.enhance_level
    }

    fn max_enhance_level(&self) -> u8 {
        4
    }

    fn available_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![],
            1 => vec![EnhanceOp::ShowAttributes {
                keys: vec!["jurisdiction".to_string(), "status".to_string()],
            }],
            2 => vec![
                EnhanceOp::ShowAttributes {
                    keys: vec!["jurisdiction".to_string(), "status".to_string()],
                },
                EnhanceOp::ExpandRelationships {
                    depth: 1,
                    rel_types: None,
                },
            ],
            3 => vec![
                EnhanceOp::ShowAttributes {
                    keys: vec![
                        "jurisdiction".to_string(),
                        "status".to_string(),
                        "registration_number".to_string(),
                        "incorporation_date".to_string(),
                    ],
                },
                EnhanceOp::ExpandRelationships {
                    depth: 1,
                    rel_types: None,
                },
                EnhanceOp::ShowConfidenceScores,
            ],
            4 => vec![
                EnhanceOp::ShowAttributes {
                    keys: vec![
                        "jurisdiction".to_string(),
                        "status".to_string(),
                        "registration_number".to_string(),
                        "incorporation_date".to_string(),
                        "registered_address".to_string(),
                        "business_nature".to_string(),
                    ],
                },
                EnhanceOp::ExpandRelationships {
                    depth: 2,
                    rel_types: None,
                },
                EnhanceOp::ShowConfidenceScores,
                EnhanceOp::ShowTemporalHistory,
                EnhanceOp::ShowEvidencePanel,
            ],
            _ => vec![],
        }
    }

    fn next_level_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![EnhanceOp::ShowAttributes {
                keys: vec!["jurisdiction".to_string(), "status".to_string()],
            }],
            1 => vec![EnhanceOp::ExpandRelationships {
                depth: 1,
                rel_types: None,
            }],
            2 => vec![
                EnhanceOp::ShowAttributes {
                    keys: vec![
                        "registration_number".to_string(),
                        "incorporation_date".to_string(),
                    ],
                },
                EnhanceOp::ShowConfidenceScores,
            ],
            3 => vec![
                EnhanceOp::ShowTemporalHistory,
                EnhanceOp::ShowEvidencePanel,
                EnhanceOp::ExpandRelationships {
                    depth: 2,
                    rel_types: None,
                },
            ],
            _ => vec![],
        }
    }

    fn level_description(&self) -> &'static str {
        match self.enhance_level {
            0 => "Minimal - Name and type badge",
            1 => "Basic - Jurisdiction and status",
            2 => "Connected - 1-hop relationships",
            3 => "Detailed - Key attributes visible",
            4 => "Full - All attributes with evidence",
            _ => "Unknown",
        }
    }
}

/// Instrument matrix enhanceable
///
/// | Level | Description |
/// |-------|-------------|
/// | L0 | Collapsed badge |
/// | L1 | Type node grid |
/// | L2 | Type counts and status |
#[derive(Debug, Clone)]
pub struct InstrumentMatrixEnhanceable {
    enhance_level: u8,
}

impl InstrumentMatrixEnhanceable {
    pub fn new(level: u8) -> Self {
        Self {
            enhance_level: level.min(2),
        }
    }
}

impl Enhanceable for InstrumentMatrixEnhanceable {
    fn enhance_level(&self) -> u8 {
        self.enhance_level
    }

    fn max_enhance_level(&self) -> u8 {
        2
    }

    fn available_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![],
            1 => vec![EnhanceOp::ExpandCluster],
            2 => vec![
                EnhanceOp::ExpandCluster,
                EnhanceOp::ShowAttributes {
                    keys: vec![
                        "instrument_type_count".to_string(),
                        "enabled_count".to_string(),
                        "restriction_count".to_string(),
                    ],
                },
            ],
            _ => vec![],
        }
    }

    fn next_level_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![EnhanceOp::ExpandCluster],
            1 => vec![EnhanceOp::ShowAttributes {
                keys: vec![
                    "instrument_type_count".to_string(),
                    "enabled_count".to_string(),
                ],
            }],
            _ => vec![],
        }
    }

    fn level_description(&self) -> &'static str {
        match self.enhance_level {
            0 => "Collapsed - Matrix badge",
            1 => "Grid - Instrument type nodes",
            2 => "Detailed - Type counts and status",
            _ => "Unknown",
        }
    }
}

/// Instrument type node enhanceable
///
/// | Level | Description |
/// |-------|-------------|
/// | L0 | Type badge |
/// | L1 | MIC/BIC/Pricing panels collapsed |
/// | L2 | Panels expanded |
/// | L3 | Full config details |
#[derive(Debug, Clone)]
pub struct InstrumentTypeEnhanceable {
    enhance_level: u8,
}

impl InstrumentTypeEnhanceable {
    pub fn new(level: u8) -> Self {
        Self {
            enhance_level: level.min(3),
        }
    }
}

impl Enhanceable for InstrumentTypeEnhanceable {
    fn enhance_level(&self) -> u8 {
        self.enhance_level
    }

    fn max_enhance_level(&self) -> u8 {
        3
    }

    fn available_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![],
            1 => vec![
                EnhanceOp::ShowMicPreferences,
                EnhanceOp::ShowBicRouting,
                EnhanceOp::ShowPricingConfig,
            ],
            2 => vec![
                EnhanceOp::ShowMicPreferences,
                EnhanceOp::ShowBicRouting,
                EnhanceOp::ShowPricingConfig,
                EnhanceOp::ShowRestrictions,
            ],
            3 => vec![
                EnhanceOp::ShowMicPreferences,
                EnhanceOp::ShowBicRouting,
                EnhanceOp::ShowPricingConfig,
                EnhanceOp::ShowRestrictions,
                EnhanceOp::ShowEvidencePanel,
            ],
            _ => vec![],
        }
    }

    fn next_level_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![
                EnhanceOp::ShowMicPreferences,
                EnhanceOp::ShowBicRouting,
                EnhanceOp::ShowPricingConfig,
            ],
            1 => vec![EnhanceOp::ShowRestrictions],
            2 => vec![EnhanceOp::ShowEvidencePanel],
            _ => vec![],
        }
    }

    fn level_description(&self) -> &'static str {
        match self.enhance_level {
            0 => "Badge - Type indicator only",
            1 => "Panels - MIC/BIC/Pricing collapsed",
            2 => "Expanded - Panels with details",
            3 => "Full - Complete config with evidence",
            _ => "Unknown",
        }
    }
}

/// Config node (MIC, BIC, Pricing) enhanceable
///
/// | Level | Description |
/// |-------|-------------|
/// | L0 | Summary line |
/// | L1 | Full detail |
/// | L2 | Detail with evidence |
#[derive(Debug, Clone)]
pub struct ConfigNodeEnhanceable {
    enhance_level: u8,
}

impl ConfigNodeEnhanceable {
    pub fn new(level: u8) -> Self {
        Self {
            enhance_level: level.min(2),
        }
    }
}

impl Enhanceable for ConfigNodeEnhanceable {
    fn enhance_level(&self) -> u8 {
        self.enhance_level
    }

    fn max_enhance_level(&self) -> u8 {
        2
    }

    fn available_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![],
            1 => vec![EnhanceOp::ShowAttributes {
                keys: vec![
                    "code".to_string(),
                    "priority".to_string(),
                    "enabled".to_string(),
                    "conditions".to_string(),
                ],
            }],
            2 => vec![
                EnhanceOp::ShowAttributes {
                    keys: vec![
                        "code".to_string(),
                        "priority".to_string(),
                        "enabled".to_string(),
                        "conditions".to_string(),
                    ],
                },
                EnhanceOp::ShowEvidencePanel,
            ],
            _ => vec![],
        }
    }

    fn next_level_ops(&self) -> Vec<EnhanceOp> {
        match self.enhance_level {
            0 => vec![EnhanceOp::ShowAttributes {
                keys: vec!["code".to_string(), "priority".to_string()],
            }],
            1 => vec![EnhanceOp::ShowEvidencePanel],
            _ => vec![],
        }
    }

    fn level_description(&self) -> &'static str {
        match self.enhance_level {
            0 => "Summary - Single line",
            1 => "Detail - Full configuration",
            2 => "Evidence - Config with proof",
            _ => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbu_enhance_levels() {
        let cbu = CbuEnhanceable::new(0);
        assert_eq!(cbu.max_enhance_level(), 2);
        assert!(cbu.can_enhance());
        assert!(!cbu.can_reduce());

        let cbu_max = CbuEnhanceable::new(2);
        assert!(!cbu_max.can_enhance());
        assert!(cbu_max.can_reduce());
    }

    #[test]
    fn test_entity_enhance_levels() {
        let entity = ConcreteEntityEnhanceable::new(2);
        assert_eq!(entity.max_enhance_level(), 4);
        assert!(entity.can_enhance());
        assert!(entity.can_reduce());

        let ops = entity.available_ops();
        assert!(ops
            .iter()
            .any(|op| matches!(op, EnhanceOp::ExpandRelationships { .. })));
    }

    #[test]
    fn test_instrument_type_enhance() {
        let itype = InstrumentTypeEnhanceable::new(1);
        assert_eq!(
            itype.level_description(),
            "Panels - MIC/BIC/Pricing collapsed"
        );

        let ops = itype.available_ops();
        assert!(ops
            .iter()
            .any(|op| matches!(op, EnhanceOp::ShowMicPreferences)));
    }

    #[test]
    fn test_level_clamping() {
        // Should clamp to max
        let cbu = CbuEnhanceable::new(10);
        assert_eq!(cbu.enhance_level(), 2);

        let entity = ConcreteEntityEnhanceable::new(100);
        assert_eq!(entity.enhance_level(), 4);
    }
}
