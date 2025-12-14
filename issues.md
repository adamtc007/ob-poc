# DSL Pipeline Issues

## Runtime crash / hard-failure risks
- Config load panics if `config/verbs.yaml` or `config/csg_rules.yaml` is missing or malformed (`panic!("Failed to load ...")`), so the process can die on bad YAML or missing files.
- Executing verb calls directly via `DslExecutor::execute_verb`/`execute_plan` with nested DSL values trips `node_to_json`’s `bail!("Nested VerbCall...")` and aborts the run; nested values must go through planning/compilation first.
- Unresolved `@symbol` at execution (e.g., missing binding/injection or skipped semantic validation) yields `anyhow!("Unresolved reference")` and aborts the current plan.
- Validator/gateway coupling: if EntityGateway is unreachable or misconfigured, validation returns an error that most routes `?` into a 500 rather than degrading gracefully.
- CRUD role link/unlink paths assume lookup config is present; misconfigured verbs (missing junction/from/to/lookup) produce runtime errors despite successful parsing.

## Semantic validation failure combos (detected early, not crashes)
- Unresolved entity refs (lookup args given as free text with no matching EntityGateway result) → `NotFound` diagnostics; execution would fail if forced without validation.
- Symbols not defined before use (e.g., `:cbu-id @fund` with no producer or injection) → `UndefinedSymbol` diagnostics.
- Unknown verbs or arguments (typos or mis-generated composite names) → `UnknownVerb`/`UnknownArg` diagnostics.
- Missing required arguments per verb YAML → `MissingRequiredArg`.
- Lookup args supplied with wrong type shape (e.g., list where scalar expected) → type mismatch diagnostics.
- Fuzzy-check configured verbs supplying near matches (e.g., new entity names similar to existing ones) emit warnings that should block auto-upserts in strict workflows.

## Notes
- DAG treats missing producers as external refs; ordering will succeed but execution can still hit DB FK errors if the “external” value truly isn’t present.
- Running execution without semantic validation or with gateway downtime converts many of the above diagnostics into runtime aborts. A guardrail step (validate → plan → execute) is required for stability.

## Error handling state (best-practice gaps)
- Mixed error styles: parser/enrichment/validator constructors return `Result<_, String>` while executor/CRUD use `anyhow`; this loses context and hampers classification. Prefer typed errors or `anyhow::Result` with `context`.
- Runtime panics/unwraps remain in operational paths (e.g., lookup config assumptions in role unlink, config load panics on bad YAML), so misconfiguration can crash the process.
- Some user mistakes map to hard errors (`bail!`) instead of structured diagnostics (nested DSL passed to direct execution, unresolved symbols at execute time), causing 500s rather than actionable feedback.
- Gateway outages bubble up as generic 500s; no graceful degradation or typed transport errors to distinguish infra vs. user faults.
- Overall: validation diagnostics are strong, but runtime reporting is inconsistent; unifying error handling and removing runtime panics would align with Rust best practices.
