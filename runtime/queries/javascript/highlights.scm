; Keywords
[
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "default"
  "delete"
  "do"
  "else"
  "extends"
  "finally"
  "for"
  "if"
  "in"
  "instanceof"
  "let"
  "new"
  "of"
  "static"
  "switch"
  "throw"
  "try"
  "typeof"
  "var"
  "void"
  "while"
  "with"
  "yield"
] @keyword

"function" @keyword.function
"return" @keyword.control.return

[
  "import"
  "export"
  "from"
] @keyword.control.import

"async" @keyword
"await" @keyword

; Types

; Builtins
(super) @variable.builtin

((identifier) @variable.builtin
  (#any-of? @variable.builtin "this" "arguments"))

((identifier) @type.builtin
  (#any-of? @type.builtin "Array" "Object" "String" "Number" "Boolean" "Symbol" "Map" "Set" "WeakMap" "WeakSet" "Promise" "RegExp" "Error" "Date" "Math" "JSON"))

((identifier) @function.builtin
  (#any-of? @function.builtin "console" "parseInt" "parseFloat" "isNaN" "isFinite" "setTimeout" "setInterval" "clearTimeout" "clearInterval" "require" "fetch"))

; Functions
(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (member_expression property: (property_identifier) @function))
(arrow_function)

; Variables
(identifier) @variable
(property_identifier) @variable
(shorthand_property_identifier) @variable

; Parameters
(formal_parameters (identifier) @variable.parameter)

; Constants
(true) @constant
(false) @constant
(null) @constant
(undefined) @constant
(number) @constant.numeric

; Strings
(string) @string
(template_string) @string
(template_substitution) @string.special
(regex) @string.special

; Comments
(comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "**"
  "=="
  "==="
  "!="
  "!=="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "~"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  ">>>"
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "**="
  "<<="
  ">>="
  ">>>="
  "&="
  "|="
  "^="
  "??"
  "=>"
  "..."
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";"] @punctuation.delimiter
