; Symbols (verb names, identifiers)
(sym_lit) @function

; Keywords (:arg-name)
(kwd_lit) @property

; Strings
(str_lit) @string

; Numbers
(num_lit) @number

; Booleans
(bool_lit) @constant.builtin

; Nil
(nil_lit) @constant.builtin

; Comments
(comment) @comment

; Lists (S-expressions)
(list_lit) @punctuation.bracket

; Brackets
["(" ")" "[" "]" "{" "}"] @punctuation.bracket

; Deref (@symbol)
(derefing_lit) @variable.special
