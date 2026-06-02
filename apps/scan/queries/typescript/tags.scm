; TypeScript / TSX — imports and declarations. Same grammar family, one query set.
(import_statement source: (string (string_fragment) @import))

(class_declaration name: (_) @name) @definition.class
(abstract_class_declaration name: (_) @name) @definition.class
(interface_declaration name: (_) @name) @definition.interface
(enum_declaration name: (_) @name) @definition.enum
(type_alias_declaration name: (_) @name) @definition.type
(function_declaration name: (_) @name) @definition.function

; Exported top-level consts (e.g. `export const userTable = pgTable(...)`).
; This is the syntax hook a convention like Drizzle/GraphQL plugs into — the
; engine never knows the framework; it just sees a recurring `export const`.
(export_statement
  declaration: (lexical_declaration
    (variable_declarator name: (identifier) @name) @definition.const))
