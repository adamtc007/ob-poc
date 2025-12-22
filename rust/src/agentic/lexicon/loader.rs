//! Lexicon configuration and loading.
//!
//! The lexicon is a YAML-driven dictionary that maps words to token types.
//! This allows adding new vocabulary without changing code.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::tokens::{EntityClass, ModifierType, PrepType, TokenType, VerbClass};

/// Root configuration structure for the lexicon YAML file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LexiconConfig {
    /// Action verbs grouped by semantic class.
    #[serde(default)]
    pub verbs: VerbsConfig,

    /// Entity type indicators.
    #[serde(default)]
    pub entities: EntitiesConfig,

    /// Role names.
    #[serde(default)]
    pub roles: Vec<String>,

    /// Product codes.
    #[serde(default)]
    pub products: Vec<String>,

    /// Instrument types (flat list - not hierarchical).
    #[serde(default)]
    pub instruments: InstrumentsConfig,

    /// Market identifiers (MIC codes).
    #[serde(default)]
    pub markets: Vec<String>,

    /// Currency codes.
    #[serde(default)]
    pub currencies: Vec<String>,

    /// CSA types.
    #[serde(default)]
    pub csa_types: Vec<String>,

    /// Governing laws.
    #[serde(default)]
    pub laws: Vec<LawEntry>,

    /// Prepositions.
    #[serde(default)]
    pub prepositions: PrepositionsConfig,

    /// Conjunctions.
    #[serde(default)]
    pub conjunctions: Vec<String>,

    /// Articles.
    #[serde(default)]
    pub articles: Vec<String>,

    /// Pronouns for coreference.
    #[serde(default)]
    pub pronouns: Vec<String>,

    /// Modifiers grouped by type.
    #[serde(default)]
    pub modifiers: ModifiersConfig,
}

/// Verbs grouped by semantic class.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VerbsConfig {
    #[serde(default)]
    pub create: Vec<String>,
    #[serde(default)]
    pub update: Vec<String>,
    #[serde(default)]
    pub delete: Vec<String>,
    #[serde(default)]
    pub query: Vec<String>,
    #[serde(default)]
    pub link: Vec<String>,
    #[serde(default)]
    pub unlink: Vec<String>,
    #[serde(default)]
    pub provision: Vec<String>,
    #[serde(default)]
    pub trade: Vec<String>,
}

/// Entity type indicators.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EntitiesConfig {
    #[serde(default)]
    pub cbu: Vec<String>,
    #[serde(default)]
    pub person: Vec<String>,
    #[serde(default)]
    pub legal_entity: Vec<String>,
    #[serde(default)]
    pub counterparty: Vec<String>,
    #[serde(default)]
    pub isda: Vec<String>,
    #[serde(default)]
    pub csa: Vec<String>,
}

/// Instruments grouped by lifecycle domain.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstrumentsConfig {
    /// OTC derivatives (ISDA/CSA lifecycle).
    #[serde(default)]
    pub otc: Vec<String>,

    /// Exchange-traded securities (SSI/booking rules lifecycle).
    #[serde(default)]
    pub exchange_traded: Vec<String>,
}

/// Law entry with canonical name and aliases.
#[derive(Debug, Clone, Deserialize)]
pub struct LawEntry {
    pub code: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Prepositions grouped by semantic role.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PrepositionsConfig {
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub for_: Vec<String>,
    #[serde(default)]
    pub as_: Vec<String>,
    #[serde(default)]
    pub with: Vec<String>,
    #[serde(default)]
    pub under: Vec<String>,
    #[serde(default)]
    pub from: Vec<String>,
    #[serde(default)]
    pub in_: Vec<String>,
    #[serde(default)]
    pub on: Vec<String>,
    #[serde(default)]
    pub by: Vec<String>,
    #[serde(default)]
    pub of: Vec<String>,
    #[serde(default)]
    pub at: Vec<String>,
}

/// Modifiers grouped by type.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModifiersConfig {
    #[serde(default)]
    pub temporal: Vec<String>,
    #[serde(default)]
    pub lateral: Vec<String>,
    #[serde(default)]
    pub otc_qualifier: Vec<String>,
    #[serde(default)]
    pub risk: Vec<String>,
    #[serde(default)]
    pub status: Vec<String>,
}

/// Entry in the compiled lexicon lookup table.
#[derive(Debug, Clone)]
pub struct LexiconEntry {
    /// The token type for this entry.
    pub token_type: TokenType,

