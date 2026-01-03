;; Partnership Capital Test
;; Tests LP/GP structure and capital accounts
;; Phase D.4 of KYC Control Enhancement

(cbu.create
    :name "Partnership Test"
    :client-type "fund"
    :jurisdiction "KY"
    :as @cbu)

;; Create the LP
(entity.create-partnership-limited
    :cbu-id @cbu
    :name "Growth Fund I LP"
    :jurisdiction "KY"
    :as @fund-lp)

;; Create GP
(entity.create-limited-company
    :cbu-id @cbu
    :name "Growth GP Ltd"
    :company-number "KY54321"
    :jurisdiction "KY"
    :as @gp)

;; Create LPs (Limited Partners)
(entity.create-limited-company
    :cbu-id @cbu
    :name "Pension Fund A"
    :company-number "US11111"
    :jurisdiction "US"
    :as @lp-pension)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Sovereign Wealth Fund B"
    :company-number "AE22222"
    :jurisdiction "AE"
    :as @lp-swf)

;; Add GP with management rights and 20% profit share
(partnership.add-partner
    :cbu-id @cbu
    :partnership-entity-id @fund-lp
    :partner-entity-id @gp
    :partner-type "GP"
    :capital-commitment 1000000
    :profit-share-pct 20.0
    :management-rights true
    :admission-date "2023-01-01")

;; Add LPs with their capital commitments
(partnership.add-partner
    :cbu-id @cbu
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-pension
    :partner-type "LP"
    :capital-commitment 50000000
    :profit-share-pct 50.0
    :admission-date "2023-01-01")

(partnership.add-partner
    :cbu-id @cbu
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-swf
    :partner-type "LP"
    :capital-commitment 30000000
    :profit-share-pct 30.0
    :admission-date "2023-03-15")

;; Record capital contributions (drawdowns)
(partnership.record-contribution
    :partnership-entity-id @fund-lp
    :partner-entity-id @gp
    :amount 200000
    :contribution-date "2023-02-01"
    :reference "GP-DRAW-001")

(partnership.record-contribution
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-pension
    :amount 10000000
    :contribution-date "2023-02-01"
    :reference "LP-DRAW-001")

(partnership.record-contribution
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-swf
    :amount 6000000
    :contribution-date "2023-04-01"
    :reference "LP-DRAW-002")

;; Record a distribution
(partnership.record-distribution
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-pension
    :amount 500000
    :distribution-type "PROFIT"
    :distribution-date "2023-12-15")

;; Reconcile partnership - verify profit shares sum to 100%
(partnership.reconcile
    :partnership-entity-id @fund-lp
    :as @partnership-recon)

;; List all partners
(partnership.list-partners
    :partnership-entity-id @fund-lp
    :as @all-partners)

;; Analyze control structure (GP has control regardless of capital %)
(partnership.analyze-control
    :partnership-entity-id @fund-lp
    :as @control-analysis)
