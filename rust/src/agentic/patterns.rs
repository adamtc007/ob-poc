//! Onboarding pattern classification
//!
//! Patterns represent the complexity level of an onboarding request.
//! Classification is deterministic based on intent fields.

use serde::{Deserialize, Serialize};

/// Deterministic classification of onboarding complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OnboardingPattern {
    /// Single market, single currency, no OTC
    /// Example: "US equity trading only"
    SimpleEquity,

    /// Multiple markets and/or cross-currency
    /// Example: "US, UK, Germany with USD cross-currency"
    MultiMarket,

    /// Includes OTC derivatives with ISDA/CSA
    /// Example: "Global equities plus IRS with Morgan Stanley"
    WithOtc,
}

impl OnboardingPattern {
    /// Domains required for this pattern
    pub fn required_domains(&self) -> Vec<&'static str> {
        match self {
            Self::SimpleEquity => vec!["cbu", "cbu-custody"],
            Self::MultiMarket => vec!["cbu", "cbu-custody"],
            Self::WithOtc => vec!["cbu", "cbu-custody", "isda", "entity"],
        }
    }

    /// Get the example DSL for this pattern
    pub fn example_dsl(&self) -> &'static str {
        match self {
            Self::SimpleEquity => include_str!("examples/simple_equity.dsl"),
            Self::MultiMarket => include_str!("examples/multi_market.dsl"),
            Self::WithOtc => include_str!("examples/with_otc.dsl"),
        }
    }

    /// Minimum expected DSL statements for this pattern
    pub fn expected_statement_count(&self) -> usize {
        match self {
            Self::SimpleEquity => 6, // CBU + universe + SSI + activate + 2 rules
            Self::MultiMarket => 15, // CBU + N×(universe + SSI) + rules + fallbacks
            Self::WithOtc => 25,     // Above + entity + ISDA + coverage + CSA
        }
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::SimpleEquity => "Single market, single currency equity trading",
            Self::MultiMarket => "Multi-market or cross-currency trading",
            Self::WithOtc => "Trading with OTC derivatives (requires ISDA/CSA)",
        }
    }

    /// Pattern name for display
    pub fn name(&self) -> &'static str {
        match self {
            Self::SimpleEquity => "SimpleEquity",
            Self::MultiMarket => "MultiMarket",
            Self::WithOtc => "WithOtc",
        }
    }
}

impl std::fmt::Display for OnboardingPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_domains() {
        assert_eq!(
            OnboardingPattern::SimpleEquity.required_domains(),
            vec!["cbu", "cbu-custody"]
        );
        assert_eq!(
            OnboardingPattern::MultiMarket.required_domains(),
            vec!["cbu", "cbu-custody"]
        );
        assert!(OnboardingPattern::WithOtc
            .required_domains()
            .contains(&"isda"));
    }

    #[test]
    fn test_expected_counts() {
        assert!(OnboardingPattern::SimpleEquity.expected_statement_count() < 10);
        assert!(OnboardingPattern::MultiMarket.expected_statement_count() >= 10);
        assert!(OnboardingPattern::WithOtc.expected_statement_count() >= 20);
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", OnboardingPattern::SimpleEquity),
            "SimpleEquity"
        );
        assert_eq!(format!("{}", OnboardingPattern::WithOtc), "WithOtc");
    }
}
