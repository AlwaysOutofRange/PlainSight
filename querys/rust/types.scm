(struct_item
  name: (type_identifier) @name
  body: (field_declaration_list
    (field_declaration
      (visibility_modifier)? @field_vis
      name: (field_identifier) @field_name
      type: (_) @field_type)))

; Tuple structs (no field names)
(struct_item
  name: (type_identifier) @name
  body: (ordered_field_declaration_list
    (_) @field_type))

; Unit structs
(struct_item
  name: (type_identifier) @name
  ";")

; Enums with struct-style variants
(enum_item
  name: (type_identifier) @name
  body: (enum_variant_list
    (enum_variant
      name: (identifier) @variant_name
      body: (field_declaration_list
        (field_declaration
          (visibility_modifier)? @field_vis
          name: (field_identifier) @field_name
          type: (_) @field_type)))))
