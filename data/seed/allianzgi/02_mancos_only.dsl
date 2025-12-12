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

