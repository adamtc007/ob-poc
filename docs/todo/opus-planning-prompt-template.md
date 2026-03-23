# Opus Planning Prompt Template for Rust Codebases
## (Generates implementation plans optimized for Sonnet execution in Claude Code / Zed)

---

## How to use

1. Copy the prompt below into a Claude Opus conversation (or API call with opus model)
2. Replace the `[BRACKETED]` placeholders with your specifics
3. Paste relevant context: `cargo tree` output, key type definitions, `tree` output of affected modules
4. Opus generates a `todo.md` — feed that into your Claude Code / Zed session

---

## The Prompt

```
You are a senior Rust architect producing an implementation plan that will be executed
by an AI coding agent (Claude Sonnet running in Claude Code inside a Zed editor session).

The agent works iteratively: it edits files, runs `cargo check` / `cargo clippy` /
`cargo test`, reads compiler errors, and fixes them in a loop. It is strong at:
- Filling in function bodies when type signatures are pinned down
- Iterating against compiler output (borrow checker, lifetime, trait bound errors)
- Following explicit, ordered steps with clear file paths
- Mechanical refactors with well-defined scope

It is weaker at:
- Inferring the right type architecture from vague prose descriptions
- Knowing when to stop refactoring adjacent code
- Recovering from early wrong-direction architectural choices
- Navigating a large codebase without explicit file paths

## Task

[DESCRIBE THE FEATURE / REFACTOR / BUG FIX HERE]

## Codebase Context

[PASTE ONE OR MORE OF THE FOLLOWING:]
- `tree -I target src/` output (or relevant subtree)
- Key type definitions, trait bounds, struct layouts the work touches
- `cargo tree` dependency info if relevant
- Any failing test output or error messages motivating this work

## Output Format

Generate a file called `todo.md` with the following structure:

### 1. Overview (3-5 sentences max)
What we're doing and why. Name the key types, traits, and modules involved.

### 2. Files Touched
An explicit list of every file that will be created or modified, with full paths
relative to the crate root. Example:
- `src/domain/model/thing.rs` — new file, defines `Thing` and `ThingBuilder`
- `src/service/handler.rs` — modify `handle_request` to accept `Thing`

### 3. Type Scaffolding (CRITICAL)
Provide actual compilable Rust code blocks for:
- New struct/enum definitions with all fields and derives
- New trait definitions with full method signatures
- Key function signatures (pub fn name, generics, where clauses, return types)
- Important type aliases

Do NOT describe types in English. Write the code. Example:

```rust
// src/domain/model/thing.rs

use crate::common::Id;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thing {
    pub id: Id,
    pub name: String,
    pub state: ThingState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThingState {
    Draft,
    Active,
    Archived,
}

pub trait ThingRepository: Send + Sync + 'static {
    async fn get(&self, id: &Id) -> Result<Option<Thing>, RepoError>;
    async fn save(&self, thing: &Thing) -> Result<(), RepoError>;
}
```

### 4. Implementation Steps (ordered, with dependencies)
Number each step. For each step provide:
- **What**: one-line description
- **File(s)**: exact paths
- **Details**: what to implement (reference the type scaffolding above)
- **Depends on**: step numbers that must compile first
- **Checkpoint**: command to run after this step

Format:

#### Step 1: Create Thing model
- **Files**: `src/domain/model/thing.rs`, `src/domain/model/mod.rs`
- **Details**: Add the `Thing`, `ThingState` structs and `ThingRepository` trait
  from the scaffolding above. Add `pub mod thing;` to `mod.rs`.
- **Depends on**: none
- **Checkpoint**: `cargo check`

#### Step 2: Implement ThingRepository for PostgresRepo
- **Files**: `src/infra/postgres/thing_repo.rs`, `src/infra/postgres/mod.rs`
- **Details**: Implement `ThingRepository` for `PostgresRepo`. Use existing
  `query_one` / `execute` patterns from `src/infra/postgres/user_repo.rs`.
- **Depends on**: Step 1
- **Checkpoint**: `cargo check`

[continue...]

#### Step N: Run full test suite
- **Checkpoint**: `cargo test`
- **Checkpoint**: `cargo clippy -- -D warnings`

### 5. Boundaries — DO NOT TOUCH
List files and modules explicitly out of scope. Example:
- `src/auth/` — no changes, authentication is unchanged
- `src/domain/model/user.rs` — do not modify User, only reference it

### 6. Known Pitfalls
Call out specific issues the agent should watch for:
- Lifetime annotations needed because of [reason]
- Orphan rule: can't impl [foreign trait] for [foreign type], use newtype
- Feature flag `[name]` must be enabled for [dependency]
- The existing `FooTrait` uses associated types, not generics — match that pattern
- [Crate X] re-exports [Type] — import from [correct path], not [wrong path]

### 7. Testing Strategy
- List specific test files to create or modify
- Provide test function signatures
- Specify what each test validates
- Include any test fixtures or mock patterns used in the codebase

## Rules for This Plan
- Every type, trait, and function signature must be actual Rust code, not prose
- Every file reference must be a full relative path from the crate root
- Every step must have an explicit cargo checkpoint command
- Steps must be ordered so that each step compiles independently after its dependencies
- Prefer small, compilable increments over large steps
- If a step involves more than 3 files, break it into smaller steps
- Match the existing codebase patterns (error handling, naming, module structure)
```

---

## Tips for Better Results

**Give Opus more context, get a better plan.** The most common failure mode is
Opus inventing types or module paths that don't exist. Counteract this by pasting:

- The actual `mod.rs` tree for affected modules
- The actual error type / Result alias the codebase uses
- A representative example of an existing similar implementation (e.g., if adding
  a new repository, paste an existing repo implementation so Opus matches the pattern)

**For very large changes**, ask Opus to split the plan into phases, where each
phase is a self-contained `todo.md` that compiles and passes tests independently.
This lets you feed one phase at a time to Sonnet without blowing context.

**After Opus generates the plan**, skim the type scaffolding section. If the types
look wrong, fix them before handing to Sonnet — it's 10x cheaper to correct a
signature in the plan than to let Sonnet build on a wrong foundation.
