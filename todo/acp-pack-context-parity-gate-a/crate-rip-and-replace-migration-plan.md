# Crate Rip and Replace Migration Plan

Status: Gate A recommendation for Gate B approval.

Migration sequence:

1. Freeze current public symbol counts from `visibility-inventory.md`.
2. Generate a use-site inventory for root `ob-poc` public items consumed by workspace crates.
3. Convert unused or internal root exports to `pub(crate)` in small batches.
4. Move diagnostic enums/codes into a narrow diagnostics module or crate.
5. Create a registry projection API that is read-only and independent of execution.
6. Route `sage_utterance` through the registry projection API only.
7. Move execution dispatch behind an explicit post-resolution boundary.
8. Add CI lint for new unrestricted `pub` after each migrated module.

Guardrails:

- One crate/module migration at a time.
- No envelope runtime wiring until route and registry boundaries are approved.
- Run `cargo check` after each visibility batch.
