; ══════════════════════════════════════════════════════════════════════════════
; Structs
; ══════════════════════════════════════════════════════════════════════════════

; Named-field struct: `struct Foo { bar: i32 }`
(struct_item
  (visibility_modifier)? @vis
  "struct" @kind
  name: (type_identifier) @name
  body: (field_declaration_list
    (field_declaration
      (visibility_modifier)? @field_vis
      name: (field_identifier) @field_name
      type: (_) @field_type)))

; Tuple struct: `struct Pair(i32, String);`
(struct_item
  (visibility_modifier)? @vis
  "struct" @kind
  name: (type_identifier) @name
  body: (ordered_field_declaration_list
    (visibility_modifier)? @field_vis
    (_) @field_type))

; Unit struct: `struct Marker;`
(struct_item
  (visibility_modifier)? @vis
  "struct" @kind
  name: (type_identifier) @name)

; ══════════════════════════════════════════════════════════════════════════════
; Enums
; ══════════════════════════════════════════════════════════════════════════════

; Enum with struct variants: `enum Msg { Quit { code: i32 } }`
(enum_item
  (visibility_modifier)? @vis
  "enum" @kind
  name: (type_identifier) @name
  body: (enum_variant_list
    (enum_variant
      name: (identifier) @field_name
      body: (field_declaration_list
        (field_declaration
          name: (field_identifier) @nested_field_name
          type: (_) @nested_field_type)))))

; Enum with tuple variants: `enum Expr { Lit(i64), Add(Box<Expr>, Box<Expr>) }`
(enum_item
  (visibility_modifier)? @vis
  "enum" @kind
  name: (type_identifier) @name
  body: (enum_variant_list
    (enum_variant
      name: (identifier) @field_name
      body: (ordered_field_declaration_list
        (_) @field_type))))

; Enum with unit variants: `enum Color { Red, Green, Blue }`
(enum_item
  (visibility_modifier)? @vis
  "enum" @kind
  name: (type_identifier) @name
  body: (enum_variant_list
    (enum_variant
      name: (identifier) @field_name)))

; ══════════════════════════════════════════════════════════════════════════════
; Type aliases
; ══════════════════════════════════════════════════════════════════════════════

; `type Result<T> = std::result::Result<T, MyError>;`
(type_item
  (visibility_modifier)? @vis
  "type" @kind
  name: (type_identifier) @name
  type: (_) @aliased_type)

; ══════════════════════════════════════════════════════════════════════════════
; Traits
; ══════════════════════════════════════════════════════════════════════════════

; `trait Iterator { ... }`
(trait_item
  (visibility_modifier)? @vis
  "trait" @kind
  name: (type_identifier) @name)

; ══════════════════════════════════════════════════════════════════════════════
; Unions
; ══════════════════════════════════════════════════════════════════════════════

; `union MyUnion { f1: u32, f2: f32 }`
(union_item
  (visibility_modifier)? @vis
  "union" @kind
  name: (type_identifier) @name
  body: (field_declaration_list
    (field_declaration
      (visibility_modifier)? @field_vis
      name: (field_identifier) @field_name
      type: (_) @field_type)))
