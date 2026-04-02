; Keywords
[
  "break"
  "case"
  "catch"
  "class"
  "co_await"
  "co_return"
  "co_yield"
  "const"
  "constexpr"
  "continue"
  "decltype"
  "default"
  "delete"
  "do"
  "else"
  "enum"
  "explicit"
  "extern"
  "final"
  "for"
  "friend"
  "goto"
  "if"
  "inline"
  "mutable"
  "namespace"
  "new"
  "noexcept"
  "operator"
  "override"
  "private"
  "protected"
  "public"
  "register"
  "sizeof"
  "static"
  "static_assert"
  "struct"
  "switch"
  "template"
  "throw"
  "try"
  "typedef"
  "typename"
  "union"
  "using"
  "virtual"
  "volatile"
  "while"
] @keyword

"return" @keyword.control.return

; Preprocessor
(preproc_include) @keyword.control.import
(preproc_def) @keyword
(preproc_ifdef) @keyword
(preproc_if) @keyword
(preproc_else) @keyword

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(auto) @type.builtin
(sized_type_specifier) @type.builtin

; Namespaces
(namespace_identifier) @namespace

; Builtins
((identifier) @variable.builtin
  (#any-of? @variable.builtin "this"))

; Functions
(function_declarator declarator: (identifier) @function)
(function_declarator declarator: (qualified_identifier name: (identifier) @function))
(call_expression function: (identifier) @function)
(call_expression function: (qualified_identifier name: (identifier) @function))
(call_expression function: (field_expression field: (field_identifier) @function))
(preproc_function_def name: (identifier) @function.macro)

; Variables
(identifier) @variable
(field_identifier) @variable

; Parameters
(parameter_declaration declarator: (identifier) @variable.parameter)

; Constants
(number_literal) @constant.numeric
(char_literal) @constant.character
(true) @constant
(false) @constant
(null) @constant
; nullptr is "null" in this grammar version

; Strings
(string_literal) @string
(raw_string_literal) @string
(system_lib_string) @string
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
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  "="
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
  "++"
  "--"
  "->"
  "."
  "::"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":"] @punctuation.delimiter

; Labels
(labeled_statement label: (statement_identifier) @label)

; Attributes
(attribute_declaration) @attribute
