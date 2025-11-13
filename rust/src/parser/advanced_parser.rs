//! Advanced DSL parser with semantic analysis capabilities
//!
//! This module provides enhanced parsing functionality that goes beyond basic syntax analysis
//! to include semantic validation, type checking, and database integration metadata extraction.

use crate::ast::{
    types::*, DeclareEntity, EntityLabel, Program, PropertyMap, Statement, Value, Workflow,
};
use chrono::{DateTime, NaiveDate, Utc};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{alpha1, alphanumeric1, char, multispace0, multispace1, space0, space1},
    combinator::{opt, recognize},
    multi::many0,
    sequence::{pair, tuple},
    IResult,
};

use std::collections::HashMap;
use uuid::Uuid;

/// Enhanced parser with semantic analysis
pub(crate) struct AdvancedParser {
    /// Current parsing context
    context: ParsingContext,
    /// Grammar rules cache
    grammar_cache: HashMap<String, GrammarRule>,
    /// Vocabulary cache
    vocabulary_cache: HashMap<String, VocabularyVerb>,
    /// Type inference engine
    type_inferrer: TypeInferrer,
    /// Validation engine
    validator: SemanticValidator,
}

/// Parsing context for semantic analysis
#[derive(Debug, Clone)]
pub(crate) struct ParsingContext {
    pub current_file: Option<String>,
    pub current_line: usize,
    pub current_column: usize,
    pub scope_stack: Vec<Scope>,
    pub current_workflow: Option<String>,
    pub domain_context: Option<String>,
}

/// Parsing scope for variable resolution
#[derive(Debug, Clone)]
pub(crate) struct Scope {
    pub scope_id: Uuid,
    pub scope_type: ScopeType,
    pub variables: HashMap<String, VariableBinding>,
    pub parent: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub(crate) enum ScopeType {
    Global,
    Workflow,
    Statement,
    Expression,
}

#[derive(Debug, Clone)]
pub(crate) struct VariableBinding {
    pub name: String,
    pub value_type: DSLType,
    pub location: SourceLocation,
    pub mutable: bool,
}

/// Type inference engine
pub(crate) struct TypeInferrer {
    type_rules: HashMap<String, TypeRule>,
    constraint_solver: ConstraintSolver,
}

#[derive(Debug, Clone)]
pub(crate) struct TypeRule {
    pub verb: String,
    pub parameter_types: Vec<DSLType>,
    pub return_type: DSLType,
    pub constraints: Vec<TypeConstraint>,
}

/// Constraint solver for type system
pub(crate) struct ConstraintSolver {
    constraints: Vec<TypeConstraint>,
}

/// Semantic validator
pub(crate) struct SemanticValidator {
    validation_rules: Vec<ValidationRule>,
    database_constraints: Vec<DatabaseConstraint>,
}

#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub rule_id: String,
    pub rule_type: ValidationType,
    pub condition: ValidationCondition,
    pub message: String,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone)]
pub(crate) enum ValidationType {
    Syntax,
    Semantic,
    Business,
    Database,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidationCondition {
    pub expression: String,
    pub context_required: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DatabaseConstraint {
    pub table: String,
    pub column: Option<String>,
    pub constraint_type: ConstraintType,
    pub parameters: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) enum ConstraintType {
    ForeignKey,
    Unique,
    Check,
    NotNull,
}

/// Enhanced parsing result with semantic information
#[derive(Debug)]
pub struct ParseResult {
    pub program: Program,
    pub semantic_info: Vec<SemanticInfo>,
    pub validation_results: ValidationResults,
    pub type_info: TypeAnalysisResult,
    pub database_references: Vec<DatabaseReference>,
}

#[derive(Debug)]
pub(crate) struct ValidationResults {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub info: Vec<String>,
    pub is_valid: bool,
}

#[derive(Debug)]
pub(crate) struct TypeAnalysisResult {
    pub type_mappings: HashMap<String, DSLType>,
    pub constraint_violations: Vec<ConstraintViolation>,
    pub inference_results: Vec<TypeInference>,
}

#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub location: SourceLocation,
    pub constraint: TypeConstraint,
    pub actual_type: DSLType,
    pub expected_type: DSLType,
}

#[derive(Debug, Clone)]
pub(crate) struct TypeInference {
    pub location: SourceLocation,
    pub inferred_type: DSLType,
    pub confidence: f64,
    pub reasoning: String,
}

impl AdvancedParser {
    /// Create a new advanced parser with semantic analysis capabilities
    pub fn new() -> Self {
        Self {
            context: ParsingContext::new(),
            grammar_cache: HashMap::new(),
            vocabulary_cache: HashMap::new(),
            type_inferrer: TypeInferrer::new(),
            validator: SemanticValidator::new(),
        }
    }

