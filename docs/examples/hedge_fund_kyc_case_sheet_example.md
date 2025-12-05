# KYC/UBO Case Sheet: Meridian Alpha Fund Ltd

## Executive Summary

**Client**: Meridian Alpha Fund Ltd  
**CBU Type**: Hedge Fund (FUND)  
**Jurisdiction**: Cayman Islands (KY)  
**Products**: Custody + Alternatives  
**Case Status**: **APPROVED**  
**Risk Rating**: MEDIUM  
**Case ID**: `CASE-MERIDIAN-2025-001`

---

## 1. Client Structure Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MERIDIAN ALPHA FUND LTD                          │
│                    (Cayman Islands - KY)                            │
│                    AUM: $850M | Strategy: Global Macro              │
└─────────────────────────────────────────────────────────────────────┘
                                    │
         ┌──────────────────────────┴──────────────────────────┐
         │                                                      │
         ▼                                                      ▼
┌─────────────────────────┐                    ┌─────────────────────────┐
│   Meridian Partners LP  │                    │  Meridian Investment    │
│   (Delaware - US)       │                    │  Management Ltd         │
│   Role: GENERAL_PARTNER │                    │  (UK - GB)              │
│   Control: 100%         │                    │  Role: INVESTMENT_MGR   │
└─────────────────────────┘                    └─────────────────────────┘
         │                                                      │
         │ 60%                                                  │ 40%
         ▼                                                      ▼
┌─────────────────────────┐                    ┌─────────────────────────┐
│   Marcus Chen           │                    │   Victoria Sterling     │
│   (US Citizen)          │                    │   (UK Citizen)          │
│   Role: UBO             │                    │   Role: UBO             │
│   Indirect: 60%         │                    │   Indirect: 40%         │
│   DOB: 1968-03-15       │                    │   DOB: 1972-08-22       │
└─────────────────────────┘                    └─────────────────────────┘
```

---

## 2. Complete DSL Program

```clojure
;; =============================================================================
;; MERIDIAN ALPHA FUND - KYC/UBO COMPLETE CASE SHEET
;; Products: Custody + Alternatives
;; Final Status: APPROVED | Risk Rating: MEDIUM
;; =============================================================================

;; =============================================================================
;; SECTION 1: CBU & ENTITY CREATION
;; =============================================================================

;; Create the commercial client head office (UK Investment Manager)
(entity.create-limited-company
  :name "Meridian Investment Management Ltd"
  :jurisdiction "GB"
  :registration-number "12345678"
  :as @investment-manager)

;; Create the fund CBU
(cbu.ensure
  :name "Meridian Alpha Fund Ltd"
  :jurisdiction "KY"
  :client-type "FUND"
  :nature-purpose "Global macro hedge fund pursuing absolute returns through directional and relative value strategies across equities, fixed income, currencies, and commodities"
  :source-of-funds "Institutional investor capital commitments and reinvested profits"
  :commercial-client-entity-id @investment-manager
  :product-id "prod-custody"
  :as @fund)

;; Create fund legal entity (the LP structure)
(entity.create-partnership-limited
  :partnership-name "Meridian Alpha Fund LP"
  :partnership-type "EXEMPTED_LIMITED_PARTNERSHIP"
  :jurisdiction "KY"
  :formation-date "2019-06-15"
  :as @fund-lp)

;; Create the General Partner entity
(entity.create-limited-company
  :name "Meridian Partners LP"
  :jurisdiction "US"
  :registration-number "DE-7891234"
  :as @general-partner)

;; Create UBO 1 - Marcus Chen
(entity.create-proper-person
  :first-name "Marcus"
  :last-name "Chen"
  :date-of-birth "1968-03-15"
  :nationality "US"
  :residence-address "450 Park Avenue, New York, NY 10022"
  :as @marcus-chen)

;; Create UBO 2 - Victoria Sterling
(entity.create-proper-person
  :first-name "Victoria"
  :last-name "Sterling"
  :date-of-birth "1972-08-22"
  :nationality "GB"
  :residence-address "15 Grosvenor Square, London W1K 6JP"
  :as @victoria-sterling)

;; =============================================================================
;; SECTION 2: ROLE ASSIGNMENTS
;; =============================================================================

(cbu.assign-role :cbu-id @fund :entity-id @fund-lp :role "PRINCIPAL")
(cbu.assign-role :cbu-id @fund :entity-id @investment-manager :role "INVESTMENT_MANAGER")
(cbu.assign-role :cbu-id @fund :entity-id @general-partner :role "GENERAL_PARTNER")
(cbu.assign-role :cbu-id @fund :entity-id @marcus-chen :role "BENEFICIAL_OWNER" :ownership-percentage 60)
(cbu.assign-role :cbu-id @fund :entity-id @victoria-sterling :role "BENEFICIAL_OWNER" :ownership-percentage 40)
(cbu.assign-role :cbu-id @fund :entity-id @marcus-chen :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @victoria-sterling :role "DIRECTOR")

;; =============================================================================
;; SECTION 3: OWNERSHIP CHAIN
;; =============================================================================

;; Marcus Chen owns 60% of General Partner
(ubo.add-ownership
  :owner-entity-id @marcus-chen
  :owned-entity-id @general-partner
  :percentage 60
  :ownership-type "DIRECT"
  :as @own-marcus-gp)

