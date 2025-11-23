//! CBU Model DSL Parser using Forth Engine

use crate::cbu_model_dsl::ast::{
    CbuAttributeGroup, CbuAttributesSpec, CbuModel, CbuModelError, CbuRoleSpec, CbuState,
    CbuStateMachine, CbuTransition,
};
use crate::forth_engine::ast::{DslParser, Expr};
use crate::forth_engine::parser_nom::NomDslParser;

pub struct CbuModelParser;

impl CbuModelParser {
    pub fn parse_str(input: &str) -> Result<CbuModel, CbuModelError> {
        let parser = NomDslParser::new();
        let exprs = parser.parse(input).map_err(|e| CbuModelError::ParseError(e.to_string()))?;
        if exprs.is_empty() {
            return Err(CbuModelError::ParseError("Empty DSL input".to_string()));
        }
        let model_expr = exprs.iter()
            .find(|e| matches!(e, Expr::WordCall { name, .. } if name == "cbu-model"))
            .ok_or_else(|| CbuModelError::ParseError("No cbu-model expression found".to_string()))?;
        Self::expr_to_cbu_model(model_expr)
    }

    fn expr_to_cbu_model(expr: &Expr) -> Result<CbuModel, CbuModelError> {
        match expr {
            Expr::WordCall { name, args } if name == "cbu-model" => {
                let mut id = String::new();
                let mut version = String::new();
                let mut description = None;
                let mut applies_to = Vec::new();
                let mut attributes = CbuAttributesSpec { groups: vec![] };
                let mut states = CbuStateMachine { initial: String::new(), finals: vec![], states: vec![], transitions: vec![] };
                let mut roles = Vec::new();

                let mut i = 0;
                while i < args.len() {
                    match &args[i] {
                        Expr::Keyword(k) if k == ":id" => {
                            if i + 1 < args.len() { id = Self::extract_string(&args[i + 1])?; i += 2; } else { i += 1; }
                        }
                        Expr::Keyword(k) if k == ":version" => {
                            if i + 1 < args.len() { version = Self::extract_string(&args[i + 1])?; i += 2; } else { i += 1; }
                        }
                        Expr::Keyword(k) if k == ":description" => {
                            if i + 1 < args.len() { description = Some(Self::extract_string(&args[i + 1])?); i += 2; } else { i += 1; }
                        }
                        Expr::Keyword(k) if k == ":applies-to" => {
                            if i + 1 < args.len() { applies_to = Self::extract_string_list(&args[i + 1])?; i += 2; } else { i += 1; }
                        }
                        Expr::WordCall { name: section_name, args: section_args } => {
                            match section_name.as_str() {
                                "attributes" => attributes = Self::parse_attributes(section_args)?,
                                "states" => states = Self::parse_states(section_args)?,
                                "transitions" => states.transitions = Self::parse_transitions(section_args)?,
                                "roles" => roles = Self::parse_roles(section_args)?,
                                _ => {}
                            }
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }
                if id.is_empty() { return Err(CbuModelError::MissingField("id".to_string())); }
                if version.is_empty() { return Err(CbuModelError::MissingField("version".to_string())); }
                Ok(CbuModel { id, version, description, applies_to, attributes, states, roles })
            }
            _ => Err(CbuModelError::ParseError("Expected cbu-model expression".to_string())),
        }
    }

    fn parse_attributes(args: &[Expr]) -> Result<CbuAttributesSpec, CbuModelError> {
        let mut groups = Vec::new();
        for arg in args {
            if let Expr::WordCall { name, args: group_args } = arg {
                if name == "group" { groups.push(Self::parse_attribute_group(group_args)?); }
            }
        }
        Ok(CbuAttributesSpec { groups })
    }

    fn parse_attribute_group(args: &[Expr]) -> Result<CbuAttributeGroup, CbuModelError> {
        let mut name = String::new();
        let mut required = Vec::new();
        let mut optional = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match &args[i] {
                Expr::Keyword(k) if k == ":name" => { if i + 1 < args.len() { name = Self::extract_string(&args[i + 1])?; i += 2; } else { i += 1; } }
                Expr::Keyword(k) if k == ":required" => { if i + 1 < args.len() { required = Self::extract_attr_refs(&args[i + 1])?; i += 2; } else { i += 1; } }
                Expr::Keyword(k) if k == ":optional" => { if i + 1 < args.len() { optional = Self::extract_attr_refs(&args[i + 1])?; i += 2; } else { i += 1; } }
                _ => i += 1,
            }
        }
        Ok(CbuAttributeGroup { name, required, optional })
    }

    fn parse_states(args: &[Expr]) -> Result<CbuStateMachine, CbuModelError> {
        let mut initial = String::new();
        let mut finals = Vec::new();
        let mut states = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match &args[i] {
                Expr::Keyword(k) if k == ":initial" => { if i + 1 < args.len() { initial = Self::extract_string(&args[i + 1])?; i += 2; } else { i += 1; } }
                Expr::Keyword(k) if k == ":final" => { if i + 1 < args.len() { finals = Self::extract_string_list(&args[i + 1])?; i += 2; } else { i += 1; } }
                Expr::WordCall { name, args: state_args } if name == "state" => { states.push(Self::parse_state_def(state_args)?); i += 1; }
                _ => i += 1,
            }
        }
        Ok(CbuStateMachine { initial, finals, states, transitions: vec![] })
    }

