//! EBNF Specification for the KYC Orchestration DSL.

pub const KYC_ORCH_EBNF: &str = r#"
    sheet           ::= { expr }
    expr            ::= s_expr | atom
    s_expr          ::= "(" word_call ")"
    word_call       ::= SYMBOL { expr }

    atom            ::= STRING | INTEGER | BOOLEAN | attribute_ref | document_ref
    attribute_ref   ::= "@attr(" STRING ")"
    document_ref    ::= "@doc(" STRING ")"

    (* Lexical Tokens *)
    SYMBOL          ::= (ALPHA | "_" | "-" | ".") { ALPHA | DIGIT | "_" | "-" | "." }
    STRING          ::= '"' { UNESCAPED_CHAR | ESCAPE_SEQUENCE } '"'
    INTEGER         ::= ["-"] DIGIT { DIGIT }
    BOOLEAN         ::= "true" | "false"

    ALPHA           ::= "a"..."z" | "A"..."Z"
    DIGIT           ::= "0"..."9"
    UNESCAPED_CHAR  ::= (* any character except double quote or backslash *)
    ESCAPE_SEQUENCE ::= "\\" ("n" | "r" | "t" | "\\" | '"')
"#;
