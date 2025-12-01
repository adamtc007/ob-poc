## cbu domain

### cbu.ensure
Create or update a CBU.
```
(cbu.ensure :name "string" :jurisdiction "XX" :client-type "FUND|CORPORATE|INDIVIDUAL" :as @variable)
```

## cbu-custody domain

### cbu-custody.add-universe
Declare what a CBU trades.
```
(cbu-custody.add-universe 
  :cbu-id @cbu 
  :instrument-class "EQUITY|GOVT_BOND|CORP_BOND|ETF|OTC_IRS|OTC_CDS"
  :market "XNYS|XLON|XFRA|XPAR" ;; optional for OTC
  :currencies ["USD" "GBP"]
  :settlement-types ["DVP"]  ;; optional, default DVP
  :counterparty @entity)     ;; optional, for OTC
```

### cbu-custody.create-ssi
Create Standing Settlement Instruction.
```
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary"
  :type "SECURITIES|CASH|COLLATERAL|FX_NOSTRO"
  :safekeeping-account "SAFE-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "CASH-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi)
```

### cbu-custody.activate-ssi
Activate an SSI.
```
(cbu-custody.activate-ssi :ssi-id @ssi)
```

### cbu-custody.add-booking-rule
Add routing rule. NULL criteria = wildcard (matches any).
```
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi
  :name "Rule Name"
  :priority 10              ;; lower = higher priority
  :instrument-class "EQUITY" ;; optional
  :market "XNYS"            ;; optional
  :currency "USD"           ;; optional
  :settlement-type "DVP"    ;; optional
  :counterparty @entity     ;; optional, for OTC
  :effective-date "2024-12-01")
```

### cbu-custody.validate-booking-coverage
Validate all universe entries have matching rules.
```
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

## isda domain

### isda.create
Create ISDA master agreement.
```
(isda.create
  :cbu-id @cbu
  :counterparty @entity
  :agreement-date "2024-12-01"
  :governing-law "NY|ENGLISH"
  :effective-date "2024-12-01"
  :as @isda)
```

### isda.add-coverage
Add instrument class coverage.
```
(isda.add-coverage :isda-id @isda :instrument-class "OTC_IRS")
```

### isda.add-csa
Add Credit Support Annex.
```
(isda.add-csa
  :isda-id @isda
  :csa-type "VM|IM"
  :threshold 250000
  :threshold-currency "USD"
  :collateral-ssi @ssi
  :effective-date "2024-12-01"
  :as @csa)
```

## entity domain

### entity.read
Lookup existing entity by name.
```
(entity.read :name "Morgan Stanley" :as @ms)
```

### entity.create-limited-company
Create a new company entity.
```
(entity.create-limited-company :name "Acme Corp" :jurisdiction "US" :as @company)
```

### entity.create-proper-person
Create a new individual entity.
```
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @person)
```
