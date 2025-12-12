;; AllianzGI CBU Load Script
;; ============================================================================
;; DSL script to load AllianzGI fund structures into CBU
;; Uses verbs from: cbu.yaml, entity.yaml, fund.yaml
;; ============================================================================

;; ============================================================================
;; STEP 1: Create top-level CBU for AllianzGI group
;; ============================================================================

(cbu.ensure :name "Allianz Global Investors (Group)" :jurisdiction "DE" :client-type "ASSET_MANAGER" :as @agi_group_cbu)

;; ============================================================================
;; STEP 2: Create ManCo entities (legal entities that manage funds)
;; ============================================================================

;; Germany - HQ ManCo
(entity.ensure-limited-company :name "Allianz Global Investors GmbH" :jurisdiction "DE" :as @manco_de)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_de :role "MANAGEMENT_COMPANY")

;; Luxembourg Branch
(entity.ensure-limited-company :name "Allianz Global Investors GmbH - Luxembourg Branch" :jurisdiction "LU" :as @manco_lu)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_lu :role "MANAGEMENT_COMPANY")

;; UK ManCo
(entity.ensure-limited-company :name "Allianz Global Investors UK Limited" :jurisdiction "GB" :as @manco_uk)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_uk :role "MANAGEMENT_COMPANY")

;; Ireland ManCo
(entity.ensure-limited-company :name "Allianz Global Investors Ireland Limited" :jurisdiction "IE" :as @manco_ie)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_ie :role "MANAGEMENT_COMPANY")

;; Switzerland ManCo
(entity.ensure-limited-company :name "Allianz Global Investors (Schweiz) AG" :jurisdiction "CH" :as @manco_ch)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_ch :role "MANAGEMENT_COMPANY")

;; Hong Kong ManCo
(entity.ensure-limited-company :name "Allianz Global Investors Asia Pacific Limited" :jurisdiction "HK" :as @manco_hk)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_hk :role "MANAGEMENT_COMPANY")

;; Singapore ManCo
(entity.ensure-limited-company :name "Allianz Global Investors Singapore Limited" :jurisdiction "SG" :as @manco_sg)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_sg :role "MANAGEMENT_COMPANY")

;; Japan ManCo
(entity.ensure-limited-company :name "Allianz Global Investors Japan Co., Ltd." :jurisdiction "JP" :as @manco_jp)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_jp :role "MANAGEMENT_COMPANY")

;; Taiwan ManCo
(entity.ensure-limited-company :name "Allianz Global Investors Taiwan Ltd." :jurisdiction "TW" :as @manco_tw)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_tw :role "MANAGEMENT_COMPANY")

;; China ManCo
(entity.ensure-limited-company :name "Allianz Global Investors Fund Management Co., Ltd." :jurisdiction "CN" :as @manco_cn)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_cn :role "MANAGEMENT_COMPANY")

;; Indonesia ManCo
(entity.ensure-limited-company :name "PT Allianz Global Investors Asset Management Indonesia" :jurisdiction "ID" :as @manco_id)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @manco_id :role "MANAGEMENT_COMPANY")

;; ============================================================================
;; STEP 3: Create Service Provider entities
;; ============================================================================

;; State Street Luxembourg (Depositary)
(entity.ensure-limited-company :name "State Street Bank International GmbH, Luxembourg Branch" :jurisdiction "LU" :as @depositary_lu)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @depositary_lu :role "DEPOSITARY")

;; State Street Ireland (Depositary)
(entity.ensure-limited-company :name "State Street Custodial Services (Ireland) Limited" :jurisdiction "IE" :as @depositary_ie)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @depositary_ie :role "DEPOSITARY")

;; PwC Luxembourg (Auditor)
(entity.ensure-limited-company :name "PricewaterhouseCoopers, Société coopérative" :jurisdiction "LU" :as @auditor_lu)
(cbu.assign-role :cbu-id @agi_group_cbu :entity-id @auditor_lu :role "AUDITOR")

;; ============================================================================
;; STEP 4: Create Fund Umbrellas (SICAV structures)
;; ============================================================================

;; Main Luxembourg SICAV
(fund.ensure-umbrella :name "Allianz Global Investors Fund" :jurisdiction "LU" :fund-structure-type "SICAV" :regulatory-status "UCITS" :cbu-id @agi_group_cbu :as @sicav_main)

;; Secondary Luxembourg SICAV
(fund.ensure-umbrella :name "Allianz Global Investors Fund II" :jurisdiction "LU" :fund-structure-type "SICAV" :regulatory-status "UCITS" :cbu-id @agi_group_cbu :as @sicav_secondary)

;; ============================================================================
;; STEP 5: Create Sub-Funds (Compartments)
;; ============================================================================

;; Global AI Sub-Fund
(fund.ensure-subfund :name "Allianz Global Artificial Intelligence" :umbrella-id @sicav_main :base-currency "USD" :cbu-id @agi_group_cbu :as @subfund_ai)

;; Emerging Markets Equity
(fund.ensure-subfund :name "Allianz Emerging Markets Equity" :umbrella-id @sicav_main :base-currency "USD" :cbu-id @agi_group_cbu :as @subfund_em)

;; Europe Equity Growth
(fund.ensure-subfund :name "Allianz Europe Equity Growth" :umbrella-id @sicav_main :base-currency "EUR" :cbu-id @agi_group_cbu :as @subfund_europe)