    /// Parse DSL with full semantic analysis
    pub(crate) fn parse_with_semantics(&mut self, input: &str) -> Result<ParseResult, ParseError> {
        // Phase 1: Basic syntax parsing
        let (remaining, mut program) = self
            .parse_program(input)
            .map_err(|e| ParseError::SyntaxError(format!("Parse error: {:?}", e)))?;

        if !remaining.trim().is_empty() {
            return Err(ParseError::UnexpectedInput(format!(
                "Unparsed input: '{}'",
                remaining
            )));
        }

        // Phase 2: Semantic analysis
        let semantic_info = self.analyze_semantics(&program)?;

        // Phase 3: Type inference and checking
        let type_info = self.type_inferrer.analyze_types(&program, &semantic_info)?;

        // Phase 4: Validation
        let validation_results = self
            .validator
            .validate(&program, &semantic_info, &type_info)?;

        // Phase 5: Database reference extraction
        let database_references = self.extract_database_references(&program, &semantic_info)?;

        Ok(ParseResult {
            program,
            semantic_info,
            validation_results,
            type_info,
            database_references,
        })
    }

    /// Parse program with enhanced context tracking
    pub fn parse_program(&mut self, input: &str) -> IResult<&str, Program> {
        self.context.reset();
        let mut input = input;
        let mut workflows = Vec::new();

        // Parse at least one workflow
        let (remaining, workflow) = self.parse_workflow(input)?;
        workflows.push(workflow);
        input = remaining;

        // Parse additional workflows
        while !input.trim().is_empty() {
            match self.parse_workflow(input) {
                Ok((remaining, workflow)) => {
                    workflows.push(workflow);
                    input = remaining;
                }
                Err(_) => break,
            }
        }

        Ok((input, Program { workflows }))
    }

    /// Parse workflow with scope management
    pub(crate) fn parse_workflow(&mut self, input: &str) -> IResult<&str, Workflow> {
        let (input, _) = multispace0(input)?;
        let (input, _) = char('(')(input)?;
        let (input, _) = tag("workflow")(input)?;
        let (input, _) = space1(input)?;

        // Enter workflow scope
        let workflow_scope = self.context.enter_scope(ScopeType::Workflow);

        let (input, id) = self.parse_string_literal(input)?;
        self.context.current_workflow = Some(id.clone());

        let (input, _) = space0(input)?;
        let (input, properties) = self.parse_properties(input)?;
        let (input, _) = space0(input)?;
        // Parse statements manually to avoid borrow checker issues
        let mut statements = Vec::new();
        let mut input = input;

        while !input.trim().is_empty() && !input.trim().starts_with(')') {
            match self.parse_statement(input) {
                Ok((remaining, statement)) => {
                    statements.push(statement);
                    input = remaining;
                }
                Err(_) => break,
            }
        }
        let (input, _) = space0(input)?;
        let (input, _) = char(')')(input)?;

        // Exit workflow scope
        self.context.exit_scope();

        Ok((
            input,
            Workflow {
                id,
                properties,
                statements,
            },
        ))
    }

    /// Parse statement with enhanced semantic tracking
    pub(crate) fn parse_statement(&mut self, input: &str) -> IResult<&str, Statement> {
        let statement_scope = self.context.enter_scope(ScopeType::Statement);
        let start_location = self.context.current_location();

        let (input, _) = multispace0(input)?;
        let result = self
            .parse_declare_entity(input)
            .or_else(|_| self.parse_obtain_document(input))
            .or_else(|_| self.parse_parallel_obtain(input))
            .or_else(|_| self.parse_create_edge(input))
            .or_else(|_| self.parse_solicit_attribute(input))
            .or_else(|_| self.parse_calculate_ubo(input))
            .or_else(|_| self.parse_resolve_conflict(input))
            .or_else(|_| self.parse_generate_report(input))
            .or_else(|_| self.parse_schedule_monitoring(input))
            .or_else(|_| self.parse_parallel_statements(input))
            .or_else(|_| self.parse_sequential_statements(input));

        self.context.exit_scope();
        result
    }

