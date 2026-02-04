//! User Reply Parser
//!
//! Parses user input into structured UserReply enum.
//! Priority order:
//! 1. CONFIRM token match
//! 2. A/B/C or 1/2/3 selection
//! 3. TYPE <text> prefix
//! 4. NARROW <term> prefix
//! 5. CANCEL/NO/ABORT
//! 6. Fallthrough → treat as TYPE

use ob_poc_types::{DecisionKind, DecisionPacket};
use thiserror::Error;

/// User reply types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserReply {
    /// User selected option by index (0-indexed)
    Select { index: usize },
    /// User confirmed with token
    Confirm { token: String },
    /// User typed free text
    Type { text: String },
    /// User wants to narrow/filter
    Narrow { term: String },
    /// User cancelled
    Cancel,
}

/// Errors that can occur during reply parsing
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Selection index {0} is out of range (max: {1})")]
    SelectionOutOfRange(usize, usize),

    #[error("Invalid selection format: {0}")]
    InvalidSelection(String),

    #[error("Empty input")]
    EmptyInput,
}

/// Parse user input into a UserReply based on the active DecisionPacket.
///
/// The parser uses a priority-based approach:
/// 1. Check for CONFIRM/yes/ok (for Proposal packets)
/// 2. Check for letter selection (A, B, C...)
/// 3. Check for number selection (1, 2, 3...)
/// 4. Check for TYPE prefix
/// 5. Check for NARROW prefix
/// 6. Check for CANCEL/no/abort
/// 7. Fallthrough: treat as TYPE text
///
/// # Arguments
/// * `input` - The raw user input string
/// * `packet` - The active DecisionPacket for context
///
/// # Returns
/// * `Ok(UserReply)` - The parsed reply
/// * `Err(ParseError)` - If parsing fails
pub fn parse_user_reply(input: &str, packet: &DecisionPacket) -> Result<UserReply, ParseError> {
    let input = input.trim();

    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let input_lower = input.to_lowercase();
    let max_choices = packet.choices.len();

    // 1. Check for CONFIRM (only valid for Proposal packets)
    if matches!(packet.kind, DecisionKind::Proposal) && is_confirm_phrase(&input_lower) {
        if let Some(token) = &packet.confirm_token {
            return Ok(UserReply::Confirm {
                token: token.clone(),
            });
        }
    }

    // 2. Check for letter selection (A, B, C, ...)
    if let Some(index) = parse_letter_selection(&input_lower, max_choices) {
        return Ok(UserReply::Select { index });
    }

    // 3. Check for number selection (1, 2, 3, ...)
    if let Some(index) = parse_number_selection(&input_lower, max_choices) {
        return Ok(UserReply::Select { index });
    }

    // 4. Check for TYPE prefix
    if let Some(text) = input_lower.strip_prefix("type ") {
        return Ok(UserReply::Type {
            text: text.trim().to_string(),
        });
    }
    if let Some(text) = input_lower.strip_prefix("type: ") {
        return Ok(UserReply::Type {
            text: text.trim().to_string(),
        });
    }

    // 5. Check for NARROW prefix
    if let Some(term) = input_lower.strip_prefix("narrow ") {
        return Ok(UserReply::Narrow {
            term: term.trim().to_string(),
        });
    }
    if let Some(term) = input_lower.strip_prefix("narrow: ") {
        return Ok(UserReply::Narrow {
            term: term.trim().to_string(),
        });
    }
    if let Some(term) = input_lower.strip_prefix("filter ") {
        return Ok(UserReply::Narrow {
            term: term.trim().to_string(),
        });
    }

    // 6. Check for CANCEL phrases
    if is_cancel_phrase(&input_lower) {
        return Ok(UserReply::Cancel);
    }

    // 7. Fallthrough: treat as TYPE text (preserve original case)
    Ok(UserReply::Type {
        text: input.to_string(),
    })
}

/// Check if input is a confirm phrase
fn is_confirm_phrase(input: &str) -> bool {
    matches!(
        input,
        "confirm" | "yes" | "ok" | "okay" | "y" | "proceed" | "execute" | "run" | "do it"
    )
}

/// Check if input is a cancel phrase
fn is_cancel_phrase(input: &str) -> bool {
    matches!(
        input,
        "cancel" | "no" | "abort" | "stop" | "quit" | "exit" | "nevermind" | "never mind" | "n"
    )
}

/// Parse letter selection (A, B, C, ...)
/// Returns 0-indexed selection index
fn parse_letter_selection(input: &str, max_choices: usize) -> Option<usize> {
    let input = input.trim();

    // Single letter
    if input.len() == 1 {
        let c = input.chars().next()?;
        if c.is_ascii_alphabetic() {
            let index = (c.to_ascii_uppercase() as u8 - b'A') as usize;
            if index < max_choices {
                return Some(index);
            }
        }
    }

    // "option A", "choice B", etc.
    for prefix in ["option ", "choice ", "select ", "pick "] {
        if let Some(rest) = input.strip_prefix(prefix) {
            let rest = rest.trim();
            if rest.len() == 1 {
                let c = rest.chars().next()?;
                if c.is_ascii_alphabetic() {
                    let index = (c.to_ascii_uppercase() as u8 - b'A') as usize;
                    if index < max_choices {
                        return Some(index);
                    }
                }
            }
        }
    }

    None
}

