//! Trading Matrix generator - transforms `TradingMatrixDocument` into projection nodes.
//!
//! Creates:
//! - InstrumentMatrix root node with category branches
//! - MatrixSlice nodes for each category
//! - Product/Service/Resource nodes for instruments/SSIs/etc.

use crate::model::{
    InspectorProjection, Node, NodeKind, NodeSummary, PagingList, RefOrList, SnapshotMeta, UiHints,
};
use crate::node_id::NodeId;
use crate::policy::RenderPolicy;
use crate::ref_value::RefValue;
use std::collections::BTreeMap;

/// Generator that transforms `TradingMatrixDocument` into an `InspectorProjection`.
///
/// The generator creates a hierarchical structure:
/// ```text
/// matrix:{cbu_id}
/// ├── slice:universe (MatrixSlice)
/// │   ├── product:equity (Product)
/// │   │   └── product:xnys (Product - market)
/// │   └── product:otc-irs (Product)
/// │       └── product:counterparty-gs (Product - counterparty)
/// ├── slice:ssi (MatrixSlice)
/// │   └── service:us-equities-ssi (Service)
/// │       └── resource:booking-rule-1 (Resource)
/// ├── slice:isda (MatrixSlice)
/// │   └── productbinding:goldman-sachs (ProductBinding)
/// │       └── productbinding:csa-vm (ProductBinding)
/// └── ...
/// ```
#[derive(Debug, Default)]
pub struct MatrixGenerator {
    /// Include empty categories.
    include_empty: bool,
}

impl MatrixGenerator {
    /// Create a new matrix generator with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure whether to include empty categories.
    pub fn with_empty_categories(mut self, include: bool) -> Self {
        self.include_empty = include;
        self
    }

    /// Generate a projection from a `TradingMatrixDocument`.
    pub fn generate(
        &self,
        cbu_id: &str,
        cbu_name: &str,
        children: &[MatrixNodeInput],
        policy: &RenderPolicy,
    ) -> InspectorProjection {
        let mut projection = InspectorProjection {
            snapshot: self.build_snapshot_meta(cbu_id),
            render_policy: policy.clone(),
            ui_hints: UiHints::default(),
            root: BTreeMap::new(),
            nodes: BTreeMap::new(),
        };

        // Create matrix root node
        let matrix_node_id =
            NodeId::new(format!("matrix:{}", cbu_id)).expect("valid matrix node id");
        let mut matrix_node =
            Node::new(matrix_node_id.clone(), NodeKind::InstrumentMatrix, cbu_name)
                .with_glyph("📊")
                .with_label_full(format!("{} Trading Matrix", cbu_name));

        // Process each category as a slice
        let mut slice_refs = Vec::new();
        let mut total_leaf_count = 0;

        for category in children {
            if category.children.is_empty() && !self.include_empty {
                continue;
            }

            let (slice_node, child_nodes) = self.build_slice(cbu_id, category, policy);

            total_leaf_count += category.leaf_count;
            slice_refs.push(RefValue::new(slice_node.id.clone()));

            projection.insert_node(slice_node);
            for node in child_nodes {
                projection.insert_node(node);
            }
        }

        // Add slices branch to matrix root
        if !slice_refs.is_empty() {
            let paging_list = PagingList::new(slice_refs, policy.max_items_per_list, None);
            matrix_node
                .branches
                .insert("slices".to_string(), RefOrList::List(paging_list));
        }

        matrix_node = matrix_node.with_summary(NodeSummary::count(total_leaf_count));

        // Insert matrix root and set root reference
        projection.insert_node(matrix_node);
        projection.set_root("matrix", matrix_node_id);

        projection
    }

