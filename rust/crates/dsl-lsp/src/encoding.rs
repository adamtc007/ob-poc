//! UTF-16 / UTF-8 Position Encoding for LSP
//!
//! LSP positions are (line, character) where character is measured in encoding units:
//! - UTF-16: Default for most editors (VS Code, Zed)
//! - UTF-8: Some editors support this for efficiency
//!
//! This module handles conversion between byte offsets (from parser spans)
//! and LSP positions in the negotiated encoding.

#![allow(dead_code)] // Public API - functions used by LSP server

use tower_lsp::lsp_types::Position;

/// Position encoding mode (negotiated at initialize)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PositionEncoding {
    /// UTF-16 code units (default for LSP)
    #[default]
    Utf16,
    /// UTF-8 bytes
    Utf8,
}

/// Convert byte offset to LSP Position (line, character)
///
/// # Arguments
/// * `text` - The full document text
/// * `offset` - Byte offset into the text
/// * `encoding` - Position encoding mode
///
/// # Returns
/// LSP Position with 0-based line and character
pub fn offset_to_position(text: &str, offset: usize, encoding: PositionEncoding) -> Position {
    let offset = offset.min(text.len());
    let mut line = 0u32;
    let mut line_start_offset = 0usize;

    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            line_start_offset = i + 1;
        }
    }

    let line_text = &text[line_start_offset..offset];
    let character = match encoding {
        PositionEncoding::Utf16 => line_text.encode_utf16().count() as u32,
        PositionEncoding::Utf8 => line_text.len() as u32,
    };

    Position { line, character }
}

/// Convert LSP Position to byte offset
///
/// # Arguments
/// * `text` - The full document text
/// * `position` - LSP Position (0-based line, character)
/// * `encoding` - Position encoding mode
///
/// # Returns
/// Byte offset, or None if position is invalid
pub fn position_to_offset(
    text: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Option<usize> {
    let mut current_line = 0u32;
    let mut line_start = 0usize;

    for (i, c) in text.char_indices() {
        if current_line == position.line {
            // Found the line, now find character offset
            let line_end = text[i..].find('\n').map(|p| i + p).unwrap_or(text.len());
            let line_text = &text[line_start..line_end];

            let byte_offset = match encoding {
                PositionEncoding::Utf16 => {
                    utf16_offset_to_byte_offset(line_text, position.character as usize)
                }
                PositionEncoding::Utf8 => Some((position.character as usize).min(line_text.len())),
            };

            return byte_offset.map(|o| line_start + o.min(line_text.len()));
        }
        if c == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }

    // Position is at or past end of file
    if current_line == position.line {
        // Line exists but we reached EOF - return end of text
        let line_text = &text[line_start..];
        let byte_offset = match encoding {
            PositionEncoding::Utf16 => {
                utf16_offset_to_byte_offset(line_text, position.character as usize)
            }
            PositionEncoding::Utf8 => Some((position.character as usize).min(line_text.len())),
        };
        byte_offset.map(|o| line_start + o.min(line_text.len()))
    } else {
        None
    }
}

/// Convert UTF-16 offset to byte offset within a line
fn utf16_offset_to_byte_offset(text: &str, utf16_offset: usize) -> Option<usize> {
    let mut utf16_count = 0usize;
    for (byte_idx, c) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return Some(byte_idx);
        }
        utf16_count += c.len_utf16();
    }
    // If we've counted enough UTF-16 units, return end of text
    if utf16_count >= utf16_offset {
        Some(text.len())
    } else {
        None
    }
}

/// Create an LSP Range from byte offsets
pub fn span_to_range(
    start_offset: usize,
    end_offset: usize,
    text: &str,
    encoding: PositionEncoding,
) -> tower_lsp::lsp_types::Range {
    tower_lsp::lsp_types::Range {
        start: offset_to_position(text, start_offset, encoding),
        end: offset_to_position(text, end_offset, encoding),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_position_simple() {
        let text = "hello\nworld";
        // "hello\n" is 6 bytes, "world" starts at offset 6

        // First line, first char
        let pos = offset_to_position(text, 0, PositionEncoding::Utf8);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);

        // First line, last char before newline
        let pos = offset_to_position(text, 5, PositionEncoding::Utf8);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 5);

        // Second line, first char
        let pos = offset_to_position(text, 6, PositionEncoding::Utf8);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);

        // Second line, third char
        let pos = offset_to_position(text, 9, PositionEncoding::Utf8);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 3);
    }

    #[test]
    fn test_offset_to_position_utf16_emoji() {
        // Emoji 🎉 is 4 bytes in UTF-8 but 2 code units in UTF-16
        let text = "hi 🎉 there";

        // After emoji in UTF-8: "hi 🎉" is 3 + 4 = 7 bytes
        let pos_utf8 = offset_to_position(text, 7, PositionEncoding::Utf8);
        assert_eq!(pos_utf8.character, 7);

        // After emoji in UTF-16: "hi 🎉" is 3 + 2 = 5 code units
        let pos_utf16 = offset_to_position(text, 7, PositionEncoding::Utf16);
        assert_eq!(pos_utf16.character, 5);
    }

    #[test]
    fn test_position_to_offset_simple() {
        let text = "hello\nworld";

        // First line, first char
        let offset = position_to_offset(
            text,
            Position {
                line: 0,
                character: 0,
            },
            PositionEncoding::Utf8,
        );
        assert_eq!(offset, Some(0));

        // Second line, third char
        let offset = position_to_offset(
            text,
            Position {
                line: 1,
                character: 3,
            },
            PositionEncoding::Utf8,
        );
        assert_eq!(offset, Some(9)); // 6 (newline offset) + 3
    }

    #[test]
    fn test_position_to_offset_utf16_emoji() {
        let text = "🎉 test";
        // 🎉 is 4 bytes, 2 UTF-16 units

        // UTF-16 position 2 should be byte offset 4 (after emoji)
        let offset = position_to_offset(
            text,
            Position {
                line: 0,
                character: 2,
            },
            PositionEncoding::Utf16,
        );
        assert_eq!(offset, Some(4));

        // UTF-8 position 4 should also be byte offset 4
        let offset = position_to_offset(
            text,
            Position {
                line: 0,
                character: 4,
            },
            PositionEncoding::Utf8,
        );
        assert_eq!(offset, Some(4));
    }

    #[test]
    fn test_roundtrip() {
        let text = "line1\nline2 with 🎉\nline3";

        for encoding in [PositionEncoding::Utf8, PositionEncoding::Utf16] {
            // Only test at character boundaries (not inside multi-byte chars)
            for (offset, _) in text.char_indices() {
                let pos = offset_to_position(text, offset, encoding);
                let recovered = position_to_offset(text, pos, encoding);
                assert_eq!(
                    recovered,
                    Some(offset),
                    "Roundtrip failed at offset {} with {:?}",
                    offset,
                    encoding
                );
            }
            // Also test at end of text
            let pos = offset_to_position(text, text.len(), encoding);
            let recovered = position_to_offset(text, pos, encoding);
            assert_eq!(
                recovered,
                Some(text.len()),
                "Roundtrip failed at end offset with {:?}",
                encoding
            );
        }
    }

    #[test]
    fn test_span_to_range() {
        let text = "(cbu.create :name \"Fund\")";
        let range = span_to_range(0, text.len(), text, PositionEncoding::Utf8);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, text.len() as u32);
    }
}
