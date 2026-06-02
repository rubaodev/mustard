; C# — contracts a type builds on: `class X : Base, IFoo`.
; @supertype names are attached (by declaration name) to the matching @definition.
; The base list holds both the base class and implemented interfaces; the engine
; mines whichever base name recurs across many types as a shared contract.

(class_declaration name: (identifier) @name (base_list (_) @supertype))
(interface_declaration name: (identifier) @name (base_list (_) @supertype))
(record_declaration name: (identifier) @name (base_list (_) @supertype))
(struct_declaration name: (identifier) @name (base_list (_) @supertype))
