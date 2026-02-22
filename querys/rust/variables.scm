; ══════════════════════════════════════════════════════════════════════════════
; let bindings
; ══════════════════════════════════════════════════════════════════════════════

; `let x: i32 = 5;`  /  `let mut y = vec![];`
(let_declaration
  (mutable_specifier)? @mut
  pattern: (identifier) @name
  type: (_)? @type
  value: (_)? @value)

; let ref x — no mutability
(let_declaration
  pattern: (ref_pattern
    (identifier) @name)
  type: (_)? @type
  value: (_)? @value)

; let ref mut x — mut_pattern nests inside ref_pattern
(let_declaration
  pattern: (ref_pattern
    (mut_pattern
      (identifier) @name)) @mut
  type: (_)? @type
  value: (_)? @value)

; Tuple destructuring: capture each binding
; `let (a, b) = pair;`
(let_declaration
  pattern: (tuple_pattern
    (identifier) @name)
  value: (_)? @value)

; Struct destructuring with shorthand: `let Foo { x, y } = foo;`
(let_declaration
  pattern: (struct_pattern
    (field_pattern
      (shorthand_field_identifier) @name))
  value: (_)? @value)

; Struct destructuring with rename: `let Foo { x: local_x } = foo;`
(let_declaration
  pattern: (struct_pattern
    (field_pattern
      name: (field_identifier) @_original
      pattern: (identifier) @name))
  value: (_)? @value)

; ══════════════════════════════════════════════════════════════════════════════
; const / static items
; ══════════════════════════════════════════════════════════════════════════════

; `const MAX: usize = 100;`
(const_item
  (visibility_modifier)? @vis
  "const" @const_keyword
  name: (identifier) @name
  type: (_) @type
  value: (_)? @value)

; `static COUNTER: AtomicUsize = AtomicUsize::new(0);`
; `static mut BUFFER: [u8; 1024] = [0; 1024];`
(static_item
  (visibility_modifier)? @vis
  "static" @static_keyword
  (mutable_specifier)? @mut
  name: (identifier) @name
  type: (_) @type
  value: (_)? @value)

; ══════════════════════════════════════════════════════════════════════════════
; for-loop bindings (iterator variable)
; ══════════════════════════════════════════════════════════════════════════════

(for_expression
  pattern: (identifier) @name
  value: (_) @value)

; ══════════════════════════════════════════════════════════════════════════════
; if-let / while-let bindings
; ══════════════════════════════════════════════════════════════════════════════

(let_condition
  pattern: (_
    (identifier) @name)
  value: (_) @value)
