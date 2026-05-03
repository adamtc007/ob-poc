# Instrument Matrix Source Paths - Service Options Pilot

Status: seed contract for Custody/Fund Accounting service-options pilots.

The v0.2 service-options implementation uses only the IM paths needed by the
approved pilot scope. Wider IM path audit remains a Phase 10 follow-up.

## SETTLEMENT

- `preferred_speed` -> `service_option_defs.option_key = settlement_speed`
- `counterparties` -> `service_option_defs.option_key = default_counterparties`

## TRADE_CAPTURE Boundary Proof

TRADE_CAPTURE is outside the Custody/Fund Accounting rollout unless needed to
prove the IM source contract. If exercised, the pilot paths are:

- `instruction_channel`
- `bic`
- `message_types`
