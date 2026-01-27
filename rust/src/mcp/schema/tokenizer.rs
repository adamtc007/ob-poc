//! S-expression tokenizer with source spans
//!
//! Tokenizes DSL input into a stream of tokens, preserving source locations
//! for error reporting and diagnostics.

use std::fmt;

/// Source span (byte offsets)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// Token kinds
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// Opening parenthesis
    LParen,
    /// Closing parenthesis
    RParen,
    /// Keyword argument (starts with :)
    Keyword(String),
    /// Symbol/identifier
    Symbol(String),
    /// String literal (quoted)
    String(String),
    /// Integer literal
    Integer(i64),
    /// Float literal
    Float(f64),
    /// Boolean literal
    Bool(bool),
    /// Entity reference <name>
    EntityRef(String),
    /// Binding reference @name
    BindingRef(String),
    /// End of input
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::Keyword(k) => write!(f, ":{}", k),
            TokenKind::Symbol(s) => write!(f, "{}", s),
            TokenKind::String(s) => write!(f, "\"{}\"", s),
            TokenKind::Integer(i) => write!(f, "{}", i),
            TokenKind::Float(fl) => write!(f, "{}", fl),
            TokenKind::Bool(b) => write!(f, "{}", b),
            TokenKind::EntityRef(e) => write!(f, "<{}>", e),
            TokenKind::BindingRef(b) => write!(f, "@{}", b),
            TokenKind::Eof => write!(f, "EOF"),
        }
    }
}

/// Token with span
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Tokenizer for s-expressions
pub struct Tokenizer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    /// Current position (byte offset)
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
            pos: 0,
        }
    }

    /// Tokenize entire input
    pub fn tokenize(input: &str) -> Result<Vec<Token>, TokenError> {
        let mut tokenizer = Tokenizer::new(input);
        let mut tokens = Vec::new();

        loop {
            let token = tokenizer.next_token()?;
            let is_eof = matches!(token.kind, TokenKind::Eof);
            tokens.push(token);
            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }

    /// Get next token
    pub fn next_token(&mut self) -> Result<Token, TokenError> {
        self.skip_whitespace_and_comments();

        let start = self.current_pos();

        match self.peek_char() {
            None => Ok(Token::new(TokenKind::Eof, Span::new(start, start))),
            Some('(') => {
                self.advance();
                Ok(Token::new(
                    TokenKind::LParen,
                    Span::new(start, self.current_pos()),
                ))
            }
            Some(')') => {
                self.advance();
                Ok(Token::new(
                    TokenKind::RParen,
                    Span::new(start, self.current_pos()),
                ))
            }
            Some(':') => self.read_keyword(start),
            Some('"') => self.read_string(start),
            Some('<') => self.read_entity_ref(start),
            Some('@') => self.read_binding_ref(start),
            Some(c) if c.is_ascii_digit() || c == '-' || c == '+' => {
                // Could be number or symbol starting with - or +
                self.read_number_or_symbol(start)
            }
            Some(_) => self.read_symbol(start),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() {
                    self.advance();
                } else {
                    break;
                }
            }

            // Skip comments (;; to end of line)
            if self.peek_char() == Some(';') {
                while let Some(c) = self.peek_char() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn read_keyword(&mut self, start: usize) -> Result<Token, TokenError> {
        self.advance(); // consume ':'

        let name_start = self.current_pos();
        while let Some(c) = self.peek_char() {
            if is_symbol_char(c) {
                self.advance();
            } else {
                break;
            }
        }

        let name = &self.input[name_start..self.current_pos()];
        if name.is_empty() {
            return Err(TokenError {
                message: "Empty keyword".to_string(),
                span: Span::new(start, self.current_pos()),
            });
        }

        Ok(Token::new(
            TokenKind::Keyword(name.to_string()),
            Span::new(start, self.current_pos()),
        ))
    }

    fn read_string(&mut self, start: usize) -> Result<Token, TokenError> {
        self.advance(); // consume opening quote

        let mut value = String::new();

        loop {
            match self.peek_char() {
                None => {
                    return Err(TokenError {
                        message: "Unterminated string".to_string(),
                        span: Span::new(start, self.current_pos()),
                    });
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.peek_char() {
                        Some('n') => {
                            self.advance();
                            value.push('\n');
                        }
                        Some('t') => {
                            self.advance();
                            value.push('\t');
                        }
                        Some('r') => {
                            self.advance();
                            value.push('\r');
                        }
                        Some('\\') => {
                            self.advance();
                            value.push('\\');
                        }
                        Some('"') => {
                            self.advance();
                            value.push('"');
                        }
                        Some(c) => {
                            self.advance();
                            value.push(c);
                        }
                        None => {
                            return Err(TokenError {
                                message: "Unterminated escape sequence".to_string(),
                                span: Span::new(start, self.current_pos()),
                            });
                        }
                    }
                }
                Some(c) => {
                    self.advance();
                    value.push(c);
                }
            }
        }

        Ok(Token::new(
            TokenKind::String(value),
            Span::new(start, self.current_pos()),
        ))
    }

    fn read_entity_ref(&mut self, start: usize) -> Result<Token, TokenError> {
        self.advance(); // consume '<'

        let name_start = self.current_pos();
        while let Some(c) = self.peek_char() {
            if c == '>' {
                break;
            }
            self.advance();
        }

        let name = &self.input[name_start..self.current_pos()];

        if self.peek_char() != Some('>') {
            return Err(TokenError {
                message: "Unterminated entity reference".to_string(),
                span: Span::new(start, self.current_pos()),
            });
        }
        self.advance(); // consume '>'

        Ok(Token::new(
            TokenKind::EntityRef(name.to_string()),
            Span::new(start, self.current_pos()),
        ))
    }

    fn read_binding_ref(&mut self, start: usize) -> Result<Token, TokenError> {
        self.advance(); // consume '@'

        let name_start = self.current_pos();
        while let Some(c) = self.peek_char() {
            if is_symbol_char(c) {
                self.advance();
            } else {
                break;
            }
        }

        let name = &self.input[name_start..self.current_pos()];
        if name.is_empty() {
            return Err(TokenError {
                message: "Empty binding reference".to_string(),
                span: Span::new(start, self.current_pos()),
            });
        }

        Ok(Token::new(
            TokenKind::BindingRef(name.to_string()),
            Span::new(start, self.current_pos()),
        ))
    }

    fn read_number_or_symbol(&mut self, start: usize) -> Result<Token, TokenError> {
        let first = self.peek_char().unwrap();

        // Check if this looks like a number
        if first.is_ascii_digit() || ((first == '-' || first == '+') && self.peek_next_is_digit()) {
            self.read_number(start)
        } else {
            self.read_symbol(start)
        }
    }

    fn peek_next_is_digit(&self) -> bool {
        let mut chars = self.chars.clone();
        chars.next(); // skip current
        chars
            .peek()
            .map(|(_, c)| c.is_ascii_digit())
            .unwrap_or(false)
    }

    fn read_number(&mut self, start: usize) -> Result<Token, TokenError> {
        let num_start = self.current_pos();

        // Consume sign
        if let Some(c) = self.peek_char() {
            if c == '-' || c == '+' {
                self.advance();
            }
        }

        // Consume digits
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Check for decimal point
        let mut is_float = false;
        if self.peek_char() == Some('.') {
            // Look ahead to ensure it's followed by digit
            let mut chars = self.chars.clone();
            chars.next();
            if chars
                .peek()
                .map(|(_, c)| c.is_ascii_digit())
                .unwrap_or(false)
            {
                is_float = true;
                self.advance(); // consume '.'

                while let Some(c) = self.peek_char() {
                    if c.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }

        let text = &self.input[num_start..self.current_pos()];

        if is_float {
            let value: f64 = text.parse().map_err(|_| TokenError {
                message: format!("Invalid float: {}", text),
                span: Span::new(start, self.current_pos()),
            })?;
            Ok(Token::new(
                TokenKind::Float(value),
                Span::new(start, self.current_pos()),
            ))
        } else {
            let value: i64 = text.parse().map_err(|_| TokenError {
                message: format!("Invalid integer: {}", text),
                span: Span::new(start, self.current_pos()),
            })?;
            Ok(Token::new(
                TokenKind::Integer(value),
                Span::new(start, self.current_pos()),
            ))
        }
    }

    fn read_symbol(&mut self, start: usize) -> Result<Token, TokenError> {
        while let Some(c) = self.peek_char() {
            if is_symbol_char(c) {
                self.advance();
            } else {
                break;
            }
        }

        let text = &self.input[start..self.current_pos()];

        // Check for boolean literals
        let kind = match text.to_lowercase().as_str() {
            "true" | "#t" => TokenKind::Bool(true),
            "false" | "#f" | "nil" => TokenKind::Bool(false),
            _ => TokenKind::Symbol(text.to_string()),
        };

        Ok(Token::new(kind, Span::new(start, self.current_pos())))
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, c)| *c)
    }

    fn advance(&mut self) -> Option<char> {
        self.chars.next().map(|(i, c)| {
            self.pos = i + c.len_utf8();
            c
        })
    }

    fn current_pos(&mut self) -> usize {
        self.chars
            .peek()
            .map(|(i, _)| *i)
            .unwrap_or(self.input.len())
    }
}

/// Check if character is valid in a symbol
fn is_symbol_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/' || c == '?' || c == '!'
}

/// Tokenization error
#[derive(Debug, Clone)]
pub struct TokenError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.message, self.span)
    }
}

