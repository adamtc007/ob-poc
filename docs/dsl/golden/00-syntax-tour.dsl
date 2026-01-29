;; ============================================================================
;; DSL Syntax Tour
;; ============================================================================
;; intent: Demonstrate all DSL syntax constructs for reference and testing
;;
;; This file shows every syntax element supported by the DSL parser.
;; Use it to verify tree-sitter highlighting, outline, and completion.

;; ----------------------------------------------------------------------------
;; 1. Basic Verb Calls
;; ----------------------------------------------------------------------------

;; intent: Simple verb with no arguments
(session.info)

;; intent: Verb with keyword arguments
(cbu.create :name "Acme Fund" :jurisdiction "LU")

;; intent: Verb with binding
(cbu.create :name "Test Fund" :as @fund)

;; ----------------------------------------------------------------------------
;; 2. Data Types
;; ----------------------------------------------------------------------------

;; intent: String values (double-quoted)
(entity.create :name "John Doe" :nationality "US")

;; intent: Numeric values (integers and decimals)
(holding.set :shares 1000 :price 45.50 :nav 1234567.89)

;; intent: Boolean values
(cbu.update :id @fund :active true :archived false)

;; intent: Null literal
(entity.update :id @person :middle-name null)

;; intent: Symbol references (bindings from previous calls)
(cbu-role.assign :cbu-id @fund :entity-id @person :role "DIRECTOR")

;; intent: Entity references (resolved by lookup)
(session.load-galaxy :apex-name <Allianz>)

;; ----------------------------------------------------------------------------
;; 3. Collections
;; ----------------------------------------------------------------------------

;; intent: Array/list values
(trading-profile.set-instruments
  :cbu-id @fund
  :instruments ["EQUITY" "FIXED_INCOME" "DERIVATIVES"])

;; intent: Nested arrays
(batch.process :items [["A" 1] ["B" 2] ["C" 3]])

;; intent: Map values
(config.set :settings {"theme" "dark" "locale" "en-US" "debug" false})

;; intent: Nested structures
(workflow.create
  :template "onboarding"
  :params {
    "client" "Acme Corp"
    "products" ["CUSTODY" "TRADING"]
    "contacts" [
      {"name" "Alice" "role" "primary"}
      {"name" "Bob" "role" "backup"}
    ]
  })

;; ----------------------------------------------------------------------------
;; 4. Keywords (Named Arguments)
;; ----------------------------------------------------------------------------

;; intent: Standard keywords with hyphenated names
(entity.create-proper-person
  :first-name "Jane"
  :last-name "Smith"
  :date-of-birth "1985-03-15"
  :tax-residency "DE"
  :as @jane)

;; intent: Keywords with underscores (also valid)
(legacy.import :source_system "SAP" :batch_id 42)

;; ----------------------------------------------------------------------------
;; 5. Verb Name Formats
;; ----------------------------------------------------------------------------

;; intent: Simple domain.action
(session.clear)

;; intent: Hyphenated action names
(kyc-case.create-from-template :template "standard-onboarding")

;; intent: Multi-part domain
(trading-profile.add-instrument :cbu-id @fund :instrument "EQUITY")

;; ----------------------------------------------------------------------------
;; 6. Comments
;; ----------------------------------------------------------------------------

;; Single line comment

;; Multi-line comments are just
;; multiple single-line comments
;; stacked together

;; intent: Comments serve as documentation
;; macro: cbu.ensure
(cbu.ensure :name "Documented Fund" :as @documented)

;; ----------------------------------------------------------------------------
;; 7. Bindings (The :as Pattern)
;; ----------------------------------------------------------------------------

;; intent: Create and immediately reference
(entity.create :name "Parent Corp" :as @parent)
(entity.create :name "Subsidiary Ltd" :as @child)
(entity.link :parent-id @parent :child-id @child :relationship "OWNS")

;; intent: Multiple bindings in a sequence
(cbu.create :name "Fund A" :as @fund_a)
(cbu.create :name "Fund B" :as @fund_b)
(cbu.create :name "Fund C" :as @fund_c)

;; ----------------------------------------------------------------------------
;; 8. Whitespace Handling
;; ----------------------------------------------------------------------------

;; intent: Compact single-line
(cbu.list :jurisdiction "LU")

;; intent: Multi-line with indentation
(trading-profile.create
  :cbu-id @fund
  :name "Growth Strategy"
  :risk-level "MODERATE"
  :benchmark "MSCI World"
  :as @profile)

;; intent: Aligned keyword style
(contract.create
  :client       "Allianz"
  :reference    "MSA-2024-001"
  :effective    "2024-01-01"
  :expires      "2029-12-31"
  :as           @contract)
