//! Executable Subset Validator for DSL
//!
//! Provides fast pre-parse validation that DSL conforms to the executable
//! subset grammar. This catches LLM hallucinations before they reach the parser.
//!
//! # Grammar
//!
//! See `grammar.ebnf` for the full EBNF specification.
//!
//! # Key Constraints
//!
//! - NO nested verb calls (single-level only)
//! - NO embedded SQL or complex expressions
//! - Pure S-expression syntax
//!
//! # Usage
//!
//! ```
//! use dsl_core::validator::validate_executable_subset;
//!
//! let result = validate_executable_subset("(cbu.create :name \"Test\")");
//! assert!(result.valid);
//! assert_eq!(result.stats.statement_count, 1);
//! ```

use thiserror::Error;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Validation error for DSL syntax
#[derive(Debug, Error, Clone)]
pub enum ValidationError {
    #[error("Unclosed parenthesis at position {0}")]
    UnclosedParen(usize),

    #[error("Unclosed string starting at position {0}")]
    UnclosedString(usize),

    #[error("Unclosed entity reference starting at position {0}")]
    UnclosedEntityRef(usize),

    #[error("Unclosed list starting at position {0}")]
    UnclosedList(usize),

    #[error("Unclosed map starting at position {0}")]
    UnclosedMap(usize),

    #[error("Invalid character '{0}' at position {1}")]
    InvalidChar(char, usize),

    #[error("Invalid verb format at position {0}: expected 'domain.verb'")]
    InvalidVerbFormat(usize),

    #[error("Invalid symbol reference at position {0}: expected '@identifier'")]
    InvalidSymbolRef(usize),

    #[error("Invalid keyword at position {0}: expected ':identifier'")]
    InvalidKeyword(usize),

    #[error("Nested verb calls not allowed (position {0})")]
    NestedVerbCall(usize),

    #[error("Empty verb call at position {0}")]
    EmptyVerbCall(usize),

    #[error("Unsupported construct '{0}' at position {1}")]
    UnsupportedConstruct(String, usize),
}

// =============================================================================
// VALIDATION RESULT
// =============================================================================

/// Result of validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub stats: ValidationStats,
}

impl ValidationResult {
    /// Create a valid result with default stats
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
            stats: ValidationStats::default(),
        }
    }

    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Statistics gathered during validation
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    pub statement_count: usize,
    pub entity_ref_count: usize,
    pub symbol_ref_count: usize,
    pub binding_count: usize,
}

// =============================================================================
// VALIDATOR
// =============================================================================

/// Validate DSL source conforms to executable subset
pub fn validate_executable_subset(source: &str) -> ValidationResult {
    let validator = SubsetValidator::new(source);
    validator.validate()
}

/// Internal validator state
struct SubsetValidator<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    errors: Vec<ValidationError>,
    warnings: Vec<String>,
    stats: ValidationStats,
    paren_depth: usize,
    bracket_depth: usize,
    brace_depth: usize,
}