    /// Build snapshot metadata.
    fn build_snapshot_meta(&self, cbu_id: &str) -> SnapshotMeta {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        cbu_id.hash(&mut hasher);
        "matrix".hash(&mut hasher);
        let source_hash = format!("{:016x}", hasher.finish());

        SnapshotMeta {
            schema_version: 1,
            source_hash,
            policy_hash: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Build a slice node and its children.
    fn build_slice(
        &self,
        cbu_id: &str,
        category: &MatrixNodeInput,
        policy: &RenderPolicy,
    ) -> (Node, Vec<Node>) {
        let slice_id = NodeId::new(format!(
            "matrixslice:{}:{}",
            cbu_id,
            Self::normalize_category_name(&category.label)
        ))
        .expect("valid slice id");

        let glyph = Self::category_glyph(&category.label);

        let mut slice_node =
            Node::new(slice_id.clone(), NodeKind::MatrixSlice, &category.label).with_glyph(glyph);

        let mut child_nodes = Vec::new();
        let mut child_refs = Vec::new();

        // Process children up to policy limit
        let max_items = policy.max_items_per_list;
        let limited_children = if category.children.len() > max_items {
            &category.children[..max_items]
        } else {
            &category.children[..]
        };

        for child in limited_children {
            let (child_node, descendants) =
                self.build_matrix_node(cbu_id, &slice_id, child, policy);
            child_refs.push(RefValue::new(child_node.id.clone()));
            child_nodes.push(child_node);
            child_nodes.extend(descendants);
        }

        // Add children branch with paging
        if !child_refs.is_empty() {
            let next_token = if category.children.len() > max_items {
                Some(
                    NodeId::new(format!(
                        "pagetoken:{}:{}:{}",
                        cbu_id,
                        Self::normalize_category_name(&category.label),
                        max_items
                    ))
                    .expect("valid pagetoken"),
                )
            } else {
                None
            };

            let paging_list = PagingList::new(child_refs, max_items, next_token);
            slice_node
                .branches
                .insert("items".to_string(), RefOrList::List(paging_list));
        }

        slice_node = slice_node.with_summary(NodeSummary::count(category.leaf_count));

        (slice_node, child_nodes)
    }

    /// Build a matrix node (recursive for nested structures).
    fn build_matrix_node(
        &self,
        cbu_id: &str,
        parent_id: &NodeId,
        node: &MatrixNodeInput,
        policy: &RenderPolicy,
    ) -> (Node, Vec<Node>) {
        let node_id = Self::build_node_id(cbu_id, parent_id, node);
        let (kind, glyph) = Self::node_kind_and_glyph(&node.node_type);

        let mut projection_node = Node::new(node_id.clone(), kind, &node.label).with_glyph(glyph);

        if let Some(ref sublabel) = node.sublabel {
            projection_node =
                projection_node.with_label_full(format!("{} - {}", node.label, sublabel));
        }

        // Add type-specific attributes
        projection_node = self.add_node_attributes(projection_node, node);

        // Process children recursively
        let mut descendants = Vec::new();
        let mut child_refs = Vec::new();

        let max_items = policy.max_items_per_list;
        let limited_children = if node.children.len() > max_items {
            &node.children[..max_items]
        } else {
            &node.children[..]
        };

        for child in limited_children {
            let (child_node, child_descendants) =
                self.build_matrix_node(cbu_id, &node_id, child, policy);
            child_refs.push(RefValue::new(child_node.id.clone()));
            descendants.push(child_node);
            descendants.extend(child_descendants);
        }

        if !child_refs.is_empty() {
            let next_token = if node.children.len() > max_items {
                Some(
                    NodeId::new(format!("pagetoken:{}:{}", cbu_id, node_id.as_str()))
                        .expect("valid pagetoken"),
                )
            } else {
                None
            };

            let paging_list = PagingList::new(child_refs, max_items, next_token);
            projection_node
                .branches
                .insert("children".to_string(), RefOrList::List(paging_list));
        }

        if node.leaf_count > 0 {
            projection_node = projection_node.with_summary(NodeSummary::count(node.leaf_count));
        }

        (projection_node, descendants)
    }

    /// Build a node ID for a matrix node.
    fn build_node_id(_cbu_id: &str, parent_id: &NodeId, node: &MatrixNodeInput) -> NodeId {
        // Use the node's ID segments if available, otherwise generate from label
        let suffix = node
            .id_segments
            .last()
            .map(|s| s.as_str())
            .unwrap_or(&node.label);

        let normalized = Self::normalize_segment(suffix);

        // Create a child node ID (child() handles invalid cases internally)
        parent_id.child(&normalized)
    }

    /// Normalize a segment for use in NodeId.
    fn normalize_segment(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
    }

    /// Normalize a category name for NodeId.
    fn normalize_category_name(name: &str) -> String {
        name.to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    }

    /// Get glyph for a category.
    fn category_glyph(name: &str) -> &'static str {
        match name.to_lowercase().as_str() {
            s if s.contains("universe") => "🌐",
            s if s.contains("settlement") && s.contains("instruction") => "📋",
            s if s.contains("settlement") && s.contains("chain") => "🔗",
            s if s.contains("tax") => "💰",
            s if s.contains("isda") => "📜",
            s if s.contains("pricing") => "💵",
            s if s.contains("manager") || s.contains("investment") => "👔",
            s if s.contains("corporate") || s.contains("action") => "📢",
            _ => "📁",
        }
    }

    /// Determine node kind and glyph from node type string.
    fn node_kind_and_glyph(node_type: &str) -> (NodeKind, &'static str) {
        match node_type {
            "instrument_class" => (NodeKind::Product, "📦"),
            "market" => (NodeKind::Product, "🏛️"),
            "counterparty" => (NodeKind::Product, "🤝"),
            "universe_entry" => (NodeKind::Product, "📄"),
            "ssi" => (NodeKind::Service, "📋"),
            "booking_rule" => (NodeKind::Resource, "📑"),
            "settlement_chain" => (NodeKind::Service, "🔗"),
            "settlement_hop" => (NodeKind::Resource, "➡️"),
            "tax_jurisdiction" => (NodeKind::Service, "💰"),
            "tax_config" => (NodeKind::Resource, "📝"),
            "isda_agreement" => (NodeKind::ProductBinding, "📜"),
            "csa_agreement" => (NodeKind::ProductBinding, "🛡️"),
            "product_coverage" => (NodeKind::Resource, "📊"),
            "investment_manager_mandate" => (NodeKind::Service, "👔"),
            "pricing_rule" => (NodeKind::Resource, "💵"),
            "corporate_actions_policy" => (NodeKind::Service, "📢"),
            "ca_event_type_config" => (NodeKind::Resource, "📅"),
            "ca_cutoff_rule_node" => (NodeKind::Resource, "⏰"),
            "ca_proceeds_mapping_node" => (NodeKind::Resource, "💸"),
            "category" => (NodeKind::MatrixSlice, "📁"),
            _ => (NodeKind::Product, "🧱"),
        }
    }

    /// Add type-specific attributes to a node.
    fn add_node_attributes(&self, mut node: Node, input: &MatrixNodeInput) -> Node {
        // Add all attributes from input
        for (key, value) in &input.attributes {
            node = node.with_attribute(key.clone(), value.clone());
        }

        // Add status if present
        if let Some(ref status) = input.status {
            node = node.with_attribute("status", status.as_str());
        }

        node
    }
}

// ============================================================================
// INPUT TYPES
// ============================================================================

/// Input matrix node (mirrors `TradingMatrixNode` from ob-poc-types).
#[derive(Debug, Clone)]
pub struct MatrixNodeInput {
    /// Node ID segments (path-based).
    pub id_segments: Vec<String>,
    /// Node type string (e.g., "instrument_class", "ssi").
    pub node_type: String,
    /// Primary label.
    pub label: String,
    /// Secondary label.
    pub sublabel: Option<String>,
    /// Child nodes.
    pub children: Vec<MatrixNodeInput>,
    /// Status (e.g., "active", "pending").
    pub status: Option<String>,
    /// Leaf count (for summary).
    pub leaf_count: usize,
    /// Additional attributes.
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl MatrixNodeInput {
    /// Create from a `TradingMatrixNode` (ob-poc-types).
    pub fn from_trading_matrix_node(
        node: &ob_poc_types::trading_matrix::TradingMatrixNode,
    ) -> Self {
        let (node_type, attributes) = Self::extract_type_and_attributes(&node.node_type);

        let children = node
            .children
            .iter()
            .map(Self::from_trading_matrix_node)
            .collect();

        let status = node.status_color.map(|c| match c {
            ob_poc_types::trading_matrix::StatusColor::Green => "active".to_string(),
            ob_poc_types::trading_matrix::StatusColor::Yellow => "pending".to_string(),
            ob_poc_types::trading_matrix::StatusColor::Red => "suspended".to_string(),
            ob_poc_types::trading_matrix::StatusColor::Gray => "inactive".to_string(),
        });

        Self {
            id_segments: node.id.0.clone(),
            node_type,
            label: node.label.clone(),
            sublabel: node.sublabel.clone(),
            children,
            status,
            leaf_count: node.leaf_count,
            attributes,
        }
    }

    /// Extract node type string and attributes from TradingMatrixNodeType.
    fn extract_type_and_attributes(
        node_type: &ob_poc_types::trading_matrix::TradingMatrixNodeType,
    ) -> (String, BTreeMap<String, serde_json::Value>) {
        use ob_poc_types::trading_matrix::TradingMatrixNodeType;

        let mut attrs = BTreeMap::new();

        let type_str = match node_type {
            TradingMatrixNodeType::Category { name } => {
                attrs.insert("name".to_string(), serde_json::json!(name));
                "category"
            }
            TradingMatrixNodeType::InstrumentClass {
                class_code,
                cfi_prefix,
                is_otc,
            } => {
                attrs.insert("class_code".to_string(), serde_json::json!(class_code));
                if let Some(cfi) = cfi_prefix {
                    attrs.insert("cfi_prefix".to_string(), serde_json::json!(cfi));
                }
                attrs.insert("is_otc".to_string(), serde_json::json!(is_otc));
                "instrument_class"
            }
            TradingMatrixNodeType::Market {
                mic,
                market_name,
                country_code,
            } => {
                attrs.insert("mic".to_string(), serde_json::json!(mic));
                attrs.insert("market_name".to_string(), serde_json::json!(market_name));
                attrs.insert("country_code".to_string(), serde_json::json!(country_code));
                "market"
            }
            TradingMatrixNodeType::Counterparty {
                entity_id,
                entity_name,
                lei,
            } => {
                attrs.insert("entity_id".to_string(), serde_json::json!(entity_id));
                attrs.insert("entity_name".to_string(), serde_json::json!(entity_name));
                if let Some(l) = lei {
                    attrs.insert("lei".to_string(), serde_json::json!(l));
                }
                "counterparty"
            }
            TradingMatrixNodeType::UniverseEntry {
                universe_id,
                currencies,
                settlement_types,
                is_held,
                is_traded,
            } => {
                attrs.insert("universe_id".to_string(), serde_json::json!(universe_id));
                attrs.insert("currencies".to_string(), serde_json::json!(currencies));
                attrs.insert(
                    "settlement_types".to_string(),
                    serde_json::json!(settlement_types),
                );
                attrs.insert("is_held".to_string(), serde_json::json!(is_held));
                attrs.insert("is_traded".to_string(), serde_json::json!(is_traded));
                "universe_entry"
            }
            TradingMatrixNodeType::Ssi {
                ssi_id,
                ssi_name,
                ssi_type,
                status,
                ..
            } => {
                attrs.insert("ssi_id".to_string(), serde_json::json!(ssi_id));
                attrs.insert("ssi_name".to_string(), serde_json::json!(ssi_name));
                attrs.insert("ssi_type".to_string(), serde_json::json!(ssi_type));
                attrs.insert("status".to_string(), serde_json::json!(status));
                "ssi"
            }
            TradingMatrixNodeType::BookingRule {
                rule_id,
                rule_name,
                priority,
                specificity_score,
                is_active,
                ..
            } => {
                attrs.insert("rule_id".to_string(), serde_json::json!(rule_id));
                attrs.insert("rule_name".to_string(), serde_json::json!(rule_name));
                attrs.insert("priority".to_string(), serde_json::json!(priority));
                attrs.insert(
                    "specificity_score".to_string(),
                    serde_json::json!(specificity_score),
                );
                attrs.insert("is_active".to_string(), serde_json::json!(is_active));
                "booking_rule"
            }
            TradingMatrixNodeType::SettlementChain {
                chain_id,
                chain_name,
                hop_count,
                is_active,
                ..
            } => {
                attrs.insert("chain_id".to_string(), serde_json::json!(chain_id));
                attrs.insert("chain_name".to_string(), serde_json::json!(chain_name));
                attrs.insert("hop_count".to_string(), serde_json::json!(hop_count));
                attrs.insert("is_active".to_string(), serde_json::json!(is_active));
                "settlement_chain"
            }
            TradingMatrixNodeType::SettlementHop {
                hop_id,
                sequence,
                role,
                ..
            } => {
                attrs.insert("hop_id".to_string(), serde_json::json!(hop_id));
                attrs.insert("sequence".to_string(), serde_json::json!(sequence));
                attrs.insert("role".to_string(), serde_json::json!(role));
                "settlement_hop"
            }
            TradingMatrixNodeType::TaxJurisdiction {
                jurisdiction_id,
                jurisdiction_code,
                jurisdiction_name,
                reclaim_available,
                ..
            } => {
                attrs.insert(
                    "jurisdiction_id".to_string(),
                    serde_json::json!(jurisdiction_id),
                );
                attrs.insert(
                    "jurisdiction_code".to_string(),
                    serde_json::json!(jurisdiction_code),
                );
                attrs.insert(
                    "jurisdiction_name".to_string(),
                    serde_json::json!(jurisdiction_name),
                );
                attrs.insert(
                    "reclaim_available".to_string(),
                    serde_json::json!(reclaim_available),
                );
                "tax_jurisdiction"
            }
            TradingMatrixNodeType::TaxConfig {
                status_id,
                investor_type,
                tax_exempt,
                ..
            } => {
                attrs.insert("status_id".to_string(), serde_json::json!(status_id));
                attrs.insert(
                    "investor_type".to_string(),
                    serde_json::json!(investor_type),
                );
                attrs.insert("tax_exempt".to_string(), serde_json::json!(tax_exempt));
                "tax_config"
            }
            TradingMatrixNodeType::IsdaAgreement {
                isda_id,
                counterparty_name,
                governing_law,
                ..
            } => {
                attrs.insert("isda_id".to_string(), serde_json::json!(isda_id));
                attrs.insert(
                    "counterparty_name".to_string(),
                    serde_json::json!(counterparty_name),
                );
                if let Some(law) = governing_law {
                    attrs.insert("governing_law".to_string(), serde_json::json!(law));
                }
                "isda_agreement"
            }
            TradingMatrixNodeType::CsaAgreement {
                csa_id, csa_type, ..
            } => {
                attrs.insert("csa_id".to_string(), serde_json::json!(csa_id));
                attrs.insert("csa_type".to_string(), serde_json::json!(csa_type));
                "csa_agreement"
            }
            TradingMatrixNodeType::ProductCoverage {
                coverage_id,
                asset_class,
                base_products,
            } => {
                attrs.insert("coverage_id".to_string(), serde_json::json!(coverage_id));
                attrs.insert("asset_class".to_string(), serde_json::json!(asset_class));
                attrs.insert(
                    "base_products".to_string(),
                    serde_json::json!(base_products),
                );
                "product_coverage"
            }
            TradingMatrixNodeType::InvestmentManagerMandate {
                mandate_id,
                manager_name,
                priority,
                role,
                can_trade,
                can_settle,
                ..
            } => {
                attrs.insert("mandate_id".to_string(), serde_json::json!(mandate_id));
                attrs.insert("manager_name".to_string(), serde_json::json!(manager_name));
                attrs.insert("priority".to_string(), serde_json::json!(priority));
                attrs.insert("role".to_string(), serde_json::json!(role));
                attrs.insert("can_trade".to_string(), serde_json::json!(can_trade));
                attrs.insert("can_settle".to_string(), serde_json::json!(can_settle));
                "investment_manager_mandate"
            }
            TradingMatrixNodeType::PricingRule {
                rule_id,
                priority,
                source,
                ..
            } => {
                attrs.insert("rule_id".to_string(), serde_json::json!(rule_id));
                attrs.insert("priority".to_string(), serde_json::json!(priority));
                attrs.insert("source".to_string(), serde_json::json!(source));
                "pricing_rule"
            }
            TradingMatrixNodeType::CorporateActionsPolicy {
                enabled_count,
                has_custom_elections,
                has_cutoff_rules,
                elector,
            } => {
                attrs.insert(
                    "enabled_count".to_string(),
                    serde_json::json!(enabled_count),
                );
                attrs.insert(
                    "has_custom_elections".to_string(),
                    serde_json::json!(has_custom_elections),
                );
                attrs.insert(
                    "has_cutoff_rules".to_string(),
                    serde_json::json!(has_cutoff_rules),
                );
                if let Some(e) = elector {
                    attrs.insert("elector".to_string(), serde_json::json!(e));
                }
                "corporate_actions_policy"
            }
            TradingMatrixNodeType::CaEventTypeConfig {
                event_code,
                event_name,
                processing_mode,
                is_elective,
                ..
            } => {
                attrs.insert("event_code".to_string(), serde_json::json!(event_code));
                attrs.insert("event_name".to_string(), serde_json::json!(event_name));
                attrs.insert(
                    "processing_mode".to_string(),
                    serde_json::json!(processing_mode),
                );
                attrs.insert("is_elective".to_string(), serde_json::json!(is_elective));
                "ca_event_type_config"
            }
            TradingMatrixNodeType::CaCutoffRuleNode {
                rule_key,
                days_before,
                warning_days,
                escalation_days,
            } => {
                attrs.insert("rule_key".to_string(), serde_json::json!(rule_key));
                attrs.insert("days_before".to_string(), serde_json::json!(days_before));
                attrs.insert("warning_days".to_string(), serde_json::json!(warning_days));
                attrs.insert(
                    "escalation_days".to_string(),
                    serde_json::json!(escalation_days),
                );
                "ca_cutoff_rule_node"
            }
            TradingMatrixNodeType::CaProceedsMappingNode {
                proceeds_type,
                currency,
                ssi_reference,
            } => {
                attrs.insert(
                    "proceeds_type".to_string(),
                    serde_json::json!(proceeds_type),
                );
                if let Some(c) = currency {
                    attrs.insert("currency".to_string(), serde_json::json!(c));
                }
                attrs.insert(
                    "ssi_reference".to_string(),
                    serde_json::json!(ssi_reference),
                );
                "ca_proceeds_mapping_node"
            }
        };

        (type_str.to_string(), attrs)
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Generate a projection directly from a `TradingMatrixDocument`.
pub fn generate_from_trading_matrix(
    doc: &ob_poc_types::trading_matrix::TradingMatrixDocument,
    policy: &RenderPolicy,
) -> InspectorProjection {
    let children: Vec<MatrixNodeInput> = doc
        .children
        .iter()
        .map(MatrixNodeInput::from_trading_matrix_node)
        .collect();

    MatrixGenerator::new().generate(&doc.cbu_id, &doc.cbu_name, &children, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;

    fn make_test_category(name: &str, children: Vec<MatrixNodeInput>) -> MatrixNodeInput {
        let leaf_count: usize = children.iter().map(|c| c.leaf_count.max(1)).sum();
        MatrixNodeInput {
            id_segments: vec![format!("_{}", name.to_uppercase())],
            node_type: "category".to_string(),
            label: name.to_string(),
            sublabel: None,
            children,
            status: None,
            leaf_count,
            attributes: BTreeMap::new(),
        }
    }

    fn make_test_instrument(class_code: &str, is_otc: bool) -> MatrixNodeInput {
        let mut attrs = BTreeMap::new();
        attrs.insert("class_code".to_string(), serde_json::json!(class_code));
        attrs.insert("is_otc".to_string(), serde_json::json!(is_otc));

        MatrixNodeInput {
            id_segments: vec!["_UNIVERSE".to_string(), class_code.to_string()],
            node_type: "instrument_class".to_string(),
            label: class_code.to_string(),
            sublabel: None,
            children: vec![],
            status: Some("active".to_string()),
            leaf_count: 1,
            attributes: attrs,
        }
    }

    #[test]
    fn test_matrix_generator_basic() {
        let children = vec![make_test_category(
            "Trading Universe",
            vec![
                make_test_instrument("EQUITY", false),
                make_test_instrument("OTC_IRS", true),
            ],
        )];

        let policy = RenderPolicy::default();
        let projection =
            MatrixGenerator::new().generate("cbu-001", "Test Fund", &children, &policy);

        // Validate the projection
        let result = validate(&projection);
        assert!(
            result.errors.is_empty(),
            "Validation errors: {:?}",
            result.errors
        );

        // Check structure
        assert!(projection.root.contains_key("matrix"));

        let matrix_id = NodeId::new("matrix:cbu-001").unwrap();
        let matrix_node = projection
            .get_node(&matrix_id)
            .expect("Matrix node should exist");
        assert_eq!(matrix_node.kind, NodeKind::InstrumentMatrix);
        assert!(matrix_node.branches.contains_key("slices"));
    }

    #[test]
    fn test_matrix_generator_multiple_categories() {
        let children = vec![
            make_test_category(
                "Trading Universe",
                vec![make_test_instrument("EQUITY", false)],
            ),
            make_test_category("Standing Settlement Instructions", vec![]),
            make_test_category("ISDA Agreements", vec![]),
        ];

        let policy = RenderPolicy::default();
        let projection = MatrixGenerator::new().with_empty_categories(true).generate(
            "cbu-001",
            "Test Fund",
            &children,
            &policy,
        );

        // Should have all 3 slices even though 2 are empty
        let matrix_id = NodeId::new("matrix:cbu-001").unwrap();
        let matrix_node = projection.get_node(&matrix_id).unwrap();

        if let Some(RefOrList::List(paging_list)) = matrix_node.branches.get("slices") {
            assert_eq!(paging_list.items.len(), 3);
        } else {
            panic!("Expected slices branch");
        }
    }

    #[test]
    fn test_matrix_generator_excludes_empty_by_default() {
        let children = vec![
            make_test_category(
                "Trading Universe",
                vec![make_test_instrument("EQUITY", false)],
            ),
            make_test_category("Standing Settlement Instructions", vec![]),
        ];

        let policy = RenderPolicy::default();
        let projection =
            MatrixGenerator::new().generate("cbu-001", "Test Fund", &children, &policy);

        // Should have only 1 slice (non-empty)
        let matrix_id = NodeId::new("matrix:cbu-001").unwrap();
        let matrix_node = projection.get_node(&matrix_id).unwrap();

        if let Some(RefOrList::List(paging_list)) = matrix_node.branches.get("slices") {
            assert_eq!(paging_list.items.len(), 1);
        } else {
            panic!("Expected slices branch");
        }
    }

    #[test]
    fn test_slice_has_correct_glyph() {
        let children = vec![
            make_test_category("Trading Universe", vec![]),
            make_test_category("Tax Configuration", vec![]),
            make_test_category("ISDA Agreements", vec![]),
        ];

        let policy = RenderPolicy::default();
        let projection = MatrixGenerator::new().with_empty_categories(true).generate(
            "cbu-001",
            "Test Fund",
            &children,
            &policy,
        );

        // Check universe slice has globe glyph
        let universe_id = NodeId::new("matrixslice:cbu-001:trading-universe").unwrap();
        let universe = projection.get_node(&universe_id).unwrap();
        assert_eq!(universe.glyph, Some("🌐".to_string()));

        // Check tax slice has money glyph
        let tax_id = NodeId::new("matrixslice:cbu-001:tax-configuration").unwrap();
        let tax = projection.get_node(&tax_id).unwrap();
        assert_eq!(tax.glyph, Some("💰".to_string()));

        // Check ISDA slice has scroll glyph
        let isda_id = NodeId::new("matrixslice:cbu-001:isda-agreements").unwrap();
        let isda = projection.get_node(&isda_id).unwrap();
        assert_eq!(isda.glyph, Some("📜".to_string()));
    }

    #[test]
    fn test_nested_children() {
        let market = MatrixNodeInput {
            id_segments: vec![
                "_UNIVERSE".to_string(),
                "EQUITY".to_string(),
                "XNYS".to_string(),
            ],
            node_type: "market".to_string(),
            label: "NYSE".to_string(),
            sublabel: Some("New York Stock Exchange".to_string()),
            children: vec![],
            status: Some("active".to_string()),
            leaf_count: 1,
            attributes: {
                let mut m = BTreeMap::new();
                m.insert("mic".to_string(), serde_json::json!("XNYS"));
                m.insert("country_code".to_string(), serde_json::json!("US"));
                m
            },
        };

        let equity = MatrixNodeInput {
            id_segments: vec!["_UNIVERSE".to_string(), "EQUITY".to_string()],
            node_type: "instrument_class".to_string(),
            label: "Equity".to_string(),
            sublabel: None,
            children: vec![market],
            status: Some("active".to_string()),
            leaf_count: 1,
            attributes: BTreeMap::new(),
        };

        let children = vec![make_test_category("Trading Universe", vec![equity])];

        let policy = RenderPolicy::default();
        let projection =
            MatrixGenerator::new().generate("cbu-001", "Test Fund", &children, &policy);

        // Validate
        let result = validate(&projection);
        assert!(
            result.errors.is_empty(),
            "Validation errors: {:?}",
            result.errors
        );

        // Verify nested structure exists
        assert!(projection.nodes.len() >= 4); // matrix + slice + equity + market
    }
}
