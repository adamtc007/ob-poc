# Super-Crate Findings

Status: audit draft for Gate A replan.

Finding:

The root `ob-poc` crate is a super-crate for the context parity work. It has the largest public surface, owns API routes, exposes binaries, depends on most local subsystems, and is pulled into `ob-poc-web` and `dsl-lsp`.

Risk:

- Repo-aware assistants can see too many callable routes and helper APIs.
- Utterance parsing can accidentally reach execution/database surfaces.
- Public items become de facto contracts even when intended as internal implementation details.

Secondary wide surfaces:

- `sem_os_core`
- `ob-poc-types`
- `dsl-runtime`
- `dsl-core`
- `sem_os_postgres`

Recommendation:

Classify root crate exports before moving code. The first mechanical migration should reduce visibility with `pub(crate)`/`pub(super)` where no external crate consumes the symbol.
