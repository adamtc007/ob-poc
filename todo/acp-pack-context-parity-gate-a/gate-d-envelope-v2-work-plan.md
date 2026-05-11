# Gate D Envelope v2 Work Plan

Status: Gate D complete for the planned Slice 1 envelope v2 scope.

Completed in this slice:

1. [x] Added `AcpPackContextEnvelopeV2` as a per-pack deterministic envelope over the Gate C registry projection.
2. [x] Added build input pins for source projection hash, SemOS DSL hash, governed config artifact hash, registered fixture hash, builder version, and builder lockfile.
3. [x] Added hard byte/token budget reports for envelope and sections.
4. [x] Added deterministic section hashes and content hash chain.
5. [x] Added key-material-backed HMAC-SHA256 signature and verification with a deterministic development fixture key.
6. [x] Added structured verification refusals for schema mismatch, unsigned envelope, hash mismatch, signature mismatch, invalid signature, serialization failure, and budget overrun.
7. [x] Added lifecycle FSM shape: `Draft -> Active -> Deprecated -> Retired`.
8. [x] Added deterministic CLI: `cargo run --bin acp_pack_context_envelope_v2 -- config [all|pack-id]`.
9. [x] Added lifecycle re-sealing for permitted transitions.
10. [x] Added active-pack deterministic rebuild verification so signed active envelopes refuse drift from rebuilt output.
11. [x] Added deterministic omission coverage for forced section budget overflow.
12. [x] Added normalized production contracts for verb `returns`/`produces` metadata.
13. [x] Added signing-key registry policy checks for signature key id and algorithm.
14. [x] Added deterministic artifact rebuild bytes and byte-equality verification for CI-style checks.
15. [x] Added persisted registry state load checks that enforce active-pack rebuild immutability.
16. [x] Added development and production online registry-state load paths.
17. [x] Replaced the id-only development signer with explicit signing key material and production keyring support.

Current deterministic output evidence (refreshed for R2b §8/§14/§15 sections, 2026-05-11):

- `cargo run --bin acp_pack_context_envelope_v2 -- config all`
  - schema_version: `acp_pack_context_envelope_v3_bundle`
  - bytes: `429083`
  - SHA-256: `7a8c77517d4882f8761bb6ab94c2c1e3414a2550cb1020a286712f542479a623`
- `cargo run --bin acp_pack_context_envelope_v2 -- config cbu-maintenance`
  - schema_version: `acp_pack_context_envelope_v3`
  - bytes: `162783`
  - SHA-256: `d6a155be817adb17346cb7de41312238cffb7f348fdd5d6c7c98bfdc1616b229`

R2a baseline (pre-§8 sections, 2026-05-11):
- `config all`: bytes 409271, SHA-256 `d80b2abda10e6b539294ea001d5d8f6cf471a16da916af15add59ae92c38ad34`
- `config cbu-maintenance`: bytes 155866, SHA-256 `7dd69c92c0183f89ce6a5fe740242fdbf8eb0697091a588b3a087f08528effa1`

R6 byte-equality CI gate (2026-05-11):

- xtask command: `cargo run -p xtask -- acp-envelope-byte-equality-check`
- bless command: `cargo run -p xtask -- acp-envelope-byte-equality-check --bless`
- persisted baseline: `rust/tools/acp_envelope_baseline_v3.json`
- wired into `xtask check`, `xtask ci`, `xtask pre-commit`
- captures all 4 configs (`all`, `onboarding-request`, `cbu-maintenance`,
  `product-service-taxonomy`) — byte count + SHA-256 per entry
- baseline measured *with* trailing newline (matches what CLI emits to
  stdout); slightly different from shell-captured `printf %s` values

Current R6 baseline (4 entries, blessed 2026-05-11):

- `all`: bytes 429084, SHA-256 `3c83898c8195b6aee67b4c8c1e7cd664599fd381e6b1d1243a754a056ca8dda3`
- `onboarding-request`: bytes 73273, SHA-256 `a3e5ce80b33f06a158a2a913dbb684224de4262e459ab5c936aa63e13ef9d877`
- `cbu-maintenance`: bytes 162784, SHA-256 `6e83bba163b0fb55f02bb153ec4fcb621a086e18f505e5be69d3c878ebb36603`
- `product-service-taxonomy`: bytes 147102, SHA-256 `1f943a1f52c15e1e0017c6d03ae064083916f5755e9f500107ae9fbda3b00b3f`

Earlier v2 baseline (pre-R2a, 2026-05-10):

- `config all`: bytes 380875, SHA-256 `736a03b03e3cf9a97815afb280ac38c2d2b770fdb0015639e8a11048b651a8c8`
- `config cbu-maintenance`: bytes 133761, SHA-256 `d1f624ac5832c206f1f2c7208212a394a103293970886d0dd131e28ccbd9cd7d`

Current verification coverage:

- Build and verify a Slice 1 pack envelope.
- Same inputs produce byte-identical envelope JSON and matching envelope hashes.
- Unsigned envelope fails with structured refusal.
- Tampered body fails with hash-mismatch refusal.
- Lifecycle FSM only permits forward transitions.
- Draft-to-active lifecycle transition re-seals and verifies the envelope.
- Active registered envelope accepts equivalent deterministic rebuild output.
- Active registered envelope refuses signed but drifted rebuild output with `acp_active_pack_rebuild_mismatch`.
- Forced section budget overflow records a stable `section_byte_limit_exceeded` omission and stable section hashes.
- Production contracts expose normalized `return_type`, `produces_entity_grain`, entity grains, policy grade, and stable contract hashes.
- Unregistered signature keys and unregistered signature algorithms fail with structured refusals.
- Artifact bytes are built through one shared deterministic pretty JSON path for the CLI and rebuild verifier.
- Byte-identical artifact rebuilds pass verification.
- Byte mismatches fail with `acp_envelope_artifact_rebuild_mismatch`.
- Bundle artifacts declare their schema version and pack count.
- Persisted registry state stores signed envelopes with the source projection hash.
- Registry state load verifies schema, projection hash, pack count, pack set, signatures, and active-pack deterministic rebuild parity.
- Active persisted envelopes with valid signatures but drifted content fail load with `acp_active_pack_rebuild_mismatch`.
- Development online load can synthesize active registry state from current inputs, then verifies it through the same registry-state verifier.
- Production online load requires a persisted registry-state path and fails closed with `acp_registry_state_required` when none is configured.
- Production online load accepts persisted state only after schema, signature, projection, pack-set, and active rebuild checks pass.
- Production signing can use explicit `AcpPackContextSigningKey` material or `ACP_PACK_CONTEXT_SIGNING_KEY_ID` plus `ACP_PACK_CONTEXT_SIGNING_KEY_HEX`.
- Wrong key material for a registered key id fails verification with `acp_envelope_signature_invalid`.
- Unsupported algorithms for a registered key id fail with `acp_envelope_signature_algorithm_unsupported`.

Latest verification checkpoint:

- `cargo check` passed.
- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- Focused envelope tests pass with 23 cases.
- Full `cargo test` was not repeated after this item; prior full-suite run was stopped during doc-tests for time-budget reasons, after unit and integration targets had passed.
- Deterministic CLI evidence was re-baselined after Gate E static-policy projection began treating `outbox_write` and three-axis confirmation/emitting surfaces as gated policy material.

Remaining Gate D work:

- None.
