;; ============================================================================
;; ALLIANZ GLEIF DATA LOAD
;; Generated: 2026-01-01T10:58:45.757836+00:00
;; Source: GLEIF API (api.gleif.org)
;; ============================================================================

;; ============================================================================
;; PHASE 1: Parent Entities (Allianz SE → AllianzGI hierarchy)
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

;; Ownership relationships

(ubo.add-ownership
    :owner-entity-id @lei_529900k9b0n5bt694847
    :owned-entity-id @lei_oj2tiqsvqnd4izyyk658
    :percentage 100.0
    :ownership-type "DIRECTLY_CONSOLIDATED"
    :corroboration "FULLY_CORROBORATED"
)

(ubo.add-ownership
    :owner-entity-id @lei_oj2tiqsvqnd4izyyk658
    :owned-entity-id @lei_5493005jtev4ovdvnh32
    :percentage 100.0
    :ownership-type "DIRECTLY_CONSOLIDATED"
)

(ubo.add-ownership
    :owner-entity-id @lei_oj2tiqsvqnd4izyyk658
    :owned-entity-id @lei_353800nvwwgob9jxqz47
    :percentage 100.0
    :ownership-type "DIRECTLY_CONSOLIDATED"
)

;; ============================================================================
;; PHASE 2: AllianzGI Subsidiaries
;; ============================================================================

;; ALLIANZ CAPITAL PARTNERS OF AMERICA LLC
(entity.ensure-limited-company
    :name "ALLIANZ CAPITAL PARTNERS OF AMERICA LLC"
    :lei "5493005JTEV4OVDVNH32"
    :jurisdiction "US-DE"
    :registration-number "3600054"
    :city "DOVER"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "HZEH"
    :direct-parent-lei "OJ2TIQSVQND4IZYYK658"
    :as @lei_5493005jtev4ovdvnh32)

;; アリアンツ・グローバル・インベスターズ・ジャパン株式会社
(entity.ensure-limited-company
    :name "アリアンツ・グローバル・インベスターズ・ジャパン株式会社"
    :lei "353800NVWWGOB9JXQZ47"
    :jurisdiction "JP"
    :registration-number "0104-01-053740"
    :city "東京都 港区"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "T417"
    :direct-parent-lei "OJ2TIQSVQND4IZYYK658"
    :as @lei_353800nvwwgob9jxqz47)

;; ============================================================================
;; PHASE 3: Managed Funds → CBUs with IM/ManCo roles
;; Total funds: 10
;; ============================================================================

;; Fund: Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF"
    :lei "529900LSFQ65EMQNBP87"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_529900lsfq65emqnbp87)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz Asia Pacific Secured Lending Fund III S.A., SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900lsfq65emqnbp87)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_529900lsfq65emqnbp87
    :entity-id @lei_529900lsfq65emqnbp87
    :role "SICAV")

;; Fund: Allianz Global Enhanced Equity Income

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz Global Enhanced Equity Income"
    :lei "529900O2D7WTTP2ECM60"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_529900o2d7wttp2ecm60)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz Global Enhanced Equity Income"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900o2d7wttp2ecm60)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_529900o2d7wttp2ecm60
    :entity-id @lei_529900o2d7wttp2ecm60
    :role "SICAV")

;; Fund: Allianz EuropEquity Crescendo

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz EuropEquity Crescendo"
    :lei "529900A4N4FMRF1QIT75"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_529900a4n4fmrf1qit75)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz EuropEquity Crescendo"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900a4n4fmrf1qit75)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_529900a4n4fmrf1qit75
    :entity-id @lei_529900a4n4fmrf1qit75
    :role "SICAV")

;; Fund: OCIRP Actions Multifacteurs

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "OCIRP Actions Multifacteurs"
    :lei "529900ZWZD2XKZ3GFO55"
    :jurisdiction "FR"
    :gleif-category "FUND"
    :as @lei_529900zwzd2xkz3gfo55)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "OCIRP Actions Multifacteurs"
    :client-type "FUND"
    :jurisdiction "FR"
    :as @cbu_529900zwzd2xkz3gfo55)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900zwzd2xkz3gfo55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900zwzd2xkz3gfo55
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Fund: CAPVIVA Infrastructure

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "CAPVIVA Infrastructure"
    :lei "5299007D2Y764JWNW850"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_5299007d2y764jwnw850)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "CAPVIVA Infrastructure"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299007d2y764jwnw850)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_5299007d2y764jwnw850
    :entity-id @lei_5299007d2y764jwnw850
    :role "SICAV")

;; Fund: Allianz European Autonomy

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz European Autonomy"
    :lei "529900E5TEG9CGU33298"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_529900e5teg9cgu33298)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz European Autonomy"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900e5teg9cgu33298)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_529900e5teg9cgu33298
    :entity-id @lei_529900e5teg9cgu33298
    :role "SICAV")

;; Fund: Aktien Dividende Global

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Aktien Dividende Global"
    :lei "529900NSPM728J89YR40"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :as @lei_529900nspm728j89yr40)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Aktien Dividende Global"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900nspm728j89yr40)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900nspm728j89yr40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900nspm728j89yr40
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Fund: Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)"
    :lei "529900UD1FMOKR9RN344"
    :jurisdiction "DE"
    :gleif-category "FUND"
    :as @lei_529900ud1fmokr9rn344)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz Private Debt Secondary Fund II EUR Feeder Fund (Germany)"
    :client-type "FUND"
    :jurisdiction "DE"
    :as @cbu_529900ud1fmokr9rn344)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900ud1fmokr9rn344
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Fund: Allianz Best Styles US Small Cap Equity

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz Best Styles US Small Cap Equity"
    :lei "5299008BUZ4793QZRD79"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_5299008buz4793qzrd79)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz Best Styles US Small Cap Equity"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_5299008buz4793qzrd79)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_5299008buz4793qzrd79
    :entity-id @lei_5299008buz4793qzrd79
    :role "SICAV")

;; Fund: Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF

;; Step 1: Create fund entity
(entity.ensure-limited-company
    :name "Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF"
    :lei "529900KP3L513IRKR804"
    :jurisdiction "LU"
    :gleif-category "FUND"
    :as @lei_529900kp3l513irkr804)

;; Step 2: Create CBU for fund onboarding
(cbu.ensure
    :name "Allianz Private Debt Secondary Fund II SCSp, SICAV-RAIF"
    :client-type "FUND"
    :jurisdiction "LU"
    :as @cbu_529900kp3l513irkr804)

;; Step 3: Assign Investment Manager role
(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "INVESTMENT_MANAGER")

;; Step 4: Assign ManCo role (self-managed)
(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_oj2tiqsvqnd4izyyk658
    :role "MANAGEMENT_COMPANY")

;; Step 5: SICAV role (Luxembourg)
(cbu.assign-role
    :cbu-id @cbu_529900kp3l513irkr804
    :entity-id @lei_529900kp3l513irkr804
    :role "SICAV")

;; ============================================================================
;; PHASE 4: Allianz SE Direct Subsidiaries
;; Total: 237
;; ============================================================================

;; Windpark Emmendorf GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Emmendorf GmbH & Co. KG"
    :lei "529900KMPEUGWRAWBE94"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900kmpeugwrawbe94)

;; ALLIANZ PRIVATE EQUITY PARTNERS IV
(entity.ensure-limited-company
    :name "ALLIANZ PRIVATE EQUITY PARTNERS IV"
    :lei "815600DD525D8C02EE49"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MILANO"
    :gleif-status "ACTIVE"
    :gleif-category "FUND"
    :legal-form-code "9999"
    :as @lei_815600dd525d8c02ee49)

;; INSIEME - FONDO PENSIONE APERTO A CONTRIBUZIONE DEFINITA
(entity.ensure-limited-company
    :name "INSIEME - FONDO PENSIONE APERTO A CONTRIBUZIONE DEFINITA"
    :lei "81560044F6DE4CDE9E77"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MILANO"
    :gleif-status "ACTIVE"
    :gleif-category "FUND"
    :legal-form-code "OQ8C"
    :as @lei_81560044f6de4cde9e77)

;; ALLIANZ PREVIDENZA FONDO PENSIONE APERTO A CONTRIBUZIONE DEFINITA
(entity.ensure-limited-company
    :name "ALLIANZ PREVIDENZA FONDO PENSIONE APERTO A CONTRIBUZIONE DEFINITA"
    :lei "8156003E18B22867E589"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MILANO"
    :gleif-status "ACTIVE"
    :gleif-category "FUND"
    :legal-form-code "OQ8C"
    :as @lei_8156003e18b22867e589)

;; UK Logistics S.C.Sp.
(entity.ensure-limited-company
    :name "UK Logistics S.C.Sp."
    :lei "529900NCVUCTCD8Z1P88"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "U8KA"
    :as @lei_529900ncvuctcd8z1p88)

;; UK Logistics PropCo V S.à r.l.
(entity.ensure-limited-company
    :name "UK Logistics PropCo V S.à r.l."
    :lei "529900X5D7405ZT3D867"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900x5d7405zt3d867)

