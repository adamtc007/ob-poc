//! Viewport Resolution Service
//!
//! Resolves viewport references (CbuRef, InstrumentMatrixRef, etc.) to concrete
//! database entities for lazy-loading during enhance/focus transitions.
//!
//! This service bridges the gap between:
//! - Viewport state (references with IDs/names)
//! - Resolved data (full entity details for rendering)
//!
//! ## Design Philosophy
//!
//! The viewport system uses lazy loading:
//! 1. `viewport.cbu` stores a CbuRef (just ID + optional name)
//! 2. On `viewport.enhance` or `viewport.focus`, we resolve to full data
//! 3. Resolved data can be cached in session for performance
//!
//! ## Confidence Zones
//!
//! Entity members include confidence scoring:
//! - Core (≥0.95): High-confidence verified entities
//! - Shell (≥0.70): Moderate confidence, may need verification
//! - Penumbra (≥0.40): Low confidence, requires investigation
//! - Speculative (<0.40): Very low confidence, flagged for review

use anyhow::Result;
use ob_poc_types::viewport::{
    CbuEntityMember, CbuRef, ConfidenceZone, InstrumentMatrixRef, InstrumentType, ResolutionError,
    ResolvedCbu, ResolvedInstrumentMatrix, ResolvedInstrumentType, ResolvedIsda, ResolvedMarket,
    ResolvedSsi,
};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::database::visualization_repository::{GraphEntityView, VisualizationRepository};

/// Service for resolving viewport references to concrete data
pub(crate) struct ViewportResolutionService {
    pool: PgPool,
}

impl ViewportResolutionService {
    /// Create a new resolution service with database pool
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Resolve a CBU reference to full CBU data
    ///
    /// Returns basic CBU information needed for viewport headers and context.
    pub(crate) async fn resolve_cbu(&self, cbu_ref: &CbuRef) -> Result<ResolvedCbu, ResolutionError> {
        let repo = VisualizationRepository::new(self.pool.clone());
        let cbu_id = cbu_ref.0;

        let cbu = repo
            .get_cbu_basic(cbu_id)
            .await
            .map_err(|e| ResolutionError::DatabaseError {
                message: e.to_string(),
            })?
            .ok_or(ResolutionError::CbuNotFound { cbu_id })?;

        Ok(ResolvedCbu {
            id: cbu.cbu_id,
            name: cbu.name,
            jurisdiction: cbu.jurisdiction,
            client_type: cbu.client_type,
        })
    }


    /// Resolve the instrument matrix (trading profile) for a CBU
    ///
    /// Returns the active trading profile with instrument types, markets, and currencies.
    /// Returns None if no trading profile exists for this CBU.
    pub(crate) async fn resolve_instrument_matrix(
        &self,
        cbu_id: Uuid,
    ) -> Result<Option<ResolvedInstrumentMatrix>, ResolutionError> {
        let repo = VisualizationRepository::new(self.pool.clone());

        // Get the active trading profile
        let profile = match repo.get_active_trading_profile(cbu_id).await {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(None),
            Err(e) => {
                return Err(ResolutionError::DatabaseError {
                    message: e.to_string(),
                })
            }
        };

        // Get universe entries for instrument types
        let universes =
            repo.get_universes(cbu_id)
                .await
                .map_err(|e| ResolutionError::DatabaseError {
                    message: e.to_string(),
                })?;

        // Group universe entries by instrument class
        let mut instrument_types: HashMap<String, ResolvedInstrumentType> = HashMap::new();

        for universe in universes {
            let class_name = universe.class_name.clone().unwrap_or_default();
            let class_code = class_name.to_uppercase().replace(' ', "_");

            let entry = instrument_types
                .entry(class_code.clone())
                .or_insert_with(|| {
                    let instrument_type = map_class_to_instrument_type(&class_name);
                    ResolvedInstrumentType {
                        instrument_type,
                        class_code: class_code.clone(),
                        class_name: class_name.clone(),
                        markets: Vec::new(),
                        is_otc: is_otc_class(&class_name),
                        currencies: Vec::new(),
                    }
                });

            // Add market if present
            if let Some(mic) = universe.mic.as_ref() {
                let market = ResolvedMarket {
                    mic: mic.clone(),
                    market_name: universe.market_name.clone(),
                    currencies: universe.currencies.clone(),
                    settlement_types: universe.settlement_types.clone(),
                };
                if !entry.markets.iter().any(|m| m.mic == market.mic) {
                    entry.markets.push(market);
                }
            }

            // Merge currencies
            for currency in &universe.currencies {
                if !entry.currencies.contains(currency) {
                    entry.currencies.push(currency.clone());
                }
            }
        }

        Ok(Some(ResolvedInstrumentMatrix {
            profile_id: profile.profile_id,
            version: profile.version,
            status: profile.status,
            instrument_types: instrument_types.into_values().collect(),
        }))
    }




}

