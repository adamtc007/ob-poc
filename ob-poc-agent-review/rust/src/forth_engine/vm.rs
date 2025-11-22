//! Stack-based Virtual Machine for the DSL Forth Engine.

use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::errors::VmError;
use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use crate::forth_engine::vocab::Vocab;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum OpCode {
    CallWord(usize),
    Halt,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Op(OpCode),
    LitInt(i64),
    LitStr(String),
    LitKeyword(String),
    AttrRef(AttributeId),
    DocRef(DocumentId),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

pub struct VM<'env> {
    pub ip: usize,
    pub program: Arc<Program>,
    pub data_stack: VecDeque<Value>,
    pub return_stack: VecDeque<usize>,
    pub env: &'env mut RuntimeEnv,
    pub vocab: Arc<Vocab>,
}

impl<'env> VM<'env> {
    pub fn new(program: Arc<Program>, vocab: Arc<Vocab>, env: &'env mut RuntimeEnv) -> Self {
        VM {
            ip: 0,
            program,
            data_stack: VecDeque::new(),
            return_stack: VecDeque::new(),
            env,
            vocab,
        }
    }

    /// Pop a string value from the stack
    pub fn pop_string(&mut self) -> Result<String, VmError> {
        let value = self.data_stack.pop_back().ok_or(VmError::StackUnderflow {
            expected: 1,
            found: 0,
        })?;
        match value {
            Value::Str(s) => Ok(s),
            _ => Err(VmError::TypeError {
                expected: "String".to_string(),
                found: format!("{:?}", value),
            }),
        }
    }

    /// Pop a keyword from the stack and verify it matches expected
    pub fn pop_keyword(&mut self, expected: &str) -> Result<(), VmError> {
        let value = self.data_stack.pop_back().ok_or(VmError::StackUnderflow {
            expected: 1,
            found: 0,
        })?;
        match value {
            Value::Keyword(k) if k == expected => Ok(()),
            Value::Keyword(k) => Err(VmError::TypeError {
                expected: format!("keyword {}", expected),
                found: format!("keyword {}", k),
            }),
            _ => Err(VmError::TypeError {
                expected: format!("keyword {}", expected),
                found: format!("{:?}", value),
            }),
        }
    }

    /// Pop a keyword-value pair from the stack
    pub fn pop_keyword_value(&mut self) -> Result<(String, Value), VmError> {
        let value = self.data_stack.pop_back().ok_or(VmError::StackUnderflow {
            expected: 2,
            found: 0,
        })?;
        let keyword = self.data_stack.pop_back().ok_or(VmError::StackUnderflow {
            expected: 2,
            found: 1,
        })?;
        match keyword {
            Value::Keyword(k) => Ok((k, value)),
            _ => Err(VmError::TypeError {
                expected: "Keyword".to_string(),
                found: format!("{:?}", keyword),
            }),
        }
    }

    /// Get current stack size
    pub fn stack_size(&self) -> usize {
        self.data_stack.len()
    }

    pub fn step_with_logging(&mut self) -> Result<Option<String>, VmError> {
        if self.ip >= self.program.instructions.len() {
            return Ok(None);
        }

        let instruction = &self.program.instructions[self.ip];
        self.ip += 1;

        let log_msg = format!(
            "[VM] IP: {}, Instr: {:?}, Stack: {:?}",
            self.ip - 1,
            instruction,
            self.data_stack
        );

        match instruction {
            Instruction::Op(op_code) => match op_code {
                OpCode::CallWord(word_idx) => {
                    let word_fn = self.vocab.specs[*word_idx].impl_fn.clone();
                    word_fn(self)?;
                }
                OpCode::Halt => {
                    // Normal program termination
                    return Ok(None);
                }
            },
            Instruction::LitInt(val) => self.data_stack.push_back(Value::Int(*val)),
            Instruction::LitStr(val) => self.data_stack.push_back(Value::Str(val.clone())),
            Instruction::LitKeyword(val) => self.data_stack.push_back(Value::Keyword(val.clone())),
            Instruction::AttrRef(id) => self.data_stack.push_back(Value::Attr(id.clone())),
            Instruction::DocRef(id) => self.data_stack.push_back(Value::Doc(id.clone())),
        }
        Ok(Some(log_msg))
    }
}
