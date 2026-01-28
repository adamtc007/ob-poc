; Playbook YAML syntax highlighting
; Using tree-sitter-yaml queries

; Keys
(block_mapping_pair
  key: (flow_node) @property)

; String values
(double_quote_scalar) @string
(single_quote_scalar) @string
(block_scalar) @string

; Numbers
(integer_scalar) @number
(float_scalar) @number

; Booleans
(boolean_scalar) @constant.builtin

; Null
(null_scalar) @constant.builtin

; Comments
(comment) @comment

; Anchors and aliases
(anchor) @label
(alias) @label

; Tags
(tag) @type

; Punctuation
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
":" @punctuation.delimiter
"-" @punctuation.special
