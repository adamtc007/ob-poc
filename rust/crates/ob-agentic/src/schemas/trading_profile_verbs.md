# Trading Profile Verbs

## Profile Lifecycle

#### trading-profile.create-draft
Create a new trading profile draft for a CBU.
```
(trading-profile.create-draft
  :cbu-id @cbu
  :base-currency "USD"
  :as @profile)
```

#### trading-profile.submit
Submit profile for approval.
```
(trading-profile.submit :profile-id @profile)
```

#### trading-profile.approve
Approve a submitted profile.
```
(trading-profile.approve :profile-id @profile)
```

## Trading Universe

#### trading-profile.add-instrument-class
Add an instrument class to the trading universe.
```
(trading-profile.add-instrument-class
  :profile-id @profile
  :class-code "EQUITY")
```

Valid class codes: EQUITY, FIXED_INCOME, OTC_IRS, OTC_FX, ETF, FUND

#### trading-profile.add-market
Add a market to the trading universe.
```
(trading-profile.add-market
  :profile-id @profile
  :market-code "XNYS"
  :currencies ["USD"])
```

## Standing Instructions (SSI)

#### trading-profile.add-standing-instruction
Add an SSI to the profile.
```
(trading-profile.add-standing-instruction
  :profile-id @profile
  :ssi-name "US Primary"
  :ssi-type "SECURITIES"
  :safekeeping-account "SAFE-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "CASH-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi)
```

SSI types: SECURITIES, COLLATERAL, CASH_ONLY

## Booking Rules

#### trading-profile.add-booking-rule
Add a booking rule to route trades to SSIs.
```
(trading-profile.add-booking-rule
  :profile-id @profile
  :ssi-ref "US Primary"
  :rule-name "US Equity DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD")
```

Priority determines matching order (lower = higher priority).

## ISDA/CSA (OTC Derivatives)

#### trading-profile.add-isda-config
Add ISDA agreement with a counterparty.
```
(trading-profile.add-isda-config
  :profile-id @profile
  :counterparty-name "Morgan Stanley"
  :agreement-date "2024-01-15")
```

#### trading-profile.add-isda-coverage
Add product coverage to an ISDA.
```
(trading-profile.add-isda-coverage
  :profile-id @profile
  :counterparty-ref "Morgan Stanley"
  :asset-class "RATES"
  :base-products ["IRS" "XCCY"])
```

#### trading-profile.add-csa-config
Add CSA collateral agreement.
```
(trading-profile.add-csa-config
  :profile-id @profile
  :counterparty-ref "Morgan Stanley"
  :threshold-amount 10000000
  :threshold-currency "USD"
  :minimum-transfer 500000)
```

#### trading-profile.add-csa-collateral
Add eligible collateral types to CSA.
```
(trading-profile.add-csa-collateral
  :profile-id @profile
  :counterparty-ref "Morgan Stanley"
  :collateral-type "CASH"
  :currencies ["USD" "EUR"])
```

#### trading-profile.link-csa-ssi
Link SSI for collateral movements.
```
(trading-profile.link-csa-ssi
  :profile-id @profile
  :counterparty-ref "Morgan Stanley"
  :ssi-ref "Collateral")
```

## Validation

#### trading-profile.validate-go-live-ready
Validate profile is ready for go-live.
```
(trading-profile.validate-go-live-ready :profile-id @profile)
```

#### trading-profile.materialize
Project profile to operational tables.
```
(trading-profile.materialize :profile-id @profile)
```