;; Victoria Sterling owns 40% of General Partner
(ubo.add-ownership
  :owner-entity-id @victoria-sterling
  :owned-entity-id @general-partner
  :percentage 40
  :ownership-type "DIRECT"
  :as @own-victoria-gp)

;; General Partner controls the Fund LP
(ubo.add-ownership
  :owner-entity-id @general-partner
  :owned-entity-id @fund-lp
  :percentage 100
  :ownership-type "CONTROL"
  :as @own-gp-fund)

;; Marcus Chen owns 50% of Investment Manager
(ubo.add-ownership
  :owner-entity-id @marcus-chen
  :owned-entity-id @investment-manager
  :percentage 50
  :ownership-type "DIRECT"
  :as @own-marcus-im)

;; Victoria Sterling owns 50% of Investment Manager
(ubo.add-ownership
  :owner-entity-id @victoria-sterling
  :owned-entity-id @investment-manager
  :percentage 50
  :ownership-type "DIRECT"
  :as @own-victoria-im)

;; =============================================================================
;; SECTION 4: KYC CASE CREATION
;; =============================================================================

(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "Hedge fund onboarding for Custody + Alternatives products. Complex Cayman LP structure with US and UK beneficial owners."
  :as @case)

;; Assign analyst and reviewer
(kyc-case.assign
  :case-id @case
  :analyst-id "analyst-sarah-jones"
  :reviewer-id "reviewer-james-wilson")

;; =============================================================================
;; SECTION 5: ENTITY WORKSTREAMS
;; =============================================================================

;; Fund LP workstream
(entity-workstream.create
  :case-id @case
  :entity-id @fund-lp
  :discovery-reason "PRINCIPAL"
  :as @ws-fund)

;; General Partner workstream
(entity-workstream.create
  :case-id @case
  :entity-id @general-partner
  :discovery-reason "GENERAL_PARTNER"
  :discovery-depth 1
  :as @ws-gp)

;; Investment Manager workstream
(entity-workstream.create
  :case-id @case
  :entity-id @investment-manager
  :discovery-reason "INVESTMENT_MANAGER"
  :as @ws-im)

;; UBO 1 workstream - Marcus Chen
(entity-workstream.create
  :case-id @case
  :entity-id @marcus-chen
  :discovery-source-id @ws-gp
  :discovery-reason "UBO_60PCT_INDIRECT"
  :discovery-depth 2
  :is-ubo true
  :ownership-percentage 60
  :as @ws-marcus)

;; UBO 2 workstream - Victoria Sterling
(entity-workstream.create
  :case-id @case
  :entity-id @victoria-sterling
  :discovery-source-id @ws-gp
  :discovery-reason "UBO_40PCT_INDIRECT"
  :discovery-depth 2
  :is-ubo true
  :ownership-percentage 40
  :as @ws-victoria)

;; Move case to DISCOVERY
(kyc-case.update-status :case-id @case :status "DISCOVERY")

;; =============================================================================
;; SECTION 6: CLIENT ALLEGATIONS (What the client claims)
;; =============================================================================

;; --- Marcus Chen Allegations ---
(allegation.record
  :cbu-id @fund
  :entity-id @marcus-chen
  :attribute-id "attr.identity.full_name"
  :value {"first": "Marcus", "last": "Chen"}
  :display-value "Marcus Chen"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-marcus-name)

(allegation.record
  :cbu-id @fund
  :entity-id @marcus-chen
  :attribute-id "attr.identity.date_of_birth"
  :value "1968-03-15"
  :display-value "March 15, 1968"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-marcus-dob)

(allegation.record
  :cbu-id @fund
  :entity-id @marcus-chen
  :attribute-id "attr.identity.nationality"
  :value "US"
  :display-value "United States"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-marcus-nationality)

(allegation.record
  :cbu-id @fund
  :entity-id @marcus-chen
  :attribute-id "attr.address.residential"
  :value {"street": "450 Park Avenue", "city": "New York", "state": "NY", "postal": "10022", "country": "US"}
  :display-value "450 Park Avenue, New York, NY 10022"
  :source "KYC_QUESTIONNAIRE"
  :case-id @case
  :as @allege-marcus-address)

(allegation.record
  :cbu-id @fund
  :entity-id @marcus-chen
  :attribute-id "attr.ownership.percentage"
  :value 60
  :display-value "60% indirect ownership"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-marcus-ownership)

(allegation.record
  :cbu-id @fund
  :entity-id @marcus-chen
  :attribute-id "attr.kyc.source_of_wealth"
  :value "Investment management fees and carried interest from hedge fund operations since 1995"
  :display-value "Investment management fees and carried interest"
  :source "KYC_QUESTIONNAIRE"
  :case-id @case
  :as @allege-marcus-sow)

;; --- Victoria Sterling Allegations ---
(allegation.record
  :cbu-id @fund
  :entity-id @victoria-sterling
  :attribute-id "attr.identity.full_name"
  :value {"first": "Victoria", "last": "Sterling"}
  :display-value "Victoria Sterling"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-victoria-name)

