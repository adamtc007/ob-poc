# DSL Examples: Enterprise Onboarding in Action

This document showcases real-world DSL examples that demonstrate the sophisticated capabilities of our DSL-as-State architecture across different enterprise onboarding scenarios.

---

## Example 1: UCITS Fund Onboarding

### Complete DSL Evolution - Accumulated State Over Time

```lisp
;; ════════════════════════════════════════════════════════════════════════════
;; VERSION 1: Initial Case Creation
;; ════════════════════════════════════════════════════════════════════════════

(case.create
  :cbu-id "CBU-2024-001"
  :nature-purpose "UCITS equity fund domiciled in Luxembourg"
  :fund-name "European Growth Opportunities Fund"
  :management-company "Premium Asset Management S.A."
  :depositary-preference "tier-1-global")

;; ════════════════════════════════════════════════════════════════════════════
;; VERSION 2: Product Selection (ACCUMULATED)
;; ════════════════════════════════════════════════════════════════════════════

(case.create
  :cbu-id "CBU-2024-001"
  :nature-purpose "UCITS equity fund domiciled in Luxembourg"
  :fund-name "European Growth Opportunities Fund"
  :management-company "Premium Asset Management S.A."
  :depositary-preference "tier-1-global")

(case.update
  :id "CBU-2024-001"
  :add-products ["CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENCY", "RISK_MANAGEMENT"])

;; ════════════════════════════════════════════════════════════════════════════
;; VERSION 3: KYC Discovery (ACCUMULATED)
;; ════════════════════════════════════════════════════════════════════════════

(case.create
  :cbu-id "CBU-2024-001"
  :nature-purpose "UCITS equity fund domiciled in Luxembourg"
  :fund-name "European Growth Opportunities Fund"
  :management-company "Premium Asset Management S.A."
  :depositary-preference "tier-1-global")

(case.update
  :id "CBU-2024-001"
  :add-products ["CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENCY", "RISK_MANAGEMENT"])

(kyc.verify
  :customer-id "CBU-2024-001"
  :method "enhanced_due_diligence"
  :jurisdictions [
    {:jurisdiction "LU", :primary true},
    {:jurisdiction "DE", :marketing true},
    {:jurisdiction "FR", :marketing true}
  ]
  :required-documents [
    "CertificateOfIncorporation",
    "ArticlesOfAssociation",
    "ProspectusAndKIID",
    "RiskManagementPolicy",
    "ConflictOfInterestPolicy"
  ]
  :regulatory-approvals [
    {:approval "CSSF-UCITS-License", :status "pending"},
    {:approval "BaFin-MarketingNotification", :status "required"},
    {:approval "AMF-MarketingNotification", :status "required"}
  ])

;; ════════════════════════════════════════════════════════════════════════════
;; VERSION 4: Service Planning (ACCUMULATED)
;; ════════════════════════════════════════════════════════════════════════════

(case.create
  :cbu-id "CBU-2024-001"
  :nature-purpose "UCITS equity fund domiciled in Luxembourg"
  :fund-name "European Growth Opportunities Fund"
  :management-company "Premium Asset Management S.A."
  :depositary-preference "tier-1-global")

(case.update
  :id "CBU-2024-001"
  :add-products ["CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENCY", "RISK_MANAGEMENT"])

(kyc.verify
  :customer-id "CBU-2024-001"
  :method "enhanced_due_diligence"
  :jurisdictions [
    {:jurisdiction "LU", :primary true},
    {:jurisdiction "DE", :marketing true},
    {:jurisdiction "FR", :marketing true}
  ]
  :required-documents [
    "CertificateOfIncorporation",
    "ArticlesOfAssociation",
    "ProspectusAndKIID",
    "RiskManagementPolicy",
    "ConflictOfInterestPolicy"
  ]
  :regulatory-approvals [
    {:approval "CSSF-UCITS-License", :status "pending"},
    {:approval "BaFin-MarketingNotification", :status "required"},
    {:approval "AMF-MarketingNotification", :status "required"}
  ])

(services.plan
  :services [
    {
      :name "Settlement"
      :sla "T+2"
      :currencies ["EUR", "USD", "GBP"]
      :markets ["XETRA", "EURONEXT", "LSE"]
    },
    {
      :name "CorporateActions"
      :automation-level "full"
      :notification-channels ["swift", "email", "portal"]
    },
    {
      :name "Reporting"
      :frequency "daily"
      :formats ["XML", "CSV", "PDF"]
      :recipients ["fund-manager", "depositary", "auditor"]
    },
    {
      :name "RiskMonitoring"
      :real-time true
      :breach-notifications "immediate"
      :regulatory-reports ["ucits-kiid", "ucits-risk"]
    }
  ])

;; ════════════════════════════════════════════════════════════════════════════
;; VERSION 5: Resource Provisioning (FINAL ACCUMULATED STATE - v3.0)
;; ════════════════════════════════════════════════════════════════════════════

(case.create
  :cbu-id "CBU-2024-001"
  :nature-purpose "UCITS equity fund domiciled in Luxembourg"
  :fund-name "European Growth Opportunities Fund"
  :management-company "Premium Asset Management S.A."
  :depositary-preference "tier-1-global")

(case.update
  :id "CBU-2024-001"
  :add-products ["CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENCY", "RISK_MANAGEMENT"])

(kyc.verify
  :customer-id "CBU-2024-001"
  :method "enhanced_due_diligence"
  :jurisdictions [
    {:jurisdiction "LU", :primary true},
    {:jurisdiction "DE", :marketing true},
    {:jurisdiction "FR", :marketing true}
  ]
  :required-documents [
    "CertificateOfIncorporation",
    "ArticlesOfAssociation",
    "ProspectusAndKIID",
    "RiskManagementPolicy",
    "ConflictOfInterestPolicy"
  ]
  :regulatory-approvals [
    {:approval "CSSF-UCITS-License", :status "pending"},
    {:approval "BaFin-MarketingNotification", :status "required"},
    {:approval "AMF-MarketingNotification", :status "required"}
  ])

(services.plan
  :services [
    {
      :name "Settlement"
      :sla "T+2"
      :currencies ["EUR", "USD", "GBP"]
      :markets ["XETRA", "EURONEXT", "LSE"]
    },
    {
      :name "CorporateActions"
      :automation-level "full"
      :notification-channels ["swift", "email", "portal"]
    },
    {
      :name "Reporting"
      :frequency "daily"
      :formats ["XML", "CSV", "PDF"]
      :recipients ["fund-manager", "depositary", "auditor"]
    },
    {
      :name "RiskMonitoring"
      :real-time true
      :breach-notifications "immediate"
      :regulatory-reports ["ucits-kiid", "ucits-risk"]
    }
  ])

(resources.plan
  :resources [
    {
      :type "CustodyAccount"
      :owner "CustodyTech"
      :account-structure "omnibus"
      :currencies ["EUR", "USD", "GBP"]
      :attr-ref @attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f}
    },
    {
      :type "FundAccountingLedger"
      :owner "AccountingTech"
      :base-currency "EUR"
      :attr-ref @attr{2c3d4e5f-6a7b-8c9d-0e1f-2a3b4c5d6e7f}
    },
    {
      :type "TransferAgencySystem"
      :owner "TransferTech"
      :share-classes ["A-EUR", "A-USD", "I-EUR"]
      :attr-ref @attr{6e7f8a9b-0c1d-2e3f-4a5b-6c7d8e9f0a1b}
    },
    {
      :type "RiskSystem"
      :owner "RiskTech"
      :monitoring-frequency "real-time"
      :attr-ref @attr{0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d}
    }
  ])

(values.bind
  :bindings [
    {:attr-id @attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f}, :value "CUST-EGOF-001"},
    {:attr-id @attr{2c3d4e5f-6a7b-8c9d-0e1f-2a3b4c5d6e7f}, :value "FA-EGOF-LU-001"},
    {:attr-id @attr{6e7f8a9b-0c1d-2e3f-4a5b-6c7d8e9f0a1b}, :value "TA-EGOF-LU"},
    {:attr-id @attr{0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d}, :value "RISK-EGOF-001"}
  ])
```