    fn parse_state_def(args: &[Expr]) -> Result<CbuState, CbuModelError> {
        let mut name = String::new();
        let mut description = None;
        let mut i = 0;
        while i < args.len() {
            match &args[i] {
                Expr::StringLiteral(s) if name.is_empty() => { name = s.clone(); i += 1; }
                Expr::Keyword(k) if k == ":description" => { if i + 1 < args.len() { description = Some(Self::extract_string(&args[i + 1])?); i += 2; } else { i += 1; } }
                _ => i += 1,
            }
        }
        Ok(CbuState { name, description })
    }

    fn parse_transitions(args: &[Expr]) -> Result<Vec<CbuTransition>, CbuModelError> {
        let mut transitions = Vec::new();
        for arg in args {
            if let Expr::WordCall { name, args: trans_args } = arg {
                if name == "->" { transitions.push(Self::parse_transition_def(trans_args)?); }
            }
        }
        Ok(transitions)
    }

    fn parse_transition_def(args: &[Expr]) -> Result<CbuTransition, CbuModelError> {
        let mut from = String::new();
        let mut to = String::new();
        let mut verb = String::new();
        let mut chunks = Vec::new();
        let mut preconditions = Vec::new();
        let mut i = 0;
        if args.len() >= 2 { from = Self::extract_string(&args[0])?; to = Self::extract_string(&args[1])?; i = 2; }
        while i < args.len() {
            match &args[i] {
                Expr::Keyword(k) if k == ":verb" => { if i + 1 < args.len() { verb = Self::extract_string(&args[i + 1])?; i += 2; } else { i += 1; } }
                Expr::Keyword(k) if k == ":chunks" => { if i + 1 < args.len() { chunks = Self::extract_string_list(&args[i + 1])?; i += 2; } else { i += 1; } }
                Expr::Keyword(k) if k == ":preconditions" => { if i + 1 < args.len() { preconditions = Self::extract_attr_refs(&args[i + 1])?; i += 2; } else { i += 1; } }
                _ => i += 1,
            }
        }
        Ok(CbuTransition { from, to, verb, chunks, preconditions })
    }

    fn parse_roles(args: &[Expr]) -> Result<Vec<CbuRoleSpec>, CbuModelError> {
        let mut roles = Vec::new();
        for arg in args {
            if let Expr::WordCall { name, args: role_args } = arg {
                if name == "role" { roles.push(Self::parse_role_def(role_args)?); }
            }
        }
        Ok(roles)
    }

    fn parse_role_def(args: &[Expr]) -> Result<CbuRoleSpec, CbuModelError> {
        let mut name = String::new();
        let mut min = 0;
        let mut max = None;
        let mut i = 0;
        while i < args.len() {
            match &args[i] {
                Expr::StringLiteral(s) if name.is_empty() => { name = s.clone(); i += 1; }
                Expr::Keyword(k) if k == ":min" => { if i + 1 < args.len() { min = Self::extract_int(&args[i + 1])? as u32; i += 2; } else { i += 1; } }
                Expr::Keyword(k) if k == ":max" => { if i + 1 < args.len() { max = Some(Self::extract_int(&args[i + 1])? as u32); i += 2; } else { i += 1; } }
                _ => i += 1,
            }
        }
        Ok(CbuRoleSpec { name, min, max })
    }

    fn extract_string(expr: &Expr) -> Result<String, CbuModelError> {
        match expr {
            Expr::StringLiteral(s) => Ok(s.clone()),
            _ => Err(CbuModelError::InvalidValue { field: "string".to_string(), message: format!("Expected string, got {:?}", expr) }),
        }
    }

    fn extract_int(expr: &Expr) -> Result<i64, CbuModelError> {
        match expr {
            Expr::IntegerLiteral(n) => Ok(*n),
            _ => Err(CbuModelError::InvalidValue { field: "integer".to_string(), message: format!("Expected integer, got {:?}", expr) }),
        }
    }

    fn extract_string_list(expr: &Expr) -> Result<Vec<String>, CbuModelError> {
        match expr {
            Expr::ListLiteral(items) => items.iter().map(Self::extract_string).collect(),
            _ => Err(CbuModelError::InvalidValue { field: "list".to_string(), message: format!("Expected list, got {:?}", expr) }),
        }
    }

    fn extract_attr_refs(expr: &Expr) -> Result<Vec<String>, CbuModelError> {
        match expr {
            Expr::ListLiteral(items) => {
                items.iter().map(|item| match item {
                    Expr::AttributeRef(s) => Ok(s.clone()),
                    _ => Err(CbuModelError::InvalidValue { field: "attr ref".to_string(), message: format!("Expected @attr(...), got {:?}", item) }),
                }).collect()
            }
            _ => Err(CbuModelError::InvalidValue { field: "attr ref list".to_string(), message: format!("Expected list, got {:?}", expr) }),
        }
    }
}
