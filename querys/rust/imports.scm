; Just capture the top-level use_declaration node.
; The parser walks the tree programmatically to handle
; arbitrary nesting depth (e.g. `use std::{collections::{HashMap, HashSet}}`).
(use_declaration) @root
