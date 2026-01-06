;; ==============================================================================
;; CLONE TRADING PROFILE TEST
;; ==============================================================================
;; This test proves that trading-profile.clone-to works with the new AST pipeline.
;; We clone the source profile (created by document_trading_profile_test.dsl)
;; to 5 Allianz CBUs.
;; ==============================================================================

;; Source profile from Document Test Fund
;; Profile ID: feb762e6-0cc4-4136-91c5-252740e53cca

;; Clone to Allianz CBU 1: ALLIANZ EPARGNE ACTIONS FRANCE
(trading-profile.clone-to
  :profile-id "feb762e6-0cc4-4136-91c5-252740e53cca"
  :target-cbu-id "06f1368d-dc2e-41c6-80f8-e6ab659d6358")

;; Clone to Allianz CBU 2: ALLIANZ EPARGNE ACTIONS MONDE
(trading-profile.clone-to
  :profile-id "feb762e6-0cc4-4136-91c5-252740e53cca"
  :target-cbu-id "3e6bca27-5eb5-42ed-a826-1af3422c1d18")

;; Clone to Allianz CBU 3: ALLIANZ EPARGNE ACTIONS SOLIDAIRE
(trading-profile.clone-to
  :profile-id "feb762e6-0cc4-4136-91c5-252740e53cca"
  :target-cbu-id "924eaf86-12eb-4316-b3c6-87ddbe22fbb8")

;; Clone to Allianz CBU 4: ALLIANZ EPARGNE DIVERSIFIE
(trading-profile.clone-to
  :profile-id "feb762e6-0cc4-4136-91c5-252740e53cca"
  :target-cbu-id "139ab2f8-dd64-4caf-8273-2e02d3baae65")

;; Clone to Allianz CBU 5: ALLIANZ EPARGNE OBLIGATIONS EURO
(trading-profile.clone-to
  :profile-id "feb762e6-0cc4-4136-91c5-252740e53cca"
  :target-cbu-id "1032a0bc-889e-4555-b170-b8678e2c0a88")