**Key Architectural Highlights:**
- **State = Complete DSL Document**: Parse final version = know everything
- **Immutable Accumulation**: Each version preserves complete history
- **AttributeID Variables**: UUIDs reference universal dictionary for governance
- **Cross-System Coordination**: All systems consume same DSL for their operations

---

## Example 2: KYC/AML with Ultimate Beneficial Owner Discovery

### Comprehensive UBO Analysis with Regulatory Compliance

```lisp
;; ════════════════════════════════════════════════════════════════════════════
;; COMPLEX CORPORATE STRUCTURE KYC/AML DSL (v3.0)
;; Client: Apex Holdings S.à r.l. (Luxembourg holding company)
;; Scenario: Multi-layered ownership requiring full UBO discovery
;; ════════════════════════════════════════════════════════════════════════════

(entity
  :id "apex-holdings-sarl"
  :label "Company"
  :props {
    :legal-name "Apex Holdings S.à r.l."
    :legal-form "private limited liability company"
    :jurisdiction "LU"
    :registration-number "B123456"
    :business-purpose "Investment holding company for European equities"
    :risk-classification "medium-high"
    :attr-ref @attr{1a2b3c4d-ubo-entity-primary}
  })

;; ────────────────────────────────────────────────────────────────────────────
;; UBO DISCOVERY - OWNERSHIP LAYER 1
;; ────────────────────────────────────────────────────────────────────────────

(edge
  :from "meridian-investment-fund-iii"
  :to "apex-holdings-sarl"
  :type "HAS_OWNERSHIP"
  :props {
    :percent 65.0
    :voting-rights 65.0
    :control-nature "direct-ownership"
    :attr-ref @attr{2b3c4d5e-ubo-shareholder-1}
  })

(edge
  :from "baltic-family-office-sa"
  :to "apex-holdings-sarl"
  :type "HAS_OWNERSHIP"
  :props {
    :percent 35.0
    :voting-rights 35.0
    :control-nature "direct-ownership"
    :attr-ref @attr{3c4d5e6f-ubo-shareholder-2}
  })

;; ────────────────────────────────────────────────────────────────────────────
;; UBO DISCOVERY - OWNERSHIP LAYER 2 (Drill Down)
;; ────────────────────────────────────────────────────────────────────────────

(ubo.ownership-structure
  (ownership-layer 2)
  (entity "Meridian Investment Fund III")
  (shareholders
    (shareholder
      (name "Meridian Capital Management Ltd")
      (type "corporate")
      (jurisdiction "BVI")  ; British Virgin Islands
      (ownership-percentage 100.0)
      (voting-rights 100.0)
      (relationship "general-partner")
      (var (attr-id @attr{4d5e6f7a-ubo-layer2-1}))
    )
  )
)

(ubo.ownership-structure
  (ownership-layer 2)
  (entity "Baltic Family Office S.A.")
  (shareholders
    (shareholder
      (name "Henrik Andersson")
      (type "individual")
      (jurisdiction "CH")
      (ownership-percentage 60.0)
      (voting-rights 60.0)
      (var (attr-id @attr{5e6f7a8b-ubo-individual-1}))  ; individual.full_name
      (control-nature "direct-ownership")
    )
    (shareholder
      (name "Andersson Family Trust")
      (type "trust")
      (jurisdiction "JE")  ; Jersey
      (ownership-percentage 40.0)
      (voting-rights 40.0)
      (var (attr-id @attr{6f7a8b9c-ubo-trust-1}))
      (control-nature "trust-beneficiary")
    )
  )
)

;; ────────────────────────────────────────────────────────────────────────────
;; UBO DISCOVERY - OWNERSHIP LAYER 3 (Final Beneficial Owners)
;; ────────────────────────────────────────────────────────────────────────────

(ubo.ownership-structure
  (ownership-layer 3)
  (entity "Meridian Capital Management Ltd")
  (shareholders
    (shareholder
      (name "Marcus Wellington")
      (type "individual")
      (jurisdiction "GB")
      (ownership-percentage 70.0)
      (voting-rights 70.0)
      (var (attr-id @attr{7a8b9c0d-ubo-individual-2}))  ; individual.full_name
      (control-nature "direct-ownership")
      (pep-status false)
      (sanctions-check "clear")
    )
    (shareholder
      (name "Sarah Chen")
      (type "individual")
      (jurisdiction "SG")
      (ownership-percentage 30.0)
      (voting-rights 30.0)
      (var (attr-id @attr{8b9c0d1e-ubo-individual-3}))  ; individual.full_name
      (control-nature "direct-ownership")
      (pep-status false)
      (sanctions-check "clear")
    )
  )
)

(ubo.ownership-structure
  (ownership-layer 3)
  (entity "Andersson Family Trust")
  (beneficial-owners
    (beneficiary
      (name "Henrik Andersson")  ; Same as direct shareholder
      (type "individual")
      (jurisdiction "CH")
      (beneficial-percentage 40.0)  ; Through trust
      (var (attr-id @attr{5e6f7a8b-ubo-individual-1}))  ; Reference same individual
      (control-nature "trust-beneficiary")
    )
    (beneficiary
      (name "Astrid Andersson")
      (type "individual") 
      (jurisdiction "CH")
      (beneficial-percentage 60.0)  ; Through trust
      (var (attr-id @attr{9c0d1e2f-ubo-individual-4}))  ; individual.full_name
      (control-nature "trust-beneficiary")
      (relationship "spouse")
    )
  )
  (trustees
    (trustee
      (name "Jersey Private Trustees Ltd")
      (type "corporate")
      (jurisdiction "JE")
      (var (attr-id @attr{0d1e2f3a-trustee-corporate}))
    )
  )
)

;; ────────────────────────────────────────────────────────────────────────────
;; FINAL UBO ANALYSIS & REGULATORY CLASSIFICATION
;; ────────────────────────────────────────────────────────────────────────────

(ubo.outcome
  :target "apex-holdings-sarl"
  :at "2024-11-10T15:00:00Z"
  :threshold 25.0
  :calculation-method "aggregated-beneficial-ownership"
  :regulatory-framework "EU4MLD"
  :ubos [
    {
      :entity "henrik-andersson"
      :jurisdiction "CH"
      :effective-percent 35.0
      :direct-ownership 21.0
      :indirect-ownership 14.0
      :above-threshold true
      :control-mechanisms ["direct-ownership", "trust-beneficiary"]
      :attr-ref @attr{5e6f7a8b-ubo-individual-1}
    },
    {
      :entity "marcus-wellington"
      :jurisdiction "GB"
      :effective-percent 45.5
      :direct-ownership 45.5
      :indirect-ownership 0.0
      :above-threshold true
      :control-mechanisms ["direct-ownership"]
      :attr-ref @attr{7a8b9c0d-ubo-individual-2}
    },
    {
      :entity "sarah-chen"
      :jurisdiction "SG"
      :effective-percent 19.5
      :direct-ownership 19.5
      :indirect-ownership 0.0
      :above-threshold false
      :control-mechanisms ["direct-ownership"]
      :attr-ref @attr{8b9c0d1e-ubo-individual-3}
    },
    {
      :entity "astrid-andersson"
      :jurisdiction "CH"
      :effective-percent 8.4
      :direct-ownership 0.0
      :indirect-ownership 8.4
      :above-threshold false
      :control-mechanisms ["trust-beneficiary"]
      :attr-ref @attr{9c0d1e2f-ubo-individual-4}
    }
  ]
  :reportable-ubos 2)

;; ────────────────────────────────────────────────────────────────────────────
;; KYC DOCUMENT COLLECTION & VERIFICATION
;; ────────────────────────────────────────────────────────────────────────────

(kyc.document-collection
  (enhanced-due-diligence true)  ; Required for complex structures
  (documents
    ;; Primary Entity Documents
    (document-set "primary-entity"
      (document "CertificateOfIncorporation" (entity "Apex Holdings S.à r.l."))
      (document "ArticlesOfAssociation" (entity "Apex Holdings S.à r.l."))
      (document "ShareRegister" (entity "Apex Holdings S.à r.l.") (current true))
      (document "BoardResolution" (entity "Apex Holdings S.à r.l."))
    )
    
    ;; UBO Individual Documents
    (document-set "ubo-henrik-andersson"
      (document "PassportCopy" (individual "Henrik Andersson") (certified true))
      (document "ProofOfAddress" (individual "Henrik Andersson") (recent-months 3))
      (document "WealthDeclaration" (individual "Henrik Andersson"))
      (document "PEPDeclaration" (individual "Henrik Andersson"))
      (document "TaxResidencyDeclaration" (individual "Henrik Andersson"))
    )
    
    (document-set "ubo-marcus-wellington"
      (document "PassportCopy" (individual "Marcus Wellington") (certified true))
      (document "ProofOfAddress" (individual "Marcus Wellington") (recent-months 3))
      (document "WealthDeclaration" (individual "Marcus Wellington"))
      (document "PEPDeclaration" (individual "Marcus Wellington"))
      (document "TaxResidencyDeclaration" (individual "Marcus Wellington"))
    )
    
    ;; Complex Structure Documentation
    (document-set "ownership-structure"
      (document "OwnershipChart" (format "visual-diagram"))
      (document "TrustDeed" (entity "Andersson Family Trust"))
      (document "TrusteeResolution" (entity "Jersey Private Trustees Ltd"))
      (document "FundDocuments" (entity "Meridian Investment Fund III"))
    )
  )
)

;; ────────────────────────────────────────────────────────────────────────────
;; AML SCREENING & RISK ASSESSMENT
;; ────────────────────────────────────────────────────────────────────────────

(aml.screening
  (screening-scope "comprehensive")
  (screening-targets
    (target "Henrik Andersson" (type "individual") (jurisdiction "CH"))
    (target "Marcus Wellington" (type "individual") (jurisdiction "GB"))
    (target "Sarah Chen" (type "individual") (jurisdiction "SG"))
    (target "Astrid Andersson" (type "individual") (jurisdiction "CH"))
    (target "Apex Holdings S.à r.l." (type "entity") (jurisdiction "LU"))
    (target "Meridian Investment Fund III" (type "entity") (jurisdiction "KY"))
    (target "Baltic Family Office S.A." (type "entity") (jurisdiction "CH"))
  )
  (screening-databases
    "OFAC-SDN" "UN-Sanctions" "EU-Sanctions" "PEP-Lists" 
    "Adverse-Media" "Enforcement-Actions"
  )
  (results
    (result "Henrik Andersson" (status "clear") (matches 0))
    (result "Marcus Wellington" (status "clear") (matches 0))
    (result "Sarah Chen" (status "clear") (matches 0))
    (result "Astrid Andersson" (status "clear") (matches 0))
    (result "Apex Holdings S.à r.l." (status "clear") (matches 0))
    (result "Meridian Investment Fund III" (status "review-required") 
            (matches 1) (reason "jurisdiction-risk-cayman"))
    (result "Baltic Family Office S.A." (status "clear") (matches 0))
  )
)

(aml.risk-assessment
  (overall-risk-rating "medium-high")
  (risk-factors
    (factor "complex-ownership-structure" (weight "high"))
    (factor "multiple-jurisdictions" (weight "medium"))
    (factor "offshore-components" (weight "medium") (details "Cayman, BVI, Jersey"))
    (factor "trust-structures" (weight "medium"))
    (factor "family-office-involvement" (weight "low"))
  )
  (enhanced-monitoring true)
  (review-frequency "quarterly")
  (approval-level "senior-management")
)

;; ────────────────────────────────────────────────────────────────────────────
;; REGULATORY REPORTING OBLIGATIONS
;; ────────────────────────────────────────────────────────────────────────────

(aml.regulatory-reporting
  (crs-reporting
    (reportable-jurisdictions "CH" "GB" "SG")
    (reportable-individuals "Henrik Andersson" "Marcus Wellington" "Sarah Chen" "Astrid Andersson")
  )
  (fatca-reporting
    (us-persons false)
    (reporting-required false)
  )
  (local-reporting
    (jurisdiction "LU")
    (ubo-register-filing true)
    (beneficial-owners "Henrik Andersson" "Marcus Wellington")
  )
)
```