;; Global Fixed Income
(fund.ensure-subfund :name "Allianz Global Fixed Income" :umbrella-id @sicav_main :base-currency "EUR" :cbu-id @agi_group_cbu :as @subfund_bond)

;; Global Sustainability
(fund.ensure-subfund :name "Allianz Global Sustainability" :umbrella-id @sicav_main :base-currency "EUR" :cbu-id @agi_group_cbu :as @subfund_sust)

;; Food Security
(fund.ensure-subfund :name "Allianz Food Security" :umbrella-id @sicav_main :base-currency "EUR" :cbu-id @agi_group_cbu :as @subfund_food)

;; ============================================================================
;; STEP 6: Create Share Classes
;; ============================================================================

;; --- Global AI Share Classes ---

(fund.ensure-share-class :name "Allianz Global Artificial Intelligence - A - EUR" :subfund-id @subfund_ai :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :isin "LU1603246700" :management-fee-bps 180 :as @sc_ai_a_eur)

(fund.ensure-share-class :name "Allianz Global Artificial Intelligence - AT - EUR" :subfund-id @subfund_ai :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :isin "LU1548497772" :management-fee-bps 210 :as @sc_ai_at_eur)

(fund.ensure-share-class :name "Allianz Global Artificial Intelligence - IT - USD" :subfund-id @subfund_ai :share-class-type "INSTITUTIONAL" :distribution-type "ACC" :currency "USD" :isin "LU1548498150" :management-fee-bps 90 :minimum-investment 10000000 :as @sc_ai_it_usd)

(fund.ensure-share-class :name "Allianz Global Artificial Intelligence - WT - USD" :subfund-id @subfund_ai :share-class-type "INSTITUTIONAL" :distribution-type "ACC" :currency "USD" :isin "LU1548498317" :management-fee-bps 75 :minimum-investment 100000000 :as @sc_ai_wt_usd)

(fund.ensure-share-class :name "Allianz Global Artificial Intelligence - AT (H2-EUR) - EUR" :subfund-id @subfund_ai :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :hedged true :isin "LU1622405166" :management-fee-bps 210 :as @sc_ai_h2_eur)

;; --- Emerging Markets Share Classes ---

(fund.ensure-share-class :name "Allianz Emerging Markets Equity - A - EUR" :subfund-id @subfund_em :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :isin "LU0256863811" :management-fee-bps 175 :as @sc_em_a_eur)

(fund.ensure-share-class :name "Allianz Emerging Markets Equity - IT - USD" :subfund-id @subfund_em :share-class-type "INSTITUTIONAL" :distribution-type "ACC" :currency "USD" :isin "LU0256864207" :management-fee-bps 85 :minimum-investment 10000000 :as @sc_em_it_usd)

;; --- Global Sustainability Share Classes ---

(fund.ensure-share-class :name "Allianz Global Sustainability - A - EUR" :subfund-id @subfund_sust :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :isin "LU0158827955" :management-fee-bps 150 :as @sc_sust_a_eur)

(fund.ensure-share-class :name "Allianz Global Sustainability - IT - EUR" :subfund-id @subfund_sust :share-class-type "INSTITUTIONAL" :distribution-type "ACC" :currency "EUR" :isin "LU0908560596" :management-fee-bps 75 :minimum-investment 10000000 :as @sc_sust_it_eur)

;; --- Food Security Share Classes ---

(fund.ensure-share-class :name "Allianz Food Security - A - EUR" :subfund-id @subfund_food :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :isin "LU2021029245" :management-fee-bps 160 :as @sc_food_a_eur)

(fund.ensure-share-class :name "Allianz Food Security - IT - EUR" :subfund-id @subfund_food :share-class-type "INSTITUTIONAL" :distribution-type "ACC" :currency "EUR" :isin "LU2021029591" :management-fee-bps 80 :minimum-investment 10000000 :as @sc_food_it_eur)

;; --- Global Fixed Income Share Classes ---

(fund.ensure-share-class :name "Allianz Global Fixed Income - A - EUR" :subfund-id @subfund_bond :share-class-type "RETAIL" :distribution-type "ACC" :currency "EUR" :isin "LU0189894933" :management-fee-bps 90 :as @sc_bond_a_eur)

(fund.ensure-share-class :name "Allianz Global Fixed Income - IT - EUR" :subfund-id @subfund_bond :share-class-type "INSTITUTIONAL" :distribution-type "ACC" :currency "EUR" :isin "LU0189895237" :management-fee-bps 45 :minimum-investment 10000000 :as @sc_bond_it_eur)

(fund.ensure-share-class :name "Allianz Global Fixed Income - ADM - EUR" :subfund-id @subfund_bond :share-class-type "RETAIL" :distribution-type "DIST" :currency "EUR" :isin "LU0189895666" :management-fee-bps 90 :as @sc_bond_adm_eur)

;; ============================================================================
;; SUMMARY
;; ============================================================================
;; Created:
;;   - 1 CBU (AllianzGI Group)
;;   - 11 ManCo entities
;;   - 3 Service Provider entities
;;   - 2 Umbrella funds (SICAV)
;;   - 6 Sub-funds
;;   - 15 Share classes
;; ============================================================================