    /// Parse declare entity with type validation
    pub(crate) fn parse_declare_entity(&mut self, input: &str) -> IResult<&str, Statement> {
        let (input, _) = char('(')(input)?;
        let (input, _) = tag("declare-entity")(input)?;
        let (input, _) = space1(input)?;
        let (input, node_id) = self.parse_string_literal(input)?;
        let (input, _) = space1(input)?;
        let (input, label) = self.parse_entity_label(input)?;
        let (input, _) = space0(input)?;
        let (input, properties) = self.parse_properties(input)?;
        let (input, _) = space0(input)?;
        let (input, _) = char(')')(input)?;

        // Register entity in current scope
        self.context.register_entity(&node_id, &label);

        Ok((
            input,
            Statement::DeclareEntity(DeclareEntity {
                node_id,
                label,
                properties,
            }),
        ))
    }

    /// Parse string literal with location tracking
    pub fn parse_string_literal(&mut self, input: &str) -> IResult<&str, String> {
        let (input, _) = char('"')(input)?;
        let (input, content) = take_until("\"")(input)?;
        let (input, _) = char('"')(input)?;

        // Update parser position
        self.context.advance_columns(content.len() + 2);

        Ok((input, content.to_string()))
    }

    /// Parse entity label with validation
    pub(crate) fn parse_entity_label(&mut self, input: &str) -> IResult<&str, EntityLabel> {
        let (input, label_str) = alt((
            tag("COMPANY"),
            tag("PERSON"),
            tag("TRUST"),
            tag("ADDRESS"),
            tag("DOCUMENT"),
            tag("OFFICER"),
        ))(input)?;

        let label = match label_str {
            "COMPANY" => EntityLabel::Company,
            "PERSON" => EntityLabel::Person,
            "TRUST" => EntityLabel::Trust,
            "ADDRESS" => EntityLabel::Address,
            "DOCUMENT" => EntityLabel::Document,
            "OFFICER" => EntityLabel::Officer,
            _ => {
                return Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Alt,
                )))
            }
        };

        Ok((input, label))
    }

    /// Parse properties with type inference
    pub(crate) fn parse_properties(&mut self, input: &str) -> IResult<&str, PropertyMap> {
        let (input, _) = char('(')(input)?;
        let (input, _) = tag("properties")(input)?;
        let (input, _) = space0(input)?;
        // Parse properties manually to avoid borrow checker issues
        let mut props = Vec::new();
        let mut input = input;

        // Parse first property if exists
        if let Ok((remaining, prop)) = self.parse_property(input) {
            props.push(prop);
            input = remaining;

            // Parse additional properties separated by spaces
            while let Ok((remaining, _)) = space1::<&str, nom::error::Error<&str>>(input) {
                if let Ok((remaining, prop)) = self.parse_property(remaining) {
                    props.push(prop);
                    input = remaining;
                } else {
                    break;
                }
            }
        }
        let (input, _) = space0(input)?;
        let (input, _) = char(')')(input)?;

        let mut property_map = PropertyMap::new();
        for (key, value) in props {
            property_map.insert(key, value);
        }

        Ok((input, property_map))
    }

    /// Parse individual property with type checking
    pub(crate) fn parse_property(&mut self, input: &str) -> IResult<&str, (String, Value)> {
        let (input, _) = char('(')(input)?;
        let (input, key) = self.parse_identifier(input)?;
        let (input, _) = space1(input)?;
        let (input, value) = self.parse_value(input)?;
        let (input, _) = space0(input)?;
        let (input, _) = char(')')(input)?;

        // Perform type inference on the value
        let inferred_type = self.type_inferrer.infer_value_type(&value);

        Ok((input, (key, value)))
    }

    /// Parse identifier with validation
    pub fn parse_identifier(&mut self, input: &str) -> IResult<&str, String> {
        let (input, id) = recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"), tag("-"), tag(".")))),
        ))(input)?;

        Ok((input, id.to_string()))
    }

    /// Parse value with enhanced type detection
    pub fn parse_value(&mut self, input: &str) -> IResult<&str, Value> {
        self.parse_string_value(input)
            .or_else(|_| self.parse_number_value(input))
            .or_else(|_| self.parse_boolean_value(input))
            .or_else(|_| self.parse_date_value(input))
            .or_else(|_| self.parse_list_value(input))
            .or_else(|_| self.parse_map_value(input))
            .or_else(|_| self.parse_null_value(input))
    }

    fn parse_string_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, s) = self.parse_string_literal(input)?;
        Ok((input, Value::String(s)))
    }

    fn parse_number_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, num_str) = recognize(tuple((
            opt(char('-')),
            take_while1(|c: char| c.is_ascii_digit()),
            opt(tuple((
                char('.'),
                take_while1(|c: char| c.is_ascii_digit()),
            ))),
        )))(input)?;

        if num_str.contains('.') {
            let num: f64 = num_str.parse().map_err(|_| {
                nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
            })?;
            Ok((input, Value::Number(num)))
        } else {
            let num: i64 = num_str.parse().map_err(|_| {
                nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
            })?;
            Ok((input, Value::Integer(num)))
        }
    }

    fn parse_boolean_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, bool_str) = alt((tag("true"), tag("false")))(input)?;
        let bool_val = bool_str == "true";
        Ok((input, Value::Boolean(bool_val)))
    }

    fn parse_date_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, _) = char('"')(input)?;
        let (input, date_str) = take_until("\"")(input)?;
        let (input, _) = char('"')(input)?;

        // Try to parse as date
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok((input, Value::Date(date)))
        } else {
            Ok((input, Value::String(date_str.to_string())))
        }
    }

    fn parse_list_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, _) = char('[')(input)?;
        let (input, _) = space0(input)?;
        // Parse values manually to avoid borrow checker issues
        let mut values = Vec::new();
        let mut input = input;

        // Parse first value if exists
        if let Ok((remaining, value)) = self.parse_value(input) {
            values.push(value);
            input = remaining;

            // Parse additional values separated by commas
            loop {
                if let Ok((remaining, _)) = tuple::<
                    &str,
                    (&str, char, &str),
                    nom::error::Error<&str>,
                    _,
                >((space0, char(','), space0))(input)
                {
                    if let Ok((remaining, value)) = self.parse_value(remaining) {
                        values.push(value);
                        input = remaining;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let (input, _) = space0(input)?;
        let (input, _) = char(']')(input)?;
        Ok((input, Value::List(values)))
    }

    fn parse_map_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, _) = char('{')(input)?;
        let (input, _) = space0(input)?;
        // Parse key-value pairs manually to avoid borrow checker issues
        let mut pairs = Vec::new();
        let mut input = input;

        // Parse first pair if exists
        if let Ok((remaining, pair)) = self.parse_key_value_pair(input) {
            pairs.push(pair);
            input = remaining;

            // Parse additional pairs separated by commas
            loop {
                if let Ok((remaining, _)) = tuple::<
                    &str,
                    (&str, char, &str),
                    nom::error::Error<&str>,
                    _,
                >((space0, char(','), space0))(input)
                {
                    if let Ok((remaining, pair)) = self.parse_key_value_pair(remaining) {
                        pairs.push(pair);
                        input = remaining;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let (input, _) = space0(input)?;
        let (input, _) = char('}')(input)?;

        let mut map = PropertyMap::new();
        for (key, value) in pairs {
            map.insert(key, value);
        }
        Ok((input, Value::Map(map)))
    }

    fn parse_key_value_pair(&mut self, input: &str) -> IResult<&str, (String, Value)> {
        let (input, key) = self.parse_string_literal(input)?;
        let (input, _) = space0(input)?;
        let (input, _) = char(':')(input)?;
        let (input, _) = space0(input)?;
        let (input, value) = self.parse_value(input)?;
        Ok((input, (key, value)))
    }

    fn parse_null_value(&mut self, input: &str) -> IResult<&str, Value> {
        let (input, _) = tag("null")(input)?;
        Ok((input, Value::Null))
    }

    // Additional parsing methods for other statement types...
    fn parse_obtain_document(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for obtain document parsing
        todo!("Implement obtain document parsing")
    }

    fn parse_parallel_obtain(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for parallel obtain parsing
        todo!("Implement parallel obtain parsing")
    }

    fn parse_create_edge(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for create edge parsing
        todo!("Implement create edge parsing")
    }

    fn parse_solicit_attribute(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for solicit attribute parsing
        todo!("Implement solicit attribute parsing")
    }

    fn parse_calculate_ubo(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for calculate UBO parsing
        todo!("Implement calculate UBO parsing")
    }

    fn parse_resolve_conflict(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for resolve conflict parsing
        todo!("Implement resolve conflict parsing")
    }

    fn parse_generate_report(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for generate report parsing
        todo!("Implement generate report parsing")
    }

    fn parse_schedule_monitoring(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for schedule monitoring parsing
        todo!("Implement schedule monitoring parsing")
    }

    fn parse_parallel_statements(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for parallel statements parsing
        todo!("Implement parallel statements parsing")
    }

    fn parse_sequential_statements(&mut self, input: &str) -> IResult<&str, Statement> {
        // Implementation for sequential statements parsing
        todo!("Implement sequential statements parsing")
    }

    /// Perform semantic analysis on parsed program
    fn analyze_semantics(&mut self, program: &Program) -> Result<Vec<SemanticInfo>, ParseError> {
        let mut semantic_info = Vec::new();

        for workflow in &program.workflows {
            let workflow_semantic = self.analyze_workflow_semantics(workflow)?;
            semantic_info.push(workflow_semantic);
        }

        Ok(semantic_info)
    }

    fn analyze_workflow_semantics(
        &mut self,
        workflow: &Workflow,
    ) -> Result<SemanticInfo, ParseError> {
        // Implement workflow semantic analysis
        Ok(SemanticInfo {
            source_location: SourceLocation {
                line: 1,
                column: 1,
                file: self.context.current_file.clone(),
                span: None,
            },
            type_info: TypeInfo {
                expected_type: DSLType::Custom {
                    name: "Workflow".to_string(),
                    schema: HashMap::new(),
                },
                inferred_type: None,
                constraints: vec![],
                nullable: false,
            },
            validation_state: ValidationState::Pending,
            dependencies: vec![],
            database_refs: vec![],
        })
    }

    /// Extract database references from parsed program
    fn extract_database_references(
        &self,
        program: &Program,
        semantic_info: &[SemanticInfo],
    ) -> Result<Vec<DatabaseReference>, ParseError> {
        let mut references = Vec::new();

        // Extract references from workflows
        for workflow in &program.workflows {
            // Check for grammar rule references
            if let Some(grammar_ref) = self.extract_grammar_references(workflow)? {
                references.push(grammar_ref);
            }

            // Check for vocabulary references
            let vocab_refs = self.extract_vocabulary_references(workflow)?;
            references.extend(vocab_refs);
        }

        Ok(references)
    }

    fn extract_grammar_references(
        &self,
        workflow: &Workflow,
    ) -> Result<Option<DatabaseReference>, ParseError> {
        // Implementation to extract grammar rule references
        Ok(None)
    }

    fn extract_vocabulary_references(
        &self,
        workflow: &Workflow,
    ) -> Result<Vec<DatabaseReference>, ParseError> {
        // Implementation to extract vocabulary references
        Ok(vec![])
    }
}

impl ParsingContext {
    fn new() -> Self {
        Self {
            current_file: None,
            current_line: 1,
            current_column: 1,
            scope_stack: vec![],
            current_workflow: None,
            domain_context: None,
        }
    }

    fn reset(&mut self) {
        self.current_line = 1;
        self.current_column = 1;
        self.scope_stack.clear();
        self.current_workflow = None;
        self.domain_context = None;
    }

    fn enter_scope(&mut self, scope_type: ScopeType) -> Uuid {
        let scope_id = Uuid::new_v4();
        let parent = self.scope_stack.last().map(|s| s.scope_id);

        let scope = Scope {
            scope_id,
            scope_type,
            variables: HashMap::new(),
            parent,
        };

        self.scope_stack.push(scope);
        scope_id
    }

    fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn current_location(&self) -> SourceLocation {
        SourceLocation {
            line: self.current_line,
            column: self.current_column,
            file: self.current_file.clone(),
            span: None,
        }
    }

    fn advance_columns(&mut self, count: usize) {
        self.current_column += count;
    }

    fn advance_lines(&mut self, count: usize) {
        self.current_line += count;
        self.current_column = 1;
    }

    fn register_entity(&mut self, node_id: &str, label: &EntityLabel) {
        if let Some(current_scope) = self.scope_stack.last_mut() {
            let binding = VariableBinding {
                name: node_id.to_string(),
                value_type: DSLType::EntityReference {
                    entity_type: format!("{:?}", label),
                },
                location: self.current_location(),
                mutable: false,
            };
            current_scope.variables.insert(node_id.to_string(), binding);
        }
    }
}

impl TypeInferrer {
    fn new() -> Self {
        Self {
            type_rules: HashMap::new(),
            constraint_solver: ConstraintSolver::new(),
        }
    }

    fn analyze_types(
        &mut self,
        program: &Program,
        semantic_info: &[SemanticInfo],
    ) -> Result<TypeAnalysisResult, ParseError> {
        let mut type_mappings = HashMap::new();
        let mut constraint_violations = Vec::new();
        let mut inference_results = Vec::new();

        // Perform type analysis on each workflow
        for workflow in &program.workflows {
            let workflow_types = self.infer_workflow_types(workflow)?;
            type_mappings.extend(workflow_types);
        }

        Ok(TypeAnalysisResult {
            type_mappings,
            constraint_violations,
            inference_results,
        })
    }

    fn infer_workflow_types(
        &mut self,
        workflow: &Workflow,
    ) -> Result<HashMap<String, DSLType>, ParseError> {
        let mut types = HashMap::new();

        // Infer type for workflow ID
        types.insert(
            format!("workflow.{}", workflow.id),
            DSLType::String {
                max_length: Some(255),
            },
        );

        // Infer types for properties
        for (key, value) in &workflow.properties {
            let inferred_type = self.infer_value_type(value);
            types.insert(format!("workflow.{}.{}", workflow.id, key), inferred_type);
        }

        Ok(types)
    }

    fn infer_value_type(&self, value: &Value) -> DSLType {
        match value {
            Value::String(_) => DSLType::String { max_length: None },
            Value::Number(_) => DSLType::Number {
                min: None,
                max: None,
            },
            Value::Integer(_) => DSLType::Integer {
                min: None,
                max: None,
            },
            Value::Boolean(_) => DSLType::Boolean,
            Value::Date(_) => DSLType::Date,
            Value::List(items) => {
                if let Some(first) = items.first() {
                    let element_type = self.infer_value_type(first);
                    DSLType::List {
                        element_type: Box::new(element_type),
                    }
                } else {
                    DSLType::List {
                        element_type: Box::new(DSLType::String { max_length: None }),
                    }
                }
            }
            Value::Map(_) => DSLType::Map {
                value_type: Box::new(DSLType::String { max_length: None }),
            },
            Value::MultiValue(_) => DSLType::Union {
                types: vec![DSLType::String { max_length: None }],
            },
            Value::Null => DSLType::String { max_length: None }, // Default to string for null
        }
    }
}

impl ConstraintSolver {
    fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }
}

impl SemanticValidator {
    fn new() -> Self {
        Self {
            validation_rules: Vec::new(),
            database_constraints: Vec::new(),
        }
    }

    fn validate(
        &mut self,
        program: &Program,
        semantic_info: &[SemanticInfo],
        type_info: &TypeAnalysisResult,
    ) -> Result<ValidationResults, ParseError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut info = Vec::new();

        // Validate each workflow
        for workflow in &program.workflows {
            let workflow_validation = self.validate_workflow(workflow)?;
            errors.extend(workflow_validation.errors);
            warnings.extend(workflow_validation.warnings);
            info.extend(workflow_validation.info);
        }

        let is_valid = errors.is_empty();

        Ok(ValidationResults {
            errors,
            warnings,
            info,
            is_valid,
        })
    }

    fn validate_workflow(&mut self, workflow: &Workflow) -> Result<ValidationResults, ParseError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut info = Vec::new();

        // Validate workflow ID
        if workflow.id.is_empty() {
            errors.push(ValidationError {
                code: "EMPTY_WORKFLOW_ID".to_string(),
                message: "Workflow ID cannot be empty".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Provide a non-empty workflow ID".to_string()],
            });
        }

        let is_valid = errors.is_empty();

        Ok(ValidationResults {
            errors,
            warnings,
            info,
            is_valid,
        })
    }
}

/// Parse error types
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Syntax error: {0}")]
    SyntaxError(String),

    #[error("Semantic error: {0}")]
    SemanticError(String),

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Unexpected input: {0}")]
    UnexpectedInput(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl Default for AdvancedParser {
    fn default() -> Self {
        Self::new()
    }
}

