//! FEEL expression normaliser for Camunda 8 sequence flow conditions.
//!
//! Strips Camunda-specific wrappers (Juel `${...}`, FEEL unary-test `= `) and
//! validates the expression against the supported subset. Returns the clean FEEL
//! string verbatim for use as a `:condition` value, or a diagnostic for
//! out-of-scope constructs.
//!
//! The normaliser does NOT translate to dmn-lite s-expressions — that is the
//! external dmn-lite peer's responsibility. Rust emits FEEL verbatim; the
//! condition passes as an opaque string through the bpmn-lite runtime.

#[derive(Debug, Clone, PartialEq)]
pub enum FeelNormaliseResult {
    /// Expression is within the supported subset. Contains the clean FEEL string.
    Clean(String),
    /// Expression contains constructs outside the supported subset.
    /// `stripped` holds the wrapper-stripped form; `reason` names the construct.
    NeedsReview { stripped: String, reason: String },
}

/// Normalise a raw `conditionExpression` string from a Camunda 8 BPMN file.
///
/// Steps:
/// 1. Strip Juel wrapper: `${expr}` → `expr`
/// 2. Strip FEEL unary-test prefix: `= expr` → `expr`
/// 3. Classify the result against the supported FEEL subset.
pub fn feel_normalise(raw: &str) -> FeelNormaliseResult {
    let stripped = strip_wrappers(raw.trim());

    match classify(&stripped) {
        Ok(()) => FeelNormaliseResult::Clean(stripped),
        Err(reason) => FeelNormaliseResult::NeedsReview { stripped, reason },
    }
}

// ── Wrapper stripping ────────────────────────────────────────────────────────

fn strip_wrappers(s: &str) -> String {
    // Juel: ${expr} or #{expr}
    if (s.starts_with("${") && s.ends_with('}')) || (s.starts_with("#{") && s.ends_with('}')) {
        return s[2..s.len() - 1].trim().to_owned();
    }
    // FEEL unary-test: leading `=` followed by whitespace or expression
    if let Some(rest) = s.strip_prefix('=') {
        let rest = rest.trim();
        if !rest.is_empty() {
            return rest.to_owned();
        }
    }
    s.to_owned()
}

// ── Classifier ───────────────────────────────────────────────────────────────

