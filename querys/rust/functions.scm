; Function definitions with body
(function_item
  name: (identifier) @name
  parameters: (parameters) @params
  return_type: (_)? @ret)

; Trait/extern function signatures (no body)
(function_signature_item
  name: (identifier) @name
  parameters: (parameters) @params
  return_type: (_)? @ret)
