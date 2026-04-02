; Keys
(bare_key) @variable
(dotted_key) @variable
(quoted_key) @variable

; Tables
(table (bare_key) @type)
(table (dotted_key) @type)
(table (quoted_key) @type)
(table_array_element (bare_key) @type)
(table_array_element (dotted_key) @type)
(table_array_element (quoted_key) @type)

; Values
(string) @string
(integer) @constant.numeric
(float) @constant.numeric
(boolean) @constant
(local_date) @constant
(local_time) @constant
(local_date_time) @constant
(offset_date_time) @constant

; Comments
(comment) @comment

; Punctuation
["[" "]" "[[" "]]"] @punctuation.bracket
["{" "}"] @punctuation.bracket
["." "," "="] @punctuation.delimiter