/// Returns Ok if the expression is within the supported subset, Err with a
/// description of the unsupported construct otherwise.
fn classify(expr: &str) -> Result<(), String> {
    let mut p = Parser::new(expr);
    p.parse_expr()?;
    if !p.is_at_end() {
        return Err(format!("unexpected trailing input: '{}'", &expr[p.pos..]));
    }
    Ok(())
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek(&self) -> &str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() && self.input.as_bytes()[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<(), String> {
        self.parse_logical()
    }

    fn parse_logical(&mut self) -> Result<(), String> {
        self.parse_comparison()?;
        loop {
            self.skip_ws();
            if self.peek().starts_with("and ") || self.peek() == "and" {
                self.pos += 3;
                self.skip_ws();
                self.parse_comparison()?;
            } else if self.peek().starts_with("or ") || self.peek() == "or" {
                self.pos += 2;
                self.skip_ws();
                self.parse_comparison()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_comparison(&mut self) -> Result<(), String> {
        self.parse_arithmetic()?;
        self.skip_ws();
        // Try to consume a comparison operator
        let ops = [">=", "<=", "!=", "=", ">", "<"];
        let mut matched = false;
        for op in &ops {
            if self.peek().starts_with(op) {
                // Make sure `=` doesn't match `==`
                if *op == "=" && self.peek().starts_with("==") {
                    continue;
                }
                self.pos += op.len();
                self.skip_ws();
                matched = true;
                break;
            }
        }
        if matched {
            // check for `in` list after operator position  — handled separately
            self.parse_arithmetic()?;
        }
        // `in [...]` membership check
        self.skip_ws();
        if self.peek().starts_with("in ") || self.peek() == "in" {
            self.pos += 2;
            self.skip_ws();
            self.parse_list()?;
        }
        Ok(())
    }

    fn parse_arithmetic(&mut self) -> Result<(), String> {
        self.parse_term()?;
        loop {
            self.skip_ws();
            if self.peek().starts_with('+') || self.peek().starts_with('-') {
                self.pos += 1;
                self.skip_ws();
                self.parse_term()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_term(&mut self) -> Result<(), String> {
        self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.peek().starts_with('*') || self.peek().starts_with('/') {
                self.pos += 1;
                self.skip_ws();
                self.parse_unary()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_unary(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.input[self.pos..].starts_with("not(") {
            self.pos += 3; // consume "not", leave '(' for parse_primary
            self.parse_primary()?;
            return Ok(());
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<(), String> {
        self.skip_ws();

        if self.pos >= self.input.len() {
            return Err("unexpected end of expression".into());
        }

        // Grouped expression
        if self.input.as_bytes()[self.pos] == b'(' {
            self.pos += 1;
            self.parse_expr()?;
            self.skip_ws();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b')' {
                self.pos += 1;
                return Ok(());
            }
            return Err("expected ')'".into());
        }

        // List literal
        if self.input.as_bytes()[self.pos] == b'[' {
            return self.parse_list();
        }

        // String literal
        if self.input.as_bytes()[self.pos] == b'"' {
            return self.parse_string();
        }

        // Number
        let b0 = self.input.as_bytes()[self.pos];
        if b0.is_ascii_digit() || b0 == b'-' {
            return self.parse_number();
        }

        // Keywords: true, false, null (check before identifier to avoid consuming as ident)
        for kw in &["true", "false", "null"] {
            if self.input[self.pos..].starts_with(kw) {
                let end = self.pos + kw.len();
                let after_ok = self
                    .input
                    .get(end..)
                    .map(|s| s.is_empty() || !s.as_bytes()[0].is_ascii_alphanumeric())
                    .unwrap_or(true);
                if after_ok {
                    self.pos += kw.len();
                    return Ok(());
                }
            }
        }

        // Unsupported quantifiers (before identifier parse)
        for kw in &["for ", "some ", "every "] {
            if self.input[self.pos..].starts_with(kw) {
                return Err(format!("quantifier not supported: {}", kw.trim()));
            }
        }

        // Identifier — could be dot-access (unsupported) or function call (unsupported)
        let before = self.pos;
        if self.try_parse_identifier() {
            self.skip_ws();
            // Dot-access: identifier.something
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'.' {
                let ident = self.input[before..self.pos].to_owned();
                let dot_start = self.pos;
                self.pos += 1;
                self.try_parse_identifier();
                let dotted = self.input[dot_start..self.pos].to_owned();
                return Err(format!("dot-access not supported: {}{}", ident, dotted));
            }
            // Function call: identifier(
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'(' {
                let fname = self.input[before..self.pos].to_owned();
                return Err(format!("function call not supported: {}", fname.trim()));
            }
            return Ok(());
        }

        let snippet = &self.input[self.pos..];
        Err(format!(
            "unrecognised token: '{}'",
            &snippet[..snippet.len().min(20)]
        ))
    }

    fn parse_list(&mut self) -> Result<(), String> {
        self.skip_ws();
        if !self.peek().starts_with('[') {
            return Err("expected '[' for list".into());
        }
        self.pos += 1;
        self.skip_ws();
        if self.peek().starts_with(']') {
            self.pos += 1;
            return Ok(());
        }
        loop {
            self.parse_primary()?;
            self.skip_ws();
            if self.peek().starts_with(']') {
                self.pos += 1;
                return Ok(());
            }
            if self.peek().starts_with(',') {
                self.pos += 1;
                self.skip_ws();
            } else {
                return Err("expected ',' or ']' in list".into());
            }
        }
    }

    fn parse_string(&mut self) -> Result<(), String> {
        self.pos += 1; // opening quote
        loop {
            if self.pos >= self.input.len() {
                return Err("unterminated string".into());
            }
            let b = self.input.as_bytes()[self.pos];
            if b == b'\\' {
                self.pos += 2; // skip escape
            } else if b == b'"' {
                self.pos += 1;
                return Ok(());
            } else {
                self.pos += 1;
            }
        }
    }

    fn parse_number(&mut self) -> Result<(), String> {
        if self.peek().starts_with('-') {
            self.pos += 1;
        }
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input.as_bytes()[self.pos].is_ascii_digit()
                || self.input.as_bytes()[self.pos] == b'.')
        {
            self.pos += 1;
        }
        if self.pos == start {
            return Err("expected number".into());
        }
        Ok(())
    }

    /// Returns true and advances pos if an identifier was consumed.
    fn try_parse_identifier(&mut self) -> bool {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if self.pos >= bytes.len() {
            return false;
        }
        let first = bytes[self.pos];
        if !first.is_ascii_alphabetic() && first != b'_' {
            return false;
        }
        self.pos += 1;
        while self.pos < bytes.len()
            && (bytes[self.pos].is_ascii_alphanumeric()
                || bytes[self.pos] == b'_'
                || bytes[self.pos] == b'-')
        {
            self.pos += 1;
        }
        self.pos > start
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(s: &str) -> String {
        match feel_normalise(s) {
            FeelNormaliseResult::Clean(v) => v,
            FeelNormaliseResult::NeedsReview { stripped, reason } => {
                panic!(
                    "expected Clean for {:?}, got NeedsReview: {} (stripped: {})",
                    s, reason, stripped
                )
            }
        }
    }

    fn needs_review(s: &str) -> (String, String) {
        match feel_normalise(s) {
            FeelNormaliseResult::Clean(v) => {
                panic!("expected NeedsReview for {:?}, got Clean: {}", s, v)
            }
            FeelNormaliseResult::NeedsReview { stripped, reason } => (stripped, reason),
        }
    }

    // ── Wrapper stripping ────────────────────────────────────────────────────

    #[test]
    fn strips_juel_wrapper() {
        assert_eq!(clean("${score >= 700}"), "score >= 700");
    }

    #[test]
    fn strips_hash_juel_wrapper() {
        assert_eq!(clean("#{amount > 0}"), "amount > 0");
    }

    #[test]
    fn strips_feel_unary_test_prefix() {
        assert_eq!(clean("= score >= 700"), "score >= 700");
    }

    #[test]
    fn plain_expression_unchanged() {
        assert_eq!(clean("score >= 700"), "score >= 700");
    }

    // ── Comparison operators ─────────────────────────────────────────────────

    #[test]
    fn comparison_gte() {
        assert_eq!(clean("score >= 700"), "score >= 700");
    }

    #[test]
    fn comparison_lte() {
        assert_eq!(clean("amount <= 1000"), "amount <= 1000");
    }

    #[test]
    fn comparison_neq() {
        assert_eq!(clean("status != \"DECLINED\""), "status != \"DECLINED\"");
    }

    #[test]
    fn comparison_eq_string() {
        assert_eq!(clean("status = \"ACTIVE\""), "status = \"ACTIVE\"");
    }

    #[test]
    fn comparison_lt() {
        assert_eq!(clean("${score < 700}"), "score < 700");
    }

    // ── Logical operators ────────────────────────────────────────────────────

    #[test]
    fn logical_and() {
        assert_eq!(
            clean("score > 500 and risk = \"LOW\""),
            "score > 500 and risk = \"LOW\""
        );
    }

    #[test]
    fn logical_or() {
        assert_eq!(
            clean("status = \"A\" or status = \"B\""),
            "status = \"A\" or status = \"B\""
        );
    }

    // ── Negation ─────────────────────────────────────────────────────────────

    #[test]
    fn negation_bool() {
        assert_eq!(clean("not(approved)"), "not(approved)");
    }

    // ── Arithmetic ───────────────────────────────────────────────────────────

    #[test]
    fn arithmetic_multiply() {
        assert_eq!(
            clean("amount * rate > threshold"),
            "amount * rate > threshold"
        );
    }

    // ── In operator ──────────────────────────────────────────────────────────

    #[test]
    fn in_list() {
        assert_eq!(
            clean("status in [\"PENDING\", \"REVIEW\"]"),
            "status in [\"PENDING\", \"REVIEW\"]"
        );
    }

    // ── Null check ───────────────────────────────────────────────────────────

    #[test]
    fn null_check() {
        assert_eq!(clean("entity != null"), "entity != null");
    }

    // ── Boolean literals ─────────────────────────────────────────────────────

    #[test]
    fn bool_literal() {
        assert_eq!(clean("approved = true"), "approved = true");
    }

    // ── Out-of-scope constructs (NeedsReview) ────────────────────────────────

    #[test]
    fn dot_access_needs_review() {
        let (_, reason) = needs_review("order.amount > 100");
        assert!(reason.contains("dot-access"), "reason: {}", reason);
    }

    #[test]
    fn date_function_needs_review() {
        let (_, reason) = needs_review("date(\"2026-01-01\") > today");
        assert!(reason.contains("function call"), "reason: {}", reason);
    }

    #[test]
    fn string_length_function_needs_review() {
        let (stripped, reason) = needs_review("string length(name) > 5");
        assert!(!stripped.is_empty());
        // "string" parses as identifier, "length(..." is trailing → unexpected trailing input
        assert!(
            reason.contains("function call")
                || reason.contains("unrecognised")
                || reason.contains("unexpected trailing"),
            "reason: {}",
            reason
        );
    }

    #[test]
    fn for_quantifier_needs_review() {
        let (_, reason) = needs_review("for i in items return i > 0");
        assert!(reason.contains("quantifier"), "reason: {}", reason);
    }

    #[test]
    fn some_quantifier_needs_review() {
        let (_, reason) = needs_review("some x in list satisfies x > 0");
        assert!(reason.contains("quantifier"), "reason: {}", reason);
    }

    // ── Juel with complex FEEL inside ────────────────────────────────────────

    #[test]
    fn juel_wrapping_and_expression() {
        assert_eq!(
            clean("${score >= 700 and risk = \"LOW\"}"),
            "score >= 700 and risk = \"LOW\""
        );
    }
}
