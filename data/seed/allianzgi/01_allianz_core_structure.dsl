;; =============================================================================
;; Allianz Global Investors - Full ETL Load
;; Generated: 2024-12-28
;; =============================================================================
;;
;; Structure:
;;   Allianz SE (public, UBO terminus)
;;       └── Allianz Global Investors GmbH (DE) - Main IM
;;               ├── Allianz Global Investors (Schweiz) AG (CH)
;;               ├── Allianz Global Investors UK Limited (GB)
;;               ├── Allianz Global Investors Ireland Limited (IE)
;;               ├── Allianz Capital Partners of America LLC (US)
;;               └── Allianz Global Investors Japan KK (JP)
;;
;; Data sources:
;;   - GLEIF API (ownership chain, LEIs)
;;   - AllianzGI websites (funds, share classes)
;; =============================================================================

;; =============================================================================
;; PHASE 1: ULTIMATE PARENT (UBO Terminus - Public Company)
;; =============================================================================

(entity.upsert-company
  :name "Allianz SE"
  :jurisdiction "DE"
  :legal-form "SE"
  :lei "529900K9B0N5BT694847"
  :registration-number "HRB 164232"
  :registration-authority "Amtsgericht München"
  :address "Königinstr. 28, München"
  :is-publicly-traded true
  :stock-exchange "XETRA"
  :is-ubo-terminus true
  :ubo-terminus-reason "PUBLIC_COMPANY"
  :as @allianz_se)

;; =============================================================================
;; PHASE 2: MAIN INVESTMENT MANAGER
;; =============================================================================

(entity.upsert-company
  :name "Allianz Global Investors GmbH"
  :jurisdiction "DE"
  :legal-form "GmbH"
  :lei "OJ2TIQSVQND4IZYYK658"
  :registration-number "HRB 9340"
  :registration-authority "Amtsgericht Frankfurt am Main"
  :address "Bockenheimer Landstraße 42-44, Frankfurt am Main"
  :bic "AGIDDEFBXXX"
  :regulatory-status "AIFM"
  :regulator "BaFin"
  :as @agi_gmbh)

;; =============================================================================
;; PHASE 3: REGIONAL MANCOS / SUBSIDIARIES
;; =============================================================================

;; Switzerland
(entity.upsert-company
  :name "Allianz Global Investors (Schweiz) AG"
  :jurisdiction "CH"
  :legal-form "AG"
  :regulatory-status "FINMA_AUTHORISED"
  :regulator "FINMA"
  :as @agi_ch)

;; UK
(entity.upsert-company
  :name "Allianz Global Investors UK Limited"
  :jurisdiction "GB"
  :legal-form "LIMITED"
  :regulatory-status "FCA_AUTHORISED"
  :regulator "FCA"
  :as @agi_uk)

;; Ireland
(entity.upsert-company
  :name "Allianz Global Investors Ireland Limited"
  :jurisdiction "IE"
  :legal-form "LIMITED"
  :regulatory-status "CBI_AUTHORISED"
  :regulator "CBI"
  :as @agi_ie)

;; Luxembourg Branch (not separate entity, but tracked)
(entity.upsert-company
  :name "Allianz Global Investors GmbH, Luxembourg Branch"
  :jurisdiction "LU"
  :legal-form "BRANCH"
  :lei "529900LMMFP4CM8ZOO35"
  :regulatory-status "CSSF_PASSPORTED"
  :regulator "CSSF"
  :parent-entity-id @agi_gmbh
  :as @agi_lu_branch)

;; US subsidiary
(entity.upsert-company
  :name "Allianz Capital Partners of America LLC"
  :jurisdiction "US"
  :legal-form "LLC"
  :lei "5493005JTEV4OVDVNH32"
  :registration-number "3600054"
  :address "838 Walker Road, Suite 21-2, Dover, DE"
  :as @agi_us)

;; Japan subsidiary
(entity.upsert-company
  :name "Allianz Global Investors Japan Co., Ltd."
  :jurisdiction "JP"
  :legal-form "KK"
  :lei "353800NVWWGOB9JXQZ47"
  :registration-number "0104-01-053740"
  :as @agi_jp)

;; =============================================================================
;; PHASE 4: OWNERSHIP RELATIONSHIPS
;; =============================================================================

;; Allianz SE owns AGI GmbH (100%)
(ownership.create
  :owner-entity-id @allianz_se
  :owned-entity-id @agi_gmbh
  :percentage 100.0
  :ownership-type "DIRECT"
  :is-controlling true)

;; AGI GmbH owns regional entities
(ownership.create
  :owner-entity-id @agi_gmbh
  :owned-entity-id @agi_ch
  :percentage 100.0
  :ownership-type "DIRECT")

(ownership.create
  :owner-entity-id @agi_gmbh
  :owned-entity-id @agi_uk
  :percentage 100.0
  :ownership-type "DIRECT")