**Hedge Fund DSL Architecture Highlights:**
- **Institutional Sophistication Assessment**: Regulatory status and expertise verification
- **Enhanced KYC for Institutions**: Corporate governance and financial standing analysis
- **Multi-Jurisdiction Compliance**: Belgian FSMA, Cayman FIU, EU regulations
- **Ongoing Monitoring**: Transaction-based AML surveillance
- **Complex Documentation**: Subscription agreements, side letters, and fee arrangements

---

## Execution Summary: DSL-as-State in Action

### **What These Examples Demonstrate**

#### **1. Configuration Over Code Philosophy**
```lisp
;; Business rules are expressed in DSL, not hardcoded
(ubo.final-analysis
  (calculation-method "aggregated-beneficial-ownership")
  (regulatory-threshold 25.0)  ; EU 4th AML Directive
)

;; Changes to thresholds require DSL updates, not code deployments
```

#### **2. Cross-System Orchestration**
Each DSL document coordinates multiple enterprise systems:
- **UCITS Example**: Custody, Fund Accounting, Transfer Agency, Risk Management
- **KYC/AML Example**: Screening databases, Document management, Regulatory reporting
- **Hedge Fund Example**: Subscription processing, Ongoing monitoring, Compliance systems

#### **3. AttributeID Governance**
```lisp
@attr{8d9e0f1a-2b3c-4d5e-6f7a-8b9c0d1e2f3a}  ; subscription.amount = 10000000
```
- UUID references universal dictionary for validation, privacy, and governance
- Same attribute can be reused across different contexts
- Metadata-driven type system with built-in compliance

