//! Idiomatic nom-based DSL parser for Ultimate Beneficial Ownership workflows
//!
//! This module provides efficient standalone parser functions using nom combinators
//! with proper error handling and zero-copy parsing where possible.

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace0},
    combinator::{map, opt, recognize, value},
    error::{ParseError, VerboseError},
    multi::{many0, many1, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    Finish, IResult,
};

use crate::parser_ast::{Form, Key, Literal, Program, Value, VerbForm};

/// Parser error type with context information
pub(crate) type NomParseError<'a> = VerboseError<&'a str>;
pub type ParseResult<'a, T> = IResult<&'a str, T, NomParseError<'a>>;

/// Parse a complete DSL program
pub fn parse_program(input: &str) -> Result<Program, NomParseError<'_>> {
    let (remaining, program) = program_internal(input).finish()?;

    if !remaining.trim().is_empty() {
        return Err(VerboseError::from_error_kind(
            remaining,
            nom::error::ErrorKind::Eof,
        ));
    }

    Ok(program)
}

/// Parse program: multiple forms
fn program_internal(input: &str) -> ParseResult<'_, Program> {
    let (input, _) = multispace0(input)?;
    let (input, forms) = many0(terminated(parse_form, multispace0))(input)?;
    Ok((input, forms))
}

/// Parse a form: (verb :key value ...) or a comment
pub fn parse_form(input: &str) -> ParseResult<'_, Form> {
    alt((
        map(parse_comment, Form::Comment),
        map(parse_verb_form, Form::Verb),
    ))(input)
}

/// Parse a comment (;; ...)
fn parse_comment(input: &str) -> ParseResult<'_, String> {
    let (input, _) = tag(";;")(input)?;
    let (input, comment_text) = take_until("\n")(input)?;
    let (input, _) = opt(char('\n'))(input)?; // Consume newline if present
    Ok((input, comment_text.to_string()))
}

/// Parse a verb form: (verb :key value ...)
pub fn parse_verb_form(input: &str) -> ParseResult<'_, VerbForm> {
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?; // Allow whitespace after '('
    let (input, verb) = parse_identifier(input)?; // Verb is an identifier
    let (input, _) = multispace0(input)?; // Allow whitespace after verb

    let (input, pairs) = many0(pair(
        preceded(multispace0, parse_key),
        preceded(multispace0, parse_value),
    ))(input)?;
    let (input, _) = multispace0(input)?; // Allow whitespace before ')'
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        VerbForm {
            verb,
            pairs: pairs.into_iter().collect(), // Collect into PropertyMap (HashMap<Key, Value>)
        },
    ))
}

/// Parse a key: :identifier or :identifier.sub_identifier
fn parse_key(input: &str) -> ParseResult<'_, Key> {
    let (input, _) = char(':')(input)?;
    let (input, id_parts) = separated_list1(char('.'), parse_key_part)(input)?;
    Ok((input, Key { parts: id_parts }))
}

/// Parse a value based on v3.0 EBNF: literal | identifier | list | map | attr-ref
pub fn parse_value(input: &str) -> ParseResult<'_, Value> {
    alt((
        map(parse_literal, Value::Literal),
        map(parse_identifier, Value::Identifier),
        parse_list_value, // returns Value::List
        parse_map_value,  // returns Value::Map
        parse_attr_ref,   // returns Value::AttrRef
    ))(input)
}

/// Parse a literal value: string | number | boolean | date | uuid
fn parse_literal(input: &str) -> ParseResult<'_, Literal> {
    alt((
        map(parse_string_literal, Literal::String),
        parse_number_literal,
        parse_boolean_literal,
        map(parse_string_literal, Literal::Date), // Dates are strings in ISO 8601 format
        map(parse_string_literal, Literal::Uuid), // UUIDs are strings
    ))(input)
}

/// Parse string literals with proper escaping.
pub fn parse_string_literal(input: &str) -> ParseResult<'_, String> {
    delimited(
        char('\"'),
        map(
            many0(alt((
                value('\n', tag("\\n")),
                value('\r', tag("\\r")),
                value('\t', tag("\\t")),
                value('\\', tag("\\\\")),
                value('\"', tag("\\\"")),
                none_of("\"\\"),
            ))),
            |chars| chars.into_iter().collect(),
        ),
        char('\"'),
    )(input)
}