(ownership.create
  :owner-entity-id @agi_gmbh
  :owned-entity-id @agi_ie
  :percentage 100.0
  :ownership-type "DIRECT")

(ownership.create
  :owner-entity-id @agi_gmbh
  :owned-entity-id @agi_us
  :percentage 100.0
  :ownership-type "DIRECT")

(ownership.create
  :owner-entity-id @agi_gmbh
  :owned-entity-id @agi_jp
  :percentage 100.0
  :ownership-type "DIRECT")

;; =============================================================================
;; PHASE 5: CREATE CBU (Client Business Unit)
;; =============================================================================

(cbu.create
  :name "Allianz Global Investors"
  :jurisdiction "DE"
  :cbu-type "ASSET_MANAGER"
  :commercial-client-entity-id @agi_gmbh
  :as @cbu_agi)

;; =============================================================================
;; PHASE 6: CBU ROLE ASSIGNMENTS
;; =============================================================================

;; Investment Manager (the main relationship)
(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @agi_gmbh
  :role "INVESTMENT_MANAGER")

;; Regional ManCos
(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @agi_ch
  :role "MANAGEMENT_COMPANY"
  :jurisdiction "CH")

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @agi_uk
  :role "MANAGEMENT_COMPANY"
  :jurisdiction "GB")

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @agi_ie
  :role "MANAGEMENT_COMPANY"
  :jurisdiction "IE")

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @agi_lu_branch
  :role "MANAGEMENT_COMPANY"
  :jurisdiction "LU")

;; Ultimate Parent (for UBO display)
(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_se
  :role "ULTIMATE_PARENT")

;; =============================================================================
;; PHASE 7: SAMPLE FUNDS (Luxembourg SICAV structure)
;; Full fund load via separate bulk script
;; =============================================================================

;; Umbrella Fund
(entity.upsert-fund
  :name "Allianz Global Investors Fund"
  :jurisdiction "LU"
  :fund-type "SICAV"
  :regulatory-status "UCITS"
  :regulator "CSSF"
  :management-company-id @agi_lu_branch
  :as @agi_sicav_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @agi_sicav_lu
  :role "UMBRELLA_FUND")

;; Sample sub-funds (first 10 from LU)
(entity.upsert-fund
  :name "Allianz Money Market US $"
  :jurisdiction "LU"
  :fund-type "SUB_FUND"
  :parent-fund-id @agi_sicav_lu
  :sfdr-category "ARTICLE_8"
  :asset-class "FIXED_INCOME"
  :as @fund_mm_usd)

(entity.upsert-fund
  :name "Allianz US High Yield"
  :jurisdiction "LU"
  :fund-type "SUB_FUND"
  :parent-fund-id @agi_sicav_lu
  :sfdr-category "ARTICLE_6"
  :asset-class "FIXED_INCOME"
  :as @fund_us_hy)

(entity.upsert-fund
  :name "Allianz Global Artificial Intelligence"
  :jurisdiction "LU"
  :fund-type "SUB_FUND"
  :parent-fund-id @agi_sicav_lu
  :sfdr-category "ARTICLE_8"
  :asset-class "EQUITY"
  :as @fund_ai)

(entity.upsert-fund
  :name "Allianz Emerging Markets Equity"
  :jurisdiction "LU"
  :fund-type "SUB_FUND"
  :parent-fund-id @agi_sicav_lu
  :sfdr-category "ARTICLE_8"
  :asset-class "EQUITY"
  :as @fund_em_eq)

(entity.upsert-fund
  :name "Allianz Europe Equity Growth"
  :jurisdiction "LU"
  :fund-type "SUB_FUND"
  :parent-fund-id @agi_sicav_lu
  :sfdr-category "ARTICLE_8"
  :asset-class "EQUITY"
  :as @fund_eu_growth)

;; Assign sub-funds to CBU
(cbu.assign-role :cbu-id @cbu_agi :entity-id @fund_mm_usd :role "SUB_FUND")
(cbu.assign-role :cbu-id @cbu_agi :entity-id @fund_us_hy :role "SUB_FUND")
(cbu.assign-role :cbu-id @cbu_agi :entity-id @fund_ai :role "SUB_FUND")
(cbu.assign-role :cbu-id @cbu_agi :entity-id @fund_em_eq :role "SUB_FUND")
(cbu.assign-role :cbu-id @cbu_agi :entity-id @fund_eu_growth :role "SUB_FUND")

;; =============================================================================
;; END OF CORE STRUCTURE
;; =============================================================================
;; 
;; To load all 671 funds, run:
;;   cargo x load-allianz-funds
;;
;; This core script establishes:
;; - Ownership chain (Allianz SE → AGI GmbH → regional subs)
;; - CBU with role assignments
;; - Sample fund structure for visualization testing
;; =============================================================================
