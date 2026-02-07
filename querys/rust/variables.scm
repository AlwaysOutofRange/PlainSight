(let_declaration pattern: (identifier) @name type: (_)? @type value: (_)? @value (mutable_specifier)? @mut)
(const_item "const" @const_keyword name: (identifier) @name type: (_)? @type value: (_)? @value)
(static_item "static" @static_keyword (mutable_specifier)? @mut name: (identifier) @name type: (_)? @type value: (_)? @value)
