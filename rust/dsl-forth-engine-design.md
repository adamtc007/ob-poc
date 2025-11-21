# `dsl-forth-engine`  
### A Forth-Style Stack Machine Runtime for KYC / Onboarding S-Expression DSL  
### Full Design & Implementation Scaffold (Drop-In for Zed + Claude)

This document defines a complete Rust crate design for running your **Clojure-style S-expression DSL** (kebab-case verbs) on top of:

- A nom-based AST parser (pluggable)
- A compiler that maps the AST to threaded Forth-like bytecode
- A runtime VM (stack machine)
- A dictionary-backed environment for Attributes & Documents
- A vocabulary registry defining legal verbs for each DSL domain

You will refactor the actual verbs (vocab) later once this framework is running end-to-end.

---

## 0. Crate Overview

Crate name: `dsl-forth-engine`

Purpose:

> Provide a deterministic, auditable, composable runtime that executes your KYC / Onboarding DSL (S-expr) using a Forth-style threaded interpreter.

Key design objectives:

- Match your existing S-expression DSL conventions  
- Provide clean interfaces for:
  - DSL sheet input
  - EBNF validation
  - AST parsing
  - Compilation
  - Execution against dictionaries
- Allow each business domain (KYC-Orch, UBO, Document, Resource) to define **its own vocabulary** without changing the engine

---

## 1. Directory Structure

Create this in the repo:

```text
dsl-forth-engine/
  Cargo.toml
  src/
    lib.rs
    ast.rs
    value.rs
    env.rs
    vocab.rs
    vm.rs
    compiler.rs
    errors.rs
    ebnf.rs
    parser_nom.rs
    kyc_vocab.rs
```

---

## 2. `Cargo.toml`

```toml
[package]
name = "dsl-forth-engine"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror = "1"
serde = { version = "1", features = ["derive"] }
nom = "7"
```

---

## 3. Public API (`src/lib.rs`)

This is the entry point the rest of your system calls.

```rust
mod ast;
mod value;
mod env;
mod vocab;
mod vm;
mod compiler;
mod errors;
pub mod ebnf;
pub mod parser_nom;
pub mod kyc_vocab;

pub use crate::ast::{Expr, DslSheet, DslParser};
pub use crate::value::{Value, AttributeId, DocumentId};
pub use crate::env::{RuntimeEnv, AttributeStore, DocumentStore, OnboardingRequestId};
pub use crate::vocab::{Vocab, WordSpec, WordId};
pub use crate::vm::{Program, VM};
pub use crate::errors::{EngineError, VmError, CompileError};

#[derive(Debug)]
pub struct ExecutionReport {
    pub success: bool,
    pub steps_executed: usize,
    pub logs: Vec<String>,
}

pub fn execute_sheet<P: DslParser>(
    sheet: &DslSheet,
    parser: &P,
    vocab: &Vocab,
    env: &mut RuntimeEnv,
) -> Result<ExecutionReport, EngineError> {
    let expr = parser.parse(sheet)?;
    let program = compiler::compile_expr(&expr, vocab)?;
    let mut vm = VM::new(program, env, vocab);

    let mut logs = Vec::new();
    let mut steps = 0;

    loop {
        match vm.step_with_logging(&mut logs) {
            Ok(()) => {
                steps += 1;
                continue;
            }
            Err(VmError::Halt) => break,
            Err(e) => return Err(EngineError::Vm(e)),
        }
    }

    Ok(ExecutionReport {
        success: true,
        steps_executed: steps,
        logs,
    })
}
```

---

## 4. DSL Sheet & AST (`src/ast.rs`)

```rust
use crate::errors::EngineError;

#[derive(Debug, Clone)]
pub struct DslSheet {
    pub id: String,
    pub domain: String,
    pub version: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum Expr {
    WordCall { name: String, args: Vec<Expr> },
    StringLiteral(String),
    IntegerLiteral(i64),
    BoolLiteral(bool),
    AttributeRef(String),
    DocumentRef(String),
}

pub trait DslParser {
    fn parse(&self, sheet: &DslSheet) -> Result<Expr, EngineError>;
}
```

