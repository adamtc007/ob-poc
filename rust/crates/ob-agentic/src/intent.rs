//! Intent structures for onboarding requests
//!
//! These structures represent the extracted intent from natural language requests.

use serde::{Deserialize, Serialize};

use crate::patterns::OnboardingPattern;

/// Result of intent extraction - either clear intent or needs clarification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntentResult {
    /// Ambiguous input - needs user clarification before proceeding
    NeedsClarification(ClarificationRequest),
    /// Clear intent - ready for DSL generation
    Clear(OnboardingIntent),
}

/// Request for user clarification when input is ambiguous
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationRequest {
    /// Flag indicating this needs clarification
    pub needs_clarification: bool,
    /// Details about the ambiguity
    pub ambiguity: AmbiguityDetails,
}

/// Details about an ambiguous input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbiguityDetails {
    /// Original user text
    pub original_text: String,
    /// Possible interpretations
    pub interpretations: Vec<Interpretation>,
    /// Question to ask the user
    pub question: String,
}

/// One possible interpretation of ambiguous input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    /// Option number (1, 2, etc.)
    pub option: u8,
    /// Extracted name under this interpretation
    pub name: String,
    /// Extracted jurisdiction under this interpretation
    pub jurisdiction: Option<String>,
    /// Human-readable description
    pub description: String,
}

impl IntentResult {
    /// Check if this result needs clarification
    pub fn needs_clarification(&self) -> bool {
        matches!(self, IntentResult::NeedsClarification(_))
    }

    /// Get the clarification request if present
    pub fn as_clarification(&self) -> Option<&ClarificationRequest> {
        match self {
            IntentResult::NeedsClarification(c) => Some(c),
            _ => None,
        }
    }

    /// Get the intent if clear
    pub fn as_intent(&self) -> Option<&OnboardingIntent> {
        match self {
            IntentResult::Clear(i) => Some(i),
            _ => None,
        }
    }
}

/// Structured representation of user's onboarding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingIntent {
    /// The client being onboarded
    pub client: ClientIntent,

    /// Instruments they will trade
    pub instruments: Vec<InstrumentIntent>,

    /// Markets they will access
    pub markets: Vec<MarketIntent>,

    /// OTC counterparty relationships
    pub otc_counterparties: Vec<CounterpartyIntent>,

    /// Explicit requirements mentioned by user
    pub explicit_requirements: Vec<String>,

    /// Original natural language request
    pub original_request: String,
}

/// Client information extracted from the request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientIntent {
    /// Client name
    pub name: String,

    /// Entity type: fund, corporate, individual
    pub entity_type: Option<String>,

    /// Jurisdiction: US, LU, IE, etc.
    pub jurisdiction: Option<String>,
}

/// Market access information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIntent {
    /// Market code (MIC): XNYS, XLON, XFRA
    pub market_code: String,

    /// Currencies for this market: USD, GBP, EUR
    pub currencies: Vec<String>,

    /// Settlement types: DVP, FOP (default: DVP)
    pub settlement_types: Vec<String>,
}

/// Instrument class information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentIntent {
    /// Instrument class: EQUITY, GOVT_BOND, OTC_IRS
    pub class: String,

    /// Specific types: ADR, ETF (optional refinement)
    pub specific_types: Vec<String>,
}

/// OTC counterparty relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartyIntent {
    /// Counterparty name
    pub name: String,

    /// Instruments traded with this counterparty
    pub instruments: Vec<String>,

    /// Governing law: NY, ENGLISH
    pub governing_law: Option<String>,

    /// Whether CSA is required (margin/collateral)
    pub csa_required: bool,
}

impl OnboardingIntent {
    /// Classify this intent into an onboarding pattern
    pub fn classify(&self) -> OnboardingPattern {
        let has_otc = !self.otc_counterparties.is_empty();
        let market_count = self.markets.len();
        let has_cross_currency = self.markets.iter().any(|m| m.currencies.len() > 1);

        match (has_otc, market_count, has_cross_currency) {
            (true, _, _) => OnboardingPattern::WithOtc,
            (false, 1, false) => OnboardingPattern::SimpleEquity,
            (false, _, _) => OnboardingPattern::MultiMarket,
        }
    }

