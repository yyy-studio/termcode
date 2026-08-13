; Comments
(comment) @comment

; Selectors
(tag_name) @tag
(class_name) @type
(id_name) @type
(nesting_selector) @tag
(universal_selector) @tag
(attribute_name) @attribute
(pseudo_class_selector (class_name) @function.builtin)
(pseudo_element_selector (tag_name) @function.builtin)

; Properties
(property_name) @variable
(feature_name) @variable

; At-rules
(at_keyword) @keyword
(keyframes_name) @constructor
(namespace_name) @namespace
(to) @keyword
(from) @keyword
(important) @keyword

; Functions
(function_name) @function

; Values
(string_value) @string
(color_value) @constant
(integer_value) @constant.numeric
(float_value) @constant.numeric
(unit) @type.builtin
(plain_value) @constant
(escape_sequence) @constant.character.escape

; Punctuation
["{" "}" "(" ")" "[" "]"] @punctuation.bracket
["," ":" ";" "::"] @punctuation.delimiter
["~" ">" "+" "-" "*" "/" "="] @operator
