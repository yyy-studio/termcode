; Variables
(identifier) @variable
(parameter name: (identifier) @variable.parameter)

; Namespaces
; A dotted name is a left-nested `qualified_name`, and capturing the outer node
; is not enough -- the `identifier` inside it is a nested capture and wins over
; it. Each level is therefore spelled out; four segments is as deep as this
; goes, and a longer namespace falls back to `variable`.
(namespace_declaration name: (identifier) @namespace)
(namespace_declaration name: (qualified_name (identifier) @namespace))
(namespace_declaration name: (qualified_name (qualified_name (identifier) @namespace)))
(namespace_declaration
  name: (qualified_name (qualified_name (qualified_name (identifier) @namespace))))
(file_scoped_namespace_declaration name: (identifier) @namespace)
(file_scoped_namespace_declaration name: (qualified_name (identifier) @namespace))
(file_scoped_namespace_declaration
  name: (qualified_name (qualified_name (identifier) @namespace)))
(file_scoped_namespace_declaration
  name: (qualified_name (qualified_name (qualified_name (identifier) @namespace))))

; Types
(_ type: (identifier) @type)
(method_declaration returns: (identifier) @type)
(base_list (identifier) @type)
(generic_name (identifier) @type)
(type_argument_list (identifier) @type)
(type_parameter (identifier) @type)
(type_parameter_constraints_clause (identifier) @type)
(as_expression right: (identifier) @type)
(is_expression right: (identifier) @type)
(class_declaration name: (identifier) @type)
(interface_declaration name: (identifier) @type)
(struct_declaration name: (identifier) @type)
(record_declaration name: (identifier) @type)
(enum_declaration name: (identifier) @type)
(delegate_declaration name: (identifier) @type)
(predefined_type) @type.builtin

; Constants
(enum_member_declaration name: (identifier) @constant)

; Functions
(method_declaration name: (identifier) @function)
(local_function_statement name: (identifier) @function)
(invocation_expression function: (identifier) @function)
(invocation_expression function: (member_access_expression name: (identifier) @function))
(constructor_declaration name: (identifier) @constructor)
(destructor_declaration name: (identifier) @constructor)

; Attributes
(attribute name: (identifier) @attribute)

; Labels
(labeled_statement (identifier) @label)

; Keywords
[
  (modifier)
  (implicit_type)
  "add"
  "alias"
  "class"
  "delegate"
  "enum"
  "event"
  "explicit"
  "extern"
  "get"
  "global"
  "implicit"
  "init"
  "interface"
  "let"
  "namespace"
  "notnull"
  "operator"
  "params"
  "record"
  "ref"
  "remove"
  "select"
  "set"
  "static"
  "struct"
  "where"
] @keyword

[
  "await"
  "break"
  "case"
  "catch"
  "continue"
  "default"
  "do"
  "else"
  "finally"
  "for"
  "foreach"
  "from"
  "goto"
  "if"
  "lock"
  "switch"
  "throw"
  "try"
  "when"
  "while"
] @keyword.control

[
  "return"
  "yield"
] @keyword.control.return

"using" @keyword.control.import

[
  "as"
  "checked"
  "in"
  "is"
  "new"
  "out"
  "sizeof"
  "stackalloc"
  "typeof"
  "unchecked"
  "with"
] @keyword.operator

[
  "this"
  "base"
] @variable.builtin

; Preprocessor
[
  "#if"
  "#else"
  "#elif"
  "#endif"
  "#region"
  "#endregion"
  "#define"
  "#undef"
  "#nullable"
  "#pragma"
  "#line"
  "#error"
  "#warning"
] @keyword.control

; Literals
[
  (integer_literal)
  (real_literal)
] @constant.numeric

(character_literal) @constant.character

[
  (string_literal)
  (raw_string_literal)
  (verbatim_string_literal)
  (interpolated_string_expression)
  (interpolation_start)
  (interpolation_quote)
] @string

(escape_sequence) @constant.character.escape

[
  (boolean_literal)
  (null_literal)
] @constant

; Comments
(comment) @comment

; Operators
[
  "--"
  "-"
  "-="
  "&"
  "&="
  "&&"
  "+"
  "++"
  "+="
  "<"
  "<="
  "<<"
  "<<="
  "="
  "=="
  "!"
  "!="
  "=>"
  ">"
  ">="
  ">>"
  ">>="
  ">>>"
  ">>>="
  "|"
  "|="
  "||"
  "?"
  "??"
  "??="
  "^"
  "^="
  "~"
  "*"
  "*="
  "/"
  "/="
  "%"
  "%="
  ".."
] @operator

; Punctuation
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  (interpolation_brace)
] @punctuation.bracket

[
  ";"
  "."
  ","
  ":"
] @punctuation.delimiter