    /// Check if this intent involves OTC instruments
    pub fn has_otc_instruments(&self) -> bool {
        self.instruments.iter().any(|i| i.class.starts_with("OTC_"))
    }

    /// Get all unique currencies across all markets
    pub fn all_currencies(&self) -> Vec<String> {
        let mut currencies: Vec<String> = self
            .markets
            .iter()
            .flat_map(|m| m.currencies.iter().cloned())
            .collect();
        currencies.sort();
        currencies.dedup();
        currencies
    }
}

impl Default for MarketIntent {
    fn default() -> Self {
        Self {
            market_code: String::new(),
            currencies: vec!["USD".to_string()],
            settlement_types: vec!["DVP".to_string()],
        }
    }
}

impl Default for CounterpartyIntent {
    fn default() -> Self {
        Self {
            name: String::new(),
            instruments: Vec::new(),
            governing_law: Some("NY".to_string()),
            csa_required: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_equity_intent() -> OnboardingIntent {
        OnboardingIntent {
            client: ClientIntent {
                name: "Test Fund".to_string(),
                entity_type: Some("fund".to_string()),
                jurisdiction: Some("US".to_string()),
            },
            instruments: vec![InstrumentIntent {
                class: "EQUITY".to_string(),
                specific_types: vec![],
            }],
            markets: vec![MarketIntent {
                market_code: "XNYS".to_string(),
                currencies: vec!["USD".to_string()],
                settlement_types: vec!["DVP".to_string()],
            }],
            otc_counterparties: vec![],
            explicit_requirements: vec![],
            original_request: "Set up Test Fund for US equities".to_string(),
        }
    }

    #[test]
    fn test_simple_equity_classification() {
        let intent = simple_equity_intent();
        assert_eq!(intent.classify(), OnboardingPattern::SimpleEquity);
    }

    #[test]
    fn test_multi_market_classification() {
        let mut intent = simple_equity_intent();
        intent.markets.push(MarketIntent {
            market_code: "XLON".to_string(),
            currencies: vec!["GBP".to_string()],
            settlement_types: vec!["DVP".to_string()],
        });
        assert_eq!(intent.classify(), OnboardingPattern::MultiMarket);
    }

    #[test]
    fn test_cross_currency_is_multi_market() {
        let mut intent = simple_equity_intent();
        intent.markets[0].currencies = vec!["USD".to_string(), "EUR".to_string()];
        assert_eq!(intent.classify(), OnboardingPattern::MultiMarket);
    }

    #[test]
    fn test_otc_classification() {
        let mut intent = simple_equity_intent();
        intent.otc_counterparties.push(CounterpartyIntent {
            name: "Morgan Stanley".to_string(),
            instruments: vec!["OTC_IRS".to_string()],
            governing_law: Some("NY".to_string()),
            csa_required: true,
        });
        assert_eq!(intent.classify(), OnboardingPattern::WithOtc);
    }

    #[test]
    fn test_all_currencies() {
        let mut intent = simple_equity_intent();
        intent.markets.push(MarketIntent {
            market_code: "XLON".to_string(),
            currencies: vec!["GBP".to_string(), "USD".to_string()],
            settlement_types: vec!["DVP".to_string()],
        });
        let currencies = intent.all_currencies();
        assert_eq!(currencies, vec!["GBP", "USD"]);
    }

    #[test]
    fn test_has_otc_instruments() {
        let mut intent = simple_equity_intent();
        assert!(!intent.has_otc_instruments());

        intent.instruments.push(InstrumentIntent {
            class: "OTC_IRS".to_string(),
            specific_types: vec![],
        });
        assert!(intent.has_otc_instruments());
    }
}