---

## 5. Values & IDs (`src/value.rs`)

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    Attr(AttributeId),
    Doc(DocumentId),
}
```

---

## 6. Environment (`src/env.rs`)

```rust
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use crate::value::{AttributeId, DocumentId, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OnboardingRequestId(pub u64);

#[derive(Debug, Default)]
pub struct AttributeStore {
    values: HashMap<(OnboardingRequestId, AttributeId), Value>,
}

impl AttributeStore {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, req: OnboardingRequestId, id: AttributeId)
        -> Option<&Value>
    {
        self.values.get(&(req, id))
    }

    pub fn set(&mut self, req: OnboardingRequestId, id: AttributeId, value: Value) {
        self.values.insert((req, id), value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: DocumentId,
    pub name: String,
    pub location: String,
}

#[derive(Debug, Default)]
pub struct DocumentStore {
    docs: HashMap<(OnboardingRequestId, DocumentId), DocumentMeta>,
}

impl DocumentStore {
    pub fn get(&self, req: OnboardingRequestId, id: DocumentId)
        -> Option<&DocumentMeta>
    { self.docs.get(&(req, id)) }

    pub fn add(&mut self, req: OnboardingRequestId, meta: DocumentMeta) {
        self.docs.insert((req, meta.id), meta);
    }
}

#[derive(Debug)]
pub struct RuntimeEnv {
    pub request_id: OnboardingRequestId,
    pub attributes: AttributeStore,
    pub documents: DocumentStore,
}

impl RuntimeEnv {
    pub fn new(request_id: OnboardingRequestId) -> Self {
        Self {
            request_id,
            attributes: AttributeStore::new(),
            documents: DocumentStore::new(),
        }
    }
}
```

---

## 7. Vocabulary & Words (`src/vocab.rs`)

```rust
use std::collections::HashMap;

use crate::vm::VM;
use crate::errors::VmError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WordId(pub u16);

pub type WordImpl = fn(&mut VM) -> Result<(), VmError>;

#[derive(Debug)]
pub struct WordSpec {
    pub id: WordId,
    pub name: &'static str,
    pub domain: &'static str,
    pub stack_effect: &'static str,
    pub impl_fn: WordImpl,
}

#[derive(Debug)]
pub struct Vocab {
    by_name: HashMap<String, WordId>,
    specs: Vec<WordSpec>,
}

impl Vocab {
    pub fn new(specs: Vec<WordSpec>) -> Self {
        let mut by_name = HashMap::new();
        for spec in &specs {
            by_name.insert(spec.name.to_string(), spec.id);
        }
        Self { by_name, specs }
    }

    pub fn lookup(&self, name: &str) -> Option<WordId> {
        self.by_name.get(name).copied()
    }

    pub fn spec(&self, id: WordId) -> &WordSpec {
        &self.specs[id.0 as usize]
    }
}
```

---

## 8. VM & Threaded Program (`src/vm.rs`)

```rust
use crate::value::{Value, AttributeId, DocumentId};
use crate::vocab::{Vocab, WordId};
use crate::env::RuntimeEnv;
use crate::errors::VmError;

#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    CallWord(WordId),
    Halt,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Op(OpCode),
    LitInt(i64),
    LitStr(String),
    AttrRef(AttributeId),
    DocRef(DocumentId),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

pub struct VM<'env> {
    pub ip: usize,
    pub program: Program,
    pub data_stack: Vec<Value>,
    pub return_stack: Vec<usize>,
    pub env: &'env mut RuntimeEnv,
    pub vocab: &'env Vocab,
}

impl<'env> VM<'env> {
    pub fn new(program: Program, env: &'env mut RuntimeEnv, vocab: &'env Vocab) -> Self {
        Self {
            ip: 0,
            program,
            data_stack: Vec::new(),
            return_stack: Vec::new(),
            env,
            vocab,
        }
    }

    pub fn step_with_logging(&mut self, logs: &mut Vec<String>)
        -> Result<(), VmError>
    {
        if self.ip >= self.program.instructions.len() {
            return Err(VmError::Halt);
        }

        let instr = &self.program.instructions[self.ip];
        logs.push(format!(
            "ip={} instr={:?} stack={:?}",
            self.ip, instr, self.data_stack
        ));

        match instr {
            Instruction::Op(OpCode::CallWord(word_id)) => {
                let spec = self.vocab.spec(*word_id);
                (spec.impl_fn)(self)?;
                self.ip += 1;
            }
            Instruction::Op(OpCode::Halt) => {
                return Err(VmError::Halt);
            }
            Instruction::LitInt(i) => {
                self.data_stack.push(Value::Int(*i));
                self.ip += 1;
            }
            Instruction::LitStr(s) => {
                self.data_stack.push(Value::Str(s.clone()));
                self.ip += 1;
            }
            Instruction::AttrRef(id) => {
                self.data_stack.push(Value::Attr(*id));
                self.ip += 1;
            }
            Instruction::DocRef(id) => {
                self.data_stack.push(Value::Doc(*id));
                self.ip += 1;
            }
        }

        Ok(())
    }
}
```

---

## 9. Error Types (`src/errors.rs`)

```rust
use thiserror::Error;
use crate::value::AttributeId;

#[derive(Debug, Error)]
pub enum VmError {
    #[error("stack underflow")]
    StackUnderflow,

    #[error("type error: expected {expected}, found {found}")]
    TypeError { expected: &'static str, found: String },

    #[error("missing attribute {0:?}")]
    MissingAttribute(AttributeId),

    #[error("halt")]
    Halt,
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("unknown word `{0}`")]
    UnknownWord(String),

    #[error("unresolved attribute `{0}`")]
    UnresolvedAttribute(String),

    #[error("unresolved document `{0}`")]
    UnresolvedDocument(String),
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("compile error: {0}")]
    Compile(#[from] CompileError),

    #[error("vm error: {0}")]
    Vm(#[from] VmError),
}
```

---

## 10. Compiler (`src/compiler.rs`)

```rust
use crate::ast::Expr;
use crate::vocab::Vocab;
use crate::vm::{Program, Instruction, OpCode};
use crate::value::{AttributeId, DocumentId};
use crate::errors::CompileError;

pub fn compile_expr(expr: &Expr, vocab: &Vocab) -> Result<Program, CompileError> {
    let mut instructions = Vec::new();
    compile_expr_inner(expr, vocab, &mut instructions)?;
    instructions.push(Instruction::Op(OpCode::Halt));
    Ok(Program { instructions })
}

fn compile_expr_inner(
    expr: &Expr,
    vocab: &Vocab,
    instructions: &mut Vec<Instruction>,
) -> Result<(), CompileError> {
    match expr {
        Expr::WordCall { name, args } => {
            for arg in args {
                compile_expr_inner(arg, vocab, instructions)?;
            }
            let word_id = vocab
                .lookup(name)
                .ok_or_else(|| CompileError::UnknownWord(name.clone()))?;
            instructions.push(Instruction::Op(OpCode::CallWord(word_id)));
        }
        Expr::StringLiteral(s) => instructions.push(Instruction::LitStr(s.clone())),
        Expr::IntegerLiteral(i) => instructions.push(Instruction::LitInt(*i)),
        Expr::BoolLiteral(b) => instructions.push(Instruction::LitInt(if *b { 1 } else { 0 })),
        Expr::AttributeRef(name) => {
            let id = resolve_attribute_id(name)?;
            instructions.push(Instruction::AttrRef(id));
        }
        Expr::DocumentRef(name) => {
            let id = resolve_document_id(name)?;
            instructions.push(Instruction::DocRef(id));
        }
    }
    Ok(())
}

fn resolve_attribute_id(name: &str) -> Result<AttributeId, CompileError> {
    Err(CompileError::UnresolvedAttribute(name.to_string()))
}

fn resolve_document_id(name: &str) -> Result<DocumentId, CompileError> {
    Err(CompileError::UnresolvedDocument(name.to_string()))
}
```

---

## 11. KYC Vocabulary Example (`src/kyc_vocab.rs`)

```rust
use crate::vocab::{Vocab, WordSpec, WordId};
use crate::vm::VM;
use crate::errors::VmError;
use crate::value::Value;

fn word_require_attribute(vm: &mut VM) -> Result<(), VmError> {
    let top = vm.data_stack.pop().ok_or(VmError::StackUnderflow)?;
    let attr_id = match top {
        Value::Attr(id) => id,
        other => return Err(VmError::TypeError {
            expected: "Attr",
            found: format!("{:?}", other),
        }),
    };

    let req_id = vm.env.request_id;
    if vm.env.attributes.get(req_id, attr_id).is_none() {
        return Err(VmError::MissingAttribute(attr_id));
    }

    Ok(())
}

fn word_set_attribute(vm: &mut VM) -> Result<(), VmError> {
    let attr = vm.data_stack.pop().ok_or(VmError::StackUnderflow)?;
    let value = vm.data_stack.pop().ok_or(VmError::StackUnderflow)?;

    let attr_id = match attr {
        Value::Attr(id) => id,
        other => return Err(VmError::TypeError {
            expected: "Attr",
            found: format!("{:?}", other),
        }),
    };

    let req_id = vm.env.request_id;
    vm.env.attributes.set(req_id, attr_id, value);
    Ok(())
}

pub fn kyc_orch_vocab() -> Vocab {
    let specs = vec![
        WordSpec {
            id: WordId(0),
            name: "require-attribute",
            domain: "kyc-orch",
            stack_effect: "( attr -- )",
            impl_fn: word_require_attribute,
        },
        WordSpec {
            id: WordId(1),
            name: "set-attribute",
            domain: "kyc-orch",
            stack_effect: "( value attr -- )",
            impl_fn: word_set_attribute,
        },
    ];

    Vocab::new(specs)
}
```

---

## 12. EBNF Spec (`src/ebnf.rs`)

```rust
pub const KYC_ORCH_EBNF: &str = r#"
KycSheet      = SExpr* ;
SExpr         = "(" Word Arg* ")" ;

Word          = KycWord | CoreWord ;

KycWord       = "require-attribute"
              | "set-attribute"
              | "require-document"
              | "mark-phase"
              | "onboard-cbu"
              ;

CoreWord      = "if" | "else" | "then" ;

Arg           = StringLiteral
              | IntegerLiteral
              | BoolLiteral
              | AttributeRef
              | DocumentRef
              | SExpr
              ;

StringLiteral = '\"' { any-character-except-quote } '\"' ;
IntegerLiteral= ['-'] digit { digit } ;
BoolLiteral   = "true" | "false" ;

AttributeRef  = "KYC." identifier { "." identifier } ;
DocumentRef   = "Doc." identifier { "." identifier } ;

identifier    = letter { letter | digit | "-" | "_" } ;
"#;
```

---

## 13. Skeleton nom Parser (`src/parser_nom.rs`)

This is intentionally minimal – Claude can extend it.

```rust
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{digit1, multispace0, one_of},
    combinator::{map, map_res, opt},
    multi::many0,
    sequence::{delimited, preceded, tuple},
};

use crate::ast::{DslParser, DslSheet, Expr};
use crate::errors::EngineError;

pub struct NomKycParser;

impl NomKycParser {
    pub fn new() -> Self { Self }
}

impl DslParser for NomKycParser {
    fn parse(&self, sheet: &DslSheet) -> Result<Expr, EngineError> {
        let input = sheet.content.as_str();
        match parse_expr(input) {
            Ok((rest, expr)) => {
                if rest.trim().is_empty() {
                    Ok(expr)
                } else {
                    Err(EngineError::Parse(format!("Trailing input: `{rest}`")))
                }
            }
            Err(e) => Err(EngineError::Parse(format!("{e:?}"))),
        }
    }
}

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    preceded(multispace0, alt((parse_s_expr, parse_atom)))(input)
}

