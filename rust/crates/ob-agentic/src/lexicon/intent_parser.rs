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
            TokenType::Article | TokenType::Unknown | TokenType::Modifier(_) => {
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

/// Parse counterparty creation in either form:
/// - "add {entity} as counterparty [for {instruments}] [under {law}]"
/// - "create counterparty {entity} [for {instruments}] [under {law}]"
fn parse_counterparty_create(input: Input) -> ParseResult<IntentAst> {
    alt((
        parse_counterparty_create_entity_first,
        parse_counterparty_create_role_first,
    ))(input)
}

/// Parse: "add {entity} as counterparty [for {instruments}] [under {law}]"
/// Also handles: "add a counterparty called {entity}"
fn parse_counterparty_create_entity_first(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;

    // After lowering, we might have a typed Entity(Counterparty) directly
    // e.g., "add Deutsche Bank AG" where lowering fused "counterparty called Deutsche Bank AG"
    // into just "Deutsche Bank AG:Entity(Counterparty)"
    if let Some(token) = input.first() {
        if matches!(
            token.token_type,
            TokenType::Entity(EntityClass::Counterparty)
        ) && token.source == super::tokens::TokenSource::Lowering
        {
            // This is a lowered/fused counterparty entity - use it directly
            let counterparty = EntityRef::Unresolved {
                name: token.text.clone(),
                entity_type: Some("counterparty".to_string()),
            };
            let (remaining, _) = skip_noise(&input[1..])?;

            // Check for optional instruments and law
            let (remaining, instruments) = opt(preceded(
                tuple((skip_noise, prep(PrepType::For), skip_noise)),
                separated_list1(
                    tuple((skip_noise, token_type(TokenType::Conj), skip_noise)),
                    instrument,
                ),
            ))(remaining)?;

            let (remaining, governing_law) = opt(preceded(
                tuple((skip_noise, prep(PrepType::Under), skip_noise)),
                law,
            ))(remaining)?;

            return Ok((
                remaining,
                IntentAst::CounterpartyCreate {
                    counterparty,
                    instruments: instruments.unwrap_or_default(),
                    governing_law,
                },
            ));
        }
    }

    // Check if this is "add a counterparty called X" pattern
    // Look for counterparty indicator before entity
    let is_counterparty_called_pattern = input.iter().take(4).any(|t| {
        matches!(
            t.token_type,
            TokenType::Entity(EntityClass::Counterparty) | TokenType::Role
        ) && input
            .iter()
            .any(|t2| t2.normalized == "called" || t2.normalized == "named")
    });

    if is_counterparty_called_pattern {
        // Skip to "called" and get the entity after
        let mut remaining = input;
        while !remaining.is_empty()
            && remaining[0].normalized != "called"
            && remaining[0].normalized != "named"
        {
            remaining = &remaining[1..];
        }
        // Skip "called"/"named"
        if !remaining.is_empty() {
            remaining = &remaining[1..];
        }
        let (remaining, _) = skip_noise(remaining)?;

        // Get the entity name (may be multi-token)
        if let Ok((rest, counterparty)) = any_entity(remaining) {
            // Check for optional law at the end
            let (rest, _) = skip_noise(rest)?;
            let (rest, governing_law) = opt(preceded(
                tuple((skip_noise, prep(PrepType::Under), skip_noise)),
                law,
            ))(rest)?;

            return Ok((
                rest,
                IntentAst::CounterpartyCreate {
                    counterparty,
                    instruments: vec![],
                    governing_law,
                },
            ));
        }
    }

    let (input, counterparty) = any_entity(input)?;
    let (input, _) = skip_noise(input)?;

    // Optional "as counterparty" - accepts either Role or Entity(Counterparty) since
    // the lexicon maps "counterparty" to Entity(Counterparty), not Role
    let (input, _) = opt(tuple((
        prep(PrepType::As),
        skip_noise,
        alt((
            token_type(TokenType::Role),
            token_type(TokenType::Entity(EntityClass::Counterparty)),
        )),
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

/// Parse: "create counterparty {entity} [for {instruments}] [under {law}]"
/// Also handles: "create counterparty called {entity}"
fn parse_counterparty_create_role_first(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;

    // "counterparty" keyword first (as Role or Entity(Counterparty))
    let (input, _) = alt((
        token_type(TokenType::Role),
        token_type(TokenType::Entity(EntityClass::Counterparty)),
    ))(input)?;
    let (input, _) = skip_noise(input)?;

    // Skip optional "called" or "named" introducer
    let input = if !input.is_empty()
        && (input[0].normalized == "called" || input[0].normalized == "named")
    {
        &input[1..]
    } else {
        input
    };
    let (input, _) = skip_noise(input)?;

    // Then the entity name
    let (input, counterparty) = any_entity(input)?;

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

/// Parse CSA add in various forms:
/// - "add CSA to {counterparty}"
/// - "add VM CSA to {counterparty}"
/// - "add CSA VM to {counterparty}"
/// - "add {type} CSA to {counterparty} [in {currency}]"
fn parse_csa_add(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Create)(input)?;
    let (input, _) = skip_noise(input)?;

    // Parse CSA type and CSA entity in either order:
    // "VM CSA" or "CSA VM" or just "CSA"
    let (input, csa_type_before) = opt(csa_type)(input)?;
    let (input, _) = skip_noise(input)?;

    // Match CSA entity indicator (required)
    let (input, _) = entity_class(EntityClass::Csa)(input)?;
    let (input, _) = skip_noise(input)?;

    // CSA type can also come after "CSA"
    let (input, csa_type_after) = opt(csa_type)(input)?;
    let (input, _) = skip_noise(input)?;

    // Determine final CSA type: prefer before, then after, default to Vm
    let csa = csa_type_before.or(csa_type_after).unwrap_or(CsaType::Vm);

    // "to {counterparty}" or "with {counterparty}"
    let (input, _) = alt((prep(PrepType::To), prep(PrepType::With)))(input)?;
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
            csa_type: csa,
            currency: curr,
        },
    ))
}

/// Parse role assignment in various forms:
/// - "assign {entity} as {role} [to {cbu}]"
/// - "add {entity} as {role} [to {cbu}]"
/// - "make {entity} {role}"
///
/// Note: The key distinction from counterparty_create is that the target
/// is a Role token (signatory, director, etc.) not "counterparty".
fn parse_role_assign(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = alt((
        verb_class(VerbClass::Link),   // "assign"
        verb_class(VerbClass::Create), // "add", "make"
    ))(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, entity) = any_entity(input)?;
    let (input, _) = skip_noise(input)?;

    // "as {role}" - the role must be a Role token (signatory, director, etc.)
    // NOT a counterparty reference
    let (input, _) = prep(PrepType::As)(input)?;
    let (input, _) = skip_noise(input)?;
    let (input, role_code) = role(input)?;

    // Reject if the "role" is actually COUNTERPARTY - that should be handled
    // by counterparty_create, not role_assign
    if role_code.as_str().eq_ignore_ascii_case("counterparty") {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

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

/// Parse universe/trading setup in various forms:
/// - "add {instruments} to universe"
/// - "add {instruments} and {instruments} to universe"
/// - "add {markets} to universe"
/// - "enable trading on {markets}"
/// - "add {instruments} on {markets} to universe"
fn parse_universe_add(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = alt((
        verb_class(VerbClass::Create),
        verb_class(VerbClass::Provision), // "enable"
    ))(input)?;
    let (input, _) = skip_noise(input)?;

    // Skip optional "trading" word and any preposition before the list
    let (input, _) = opt(token_type(TokenType::Unknown))(input)?;
    let (input, _) = skip_noise(input)?;
    // Skip optional leading preposition "on"/"in" (for "enable trading on NYSE")
    let (input, _) = opt(alt((prep(PrepType::On), prep(PrepType::In))))(input)?;
    let (input, _) = skip_noise(input)?;

    // Parse a flexible list of instruments and/or markets
    let mut instruments = Vec::new();
    let mut markets = Vec::new();
    let mut remaining = input;

    // First item (required)
    if let Ok((rest, inst)) = instrument(remaining) {
        instruments.push(inst);
        remaining = rest;
    } else if let Ok((rest, mkt)) = market(remaining) {
        markets.push(mkt);
        remaining = rest;
    } else {
        return Err(nom::Err::Error(nom::error::Error::new(
            remaining,
            nom::error::ErrorKind::Alt,
        )));
    }

    // Additional items separated by "and" or "on"/"in" for markets
    loop {
        let (rest, _) = skip_noise(remaining)?;

        // Check for conjunction "and"
        if let Ok((rest2, _)) = token_type(TokenType::Conj)(rest) {
            let (rest3, _) = skip_noise(rest2)?;
            if let Ok((rest4, inst)) = instrument(rest3) {
                instruments.push(inst);
                remaining = rest4;
                continue;
            } else if let Ok((rest4, mkt)) = market(rest3) {
                markets.push(mkt);
                remaining = rest4;
                continue;
            }
        }

        // Check for "on"/"in" followed by markets
        if let Ok((rest2, _)) = alt((prep(PrepType::On), prep(PrepType::In)))(rest) {
            let (rest3, _) = skip_noise(rest2)?;
            if let Ok((rest4, mkt)) = market(rest3) {
                markets.push(mkt);
                remaining = rest4;
                // Continue to parse more markets after "and"
                loop {
                    let (rest5, _) = skip_noise(remaining)?;
                    if let Ok((rest6, _)) = token_type(TokenType::Conj)(rest5) {
                        let (rest7, _) = skip_noise(rest6)?;
                        if let Ok((rest8, mkt)) = market(rest7) {
                            markets.push(mkt);
                            remaining = rest8;
                            continue;
                        }
                    }
                    break;
                }
                continue;
            }
        }

        // Check for "to universe" terminator
        if let Ok((rest2, _)) = prep(PrepType::To)(rest) {
            let (rest3, _) = skip_noise(rest2)?;
            // Accept Entity(ScopeType) for "universe"
            if let Ok((rest4, _)) = token_type(TokenType::Entity(EntityClass::ScopeType))(rest3) {
                remaining = rest4;
            }
        }

        break;
    }

    // Skip remaining noise
    let (remaining, _) = skip_noise(remaining)?;

    // If no instruments or markets found, this isn't a universe_add
    if instruments.is_empty() && markets.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            remaining,
            nom::error::ErrorKind::Alt,
        )));
    }

    Ok((
        remaining,
        IntentAst::UniverseAdd {
            cbu: EntityRef::Pronoun {
                text: "it".to_string(),
                referent: None,
            },
            markets,
            instruments,
            currencies: Vec::new(),
        },
    ))
}