;; UK Logistics PropCo IV S.à r.l.
(entity.ensure-limited-company
    :name "UK Logistics PropCo IV S.à r.l."
    :lei "5299005ORQPYGM8ESL85"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_5299005orqpygm8esl85)

;; PIMCO GLOBAL ADVISORS LLC
(entity.ensure-limited-company
    :name "PIMCO GLOBAL ADVISORS LLC"
    :lei "254900PKUZU0PVS5TA11"
    :jurisdiction "US-DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "DOVER"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "HZEH"
    :as @lei_254900pkuzu0pvs5ta11)

;; Allvest GmbH
(entity.ensure-limited-company
    :name "Allvest GmbH"
    :lei "391200YC6MPAB9UCR633"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_391200yc6mpab9ucr633)

;; Allianz Esa GmbH
(entity.ensure-limited-company
    :name "Allianz Esa GmbH"
    :lei "529900BRR7RE7TRK1869"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bad Friedrichshall"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900brr7re7trk1869)

;; INNOVATION GROUP HOLDINGS LIMITED
(entity.ensure-limited-company
    :name "INNOVATION GROUP HOLDINGS LIMITED"
    :lei "213800SL2ATL3XQX1Y90"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "FAREHAM"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800sl2atl3xqx1y90)

;; SOCIETE EUROPEENNE DE PROTECTION ET DE SERVICES D'ASSISTANCE A DOMICILE EN ABREGE "SEPSAD"
(entity.ensure-limited-company
    :name "SOCIETE EUROPEENNE DE PROTECTION ET DE SERVICES D'ASSISTANCE A DOMICILE EN ABREGE \"SEPSAD\""
    :lei "969500RUNU9G6Y9X4V29"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "SAINT-OUEN-SUR-SEINE"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K65D"
    :as @lei_969500runu9g6y9x4v29)

;; AWP REUNION SAS
(entity.ensure-limited-company
    :name "AWP REUNION SAS"
    :lei "969500ERN5OPXVJQ4G17"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "SAINTE-MARIE"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K65D"
    :as @lei_969500ern5opxvjq4g17)

;; Volkswagen Autoversicherung Holding GmbH
(entity.ensure-limited-company
    :name "Volkswagen Autoversicherung Holding GmbH"
    :lei "529900GT71P0QS6L5Z56"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Braunschweig"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900gt71p0qs6l5z56)

;; Allianz 101 Moorgate Holding SARL
(entity.ensure-limited-company
    :name "Allianz 101 Moorgate Holding SARL"
    :lei "529900LEOQK91YSY5047"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900leoqk91ysy5047)

;; Allianz 1 Liverpool Street Holding SARL
(entity.ensure-limited-company
    :name "Allianz 1 Liverpool Street Holding SARL"
    :lei "529900VPLZF56U3F4T41"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900vplzf56u3f4t41)

;; Ceres Holding I S.à r.l.
(entity.ensure-limited-company
    :name "Ceres Holding I S.à r.l."
    :lei "5299007RS1RSLNP6BE43"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_5299007rs1rslnp6be43)

;; Redoma 2 S.A.
(entity.ensure-limited-company
    :name "Redoma 2 S.A."
    :lei "5299001QV4UX3TVK4G61"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299001qv4ux3tvk4g61)

;; Valderrama S.A.
(entity.ensure-limited-company
    :name "Valderrama S.A."
    :lei "529900JMGN7Y9S1A5023"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900jmgn7y9s1a5023)

;; Viridium Group Sarl
(entity.ensure-limited-company
    :name "Viridium Group Sarl"
    :lei "529900KV8TZ3U89I6O72"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900kv8tz3u89i6o72)

;; APK Investments Holding S.à r.l.
(entity.ensure-limited-company
    :name "APK Investments Holding S.à r.l."
    :lei "529900ZRMMC9EF0CHG57"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900zrmmc9ef0chg57)

;; Redoma S.à r.l.
(entity.ensure-limited-company
    :name "Redoma S.à r.l."
    :lei "529900JAQ9MFHV2CEF65"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900jaq9mfhv2cef65)

;; UK Logistics GP S.à r.l.
(entity.ensure-limited-company
    :name "UK Logistics GP S.à r.l."
    :lei "529900C1VFT8SRODZ154"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900c1vft8srodz154)

;; Element 926 Investors Fund GP Sarl
(entity.ensure-limited-company
    :name "Element 926 Investors Fund GP Sarl"
    :lei "5299007E9ZOJ60K9GC16"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_5299007e9zoj60k9gc16)

;; Allianz Investments HoldCo S.à r.l
(entity.ensure-limited-company
    :name "Allianz Investments HoldCo S.à r.l"
    :lei "5299001SAKH6DYTX4P76"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_5299001sakh6dytx4p76)

;; AZ Euro Investments II S.à r.l.
(entity.ensure-limited-company
    :name "AZ Euro Investments II S.à r.l."
    :lei "529900ZSP3T30HEG1I96"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900zsp3t30heg1i96)

;; Allianz Infrastructure Luxembourg Holdco II S.A.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg Holdco II S.A."
    :lei "529900JYC93AXK30JW23"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900jyc93axk30jw23)

;; Allianz US Debt Holding S.A.
(entity.ensure-limited-company
    :name "Allianz US Debt Holding S.A."
    :lei "529900ODB19ZWB493Z96"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900odb19zwb493z96)

;; Allianz X Euler Hermes Co-Investments S.à r.l.
(entity.ensure-limited-company
    :name "Allianz X Euler Hermes Co-Investments S.à r.l."
    :lei "529900JBIGHR5KOZ9N29"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900jbighr5koz9n29)

;; Allianz Société Financière S.à.r.l.
(entity.ensure-limited-company
    :name "Allianz Société Financière S.à.r.l."
    :lei "529900ETBIZPTHG66X03"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900etbizpthg66x03)

;; Allianz Renewable Energy Partners Luxembourg Holdco VI S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg Holdco VI S.A."
    :lei "529900SM2KPMVXLHOZ24"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900sm2kpmvxlhoz24)

;; Allianz Renewable Energy Partners Luxembourg Holdco IV S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg Holdco IV S.A."
    :lei "5299008Y0LUIFD0J2G82"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299008y0luifd0j2g82)

;; Allianz Renewable Energy Partners Luxembourg Holdco II S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg Holdco II S.à r.l."
    :lei "529900XRZQ5892V5AT42"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900xrzq5892v5at42)

;; Allianz Renewable Energy Partners Luxembourg X S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg X S.A."
    :lei "52990049INQDGKOXSP55"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_52990049inqdgkoxsp55)

;; Allianz Leben Real Estate Holding II S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Leben Real Estate Holding II S.à r.l."
    :lei "529900NPBR9W5ACQV268"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900npbr9w5acqv268)

;; Allianz Presse Infra GP S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Presse Infra GP S.à r.l."
    :lei "529900V4TE6AGDDMLB46"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900v4te6agddmlb46)

;; Allianz lnfrastructure Norway Holdco I S.à r.l.
(entity.ensure-limited-company
    :name "Allianz lnfrastructure Norway Holdco I S.à r.l."
    :lei "529900QBRV0OO127MB59"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900qbrv0oo127mb59)

;; Allianz Leben Real Estate Holding I S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Leben Real Estate Holding I S.à r.l."
    :lei "529900I85IBTARGY8404"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900i85ibtargy8404)

;; Allianz Infrastructure Luxembourg Holdco III S.A.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg Holdco III S.A."
    :lei "5299006KBTNVWTGTM652"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299006kbtnvwtgtm652)

;; Allianz Infrastructure Luxembourg Holdco IV S.A.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg Holdco IV S.A."
    :lei "529900AN1HP9OY6N2P89"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900an1hp9oy6n2p89)

;; Allianz Infrastructure Luxembourg Holdco I S.A.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg Holdco I S.A."
    :lei "5299006FN0KQWT20NS75"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299006fn0kqwt20ns75)

;; Allianz Finance VIII Luxembourg S.A.
(entity.ensure-limited-company
    :name "Allianz Finance VIII Luxembourg S.A."
    :lei "5299004UU8E7H7RBVC60"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299004uu8e7h7rbvc60)

;; Allianz Debt Investments S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Debt Investments S.à r.l."
    :lei "529900HDYGHJOFNGIR39"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900hdyghjofngir39)

;; Allianz Debt Fund S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Debt Fund S.à r.l."
    :lei "529900HOO5UCFRWLXD68"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900hoo5ucfrwlxd68)

;; Windpark Berge-Kleeste GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Berge-Kleeste GmbH & Co. KG"
    :lei "529900EXWHNP8FJNE695"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900exwhnp8fjne695)

;; Allianz Pension Service GmbH
(entity.ensure-limited-company
    :name "Allianz Pension Service GmbH"
    :lei "529900CN1SHZZ2DGGV10"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900cn1shzz2dggv10)

