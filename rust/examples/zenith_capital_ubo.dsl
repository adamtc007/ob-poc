;; ==========================================================================
;; ZENITH CAPITAL PARTNERS LP - Complex UBO Discovery Example
;; ==========================================================================
;; This is a simplified excerpt from the full Zenith workflow
;; demonstrating the key DSL patterns

(define-kyc-investigation "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :investigation-depth 5
  :ubo-threshold 25.0

  ;; Declare target entity
  (declare-entity
    :node-id "company-zenith-spv-001"
    :label Company
    :properties {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
      :entity-type "Limited Partnership"
      :incorporation-date "2023-01-15"
      :status "active"
    })

  ;; Declare Alpha Holdings (Singapore)
  (declare-entity
    :node-id "company-alpha-holdings-sg"
    :label Company
    :properties {
      :legal-name "Alpha Holdings Pte. Ltd."
      :registration-number "202200001A"
      :jurisdiction "SG"
      :entity-type "Private Limited Company"
      :status "active"
    })

  ;; Ownership edge: Alpha → Zenith
  (create-edge
    :from "company-alpha-holdings-sg"
    :to "company-zenith-spv-001"
    :type HAS_OWNERSHIP
    :properties {
      :percent 45.0
      :share-class "Class A Ordinary"
      :acquisition-date "2023-01-15"
    }
    :evidenced-by ["doc-cayman-registry-001" "doc-acra-sg-001"])

  ;; Wellington Trust (Blind Trust)
  (declare-entity
    :node-id "trust-wellington-jersey"
    :label Trust
    :properties {
      :trust-name "Wellington Family Trust"
      :registration-number "JT-567890"
      :jurisdiction "JE"
      :trust-type "Discretionary Trust"
      :established-date "2019-08-12"
      :beneficiaries-disclosed false
      :opacity-level "maximum"
    })

  ;; Ownership edge: Trust → Alpha
  (create-edge
    :from "trust-wellington-jersey"
    :to "company-alpha-holdings-sg"
    :type HAS_OWNERSHIP
    :properties {
      :percent 100.0
      :control-type "Full Ownership"
    }
    :evidenced-by ["doc-jersey-trust-001" "doc-acra-sg-001"])

  ;; Chen Wei with fragmented data
  (declare-entity
    :node-id "person-chen-wei"
    :label Person
    :properties {
      :full-name [
        { :value "Chen Wei" :source "doc-delaware-llc-001" }
        { :value "Wei Chen" :source "doc-dd-report-001" }
      ]
      :date-of-birth [
        { :value "1975-06-15" :source "doc-delaware-llc-001" :confidence 0.90 }
        { :value "1975-06-14" :source "doc-bo-cert-001" :confidence 0.60 }
      ]
      :nationality ["SG" "US"]
    })

  ;; Calculate UBO prongs
  (calculate-ubo-prongs
    :target "company-zenith-spv-001"
    :algorithm "recursive-ownership-chain"
    :max-depth 5
    :threshold 25.0
    :traversal-rules {
      :follow-edges [HAS_OWNERSHIP HAS_CONTROL]
      :terminal-nodes [Person]
      :ignore-nominees true
    }
    :output {
      :prong-1 {
        :path ["company-zenith-spv-001" "company-alpha-holdings-sg" "trust-wellington-jersey"]
        :ownership-percent 45.0
        :status "BLOCKED-BLIND-TRUST"
      }
    })

  ;; Resolve conflicts using waterfall
  (resolve-conflicts
    :node "person-chen-wei"
    :property "date-of-birth"
    :strategy (waterfall
                (government-registry "ACRA" :confidence 0.90)
                (self-declared "beneficial-ownership-cert" :confidence 0.60))
    :resolution {
      :winning-value "1975-06-15"
      :winning-source "doc-delaware-llc-001"
      :confidence 0.90
    })

  ;; Generate final UBO report
  (generate-ubo-report
    :target "company-zenith-spv-001"
    :status "INCOMPLETE-CANNOT-CERTIFY"
    :identified-ubos [
      { :person "person-park-min-jung" :prong-type "control" }
    ]
    :unresolved-prongs [
      {
        :prong-id "prong-1"
        :blocker "Blind trust - beneficiaries not disclosed"
        :ownership-at-risk 45.0
      }
    ]))
