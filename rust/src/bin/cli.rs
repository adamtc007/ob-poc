use std::env;
use std::fs;
use std::process;

use ob_poc::parser::parse_program;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <dsl-file>", args[0]);
        eprintln!("Example: {} examples/zenith_capital_ubo.dsl", args[0]);
        process::exit(1);
    }

    let filename = &args[1];

    // Read the DSL file
    let content = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading file '{}': {}", filename, err);
            process::exit(1);
        }
    };

    // Parse the DSL content
    match parse_program(&content) {
        Ok((remaining, program)) => {
            if !remaining.trim().is_empty() {
                eprintln!("Warning: Unparsed content remaining: '{}'", remaining);
            }

            println!("Successfully parsed DSL program:");
            println!("Found {} workflow(s)", program.workflows.len());

            for (i, workflow) in program.workflows.iter().enumerate() {
                println!("  Workflow {}: ID = '{}'", i + 1, workflow.id);
                println!("    Properties: {} items", workflow.properties.len());
                println!("    Statements: {} items", workflow.statements.len());
            }
        }
        Err(err) => {
            eprintln!("Parse error: {}", err);
            process::exit(1);
        }
    }
}