;; 安联（中国）保险控股有限公司
(entity.ensure-limited-company
    :name "安联（中国）保险控股有限公司"
    :lei "836800F5011731000058"
    :jurisdiction "CN"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "浦东新区"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "ECAK"
    :as @lei_836800f5011731000058)

;; Euler Hermes North America Holding, Inc.
(entity.ensure-limited-company
    :name "Euler Hermes North America Holding, Inc."
    :lei "529900SPZKHUR0S2Z985"
    :jurisdiction "US-DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WILMINGTON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_529900spzkhur0s2z985)

;; 율러허미스한국손해보험중개 주식회사
(entity.ensure-limited-company
    :name "율러허미스한국손해보험중개 주식회사"
    :lei "529900MI7QZ401XSM482"
    :jurisdiction "KR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Jung-gu"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "J3U5"
    :as @lei_529900mi7qz401xsm482)

;; Euler Hermes Services Belgium
(entity.ensure-limited-company
    :name "Euler Hermes Services Belgium"
    :lei "699400SZYEEQW3Q9OZ64"
    :jurisdiction "BE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Brussel"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "R85P"
    :as @lei_699400szyeeqw3q9oz64)

;; EULER HERMES Services B.V.
(entity.ensure-limited-company
    :name "EULER HERMES Services B.V."
    :lei "69940092VYAJAVEVC598"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "'s-Hertogenbosch"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "54M6"
    :as @lei_69940092vyajavevc598)

;; Allianz Residential Mortgage Company S.A.
(entity.ensure-limited-company
    :name "Allianz Residential Mortgage Company S.A."
    :lei "5299005VQRXW26BI8T24"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299005vqrxw26bi8t24)

;; Vivy GmbH
(entity.ensure-limited-company
    :name "Vivy GmbH"
    :lei "5299005953HTHIFLB921"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_5299005953hthiflb921)

;; 台灣裕利安宜有限公司
(entity.ensure-limited-company
    :name "台灣裕利安宜有限公司"
    :lei "5299005AGE5ZYF6DVO90"
    :jurisdiction "TW"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Taipei"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "TD8P"
    :as @lei_5299005age5zyf6dvo90)

;; Solvd GmbH
(entity.ensure-limited-company
    :name "Solvd GmbH"
    :lei "391200ONQC54ULSH6F19"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_391200onqc54ulsh6f19)

;; INNOVATION FSP (PTY) LTD
(entity.ensure-limited-company
    :name "INNOVATION FSP (PTY) LTD"
    :lei "213800CCVZYZHNK22765"
    :jurisdiction "ZA"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "SANDTON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "GQVQ"
    :as @lei_213800ccvzyzhnk22765)

;; AZL-Argos 93 Vermögensverwaltungsgesellschaft mbH
(entity.ensure-limited-company
    :name "AZL-Argos 93 Vermögensverwaltungsgesellschaft mbH"
    :lei "529900UHOD1GLFHOI798"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900uhod1glfhoi798)

;; Kaiser X Labs GmbH
(entity.ensure-limited-company
    :name "Kaiser X Labs GmbH"
    :lei "5299006E3R01U1Q5ZP27"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_5299006e3r01u1q5zp27)

;; ริษัท อลิอันซ์ อยุธยา ประกันภัย จำกัด (มหาชน)
(entity.ensure-limited-company
    :name "ริษัท อลิอันซ์ อยุธยา ประกันภัย จำกัด (มหาชน)"
    :lei "5299000V0BZSEOT5SV34"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bangkok"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "O0OZ"
    :as @lei_5299000v0bzseot5sv34)

;; Allianz France Real Estate S.à r.l.
(entity.ensure-limited-company
    :name "Allianz France Real Estate S.à r.l."
    :lei "529900DW0KEF97HQ9T30"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900dw0kef97hq9t30)

;; Jubilee Allianz General Insurance (Mauritius) Limited
(entity.ensure-limited-company
    :name "Jubilee Allianz General Insurance (Mauritius) Limited"
    :lei "529900C3W9DEEWUS8S66"
    :jurisdiction "MU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Port Louis"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_529900c3w9deewus8s66)

;; WINDPOWER UJŚCIE SPÓŁKA Z OGRANICZONĄ ODPOWIEDZIALNOŚCIĄ
(entity.ensure-limited-company
    :name "WINDPOWER UJŚCIE SPÓŁKA Z OGRANICZONĄ ODPOWIEDZIALNOŚCIĄ"
    :lei "529900KXI69UKUZVZ369"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "POZNAŃ"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "O7XB"
    :as @lei_529900kxi69ukuzvz369)

;; Windpark Kittlitz Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Kittlitz Repowering GmbH & Co. KG"
    :lei "529900Y49TIJEBGSUM96"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900y49tijebgsum96)

;; Windpark Cottbuser See Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Cottbuser See Repowering GmbH & Co. KG"
    :lei "529900WSAOY5BSPJ3Y39"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900wsaoy5bspj3y39)

;; Windpark Freyenstein-Halenbeck Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Freyenstein-Halenbeck Repowering GmbH & Co. KG"
    :lei "529900UPMS9AFC4NJV70"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900upms9afc4njv70)

;; Windpark Schönwalde Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Schönwalde Repowering GmbH & Co. KG"
    :lei "529900QCZUMUKE1G5X10"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900qczumuke1g5x10)

;; Windpark Pröttlin Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Pröttlin Repowering GmbH & Co. KG"
    :lei "529900N6880B90DJ4066"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900n6880b90dj4066)

;; Windpark Werder Zinndorf Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Werder Zinndorf Repowering GmbH & Co. KG"
    :lei "529900E2ONHRMXUCH677"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900e2onhrmxuch677)

;; Windpark Kirf Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Kirf Repowering GmbH & Co. KG"
    :lei "529900ACJEWSMG8YZ727"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_529900acjewsmg8yz727)

;; Windpark Redekin-Genthin Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Redekin-Genthin Repowering GmbH & Co. KG"
    :lei "5299008I1E0MQEJPC987"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_5299008i1e0mqejpc987)

;; Windpark Waltersdorf Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Waltersdorf Repowering GmbH & Co. KG"
    :lei "5299003THKILAGT1LK42"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_5299003thkilagt1lk42)

;; Windpark Kesfeld Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Kesfeld Repowering GmbH & Co. KG"
    :lei "52990036NSG5MVV3CA67"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_52990036nsg5mvv3ca67)

;; Windpark Quitzow Repowering GmbH & Co. KG
(entity.ensure-limited-company
    :name "Windpark Quitzow Repowering GmbH & Co. KG"
    :lei "5299002H605WYURYDZ63"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sehestedt"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8Z6G"
    :as @lei_5299002h605wyurydz63)

;; Jubilee Allianz General Insurance Company Limited
(entity.ensure-limited-company
    :name "Jubilee Allianz General Insurance Company Limited"
    :lei "52990050IBGLHYIC3T71"
    :jurisdiction "UG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Kampala"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "9999"
    :as @lei_52990050ibglhyic3t71)

;; Allianz Infrastructure Luxembourg III S.A.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg III S.A."
    :lei "529900WP0ZLI82XDUU70"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900wp0zli82xduu70)

;; Allianz Renewable Energy Partners Luxembourg V S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg V S.A."
    :lei "529900QPF26J8K9W1R07"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900qpf26j8k9w1r07)

;; Allianz Renewable Energy Partners Luxembourg IV S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg IV S.A."
    :lei "529900NZYRSKAV232H08"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900nzyrskav232h08)

;; Allianz Renewable Energy Partners Luxembourg VIII S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg VIII S.A."
    :lei "52990072CZV57R3LDN75"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_52990072czv57r3ldn75)

;; Allianz Renewable Energy Partners Luxembourg II S.A.
(entity.ensure-limited-company
    :name "Allianz Renewable Energy Partners Luxembourg II S.A."
    :lei "52990057OWTKMND5RJ23"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_52990057owtkmnd5rj23)

;; Allianz Presse Infra S.C.S.
(entity.ensure-limited-company
    :name "Allianz Presse Infra S.C.S."
    :lei "52990032TDGU76MREG55"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "63P9"
    :as @lei_52990032tdgu76mreg55)

;; Allianz Finance X Luxembourg S.A.
(entity.ensure-limited-company
    :name "Allianz Finance X Luxembourg S.A."
    :lei "529900F0JX4Q55N28B14"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900f0jx4q55n28b14)

;; AZ-Argos 88 Vermögensverwaltungsgesellschaft mbH
(entity.ensure-limited-company
    :name "AZ-Argos 88 Vermögensverwaltungsgesellschaft mbH"
    :lei "529900FB0HPFPSOHMH46"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900fb0hpfpsohmh46)

;; AZL AI Nr. 1 GmbH
(entity.ensure-limited-company
    :name "AZL AI Nr. 1 GmbH"
    :lei "529900LD38JYGEBBNA70"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900ld38jygebbna70)

;; ARE Funds APK GmbH
(entity.ensure-limited-company
    :name "ARE Funds APK GmbH"
    :lei "8755001LHZQUSBXLMK93"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_8755001lhzqusbxlmk93)

