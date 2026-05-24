//! CLI entry point: `dsl-migrate <input.bpmn> <output.dsl> [--report <report.json>]`

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dsl-migrate <input.bpmn> <output.dsl> [--report <report.json>]");
        std::process::exit(1);
    }
    let input = &args[1];
    let output = &args[2];

    let xml = std::fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", input, e);
        std::process::exit(1);
    });

    let process = dsl_migrate::parse_bpmn_xml(&xml).unwrap_or_else(|e| {
        eprintln!("Error parsing BPMN: {}", e);
        std::process::exit(1);
    });

    let result = dsl_migrate::emit(&process);

    std::fs::write(output, &result.dsl_source).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", output, e);
        std::process::exit(1);
    });

    eprintln!("{}", result.coverage.summary());

    // Optional: write coverage report as JSON
    if let Some(pos) = args.iter().position(|a| a == "--report") {
        if let Some(report_path) = args.get(pos + 1) {
            let json = serde_json::to_string_pretty(&result.coverage).unwrap_or_default();
            if let Err(e) = std::fs::write(report_path, json) {
                eprintln!("Warning: could not write report to {}: {}", report_path, e);
            }
        }
    }

    // Exit 2 if human-resolve items remain (machine-detectable incomplete migration)
    if result.coverage.human_resolve > 0 {
        eprintln!(
            "{} element(s) require human resolution",
            result.coverage.human_resolve
        );
        std::process::exit(2);
    }
}
