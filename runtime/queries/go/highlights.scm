; Keywords
[
  "break"
  "case"
  "chan"
  "const"
  "continue"
  "default"
  "defer"
  "else"
  "fallthrough"
  "for"
  "go"
  "goto"
  "if"
  "interface"
  "map"
  "range"
  "select"
  "struct"
  "switch"
  "type"
  "var"
] @keyword

"func" @keyword.function
"return" @keyword.control.return

[
  "import"
  "package"
] @keyword.control.import

; Types
(type_identifier) @type

((identifier) @type.builtin
  (#any-of? @type.builtin "bool" "byte" "int" "int8" "int16" "int32" "int64" "uint" "uint8" "uint16" "uint32" "uint64" "uintptr" "float32" "float64" "complex64" "complex128" "string" "error" "rune" "any"))

; Builtins
((identifier) @function.builtin
  (#any-of? @function.builtin "append" "cap" "close" "copy" "delete" "len" "make" "new" "panic" "print" "println" "recover" "complex" "imag" "real"))

; Functions
(function_declaration name: (identifier) @function)
(method_declaration name: (field_identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (selector_expression field: (field_identifier) @function))

; Variables
(identifier) @variable
(field_identifier) @variable

; Parameters
(parameter_declaration name: (identifier) @variable.parameter)
(variadic_parameter_declaration name: (identifier) @variable.parameter)

; Constants
(true) @constant
(false) @constant
(nil) @constant
(int_literal) @constant.numeric
(float_literal) @constant.numeric
(imaginary_literal) @constant.numeric
(rune_literal) @constant.character

; Strings
(raw_string_literal) @string
(interpreted_string_literal) @string
(escape_sequence) @constant.character.escape

; Comments
(comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "&^"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "="
  ":="
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  "&^="
  "<-"
  "++"
  "--"
  "..."
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";" ":"] @punctuation.delimiter

; Labels
(label_name) @label

; Namespaces
(package_identifier) @namespace