;; Allianz Holding eins GmbH
(entity.ensure-limited-company
    :name "Allianz Holding eins GmbH"
    :lei "9845000E0478C3CA6574"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "AXSB"
    :as @lei_9845000e0478c3ca6574)

;; ALLIANZ SERVICES PRIVATE LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ SERVICES PRIVATE LIMITED"
    :lei "33580069FYH9OR7Z3B11"
    :jurisdiction "IN"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "TRIVANDRUM"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "YSP9"
    :as @lei_33580069fyh9or7z3b11)

;; Allianz Strategic Investments S.a r.l
(entity.ensure-limited-company
    :name "Allianz Strategic Investments S.a r.l"
    :lei "529900B1YQQQ0BH7UQ67"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900b1yqqq0bh7uq67)

;; Allianz PCREL US Debt S.A.
(entity.ensure-limited-company
    :name "Allianz PCREL US Debt S.A."
    :lei "529900P6A249CO4TUW77"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900p6a249co4tuw77)

;; LIVERPOOL VICTORIA GENERAL INSURANCE GROUP LIMITED
(entity.ensure-limited-company
    :name "LIVERPOOL VICTORIA GENERAL INSURANCE GROUP LIMITED"
    :lei "213800LCZNQNMVLS8T93"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800lcznqnmvls8t93)

;; HIGHWAY INSURANCE GROUP LIMITED
(entity.ensure-limited-company
    :name "HIGHWAY INSURANCE GROUP LIMITED"
    :lei "213800LPY1CUSQCBNC84"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800lpy1cusqcbnc84)

;; LV REPAIR SERVICES LIMITED
(entity.ensure-limited-company
    :name "LV REPAIR SERVICES LIMITED"
    :lei "213800XRI2UO3DTMCP62"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800xri2uo3dtmcp62)

;; BUDDIES ENTERPRISES LIMITED
(entity.ensure-limited-company
    :name "BUDDIES ENTERPRISES LIMITED"
    :lei "213800IYLUUCV9A4KM18"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800iyluucv9a4km18)

;; Allianz Nigeria Insurance PLC
(entity.ensure-limited-company
    :name "Allianz Nigeria Insurance PLC"
    :lei "529900RS2GZ9RI7H5G67"
    :jurisdiction "NG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Lagos"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "9999"
    :as @lei_529900rs2gz9ri7h5g67)

;; Allvest Active Invest
(entity.ensure-limited-company
    :name "Allvest Active Invest"
    :lei "549300VFU8KQQT7OXT28"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "FUND"
    :legal-form-code "UDY2"
    :as @lei_549300vfu8kqqt7oxt28)

;; Allvest Passive Invest
(entity.ensure-limited-company
    :name "Allvest Passive Invest"
    :lei "5493009K3LX6KM2X7P46"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "FUND"
    :legal-form-code "UDY2"
    :as @lei_5493009k3lx6km2x7p46)

;; Allianz Africa Holding GmbH
(entity.ensure-limited-company
    :name "Allianz Africa Holding GmbH"
    :lei "5299002KTF9OPJYICL36"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_5299002ktf9opjyicl36)

;; AZL-Private Finance GmbH
(entity.ensure-limited-company
    :name "AZL-Private Finance GmbH"
    :lei "529900K46B2UM3NVLO79"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Stuttgart"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900k46b2um3nvlo79)

;; Societe Fonciere Europeenne B.V.
(entity.ensure-limited-company
    :name "Societe Fonciere Europeenne B.V."
    :lei "529900QJBZ9AUO5VSC58"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Amsterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "54M6"
    :as @lei_529900qjbz9auo5vsc58)

;; EULER HERMES SERVIÇOS DE GESTÃO DE RISCOS LTDA.
(entity.ensure-limited-company
    :name "EULER HERMES SERVIÇOS DE GESTÃO DE RISCOS LTDA."
    :lei "213800TZITJMQX3AMU18"
    :jurisdiction "BR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "SÃO PAULO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "QHW1"
    :as @lei_213800tzitjmqx3amu18)

;; MORNINGCHAPTER, S.A.
(entity.ensure-limited-company
    :name "MORNINGCHAPTER, S.A."
    :lei "213800O8CFYJSAOHZA57"
    :jurisdiction "PT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "OURIQUE"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DFE5"
    :as @lei_213800o8cfyjsaohza57)

;; Allianz Fund Investments S.A.
(entity.ensure-limited-company
    :name "Allianz Fund Investments S.A."
    :lei "529900LSHMN704CFI287"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900lshmn704cfi287)

;; ALLIANZ GLOBAL INVESTORS UK LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ GLOBAL INVESTORS UK LIMITED"
    :lei "5299002MF5D6UOVE1355"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "London"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_5299002mf5d6uove1355)

;; Asit Services SRL
(entity.ensure-limited-company
    :name "Asit Services SRL"
    :lei "529900LMAQU5W26D1610"
    :jurisdiction "RO"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bucuresti"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "XHN1"
    :as @lei_529900lmaqu5w26d1610)

;; ARE Funds AZV GmbH
(entity.ensure-limited-company
    :name "ARE Funds AZV GmbH"
    :lei "875500HFLFSVMV4EUM36"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_875500hflfsvmv4eum36)

;; ARE Funds APKV GmbH
(entity.ensure-limited-company
    :name "ARE Funds APKV GmbH"
    :lei "875500J4XLZYBUF6RO74"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_875500j4xlzybuf6ro74)

;; ARE Funds AZL GmbH
(entity.ensure-limited-company
    :name "ARE Funds AZL GmbH"
    :lei "875500F9Y1LL071D5K73"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_875500f9y1ll071d5k73)

;; Vanilla Capital Markets S.A.
(entity.ensure-limited-company
    :name "Vanilla Capital Markets S.A."
    :lei "529900MGWMHZWMQYZW11"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900mgwmhzwmqyzw11)

;; Allianz Finance IX Luxembourg S.A.
(entity.ensure-limited-company
    :name "Allianz Finance IX Luxembourg S.A."
    :lei "5299000AZ2SS47C58B15"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_5299000az2ss47c58b15)

;; ALLIANZ GLOBAL CORPORATE & SPECIALTY DO BRASIL PARTICIPACOES LTDA.
(entity.ensure-limited-company
    :name "ALLIANZ GLOBAL CORPORATE & SPECIALTY DO BRASIL PARTICIPACOES LTDA."
    :lei "529900CI1I7BOU4EZ770"
    :jurisdiction "BR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "São Paulo"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "QHW1"
    :as @lei_529900ci1i7bou4ez770)

;; Allianz Real Estate Investment S.A.
(entity.ensure-limited-company
    :name "Allianz Real Estate Investment S.A."
    :lei "529900X3NQY79FO4O250"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900x3nqy79fo4o250)

;; EULER HERMES AUSTRALIA PTY LIMITED
(entity.ensure-limited-company
    :name "EULER HERMES AUSTRALIA PTY LIMITED"
    :lei "2549008IFT5K1LFPWX32"
    :jurisdiction "AU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sydney"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "TXVC"
    :as @lei_2549008ift5k1lfpwx32)

;; AWP Assistance (India) Private Limited
(entity.ensure-limited-company
    :name "AWP Assistance (India) Private Limited"
    :lei "529900FYNJQQ48K35E40"
    :jurisdiction "IN"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Gurgaon"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "YSP9"
    :as @lei_529900fynjqq48k35e40)

;; EULER HERMES COLLECTIONS SPÓŁKA Z OGRANICZONĄ ODPOWIEDZIALNOŚCIĄ
(entity.ensure-limited-company
    :name "EULER HERMES COLLECTIONS SPÓŁKA Z OGRANICZONĄ ODPOWIEDZIALNOŚCIĄ"
    :lei "259400AEKREN0R9K8253"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WARSZAWA"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "O7XB"
    :as @lei_259400aekren0r9k8253)

;; Allianz Infrastructure Luxembourg II S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg II S.à r.l."
    :lei "529900ED4HQN430UYW15"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900ed4hqn430uyw15)

;; บริษัท บีเอสเอ็มซี (ประเทศไทย) จำกัด
(entity.ensure-limited-company
    :name "บริษัท บีเอสเอ็มซี (ประเทศไทย) จำกัด"
    :lei "52990011DMJPYEB0FD90"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bangkok"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K09K"
    :as @lei_52990011dmjpyeb0fd90)

;; บริษัท ซีพีอาร์เอ็น (ประเทศไทย) จำกัด
(entity.ensure-limited-company
    :name "บริษัท ซีพีอาร์เอ็น (ประเทศไทย) จำกัด"
    :lei "529900ZYV65DZCT32I95"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bangkok"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K09K"
    :as @lei_529900zyv65dzct32i95)

;; บริษัท เจซีอาร์ อินเตอร์เทรด จำกัด
(entity.ensure-limited-company
    :name "บริษัท เจซีอาร์ อินเตอร์เทรด จำกัด"
    :lei "529900SN82Y8A2342J57"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bangkok"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K09K"
    :as @lei_529900sn82y8a2342j57)