/// Parse query operations in various forms:
/// - "list counterparties [for {cbu}]"
/// - "show all counterparties"
/// - "show {entity}"
/// - "show ISDA with Goldman" / "show ISDA for Goldman"
/// - "list entities"
fn parse_query(input: Input) -> ParseResult<IntentAst> {
    let (input, _) = verb_class(VerbClass::Query)(input)?;

    // Early check: if the remaining tokens are all noise/unknown with no domain
    // keywords, this is likely an off-topic query like "what is the weather"
    let has_domain_token = input.iter().any(|t| {
        matches!(
            t.token_type,
            TokenType::Entity(_)
                | TokenType::Instrument
                | TokenType::Market
                | TokenType::Law
                | TokenType::Role
                | TokenType::Prep(_)
        ) || t.normalized.contains("counterpart")
            || t.normalized.contains("isda")
            || t.normalized.contains("csa")
            || t.normalized.contains("entity")
            || t.normalized.contains("cbu")
    });

    if !has_domain_token && !input.is_empty() {
        // No domain-relevant tokens - this is off-topic
        let raw_text = input
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return Ok((
            &input[input.len()..],
            IntentAst::Unknown {
                verb_class: Some(VerbClass::Query),
                raw_text,
            },
        ));
    }

    let (input, _) = skip_noise(input)?;

    // Check for counterparty queries - look ahead to find "counterpart*" anywhere
    // This handles "list counterparties", "show all counterparties", etc.
    let has_counterparty_token = input.iter().any(|t| {
        t.normalized.contains("counterpart")
            || matches!(t.token_type, TokenType::Entity(EntityClass::Counterparty))
    });

    if has_counterparty_token {
        // Skip past any scope words like "all" to find the counterparty token
        let mut remaining = input;
        while !remaining.is_empty() {
            let token = &remaining[0];
            if token.normalized.contains("counterpart")
                || matches!(
                    token.token_type,
                    TokenType::Entity(EntityClass::Counterparty)
                )
            {
                remaining = &remaining[1..];
                break;
            }
            remaining = &remaining[1..];
        }

        // Optional "for {cbu}"
        let (remaining, cbu) = opt(preceded(
            tuple((skip_noise, prep(PrepType::For), skip_noise)),
            entity_class(EntityClass::Cbu),
        ))(remaining)?;

        return Ok((remaining, IntentAst::CounterpartyList { cbu }));
    }

    // Check for ISDA queries: "show ISDA with Goldman", "show ISDA for Goldman"
    // Also: "what ISDAs do we have with Goldman"
    if let Ok((remaining, _isda)) = entity_class(EntityClass::Isda)(input) {
        let (remaining, _) = skip_noise(remaining)?;

        // Optional "with/for {counterparty}"
        let (remaining, counterparty) = opt(preceded(
            tuple((alt((prep(PrepType::With), prep(PrepType::For))), skip_noise)),
            any_entity,
        ))(remaining)?;

        return Ok((remaining, IntentAst::IsdaShow { counterparty }));
    }

    // Check for conversational ISDA queries: "what ISDAs do we have with Goldman"
    // Look for ISDA-like token anywhere, then find counterparty after "with"
    let has_isda_token = input.iter().any(|t| {
        matches!(t.token_type, TokenType::Entity(EntityClass::Isda))
            || t.normalized.contains("isda")
    });
    let has_with_prep = input
        .iter()
        .any(|t| matches!(t.token_type, TokenType::Prep(PrepType::With)));

    if has_isda_token && has_with_prep {
        // Find the counterparty after "with"
        let mut found_with = false;
        for (i, token) in input.iter().enumerate() {
            if matches!(token.token_type, TokenType::Prep(PrepType::With)) {
                found_with = true;
            } else if found_with {
                // Try to parse entity from here
                if let Ok((remaining, counterparty)) = any_entity(&input[i..]) {
                    return Ok((
                        remaining,
                        IntentAst::IsdaShow {
                            counterparty: Some(counterparty),
                        },
                    ));
                }
            }
        }
        // ISDA query without specific counterparty
        return Ok((
            &input[input.len()..],
            IntentAst::IsdaShow { counterparty: None },
        ));
    }

    // Check what kind of entity we're querying (generic entity show)
    // But only if we have a proper entity, not just a pronoun or noise
    if let Ok((remaining, entity)) = any_entity(input) {
        // Don't match bare pronouns like "it" in "what time is it"
        // Only match if it's a real entity reference
        let is_real_entity = match &entity {
            EntityRef::Pronoun { referent, .. } => referent.is_some(),
            EntityRef::Resolved(_) => true,
            EntityRef::Unresolved { entity_type, .. } => entity_type.is_some(),
        };

        if is_real_entity {
            return Ok((remaining, IntentAst::EntityShow { entity }));
        }
    }

    // Generic entity list (or unknown if no clear intent)
    // If there's still unknown tokens, it's likely off-topic
    let has_only_noise = input.iter().all(|t| {
        matches!(
            t.token_type,
            TokenType::Unknown | TokenType::Article | TokenType::Pronoun
        )
    });

    if has_only_noise && !input.is_empty() {
        // This looks like off-topic noise, not a real query
        let raw_text = input
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return Ok((
            &input[input.len()..],
            IntentAst::Unknown {
                verb_class: Some(VerbClass::Query),
                raw_text,
            },
        ));
    }

    Ok((
        input,
        IntentAst::EntityList {
            entity_type: None,
            filters: vec![],
        },
    ))
}

