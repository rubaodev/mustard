; Dart — contracts a type builds on: `class X extends Base implements IFoo`
; and `class X with Mixin`. @supertype names are attached (by declaration name)
; to the matching @definition. The engine mines whichever base name recurs
; across many types as a shared contract. Only the generic capture vocabulary.
;
; Verified against tree-sitter-dart-orchard 0.3 node-types.json:
;   class_definition has a `superclass:` field (node `superclass`, holding the
;     extended (type_identifier) and an optional `with` (mixins)) and an
;     `interfaces:` field (node `interfaces`, the `implements` list).
(class_definition
  name: (identifier) @name
  superclass: (superclass (type_identifier) @supertype))
(class_definition
  name: (identifier) @name
  superclass: (superclass (mixins (type_identifier) @supertype)))
(class_definition
  name: (identifier) @name
  interfaces: (interfaces (type_identifier) @supertype))