(allegation.record
  :cbu-id @fund
  :entity-id @victoria-sterling
  :attribute-id "attr.identity.date_of_birth"
  :value "1972-08-22"
  :display-value "August 22, 1972"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-victoria-dob)

(allegation.record
  :cbu-id @fund
  :entity-id @victoria-sterling
  :attribute-id "attr.identity.nationality"
  :value "GB"
  :display-value "United Kingdom"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-victoria-nationality)

(allegation.record
  :cbu-id @fund
  :entity-id @victoria-sterling
  :attribute-id "attr.address.residential"
  :value {"street": "15 Grosvenor Square", "city": "London", "postal": "W1K 6JP", "country": "GB"}
  :display-value "15 Grosvenor Square, London W1K 6JP"
  :source "KYC_QUESTIONNAIRE"
  :case-id @case
  :as @allege-victoria-address)

(allegation.record
  :cbu-id @fund
  :entity-id @victoria-sterling
  :attribute-id "attr.ownership.percentage"
  :value 40
  :display-value "40% indirect ownership"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-victoria-ownership)

(allegation.record
  :cbu-id @fund
  :entity-id @victoria-sterling
  :attribute-id "attr.kyc.source_of_wealth"
  :value "Investment banking career (Goldman Sachs 1995-2010) and fund management"
  :display-value "Investment banking and fund management"
  :source "KYC_QUESTIONNAIRE"
  :case-id @case
  :as @allege-victoria-sow)

;; --- Fund Allegations ---
(allegation.record
  :cbu-id @fund
  :entity-id @fund-lp
  :attribute-id "attr.fund.aum"
  :value 850000000
  :display-value "$850,000,000"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-fund-aum)

(allegation.record
  :cbu-id @fund
  :entity-id @fund-lp
  :attribute-id "attr.fund.strategy"
  :value "GLOBAL_MACRO"
  :display-value "Global Macro"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allege-fund-strategy)

;; =============================================================================
;; SECTION 7: DOCUMENT COLLECTION (Proofs)
;; =============================================================================

;; --- Document Requests ---
(doc-request.create :workstream-id @ws-marcus :doc-type "PASSPORT" :is-mandatory true :priority "HIGH" :as @req-marcus-passport)
(doc-request.create :workstream-id @ws-marcus :doc-type "PROOF_OF_ADDRESS" :is-mandatory true :priority "HIGH" :as @req-marcus-poa)
(doc-request.create :workstream-id @ws-marcus :doc-type "SOURCE_OF_WEALTH" :is-mandatory true :priority "NORMAL" :as @req-marcus-sow)
(doc-request.create :workstream-id @ws-victoria :doc-type "PASSPORT" :is-mandatory true :priority "HIGH" :as @req-victoria-passport)
(doc-request.create :workstream-id @ws-victoria :doc-type "PROOF_OF_ADDRESS" :is-mandatory true :priority "HIGH" :as @req-victoria-poa)
(doc-request.create :workstream-id @ws-victoria :doc-type "SOURCE_OF_WEALTH" :is-mandatory true :priority "NORMAL" :as @req-victoria-sow)
(doc-request.create :workstream-id @ws-fund :doc-type "CERTIFICATE_OF_REGISTRATION" :is-mandatory true :priority "HIGH" :as @req-fund-cert)
(doc-request.create :workstream-id @ws-fund :doc-type "PARTNERSHIP_AGREEMENT" :is-mandatory true :priority "HIGH" :as @req-fund-lpa)
(doc-request.create :workstream-id @ws-gp :doc-type "REGISTER_OF_SHAREHOLDERS" :is-mandatory true :priority "HIGH" :as @req-gp-shareholders)
(doc-request.create :workstream-id @ws-im :doc-type "FCA_AUTHORIZATION" :is-mandatory true :priority "HIGH" :as @req-im-fca)

;; --- Catalog Documents ---

;; Marcus Chen documents
(document.catalog
  :cbu-id @fund
  :entity-id @marcus-chen
  :document-type "PASSPORT"
  :title "US Passport - Marcus Chen"
  :as @doc-marcus-passport)

(document.catalog
  :cbu-id @fund
  :entity-id @marcus-chen
  :document-type "UTILITY_BILL"
  :title "Con Edison Statement - Marcus Chen"
  :as @doc-marcus-poa)

(document.catalog
  :cbu-id @fund
  :entity-id @marcus-chen
  :document-type "SOURCE_OF_WEALTH"
  :title "Source of Wealth Declaration - Marcus Chen"
  :as @doc-marcus-sow)

;; Victoria Sterling documents
(document.catalog
  :cbu-id @fund
  :entity-id @victoria-sterling
  :document-type "PASSPORT"
  :title "UK Passport - Victoria Sterling"
  :as @doc-victoria-passport)

(document.catalog
  :cbu-id @fund
  :entity-id @victoria-sterling
  :document-type "BANK_STATEMENT"
  :title "Barclays Statement - Victoria Sterling"
  :as @doc-victoria-poa)

(document.catalog
  :cbu-id @fund
  :entity-id @victoria-sterling
  :document-type "SOURCE_OF_WEALTH"
  :title "Source of Wealth Declaration - Victoria Sterling"
  :as @doc-victoria-sow)

