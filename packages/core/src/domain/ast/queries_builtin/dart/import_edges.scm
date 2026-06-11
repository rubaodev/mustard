; Dart import directives.
; Verified against tree-sitter-dart node-types.json:
;   import_specification carries a `configurable_uri`/`uri` whose value is a
;   (string_literal). Capturing the whole specification keeps the extractor
;   agnostic about `as`/`show`/`hide`/`deferred` variants.
(import_specification) @import