;; บริษัท เอสโอเอฟอี วัน จำกัด
(entity.ensure-limited-company
    :name "บริษัท เอสโอเอฟอี วัน จำกัด"
    :lei "529900WE7CXW12UBMV28"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bangkok"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K09K"
    :as @lei_529900we7cxw12ubmv28)

;; บริษัท เอสโอเอฟอี ทู จำกัด
(entity.ensure-limited-company
    :name "บริษัท เอสโอเอฟอี ทู จำกัด"
    :lei "52990023XOWCXJK03D45"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "PATHUM WAN, BANGKOK"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "K09K"
    :as @lei_52990023xowcxjk03d45)

;; ALLIANZ FINANCE CORPORATION
(entity.ensure-limited-company
    :name "ALLIANZ FINANCE CORPORATION"
    :lei "549300LX561OW737I485"
    :jurisdiction "US-DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WILMINGTON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "XTIQ"
    :as @lei_549300lx561ow737i485)

;; Allianz Global Corporate & Specialty of Bermuda Ltd
(entity.ensure-limited-company
    :name "Allianz Global Corporate & Specialty of Bermuda Ltd"
    :lei "549300CE8K2G6TNANR36"
    :jurisdiction "US-NY"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "New York"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_549300ce8k2g6tnanr36)

;; Euler Hermes Collections GmbH
(entity.ensure-limited-company
    :name "Euler Hermes Collections GmbH"
    :lei "529900JARUYIRDXT3J21"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Potsdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900jaruyirdxt3j21)

;; Euler Hermes Risk Yönetimi ve Danışmanlık Hizmetleri Limited Şirketi
(entity.ensure-limited-company
    :name "Euler Hermes Risk Yönetimi ve Danışmanlık Hizmetleri Limited Şirketi"
    :lei "529900SIE4S7C505KB38"
    :jurisdiction "TR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Istanbul"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "W2SQ"
    :as @lei_529900sie4s7c505kb38)

;; PET PLAN LIMITED
(entity.ensure-limited-company
    :name "PET PLAN LIMITED"
    :lei "213800RCUHURMG5EFJ51"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800rcuhurmg5efj51)

;; ALLIANZ PROPERTIES LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ PROPERTIES LIMITED"
    :lei "213800UKLJ4OWNE65Y83"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800uklj4owne65y83)

;; ALLIANZ ENGINEERING INSPECTION SERVICES LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ ENGINEERING INSPECTION SERVICES LIMITED"
    :lei "213800GEVVCUAFNVET58"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800gevvcuafnvet58)

;; PETIOS LIMITED
(entity.ensure-limited-company
    :name "PETIOS LIMITED"
    :lei "21380092XCNGK4CL2M64"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_21380092xcngk4cl2m64)

;; ALLIANZ HOLDINGS PLC
(entity.ensure-limited-company
    :name "ALLIANZ HOLDINGS PLC"
    :lei "21380039ET3UD11RBS65"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "LONDON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "B6ES"
    :as @lei_21380039et3ud11rbs65)

;; ALLIANZ (UK) LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ (UK) LIMITED"
    :lei "213800UHGR8BPHS6RQ67"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "LONDON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800uhgr8bphs6rq67)

;; ALLIANZ PENSION FUND TRUSTEES LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ PENSION FUND TRUSTEES LIMITED"
    :lei "213800MMPMNKCVTR4375"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "LONDON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800mmpmnkcvtr4375)

;; ALLIANZ UK DISTRIBUTION LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ UK DISTRIBUTION LIMITED"
    :lei "213800ZXNHYS9YGMS961"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "LONDON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800zxnhys9ygms961)

;; ALLIANZ EQUITY INVESTMENTS LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ EQUITY INVESTMENTS LIMITED"
    :lei "213800L7M9UC5STANH88"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800l7m9uc5stanh88)

;; ALLIANZ MANAGEMENT SERVICES LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ MANAGEMENT SERVICES LIMITED"
    :lei "213800RNA78IJUC16I17"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "LONDON"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_213800rna78ijuc16i17)

;; Atropos Vermögensverwaltungsgesellschaft mbH
(entity.ensure-limited-company
    :name "Atropos Vermögensverwaltungsgesellschaft mbH"
    :lei "5299001S9EMYFVIZ2613"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_5299001s9emyfviz2613)

;; Allianz of Asia-Pacific and Africa GmbH
(entity.ensure-limited-company
    :name "Allianz of Asia-Pacific and Africa GmbH"
    :lei "5299002P6NBPK3SMF889"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_5299002p6nbpk3smf889)

;; Arges Investments II N.V.
(entity.ensure-limited-company
    :name "Arges Investments II N.V."
    :lei "529900JFZNCO71G4UB59"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Amsterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "B5PM"
    :as @lei_529900jfznco71g4ub59)

;; Arges Investments I N.V.
(entity.ensure-limited-company
    :name "Arges Investments I N.V."
    :lei "529900NREB0L9FEPXM52"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Amsterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "B5PM"
    :as @lei_529900nreb0l9fepxm52)

;; AP Solutions GmbH
(entity.ensure-limited-company
    :name "AP Solutions GmbH"
    :lei "529900O99GMU3P8U0S07"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900o99gmu3p8u0s07)

;; Allianz Global Investors Asia Pacific Limited
(entity.ensure-limited-company
    :name "Allianz Global Investors Asia Pacific Limited"
    :lei "549300J4ASJ4UGJ5R887"
    :jurisdiction "HK"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Hong Kong"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "254M"
    :as @lei_549300j4asj4ugj5r887)

;; Euler Hermes Aktiengesellschaft
(entity.ensure-limited-company
    :name "Euler Hermes Aktiengesellschaft"
    :lei "529900WXN7CL3XEECH32"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Hamburg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900wxn7cl3xeech32)

;; CIC ALLIANZ INSURANCE LIMITED
(entity.ensure-limited-company
    :name "CIC ALLIANZ INSURANCE LIMITED"
    :lei "549300GJ8MPHZKLG9N18"
    :jurisdiction "AU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sydney"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "R4KK"
    :as @lei_549300gj8mphzklg9n18)

;; ALLIANZ TECHNOLOGY S.L.
(entity.ensure-limited-company
    :name "ALLIANZ TECHNOLOGY S.L."
    :lei "529900VGY0TXJIYVBT39"
    :jurisdiction "ES"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Barcelona"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "JB2M"
    :as @lei_529900vgy0txjiyvbt39)

;; Allianz - Sociedade Gestora de Fundos de Pensões, S.A.
(entity.ensure-limited-company
    :name "Allianz - Sociedade Gestora de Fundos de Pensões, S.A."
    :lei "5299000X6LJCR7K03Z61"
    :jurisdiction "PT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Lisboa"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DFE5"
    :as @lei_5299000x6ljcr7k03z61)

;; ALLIANZ HOLDING FRANCE
(entity.ensure-limited-company
    :name "ALLIANZ HOLDING FRANCE"
    :lei "969500CUK3OMCMPMWR55"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "PUTEAUX"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_969500cuk3omcmpmwr55)

;; Allianz Finance II B.V.
(entity.ensure-limited-company
    :name "Allianz Finance II B.V."
    :lei "529900C9NVPTCPDI1D65"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Amsterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "54M6"
    :as @lei_529900c9nvptcpdi1d65)

;; Allianz Finance III B.V.
(entity.ensure-limited-company
    :name "Allianz Finance III B.V."
    :lei "5299000TG8YATYNK8P87"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Amsterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "54M6"
    :as @lei_5299000tg8yatynk8p87)

;; AZ-Arges Vermögensverwaltungsgesellschaft mbH
(entity.ensure-limited-company
    :name "AZ-Arges Vermögensverwaltungsgesellschaft mbH"
    :lei "529900XVKEQSZ25VYB06"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900xvkeqsz25vyb06)

;; ALLIANZ RISK TRANSFER, INC.
(entity.ensure-limited-company
    :name "ALLIANZ RISK TRANSFER, INC."
    :lei "549300SC8ZD5MU1TC314"
    :jurisdiction "US-NY"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "NEW YORK"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "PJ10"
    :as @lei_549300sc8zd5mu1tc314)

;; ALLIANZ AFRICA
(entity.ensure-limited-company
    :name "ALLIANZ AFRICA"
    :lei "9695009HV2986MDEQ760"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "PARIS"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_9695009hv2986mdeq760)

;; ALLIANZ SPÓŁKA Z OGRANICZONĄ ODPOWIEDZIALNOŚCIĄ
(entity.ensure-limited-company
    :name "ALLIANZ SPÓŁKA Z OGRANICZONĄ ODPOWIEDZIALNOŚCIĄ"
    :lei "2138003C8U4NUDKTGR85"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WARSZAWA"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "O7XB"
    :as @lei_2138003c8u4nudktgr85)

