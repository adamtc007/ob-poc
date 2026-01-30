;; ============================================================================
;; LSP Interactive Test File
;; ============================================================================
;; intent: Minimal syntax tour for testing LSP features
;;
;; Open this file in Zed to test:
;; - Syntax highlighting
;; - Diagnostics (errors/warnings)
;; - Outline view
;; - Hover information
;; - Completions

;; ----------------------------------------------------------------------------
;; 1. Basic Verb Call
;; ----------------------------------------------------------------------------

;; intent: Simple verb with string argument
;; macro: primitive
;; constraints: none
(cbu.create :name "Syntax Tour Fund")

;; ----------------------------------------------------------------------------
;; 2. Binding with :as
;; ----------------------------------------------------------------------------

;; intent: Capture result for later reference
;; macro: primitive
;; constraints: none
(cbu.create :name "Bound Fund" :type "FUND" :jurisdiction "LU" :as @demo-fund)

;; ----------------------------------------------------------------------------
;; 3. Map Literal
;; ----------------------------------------------------------------------------

;; intent: Pass configuration as map value
;; macro: primitive
;; constraints: @demo-fund must exist
(trading.profile.create
  :cbu-id @demo-fund
  :name "Main Profile"
  :config {:default-currency "EUR" :settlement-cycle 2}
  :as @profile)

;; ----------------------------------------------------------------------------
;; 4. Nested Call (if supported)
;; ----------------------------------------------------------------------------

;; intent: Create entity inline within role assignment
;; macro: primitive
;; constraints: @demo-fund must exist
;; NOTE: Nested calls ARE supported - entity created inline
(cbu.role.assign
  :cbu-id @demo-fund
  :entity-id (entity.create :name "Demo Director" :type "NATURAL" :as @director)
  :role "DIRECTOR"
  :effective-date "2024-01-01")

;; Alternative: separate calls (always works)
(entity.create :name "Demo Director" :type "NATURAL" :as @director)
(cbu.role.assign
  :cbu-id @demo-fund
  :entity-id @director
  :role "DIRECTOR"
  :effective-date "2024-01-01")

;; ----------------------------------------------------------------------------
;; 5. Test Editing Below This Line
;; ----------------------------------------------------------------------------

;; Try typing here to test completions:
;; - Type "(" then a domain like "cbu" or "kyc"
;; - Type ":" for keyword completions
;; - Reference @demo-fund or @profile
