# AGENTS.md - Project Context for GPT-5.3 Codex

## Project Personality
- **Language:** Rust (2024 Edition).
- **Style:** Prioritize idiomatic, safe Rust. Avoid `unsafe` unless explicitly required.
- **Error Handling:** Use the `anyhow` crate for application logic and `thiserror` for library components.

## Development Commands
- **Check:** `cargo check` (Run this after every file edit).
- **Format:** `cargo fmt`.
- **Lint:** `cargo clippy -- -D warnings`.
- **Test:** `cargo test`.

## Implementation Rules
- **Match Statements:** Must be exhaustive; avoid wildcard `_` where possible.
- **Cloning:** Prefer references over `.clone()` unless ownership is strictly required.
- **Async:** We use `tokio`. Ensure all I/O is non-blocking.
- **Documentation:** Every public function MUST have a `///` doc comment with an `Examples` section.

## Verification Gate
Before marking a TODO item as complete, you MUST verify that `cargo check` passes. If it fails, you are authorized to self-correct up to 3 times before asking the user for help.