;; Euler Hermes Asset Management France
(entity.ensure-limited-company
    :name "Euler Hermes Asset Management France"
    :lei "5299004OAO2LCDHTJ514"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "PARIS LA DEFENSE Cedex"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_5299004oao2lcdhtj514)

;; Allianz Technology SAS
(entity.ensure-limited-company
    :name "Allianz Technology SAS"
    :lei "5299007NQRNTW1EYYN89"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Saint-Ouen"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_5299007nqrntw1eyyn89)

;; TOWARZYSTWO UBEZPIECZEŃ EULER HERMES SPÓŁKA AKCYJNA
(entity.ensure-limited-company
    :name "TOWARZYSTWO UBEZPIECZEŃ EULER HERMES SPÓŁKA AKCYJNA"
    :lei "259400UNFL1GUH63DE55"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WARSZAWA"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "FJ0E"
    :as @lei_259400unfl1guh63de55)

;; ALLIANZ NEXT S.P.A.
(entity.ensure-limited-company
    :name "ALLIANZ NEXT S.P.A."
    :lei "815600A7E126B8274502"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MILANO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "X32V"
    :as @lei_815600a7e126b8274502)

;; POWSZECHNE TOWARZYSTWO EMERYTALNE ALLIANZ POLSKA SPÓŁKA AKCYJNA
(entity.ensure-limited-company
    :name "POWSZECHNE TOWARZYSTWO EMERYTALNE ALLIANZ POLSKA SPÓŁKA AKCYJNA"
    :lei "259400GYKYKIQO2AY336"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WARSZAWA"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "FJ0E"
    :as @lei_259400gykykiqo2ay336)

;; Allianz Pensionskasse Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Pensionskasse Aktiengesellschaft"
    :lei "529900R7CSE082VKF992"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "EQOV"
    :as @lei_529900r7cse082vkf992)

;; ALLIANZ LIFE INSURANCE LANKA LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ LIFE INSURANCE LANKA LIMITED"
    :lei "549300PW0UOTOKNSVO97"
    :jurisdiction "LK"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "COLOMBO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "9999"
    :as @lei_549300pw0uotoknsvo97)

;; ALLIANZ-TIRIAC PENSII PRIVATE SOCIETATE DE ADMINISTRARE A FONDURILOR DE PENSII PRIVATE SA
(entity.ensure-limited-company
    :name "ALLIANZ-TIRIAC PENSII PRIVATE SOCIETATE DE ADMINISTRARE A FONDURILOR DE PENSII PRIVATE SA"
    :lei "213800EMXABRC8G7O674"
    :jurisdiction "RO"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "BUCUREȘTI, SECTORUL 1"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "ANDT"
    :as @lei_213800emxabrc8g7o674)

;; Allianz Pensionsfonds Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Pensionsfonds Aktiengesellschaft"
    :lei "529900QIECQ5ML8O8P18"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Stuttgart"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900qiecq5ml8o8p18)

