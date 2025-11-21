//! Parser for CBU Model DSL
//!
//! Uses nom to parse the CBU Model specification DSL into AST structures.

use crate::cbu_model_dsl::ast::{
    CbuAttributeGroup, CbuAttributesSpec, CbuModel, CbuRoleSpec, CbuState, CbuStateMachine,
    CbuTransition,
};
use nom::{
    bytes::complete::{tag, take_until},
    character::complete::{char, digit1, multispace0},
    combinator::{map, map_res, opt, value},
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, tuple},
    IResult,
};
use thiserror::Error;

/// Errors that can occur during CBU Model DSL parsing
#[derive(Debug, Error)]
pub enum CbuModelError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Parser for CBU Model DSL
pub struct CbuModelParser;

impl CbuModelParser {
    /// Parse a CBU Model DSL string into a CbuModel AST
    pub fn parse_str(input: &str) -> Result<CbuModel, CbuModelError> {
        match parse_cbu_model(input) {
            Ok((remaining, model)) => {
                // Check for unparsed content
                let remaining = remaining.trim();
                if !remaining.is_empty() {
                    return Err(CbuModelError::ParseError(format!(
                        "Unexpected content after model: {}",
                        &remaining[..remaining.len().min(50)]
                    )));
                }
                Ok(model)
            }
            Err(e) => Err(CbuModelError::ParseError(format!("{:?}", e))),
        }
    }
}

// ============================================================================
// Nom Parser Combinators
// ============================================================================

/// Parse whitespace and comments
fn ws(input: &str) -> IResult<&str, ()> {
    value((), multispace0)(input)
}

/// Parse a quoted string
fn quoted_string(input: &str) -> IResult<&str, String> {
    delimited(
        char('"'),
        map(take_until("\""), |s: &str| s.to_string()),
        char('"'),
    )(input)
}

/// Parse a number
fn number(input: &str) -> IResult<&str, u32> {
    map_res(digit1, |s: &str| s.parse::<u32>())(input)
}

/// Parse a string list: ["item1", "item2"]
fn string_list(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        tuple((char('['), ws)),
        separated_list0(tuple((ws, char(','), ws)), quoted_string),
        tuple((ws, char(']'))),
    )(input)
}

/// Parse an attribute reference: @attr("ATTR_NAME")
fn attr_ref(input: &str) -> IResult<&str, String> {
    delimited(
        tag("@attr("),
        delimited(
            char('"'),
            map(take_until("\""), |s: &str| s.to_string()),
            char('"'),
        ),
        char(')'),
    )(input)
}

/// Parse a list of attribute references
fn attr_ref_list(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        tuple((char('['), ws)),
        separated_list0(tuple((ws, char(','), ws)), attr_ref),
        tuple((ws, char(']'))),
    )(input)
}