/// Skip conversational preamble until we find a verb.
/// Handles patterns like "I need to add...", "can you please add...", etc.
fn skip_to_verb(input: Input) -> Input {
    // Find the first verb token
    let verb_pos = input
        .iter()
        .position(|t| matches!(t.token_type, TokenType::Verb(_)));

    match verb_pos {
        Some(0) => input,           // Already at a verb
        Some(pos) => &input[pos..], // Skip to the verb
        None => input,              // No verb found, return as-is
    }
}

/// Parse terse/verbless patterns by inferring intent from entity types.
/// Handles patterns like:
/// - "counterparty: Goldman" → CounterpartyCreate
/// - "counterparty add: Goldman" → CounterpartyCreate (inverted noun-verb)
/// - "ISDA Goldman NY" → IsdaEstablish
/// - "CSA Goldman" → CsaAdd
fn parse_verbless(input: Input) -> ParseResult<IntentAst> {
    // Skip any punctuation at the start
    let input = if !input.is_empty() && input[0].token_type == TokenType::Punct {
        &input[1..]
    } else {
        input
    };

    let (input, _) = skip_noise(input)?;

    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }

    // Pattern: Single typed Counterparty entity (after lowering fused type+name)
    // e.g., "counterparty add: HSBC" → lowered to [HSBC:Entity(Counterparty)]
    if matches!(
        input[0].token_type,
        TokenType::Entity(EntityClass::Counterparty)
    ) {
        // Check if this IS the entity (not just a type indicator)
        // A type indicator would be followed by another entity; a fused entity stands alone
        let is_fused_entity = input.len() == 1
            || !matches!(
                input.get(1).map(|t| &t.token_type),
                Some(TokenType::Entity(_)) | Some(TokenType::Unknown)
            );

        if is_fused_entity && input[0].source == super::tokens::TokenSource::Lowering {
            // This is a fused entity from lowering - use it directly
            let counterparty = EntityRef::Unresolved {
                name: input[0].text.clone(),
                entity_type: Some("counterparty".to_string()),
            };
            return Ok((
                &input[1..],
                IntentAst::CounterpartyCreate {
                    counterparty,
                    instruments: vec![],
                    governing_law: None,
                },
            ));
        }

        // Original pattern: "counterparty: {name}" or "counterparty {name}" or "counterparty add: {name}"
        // Skip the counterparty indicator, any verb (for inverted patterns), and punctuation
        let mut remaining = &input[1..];
        while !remaining.is_empty()
            && matches!(
                remaining[0].token_type,
                TokenType::Punct | TokenType::Article | TokenType::Verb(_)
            )
        {
            remaining = &remaining[1..];
        }

        // Next should be the entity name
        if let Ok((rest, counterparty)) = any_entity(remaining) {
            return Ok((
                rest,
                IntentAst::CounterpartyCreate {
                    counterparty,
                    instruments: vec![],
                    governing_law: None,
                },
            ));
        }
    }

    // Pattern: "ISDA {counterparty} {law}" - ultra-terse ISDA
    if matches!(input[0].token_type, TokenType::Entity(EntityClass::Isda)) {
        let remaining = &input[1..];
        let (remaining, _) = skip_noise(remaining)?;

        // Get counterparty
        if let Ok((remaining, counterparty)) = any_entity(remaining) {
            let (remaining, _) = skip_noise(remaining)?;

            // Get optional law
            let (remaining, governing_law) = opt(law)(remaining)?;

            if let Some(gl) = governing_law {
                return Ok((
                    remaining,
                    IntentAst::IsdaEstablish {
                        counterparty,
                        governing_law: gl,
                        instruments: vec![],
                    },
                ));
            }
        }
    }

    // Pattern: "CSA {counterparty}" - ultra-terse CSA
    if matches!(input[0].token_type, TokenType::Entity(EntityClass::Csa)) {
        let remaining = &input[1..];
        let (remaining, _) = skip_noise(remaining)?;

        // Optional CSA type
        let (remaining, csa) = opt(csa_type)(remaining)?;
        let (remaining, _) = skip_noise(remaining)?;

        // Get counterparty
        if let Ok((remaining, counterparty)) = any_entity(remaining) {
            return Ok((
                remaining,
                IntentAst::CsaAdd {
                    counterparty,
                    csa_type: csa.unwrap_or(CsaType::Vm),
                    currency: None,
                },
            ));
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Alt,
    )))
}

/// Try all intent parsers and return the first match.
///
/// Parser order matters for disambiguation:
/// 1. ISDA establish - specific pattern with "ISDA" entity
/// 2. CSA add - specific pattern with CSA entity
/// 3. Role assign - "add X as {role}" where role is signatory/director/etc
/// 4. Counterparty create - "add X as counterparty" - must come AFTER role_assign
/// 5. Universe add - instruments/markets
/// 6. Query - list/show operations
/// 7. Verbless - infer intent from entity types (fallback)
pub fn parse_intent(input: Input) -> ParseResult<IntentAst> {
    // Skip conversational preamble like "I need to", "can you please"
    let input = skip_to_verb(input);

    // Skip leading noise (articles, unknown words)
    let (input, _) = skip_noise(input)?;

    // Try verb-based parsers first
    if let Ok(result) = alt((
        parse_isda_establish,
        parse_csa_add,
        parse_role_assign, // Must be before counterparty_create!
        parse_counterparty_create,
        parse_universe_add,
        parse_query,
    ))(input)
    {
        return Ok(result);
    }

    // Fall back to verbless pattern inference
    parse_verbless(input)
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
