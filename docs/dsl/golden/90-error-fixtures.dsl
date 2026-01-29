;; ============================================================================
;; Error Fixtures
;; ============================================================================
;; intent: Intentional parse errors for testing error recovery and diagnostics
;;
;; This file contains INTENTIONAL errors to test:
;; - Parser error recovery
;; - LSP diagnostic reporting
;; - Tree-sitter incremental parsing stability
;;
;; DO NOT "fix" these errors - they are test fixtures.

;; ============================================================================
;; Section 1: Syntax Errors
;; ============================================================================

;; ERROR: Missing closing parenthesis
;; Expected: Diagnostic at end of expression
(cbu.create :name "Unclosed"

;; ERROR: Missing opening parenthesis
;; Expected: Diagnostic at verb name
cbu.create :name "No open paren")

;; ERROR: Unmatched brackets
;; Expected: Diagnostic showing bracket mismatch
(trading-profile.set-instruments :instruments ["A" "B" "C")

;; ERROR: Unmatched braces
;; Expected: Diagnostic showing brace mismatch
(config.set :settings {"key" "value")

;; ============================================================================
;; Section 2: Invalid Tokens
;; ============================================================================

;; ERROR: Invalid verb name (no domain)
;; Expected: Diagnostic "verb must have domain.action format"
(create :name "No Domain")

;; ERROR: Invalid keyword (missing colon)
;; Expected: Diagnostic for invalid keyword
(cbu.create name "Missing Colon")

;; ERROR: Unterminated string
;; Expected: Diagnostic "unterminated string literal"
(entity.create :name "Unterminated)

;; ERROR: Invalid number
;; Expected: Diagnostic for malformed number
(holding.set :shares 12.34.56)

;; ============================================================================
;; Section 3: Binding Errors
;; ============================================================================

;; ERROR: Binding without symbol
;; Expected: Diagnostic "expected symbol after :as"
(cbu.create :name "No Symbol" :as)

;; ERROR: Invalid binding symbol (missing @)
;; Expected: Diagnostic "binding symbol must start with @"
(cbu.create :name "Bad Symbol" :as fund)

;; ERROR: Duplicate binding in same file
;; Expected: Warning "binding @dup shadows previous definition"
(cbu.create :name "First" :as @dup)
(cbu.create :name "Second" :as @dup)

;; ============================================================================
;; Section 4: Reference Errors
;; ============================================================================

;; ERROR: Undefined symbol reference
;; Expected: Diagnostic "undefined symbol @nonexistent"
(cbu-role.assign :cbu-id @nonexistent :entity-id @also_missing :role "TEST")

;; ERROR: Invalid entity reference syntax
;; Expected: Diagnostic for malformed entity ref
(session.load-galaxy :apex-name <Unclosed Entity)

;; ============================================================================
;; Section 5: Structural Errors
;; ============================================================================

;; ERROR: Empty verb call
;; Expected: Diagnostic "empty expression"
()

;; ERROR: Nested verb calls (not supported)
;; Expected: Diagnostic or parse error
(cbu.create :name (entity.get-name :id @entity))

;; ERROR: List where scalar expected
;; Expected: Type diagnostic
(cbu.create :name ["Array" "Not" "Allowed"])

;; ============================================================================
;; Section 6: Recoverable Parse States
;; ============================================================================

;; These test incremental parsing recovery - parser should recover
;; and continue parsing subsequent valid expressions.

;; Valid after error
(cbu.create :name "Valid After Errors" :as @recovered)

;; This should parse correctly
(cbu.get :id @recovered)

;; ============================================================================
;; Section 7: Edge Cases
;; ============================================================================

;; ERROR: Very long string (may cause issues)
(entity.create :name "This is an extremely long name that goes on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and might cause buffer issues in some parsers")

;; ERROR: Special characters in unexpected places
(cbu.create :name "Test\nNewline\tTab")

;; ERROR: Unicode edge cases
(entity.create :name "Emoji 🎉 in name")

;; ERROR: Null byte (should be rejected)
;; Note: Cannot actually include null byte in source file

;; ============================================================================
;; End of Error Fixtures
;; ============================================================================
;; The following line should parse correctly, proving error recovery worked.
(session.info)
