;; ============================================================================
;; ALLIANZ GLEIF COMPLETE FUND DATA LOAD
;; Generated: 2026-01-01T11:30:44.248471+00:00
;; Source: GLEIF API (api.gleif.org)
;; Total funds: 417
;; Umbrella SICAVs: 29
;; ============================================================================

;; ============================================================================
;; PHASE 1: Parent Entities (Allianz SE → AllianzGI)
;; ============================================================================

;; Allianz SE
(entity.ensure-limited-company
    :name "Allianz SE"
    :lei "529900K9B0N5BT694847"
    :jurisdiction "DE"
    :registration-number "HRB 164232"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "SGST"
    :gleif-validation-level "FULLY_CORROBORATED"
    :parent-exception "NO_KNOWN_PERSON"
    :as @lei_529900k9b0n5bt694847)

;; Allianz Global Investors GmbH
(entity.ensure-limited-company
    :name "Allianz Global Investors GmbH"
    :lei "OJ2TIQSVQND4IZYYK658"
    :jurisdiction "DE"
    :registration-number "HRB 9340"
    :city "Frankfurt am Main"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :gleif-validation-level "FULLY_CORROBORATED"
    :direct-parent-lei "529900K9B0N5BT694847"
    :ultimate-parent-lei "529900K9B0N5BT694847"
    :as @lei_oj2tiqsvqnd4izyyk658)

;; Ownership: Allianz SE → AllianzGI
(ubo.add-ownership
    :owner-entity-id @lei_529900k9b0n5bt694847
    :owned-entity-id @lei_oj2tiqsvqnd4izyyk658
    :percentage 100.0
    :ownership-type "DIRECT")

;; ============================================================================
;; PHASE 2: Umbrella SICAV Entities (26 umbrellas)
;; MUST be created before sub-funds reference them via SICAV role
;; ============================================================================

;; Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF"
    :lei "529900LSFQ65EMQNBP87"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Luxembourg"
    :as @lei_529900lsfq65emqnbp87)

;; Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF"
    :lei "529900KP3L513IRKR804"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Luxembourg"
    :as @lei_529900kp3l513irkr804)

;; Allianz Private Credit French Fund
(entity.ensure-limited-company
    :name "Allianz Private Credit French Fund"
    :lei "529900UVAZHC35LULZ81"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900uvazhc35lulz81)

;; Allianz Asia Pacific Infrastructure Credit Fund S.A., SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Asia Pacific Infrastructure Credit Fund S.A., SICAV-RAIF"
    :lei "529900BGQ1UJWROM5949"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900bgq1ujwrom5949)

;; Allianz Credit Emerging Markets Fund S.A., SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Credit Emerging Markets Fund S.A., SICAV-RAIF"
    :lei "52990031JHQ1YL8OHB45"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_52990031jhq1yl8ohb45)

;; Allianz AlTi SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz AlTi SCSp, SICAV-RAIF"
    :lei "529900NCY53I2UY5QV52"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900ncy53i2uy5qv52)

;; Allianz Infrastructure Credit Opportunities Fund II SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Infrastructure Credit Opportunities Fund II SCSp, SICAV-RAIF"
    :lei "529900NCP5DEVQ6EOK97"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "U8KA"
    :city "Luxembourg"
    :as @lei_529900ncp5devq6eok97)

;; ALLIANZ SELECTION
(entity.ensure-limited-company
    :name "ALLIANZ SELECTION"
    :lei "529900B2KZCGBFOBMG71"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900b2kzcgbfobmg71)

;; ALLIANZ EPARGNE RETRAITE
(entity.ensure-limited-company
    :name "ALLIANZ EPARGNE RETRAITE"
    :lei "529900X8DCCDLCNIPF48"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900x8dccdlcnipf48)

;; Allianz Trade Finance Funds S.A., SICAV-RAIF - ALLIANZ WORKING CAPITAL INVOICE FINANCE FUND
(entity.ensure-limited-company
    :name "Allianz Trade Finance Funds S.A., SICAV-RAIF - ALLIANZ WORKING CAPITAL INVOICE FINANCE FUND"
    :lei "52990088A09WYIQR6770"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_52990088a09wyiqr6770)

;; Allianz Impact Private Credit S.A., SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Impact Private Credit S.A., SICAV-RAIF"
    :lei "5299006VQYOURKK6RR02"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_5299006vqyourkk6rr02)

;; Allianz Global Diversified Private Debt Fund II SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Diversified Private Debt Fund II SCSp, SICAV-RAIF"
    :lei "529900YK8Y4SJ860CY04"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900yk8y4sj860cy04)

;; ALLIANZ SENIOR EUROPEAN INFRASTRUCTURE DEBT FUND, SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "ALLIANZ SENIOR EUROPEAN INFRASTRUCTURE DEBT FUND, SCSp, SICAV-RAIF"
    :lei "529900NTZS8R02EFYZ08"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900ntzs8r02efyz08)

;; Allianz Private Markets Solutions Fund S.A. SICAV-RAIF - Allianz Core Private Markets Fund
(entity.ensure-limited-company
    :name "Allianz Private Markets Solutions Fund S.A. SICAV-RAIF - Allianz Core Private Markets Fund"
    :lei "529900ZQ9E9V5M7I1291"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900zq9e9v5m7i1291)

;; Allianz Global Diversified Infrastructure and Energy Transition Debt Fund SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Diversified Infrastructure and Energy Transition Debt Fund SCSp, SICAV-RAIF"
    :lei "529900GR9M1XZOHTWI24"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900gr9m1xzohtwi24)

;; ALLIANZ US AND ASIA-PACIFIC REAL ESTATE DEBT OPPORTUNITIES FUND SCSP, SICAV-RAIF
(entity.ensure-limited-company
    :name "ALLIANZ US AND ASIA-PACIFIC REAL ESTATE DEBT OPPORTUNITIES FUND SCSP, SICAV-RAIF"
    :lei "52990052N8HOO6F7GQ66"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_52990052n8hoo6f7gq66)

;; Allianz Global Real Estate Debt Opportunities Feeder Fund SA, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Real Estate Debt Opportunities Feeder Fund SA, SICAV-RAIF"
    :lei "529900WXDNKXPO62H858"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900wxdnkxpo62h858)

;; Allianz Global Private Debt Opportunities Fund SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Private Debt Opportunities Fund SCSp, SICAV-RAIF"
    :lei "529900HFS1UW1F1XUC79"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900hfs1uw1f1xuc79)

;; Allianz Global Private Debt Opportunities Feeder Fund SA, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Private Debt Opportunities Feeder Fund SA, SICAV-RAIF"
    :lei "529900DIM2CXR3LKNV85"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Frankfurt am Main"
    :as @lei_529900dim2cxr3lknv85)

;; Allianz Global Real Estate Debt Opportunities Fund SCSp, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Real Estate Debt Opportunities Fund SCSp, SICAV-RAIF"
    :lei "529900ANUC6HREM95723"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900anuc6hrem95723)

;; Allianz FLG Private Debt Fund SA, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz FLG Private Debt Fund SA, SICAV-RAIF"
    :lei "5299009DG2FFOGNZ5Y64"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299009dg2ffognz5y64)

;; Emerging Market Climate Action Fund, SCSp SICAV-RAIF
(entity.ensure-limited-company
    :name "Emerging Market Climate Action Fund, SCSp SICAV-RAIF"
    :lei "529900SYYMQ0VR4EPR77"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Frankfurt am Main"
    :as @lei_529900syymq0vr4epr77)

;; Allianz Resilient Opportunistic Credit Feeder Fund SA, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Resilient Opportunistic Credit Feeder Fund SA, SICAV-RAIF"
    :lei "529900GSAXKVEAS64X94"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900gsaxkveas64x94)

;; Allianz Global Diversified Private Debt Feeder Fund SA, SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Diversified Private Debt Feeder Fund SA, SICAV-RAIF"
    :lei "5299009XV3OYH5SCBN33"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299009xv3oyh5scbn33)

;; Allianz Impact Investment Fund, S.A. SICAV-RAIF - Allianz Impact Investment Fund Compartment I
(entity.ensure-limited-company
    :name "Allianz Impact Investment Fund, S.A. SICAV-RAIF - Allianz Impact Investment Fund Compartment I"
    :lei "529900X36I1JQX58QI40"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900x36i1jqx58qi40)

;; Allianz Global Investors Fund
(entity.ensure-limited-company
    :name "Allianz Global Investors Fund"
    :lei "4KT8DCRLAREP7C35MW05"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_4kt8dcrlarep7c35mw05)

;; ============================================================================
;; PHASE 2.5: External Umbrella Entities (6 external)
;; These umbrellas are referenced by sub-funds but not in our fund list
;; Creating placeholder entities so SICAV role can reference them
;; ============================================================================

;; External umbrella: ALLIANZ MULTI STRATEGIES FUND
(entity.ensure-limited-company
    :name "ALLIANZ MULTI STRATEGIES FUND"
    :lei "213800MTIDAJ6BMV8R40"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_213800mtidaj6bmv8r40)

;; External umbrella: Allianz Allvest Invest SICAV-SIF
(entity.ensure-limited-company
    :name "Allianz Allvest Invest SICAV-SIF"
    :lei "2549002S5ROHUYGFVT38"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_2549002s5rohuygfvt38)

;; External umbrella: Allianz European Pension Investments
(entity.ensure-limited-company
    :name "Allianz European Pension Investments"
    :lei "5299000S45HP7B90ZB16"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_5299000s45hp7b90zb16)

;; External umbrella: Allianz Global Real Assets and Private Markets Fund S.A. SICAV-RAIF
(entity.ensure-limited-company
    :name "Allianz Global Real Assets and Private Markets Fund S.A. SICAV-RAIF"
    :lei "5299001KBL1HP5T3TJ23"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_5299001kbl1hp5t3tj23)

;; External umbrella: ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF
(entity.ensure-limited-company
    :name "ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF"
    :lei "5299008FHADWURGMXP80"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_5299008fhadwurgmxp80)

;; External umbrella: Allianz ELTIF Umbrella SCA SICAV
(entity.ensure-limited-company
    :name "Allianz ELTIF Umbrella SCA SICAV"
    :lei "529900J65RDOCLJQUC44"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_529900j65rdocljquc44)

;; ============================================================================
;; PHASE 3: Sub-Fund and Standalone Fund Entities (391)
;; ============================================================================

;; Allianz Global Enhanced Equity Income
(entity.ensure-limited-company
    :name "Allianz Global Enhanced Equity Income"
    :lei "529900O2D7WTTP2ECM60"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900o2d7wttp2ecm60)

;; Allianz EuropEquity Crescendo
(entity.ensure-limited-company
    :name "Allianz EuropEquity Crescendo"
    :lei "529900A4N4FMRF1QIT75"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900a4n4fmrf1qit75)

;; OCIRP Actions Multifacteurs
(entity.ensure-limited-company
    :name "OCIRP Actions Multifacteurs"
    :lei "529900ZWZD2XKZ3GFO55"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900zwzd2xkz3gfo55)

;; CAPVIVA Infrastructure
(entity.ensure-limited-company
    :name "CAPVIVA Infrastructure"
    :lei "5299007D2Y764JWNW850"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "U8KA"
    :city "Senningerberg"
    :as @lei_5299007d2y764jwnw850)

;; Allianz European Autonomy
(entity.ensure-limited-company
    :name "Allianz European Autonomy"
    :lei "529900E5TEG9CGU33298"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900e5teg9cgu33298)

;; Aktien Dividende Global
(entity.ensure-limited-company
    :name "Aktien Dividende Global"
    :lei "529900NSPM728J89YR40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900nspm728j89yr40)

;; Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)
(entity.ensure-limited-company
    :name "Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)"
    :lei "529900UD1FMOKR9RN344"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ud1fmokr9rn344)

;; Allianz Best Styles US Small Cap Equity
(entity.ensure-limited-company
    :name "Allianz Best Styles US Small Cap Equity"
    :lei "5299008BUZ4793QZRD79"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_5299008buz4793qzrd79)

;; EPC III Compartment
(entity.ensure-limited-company
    :name "EPC III Compartment"
    :lei "5299006DHNN51CVBHF80"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_5299006dhnn51cvbhf80)

;; Private Debt Co-Investments
(entity.ensure-limited-company
    :name "Private Debt Co-Investments"
    :lei "529900LSI5WOY3J9UM22"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Luxembourg"
    :as @lei_529900lsi5woy3j9um22)

;; Private Debt Secondaries
(entity.ensure-limited-company
    :name "Private Debt Secondaries"
    :lei "5299004Q6C8TKHNT0V91"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Luxembourg"
    :as @lei_5299004q6c8tkhnt0v91)

;; Allianz Global Infrastructure
(entity.ensure-limited-company
    :name "Allianz Global Infrastructure"
    :lei "529900FVQR2VCW730O08"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900fvqr2vcw730o08)

;; Allianz Global Diversified Private Debt II EUR Feeder Fund (Germany)
(entity.ensure-limited-company
    :name "Allianz Global Diversified Private Debt II EUR Feeder Fund (Germany)"
    :lei "5299001LCV2L4TQTYD24"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299001lcv2l4tqtyd24)

;; Allianz Impact Private Credit Dedicated Holding SCSp
(entity.ensure-limited-company
    :name "Allianz Impact Private Credit Dedicated Holding SCSp"
    :lei "5299004LFEORXS4M8I86"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "U8KA"
    :city "Senningerberg"
    :as @lei_5299004lfeorxs4m8i86)

;; Allianz Target Maturity Euro Bond V
(entity.ensure-limited-company
    :name "Allianz Target Maturity Euro Bond V"
    :lei "529900VEBFBDBXFW0O55"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900vebfbdbxfw0o55)

;; ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF - EIS
(entity.ensure-limited-company
    :name "ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF - EIS"
    :lei "529900VD1WF5GYWVK357"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Luxembourg"
    :as @lei_529900vd1wf5gywvk357)

;; ALLIANZ EPARGNE OBLIGATIONS EURO
(entity.ensure-limited-company
    :name "ALLIANZ EPARGNE OBLIGATIONS EURO"
    :lei "529900VEL50YOZJVDO67"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900vel50yozjvdo67)

;; ALLIANZ EPARGNE ACTIONS FRANCE
(entity.ensure-limited-company
    :name "ALLIANZ EPARGNE ACTIONS FRANCE"
    :lei "529900X88M8AG6ECK323"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900x88m8ag6eck323)

;; Allianz Target Maturity Euro Bond IV
(entity.ensure-limited-company
    :name "Allianz Target Maturity Euro Bond IV"
    :lei "529900LIFZM3ONFVC719"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_529900lifzm3onfvc719)

;; ALLIANZ EPARGNE ACTIONS MONDE
(entity.ensure-limited-company
    :name "ALLIANZ EPARGNE ACTIONS MONDE"
    :lei "529900RME4YZO5NPPO40"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900rme4yzo5nppo40)

;; ALLIANZ EPARGNE ACTIONS SOLIDAIRE
(entity.ensure-limited-company
    :name "ALLIANZ EPARGNE ACTIONS SOLIDAIRE"
    :lei "529900PQAW8FFYAB8344"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900pqaw8ffyab8344)

;; ALLIANZ EPARGNE DIVERSIFIE
(entity.ensure-limited-company
    :name "ALLIANZ EPARGNE DIVERSIFIE"
    :lei "529900ERI9BH5YPF3105"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900eri9bh5ypf3105)

;; Allianz Global Infrastucture ELTIF
(entity.ensure-limited-company
    :name "Allianz Global Infrastucture ELTIF"
    :lei "5299002QOGYF6UNRLJ60"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_5299002qogyf6unrlj60)

;; ALLIANZ RETRAITE SELECTION ACTIONS
(entity.ensure-limited-company
    :name "ALLIANZ RETRAITE SELECTION ACTIONS"
    :lei "52990050OZDQ0H80HC61"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_52990050ozdq0h80hc61)

;; ALLIANZ RETRAITE SELECTION OBLIGATIONS
(entity.ensure-limited-company
    :name "ALLIANZ RETRAITE SELECTION OBLIGATIONS"
    :lei "529900GCDE0IQU4ELP86"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900gcde0iqu4elp86)

;; Allianz Private Debt Secondary EUR Feeder Fund I (Germany)
(entity.ensure-limited-company
    :name "Allianz Private Debt Secondary EUR Feeder Fund I (Germany)"
    :lei "529900LSWCTMKMF7V008"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900lswctmkmf7v008)

;; Allianz Impact Private Credit Dedicated Fund
(entity.ensure-limited-company
    :name "Allianz Impact Private Credit Dedicated Fund"
    :lei "529900ZVN7CXF7H4UP44"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900zvn7cxf7h4up44)

;; AllianzGI-Fonds MRPF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MRPF"
    :lei "529900DEJWPIKN90P361"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900dejwpikn90p361)

;; ALLIANZ GLOBAL DIVERSIFIED INFRASTRUCTURE AND ENERGY TRANSITION DEBT FEEDER FUND (GERMANY)
(entity.ensure-limited-company
    :name "ALLIANZ GLOBAL DIVERSIFIED INFRASTRUCTURE AND ENERGY TRANSITION DEBT FEEDER FUND (GERMANY)"
    :lei "529900MN4PZEXO6J5Z18"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900mn4pzexo6j5z18)

;; Allianz Emerging Europe Equity 2
(entity.ensure-limited-company
    :name "Allianz Emerging Europe Equity 2"
    :lei "529900P7QI4AHSNI2026"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900p7qi4ahsni2026)

;; Allianz Premium Champions
(entity.ensure-limited-company
    :name "Allianz Premium Champions"
    :lei "5299002YFR7XDEA6EC13"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299002yfr7xdea6ec13)

;; Allianz Target Maturity Euro Bond III
(entity.ensure-limited-company
    :name "Allianz Target Maturity Euro Bond III"
    :lei "529900YTC9IHB6RCQ908"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900ytc9ihb6rcq908)

;; Allianz Social Conviction Equity
(entity.ensure-limited-company
    :name "Allianz Social Conviction Equity"
    :lei "529900RICS54KA4ZV927"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900rics54ka4zv927)

;; AllianzGI-Fonds BSVF.23
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BSVF.23"
    :lei "529900PFZXNDVEQKAL97"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900pfzxndveqkal97)

;; AllianzGI-Fonds BTH
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BTH"
    :lei "529900OH76PKT5ON8J74"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900oh76pkt5on8j74)

;; AllianzGI-Fonds VTH
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VTH"
    :lei "529900K5L0CMTXH0Q370"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900k5l0cmtxh0q370)

;; Allianz US Large Cap Value
(entity.ensure-limited-company
    :name "Allianz US Large Cap Value"
    :lei "529900PCEKY03SO2GS40"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900pceky03so2gs40)

;; Allianz AZSE Master Funds
(entity.ensure-limited-company
    :name "Allianz AZSE Master Funds"
    :lei "529900TXF4SCPCT4VX78"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900txf4scpct4vx78)

;; AllianzGI-Fonds Transformation SAG
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Transformation SAG"
    :lei "529900P666S0I9B8R306"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900p666s0i9b8r306)

;; AllianzGI-Fonds Pure Steel +
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Pure Steel +"
    :lei "529900NO8488WBF8SO93"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900no8488wbf8so93)

;; Allianz Target Maturity Euro Bond I
(entity.ensure-limited-company
    :name "Allianz Target Maturity Euro Bond I"
    :lei "529900AGQHNHIYLXHR90"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900agqhnhiylxhr90)

;; Allianz US Investment Grade Credit
(entity.ensure-limited-company
    :name "Allianz US Investment Grade Credit"
    :lei "529900XTY5ODOFJVZ671"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900xty5odofjvz671)

;; Allianz PVK Fonds
(entity.ensure-limited-company
    :name "Allianz PVK Fonds"
    :lei "52990060XVMDOT4S0102"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990060xvmdot4s0102)

;; Sirius
(entity.ensure-limited-company
    :name "Sirius"
    :lei "529900UFESZW3KH1NZ15"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900ufeszw3kh1nz15)

;; Kennedy 1
(entity.ensure-limited-company
    :name "Kennedy 1"
    :lei "529900CJN5QOC67HDN27"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Senningerberg"
    :as @lei_529900cjn5qoc67hdn27)

;; Allianz VG EU PD
(entity.ensure-limited-company
    :name "Allianz VG EU PD"
    :lei "529900XUR86NWW7N7457"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Luxembourg"
    :as @lei_529900xur86nww7n7457)

;; AllianzGI-Fonds VVV
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VVV"
    :lei "529900DRY37S2WJKM906"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900dry37s2wjkm906)

;; Allianz Strategic Bond Conservative
(entity.ensure-limited-company
    :name "Allianz Strategic Bond Conservative"
    :lei "529900LSH98HDOEN1P39"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900lsh98hdoen1p39)

;; ALLIANZ SYSTEMATIC ENHANCED US EQUITY
(entity.ensure-limited-company
    :name "ALLIANZ SYSTEMATIC ENHANCED US EQUITY"
    :lei "5299002WOGP7C2R2FD60"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299002wogp7c2r2fd60)

;; Airbus Aerostructures Dachfonds
(entity.ensure-limited-company
    :name "Airbus Aerostructures Dachfonds"
    :lei "52990037LRSWC0FZP452"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990037lrswc0fzp452)

;; AllianzGI-Fonds SFCBUSD3
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFCBUSD3"
    :lei "529900RAYSEMMI7DKY56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900raysemmi7dky56)

;; AllianzGI-Fonds SFCBUSD2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFCBUSD2"
    :lei "529900EHX48497PH0236"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ehx48497ph0236)

;; AllianzGI-Fonds SFCBUSD1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFCBUSD1"
    :lei "529900DV1C0TVB1HSB06"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900dv1c0tvb1hsb06)

;; AllianzGI-Fonds SFCBEUR1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFCBEUR1"
    :lei "5299008VH2NKK5AEX340"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299008vh2nkk5aex340)

;; AllianzGI-Fonds SFCBEUR2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFCBEUR2"
    :lei "529900BTU9R0JYWFI949"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900btu9r0jywfi949)

;; AllianzGI-Fonds SFEQEUR1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFEQEUR1"
    :lei "5299002KL9879OCDNQ53"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002kl9879ocdnq53)

;; AllianzGI-Fonds SFCBEUR3
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFCBEUR3"
    :lei "5299002N1PKHMDSYCQ81"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002n1pkhmdsycq81)

;; ALLIANZ PATRIMONIAL DIVERSIFIE
(entity.ensure-limited-company
    :name "ALLIANZ PATRIMONIAL DIVERSIFIE"
    :lei "529900G9N3ECMGVAVV43"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900g9n3ecmgvavv43)

;; AllianzGI-Fonds AOKNW-AR
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AOKNW-AR"
    :lei "5299004O4I1HZ9YUD990"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004o4i1hz9yud990)

;; Airbus Invest for Life Rentenfonds APP
(entity.ensure-limited-company
    :name "Airbus Invest for Life Rentenfonds APP"
    :lei "529900266FN01AARNM32"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900266fn01aarnm32)

;; AllianzGI-Fonds CEC
(entity.ensure-limited-company
    :name "AllianzGI-Fonds CEC"
    :lei "529900TGXD6FOEYS7708"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900tgxd6foeys7708)

;; ERAFP MULTI-ACTIFS 2
(entity.ensure-limited-company
    :name "ERAFP MULTI-ACTIFS 2"
    :lei "529900VKKUHVZPC9OZ90"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900vkkuhvzpc9oz90)

;; Allianz Europe Equity powered by Artificial Intelligence
(entity.ensure-limited-company
    :name "Allianz Europe Equity powered by Artificial Intelligence"
    :lei "529900JYTQGOA1FWYI63"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900jytqgoa1fwyi63)

;; Allianz US Equity powered by Artificial Intelligence
(entity.ensure-limited-company
    :name "Allianz US Equity powered by Artificial Intelligence"
    :lei "5299003LZK5DIE22V897"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299003lzk5die22v897)

;; Allianz Global Equity powered by Artificial Intelligence
(entity.ensure-limited-company
    :name "Allianz Global Equity powered by Artificial Intelligence"
    :lei "5299008HUU0V6AO0KZ44"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299008huu0v6ao0kz44)

;; money mate defensiv
(entity.ensure-limited-company
    :name "money mate defensiv"
    :lei "5299004LACFVE3A9UE80"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_5299004lacfve3a9ue80)

;; money mate mutig
(entity.ensure-limited-company
    :name "money mate mutig"
    :lei "5299000EFUT8CU0S4G03"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_5299000efut8cu0s4g03)

;; ALLIANZ GLOBAL HYBRID SECURITIES FUND
(entity.ensure-limited-company
    :name "ALLIANZ GLOBAL HYBRID SECURITIES FUND"
    :lei "213800REX1UXMZV4PL75"
    :jurisdiction "KY"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "GEORGE TOWN"
    :as @lei_213800rex1uxmzv4pl75)

;; AllianzGI-Fonds Ernest
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Ernest"
    :lei "5299005CPQPC0WW76836"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299005cpqpc0ww76836)

;; ACTINIUM
(entity.ensure-limited-company
    :name "ACTINIUM"
    :lei "529900WCJYHZYIB46M51"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900wcjyhzyib46m51)

;; Allianz France Relance
(entity.ensure-limited-company
    :name "Allianz France Relance"
    :lei "529900APVUKIHETVAN54"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900apvukihetvan54)

;; Allianz Dynamic Allocation Plus Equity
(entity.ensure-limited-company
    :name "Allianz Dynamic Allocation Plus Equity"
    :lei "529900R0F5AODZYTEH16"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900r0f5aodzyteh16)

;; Allianz Trend and Brands
(entity.ensure-limited-company
    :name "Allianz Trend and Brands"
    :lei "529900A66ACGUDUEFO55"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900a66acguduefo55)

;; AllianzGI-Fonds SDK Pensionen
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SDK Pensionen"
    :lei "52990069HD3AA39FM024"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990069hd3aa39fm024)

;; AllianzGI-Fonds Surprise
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Surprise"
    :lei "5299007RUMWKDVJRSV24"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299007rumwkdvjrsv24)

;; Allianz Advanced Fixed Income Euro Aggregate
(entity.ensure-limited-company
    :name "Allianz Advanced Fixed Income Euro Aggregate"
    :lei "5299002BK0GL27V7RO88"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_5299002bk0gl27v7ro88)

;; AllianzGI-Fonds THV RG
(entity.ensure-limited-company
    :name "AllianzGI-Fonds THV RG"
    :lei "5299001Z1S34EVX7QV51"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299001z1s34evx7qv51)

;; AllianzGI-Fonds Scout24
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Scout24"
    :lei "529900QBHFV9PIBE2V65"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900qbhfv9pibe2v65)

;; AllianzGI-Fonds Selective Ownership 2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Selective Ownership 2"
    :lei "52990081G1E8D6EKCJ14"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990081g1e8d6ekcj14)

;; Allvest Active Invest
(entity.ensure-limited-company
    :name "Allvest Active Invest"
    :lei "549300VFU8KQQT7OXT28"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_549300vfu8kqqt7oxt28)

;; Allvest Passive Invest
(entity.ensure-limited-company
    :name "Allvest Passive Invest"
    :lei "5493009K3LX6KM2X7P46"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Luxembourg"
    :as @lei_5493009k3lx6km2x7p46)

;; AllianzGI-Fonds OLB Pensionen
(entity.ensure-limited-company
    :name "AllianzGI-Fonds OLB Pensionen"
    :lei "52990025L9DP1WMJIC93"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990025l9dp1wmjic93)

;; Allianz ADAC AV Fonds
(entity.ensure-limited-company
    :name "Allianz ADAC AV Fonds"
    :lei "529900RL1BE88XT0Y715"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900rl1be88xt0y715)

;; AllianzGI-Fonds STG-Pensions
(entity.ensure-limited-company
    :name "AllianzGI-Fonds STG-Pensions"
    :lei "529900W707JZW77SS292"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900w707jzw77ss292)

;; Allianz Impact Green Bond
(entity.ensure-limited-company
    :name "Allianz Impact Green Bond"
    :lei "52990099KFO3IMCLYJ15"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_52990099kfo3imclyj15)

;; AllianzGI-Fonds DID
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DID"
    :lei "529900XE8NGII96Y4811"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xe8ngii96y4811)

;; AllianzGI-S Aktien
(entity.ensure-limited-company
    :name "AllianzGI-S Aktien"
    :lei "529900SNAJQRQJUWMI06"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900snajqrqjuwmi06)

;; AllianzGI-S Anleihen IG
(entity.ensure-limited-company
    :name "AllianzGI-S Anleihen IG"
    :lei "529900Q3QYNHX3S3NV90"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900q3qynhx3s3nv90)

;; AllianzGI-Fonds VO93 E+H
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VO93 E+H"
    :lei "529900GDCKE823V5GE96"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900gdcke823v5ge96)

;; Allianz SE Ashmore Emerging Markets Corporates Fund
(entity.ensure-limited-company
    :name "Allianz SE Ashmore Emerging Markets Corporates Fund"
    :lei "529900GKUNMXQ7K4C094"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900gkunmxq7k4c094)

;; Allianz Pet and Animal Wellbeing
(entity.ensure-limited-company
    :name "Allianz Pet and Animal Wellbeing"
    :lei "529900WR8ULGAFRMOS18"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900wr8ulgafrmos18)

;; AllianzGI-Fonds Ex Euro Corporate Bonds
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Ex Euro Corporate Bonds"
    :lei "5299001KV55PH739CP54"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299001kv55ph739cp54)

;; Allianz German Small and Micro Cap
(entity.ensure-limited-company
    :name "Allianz German Small and Micro Cap"
    :lei "5299003TRG3ZTP6MA754"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5299003trg3ztp6ma754)

;; Pfizer Sérénité
(entity.ensure-limited-company
    :name "Pfizer Sérénité"
    :lei "529900ZJ9BHLF4CPOM48"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900zj9bhlf4cpom48)

;; 117 EURO ST
(entity.ensure-limited-company
    :name "117 EURO ST"
    :lei "529900GSOG8EFKZZ4691"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900gsog8efkzz4691)

;; AllianzGI-Fonds PencAbbV Pensions
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PencAbbV Pensions"
    :lei "529900C177B10RZK1Y35"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900c177b10rzk1y35)

;; AllianzGI-Fonds Luna A
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Luna A"
    :lei "52990044P8FDV0MHZL09"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990044p8fdv0mhzl09)

;; AllianzGI-Fonds Luna B
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Luna B"
    :lei "529900SOHAUP55S8PU50"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900sohaup55s8pu50)

