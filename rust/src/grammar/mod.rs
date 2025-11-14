//! Grammar module for the UBO DSL system
//!
//! This module provides idiomatic Rust implementations for EBNF grammar parsing,
//! validation, and compilation with strong typing and proper error handling.

// Allow unused code - this is experimental EBNF grammar implementation
#![allow(dead_code)]
#![allow(unused_imports)]

pub(crate) mod idiomatic_ebnf;

use crate::error::{DSLError, GrammarError};
use std::collections::HashMap;

// Re-export the main types
pub(crate) use idiomatic_ebnf::{EBNFExpression, EBNFGrammar, EBNFParser};

/// Grammar engine for managing DSL grammars
#[derive(Debug, Default)]
pub struct GrammarEngine {
    /// Loaded grammars indexed by name
    grammars: HashMap<String, EBNFGrammar>,
    /// Active grammar name
    active_grammar: Option<String>,
}

impl GrammarEngine {
    /// Create a new grammar engine
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a grammar from EBNF source
    pub(crate) fn load_grammar(
        &mut self,
        name: impl Into<String>,
        source: &str,
    ) -> Result<(), DSLError> {
        let parser = EBNFParser::new();
        let grammar = parser
            .parse_grammar(source)
            .map_err(|e| DSLError::Parse(e.into()))?;

        // Validate the grammar
        grammar.validate().map_err(|errors| {
            DSLError::Grammar(GrammarError::CompilationError {
                message: format!("Grammar validation failed: {}", errors.join(", ")),
            })
        })?;

        let name = name.into();
        self.grammars.insert(name.clone(), grammar);

        // Set as active if this is the first grammar
        if self.active_grammar.is_none() {
            self.active_grammar = Some(name);
        }

        Ok(())
    }

    /// Get a grammar by name
    pub(crate) fn get_grammar(&self, name: &str) -> Option<&EBNFGrammar> {
        self.grammars.get(name)
    }

    /// Get the active grammar
    pub(crate) fn active_grammar(&self) -> Option<&EBNFGrammar> {
        self.active_grammar
            .as_ref()
            .and_then(|name| self.grammars.get(name))
    }

    /// Validate a rule exists in the active grammar
    pub fn validate_rule(&self, rule_name: &str) -> Result<(), DSLError> {
        let grammar = self.active_grammar().ok_or_else(|| {
            DSLError::Grammar(GrammarError::CompilationError {
                message: "No active grammar".to_string(),
            })
        })?;

        if grammar.rules.contains_key(rule_name) {
            Ok(())
        } else {
            Err(DSLError::Grammar(GrammarError::RuleNotFound {
                rule: rule_name.to_string(),
            }))
        }
    }

