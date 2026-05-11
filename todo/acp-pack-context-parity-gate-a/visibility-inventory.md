# Visibility Inventory

Status: audit draft for Gate A replan.

Evidence command:

- `for dir in ../../rust/src ../../rust/crates/*/src; do [ -d "$dir" ] || continue; count=$(rg -n "^[[:space:]]*pub(\\(|[[:space:]])" "$dir" -g '*.rs' | wc -l | tr -d ' '); printf "%s %s\n" "$dir" "$count"; done`

Current `pub`-prefixed line counts:

| Source root | Count |
| --- | ---: |
| `rust/src` | 17521 |
| `rust/crates/determinism-harness/src` | 7 |
| `rust/crates/dsl-core/src` | 1201 |
| `rust/crates/dsl-lsp/src` | 109 |
| `rust/crates/dsl-runtime/src` | 1623 |
| `rust/crates/entity-gateway/src` | 187 |
| `rust/crates/governed_query_proc/src` | 30 |
| `rust/crates/inspector-projection/src` | 225 |
| `rust/crates/ob-agentic/src` | 504 |
| `rust/crates/ob-poc-macros/src` | 5 |
| `rust/crates/ob-poc-types/src` | 2216 |
| `rust/crates/ob-poc-web/src` | 63 |
| `rust/crates/ob-semantic-matcher/src` | 303 |
| `rust/crates/ob-templates/src` | 126 |
| `rust/crates/ob-workflow/src` | 351 |
| `rust/crates/playbook-core/src` | 34 |
| `rust/crates/playbook-lower/src` | 16 |
| `rust/crates/round-trip-harness/src` | 16 |
| `rust/crates/sem_os_client/src` | 8 |
| `rust/crates/sem_os_core/src` | 2255 |
| `rust/crates/sem_os_harness/src` | 9 |
| `rust/crates/sem_os_obpoc_adapter/src` | 119 |
| `rust/crates/sem_os_postgres/src` | 1077 |
| `rust/crates/sem_os_server/src` | 57 |

Gate A finding:

The root `ob-poc` crate is the dominant public-surface risk. The current audit count is lexical, so it is a starting inventory, not a contract classification. Gate B should classify root crate exports first, then `sem_os_core`, `ob-poc-types`, `dsl-runtime`, and `dsl-core`.
