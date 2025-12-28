//! Nom-based intent grammar parser.
//!
//! This module parses a token stream into IntentAst using nom combinators.
//! The grammar is designed to handle natural language variation while
//! producing deterministic, typed AST nodes.

use nom::{
    branch::alt,
    combinator::opt,
    multi::separated_list1,
    sequence::{preceded, tuple},
    IResult,
};

use super::intent_ast::{
    CsaType, CurrencyCode, EntityRef, GoverningLaw, InstrumentCode, IntentAst, MarketCode, RoleCode,
};
use super::tokenizer::ResolvedEntity;
use super::tokens::{EntityClass, PrepType, Token, TokenType, VerbClass};

/// Parser input: a slice of tokens.
type Input<'a> = &'a [Token];

/// Parser result type.
type ParseResult<'a, T> = IResult<Input<'a>, T>;

// ============================================================================
// Token Matchers
// ============================================================================

/// Match a token with a specific type.
fn token_type(expected: TokenType) -> impl Fn(Input) -> ParseResult<&Token> {
    move |input: Input| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Eof,
            )));
        }

        if input[0].token_type == expected {
            Ok((&input[1..], &input[0]))
        } else {
            Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )))
        }
    }
}

/// Match a verb of a specific class.
fn verb_class(expected: VerbClass) -> impl Fn(Input) -> ParseResult<&Token> {
    move |input: Input| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Eof,
            )));
        }

        if let TokenType::Verb(class) = &input[0].token_type {
            if *class == expected {
                return Ok((&input[1..], &input[0]));
            }
        }

        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match any entity token.
fn any_entity(input: Input) -> ParseResult<EntityRef> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if let TokenType::Entity(class) = &input[0].token_type {
        let entity_ref = if let Some(ref id) = input[0].resolved_id {
            EntityRef::Resolved(ResolvedEntity {
                id: id.clone(),
                name: input[0].text.clone(),
                entity_type: class.type_code().to_string(),
                confidence: input[0].confidence,
            })
        } else {
            EntityRef::Unresolved {
                name: input[0].text.clone(),
                entity_type: Some(class.type_code().to_string()),
            }
        };

        Ok((&input[1..], entity_ref))
    } else if input[0].token_type == TokenType::Pronoun {
        // Handle pronoun as entity reference
        let entity_ref = if let Some(ref id) = input[0].resolved_id {
            EntityRef::Pronoun {
                text: input[0].text.clone(),
                referent: Some(ResolvedEntity {
                    id: id.clone(),
                    name: input[0].text.clone(),
                    entity_type: "entity".to_string(),
                    confidence: input[0].confidence,
                }),
            }
        } else {
            EntityRef::Pronoun {
                text: input[0].text.clone(),
                referent: None,
            }
        };

        Ok((&input[1..], entity_ref))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match an entity of a specific class.
fn entity_class(expected: EntityClass) -> impl Fn(Input) -> ParseResult<EntityRef> {
    move |input: Input| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Eof,
            )));
        }

        if let TokenType::Entity(class) = &input[0].token_type {
            if *class == expected {
                let entity_ref = if let Some(ref id) = input[0].resolved_id {
                    EntityRef::Resolved(ResolvedEntity {
                        id: id.clone(),
                        name: input[0].text.clone(),
                        entity_type: class.type_code().to_string(),
                        confidence: input[0].confidence,
                    })
                } else {
                    EntityRef::Unresolved {
                        name: input[0].text.clone(),
                        entity_type: Some(class.type_code().to_string()),
                    }
                };

                return Ok((&input[1..], entity_ref));
            }
        }

        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match a preposition of a specific type.
fn prep(expected: PrepType) -> impl Fn(Input) -> ParseResult<&Token> {
    move |input: Input| {
        if input.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Eof,
            )));
        }

        if let TokenType::Prep(p) = &input[0].token_type {
            if *p == expected {
                return Ok((&input[1..], &input[0]));
            }
        }

        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match an instrument token.
