# `runbook-gate-vnext` Deployment Audit — 2026-04-22

**Slice:** 0.2 of `three-plane-correction-slice-plan-2026-04-22.md`.
**Purpose:** enumerate every binary that serves traffic and every deployment surface that builds
one, so Slice 4.1 (rip the `cfg(not(feature = "runbook-gate-vnext"))` branch) can proceed without
silently breaking a deployment where the feature is OFF.

**TL;DR:** two binaries exist with different feature sets. `ob-poc-web` has the feature ON;
`dsl_api` (the Docker image) has it OFF. Slice 4.1 is blocked until one path is chosen: retire
`dsl_api`, unify its feature set, or invert the peer review's conclusion and keep the legacy
branch.

---

## 1. Binary × feature matrix

| Binary | Source | Build command | Feature chain | `runbook-gate-vnext` |
|--------|--------|---------------|---------------|----------------------|
| `ob-poc-web` | `rust/crates/ob-poc-web/src/main.rs` | `cargo build --release -p ob-poc-web` (from `rust/xtask/src/main.rs:2272,2276`, i.e. `cargo x deploy`) | `ob-poc = { path = "../..", features = ["server", "vnext-repl", "runbook-gate-vnext"] }` (`rust/crates/ob-poc-web/Cargo.toml:31`) | **ON** |
| `dsl_api` | `rust/src/bin/dsl_api.rs` | `cargo build --locked --release --features server --bin dsl_api` (`Dockerfile.api:32`) | workspace default features = `["server"]` (`rust/Cargo.toml:204`); `server = ["database", "mcp", ...]` — no `runbook-gate-vnext` | **OFF** |
| `xtask` binaries | `rust/xtask/src/**` | `cargo run -p xtask -- ...` | `ob-poc = { path = "..", features = ["database", "vnext-repl"] }` (`rust/xtask/Cargo.toml:20`) | **OFF** (xtask does not serve HTTP traffic — harness/CLI only) |

Feature definitions live at `rust/Cargo.toml:203-212`:

```toml
[features]
default = ["server"]
database = ["dep:sqlx", ...]
server = ["database", "mcp", "dep:axum", ...]
cli = [...]
mcp = ["database"]
vnext-repl = ["database"]  # DEPRECATED — REPL V2 always enabled
write-set-contract = []
runbook-gate-vnext = ["database"]
```

`runbook-gate-vnext` is **not implied** by `server`. It must be added explicitly to the feature
list at the call site. Only `ob-poc-web`'s Cargo.toml does this.

---

## 2. Deployment surface × binary

| Surface | Artefact | Binary | `runbook-gate-vnext` |
|---------|----------|--------|----------------------|
| Local dev (`cargo x deploy`) | `rust/target/release/ob-poc-web` | `ob-poc-web` | ON |
| Docker image (API) | `Dockerfile.api` | `dsl_api` | OFF |
| xtask harness runs | per-command | various xtask bins | OFF (xtask-local) |
| CI builds | — | — | none enumerated (see §4) |

### 2a. `cargo x deploy` path (authoritative for local dev)

`rust/xtask/src/main.rs:2209` `deploy()` function:

- L2272-2278: `cargo build [-release] -p ob-poc-web`
- L2298-2300: chooses `rust/target/{release,debug}/ob-poc-web`
- L2220: kills any existing `ob-poc-web` process before start

No `dsl_api` references in `deploy()`. This path is `ob-poc-web` exclusively.

### 2b. Dockerfile.api path (authoritative for containerised API)

`Dockerfile.api:32`:

```dockerfile
RUN cargo build --locked --release --features server --bin dsl_api
```

- Binary built: `dsl_api` (workspace-level bin defined at `rust/Cargo.toml:188-190`, source at
  `rust/src/bin/dsl_api.rs`).
- Feature chain: `--features server` → inherits workspace default, which does NOT include
  `runbook-gate-vnext`.
- Runtime: `CMD ["/app/dsl_api"]` on port 3001.

### 2c. `bpmn-lite/Dockerfile` (out of scope for this audit)

This Dockerfile builds the separate BPMN-Lite gRPC service. It does not build any `ob-poc` binary.
No `runbook-gate-vnext` relevance.

---

## 3. Which binary runs production traffic?

**[USER INPUT REQUIRED]**

Current state on the main repo: two binaries can both build successfully; the feature split is not
resolved by any automation in the tree. The answer determines Slice 4.1 scope.

Resolution paths:

### Path (a) — `ob-poc-web` is authoritative; retire `dsl_api`

- Delete `rust/src/bin/dsl_api.rs` and the `[[bin]] name = "dsl_api"` entry at
  `rust/Cargo.toml:188-190`.
- Delete `Dockerfile.api`.
- Slice 4.1 proceeds as originally scoped (rip `cfg(not(feature = "runbook-gate-vnext"))` branch).
- Risk: any deployment pipeline outside this repo that builds from `Dockerfile.api` breaks. The
  owner of those pipelines must be informed first.

