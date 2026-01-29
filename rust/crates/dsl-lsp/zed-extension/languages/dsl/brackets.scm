;; brackets.scm - Rainbow bracket support for Zed
;;
;; Zed expects paired @open/@close in a single pattern for rainbow brackets.

;; Parentheses (S-expressions)
("(" @open ")" @close)

;; Square brackets (arrays/lists)
("[" @open "]" @close)

;; Curly braces (maps)
("{" @open "}" @close)