;; Deutsche Lebensversicherungs-Aktiengesellschaft
(entity.ensure-limited-company
    :name "Deutsche Lebensversicherungs-Aktiengesellschaft"
    :lei "529900YI4HYCORU97L35"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Berlin"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900yi4hycoru97l35)

;; Allianz Direct Versicherungs-AG
(entity.ensure-limited-company
    :name "Allianz Direct Versicherungs-AG"
    :lei "5299008FXA9QQZ79GM59"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_5299008fxa9qqz79gm59)

;; Volkswagen Autoversicherung AG
(entity.ensure-limited-company
    :name "Volkswagen Autoversicherung AG"
    :lei "529900MXPCB0TV1TVJ64"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Braunschweig"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900mxpcb0tv1tvj64)

;; Allianz Agrar AG
(entity.ensure-limited-company
    :name "Allianz Agrar AG"
    :lei "5299006N81IPKYWADC44"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_5299006n81ipkywadc44)

;; ADEUS Aktienregister-Service-GmbH
(entity.ensure-limited-company
    :name "ADEUS Aktienregister-Service-GmbH"
    :lei "391200YYKVSXR85NTU31"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_391200yykvsxr85ntu31)

;; Allianz Vorsorgekasse AG
(entity.ensure-limited-company
    :name "Allianz Vorsorgekasse AG"
    :lei "5299007024XT1N1WQ539"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "EQOV"
    :as @lei_5299007024xt1n1wq539)

;; Euler Hermes Patrimonia
(entity.ensure-limited-company
    :name "Euler Hermes Patrimonia"
    :lei "5299001Z6DYJG67B4298"
    :jurisdiction "BE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Brussel"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "R85P"
    :as @lei_5299001z6dyjg67b4298)

;; ALLIANZ MENA HOLDING (BERMUDA) LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ MENA HOLDING (BERMUDA) LIMITED"
    :lei "549300PO5O1Z7LWWZF63"
    :jurisdiction "BM"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Hamilton"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "7AS7"
    :as @lei_549300po5o1z7lwwzf63)

;; CAP, Rechtsschutz-Versicherungsgesellschaft AG
(entity.ensure-limited-company
    :name "CAP, Rechtsschutz-Versicherungsgesellschaft AG"
    :lei "529900JCO0G42Q4RXW52"
    :jurisdiction "CH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wallisellen"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "MVII"
    :as @lei_529900jco0g42q4rxw52)

;; บริษัท อลิอันซ์ อยุธยา ประกันชีวิต จำกัด (มหาชน)
(entity.ensure-limited-company
    :name "บริษัท อลิอันซ์ อยุธยา ประกันชีวิต จำกัด (มหาชน)"
    :lei "5299000VHRS2VTQSYM59"
    :jurisdiction "TH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bangkok"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "O0OZ"
    :as @lei_5299000vhrs2vtqsym59)

;; EULER HERMES SERVICES UK LIMITED
(entity.ensure-limited-company
    :name "EULER HERMES SERVICES UK LIMITED"
    :lei "529900YFJCLKSS39VM62"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "London"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_529900yfjclkss39vm62)

;; EULER HERMES SINGAPORE SERVICES PTE. LTD.
(entity.ensure-limited-company
    :name "EULER HERMES SINGAPORE SERVICES PTE. LTD."
    :lei "529900TR00UOR38YIA65"
    :jurisdiction "SG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Singapore"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "LWXI"
    :as @lei_529900tr00uor38yia65)

;; EULER HERMES NORTH AMERICA INSURANCE COMPANY
(entity.ensure-limited-company
    :name "EULER HERMES NORTH AMERICA INSURANCE COMPANY"
    :lei "529900MZO2VQ5616L328"
    :jurisdiction "US-MD"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "BALTIMORE"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "HLR4"
    :as @lei_529900mzo2vq5616l328)

;; Euler Hermes Hong Kong Services Limited
(entity.ensure-limited-company
    :name "Euler Hermes Hong Kong Services Limited"
    :lei "529900O1ST5IYTI97S88"
    :jurisdiction "HK"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Taikoo Shing"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "254M"
    :as @lei_529900o1st5iyti97s88)

;; EULER HERMES
(entity.ensure-limited-company
    :name "EULER HERMES"
    :lei "52990053AH5LF0YZWD07"
    :jurisdiction "BE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Brussel"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "R85P"
    :as @lei_52990053ah5lf0yzwd07)

;; Euler Hermes Reinsurance AG
(entity.ensure-limited-company
    :name "Euler Hermes Reinsurance AG"
    :lei "5299006NV9SQA4XFTB22"
    :jurisdiction "CH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wallisellen"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "MVII"
    :as @lei_5299006nv9sqa4xftb22)

;; Euler Hermes Services SAS
(entity.ensure-limited-company
    :name "Euler Hermes Services SAS"
    :lei "5299007V9H7DDUANPV51"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Paris-La-Défense Cédex"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_5299007v9h7dduanpv51)

;; EULER HERMES GROUP
(entity.ensure-limited-company
    :name "EULER HERMES GROUP"
    :lei "529900AJFTU1CPN1X176"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Paris-La-Défense Cédex"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_529900ajftu1cpn1x176)

;; ALLIANZ LIFE INSURANCE JAPAN LTD.
(entity.ensure-limited-company
    :name "ALLIANZ LIFE INSURANCE JAPAN LTD."
    :lei "549300HRIYJIWUR34Y94"
    :jurisdiction "JP"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MINATO KU"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "T417"
    :as @lei_549300hriyjiwur34y94)

;; АЛИАНЦ БЪЛГАРИЯ ХОЛДИНГ
(entity.ensure-limited-company
    :name "АЛИАНЦ БЪЛГАРИЯ ХОЛДИНГ"
    :lei "529900NJYUGRO908KV84"
    :jurisdiction "BG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sofia"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "WTK4"
    :as @lei_529900njyugro908kv84)

;; Allianz Sigorta Anonim Şirketi
(entity.ensure-limited-company
    :name "Allianz Sigorta Anonim Şirketi"
    :lei "7890006U2TVGMCPE3F49"
    :jurisdiction "TR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "İSTANBUL"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "W2SQ"
    :as @lei_7890006u2tvgmcpe3f49)

;; Top VS GmbH
(entity.ensure-limited-company
    :name "Top VS GmbH"
    :lei "5299001N8J3IUQ4E9110"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "AXSB"
    :as @lei_5299001n8j3iuq4e9110)

;; Allianz Capital Partners GmbH
(entity.ensure-limited-company
    :name "Allianz Capital Partners GmbH"
    :lei "529900LP85FZLRHOP912"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900lp85fzlrhop912)

;; PIMCO Prime Real Estate GmbH
(entity.ensure-limited-company
    :name "PIMCO Prime Real Estate GmbH"
    :lei "5299009IY3NJ46YAAC63"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_5299009iy3nj46yaac63)

;; ALLIANZ MEXICO SA COMPAÑIA DE SEGUROS
(entity.ensure-limited-company
    :name "ALLIANZ MEXICO SA COMPAÑIA DE SEGUROS"
    :lei "549300I24TYYGCT38U98"
    :jurisdiction "MX"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Mexico City"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "B8CE"
    :as @lei_549300i24tyygct38u98)

;; Allianz Asset Management GmbH
(entity.ensure-limited-company
    :name "Allianz Asset Management GmbH"
    :lei "529900ASFI2IZU3QYD26"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900asfi2izu3qyd26)

;; Allianz Technology SE
(entity.ensure-limited-company
    :name "Allianz Technology SE"
    :lei "529900D4X8B3UWGFCX06"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "SGST"
    :as @lei_529900d4x8b3uwgfcx06)

;; ЗАСТРАХОВАТЕЛНО АКЦИОНЕРНО ДРУЖЕСТВО "ЕНЕРГИЯ"
(entity.ensure-limited-company
    :name "ЗАСТРАХОВАТЕЛНО АКЦИОНЕРНО ДРУЖЕСТВО \"ЕНЕРГИЯ\""
    :lei "529900AY9GPDH3OQF009"
    :jurisdiction "BG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sofia"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "WTK4"
    :as @lei_529900ay9gpdh3oqf009)

;; Allianz Alapkezelő Zrt.
(entity.ensure-limited-company
    :name "Allianz Alapkezelő Zrt."
    :lei "5299000EII0XC5VJIO94"
    :jurisdiction "HU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Budapest"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "BKUX"
    :as @lei_5299000eii0xc5vjio94)

;; Allianz Nederland Groep N.V.
(entity.ensure-limited-company
    :name "Allianz Nederland Groep N.V."
    :lei "724500P01O2EB9B45325"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Rotterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "B5PM"
    :as @lei_724500p01o2eb9b45325)

;; YAO NEWREP Investments S.A.
(entity.ensure-limited-company
    :name "YAO NEWREP Investments S.A."
    :lei "529900AE1WSQ5GZXBF12"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_529900ae1wsq5gzxbf12)

;; Allianz Finance II Luxembourg S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Finance II Luxembourg S.à r.l."
    :lei "5299007FSUGQCW1R8I33"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_5299007fsugqcw1r8i33)

;; COMPANHIA DE SEGUROS ALLIANZ PORTUGAL, S.A.
(entity.ensure-limited-company
    :name "COMPANHIA DE SEGUROS ALLIANZ PORTUGAL, S.A."
    :lei "529900LP62SEK9MXDB79"
    :jurisdiction "PT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Lisboa"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DFE5"
    :as @lei_529900lp62sek9mxdb79)

;; IDS GmbH - Analysis and Reporting Services
(entity.ensure-limited-company
    :name "IDS GmbH - Analysis and Reporting Services"
    :lei "529900PQUHKZJAAWX304"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "2HBR"
    :as @lei_529900pquhkzjaawx304)

;; ALLIANZ PARTNERS SAS
(entity.ensure-limited-company
    :name "ALLIANZ PARTNERS SAS"
    :lei "969500GNHCXXTP2EL222"
    :jurisdiction "FR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "SAINT-OUEN-SUR-SEINE"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CHY"
    :as @lei_969500gnhcxxtp2el222)

;; ALLIANZ COMPAÑIA DE SEGUROS Y REASEGUROS SA
(entity.ensure-limited-company
    :name "ALLIANZ COMPAÑIA DE SEGUROS Y REASEGUROS SA"
    :lei "529900E0961XXFO5Z292"
    :jurisdiction "ES"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Madrid"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5RDO"
    :as @lei_529900e0961xxfo5z292)

;; INVESTITORI SOCIETA' DI GESTIONE DEL RISPARMIO S.P.A.
(entity.ensure-limited-company
    :name "INVESTITORI SOCIETA' DI GESTIONE DEL RISPARMIO S.P.A."
    :lei "529900BUVMOECVUTQO64"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Milano"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "X32V"
    :as @lei_529900buvmoecvutqo64)

;; Allianz Europe B.V.
(entity.ensure-limited-company
    :name "Allianz Europe B.V."
    :lei "529900PVKWU48UKGOC87"
    :jurisdiction "NL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Amsterdam"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "54M6"
    :as @lei_529900pvkwu48ukgoc87)

;; Allianz Deutschland AG
(entity.ensure-limited-company
    :name "Allianz Deutschland AG"
    :lei "529900CRHRWZ5DB8BK41"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900crhrwz5db8bk41)

;; Allianz Beratungs- und Vertriebs-AG
(entity.ensure-limited-company
    :name "Allianz Beratungs- und Vertriebs-AG"
    :lei "529900X0YREMYUI5MX73"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900x0yremyui5mx73)

;; Allianz Finance VII Luxembourg S.A.
(entity.ensure-limited-company
    :name "Allianz Finance VII Luxembourg S.A."
    :lei "52990093ZNFUHOSM9498"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "5GGB"
    :as @lei_52990093znfuhosm9498)

;; Allianz Infrastructure Luxembourg I S.à r.l.
(entity.ensure-limited-company
    :name "Allianz Infrastructure Luxembourg I S.à r.l."
    :lei "5299006228ACTH08JX97"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_5299006228acth08jx97)

;; Allianz European Reliance Single Member Insurance S.A.
(entity.ensure-limited-company
    :name "Allianz European Reliance Single Member Insurance S.A."
    :lei "529900SUMKB7MEIJWP03"
    :jurisdiction "GR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Athens"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_529900sumkb7meijwp03)

;; ALLIANZ EUROPE LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ EUROPE LIMITED"
    :lei "529900EO37QGS7QP0F54"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "London"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "H0PO"
    :as @lei_529900eo37qgs7qp0f54)

;; ALLIANZ - TIRIAC ASIGURARI SA
(entity.ensure-limited-company
    :name "ALLIANZ - TIRIAC ASIGURARI SA"
    :lei "529900XKNXM9MBH8GS45"
    :jurisdiction "RO"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Bucureşti"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "ANDT"
    :as @lei_529900xknxm9mbh8gs45)

;; Allianz Finance IV Luxembourg s.à r.l.
(entity.ensure-limited-company
    :name "Allianz Finance IV Luxembourg s.à r.l."
    :lei "529900PY7D6FGPYQPH76"
    :jurisdiction "LU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Luxembourg"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "DVXS"
    :as @lei_529900py7d6fgpyqph76)

;; Allianz Hungária Biztosító Zártkörűen Működő Részvénytársaság
(entity.ensure-limited-company
    :name "Allianz Hungária Biztosító Zártkörűen Működő Részvénytársaság"
    :lei "529900IJSHSLTES6PQ72"
    :jurisdiction "HU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Budapest"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "BKUX"
    :as @lei_529900ijshsltes6pq72)

;; Allianz Hrvatska dioničko društvo za osiguranje
(entity.ensure-limited-company
    :name "Allianz Hrvatska dioničko društvo za osiguranje"
    :lei "5493006D8G55YM441622"
    :jurisdiction "HR"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Zagreb"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "LUMA"
    :as @lei_5493006d8g55ym441622)

;; TOWARZYSTWO UBEZPIECZEŃ I REASEKURACJI ALLIANZ POLSKA SPÓŁKA AKCYJNA
(entity.ensure-limited-company
    :name "TOWARZYSTWO UBEZPIECZEŃ I REASEKURACJI ALLIANZ POLSKA SPÓŁKA AKCYJNA"
    :lei "259400MDL4OD6BLVIB72"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WARSZAWA"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "FJ0E"
    :as @lei_259400mdl4od6blvib72)

;; TOWARZYSTWO UBEZPIECZEŃ ALLIANZ ŻYCIE POLSKA SPÓŁKA AKCYJNA
(entity.ensure-limited-company
    :name "TOWARZYSTWO UBEZPIECZEŃ ALLIANZ ŻYCIE POLSKA SPÓŁKA AKCYJNA"
    :lei "259400IBCICD0KY7ZW46"
    :jurisdiction "PL"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "WARSZAWA"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "FJ0E"
    :as @lei_259400ibcicd0ky7zw46)

;; ЗАД "АЛИАНЦ БЪЛГАРИЯ"
(entity.ensure-limited-company
    :name "ЗАД \"АЛИАНЦ БЪЛГАРИЯ\""
    :lei "529900BNGN523NOYWP15"
    :jurisdiction "BG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sofia"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "WTK4"
    :as @lei_529900bngn523noywp15)

;; ПЕНСИОННО ОСИГУРИТЕЛНО ДРУЖЕСТВО АЛИАНЦ БЪЛГАРИЯ
(entity.ensure-limited-company
    :name "ПЕНСИОННО ОСИГУРИТЕЛНО ДРУЖЕСТВО АЛИАНЦ БЪЛГАРИЯ"
    :lei "529900B6DRCZ3ROAQW27"
    :jurisdiction "BG"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sofia"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "WTK4"
    :as @lei_529900b6drcz3roaqw27)

;; ALLIANZ SOCIETA' PER AZIONI IN FORMA ABBREVIATA "ALLIANZ S.P.A."
(entity.ensure-limited-company
    :name "ALLIANZ SOCIETA' PER AZIONI IN FORMA ABBREVIATA \"ALLIANZ S.P.A.\""
    :lei "529900UGESEV6GHUN018"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MILANO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "P418"
    :as @lei_529900ugesev6ghun018)

;; UNICREDIT VITA ASSICURAZIONI S.P.A.
(entity.ensure-limited-company
    :name "UNICREDIT VITA ASSICURAZIONI S.P.A."
    :lei "529900W51ZNEU53S1P78"
    :jurisdiction "IT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MILANO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "P418"
    :as @lei_529900w51zneu53s1p78)

;; Allianz Investment Management SE
(entity.ensure-limited-company
    :name "Allianz Investment Management SE"
    :lei "529900HLUAHG5YJSGB42"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "SGST"
    :as @lei_529900hluahg5yjsgb42)

;; ALLIANZ INSURANCE PLC
(entity.ensure-limited-company
    :name "ALLIANZ INSURANCE PLC"
    :lei "213800QXY6G66CQVB770"
    :jurisdiction "GB"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "GUILDFORD"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "B6ES"
    :as @lei_213800qxy6g66cqvb770)

;; Allianz Invest Kapitalanlagegesellschaft mbH
(entity.ensure-limited-company
    :name "Allianz Invest Kapitalanlagegesellschaft mbH"
    :lei "529900Y5ZGJRS7GG0D68"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "AXSB"
    :as @lei_529900y5zgjrs7gg0d68)

;; Allianz Elementar Versicherungs-Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Elementar Versicherungs-Aktiengesellschaft"
    :lei "529900ETI7480XT9MU29"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "EQOV"
    :as @lei_529900eti7480xt9mu29)

