;; ============================================================================
;; CBU Create - Hello World
;; ============================================================================
;; intent: Create a basic Client Business Unit (fund structure)
;;
;; The CBU is the atomic unit of the system. All trading, custody, and
;; regulatory relationships attach to a CBU. This example shows the
;; minimal setup for a Luxembourg SICAV fund.

;; ----------------------------------------------------------------------------
;; Step 1: Create the CBU
;; ----------------------------------------------------------------------------

;; intent: Create a new Luxembourg SICAV fund
;; macro: structure.setup
(cbu.create
  :name "Acme Global Equity Fund"
  :type "FUND"
  :jurisdiction "LU"
  :legal-form "SICAV"
  :as @fund)

;; ----------------------------------------------------------------------------
;; Step 2: Add Basic Metadata
;; ----------------------------------------------------------------------------

;; intent: Set the fund's base currency
(cbu.set-currency :cbu-id @fund :currency "EUR")

;; intent: Set the fund's domicile details
(cbu.set-domicile
  :cbu-id @fund
  :country "LU"
  :regulator "CSSF"
  :registration-number "O-123456")

;; ----------------------------------------------------------------------------
;; Step 3: Verify Creation
;; ----------------------------------------------------------------------------

;; intent: Confirm the CBU was created successfully
(cbu.get :id @fund)
