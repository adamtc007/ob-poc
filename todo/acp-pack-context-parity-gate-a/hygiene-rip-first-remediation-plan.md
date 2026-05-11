# Hygiene Rip-First Remediation Plan

Status: Gate A recommendation for Gate B approval.

Proposed order:

1. Declare `POST /api/session/:id/input` the only production HTTP utterance ingress for Slice 1.
2. Classify ACP protocol prompt handling as either same-path ingress or out-of-scope protocol surface.
3. Disable or quarantine `/api/session/:id/execute` from normal utterance tests and UI flows.
4. Replace `try_route_through_repl` with an envelope-gated equivalent, or quarantine it until Gate E.
5. Rename/refactor tests that still imply direct DSL bypass is expected behavior.
6. Delete or rewrite comments/examples that preserve obsolete utterance-route vocabulary.
7. Add route invariant tests: raw DSL bait, old chat route bait, direct.dsl bait, and fallback bait must refuse.

Stop condition:

No production utterance path may generate a REPL draft or dispatch a verb unless it passes through the approved envelope verification path.