fn parse_s_expr(input: &str) -> IResult<&str, Expr> {
    delimited(
        preceded(multispace0, tag("(")),
        parse_word_call,
        preceded(multispace0, tag(")")),
    )(input)
}

fn parse_word_call(input: &str) -> IResult<&str, Expr> {
    let (input, name) = parse_symbol(input)?;
    let (input, args) = many0(parse_expr)(input)?;
    Ok((
        input,
        Expr::WordCall {
            name: name.to_string(),
            args,
        },
    ))
}

fn parse_atom(input: &str) -> IResult<&str, Expr> {
    preceded(
        multispace0,
        alt((
            parse_string_literal,
            parse_integer_literal,
            parse_bool_literal,
            parse_attribute_ref,
            parse_document_ref,
        )),
    )(input)
}

fn parse_string_literal(input: &str) -> IResult<&str, Expr> {
    let (input, _) = tag("\"")(input)?;
    let (input, content) = take_while1(|c| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;
    Ok((input, Expr::StringLiteral(content.to_string())))
}

fn parse_integer_literal(input: &str) -> IResult<&str, Expr> {
    map(
        preceded(
            multispace0,
            map_res(
                tuple((opt_sign, digit1)),
                |(sign, digits): (Option<char>, &str)| {
                    let mut s = String::new();
                    if let Some(sign) = sign {
                        s.push(sign);
                    }
                    s.push_str(digits);
                    s.parse::<i64>()
                },
            ),
        ),
        Expr::IntegerLiteral,
    )(input)
}

fn opt_sign(input: &str) -> IResult<&str, Option<char>> {
    opt(one_of("+-"))(input)
}

fn parse_bool_literal(input: &str) -> IResult<&str, Expr> {
    let (input, val) = preceded(
        multispace0,
        alt((tag("true"), tag("false"))),
    )(input)?;
    Ok((input, Expr::BoolLiteral(val == "true")))
}

fn parse_symbol(input: &str) -> IResult<&str, &str> {
    preceded(
        multispace0,
        take_while1(|c: char| {
            c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
        }),
    )(input)
}

fn parse_attribute_ref(input: &str) -> IResult<&str, Expr> {
    let (input, sym) = parse_symbol(input)?;
    if sym.starts_with("KYC.") {
        Ok((input, Expr::AttributeRef(sym.to_string())))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}

fn parse_document_ref(input: &str) -> IResult<&str, Expr> {
    let (input, sym) = parse_symbol(input)?;
    if sym.starts_with("Doc.") {
        Ok((input, Expr::DocumentRef(sym.to_string())))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))
    }
}
```

---

## 14. Example Caller Usage (from your ob-poc)

```rust
use dsl_forth_engine::{
    DslSheet,
    RuntimeEnv,
    OnboardingRequestId,
    kyc_vocab::kyc_orch_vocab,
    parser_nom::NomKycParser,
    execute_sheet,
};

fn run_example() -> anyhow::Result<()> {
    let sheet = DslSheet {
        id: "KYC-CASE-123".into(),
        domain: "kyc-orch".into(),
        version: "v0.1".into(),
        content: r#"
          (require-attribute KYC.LEI)
        "#.into(),
    };

    let vocab = kyc_orch_vocab();
    let parser = NomKycParser::new();
    let mut env = RuntimeEnv::new(OnboardingRequestId(123));

    // Optionally pre-populate env.attributes here.

    let report = execute_sheet(&sheet, &parser, &vocab, &mut env)?;
    println!("Executed {} steps", report.steps_executed);
    for log in report.logs {
        println!("{log}");
    }

    Ok(())
}
```

---
