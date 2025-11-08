//! Domain-Specific Visualization Features (Phase 3)
//!
//! This module provides domain-aware visualization capabilities that understand
//! the specific semantics and workflows of different DSL domains like KYC,
//! Onboarding, and Account Opening.
//!
//! Features:
//! - Domain-specific node styling and layout
//! - Functional state progression visualization
//! - KYC/UBO workflow highlighting
//! - Entity relationship emphasis
//! - Compliance workflow tracking

use crate::dsl_manager_v2::{ASTVisualization, VisualEdge, VisualNode, VisualizationOptions};
use crate::models::{DslDomain, DslVersion};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Domain-specific visualization enhancer
pub struct DomainVisualizer {
    pub domain_rules: HashMap<String, DomainVisualizationRules>,
}

impl DomainVisualizer {
    /// Create a new domain visualizer with built-in domain rules
    pub fn new() -> Self {
        let mut domain_rules = HashMap::new();

        // KYC Domain Rules
        domain_rules.insert("KYC".to_string(), create_kyc_visualization_rules());
        domain_rules.insert("KYC_UBO".to_string(), create_kyc_ubo_visualization_rules());

        // Onboarding Domain Rules
        domain_rules.insert(
            "Onboarding".to_string(),
            create_onboarding_visualization_rules(),
        );

        // Account Opening Domain Rules
        domain_rules.insert(
            "Account_Opening".to_string(),
            create_account_opening_visualization_rules(),
        );

        // Compliance Domain Rules
        domain_rules.insert(
            "Compliance".to_string(),
            create_compliance_visualization_rules(),
        );

        Self { domain_rules }
    }

    /// Enhance visualization with domain-specific features
    pub fn enhance_visualization(
        &self,
        mut visualization: ASTVisualization,
        domain: &DslDomain,
        version: &DslVersion,
        options: &VisualizationOptions,
    ) -> DomainEnhancedVisualization {
        let domain_rules = self
            .domain_rules
            .get(&domain.domain_name)
            .cloned()
            .unwrap_or_default();

        // Apply domain-specific styling
        let enhanced_nodes = self.apply_domain_styling(&visualization.root_node, &domain_rules);
        let enhanced_edges = self.apply_edge_styling(&visualization.edges, &domain_rules);

        // Add functional state visualization if requested
        let functional_state_info = if options.show_functional_states {
            Some(self.create_functional_state_visualization(domain, version, &domain_rules))
        } else {
            None
        };

        // Calculate domain-specific metrics
        let domain_metrics = self.calculate_domain_metrics(&visualization, &domain_rules);

        // Create workflow progression if applicable
        let workflow_progression = self.create_workflow_progression(version, &domain_rules);

        DomainEnhancedVisualization {
            base_visualization: visualization,
            domain_rules,
            enhanced_root_node: enhanced_nodes,
            enhanced_edges,
            functional_state_info,
            domain_metrics,
            workflow_progression,
            domain_specific_highlights: self.identify_domain_highlights(&domain.domain_name),
        }
    }

    /// Apply domain-specific styling to nodes
    fn apply_domain_styling(
        &self,
        root_node: &VisualNode,
        rules: &DomainVisualizationRules,
    ) -> EnhancedVisualNode {
        let mut enhanced = EnhancedVisualNode::from_visual_node(root_node);

        // Apply domain-specific colors and styles
        if let Some(style) = rules.node_styles.get(&root_node.node_type) {
            enhanced.domain_style = Some(style.clone());
        }

        // Apply priority-based styling
        enhanced.priority_level = self.calculate_node_priority(&root_node.node_type, rules);

        // Add domain-specific annotations
        enhanced.domain_annotations = self.generate_node_annotations(root_node, rules);

        enhanced
    }

    /// Apply domain-specific edge styling
    fn apply_edge_styling(
        &self,
        edges: &[VisualEdge],
        rules: &DomainVisualizationRules,
    ) -> Vec<EnhancedVisualEdge> {
        edges
            .iter()
            .map(|edge| {
                let mut enhanced = EnhancedVisualEdge::from_visual_edge(edge);

                // Apply edge-specific styling
                if let Some(style) = rules.edge_styles.get(&edge.edge_type) {
                    enhanced.domain_style = Some(style.clone());
                }

                // Identify critical paths
                enhanced.is_critical_path = rules.critical_edge_types.contains(&edge.edge_type);

                enhanced
            })
            .collect()
    }

