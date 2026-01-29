;; runnables.scm - Run buttons for Zed
;;
;; Zed exposes non-underscore-prefixed captures as ZED_CUSTOM_<name> env vars.
;; Use (#set! tag ...) to bind tasks by tag.

;; Basic form: run button on verb name
(
  (list
    (verb_name) @run @verb)
  (#set! tag dsl-form)
)

;; Form with binding: also capture the symbol
(
  (list
    (verb_name) @run @verb
    (binding
      (symbol_ref) @binding))
  (#set! tag dsl-form-with-binding)
)
