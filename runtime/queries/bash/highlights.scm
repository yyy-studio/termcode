; Keywords
[
  "if"
  "then"
  "else"
  "elif"
  "fi"
  "case"
  "esac"
  "for"
  "while"
  "until"
  "do"
  "done"
  "in"
  "function"
  "select"
] @keyword

; Bash has no 'return' keyword node - it's a command

; Import is handled via command_name matching

; Builtins
((command_name (word) @function.builtin)
  (#any-of? @function.builtin "echo" "cd" "exit" "export" "unset" "set" "eval" "exec" "read" "shift" "test" "trap" "wait" "printf" "local" "declare" "typeset" "readonly" "let" "getopts" "pushd" "popd" "dirs"))

; Functions
(function_definition name: (word) @function)
(command_name (word) @function)

; Variables
(variable_name) @variable
(special_variable_name) @variable.builtin
(word) @variable

; Strings
(string) @string
(raw_string) @string
(heredoc_body) @string
(heredoc_start) @label

; Comments
(comment) @comment

; Constants
(number) @constant.numeric

; Operators
[
  "="
  "=="
  "!="
  "<"
  ">"
  ">="
  "<="
  "&&"
  "||"
  "!"
  "|"
  "+="
] @operator

; Punctuation
["(" ")" "[" "]" "[[" "]]" "{" "}"] @punctuation.bracket
[";" ";;" "&" "|"] @punctuation.delimiter

; Expansion
(expansion) @variable
(command_substitution) @string.special