impl std::error::Error for TokenError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_sexpr() {
        let tokens = Tokenizer::tokenize("(view.drill :entity \"Allianz\")").unwrap();
        assert_eq!(tokens.len(), 6); // ( symbol keyword string ) EOF
        assert!(matches!(tokens[0].kind, TokenKind::LParen));
        assert!(matches!(&tokens[1].kind, TokenKind::Symbol(s) if s == "view.drill"));
        assert!(matches!(&tokens[2].kind, TokenKind::Keyword(k) if k == "entity"));
        assert!(matches!(&tokens[3].kind, TokenKind::String(s) if s == "Allianz"));
        assert!(matches!(tokens[4].kind, TokenKind::RParen));
        assert!(matches!(tokens[5].kind, TokenKind::Eof));
    }

    #[test]
    fn test_entity_ref() {
        let tokens = Tokenizer::tokenize("(drill <Allianz SE>)").unwrap();
        assert!(matches!(&tokens[2].kind, TokenKind::EntityRef(e) if e == "Allianz SE"));
    }

    #[test]
    fn test_binding_ref() {
        let tokens = Tokenizer::tokenize("(create :as @my_entity)").unwrap();
        assert!(matches!(&tokens[3].kind, TokenKind::BindingRef(b) if b == "my_entity"));
    }

    #[test]
    fn test_numbers() {
        let tokens = Tokenizer::tokenize("(foo 42 -10 3.14)").unwrap();
        assert!(matches!(tokens[2].kind, TokenKind::Integer(42)));
        assert!(matches!(tokens[3].kind, TokenKind::Integer(-10)));
        assert!(matches!(tokens[4].kind, TokenKind::Float(f) if (f - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_booleans() {
        let tokens = Tokenizer::tokenize("(foo true false)").unwrap();
        assert!(matches!(tokens[2].kind, TokenKind::Bool(true)));
        assert!(matches!(tokens[3].kind, TokenKind::Bool(false)));
    }
}