/// Calculate entity confidence score based on available data
///
/// Heuristic scoring based on:
/// - Person state (VERIFIED > IDENTIFIED > GHOST)
/// - KYC obligation completion
/// - Role taxonomy data completeness
fn calculate_entity_confidence(entity: &GraphEntityView) -> f32 {
    let mut score: f32 = 0.50; // Base score

    // Person state (for natural persons)
    match entity.person_state.as_deref() {
        Some("VERIFIED") => score += 0.40,
        Some("IDENTIFIED") => score += 0.25,
        Some("GHOST") => score += 0.0, // Ghosts stay at base
        _ => score += 0.20,            // Non-person entities get moderate boost
    }

    // Has primary role assigned
    if entity.primary_role.is_some() {
        score += 0.05;
    }

    // Has role category (taxonomy completeness)
    if entity.primary_role_category.is_some() {
        score += 0.05;
    }

    // Clamp to valid range
    score.clamp(0.0, 1.0)
}

/// Map instrument class name to InstrumentType enum
///
/// Maps database instrument class names to the viewport's InstrumentType enum.
/// The viewport uses a simplified categorization for visualization purposes.
fn map_class_to_instrument_type(class_name: &str) -> InstrumentType {
    let normalized = class_name.to_uppercase();

    if normalized.contains("EQUITY") || normalized.contains("STOCK") {
        InstrumentType::Equity
    } else if normalized.contains("BOND") || normalized.contains("FIXED") {
        InstrumentType::FixedIncome
    } else if normalized.contains("DERIVATIVE")
        || normalized.contains("IRS")
        || normalized.contains("SWAP")
        || normalized.contains("CDS")
        || normalized.contains("OPTION")
        || normalized.contains("FUTURE")
    {
        InstrumentType::Derivative
    } else if normalized.contains("FX") || normalized.contains("FOREX") {
        InstrumentType::Fx
    } else if normalized.contains("FUND")
        || normalized.contains("ETF")
        || normalized.contains("COLLECTIVE")
    {
        InstrumentType::Fund
    } else if normalized.contains("CASH") || normalized.contains("MONEY MARKET") {
        InstrumentType::Cash
    } else if normalized.contains("COMMODITY") {
        InstrumentType::Commodity
    } else if normalized.contains("STRUCTURED") {
        InstrumentType::StructuredProduct
    } else {
        // Default to Equity for unknown types
        InstrumentType::Equity
    }
}

/// Check if an instrument class is OTC-traded
fn is_otc_class(class_name: &str) -> bool {
    let normalized = class_name.to_uppercase();
    normalized.contains("OTC")
        || normalized.contains("IRS")
        || normalized.contains("CDS")
        || normalized.contains("SWAP")
        || normalized.contains("FX FORWARD")
        || normalized.contains("OPTION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_class_to_instrument_type() {
        // Equity types
        assert_eq!(
            map_class_to_instrument_type("EQUITY"),
            InstrumentType::Equity
        );
        assert_eq!(
            map_class_to_instrument_type("Common Stock"),
            InstrumentType::Equity
        );

        // Fixed income types
        assert_eq!(
            map_class_to_instrument_type("Govt Bond"),
            InstrumentType::FixedIncome
        );
        assert_eq!(
            map_class_to_instrument_type("CORP_BOND"),
            InstrumentType::FixedIncome
        );
        assert_eq!(
            map_class_to_instrument_type("Fixed Income"),
            InstrumentType::FixedIncome
        );

        // Fund types (including ETFs)
        assert_eq!(map_class_to_instrument_type("ETF"), InstrumentType::Fund);
        assert_eq!(
            map_class_to_instrument_type("Mutual Fund"),
            InstrumentType::Fund
        );

        // Derivative types
        assert_eq!(
            map_class_to_instrument_type("OTC_IRS"),
            InstrumentType::Derivative
        );
        assert_eq!(
            map_class_to_instrument_type("Interest Rate Swap"),
            InstrumentType::Derivative
        );
        assert_eq!(
            map_class_to_instrument_type("CDS"),
            InstrumentType::Derivative
        );

        // FX types
        assert_eq!(
            map_class_to_instrument_type("FX Forward"),
            InstrumentType::Fx
        );
        assert_eq!(map_class_to_instrument_type("FOREX"), InstrumentType::Fx);

        // Cash and commodity
        assert_eq!(map_class_to_instrument_type("Cash"), InstrumentType::Cash);
        assert_eq!(
            map_class_to_instrument_type("Commodity"),
            InstrumentType::Commodity
        );
    }

    #[test]
    fn test_is_otc_class() {
        assert!(is_otc_class("OTC_IRS"));
        assert!(is_otc_class("Interest Rate Swap"));
        assert!(is_otc_class("CDS"));
        assert!(!is_otc_class("EQUITY"));
        assert!(!is_otc_class("ETF"));
    }
}
