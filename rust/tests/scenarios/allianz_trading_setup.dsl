;; ============================================================================
;; Allianz Trading Setup - Add instrument universe, ISDA, and CSA to all Allianz CBUs
;; ============================================================================
;; This script sets up trading data for testing the Trading view:
;; 1. Instrument universe (equities, bonds, FX, OTC derivatives)
;; 2. ISDA master agreements with Goldman Sachs and Morgan Stanley
;; 3. CSA (Credit Support Annex) for collateral management
;; ============================================================================

;; --- Pick a sample Allianz CBU to work with ---
;; Using "ALLIANZ PRIVATE EQUITY PARTNERS IV" as the test CBU

;; Add trading universe - Exchange traded instruments
(cbu-custody.add-universe
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :instrument-class "EQUITY_COMMON"
  :market "XPAR"
  :currencies ["EUR"]
  :settlement-types ["DVP"])

(cbu-custody.add-universe
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :instrument-class "EQUITY_ETF"
  :market "XPAR"
  :currencies ["EUR"]
  :settlement-types ["DVP"])

(cbu-custody.add-universe
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :instrument-class "CORP_BOND"
  :currencies ["EUR" "USD"]
  :settlement-types ["DVP"])

;; Add OTC universe - no market, counterparty-specific
(cbu-custody.add-universe
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :instrument-class "OTC_CDS"
  :currencies ["EUR" "USD"]
  :counterparty "5a662017-83b4-4046-8e3f-265ebe94c8d1")

(cbu-custody.add-universe
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :instrument-class "FX_FORWARD"
  :currencies ["EUR" "USD" "GBP"]
  :counterparty "5a662017-83b4-4046-8e3f-265ebe94c8d1")

;; --- ISDA Master Agreement with Goldman Sachs ---
(isda.create
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :counterparty "5a662017-83b4-4046-8e3f-265ebe94c8d1"
  :agreement-date "2020-01-15"
  :governing-law "ENGLISH"
  :effective-date "2020-02-01"
  :as @isda-gs)

;; Add coverage for OTC instruments under this ISDA
(isda.add-coverage
  :isda-id @isda-gs
  :instrument-class "OTC_CDS")

(isda.add-coverage
  :isda-id @isda-gs
  :instrument-class "FX_FORWARD")

(isda.add-coverage
  :isda-id @isda-gs
  :instrument-class "FX_SWAP")

;; Add VM CSA (Variation Margin)
(isda.add-csa
  :isda-id @isda-gs
  :csa-type "VM"
  :threshold 0
  :threshold-currency "EUR"
  :mta 500000
  :effective-date "2020-02-01"
  :as @csa-gs-vm)

;; --- ISDA Master Agreement with Morgan Stanley ---
(isda.create
  :cbu-id "8062177c-b3d2-441a-9347-226a25f0e0b1"
  :counterparty "ddea8df8-0f46-49c5-ae9d-6b7a300d76d8"
  :agreement-date "2021-06-01"
  :governing-law "NY"
  :effective-date "2021-07-01"
  :as @isda-ms)

;; Add coverage
(isda.add-coverage
  :isda-id @isda-ms
  :instrument-class "OTC_EQD")

(isda.add-coverage
  :isda-id @isda-ms
  :instrument-class "FX_OPTION")

;; Add both VM and IM CSA
(isda.add-csa
  :isda-id @isda-ms
  :csa-type "VM"
  :threshold 0
  :threshold-currency "USD"
  :mta 250000
  :effective-date "2021-07-01")

(isda.add-csa
  :isda-id @isda-ms
  :csa-type "IM"
  :threshold 50000000
  :threshold-currency "USD"
  :mta 1000000
  :effective-date "2021-07-01")