    /// Create functional state visualization
    fn create_functional_state_visualization(
        &self,
        domain: &DslDomain,
        version: &DslVersion,
        rules: &DomainVisualizationRules,
    ) -> FunctionalStateVisualization {
        let current_state = version.functional_state.clone().unwrap_or_default();
        let available_states = &rules.functional_states;

        // Find current state position in the workflow
        let current_position = available_states
            .iter()
            .position(|state| state.name == current_state)
            .unwrap_or(0);

        // Create state progression
        let state_progression: Vec<StateProgressionStep> = available_states
            .iter()
            .enumerate()
            .map(|(index, state)| StateProgressionStep {
                state_name: state.name.clone(),
                description: state.description.clone(),
                is_current: index == current_position,
                is_completed: index < current_position,
                is_available: index <= current_position + 1,
                estimated_effort: state.estimated_effort,
                dependencies: state.dependencies.clone(),
            })
            .collect();

        FunctionalStateVisualization {
            domain_name: domain.domain_name.clone(),
            current_state,
            available_states: available_states.clone(),
            state_progression,
            completion_percentage: self
                .calculate_completion_percentage(current_position, &available_states),
            next_possible_states: self.identify_next_states(current_position, &available_states),
        }
    }

    /// Calculate domain-specific metrics
    fn calculate_domain_metrics(
        &self,
        visualization: &ASTVisualization,
        rules: &DomainVisualizationRules,
    ) -> DomainMetrics {
        let entity_count = 0;
        let relationship_count = 0;
        let compliance_operations = 0;
        let ubo_calculations = 0;
        let document_collections = 0;

        // Count domain-specific operations (would need to traverse actual nodes)
        // This is a simplified version - in practice would analyze the full node tree

        DomainMetrics {
            entity_count,
            relationship_count,
            compliance_operations,
            ubo_calculations,
            document_collections,
            complexity_score: visualization.statistics.complexity_score,
            estimated_execution_time: self.estimate_execution_time(rules),
            risk_score: self.calculate_risk_score(&visualization.statistics),
        }
    }

    /// Create workflow progression visualization
    fn create_workflow_progression(
        &self,
        version: &DslVersion,
        rules: &DomainVisualizationRules,
    ) -> Option<WorkflowProgression> {
        if let Some(ref current_state) = version.functional_state {
            let workflow_steps =
                self.map_functional_states_to_workflow_steps(&rules.functional_states);

            Some(WorkflowProgression {
                current_step: current_state.clone(),
                workflow_steps,
                completion_status: self.calculate_workflow_completion_status(current_state, rules),
                blocked_steps: self.identify_blocked_steps(current_state, rules),
                recommended_next_actions: self.suggest_next_actions(current_state, rules),
            })
        } else {
            None
        }
    }

    /// Identify domain-specific highlights
    pub fn identify_domain_highlights(&self, domain_name: &str) -> Vec<DomainHighlight> {
        match domain_name {
            "KYC" => vec![
                DomainHighlight {
                    highlight_type: "UBO_CALCULATION".to_string(),
                    description: "Ultimate Beneficial Ownership calculation flows".to_string(),
                    color: "#FF6B35".to_string(),
                    priority: HighlightPriority::High,
                },
                DomainHighlight {
                    highlight_type: "ENTITY_RELATIONSHIPS".to_string(),
                    description: "Entity relationship mapping and verification".to_string(),
                    color: "#4ECDC4".to_string(),
                    priority: HighlightPriority::Medium,
                },
                DomainHighlight {
                    highlight_type: "COMPLIANCE_CHECKS".to_string(),
                    description: "Regulatory compliance verification steps".to_string(),
                    color: "#45B7D1".to_string(),
                    priority: HighlightPriority::High,
                },
            ],
            "Onboarding" => vec![
                DomainHighlight {
                    highlight_type: "WORKFLOW_PROGRESSION".to_string(),
                    description: "Customer onboarding workflow stages".to_string(),
                    color: "#96CEB4".to_string(),
                    priority: HighlightPriority::High,
                },
                DomainHighlight {
                    highlight_type: "DECISION_POINTS".to_string(),
                    description: "Critical decision and approval points".to_string(),
                    color: "#FFEAA7".to_string(),
                    priority: HighlightPriority::Medium,
                },
            ],
            "Account_Opening" => vec![
                DomainHighlight {
                    highlight_type: "REQUIREMENT_VALIDATION".to_string(),
                    description: "Account opening requirement validation".to_string(),
                    color: "#DDA0DD".to_string(),
                    priority: HighlightPriority::High,
                },
                DomainHighlight {
                    highlight_type: "APPROVAL_WORKFLOW".to_string(),
                    description: "Account approval workflow and checks".to_string(),
                    color: "#98D8C8".to_string(),
                    priority: HighlightPriority::Medium,
                },
            ],
            _ => vec![],
        }
    }

