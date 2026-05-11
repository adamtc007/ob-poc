# Crate Decomposition Recommendation

Status: audit draft for Gate A replan.

Assessment of the v0.5 five-crate sketch:

| Proposed crate | Current evidence | Recommendation |
| --- | --- | --- |
| `sem_os_registry` | Registry metadata is spread across config, `sem_os_core`, root `ob-poc`, and adapters. | Create or carve out only after Slice 1 metadata model is fixed. |
| `sem_os_execution` | Execution is split across root `ob-poc`, `dsl-runtime`, `sem_os_postgres`, BPMN integration, and handlers. | Do not migrate first. Stabilize registry contracts first. |
| `sem_os_diagnostics` | Diagnostic strings/codes are distributed. | Good early extraction candidate if kept data-only. |
| `sage_utterance` | Utterance routing lives in API, REPL, ACP protocol, and resolver code. | Create after route rip decisions are approved. |
| `acp_context_envelope` | Not implemented yet by plan constraint. | Add after Gate C metadata enrichment and determinism command. |

Recommended migration order:

1. Narrow root `ob-poc` public exports around SemOS registry data.
2. Extract/normalize diagnostics used by route refusals.
3. Create registry projection boundary.
4. Move utterance route selection behind a small `sage_utterance` API.
5. Add envelope crate after deterministic projection exists.
6. Move execution behind explicit post-route dispatch boundary.
