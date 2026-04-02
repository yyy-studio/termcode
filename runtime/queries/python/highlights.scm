; Keywords
[
  "and"
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "del"
  "elif"
  "else"
  "except"
  "exec"
  "finally"
  "for"
  "global"
  "if"
  "in"
  "is"
  "lambda"
  "nonlocal"
  "not"
  "or"
  "pass"
  "print"
  "raise"
  "try"
  "while"
  "with"
  "yield"
] @keyword

"def" @keyword.function
"return" @keyword.control.return

[
  "from"
  "import"
] @keyword.control.import

; Types
(type (identifier) @type)
(class_definition name: (identifier) @type)

; Builtins
((identifier) @variable.builtin
  (#any-of? @variable.builtin "self" "cls"))

((identifier) @type.builtin
  (#any-of? @type.builtin "int" "float" "str" "bool" "list" "dict" "tuple" "set" "bytes" "None" "type" "object"))

((identifier) @function.builtin
  (#any-of? @function.builtin "print" "len" "range" "enumerate" "zip" "map" "filter" "isinstance" "issubclass" "hasattr" "getattr" "setattr" "super" "property" "staticmethod" "classmethod" "abs" "all" "any" "bin" "chr" "dir" "divmod" "eval" "exec" "format" "globals" "hash" "hex" "id" "input" "iter" "locals" "max" "min" "next" "oct" "open" "ord" "pow" "repr" "reversed" "round" "sorted" "sum" "vars"))

; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function)
(call function: (attribute attribute: (identifier) @function))
(decorator) @attribute

; Variables
(identifier) @variable
(attribute attribute: (identifier) @variable)

; Parameters
(parameters (identifier) @variable.parameter)
(default_parameter name: (identifier) @variable.parameter)
(typed_parameter (identifier) @variable.parameter)
(typed_default_parameter name: (identifier) @variable.parameter)

; Constants
(true) @constant
(false) @constant
(none) @constant
(integer) @constant.numeric
(float) @constant.numeric

; Strings
(string) @string
(interpolation) @string.special

; Comments
(comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "**"
  "/"
  "//"
  "%"
  "|"
  "&"
  "^"
  "~"
  "<<"
  ">>"
  "<"
  "<="
  "=="
  "!="
  ">="
  ">"
  "="
  "+="
  "-="
  "*="
  "/="
  "//="
  "%="
  "**="
  ">>="
  "<<="
  "&="
  "^="
  "|="
  ":="
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ":" ";"] @punctuation.delimiter