    // Helper methods

    fn calculate_node_priority(&self, node_type: &str, rules: &DomainVisualizationRules) -> u32 {
        rules.priority_mapping.get(node_type).copied().unwrap_or(1)
    }

    fn generate_node_annotations(
        &self,
        node: &VisualNode,
        rules: &DomainVisualizationRules,
    ) -> Vec<String> {
        let mut annotations = Vec::new();

        // Add domain-specific annotations based on node type
        match node.node_type.as_str() {
            "CalculateUbo" => annotations.push("UBO Calculation - Critical Path".to_string()),
            "DeclareEntity" => annotations.push("Entity Declaration".to_string()),
            "CreateEdge" => annotations.push("Relationship Creation".to_string()),
            "ObtainDocument" => annotations.push("Document Collection".to_string()),
            _ => {}
        }

        annotations
    }

    fn calculate_completion_percentage(
        &self,
        current_position: usize,
        states: &[FunctionalState],
    ) -> f64 {
        if states.is_empty() {
            return 0.0;
        }
        (current_position as f64 / states.len() as f64) * 100.0
    }

    fn identify_next_states(
        &self,
        current_position: usize,
        states: &[FunctionalState],
    ) -> Vec<String> {
        if current_position + 1 < states.len() {
            vec![states[current_position + 1].name.clone()]
        } else {
            vec![]
        }
    }

    fn estimate_execution_time(&self, rules: &DomainVisualizationRules) -> u32 {
        // Simplified estimation based on domain complexity
        rules.base_execution_time_ms
    }

    fn calculate_risk_score(&self, statistics: &crate::dsl_manager_v2::ASTStatistics) -> f64 {
        // Simple risk calculation based on complexity
        let complexity_factor = statistics
            .complexity_score
            .to_string()
            .parse::<f64>()
            .unwrap_or(1.0);
        let node_factor = statistics.total_nodes as f64;

        (complexity_factor + node_factor) / 20.0 // Normalized risk score
    }

    fn map_functional_states_to_workflow_steps(
        &self,
        states: &[FunctionalState],
    ) -> Vec<WorkflowStep> {
        states
            .iter()
            .map(|state| WorkflowStep {
                step_id: state.name.clone(),
                step_name: state.description.clone(),
                estimated_duration: state.estimated_effort,
                dependencies: state.dependencies.clone(),
                is_automated: state.name.contains("Calculate") || state.name.contains("Generate"),
                requires_approval: state.name.contains("Review") || state.name.contains("Approve"),
            })
            .collect()
    }

    fn calculate_workflow_completion_status(
        &self,
        current_state: &str,
        _rules: &DomainVisualizationRules,
    ) -> String {
        match current_state {
            s if s.contains("Create") => "Initiated".to_string(),
            s if s.contains("Review") => "In Review".to_string(),
            s if s.contains("Approve") => "Pending Approval".to_string(),
            s if s.contains("Complete") => "Completed".to_string(),
            _ => "In Progress".to_string(),
        }
    }

    fn identify_blocked_steps(
        &self,
        _current_state: &str,
        _rules: &DomainVisualizationRules,
    ) -> Vec<String> {
        // Identify any steps that might be blocked based on current state
        vec![] // Simplified for now
    }

    fn suggest_next_actions(
        &self,
        current_state: &str,
        _rules: &DomainVisualizationRules,
    ) -> Vec<String> {
        match current_state {
            "Create_Case" => vec![
                "Collect required documents".to_string(),
                "Verify entity information".to_string(),
            ],
            "Generate_UBO" => vec![
                "Review UBO calculation results".to_string(),
                "Verify ownership percentages".to_string(),
            ],
            "Review_Edit" => vec![
                "Complete review process".to_string(),
                "Submit for final approval".to_string(),
            ],
            _ => vec!["Continue with next workflow step".to_string()],
        }
    }
}

impl Default for DomainVisualizer {
    fn default() -> Self {
        Self::new()
    }
}