#### **4. Incremental State Building**
- **UCITS**: 5 versions from initial creation to full resource provisioning
- **KYC/AML**: Accumulated through 3 layers of ownership discovery
- **Hedge Fund**: Progressive from opportunity through subscription execution

#### **5. Audit Trail by Design**
Every DSL version contains:
- ✅ Complete current state (parse DSL = know everything)
- ✅ Full historical progression (how we got here)
- ✅ Regulatory compliance evidence (what decisions were made)
- ✅ Cross-system coordination instructions (what happens next)

### **Real-World Impact**

| **Traditional Approach** | **DSL-as-State Solution** |
|--------------------------|---------------------------|
| State scattered across 15+ tables | State = single DSL document |
| 6-month integration projects | Hours to add new systems |
| Manual compliance preparation | Automatic audit trail generation |
| Rigid workflows requiring code changes | Configuration-driven adaptability |
| AI integration impossible | Safe AI with approved vocabulary |
| "What happened?" = forensic analysis | Parse any DSL version instantly |

### **Enterprise Benefits Realized**

#### **Regulatory Compliance**
- **UCITS**: CSSF, BaFin, AMF approvals tracked automatically
- **KYC/AML**: EU 4th AML Directive UBO compliance built-in
- **Hedge Fund**: Multi-jurisdiction reporting (FSMA, Cayman FIU) coordinated

