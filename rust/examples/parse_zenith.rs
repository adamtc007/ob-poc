//! Example: Parse the Zenith Capital DSL file

use ob_poc::parser::parse_program;
use std::fs;

fn main() {
    println!("🔍 Parsing Zenith Capital UBO DSL Example\n");

    let dsl_content =
        fs::read_to_string("examples/zenith_capital_ubo.dsl").expect("Failed to read DSL file");

    match parse_program(&dsl_content) {
        Ok((remaining, program)) => {
            println!("✅ Parse successful!");
            println!("   Remaining input: {} bytes", remaining.len());
            println!("   Workflows parsed: {}", program.workflows.len());

            if let Some(workflow) = program.workflows.first() {
                println!("\n📋 Workflow: {}", workflow.id);
                println!("   Statements: {}", workflow.statements.len());

                // Count statement types
                let mut entities = 0;
                let mut edges = 0;
                let mut calculations = 0;

                for stmt in &workflow.statements {
                    match stmt {
                        ob_poc::ast::Statement::DeclareEntity(_) => entities += 1,
                        ob_poc::ast::Statement::CreateEdge(_) => edges += 1,
                        ob_poc::ast::Statement::CalculateUbo(_) => calculations += 1,
                        _ => {}
                    }
                }

                println!("\n📊 Statement breakdown:");
                println!("   Entities declared: {}", entities);
                println!("   Edges created: {}", edges);
                println!("   UBO calculations: {}", calculations);
            }

            println!("\n✨ Full AST:\n{:#?}", program);
        }
        Err(e) => {
            eprintln!("❌ Parse failed: {:?}", e);
            std::process::exit(1);
        }
    }
}
