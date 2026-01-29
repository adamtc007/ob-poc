;; overrides.scm - Scope overrides for Zed
;;
;; Use @comment.inclusive for line comments so scope reaches newline.

;; Inside strings: disable certain completions
(string) @string

;; Inside comments: inclusive so scope reaches newline
(comment) @comment.inclusive
