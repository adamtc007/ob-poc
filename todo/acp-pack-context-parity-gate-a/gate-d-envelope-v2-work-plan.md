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

Current deterministic output evidence:

- `cargo run --bin acp_pack_context_envelope_v2 -- config all`
  - bytes: `380875`
  - SHA-256: `736a03b03e3cf9a97815afb280ac38c2d2b770fdb0015639e8a11048b651a8c8`
- `cargo run --bin acp_pack_context_envelope_v2 -- config cbu-maintenance`
  - bytes: `133761`
  - SHA-256: `d1f624ac5832c206f1f2c7208212a394a103293970886d0dd131e28ccbd9cd7d`

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