### Path (b) — Both are live; unify feature set

- Change `Dockerfile.api:32` to `cargo build --locked --release --features server,runbook-gate-vnext --bin dsl_api`.
- Verify `dsl_api` compiles with the feature on (it likely does — the feature guards code paths in
  `rust/src/api/`, all of which are shared between binaries).
- Slice 4.1 proceeds.
- Risk: changing the runtime behaviour of the Docker image without a staged rollout. Pre-deploy
  harness run mandatory.

### Path (c) — `dsl_api` is authoritative; retire `ob-poc-web`

- F18 claim in the peer review is inverted. The legacy `execute_resolved_dsl` path is the keeper.
- Slice 4.1 deletes the `#[cfg(feature = "runbook-gate-vnext")]` branch, not the `#[cfg(not)]`
  branch.
- Retire `rust/crates/ob-poc-web/`.
- Risk: high — this is a much larger change, and the React frontend is served by `ob-poc-web`.
  Path (c) is unlikely to be correct.

**Recommended:** (a) or (b). (a) if the Docker image is legacy; (b) if it's still in active use.

---

## 4. Other deployment surfaces audited

### CI

- `.github/workflows/*.yml`: **zero files found** (glob returned empty).
- No Travis, CircleCI, Jenkins config at repo root.
- Conclusion: no automated CI build matrix to audit; if CI exists it lives outside this repo.

### Docker Compose

- **No `docker-compose.yml`** at repo root.
- `bpmn-lite/` has its own Dockerfile but no compose file.
- Conclusion: deployment orchestration is external to this repo.

### Other deployment scripts

- `rust/xtask/src/main.rs` is the only in-repo deploy orchestrator. It only targets `ob-poc-web`.
- `rust/xtask/src/bpmn_lite.rs` builds + deploys BPMN-Lite via `docker build -t bpmn-lite ./bpmn-lite` — separate service.

---

## 5. Deprecated `vnext-repl` feature (Slice 4.2 removal target)

`rust/Cargo.toml:210`:

```toml
vnext-repl = ["database"]  # DEPRECATED: REPL V2 always enabled. Kept for downstream Cargo.toml compat.
```

Referenced at:

- `rust/crates/ob-poc-web/Cargo.toml:31` — in ob-poc-web's feature chain.
- `rust/xtask/Cargo.toml:20` — in xtask's feature chain.

Neither reference activates conditional compilation (grep for `cfg(feature = "vnext-repl")` in
`rust/src/`: zero hits in code — review finding confirmed). The feature is vestigial.

Slice 4.2 action:

1. Remove the two consumer references.
2. Remove the definition line.

No runtime impact.

---

## 6. `runbook-gate-vnext` cfg sites inventory (for Slice 4.1 execution)

Pre-computed so Slice 4.1 executor has a complete file list.

Active cfg annotations in `rust/src/`:

- `rust/src/api/agent_service.rs:116` `#[cfg(not(feature = "runbook-gate-vnext"))]` (import block)
- `rust/src/api/agent_service.rs:123` `#[cfg(not(feature = "runbook-gate-vnext"))]`
- `rust/src/api/agent_service.rs:254` `#[cfg(feature = "runbook-gate-vnext")]`
- `rust/src/api/agent_service.rs:282` `#[cfg(feature = "runbook-gate-vnext")]`
- `rust/src/api/agent_service.rs:292` `#[cfg(feature = "runbook-gate-vnext")]`
- `rust/src/api/agent_service.rs:1144` `#[cfg(not(feature = "runbook-gate-vnext"))]` (legacy `execute_resolved_dsl`)
- `rust/src/api/agent_service.rs:1289` `#[cfg(feature = "runbook-gate-vnext")]`
- `rust/src/api/agent_service.rs:1594` `#[cfg(not(feature = "runbook-gate-vnext"))]`

Invariant test referencing the cfg:

- `rust/src/runbook/invariant_tests.rs:59` — string-scan test enforcing INV-1; must be rewritten or
  retired when the cfg goes away.
- `rust/src/runbook/executor.rs:1563-1613` — INV-1 documentation + validator. Same story.

Docs:

- `docs/todo/phase-2.5-dispatch-audit.md` — multiple historical references; can stay as-is.

---

## 7. Recommended next action

Open a focused question to the user:

> "Production API traffic — which binary serves it?
> 
> - `ob-poc-web` (cargo x deploy, feature ON), or
> - `dsl_api` (Dockerfile.api, feature OFF), or
> - both (different environments)?
> 
> The answer determines Slice 4.1 scope."

Once answered, update the master plan Slice 4.1 section with the chosen resolution path from §3.

---

## 8. Audit status

- Binary × feature matrix: **complete**.
- Deployment surface enumeration: **complete** (this repo — external infra out of scope).
- Slice 4.1 cfg-site inventory: **complete**.
- Blocking question for Slice 4.1: **filed** (see §3 and §7).
- Slice 0.2 acceptance: **met**.
