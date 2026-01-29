;; ============================================================================
;; UBO Mini Graph
;; ============================================================================
;; intent: Build an ownership chain for UBO discovery
;;
;; This example creates a typical holding structure to demonstrate
;; ownership percentage tracking and ultimate beneficial owner discovery.
;;
;;                    Anna Schmidt (UBO)
;;                           |
;;                          100%
;;                           v
;;                   Schmidt Family Trust
;;                           |
;;                          75%
;;                           v
;;                    Schmidt Holding AG
;;                     /           \
;;                   60%           40%
;;                   v              v
;;             Alpha GmbH     Beta S.a r.l.
;;                   \            /
;;                   30%        25%
;;                     \        /
;;                      v      v
;;                Target Fund (CBU)

;; ----------------------------------------------------------------------------
;; Step 1: Create the UBO (Natural Person at Top)
;; ----------------------------------------------------------------------------

;; intent: Create ultimate beneficial owner
;; macro: party.create
(entity.create-proper-person
  :first-name "Anna"
  :last-name "Schmidt"
  :nationality "DE"
  :date-of-birth "1965-08-12"
  :as @ubo)

;; ----------------------------------------------------------------------------
;; Step 2: Create Intermediate Holding Entities
;; ----------------------------------------------------------------------------

;; intent: Create family trust
;; macro: party.create
(entity.create
  :name "Schmidt Family Trust"
  :type "TRUST"
  :jurisdiction "LI"
  :as @trust)

;; intent: Create holding company
;; macro: party.create
(entity.create
  :name "Schmidt Holding AG"
  :type "LEGAL"
  :jurisdiction "CH"
  :as @holding)

;; intent: Create first intermediate company
;; macro: party.create
(entity.create
  :name "Alpha Investments GmbH"
  :type "LEGAL"
  :jurisdiction "DE"
  :as @alpha)

;; intent: Create second intermediate company
;; macro: party.create
(entity.create
  :name "Beta Capital S.a r.l."
  :type "LEGAL"
  :jurisdiction "LU"
  :as @beta)

;; ----------------------------------------------------------------------------
;; Step 3: Create the Target Fund
;; ----------------------------------------------------------------------------

;; intent: Create the fund being analyzed
;; macro: structure.setup
(cbu.create
  :name "European Growth Opportunities Fund"
  :type "FUND"
  :jurisdiction "LU"
  :as @fund)

;; ----------------------------------------------------------------------------
;; Step 4: Build Ownership Chain (Top Down)
;; ----------------------------------------------------------------------------

;; intent: UBO owns trust 100%
(ownership.create
  :owner-id @ubo
  :owned-id @trust
  :percentage 100.00
  :type "DIRECT"
  :as @link1)

;; intent: Trust owns holding 75%
(ownership.create
  :owner-id @trust
  :owned-id @holding
  :percentage 75.00
  :type "DIRECT"
  :as @link2)

;; intent: Holding owns Alpha 60%
(ownership.create
  :owner-id @holding
  :owned-id @alpha
  :percentage 60.00
  :type "DIRECT"
  :as @link3)

;; intent: Holding owns Beta 40%
(ownership.create
  :owner-id @holding
  :owned-id @beta
  :percentage 40.00
  :type "DIRECT"
  :as @link4)

;; intent: Alpha owns Fund 30%
(ownership.create
  :owner-id @alpha
  :owned-id @fund
  :percentage 30.00
  :type "DIRECT"
  :as @link5)

;; intent: Beta owns Fund 25%
(ownership.create
  :owner-id @beta
  :owned-id @fund
  :percentage 25.00
  :type "DIRECT"
  :as @link6)

;; ----------------------------------------------------------------------------
;; Step 5: Discover UBO
;; ----------------------------------------------------------------------------

;; intent: Calculate effective ownership through all chains
;; Anna's ownership of Fund:
;;   Path 1: 100% x 75% x 60% x 30% = 13.5%
;;   Path 2: 100% x 75% x 40% x 25% = 7.5%
;;   Total: 21.0%
(ubo.discover :entity-id @fund :threshold 10.0)

;; intent: Get full ownership graph
(ubo.get-ownership-graph :entity-id @fund :max-depth 5)

;; intent: List all ownership paths
(ubo.list-paths :entity-id @fund :ubo-id @ubo)