#### **AI Integration**
- **Approved Vocabulary**: 70+ validated verbs prevent AI hallucination
- **Structured Context**: AI operates within safe, business-approved boundaries
- **Human Oversight**: Business users can read and validate AI decisions

#### **Operational Efficiency**
- **Time Travel**: Access any historical state instantly
- **Cross-System Coordination**: Universal DSL language eliminates integration complexity
- **Configuration Changes**: Adapt to new regulations without code deployments

#### **Data Governance**
- **Privacy by Design**: AttributeID system embeds privacy flags
- **Universal Contracts**: Same data types across all systems
- **Audit Readiness**: Complete provenance tracking built-in

---

**The DSL-as-State architecture transforms enterprise onboarding from a technical liability into a strategic business asset—enabling regulatory compliance, AI integration, and operational agility through sophisticated yet practical design patterns.**
```

**UBO DSL Architecture Highlights:**
- **Layered Ownership Discovery**: Systematic drilling through corporate layers
- **Aggregated Beneficial Ownership**: Mathematical calculation of final percentages  
- **Regulatory Threshold Analysis**: 25% EU AML Directive compliance
- **Enhanced Due Diligence**: Document collection for complex structures
- **Cross-Jurisdiction Screening**: Multi-database AML screening
- **Regulatory Reporting**: CRS, FATCA, and local UBO register compliance

---

## Example 3: Hedge Fund Investor Onboarding

### Sophisticated Investor Journey with Risk Assessment

```lisp
;; ════════════════════════════════════════════════════════════════════════════
;; HEDGE FUND INVESTOR ONBOARDING DSL
;; Fund: Quantum Alpha Master Fund Ltd (Cayman Islands)
;; Investor: Institutional - European Pension Fund
;; ════════════════════════════════════════════════════════════════════════════

