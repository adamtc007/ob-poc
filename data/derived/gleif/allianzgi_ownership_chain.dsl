;; GLEIF-Verified Allianz Global Investors Ownership Structure
;; Generated: 2025-12-28T17:43:19.914029
;; Source: GLEIF API (api.gleif.org)
;; Relationship Type: IS_DIRECTLY_CONSOLIDATED_BY (accounting consolidation = 100%)

;; === ENTITIES ===

(entity.ensure-limited-company
    :name "Allianz SE"
    :lei "529900K9B0N5BT694847"
    :jurisdiction "DE"
    :registration-number "HRB 164232"
    :city "München"
    :as @529900k9)

(entity.ensure-limited-company
    :name "Allianz Global Investors GmbH"
    :lei "OJ2TIQSVQND4IZYYK658"
    :jurisdiction "DE"
    :registration-number "HRB 9340"
    :city "Frankfurt am Main"
    :as @oj2tiqsv)

;; === SUBSIDIARIES ===

(entity.ensure-limited-company
    :name "ALLIANZ CAPITAL PARTNERS OF AMERICA LLC"
    :lei "5493005JTEV4OVDVNH32"
    :jurisdiction "US-DE"
    :as @5493005j)

(entity.ensure-limited-company
    :name "アリアンツ・グローバル・インベスターズ・ジャパン株式会社"
    :lei "353800NVWWGOB9JXQZ47"
    :jurisdiction "JP"
    :as @353800nv)

;; === OWNERSHIP CHAIN ===
;; GLEIF reports 'IS_DIRECTLY_CONSOLIDATED_BY' = 100% accounting ownership

(cbu.role:assign-ownership
    :owner-entity-id @529900k9
    :owned-entity-id @oj2tiqsv
    :percentage 100.0
    :ownership-type "ACCOUNTING_CONSOLIDATION"
    :source "GLEIF"
    :corroboration "FULLY_CORROBORATED")

(cbu.role:assign-ownership
    :owner-entity-id @oj2tiqsv
    :owned-entity-id @5493005j
    :percentage 100.0
    :ownership-type "ACCOUNTING_CONSOLIDATION"
    :source "GLEIF"
    :corroboration "UNKNOWN")

(cbu.role:assign-ownership
    :owner-entity-id @oj2tiqsv
    :owned-entity-id @353800nv
    :percentage 100.0
    :ownership-type "ACCOUNTING_CONSOLIDATION"
    :source "GLEIF"
    :corroboration "UNKNOWN")

;; === UBO TERMINUS ===
;; Allianz SE is publicly traded with dispersed ownership

(cbu.role:mark-ubo-terminus
    :entity-id @529900k9
    :reason "NO_KNOWN_PERSON"
    :notes "GLEIF reporting exception - no consolidating parent")

;; === FUND MANAGEMENT (sample) ===
;; AllianzGI manages 300 funds registered in GLEIF

;; Fund: Allianz Asia Pacific Secured Lending Fund III S.A....
;; LEI: 529900LSFQ65EMQNBP87

;; Fund: Allianz Global Enhanced Equity Income...
;; LEI: 529900O2D7WTTP2ECM60

;; Fund: Allianz EuropEquity Crescendo...
;; LEI: 529900A4N4FMRF1QIT75

;; Fund: OCIRP Actions Multifacteurs...
;; LEI: 529900ZWZD2XKZ3GFO55

;; Fund: CAPVIVA Infrastructure...
;; LEI: 5299007D2Y764JWNW850
