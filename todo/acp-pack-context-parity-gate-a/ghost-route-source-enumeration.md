# Ghost Route Source Enumeration

Status: audit draft for Gate A replan.

Search scope:

- Production code: `rust/src`
- Tests: `rust/tests`
- Examples and documentation samples: `rust/examples`, `rust/docs`
- CLI/debug endpoints: route inventory in `rust/src/api`, binaries from Cargo metadata
- Fixture loaders: `rust/tests/fixtures`, config loaders
- Comments: included in `rg` searches

Findings:

| Source | Examples | Decision needed |
| --- | --- | --- |
| Production route comments | `agent_routes.rs` mentions unified input, removed chat, legacy execute. | Keep only current route language after remediation. |
| Production fallback names | `try_route_through_repl`, proposal engine fallback, sentence generation fallback. | Classify fallback paths as allowed UI fallback or route contamination. |
| Production bypass vocabulary | `direct.dsl`, `bypass`, legacy raw DSL. | Retain only as refusal/regression terminology. |
| Tests | `repl_v2_phase3.rs`, `repl_v2_phase6.rs`, `p0_bypass_regression.rs`. | Refactor tests that encode desired invariant; delete tests for retired behavior. |
| Docs/examples | Fallback wording in scenario comments and governance docs. | Update only if they describe utterance routing; leave domain fallback concepts alone. |

Gate A decision:

Legacy vocabulary should not be blanket-deleted. Some instances are domain concepts or regression guards. Gate B needs file-level rip/refactor/delete decisions.