(investor.start-opportunity
  @attr{1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d}  ; investor_id
  @attr{2b3c4d5e-6f7a-8b9c-0d1e-2f3a4b5c6d7e}  ; investor.legal_name = "European Pension Scheme AISBL"
  @attr{3c4d5e6f-7a8b-9c0d-1e2f-3a4b5c6d7e8f}  ; investor.type = "INSTITUTIONAL_PENSION"
  @attr{4d5e6f7a-8b9c-0d1e-2f3a-4b5c6d7e8f9a}  ; investor.domicile = "BE"
  @attr{5e6f7a8b-9c0d-1e2f-3a4b-5c6d7e8f9a0b}  ; investor.aum = 2500000000  ; €2.5B AUM
)

(fund.context
  @attr{6f7a8b9c-0d1e-2f3a-4b5c-6d7e8f9a0b1c}  ; fund_id = "quantum-alpha-master"
  @attr{7a8b9c0d-1e2f-3a4b-5c6d-7e8f9a0b1c2d}  ; fund.name = "Quantum Alpha Master Fund Ltd"
  @attr{8b9c0d1e-2f3a-4b5c-6d7e-8f9a0b1c2d3e}  ; fund.strategy = "equity-long-short"
  @attr{9c0d1e2f-3a4b-5c6d-7e8f-9a0b1c2d3e4f}  ; fund.domicile = "KY"
  @attr{0d1e2f3a-4b5c-6d7e-8f9a-0b1c2d3e4f5a}  ; fund.minimum_investment = 5000000  ; €5M minimum
)

