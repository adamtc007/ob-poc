# DSL Development

The DSL is the single source of truth for onboarding workflows. Before modifying DSL code:

## Key Architecture
- Parser (Nom) → CSG Linter → Compiler → Executor
- All verbs defined in YAML: rust/config/verbs/
- GenericCrudExecutor handles CRUD ops; custom_ops/ handles plugins

## Key Files
- rust/src/dsl_v2/parser.rs - Nom-based S-expression parser
- rust/src/dsl_v2/ast.rs - Program, Statement, VerbCall, AstNode
- rust/src/dsl_v2/generic_executor.rs - YAML-driven CRUD executor
- rust/src/dsl_v2/custom_ops/ - Plugin handlers for non-CRUD ops
- rust/config/verbs/ - Verb definitions (add verbs here, not Rust code)

## Adding a Verb
Edit rust/config/verbs/<domain>.yaml:
```yaml
verbs:
  my-verb:
    description: "What it does"
    behavior: crud
    crud:
      operation: insert
      table: my_table
      schema: ob-poc
    args:
      - name: my-arg
        type: string
        required: true
        maps_to: my_column
```

No Rust code needed for standard CRUD.

Read CLAUDE.md sections "DSL Pipeline Detail" and "Complete DSL Verb Reference".