fn instrument(input: Input) -> ParseResult<InstrumentCode> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if input[0].token_type == TokenType::Instrument {
        Ok((&input[1..], InstrumentCode::new(&input[0].normalized)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match a market token.
fn market(input: Input) -> ParseResult<MarketCode> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if input[0].token_type == TokenType::Market {
        Ok((&input[1..], MarketCode::new(&input[0].normalized)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match a currency token.
fn currency(input: Input) -> ParseResult<CurrencyCode> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if input[0].token_type == TokenType::Currency {
        Ok((&input[1..], CurrencyCode::new(&input[0].normalized)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match a role token.
fn role(input: Input) -> ParseResult<RoleCode> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if input[0].token_type == TokenType::Role {
        Ok((&input[1..], RoleCode::new(&input[0].normalized)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

/// Match a governing law token.
fn law(input: Input) -> ParseResult<GoverningLaw> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if input[0].token_type == TokenType::Law {
        if let Some(law) = GoverningLaw::parse(&input[0].text) {
            return Ok((&input[1..], law));
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Tag,
    )))
}

/// Match a CSA type token.
fn csa_type(input: Input) -> ParseResult<CsaType> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    if input[0].token_type == TokenType::CsaType {
        if let Some(csa) = CsaType::parse(&input[0].text) {
            return Ok((&input[1..], csa));
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Tag,
    )))
}

/// Skip optional articles and unknown words (but not conjunctions - they're structural).
fn skip_noise(input: Input) -> ParseResult<()> {
    let mut remaining = input;

    while !remaining.is_empty() {
        match &remaining[0].token_type {
            TokenType::Article | TokenType::Unknown => {
                remaining = &remaining[1..];
            }
            _ => break,
        }
    }

    Ok((remaining, ()))
}

// ============================================================================
// Intent Parsers
// ============================================================================

/// Parse: "add {entity} as counterparty [for {instruments}] [under {law}]"
fn parse_counterparty_create(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, counterparty) = any_entity(input)?;
    let (input, _) = skip_noise(input)?;

    // Optional "as counterparty"
    let (input, _) = opt(tuple((
        prep(PrepType::As),
        skip_noise,
        token_type(TokenType::Role),
    )))(input)?;

    // Optional instruments
    let (input, instruments) = opt(preceded(
        tuple((skip_noise, prep(PrepType::For), skip_noise)),
        separated_list1(
            tuple((skip_noise, token_type(TokenType::Conj), skip_noise)),
            instrument,
        ),
    ))(input)?;

    // Optional governing law
    let (input, governing_law) = opt(preceded(
        tuple((skip_noise, prep(PrepType::Under), skip_noise)),
        law,
    ))(input)?;

    Ok((
        input,
        IntentAst::CounterpartyCreate {
            counterparty,
            instruments: instruments.unwrap_or_default(),
            governing_law,
        },
    ))
}

/// Parse: "establish ISDA with {counterparty} under {law} [for {instruments}]"
fn parse_isda_establish(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;

    // Match "ISDA" entity indicator
    let (input, _) = entity_class(EntityClass::Isda)(input)?;
    let (input, _) = skip_noise(input)?;

    // "with {counterparty}"
    let (input, _) = prep(PrepType::With)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, counterparty) = any_entity(input)?;

    // "under {law}"
    let (input, _) = skip_noise(input)?;
    let (input, _) = prep(PrepType::Under)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, governing_law) = law(input)?;

    // Optional instruments
    let (input, instruments) = opt(preceded(
        tuple((skip_noise, prep(PrepType::For), skip_noise)),
        separated_list1(
            tuple((skip_noise, token_type(TokenType::Conj), skip_noise)),
            instrument,
        ),
    ))(input)?;

    Ok((
        input,
        IntentAst::IsdaEstablish {
            counterparty,
            governing_law,
            instruments: instruments.unwrap_or_default(),
        },
    ))
}

/// Parse: "add CSA [type] to {counterparty} [in {currency}]"
fn parse_csa_add(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;

    // Match CSA entity indicator
    let (input, _) = entity_class(EntityClass::Csa)(input)?;
    let (input, _) = skip_noise(input)?;

    // Optional CSA type
    let (input, csa) = opt(csa_type)(input)?;
    let (input, _) = skip_noise(input)?;

    // "to {counterparty}"
    let (input, _) = prep(PrepType::To)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, counterparty) = any_entity(input)?;

    // Optional currency
    let (input, curr) = opt(preceded(
        tuple((skip_noise, prep(PrepType::In), skip_noise)),
        currency,
    ))(input)?;

    Ok((
        input,
        IntentAst::CsaAdd {
            counterparty,
            csa_type: csa.unwrap_or(CsaType::Vm),
            currency: curr,
        },
    ))
}

/// Parse: "assign {entity} as {role} [to {cbu}]"
fn parse_role_assign(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Link)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, entity) = any_entity(input)?;
    let (input, _) = skip_noise(input)?;

    // "as {role}"
    let (input, _) = prep(PrepType::As)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, role_code) = role(input)?;

    // Optional "to {cbu}"
    let (input, cbu) = opt(preceded(
        tuple((skip_noise, prep(PrepType::To), skip_noise)),
        entity_class(EntityClass::Cbu),
    ))(input)?;

    // If no explicit CBU, use a placeholder that will be resolved from context
    let cbu_ref = cbu.unwrap_or(EntityRef::Pronoun {
        text: "it".to_string(),
        referent: None,
    });

    Ok((
        input,
        IntentAst::RoleAssign {
            cbu: cbu_ref,
            entity,
            role: role_code,
        },
    ))
}

/// Parse: "add {markets} trading [for {instruments}] [in {currencies}]"
fn parse_universe_add(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;

    // Markets
    let (input, markets) = separated_list1(
        tuple((skip_noise, token_type(TokenType::Conj), skip_noise)),
        market,
    )(input)?;
    let (input, _) = skip_noise(input)?;

    // Optional instruments
    let (input, instruments) = opt(preceded(
        tuple((skip_noise, prep(PrepType::For), skip_noise)),
        separated_list1(
            tuple((skip_noise, token_type(TokenType::Conj), skip_noise)),
            instrument,
        ),
    ))(input)?;

    // Optional currencies
    let (input, currencies) = opt(preceded(
        tuple((skip_noise, prep(PrepType::In), skip_noise)),
        separated_list1(
            tuple((skip_noise, token_type(TokenType::Conj), skip_noise)),
            currency,
        ),
    ))(input)?;

    // Optional "for/to {cbu}"
    let (input, cbu) = opt(preceded(
        tuple((
            skip_noise,
            alt((prep(PrepType::For), prep(PrepType::To))),
            skip_noise,
        )),
        entity_class(EntityClass::Cbu),
    ))(input)?;

    let cbu_ref = cbu.unwrap_or(EntityRef::Pronoun {
        text: "it".to_string(),
        referent: None,
    });

    Ok((
        input,
        IntentAst::UniverseAdd {
            cbu: cbu_ref,
            markets,
            instruments: instruments.unwrap_or_default(),
            currencies: currencies.unwrap_or_default(),
        },
    ))
}

/// Parse: "list/show {entities}"
fn parse_query(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Query)(input)?;
    let (input, _) = skip_noise(input)?;

    // Check what kind of entity we're querying
    if let Ok((remaining, entity)) = any_entity(input) {
        return Ok((remaining, IntentAst::EntityShow { entity }));
    }

    // Check for "counterparties"
    if !input.is_empty() && input[0].normalized.contains("counterpart") {
        let (input, _) = (&input[1..], ());

        // Optional "for {cbu}"
        let (input, cbu) = opt(preceded(
            tuple((skip_noise, prep(PrepType::For), skip_noise)),
            entity_class(EntityClass::Cbu),
        ))(input)?;

        return Ok((input, IntentAst::CounterpartyList { cbu }));
    }

    // Generic entity list
    Ok((
        input,
        IntentAst::EntityList {
            entity_type: None,
            filters: vec![],
        },
    ))
}

