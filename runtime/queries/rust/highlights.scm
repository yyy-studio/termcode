; Keywords
[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "default"
  "dyn"
  "else"
  "enum"
  "extern"
  "for"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "match"
  "mod"
  "move"
  "pub"
  "ref"
  "return"
  "static"
  "struct"
  "trait"
  "type"
  "union"
  "unsafe"
  "use"
  "where"
  "while"
  "yield"
] @keyword

"fn" @keyword.function
"return" @keyword.control.return

[
  "use"
  "mod"
] @keyword.control.import

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(self) @variable.builtin

; Functions
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))
(generic_function function: (identifier) @function)
(generic_function function: (field_expression field: (field_identifier) @function))

; Macros
(macro_invocation macro: (identifier) @function.macro)
(macro_definition name: (identifier) @function.macro)
(attribute_item) @attribute

; Variables
(identifier) @variable
(field_identifier) @variable
(shorthand_field_initializer (identifier) @variable)

; Parameters
(parameter pattern: (identifier) @variable.parameter)
(closure_parameters (identifier) @variable.parameter)

; Constants
(const_item name: (identifier) @constant)
(boolean_literal) @constant
(integer_literal) @constant.numeric
(float_literal) @constant.numeric

; Strings
(string_literal) @string
(raw_string_literal) @string
(char_literal) @constant.character
(escape_sequence) @constant.character.escape

; Comments
(line_comment) @comment
(block_comment) @comment

; Operators
[
  "!"
  "!="
  "%"
  "%="
  "&"
  "&&"
  "&="
  "*"
  "*="
  "+"
  "+="
  "-"
  "-="
  ".."
  "..="
  "/"
  "/="
  "<"
  "<<"
  "<<="
  "<="
  "="
  "=="
  ">"
  ">="
  ">>"
  ">>="
  "?"
  "^"
  "^="
  "|"
  "|="
  "||"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ":" "::" ";" "->" "=>" "#"] @punctuation.delimiter

; Namespaces
(scoped_identifier path: (identifier) @namespace)
(use_declaration argument: (scoped_identifier path: (identifier) @namespace))

; Constructors
(struct_expression name: (type_identifier) @constructor)
(enum_variant name: (identifier) @constructor)

; Labels
(label (identifier) @label)