// Domain-specific types and structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEnhancedVisualization {
    pub base_visualization: ASTVisualization,
    pub domain_rules: DomainVisualizationRules,
    pub enhanced_root_node: EnhancedVisualNode,
    pub enhanced_edges: Vec<EnhancedVisualEdge>,
    pub functional_state_info: Option<FunctionalStateVisualization>,
    pub domain_metrics: DomainMetrics,
    pub workflow_progression: Option<WorkflowProgression>,
    pub domain_specific_highlights: Vec<DomainHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainVisualizationRules {
    pub domain_name: String,
    pub node_styles: HashMap<String, NodeStyle>,
    pub edge_styles: HashMap<String, EdgeStyle>,
    pub functional_states: Vec<FunctionalState>,
    pub critical_edge_types: Vec<String>,
    pub priority_mapping: HashMap<String, u32>,
    pub base_execution_time_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStyle {
    pub color: String,
    pub border_color: String,
    pub border_width: u32,
    pub font_color: String,
    pub font_size: u32,
    pub shape: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeStyle {
    pub color: String,
    pub width: u32,
    pub style: String, // "solid", "dashed", "dotted"
    pub arrow_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionalState {
    pub name: String,
    pub description: String,
    pub estimated_effort: u32, // in minutes
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedVisualNode {
    pub base_node: VisualNode,
    pub domain_style: Option<NodeStyle>,
    pub priority_level: u32,
    pub domain_annotations: Vec<String>,
    pub functional_relevance: Option<String>,
}

impl EnhancedVisualNode {
    pub fn from_visual_node(node: &VisualNode) -> Self {
        Self {
            base_node: node.clone(),
            domain_style: None,
            priority_level: 1,
            domain_annotations: vec![],
            functional_relevance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedVisualEdge {
    pub base_edge: VisualEdge,
    pub domain_style: Option<EdgeStyle>,
    pub is_critical_path: bool,
    pub relationship_strength: f64,
}

impl EnhancedVisualEdge {
    pub fn from_visual_edge(edge: &VisualEdge) -> Self {
        Self {
            base_edge: edge.clone(),
            domain_style: None,
            is_critical_path: false,
            relationship_strength: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionalStateVisualization {
    pub domain_name: String,
    pub current_state: String,
    pub available_states: Vec<FunctionalState>,
    pub state_progression: Vec<StateProgressionStep>,
    pub completion_percentage: f64,
    pub next_possible_states: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateProgressionStep {
    pub state_name: String,
    pub description: String,
    pub is_current: bool,
    pub is_completed: bool,
    pub is_available: bool,
    pub estimated_effort: u32,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMetrics {
    pub entity_count: u32,
    pub relationship_count: u32,
    pub compliance_operations: u32,
    pub ubo_calculations: u32,
    pub document_collections: u32,
    pub complexity_score: rust_decimal::Decimal,
    pub estimated_execution_time: u32,
    pub risk_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgression {
    pub current_step: String,
    pub workflow_steps: Vec<WorkflowStep>,
    pub completion_status: String,
    pub blocked_steps: Vec<String>,
    pub recommended_next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub step_id: String,
    pub step_name: String,
    pub estimated_duration: u32,
    pub dependencies: Vec<String>,
    pub is_automated: bool,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainHighlight {
    pub highlight_type: String,
    pub description: String,
    pub color: String,
    pub priority: HighlightPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HighlightPriority {
    Low,
    Medium,
    High,
    Critical,
}

// Domain-specific rule creation functions

fn create_kyc_visualization_rules() -> DomainVisualizationRules {
    let mut node_styles = HashMap::new();
    let mut edge_styles = HashMap::new();
    let mut priority_mapping = HashMap::new();

    // KYC-specific node styling
    node_styles.insert(
        "CalculateUbo".to_string(),
        NodeStyle {
            color: "#FF6B35".to_string(),
            border_color: "#E55A31".to_string(),
            border_width: 3,
            font_color: "#FFFFFF".to_string(),
            font_size: 14,
            shape: "diamond".to_string(),
            icon: Some("calculate".to_string()),
        },
    );

    node_styles.insert(
        "DeclareEntity".to_string(),
        NodeStyle {
            color: "#4ECDC4".to_string(),
            border_color: "#45B7B8".to_string(),
            border_width: 2,
            font_color: "#2C3E50".to_string(),
            font_size: 12,
            shape: "rectangle".to_string(),
            icon: Some("entity".to_string()),
        },
    );

    node_styles.insert(
        "CreateEdge".to_string(),
        NodeStyle {
            color: "#45B7D1".to_string(),
            border_color: "#3867D6".to_string(),
            border_width: 2,
            font_color: "#FFFFFF".to_string(),
            font_size: 11,
            shape: "ellipse".to_string(),
            icon: Some("relationship".to_string()),
        },
    );

    // KYC-specific edge styling
    edge_styles.insert(
        "beneficial-owner".to_string(),
        EdgeStyle {
            color: "#E74C3C".to_string(),
            width: 3,
            style: "solid".to_string(),
            arrow_style: "vee".to_string(),
        },
    );

    edge_styles.insert(
        "control".to_string(),
        EdgeStyle {
            color: "#9B59B6".to_string(),
            width: 2,
            style: "dashed".to_string(),
            arrow_style: "diamond".to_string(),
        },
    );

    // Priority mapping for KYC domain
    priority_mapping.insert("CalculateUbo".to_string(), 10);
    priority_mapping.insert("DeclareEntity".to_string(), 8);
    priority_mapping.insert("CreateEdge".to_string(), 7);
    priority_mapping.insert("ObtainDocument".to_string(), 6);

    // KYC functional states
    let functional_states = vec![
        FunctionalState {
            name: "Create_Case".to_string(),
            description: "Initialize KYC investigation case".to_string(),
            estimated_effort: 30,
            dependencies: vec![],
        },
        FunctionalState {
            name: "Collect_Documents".to_string(),
            description: "Gather required documentation".to_string(),
            estimated_effort: 120,
            dependencies: vec!["Create_Case".to_string()],
        },
        FunctionalState {
            name: "Verify_Entities".to_string(),
            description: "Verify entity information and relationships".to_string(),
            estimated_effort: 90,
            dependencies: vec!["Collect_Documents".to_string()],
        },
        FunctionalState {
            name: "Generate_UBO".to_string(),
            description: "Calculate Ultimate Beneficial Ownership".to_string(),
            estimated_effort: 45,
            dependencies: vec!["Verify_Entities".to_string()],
        },
        FunctionalState {
            name: "Review_Edit".to_string(),
            description: "Review and edit results".to_string(),
            estimated_effort: 60,
            dependencies: vec!["Generate_UBO".to_string()],
        },
        FunctionalState {
            name: "Confirm_Compile".to_string(),
            description: "Confirm and compile final report".to_string(),
            estimated_effort: 30,
            dependencies: vec!["Review_Edit".to_string()],
        },
        FunctionalState {
            name: "Run".to_string(),
            description: "Execute final compliance checks".to_string(),
            estimated_effort: 15,
            dependencies: vec!["Confirm_Compile".to_string()],
        },
    ];

    DomainVisualizationRules {
        domain_name: "KYC".to_string(),
        node_styles,
        edge_styles,
        functional_states,
        critical_edge_types: vec!["beneficial-owner".to_string(), "control".to_string()],
        priority_mapping,
        base_execution_time_ms: 5000,
    }
}

fn create_kyc_ubo_visualization_rules() -> DomainVisualizationRules {
    let mut rules = create_kyc_visualization_rules();
    rules.domain_name = "KYC_UBO".to_string();

    // Enhanced UBO-specific styling
    rules.node_styles.insert(
        "CalculateUbo".to_string(),
        NodeStyle {
            color: "#FF4757".to_string(),
            border_color: "#FF3742".to_string(),
            border_width: 4,
            font_color: "#FFFFFF".to_string(),
            font_size: 16,
            shape: "star".to_string(),
            icon: Some("ubo_calculation".to_string()),
        },
    );

    rules.base_execution_time_ms = 7500;
    rules
}

fn create_onboarding_visualization_rules() -> DomainVisualizationRules {
    let mut node_styles = HashMap::new();
    let mut edge_styles = HashMap::new();
    let mut priority_mapping = HashMap::new();

    // Onboarding-specific node styling
    node_styles.insert(
        "Workflow".to_string(),
        NodeStyle {
            color: "#96CEB4".to_string(),
            border_color: "#6C7B7F".to_string(),
            border_width: 2,
            font_color: "#2C3E50".to_string(),
            font_size: 14,
            shape: "rectangle".to_string(),
            icon: Some("workflow".to_string()),
        },
    );

    edge_styles.insert(
        "next_step".to_string(),
        EdgeStyle {
            color: "#55A3FF".to_string(),
            width: 2,
            style: "solid".to_string(),
            arrow_style: "vee".to_string(),
        },
    );

    priority_mapping.insert("Workflow".to_string(), 9);
    priority_mapping.insert("Sequential".to_string(), 8);
    priority_mapping.insert("Parallel".to_string(), 7);

    let functional_states = vec![
        FunctionalState {
            name: "Initial_Contact".to_string(),
            description: "Customer initiates onboarding process".to_string(),
            estimated_effort: 15,
            dependencies: vec![],
        },
        FunctionalState {
            name: "Document_Collection".to_string(),
            description: "Collect required onboarding documents".to_string(),
            estimated_effort: 180,
            dependencies: vec!["Initial_Contact".to_string()],
        },
        FunctionalState {
            name: "Information_Verification".to_string(),
            description: "Verify provided information".to_string(),
            estimated_effort: 120,
            dependencies: vec!["Document_Collection".to_string()],
        },
        FunctionalState {
            name: "Risk_Assessment".to_string(),
            description: "Assess customer risk profile".to_string(),
            estimated_effort: 90,
            dependencies: vec!["Information_Verification".to_string()],
        },
        FunctionalState {
            name: "Final_Approval".to_string(),
            description: "Final approval and account activation".to_string(),
            estimated_effort: 45,
            dependencies: vec!["Risk_Assessment".to_string()],
        },
    ];

    DomainVisualizationRules {
        domain_name: "Onboarding".to_string(),
        node_styles,
        edge_styles,
        functional_states,
        critical_edge_types: vec!["approval_required".to_string()],
        priority_mapping,
        base_execution_time_ms: 3000,
    }
}

fn create_account_opening_visualization_rules() -> DomainVisualizationRules {
    let mut node_styles = HashMap::new();
    let mut edge_styles = HashMap::new();
    let mut priority_mapping = HashMap::new();

    node_styles.insert(
        "ValidateRequirements".to_string(),
        NodeStyle {
            color: "#DDA0DD".to_string(),
            border_color: "#BA68C8".to_string(),
            border_width: 2,
            font_color: "#4A148C".to_string(),
            font_size: 12,
            shape: "rectangle".to_string(),
            icon: Some("validate".to_string()),
        },
    );

    edge_styles.insert(
        "validation_flow".to_string(),
        EdgeStyle {
            color: "#8E24AA".to_string(),
            width: 2,
            style: "solid".to_string(),
            arrow_style: "vee".to_string(),
        },
    );

    priority_mapping.insert("ValidateRequirements".to_string(), 9);
    priority_mapping.insert("ApprovalWorkflow".to_string(), 8);

    let functional_states = vec![
        FunctionalState {
            name: "Requirements_Check".to_string(),
            description: "Check account opening requirements".to_string(),
            estimated_effort: 60,
            dependencies: vec![],
        },
        FunctionalState {
            name: "Documentation_Review".to_string(),
            description: "Review submitted documentation".to_string(),
            estimated_effort: 90,
            dependencies: vec!["Requirements_Check".to_string()],
        },
        FunctionalState {
            name: "Approval_Process".to_string(),
            description: "Process account opening approval".to_string(),
            estimated_effort: 120,
            dependencies: vec!["Documentation_Review".to_string()],
        },
        FunctionalState {
            name: "Account_Creation".to_string(),
            description: "Create and activate account".to_string(),
            estimated_effort: 30,
            dependencies: vec!["Approval_Process".to_string()],
        },
    ];

    DomainVisualizationRules {
        domain_name: "Account_Opening".to_string(),
        node_styles,
        edge_styles,
        functional_states,
        critical_edge_types: vec![
            "approval_required".to_string(),
            "validation_flow".to_string(),
        ],
        priority_mapping,
        base_execution_time_ms: 4000,
    }
}

fn create_compliance_visualization_rules() -> DomainVisualizationRules {
    let mut node_styles = HashMap::new();
    let mut edge_styles = HashMap::new();
    let mut priority_mapping = HashMap::new();

    node_styles.insert(
        "ComplianceCheck".to_string(),
        NodeStyle {
            color: "#FF9F43".to_string(),
            border_color: "#EE5A24".to_string(),
            border_width: 2,
            font_color: "#2F3640".to_string(),
            font_size: 12,
            shape: "hexagon".to_string(),
            icon: Some("compliance".to_string()),
        },
    );

    edge_styles.insert(
        "compliance_flow".to_string(),
        EdgeStyle {
            color: "#F39C12".to_string(),
            width: 2,
            style: "solid".to_string(),
            arrow_style: "vee".to_string(),
        },
    );

    priority_mapping.insert("ComplianceCheck".to_string(), 10);
    priority_mapping.insert("RiskAssessment".to_string(), 9);

    let functional_states = vec![
        FunctionalState {
            name: "Risk_Identification".to_string(),
            description: "Identify compliance risks".to_string(),
            estimated_effort: 45,
            dependencies: vec![],
        },
        FunctionalState {
            name: "Control_Assessment".to_string(),
            description: "Assess existing controls".to_string(),
            estimated_effort: 60,
            dependencies: vec!["Risk_Identification".to_string()],
        },
        FunctionalState {
            name: "Compliance_Testing".to_string(),
            description: "Test compliance controls".to_string(),
            estimated_effort: 90,
            dependencies: vec!["Control_Assessment".to_string()],
        },
        FunctionalState {
            name: "Remediation".to_string(),
            description: "Address compliance gaps".to_string(),
            estimated_effort: 120,
            dependencies: vec!["Compliance_Testing".to_string()],
        },
    ];

    DomainVisualizationRules {
        domain_name: "Compliance".to_string(),
        node_styles,
        edge_styles,
        functional_states,
        critical_edge_types: vec!["compliance_flow".to_string()],
        priority_mapping,
        base_execution_time_ms: 6000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_manager_v2::{
        ASTStatistics, CompilationInfo, LayoutType, VisualizationMetadata,
    };
    use crate::models::CompilationStatus;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn create_test_domain() -> DslDomain {
        DslDomain {
            domain_id: Uuid::new_v4(),
            domain_name: "KYC".to_string(),
            description: Some("Test KYC domain".to_string()),
            base_grammar_version: "1.0.0".to_string(),
            vocabulary_version: "1.0.0".to_string(),
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_version() -> DslVersion {
        DslVersion {
            version_id: Uuid::new_v4(),
            domain_id: Uuid::new_v4(),
            request_id: None,
            version_number: 1,
            functional_state: Some("Generate_UBO".to_string()),
            dsl_source_code: "test".to_string(),
            compilation_status: CompilationStatus::Compiled,
            change_description: Some("Test".to_string()),
            parent_version_id: None,
            created_by: Some("test_user".to_string()),
            created_at: Utc::now(),
            compiled_at: Some(Utc::now()),
            activated_at: None,
        }
    }

    fn create_test_visualization() -> ASTVisualization {
        use crate::dsl_manager_v2::{DomainContext, VisualNode};

        ASTVisualization {
            metadata: VisualizationMetadata {
                generated_at: Utc::now(),
                generator_version: "test".to_string(),
                layout_type: LayoutType::Tree,
            },
            domain_context: DomainContext {
                domain_name: "KYC".to_string(),
                version_number: 1,
                functional_state: Some("Generate_UBO".to_string()),
                grammar_version: "1.0.0".to_string(),
                compilation_status: CompilationStatus::Compiled,
            },
            root_node: VisualNode {
                id: "root".to_string(),
                node_type: "Program".to_string(),
                label: "Test Program".to_string(),
                children: vec![],
                properties: HashMap::new(),
            },
            edges: vec![],
            statistics: ASTStatistics {
                total_nodes: 5,
                total_edges: 3,
                max_depth: 2,
                complexity_score: Decimal::from(10),
            },
            compilation_info: CompilationInfo {
                parsed_at: Utc::now(),
                parser_version: "test".to_string(),
                grammar_version: "1.0.0".to_string(),
                parse_duration_ms: 100,
            },
        }
    }

    #[test]
    fn test_domain_visualizer_creation() {
        let visualizer = DomainVisualizer::new();
        assert!(visualizer.domain_rules.contains_key("KYC"));
        assert!(visualizer.domain_rules.contains_key("Onboarding"));
        assert!(visualizer.domain_rules.contains_key("Account_Opening"));
    }

    #[test]
    fn test_kyc_domain_rules() {
        let rules = create_kyc_visualization_rules();
        assert_eq!(rules.domain_name, "KYC");
        assert!(rules.node_styles.contains_key("CalculateUbo"));
        assert!(rules.edge_styles.contains_key("beneficial-owner"));
        assert_eq!(rules.functional_states.len(), 7);

        // Check functional state progression
        let create_case_state = rules
            .functional_states
            .iter()
            .find(|s| s.name == "Create_Case")
            .expect("Create_Case state should exist");
        assert!(create_case_state.dependencies.is_empty());

        let generate_ubo_state = rules
            .functional_states
            .iter()
            .find(|s| s.name == "Generate_UBO")
            .expect("Generate_UBO state should exist");
        assert!(generate_ubo_state
            .dependencies
            .contains(&"Verify_Entities".to_string()));
    }

    #[test]
    fn test_functional_state_visualization() {
        let visualizer = DomainVisualizer::new();
        let domain = create_test_domain();
        let version = create_test_version();
        let rules = create_kyc_visualization_rules();

        let functional_viz =
            visualizer.create_functional_state_visualization(&domain, &version, &rules);

        assert_eq!(functional_viz.domain_name, "KYC");
        assert_eq!(functional_viz.current_state, "Generate_UBO");
        assert!(functional_viz.completion_percentage > 0.0);
        assert!(!functional_viz.state_progression.is_empty());

        // Check that current state is marked correctly
        let current_step = functional_viz
            .state_progression
            .iter()
            .find(|step| step.state_name == "Generate_UBO")
            .expect("Current step should be found");
        assert!(current_step.is_current);
    }

    #[test]
    fn test_domain_enhancement() {
        let visualizer = DomainVisualizer::new();
        let domain = create_test_domain();
        let version = create_test_version();
        let visualization = create_test_visualization();

        let options = VisualizationOptions {
            show_functional_states: true,
            ..Default::default()
        };

        let enhanced = visualizer.enhance_visualization(visualization, &domain, &version, &options);

        assert_eq!(enhanced.domain_rules.domain_name, "KYC");
        assert!(enhanced.functional_state_info.is_some());
        assert!(!enhanced.domain_specific_highlights.is_empty());

        // Check KYC-specific highlights
        let ubo_highlight = enhanced
            .domain_specific_highlights
            .iter()
            .find(|h| h.highlight_type == "UBO_CALCULATION")
            .expect("UBO calculation highlight should exist");
        assert_eq!(ubo_highlight.color, "#FF6B35");
    }

    #[test]
    fn test_enhanced_visual_node() {
        let base_node = VisualNode {
            id: "test".to_string(),
            node_type: "CalculateUbo".to_string(),
            label: "Test UBO Calculation".to_string(),
            children: vec![],
            properties: HashMap::new(),
        };

        let enhanced = EnhancedVisualNode::from_visual_node(&base_node);
        assert_eq!(enhanced.base_node.id, "test");
        assert_eq!(enhanced.priority_level, 1);
        assert!(enhanced.domain_annotations.is_empty());
    }

    #[test]
    fn test_workflow_progression() {
        let visualizer = DomainVisualizer::new();
        let version = create_test_version();
        let rules = create_kyc_visualization_rules();

        let progression = visualizer.create_workflow_progression(&version, &rules);
        assert!(progression.is_some());

        let progression = progression.unwrap();
        assert_eq!(progression.current_step, "Generate_UBO");
        assert!(!progression.workflow_steps.is_empty());
        assert!(!progression.recommended_next_actions.is_empty());
    }

    #[test]
    fn test_domain_metrics_calculation() {
        let visualizer = DomainVisualizer::new();
        let visualization = create_test_visualization();
        let rules = create_kyc_visualization_rules();

        let metrics = visualizer.calculate_domain_metrics(&visualization, &rules);

        assert_eq!(metrics.complexity_score, Decimal::from(10));
        assert!(metrics.estimated_execution_time > 0);
        assert!(metrics.risk_score >= 0.0);
    }

    #[test]
    fn test_multiple_domain_support() {
        let visualizer = DomainVisualizer::new();

        // Test KYC domain
        assert!(visualizer.domain_rules.get("KYC").is_some());

        // Test Onboarding domain
        let onboarding_rules = visualizer.domain_rules.get("Onboarding");
        assert!(onboarding_rules.is_some());
        assert_eq!(onboarding_rules.unwrap().base_execution_time_ms, 3000);

        // Test Account Opening domain
        let account_rules = visualizer.domain_rules.get("Account_Opening");
        assert!(account_rules.is_some());
        assert_eq!(account_rules.unwrap().base_execution_time_ms, 4000);
    }

    #[test]
    fn test_completion_percentage_calculation() {
        let visualizer = DomainVisualizer::new();
        let states = vec![
            FunctionalState {
                name: "State1".to_string(),
                description: "First state".to_string(),
                estimated_effort: 30,
                dependencies: vec![],
            },
            FunctionalState {
                name: "State2".to_string(),
                description: "Second state".to_string(),
                estimated_effort: 60,
                dependencies: vec![],
            },
        ];

        // Test at beginning (position 0)
        let completion = visualizer.calculate_completion_percentage(0, &states);
        assert_eq!(completion, 0.0);

        // Test at middle (position 1 of 2)
        let completion = visualizer.calculate_completion_percentage(1, &states);
        assert_eq!(completion, 50.0);
    }
}
