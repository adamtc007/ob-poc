# Routing Feature Flag Inventory

Status: audit draft for Gate A replan.

Evidence:

- Cargo metadata shows feature-gated server/CLI/database surfaces in the root crate and member crates.
- Route inventory found live server routes under `rust/src/api`.

Feature surfaces to classify before Gate B:

| Surface | Risk |
| --- | --- |
| Root crate `server` feature | May expose HTTP session routes and ACP protocol surfaces. |
| Root crate `database` feature | Enables execution paths that write persistent state. |
| Root crate `cli`/`mcp` features | May expose direct DSL or prompt flows outside HTTP route policy. |
| `ob-poc-web` crate | Depends on root `ob-poc` with `server`; likely user-facing server binary. |
| `dsl-lsp` crate | Depends on root `ob-poc` with `database`; may expose alternate coding/proposal behavior. |

Gate B requirement:

Add an explicit matrix of feature combinations that are allowed to expose utterance parsing, macro resolution, or verb dispatch. The matrix should fail closed for any new route surface added after migration.