(share-class.selection
  @attr{1e2f3a4b-5c6d-7e8f-9a0b-1c2d3e4f5a6b}  ; class_id = "institutional-eur"
  @attr{2f3a4b5c-6d7e-8f9a-0b1c-2d3e4f5a6b7c}  ; class.currency = "EUR"
  @attr{3a4b5c6d-7e8f-9a0b-1c2d-3e4f5a6b7c8d}  ; class.management_fee = 1.50
  @attr{4b5c6d7e-8f9a-0b1c-2d3e-4f5a6b7c8d9e}  ; class.performance_fee = 20.00
  @attr{5c6d7e8f-9a0b-1c2d-3e4f-5a6b7c8d9e0f}  ; class.high_water_mark = true
)

;; ────────────────────────────────────────────────────────────────────────────
;; SOPHISTICATED INVESTOR VERIFICATION
;; ────────────────────────────────────────────────────────────────────────────

(investor.sophistication-assessment
  (assessment-type "institutional-qualification")
  (criteria
    (criterion "regulatory-status"
      (status "IORP-II-compliant")  ; EU pension fund regulation
      (jurisdiction "BE")
      @attr{6d7e8f9a-0b1c-2d3e-4f5a-6b7c8d9e0f1a}  ; regulatory_status
    )
    (criterion "investment-expertise"
      (professional-investment-team true)
      (derivatives-experience true)
      (alternative-investments-experience true)
      @attr{7e8f9a0b-1c2d-3e4f-5a6b-7c8d9e0f1a2b}  ; expertise_level = "EXPERT"
    )
    (criterion "financial-resources"
      (net-assets 2500000000)
      (investment-allocation 10000000)  ; €10M intended investment
      (liquidity-requirements "quarterly")
      @attr{8f9a0b1c-2d3e-4f5a-6b7c-8d9e0f1a2b3c}  ; financial_capacity
    )
  )
  (sophistication-level "QUALIFIED_INSTITUTIONAL")
)

;; ────────────────────────────────────────────────────────────────────────────
;; KYC FOR INSTITUTIONAL INVESTOR
;; ────────────────────────────────────────────────────────────────────────────

(kyc.begin
  @attr{1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d}  ; investor_id (reference)
  @attr{9a0b1c2d-3e4f-5a6b-7c8d-9e0f1a2b3c4d}  ; kyc.tier = "ENHANCED_INSTITUTIONAL"
)

(kyc.institutional-verification
  (entity-type "non-profit-pension-scheme")
  (regulatory-oversight
    (primary-regulator "FSMA")  ; Belgian Financial Services and Markets Authority
    (license-number "PF-2019-001234")
    (license-status "active")
    @attr{0b1c2d3e-4f5a-6b7c-8d9e-0f1a2b3c4d5e}  ; regulatory_license
  )
  (governance-structure
    (board-composition "tripartite")  ; employers, employees, independents
    (investment-committee true)
    (independent-oversight true)
    @attr{1c2d3e4f-5a6b-7c8d-9e0f-1a2b3c4d5e6f}  ; governance_structure
  )
  (financial-standing
    (total-assets 2500000000)
    (investment-grade-rating "AA")
    (annual-audit "Deloitte Belgium")
    @attr{2d3e4f5a-6b7c-8d9e-0f1a-2b3c4d5e6f7a}  ; financial_metrics
  )
)

