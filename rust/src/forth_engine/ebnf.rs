//! EBNF Specification for the KYC Orchestration DSL.
//!
//! This grammar describes the s-expression based DSL used throughout the ob-poc system.
//! The DSL follows Clojure-style conventions with kebab-case identifiers.

pub const KYC_ORCH_EBNF: &str = r#"
    (* OB-POC DSL Grammar - S-Expression Style with Kebab-Case *)
    
    sheet           ::= { expr }
    expr            ::= s_expr | atom
    s_expr          ::= "(" word_call ")"
    word_call       ::= SYMBOL { expr }
    
    atom            ::= KEYWORD | STRING | INTEGER | BOOLEAN | attribute_ref | document_ref
    KEYWORD         ::= ":" SYMBOL
    attribute_ref   ::= "@attr(" STRING ")"
    document_ref    ::= "@doc(" STRING ")"
    
    SYMBOL          ::= (ALPHA | "_" | "-" | ".") { ALPHA | DIGIT | "_" | "-" | "." }
    STRING          ::= '"' { UNESCAPED_CHAR | ESCAPE_SEQUENCE } '"'
    INTEGER         ::= ["-"] DIGIT { DIGIT }
    BOOLEAN         ::= "true" | "false"
    
    ALPHA           ::= "a"..."z" | "A"..."Z"
    DIGIT           ::= "0"..."9"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebnf_is_valid_string() {
        assert!(KYC_ORCH_EBNF.contains("KEYWORD"));
        assert!(KYC_ORCH_EBNF.len() > 100);
    }
}