/// Try all intent parsers and return the first match.
pub fn parse_intent(input: Input) -> ParseResult<IntentAst> {
    // Skip leading noise
    let (input, _) = skip_noise(input)?;

    alt((
        parse_isda_establish,
        parse_csa_add,
        parse_counterparty_create,
        parse_role_assign,
        parse_universe_add,
        parse_query,
    ))(input)
}

/// Parse a token stream into an IntentAst.
///
/// This is the main entry point for the parser.
pub fn parse_tokens(tokens: &[Token]) -> Result<IntentAst, String> {
    if tokens.is_empty() {
        return Err("Empty token stream".to_string());
    }

    match parse_intent(tokens) {
        Ok((remaining, intent)) => {
            // Check if we consumed most of the input
            if remaining.len() > tokens.len() / 2 {
                // Too many tokens remaining, might be a bad parse
                let raw_text = tokens
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");

                // Try to extract verb class from first token
                let verb_class = tokens.iter().find_map(|t| {
                    if let TokenType::Verb(vc) = &t.token_type {
                        Some(*vc)
                    } else {
                        None
                    }
                });

                Ok(IntentAst::Unknown {
                    verb_class,
                    raw_text,
                })
            } else {
                Ok(intent)
            }
        }
        Err(_) => {
            let raw_text = tokens
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            let verb_class = tokens.iter().find_map(|t| {
                if let TokenType::Verb(vc) = &t.token_type {
                    Some(*vc)
                } else {
                    None
                }
            });

            Ok(IntentAst::Unknown {
                verb_class,
                raw_text,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tokens::TokenSource;
    use super::*;

    fn make_token(text: &str, token_type: TokenType) -> Token {
        Token {
            text: text.to_string(),
            normalized: text.to_lowercase(),
            token_type,
            span: (0, text.len()),
            source: TokenSource::Lexicon,
            resolved_id: None,
            confidence: 1.0,
        }
    }

    #[test]
    fn test_parse_counterparty_create() {
        let tokens = vec![
            make_token("add", TokenType::Verb(VerbClass::Create)),
            make_token(
                "Goldman Sachs",
                TokenType::Entity(EntityClass::Counterparty),
            ),
            make_token("as", TokenType::Prep(PrepType::As)),
            make_token("counterparty", TokenType::Role),
        ];

        let result = parse_tokens(&tokens).unwrap();

        match result {
            IntentAst::CounterpartyCreate { counterparty, .. } => {
                assert_eq!(counterparty.name(), "Goldman Sachs");
            }
            _ => panic!("Expected CounterpartyCreate, got {:?}", result),
        }
    }

    #[test]
    fn test_parse_counterparty_with_instruments() {
        let tokens = vec![
            make_token("add", TokenType::Verb(VerbClass::Create)),
            make_token("Goldman", TokenType::Entity(EntityClass::Counterparty)),
            make_token("as", TokenType::Prep(PrepType::As)),
            make_token("counterparty", TokenType::Role),
            make_token("for", TokenType::Prep(PrepType::For)),
            make_token("irs", TokenType::Instrument),
            make_token("and", TokenType::Conj),
            make_token("cds", TokenType::Instrument),
        ];

        let result = parse_tokens(&tokens).unwrap();

        match result {
            IntentAst::CounterpartyCreate { instruments, .. } => {
                assert_eq!(instruments.len(), 2);
                assert_eq!(instruments[0].as_str(), "IRS");
                assert_eq!(instruments[1].as_str(), "CDS");
            }
            _ => panic!("Expected CounterpartyCreate, got {:?}", result),
        }
    }

    #[test]
    fn test_parse_role_assign() {
        let tokens = vec![
            make_token("assign", TokenType::Verb(VerbClass::Link)),
            make_token("John Smith", TokenType::Entity(EntityClass::Person)),
            make_token("as", TokenType::Prep(PrepType::As)),
            make_token("director", TokenType::Role),
        ];

        let result = parse_tokens(&tokens).unwrap();

        match result {
            IntentAst::RoleAssign { entity, role, .. } => {
                assert_eq!(entity.name(), "John Smith");
                assert_eq!(role.as_str(), "DIRECTOR");
            }
            _ => panic!("Expected RoleAssign, got {:?}", result),
        }
    }

    #[test]
    fn test_parse_with_articles() {
        // "add a counterparty" - the article should be skipped
        let tokens = vec![
            make_token("add", TokenType::Verb(VerbClass::Create)),
            make_token("a", TokenType::Article),
            make_token("Goldman", TokenType::Entity(EntityClass::Counterparty)),
        ];

        let result = parse_tokens(&tokens).unwrap();

        match result {
            IntentAst::CounterpartyCreate { counterparty, .. } => {
                assert_eq!(counterparty.name(), "Goldman");
            }
            _ => panic!("Expected CounterpartyCreate, got {:?}", result),
        }
    }

    #[test]
    fn test_unknown_falls_back() {
        let tokens = vec![
            make_token("hello", TokenType::Unknown),
            make_token("world", TokenType::Unknown),
        ];

        let result = parse_tokens(&tokens).unwrap();

        match result {
            IntentAst::Unknown { raw_text, .. } => {
                assert!(raw_text.contains("hello"));
            }
            _ => panic!("Expected Unknown, got {:?}", result),
        }
    }
}
