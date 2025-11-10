;; Phase 4 Simple Test DSL
;; Basic validation of document and ISDA verbs for parser testing

;; Simple document cataloging test
(document.catalog
  :document-id "doc-test-001"
  :document-type "CONTRACT"
  :issuer "test-authority"
  :title "Test Document for Phase 4"
  :jurisdiction "US"
  :language "EN")

;; Simple document verification test
(document.verify
  :document-id "doc-test-001"
  :verification-method "DIGITAL_SIGNATURE"
  :verification-result "AUTHENTIC")

;; Simple ISDA master agreement test
(isda.establish_master
  :agreement-id "ISDA-TEST-001"
  :party-a "entity-a"
  :party-b "entity-b"
  :version "2002"
  :governing-law "NY"
  :agreement-date "2024-01-15")

;; Simple ISDA trade execution test
(isda.execute_trade
  :trade-id "TRADE-TEST-001"
  :master-agreement-id "ISDA-TEST-001"
  :product-type "IRS"
  :trade-date "2024-03-15"
  :notional-amount 10000000
  :currency "USD")

;; Test basic entity creation (existing verb)
(entity
  :id "test-entity-001"
  :label "Company"
  :props {
    :legal-name "Test Company Inc"
    :jurisdiction "DE"
  })