impl<'a> SubsetValidator<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            errors: vec![],
            warnings: vec![],
            stats: ValidationStats::default(),
            paren_depth: 0,
            bracket_depth: 0,
            brace_depth: 0,
        }
    }

    fn validate(mut self) -> ValidationResult {
        while let Some((pos, ch)) = self.chars.next() {
            match ch {
                '(' => self.validate_verb_call(pos),
                ';' => self.skip_comment(),
                ' ' | '\t' | '\n' | '\r' => continue,
                _ => {
                    self.errors.push(ValidationError::InvalidChar(ch, pos));
                }
            }
        }

        // Check for unclosed delimiters
        if self.paren_depth > 0 {
            self.errors
                .push(ValidationError::UnclosedParen(self.source.len()));
        }
        if self.bracket_depth > 0 {
            self.errors
                .push(ValidationError::UnclosedList(self.source.len()));
        }
        if self.brace_depth > 0 {
            self.errors
                .push(ValidationError::UnclosedMap(self.source.len()));
        }

        ValidationResult {
            valid: self.errors.is_empty(),
            errors: self.errors,
            warnings: self.warnings,
            stats: self.stats,
        }
    }

    fn validate_verb_call(&mut self, start_pos: usize) {
        self.paren_depth += 1;
        self.stats.statement_count += 1;

        self.skip_whitespace();

        // Check for empty verb call
        if self.peek_char() == Some(')') {
            self.errors.push(ValidationError::EmptyVerbCall(start_pos));
            self.chars.next();
            self.paren_depth -= 1;
            return;
        }

        // Expect verb FQN: domain.verb
        let verb_start = self.current_pos();
        if !self.validate_verb_fqn() {
            self.errors
                .push(ValidationError::InvalidVerbFormat(verb_start));
        }

        // Validate arguments until closing paren
        loop {
            self.skip_whitespace();

            match self.peek_char() {
                Some(')') => {
                    self.chars.next();
                    self.paren_depth -= 1;
                    return;
                }
                Some(':') => {
                    self.validate_argument();
                }
                Some('(') => {
                    // Nested verb calls not supported in executable subset
                    let pos = self.current_pos();
                    self.errors.push(ValidationError::NestedVerbCall(pos));
                    self.skip_until(')');
                }
                None => {
                    self.errors.push(ValidationError::UnclosedParen(start_pos));
                    return;
                }
                Some(ch) => {
                    let pos = self.current_pos();
                    self.errors.push(ValidationError::InvalidChar(ch, pos));
                    self.chars.next();
                }
            }
        }
    }

    fn validate_verb_fqn(&mut self) -> bool {
        // domain.verb pattern
        if !self.validate_identifier() {
            return false;
        }

        if self.peek_char() != Some('.') {
            return false;
        }
        self.chars.next();

        self.validate_identifier_or_kebab()
    }

    fn validate_argument(&mut self) {
        // Expect :keyword
        if self.peek_char() != Some(':') {
            return;
        }
        self.chars.next();

        // Handle :as binding specially
        let keyword = self.read_identifier_or_kebab();

        if keyword == "as" {
            self.skip_whitespace();
            if !self.validate_symbol_ref() {
                let pos = self.current_pos();
                self.errors.push(ValidationError::InvalidSymbolRef(pos));
            }
            self.stats.binding_count += 1;
            return;
        }

        // Validate argument value
        self.skip_whitespace();
        self.validate_arg_value();
    }

    fn validate_arg_value(&mut self) {
        match self.peek_char() {
            Some('"') => self.validate_string(),
            Some('@') => {
                if !self.validate_symbol_ref() {
                    let pos = self.current_pos();
                    self.errors.push(ValidationError::InvalidSymbolRef(pos));
                }
                self.stats.symbol_ref_count += 1;
            }
            Some('<') => {
                if !self.validate_entity_ref() {
                    let pos = self.current_pos();
                    self.errors.push(ValidationError::UnclosedEntityRef(pos));
                }
                self.stats.entity_ref_count += 1;
            }
            Some('[') => self.validate_list(),
            Some('{') => self.validate_map(),
            Some(c) if c.is_ascii_digit() || c == '-' => self.validate_number(),
            Some('t') | Some('f') => self.validate_boolean(),
            Some('n') => self.validate_nil(),
            _ => {}
        }
    }

    fn validate_string(&mut self) {
        let start = self.current_pos();
        self.chars.next(); // consume opening "

        loop {
            match self.chars.next() {
                Some((_, '"')) => return,
                Some((_, '\\')) => {
                    self.chars.next();
                } // skip escaped char
                Some(_) => continue,
                None => {
                    self.errors.push(ValidationError::UnclosedString(start));
                    return;
                }
            }
        }
    }

    fn validate_symbol_ref(&mut self) -> bool {
        if self.peek_char() != Some('@') {
            return false;
        }
        self.chars.next();
        self.validate_identifier()
    }

    fn validate_entity_ref(&mut self) -> bool {
        let start = self.current_pos();
        if self.peek_char() != Some('<') {
            return false;
        }
        self.chars.next();

        // Read until >
        loop {
            match self.chars.next() {
                Some((_, '>')) => return true,
                Some(_) => continue,
                None => {
                    self.errors.push(ValidationError::UnclosedEntityRef(start));
                    return false;
                }
            }
        }
    }

    fn validate_list(&mut self) {
        let start = self.current_pos();
        self.chars.next(); // consume [
        self.bracket_depth += 1;

        loop {
            self.skip_whitespace();
            match self.peek_char() {
                Some(']') => {
                    self.chars.next();
                    self.bracket_depth -= 1;
                    return;
                }
                Some(',') => {
                    self.chars.next();
                }
                None => {
                    self.errors.push(ValidationError::UnclosedList(start));
                    return;
                }
                _ => self.validate_arg_value(),
            }
        }
    }

    fn validate_map(&mut self) {
        let start = self.current_pos();
        self.chars.next(); // consume {
        self.brace_depth += 1;

        loop {
            self.skip_whitespace();
            match self.peek_char() {
                Some('}') => {
                    self.chars.next();
                    self.brace_depth -= 1;
                    return;
                }
                Some(',') => {
                    self.chars.next();
                }
                Some(':') | Some('"') => {
                    // Key
                    if self.peek_char() == Some(':') {
                        self.chars.next();
                        self.read_identifier_or_kebab();
                    } else {
                        self.validate_string();
                    }
                    self.skip_whitespace();
                    // Value
                    self.validate_arg_value();
                }
                None => {
                    self.errors.push(ValidationError::UnclosedMap(start));
                    return;
                }
                _ => {
                    self.chars.next();
                }
            }
        }
    }

    fn validate_identifier(&mut self) -> bool {
        match self.peek_char() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                self.chars.next();
                while let Some(c) = self.peek_char() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn validate_identifier_or_kebab(&mut self) -> bool {
        match self.peek_char() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                self.chars.next();
                while let Some(c) = self.peek_char() {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn validate_number(&mut self) {
        if self.peek_char() == Some('-') {
            self.chars.next();
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn validate_boolean(&mut self) {
        // Check for "true" or "false"
        let s: String = self.chars.clone().take(5).map(|(_, c)| c).collect();

        if s.starts_with("true") {
            for _ in 0..4 {
                self.chars.next();
            }
        } else if s.starts_with("false") {
            for _ in 0..5 {
                self.chars.next();
            }
        }
    }

    fn validate_nil(&mut self) {
        let s: String = self.chars.clone().take(3).map(|(_, c)| c).collect();

        if s == "nil" {
            for _ in 0..3 {
                self.chars.next();
            }
        }
    }

    // Helper methods
    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, c)| *c)
    }

    fn current_pos(&mut self) -> usize {
        self.chars
            .peek()
            .map(|(p, _)| *p)
            .unwrap_or(self.source.len())
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) {
        while let Some((_, c)) = self.chars.next() {
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_until(&mut self, target: char) {
        while let Some((_, c)) = self.chars.next() {
            if c == target {
                break;
            }
        }
    }

    fn read_identifier_or_kebab(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        s
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple() {
        let result = validate_executable_subset("(cbu.create :name \"Test\")");
        assert!(result.valid);
        assert_eq!(result.stats.statement_count, 1);
    }

    #[test]
    fn test_valid_with_binding() {
        let result = validate_executable_subset("(cbu.create :name \"Test\" :as @cbu)");
        assert!(result.valid);
        assert_eq!(result.stats.binding_count, 1);
    }

    #[test]
    fn test_valid_with_entity_ref() {
        let result = validate_executable_subset("(session.load-cluster :client <Allianz>)");
        assert!(result.valid);
        assert_eq!(result.stats.entity_ref_count, 1);
    }

    #[test]
    fn test_invalid_unclosed_paren() {
        let result = validate_executable_subset("(cbu.create :name \"Test\"");
        assert!(!result.valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::UnclosedParen(_)
        ));
    }

    #[test]
    fn test_invalid_nested_verb() {
        let result = validate_executable_subset("(cbu.create :name (other.verb))");
        assert!(!result.valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::NestedVerbCall(_)
        ));
    }

    #[test]
    fn test_valid_list() {
        let result =
            validate_executable_subset("(scope.commit :entity-ids [\"uuid1\", \"uuid2\"])");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_multi_statement() {
        let result = validate_executable_subset(
            r#"
            (cbu.create :name "Fund A" :as @cbu1)
            (cbu.create :name "Fund B" :as @cbu2)
            (entity.assign :cbu-id @cbu1 :entity-id <John Smith>)
        "#,
        );
        assert!(result.valid);
        assert_eq!(result.stats.statement_count, 3);
        assert_eq!(result.stats.binding_count, 2);
        assert_eq!(result.stats.entity_ref_count, 1);
    }

    #[test]
    fn test_valid_with_comment() {
        let result = validate_executable_subset(
            r#"
            ; This is a comment
            (cbu.create :name "Test")
        "#,
        );
        assert!(result.valid);
        assert_eq!(result.stats.statement_count, 1);
    }

    #[test]
    fn test_invalid_unclosed_string() {
        let result = validate_executable_subset("(cbu.create :name \"Test)");
        assert!(!result.valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::UnclosedString(_)
        ));
    }

    #[test]
    fn test_invalid_unclosed_entity_ref() {
        let result = validate_executable_subset("(session.load :client <Allianz)");
        assert!(!result.valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::UnclosedEntityRef(_)
        ));
    }

    #[test]
    fn test_valid_symbol_ref() {
        let result = validate_executable_subset("(cbu.delete :id @my_cbu)");
        assert!(result.valid);
        assert_eq!(result.stats.symbol_ref_count, 1);
    }

    #[test]
    fn test_valid_boolean() {
        let result = validate_executable_subset("(entity.update :active true)");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_nil() {
        let result = validate_executable_subset("(entity.update :manager nil)");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_number() {
        let result = validate_executable_subset("(entity.update :balance 123.45)");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_negative_number() {
        let result = validate_executable_subset("(entity.update :balance -99.50)");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_map() {
        let result = validate_executable_subset("(entity.update :attrs {:name \"Test\" :age 30})");
        assert!(result.valid);
    }

    #[test]
    fn test_empty_verb_call() {
        let result = validate_executable_subset("()");
        assert!(!result.valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::EmptyVerbCall(_)
        ));
    }

    #[test]
    fn test_invalid_verb_format() {
        let result = validate_executable_subset("(create :name \"Test\")");
        assert!(!result.valid);
        assert!(matches!(
            result.errors[0],
            ValidationError::InvalidVerbFormat(_)
        ));
    }

    #[test]
    fn test_valid_list_with_entity_refs() {
        let result = validate_executable_subset(
            "(scope.commit :entities [<Alpha Corp>, <Beta Ltd>, <Gamma Inc>])",
        );
        assert!(result.valid);
        assert_eq!(result.stats.entity_ref_count, 3);
    }
}