    /// Get all rule names from the active grammar
    pub(crate) fn rule_names(&self) -> Vec<String> {
        self.active_grammar()
            .map(|g| g.rules.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Check for circular dependencies in the active grammar
    pub(crate) fn check_circular_dependencies(&self) -> Result<(), DSLError> {
        let grammar = self.active_grammar().ok_or_else(|| {
            DSLError::Grammar(GrammarError::CompilationError {
                message: "No active grammar".to_string(),
            })
        })?;

        self.detect_cycles(grammar)
    }

    fn detect_cycles(&self, grammar: &EBNFGrammar) -> Result<(), DSLError> {
        use std::collections::HashSet;

        fn visit(
            rule_name: &str,
            grammar: &EBNFGrammar,
            visiting: &mut HashSet<String>,
            visited: &mut HashSet<String>,
        ) -> Result<(), DSLError> {
            if visiting.contains(rule_name) {
                return Err(DSLError::Grammar(GrammarError::CircularDependency {
                    chain: format!("Rule '{}' has circular dependency", rule_name),
                }));
            }

            if visited.contains(rule_name) {
                return Ok(());
            }

            visiting.insert(rule_name.to_string());

            if let Some(rule) = grammar.rules.get(rule_name) {
                visit_expression(&rule.expression, grammar, visiting, visited)?;
            }

            visiting.remove(rule_name);
            visited.insert(rule_name.to_string());
            Ok(())
        }

        fn visit_expression(
            expr: &EBNFExpression,
            grammar: &EBNFGrammar,
            visiting: &mut HashSet<String>,
            visited: &mut HashSet<String>,
        ) -> Result<(), DSLError> {
            match expr {
                EBNFExpression::NonTerminal(name) => {
                    visit(name, grammar, visiting, visited)?;
                }
                EBNFExpression::Sequence(exprs) | EBNFExpression::Choice(exprs) => {
                    for e in exprs {
                        visit_expression(e, grammar, visiting, visited)?;
                    }
                }
                EBNFExpression::Optional(e)
                | EBNFExpression::ZeroOrMore(e)
                | EBNFExpression::OneOrMore(e)
                | EBNFExpression::Group(e) => {
                    visit_expression(e, grammar, visiting, visited)?;
                }
                _ => {} // Terminal expressions don't have dependencies
            }
            Ok(())
        }

        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        for rule_name in grammar.rules.keys() {
            if !visited.contains(rule_name) {
                visit(rule_name, grammar, &mut visiting, &mut visited)?;
            }
        }

        Ok(())
    }
}

fn count_expression_features(
    expr: &EBNFExpression,
    terminal_count: &mut usize,
    optional_count: &mut usize,
    repetition_count: &mut usize,
) {
    match expr {
        EBNFExpression::Terminal(_)
        | EBNFExpression::CharacterClass(_)
        | EBNFExpression::CharacterRange(_, _) => {
            *terminal_count += 1;
        }
        EBNFExpression::Optional(_) => {
            *optional_count += 1;
        }
        EBNFExpression::ZeroOrMore(_) | EBNFExpression::OneOrMore(_) => {
            *repetition_count += 1;
        }
        EBNFExpression::Sequence(exprs) | EBNFExpression::Choice(exprs) => {
            for e in exprs {
                count_expression_features(e, terminal_count, optional_count, repetition_count);
            }
        }
        EBNFExpression::Group(e) => {
            count_expression_features(e, terminal_count, optional_count, repetition_count);
        }
        _ => {}
    }
}

/// Summary information about a grammar
#[derive(Debug, Clone)]
pub(crate) struct GrammarSummary {
    pub rule_count: usize,
    pub terminal_count: usize,
    pub optional_count: usize,
    pub repetition_count: usize,
    pub start_rule: Option<String>,
}

/// Load the default DSL grammar
pub(crate) fn load_default_grammar() -> Result<GrammarEngine, DSLError> {
    let mut engine = GrammarEngine::new();

    let default_grammar = r#"
        program ::= workflow* ;

        workflow ::= "(" "workflow" string_literal properties? statement* ")" ;

        statement ::= declare_entity | obtain_document | create_edge | calculate_ubo | placeholder ;

        declare_entity ::= "(" "declare-entity" string_literal string_literal properties? ")" ;

        obtain_document ::= "(" "obtain-document" string_literal string_literal properties? ")" ;

        create_edge ::= "(" "create-edge" string_literal string_literal string_literal properties? ")" ;

        calculate_ubo ::= "(" "calculate-ubo" string_literal properties? ")" ;

        placeholder ::= "(" identifier value* ")" ;

        properties ::= "(" "properties" property_pair* ")" ;

        property_pair ::= identifier value ;

        value ::= string_literal | number | boolean | list | map | null ;

        string_literal ::= '"' string_char* '"' | "'" string_char* "'" ;

        string_char ::= [^"\\] | '\\' escape_char ;

        escape_char ::= 'n' | 'r' | 't' | '\\' | '"' | "'" ;

        number ::= '-'? digit+ ('.' digit+)? ;

        boolean ::= "true" | "false" ;

        null ::= "null" ;

        list ::= "[" (value ("," value)*)? "]" ;

        map ::= "{" (string_literal ":" value ("," string_literal ":" value)*)? "}" ;

        identifier ::= (letter | '_') (letter | digit | '_' | '-' | '.')* ;

        letter ::= [a-zA-Z] ;

        digit ::= [0-9] ;
    "#;

    engine.load_grammar("dsl", default_grammar)?;
    Ok(engine)
}