;; AllianzGI-Fonds DEAL
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DEAL"
    :lei "52990015BZVBF05JWN74"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990015bzvbf05jwn74)

;; AllianzGI-Fonds OB Pension
(entity.ensure-limited-company
    :name "AllianzGI-Fonds OB Pension"
    :lei "529900FI2F4FBT09D708"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900fi2f4fbt09d708)

;; Allianz VIE Multi-Assets
(entity.ensure-limited-company
    :name "Allianz VIE Multi-Assets"
    :lei "529900K3ONY5LWZOHA27"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900k3ony5lwzoha27)

;; Airbus Invest for Life Aktienfonds 2
(entity.ensure-limited-company
    :name "Airbus Invest for Life Aktienfonds 2"
    :lei "529900WGPUIJHVK37680"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900wgpuijhvk37680)

;; AllianzGI-Fonds Selective Ownership
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Selective Ownership"
    :lei "5299004EO0JH52D3HP18"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004eo0jh52d3hp18)

;; AllianzGI-Fonds GEPAG
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GEPAG"
    :lei "529900GXQ1KDUMLK2763"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900gxq1kdumlk2763)

;; AllianzGI-Fonds Bremen
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Bremen"
    :lei "529900QROKYIF91OQ895"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900qrokyif91oq895)

;; AllianzGI-Fonds VE USA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VE USA"
    :lei "549300W4EA022BH9WI74"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300w4ea022bh9wi74)

;; AllianzGI-Fonds VE Global
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VE Global"
    :lei "549300J9TB79YDGM9643"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300j9tb79ydgm9643)

;; Allianz Thematica
(entity.ensure-limited-company
    :name "Allianz Thematica"
    :lei "5493004ZRV2CSS15YF05"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_5493004zrv2css15yf05)

;; AllianzGI-Fonds LBS SHH bAV
(entity.ensure-limited-company
    :name "AllianzGI-Fonds LBS SHH bAV"
    :lei "549300K6GRJ0776JII48"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300k6grj0776jii48)

;; AllianzGI-Fonds MPF1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MPF1"
    :lei "5493008E4UDYDEN16U62"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493008e4udyden16u62)

;; Ircantec Crédit Euro AGI
(entity.ensure-limited-company
    :name "Ircantec Crédit Euro AGI"
    :lei "549300KHM7O2ML2UUK42"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_549300khm7o2ml2uuk42)

;; AllianzGI-Fonds Beilstein-Institut
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Beilstein-Institut"
    :lei "549300SQ6EEPI62H7P52"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300sq6eepi62h7p52)

;; AllianzGI-Fonds RHM01
(entity.ensure-limited-company
    :name "AllianzGI-Fonds RHM01"
    :lei "54930068HUQPPWVUHT61"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930068huqppwvuht61)

;; VermögensManagement RenditeStars
(entity.ensure-limited-company
    :name "VermögensManagement RenditeStars"
    :lei "5493000GHH35FTFCM044"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_5493000ghh35ftfcm044)

;; AllianzGI-Fonds NASPA Pensionsfonds
(entity.ensure-limited-company
    :name "AllianzGI-Fonds NASPA Pensionsfonds"
    :lei "5493001B4YYB4RUTNV40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493001b4yyb4rutnv40)

;; Allianz V-PD Fonds
(entity.ensure-limited-company
    :name "Allianz V-PD Fonds"
    :lei "5493001L0CQ83S70CZ91"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493001l0cq83s70cz91)

;; Allianz SE-PD Fonds
(entity.ensure-limited-company
    :name "Allianz SE-PD Fonds"
    :lei "549300CVT30FX9P97463"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300cvt30fx9p97463)

;; Allianz PK-PD Fonds
(entity.ensure-limited-company
    :name "Allianz PK-PD Fonds"
    :lei "5493006GP001SQROD821"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493006gp001sqrod821)

;; AllianzGI-Fonds BRML
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BRML"
    :lei "549300AXU3HU4R5VE652"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300axu3hu4r5ve652)

;; Allianz L-PD Fonds
(entity.ensure-limited-company
    :name "Allianz L-PD Fonds"
    :lei "549300KG4RWKWUY6NT58"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300kg4rwkwuy6nt58)

;; Allianz PKV-PD Fonds
(entity.ensure-limited-company
    :name "Allianz PKV-PD Fonds"
    :lei "549300ZJFQIC44OI6T88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300zjfqic44oi6t88)

;; AllianzGI-Fonds ACH
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ACH"
    :lei "5493000OF8LYLLUS4J50"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493000of8lyllus4j50)

;; Allianz Vermögensbildung Deutschland
(entity.ensure-limited-company
    :name "Allianz Vermögensbildung Deutschland"
    :lei "549300HRNNEC3VBQW438"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300hrnnec3vbqw438)

;; Allianz Wachstum Euroland
(entity.ensure-limited-company
    :name "Allianz Wachstum Euroland"
    :lei "549300F0GR1N43BZW173"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300f0gr1n43bzw173)

;; Allianz Wachstum Europa
(entity.ensure-limited-company
    :name "Allianz Wachstum Europa"
    :lei "5493005N3WEXI56SI903"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493005n3wexi56si903)

;; Allianz Vermögensbildung Europa
(entity.ensure-limited-company
    :name "Allianz Vermögensbildung Europa"
    :lei "5493002EM7XLAPRUVZ50"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493002em7xlapruvz50)

;; Allianz Informationstechnologie
(entity.ensure-limited-company
    :name "Allianz Informationstechnologie"
    :lei "549300Y3AA9US1U3PT72"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300y3aa9us1u3pt72)

;; AllianzGI-Fonds Salzgitter
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Salzgitter"
    :lei "5493003ZUJSTL7EZ1U78"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493003zujstl7ez1u78)

;; AllianzGI-Fonds PKBMA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PKBMA"
    :lei "5493001NH47XGBTZW868"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493001nh47xgbtzw868)

;; AllianzGI-Fonds RP APNUS
(entity.ensure-limited-company
    :name "AllianzGI-Fonds RP APNUS"
    :lei "549300ZD2WJ3K48JYU88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300zd2wj3k48jyu88)

;; SK Themen
(entity.ensure-limited-company
    :name "SK Themen"
    :lei "222100JGDWHCU45IKT89"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_222100jgdwhcu45ikt89)

;; SK Welt
(entity.ensure-limited-company
    :name "SK Welt"
    :lei "2221001OQJE2EF46YL59"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_2221001oqje2ef46yl59)

;; SK Europa
(entity.ensure-limited-company
    :name "SK Europa"
    :lei "2221008LNLXD5G7Q2G15"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_2221008lnlxd5g7q2g15)

;; CKA Renten
(entity.ensure-limited-company
    :name "CKA Renten"
    :lei "54930058NZ8WP1LKON22"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930058nz8wp1lkon22)

;; Allianz Securicash SRI
(entity.ensure-limited-company
    :name "Allianz Securicash SRI"
    :lei "549300F44VV2MMKS9707"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_549300f44vv2mmks9707)

;; Allianz Euro Oblig Court Terme ISR
(entity.ensure-limited-company
    :name "Allianz Euro Oblig Court Terme ISR"
    :lei "549300PGXL5GTMG8PC85"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_549300pgxl5gtmg8pc85)

;; Allianz Europazins
(entity.ensure-limited-company
    :name "Allianz Europazins"
    :lei "549300PEJYHX4WA43I14"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300pejyhx4wa43i14)

;; Allianz Biotechnologie
(entity.ensure-limited-company
    :name "Allianz Biotechnologie"
    :lei "5493003A7RKULRCH1976"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493003a7rkulrch1976)

;; AllianzGI-Fonds MAF5
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF5"
    :lei "5299007LLQHDKJ8KBC33"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299007llqhdkj8kbc33)

;; AllianzGI PIMCO Euro Covered Bonds - gleitend 10 J.
(entity.ensure-limited-company
    :name "AllianzGI PIMCO Euro Covered Bonds - gleitend 10 J."
    :lei "52990059RKN0RQ20HO78"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990059rkn0rq20ho78)

;; AllianzGI-Fonds CPT2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds CPT2"
    :lei "529900CCI0WF7CWGXV82"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900cci0wf7cwgxv82)

;; AllianzGI-Fonds ABCO III
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ABCO III"
    :lei "529900DFJDFBAOPOIG76"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900dfjdfbaopoig76)

;; Allianz DLVR Fonds
(entity.ensure-limited-company
    :name "Allianz DLVR Fonds"
    :lei "5299005ERRLFDF1IWT25"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299005errlfdf1iwt25)

;; VermögensManagement Stars of Multi Asset
(entity.ensure-limited-company
    :name "VermögensManagement Stars of Multi Asset"
    :lei "529900IYHAOPPKT61430"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900iyhaoppkt61430)

;; AZRE AZD P&C Master Fund
(entity.ensure-limited-company
    :name "AZRE AZD P&C Master Fund"
    :lei "529900B5A2DWME31C402"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900b5a2dwme31c402)

;; AllianzGI-Fonds LANXESS Pension Trust 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds LANXESS Pension Trust 1"
    :lei "529900NP5H3TN7P1HD24"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900np5h3tn7p1hd24)

;; Allianz Re Asia PIMCO USD Fund
(entity.ensure-limited-company
    :name "Allianz Re Asia PIMCO USD Fund"
    :lei "529900UZNP8YDQEGYM98"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900uznp8ydqegym98)

;; ELA-Fonds
(entity.ensure-limited-company
    :name "ELA-Fonds"
    :lei "52990076KQ6RLYCUO006"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990076kq6rlycuo006)

;; HAMELIN MULTI-ACTIFS
(entity.ensure-limited-company
    :name "HAMELIN MULTI-ACTIFS"
    :lei "969500BQTWY38RZJCA39"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_969500bqtwy38rzjca39)

;; PFIZER - Pfizer Moyen Terme
(entity.ensure-limited-company
    :name "PFIZER - Pfizer Moyen Terme"
    :lei "529900DUOU39LFAI2U90"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900duou39lfai2u90)

;; Allianz Dynamic Commodities
(entity.ensure-limited-company
    :name "Allianz Dynamic Commodities"
    :lei "529900UBJTRBKW4W3M49"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900ubjtrbkw4w3m49)

;; AllianzGI-Fonds Degussa Trust e.V.
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Degussa Trust e.V."
    :lei "529900SC3JW2O3LHHW25"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900sc3jw2o3lhhw25)

;; CDI-Cofonds I
(entity.ensure-limited-company
    :name "CDI-Cofonds I"
    :lei "529900XMT6ZLWL059J27"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xmt6zlwl059j27)

;; AllianzGI-Fonds TOSCA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds TOSCA"
    :lei "529900WVD0MBZ1IN2017"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900wvd0mbz1in2017)

;; CDP-Cofonds
(entity.ensure-limited-company
    :name "CDP-Cofonds"
    :lei "529900AEQPNXUYG0AE43"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900aeqpnxuyg0ae43)

;; AllianzGI-Fonds GBG
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GBG"
    :lei "529900M8DWIWU75VRL48"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900m8dwiwu75vrl48)

;; AllianzGI-Fonds KUF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KUF"
    :lei "5299008IKQ5GE1PHSG06"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299008ikq5ge1phsg06)

;; AllianzGI-Fonds H-KGaA-Bonds
(entity.ensure-limited-company
    :name "AllianzGI-Fonds H-KGaA-Bonds"
    :lei "529900UXC9FIMP4SBY55"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900uxc9fimp4sby55)

;; Airbus ATZ Dachfonds
(entity.ensure-limited-company
    :name "Airbus ATZ Dachfonds"
    :lei "529900MOC271Q3LHRT16"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900moc271q3lhrt16)

;; AllianzGI-Fonds MAF-L
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF-L"
    :lei "529900XAZLOI53WS7A56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xazloi53ws7a56)

;; Allianz Innovation Souveraineté Européenne
(entity.ensure-limited-company
    :name "Allianz Innovation Souveraineté Européenne"
    :lei "5299005U1YHDK4D6RP66"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_5299005u1yhdk4d6rp66)

;; AllianzGI-Fonds VMN II
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VMN II"
    :lei "5299004R2I18YBBLE490"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004r2i18ybble490)

;; AllianzGI-Fonds DD I
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DD I"
    :lei "529900JCXD3V2BM9EV71"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900jcxd3v2bm9ev71)

;; AllianzGI-Fonds MSSD
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MSSD"
    :lei "529900JGBWDA5H318Y94"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900jgbwda5h318y94)

;; EURO-Cofonds
(entity.ensure-limited-company
    :name "EURO-Cofonds"
    :lei "529900J1R9K61CY95U65"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900j1r9k61cy95u65)

;; AllianzGI-Fonds AOKNW-BM
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AOKNW-BM"
    :lei "52990022H26JLQN68J60"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990022h26jlqn68j60)

;; AOK-Cofonds
(entity.ensure-limited-company
    :name "AOK-Cofonds"
    :lei "529900XP0RU01AZTO252"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xp0ru01azto252)

;; AllianzGI-Fonds pca-bau
(entity.ensure-limited-company
    :name "AllianzGI-Fonds pca-bau"
    :lei "529900OIFSYM6OAHAW73"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900oifsym6oahaw73)

;; AllianzGI-Fonds MAF2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF2"
    :lei "529900Z8P9H4YIAAKI23"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900z8p9h4yiaaki23)

;; Airbus SIKO Dachfonds
(entity.ensure-limited-company
    :name "Airbus SIKO Dachfonds"
    :lei "529900ZYLVV3XS8OFL49"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900zylvv3xs8ofl49)

;; AllianzGI-Fonds DIN
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DIN"
    :lei "52990026BC9272IWOG38"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990026bc9272iwog38)

;; AllianzGI-Fonds RBB
(entity.ensure-limited-company
    :name "AllianzGI-Fonds RBB"
    :lei "5299002QYV8DQ0O5F173"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002qyv8dq0o5f173)

;; AllianzGI-Fonds WAF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds WAF"
    :lei "529900KISVGC4SKD0492"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900kisvgc4skd0492)

;; Airbus Invest for Life Rentenfonds kurz
(entity.ensure-limited-company
    :name "Airbus Invest for Life Rentenfonds kurz"
    :lei "529900KE3UBJ5TWCFJ23"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ke3ubj5twcfj23)

;; Elbe Flugzeugwerke Dachfonds
(entity.ensure-limited-company
    :name "Elbe Flugzeugwerke Dachfonds"
    :lei "5299004NH57NXA6MQR45"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004nh57nxa6mqr45)

;; AllianzGI-Fonds Master DRT
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Master DRT"
    :lei "529900DSYPV8YLK7QL92"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900dsypv8ylk7ql92)

;; Allianz Strategy 50
(entity.ensure-limited-company
    :name "Allianz Strategy 50"
    :lei "529900U565TVTIRHJ104"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900u565tvtirhj104)

;; AllianzGI-E
(entity.ensure-limited-company
    :name "AllianzGI-E"
    :lei "5299000MJEBU65DJGX45"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299000mjebu65djgx45)

;; ALLIANZ STRATEGY 75
(entity.ensure-limited-company
    :name "ALLIANZ STRATEGY 75"
    :lei "529900589OY2G0CVOT53"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "UDY2"
    :city "Senningerberg"
    :as @lei_529900589oy2g0cvot53)

;; Airbus Helicopters Deutschland Dachfonds
(entity.ensure-limited-company
    :name "Airbus Helicopters Deutschland Dachfonds"
    :lei "529900RXR81SM6MDJI41"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900rxr81sm6mdji41)

;; Airbus Dachfonds
(entity.ensure-limited-company
    :name "Airbus Dachfonds"
    :lei "5299003TMLUS0SE2HG89"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299003tmlus0se2hg89)

;; Allianz PV-WS Fonds
(entity.ensure-limited-company
    :name "Allianz PV-WS Fonds"
    :lei "529900H2Y17B1LIB6Z90"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900h2y17b1lib6z90)

;; ALLIANZ MONETAIRE
(entity.ensure-limited-company
    :name "ALLIANZ MONETAIRE"
    :lei "529900TIGC4UBSCOF384"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Paris"
    :as @lei_529900tigc4ubscof384)

;; KomfortDynamik Sondervermögen
(entity.ensure-limited-company
    :name "KomfortDynamik Sondervermögen"
    :lei "529900QB1U2U45OUD544"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900qb1u2u45oud544)

;; Allianz Strategie 2031 Plus
(entity.ensure-limited-company
    :name "Allianz Strategie 2031 Plus"
    :lei "52990053APPI4KHK3J88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990053appi4khk3j88)

;; AllianzGI-Fonds ZDD3
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ZDD3"
    :lei "529900YHSV3XC5TVHO63"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900yhsv3xc5tvho63)

;; AllianzGI-Fonds ESMT
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ESMT"
    :lei "529900P7ULFLAREWNR54"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900p7ulflarewnr54)

;; AllianzGI-Fonds DEW-Co
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DEW-Co"
    :lei "529900GPMZ8MH65H0Y58"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900gpmz8mh65h0y58)

;; AllianzGI-Fonds UGF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds UGF"
    :lei "529900X52DTP1CK80O56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900x52dtp1ck80o56)

;; AllianzGI-Fonds DSW-Co
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DSW-Co"
    :lei "5299008TA33FC7TUO729"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299008ta33fc7tuo729)

;; AllianzGI-Fonds PSDN
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PSDN"
    :lei "529900S5HYEHZ7TS9R61"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900s5hyehz7ts9r61)

;; PremiumStars Wachstum
(entity.ensure-limited-company
    :name "PremiumStars Wachstum"
    :lei "529900EORRODR5PXWY88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900eorrodr5pxwy88)

;; AllianzGI-Fonds LTSA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds LTSA"
    :lei "529900VQ0WM4JJM9V953"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900vq0wm4jjm9v953)

;; AllianzGI-Fonds MAV
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAV"
    :lei "5299006SZMWR0GK6YR22"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299006szmwr0gk6yr22)

;; LHCO-Fonds
(entity.ensure-limited-company
    :name "LHCO-Fonds"
    :lei "5299006YJ3I94IB6EX97"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299006yj3i94ib6ex97)

;; AllianzGI-Fonds FIB
(entity.ensure-limited-company
    :name "AllianzGI-Fonds FIB"
    :lei "529900M2AAYKFO7UZA31"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900m2aaykfo7uza31)

;; AllianzGI-Fonds Lipco III
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Lipco III"
    :lei "5299009QJ6W4QHJTZM78"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009qj6w4qhjtzm78)

;; SVCO III-Fonds
(entity.ensure-limited-company
    :name "SVCO III-Fonds"
    :lei "529900UODSCJ4AVUVG63"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900uodscj4avuvg63)

;; BAT-Cofonds
(entity.ensure-limited-company
    :name "BAT-Cofonds"
    :lei "5299004YDZRTQI6GVU40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004ydzrtqi6gvu40)

;; AllianzGI-Fonds KDCO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KDCO"
    :lei "529900TLURPFCD42Q108"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900tlurpfcd42q108)

;; AllianzGI-Fonds BSAF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BSAF"
    :lei "529900YZ0M6877WKYQ88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900yz0m6877wkyq88)

;; AllianzGI-Fonds GdP
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GdP"
    :lei "5299009LEE7K3A8DUV60"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009lee7k3a8duv60)

;; AllianzGI-Fonds DHCO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DHCO"
    :lei "529900BYM03U5N34QQ89"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900bym03u5n34qq89)

;; ELK-Cofonds
(entity.ensure-limited-company
    :name "ELK-Cofonds"
    :lei "529900RCCD73Q1ZFHE76"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900rccd73q1zfhe76)

;; AllianzGI-Fonds SFT2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SFT2"
    :lei "529900EBQXCS1RV51M18"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ebqxcs1rv51m18)

;; AllianzGI-Fonds BMS
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BMS"
    :lei "529900OFNLIX58PR0B55"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ofnlix58pr0b55)

;; CONVEST 21 VL
(entity.ensure-limited-company
    :name "CONVEST 21 VL"
    :lei "52990002V1VXKPTBBD46"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990002v1vxkptbbd46)

;; AllianzGI-Fonds Dietzb
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Dietzb"
    :lei "529900WWQK94R17CKT46"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900wwqk94r17ckt46)

;; AllianzGI-Fonds PVT
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PVT"
    :lei "529900360WUOB94X5369"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900360wuob94x5369)

;; Fondra
(entity.ensure-limited-company
    :name "Fondra"
    :lei "5299002DVSVXRCB3BS68"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002dvsvxrcb3bs68)

;; AllianzGI-Fonds EJS Stiftungsfonds
(entity.ensure-limited-company
    :name "AllianzGI-Fonds EJS Stiftungsfonds"
    :lei "529900PPV5981WYMNG95"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ppv5981wymng95)

;; VermögensManagement Stabilität
(entity.ensure-limited-company
    :name "VermögensManagement Stabilität"
    :lei "529900ROL8R94H3YXP57"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900rol8r94h3yxp57)

;; AllianzGI-Fonds Süwe
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Süwe"
    :lei "5299007J2WJ8MLRCY643"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299007j2wj8mlrcy643)

;; AllianzGI-H
(entity.ensure-limited-company
    :name "AllianzGI-H"
    :lei "529900SCYAV02RYZJF69"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900scyav02ryzjf69)

;; AllianzGI-Fonds FS Pension
(entity.ensure-limited-company
    :name "AllianzGI-Fonds FS Pension"
    :lei "5299009SXMBBM9K57231"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009sxmbbm9k57231)

;; AllianzGI-Fonds CT-DRAECO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds CT-DRAECO"
    :lei "529900GE17XO5TAIDG51"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ge17xo5taidg51)

;; PremiumStars Chance
(entity.ensure-limited-company
    :name "PremiumStars Chance"
    :lei "5299005VNUFF0I1P9068"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299005vnuff0i1p9068)

;; KD-Cofonds
(entity.ensure-limited-company
    :name "KD-Cofonds"
    :lei "529900CTWDGM8PYT2G43"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ctwdgm8pyt2g43)

;; AllianzGI-Fonds SCHLUCO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SCHLUCO"
    :lei "5299008KCXOJ7IO95X34"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299008kcxoj7io95x34)

;; AllianzGI-Fonds GUV
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GUV"
    :lei "529900IMUMRS4SYLZ250"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900imumrs4sylz250)

;; AllianzGI-Fonds BG RCI
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BG RCI"
    :lei "5299003EBXJDY9BWZ516"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299003ebxjdy9bwz516)

;; AllianzGI-Fonds WERT
(entity.ensure-limited-company
    :name "AllianzGI-Fonds WERT"
    :lei "529900088Z4VXZEE8R51"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900088z4vxzee8r51)

;; BGV-Masterfonds
(entity.ensure-limited-company
    :name "BGV-Masterfonds"
    :lei "529900XJ1FIG6E2SG912"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xj1fig6e2sg912)

;; AllianzGI-Fonds GEW
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GEW"
    :lei "5299006762RV2J611G05"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299006762rv2j611g05)

;; AllianzGI- Fonds DPF Dillinger Pensionsfonds
(entity.ensure-limited-company
    :name "AllianzGI- Fonds DPF Dillinger Pensionsfonds"
    :lei "529900WP83T353R8P197"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900wp83t353r8p197)

;; NUERNBERGER Euroland A
(entity.ensure-limited-company
    :name "NUERNBERGER Euroland A"
    :lei "5299000JWXHBQ3GD4C12"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299000jwxhbq3gd4c12)

;; Fondak
(entity.ensure-limited-company
    :name "Fondak"
    :lei "529900UGQI1MKHIHV006"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ugqi1mkhihv006)

;; Allianz Multi Manager Global Balanced
(entity.ensure-limited-company
    :name "Allianz Multi Manager Global Balanced"
    :lei "529900VFXA5H49KV6V41"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900vfxa5h49kv6v41)

;; AllianzGI-Fonds MAF1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF1"
    :lei "529900P56OSTX568P297"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900p56ostx568p297)

;; Allianz Interglobal
(entity.ensure-limited-company
    :name "Allianz Interglobal"
    :lei "52990091AYONT72HIM61"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990091ayont72him61)

;; AllianzGI-Fonds OJU
(entity.ensure-limited-company
    :name "AllianzGI-Fonds OJU"
    :lei "529900SOXLUMP8S0T658"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900soxlump8s0t658)

;; BGO-Cofonds
(entity.ensure-limited-company
    :name "BGO-Cofonds"
    :lei "529900OFKUNYU2OGC602"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ofkunyu2ogc602)

;; AllianzGI-Fonds SiV
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SiV"
    :lei "5299005HIRH5B12FYE11"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299005hirh5b12fye11)

;; AllianzGI-Fonds VBDK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VBDK"
    :lei "529900U7D6LIRGGHN979"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900u7d6lirgghn979)

;; AllianzGI-Fonds PHCO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PHCO"
    :lei "529900N7G3ULSEW9WU15"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900n7g3ulsew9wu15)

;; AllianzGI-Fonds INDU
(entity.ensure-limited-company
    :name "AllianzGI-Fonds INDU"
    :lei "5299003YPNIV8CQXFY82"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299003ypniv8cqxfy82)

;; AllianzGI-Fonds Mesco
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Mesco"
    :lei "52990034YDI7E37XD396"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990034ydi7e37xd396)

;; AllianzGI-Fonds Alpen
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Alpen"
    :lei "52990084AJ38IJUTFU22"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990084aj38ijutfu22)

;; Plusfonds
(entity.ensure-limited-company
    :name "Plusfonds"
    :lei "5299001RD7ZQVO3IXI69"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299001rd7zqvo3ixi69)

;; AllianzGI-Fonds Ukah
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Ukah"
    :lei "5299007PWT44DI9O7O29"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299007pwt44di9o7o29)

;; Allianz Adifonds
(entity.ensure-limited-company
    :name "Allianz Adifonds"
    :lei "5299004EJ7SR98TBV869"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004ej7sr98tbv869)

;; Allianz Adiverba
(entity.ensure-limited-company
    :name "Allianz Adiverba"
    :lei "529900NFLZPCPIGINK31"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900nflzpcpigink31)

;; Fondis
(entity.ensure-limited-company
    :name "Fondis"
    :lei "529900PCCXO63YVPHU80"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900pccxo63yvphu80)

;; AllianzGI-Fonds PRI
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PRI"
    :lei "529900JYVT8871LJ1056"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900jyvt8871lj1056)

;; AllianzGI-Fonds AKT-E
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AKT-E"
    :lei "5299005RXH9B59HL3G64"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299005rxh9b59hl3g64)

;; AllianzGI-Fonds BOGESTRA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BOGESTRA"
    :lei "529900W0WZM6ZALROX07"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900w0wzm6zalrox07)

;; Allianz SOA Fonds
(entity.ensure-limited-company
    :name "Allianz SOA Fonds"
    :lei "529900RLO7ES96HDJN72"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900rlo7es96hdjn72)

;; Allianz EEE Fonds
(entity.ensure-limited-company
    :name "Allianz EEE Fonds"
    :lei "5299002YEMGSRIDVK953"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002yemgsridvk953)

;; AllianzGI-Fonds Gano
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Gano"
    :lei "529900QV6MUUF99L7D71"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900qv6muuf99l7d71)

;; AllianzGI-Fonds BGHW
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BGHW"
    :lei "529900OPF7DUYXDC4T42"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900opf7duyxdc4t42)

;; Allianz Fonds Japan
(entity.ensure-limited-company
    :name "Allianz Fonds Japan"
    :lei "529900ZLOW5P1NSYO135"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900zlow5p1nsyo135)

;; AllianzGI-Fonds AMAG 2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AMAG 2"
    :lei "529900O1215SBISHVT23"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900o1215sbishvt23)

;; AllianzGI-Fonds SRP
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SRP"
    :lei "529900AH9AI7C2UE7C86"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ah9ai7c2ue7c86)

;; AllianzGI-Fonds Dunhill
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Dunhill"
    :lei "529900UXCFSJ30NDZJ43"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900uxcfsj30ndzj43)

;; AllianzGI-Fonds Grillparzer
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Grillparzer"
    :lei "5299009UQFEDBJ9A7B90"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009uqfedbj9a7b90)

;; AllianzGI-Fonds DC 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DC 1"
    :lei "529900IDJ96JK5AA5D14"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900idj96jk5aa5d14)

;; Allianz Strategiefonds Wachstum
(entity.ensure-limited-company
    :name "Allianz Strategiefonds Wachstum"
    :lei "529900F69YCTZAU4HY44"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900f69yctzau4hy44)

;; Allianz Fondsvorsorge 1957-1966
(entity.ensure-limited-company
    :name "Allianz Fondsvorsorge 1957-1966"
    :lei "5299009009C8ZNGTX548"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009009c8zngtx548)

;; Allianz US Large Cap Growth
(entity.ensure-limited-company
    :name "Allianz US Large Cap Growth"
    :lei "529900KI3KYVIDTGUB13"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ki3kyvidtgub13)

;; Allianz Strategiefonds Balance
(entity.ensure-limited-company
    :name "Allianz Strategiefonds Balance"
    :lei "529900O77QS2RDJCIX58"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900o77qs2rdjcix58)

;; Allianz Fondsvorsorge 1952-1956
(entity.ensure-limited-company
    :name "Allianz Fondsvorsorge 1952-1956"
    :lei "52990045V3079DAQ1841"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990045v3079daq1841)

;; Allianz Global Equity Dividend
(entity.ensure-limited-company
    :name "Allianz Global Equity Dividend"
    :lei "529900VRVENOFL7GGL10"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900vrvenofl7ggl10)

;; Allianz Fonds Schweiz
(entity.ensure-limited-company
    :name "Allianz Fonds Schweiz"
    :lei "529900NCQY88F9BU4B07"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ncqy88f9bu4b07)

;; Industria
(entity.ensure-limited-company
    :name "Industria"
    :lei "5299007TA5NYYK85LG87"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299007ta5nyyk85lg87)

;; Allianz Fondsvorsorge 1977-1996
(entity.ensure-limited-company
    :name "Allianz Fondsvorsorge 1977-1996"
    :lei "5299007KGBHS0KK5RR64"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299007kgbhs0kk5rr64)

;; Allianz Fondsvorsorge 1947-1951
(entity.ensure-limited-company
    :name "Allianz Fondsvorsorge 1947-1951"
    :lei "529900TTJJQAZA84X387"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ttjjqaza84x387)

;; Allianz Thesaurus
(entity.ensure-limited-company
    :name "Allianz Thesaurus"
    :lei "529900T7EX4CWNE7LZ52"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900t7ex4cwne7lz52)