;; Fund documents
(document.catalog
  :cbu-id @fund
  :entity-id @fund-lp
  :document-type "CERTIFICATE_OF_REGISTRATION"
  :title "CIMA Certificate of Registration"
  :as @doc-fund-cert)

(document.catalog
  :cbu-id @fund
  :entity-id @fund-lp
  :document-type "PARTNERSHIP_AGREEMENT"
  :title "Exempted Limited Partnership Agreement"
  :as @doc-fund-lpa)

;; General Partner documents
(document.catalog
  :cbu-id @fund
  :entity-id @general-partner
  :document-type "REGISTER_OF_SHAREHOLDERS"
  :title "Meridian Partners LP - Share Register"
  :as @doc-gp-shareholders)

;; Investment Manager documents
(document.catalog
  :cbu-id @fund
  :entity-id @investment-manager
  :document-type "FCA_AUTHORIZATION"
  :title "FCA Authorization Letter"
  :as @doc-im-fca)

;; --- Link Documents to Requests ---
(doc-request.receive :request-id @req-marcus-passport :document-id @doc-marcus-passport)
(doc-request.receive :request-id @req-marcus-poa :document-id @doc-marcus-poa)
(doc-request.receive :request-id @req-marcus-sow :document-id @doc-marcus-sow)
(doc-request.receive :request-id @req-victoria-passport :document-id @doc-victoria-passport)
(doc-request.receive :request-id @req-victoria-poa :document-id @doc-victoria-poa)
(doc-request.receive :request-id @req-victoria-sow :document-id @doc-victoria-sow)
(doc-request.receive :request-id @req-fund-cert :document-id @doc-fund-cert)
(doc-request.receive :request-id @req-fund-lpa :document-id @doc-fund-lpa)
(doc-request.receive :request-id @req-gp-shareholders :document-id @doc-gp-shareholders)
(doc-request.receive :request-id @req-im-fca :document-id @doc-im-fca)

;; =============================================================================
;; SECTION 8: OBSERVATIONS (Extracted from Documents)
;; =============================================================================

;; --- Marcus Chen Observations ---
(observation.record-from-document
  :entity-id @marcus-chen
  :document-id @doc-marcus-passport
  :attribute "attr.identity.full_name"
  :value "Marcus J Chen"
  :extraction-method "MRZ_SCAN"
  :confidence 0.99
  :as @obs-marcus-name)

(observation.record-from-document
  :entity-id @marcus-chen
  :document-id @doc-marcus-passport
  :attribute "attr.identity.date_of_birth"
  :value "1968-03-15"
  :extraction-method "MRZ_SCAN"
  :confidence 0.99
  :as @obs-marcus-dob)

(observation.record-from-document
  :entity-id @marcus-chen
  :document-id @doc-marcus-passport
  :attribute "attr.identity.nationality"
  :value "US"
  :extraction-method "MRZ_SCAN"
  :confidence 0.99
  :as @obs-marcus-nationality)

(observation.record-from-document
  :entity-id @marcus-chen
  :document-id @doc-marcus-poa
  :attribute "attr.address.residential"
  :value {"street": "450 Park Avenue", "city": "New York", "state": "NY", "postal": "10022", "country": "US"}
  :extraction-method "AI_OCR"
  :confidence 0.95
  :as @obs-marcus-address)

(observation.record-from-document
  :entity-id @marcus-chen
  :document-id @doc-gp-shareholders
  :attribute "attr.ownership.percentage"
  :value 60
  :extraction-method "MANUAL"
  :confidence 1.0
  :as @obs-marcus-ownership)

;; --- Victoria Sterling Observations ---
(observation.record-from-document
  :entity-id @victoria-sterling
  :document-id @doc-victoria-passport
  :attribute "attr.identity.full_name"
  :value "Victoria Anne Sterling"
  :extraction-method "MRZ_SCAN"
  :confidence 0.99
  :as @obs-victoria-name)

(observation.record-from-document
  :entity-id @victoria-sterling
  :document-id @doc-victoria-passport
  :attribute "attr.identity.date_of_birth"
  :value "1972-08-22"
  :extraction-method "MRZ_SCAN"
  :confidence 0.99
  :as @obs-victoria-dob)

(observation.record-from-document
  :entity-id @victoria-sterling
  :document-id @doc-victoria-passport
  :attribute "attr.identity.nationality"
  :value "GB"
  :extraction-method "MRZ_SCAN"
  :confidence 0.99
  :as @obs-victoria-nationality)

(observation.record-from-document
  :entity-id @victoria-sterling
  :document-id @doc-victoria-poa
  :attribute "attr.address.residential"
  :value {"street": "15 Grosvenor Square", "city": "London", "postal": "W1K 6JP", "country": "GB"}
  :extraction-method "AI_OCR"
  :confidence 0.93
  :as @obs-victoria-address)

(observation.record-from-document
  :entity-id @victoria-sterling
  :document-id @doc-gp-shareholders
  :attribute "attr.ownership.percentage"
  :value 40
  :extraction-method "MANUAL"
  :confidence 1.0
  :as @obs-victoria-ownership)

;; =============================================================================
;; SECTION 9: ALLEGATION VERIFICATION (Compare Allegations to Observations)
;; =============================================================================

