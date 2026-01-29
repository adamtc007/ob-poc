;; highlights.scm - Syntax highlighting for OB-POC DSL
;;
;; Uses DSL-specific node names from tree-sitter-dsl/grammar.js

;; Comments (;; to end of line)
(comment) @comment

;; Verb names (domain.verb)
(verb_name) @function

;; Keywords (:arg-name)
(keyword) @property

;; Binding (:as @symbol) - special highlighting
(binding ":as" @keyword.special)
(binding (symbol_ref) @variable.special)

;; Symbol references (@name)
(symbol_ref) @variable

;; String literals
(string) @string

;; Number literals
(number) @number

;; Boolean literals (true, false)
(boolean) @constant.builtin

;; Null literal (nil)
(null_literal) @constant.builtin

;; Brackets
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
