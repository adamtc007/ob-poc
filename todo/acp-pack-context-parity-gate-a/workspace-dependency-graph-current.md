# Workspace Dependency Graph Current

Status: audit draft for Gate A replan.

Evidence:

- `sed -n '236,272p' ../../rust/Cargo.toml`
- `cargo metadata --format-version 1 --no-deps --manifest-path ../../rust/Cargo.toml`
- `rg -n "^\\[workspace\\]|members|name =|path =|dependencies" ../../rust/Cargo.toml ../../rust/crates/*/Cargo.toml`

Workspace members:

```text
.
crates/dsl-core
crates/ob-agentic
crates/ob-templates
crates/ob-workflow
crates/dsl-lsp
crates/ob-poc-web
crates/ob-poc-types
crates/entity-gateway
crates/ob-semantic-matcher
crates/ob-poc-macros
crates/governed_query_proc
xtask
crates/playbook-core
crates/playbook-lower
crates/inspector-projection
crates/sem_os_core
crates/sem_os_postgres
crates/sem_os_server
crates/sem_os_client
crates/sem_os_obpoc_adapter
crates/sem_os_harness
crates/dsl-runtime
crates/determinism-harness
crates/round-trip-harness
```

High-risk current edges:

| Crate | Local dependencies of interest |
| --- | --- |
| root `ob-poc` | `dsl-core`, `dsl-runtime`, `entity-gateway`, `ob-agentic`, `ob-workflow`, `sem_os_core`, `sem_os_client`, `sem_os_postgres`, `sem_os_obpoc_adapter` |
| `ob-poc-web` | root `ob-poc`, `dsl-lsp`, `entity-gateway`, `ob-semantic-matcher`, `sem_os_core`, `sem_os_client`, `sem_os_postgres`, `dsl-runtime` |
| `dsl-runtime` | `dsl-core`, `entity-gateway`, `sem_os_core`, `governed_query_proc` |
| `sem_os_postgres` | `sem_os_core`, `dsl-runtime`, `ob-poc-types`, `dsl-core`, `entity-gateway` |
| `dsl-lsp` | root `ob-poc`, `playbook-core`, `playbook-lower`, `entity-gateway` |

Finding:

The dependency direction is too permissive for the target invariant. Utterance-facing crates can reach execution/database surfaces through root `ob-poc` and `ob-poc-web`.