/// Parse numeric literal values (f64 for both integers and floats)
fn parse_number_literal(input: &str) -> ParseResult<'_, Literal> {
    let (input, num_str) = recognize(tuple((
        opt(char('-')),
        digit1,
        opt(preceded(char('.'), digit1)),
    )))(input)?;
    let num = num_str.parse::<f64>().map_err(|_| {
        nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Float, // Use Float for any numeric parsing error
        ))
    })?;
    Ok((input, Literal::Number(num)))
}

/// Parse boolean literal values
fn parse_boolean_literal(input: &str) -> ParseResult<'_, Literal> {
    alt((
        value(Literal::Boolean(true), tag("true")),
        value(Literal::Boolean(false), tag("false")),
    ))(input)
}

/// Parse list values: [item1 item2 item3] or [item1, item2, item3] (both formats supported)
/// Implements V3.1 EBNF: "[" (value (("," | whitespace) value)*)? "]"
pub fn parse_list_value(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = char('[')(input)?;
    let (input, _) = multispace0(input)?;

    // Handle empty list
    if let Ok((remaining, _)) = char::<_, VerboseError<&str>>(']')(input) {
        return Ok((remaining, Value::List(vec![])));
    }

    // Parse first value
    let (input, first_value) = parse_value(input)?;
    let (input, _) = multispace0(input)?;

    let mut values = vec![first_value];
    let mut current_input = input;

    // Parse remaining values with comma or space separators
    loop {
        // If we see a closing bracket, we're done
        if current_input.starts_with(']') {
            break;
        }

        // Try comma separator first
        if let Ok((remaining, _)) = char::<_, VerboseError<&str>>(',')(current_input) {
            let (remaining, _) = multispace0(remaining)?;
            let (remaining, value) = parse_value(remaining)?;
            values.push(value);
            let (remaining, _) = multispace0(remaining)?;
            current_input = remaining;
        } else {
            // Try space separator (whitespace already consumed)
            if let Ok((remaining, value)) = parse_value(current_input) {
                values.push(value);
                let (remaining, _) = multispace0(remaining)?;
                current_input = remaining;
            } else {
                // No more values to parse
                break;
            }
        }
    }

    let (input, _) = char(']')(current_input)?;
    Ok((input, Value::List(values)))
}

/// Parse map values: {key1: value1, key2: value2}
pub(crate) fn parse_map_value(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = char('{')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, pairs) = many0(|input| {
        let (input, _) = multispace0(input)?;
        let (input, key) = parse_key(input)?;
        let (input, _) = multispace0(input)?;
        let (input, value) = parse_value(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, (key, value)))
    })(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, Value::Map(pairs.into_iter().collect())))
}

/// Parse an attribute reference: @attr{uuid} or @attr.category.name
/// Implements Phase 1: UUID-based and semantic attribute references
fn parse_attr_ref(input: &str) -> ParseResult<'_, Value> {
    alt((
        parse_attr_uuid,     // Try UUID format first: @attr{uuid}
        parse_attr_semantic, // Fall back to semantic format: @attr.category.name
    ))(input)
}

/// Parse UUID-based attribute reference: @attr{uuid}
/// Example: @attr{3020d46f-472c-5437-9647-1b0682c35935}
fn parse_attr_uuid(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = tag("@attr{")(input)?;
    let (input, uuid_str) = take_while1(|c: char| c.is_ascii_hexdigit() || c == '-')(input)?;
    let (input, _) = char('}')(input)?;

    // Parse the UUID string into a proper Uuid type
    let uuid = uuid::Uuid::parse_str(uuid_str).map_err(|_| {
        nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Verify,
        ))
    })?;

    Ok((input, Value::AttrUuid(uuid)))
}

/// Parse semantic attribute reference: @attr.category.name
/// Example: @attr.identity.first_name
fn parse_attr_semantic(input: &str) -> ParseResult<'_, Value> {
    let (input, attr_id) = parse_string_attr_ref(input)?;
    Ok((input, Value::AttrRef(attr_id)))
}

