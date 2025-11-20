//! Compiler for the DSL Forth Engine (AST to Bytecode).

use crate::forth_engine::ast::Expr;
use crate::forth_engine::errors::CompileError;
use crate::forth_engine::value::{AttributeId, DocumentId};
use crate::forth_engine::vm::{Instruction, OpCode, Program};
use crate::forth_engine::vocab::Vocab;

pub fn compile_sheet(exprs: &[Expr], vocab: &Vocab) -> Result<Program, CompileError> {
    let mut instructions = Vec::new();
    for expr in exprs {
        compile_expr_inner(expr, vocab, &mut instructions)?;
    }
    instructions.push(Instruction::Op(OpCode::Halt));

    let program = Program { instructions };

    // Validate stack effects at compile time
    validate_stack_effects(&program, vocab)?;

    Ok(program)
}

/// Validate stack effects at compile time to catch stack underflow errors early
fn validate_stack_effects(program: &Program, vocab: &Vocab) -> Result<(), CompileError> {
    let mut stack_depth: isize = 0;

    for instr in &program.instructions {
        match instr {
            Instruction::Op(OpCode::CallWord(word_idx)) => {
                let word = &vocab.specs[*word_idx];
                let (inputs, outputs) = word.stack_effect;

                // Check if we have enough items on the stack
                if (stack_depth as usize) < inputs {
                    return Err(CompileError::StackUnderflow {
                        word: word.name.clone(),
                        required: inputs,
                        available: stack_depth as usize,
                    });
                }

                // Update stack depth: consume inputs, produce outputs
                stack_depth -= inputs as isize;
                stack_depth += outputs as isize;
            }
            Instruction::Op(OpCode::Halt) => {
                // Program termination - stack should be empty or have expected results
                // For now, we allow any final stack state
                break;
            }
            // All literal instructions push one value onto the stack
            Instruction::LitInt(_)
            | Instruction::LitStr(_)
            | Instruction::LitKeyword(_)
            | Instruction::AttrRef(_)
            | Instruction::DocRef(_) => {
                stack_depth += 1;
            }
        }
    }

    Ok(())
}

fn compile_expr_inner(
    expr: &Expr,
    vocab: &Vocab,
    instructions: &mut Vec<Instruction>,
) -> Result<(), CompileError> {
    match expr {
        Expr::WordCall { name, args } => {
            // Compile arguments in order - they get pushed onto stack left-to-right
            // so that when popped, keyword-value pairs come off correctly
            for arg in args.iter() {
                compile_expr_inner(arg, vocab, instructions)?;
            }
            // Then, compile the word call
            let word_id = vocab
                .lookup(name)
                .ok_or_else(|| CompileError::UnknownWord(name.clone()))?;
            instructions.push(Instruction::Op(OpCode::CallWord(word_id.0)));
        }
        Expr::StringLiteral(s) => instructions.push(Instruction::LitStr(s.clone())),
        Expr::IntegerLiteral(i) => instructions.push(Instruction::LitInt(*i)),
        Expr::BoolLiteral(b) => instructions.push(Instruction::LitInt(if *b { 1 } else { 0 })),
        Expr::Keyword(k) => instructions.push(Instruction::LitKeyword(k.clone())),
        Expr::AttributeRef(name) => {
            let attr_id = resolve_attribute_id(name)?;
            instructions.push(Instruction::AttrRef(attr_id));
        }
        Expr::DocumentRef(name) => {
            let doc_id = resolve_document_id(name)?;
            instructions.push(Instruction::DocRef(doc_id));
        }
    }
    Ok(())
}

// Placeholder for future compile-time resolution logic
fn resolve_attribute_id(name: &str) -> Result<AttributeId, CompileError> {
    Ok(AttributeId(name.to_string()))
}

// Placeholder for future compile-time resolution logic
fn resolve_document_id(name: &str) -> Result<DocumentId, CompileError> {
    Ok(DocumentId(name.to_string()))
}
