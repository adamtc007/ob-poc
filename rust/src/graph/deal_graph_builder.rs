//! DealGraphBuilder - Builds deal taxonomy graph for visualization
//!
//! This builder constructs the deal hierarchy graph:
//!   Deal (root)
//!   ├── Products (commercial scope)
//!   │   └── Rate Cards
//!   │       └── Rate Card Lines
//!   ├── Participants (regional LEIs)
//!   ├── Contracts (legal agreements)
//!   └── Onboarding Requests
//!       └── CBU (if onboarded)

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::deal_types::{
    DealContractSummary, DealGraphResponse, DealParticipantSummary, DealProductSummary,
    DealSummary, DealViewMode, OnboardingRequestSummary, RateCardSummary,
};
use crate::database::DealRepository;

/// Builder for constructing deal taxonomy graphs
pub struct DealGraphBuilder {
    deal_id: Uuid,
    view_mode: DealViewMode,
}

impl DealGraphBuilder {
    /// Create a new builder for the given deal
    pub fn new(deal_id: Uuid) -> Self {
        Self {
            deal_id,
            view_mode: DealViewMode::default(),
        }
    }

    /// Set the view mode (COMMERCIAL, FINANCIAL, STATUS)
    pub fn with_view_mode(mut self, view_mode: DealViewMode) -> Self {
        self.view_mode = view_mode;
        self
    }

    /// Build the full deal graph
    pub async fn build(self, pool: &PgPool) -> Result<DealGraphResponse> {
        // Fetch deal summary
        let deal = DealRepository::get_deal_summary(pool, self.deal_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Deal not found: {}", self.deal_id))?;

        // Fetch related data based on view mode
        let (products, rate_cards, participants, contracts, onboarding_requests) = match self
            .view_mode
        {
            DealViewMode::Commercial => {
                // Full commercial view: products, participants, contracts
                let products = DealRepository::get_deal_products(pool, self.deal_id).await?;
                let participants =
                    DealRepository::get_deal_participants(pool, self.deal_id).await?;
                let contracts = DealRepository::get_deal_contracts(pool, self.deal_id).await?;
                let rate_cards = DealRepository::get_deal_rate_cards(pool, self.deal_id).await?;
                let onboarding_requests =
                    DealRepository::get_deal_onboarding_requests(pool, self.deal_id).await?;
                (
                    products,
                    rate_cards,
                    participants,
                    contracts,
                    onboarding_requests,
                )
            }
            DealViewMode::Financial => {
                // Financial view: products with rate cards focus
                let products = DealRepository::get_deal_products(pool, self.deal_id).await?;
                let rate_cards = DealRepository::get_deal_rate_cards(pool, self.deal_id).await?;
                let contracts = DealRepository::get_deal_contracts(pool, self.deal_id).await?;
                (products, rate_cards, vec![], contracts, vec![])
            }
            DealViewMode::Status => {
                // Status view: onboarding progress focus
                let products = DealRepository::get_deal_products(pool, self.deal_id).await?;
                let onboarding_requests =
                    DealRepository::get_deal_onboarding_requests(pool, self.deal_id).await?;
                (products, vec![], vec![], vec![], onboarding_requests)
            }
        };

        Ok(DealGraphResponse {
            deal,
            products,
            rate_cards,
            participants,
            contracts,
            onboarding_requests,
            view_mode: self.view_mode.to_string(),
        })
    }

    /// Build a minimal deal summary (for quick loading)
    pub async fn build_summary(pool: &PgPool, deal_id: Uuid) -> Result<Option<DealSummary>> {
        DealRepository::get_deal_summary(pool, deal_id).await
    }

    /// Get products for the deal
    pub async fn get_products(pool: &PgPool, deal_id: Uuid) -> Result<Vec<DealProductSummary>> {
        DealRepository::get_deal_products(pool, deal_id).await
    }

    /// Get rate cards for a specific product
    pub async fn get_product_rate_cards(
        pool: &PgPool,
        deal_id: Uuid,
        product_id: Uuid,
    ) -> Result<Vec<RateCardSummary>> {
        DealRepository::get_product_rate_cards(pool, deal_id, product_id).await
    }

    /// Get participants for the deal
    pub async fn get_participants(
        pool: &PgPool,
        deal_id: Uuid,
    ) -> Result<Vec<DealParticipantSummary>> {
        DealRepository::get_deal_participants(pool, deal_id).await
    }

    /// Get contracts for the deal
    pub async fn get_contracts(pool: &PgPool, deal_id: Uuid) -> Result<Vec<DealContractSummary>> {
        DealRepository::get_deal_contracts(pool, deal_id).await
    }

    /// Get onboarding requests for the deal
    pub async fn get_onboarding_requests(
        pool: &PgPool,
        deal_id: Uuid,
    ) -> Result<Vec<OnboardingRequestSummary>> {
        DealRepository::get_deal_onboarding_requests(pool, deal_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creation() {
        let deal_id = Uuid::new_v4();
        let builder = DealGraphBuilder::new(deal_id);
        assert_eq!(builder.deal_id, deal_id);
        assert_eq!(builder.view_mode, DealViewMode::Commercial);
    }

    #[test]
    fn test_builder_with_view_mode() {
        let deal_id = Uuid::new_v4();
        let builder = DealGraphBuilder::new(deal_id).with_view_mode(DealViewMode::Financial);
        assert_eq!(builder.view_mode, DealViewMode::Financial);
    }
}