/// Parse a keyword-value pair with string value
fn kv_string<'a>(keyword: &'a str) -> impl FnMut(&'a str) -> IResult<&'a str, String> {
    move |input| preceded(tuple((ws, tag(keyword), ws)), quoted_string)(input)
}

/// Parse a keyword-value pair with string list value
fn kv_string_list<'a>(keyword: &'a str) -> impl FnMut(&'a str) -> IResult<&'a str, Vec<String>> {
    move |input| preceded(tuple((ws, tag(keyword), ws)), string_list)(input)
}

/// Parse a keyword-value pair with number value
fn kv_number<'a>(keyword: &'a str) -> impl FnMut(&'a str) -> IResult<&'a str, u32> {
    move |input| preceded(tuple((ws, tag(keyword), ws)), number)(input)
}

/// Parse a keyword-value pair with attr ref list value
fn kv_attr_list<'a>(keyword: &'a str) -> impl FnMut(&'a str) -> IResult<&'a str, Vec<String>> {
    move |input| preceded(tuple((ws, tag(keyword), ws)), attr_ref_list)(input)
}

// ============================================================================
// CBU Model Structure Parsers
// ============================================================================

/// Parse an attribute group
fn parse_attribute_group(input: &str) -> IResult<&str, CbuAttributeGroup> {
    let (input, _) = tuple((ws, char('('), ws, tag("group")))(input)?;
    let (input, name) = kv_string(":name")(input)?;
    let (input, required) = opt(kv_attr_list(":required"))(input)?;
    let (input, optional) = opt(kv_attr_list(":optional"))(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((
        input,
        CbuAttributeGroup {
            name,
            required: required.unwrap_or_default(),
            optional: optional.unwrap_or_default(),
        },
    ))
}

/// Parse the attributes section
fn parse_attributes_section(input: &str) -> IResult<&str, CbuAttributesSpec> {
    let (input, _) = tuple((ws, char('('), ws, tag("attributes")))(input)?;
    let (input, groups) = many0(parse_attribute_group)(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((input, CbuAttributesSpec { groups }))
}

/// Parse a state definition
fn parse_state_def(input: &str) -> IResult<&str, CbuState> {
    let (input, _) = tuple((ws, char('('), ws, tag("state"), ws))(input)?;
    let (input, name) = quoted_string(input)?;
    let (input, description) = opt(kv_string(":description"))(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((input, CbuState { name, description }))
}

/// Parse the states section
fn parse_states_section(input: &str) -> IResult<&str, CbuStateMachine> {
    let (input, _) = tuple((ws, char('('), ws, tag("states")))(input)?;
    let (input, initial) = kv_string(":initial")(input)?;
    let (input, finals) = kv_string_list(":final")(input)?;
    let (input, states) = many0(parse_state_def)(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((
        input,
        CbuStateMachine {
            initial,
            finals,
            states,
            transitions: vec![], // Filled in separately
        },
    ))
}

/// Parse a transition definition
fn parse_transition_def(input: &str) -> IResult<&str, CbuTransition> {
    let (input, _) = tuple((ws, char('('), ws, tag("->"), ws))(input)?;
    let (input, from) = quoted_string(input)?;
    let (input, _) = ws(input)?;
    let (input, to) = quoted_string(input)?;
    let (input, verb) = kv_string(":verb")(input)?;
    let (input, chunks) = opt(kv_string_list(":chunks"))(input)?;
    let (input, preconditions) = opt(kv_attr_list(":preconditions"))(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((
        input,
        CbuTransition {
            from,
            to,
            verb,
            chunks: chunks.unwrap_or_default(),
            preconditions: preconditions.unwrap_or_default(),
        },
    ))
}

/// Parse the transitions section
fn parse_transitions_section(input: &str) -> IResult<&str, Vec<CbuTransition>> {
    let (input, _) = tuple((ws, char('('), ws, tag("transitions")))(input)?;
    let (input, transitions) = many0(parse_transition_def)(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((input, transitions))
}

/// Parse a role definition
fn parse_role_def(input: &str) -> IResult<&str, CbuRoleSpec> {
    let (input, _) = tuple((ws, char('('), ws, tag("role"), ws))(input)?;
    let (input, name) = quoted_string(input)?;
    let (input, min) = kv_number(":min")(input)?;
    let (input, max) = opt(kv_number(":max"))(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((input, CbuRoleSpec { name, min, max }))
}

/// Parse the roles section
fn parse_roles_section(input: &str) -> IResult<&str, Vec<CbuRoleSpec>> {
    let (input, _) = tuple((ws, char('('), ws, tag("roles")))(input)?;
    let (input, roles) = many0(parse_role_def)(input)?;
    let (input, _) = tuple((ws, char(')')))(input)?;

    Ok((input, roles))
}

/// Parse the complete CBU Model
fn parse_cbu_model(input: &str) -> IResult<&str, CbuModel> {
    let (input, _) = tuple((ws, char('('), ws, tag("cbu-model")))(input)?;

    // Parse header fields
    let (input, id) = kv_string(":id")(input)?;
    let (input, version) = kv_string(":version")(input)?;
    let (input, description) = opt(kv_string(":description"))(input)?;
    let (input, applies_to) = opt(kv_string_list(":applies-to"))(input)?;

    // Parse sections
    let (input, attributes) = parse_attributes_section(input)?;
    let (input, mut states) = parse_states_section(input)?;
    let (input, transitions) = parse_transitions_section(input)?;
    let (input, roles) = parse_roles_section(input)?;

    let (input, _) = tuple((ws, char(')')))(input)?;

    // Merge transitions into state machine
    states.transitions = transitions;

    Ok((
        input,
        CbuModel {
            id,
            version,
            description,
            applies_to: applies_to.unwrap_or_default(),
            attributes,
            states,
            roles,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quoted_string() {
        let (remaining, result) = quoted_string(r#""hello world""#).unwrap();
        assert_eq!(result, "hello world");
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_string_list() {
        let (remaining, result) = string_list(r#"["a", "b", "c"]"#).unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_attr_ref() {
        let (remaining, result) = attr_ref(r#"@attr("CBU.LEGAL_NAME")"#).unwrap();
        assert_eq!(result, "CBU.LEGAL_NAME");
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_attr_ref_list() {
        let (remaining, result) = attr_ref_list(r#"[@attr("ATTR1"), @attr("ATTR2")]"#).unwrap();
        assert_eq!(result, vec!["ATTR1", "ATTR2"]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_attribute_group() {
        let input = r#"(group :name "core"
            :required [@attr("ATTR1"), @attr("ATTR2")]
            :optional [@attr("ATTR3")])"#;

        let (remaining, group) = parse_attribute_group(input).unwrap();
        assert_eq!(group.name, "core");
        assert_eq!(group.required, vec!["ATTR1", "ATTR2"]);
        assert_eq!(group.optional, vec!["ATTR3"]);
        assert_eq!(remaining.trim(), "");
    }

    #[test]
    fn test_parse_state_def() {
        let input = r#"(state "Active" :description "CBU is active")"#;
        let (remaining, state) = parse_state_def(input).unwrap();
        assert_eq!(state.name, "Active");
        assert_eq!(state.description, Some("CBU is active".to_string()));
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_transition_def() {
        let input = r#"(-> "Proposed" "Active" :verb "cbu.approve" :chunks ["core", "contact"] :preconditions [@attr("ATTR1")])"#;
        let (remaining, trans) = parse_transition_def(input).unwrap();
        assert_eq!(trans.from, "Proposed");
        assert_eq!(trans.to, "Active");
        assert_eq!(trans.verb, "cbu.approve");
        assert_eq!(trans.chunks, vec!["core", "contact"]);
        assert_eq!(trans.preconditions, vec!["ATTR1"]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_transition_def_no_chunks() {
        let input =
            r#"(-> "Proposed" "Active" :verb "cbu.approve" :preconditions [@attr("ATTR1")])"#;
        let (remaining, trans) = parse_transition_def(input).unwrap();
        assert_eq!(trans.from, "Proposed");
        assert_eq!(trans.to, "Active");
        assert_eq!(trans.verb, "cbu.approve");
        assert_eq!(trans.chunks, Vec::<String>::new());
        assert_eq!(trans.preconditions, vec!["ATTR1"]);
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_role_def() {
        let input = r#"(role "BeneficialOwner" :min 1 :max 10)"#;
        let (remaining, role) = parse_role_def(input).unwrap();
        assert_eq!(role.name, "BeneficialOwner");
        assert_eq!(role.min, 1);
        assert_eq!(role.max, Some(10));
        assert_eq!(remaining, "");
    }

    #[test]
    fn test_parse_minimal_cbu_model() {
        let input = r#"
        (cbu-model
          :id "CBU.TEST"
          :version "1.0"

          (attributes
            (group :name "core"
              :required [@attr("ATTR1")]))

          (states
            :initial "Proposed"
            :final ["Closed"]
            (state "Proposed" :description "Initial state")
            (state "Closed" :description "Final state"))

          (transitions
            (-> "Proposed" "Closed" :verb "cbu.close" :chunks ["core"] :preconditions []))

          (roles
            (role "Owner" :min 1)))
        "#;

        let model = CbuModelParser::parse_str(input).unwrap();
        assert_eq!(model.id, "CBU.TEST");
        assert_eq!(model.version, "1.0");
        assert_eq!(model.attributes.groups.len(), 1);
        assert_eq!(model.states.states.len(), 2);
        assert_eq!(model.states.transitions.len(), 1);
        assert_eq!(model.states.transitions[0].chunks, vec!["core"]);
        assert_eq!(model.roles.len(), 1);
    }

    #[test]
    fn test_parse_full_cbu_model() {
        use crate::cbu_model_dsl::ebnf::CBU_MODEL_EXAMPLE;

        let model = CbuModelParser::parse_str(CBU_MODEL_EXAMPLE).unwrap();
        assert_eq!(model.id, "CBU.GENERIC");
        assert_eq!(model.version, "1.0");
        assert_eq!(model.applies_to, vec!["Fund", "SPV", "Corporation"]);
        assert_eq!(model.attributes.groups.len(), 3);
        assert_eq!(model.states.states.len(), 7);
        assert_eq!(model.states.transitions.len(), 8);
        assert_eq!(model.roles.len(), 4);
    }
}
