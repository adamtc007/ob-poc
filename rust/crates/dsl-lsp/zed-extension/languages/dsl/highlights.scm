; Keywords (verb names)
(symbol) @function
((symbol) @function.builtin
  (#match? @function.builtin "^(cbu|entity|document|investigation|risk|screening|decision)\\."))

; Keywords (arguments)
(keyword) @property

; Strings
(string) @string

; Numbers
(number) @number

; Symbol references (@name)
(symbol_ref) @variable.special

; Comments
(comment) @comment

; Parentheses
["(" ")"] @punctuation.bracket

; Colons
":" @punctuation.delimiter