;; Marcus Chen verifications
(allegation.verify
  :allegation-id @allege-marcus-name
  :observation-id @obs-marcus-name
  :result "ACCEPTABLE_VARIATION"
  :notes "Middle initial 'J' present in passport but not in onboarding form - acceptable")

(allegation.verify
  :allegation-id @allege-marcus-dob
  :observation-id @obs-marcus-dob
  :result "EXACT_MATCH"
  :notes "DOB verified from passport MRZ")

(allegation.verify
  :allegation-id @allege-marcus-nationality
  :observation-id @obs-marcus-nationality
  :result "EXACT_MATCH"
  :notes "US nationality confirmed")

(allegation.verify
  :allegation-id @allege-marcus-address
  :observation-id @obs-marcus-address
  :result "EXACT_MATCH"
  :notes "Address verified from utility bill")

(allegation.verify
  :allegation-id @allege-marcus-ownership
  :observation-id @obs-marcus-ownership
  :result "EXACT_MATCH"
  :notes "60% ownership confirmed from share register")

;; Victoria Sterling verifications
(allegation.verify
  :allegation-id @allege-victoria-name
  :observation-id @obs-victoria-name
  :result "ACCEPTABLE_VARIATION"
  :notes "Middle name 'Anne' in passport but not in form - acceptable")

(allegation.verify
  :allegation-id @allege-victoria-dob
  :observation-id @obs-victoria-dob
  :result "EXACT_MATCH"
  :notes "DOB verified from passport MRZ")

(allegation.verify
  :allegation-id @allege-victoria-nationality
  :observation-id @obs-victoria-nationality
  :result "EXACT_MATCH"
  :notes "UK nationality confirmed")

(allegation.verify
  :allegation-id @allege-victoria-address
  :observation-id @obs-victoria-address
  :result "EXACT_MATCH"
  :notes "Address verified from bank statement")

(allegation.verify
  :allegation-id @allege-victoria-ownership
  :observation-id @obs-victoria-ownership
  :result "EXACT_MATCH"
  :notes "40% ownership confirmed from share register")

;; =============================================================================
;; SECTION 10: DOCUMENT VERIFICATION
;; =============================================================================

(doc-request.verify :request-id @req-marcus-passport :verification-notes "Valid US passport, expires 2029-06-20, no alterations detected")
(doc-request.verify :request-id @req-marcus-poa :verification-notes "Utility bill dated within 90 days, address matches allegation")
(doc-request.verify :request-id @req-marcus-sow :verification-notes "SOW declaration consistent with public records and career history")
(doc-request.verify :request-id @req-victoria-passport :verification-notes "Valid UK passport, expires 2031-02-15, biometric chip verified")
(doc-request.verify :request-id @req-victoria-poa :verification-notes "Bank statement within 30 days, address matches")
(doc-request.verify :request-id @req-victoria-sow :verification-notes "Goldman Sachs employment verified, fund management income consistent")
(doc-request.verify :request-id @req-fund-cert :verification-notes "CIMA registration active, last annual return filed")
(doc-request.verify :request-id @req-fund-lpa :verification-notes "LPA executed, GP authority confirmed")
(doc-request.verify :request-id @req-gp-shareholders :verification-notes "Share register certified by corporate secretary")
(doc-request.verify :request-id @req-im-fca :verification-notes "FCA authorization active, no restrictions")

;; =============================================================================
;; SECTION 11: THRESHOLD DERIVATION & EVALUATION
;; =============================================================================

;; Derive risk-based requirements for each entity
(threshold.derive :cbu-id @fund :entity-id @marcus-chen :as @threshold-marcus)
(threshold.derive :cbu-id @fund :entity-id @victoria-sterling :as @threshold-victoria)
(threshold.derive :cbu-id @fund :entity-id @fund-lp :as @threshold-fund)

;; Evaluate thresholds
(threshold.evaluate :cbu-id @fund :entity-id @marcus-chen :risk-band "MEDIUM")
(threshold.evaluate :cbu-id @fund :entity-id @victoria-sterling :risk-band "MEDIUM")
(threshold.evaluate :cbu-id @fund :entity-id @fund-lp :risk-band "MEDIUM")

;; Check entities meet requirements
(threshold.check-entity :cbu-id @fund :entity-id @marcus-chen)
(threshold.check-entity :cbu-id @fund :entity-id @victoria-sterling)
(threshold.check-entity :cbu-id @fund :entity-id @fund-lp)

;; =============================================================================
;; SECTION 12: SCREENINGS
;; =============================================================================

;; --- Marcus Chen Screenings ---
(case-screening.run :workstream-id @ws-marcus :screening-type "SANCTIONS" :as @screen-marcus-sanctions)
(case-screening.run :workstream-id @ws-marcus :screening-type "PEP" :as @screen-marcus-pep)
(case-screening.run :workstream-id @ws-marcus :screening-type "ADVERSE_MEDIA" :as @screen-marcus-media)

;; --- Victoria Sterling Screenings ---
(case-screening.run :workstream-id @ws-victoria :screening-type "SANCTIONS" :as @screen-victoria-sanctions)
(case-screening.run :workstream-id @ws-victoria :screening-type "PEP" :as @screen-victoria-pep)
(case-screening.run :workstream-id @ws-victoria :screening-type "ADVERSE_MEDIA" :as @screen-victoria-media)

