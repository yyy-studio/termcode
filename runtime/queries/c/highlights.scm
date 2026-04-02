; Keywords
[
  "break"
  "case"
  "const"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extern"
  "for"
  "goto"
  "if"
  "inline"
  "register"
  "restrict"
  "sizeof"
  "static"
  "struct"
  "switch"
  "typedef"
  "union"
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
(sized_type_specifier) @type.builtin
(primitive_type) @type.builtin

; Functions
(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function)
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

; Strings
(string_literal) @string
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
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":"] @punctuation.delimiter

; Labels
(labeled_statement label: (statement_identifier) @label)