/// Parse new string-based attribute reference: @attr.category.name
fn parse_string_attr_ref(input: &str) -> ParseResult<'_, String> {
    let (input, _) = tag("@attr.")(input)?;
    let (input, attr_id) = recognize(pair(
        // First part: category (e.g., "identity")
        alpha1,
        // Remaining parts: .subcategory.name
        many1(pair(
            char('.'),
            take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        )),
    ))(input)?;

    // Reconstruct the full attribute ID
    let full_id = format!("attr.{}", attr_id);
    Ok((input, full_id))
}

/// Parse identifiers: alphanumeric with underscore, dash, dot
pub fn parse_identifier(input: &str) -> ParseResult<'_, String> {
    let (input, id) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-"), tag(".")))),
    ))(input)?;
    Ok((input, id.to_string()))
}

/// Parse key parts: alphanumeric with underscore, dash (but not dot for key splitting)
fn parse_key_part(input: &str) -> ParseResult<'_, String> {
    let (input, id) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(input)?;
    Ok((input, id.to_string()))
}

/// Helper function to parse single characters not in the given set
fn none_of(chars: &'static str) -> impl Fn(&str) -> ParseResult<char> {
    move |input| {
        if let Some(c) = input.chars().next() {
            if !chars.contains(c) {
                Ok((&input[c.len_utf8()..], c))
            } else {
                Err(nom::Err::Error(VerboseError::from_error_kind(
                    input,
                    nom::error::ErrorKind::OneOf,
                )))
            }
        } else {
            Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                nom::error::ErrorKind::Eof,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_new_string_attr_ref() {
        // Test new format: @attr.identity.first_name
        let input = "@attr.identity.first_name";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            value,
            Value::AttrRef("attr.identity.first_name".to_string())
        );
    }

    #[test]
    fn test_parse_uuid_attr_ref() {
        // Test UUID format: @attr{uuid} (Phase 1 implementation)
        let input = "@attr{123e4567-e89b-12d3-a456-426614174001}";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");

        // Should parse to AttrUuid variant
        if let Value::AttrUuid(uuid) = value {
            assert_eq!(uuid.to_string(), "123e4567-e89b-12d3-a456-426614174001");
        } else {
            panic!("Expected AttrUuid variant, got {:?}", value);
        }
    }

    #[test]
    fn test_parse_real_uuid_from_phase0() {
        // Test with actual UUID from Phase 0 (FirstName)
        let input = "@attr{3020d46f-472c-5437-9647-1b0682c35935}";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");

        if let Value::AttrUuid(uuid) = value {
            assert_eq!(uuid.to_string(), "3020d46f-472c-5437-9647-1b0682c35935");
        } else {
            panic!("Expected AttrUuid variant");
        }
    }

    #[test]
    fn test_parse_invalid_uuid_format() {
        // Test invalid UUID format should fail
        let input = "@attr{not-a-valid-uuid}";
        let result = parse_attr_ref(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_attr_ref_with_underscores() {
        // Test format with underscores: @attr.kyc.proper_person.net_worth
        let input = "@attr.kyc.proper_person.net_worth";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            value,
            Value::AttrRef("attr.kyc.proper_person.net_worth".to_string())
        );
    }

    #[test]
    fn test_parse_attr_ref_in_verb_form() {
        // Test attribute reference within a verb form
        let input = r#"(entity.set-attribute :entity-id "test-123" :attribute @attr.identity.first_name :value "John")"#;
        let result = parse_program(input);
        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.len(), 1);

        if let Form::Verb(verb_form) = &program[0] {
            assert_eq!(verb_form.verb, "entity.set-attribute");
            // Check that the attribute was parsed correctly
            let attr_key = Key::new("attribute");
            assert!(verb_form.pairs.contains_key(&attr_key));
            if let Some(Value::AttrRef(attr_id)) = verb_form.pairs.get(&attr_key) {
                assert_eq!(attr_id, "attr.identity.first_name");
            } else {
                panic!("Expected AttrRef value");
            }
        } else {
            panic!("Expected Verb form");
        }
    }

    #[test]
    fn test_parse_multiple_attr_refs() {
        // Test multiple attribute references in one form
        let input = r#"(validation.check :attr1 @attr.identity.first_name :attr2 @attr.identity.last_name)"#;
        let result = parse_program(input);
        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.len(), 1);

        if let Form::Verb(verb_form) = &program[0] {
            assert_eq!(verb_form.verb, "validation.check");

            let attr1_key = Key::new("attr1");
            let attr2_key = Key::new("attr2");

            assert!(verb_form.pairs.contains_key(&attr1_key));
            assert!(verb_form.pairs.contains_key(&attr2_key));

            if let Some(Value::AttrRef(attr_id)) = verb_form.pairs.get(&attr1_key) {
                assert_eq!(attr_id, "attr.identity.first_name");
            }

            if let Some(Value::AttrRef(attr_id)) = verb_form.pairs.get(&attr2_key) {
                assert_eq!(attr_id, "attr.identity.last_name");
            }
        }
    }

    #[test]
    fn test_parse_ubo_attr_refs() {
        // Test UBO-specific attributes
        let input = "@attr.ubo.ownership_percentage";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            value,
            Value::AttrRef("attr.ubo.ownership_percentage".to_string())
        );
    }

    #[test]
    fn test_parse_compliance_attr_refs() {
        // Test compliance attributes
        let input = "@attr.compliance.fatca_status";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            value,
            Value::AttrRef("attr.compliance.fatca_status".to_string())
        );
    }

    #[test]
    fn test_parse_contact_attr_refs() {
        // Test contact attributes
        let input = "@attr.contact.email";
        let result = parse_attr_ref(input);
        assert!(result.is_ok());
        let (remaining, value) = result.unwrap();
        assert_eq!(remaining, "");
        assert_eq!(value, Value::AttrRef("attr.contact.email".to_string()));
    }

    // ============================================================================
    // PHASE 1 UUID PARSER TESTS - Comprehensive UUID-based attribute reference testing
    // ============================================================================

    #[test]
    fn test_uuid_in_verb_form() {
        // Test UUID attribute reference within a verb form
        let input = r#"(entity.set-attribute :entity-id "test-123" :attribute @attr{3020d46f-472c-5437-9647-1b0682c35935} :value "John")"#;
        let result = parse_program(input);
        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.len(), 1);

        if let Form::Verb(verb_form) = &program[0] {
            assert_eq!(verb_form.verb, "entity.set-attribute");
            let attr_key = Key::new("attribute");
            assert!(verb_form.pairs.contains_key(&attr_key));

            if let Some(Value::AttrUuid(uuid)) = verb_form.pairs.get(&attr_key) {
                assert_eq!(uuid.to_string(), "3020d46f-472c-5437-9647-1b0682c35935");
            } else {
                panic!(
                    "Expected AttrUuid value, got {:?}",
                    verb_form.pairs.get(&attr_key)
                );
            }
        } else {
            panic!("Expected Verb form");
        }
    }

    #[test]
    fn test_mixed_uuid_and_semantic_refs() {
        // Test mixing both UUID and semantic references in the same form
        let input = r#"(validation.check :uuid-attr @attr{3020d46f-472c-5437-9647-1b0682c35935} :semantic-attr @attr.identity.last_name)"#;
        let result = parse_program(input);
        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.len(), 1);

        if let Form::Verb(verb_form) = &program[0] {
            let uuid_key = Key::new("uuid-attr");
            let semantic_key = Key::new("semantic-attr");

            // Check UUID reference
            if let Some(Value::AttrUuid(uuid)) = verb_form.pairs.get(&uuid_key) {
                assert_eq!(uuid.to_string(), "3020d46f-472c-5437-9647-1b0682c35935");
            } else {
                panic!("Expected AttrUuid for uuid-attr");
            }

            // Check semantic reference
            if let Some(Value::AttrRef(attr_id)) = verb_form.pairs.get(&semantic_key) {
                assert_eq!(attr_id, "attr.identity.last_name");
            } else {
                panic!("Expected AttrRef for semantic-attr");
            }
        }
    }

    #[test]
    fn test_uuid_list_values() {
        // Test UUIDs in list values
        let input = r#"(collect-attributes :attrs [@attr{3020d46f-472c-5437-9647-1b0682c35935} @attr{0af112fd-ec04-5938-84e8-6e5949db0b52}])"#;
        let result = parse_program(input);
        assert!(result.is_ok());
        let program = result.unwrap();

        if let Form::Verb(verb_form) = &program[0] {
            let attrs_key = Key::new("attrs");
            if let Some(Value::List(items)) = verb_form.pairs.get(&attrs_key) {
                assert_eq!(items.len(), 2);

                if let Value::AttrUuid(uuid1) = &items[0] {
                    assert_eq!(uuid1.to_string(), "3020d46f-472c-5437-9647-1b0682c35935");
                } else {
                    panic!("Expected first item to be AttrUuid");
                }

                if let Value::AttrUuid(uuid2) = &items[1] {
                    assert_eq!(uuid2.to_string(), "0af112fd-ec04-5938-84e8-6e5949db0b52");
                } else {
                    panic!("Expected second item to be AttrUuid");
                }
            } else {
                panic!("Expected List value");
            }
        }
    }

    #[test]
    fn test_all_phase0_uuids() {
        // Test parsing all major attribute UUIDs from Phase 0
        let test_cases = vec![
            ("3020d46f-472c-5437-9647-1b0682c35935", "FirstName"),
            ("0af112fd-ec04-5938-84e8-6e5949db0b52", "LastName"),
            ("d655aadd-3605-5490-80be-20e6202b004b", "LegalName"),
            ("1f90b8a8-526f-5b9e-873a-ec6d0e2e3c3f", "Email"),
            ("aa7eb6e5-0c9b-51cf-ae47-2e2c47d91c1a", "PhoneNumber"),
        ];

        for (uuid_str, name) in test_cases {
            let input = format!("@attr{{{}}}", uuid_str);
            let result = parse_attr_ref(&input);
            assert!(result.is_ok(), "Failed to parse UUID for {}", name);

            let (remaining, value) = result.unwrap();
            assert_eq!(remaining, "", "Unexpected remaining input for {}", name);

            if let Value::AttrUuid(uuid) = value {
                assert_eq!(uuid.to_string(), uuid_str, "UUID mismatch for {}", name);
            } else {
                panic!("Expected AttrUuid for {}, got {:?}", name, value);
            }
        }
    }

    #[test]
    fn test_uuid_format_variations() {
        // Test UUID with different case (should work - UUIDs are case-insensitive)
        let input_upper = "@attr{3020D46F-472C-5437-9647-1B0682C35935}";
        let result = parse_attr_ref(input_upper);
        assert!(result.is_ok());

        let input_mixed = "@attr{3020d46F-472c-5437-9647-1B0682c35935}";
        let result = parse_attr_ref(input_mixed);
        assert!(result.is_ok());
    }

    #[test]
    fn test_uuid_without_hyphens_accepted() {
        // UUIDs without hyphens are actually accepted by the uuid crate
        // This is valid according to RFC 4122 "simple" format
        let input = "@attr{3020d46f472c543796471b0682c35935}";
        let result = parse_attr_ref(input);

        // Should successfully parse (uuid crate accepts simple format)
        assert!(
            result.is_ok(),
            "UUID simple format (no hyphens) should be accepted"
        );

        if let Ok((remaining, Value::AttrUuid(uuid))) = result {
            assert_eq!(remaining, "");
            assert_eq!(uuid.to_string(), "3020d46f-472c-5437-9647-1b0682c35935");
        } else {
            panic!("Expected AttrUuid variant");
        }
    }

    #[test]
    fn test_semantic_vs_uuid_disambiguation() {
        // Ensure parser correctly distinguishes between semantic and UUID formats

        // Semantic format
        let semantic = "@attr.identity.first_name";
        let result = parse_attr_ref(semantic);
        assert!(result.is_ok());
        if let Ok((_, Value::AttrRef(_))) = result {
            // Correct - semantic format parsed as AttrRef
        } else {
            panic!("Semantic format should parse as AttrRef");
        }

        // UUID format
        let uuid = "@attr{3020d46f-472c-5437-9647-1b0682c35935}";
        let result = parse_attr_ref(uuid);
        assert!(result.is_ok());
        if let Ok((_, Value::AttrUuid(_))) = result {
            // Correct - UUID format parsed as AttrUuid
        } else {
            panic!("UUID format should parse as AttrUuid");
        }
    }
}
