# Scenario YAML Schema (v0.1)

## Top-level structure

```yaml
name: "Human readable suite name"
suite_id: "unique_id"              # Used for sorting/filtering
description: "Optional description"
mode_expectations:                 # PolicyGate configuration
  strict_semreg: true              # Default: true
  strict_single_pipeline: true     # Default: true
  allow_direct_dsl: false          # Default: false
  allow_raw_execute: false         # Default: false
session_seed:                      # Session initialization
  scope: "Allianz GI"             # Optional scope context
  dominant_entity: null            # Optional entity hint
  actor:
    actor_id: "test.user"          # Default: "test.user"
    roles: ["viewer"]              # Default: ["viewer"]
    clearance: null                # Optional security clearance
scenarios:
  - name: "scenario_name"
    description: "Optional"
    tags: ["tag1", "tag2"]         # Optional tags for filtering
    steps: [...]
```

## Step structure

```yaml
steps:
  - user: "utterance text"         # What the user says
    expect:                        # Partial assertions (only check specified fields)
      outcome: "Ready"             # PipelineOutcome label
      chosen_verb: "kyc.open-case" # Expected chosen verb FQN
      forced_verb: "kyc.open-case" # Expected forced verb FQN
      semreg_mode: "strict"        # Expected SemReg mode
      selection_source: "discovery" # Expected selection source
      selection_source_in: ["discovery", "semreg"]  # One-of set
      run_sheet_delta: 1           # Expected entry count change
      runnable_count: 2            # Expected runnable entries
      sem_reg_denied_all: false    # SemReg denied all flag
      semreg_unavailable: false    # SemReg unavailable flag
      bypass_used: "direct_dsl"    # Bypass mechanism used
      dsl_non_empty: true          # Whether DSL was generated
      trace:                       # Trace sub-field assertions
        macro_semreg_checked: true
        macro_denied_verbs_non_empty: false
    on_outcome:                    # Handlers for interactive outcomes
      ClarifyVerb:
        choose_index: 1            # Select option by index
      ClarifyArgs:
        reply: "passport for John" # Provide missing args
      ScopeClarify:
        choose_index: 1            # Select scope option
```

## Outcome labels

| Label | PipelineOutcome |
|-------|----------------|
| `Ready` | DSL ready for execution |
| `NeedsUserInput` | Missing required arguments |
| `NeedsClarification` | Ambiguous verb |
| `NoMatch` | No verb found |
| `ScopeResolved` | Scope phrase consumed |
| `ScopeCandidates` | Multiple scope options |
| `DirectDslNotAllowed` | dsl: prefix denied |
| `NoAllowedVerbs` | SemReg denied all |
| `MacroExpanded` | Macro expanded to DSL |

## Global invariants (always checked)

1. `NoAllowedVerbs` → DSL must be empty
2. Only specified fields are asserted (partial matching)
3. Stub mode uses `HybridVerbSearcher::minimal()` (returns NoMatch by default)

## Running

```bash
cargo x harness list                    # Show all suites
cargo x harness run --all               # Run everything
cargo x harness run --suite <path>      # Run one suite
cargo x harness run --scenario <name>   # Run one scenario
cargo x harness dump --scenario <name>  # Dump artifacts
```