;; --- Entity Screenings ---
(case-screening.run :workstream-id @ws-fund :screening-type "SANCTIONS" :as @screen-fund-sanctions)
(case-screening.run :workstream-id @ws-gp :screening-type "SANCTIONS" :as @screen-gp-sanctions)
(case-screening.run :workstream-id @ws-im :screening-type "SANCTIONS" :as @screen-im-sanctions)

;; --- Screening Results ---
(case-screening.complete :screening-id @screen-marcus-sanctions :status "CLEAR" :result-summary "No OFAC/UN/EU sanctions matches")
(case-screening.complete :screening-id @screen-marcus-pep :status "CLEAR" :result-summary "Not a PEP - no government positions held")
(case-screening.complete :screening-id @screen-marcus-media :status "CLEAR" :result-summary "No adverse media findings")

(case-screening.complete :screening-id @screen-victoria-sanctions :status "CLEAR" :result-summary "No OFAC/UN/EU sanctions matches")
(case-screening.complete :screening-id @screen-victoria-pep :status "CLEAR" :result-summary "Not a PEP - private sector career only")
(case-screening.complete :screening-id @screen-victoria-media :status "HIT_PENDING_REVIEW" :result-summary "Minor mention: 2019 FT article on women in hedge funds")

(case-screening.complete :screening-id @screen-fund-sanctions :status "CLEAR" :result-summary "Fund not on any sanctions lists")
(case-screening.complete :screening-id @screen-gp-sanctions :status "CLEAR" :result-summary "GP entity clear of sanctions")
(case-screening.complete :screening-id @screen-im-sanctions :status "CLEAR" :result-summary "FCA-regulated entity - clear")

;; Review and clear the media hit
(case-screening.review-hit
  :screening-id @screen-victoria-media
  :status "HIT_DISMISSED"
  :notes "Article is positive coverage about women in finance leadership. Not adverse.")

;; =============================================================================
;; SECTION 13: RED FLAGS & RISK INDICATORS
;; =============================================================================

;; Raise red flag for complex structure (offshore)
(red-flag.raise
  :case-id @case
  :workstream-id @ws-fund
  :flag-type "COMPLEX_STRUCTURE"
  :severity "SOFT"
  :description "Cayman Islands LP structure with US and UK beneficial owners requires enhanced documentation"
  :source "ANALYST"
  :as @flag-structure)

;; Raise red flag for high AUM
(red-flag.raise
  :case-id @case
  :flag-type "HIGH_VALUE_CLIENT"
  :severity "SOFT"
  :description "Fund AUM of $850M exceeds $500M enhanced due diligence threshold"
  :source "SYSTEM"
  :as @flag-aum)

;; Mitigate red flags with evidence
(red-flag.mitigate
  :red-flag-id @flag-structure
  :notes "Full ownership chain documented with certified share registers. Both UBOs identified and verified with passport and proof of address. Fund registered with CIMA.")

(red-flag.mitigate
  :red-flag-id @flag-aum
  :notes "Enhanced due diligence completed. Both UBOs have clean screening results. Source of wealth documented and verified for both individuals.")

;; =============================================================================
;; SECTION 14: UBO REGISTRATION & VERIFICATION
;; =============================================================================

;; Register Marcus Chen as UBO
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @fund-lp
  :ubo-person-id @marcus-chen
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 60
  :control-type "VOTING_CONTROL"
  :workflow-type "ONBOARDING"
  :evidence-doc-id @doc-gp-shareholders
  :as @ubo-marcus)

;; Register Victoria Sterling as UBO
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @fund-lp
  :ubo-person-id @victoria-sterling
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 40
  :control-type "VOTING_CONTROL"
  :workflow-type "ONBOARDING"
  :evidence-doc-id @doc-gp-shareholders
  :as @ubo-victoria)

;; Verify UBOs
(ubo.verify-ubo
  :ubo-id @ubo-marcus
  :verification-status "VERIFIED"
  :risk-rating "LOW")

(ubo.verify-ubo
  :ubo-id @ubo-victoria
  :verification-status "VERIFIED"
  :risk-rating "LOW")

;; Trace complete ownership chains
(ubo.trace-chains :cbu-id @fund)

;; Check UBO completeness
(ubo.check-completeness :cbu-id @fund)

;; Create UBO snapshot for audit
(ubo.snapshot-cbu :cbu-id @fund :as @ubo-snapshot)

;; =============================================================================
;; SECTION 15: WORKSTREAM COMPLETION
;; =============================================================================

(entity-workstream.update-status :workstream-id @ws-fund :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-gp :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-im :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-marcus :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-victoria :status "COMPLETE")

;; =============================================================================
;; SECTION 16: CASE ASSESSMENT
;; =============================================================================

;; Move to ASSESSMENT
(kyc-case.update-status :case-id @case :status "ASSESSMENT")

;; Log assessment event
(case-event.log
  :case-id @case
  :event-type "ASSESSMENT_STARTED"
  :event-data {"assessor": "analyst-sarah-jones", "entities_reviewed": 5, "documents_verified": 10}
  :comment "All workstreams complete. Beginning final assessment.")