;; ALLIANZ RE DUBLIN DESIGNATED ACTIVITY COMPANY
(entity.ensure-limited-company
    :name "ALLIANZ RE DUBLIN DESIGNATED ACTIVITY COMPANY"
    :lei "529900KDXMUUS7EMLJ38"
    :jurisdiction "IE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "DUBLIN 4"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "LGWG"
    :as @lei_529900kdxmuus7emlj38)

;; ALLIANZ BENELUX
(entity.ensure-limited-company
    :name "ALLIANZ BENELUX"
    :lei "529900EU2PIG4IH6RF36"
    :jurisdiction "BE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Brussel"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "R85P"
    :as @lei_529900eu2pig4ih6rf36)

;; Allianz Elementar Lebensversicherungs-Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Elementar Lebensversicherungs-Aktiengesellschaft"
    :lei "5299003F8XGRHET9H154"
    :jurisdiction "AT"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wien"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "EQOV"
    :as @lei_5299003f8xgrhet9h154)

;; Allianz Suisse Versicherungs-Gesellschaft AG
(entity.ensure-limited-company
    :name "Allianz Suisse Versicherungs-Gesellschaft AG"
    :lei "529900HTG21VUCKUSU16"
    :jurisdiction "CH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wallisellen"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "MVII"
    :as @lei_529900htg21vuckusu16)

;; Allianz Suisse Lebensversicherungs-Gesellschaft AG
(entity.ensure-limited-company
    :name "Allianz Suisse Lebensversicherungs-Gesellschaft AG"
    :lei "529900J9ZH2YN87MPE59"
    :jurisdiction "CH"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Wallisellen"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "MVII"
    :as @lei_529900j9zh2yn87mpe59)

;; Allianz penzijní společnost, a.s.
(entity.ensure-limited-company
    :name "Allianz penzijní společnost, a.s."
    :lei "529900UM73NGF8E4YY91"
    :jurisdiction "CZ"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Praha 8"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CQN"
    :as @lei_529900um73ngf8e4yy91)

;; Allianz pojišťovna, a.s.
(entity.ensure-limited-company
    :name "Allianz pojišťovna, a.s."
    :lei "5299007KUKZ04LK29K58"
    :jurisdiction "CZ"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Praha 8"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6CQN"
    :as @lei_5299007kukz04lk29k58)

;; Allianz Lebensversicherungs-Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Lebensversicherungs-Aktiengesellschaft"
    :lei "529900Z5H1N62JMB3K96"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Stuttgart"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900z5h1n62jmb3k96)

;; Allianz Private Krankenversicherungs-Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Private Krankenversicherungs-Aktiengesellschaft"
    :lei "529900APQGQWPAT1YI78"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900apqgqwpat1yi78)

;; Allianz Pensionskasse Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Pensionskasse Aktiengesellschaft"
    :lei "529900J2RGEB3V10PJ36"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Stuttgart"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900j2rgeb3v10pj36)

;; Allianz Versicherungs-Aktiengesellschaft
(entity.ensure-limited-company
    :name "Allianz Versicherungs-Aktiengesellschaft"
    :lei "529900X5FHSYN4P5R285"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "6QQB"
    :as @lei_529900x5fhsyn4p5r285)

;; ALLIANZ GLOBAL LIFE DESIGNATED ACTIVITY COMPANY
(entity.ensure-limited-company
    :name "ALLIANZ GLOBAL LIFE DESIGNATED ACTIVITY COMPANY"
    :lei "529900ZJCA8LOT6XX119"
    :jurisdiction "IE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Dublin"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "LGWG"
    :as @lei_529900zjca8lot6xx119)

;; ALLIANZ RISK TRANSFER AG
(entity.ensure-limited-company
    :name "ALLIANZ RISK TRANSFER AG"
    :lei "5493005WW64PFITU7G71"
    :jurisdiction "LI"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "SCHAAN"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "7RRP"
    :as @lei_5493005ww64pfitu7g71)

;; ALLIANZ AUSTRALIA LIFE INSURANCE LIMITED
(entity.ensure-limited-company
    :name "ALLIANZ AUSTRALIA LIFE INSURANCE LIMITED"
    :lei "PGRZ8FTXX81EOGOTJZ28"
    :jurisdiction "AU"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "Sydney"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "R4KK"
    :as @lei_pgrz8ftxx81eogotjz28)

;; DARTA SAVING LIFE ASSURANCE DESIGNATED ACTIVITY COMPANY
(entity.ensure-limited-company
    :name "DARTA SAVING LIFE ASSURANCE DESIGNATED ACTIVITY COMPANY"
    :lei "WUYDW18YG7QXGWBK3804"
    :jurisdiction "IE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "DUBLIN"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "LGWG"
    :as @lei_wuydw18yg7qxgwbk3804)

;; ALLIANZ GLOBAL RISKS US INSURANCE COMPANY
(entity.ensure-limited-company
    :name "ALLIANZ GLOBAL RISKS US INSURANCE COMPANY"
    :lei "61CF7K34JWL1YFRK5K35"
    :jurisdiction "US"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "CHICAGO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_61cf7k34jwl1yfrk5k35)

;; Allianz Global Corporate & Specialty SE
(entity.ensure-limited-company
    :name "Allianz Global Corporate & Specialty SE"
    :lei "F240A7PWJB2BLKELB442"
    :jurisdiction "DE"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "München"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "SGST"
    :as @lei_f240a7pwjb2blkelb442)

;; FIREMAN'S FUND INSURANCE COMPANY
(entity.ensure-limited-company
    :name "FIREMAN'S FUND INSURANCE COMPANY"
    :lei "0JJ27TIZIU2LZJ1JYM80"
    :jurisdiction "US"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "CHICAGO"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_0jj27tiziu2lzj1jym80)
;; Skipped 1 entities already defined in earlier phases

;; ALLIANZ LIFE INSURANCE COMPANY OF NORTH AMERICA
(entity.ensure-limited-company
    :name "ALLIANZ LIFE INSURANCE COMPANY OF NORTH AMERICA"
    :lei "DKBD555YIJCQ30PMHF22"
    :jurisdiction "US"
    :direct-parent-lei "529900K9B0N5BT694847"
    :city "MINNEAPOLIS"
    :gleif-status "ACTIVE"
    :gleif-category "GENERAL"
    :legal-form-code "8888"
    :as @lei_dkbd555yijcq30pmhf22)

;; ============================================================================
;; END OF ALLIANZ GLEIF DATA LOAD
;; ============================================================================