# Custody & Settlement

Three-layer model for settlement instruction routing (SWIFT/ISO aligned).

## Architecture
```
Layer 1: UNIVERSE - What does the CBU trade?
         (instrument classes, markets, currencies)
              ↓
Layer 2: SSI DATA - Pure account information
         (safekeeping account + BIC, cash account, PSET BIC)
              ↓
Layer 3: BOOKING RULES - ALERT-style routing
         (trade characteristics → SSI, priority-based)
```

## Key Tables (custody schema)
- custody.cbu_instrument_universe - Tradeable instruments/markets
- custody.cbu_ssi - Standing Settlement Instructions
- custody.ssi_booking_rules - ALERT-style routing rules
- custody.isda_agreements - ISDA master agreements
- custody.csa_agreements - Credit support annexes

## Reference Tables
- custody.instrument_classes - CFI-based (EQUITY, GOVT_BOND, etc.)
- custody.markets - ISO 10383 MIC codes (XNYS, XLON, etc.)
- custody.security_types - SMPG/ALERT taxonomy

## DSL Verbs (cbu-custody.* domain)
- add-universe, list-universe
- create-ssi, activate-ssi, suspend-ssi, list-ssis
- add-booking-rule, list-booking-rules, update-rule-priority
- validate-booking-coverage - Check rules cover universe
- lookup-ssi - Find SSI for trade characteristics

Read CLAUDE.md section "Custody & Settlement DSL".