;; =============================================================================
;; SECTION 17: CASE REVIEW & APPROVAL
;; =============================================================================

;; Move to REVIEW
(kyc-case.update-status :case-id @case :status "REVIEW")

;; Set risk rating
(kyc-case.set-risk-rating :case-id @case :risk-rating "MEDIUM")

;; Log review event
(case-event.log
  :case-id @case
  :event-type "REVIEW_COMPLETED"
  :event-data {"reviewer": "reviewer-james-wilson", "recommendation": "APPROVE", "risk_rating": "MEDIUM"}
  :comment "Review complete. Recommend approval with standard monitoring.")

;; =============================================================================
;; SECTION 18: FINAL DECISION - KYC/AML APPROVAL
;; =============================================================================

;; Close case as APPROVED
(kyc-case.close
  :case-id @case
  :status "APPROVED"
  :notes "KYC/AML onboarding complete for Meridian Alpha Fund Ltd. Both UBOs (Marcus Chen 60%, Victoria Sterling 40%) identified, verified, and screened. All allegations verified against documentary evidence. Risk rating: MEDIUM. Approved for Custody + Alternatives products.")

;; Log final decision
(case-event.log
  :case-id @case
  :event-type "CASE_APPROVED"
  :event-data {
    "decision": "APPROVED"
    "risk_rating": "MEDIUM"
    "products_approved": ["CUSTODY" "ALTERNATIVES"]
    "ubos_identified": 2
    "documents_verified": 10
    "screenings_completed": 9
    "red_flags_mitigated": 2
    "next_review_date": "2026-05-21"
  }
  :comment "Final approval granted by Compliance Committee.")

;; =============================================================================
;; SECTION 19: SERVICE PROVISIONING (Post-Approval)
;; =============================================================================

;; Provision Custody Account
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.bank.com/accounts/MERIDIAN-001"
  :as @custody-account)

(service-resource.set-attr :instance-id @custody-account :attr "account_number" :value "CUST-MER-2025-001")
(service-resource.set-attr :instance-id @custody-account :attr "custodian_bic" :value "CITIUS33")
(service-resource.set-attr :instance-id @custody-account :attr "base_currency" :value "USD")

(service-resource.validate-attrs :instance-id @custody-account)
(service-resource.activate :instance-id @custody-account)

;; Provision Alternatives Platform Access
(service-resource.provision
  :cbu-id @fund
  :resource-type "ALTS_GENEVA"
  :instance-url "https://geneva.statestreet.com/MERIDIAN"
  :as @alts-platform)

(service-resource.set-attr :instance-id @alts-platform :attr "client_code" :value "MERIDIAN")
(service-resource.set-attr :instance-id @alts-platform :attr "fund_accounting_enabled" :value true)
(service-resource.set-attr :instance-id @alts-platform :attr "investor_services_enabled" :value true)

(service-resource.validate-attrs :instance-id @alts-platform)
(service-resource.activate :instance-id @alts-platform)

;; =============================================================================
;; FINAL OUTCOME SUMMARY
;; =============================================================================

;; List final UBOs
(ubo.list-ubos :cbu-id @fund)

