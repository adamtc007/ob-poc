;; Each verb call is an outline item
(list (verb_name) @name) @item

;; Show binding in outline
(list
  (verb_name) @name
  (binding (symbol_ref) @context.extra)) @item

;; Preceding comments become @annotation for Assistant (grouped pattern)
(
  (comment)+ @annotation
  .
  (list (verb_name) @name) @item
)
