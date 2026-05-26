//! `dsl-migrate-verify <input.bpmn>`
//!
//! Migrates a Camunda 8 BPMN file and runs the full round-trip verifier.
//!
//! Exit codes:
//!   0 — clean migration and round-trip succeeded
//!   2 — HUMAN-RESOLVE items remain (incomplete migration)
//!   3 — DSL emitted but round-trip failed (structural issue)
//!   1 — fatal error (file not found, parse failure, etc.)

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: dsl-migrate-verify <input.bpmn> [process-name]");
        std::process::exit(1);
    }
    let input = &args[1];
    let process_name = args.get(2).map(String::as_str).unwrap_or("migrated-process");

    let xml = std::fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", input, e);
        std::process::exit(1);
    });

    let process = dsl_migrate::parse_bpmn_xml(&xml).unwrap_or_else(|e| {
        eprintln!("Error parsing BPMN: {}", e);
        std::process::exit(1);
    });

    let result = dsl_migrate::emit(&process);
    eprintln!("Migration: {}", result.coverage.summary());

    let verify = dsl_migrate_verify::verify_dsl_source_sync(&result.dsl_source, process_name);

    if !verify.is_ok() {
        for d in &verify.diagnostics {
            eprintln!("  verify: {}", d);
        }
        std::process::exit(3);
    }

    eprintln!("Round-trip: OK (parsed={} validated={} lowered={} started={})",
        verify.parsed, verify.validated, verify.lowered, verify.started);

    if result.coverage.human_resolve > 0 {
        eprintln!("{} element(s) require human resolution", result.coverage.human_resolve);
        std::process::exit(2);
    }
}