;; Allianz Flexi Rentenfonds
(entity.ensure-limited-company
    :name "Allianz Flexi Rentenfonds"
    :lei "5299000ZLX6W8S070P45"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299000zlx6w8s070p45)

;; Allianz Strategiefonds Stabilität
(entity.ensure-limited-company
    :name "Allianz Strategiefonds Stabilität"
    :lei "529900SBL8UFQQDO9O87"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900sbl8ufqqdo9o87)

;; Allianz Fondsvorsorge 1967-1976
(entity.ensure-limited-company
    :name "Allianz Fondsvorsorge 1967-1976"
    :lei "529900VD8IZWGYD78932"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900vd8izwgyd78932)

;; Allianz Rohstofffonds
(entity.ensure-limited-company
    :name "Allianz Rohstofffonds"
    :lei "529900587TKM066GLL47"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900587tkm066gll47)

;; Allianz Nebenwerte Deutschland
(entity.ensure-limited-company
    :name "Allianz Nebenwerte Deutschland"
    :lei "529900HGAKCLBOEETG65"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900hgakclboeetg65)

;; AllianzGI- Fonds Stiftungsfonds Wissenschaft
(entity.ensure-limited-company
    :name "AllianzGI- Fonds Stiftungsfonds Wissenschaft"
    :lei "52990065NTI60CK2RE67"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990065nti60ck2re67)

;; AllianzGI-Fonds TOB
(entity.ensure-limited-company
    :name "AllianzGI-Fonds TOB"
    :lei "529900J7WT0EYSTHD675"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900j7wt0eysthd675)

;; Krebshilfe-2-Fonds
(entity.ensure-limited-company
    :name "Krebshilfe-2-Fonds"
    :lei "5299002PTUNXFBBV3780"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002ptunxfbbv3780)

;; AllianzGI-Fonds SRF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SRF"
    :lei "5299009OTTEL5LC54H88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009ottel5lc54h88)

;; AllianzGI-Fonds KTM
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KTM"
    :lei "529900G19SEHCNEZ7532"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900g19sehcnez7532)

;; AllianzGI-Fonds DRB 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DRB 1"
    :lei "529900TDL6L151998D28"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900tdl6l151998d28)

;; AllianzGI-Fonds MAF3
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF3"
    :lei "529900QHDGEHI7D5L121"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900qhdgehi7d5l121)

;; AllianzGI-Fonds HKL
(entity.ensure-limited-company
    :name "AllianzGI-Fonds HKL"
    :lei "5299002WIIK8BUMHQ916"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299002wiik8bumhq916)

;; AllianzGI-Fonds TSF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds TSF"
    :lei "529900JNF6XJ1DZ6EC42"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900jnf6xj1dz6ec42)

;; AllianzGI-Fonds SHL
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SHL"
    :lei "529900XM0QOT4APY0395"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xm0qot4apy0395)

;; AllianzGI-Fonds PUK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PUK"
    :lei "5299009BV0JGNIU62Z56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009bv0jgniu62z56)

;; AllianzGI-SAS Master
(entity.ensure-limited-company
    :name "AllianzGI-SAS Master"
    :lei "529900QQEBM60UU24E73"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900qqebm60uu24e73)

;; PremiumMandat Konservativ
(entity.ensure-limited-company
    :name "PremiumMandat Konservativ"
    :lei "529900I3KI2ILAPLKR33"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900i3ki2ilaplkr33)

;; AllianzGI-Fonds FEV
(entity.ensure-limited-company
    :name "AllianzGI-Fonds FEV"
    :lei "529900XM52QP9OLA4326"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xm52qp9ola4326)

;; AllianzGI-Fonds ALLRA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ALLRA"
    :lei "529900MKP1W0IJCEBT88"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900mkp1w0ijcebt88)

;; AllianzGI-Fonds BAT-LS
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BAT-LS"
    :lei "52990077N0R2A5BA4O89"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990077n0r2a5ba4o89)

;; AllianzGI-Fonds VSF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VSF"
    :lei "5299009KCIGTD2IIQB26"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009kcigtd2iiqb26)

;; AllianzGI-Fonds PAK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PAK"
    :lei "529900AR9CS4QRFT0L40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ar9cs4qrft0l40)

;; AllianzGI-Fonds DSW-Drefonds
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DSW-Drefonds"
    :lei "529900R84DGLIG3IIL30"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900r84dglig3iil30)

;; Allianz Strategiefonds Wachstum Plus
(entity.ensure-limited-company
    :name "Allianz Strategiefonds Wachstum Plus"
    :lei "529900HY7VMTCURBBI22"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900hy7vmtcurbbi22)

;; AllianzGI-Fonds Gano 2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Gano 2"
    :lei "529900RBD4YIQ66NSJ87"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900rbd4yiq66nsj87)

;; dbi-Fonds EKiBB
(entity.ensure-limited-company
    :name "dbi-Fonds EKiBB"
    :lei "5299006U3OW5X2QE9P56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299006u3ow5x2qe9p56)

;; AllianzGI-Fonds HNE
(entity.ensure-limited-company
    :name "AllianzGI-Fonds HNE"
    :lei "5299000NQHSPXQ422C64"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299000nqhspxq422c64)

;; Concentra
(entity.ensure-limited-company
    :name "Concentra"
    :lei "529900DM2Q9NT4ORX305"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900dm2q9nt4orx305)

;; Kapital Plus
(entity.ensure-limited-company
    :name "Kapital Plus"
    :lei "5299008YE9T4YKIER075"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299008ye9t4ykier075)

;; AllianzGI-Fonds MBRF 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MBRF 1"
    :lei "529900UFHO7ARBNJSM28"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900ufho7arbnjsm28)

;; AllianzGI-Fonds APF Renten
(entity.ensure-limited-company
    :name "AllianzGI-Fonds APF Renten"
    :lei "529900XBHBS44RT9QC24"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900xbhbs44rt9qc24)

;; dbi-Fonds DEWDI
(entity.ensure-limited-company
    :name "dbi-Fonds DEWDI"
    :lei "52990025WNXQSL3I1H18"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_52990025wnxqsl3i1h18)

;; AllianzGI-Fonds BRR
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BRR"
    :lei "529900CZ8P9Q2WR9YA20"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900cz8p9q2wr9ya20)

;; AllianzGI-Fonds BVK 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BVK 1"
    :lei "529900YJHYLQVNGT2G58"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900yjhylqvngt2g58)

;; AllianzGI-Fonds GVO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GVO"
    :lei "5299009U2RURRPA93S64"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299009u2rurrpa93s64)

;; AllianzGI-Fonds POTAK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds POTAK"
    :lei "529900CAKJUCPUXICH20"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900cakjucpuxich20)

;; AllianzGI-Fonds BIP
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BIP"
    :lei "529900ZUWZTQIR0ES354"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900zuwztqir0es354)

;; AllianzGI-Fonds KHP 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KHP 1"
    :lei "529900Q7VCTB9WCJ5W61"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900q7vctb9wcj5w61)

;; AllianzGI-Fonds BEE
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BEE"
    :lei "529900JH49LM32KO4342"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900jh49lm32ko4342)

;; Allianz CGI Fonds
(entity.ensure-limited-company
    :name "Allianz CGI Fonds"
    :lei "529900P3QSDMEPXMOH96"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900p3qsdmepxmoh96)

;; Allianz LAD Fonds
(entity.ensure-limited-company
    :name "Allianz LAD Fonds"
    :lei "529900VUQOOP1XWWX889"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900vuqoop1xwwx889)

;; AllianzGI-Fonds VEMK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VEMK"
    :lei "529900EN9TK8MJWMSG13"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900en9tk8mjwmsg13)

;; AllianzGI-Fonds BAVC
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BAVC"
    :lei "529900T3A020J5CR9H38"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900t3a020j5cr9h38)

;; AllianzGI-Fonds SVKK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds SVKK"
    :lei "529900A0JM8DVYUFIK10"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900a0jm8dvyufik10)

;; AllianzGI-Fonds BFKW
(entity.ensure-limited-company
    :name "AllianzGI-Fonds BFKW"
    :lei "5299005OP8GZCXLH4I74"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299005op8gzcxlh4i74)

;; AllianzGI-Fonds RANW II
(entity.ensure-limited-company
    :name "AllianzGI-Fonds RANW II"
    :lei "5299003Z8DF18PD1KE48"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299003z8df18pd1ke48)

;; AllianzGI-Fonds KMU SGB
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KMU SGB"
    :lei "529900OLVXY8IJTLMO91"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900olvxy8ijtlmo91)

;; AllianzGI-Fonds AVP
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AVP"
    :lei "529900JL34BZLAZLNQ56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900jl34bzlazlnq56)

;; AllianzGI-Fonds Spree
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Spree"
    :lei "529900UVCC5SNTJP3422"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900uvcc5sntjp3422)

;; AllianzGI-Fonds KVR 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KVR 1"
    :lei "529900999M0C8FIMMV40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900999m0c8fimmv40)

;; AllianzGI-Fonds KVT 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds KVT 1"
    :lei "529900X1BKU4R9VZ5Q70"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900x1bku4r9vz5q70)

;; AllianzGI-Fonds AOKDRE
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AOKDRE"
    :lei "529900O1I65JESM0DW48"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900o1i65jesm0dw48)

;; AllianzGI-Fonds ABF
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ABF"
    :lei "529900DJB93NK0W32H96"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900djb93nk0w32h96)

;; AllianzGI-Fonds PFD
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PFD"
    :lei "529900TDEBK6P85UML40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900tdebk6p85uml40)

;; dbi-Fonds ACU-K
(entity.ensure-limited-company
    :name "dbi-Fonds ACU-K"
    :lei "529900Q5FBZSBP8GBR35"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900q5fbzsbp8gbr35)

;; AllianzGI-Fonds GSH
(entity.ensure-limited-company
    :name "AllianzGI-Fonds GSH"
    :lei "529900Y8ENPN6GELD777"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900y8enpn6geld777)

;; AllianzGI-Fonds ACK
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ACK"
    :lei "529900ZPFVTMCXSG2O18"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900zpfvtmcxsg2o18)

;; AllianzGI-Fonds VSBW
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VSBW"
    :lei "529900EFEF5RG0ZRIT54"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_529900efef5rg0zrit54)

;; AllianzGI-Fonds VDB
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VDB"
    :lei "5299004ISYECGMWL8Z24"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5299004isyecgmwl8z24)

;; AllianzGI-Fonds DPWS
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DPWS"
    :lei "549300MORU7E364SHP69"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300moru7e364shp69)

;; AllianzGI-BAS Master
(entity.ensure-limited-company
    :name "AllianzGI-BAS Master"
    :lei "549300DURGRH4HQ2JD79"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300durgrh4hq2jd79)

;; AllianzGI-SKS Master
(entity.ensure-limited-company
    :name "AllianzGI-SKS Master"
    :lei "549300MKA4GT1VIOOD61"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300mka4gt1viood61)

;; AllianzGI-Fonds MAF6
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF6"
    :lei "549300SY6O0XNK1R5E03"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300sy6o0xnk1r5e03)

;; AllianzGI-Fonds MAF4
(entity.ensure-limited-company
    :name "AllianzGI-Fonds MAF4"
    :lei "54930005YMER04EKG287"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930005ymer04ekg287)

;; AllianzGI-Fonds AKT-W
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AKT-W"
    :lei "549300ZUG8XT0UT5X531"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300zug8xt0ut5x531)

;; VW AV
(entity.ensure-limited-company
    :name "VW AV"
    :lei "549300WSB0DFJXPF5C84"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300wsb0dfjxpf5c84)

;; Pimco EM Corporates
(entity.ensure-limited-company
    :name "Pimco EM Corporates"
    :lei "549300H4EPZHHZ2J8175"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300h4epzhhz2j8175)

;; Allianz Global Investors Investmentaktiengesellschaft mit Teilgesellschaftsvermögen - Ashmore Emerging Market Corporates
(entity.ensure-limited-company
    :name "Allianz Global Investors Investmentaktiengesellschaft mit Teilgesellschaftsvermögen - Ashmore Emerging Market Corporates"
    :lei "5493000NZ60PSV04VO86"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493000nz60psv04vo86)

;; Allianz VKA Fonds
(entity.ensure-limited-company
    :name "Allianz VKA Fonds"
    :lei "549300Z41D3PXCTBWZ68"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300z41d3pxctbwz68)

;; Allianz VK RentenDirekt Fonds
(entity.ensure-limited-company
    :name "Allianz VK RentenDirekt Fonds"
    :lei "5493000L9DKNVEKE8M45"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493000l9dknveke8m45)

;; Allianz VGL Fonds
(entity.ensure-limited-company
    :name "Allianz VGL Fonds"
    :lei "5493004YX8WHFNG6XF28"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493004yx8whfng6xf28)

;; Allianz VGI 1 Fonds
(entity.ensure-limited-company
    :name "Allianz VGI 1 Fonds"
    :lei "549300PA2SO76ETZKB21"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300pa2so76etzkb21)

;; AllianzGI-Fonds VBE
(entity.ensure-limited-company
    :name "AllianzGI-Fonds VBE"
    :lei "549300DOU8OVOPU9MQ64"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300dou8ovopu9mq64)

;; Allianz UGD 1 Fonds
(entity.ensure-limited-company
    :name "Allianz UGD 1 Fonds"
    :lei "5493003EFZ2ITCZ8GL70"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493003efz2itcz8gl70)

;; Allianz RFG Fonds
(entity.ensure-limited-company
    :name "Allianz RFG Fonds"
    :lei "549300H0LRH7OSKNP750"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300h0lrh7osknp750)

;; Allianz Re Asia Fonds
(entity.ensure-limited-company
    :name "Allianz Re Asia Fonds"
    :lei "549300B60KK4HE62ZB78"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300b60kk4he62zb78)

;; AllianzGI-Fonds OAD
(entity.ensure-limited-company
    :name "AllianzGI-Fonds OAD"
    :lei "54930048NY6KYSOHEF78"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930048ny6kysohef78)

;; AllianzGI-Fonds NBP
(entity.ensure-limited-company
    :name "AllianzGI-Fonds NBP"
    :lei "549300F7GT6KRJMLEH47"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300f7gt6krjmleh47)

;; Allianz GLRS Fonds
(entity.ensure-limited-company
    :name "Allianz GLRS Fonds"
    :lei "549300KPSVP4LEC4M973"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300kpsvp4lec4m973)

;; Allianz GLR Fonds
(entity.ensure-limited-company
    :name "Allianz GLR Fonds"
    :lei "549300ULN27VFTVHZB09"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300uln27vftvhzb09)

;; AllianzGI-Fonds Elysee
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Elysee"
    :lei "549300T00X81I1NSDD81"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300t00x81i1nsdd81)

;; CBP LDI
(entity.ensure-limited-company
    :name "CBP LDI"
    :lei "5493002HZJZQN408TJ61"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493002hzjzqn408tj61)

;; CBP Growth
(entity.ensure-limited-company
    :name "CBP Growth"
    :lei "549300MYJKSVN0CTQE40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300myjksvn0ctqe40)

;; ATZ Banken
(entity.ensure-limited-company
    :name "ATZ Banken"
    :lei "5493007F1KYP48B6JN93"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493007f1kyp48b6jn93)

;; Allianz ARD Fonds
(entity.ensure-limited-company
    :name "Allianz ARD Fonds"
    :lei "549300VS43E6O485MZ38"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300vs43e6o485mz38)

;; Allianz APAV Fonds
(entity.ensure-limited-company
    :name "Allianz APAV Fonds"
    :lei "5493008YX91FLJIHCM09"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493008yx91fljihcm09)

;; dbi-Fonds ANDUS
(entity.ensure-limited-company
    :name "dbi-Fonds ANDUS"
    :lei "549300GHTWN657EOGS58"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300ghtwn657eogs58)

;; AllianzGI-Fonds ZGEFO
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ZGEFO"
    :lei "549300IGN32C5BIPU733"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300ign32c5bipu733)

;; AllianzGI-Fonds ZDD2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds ZDD2"
    :lei "549300YHNNH7C39ZFB47"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300yhnnh7c39zfb47)

;; AllianzGI-Fonds HS 2
(entity.ensure-limited-company
    :name "AllianzGI-Fonds HS 2"
    :lei "549300YE7XEN764Z3U71"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300ye7xen764z3u71)

;; AllianzGI-Fonds Ferrostaal Renten 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Ferrostaal Renten 1"
    :lei "549300B1TVDIF4GT6108"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300b1tvdif4gt6108)

;; Airbus group for Life Rentenfonds
(entity.ensure-limited-company
    :name "Airbus group for Life Rentenfonds"
    :lei "549300ECP0CGAUGN4Q79"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300ecp0cgaugn4q79)

;; Allianz VSR Fonds
(entity.ensure-limited-company
    :name "Allianz VSR Fonds"
    :lei "549300GJ5E3P7OV88637"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300gj5e3p7ov88637)

;; Allianz VAE Fonds
(entity.ensure-limited-company
    :name "Allianz VAE Fonds"
    :lei "549300WFF1MLGKRQX490"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300wff1mlgkrqx490)

;; Allianz SGI 1 Fonds
(entity.ensure-limited-company
    :name "Allianz SGI 1 Fonds"
    :lei "549300YXY6TY3210FW82"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300yxy6ty3210fw82)

;; Allianz SDR Fonds
(entity.ensure-limited-company
    :name "Allianz SDR Fonds"
    :lei "549300LSBI7O1KV6ZN56"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300lsbi7o1kv6zn56)

;; Allianz SGB Renten
(entity.ensure-limited-company
    :name "Allianz SGB Renten"
    :lei "549300KIN76YY6GER036"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300kin76yy6ger036)

;; Allianz Rentenfonds
(entity.ensure-limited-company
    :name "Allianz Rentenfonds"
    :lei "549300E951GZT57Y7C57"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300e951gzt57y7c57)

;; Allianz Money Market US $
(entity.ensure-limited-company
    :name "Allianz Money Market US $"
    :lei "549300OCOIAZEX0BDB67"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300ocoiazex0bdb67)

;; Allianz Mobil-Fonds
(entity.ensure-limited-company
    :name "Allianz Mobil-Fonds"
    :lei "549300B25J18HDD4YF12"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300b25j18hdd4yf12)

;; Allianz Internationaler Rentenfonds
(entity.ensure-limited-company
    :name "Allianz Internationaler Rentenfonds"
    :lei "549300YAHRMZ64WMDV94"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300yahrmz64wmdv94)

;; Allianz PIMCO High Yield Income Fund
(entity.ensure-limited-company
    :name "Allianz PIMCO High Yield Income Fund"
    :lei "549300ERI18Q4K90HF46"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "AIR5"
    :city "Senningerberg"
    :as @lei_549300eri18q4k90hf46)

;; Allianz Euro Rentenfonds
(entity.ensure-limited-company
    :name "Allianz Euro Rentenfonds"
    :lei "549300BLGUFVSKDBHQ66"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300blgufvskdbhq66)

;; Allianz MET 1 Fonds
(entity.ensure-limited-company
    :name "Allianz MET 1 Fonds"
    :lei "5493009WM4S9HPXSQ012"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493009wm4s9hpxsq012)

;; Allianz GRGB Fonds
(entity.ensure-limited-company
    :name "Allianz GRGB Fonds"
    :lei "5493003BNXUKCP2WTL71"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493003bnxukcp2wtl71)

;; Allianz FAD Fonds
(entity.ensure-limited-company
    :name "Allianz FAD Fonds"
    :lei "549300I5FGD97GY1C248"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300i5fgd97gy1c248)

;; Allianz ABA Fonds
(entity.ensure-limited-company
    :name "Allianz ABA Fonds"
    :lei "549300FCSTERROINHY34"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300fcsterroinhy34)

;; Allianz ALD Fonds
(entity.ensure-limited-company
    :name "Allianz ALD Fonds"
    :lei "5493009GSRC2GZ0FXN14"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493009gsrc2gz0fxn14)

;; Allianz AKR Fonds
(entity.ensure-limited-company
    :name "Allianz AKR Fonds"
    :lei "5493002Z2VKYUQJCSX22"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493002z2vkyuqjcsx22)

;; Allianz AADB Fonds
(entity.ensure-limited-company
    :name "Allianz AADB Fonds"
    :lei "5493006W0OXZHNT7LI48"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493006w0oxzhnt7li48)

;; AllianzGI-Fonds A200
(entity.ensure-limited-company
    :name "AllianzGI-Fonds A200"
    :lei "549300BJ0U1KWA7O2D49"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300bj0u1kwa7o2d49)

;; Allianz PV-RD Fonds
(entity.ensure-limited-company
    :name "Allianz PV-RD Fonds"
    :lei "549300VZ6U79WMSPIN73"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300vz6u79wmspin73)

;; AllianzGI-Fonds Pfalco
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Pfalco"
    :lei "549300EZ4EPNJYMIW145"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300ez4epnjymiw145)

;; AllianzGI-Fonds RG-Anlage
(entity.ensure-limited-company
    :name "AllianzGI-Fonds RG-Anlage"
    :lei "549300MY7X3VX8RZS840"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300my7x3vx8rzs840)

;; AllianzGI-Fonds APNIESA
(entity.ensure-limited-company
    :name "AllianzGI-Fonds APNIESA"
    :lei "549300SPCMF4BY627T62"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300spcmf4by627t62)

;; AllianzGI-Fonds PF 1
(entity.ensure-limited-company
    :name "AllianzGI-Fonds PF 1"
    :lei "549300N1L9DBR2F5U334"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300n1l9dbr2f5u334)

;; AllianzGI-Fonds Pensions
(entity.ensure-limited-company
    :name "AllianzGI-Fonds Pensions"
    :lei "54930028LV63BJAYYM34"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930028lv63bjayym34)

;; AllianzGI-Fonds HPT
(entity.ensure-limited-company
    :name "AllianzGI-Fonds HPT"
    :lei "54930004B8611HXVOQ12"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930004b8611hxvoq12)

;; AllianzGI - Fonds PKM Degussa
(entity.ensure-limited-company
    :name "AllianzGI - Fonds PKM Degussa"
    :lei "549300WYE06A2HRQ0Q04"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300wye06a2hrq0q04)

;; AllianzGI-Fonds DSPT
(entity.ensure-limited-company
    :name "AllianzGI-Fonds DSPT"
    :lei "549300U6UYHNIFAZLB73"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300u6uyhnifazlb73)

;; AllianzGI-Fonds D300
(entity.ensure-limited-company
    :name "AllianzGI-Fonds D300"
    :lei "549300Y7NO20WRDDXK54"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_549300y7no20wrddxk54)

;; AllianzGI-Fonds AFE
(entity.ensure-limited-company
    :name "AllianzGI-Fonds AFE"
    :lei "5493007XYK1V6UOYTV08"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_5493007xyk1v6uoytv08)

;; SGB Geldmarkt
(entity.ensure-limited-company
    :name "SGB Geldmarkt"
    :lei "54930082YQ3IU7OTG277"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :gleif-status "ACTIVE"
    :legal-form-code "8888"
    :city "Frankfurt am Main"
    :as @lei_54930082yq3iu7otg277)

;; ============================================================================
;; PHASE 4: CBUs with Role Assignments (417)
;; Roles: ASSET_OWNER, INVESTMENT_MANAGER, MANAGEMENT_COMPANY, SICAV*, ULTIMATE_CLIENT
;; *SICAV only for sub-funds, points to umbrella entity (not fund itself!)
;; ============================================================================

;; CBU: Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF

(cbu.ensure
    :name "Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900lsfq65emqnbp87)

(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_529900lsfq65emqnbp87
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900kp3l513irkr804)

(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_529900kp3l513irkr804
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Private Credit French Fund

(cbu.ensure
    :name "Allianz Private Credit French Fund"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900uvazhc35lulz81)

(cbu.assign-role
    :cbu-id @cbu_529900uvazhc35lulz81
    :entity-id @lei_529900uvazhc35lulz81
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900uvazhc35lulz81
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900uvazhc35lulz81
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900uvazhc35lulz81
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Asia Pacific Infrastructure Credit Fund S.A., SICAV-RAIF

(cbu.ensure
    :name "Allianz Asia Pacific Infrastructure Credit Fund S.A., SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900bgq1ujwrom5949)

(cbu.assign-role
    :cbu-id @cbu_529900bgq1ujwrom5949
    :entity-id @lei_529900bgq1ujwrom5949
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900bgq1ujwrom5949
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900bgq1ujwrom5949
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900bgq1ujwrom5949
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Credit Emerging Markets Fund S.A., SICAV-RAIF

(cbu.ensure
    :name "Allianz Credit Emerging Markets Fund S.A., SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_52990031jhq1yl8ohb45)

(cbu.assign-role
    :cbu-id @cbu_52990031jhq1yl8ohb45
    :entity-id @lei_52990031jhq1yl8ohb45
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990031jhq1yl8ohb45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990031jhq1yl8ohb45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990031jhq1yl8ohb45
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz AlTi SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz AlTi SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900ncy53i2uy5qv52)

(cbu.assign-role
    :cbu-id @cbu_529900ncy53i2uy5qv52
    :entity-id @lei_529900ncy53i2uy5qv52
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ncy53i2uy5qv52
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ncy53i2uy5qv52
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ncy53i2uy5qv52
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Infrastructure Credit Opportunities Fund II SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz Infrastructure Credit Opportunities Fund II SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900ncp5devq6eok97)

(cbu.assign-role
    :cbu-id @cbu_529900ncp5devq6eok97
    :entity-id @lei_529900ncp5devq6eok97
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ncp5devq6eok97
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ncp5devq6eok97
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ncp5devq6eok97
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ SELECTION

(cbu.ensure
    :name "ALLIANZ SELECTION"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900b2kzcgbfobmg71)

(cbu.assign-role
    :cbu-id @cbu_529900b2kzcgbfobmg71
    :entity-id @lei_529900b2kzcgbfobmg71
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900b2kzcgbfobmg71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900b2kzcgbfobmg71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900b2kzcgbfobmg71
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ EPARGNE RETRAITE

(cbu.ensure
    :name "ALLIANZ EPARGNE RETRAITE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900x8dccdlcnipf48)

(cbu.assign-role
    :cbu-id @cbu_529900x8dccdlcnipf48
    :entity-id @lei_529900x8dccdlcnipf48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900x8dccdlcnipf48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900x8dccdlcnipf48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900x8dccdlcnipf48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Trade Finance Funds S.A., SICAV-RAIF - ALLIANZ WORKING CAPITAL INVOICE FINANCE FUND

(cbu.ensure
    :name "Allianz Trade Finance Funds S.A., SICAV-RAIF - ALLIANZ WORKING CAPITAL INVOICE FINANCE FUND"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_52990088a09wyiqr6770)

(cbu.assign-role
    :cbu-id @cbu_52990088a09wyiqr6770
    :entity-id @lei_52990088a09wyiqr6770
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990088a09wyiqr6770
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990088a09wyiqr6770
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990088a09wyiqr6770
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Impact Private Credit S.A., SICAV-RAIF

(cbu.ensure
    :name "Allianz Impact Private Credit S.A., SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299006vqyourkk6rr02)

(cbu.assign-role
    :cbu-id @cbu_5299006vqyourkk6rr02
    :entity-id @lei_5299006vqyourkk6rr02
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299006vqyourkk6rr02
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299006vqyourkk6rr02
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299006vqyourkk6rr02
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Diversified Private Debt Fund II SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Diversified Private Debt Fund II SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900yk8y4sj860cy04)

(cbu.assign-role
    :cbu-id @cbu_529900yk8y4sj860cy04
    :entity-id @lei_529900yk8y4sj860cy04
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900yk8y4sj860cy04
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900yk8y4sj860cy04
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900yk8y4sj860cy04
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ SENIOR EUROPEAN INFRASTRUCTURE DEBT FUND, SCSp, SICAV-RAIF

(cbu.ensure
    :name "ALLIANZ SENIOR EUROPEAN INFRASTRUCTURE DEBT FUND, SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900ntzs8r02efyz08)

(cbu.assign-role
    :cbu-id @cbu_529900ntzs8r02efyz08
    :entity-id @lei_529900ntzs8r02efyz08
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ntzs8r02efyz08
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ntzs8r02efyz08
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ntzs8r02efyz08
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Private Markets Solutions Fund S.A. SICAV-RAIF - Allianz Core Private Markets Fund

(cbu.ensure
    :name "Allianz Private Markets Solutions Fund S.A. SICAV-RAIF - Allianz Core Private Markets Fund"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900zq9e9v5m7i1291)

(cbu.assign-role
    :cbu-id @cbu_529900zq9e9v5m7i1291
    :entity-id @lei_529900zq9e9v5m7i1291
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zq9e9v5m7i1291
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zq9e9v5m7i1291
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zq9e9v5m7i1291
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Diversified Infrastructure and Energy Transition Debt Fund SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Diversified Infrastructure and Energy Transition Debt Fund SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900gr9m1xzohtwi24)

(cbu.assign-role
    :cbu-id @cbu_529900gr9m1xzohtwi24
    :entity-id @lei_529900gr9m1xzohtwi24
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gr9m1xzohtwi24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gr9m1xzohtwi24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gr9m1xzohtwi24
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ US AND ASIA-PACIFIC REAL ESTATE DEBT OPPORTUNITIES FUND SCSP, SICAV-RAIF

(cbu.ensure
    :name "ALLIANZ US AND ASIA-PACIFIC REAL ESTATE DEBT OPPORTUNITIES FUND SCSP, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_52990052n8hoo6f7gq66)

(cbu.assign-role
    :cbu-id @cbu_52990052n8hoo6f7gq66
    :entity-id @lei_52990052n8hoo6f7gq66
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990052n8hoo6f7gq66
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990052n8hoo6f7gq66
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990052n8hoo6f7gq66
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Real Estate Debt Opportunities Feeder Fund SA, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Real Estate Debt Opportunities Feeder Fund SA, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900wxdnkxpo62h858)

(cbu.assign-role
    :cbu-id @cbu_529900wxdnkxpo62h858
    :entity-id @lei_529900wxdnkxpo62h858
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wxdnkxpo62h858
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wxdnkxpo62h858
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wxdnkxpo62h858
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Private Debt Opportunities Fund SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Private Debt Opportunities Fund SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900hfs1uw1f1xuc79)

(cbu.assign-role
    :cbu-id @cbu_529900hfs1uw1f1xuc79
    :entity-id @lei_529900hfs1uw1f1xuc79
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900hfs1uw1f1xuc79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900hfs1uw1f1xuc79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900hfs1uw1f1xuc79
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Private Debt Opportunities Feeder Fund SA, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Private Debt Opportunities Feeder Fund SA, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900dim2cxr3lknv85)

(cbu.assign-role
    :cbu-id @cbu_529900dim2cxr3lknv85
    :entity-id @lei_529900dim2cxr3lknv85
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dim2cxr3lknv85
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dim2cxr3lknv85
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dim2cxr3lknv85
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Real Estate Debt Opportunities Fund SCSp, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Real Estate Debt Opportunities Fund SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900anuc6hrem95723)

(cbu.assign-role
    :cbu-id @cbu_529900anuc6hrem95723
    :entity-id @lei_529900anuc6hrem95723
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900anuc6hrem95723
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900anuc6hrem95723
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900anuc6hrem95723
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz FLG Private Debt Fund SA, SICAV-RAIF

(cbu.ensure
    :name "Allianz FLG Private Debt Fund SA, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299009dg2ffognz5y64)

(cbu.assign-role
    :cbu-id @cbu_5299009dg2ffognz5y64
    :entity-id @lei_5299009dg2ffognz5y64
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009dg2ffognz5y64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009dg2ffognz5y64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009dg2ffognz5y64
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Emerging Market Climate Action Fund, SCSp SICAV-RAIF

(cbu.ensure
    :name "Emerging Market Climate Action Fund, SCSp SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900syymq0vr4epr77)

(cbu.assign-role
    :cbu-id @cbu_529900syymq0vr4epr77
    :entity-id @lei_529900syymq0vr4epr77
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900syymq0vr4epr77
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900syymq0vr4epr77
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900syymq0vr4epr77
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Resilient Opportunistic Credit Feeder Fund SA, SICAV-RAIF

(cbu.ensure
    :name "Allianz Resilient Opportunistic Credit Feeder Fund SA, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900gsaxkveas64x94)

(cbu.assign-role
    :cbu-id @cbu_529900gsaxkveas64x94
    :entity-id @lei_529900gsaxkveas64x94
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gsaxkveas64x94
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gsaxkveas64x94
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gsaxkveas64x94
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Diversified Private Debt Feeder Fund SA, SICAV-RAIF

(cbu.ensure
    :name "Allianz Global Diversified Private Debt Feeder Fund SA, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299009xv3oyh5scbn33)

(cbu.assign-role
    :cbu-id @cbu_5299009xv3oyh5scbn33
    :entity-id @lei_5299009xv3oyh5scbn33
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009xv3oyh5scbn33
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009xv3oyh5scbn33
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009xv3oyh5scbn33
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Impact Investment Fund, S.A. SICAV-RAIF - Allianz Impact Investment Fund Compartment I

(cbu.ensure
    :name "Allianz Impact Investment Fund, S.A. SICAV-RAIF - Allianz Impact Investment Fund Compartment I"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900x36i1jqx58qi40)

(cbu.assign-role
    :cbu-id @cbu_529900x36i1jqx58qi40
    :entity-id @lei_529900x36i1jqx58qi40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900x36i1jqx58qi40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900x36i1jqx58qi40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900x36i1jqx58qi40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Investors Fund

(cbu.ensure
    :name "Allianz Global Investors Fund"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_4kt8dcrlarep7c35mw05)

(cbu.assign-role
    :cbu-id @cbu_4kt8dcrlarep7c35mw05
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_4kt8dcrlarep7c35mw05
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_4kt8dcrlarep7c35mw05
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_4kt8dcrlarep7c35mw05
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Enhanced Equity Income

(cbu.ensure
    :name "Allianz Global Enhanced Equity Income"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900o2d7wttp2ecm60)

(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_529900o2d7wttp2ecm60
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz EuropEquity Crescendo

(cbu.ensure
    :name "Allianz EuropEquity Crescendo"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900a4n4fmrf1qit75)

(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_529900a4n4fmrf1qit75
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: OCIRP Actions Multifacteurs

(cbu.ensure
    :name "OCIRP Actions Multifacteurs"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900zwzd2xkz3gfo55)

(cbu.assign-role
    :cbu-id @cbu_529900zwzd2xkz3gfo55
    :entity-id @lei_529900zwzd2xkz3gfo55
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zwzd2xkz3gfo55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zwzd2xkz3gfo55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zwzd2xkz3gfo55
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CAPVIVA Infrastructure

(cbu.ensure
    :name "CAPVIVA Infrastructure"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299007d2y764jwnw850)

(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_5299007d2y764jwnw850
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_5299008fhadwurgmxp80
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz European Autonomy

(cbu.ensure
    :name "Allianz European Autonomy"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900e5teg9cgu33298)

(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_529900e5teg9cgu33298
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Aktien Dividende Global

(cbu.ensure
    :name "Aktien Dividende Global"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900nspm728j89yr40)

(cbu.assign-role
    :cbu-id @cbu_529900nspm728j89yr40
    :entity-id @lei_529900nspm728j89yr40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900nspm728j89yr40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900nspm728j89yr40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900nspm728j89yr40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)

(cbu.ensure
    :name "Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ud1fmokr9rn344)

(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_529900ud1fmokr9rn344
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_529900kp3l513irkr804
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Best Styles US Small Cap Equity

(cbu.ensure
    :name "Allianz Best Styles US Small Cap Equity"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299008buz4793qzrd79)

(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_5299008buz4793qzrd79
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: EPC III Compartment

(cbu.ensure
    :name "EPC III Compartment"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_5299006dhnn51cvbhf80)

(cbu.assign-role
    :cbu-id @cbu_5299006dhnn51cvbhf80
    :entity-id @lei_5299006dhnn51cvbhf80
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299006dhnn51cvbhf80
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299006dhnn51cvbhf80
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Private Credit French Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299006dhnn51cvbhf80
    :entity-id @lei_529900uvazhc35lulz81
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299006dhnn51cvbhf80
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Private Debt Co-Investments

(cbu.ensure
    :name "Private Debt Co-Investments"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900lsi5woy3j9um22)

(cbu.assign-role
    :cbu-id @cbu_529900lsi5woy3j9um22
    :entity-id @lei_529900lsi5woy3j9um22
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900lsi5woy3j9um22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900lsi5woy3j9um22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz AlTi SCSp, SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900lsi5woy3j9um22
    :entity-id @lei_529900ncy53i2uy5qv52
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900lsi5woy3j9um22
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Private Debt Secondaries

(cbu.ensure
    :name "Private Debt Secondaries"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299004q6c8tkhnt0v91)

(cbu.assign-role
    :cbu-id @cbu_5299004q6c8tkhnt0v91
    :entity-id @lei_5299004q6c8tkhnt0v91
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004q6c8tkhnt0v91
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004q6c8tkhnt0v91
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz AlTi SCSp, SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299004q6c8tkhnt0v91
    :entity-id @lei_529900ncy53i2uy5qv52
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004q6c8tkhnt0v91
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Infrastructure

(cbu.ensure
    :name "Allianz Global Infrastructure"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900fvqr2vcw730o08)

(cbu.assign-role
    :cbu-id @cbu_529900fvqr2vcw730o08
    :entity-id @lei_529900fvqr2vcw730o08
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900fvqr2vcw730o08
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900fvqr2vcw730o08
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Real Assets and Private Markets Fund S.A. SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900fvqr2vcw730o08
    :entity-id @lei_5299001kbl1hp5t3tj23
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900fvqr2vcw730o08
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Diversified Private Debt II EUR Feeder Fund (Germany)

(cbu.ensure
    :name "Allianz Global Diversified Private Debt II EUR Feeder Fund (Germany)"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299001lcv2l4tqtyd24)

(cbu.assign-role
    :cbu-id @cbu_5299001lcv2l4tqtyd24
    :entity-id @lei_5299001lcv2l4tqtyd24
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299001lcv2l4tqtyd24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299001lcv2l4tqtyd24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Diversified Private Debt Fund II SCSp, SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299001lcv2l4tqtyd24
    :entity-id @lei_529900yk8y4sj860cy04
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299001lcv2l4tqtyd24
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Impact Private Credit Dedicated Holding SCSp

(cbu.ensure
    :name "Allianz Impact Private Credit Dedicated Holding SCSp"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299004lfeorxs4m8i86)

(cbu.assign-role
    :cbu-id @cbu_5299004lfeorxs4m8i86
    :entity-id @lei_5299004lfeorxs4m8i86
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004lfeorxs4m8i86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004lfeorxs4m8i86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Real Assets and Private Markets Fund S.A. SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299004lfeorxs4m8i86
    :entity-id @lei_5299001kbl1hp5t3tj23
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004lfeorxs4m8i86
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Target Maturity Euro Bond V

(cbu.ensure
    :name "Allianz Target Maturity Euro Bond V"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900vebfbdbxfw0o55)

(cbu.assign-role
    :cbu-id @cbu_529900vebfbdbxfw0o55
    :entity-id @lei_529900vebfbdbxfw0o55
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vebfbdbxfw0o55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vebfbdbxfw0o55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900vebfbdbxfw0o55
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vebfbdbxfw0o55
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF - EIS

(cbu.ensure
    :name "ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF - EIS"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900vd1wf5gywvk357)

(cbu.assign-role
    :cbu-id @cbu_529900vd1wf5gywvk357
    :entity-id @lei_529900vd1wf5gywvk357
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vd1wf5gywvk357
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vd1wf5gywvk357
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900vd1wf5gywvk357
    :entity-id @lei_5299008fhadwurgmxp80
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vd1wf5gywvk357
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ EPARGNE OBLIGATIONS EURO

(cbu.ensure
    :name "ALLIANZ EPARGNE OBLIGATIONS EURO"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900vel50yozjvdo67)

(cbu.assign-role
    :cbu-id @cbu_529900vel50yozjvdo67
    :entity-id @lei_529900vel50yozjvdo67
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vel50yozjvdo67
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vel50yozjvdo67
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ EPARGNE RETRAITE (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900vel50yozjvdo67
    :entity-id @lei_529900x8dccdlcnipf48
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vel50yozjvdo67
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ EPARGNE ACTIONS FRANCE

(cbu.ensure
    :name "ALLIANZ EPARGNE ACTIONS FRANCE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900x88m8ag6eck323)

(cbu.assign-role
    :cbu-id @cbu_529900x88m8ag6eck323
    :entity-id @lei_529900x88m8ag6eck323
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900x88m8ag6eck323
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900x88m8ag6eck323
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ EPARGNE RETRAITE (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900x88m8ag6eck323
    :entity-id @lei_529900x8dccdlcnipf48
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900x88m8ag6eck323
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Target Maturity Euro Bond IV

(cbu.ensure
    :name "Allianz Target Maturity Euro Bond IV"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900lifzm3onfvc719)

(cbu.assign-role
    :cbu-id @cbu_529900lifzm3onfvc719
    :entity-id @lei_529900lifzm3onfvc719
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900lifzm3onfvc719
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900lifzm3onfvc719
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900lifzm3onfvc719
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900lifzm3onfvc719
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ EPARGNE ACTIONS MONDE

(cbu.ensure
    :name "ALLIANZ EPARGNE ACTIONS MONDE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900rme4yzo5nppo40)

(cbu.assign-role
    :cbu-id @cbu_529900rme4yzo5nppo40
    :entity-id @lei_529900rme4yzo5nppo40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rme4yzo5nppo40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rme4yzo5nppo40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ EPARGNE RETRAITE (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900rme4yzo5nppo40
    :entity-id @lei_529900x8dccdlcnipf48
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rme4yzo5nppo40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ EPARGNE ACTIONS SOLIDAIRE

(cbu.ensure
    :name "ALLIANZ EPARGNE ACTIONS SOLIDAIRE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900pqaw8ffyab8344)

(cbu.assign-role
    :cbu-id @cbu_529900pqaw8ffyab8344
    :entity-id @lei_529900pqaw8ffyab8344
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900pqaw8ffyab8344
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900pqaw8ffyab8344
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ EPARGNE RETRAITE (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900pqaw8ffyab8344
    :entity-id @lei_529900x8dccdlcnipf48
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900pqaw8ffyab8344
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ EPARGNE DIVERSIFIE

(cbu.ensure
    :name "ALLIANZ EPARGNE DIVERSIFIE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900eri9bh5ypf3105)

(cbu.assign-role
    :cbu-id @cbu_529900eri9bh5ypf3105
    :entity-id @lei_529900eri9bh5ypf3105
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900eri9bh5ypf3105
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900eri9bh5ypf3105
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ EPARGNE RETRAITE (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900eri9bh5ypf3105
    :entity-id @lei_529900x8dccdlcnipf48
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900eri9bh5ypf3105
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Infrastucture ELTIF

(cbu.ensure
    :name "Allianz Global Infrastucture ELTIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299002qogyf6unrlj60)

(cbu.assign-role
    :cbu-id @cbu_5299002qogyf6unrlj60
    :entity-id @lei_5299002qogyf6unrlj60
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002qogyf6unrlj60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002qogyf6unrlj60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz ELTIF Umbrella SCA SICAV (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299002qogyf6unrlj60
    :entity-id @lei_529900j65rdocljquc44
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002qogyf6unrlj60
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ RETRAITE SELECTION ACTIONS

(cbu.ensure
    :name "ALLIANZ RETRAITE SELECTION ACTIONS"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_52990050ozdq0h80hc61)

(cbu.assign-role
    :cbu-id @cbu_52990050ozdq0h80hc61
    :entity-id @lei_52990050ozdq0h80hc61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990050ozdq0h80hc61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990050ozdq0h80hc61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ SELECTION (umbrella)
(cbu.assign-role
    :cbu-id @cbu_52990050ozdq0h80hc61
    :entity-id @lei_529900b2kzcgbfobmg71
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990050ozdq0h80hc61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ RETRAITE SELECTION OBLIGATIONS

(cbu.ensure
    :name "ALLIANZ RETRAITE SELECTION OBLIGATIONS"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900gcde0iqu4elp86)

(cbu.assign-role
    :cbu-id @cbu_529900gcde0iqu4elp86
    :entity-id @lei_529900gcde0iqu4elp86
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gcde0iqu4elp86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gcde0iqu4elp86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ SELECTION (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900gcde0iqu4elp86
    :entity-id @lei_529900b2kzcgbfobmg71
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gcde0iqu4elp86
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Private Debt Secondary EUR Feeder Fund I (Germany)

(cbu.ensure
    :name "Allianz Private Debt Secondary EUR Feeder Fund I (Germany)"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900lswctmkmf7v008)

(cbu.assign-role
    :cbu-id @cbu_529900lswctmkmf7v008
    :entity-id @lei_529900lswctmkmf7v008
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900lswctmkmf7v008
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900lswctmkmf7v008
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900lswctmkmf7v008
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Impact Private Credit Dedicated Fund

(cbu.ensure
    :name "Allianz Impact Private Credit Dedicated Fund"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900zvn7cxf7h4up44)

(cbu.assign-role
    :cbu-id @cbu_529900zvn7cxf7h4up44
    :entity-id @lei_529900zvn7cxf7h4up44
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zvn7cxf7h4up44
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zvn7cxf7h4up44
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Real Assets and Private Markets Fund S.A. SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900zvn7cxf7h4up44
    :entity-id @lei_5299001kbl1hp5t3tj23
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zvn7cxf7h4up44
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MRPF

(cbu.ensure
    :name "AllianzGI-Fonds MRPF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900dejwpikn90p361)

(cbu.assign-role
    :cbu-id @cbu_529900dejwpikn90p361
    :entity-id @lei_529900dejwpikn90p361
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dejwpikn90p361
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dejwpikn90p361
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dejwpikn90p361
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ GLOBAL DIVERSIFIED INFRASTRUCTURE AND ENERGY TRANSITION DEBT FEEDER FUND (GERMANY)

(cbu.ensure
    :name "ALLIANZ GLOBAL DIVERSIFIED INFRASTRUCTURE AND ENERGY TRANSITION DEBT FEEDER FUND (GERMANY)"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900mn4pzexo6j5z18)

(cbu.assign-role
    :cbu-id @cbu_529900mn4pzexo6j5z18
    :entity-id @lei_529900mn4pzexo6j5z18
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900mn4pzexo6j5z18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900mn4pzexo6j5z18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900mn4pzexo6j5z18
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Emerging Europe Equity 2

(cbu.ensure
    :name "Allianz Emerging Europe Equity 2"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900p7qi4ahsni2026)

(cbu.assign-role
    :cbu-id @cbu_529900p7qi4ahsni2026
    :entity-id @lei_529900p7qi4ahsni2026
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900p7qi4ahsni2026
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900p7qi4ahsni2026
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900p7qi4ahsni2026
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900p7qi4ahsni2026
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Premium Champions

(cbu.ensure
    :name "Allianz Premium Champions"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299002yfr7xdea6ec13)

(cbu.assign-role
    :cbu-id @cbu_5299002yfr7xdea6ec13
    :entity-id @lei_5299002yfr7xdea6ec13
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002yfr7xdea6ec13
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002yfr7xdea6ec13
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299002yfr7xdea6ec13
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002yfr7xdea6ec13
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Target Maturity Euro Bond III

(cbu.ensure
    :name "Allianz Target Maturity Euro Bond III"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900ytc9ihb6rcq908)

(cbu.assign-role
    :cbu-id @cbu_529900ytc9ihb6rcq908
    :entity-id @lei_529900ytc9ihb6rcq908
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ytc9ihb6rcq908
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ytc9ihb6rcq908
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900ytc9ihb6rcq908
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ytc9ihb6rcq908
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Social Conviction Equity

(cbu.ensure
    :name "Allianz Social Conviction Equity"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900rics54ka4zv927)

(cbu.assign-role
    :cbu-id @cbu_529900rics54ka4zv927
    :entity-id @lei_529900rics54ka4zv927
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rics54ka4zv927
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rics54ka4zv927
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900rics54ka4zv927
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rics54ka4zv927
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BSVF.23

(cbu.ensure
    :name "AllianzGI-Fonds BSVF.23"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900pfzxndveqkal97)

(cbu.assign-role
    :cbu-id @cbu_529900pfzxndveqkal97
    :entity-id @lei_529900pfzxndveqkal97
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900pfzxndveqkal97
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900pfzxndveqkal97
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900pfzxndveqkal97
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BTH

(cbu.ensure
    :name "AllianzGI-Fonds BTH"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900oh76pkt5on8j74)

(cbu.assign-role
    :cbu-id @cbu_529900oh76pkt5on8j74
    :entity-id @lei_529900oh76pkt5on8j74
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900oh76pkt5on8j74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900oh76pkt5on8j74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900oh76pkt5on8j74
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VTH

(cbu.ensure
    :name "AllianzGI-Fonds VTH"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900k5l0cmtxh0q370)

(cbu.assign-role
    :cbu-id @cbu_529900k5l0cmtxh0q370
    :entity-id @lei_529900k5l0cmtxh0q370
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900k5l0cmtxh0q370
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900k5l0cmtxh0q370
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900k5l0cmtxh0q370
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz US Large Cap Value

(cbu.ensure
    :name "Allianz US Large Cap Value"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900pceky03so2gs40)

(cbu.assign-role
    :cbu-id @cbu_529900pceky03so2gs40
    :entity-id @lei_529900pceky03so2gs40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900pceky03so2gs40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900pceky03so2gs40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900pceky03so2gs40
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900pceky03so2gs40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz AZSE Master Funds

(cbu.ensure
    :name "Allianz AZSE Master Funds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900txf4scpct4vx78)

(cbu.assign-role
    :cbu-id @cbu_529900txf4scpct4vx78
    :entity-id @lei_529900txf4scpct4vx78
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900txf4scpct4vx78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900txf4scpct4vx78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900txf4scpct4vx78
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Transformation SAG

(cbu.ensure
    :name "AllianzGI-Fonds Transformation SAG"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900p666s0i9b8r306)

(cbu.assign-role
    :cbu-id @cbu_529900p666s0i9b8r306
    :entity-id @lei_529900p666s0i9b8r306
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900p666s0i9b8r306
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900p666s0i9b8r306
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900p666s0i9b8r306
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Pure Steel +

(cbu.ensure
    :name "AllianzGI-Fonds Pure Steel +"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900no8488wbf8so93)

(cbu.assign-role
    :cbu-id @cbu_529900no8488wbf8so93
    :entity-id @lei_529900no8488wbf8so93
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900no8488wbf8so93
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900no8488wbf8so93
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900no8488wbf8so93
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Target Maturity Euro Bond I

(cbu.ensure
    :name "Allianz Target Maturity Euro Bond I"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900agqhnhiylxhr90)

(cbu.assign-role
    :cbu-id @cbu_529900agqhnhiylxhr90
    :entity-id @lei_529900agqhnhiylxhr90
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900agqhnhiylxhr90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900agqhnhiylxhr90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900agqhnhiylxhr90
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900agqhnhiylxhr90
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz US Investment Grade Credit

(cbu.ensure
    :name "Allianz US Investment Grade Credit"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900xty5odofjvz671)

(cbu.assign-role
    :cbu-id @cbu_529900xty5odofjvz671
    :entity-id @lei_529900xty5odofjvz671
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xty5odofjvz671
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xty5odofjvz671
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900xty5odofjvz671
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xty5odofjvz671
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz PVK Fonds

(cbu.ensure
    :name "Allianz PVK Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990060xvmdot4s0102)

(cbu.assign-role
    :cbu-id @cbu_52990060xvmdot4s0102
    :entity-id @lei_52990060xvmdot4s0102
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990060xvmdot4s0102
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990060xvmdot4s0102
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990060xvmdot4s0102
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Sirius

(cbu.ensure
    :name "Sirius"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900ufeszw3kh1nz15)

(cbu.assign-role
    :cbu-id @cbu_529900ufeszw3kh1nz15
    :entity-id @lei_529900ufeszw3kh1nz15
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ufeszw3kh1nz15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ufeszw3kh1nz15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900ufeszw3kh1nz15
    :entity-id @lei_5299008fhadwurgmxp80
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ufeszw3kh1nz15
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Kennedy 1

(cbu.ensure
    :name "Kennedy 1"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900cjn5qoc67hdn27)

(cbu.assign-role
    :cbu-id @cbu_529900cjn5qoc67hdn27
    :entity-id @lei_529900cjn5qoc67hdn27
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900cjn5qoc67hdn27
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900cjn5qoc67hdn27
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ PRIVATE MARKETS SCSp SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900cjn5qoc67hdn27
    :entity-id @lei_5299008fhadwurgmxp80
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900cjn5qoc67hdn27
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VG EU PD

(cbu.ensure
    :name "Allianz VG EU PD"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900xur86nww7n7457)

(cbu.assign-role
    :cbu-id @cbu_529900xur86nww7n7457
    :entity-id @lei_529900xur86nww7n7457
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xur86nww7n7457
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xur86nww7n7457
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Real Assets and Private Markets Fund S.A. SICAV-RAIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900xur86nww7n7457
    :entity-id @lei_5299001kbl1hp5t3tj23
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xur86nww7n7457
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VVV

(cbu.ensure
    :name "AllianzGI-Fonds VVV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900dry37s2wjkm906)

(cbu.assign-role
    :cbu-id @cbu_529900dry37s2wjkm906
    :entity-id @lei_529900dry37s2wjkm906
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dry37s2wjkm906
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dry37s2wjkm906
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dry37s2wjkm906
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategic Bond Conservative

(cbu.ensure
    :name "Allianz Strategic Bond Conservative"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900lsh98hdoen1p39)

(cbu.assign-role
    :cbu-id @cbu_529900lsh98hdoen1p39
    :entity-id @lei_529900lsh98hdoen1p39
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900lsh98hdoen1p39
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900lsh98hdoen1p39
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900lsh98hdoen1p39
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900lsh98hdoen1p39
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ SYSTEMATIC ENHANCED US EQUITY

(cbu.ensure
    :name "ALLIANZ SYSTEMATIC ENHANCED US EQUITY"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299002wogp7c2r2fd60)

(cbu.assign-role
    :cbu-id @cbu_5299002wogp7c2r2fd60
    :entity-id @lei_5299002wogp7c2r2fd60
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002wogp7c2r2fd60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002wogp7c2r2fd60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299002wogp7c2r2fd60
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002wogp7c2r2fd60
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus Aerostructures Dachfonds

(cbu.ensure
    :name "Airbus Aerostructures Dachfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990037lrswc0fzp452)

(cbu.assign-role
    :cbu-id @cbu_52990037lrswc0fzp452
    :entity-id @lei_52990037lrswc0fzp452
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990037lrswc0fzp452
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990037lrswc0fzp452
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990037lrswc0fzp452
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFCBUSD3

(cbu.ensure
    :name "AllianzGI-Fonds SFCBUSD3"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900raysemmi7dky56)

(cbu.assign-role
    :cbu-id @cbu_529900raysemmi7dky56
    :entity-id @lei_529900raysemmi7dky56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900raysemmi7dky56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900raysemmi7dky56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900raysemmi7dky56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFCBUSD2

(cbu.ensure
    :name "AllianzGI-Fonds SFCBUSD2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ehx48497ph0236)

(cbu.assign-role
    :cbu-id @cbu_529900ehx48497ph0236
    :entity-id @lei_529900ehx48497ph0236
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ehx48497ph0236
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ehx48497ph0236
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ehx48497ph0236
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFCBUSD1

(cbu.ensure
    :name "AllianzGI-Fonds SFCBUSD1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900dv1c0tvb1hsb06)

(cbu.assign-role
    :cbu-id @cbu_529900dv1c0tvb1hsb06
    :entity-id @lei_529900dv1c0tvb1hsb06
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dv1c0tvb1hsb06
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dv1c0tvb1hsb06
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dv1c0tvb1hsb06
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFCBEUR1

(cbu.ensure
    :name "AllianzGI-Fonds SFCBEUR1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299008vh2nkk5aex340)

(cbu.assign-role
    :cbu-id @cbu_5299008vh2nkk5aex340
    :entity-id @lei_5299008vh2nkk5aex340
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008vh2nkk5aex340
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008vh2nkk5aex340
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008vh2nkk5aex340
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFCBEUR2

(cbu.ensure
    :name "AllianzGI-Fonds SFCBEUR2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900btu9r0jywfi949)

(cbu.assign-role
    :cbu-id @cbu_529900btu9r0jywfi949
    :entity-id @lei_529900btu9r0jywfi949
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900btu9r0jywfi949
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900btu9r0jywfi949
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900btu9r0jywfi949
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFEQEUR1

(cbu.ensure
    :name "AllianzGI-Fonds SFEQEUR1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002kl9879ocdnq53)

(cbu.assign-role
    :cbu-id @cbu_5299002kl9879ocdnq53
    :entity-id @lei_5299002kl9879ocdnq53
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002kl9879ocdnq53
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002kl9879ocdnq53
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002kl9879ocdnq53
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFCBEUR3

(cbu.ensure
    :name "AllianzGI-Fonds SFCBEUR3"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002n1pkhmdsycq81)

(cbu.assign-role
    :cbu-id @cbu_5299002n1pkhmdsycq81
    :entity-id @lei_5299002n1pkhmdsycq81
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002n1pkhmdsycq81
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002n1pkhmdsycq81
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002n1pkhmdsycq81
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ PATRIMONIAL DIVERSIFIE

(cbu.ensure
    :name "ALLIANZ PATRIMONIAL DIVERSIFIE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900g9n3ecmgvavv43)

(cbu.assign-role
    :cbu-id @cbu_529900g9n3ecmgvavv43
    :entity-id @lei_529900g9n3ecmgvavv43
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900g9n3ecmgvavv43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900g9n3ecmgvavv43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900g9n3ecmgvavv43
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AOKNW-AR

(cbu.ensure
    :name "AllianzGI-Fonds AOKNW-AR"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004o4i1hz9yud990)

(cbu.assign-role
    :cbu-id @cbu_5299004o4i1hz9yud990
    :entity-id @lei_5299004o4i1hz9yud990
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004o4i1hz9yud990
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004o4i1hz9yud990
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004o4i1hz9yud990
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus Invest for Life Rentenfonds APP

(cbu.ensure
    :name "Airbus Invest for Life Rentenfonds APP"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900266fn01aarnm32)

(cbu.assign-role
    :cbu-id @cbu_529900266fn01aarnm32
    :entity-id @lei_529900266fn01aarnm32
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900266fn01aarnm32
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900266fn01aarnm32
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900266fn01aarnm32
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds CEC

(cbu.ensure
    :name "AllianzGI-Fonds CEC"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900tgxd6foeys7708)

(cbu.assign-role
    :cbu-id @cbu_529900tgxd6foeys7708
    :entity-id @lei_529900tgxd6foeys7708
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900tgxd6foeys7708
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900tgxd6foeys7708
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900tgxd6foeys7708
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ERAFP MULTI-ACTIFS 2

(cbu.ensure
    :name "ERAFP MULTI-ACTIFS 2"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900vkkuhvzpc9oz90)

(cbu.assign-role
    :cbu-id @cbu_529900vkkuhvzpc9oz90
    :entity-id @lei_529900vkkuhvzpc9oz90
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vkkuhvzpc9oz90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vkkuhvzpc9oz90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vkkuhvzpc9oz90
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Europe Equity powered by Artificial Intelligence

(cbu.ensure
    :name "Allianz Europe Equity powered by Artificial Intelligence"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900jytqgoa1fwyi63)

(cbu.assign-role
    :cbu-id @cbu_529900jytqgoa1fwyi63
    :entity-id @lei_529900jytqgoa1fwyi63
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jytqgoa1fwyi63
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jytqgoa1fwyi63
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900jytqgoa1fwyi63
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jytqgoa1fwyi63
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz US Equity powered by Artificial Intelligence

(cbu.ensure
    :name "Allianz US Equity powered by Artificial Intelligence"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299003lzk5die22v897)

(cbu.assign-role
    :cbu-id @cbu_5299003lzk5die22v897
    :entity-id @lei_5299003lzk5die22v897
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299003lzk5die22v897
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299003lzk5die22v897
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299003lzk5die22v897
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299003lzk5die22v897
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Equity powered by Artificial Intelligence

(cbu.ensure
    :name "Allianz Global Equity powered by Artificial Intelligence"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299008huu0v6ao0kz44)

(cbu.assign-role
    :cbu-id @cbu_5299008huu0v6ao0kz44
    :entity-id @lei_5299008huu0v6ao0kz44
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008huu0v6ao0kz44
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008huu0v6ao0kz44
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299008huu0v6ao0kz44
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008huu0v6ao0kz44
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: money mate defensiv

(cbu.ensure
    :name "money mate defensiv"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299004lacfve3a9ue80)

(cbu.assign-role
    :cbu-id @cbu_5299004lacfve3a9ue80
    :entity-id @lei_5299004lacfve3a9ue80
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004lacfve3a9ue80
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004lacfve3a9ue80
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004lacfve3a9ue80
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: money mate mutig

(cbu.ensure
    :name "money mate mutig"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299000efut8cu0s4g03)

(cbu.assign-role
    :cbu-id @cbu_5299000efut8cu0s4g03
    :entity-id @lei_5299000efut8cu0s4g03
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299000efut8cu0s4g03
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299000efut8cu0s4g03
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299000efut8cu0s4g03
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ GLOBAL HYBRID SECURITIES FUND

(cbu.ensure
    :name "ALLIANZ GLOBAL HYBRID SECURITIES FUND"
    :client-type "FUND"
    :jurisdiction "KY"
    :as @cbu_213800rex1uxmzv4pl75)

(cbu.assign-role
    :cbu-id @cbu_213800rex1uxmzv4pl75
    :entity-id @lei_213800rex1uxmzv4pl75
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_213800rex1uxmzv4pl75
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_213800rex1uxmzv4pl75
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: ALLIANZ MULTI STRATEGIES FUND (umbrella)
(cbu.assign-role
    :cbu-id @cbu_213800rex1uxmzv4pl75
    :entity-id @lei_213800mtidaj6bmv8r40
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_213800rex1uxmzv4pl75
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Ernest

(cbu.ensure
    :name "AllianzGI-Fonds Ernest"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299005cpqpc0ww76836)

(cbu.assign-role
    :cbu-id @cbu_5299005cpqpc0ww76836
    :entity-id @lei_5299005cpqpc0ww76836
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005cpqpc0ww76836
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005cpqpc0ww76836
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005cpqpc0ww76836
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ACTINIUM

(cbu.ensure
    :name "ACTINIUM"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900wcjyhzyib46m51)

(cbu.assign-role
    :cbu-id @cbu_529900wcjyhzyib46m51
    :entity-id @lei_529900wcjyhzyib46m51
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wcjyhzyib46m51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wcjyhzyib46m51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wcjyhzyib46m51
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz France Relance

(cbu.ensure
    :name "Allianz France Relance"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900apvukihetvan54)

(cbu.assign-role
    :cbu-id @cbu_529900apvukihetvan54
    :entity-id @lei_529900apvukihetvan54
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900apvukihetvan54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900apvukihetvan54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900apvukihetvan54
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Dynamic Allocation Plus Equity

(cbu.ensure
    :name "Allianz Dynamic Allocation Plus Equity"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900r0f5aodzyteh16)

(cbu.assign-role
    :cbu-id @cbu_529900r0f5aodzyteh16
    :entity-id @lei_529900r0f5aodzyteh16
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900r0f5aodzyteh16
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900r0f5aodzyteh16
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900r0f5aodzyteh16
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900r0f5aodzyteh16
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Trend and Brands

(cbu.ensure
    :name "Allianz Trend and Brands"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900a66acguduefo55)

(cbu.assign-role
    :cbu-id @cbu_529900a66acguduefo55
    :entity-id @lei_529900a66acguduefo55
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900a66acguduefo55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900a66acguduefo55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900a66acguduefo55
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900a66acguduefo55
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SDK Pensionen

(cbu.ensure
    :name "AllianzGI-Fonds SDK Pensionen"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990069hd3aa39fm024)

(cbu.assign-role
    :cbu-id @cbu_52990069hd3aa39fm024
    :entity-id @lei_52990069hd3aa39fm024
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990069hd3aa39fm024
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990069hd3aa39fm024
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990069hd3aa39fm024
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Surprise

(cbu.ensure
    :name "AllianzGI-Fonds Surprise"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299007rumwkdvjrsv24)

(cbu.assign-role
    :cbu-id @cbu_5299007rumwkdvjrsv24
    :entity-id @lei_5299007rumwkdvjrsv24
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007rumwkdvjrsv24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007rumwkdvjrsv24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007rumwkdvjrsv24
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Advanced Fixed Income Euro Aggregate

(cbu.ensure
    :name "Allianz Advanced Fixed Income Euro Aggregate"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299002bk0gl27v7ro88)

(cbu.assign-role
    :cbu-id @cbu_5299002bk0gl27v7ro88
    :entity-id @lei_5299002bk0gl27v7ro88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002bk0gl27v7ro88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002bk0gl27v7ro88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002bk0gl27v7ro88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds THV RG

(cbu.ensure
    :name "AllianzGI-Fonds THV RG"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299001z1s34evx7qv51)

(cbu.assign-role
    :cbu-id @cbu_5299001z1s34evx7qv51
    :entity-id @lei_5299001z1s34evx7qv51
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299001z1s34evx7qv51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299001z1s34evx7qv51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299001z1s34evx7qv51
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Scout24

(cbu.ensure
    :name "AllianzGI-Fonds Scout24"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900qbhfv9pibe2v65)

(cbu.assign-role
    :cbu-id @cbu_529900qbhfv9pibe2v65
    :entity-id @lei_529900qbhfv9pibe2v65
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900qbhfv9pibe2v65
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900qbhfv9pibe2v65
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900qbhfv9pibe2v65
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Selective Ownership 2

(cbu.ensure
    :name "AllianzGI-Fonds Selective Ownership 2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990081g1e8d6ekcj14)

(cbu.assign-role
    :cbu-id @cbu_52990081g1e8d6ekcj14
    :entity-id @lei_52990081g1e8d6ekcj14
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990081g1e8d6ekcj14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990081g1e8d6ekcj14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990081g1e8d6ekcj14
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allvest Active Invest

(cbu.ensure
    :name "Allvest Active Invest"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_549300vfu8kqqt7oxt28)

(cbu.assign-role
    :cbu-id @cbu_549300vfu8kqqt7oxt28
    :entity-id @lei_549300vfu8kqqt7oxt28
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300vfu8kqqt7oxt28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300vfu8kqqt7oxt28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Allvest Invest SICAV-SIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_549300vfu8kqqt7oxt28
    :entity-id @lei_2549002s5rohuygfvt38
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300vfu8kqqt7oxt28
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allvest Passive Invest

(cbu.ensure
    :name "Allvest Passive Invest"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5493009k3lx6km2x7p46)

(cbu.assign-role
    :cbu-id @cbu_5493009k3lx6km2x7p46
    :entity-id @lei_5493009k3lx6km2x7p46
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493009k3lx6km2x7p46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493009k3lx6km2x7p46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Allvest Invest SICAV-SIF (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5493009k3lx6km2x7p46
    :entity-id @lei_2549002s5rohuygfvt38
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493009k3lx6km2x7p46
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds OLB Pensionen

(cbu.ensure
    :name "AllianzGI-Fonds OLB Pensionen"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990025l9dp1wmjic93)

(cbu.assign-role
    :cbu-id @cbu_52990025l9dp1wmjic93
    :entity-id @lei_52990025l9dp1wmjic93
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990025l9dp1wmjic93
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990025l9dp1wmjic93
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990025l9dp1wmjic93
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz ADAC AV Fonds

(cbu.ensure
    :name "Allianz ADAC AV Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900rl1be88xt0y715)

(cbu.assign-role
    :cbu-id @cbu_529900rl1be88xt0y715
    :entity-id @lei_529900rl1be88xt0y715
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rl1be88xt0y715
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rl1be88xt0y715
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rl1be88xt0y715
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds STG-Pensions

(cbu.ensure
    :name "AllianzGI-Fonds STG-Pensions"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900w707jzw77ss292)

(cbu.assign-role
    :cbu-id @cbu_529900w707jzw77ss292
    :entity-id @lei_529900w707jzw77ss292
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900w707jzw77ss292
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900w707jzw77ss292
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900w707jzw77ss292
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Impact Green Bond

(cbu.ensure
    :name "Allianz Impact Green Bond"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_52990099kfo3imclyj15)

(cbu.assign-role
    :cbu-id @cbu_52990099kfo3imclyj15
    :entity-id @lei_52990099kfo3imclyj15
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990099kfo3imclyj15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990099kfo3imclyj15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990099kfo3imclyj15
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DID

(cbu.ensure
    :name "AllianzGI-Fonds DID"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xe8ngii96y4811)

(cbu.assign-role
    :cbu-id @cbu_529900xe8ngii96y4811
    :entity-id @lei_529900xe8ngii96y4811
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xe8ngii96y4811
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xe8ngii96y4811
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xe8ngii96y4811
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-S Aktien

(cbu.ensure
    :name "AllianzGI-S Aktien"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900snajqrqjuwmi06)

(cbu.assign-role
    :cbu-id @cbu_529900snajqrqjuwmi06
    :entity-id @lei_529900snajqrqjuwmi06
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900snajqrqjuwmi06
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900snajqrqjuwmi06
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900snajqrqjuwmi06
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-S Anleihen IG

(cbu.ensure
    :name "AllianzGI-S Anleihen IG"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900q3qynhx3s3nv90)

(cbu.assign-role
    :cbu-id @cbu_529900q3qynhx3s3nv90
    :entity-id @lei_529900q3qynhx3s3nv90
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900q3qynhx3s3nv90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900q3qynhx3s3nv90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900q3qynhx3s3nv90
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VO93 E+H

(cbu.ensure
    :name "AllianzGI-Fonds VO93 E+H"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900gdcke823v5ge96)

(cbu.assign-role
    :cbu-id @cbu_529900gdcke823v5ge96
    :entity-id @lei_529900gdcke823v5ge96
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gdcke823v5ge96
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gdcke823v5ge96
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gdcke823v5ge96
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz SE Ashmore Emerging Markets Corporates Fund

(cbu.ensure
    :name "Allianz SE Ashmore Emerging Markets Corporates Fund"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900gkunmxq7k4c094)

(cbu.assign-role
    :cbu-id @cbu_529900gkunmxq7k4c094
    :entity-id @lei_529900gkunmxq7k4c094
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gkunmxq7k4c094
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gkunmxq7k4c094
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gkunmxq7k4c094
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Pet and Animal Wellbeing

(cbu.ensure
    :name "Allianz Pet and Animal Wellbeing"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900wr8ulgafrmos18)

(cbu.assign-role
    :cbu-id @cbu_529900wr8ulgafrmos18
    :entity-id @lei_529900wr8ulgafrmos18
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wr8ulgafrmos18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wr8ulgafrmos18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900wr8ulgafrmos18
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wr8ulgafrmos18
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Ex Euro Corporate Bonds

(cbu.ensure
    :name "AllianzGI-Fonds Ex Euro Corporate Bonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299001kv55ph739cp54)

(cbu.assign-role
    :cbu-id @cbu_5299001kv55ph739cp54
    :entity-id @lei_5299001kv55ph739cp54
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299001kv55ph739cp54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299001kv55ph739cp54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299001kv55ph739cp54
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz German Small and Micro Cap

(cbu.ensure
    :name "Allianz German Small and Micro Cap"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299003trg3ztp6ma754)

(cbu.assign-role
    :cbu-id @cbu_5299003trg3ztp6ma754
    :entity-id @lei_5299003trg3ztp6ma754
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299003trg3ztp6ma754
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299003trg3ztp6ma754
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5299003trg3ztp6ma754
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299003trg3ztp6ma754
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Pfizer Sérénité

(cbu.ensure
    :name "Pfizer Sérénité"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900zj9bhlf4cpom48)

(cbu.assign-role
    :cbu-id @cbu_529900zj9bhlf4cpom48
    :entity-id @lei_529900zj9bhlf4cpom48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zj9bhlf4cpom48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zj9bhlf4cpom48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zj9bhlf4cpom48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: 117 EURO ST

(cbu.ensure
    :name "117 EURO ST"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900gsog8efkzz4691)

(cbu.assign-role
    :cbu-id @cbu_529900gsog8efkzz4691
    :entity-id @lei_529900gsog8efkzz4691
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gsog8efkzz4691
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gsog8efkzz4691
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gsog8efkzz4691
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PencAbbV Pensions

(cbu.ensure
    :name "AllianzGI-Fonds PencAbbV Pensions"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900c177b10rzk1y35)

(cbu.assign-role
    :cbu-id @cbu_529900c177b10rzk1y35
    :entity-id @lei_529900c177b10rzk1y35
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900c177b10rzk1y35
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900c177b10rzk1y35
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900c177b10rzk1y35
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Luna A

(cbu.ensure
    :name "AllianzGI-Fonds Luna A"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990044p8fdv0mhzl09)

(cbu.assign-role
    :cbu-id @cbu_52990044p8fdv0mhzl09
    :entity-id @lei_52990044p8fdv0mhzl09
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990044p8fdv0mhzl09
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990044p8fdv0mhzl09
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990044p8fdv0mhzl09
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Luna B

(cbu.ensure
    :name "AllianzGI-Fonds Luna B"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900sohaup55s8pu50)

(cbu.assign-role
    :cbu-id @cbu_529900sohaup55s8pu50
    :entity-id @lei_529900sohaup55s8pu50
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900sohaup55s8pu50
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900sohaup55s8pu50
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900sohaup55s8pu50
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DEAL

(cbu.ensure
    :name "AllianzGI-Fonds DEAL"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990015bzvbf05jwn74)

(cbu.assign-role
    :cbu-id @cbu_52990015bzvbf05jwn74
    :entity-id @lei_52990015bzvbf05jwn74
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990015bzvbf05jwn74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990015bzvbf05jwn74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990015bzvbf05jwn74
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds OB Pension

(cbu.ensure
    :name "AllianzGI-Fonds OB Pension"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900fi2f4fbt09d708)

(cbu.assign-role
    :cbu-id @cbu_529900fi2f4fbt09d708
    :entity-id @lei_529900fi2f4fbt09d708
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900fi2f4fbt09d708
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900fi2f4fbt09d708
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900fi2f4fbt09d708
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VIE Multi-Assets

(cbu.ensure
    :name "Allianz VIE Multi-Assets"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900k3ony5lwzoha27)

(cbu.assign-role
    :cbu-id @cbu_529900k3ony5lwzoha27
    :entity-id @lei_529900k3ony5lwzoha27
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900k3ony5lwzoha27
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900k3ony5lwzoha27
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900k3ony5lwzoha27
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus Invest for Life Aktienfonds 2

(cbu.ensure
    :name "Airbus Invest for Life Aktienfonds 2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900wgpuijhvk37680)

(cbu.assign-role
    :cbu-id @cbu_529900wgpuijhvk37680
    :entity-id @lei_529900wgpuijhvk37680
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wgpuijhvk37680
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wgpuijhvk37680
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wgpuijhvk37680
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Selective Ownership

(cbu.ensure
    :name "AllianzGI-Fonds Selective Ownership"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004eo0jh52d3hp18)

(cbu.assign-role
    :cbu-id @cbu_5299004eo0jh52d3hp18
    :entity-id @lei_5299004eo0jh52d3hp18
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004eo0jh52d3hp18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004eo0jh52d3hp18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004eo0jh52d3hp18
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GEPAG

(cbu.ensure
    :name "AllianzGI-Fonds GEPAG"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900gxq1kdumlk2763)

(cbu.assign-role
    :cbu-id @cbu_529900gxq1kdumlk2763
    :entity-id @lei_529900gxq1kdumlk2763
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gxq1kdumlk2763
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gxq1kdumlk2763
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gxq1kdumlk2763
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Bremen

(cbu.ensure
    :name "AllianzGI-Fonds Bremen"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900qrokyif91oq895)

(cbu.assign-role
    :cbu-id @cbu_529900qrokyif91oq895
    :entity-id @lei_529900qrokyif91oq895
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900qrokyif91oq895
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900qrokyif91oq895
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900qrokyif91oq895
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VE USA

(cbu.ensure
    :name "AllianzGI-Fonds VE USA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300w4ea022bh9wi74)

(cbu.assign-role
    :cbu-id @cbu_549300w4ea022bh9wi74
    :entity-id @lei_549300w4ea022bh9wi74
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300w4ea022bh9wi74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300w4ea022bh9wi74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300w4ea022bh9wi74
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VE Global

(cbu.ensure
    :name "AllianzGI-Fonds VE Global"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300j9tb79ydgm9643)

(cbu.assign-role
    :cbu-id @cbu_549300j9tb79ydgm9643
    :entity-id @lei_549300j9tb79ydgm9643
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300j9tb79ydgm9643
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300j9tb79ydgm9643
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300j9tb79ydgm9643
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Thematica

(cbu.ensure
    :name "Allianz Thematica"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5493004zrv2css15yf05)

(cbu.assign-role
    :cbu-id @cbu_5493004zrv2css15yf05
    :entity-id @lei_5493004zrv2css15yf05
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493004zrv2css15yf05
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493004zrv2css15yf05
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_5493004zrv2css15yf05
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493004zrv2css15yf05
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds LBS SHH bAV

(cbu.ensure
    :name "AllianzGI-Fonds LBS SHH bAV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300k6grj0776jii48)

(cbu.assign-role
    :cbu-id @cbu_549300k6grj0776jii48
    :entity-id @lei_549300k6grj0776jii48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300k6grj0776jii48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300k6grj0776jii48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300k6grj0776jii48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MPF1

(cbu.ensure
    :name "AllianzGI-Fonds MPF1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493008e4udyden16u62)

(cbu.assign-role
    :cbu-id @cbu_5493008e4udyden16u62
    :entity-id @lei_5493008e4udyden16u62
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493008e4udyden16u62
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493008e4udyden16u62
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493008e4udyden16u62
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Ircantec Crédit Euro AGI

(cbu.ensure
    :name "Ircantec Crédit Euro AGI"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_549300khm7o2ml2uuk42)

(cbu.assign-role
    :cbu-id @cbu_549300khm7o2ml2uuk42
    :entity-id @lei_549300khm7o2ml2uuk42
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300khm7o2ml2uuk42
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300khm7o2ml2uuk42
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300khm7o2ml2uuk42
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Beilstein-Institut

(cbu.ensure
    :name "AllianzGI-Fonds Beilstein-Institut"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300sq6eepi62h7p52)

(cbu.assign-role
    :cbu-id @cbu_549300sq6eepi62h7p52
    :entity-id @lei_549300sq6eepi62h7p52
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300sq6eepi62h7p52
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300sq6eepi62h7p52
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300sq6eepi62h7p52
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds RHM01

(cbu.ensure
    :name "AllianzGI-Fonds RHM01"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930068huqppwvuht61)

(cbu.assign-role
    :cbu-id @cbu_54930068huqppwvuht61
    :entity-id @lei_54930068huqppwvuht61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930068huqppwvuht61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930068huqppwvuht61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930068huqppwvuht61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: VermögensManagement RenditeStars

(cbu.ensure
    :name "VermögensManagement RenditeStars"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5493000ghh35ftfcm044)

(cbu.assign-role
    :cbu-id @cbu_5493000ghh35ftfcm044
    :entity-id @lei_5493000ghh35ftfcm044
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493000ghh35ftfcm044
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493000ghh35ftfcm044
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493000ghh35ftfcm044
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds NASPA Pensionsfonds

(cbu.ensure
    :name "AllianzGI-Fonds NASPA Pensionsfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493001b4yyb4rutnv40)

(cbu.assign-role
    :cbu-id @cbu_5493001b4yyb4rutnv40
    :entity-id @lei_5493001b4yyb4rutnv40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493001b4yyb4rutnv40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493001b4yyb4rutnv40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493001b4yyb4rutnv40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz V-PD Fonds

(cbu.ensure
    :name "Allianz V-PD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493001l0cq83s70cz91)

(cbu.assign-role
    :cbu-id @cbu_5493001l0cq83s70cz91
    :entity-id @lei_5493001l0cq83s70cz91
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493001l0cq83s70cz91
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493001l0cq83s70cz91
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493001l0cq83s70cz91
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz SE-PD Fonds

(cbu.ensure
    :name "Allianz SE-PD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300cvt30fx9p97463)

(cbu.assign-role
    :cbu-id @cbu_549300cvt30fx9p97463
    :entity-id @lei_549300cvt30fx9p97463
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300cvt30fx9p97463
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300cvt30fx9p97463
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300cvt30fx9p97463
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz PK-PD Fonds

(cbu.ensure
    :name "Allianz PK-PD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493006gp001sqrod821)

(cbu.assign-role
    :cbu-id @cbu_5493006gp001sqrod821
    :entity-id @lei_5493006gp001sqrod821
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493006gp001sqrod821
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493006gp001sqrod821
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493006gp001sqrod821
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BRML

(cbu.ensure
    :name "AllianzGI-Fonds BRML"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300axu3hu4r5ve652)

(cbu.assign-role
    :cbu-id @cbu_549300axu3hu4r5ve652
    :entity-id @lei_549300axu3hu4r5ve652
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300axu3hu4r5ve652
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300axu3hu4r5ve652
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300axu3hu4r5ve652
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz L-PD Fonds

(cbu.ensure
    :name "Allianz L-PD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300kg4rwkwuy6nt58)

(cbu.assign-role
    :cbu-id @cbu_549300kg4rwkwuy6nt58
    :entity-id @lei_549300kg4rwkwuy6nt58
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300kg4rwkwuy6nt58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300kg4rwkwuy6nt58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300kg4rwkwuy6nt58
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz PKV-PD Fonds

(cbu.ensure
    :name "Allianz PKV-PD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300zjfqic44oi6t88)

(cbu.assign-role
    :cbu-id @cbu_549300zjfqic44oi6t88
    :entity-id @lei_549300zjfqic44oi6t88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300zjfqic44oi6t88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300zjfqic44oi6t88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300zjfqic44oi6t88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ACH

(cbu.ensure
    :name "AllianzGI-Fonds ACH"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493000of8lyllus4j50)

(cbu.assign-role
    :cbu-id @cbu_5493000of8lyllus4j50
    :entity-id @lei_5493000of8lyllus4j50
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493000of8lyllus4j50
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493000of8lyllus4j50
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493000of8lyllus4j50
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Vermögensbildung Deutschland

(cbu.ensure
    :name "Allianz Vermögensbildung Deutschland"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300hrnnec3vbqw438)

(cbu.assign-role
    :cbu-id @cbu_549300hrnnec3vbqw438
    :entity-id @lei_549300hrnnec3vbqw438
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300hrnnec3vbqw438
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300hrnnec3vbqw438
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300hrnnec3vbqw438
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Wachstum Euroland

(cbu.ensure
    :name "Allianz Wachstum Euroland"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300f0gr1n43bzw173)

(cbu.assign-role
    :cbu-id @cbu_549300f0gr1n43bzw173
    :entity-id @lei_549300f0gr1n43bzw173
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300f0gr1n43bzw173
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300f0gr1n43bzw173
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300f0gr1n43bzw173
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Wachstum Europa

(cbu.ensure
    :name "Allianz Wachstum Europa"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493005n3wexi56si903)

(cbu.assign-role
    :cbu-id @cbu_5493005n3wexi56si903
    :entity-id @lei_5493005n3wexi56si903
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493005n3wexi56si903
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493005n3wexi56si903
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493005n3wexi56si903
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Vermögensbildung Europa

(cbu.ensure
    :name "Allianz Vermögensbildung Europa"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493002em7xlapruvz50)

(cbu.assign-role
    :cbu-id @cbu_5493002em7xlapruvz50
    :entity-id @lei_5493002em7xlapruvz50
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493002em7xlapruvz50
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493002em7xlapruvz50
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493002em7xlapruvz50
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Informationstechnologie

(cbu.ensure
    :name "Allianz Informationstechnologie"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300y3aa9us1u3pt72)

(cbu.assign-role
    :cbu-id @cbu_549300y3aa9us1u3pt72
    :entity-id @lei_549300y3aa9us1u3pt72
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300y3aa9us1u3pt72
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300y3aa9us1u3pt72
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300y3aa9us1u3pt72
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Salzgitter

(cbu.ensure
    :name "AllianzGI-Fonds Salzgitter"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493003zujstl7ez1u78)

(cbu.assign-role
    :cbu-id @cbu_5493003zujstl7ez1u78
    :entity-id @lei_5493003zujstl7ez1u78
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493003zujstl7ez1u78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493003zujstl7ez1u78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493003zujstl7ez1u78
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PKBMA

(cbu.ensure
    :name "AllianzGI-Fonds PKBMA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493001nh47xgbtzw868)

(cbu.assign-role
    :cbu-id @cbu_5493001nh47xgbtzw868
    :entity-id @lei_5493001nh47xgbtzw868
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493001nh47xgbtzw868
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493001nh47xgbtzw868
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493001nh47xgbtzw868
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds RP APNUS

(cbu.ensure
    :name "AllianzGI-Fonds RP APNUS"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300zd2wj3k48jyu88)

(cbu.assign-role
    :cbu-id @cbu_549300zd2wj3k48jyu88
    :entity-id @lei_549300zd2wj3k48jyu88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300zd2wj3k48jyu88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300zd2wj3k48jyu88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300zd2wj3k48jyu88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: SK Themen

(cbu.ensure
    :name "SK Themen"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_222100jgdwhcu45ikt89)

(cbu.assign-role
    :cbu-id @cbu_222100jgdwhcu45ikt89
    :entity-id @lei_222100jgdwhcu45ikt89
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_222100jgdwhcu45ikt89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_222100jgdwhcu45ikt89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_222100jgdwhcu45ikt89
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: SK Welt

(cbu.ensure
    :name "SK Welt"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_2221001oqje2ef46yl59)

(cbu.assign-role
    :cbu-id @cbu_2221001oqje2ef46yl59
    :entity-id @lei_2221001oqje2ef46yl59
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_2221001oqje2ef46yl59
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_2221001oqje2ef46yl59
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_2221001oqje2ef46yl59
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: SK Europa

(cbu.ensure
    :name "SK Europa"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_2221008lnlxd5g7q2g15)

(cbu.assign-role
    :cbu-id @cbu_2221008lnlxd5g7q2g15
    :entity-id @lei_2221008lnlxd5g7q2g15
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_2221008lnlxd5g7q2g15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_2221008lnlxd5g7q2g15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_2221008lnlxd5g7q2g15
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CKA Renten

(cbu.ensure
    :name "CKA Renten"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930058nz8wp1lkon22)

(cbu.assign-role
    :cbu-id @cbu_54930058nz8wp1lkon22
    :entity-id @lei_54930058nz8wp1lkon22
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930058nz8wp1lkon22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930058nz8wp1lkon22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930058nz8wp1lkon22
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Securicash SRI

(cbu.ensure
    :name "Allianz Securicash SRI"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_549300f44vv2mmks9707)

(cbu.assign-role
    :cbu-id @cbu_549300f44vv2mmks9707
    :entity-id @lei_549300f44vv2mmks9707
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300f44vv2mmks9707
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300f44vv2mmks9707
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300f44vv2mmks9707
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Euro Oblig Court Terme ISR

(cbu.ensure
    :name "Allianz Euro Oblig Court Terme ISR"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_549300pgxl5gtmg8pc85)

(cbu.assign-role
    :cbu-id @cbu_549300pgxl5gtmg8pc85
    :entity-id @lei_549300pgxl5gtmg8pc85
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300pgxl5gtmg8pc85
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300pgxl5gtmg8pc85
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300pgxl5gtmg8pc85
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Europazins

(cbu.ensure
    :name "Allianz Europazins"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300pejyhx4wa43i14)

(cbu.assign-role
    :cbu-id @cbu_549300pejyhx4wa43i14
    :entity-id @lei_549300pejyhx4wa43i14
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300pejyhx4wa43i14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300pejyhx4wa43i14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300pejyhx4wa43i14
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Biotechnologie

(cbu.ensure
    :name "Allianz Biotechnologie"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493003a7rkulrch1976)

(cbu.assign-role
    :cbu-id @cbu_5493003a7rkulrch1976
    :entity-id @lei_5493003a7rkulrch1976
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493003a7rkulrch1976
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493003a7rkulrch1976
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493003a7rkulrch1976
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF5

(cbu.ensure
    :name "AllianzGI-Fonds MAF5"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299007llqhdkj8kbc33)

(cbu.assign-role
    :cbu-id @cbu_5299007llqhdkj8kbc33
    :entity-id @lei_5299007llqhdkj8kbc33
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007llqhdkj8kbc33
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007llqhdkj8kbc33
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007llqhdkj8kbc33
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI PIMCO Euro Covered Bonds - gleitend 10 J.

(cbu.ensure
    :name "AllianzGI PIMCO Euro Covered Bonds - gleitend 10 J."
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990059rkn0rq20ho78)

(cbu.assign-role
    :cbu-id @cbu_52990059rkn0rq20ho78
    :entity-id @lei_52990059rkn0rq20ho78
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990059rkn0rq20ho78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990059rkn0rq20ho78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990059rkn0rq20ho78
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds CPT2

(cbu.ensure
    :name "AllianzGI-Fonds CPT2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900cci0wf7cwgxv82)

(cbu.assign-role
    :cbu-id @cbu_529900cci0wf7cwgxv82
    :entity-id @lei_529900cci0wf7cwgxv82
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900cci0wf7cwgxv82
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900cci0wf7cwgxv82
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900cci0wf7cwgxv82
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ABCO III

(cbu.ensure
    :name "AllianzGI-Fonds ABCO III"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900dfjdfbaopoig76)

(cbu.assign-role
    :cbu-id @cbu_529900dfjdfbaopoig76
    :entity-id @lei_529900dfjdfbaopoig76
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dfjdfbaopoig76
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dfjdfbaopoig76
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dfjdfbaopoig76
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz DLVR Fonds

(cbu.ensure
    :name "Allianz DLVR Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299005errlfdf1iwt25)

(cbu.assign-role
    :cbu-id @cbu_5299005errlfdf1iwt25
    :entity-id @lei_5299005errlfdf1iwt25
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005errlfdf1iwt25
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005errlfdf1iwt25
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005errlfdf1iwt25
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: VermögensManagement Stars of Multi Asset

(cbu.ensure
    :name "VermögensManagement Stars of Multi Asset"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900iyhaoppkt61430)

(cbu.assign-role
    :cbu-id @cbu_529900iyhaoppkt61430
    :entity-id @lei_529900iyhaoppkt61430
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900iyhaoppkt61430
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900iyhaoppkt61430
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900iyhaoppkt61430
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AZRE AZD P&C Master Fund

(cbu.ensure
    :name "AZRE AZD P&C Master Fund"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900b5a2dwme31c402)

(cbu.assign-role
    :cbu-id @cbu_529900b5a2dwme31c402
    :entity-id @lei_529900b5a2dwme31c402
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900b5a2dwme31c402
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900b5a2dwme31c402
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900b5a2dwme31c402
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds LANXESS Pension Trust 1

(cbu.ensure
    :name "AllianzGI-Fonds LANXESS Pension Trust 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900np5h3tn7p1hd24)

(cbu.assign-role
    :cbu-id @cbu_529900np5h3tn7p1hd24
    :entity-id @lei_529900np5h3tn7p1hd24
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900np5h3tn7p1hd24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900np5h3tn7p1hd24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900np5h3tn7p1hd24
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Re Asia PIMCO USD Fund

(cbu.ensure
    :name "Allianz Re Asia PIMCO USD Fund"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900uznp8ydqegym98)

(cbu.assign-role
    :cbu-id @cbu_529900uznp8ydqegym98
    :entity-id @lei_529900uznp8ydqegym98
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900uznp8ydqegym98
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900uznp8ydqegym98
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900uznp8ydqegym98
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ELA-Fonds

(cbu.ensure
    :name "ELA-Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990076kq6rlycuo006)

(cbu.assign-role
    :cbu-id @cbu_52990076kq6rlycuo006
    :entity-id @lei_52990076kq6rlycuo006
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990076kq6rlycuo006
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990076kq6rlycuo006
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990076kq6rlycuo006
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: HAMELIN MULTI-ACTIFS

(cbu.ensure
    :name "HAMELIN MULTI-ACTIFS"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_969500bqtwy38rzjca39)

(cbu.assign-role
    :cbu-id @cbu_969500bqtwy38rzjca39
    :entity-id @lei_969500bqtwy38rzjca39
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_969500bqtwy38rzjca39
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_969500bqtwy38rzjca39
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_969500bqtwy38rzjca39
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: PFIZER - Pfizer Moyen Terme

(cbu.ensure
    :name "PFIZER - Pfizer Moyen Terme"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900duou39lfai2u90)

(cbu.assign-role
    :cbu-id @cbu_529900duou39lfai2u90
    :entity-id @lei_529900duou39lfai2u90
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900duou39lfai2u90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900duou39lfai2u90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900duou39lfai2u90
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Dynamic Commodities

(cbu.ensure
    :name "Allianz Dynamic Commodities"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900ubjtrbkw4w3m49)

(cbu.assign-role
    :cbu-id @cbu_529900ubjtrbkw4w3m49
    :entity-id @lei_529900ubjtrbkw4w3m49
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ubjtrbkw4w3m49
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ubjtrbkw4w3m49
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz Global Investors Fund (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900ubjtrbkw4w3m49
    :entity-id @lei_4kt8dcrlarep7c35mw05
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ubjtrbkw4w3m49
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Degussa Trust e.V.

(cbu.ensure
    :name "AllianzGI-Fonds Degussa Trust e.V."
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900sc3jw2o3lhhw25)

(cbu.assign-role
    :cbu-id @cbu_529900sc3jw2o3lhhw25
    :entity-id @lei_529900sc3jw2o3lhhw25
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900sc3jw2o3lhhw25
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900sc3jw2o3lhhw25
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900sc3jw2o3lhhw25
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CDI-Cofonds I

(cbu.ensure
    :name "CDI-Cofonds I"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xmt6zlwl059j27)

(cbu.assign-role
    :cbu-id @cbu_529900xmt6zlwl059j27
    :entity-id @lei_529900xmt6zlwl059j27
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xmt6zlwl059j27
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xmt6zlwl059j27
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xmt6zlwl059j27
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds TOSCA

(cbu.ensure
    :name "AllianzGI-Fonds TOSCA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900wvd0mbz1in2017)

(cbu.assign-role
    :cbu-id @cbu_529900wvd0mbz1in2017
    :entity-id @lei_529900wvd0mbz1in2017
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wvd0mbz1in2017
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wvd0mbz1in2017
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wvd0mbz1in2017
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CDP-Cofonds

(cbu.ensure
    :name "CDP-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900aeqpnxuyg0ae43)

(cbu.assign-role
    :cbu-id @cbu_529900aeqpnxuyg0ae43
    :entity-id @lei_529900aeqpnxuyg0ae43
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900aeqpnxuyg0ae43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900aeqpnxuyg0ae43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900aeqpnxuyg0ae43
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GBG

(cbu.ensure
    :name "AllianzGI-Fonds GBG"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900m8dwiwu75vrl48)

(cbu.assign-role
    :cbu-id @cbu_529900m8dwiwu75vrl48
    :entity-id @lei_529900m8dwiwu75vrl48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900m8dwiwu75vrl48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900m8dwiwu75vrl48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900m8dwiwu75vrl48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KUF

(cbu.ensure
    :name "AllianzGI-Fonds KUF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299008ikq5ge1phsg06)

(cbu.assign-role
    :cbu-id @cbu_5299008ikq5ge1phsg06
    :entity-id @lei_5299008ikq5ge1phsg06
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008ikq5ge1phsg06
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008ikq5ge1phsg06
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008ikq5ge1phsg06
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds H-KGaA-Bonds

(cbu.ensure
    :name "AllianzGI-Fonds H-KGaA-Bonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900uxc9fimp4sby55)

(cbu.assign-role
    :cbu-id @cbu_529900uxc9fimp4sby55
    :entity-id @lei_529900uxc9fimp4sby55
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900uxc9fimp4sby55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900uxc9fimp4sby55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900uxc9fimp4sby55
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus ATZ Dachfonds

(cbu.ensure
    :name "Airbus ATZ Dachfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900moc271q3lhrt16)

(cbu.assign-role
    :cbu-id @cbu_529900moc271q3lhrt16
    :entity-id @lei_529900moc271q3lhrt16
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900moc271q3lhrt16
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900moc271q3lhrt16
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900moc271q3lhrt16
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF-L

(cbu.ensure
    :name "AllianzGI-Fonds MAF-L"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xazloi53ws7a56)

(cbu.assign-role
    :cbu-id @cbu_529900xazloi53ws7a56
    :entity-id @lei_529900xazloi53ws7a56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xazloi53ws7a56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xazloi53ws7a56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xazloi53ws7a56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Innovation Souveraineté Européenne

(cbu.ensure
    :name "Allianz Innovation Souveraineté Européenne"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_5299005u1yhdk4d6rp66)

(cbu.assign-role
    :cbu-id @cbu_5299005u1yhdk4d6rp66
    :entity-id @lei_5299005u1yhdk4d6rp66
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005u1yhdk4d6rp66
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005u1yhdk4d6rp66
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005u1yhdk4d6rp66
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VMN II

(cbu.ensure
    :name "AllianzGI-Fonds VMN II"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004r2i18ybble490)

(cbu.assign-role
    :cbu-id @cbu_5299004r2i18ybble490
    :entity-id @lei_5299004r2i18ybble490
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004r2i18ybble490
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004r2i18ybble490
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004r2i18ybble490
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DD I

(cbu.ensure
    :name "AllianzGI-Fonds DD I"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900jcxd3v2bm9ev71)

(cbu.assign-role
    :cbu-id @cbu_529900jcxd3v2bm9ev71
    :entity-id @lei_529900jcxd3v2bm9ev71
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jcxd3v2bm9ev71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jcxd3v2bm9ev71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jcxd3v2bm9ev71
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MSSD

(cbu.ensure
    :name "AllianzGI-Fonds MSSD"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900jgbwda5h318y94)

(cbu.assign-role
    :cbu-id @cbu_529900jgbwda5h318y94
    :entity-id @lei_529900jgbwda5h318y94
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jgbwda5h318y94
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jgbwda5h318y94
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jgbwda5h318y94
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: EURO-Cofonds

(cbu.ensure
    :name "EURO-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900j1r9k61cy95u65)

(cbu.assign-role
    :cbu-id @cbu_529900j1r9k61cy95u65
    :entity-id @lei_529900j1r9k61cy95u65
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900j1r9k61cy95u65
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900j1r9k61cy95u65
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900j1r9k61cy95u65
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AOKNW-BM

(cbu.ensure
    :name "AllianzGI-Fonds AOKNW-BM"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990022h26jlqn68j60)

(cbu.assign-role
    :cbu-id @cbu_52990022h26jlqn68j60
    :entity-id @lei_52990022h26jlqn68j60
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990022h26jlqn68j60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990022h26jlqn68j60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990022h26jlqn68j60
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AOK-Cofonds

(cbu.ensure
    :name "AOK-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xp0ru01azto252)

(cbu.assign-role
    :cbu-id @cbu_529900xp0ru01azto252
    :entity-id @lei_529900xp0ru01azto252
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xp0ru01azto252
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xp0ru01azto252
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xp0ru01azto252
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds pca-bau

(cbu.ensure
    :name "AllianzGI-Fonds pca-bau"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900oifsym6oahaw73)

(cbu.assign-role
    :cbu-id @cbu_529900oifsym6oahaw73
    :entity-id @lei_529900oifsym6oahaw73
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900oifsym6oahaw73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900oifsym6oahaw73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900oifsym6oahaw73
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF2

(cbu.ensure
    :name "AllianzGI-Fonds MAF2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900z8p9h4yiaaki23)

(cbu.assign-role
    :cbu-id @cbu_529900z8p9h4yiaaki23
    :entity-id @lei_529900z8p9h4yiaaki23
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900z8p9h4yiaaki23
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900z8p9h4yiaaki23
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900z8p9h4yiaaki23
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus SIKO Dachfonds

(cbu.ensure
    :name "Airbus SIKO Dachfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900zylvv3xs8ofl49)

(cbu.assign-role
    :cbu-id @cbu_529900zylvv3xs8ofl49
    :entity-id @lei_529900zylvv3xs8ofl49
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zylvv3xs8ofl49
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zylvv3xs8ofl49
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zylvv3xs8ofl49
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DIN

(cbu.ensure
    :name "AllianzGI-Fonds DIN"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990026bc9272iwog38)

(cbu.assign-role
    :cbu-id @cbu_52990026bc9272iwog38
    :entity-id @lei_52990026bc9272iwog38
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990026bc9272iwog38
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990026bc9272iwog38
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990026bc9272iwog38
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds RBB

(cbu.ensure
    :name "AllianzGI-Fonds RBB"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002qyv8dq0o5f173)

(cbu.assign-role
    :cbu-id @cbu_5299002qyv8dq0o5f173
    :entity-id @lei_5299002qyv8dq0o5f173
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002qyv8dq0o5f173
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002qyv8dq0o5f173
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002qyv8dq0o5f173
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds WAF

(cbu.ensure
    :name "AllianzGI-Fonds WAF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900kisvgc4skd0492)

(cbu.assign-role
    :cbu-id @cbu_529900kisvgc4skd0492
    :entity-id @lei_529900kisvgc4skd0492
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900kisvgc4skd0492
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900kisvgc4skd0492
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900kisvgc4skd0492
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus Invest for Life Rentenfonds kurz

(cbu.ensure
    :name "Airbus Invest for Life Rentenfonds kurz"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ke3ubj5twcfj23)

(cbu.assign-role
    :cbu-id @cbu_529900ke3ubj5twcfj23
    :entity-id @lei_529900ke3ubj5twcfj23
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ke3ubj5twcfj23
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ke3ubj5twcfj23
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ke3ubj5twcfj23
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Elbe Flugzeugwerke Dachfonds

(cbu.ensure
    :name "Elbe Flugzeugwerke Dachfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004nh57nxa6mqr45)

(cbu.assign-role
    :cbu-id @cbu_5299004nh57nxa6mqr45
    :entity-id @lei_5299004nh57nxa6mqr45
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004nh57nxa6mqr45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004nh57nxa6mqr45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004nh57nxa6mqr45
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Master DRT

(cbu.ensure
    :name "AllianzGI-Fonds Master DRT"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900dsypv8ylk7ql92)

(cbu.assign-role
    :cbu-id @cbu_529900dsypv8ylk7ql92
    :entity-id @lei_529900dsypv8ylk7ql92
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dsypv8ylk7ql92
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dsypv8ylk7ql92
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dsypv8ylk7ql92
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategy 50

(cbu.ensure
    :name "Allianz Strategy 50"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900u565tvtirhj104)

(cbu.assign-role
    :cbu-id @cbu_529900u565tvtirhj104
    :entity-id @lei_529900u565tvtirhj104
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900u565tvtirhj104
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900u565tvtirhj104
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz European Pension Investments (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900u565tvtirhj104
    :entity-id @lei_5299000s45hp7b90zb16
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900u565tvtirhj104
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-E

(cbu.ensure
    :name "AllianzGI-E"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299000mjebu65djgx45)

(cbu.assign-role
    :cbu-id @cbu_5299000mjebu65djgx45
    :entity-id @lei_5299000mjebu65djgx45
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299000mjebu65djgx45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299000mjebu65djgx45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299000mjebu65djgx45
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ STRATEGY 75

(cbu.ensure
    :name "ALLIANZ STRATEGY 75"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900589oy2g0cvot53)

(cbu.assign-role
    :cbu-id @cbu_529900589oy2g0cvot53
    :entity-id @lei_529900589oy2g0cvot53
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900589oy2g0cvot53
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900589oy2g0cvot53
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; SICAV: Allianz European Pension Investments (umbrella)
(cbu.assign-role
    :cbu-id @cbu_529900589oy2g0cvot53
    :entity-id @lei_5299000s45hp7b90zb16
    :role "SICAV")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900589oy2g0cvot53
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus Helicopters Deutschland Dachfonds

(cbu.ensure
    :name "Airbus Helicopters Deutschland Dachfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900rxr81sm6mdji41)

(cbu.assign-role
    :cbu-id @cbu_529900rxr81sm6mdji41
    :entity-id @lei_529900rxr81sm6mdji41
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rxr81sm6mdji41
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rxr81sm6mdji41
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rxr81sm6mdji41
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus Dachfonds

(cbu.ensure
    :name "Airbus Dachfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299003tmlus0se2hg89)

(cbu.assign-role
    :cbu-id @cbu_5299003tmlus0se2hg89
    :entity-id @lei_5299003tmlus0se2hg89
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299003tmlus0se2hg89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299003tmlus0se2hg89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299003tmlus0se2hg89
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz PV-WS Fonds

(cbu.ensure
    :name "Allianz PV-WS Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900h2y17b1lib6z90)

(cbu.assign-role
    :cbu-id @cbu_529900h2y17b1lib6z90
    :entity-id @lei_529900h2y17b1lib6z90
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900h2y17b1lib6z90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900h2y17b1lib6z90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900h2y17b1lib6z90
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ALLIANZ MONETAIRE

(cbu.ensure
    :name "ALLIANZ MONETAIRE"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900tigc4ubscof384)

(cbu.assign-role
    :cbu-id @cbu_529900tigc4ubscof384
    :entity-id @lei_529900tigc4ubscof384
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900tigc4ubscof384
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900tigc4ubscof384
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900tigc4ubscof384
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: KomfortDynamik Sondervermögen

(cbu.ensure
    :name "KomfortDynamik Sondervermögen"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900qb1u2u45oud544)

(cbu.assign-role
    :cbu-id @cbu_529900qb1u2u45oud544
    :entity-id @lei_529900qb1u2u45oud544
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900qb1u2u45oud544
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900qb1u2u45oud544
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900qb1u2u45oud544
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategie 2031 Plus

(cbu.ensure
    :name "Allianz Strategie 2031 Plus"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990053appi4khk3j88)

(cbu.assign-role
    :cbu-id @cbu_52990053appi4khk3j88
    :entity-id @lei_52990053appi4khk3j88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990053appi4khk3j88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990053appi4khk3j88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990053appi4khk3j88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ZDD3

(cbu.ensure
    :name "AllianzGI-Fonds ZDD3"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900yhsv3xc5tvho63)

(cbu.assign-role
    :cbu-id @cbu_529900yhsv3xc5tvho63
    :entity-id @lei_529900yhsv3xc5tvho63
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900yhsv3xc5tvho63
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900yhsv3xc5tvho63
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900yhsv3xc5tvho63
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ESMT

(cbu.ensure
    :name "AllianzGI-Fonds ESMT"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900p7ulflarewnr54)

(cbu.assign-role
    :cbu-id @cbu_529900p7ulflarewnr54
    :entity-id @lei_529900p7ulflarewnr54
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900p7ulflarewnr54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900p7ulflarewnr54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900p7ulflarewnr54
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DEW-Co

(cbu.ensure
    :name "AllianzGI-Fonds DEW-Co"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900gpmz8mh65h0y58)

(cbu.assign-role
    :cbu-id @cbu_529900gpmz8mh65h0y58
    :entity-id @lei_529900gpmz8mh65h0y58
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900gpmz8mh65h0y58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900gpmz8mh65h0y58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900gpmz8mh65h0y58
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds UGF

(cbu.ensure
    :name "AllianzGI-Fonds UGF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900x52dtp1ck80o56)

(cbu.assign-role
    :cbu-id @cbu_529900x52dtp1ck80o56
    :entity-id @lei_529900x52dtp1ck80o56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900x52dtp1ck80o56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900x52dtp1ck80o56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900x52dtp1ck80o56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DSW-Co

(cbu.ensure
    :name "AllianzGI-Fonds DSW-Co"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299008ta33fc7tuo729)

(cbu.assign-role
    :cbu-id @cbu_5299008ta33fc7tuo729
    :entity-id @lei_5299008ta33fc7tuo729
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008ta33fc7tuo729
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008ta33fc7tuo729
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008ta33fc7tuo729
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PSDN

(cbu.ensure
    :name "AllianzGI-Fonds PSDN"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900s5hyehz7ts9r61)

(cbu.assign-role
    :cbu-id @cbu_529900s5hyehz7ts9r61
    :entity-id @lei_529900s5hyehz7ts9r61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900s5hyehz7ts9r61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900s5hyehz7ts9r61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900s5hyehz7ts9r61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: PremiumStars Wachstum

(cbu.ensure
    :name "PremiumStars Wachstum"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900eorrodr5pxwy88)

(cbu.assign-role
    :cbu-id @cbu_529900eorrodr5pxwy88
    :entity-id @lei_529900eorrodr5pxwy88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900eorrodr5pxwy88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900eorrodr5pxwy88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900eorrodr5pxwy88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds LTSA

(cbu.ensure
    :name "AllianzGI-Fonds LTSA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900vq0wm4jjm9v953)

(cbu.assign-role
    :cbu-id @cbu_529900vq0wm4jjm9v953
    :entity-id @lei_529900vq0wm4jjm9v953
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vq0wm4jjm9v953
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vq0wm4jjm9v953
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vq0wm4jjm9v953
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAV

(cbu.ensure
    :name "AllianzGI-Fonds MAV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299006szmwr0gk6yr22)

(cbu.assign-role
    :cbu-id @cbu_5299006szmwr0gk6yr22
    :entity-id @lei_5299006szmwr0gk6yr22
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299006szmwr0gk6yr22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299006szmwr0gk6yr22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299006szmwr0gk6yr22
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: LHCO-Fonds

(cbu.ensure
    :name "LHCO-Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299006yj3i94ib6ex97)

(cbu.assign-role
    :cbu-id @cbu_5299006yj3i94ib6ex97
    :entity-id @lei_5299006yj3i94ib6ex97
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299006yj3i94ib6ex97
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299006yj3i94ib6ex97
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299006yj3i94ib6ex97
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds FIB

(cbu.ensure
    :name "AllianzGI-Fonds FIB"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900m2aaykfo7uza31)

(cbu.assign-role
    :cbu-id @cbu_529900m2aaykfo7uza31
    :entity-id @lei_529900m2aaykfo7uza31
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900m2aaykfo7uza31
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900m2aaykfo7uza31
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900m2aaykfo7uza31
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Lipco III

(cbu.ensure
    :name "AllianzGI-Fonds Lipco III"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009qj6w4qhjtzm78)

(cbu.assign-role
    :cbu-id @cbu_5299009qj6w4qhjtzm78
    :entity-id @lei_5299009qj6w4qhjtzm78
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009qj6w4qhjtzm78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009qj6w4qhjtzm78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009qj6w4qhjtzm78
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: SVCO III-Fonds

(cbu.ensure
    :name "SVCO III-Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900uodscj4avuvg63)

(cbu.assign-role
    :cbu-id @cbu_529900uodscj4avuvg63
    :entity-id @lei_529900uodscj4avuvg63
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900uodscj4avuvg63
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900uodscj4avuvg63
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900uodscj4avuvg63
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: BAT-Cofonds

(cbu.ensure
    :name "BAT-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004ydzrtqi6gvu40)

(cbu.assign-role
    :cbu-id @cbu_5299004ydzrtqi6gvu40
    :entity-id @lei_5299004ydzrtqi6gvu40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004ydzrtqi6gvu40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004ydzrtqi6gvu40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004ydzrtqi6gvu40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KDCO

(cbu.ensure
    :name "AllianzGI-Fonds KDCO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900tlurpfcd42q108)

(cbu.assign-role
    :cbu-id @cbu_529900tlurpfcd42q108
    :entity-id @lei_529900tlurpfcd42q108
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900tlurpfcd42q108
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900tlurpfcd42q108
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900tlurpfcd42q108
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BSAF

(cbu.ensure
    :name "AllianzGI-Fonds BSAF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900yz0m6877wkyq88)

(cbu.assign-role
    :cbu-id @cbu_529900yz0m6877wkyq88
    :entity-id @lei_529900yz0m6877wkyq88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900yz0m6877wkyq88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900yz0m6877wkyq88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900yz0m6877wkyq88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GdP

(cbu.ensure
    :name "AllianzGI-Fonds GdP"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009lee7k3a8duv60)

(cbu.assign-role
    :cbu-id @cbu_5299009lee7k3a8duv60
    :entity-id @lei_5299009lee7k3a8duv60
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009lee7k3a8duv60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009lee7k3a8duv60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009lee7k3a8duv60
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DHCO

(cbu.ensure
    :name "AllianzGI-Fonds DHCO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900bym03u5n34qq89)

(cbu.assign-role
    :cbu-id @cbu_529900bym03u5n34qq89
    :entity-id @lei_529900bym03u5n34qq89
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900bym03u5n34qq89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900bym03u5n34qq89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900bym03u5n34qq89
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ELK-Cofonds

(cbu.ensure
    :name "ELK-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900rccd73q1zfhe76)

(cbu.assign-role
    :cbu-id @cbu_529900rccd73q1zfhe76
    :entity-id @lei_529900rccd73q1zfhe76
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rccd73q1zfhe76
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rccd73q1zfhe76
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rccd73q1zfhe76
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SFT2

(cbu.ensure
    :name "AllianzGI-Fonds SFT2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ebqxcs1rv51m18)

(cbu.assign-role
    :cbu-id @cbu_529900ebqxcs1rv51m18
    :entity-id @lei_529900ebqxcs1rv51m18
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ebqxcs1rv51m18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ebqxcs1rv51m18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ebqxcs1rv51m18
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BMS

(cbu.ensure
    :name "AllianzGI-Fonds BMS"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ofnlix58pr0b55)

(cbu.assign-role
    :cbu-id @cbu_529900ofnlix58pr0b55
    :entity-id @lei_529900ofnlix58pr0b55
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ofnlix58pr0b55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ofnlix58pr0b55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ofnlix58pr0b55
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CONVEST 21 VL

(cbu.ensure
    :name "CONVEST 21 VL"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990002v1vxkptbbd46)

(cbu.assign-role
    :cbu-id @cbu_52990002v1vxkptbbd46
    :entity-id @lei_52990002v1vxkptbbd46
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990002v1vxkptbbd46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990002v1vxkptbbd46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990002v1vxkptbbd46
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Dietzb

(cbu.ensure
    :name "AllianzGI-Fonds Dietzb"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900wwqk94r17ckt46)

(cbu.assign-role
    :cbu-id @cbu_529900wwqk94r17ckt46
    :entity-id @lei_529900wwqk94r17ckt46
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wwqk94r17ckt46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wwqk94r17ckt46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wwqk94r17ckt46
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PVT

(cbu.ensure
    :name "AllianzGI-Fonds PVT"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900360wuob94x5369)

(cbu.assign-role
    :cbu-id @cbu_529900360wuob94x5369
    :entity-id @lei_529900360wuob94x5369
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900360wuob94x5369
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900360wuob94x5369
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900360wuob94x5369
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Fondra

(cbu.ensure
    :name "Fondra"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002dvsvxrcb3bs68)

(cbu.assign-role
    :cbu-id @cbu_5299002dvsvxrcb3bs68
    :entity-id @lei_5299002dvsvxrcb3bs68
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002dvsvxrcb3bs68
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002dvsvxrcb3bs68
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002dvsvxrcb3bs68
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds EJS Stiftungsfonds

(cbu.ensure
    :name "AllianzGI-Fonds EJS Stiftungsfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ppv5981wymng95)

(cbu.assign-role
    :cbu-id @cbu_529900ppv5981wymng95
    :entity-id @lei_529900ppv5981wymng95
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ppv5981wymng95
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ppv5981wymng95
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ppv5981wymng95
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: VermögensManagement Stabilität

(cbu.ensure
    :name "VermögensManagement Stabilität"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900rol8r94h3yxp57)

(cbu.assign-role
    :cbu-id @cbu_529900rol8r94h3yxp57
    :entity-id @lei_529900rol8r94h3yxp57
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rol8r94h3yxp57
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rol8r94h3yxp57
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rol8r94h3yxp57
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Süwe

(cbu.ensure
    :name "AllianzGI-Fonds Süwe"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299007j2wj8mlrcy643)

(cbu.assign-role
    :cbu-id @cbu_5299007j2wj8mlrcy643
    :entity-id @lei_5299007j2wj8mlrcy643
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007j2wj8mlrcy643
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007j2wj8mlrcy643
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007j2wj8mlrcy643
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-H

(cbu.ensure
    :name "AllianzGI-H"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900scyav02ryzjf69)

(cbu.assign-role
    :cbu-id @cbu_529900scyav02ryzjf69
    :entity-id @lei_529900scyav02ryzjf69
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900scyav02ryzjf69
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900scyav02ryzjf69
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900scyav02ryzjf69
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds FS Pension

(cbu.ensure
    :name "AllianzGI-Fonds FS Pension"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009sxmbbm9k57231)

(cbu.assign-role
    :cbu-id @cbu_5299009sxmbbm9k57231
    :entity-id @lei_5299009sxmbbm9k57231
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009sxmbbm9k57231
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009sxmbbm9k57231
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009sxmbbm9k57231
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds CT-DRAECO

(cbu.ensure
    :name "AllianzGI-Fonds CT-DRAECO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ge17xo5taidg51)

(cbu.assign-role
    :cbu-id @cbu_529900ge17xo5taidg51
    :entity-id @lei_529900ge17xo5taidg51
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ge17xo5taidg51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ge17xo5taidg51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ge17xo5taidg51
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: PremiumStars Chance

(cbu.ensure
    :name "PremiumStars Chance"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299005vnuff0i1p9068)

(cbu.assign-role
    :cbu-id @cbu_5299005vnuff0i1p9068
    :entity-id @lei_5299005vnuff0i1p9068
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005vnuff0i1p9068
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005vnuff0i1p9068
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005vnuff0i1p9068
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: KD-Cofonds

(cbu.ensure
    :name "KD-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ctwdgm8pyt2g43)

(cbu.assign-role
    :cbu-id @cbu_529900ctwdgm8pyt2g43
    :entity-id @lei_529900ctwdgm8pyt2g43
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ctwdgm8pyt2g43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ctwdgm8pyt2g43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ctwdgm8pyt2g43
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SCHLUCO

(cbu.ensure
    :name "AllianzGI-Fonds SCHLUCO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299008kcxoj7io95x34)

(cbu.assign-role
    :cbu-id @cbu_5299008kcxoj7io95x34
    :entity-id @lei_5299008kcxoj7io95x34
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008kcxoj7io95x34
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008kcxoj7io95x34
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008kcxoj7io95x34
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GUV

(cbu.ensure
    :name "AllianzGI-Fonds GUV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900imumrs4sylz250)

(cbu.assign-role
    :cbu-id @cbu_529900imumrs4sylz250
    :entity-id @lei_529900imumrs4sylz250
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900imumrs4sylz250
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900imumrs4sylz250
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900imumrs4sylz250
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BG RCI

(cbu.ensure
    :name "AllianzGI-Fonds BG RCI"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299003ebxjdy9bwz516)

(cbu.assign-role
    :cbu-id @cbu_5299003ebxjdy9bwz516
    :entity-id @lei_5299003ebxjdy9bwz516
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299003ebxjdy9bwz516
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299003ebxjdy9bwz516
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299003ebxjdy9bwz516
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds WERT

(cbu.ensure
    :name "AllianzGI-Fonds WERT"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900088z4vxzee8r51)

(cbu.assign-role
    :cbu-id @cbu_529900088z4vxzee8r51
    :entity-id @lei_529900088z4vxzee8r51
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900088z4vxzee8r51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900088z4vxzee8r51
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900088z4vxzee8r51
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: BGV-Masterfonds

(cbu.ensure
    :name "BGV-Masterfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xj1fig6e2sg912)

(cbu.assign-role
    :cbu-id @cbu_529900xj1fig6e2sg912
    :entity-id @lei_529900xj1fig6e2sg912
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xj1fig6e2sg912
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xj1fig6e2sg912
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xj1fig6e2sg912
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GEW

(cbu.ensure
    :name "AllianzGI-Fonds GEW"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299006762rv2j611g05)

(cbu.assign-role
    :cbu-id @cbu_5299006762rv2j611g05
    :entity-id @lei_5299006762rv2j611g05
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299006762rv2j611g05
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299006762rv2j611g05
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299006762rv2j611g05
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI- Fonds DPF Dillinger Pensionsfonds

(cbu.ensure
    :name "AllianzGI- Fonds DPF Dillinger Pensionsfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900wp83t353r8p197)

(cbu.assign-role
    :cbu-id @cbu_529900wp83t353r8p197
    :entity-id @lei_529900wp83t353r8p197
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900wp83t353r8p197
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900wp83t353r8p197
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900wp83t353r8p197
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: NUERNBERGER Euroland A

(cbu.ensure
    :name "NUERNBERGER Euroland A"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299000jwxhbq3gd4c12)

(cbu.assign-role
    :cbu-id @cbu_5299000jwxhbq3gd4c12
    :entity-id @lei_5299000jwxhbq3gd4c12
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299000jwxhbq3gd4c12
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299000jwxhbq3gd4c12
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299000jwxhbq3gd4c12
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Fondak

(cbu.ensure
    :name "Fondak"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ugqi1mkhihv006)

(cbu.assign-role
    :cbu-id @cbu_529900ugqi1mkhihv006
    :entity-id @lei_529900ugqi1mkhihv006
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ugqi1mkhihv006
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ugqi1mkhihv006
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ugqi1mkhihv006
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Multi Manager Global Balanced

(cbu.ensure
    :name "Allianz Multi Manager Global Balanced"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900vfxa5h49kv6v41)

(cbu.assign-role
    :cbu-id @cbu_529900vfxa5h49kv6v41
    :entity-id @lei_529900vfxa5h49kv6v41
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vfxa5h49kv6v41
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vfxa5h49kv6v41
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vfxa5h49kv6v41
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF1

(cbu.ensure
    :name "AllianzGI-Fonds MAF1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900p56ostx568p297)

(cbu.assign-role
    :cbu-id @cbu_529900p56ostx568p297
    :entity-id @lei_529900p56ostx568p297
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900p56ostx568p297
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900p56ostx568p297
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900p56ostx568p297
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Interglobal

(cbu.ensure
    :name "Allianz Interglobal"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990091ayont72him61)

(cbu.assign-role
    :cbu-id @cbu_52990091ayont72him61
    :entity-id @lei_52990091ayont72him61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990091ayont72him61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990091ayont72him61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990091ayont72him61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds OJU

(cbu.ensure
    :name "AllianzGI-Fonds OJU"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900soxlump8s0t658)

(cbu.assign-role
    :cbu-id @cbu_529900soxlump8s0t658
    :entity-id @lei_529900soxlump8s0t658
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900soxlump8s0t658
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900soxlump8s0t658
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900soxlump8s0t658
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: BGO-Cofonds

(cbu.ensure
    :name "BGO-Cofonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ofkunyu2ogc602)

(cbu.assign-role
    :cbu-id @cbu_529900ofkunyu2ogc602
    :entity-id @lei_529900ofkunyu2ogc602
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ofkunyu2ogc602
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ofkunyu2ogc602
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ofkunyu2ogc602
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SiV

(cbu.ensure
    :name "AllianzGI-Fonds SiV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299005hirh5b12fye11)

(cbu.assign-role
    :cbu-id @cbu_5299005hirh5b12fye11
    :entity-id @lei_5299005hirh5b12fye11
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005hirh5b12fye11
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005hirh5b12fye11
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005hirh5b12fye11
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VBDK

(cbu.ensure
    :name "AllianzGI-Fonds VBDK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900u7d6lirgghn979)

(cbu.assign-role
    :cbu-id @cbu_529900u7d6lirgghn979
    :entity-id @lei_529900u7d6lirgghn979
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900u7d6lirgghn979
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900u7d6lirgghn979
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900u7d6lirgghn979
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PHCO

(cbu.ensure
    :name "AllianzGI-Fonds PHCO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900n7g3ulsew9wu15)

(cbu.assign-role
    :cbu-id @cbu_529900n7g3ulsew9wu15
    :entity-id @lei_529900n7g3ulsew9wu15
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900n7g3ulsew9wu15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900n7g3ulsew9wu15
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900n7g3ulsew9wu15
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds INDU

(cbu.ensure
    :name "AllianzGI-Fonds INDU"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299003ypniv8cqxfy82)

(cbu.assign-role
    :cbu-id @cbu_5299003ypniv8cqxfy82
    :entity-id @lei_5299003ypniv8cqxfy82
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299003ypniv8cqxfy82
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299003ypniv8cqxfy82
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299003ypniv8cqxfy82
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Mesco

(cbu.ensure
    :name "AllianzGI-Fonds Mesco"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990034ydi7e37xd396)

(cbu.assign-role
    :cbu-id @cbu_52990034ydi7e37xd396
    :entity-id @lei_52990034ydi7e37xd396
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990034ydi7e37xd396
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990034ydi7e37xd396
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990034ydi7e37xd396
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Alpen

(cbu.ensure
    :name "AllianzGI-Fonds Alpen"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990084aj38ijutfu22)

(cbu.assign-role
    :cbu-id @cbu_52990084aj38ijutfu22
    :entity-id @lei_52990084aj38ijutfu22
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990084aj38ijutfu22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990084aj38ijutfu22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990084aj38ijutfu22
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Plusfonds

(cbu.ensure
    :name "Plusfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299001rd7zqvo3ixi69)

(cbu.assign-role
    :cbu-id @cbu_5299001rd7zqvo3ixi69
    :entity-id @lei_5299001rd7zqvo3ixi69
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299001rd7zqvo3ixi69
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299001rd7zqvo3ixi69
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299001rd7zqvo3ixi69
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Ukah

(cbu.ensure
    :name "AllianzGI-Fonds Ukah"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299007pwt44di9o7o29)

(cbu.assign-role
    :cbu-id @cbu_5299007pwt44di9o7o29
    :entity-id @lei_5299007pwt44di9o7o29
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007pwt44di9o7o29
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007pwt44di9o7o29
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007pwt44di9o7o29
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Adifonds

(cbu.ensure
    :name "Allianz Adifonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004ej7sr98tbv869)

(cbu.assign-role
    :cbu-id @cbu_5299004ej7sr98tbv869
    :entity-id @lei_5299004ej7sr98tbv869
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004ej7sr98tbv869
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004ej7sr98tbv869
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004ej7sr98tbv869
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Adiverba

(cbu.ensure
    :name "Allianz Adiverba"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900nflzpcpigink31)

(cbu.assign-role
    :cbu-id @cbu_529900nflzpcpigink31
    :entity-id @lei_529900nflzpcpigink31
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900nflzpcpigink31
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900nflzpcpigink31
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900nflzpcpigink31
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Fondis

(cbu.ensure
    :name "Fondis"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900pccxo63yvphu80)

(cbu.assign-role
    :cbu-id @cbu_529900pccxo63yvphu80
    :entity-id @lei_529900pccxo63yvphu80
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900pccxo63yvphu80
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900pccxo63yvphu80
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900pccxo63yvphu80
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PRI

(cbu.ensure
    :name "AllianzGI-Fonds PRI"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900jyvt8871lj1056)

(cbu.assign-role
    :cbu-id @cbu_529900jyvt8871lj1056
    :entity-id @lei_529900jyvt8871lj1056
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jyvt8871lj1056
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jyvt8871lj1056
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jyvt8871lj1056
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AKT-E

(cbu.ensure
    :name "AllianzGI-Fonds AKT-E"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299005rxh9b59hl3g64)

(cbu.assign-role
    :cbu-id @cbu_5299005rxh9b59hl3g64
    :entity-id @lei_5299005rxh9b59hl3g64
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005rxh9b59hl3g64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005rxh9b59hl3g64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005rxh9b59hl3g64
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BOGESTRA

(cbu.ensure
    :name "AllianzGI-Fonds BOGESTRA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900w0wzm6zalrox07)

(cbu.assign-role
    :cbu-id @cbu_529900w0wzm6zalrox07
    :entity-id @lei_529900w0wzm6zalrox07
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900w0wzm6zalrox07
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900w0wzm6zalrox07
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900w0wzm6zalrox07
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz SOA Fonds

(cbu.ensure
    :name "Allianz SOA Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900rlo7es96hdjn72)

(cbu.assign-role
    :cbu-id @cbu_529900rlo7es96hdjn72
    :entity-id @lei_529900rlo7es96hdjn72
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rlo7es96hdjn72
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rlo7es96hdjn72
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rlo7es96hdjn72
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz EEE Fonds

(cbu.ensure
    :name "Allianz EEE Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002yemgsridvk953)

(cbu.assign-role
    :cbu-id @cbu_5299002yemgsridvk953
    :entity-id @lei_5299002yemgsridvk953
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002yemgsridvk953
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002yemgsridvk953
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002yemgsridvk953
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Gano

(cbu.ensure
    :name "AllianzGI-Fonds Gano"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900qv6muuf99l7d71)

(cbu.assign-role
    :cbu-id @cbu_529900qv6muuf99l7d71
    :entity-id @lei_529900qv6muuf99l7d71
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900qv6muuf99l7d71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900qv6muuf99l7d71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900qv6muuf99l7d71
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BGHW

(cbu.ensure
    :name "AllianzGI-Fonds BGHW"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900opf7duyxdc4t42)

(cbu.assign-role
    :cbu-id @cbu_529900opf7duyxdc4t42
    :entity-id @lei_529900opf7duyxdc4t42
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900opf7duyxdc4t42
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900opf7duyxdc4t42
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900opf7duyxdc4t42
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fonds Japan

(cbu.ensure
    :name "Allianz Fonds Japan"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900zlow5p1nsyo135)

(cbu.assign-role
    :cbu-id @cbu_529900zlow5p1nsyo135
    :entity-id @lei_529900zlow5p1nsyo135
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zlow5p1nsyo135
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zlow5p1nsyo135
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zlow5p1nsyo135
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AMAG 2

(cbu.ensure
    :name "AllianzGI-Fonds AMAG 2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900o1215sbishvt23)

(cbu.assign-role
    :cbu-id @cbu_529900o1215sbishvt23
    :entity-id @lei_529900o1215sbishvt23
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900o1215sbishvt23
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900o1215sbishvt23
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900o1215sbishvt23
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SRP

(cbu.ensure
    :name "AllianzGI-Fonds SRP"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ah9ai7c2ue7c86)

(cbu.assign-role
    :cbu-id @cbu_529900ah9ai7c2ue7c86
    :entity-id @lei_529900ah9ai7c2ue7c86
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ah9ai7c2ue7c86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ah9ai7c2ue7c86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ah9ai7c2ue7c86
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Dunhill

(cbu.ensure
    :name "AllianzGI-Fonds Dunhill"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900uxcfsj30ndzj43)

(cbu.assign-role
    :cbu-id @cbu_529900uxcfsj30ndzj43
    :entity-id @lei_529900uxcfsj30ndzj43
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900uxcfsj30ndzj43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900uxcfsj30ndzj43
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900uxcfsj30ndzj43
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Grillparzer

(cbu.ensure
    :name "AllianzGI-Fonds Grillparzer"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009uqfedbj9a7b90)

(cbu.assign-role
    :cbu-id @cbu_5299009uqfedbj9a7b90
    :entity-id @lei_5299009uqfedbj9a7b90
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009uqfedbj9a7b90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009uqfedbj9a7b90
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009uqfedbj9a7b90
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DC 1

(cbu.ensure
    :name "AllianzGI-Fonds DC 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900idj96jk5aa5d14)

(cbu.assign-role
    :cbu-id @cbu_529900idj96jk5aa5d14
    :entity-id @lei_529900idj96jk5aa5d14
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900idj96jk5aa5d14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900idj96jk5aa5d14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900idj96jk5aa5d14
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategiefonds Wachstum

(cbu.ensure
    :name "Allianz Strategiefonds Wachstum"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900f69yctzau4hy44)

(cbu.assign-role
    :cbu-id @cbu_529900f69yctzau4hy44
    :entity-id @lei_529900f69yctzau4hy44
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900f69yctzau4hy44
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900f69yctzau4hy44
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900f69yctzau4hy44
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fondsvorsorge 1957-1966

(cbu.ensure
    :name "Allianz Fondsvorsorge 1957-1966"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009009c8zngtx548)

(cbu.assign-role
    :cbu-id @cbu_5299009009c8zngtx548
    :entity-id @lei_5299009009c8zngtx548
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009009c8zngtx548
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009009c8zngtx548
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009009c8zngtx548
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz US Large Cap Growth

(cbu.ensure
    :name "Allianz US Large Cap Growth"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ki3kyvidtgub13)

(cbu.assign-role
    :cbu-id @cbu_529900ki3kyvidtgub13
    :entity-id @lei_529900ki3kyvidtgub13
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ki3kyvidtgub13
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ki3kyvidtgub13
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ki3kyvidtgub13
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategiefonds Balance

(cbu.ensure
    :name "Allianz Strategiefonds Balance"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900o77qs2rdjcix58)

(cbu.assign-role
    :cbu-id @cbu_529900o77qs2rdjcix58
    :entity-id @lei_529900o77qs2rdjcix58
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900o77qs2rdjcix58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900o77qs2rdjcix58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900o77qs2rdjcix58
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fondsvorsorge 1952-1956

(cbu.ensure
    :name "Allianz Fondsvorsorge 1952-1956"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990045v3079daq1841)

(cbu.assign-role
    :cbu-id @cbu_52990045v3079daq1841
    :entity-id @lei_52990045v3079daq1841
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990045v3079daq1841
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990045v3079daq1841
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990045v3079daq1841
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Equity Dividend

(cbu.ensure
    :name "Allianz Global Equity Dividend"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900vrvenofl7ggl10)

(cbu.assign-role
    :cbu-id @cbu_529900vrvenofl7ggl10
    :entity-id @lei_529900vrvenofl7ggl10
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vrvenofl7ggl10
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vrvenofl7ggl10
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vrvenofl7ggl10
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fonds Schweiz

(cbu.ensure
    :name "Allianz Fonds Schweiz"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ncqy88f9bu4b07)

(cbu.assign-role
    :cbu-id @cbu_529900ncqy88f9bu4b07
    :entity-id @lei_529900ncqy88f9bu4b07
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ncqy88f9bu4b07
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ncqy88f9bu4b07
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ncqy88f9bu4b07
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Industria

(cbu.ensure
    :name "Industria"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299007ta5nyyk85lg87)

(cbu.assign-role
    :cbu-id @cbu_5299007ta5nyyk85lg87
    :entity-id @lei_5299007ta5nyyk85lg87
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007ta5nyyk85lg87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007ta5nyyk85lg87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007ta5nyyk85lg87
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fondsvorsorge 1977-1996

(cbu.ensure
    :name "Allianz Fondsvorsorge 1977-1996"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299007kgbhs0kk5rr64)

(cbu.assign-role
    :cbu-id @cbu_5299007kgbhs0kk5rr64
    :entity-id @lei_5299007kgbhs0kk5rr64
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299007kgbhs0kk5rr64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299007kgbhs0kk5rr64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299007kgbhs0kk5rr64
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fondsvorsorge 1947-1951

(cbu.ensure
    :name "Allianz Fondsvorsorge 1947-1951"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ttjjqaza84x387)

(cbu.assign-role
    :cbu-id @cbu_529900ttjjqaza84x387
    :entity-id @lei_529900ttjjqaza84x387
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ttjjqaza84x387
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ttjjqaza84x387
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ttjjqaza84x387
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Thesaurus

(cbu.ensure
    :name "Allianz Thesaurus"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900t7ex4cwne7lz52)

(cbu.assign-role
    :cbu-id @cbu_529900t7ex4cwne7lz52
    :entity-id @lei_529900t7ex4cwne7lz52
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900t7ex4cwne7lz52
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900t7ex4cwne7lz52
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900t7ex4cwne7lz52
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Flexi Rentenfonds

(cbu.ensure
    :name "Allianz Flexi Rentenfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299000zlx6w8s070p45)

(cbu.assign-role
    :cbu-id @cbu_5299000zlx6w8s070p45
    :entity-id @lei_5299000zlx6w8s070p45
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299000zlx6w8s070p45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299000zlx6w8s070p45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299000zlx6w8s070p45
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategiefonds Stabilität

(cbu.ensure
    :name "Allianz Strategiefonds Stabilität"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900sbl8ufqqdo9o87)

(cbu.assign-role
    :cbu-id @cbu_529900sbl8ufqqdo9o87
    :entity-id @lei_529900sbl8ufqqdo9o87
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900sbl8ufqqdo9o87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900sbl8ufqqdo9o87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900sbl8ufqqdo9o87
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Fondsvorsorge 1967-1976

(cbu.ensure
    :name "Allianz Fondsvorsorge 1967-1976"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900vd8izwgyd78932)

(cbu.assign-role
    :cbu-id @cbu_529900vd8izwgyd78932
    :entity-id @lei_529900vd8izwgyd78932
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vd8izwgyd78932
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vd8izwgyd78932
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vd8izwgyd78932
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Rohstofffonds

(cbu.ensure
    :name "Allianz Rohstofffonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900587tkm066gll47)

(cbu.assign-role
    :cbu-id @cbu_529900587tkm066gll47
    :entity-id @lei_529900587tkm066gll47
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900587tkm066gll47
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900587tkm066gll47
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900587tkm066gll47
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Nebenwerte Deutschland

(cbu.ensure
    :name "Allianz Nebenwerte Deutschland"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900hgakclboeetg65)

(cbu.assign-role
    :cbu-id @cbu_529900hgakclboeetg65
    :entity-id @lei_529900hgakclboeetg65
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900hgakclboeetg65
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900hgakclboeetg65
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900hgakclboeetg65
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI- Fonds Stiftungsfonds Wissenschaft

(cbu.ensure
    :name "AllianzGI- Fonds Stiftungsfonds Wissenschaft"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990065nti60ck2re67)

(cbu.assign-role
    :cbu-id @cbu_52990065nti60ck2re67
    :entity-id @lei_52990065nti60ck2re67
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990065nti60ck2re67
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990065nti60ck2re67
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990065nti60ck2re67
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds TOB

(cbu.ensure
    :name "AllianzGI-Fonds TOB"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900j7wt0eysthd675)

(cbu.assign-role
    :cbu-id @cbu_529900j7wt0eysthd675
    :entity-id @lei_529900j7wt0eysthd675
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900j7wt0eysthd675
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900j7wt0eysthd675
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900j7wt0eysthd675
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Krebshilfe-2-Fonds

(cbu.ensure
    :name "Krebshilfe-2-Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002ptunxfbbv3780)

(cbu.assign-role
    :cbu-id @cbu_5299002ptunxfbbv3780
    :entity-id @lei_5299002ptunxfbbv3780
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002ptunxfbbv3780
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002ptunxfbbv3780
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002ptunxfbbv3780
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SRF

(cbu.ensure
    :name "AllianzGI-Fonds SRF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009ottel5lc54h88)

(cbu.assign-role
    :cbu-id @cbu_5299009ottel5lc54h88
    :entity-id @lei_5299009ottel5lc54h88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009ottel5lc54h88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009ottel5lc54h88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009ottel5lc54h88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KTM

(cbu.ensure
    :name "AllianzGI-Fonds KTM"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900g19sehcnez7532)

(cbu.assign-role
    :cbu-id @cbu_529900g19sehcnez7532
    :entity-id @lei_529900g19sehcnez7532
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900g19sehcnez7532
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900g19sehcnez7532
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900g19sehcnez7532
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DRB 1

(cbu.ensure
    :name "AllianzGI-Fonds DRB 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900tdl6l151998d28)

(cbu.assign-role
    :cbu-id @cbu_529900tdl6l151998d28
    :entity-id @lei_529900tdl6l151998d28
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900tdl6l151998d28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900tdl6l151998d28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900tdl6l151998d28
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF3

(cbu.ensure
    :name "AllianzGI-Fonds MAF3"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900qhdgehi7d5l121)

(cbu.assign-role
    :cbu-id @cbu_529900qhdgehi7d5l121
    :entity-id @lei_529900qhdgehi7d5l121
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900qhdgehi7d5l121
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900qhdgehi7d5l121
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900qhdgehi7d5l121
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds HKL

(cbu.ensure
    :name "AllianzGI-Fonds HKL"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299002wiik8bumhq916)

(cbu.assign-role
    :cbu-id @cbu_5299002wiik8bumhq916
    :entity-id @lei_5299002wiik8bumhq916
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299002wiik8bumhq916
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299002wiik8bumhq916
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299002wiik8bumhq916
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds TSF

(cbu.ensure
    :name "AllianzGI-Fonds TSF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900jnf6xj1dz6ec42)

(cbu.assign-role
    :cbu-id @cbu_529900jnf6xj1dz6ec42
    :entity-id @lei_529900jnf6xj1dz6ec42
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jnf6xj1dz6ec42
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jnf6xj1dz6ec42
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jnf6xj1dz6ec42
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SHL

(cbu.ensure
    :name "AllianzGI-Fonds SHL"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xm0qot4apy0395)

(cbu.assign-role
    :cbu-id @cbu_529900xm0qot4apy0395
    :entity-id @lei_529900xm0qot4apy0395
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xm0qot4apy0395
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xm0qot4apy0395
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xm0qot4apy0395
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PUK

(cbu.ensure
    :name "AllianzGI-Fonds PUK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009bv0jgniu62z56)

(cbu.assign-role
    :cbu-id @cbu_5299009bv0jgniu62z56
    :entity-id @lei_5299009bv0jgniu62z56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009bv0jgniu62z56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009bv0jgniu62z56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009bv0jgniu62z56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-SAS Master

(cbu.ensure
    :name "AllianzGI-SAS Master"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900qqebm60uu24e73)

(cbu.assign-role
    :cbu-id @cbu_529900qqebm60uu24e73
    :entity-id @lei_529900qqebm60uu24e73
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900qqebm60uu24e73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900qqebm60uu24e73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900qqebm60uu24e73
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: PremiumMandat Konservativ

(cbu.ensure
    :name "PremiumMandat Konservativ"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900i3ki2ilaplkr33)

(cbu.assign-role
    :cbu-id @cbu_529900i3ki2ilaplkr33
    :entity-id @lei_529900i3ki2ilaplkr33
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900i3ki2ilaplkr33
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900i3ki2ilaplkr33
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900i3ki2ilaplkr33
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds FEV

(cbu.ensure
    :name "AllianzGI-Fonds FEV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xm52qp9ola4326)

(cbu.assign-role
    :cbu-id @cbu_529900xm52qp9ola4326
    :entity-id @lei_529900xm52qp9ola4326
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xm52qp9ola4326
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xm52qp9ola4326
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xm52qp9ola4326
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ALLRA

(cbu.ensure
    :name "AllianzGI-Fonds ALLRA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900mkp1w0ijcebt88)

(cbu.assign-role
    :cbu-id @cbu_529900mkp1w0ijcebt88
    :entity-id @lei_529900mkp1w0ijcebt88
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900mkp1w0ijcebt88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900mkp1w0ijcebt88
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900mkp1w0ijcebt88
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BAT-LS

(cbu.ensure
    :name "AllianzGI-Fonds BAT-LS"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990077n0r2a5ba4o89)

(cbu.assign-role
    :cbu-id @cbu_52990077n0r2a5ba4o89
    :entity-id @lei_52990077n0r2a5ba4o89
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990077n0r2a5ba4o89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990077n0r2a5ba4o89
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990077n0r2a5ba4o89
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VSF

(cbu.ensure
    :name "AllianzGI-Fonds VSF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009kcigtd2iiqb26)

(cbu.assign-role
    :cbu-id @cbu_5299009kcigtd2iiqb26
    :entity-id @lei_5299009kcigtd2iiqb26
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009kcigtd2iiqb26
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009kcigtd2iiqb26
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009kcigtd2iiqb26
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PAK

(cbu.ensure
    :name "AllianzGI-Fonds PAK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ar9cs4qrft0l40)

(cbu.assign-role
    :cbu-id @cbu_529900ar9cs4qrft0l40
    :entity-id @lei_529900ar9cs4qrft0l40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ar9cs4qrft0l40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ar9cs4qrft0l40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ar9cs4qrft0l40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DSW-Drefonds

(cbu.ensure
    :name "AllianzGI-Fonds DSW-Drefonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900r84dglig3iil30)

(cbu.assign-role
    :cbu-id @cbu_529900r84dglig3iil30
    :entity-id @lei_529900r84dglig3iil30
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900r84dglig3iil30
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900r84dglig3iil30
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900r84dglig3iil30
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Strategiefonds Wachstum Plus

(cbu.ensure
    :name "Allianz Strategiefonds Wachstum Plus"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900hy7vmtcurbbi22)

(cbu.assign-role
    :cbu-id @cbu_529900hy7vmtcurbbi22
    :entity-id @lei_529900hy7vmtcurbbi22
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900hy7vmtcurbbi22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900hy7vmtcurbbi22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900hy7vmtcurbbi22
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Gano 2

(cbu.ensure
    :name "AllianzGI-Fonds Gano 2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900rbd4yiq66nsj87)

(cbu.assign-role
    :cbu-id @cbu_529900rbd4yiq66nsj87
    :entity-id @lei_529900rbd4yiq66nsj87
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900rbd4yiq66nsj87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900rbd4yiq66nsj87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900rbd4yiq66nsj87
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: dbi-Fonds EKiBB

(cbu.ensure
    :name "dbi-Fonds EKiBB"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299006u3ow5x2qe9p56)

(cbu.assign-role
    :cbu-id @cbu_5299006u3ow5x2qe9p56
    :entity-id @lei_5299006u3ow5x2qe9p56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299006u3ow5x2qe9p56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299006u3ow5x2qe9p56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299006u3ow5x2qe9p56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds HNE

(cbu.ensure
    :name "AllianzGI-Fonds HNE"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299000nqhspxq422c64)

(cbu.assign-role
    :cbu-id @cbu_5299000nqhspxq422c64
    :entity-id @lei_5299000nqhspxq422c64
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299000nqhspxq422c64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299000nqhspxq422c64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299000nqhspxq422c64
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Concentra

(cbu.ensure
    :name "Concentra"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900dm2q9nt4orx305)

(cbu.assign-role
    :cbu-id @cbu_529900dm2q9nt4orx305
    :entity-id @lei_529900dm2q9nt4orx305
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900dm2q9nt4orx305
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900dm2q9nt4orx305
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900dm2q9nt4orx305
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Kapital Plus

(cbu.ensure
    :name "Kapital Plus"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299008ye9t4ykier075)

(cbu.assign-role
    :cbu-id @cbu_5299008ye9t4ykier075
    :entity-id @lei_5299008ye9t4ykier075
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299008ye9t4ykier075
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299008ye9t4ykier075
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299008ye9t4ykier075
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MBRF 1

(cbu.ensure
    :name "AllianzGI-Fonds MBRF 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ufho7arbnjsm28)

(cbu.assign-role
    :cbu-id @cbu_529900ufho7arbnjsm28
    :entity-id @lei_529900ufho7arbnjsm28
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900ufho7arbnjsm28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900ufho7arbnjsm28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900ufho7arbnjsm28
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds APF Renten

(cbu.ensure
    :name "AllianzGI-Fonds APF Renten"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900xbhbs44rt9qc24)

(cbu.assign-role
    :cbu-id @cbu_529900xbhbs44rt9qc24
    :entity-id @lei_529900xbhbs44rt9qc24
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900xbhbs44rt9qc24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900xbhbs44rt9qc24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900xbhbs44rt9qc24
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: dbi-Fonds DEWDI

(cbu.ensure
    :name "dbi-Fonds DEWDI"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_52990025wnxqsl3i1h18)

(cbu.assign-role
    :cbu-id @cbu_52990025wnxqsl3i1h18
    :entity-id @lei_52990025wnxqsl3i1h18
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_52990025wnxqsl3i1h18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_52990025wnxqsl3i1h18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_52990025wnxqsl3i1h18
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BRR

(cbu.ensure
    :name "AllianzGI-Fonds BRR"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900cz8p9q2wr9ya20)

(cbu.assign-role
    :cbu-id @cbu_529900cz8p9q2wr9ya20
    :entity-id @lei_529900cz8p9q2wr9ya20
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900cz8p9q2wr9ya20
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900cz8p9q2wr9ya20
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900cz8p9q2wr9ya20
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BVK 1

(cbu.ensure
    :name "AllianzGI-Fonds BVK 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900yjhylqvngt2g58)

(cbu.assign-role
    :cbu-id @cbu_529900yjhylqvngt2g58
    :entity-id @lei_529900yjhylqvngt2g58
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900yjhylqvngt2g58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900yjhylqvngt2g58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900yjhylqvngt2g58
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GVO

(cbu.ensure
    :name "AllianzGI-Fonds GVO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299009u2rurrpa93s64)

(cbu.assign-role
    :cbu-id @cbu_5299009u2rurrpa93s64
    :entity-id @lei_5299009u2rurrpa93s64
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299009u2rurrpa93s64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299009u2rurrpa93s64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299009u2rurrpa93s64
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds POTAK

(cbu.ensure
    :name "AllianzGI-Fonds POTAK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900cakjucpuxich20)

(cbu.assign-role
    :cbu-id @cbu_529900cakjucpuxich20
    :entity-id @lei_529900cakjucpuxich20
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900cakjucpuxich20
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900cakjucpuxich20
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900cakjucpuxich20
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BIP

(cbu.ensure
    :name "AllianzGI-Fonds BIP"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900zuwztqir0es354)

(cbu.assign-role
    :cbu-id @cbu_529900zuwztqir0es354
    :entity-id @lei_529900zuwztqir0es354
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zuwztqir0es354
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zuwztqir0es354
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zuwztqir0es354
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KHP 1

(cbu.ensure
    :name "AllianzGI-Fonds KHP 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900q7vctb9wcj5w61)

(cbu.assign-role
    :cbu-id @cbu_529900q7vctb9wcj5w61
    :entity-id @lei_529900q7vctb9wcj5w61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900q7vctb9wcj5w61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900q7vctb9wcj5w61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900q7vctb9wcj5w61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BEE

(cbu.ensure
    :name "AllianzGI-Fonds BEE"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900jh49lm32ko4342)

(cbu.assign-role
    :cbu-id @cbu_529900jh49lm32ko4342
    :entity-id @lei_529900jh49lm32ko4342
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jh49lm32ko4342
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jh49lm32ko4342
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jh49lm32ko4342
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz CGI Fonds

(cbu.ensure
    :name "Allianz CGI Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900p3qsdmepxmoh96)

(cbu.assign-role
    :cbu-id @cbu_529900p3qsdmepxmoh96
    :entity-id @lei_529900p3qsdmepxmoh96
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900p3qsdmepxmoh96
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900p3qsdmepxmoh96
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900p3qsdmepxmoh96
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz LAD Fonds

(cbu.ensure
    :name "Allianz LAD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900vuqoop1xwwx889)

(cbu.assign-role
    :cbu-id @cbu_529900vuqoop1xwwx889
    :entity-id @lei_529900vuqoop1xwwx889
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900vuqoop1xwwx889
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900vuqoop1xwwx889
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900vuqoop1xwwx889
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VEMK

(cbu.ensure
    :name "AllianzGI-Fonds VEMK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900en9tk8mjwmsg13)

(cbu.assign-role
    :cbu-id @cbu_529900en9tk8mjwmsg13
    :entity-id @lei_529900en9tk8mjwmsg13
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900en9tk8mjwmsg13
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900en9tk8mjwmsg13
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900en9tk8mjwmsg13
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BAVC

(cbu.ensure
    :name "AllianzGI-Fonds BAVC"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900t3a020j5cr9h38)

(cbu.assign-role
    :cbu-id @cbu_529900t3a020j5cr9h38
    :entity-id @lei_529900t3a020j5cr9h38
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900t3a020j5cr9h38
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900t3a020j5cr9h38
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900t3a020j5cr9h38
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds SVKK

(cbu.ensure
    :name "AllianzGI-Fonds SVKK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900a0jm8dvyufik10)

(cbu.assign-role
    :cbu-id @cbu_529900a0jm8dvyufik10
    :entity-id @lei_529900a0jm8dvyufik10
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900a0jm8dvyufik10
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900a0jm8dvyufik10
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900a0jm8dvyufik10
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds BFKW

(cbu.ensure
    :name "AllianzGI-Fonds BFKW"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299005op8gzcxlh4i74)

(cbu.assign-role
    :cbu-id @cbu_5299005op8gzcxlh4i74
    :entity-id @lei_5299005op8gzcxlh4i74
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299005op8gzcxlh4i74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299005op8gzcxlh4i74
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299005op8gzcxlh4i74
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds RANW II

(cbu.ensure
    :name "AllianzGI-Fonds RANW II"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299003z8df18pd1ke48)

(cbu.assign-role
    :cbu-id @cbu_5299003z8df18pd1ke48
    :entity-id @lei_5299003z8df18pd1ke48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299003z8df18pd1ke48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299003z8df18pd1ke48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299003z8df18pd1ke48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KMU SGB

(cbu.ensure
    :name "AllianzGI-Fonds KMU SGB"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900olvxy8ijtlmo91)

(cbu.assign-role
    :cbu-id @cbu_529900olvxy8ijtlmo91
    :entity-id @lei_529900olvxy8ijtlmo91
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900olvxy8ijtlmo91
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900olvxy8ijtlmo91
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900olvxy8ijtlmo91
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AVP

(cbu.ensure
    :name "AllianzGI-Fonds AVP"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900jl34bzlazlnq56)

(cbu.assign-role
    :cbu-id @cbu_529900jl34bzlazlnq56
    :entity-id @lei_529900jl34bzlazlnq56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900jl34bzlazlnq56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900jl34bzlazlnq56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900jl34bzlazlnq56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Spree

(cbu.ensure
    :name "AllianzGI-Fonds Spree"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900uvcc5sntjp3422)

(cbu.assign-role
    :cbu-id @cbu_529900uvcc5sntjp3422
    :entity-id @lei_529900uvcc5sntjp3422
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900uvcc5sntjp3422
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900uvcc5sntjp3422
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900uvcc5sntjp3422
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KVR 1

(cbu.ensure
    :name "AllianzGI-Fonds KVR 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900999m0c8fimmv40)

(cbu.assign-role
    :cbu-id @cbu_529900999m0c8fimmv40
    :entity-id @lei_529900999m0c8fimmv40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900999m0c8fimmv40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900999m0c8fimmv40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900999m0c8fimmv40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds KVT 1

(cbu.ensure
    :name "AllianzGI-Fonds KVT 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900x1bku4r9vz5q70)

(cbu.assign-role
    :cbu-id @cbu_529900x1bku4r9vz5q70
    :entity-id @lei_529900x1bku4r9vz5q70
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900x1bku4r9vz5q70
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900x1bku4r9vz5q70
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900x1bku4r9vz5q70
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AOKDRE

(cbu.ensure
    :name "AllianzGI-Fonds AOKDRE"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900o1i65jesm0dw48)

(cbu.assign-role
    :cbu-id @cbu_529900o1i65jesm0dw48
    :entity-id @lei_529900o1i65jesm0dw48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900o1i65jesm0dw48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900o1i65jesm0dw48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900o1i65jesm0dw48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ABF

(cbu.ensure
    :name "AllianzGI-Fonds ABF"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900djb93nk0w32h96)

(cbu.assign-role
    :cbu-id @cbu_529900djb93nk0w32h96
    :entity-id @lei_529900djb93nk0w32h96
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900djb93nk0w32h96
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900djb93nk0w32h96
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900djb93nk0w32h96
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PFD

(cbu.ensure
    :name "AllianzGI-Fonds PFD"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900tdebk6p85uml40)

(cbu.assign-role
    :cbu-id @cbu_529900tdebk6p85uml40
    :entity-id @lei_529900tdebk6p85uml40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900tdebk6p85uml40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900tdebk6p85uml40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900tdebk6p85uml40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: dbi-Fonds ACU-K

(cbu.ensure
    :name "dbi-Fonds ACU-K"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900q5fbzsbp8gbr35)

(cbu.assign-role
    :cbu-id @cbu_529900q5fbzsbp8gbr35
    :entity-id @lei_529900q5fbzsbp8gbr35
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900q5fbzsbp8gbr35
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900q5fbzsbp8gbr35
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900q5fbzsbp8gbr35
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds GSH

(cbu.ensure
    :name "AllianzGI-Fonds GSH"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900y8enpn6geld777)

(cbu.assign-role
    :cbu-id @cbu_529900y8enpn6geld777
    :entity-id @lei_529900y8enpn6geld777
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900y8enpn6geld777
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900y8enpn6geld777
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900y8enpn6geld777
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ACK

(cbu.ensure
    :name "AllianzGI-Fonds ACK"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900zpfvtmcxsg2o18)

(cbu.assign-role
    :cbu-id @cbu_529900zpfvtmcxsg2o18
    :entity-id @lei_529900zpfvtmcxsg2o18
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900zpfvtmcxsg2o18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900zpfvtmcxsg2o18
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900zpfvtmcxsg2o18
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VSBW

(cbu.ensure
    :name "AllianzGI-Fonds VSBW"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900efef5rg0zrit54)

(cbu.assign-role
    :cbu-id @cbu_529900efef5rg0zrit54
    :entity-id @lei_529900efef5rg0zrit54
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_529900efef5rg0zrit54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_529900efef5rg0zrit54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_529900efef5rg0zrit54
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VDB

(cbu.ensure
    :name "AllianzGI-Fonds VDB"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5299004isyecgmwl8z24)

(cbu.assign-role
    :cbu-id @cbu_5299004isyecgmwl8z24
    :entity-id @lei_5299004isyecgmwl8z24
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5299004isyecgmwl8z24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5299004isyecgmwl8z24
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5299004isyecgmwl8z24
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DPWS

(cbu.ensure
    :name "AllianzGI-Fonds DPWS"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300moru7e364shp69)

(cbu.assign-role
    :cbu-id @cbu_549300moru7e364shp69
    :entity-id @lei_549300moru7e364shp69
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300moru7e364shp69
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300moru7e364shp69
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300moru7e364shp69
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-BAS Master

(cbu.ensure
    :name "AllianzGI-BAS Master"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300durgrh4hq2jd79)

(cbu.assign-role
    :cbu-id @cbu_549300durgrh4hq2jd79
    :entity-id @lei_549300durgrh4hq2jd79
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300durgrh4hq2jd79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300durgrh4hq2jd79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300durgrh4hq2jd79
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-SKS Master

(cbu.ensure
    :name "AllianzGI-SKS Master"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300mka4gt1viood61)

(cbu.assign-role
    :cbu-id @cbu_549300mka4gt1viood61
    :entity-id @lei_549300mka4gt1viood61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300mka4gt1viood61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300mka4gt1viood61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300mka4gt1viood61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF6

(cbu.ensure
    :name "AllianzGI-Fonds MAF6"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300sy6o0xnk1r5e03)

(cbu.assign-role
    :cbu-id @cbu_549300sy6o0xnk1r5e03
    :entity-id @lei_549300sy6o0xnk1r5e03
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300sy6o0xnk1r5e03
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300sy6o0xnk1r5e03
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300sy6o0xnk1r5e03
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds MAF4

(cbu.ensure
    :name "AllianzGI-Fonds MAF4"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930005ymer04ekg287)

(cbu.assign-role
    :cbu-id @cbu_54930005ymer04ekg287
    :entity-id @lei_54930005ymer04ekg287
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930005ymer04ekg287
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930005ymer04ekg287
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930005ymer04ekg287
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AKT-W

(cbu.ensure
    :name "AllianzGI-Fonds AKT-W"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300zug8xt0ut5x531)

(cbu.assign-role
    :cbu-id @cbu_549300zug8xt0ut5x531
    :entity-id @lei_549300zug8xt0ut5x531
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300zug8xt0ut5x531
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300zug8xt0ut5x531
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300zug8xt0ut5x531
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: VW AV

(cbu.ensure
    :name "VW AV"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300wsb0dfjxpf5c84)

(cbu.assign-role
    :cbu-id @cbu_549300wsb0dfjxpf5c84
    :entity-id @lei_549300wsb0dfjxpf5c84
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300wsb0dfjxpf5c84
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300wsb0dfjxpf5c84
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300wsb0dfjxpf5c84
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Pimco EM Corporates

(cbu.ensure
    :name "Pimco EM Corporates"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300h4epzhhz2j8175)

(cbu.assign-role
    :cbu-id @cbu_549300h4epzhhz2j8175
    :entity-id @lei_549300h4epzhhz2j8175
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300h4epzhhz2j8175
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300h4epzhhz2j8175
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300h4epzhhz2j8175
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Global Investors Investmentaktiengesellschaft mit Teilgesellschaftsvermögen - Ashmore Emerging Market Corporates

(cbu.ensure
    :name "Allianz Global Investors Investmentaktiengesellschaft mit Teilgesellschaftsvermögen - Ashmore Emerging Market Corporates"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493000nz60psv04vo86)

(cbu.assign-role
    :cbu-id @cbu_5493000nz60psv04vo86
    :entity-id @lei_5493000nz60psv04vo86
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493000nz60psv04vo86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493000nz60psv04vo86
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493000nz60psv04vo86
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VKA Fonds

(cbu.ensure
    :name "Allianz VKA Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300z41d3pxctbwz68)

(cbu.assign-role
    :cbu-id @cbu_549300z41d3pxctbwz68
    :entity-id @lei_549300z41d3pxctbwz68
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300z41d3pxctbwz68
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300z41d3pxctbwz68
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300z41d3pxctbwz68
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VK RentenDirekt Fonds

(cbu.ensure
    :name "Allianz VK RentenDirekt Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493000l9dknveke8m45)

(cbu.assign-role
    :cbu-id @cbu_5493000l9dknveke8m45
    :entity-id @lei_5493000l9dknveke8m45
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493000l9dknveke8m45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493000l9dknveke8m45
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493000l9dknveke8m45
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VGL Fonds

(cbu.ensure
    :name "Allianz VGL Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493004yx8whfng6xf28)

(cbu.assign-role
    :cbu-id @cbu_5493004yx8whfng6xf28
    :entity-id @lei_5493004yx8whfng6xf28
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493004yx8whfng6xf28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493004yx8whfng6xf28
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493004yx8whfng6xf28
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VGI 1 Fonds

(cbu.ensure
    :name "Allianz VGI 1 Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300pa2so76etzkb21)

(cbu.assign-role
    :cbu-id @cbu_549300pa2so76etzkb21
    :entity-id @lei_549300pa2so76etzkb21
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300pa2so76etzkb21
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300pa2so76etzkb21
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300pa2so76etzkb21
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds VBE

(cbu.ensure
    :name "AllianzGI-Fonds VBE"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300dou8ovopu9mq64)

(cbu.assign-role
    :cbu-id @cbu_549300dou8ovopu9mq64
    :entity-id @lei_549300dou8ovopu9mq64
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300dou8ovopu9mq64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300dou8ovopu9mq64
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300dou8ovopu9mq64
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz UGD 1 Fonds

(cbu.ensure
    :name "Allianz UGD 1 Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493003efz2itcz8gl70)

(cbu.assign-role
    :cbu-id @cbu_5493003efz2itcz8gl70
    :entity-id @lei_5493003efz2itcz8gl70
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493003efz2itcz8gl70
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493003efz2itcz8gl70
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493003efz2itcz8gl70
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz RFG Fonds

(cbu.ensure
    :name "Allianz RFG Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300h0lrh7osknp750)

(cbu.assign-role
    :cbu-id @cbu_549300h0lrh7osknp750
    :entity-id @lei_549300h0lrh7osknp750
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300h0lrh7osknp750
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300h0lrh7osknp750
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300h0lrh7osknp750
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Re Asia Fonds

(cbu.ensure
    :name "Allianz Re Asia Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300b60kk4he62zb78)

(cbu.assign-role
    :cbu-id @cbu_549300b60kk4he62zb78
    :entity-id @lei_549300b60kk4he62zb78
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300b60kk4he62zb78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300b60kk4he62zb78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300b60kk4he62zb78
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds OAD

(cbu.ensure
    :name "AllianzGI-Fonds OAD"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930048ny6kysohef78)

(cbu.assign-role
    :cbu-id @cbu_54930048ny6kysohef78
    :entity-id @lei_54930048ny6kysohef78
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930048ny6kysohef78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930048ny6kysohef78
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930048ny6kysohef78
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds NBP

(cbu.ensure
    :name "AllianzGI-Fonds NBP"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300f7gt6krjmleh47)

(cbu.assign-role
    :cbu-id @cbu_549300f7gt6krjmleh47
    :entity-id @lei_549300f7gt6krjmleh47
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300f7gt6krjmleh47
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300f7gt6krjmleh47
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300f7gt6krjmleh47
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz GLRS Fonds

(cbu.ensure
    :name "Allianz GLRS Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300kpsvp4lec4m973)

(cbu.assign-role
    :cbu-id @cbu_549300kpsvp4lec4m973
    :entity-id @lei_549300kpsvp4lec4m973
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300kpsvp4lec4m973
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300kpsvp4lec4m973
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300kpsvp4lec4m973
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz GLR Fonds

(cbu.ensure
    :name "Allianz GLR Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300uln27vftvhzb09)

(cbu.assign-role
    :cbu-id @cbu_549300uln27vftvhzb09
    :entity-id @lei_549300uln27vftvhzb09
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300uln27vftvhzb09
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300uln27vftvhzb09
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300uln27vftvhzb09
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Elysee

(cbu.ensure
    :name "AllianzGI-Fonds Elysee"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300t00x81i1nsdd81)

(cbu.assign-role
    :cbu-id @cbu_549300t00x81i1nsdd81
    :entity-id @lei_549300t00x81i1nsdd81
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300t00x81i1nsdd81
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300t00x81i1nsdd81
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300t00x81i1nsdd81
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CBP LDI

(cbu.ensure
    :name "CBP LDI"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493002hzjzqn408tj61)

(cbu.assign-role
    :cbu-id @cbu_5493002hzjzqn408tj61
    :entity-id @lei_5493002hzjzqn408tj61
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493002hzjzqn408tj61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493002hzjzqn408tj61
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493002hzjzqn408tj61
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: CBP Growth

(cbu.ensure
    :name "CBP Growth"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300myjksvn0ctqe40)

(cbu.assign-role
    :cbu-id @cbu_549300myjksvn0ctqe40
    :entity-id @lei_549300myjksvn0ctqe40
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300myjksvn0ctqe40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300myjksvn0ctqe40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300myjksvn0ctqe40
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: ATZ Banken

(cbu.ensure
    :name "ATZ Banken"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493007f1kyp48b6jn93)

(cbu.assign-role
    :cbu-id @cbu_5493007f1kyp48b6jn93
    :entity-id @lei_5493007f1kyp48b6jn93
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493007f1kyp48b6jn93
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493007f1kyp48b6jn93
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493007f1kyp48b6jn93
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz ARD Fonds

(cbu.ensure
    :name "Allianz ARD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300vs43e6o485mz38)

(cbu.assign-role
    :cbu-id @cbu_549300vs43e6o485mz38
    :entity-id @lei_549300vs43e6o485mz38
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300vs43e6o485mz38
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300vs43e6o485mz38
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300vs43e6o485mz38
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz APAV Fonds

(cbu.ensure
    :name "Allianz APAV Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493008yx91fljihcm09)

(cbu.assign-role
    :cbu-id @cbu_5493008yx91fljihcm09
    :entity-id @lei_5493008yx91fljihcm09
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493008yx91fljihcm09
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493008yx91fljihcm09
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493008yx91fljihcm09
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: dbi-Fonds ANDUS

(cbu.ensure
    :name "dbi-Fonds ANDUS"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300ghtwn657eogs58)

(cbu.assign-role
    :cbu-id @cbu_549300ghtwn657eogs58
    :entity-id @lei_549300ghtwn657eogs58
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300ghtwn657eogs58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300ghtwn657eogs58
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300ghtwn657eogs58
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ZGEFO

(cbu.ensure
    :name "AllianzGI-Fonds ZGEFO"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300ign32c5bipu733)

(cbu.assign-role
    :cbu-id @cbu_549300ign32c5bipu733
    :entity-id @lei_549300ign32c5bipu733
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300ign32c5bipu733
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300ign32c5bipu733
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300ign32c5bipu733
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds ZDD2

(cbu.ensure
    :name "AllianzGI-Fonds ZDD2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300yhnnh7c39zfb47)

(cbu.assign-role
    :cbu-id @cbu_549300yhnnh7c39zfb47
    :entity-id @lei_549300yhnnh7c39zfb47
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300yhnnh7c39zfb47
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300yhnnh7c39zfb47
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300yhnnh7c39zfb47
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds HS 2

(cbu.ensure
    :name "AllianzGI-Fonds HS 2"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300ye7xen764z3u71)

(cbu.assign-role
    :cbu-id @cbu_549300ye7xen764z3u71
    :entity-id @lei_549300ye7xen764z3u71
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300ye7xen764z3u71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300ye7xen764z3u71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300ye7xen764z3u71
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Ferrostaal Renten 1

(cbu.ensure
    :name "AllianzGI-Fonds Ferrostaal Renten 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300b1tvdif4gt6108)

(cbu.assign-role
    :cbu-id @cbu_549300b1tvdif4gt6108
    :entity-id @lei_549300b1tvdif4gt6108
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300b1tvdif4gt6108
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300b1tvdif4gt6108
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300b1tvdif4gt6108
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Airbus group for Life Rentenfonds

(cbu.ensure
    :name "Airbus group for Life Rentenfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300ecp0cgaugn4q79)

(cbu.assign-role
    :cbu-id @cbu_549300ecp0cgaugn4q79
    :entity-id @lei_549300ecp0cgaugn4q79
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300ecp0cgaugn4q79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300ecp0cgaugn4q79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300ecp0cgaugn4q79
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VSR Fonds

(cbu.ensure
    :name "Allianz VSR Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300gj5e3p7ov88637)

(cbu.assign-role
    :cbu-id @cbu_549300gj5e3p7ov88637
    :entity-id @lei_549300gj5e3p7ov88637
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300gj5e3p7ov88637
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300gj5e3p7ov88637
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300gj5e3p7ov88637
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz VAE Fonds

(cbu.ensure
    :name "Allianz VAE Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300wff1mlgkrqx490)

(cbu.assign-role
    :cbu-id @cbu_549300wff1mlgkrqx490
    :entity-id @lei_549300wff1mlgkrqx490
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300wff1mlgkrqx490
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300wff1mlgkrqx490
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300wff1mlgkrqx490
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz SGI 1 Fonds

(cbu.ensure
    :name "Allianz SGI 1 Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300yxy6ty3210fw82)

(cbu.assign-role
    :cbu-id @cbu_549300yxy6ty3210fw82
    :entity-id @lei_549300yxy6ty3210fw82
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300yxy6ty3210fw82
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300yxy6ty3210fw82
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300yxy6ty3210fw82
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz SDR Fonds

(cbu.ensure
    :name "Allianz SDR Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300lsbi7o1kv6zn56)

(cbu.assign-role
    :cbu-id @cbu_549300lsbi7o1kv6zn56
    :entity-id @lei_549300lsbi7o1kv6zn56
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300lsbi7o1kv6zn56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300lsbi7o1kv6zn56
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300lsbi7o1kv6zn56
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz SGB Renten

(cbu.ensure
    :name "Allianz SGB Renten"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300kin76yy6ger036)

(cbu.assign-role
    :cbu-id @cbu_549300kin76yy6ger036
    :entity-id @lei_549300kin76yy6ger036
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300kin76yy6ger036
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300kin76yy6ger036
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300kin76yy6ger036
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Rentenfonds

(cbu.ensure
    :name "Allianz Rentenfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300e951gzt57y7c57)

(cbu.assign-role
    :cbu-id @cbu_549300e951gzt57y7c57
    :entity-id @lei_549300e951gzt57y7c57
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300e951gzt57y7c57
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300e951gzt57y7c57
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300e951gzt57y7c57
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Money Market US $

(cbu.ensure
    :name "Allianz Money Market US $"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300ocoiazex0bdb67)

(cbu.assign-role
    :cbu-id @cbu_549300ocoiazex0bdb67
    :entity-id @lei_549300ocoiazex0bdb67
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300ocoiazex0bdb67
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300ocoiazex0bdb67
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300ocoiazex0bdb67
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Mobil-Fonds

(cbu.ensure
    :name "Allianz Mobil-Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300b25j18hdd4yf12)

(cbu.assign-role
    :cbu-id @cbu_549300b25j18hdd4yf12
    :entity-id @lei_549300b25j18hdd4yf12
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300b25j18hdd4yf12
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300b25j18hdd4yf12
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300b25j18hdd4yf12
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Internationaler Rentenfonds

(cbu.ensure
    :name "Allianz Internationaler Rentenfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300yahrmz64wmdv94)

(cbu.assign-role
    :cbu-id @cbu_549300yahrmz64wmdv94
    :entity-id @lei_549300yahrmz64wmdv94
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300yahrmz64wmdv94
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300yahrmz64wmdv94
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300yahrmz64wmdv94
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz PIMCO High Yield Income Fund

(cbu.ensure
    :name "Allianz PIMCO High Yield Income Fund"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_549300eri18q4k90hf46)

(cbu.assign-role
    :cbu-id @cbu_549300eri18q4k90hf46
    :entity-id @lei_549300eri18q4k90hf46
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300eri18q4k90hf46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300eri18q4k90hf46
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300eri18q4k90hf46
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz Euro Rentenfonds

(cbu.ensure
    :name "Allianz Euro Rentenfonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300blgufvskdbhq66)

(cbu.assign-role
    :cbu-id @cbu_549300blgufvskdbhq66
    :entity-id @lei_549300blgufvskdbhq66
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300blgufvskdbhq66
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300blgufvskdbhq66
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300blgufvskdbhq66
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz MET 1 Fonds

(cbu.ensure
    :name "Allianz MET 1 Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493009wm4s9hpxsq012)

(cbu.assign-role
    :cbu-id @cbu_5493009wm4s9hpxsq012
    :entity-id @lei_5493009wm4s9hpxsq012
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493009wm4s9hpxsq012
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493009wm4s9hpxsq012
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493009wm4s9hpxsq012
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz GRGB Fonds

(cbu.ensure
    :name "Allianz GRGB Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493003bnxukcp2wtl71)

(cbu.assign-role
    :cbu-id @cbu_5493003bnxukcp2wtl71
    :entity-id @lei_5493003bnxukcp2wtl71
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493003bnxukcp2wtl71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493003bnxukcp2wtl71
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493003bnxukcp2wtl71
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz FAD Fonds

(cbu.ensure
    :name "Allianz FAD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300i5fgd97gy1c248)

(cbu.assign-role
    :cbu-id @cbu_549300i5fgd97gy1c248
    :entity-id @lei_549300i5fgd97gy1c248
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300i5fgd97gy1c248
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300i5fgd97gy1c248
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300i5fgd97gy1c248
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz ABA Fonds

(cbu.ensure
    :name "Allianz ABA Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300fcsterroinhy34)

(cbu.assign-role
    :cbu-id @cbu_549300fcsterroinhy34
    :entity-id @lei_549300fcsterroinhy34
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300fcsterroinhy34
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300fcsterroinhy34
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300fcsterroinhy34
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz ALD Fonds

(cbu.ensure
    :name "Allianz ALD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493009gsrc2gz0fxn14)

(cbu.assign-role
    :cbu-id @cbu_5493009gsrc2gz0fxn14
    :entity-id @lei_5493009gsrc2gz0fxn14
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493009gsrc2gz0fxn14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493009gsrc2gz0fxn14
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493009gsrc2gz0fxn14
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz AKR Fonds

(cbu.ensure
    :name "Allianz AKR Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493002z2vkyuqjcsx22)

(cbu.assign-role
    :cbu-id @cbu_5493002z2vkyuqjcsx22
    :entity-id @lei_5493002z2vkyuqjcsx22
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493002z2vkyuqjcsx22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493002z2vkyuqjcsx22
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493002z2vkyuqjcsx22
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz AADB Fonds

(cbu.ensure
    :name "Allianz AADB Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493006w0oxzhnt7li48)

(cbu.assign-role
    :cbu-id @cbu_5493006w0oxzhnt7li48
    :entity-id @lei_5493006w0oxzhnt7li48
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493006w0oxzhnt7li48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493006w0oxzhnt7li48
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493006w0oxzhnt7li48
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds A200

(cbu.ensure
    :name "AllianzGI-Fonds A200"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300bj0u1kwa7o2d49)

(cbu.assign-role
    :cbu-id @cbu_549300bj0u1kwa7o2d49
    :entity-id @lei_549300bj0u1kwa7o2d49
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300bj0u1kwa7o2d49
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300bj0u1kwa7o2d49
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300bj0u1kwa7o2d49
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: Allianz PV-RD Fonds

(cbu.ensure
    :name "Allianz PV-RD Fonds"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300vz6u79wmspin73)

(cbu.assign-role
    :cbu-id @cbu_549300vz6u79wmspin73
    :entity-id @lei_549300vz6u79wmspin73
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300vz6u79wmspin73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300vz6u79wmspin73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300vz6u79wmspin73
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Pfalco

(cbu.ensure
    :name "AllianzGI-Fonds Pfalco"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300ez4epnjymiw145)

(cbu.assign-role
    :cbu-id @cbu_549300ez4epnjymiw145
    :entity-id @lei_549300ez4epnjymiw145
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300ez4epnjymiw145
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300ez4epnjymiw145
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300ez4epnjymiw145
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds RG-Anlage

(cbu.ensure
    :name "AllianzGI-Fonds RG-Anlage"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300my7x3vx8rzs840)

(cbu.assign-role
    :cbu-id @cbu_549300my7x3vx8rzs840
    :entity-id @lei_549300my7x3vx8rzs840
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300my7x3vx8rzs840
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300my7x3vx8rzs840
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300my7x3vx8rzs840
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds APNIESA

(cbu.ensure
    :name "AllianzGI-Fonds APNIESA"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300spcmf4by627t62)

(cbu.assign-role
    :cbu-id @cbu_549300spcmf4by627t62
    :entity-id @lei_549300spcmf4by627t62
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300spcmf4by627t62
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300spcmf4by627t62
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300spcmf4by627t62
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds PF 1

(cbu.ensure
    :name "AllianzGI-Fonds PF 1"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300n1l9dbr2f5u334)

(cbu.assign-role
    :cbu-id @cbu_549300n1l9dbr2f5u334
    :entity-id @lei_549300n1l9dbr2f5u334
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300n1l9dbr2f5u334
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300n1l9dbr2f5u334
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300n1l9dbr2f5u334
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds Pensions

(cbu.ensure
    :name "AllianzGI-Fonds Pensions"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930028lv63bjayym34)

(cbu.assign-role
    :cbu-id @cbu_54930028lv63bjayym34
    :entity-id @lei_54930028lv63bjayym34
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930028lv63bjayym34
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930028lv63bjayym34
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930028lv63bjayym34
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds HPT

(cbu.ensure
    :name "AllianzGI-Fonds HPT"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930004b8611hxvoq12)

(cbu.assign-role
    :cbu-id @cbu_54930004b8611hxvoq12
    :entity-id @lei_54930004b8611hxvoq12
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930004b8611hxvoq12
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930004b8611hxvoq12
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930004b8611hxvoq12
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI - Fonds PKM Degussa

(cbu.ensure
    :name "AllianzGI - Fonds PKM Degussa"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300wye06a2hrq0q04)

(cbu.assign-role
    :cbu-id @cbu_549300wye06a2hrq0q04
    :entity-id @lei_549300wye06a2hrq0q04
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300wye06a2hrq0q04
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300wye06a2hrq0q04
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300wye06a2hrq0q04
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds DSPT

(cbu.ensure
    :name "AllianzGI-Fonds DSPT"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300u6uyhnifazlb73)

(cbu.assign-role
    :cbu-id @cbu_549300u6uyhnifazlb73
    :entity-id @lei_549300u6uyhnifazlb73
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300u6uyhnifazlb73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300u6uyhnifazlb73
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300u6uyhnifazlb73
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds D300

(cbu.ensure
    :name "AllianzGI-Fonds D300"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_549300y7no20wrddxk54)

(cbu.assign-role
    :cbu-id @cbu_549300y7no20wrddxk54
    :entity-id @lei_549300y7no20wrddxk54
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_549300y7no20wrddxk54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_549300y7no20wrddxk54
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_549300y7no20wrddxk54
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: AllianzGI-Fonds AFE

(cbu.ensure
    :name "AllianzGI-Fonds AFE"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_5493007xyk1v6uoytv08)

(cbu.assign-role
    :cbu-id @cbu_5493007xyk1v6uoytv08
    :entity-id @lei_5493007xyk1v6uoytv08
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_5493007xyk1v6uoytv08
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_5493007xyk1v6uoytv08
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_5493007xyk1v6uoytv08
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; CBU: SGB Geldmarkt

(cbu.ensure
    :name "SGB Geldmarkt"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_54930082yq3iu7otg277)

(cbu.assign-role
    :cbu-id @cbu_54930082yq3iu7otg277
    :entity-id @lei_54930082yq3iu7otg277
    :role "ASSET_OWNER")

(cbu.assign-role
    :cbu-id @cbu_54930082yq3iu7otg277
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

(cbu.assign-role
    :cbu-id @cbu_54930082yq3iu7otg277
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Ultimate Client: Allianz SE
(cbu.assign-role
    :cbu-id @cbu_54930082yq3iu7otg277
    :entity-id @lei_529900k9b0n5bt694847
    :role "ULTIMATE_CLIENT")

;; ============================================================================
;; END OF ALLIANZ GLEIF COMPLETE FUND DATA LOAD
;; ============================================================================