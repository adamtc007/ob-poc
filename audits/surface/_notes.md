# Notes — Phase 1 surface sweep

HEAD=86031a08098ee6b86f2f5c3a07acf3ab929d9c3c

## booking_principal disposition (Step 6)

`rg -l "booking_principal_types|BookingPrincipal" --type rust` hits:
- `crates/sem_os_postgres/src/service_options.rs` — unrelated: a `FanoutAxis::BookingPrincipal`
  enum variant (string tag `"booking_principal"`), not the DTO module.
- `crates/ob-poc-boundary/src/lib.rs:66` — a historical comment only (Phase 4.1, 2026-05-13),
  documenting that `booking_principal_types` was relocated out of boundary at that time.

**No `ob-poc-booking-principal` crate exists in the workspace today** (confirmed absent from
`cargo metadata`'s 56-member list and absent from `crates/`).

Full file history (`git log --all --follow --name-status`):

1. `rust/src/api/booking_principal_types.rs` — added 2026-02-11 (`c43c248c`).
2. → renamed into `ob-poc-envelope/src/booking_principal_types.rs` (2026-05-13, `b1ed5304`).
3. → renamed into `ob-poc-boundary/src/booking_principal_types.rs` (2026-05-13, `3c3d990b`,
   same commit that produced the boundary comment above).
4. → renamed into `ob-poc-domain/src/booking_principal_types.rs` (2026-05-13, `c0e9b691`).
5. → **standalone crate created**: commit `88943b17` (2026-05-14, "split A3:
   ob-poc-booking-principal ← booking_principal_types") moved the 485-LOC file into a new
   `ob-poc-booking-principal` crate — this is exactly what `ob-poc-domain-split-v1.md`
   proposed ("distinct ownership — do not fold"), and it landed as specified.
6. → **deleted** 2026-06-15, commit `4d513f26` / `0078159` ("refactor(deal): remove booking
   principals, update SLAs and coverage banker, clippy clean workspace") — dropped alongside
   `migrations/072_booking_principals.sql` and `072b_booking_principals_seed.sql`. This was a
   **product decision to remove the booking-principal feature entirely**, not a fold-back or
   an abandoned migration. The crate-split plan's intent was honored right up until the
   feature itself was cut.

**Conclusion:** the domain-split-v1 DRIFT flag on booking_principal is stale. There is nothing
to reconcile — the crate existed correctly, then the feature it hosted was deliberately
removed. No further action.

(Residual unrelated hits, not booking-principal DTOs: `xtask/src/instrument_harness.rs:265`
comment referencing a `booking_principal` FK column in a still-live `legal_entity` seed;
`src/sem_os_runtime/constellation_runtime.rs:1003` string `"booking_principals"` — likely a
stale table-name reference worth a follow-up grep in Phase 2, not investigated further here
since it's out of scope for a booking_principal_types *crate* disposition check.)