    /// Canonical form (for normalization).
    pub canonical: String,

    /// Which lifecycle domain this term belongs to (for domain detection).
    pub domain_hint: Option<LifecycleDomain>,
}

/// Lifecycle domain for domain detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleDomain {
    /// OTC derivatives: ISDA, CSA, counterparty, collateral.
    Otc,

    /// Exchange-traded: SSI, booking rules, custody, SWIFT.
    ExchangeTraded,
}

/// Compiled lexicon with O(1) lookup.
#[derive(Debug, Clone)]
pub struct Lexicon {
    /// Main lookup table: normalized word → entry.
    lookup: HashMap<String, LexiconEntry>,

    /// Multi-word phrase lookup (e.g., "interest rate swap").
    phrases: HashMap<String, LexiconEntry>,

    /// OTC domain keywords for quick domain detection.
    otc_keywords: Vec<String>,

    /// Exchange-traded domain keywords.
    exchange_keywords: Vec<String>,
}

impl Lexicon {
    /// Load lexicon from a YAML file.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content =
            std::fs::read_to_string(path.as_ref()).context("Failed to read lexicon file")?;

        let config: LexiconConfig =
            serde_yaml::from_str(&content).context("Failed to parse lexicon YAML")?;

        Self::from_config(config)
    }

    /// Build lexicon from configuration.
    pub fn from_config(config: LexiconConfig) -> Result<Self> {
        let mut lookup = HashMap::new();
        let mut phrases = HashMap::new();
        let mut otc_keywords = Vec::new();
        let mut exchange_keywords = Vec::new();

        // Helper to add entries
        let mut add_entry =
            |word: &str, token_type: TokenType, domain_hint: Option<LifecycleDomain>| {
                let normalized = word.to_lowercase();
                let entry = LexiconEntry {
                    token_type,
                    canonical: word.to_string(),
                    domain_hint,
                };

                if normalized.contains(' ') {
                    phrases.insert(normalized, entry);
                } else {
                    lookup.insert(normalized, entry);
                }
            };

        // Add verbs
        for verb in &config.verbs.create {
            add_entry(verb, TokenType::Verb(VerbClass::Create), None);
        }
        for verb in &config.verbs.update {
            add_entry(verb, TokenType::Verb(VerbClass::Update), None);
        }
        for verb in &config.verbs.delete {
            add_entry(verb, TokenType::Verb(VerbClass::Delete), None);
        }
        for verb in &config.verbs.query {
            add_entry(verb, TokenType::Verb(VerbClass::Query), None);
        }
        for verb in &config.verbs.link {
            add_entry(verb, TokenType::Verb(VerbClass::Link), None);
        }
        for verb in &config.verbs.unlink {
            add_entry(verb, TokenType::Verb(VerbClass::Unlink), None);
        }
        for verb in &config.verbs.provision {
            add_entry(verb, TokenType::Verb(VerbClass::Provision), None);
        }
        for verb in &config.verbs.trade {
            add_entry(verb, TokenType::Verb(VerbClass::Trade), None);
        }

        // Add entity indicators
        for word in &config.entities.cbu {
            add_entry(word, TokenType::Entity(EntityClass::Cbu), None);
        }
        for word in &config.entities.person {
            add_entry(word, TokenType::Entity(EntityClass::Person), None);
        }
        for word in &config.entities.legal_entity {
            add_entry(word, TokenType::Entity(EntityClass::LegalEntity), None);
        }
        for word in &config.entities.counterparty {
            add_entry(
                word,
                TokenType::Entity(EntityClass::Counterparty),
                Some(LifecycleDomain::Otc),
            );
            otc_keywords.push(word.to_lowercase());
        }
        for word in &config.entities.isda {
            add_entry(
                word,
                TokenType::Entity(EntityClass::Isda),
                Some(LifecycleDomain::Otc),
            );
            otc_keywords.push(word.to_lowercase());
        }
        for word in &config.entities.csa {
            add_entry(
                word,
                TokenType::Entity(EntityClass::Csa),
                Some(LifecycleDomain::Otc),
            );
            otc_keywords.push(word.to_lowercase());
        }

        // Add roles
        for role in &config.roles {
            add_entry(role, TokenType::Role, None);
        }

        // Add products
        for product in &config.products {
            add_entry(product, TokenType::Product, None);
        }

        // Add instruments with domain hints
        for instrument in &config.instruments.otc {
            add_entry(
                instrument,
                TokenType::Instrument,
                Some(LifecycleDomain::Otc),
            );
            otc_keywords.push(instrument.to_lowercase());
        }
        for instrument in &config.instruments.exchange_traded {
            add_entry(
                instrument,
                TokenType::Instrument,
                Some(LifecycleDomain::ExchangeTraded),
            );
            exchange_keywords.push(instrument.to_lowercase());
        }

        // Add markets (exchange-traded domain)
        for market in &config.markets {
            add_entry(
                market,
                TokenType::Market,
                Some(LifecycleDomain::ExchangeTraded),
            );
            exchange_keywords.push(market.to_lowercase());
        }

        // Add currencies
        for currency in &config.currencies {
            add_entry(currency, TokenType::Currency, None);
        }

        // Add CSA types (OTC domain)
        for csa_type in &config.csa_types {
            add_entry(csa_type, TokenType::CsaType, Some(LifecycleDomain::Otc));
            otc_keywords.push(csa_type.to_lowercase());
        }

        // Add laws (OTC domain)
        for law in &config.laws {
            add_entry(&law.code, TokenType::Law, Some(LifecycleDomain::Otc));
            for alias in &law.aliases {
                add_entry(alias, TokenType::Law, Some(LifecycleDomain::Otc));
            }
        }

        // Add prepositions
        for word in &config.prepositions.to {
            add_entry(word, TokenType::Prep(PrepType::To), None);
        }
        for word in &config.prepositions.for_ {
            add_entry(word, TokenType::Prep(PrepType::For), None);
        }
        for word in &config.prepositions.as_ {
            add_entry(word, TokenType::Prep(PrepType::As), None);
        }
        for word in &config.prepositions.with {
            add_entry(word, TokenType::Prep(PrepType::With), None);
        }
        for word in &config.prepositions.under {
            add_entry(word, TokenType::Prep(PrepType::Under), None);
        }
        for word in &config.prepositions.from {
            add_entry(word, TokenType::Prep(PrepType::From), None);
        }
        for word in &config.prepositions.in_ {
            add_entry(word, TokenType::Prep(PrepType::In), None);
        }
        for word in &config.prepositions.on {
            add_entry(word, TokenType::Prep(PrepType::On), None);
        }
        for word in &config.prepositions.by {
            add_entry(word, TokenType::Prep(PrepType::By), None);
        }
        for word in &config.prepositions.of {
            add_entry(word, TokenType::Prep(PrepType::Of), None);
        }
        for word in &config.prepositions.at {
            add_entry(word, TokenType::Prep(PrepType::At), None);
        }

        // Add conjunctions
        for word in &config.conjunctions {
            add_entry(word, TokenType::Conj, None);
        }

        // Add articles
        for word in &config.articles {
            add_entry(word, TokenType::Article, None);
        }

        // Add pronouns
        for word in &config.pronouns {
            add_entry(word, TokenType::Pronoun, None);
        }

        // Add modifiers
        for word in &config.modifiers.temporal {
            add_entry(word, TokenType::Modifier(ModifierType::Temporal), None);
        }
        for word in &config.modifiers.lateral {
            add_entry(word, TokenType::Modifier(ModifierType::Lateral), None);
        }
        for word in &config.modifiers.otc_qualifier {
            add_entry(
                word,
                TokenType::Modifier(ModifierType::OtcQualifier),
                Some(LifecycleDomain::Otc),
            );
            otc_keywords.push(word.to_lowercase());
        }
        for word in &config.modifiers.risk {
            add_entry(word, TokenType::Modifier(ModifierType::Risk), None);
        }
        for word in &config.modifiers.status {
            add_entry(word, TokenType::Modifier(ModifierType::Status), None);
        }

        Ok(Self {
            lookup,
            phrases,
            otc_keywords,
            exchange_keywords,
        })
    }

    /// Look up a single word in the lexicon.
    pub fn lookup_word(&self, word: &str) -> Option<&LexiconEntry> {
        self.lookup.get(&word.to_lowercase())
    }

    /// Look up a multi-word phrase.
    pub fn lookup_phrase(&self, phrase: &str) -> Option<&LexiconEntry> {
        self.phrases.get(&phrase.to_lowercase())
    }

    /// Check if a word exists in the lexicon.
    pub fn contains(&self, word: &str) -> bool {
        self.lookup.contains_key(&word.to_lowercase())
    }

    /// Get all entries in the lexicon.
    pub fn entries(&self) -> impl Iterator<Item = (&String, &LexiconEntry)> {
        self.lookup.iter().chain(self.phrases.iter())
    }

    /// Get the number of entries in the lexicon.
    pub fn len(&self) -> usize {
        self.lookup.len() + self.phrases.len()
    }

    /// Check if the lexicon is empty.
    pub fn is_empty(&self) -> bool {
        self.lookup.is_empty() && self.phrases.is_empty()
    }

    /// Detect the primary lifecycle domain from text.
    pub fn detect_domain(&self, text: &str) -> Option<LifecycleDomain> {
        let text_lower = text.to_lowercase();

        let otc_count = self
            .otc_keywords
            .iter()
            .filter(|kw| text_lower.contains(kw.as_str()))
            .count();

        let exchange_count = self
            .exchange_keywords
            .iter()
            .filter(|kw| text_lower.contains(kw.as_str()))
            .count();

        if otc_count > exchange_count && otc_count > 0 {
            Some(LifecycleDomain::Otc)
        } else if exchange_count > otc_count && exchange_count > 0 {
            Some(LifecycleDomain::ExchangeTraded)
        } else {
            None
        }
    }

    /// Get OTC keywords for domain detection.
    pub fn otc_keywords(&self) -> &[String] {
        &self.otc_keywords
    }

    /// Get exchange-traded keywords for domain detection.
    pub fn exchange_keywords(&self) -> &[String] {
        &self.exchange_keywords
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LexiconConfig {
        LexiconConfig {
            verbs: VerbsConfig {
                create: vec!["add".to_string(), "create".to_string()],
                link: vec!["assign".to_string()],
                ..Default::default()
            },
            entities: EntitiesConfig {
                counterparty: vec!["counterparty".to_string()],
                isda: vec!["isda".to_string()],
                ..Default::default()
            },
            instruments: InstrumentsConfig {
                otc: vec!["irs".to_string(), "cds".to_string()],
                exchange_traded: vec!["equity".to_string(), "bond".to_string()],
            },
            prepositions: PrepositionsConfig {
                as_: vec!["as".to_string()],
                under: vec!["under".to_string()],
                ..Default::default()
            },
            laws: vec![LawEntry {
                code: "NY_LAW".to_string(),
                aliases: vec!["new york law".to_string(), "ny law".to_string()],
            }],
            articles: vec!["a".to_string(), "an".to_string(), "the".to_string()],
            ..Default::default()
        }
    }

    #[test]
    fn test_lexicon_lookup() {
        let lexicon = Lexicon::from_config(test_config()).unwrap();

        // Verb lookup
        let entry = lexicon.lookup_word("add").unwrap();
        assert!(matches!(
            entry.token_type,
            TokenType::Verb(VerbClass::Create)
        ));

        // Instrument lookup
        let entry = lexicon.lookup_word("IRS").unwrap();
        assert!(matches!(entry.token_type, TokenType::Instrument));
        assert_eq!(entry.domain_hint, Some(LifecycleDomain::Otc));

        // Article lookup
        let entry = lexicon.lookup_word("a").unwrap();
        assert!(matches!(entry.token_type, TokenType::Article));
    }

    #[test]
    fn test_phrase_lookup() {
        let lexicon = Lexicon::from_config(test_config()).unwrap();

        let entry = lexicon.lookup_phrase("new york law").unwrap();
        assert!(matches!(entry.token_type, TokenType::Law));
    }

    #[test]
    fn test_domain_detection() {
        let lexicon = Lexicon::from_config(test_config()).unwrap();

        // OTC domain
        let domain = lexicon.detect_domain("Add Goldman Sachs as counterparty for IRS trades");
        assert_eq!(domain, Some(LifecycleDomain::Otc));

        // Exchange-traded domain
        let domain = lexicon.detect_domain("Set up equity trading in US markets");
        assert_eq!(domain, Some(LifecycleDomain::ExchangeTraded));

        // Ambiguous
        let domain = lexicon.detect_domain("Hello world");
        assert_eq!(domain, None);
    }

    #[test]
    fn test_case_insensitive() {
        let lexicon = Lexicon::from_config(test_config()).unwrap();

        assert!(lexicon.lookup_word("ADD").is_some());
        assert!(lexicon.lookup_word("Add").is_some());
        assert!(lexicon.lookup_word("add").is_some());
    }
}