/// Parse number selection (1, 2, 3, ...)
/// Returns 0-indexed selection index
fn parse_number_selection(input: &str, max_choices: usize) -> Option<usize> {
    let input = input.trim();

    // Try to parse as number
    if let Ok(num) = input.parse::<usize>() {
        if num >= 1 && num <= max_choices {
            return Some(num - 1); // Convert to 0-indexed
        }
    }

    // "option 1", "choice 2", etc.
    for prefix in ["option ", "choice ", "select ", "pick ", "number "] {
        if let Some(rest) = input.strip_prefix(prefix) {
            if let Ok(num) = rest.trim().parse::<usize>() {
                if num >= 1 && num <= max_choices {
                    return Some(num - 1);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_types::{
        ClarificationPayload, DecisionTrace, GroupClarificationPayload, UserChoice,
    };

    fn make_test_packet(kind: DecisionKind, num_choices: usize) -> DecisionPacket {
        let choices: Vec<UserChoice> = (0..num_choices)
            .map(|i| UserChoice {
                id: ((b'A' + i as u8) as char).to_string(),
                label: format!("Option {}", i + 1),
                description: format!("Description for option {}", i + 1),
                is_escape: false,
            })
            .collect();

        DecisionPacket {
            packet_id: "test".to_string(),
            kind: kind.clone(),
            session: ob_poc_types::SessionStateView::default(),
            utterance: "test input".to_string(),
            confirm_token: if matches!(kind, DecisionKind::Proposal) {
                Some("test-token".to_string())
            } else {
                None
            },
            payload: ClarificationPayload::Group(GroupClarificationPayload { options: vec![] }),
            prompt: "Test prompt".to_string(),
            choices,
            best_plan: None,
            alternatives: vec![],
            requires_confirm: true,
            trace: DecisionTrace {
                config_version: "1.0".to_string(),
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: true,
                embedding_model_id: None,
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: "test".to_string(),
            },
        }
    }

    #[test]
    fn test_parse_letter_selection() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        assert!(matches!(
            parse_user_reply("a", &packet),
            Ok(UserReply::Select { index: 0 })
        ));
        assert!(matches!(
            parse_user_reply("B", &packet),
            Ok(UserReply::Select { index: 1 })
        ));
        assert!(matches!(
            parse_user_reply("c", &packet),
            Ok(UserReply::Select { index: 2 })
        ));
    }

    #[test]
    fn test_parse_number_selection() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        assert!(matches!(
            parse_user_reply("1", &packet),
            Ok(UserReply::Select { index: 0 })
        ));
        assert!(matches!(
            parse_user_reply("2", &packet),
            Ok(UserReply::Select { index: 1 })
        ));
        assert!(matches!(
            parse_user_reply("3", &packet),
            Ok(UserReply::Select { index: 2 })
        ));
    }

    #[test]
    fn test_parse_confirm() {
        let packet = make_test_packet(DecisionKind::Proposal, 2);

        assert!(matches!(
            parse_user_reply("confirm", &packet),
            Ok(UserReply::Confirm { .. })
        ));
        assert!(matches!(
            parse_user_reply("yes", &packet),
            Ok(UserReply::Confirm { .. })
        ));
        assert!(matches!(
            parse_user_reply("ok", &packet),
            Ok(UserReply::Confirm { .. })
        ));
    }

    #[test]
    fn test_parse_cancel() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        assert!(matches!(
            parse_user_reply("cancel", &packet),
            Ok(UserReply::Cancel)
        ));
        assert!(matches!(
            parse_user_reply("no", &packet),
            Ok(UserReply::Cancel)
        ));
        assert!(matches!(
            parse_user_reply("abort", &packet),
            Ok(UserReply::Cancel)
        ));
    }

    #[test]
    fn test_parse_type_prefix() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        match parse_user_reply("type custom input", &packet) {
            Ok(UserReply::Type { text }) => assert_eq!(text, "custom input"),
            other => panic!("Expected Type, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_narrow_prefix() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        match parse_user_reply("narrow luxembourg", &packet) {
            Ok(UserReply::Narrow { term }) => assert_eq!(term, "luxembourg"),
            other => panic!("Expected Narrow, got {:?}", other),
        }

        match parse_user_reply("filter fund", &packet) {
            Ok(UserReply::Narrow { term }) => assert_eq!(term, "fund"),
            other => panic!("Expected Narrow, got {:?}", other),
        }
    }

    #[test]
    fn test_fallthrough_to_type() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        // Unrecognized input should fall through to Type
        match parse_user_reply("some random text", &packet) {
            Ok(UserReply::Type { text }) => assert_eq!(text, "some random text"),
            other => panic!("Expected Type fallthrough, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_input() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);
        assert!(matches!(
            parse_user_reply("", &packet),
            Err(ParseError::EmptyInput)
        ));
        assert!(matches!(
            parse_user_reply("   ", &packet),
            Err(ParseError::EmptyInput)
        ));
    }

    #[test]
    fn test_option_prefix_selection() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        assert!(matches!(
            parse_user_reply("option a", &packet),
            Ok(UserReply::Select { index: 0 })
        ));
        assert!(matches!(
            parse_user_reply("choice b", &packet),
            Ok(UserReply::Select { index: 1 })
        ));
        assert!(matches!(
            parse_user_reply("select 2", &packet),
            Ok(UserReply::Select { index: 1 })
        ));
    }

    #[test]
    fn test_out_of_range_selection_falls_through() {
        let packet = make_test_packet(DecisionKind::ClarifyGroup, 3);

        // "d" is out of range for 3 choices, should fall through to Type
        match parse_user_reply("d", &packet) {
            Ok(UserReply::Type { text }) => assert_eq!(text, "d"),
            other => panic!("Expected Type fallthrough, got {:?}", other),
        }

        // "5" is out of range, should fall through to Type
        match parse_user_reply("5", &packet) {
            Ok(UserReply::Type { text }) => assert_eq!(text, "5"),
            other => panic!("Expected Type fallthrough, got {:?}", other),
        }
    }
}
