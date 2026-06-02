; Go — package (namespace), imports, top-level types and funcs.
(package_clause (package_identifier) @namespace)
(import_spec path: (interpreted_string_literal) @import)

(type_spec name: (type_identifier) @name type: (struct_type)) @definition.struct
(type_spec name: (type_identifier) @name type: (interface_type)) @definition.interface
(type_alias name: (type_identifier) @name) @definition.type
(function_declaration name: (identifier) @name) @definition.function
(method_declaration name: (field_identifier) @name) @definition.function