(kyc.collect-doc
  @attr{1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d}  ; investor_id (reference)
  @attr{3e4f5a6b-7c8d-9e0f-1a2b-3c4d5e6f7a8b}  ; document.type = "CertificateOfIncorporation"
  @attr{2b3c4d5e-6f7a-8b9c-0d1e-2f3a4b5c6d7e}  ; document.subject = "European Pension Scheme AISBL"
  (document-details
    (jurisdiction "BE")
    (language "EN")
    (certification "apostille")
    (expiry-date "2029-12-31")
  )
)

(kyc.collect-doc
  @attr{1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d}  ; investor_id (reference)
  @attr{4f5a6b7c-8d9e-0f1a-2b3c-4d5e6f7a8b9c}  ; document.type = "BoardResolution"
  @attr{2b3c4d5e-6f7a-8b9c-0d1e-2f3a4b5c6d7e}  ; document.subject = "European Pension Scheme AISBL"
  (resolution-details
    (resolution-date "2024-01-15")
    (investment-authorization 10000000)  ; €10M authorized
    (signatory-authority "Investment Committee Chair")
  )
)

(kyc.collect-doc
  @attr{1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d}  ; investor_id (reference)
  @attr{5a6b7c8d-9e0f-1a2b-3c4d-5e6f7a8b9c0d}  ; document.type = "AuditedFinancials"
  @attr{2b3c4d5e-6f7a-8b9c-0d1e-2f3a4b5c6d7e}  ; document.subject = "European Pension Scheme AISBL"
  (audit-details
    (audit-year "2023")
    (auditor "Deloitte Belgium")
    (opinion "unqualified")
    (total-assets 2500000000)
  )
)

;; ────────────────────────────────────────────────────────────────────────────
;; RISK ASSESSMENT & SUITABILITY
;; ────────────────────────────────────────────────────────────────────────────

(investor.risk-assessment
  (risk-profile
    (risk-tolerance "moderate-to-high")
    (investment-horizon "long-term")  ; 7-10 years
    (liquidity-needs "quarterly")
    @attr{6b7c8d9e-0f1a-2b3c-4d5e-6f7a8b9c0d1e}  ; risk_profile
  )
  (institutional-factors
    (fiduciary-duty "prudent-person-principle")
    (diversification-requirements true)
    (esg-mandate true)
    (benchmark "MSCI Europe Equity Index")
    @attr{7c8d9e0f-1a2b-3c4d-5e6f-7a8b9c0d1e2f}  ; institutional_constraints
  )
  (suitability-conclusion "SUITABLE_SUBJECT_TO_CONDITIONS")
)

;; ────────────────────────────────────────────────────────────────────────────
;; SUBSCRIPTION PROCESSING
;; ────────────────────────────────────────────────────────────────────────────

(subscription.initiate
  @attr{1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d}  ; investor_id (reference)
  @attr{6f7a8b9c-0d1e-2f3a-4b5c-6d7e8f9a0b1c}  ; fund_id (reference)
  @attr{1e2f3a4b-5c6d-7e8f-9a0b-1c2d3e4f5a6b}  ; class_id (reference)
  @attr{8d9e0f1a-2b3c-4d5e-6f7a-8b9c0d1e2f3a}  ; subscription.amount = 10000000  ; €10M
  @attr{9e0f1a2b-3c4d-5e6f-7a8b-9c0d1e2f3a4b}  ; subscription.currency = "EUR"
  (subscription-terms
    (dealing-date "2024-03-01")
    (settlement-date "2024-03-05")
    (nav-basis "forward-pricing")
    (management-fee-basis "daily-accrual")
  )
)

(subscription.documentation
  (subscription-agreement
    (executed-date "2024-02-15")
    (governing-law "Cayman Islands")
    (dispute-resolution "London arbitration")
  )
  (side-letters
    (fee-arrangements "institutional-discount")
    (reporting-requirements "monthly-detailed")
    (redemption-terms "quarterly-liquidity")
  )
)

(aml.ongoing-monitoring
  (monitoring-scope "transaction-based")
  (monitoring-frequency "continuous")
  (thresholds
    (large-transaction 1000000)  ; €1M threshold
    (unusual-pattern-detection true)
    (source-of-funds-verification "periodic")
  )
  (reporting-obligations
    (belgium-ctif true)  ; Financial Intelligence Unit
    (cayman-fiu true)    ; Fund domicile reporting
  )
)
```