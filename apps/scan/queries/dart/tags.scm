; Dart — imports and definitions. Generic capture vocabulary only:
;   @import            an import directive's URI (the `package:`/relative path)
;   @name              the identifier of the enclosing @definition.*
;   @definition.<kind> a declaration; <kind> becomes Decl.kind verbatim
; The engine knows ONLY these capture names — never a node name or a language.
;
; Verified against tree-sitter-dart-orchard 0.3 node-types.json:
;   library_import -> import_specification -> (uri (string_literal)); capturing
;     the (uri) keeps the path out of any `as`/`show`/`hide` combinator, and the
;     engine's clean_import strips the surrounding quotes.
;   class_definition / mixin_declaration / enum_declaration /
;     extension_declaration each expose a `name:` field of type (identifier).
;   a class body member is a (function_signature name: (identifier)) — the
;     closest the grammar has to a method declaration node.
(import_specification (configurable_uri (uri) @import))
(import_specification (uri) @import)

(class_definition name: (identifier) @name) @definition.class
; `mixin_declaration` has NO `name:` field (node-types.json: fields = {}); the
; mixin name is a positional (identifier) child — a `name:` pattern would fail
; to compile and be dropped silently, so match it positionally.
(mixin_declaration (identifier) @name) @definition.mixin
(enum_declaration name: (identifier) @name) @definition.enum
(extension_declaration name: (identifier) @name) @definition.extension

; Members — methods/functions declared in a class or library body. Member kinds
; feed the digest's domain-term index only: the miner's significance gate
; (mine.rs) is kind-based and never sees them.
(function_signature name: (identifier) @name) @definition.method
