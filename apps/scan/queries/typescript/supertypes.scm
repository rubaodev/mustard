; TypeScript — `class A extends B implements C, D` and `interface A extends B`.
; Generic-argument lists captured by the wildcard are dropped by the engine's
; name cleaner (a "<T>" reduces to nothing), so only real type names survive.
(class_declaration name: (_) @name (class_heritage (extends_clause (_) @supertype)))
(class_declaration name: (_) @name (class_heritage (implements_clause (_) @supertype)))
(abstract_class_declaration name: (_) @name (class_heritage (extends_clause (_) @supertype)))
(abstract_class_declaration name: (_) @name (class_heritage (implements_clause (_) @supertype)))
(interface_declaration name: (_) @name (extends_type_clause (_) @supertype))
