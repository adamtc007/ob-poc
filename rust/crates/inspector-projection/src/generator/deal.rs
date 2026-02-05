//! Deal taxonomy generator for Inspector projections.
//!
//! Transforms Deal data into InspectorProjection format with proper
//! $ref linking and LOD support.

use crate::{
    model::{InspectorProjection, Node, NodeKind, NodeSummary, PagingList, SnapshotMeta},
    node_id::NodeId,
    policy::RenderPolicy,
    ref_value::RefValue,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ============================================================================
// INPUT TYPES
// ============================================================================

/// Input data for generating a deal projection.
#[derive(Debug, Clone)]
pub struct DealInput {
    pub deal_id: Uuid,
    pub deal_name: String,
    pub deal_status: String,
    pub client_group_id: Option<Uuid>,
    pub client_group_name: Option<String>,
    pub product_count: i32,
    pub rate_card_count: i32,
    pub participant_count: i32,
    pub contract_count: i32,
    pub onboarding_request_count: i32,
    pub products: Vec<DealProductInput>,
    pub participants: Vec<DealParticipantInput>,
    pub contracts: Vec<DealContractInput>,
    pub onboarding_requests: Vec<OnboardingRequestInput>,
}

/// Product within a deal.
#[derive(Debug, Clone)]
pub struct DealProductInput {
    pub deal_product_id: Uuid,
    pub product_name: String,
    pub product_code: Option<String>,
    pub product_category: Option<String>,
    pub product_status: String,
    pub rate_cards: Vec<RateCardInput>,
}

/// Rate card within a product.
#[derive(Debug, Clone)]
pub struct RateCardInput {
    pub rate_card_id: Uuid,
    pub rate_card_name: String,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub status: Option<String>,
    pub lines: Vec<RateCardLineInput>,
}

/// Line item within a rate card.
#[derive(Debug, Clone)]
pub struct RateCardLineInput {
    pub line_id: Uuid,
    pub fee_type: String,
    pub fee_subtype: String,
    pub pricing_model: String,
    pub rate_value: Option<String>,
    pub currency: Option<String>,
}

/// Participant in a deal.
#[derive(Debug, Clone)]
pub struct DealParticipantInput {
    pub participant_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub role: String,
    pub jurisdiction: Option<String>,
}

/// Contract within a deal.
#[derive(Debug, Clone)]
pub struct DealContractInput {
    pub contract_id: Uuid,
    pub contract_name: String,
    pub contract_type: String,
    pub effective_date: Option<String>,
    pub status: String,
}

/// Onboarding request linked to a deal.
#[derive(Debug, Clone)]
pub struct OnboardingRequestInput {
    pub request_id: Uuid,
    pub request_type: String,
    pub status: String,
    pub cbu_id: Option<Uuid>,
    pub cbu_name: Option<String>,
    pub created_at: String,
}

// ============================================================================
// GENERATOR
// ============================================================================

/// Generator for deal taxonomy projections.
pub struct DealGenerator {
    policy: RenderPolicy,
}

impl DealGenerator {
    /// Create a new generator with the given render policy.
    pub fn new(policy: RenderPolicy) -> Self {
        Self { policy }
    }

    /// Generate an InspectorProjection from deal input data.
    pub fn generate(&self, deal: &DealInput) -> InspectorProjection {
        let mut projection = InspectorProjection::new();

        // Set snapshot metadata
        let source_hash = self.compute_source_hash(deal);
        projection.snapshot = SnapshotMeta {
            schema_version: 1,
            source_hash,
            policy_hash: self.policy.policy_hash(),
            created_at: Utc::now().to_rfc3339(),
        };
        projection.render_policy = self.policy.clone();

        // Build deal root node
        let deal_id = NodeId::new(format!("deal:{}", deal.deal_id)).expect("valid node id");

        let mut deal_node = Node::new(deal_id.clone(), NodeKind::Deal, &deal.deal_name)
            .with_glyph(NodeKind::Deal.default_glyph())
            .with_attribute("deal_id", deal.deal_id.to_string())
            .with_attribute("deal_status", deal.deal_status.as_str());

        if let Some(ref client) = deal.client_group_name {
            deal_node = deal_node.with_attribute("client_group", client.as_str());
        }

        deal_node = deal_node.with_summary(NodeSummary::with_status(
            deal.product_count as usize,
            format!("{} products", deal.product_count),
        ));

        // Build product list branch
        if !deal.products.is_empty() {
            let product_list_id =
                NodeId::new(format!("deal:{}:products", deal.deal_id)).expect("valid node id");
            self.build_product_list(
                &mut projection,
                &product_list_id,
                &deal.products,
                deal.deal_id,
            );
            deal_node = deal_node.with_branch("products", product_list_id);
        }

        // Build participant list branch
        if !deal.participants.is_empty() {
            let participant_list_id =
                NodeId::new(format!("deal:{}:participants", deal.deal_id)).expect("valid node id");
            self.build_participant_list(
                &mut projection,
                &participant_list_id,
                &deal.participants,
                deal.deal_id,
            );
            deal_node = deal_node.with_branch("participants", participant_list_id);
        }

        // Build contract list branch
        if !deal.contracts.is_empty() {
            let contract_list_id =
                NodeId::new(format!("deal:{}:contracts", deal.deal_id)).expect("valid node id");
            self.build_contract_list(
                &mut projection,
                &contract_list_id,
                &deal.contracts,
                deal.deal_id,
            );
            deal_node = deal_node.with_branch("contracts", contract_list_id);
        }

        // Build onboarding request list branch
        if !deal.onboarding_requests.is_empty() {
            let onboarding_list_id =
                NodeId::new(format!("deal:{}:onboarding", deal.deal_id)).expect("valid node id");
            self.build_onboarding_list(
                &mut projection,
                &onboarding_list_id,
                &deal.onboarding_requests,
                deal.deal_id,
            );
            deal_node = deal_node.with_branch("onboarding", onboarding_list_id);
        }

        // Insert deal node and set as root
        projection.insert_node(deal_node);
        projection.set_root("deal", deal_id);

        projection
    }

    /// Build the product list node and all product children.
    fn build_product_list(
        &self,
        projection: &mut InspectorProjection,
        list_id: &NodeId,
        products: &[DealProductInput],
        deal_id: Uuid,
    ) {
        let mut list_node = Node::new(
            list_id.clone(),
            NodeKind::DealProductList,
            format!("Products ({})", products.len()),
        )
        .with_glyph(NodeKind::DealProductList.default_glyph())
        .with_summary(NodeSummary::count(products.len()));

        // Build each product node
        let mut product_refs = Vec::new();
        for product in products {
            let product_id = NodeId::new(format!(
                "deal:{}:product:{}",
                deal_id, product.deal_product_id
            ))
            .expect("valid node id");

            let mut product_node = Node::new(
                product_id.clone(),
                NodeKind::DealProduct,
                &product.product_name,
            )
            .with_glyph(NodeKind::DealProduct.default_glyph())
            .with_attribute("status", product.product_status.as_str());

            if let Some(ref code) = product.product_code {
                product_node = product_node.with_attribute("product_code", code.as_str());
            }
            if let Some(ref cat) = product.product_category {
                product_node = product_node.with_attribute("category", cat.as_str());
            }

            // Build rate cards for this product
            if !product.rate_cards.is_empty() {
                let rc_list_id = NodeId::new(format!(
                    "deal:{}:product:{}:ratecards",
                    deal_id, product.deal_product_id
                ))
                .expect("valid node id");
                self.build_rate_card_list(
                    projection,
                    &rc_list_id,
                    &product.rate_cards,
                    deal_id,
                    product.deal_product_id,
                );
                product_node = product_node.with_branch("rate_cards", rc_list_id);
            }

            product_refs.push(RefValue::new(product_id.clone()));
            projection.insert_node(product_node);
        }

        // Add items branch with paging
        let paging_list = PagingList::new(product_refs, self.policy.max_items_per_list, None);
        list_node = list_node.with_branch_list("items", paging_list);

        projection.insert_node(list_node);
    }

    /// Build the rate card list node and all rate card children.
    fn build_rate_card_list(
        &self,
        projection: &mut InspectorProjection,
        list_id: &NodeId,
        rate_cards: &[RateCardInput],
        deal_id: Uuid,
        product_id: Uuid,
    ) {
        let mut list_node = Node::new(
            list_id.clone(),
            NodeKind::DealRateCardList,
            format!("Rate Cards ({})", rate_cards.len()),
        )
        .with_glyph(NodeKind::DealRateCardList.default_glyph())
        .with_summary(NodeSummary::count(rate_cards.len()));

        let mut rc_refs = Vec::new();
        for rc in rate_cards {
            let rc_id = NodeId::new(format!(
                "deal:{}:product:{}:ratecard:{}",
                deal_id, product_id, rc.rate_card_id
            ))
            .expect("valid node id");

            let mut rc_node = Node::new(rc_id.clone(), NodeKind::DealRateCard, &rc.rate_card_name)
                .with_glyph(NodeKind::DealRateCard.default_glyph())
                .with_attribute("effective_from", rc.effective_from.as_str());

            if let Some(ref status) = rc.status {
                rc_node = rc_node.with_attribute("status", status.as_str());
            }
            if let Some(ref to) = rc.effective_to {
                rc_node = rc_node.with_attribute("effective_to", to.as_str());
            }

            // Build lines for this rate card (only if LOD >= 2)
            if self.policy.lod >= 2 && !rc.lines.is_empty() {
                let lines_list_id = NodeId::new(format!(
                    "deal:{}:product:{}:ratecard:{}:lines",
                    deal_id, product_id, rc.rate_card_id
                ))
                .expect("valid node id");

                let mut line_refs = Vec::new();
                for line in &rc.lines {
                    let line_id = NodeId::new(format!(
                        "deal:{}:product:{}:ratecard:{}:line:{}",
                        deal_id, product_id, rc.rate_card_id, line.line_id
                    ))
                    .expect("valid node id");

                    let mut line_node = Node::new(
                        line_id.clone(),
                        NodeKind::DealRateCardLine,
                        format!("{} - {}", line.fee_type, line.fee_subtype),
                    )
                    .with_glyph(NodeKind::DealRateCardLine.default_glyph())
                    .with_attribute("fee_type", line.fee_type.as_str())
                    .with_attribute("fee_subtype", line.fee_subtype.as_str())
                    .with_attribute("pricing_model", line.pricing_model.as_str());

                    if let Some(ref rate) = line.rate_value {
                        line_node = line_node.with_attribute("rate_value", rate.as_str());
                    }
                    if let Some(ref ccy) = line.currency {
                        line_node = line_node.with_attribute("currency", ccy.as_str());
                    }

                    line_refs.push(RefValue::new(line_id.clone()));
                    projection.insert_node(line_node);
                }

                // Create lines list node
                let lines_list_node = Node::new(
                    lines_list_id.clone(),
                    NodeKind::DealRateCardList, // Reuse for lines list
                    format!("Lines ({})", rc.lines.len()),
                )
                .with_summary(NodeSummary::count(rc.lines.len()))
                .with_branch_list(
                    "items",
                    PagingList::new(line_refs, self.policy.max_items_per_list, None),
                );

                projection.insert_node(lines_list_node);
                rc_node = rc_node.with_branch("lines", lines_list_id);
            }

            rc_refs.push(RefValue::new(rc_id.clone()));
            projection.insert_node(rc_node);
        }

        // Add items branch with paging
        let paging_list = PagingList::new(rc_refs, self.policy.max_items_per_list, None);
        list_node = list_node.with_branch_list("items", paging_list);

        projection.insert_node(list_node);
    }

    /// Build the participant list node.
    fn build_participant_list(
        &self,
        projection: &mut InspectorProjection,
        list_id: &NodeId,
        participants: &[DealParticipantInput],
        deal_id: Uuid,
    ) {
        let mut list_node = Node::new(
            list_id.clone(),
            NodeKind::DealParticipantList,
            format!("Participants ({})", participants.len()),
        )
        .with_glyph(NodeKind::DealParticipantList.default_glyph())
        .with_summary(NodeSummary::count(participants.len()));

        let mut participant_refs = Vec::new();
        for p in participants {
            let p_id = NodeId::new(format!("deal:{}:participant:{}", deal_id, p.participant_id))
                .expect("valid node id");

            let mut p_node = Node::new(p_id.clone(), NodeKind::DealParticipant, &p.entity_name)
                .with_glyph(NodeKind::DealParticipant.default_glyph())
                .with_attribute("role", p.role.as_str())
                .with_attribute("entity_id", p.entity_id.to_string());

            if let Some(ref jur) = p.jurisdiction {
                p_node = p_node.with_attribute("jurisdiction", jur.as_str());
            }

            participant_refs.push(RefValue::new(p_id.clone()));
            projection.insert_node(p_node);
        }

        let paging_list = PagingList::new(participant_refs, self.policy.max_items_per_list, None);
        list_node = list_node.with_branch_list("items", paging_list);

        projection.insert_node(list_node);
    }

    /// Build the contract list node.
    fn build_contract_list(
        &self,
        projection: &mut InspectorProjection,
        list_id: &NodeId,
        contracts: &[DealContractInput],
        deal_id: Uuid,
    ) {
        let mut list_node = Node::new(
            list_id.clone(),
            NodeKind::DealContractList,
            format!("Contracts ({})", contracts.len()),
        )
        .with_glyph(NodeKind::DealContractList.default_glyph())
        .with_summary(NodeSummary::count(contracts.len()));

        let mut contract_refs = Vec::new();
        for c in contracts {
            let c_id = NodeId::new(format!("deal:{}:contract:{}", deal_id, c.contract_id))
                .expect("valid node id");

            let mut c_node = Node::new(c_id.clone(), NodeKind::DealContract, &c.contract_name)
                .with_glyph(NodeKind::DealContract.default_glyph())
                .with_attribute("contract_type", c.contract_type.as_str())
                .with_attribute("status", c.status.as_str());

            if let Some(ref eff) = c.effective_date {
                c_node = c_node.with_attribute("effective_date", eff.as_str());
            }

            contract_refs.push(RefValue::new(c_id.clone()));
            projection.insert_node(c_node);
        }

        let paging_list = PagingList::new(contract_refs, self.policy.max_items_per_list, None);
        list_node = list_node.with_branch_list("items", paging_list);

        projection.insert_node(list_node);
    }

    /// Build the onboarding request list node.
    fn build_onboarding_list(
        &self,
        projection: &mut InspectorProjection,
        list_id: &NodeId,
        requests: &[OnboardingRequestInput],
        deal_id: Uuid,
    ) {
        let mut list_node = Node::new(
            list_id.clone(),
            NodeKind::DealOnboardingRequestList,
            format!("Onboarding Requests ({})", requests.len()),
        )
        .with_glyph(NodeKind::DealOnboardingRequestList.default_glyph())
        .with_summary(NodeSummary::count(requests.len()));

        let mut request_refs = Vec::new();
        for r in requests {
            let r_id = NodeId::new(format!("deal:{}:onboarding:{}", deal_id, r.request_id))
                .expect("valid node id");

            let mut r_node = Node::new(
                r_id.clone(),
                NodeKind::DealOnboardingRequest,
                format!("{} - {}", r.request_type, r.status),
            )
            .with_glyph(NodeKind::DealOnboardingRequest.default_glyph())
            .with_attribute("request_type", r.request_type.as_str())
            .with_attribute("status", r.status.as_str())
            .with_attribute("created_at", r.created_at.as_str());

            if let Some(cbu_id) = r.cbu_id {
                r_node = r_node.with_attribute("cbu_id", cbu_id.to_string());
            }
            if let Some(ref cbu_name) = r.cbu_name {
                r_node = r_node.with_attribute("cbu_name", cbu_name.as_str());
            }

            request_refs.push(RefValue::new(r_id.clone()));
            projection.insert_node(r_node);
        }

        let paging_list = PagingList::new(request_refs, self.policy.max_items_per_list, None);
        list_node = list_node.with_branch_list("items", paging_list);

        projection.insert_node(list_node);
    }

    /// Compute a hash of the source data for cache invalidation.
    fn compute_source_hash(&self, deal: &DealInput) -> String {
        let mut hasher = Sha256::new();
        hasher.update(deal.deal_id.as_bytes());
        hasher.update(deal.deal_name.as_bytes());
        hasher.update(deal.deal_status.as_bytes());
        hasher.update((deal.product_count as u32).to_le_bytes());
        hasher.update((deal.rate_card_count as u32).to_le_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..16])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_deal() -> DealInput {
        DealInput {
            deal_id: Uuid::new_v4(),
            deal_name: "Test Deal".to_string(),
            deal_status: "ACTIVE".to_string(),
            client_group_id: Some(Uuid::new_v4()),
            client_group_name: Some("Test Client".to_string()),
            product_count: 2,
            rate_card_count: 3,
            participant_count: 1,
            contract_count: 1,
            onboarding_request_count: 0,
            products: vec![DealProductInput {
                deal_product_id: Uuid::new_v4(),
                product_name: "Custody".to_string(),
                product_code: Some("CUST".to_string()),
                product_category: Some("Core".to_string()),
                product_status: "ACTIVE".to_string(),
                rate_cards: vec![RateCardInput {
                    rate_card_id: Uuid::new_v4(),
                    rate_card_name: "Standard Rate Card".to_string(),
                    effective_from: "2024-01-01".to_string(),
                    effective_to: None,
                    status: Some("ACTIVE".to_string()),
                    lines: vec![RateCardLineInput {
                        line_id: Uuid::new_v4(),
                        fee_type: "Safekeeping".to_string(),
                        fee_subtype: "Equity".to_string(),
                        pricing_model: "BPS".to_string(),
                        rate_value: Some("5".to_string()),
                        currency: Some("USD".to_string()),
                    }],
                }],
            }],
            participants: vec![DealParticipantInput {
                participant_id: Uuid::new_v4(),
                entity_id: Uuid::new_v4(),
                entity_name: "Participant Entity".to_string(),
                role: "COUNTERPARTY".to_string(),
                jurisdiction: Some("US".to_string()),
            }],
            contracts: vec![DealContractInput {
                contract_id: Uuid::new_v4(),
                contract_name: "Master Agreement".to_string(),
                contract_type: "MSA".to_string(),
                effective_date: Some("2024-01-01".to_string()),
                status: "ACTIVE".to_string(),
            }],
            onboarding_requests: vec![],
        }
    }

    #[test]
    fn test_deal_generator_basic() {
        let deal = sample_deal();
        let policy = RenderPolicy::default();
        let generator = DealGenerator::new(policy);

        let projection = generator.generate(&deal);

        // Check root is set
        assert!(projection.root.contains_key("deal"));

        // Check deal node exists
        let deal_node_id = NodeId::new(&format!("deal:{}", deal.deal_id)).unwrap();
        let deal_node = projection.get_node(&deal_node_id).unwrap();
        assert_eq!(deal_node.kind, NodeKind::Deal);
        assert_eq!(deal_node.label_short, "Test Deal");
    }

    #[test]
    fn test_deal_generator_with_products() {
        let deal = sample_deal();
        let policy = RenderPolicy::default();
        let generator = DealGenerator::new(policy);

        let projection = generator.generate(&deal);

        // Check product list exists
        let product_list_id = NodeId::new(&format!("deal:{}:products", deal.deal_id)).unwrap();
        let product_list = projection.get_node(&product_list_id).unwrap();
        assert_eq!(product_list.kind, NodeKind::DealProductList);
    }

    #[test]
    fn test_deal_generator_determinism() {
        let deal = sample_deal();
        let policy = RenderPolicy::default();
        let generator = DealGenerator::new(policy);

        let proj1 = generator.generate(&deal);
        let proj2 = generator.generate(&deal);

        // Source hashes should match
        assert_eq!(proj1.snapshot.source_hash, proj2.snapshot.source_hash);
    }
}
