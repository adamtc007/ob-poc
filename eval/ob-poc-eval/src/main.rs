#![forbid(unsafe_code)]

use ob_poc_eval_schema::parse_eval_case_yaml;
use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [] => {
            print_help();
            Ok(())
        }
        [flag] if flag == "--help" || flag == "-h" => {
            print_help();
            Ok(())
        }
        [scope, command, path] if scope == "case" && command == "validate" => {
            let contents = fs::read_to_string(path)
                .map_err(|err| format!("failed to read case file '{path}': {err}"))?;
            let case = parse_eval_case_yaml(&contents)
                .map_err(|err| format!("failed to parse case file '{path}': {err}"))?;
            println!("validated eval case {}", case.case_id.0);
            Ok(())
        }
        [..] => Err("unsupported command; run `ob-poc-eval --help`".to_string()),
    }
}

fn print_help() {
    println!(
        "ob-poc-eval\n\nCommands:\n  case validate <path>    Parse and validate an EvalCase YAML file"
    );
}
