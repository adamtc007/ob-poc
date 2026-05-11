# Pub Lint CI Enforcement Spec

Status: Gate E initial enforcement implemented for the root crate ACP boundary.

Implemented enforcement:

1. Added `cargo run -p xtask -- pub-lint`.
2. Added `cargo run -p xtask -- pub-lint --bless` for intentional reviewed boundary changes.
3. Added `tools/public-api-allowlist.txt` as the reviewed allowlist.
4. Wired `pub_lint::run(false)` into `xtask check`, `xtask ci`, and `xtask pre-commit`.
5. Initial scope is the root crate ACP boundary:
   - `src/lib.rs`
   - `src/acp_dag_semantic.rs`
   - `src/acp_pack_context_envelope_v2.rs`
   - `src/acp_registry_projection.rs`
   - `src/acp_static_context_acceptance.rs`
6. CI/pre-commit now reports:
   - new unrestricted `pub`,
   - removed allowlist entries,
   - exact file/public-item drift.

Boundary rule:

- Any new public item in the scanned ACP boundary fails until reviewed and blessed.
- `pub(crate)`, `pub(super)`, and `pub(in path)` remain preferred for internal cross-module use.
- The allowlist is a short-term root-crate discipline gate, not a final crate decomposition.

Accepted residual scope:

- Historical `pub` proliferation outside the scanned ACP boundary remains outside this first pass.
- The checker is line-based and intentionally conservative. It should move to rustdoc JSON or a parser-backed inventory after crate boundaries stabilize.
- Workspace crates with existing explicit public module policies should get their own allowlists only when they enter the ACP context parity slice.