;; List case events for audit trail
(case-event.list-by-case :case-id @case)
```

---

## 3. Verification Summary Tables

### 3.1 Allegation → Observation Verification Matrix

| Entity | Attribute | Alleged Value | Observed Value | Source Doc | Result |
|--------|-----------|---------------|----------------|------------|--------|
| Marcus Chen | Full Name | Marcus Chen | Marcus J Chen | Passport | ✅ ACCEPTABLE_VARIATION |
| Marcus Chen | DOB | 1968-03-15 | 1968-03-15 | Passport | ✅ EXACT_MATCH |
| Marcus Chen | Nationality | US | US | Passport | ✅ EXACT_MATCH |
| Marcus Chen | Address | 450 Park Ave, NY | 450 Park Ave, NY | Utility Bill | ✅ EXACT_MATCH |
| Marcus Chen | Ownership | 60% | 60% | Share Register | ✅ EXACT_MATCH |
| Victoria Sterling | Full Name | Victoria Sterling | Victoria Anne Sterling | Passport | ✅ ACCEPTABLE_VARIATION |
| Victoria Sterling | DOB | 1972-08-22 | 1972-08-22 | Passport | ✅ EXACT_MATCH |
| Victoria Sterling | Nationality | GB | GB | Passport | ✅ EXACT_MATCH |
| Victoria Sterling | Address | 15 Grosvenor Sq | 15 Grosvenor Sq | Bank Statement | ✅ EXACT_MATCH |
| Victoria Sterling | Ownership | 40% | 40% | Share Register | ✅ EXACT_MATCH |

### 3.2 Document Verification Status

| Document Type | Entity | Status | Verification Notes |
|--------------|--------|--------|-------------------|
| Passport | Marcus Chen | ✅ VERIFIED | Valid US passport, expires 2029-06-20 |
| Proof of Address | Marcus Chen | ✅ VERIFIED | Utility bill within 90 days |
| Source of Wealth | Marcus Chen | ✅ VERIFIED | Career history verified |
| Passport | Victoria Sterling | ✅ VERIFIED | Valid UK passport, biometric verified |
| Proof of Address | Victoria Sterling | ✅ VERIFIED | Bank statement within 30 days |
| Source of Wealth | Victoria Sterling | ✅ VERIFIED | Goldman Sachs employment confirmed |
| CIMA Certificate | Fund LP | ✅ VERIFIED | Registration active |
| Partnership Agreement | Fund LP | ✅ VERIFIED | GP authority confirmed |
| Share Register | General Partner | ✅ VERIFIED | Certified by corporate secretary |
| FCA Authorization | Investment Manager | ✅ VERIFIED | Active, no restrictions |

### 3.3 Screening Results

| Entity | Screening Type | Status | Notes |
|--------|---------------|--------|-------|
| Marcus Chen | SANCTIONS | ✅ CLEAR | No OFAC/UN/EU matches |
| Marcus Chen | PEP | ✅ CLEAR | Not a PEP |
| Marcus Chen | ADVERSE_MEDIA | ✅ CLEAR | No adverse findings |
| Victoria Sterling | SANCTIONS | ✅ CLEAR | No matches |
| Victoria Sterling | PEP | ✅ CLEAR | Private sector only |
| Victoria Sterling | ADVERSE_MEDIA | ✅ DISMISSED | Positive press coverage |
| Meridian Alpha Fund LP | SANCTIONS | ✅ CLEAR | Not sanctioned |
| Meridian Partners LP | SANCTIONS | ✅ CLEAR | Clear |
| Meridian Investment Mgmt | SANCTIONS | ✅ CLEAR | FCA-regulated |

### 3.4 Threshold Requirements (Risk Band: MEDIUM)

| Entity | Required Docs | Status | Notes |
|--------|--------------|--------|-------|
| Marcus Chen (UBO) | Identity, Address, SOW | ✅ MET | All documents verified |
| Victoria Sterling (UBO) | Identity, Address, SOW | ✅ MET | All documents verified |
| Fund LP | Registration, LPA | ✅ MET | CIMA registration current |

### 3.5 Red Flags Summary

| Flag Type | Severity | Status | Resolution |
|-----------|----------|--------|------------|
| COMPLEX_STRUCTURE | SOFT | ✅ MITIGATED | Full ownership chain documented |
| HIGH_VALUE_CLIENT | SOFT | ✅ MITIGATED | Enhanced DD completed |

---

## 4. UBO Determination

### 4.1 Ownership Chain Analysis

```
Meridian Alpha Fund LP (KY)
    │
    └── 100% CONTROL ──► Meridian Partners LP (US)
                              │
                              ├── 60% DIRECT ──► Marcus Chen (US)     ═══► UBO #1
                              │
                              └── 40% DIRECT ──► Victoria Sterling (UK) ═══► UBO #2
```

### 4.2 UBO Registry

| UBO | Nationality | Relationship | Ownership % | Control Type | Risk Rating | Status |
|-----|-------------|--------------|-------------|--------------|-------------|--------|
| Marcus Chen | US | INDIRECT_OWNER | 60% | VOTING_CONTROL | LOW | ✅ VERIFIED |
| Victoria Sterling | GB | INDIRECT_OWNER | 40% | VOTING_CONTROL | LOW | ✅ VERIFIED |

---

## 5. Final Decision

| Field | Value |
|-------|-------|
| **Case ID** | CASE-MERIDIAN-2025-001 |
| **Case Type** | NEW_CLIENT |
| **Client** | Meridian Alpha Fund Ltd |
| **Products** | Custody + Alternatives |
| **Status** | **APPROVED** |
| **Risk Rating** | MEDIUM |
| **Analyst** | Sarah Jones |
| **Reviewer** | James Wilson |
| **Approval Date** | 2025-05-21 |
| **Next Review Date** | 2026-05-21 |

### 5.1 Approval Conditions

1. Annual KYC refresh required
2. Quarterly AML transaction monitoring
3. Immediate notification if UBO structure changes
4. Enhanced monitoring for transactions > $10M

---

## 6. Service Activation

| Resource | Type | Account/ID | Status |
|----------|------|------------|--------|
| Custody Account | CUSTODY_ACCT | CUST-MER-2025-001 | ✅ ACTIVE |
| Alternatives Platform | ALTS_GENEVA | MERIDIAN | ✅ ACTIVE |

---

## Appendix: DSL Symbol Reference

| Symbol | Entity/Resource | Description |
|--------|----------------|-------------|
| `@fund` | CBU | Meridian Alpha Fund Ltd |
| `@fund-lp` | Entity | Meridian Alpha Fund LP (Cayman) |
| `@general-partner` | Entity | Meridian Partners LP (US) |
| `@investment-manager` | Entity | Meridian Investment Management Ltd (UK) |
| `@marcus-chen` | Entity | Marcus Chen - UBO 60% |
| `@victoria-sterling` | Entity | Victoria Sterling - UBO 40% |
| `@case` | KYC Case | Main onboarding case |
| `@ubo-marcus` | UBO Record | Marcus Chen UBO determination |
| `@ubo-victoria` | UBO Record | Victoria Sterling UBO determination |
| `@custody-account` | Resource Instance | Custody account provisioned |
| `@alts-platform` | Resource Instance | Geneva alternatives platform |
