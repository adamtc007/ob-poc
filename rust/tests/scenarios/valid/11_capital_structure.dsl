;; Capital Structure Test
;; Tests corporate share registry and ownership reconciliation
;; Phase D.1 of KYC Control Enhancement

(cbu.create
    :name "Capital Structure Test Co"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

;; Create the company
(entity.create-limited-company
    :cbu-id @cbu
    :name "Test Holdings Ltd"
    :company-number "UK999888"
    :jurisdiction "GB"
    :as @company)

;; Create shareholders
(entity.create-limited-company
    :cbu-id @cbu
    :name "Majority Investor Ltd"
    :company-number "UK999777"
    :jurisdiction "GB"
    :as @majority-holder)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "John"
    :last-name "Founder"
    :date-of-birth "1970-01-01"
    :nationality "GB"
    :as @founder)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Jane"
    :last-name "Angel"
    :date-of-birth "1980-05-15"
    :nationality "US"
    :as @angel)

;; Define share classes
(capital.define-share-class
    :cbu-id @cbu
    :issuer-entity-id @company
    :name "Ordinary Shares"
    :share-type "ORDINARY"
    :issued-shares 1000000
    :voting-rights-per-share 1.0
    :par-value 0.01
    :as @ordinary)

(capital.define-share-class
    :cbu-id @cbu
    :issuer-entity-id @company
    :name "A Preference Shares"
    :share-type "PREFERENCE_A"
    :issued-shares 500000
    :voting-rights-per-share 0
    :par-value 1.00
    :as @pref-a)

;; Allocate shares - should sum to 100% of voting shares
(capital.allocate
    :share-class-id @ordinary
    :shareholder-entity-id @majority-holder
    :units 600000
    :acquisition-date "2020-01-01")

(capital.allocate
    :share-class-id @ordinary
    :shareholder-entity-id @founder
    :units 300000
    :acquisition-date "2020-01-01")

(capital.allocate
    :share-class-id @ordinary
    :shareholder-entity-id @angel
    :units 100000
    :acquisition-date "2021-06-15")

(capital.allocate
    :share-class-id @pref-a
    :shareholder-entity-id @majority-holder
    :units 500000
    :acquisition-date "2022-03-01")

;; Reconcile - verify voting shares sum to 100%
(capital.reconcile
    :entity-id @company
    :as @reconciliation)

;; Get ownership chain for UBO tracing
(capital.get-ownership-chain
    :entity-id @company
    :as @chain)
