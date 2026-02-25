; ── Regular function definitions ──────────────────────────────────────────────
(function_item
  (visibility_modifier)? @vis
  name: (identifier) @name
  parameters: (parameters) @params
  return_type: (_)? @ret)

; ── Trait / extern function signatures (no body) ─────────────────────────────
(function_signature_item
  (visibility_modifier)? @vis
  name: (identifier) @name
  parameters: (parameters) @params
  return_type: (_)? @ret)

; ── Associated functions inside `impl` blocks ────────────────────────────────
; Captures the impl target so callers can associate method → type.
(impl_item
  type: (_) @impl_target
  body: (declaration_list
    (function_item
      (visibility_modifier)? @vis
      name: (identifier) @name
      parameters: (parameters) @params
      return_type: (_)? @ret)))

; ── Associated function signatures inside `trait` blocks ─────────────────────
(trait_item
  name: (type_identifier) @impl_target
  body: (declaration_list
    (function_signature_item
      (visibility_modifier)? @vis
      name: (identifier) @name
      parameters: (parameters) @params
      return_type: (_)? @ret)))

; ── Extern "C" function declarations ─────────────────────────────────────────
(foreign_mod_item
  body: (declaration_list
    (function_signature_item
      (visibility_modifier)? @vis
      name: (identifier) @name
      parameters: (parameters) @params
      return_type: (_)? @ret)))
