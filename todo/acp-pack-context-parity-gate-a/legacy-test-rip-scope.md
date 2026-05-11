# Legacy Test Rip Scope

Status: audit draft for Gate A replan.

Initial classification:

| Test/source family | Recommendation |
| --- | --- |
| `p0_bypass_regression` | Keep/refactor. It protects against a bypass class. |
| REPL phase tests that assert no direct DSL bypass | Keep/refactor names and expected path. |
| REPL phase tests for old fallback behavior | Refactor if fallback survives as envelope-gated behavior; delete if fallback is retired. |
| ACP DAG semantic resolver tests | Keep only if resolver becomes part of envelope route; otherwise quarantine. |
| Executor/DB integration tests that directly call verbs | Keep as lower-level execution tests, but exclude from utterance-route claims. |
| Docs/examples mentioning domain fallback rules | Do not rip unless they describe utterance routing. |

Gate B action:

Create a file-level test migration list after the route owner chooses which live paths survive. Do not mass-delete tests from word search alone.
