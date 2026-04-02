; Headings
(atx_heading (atx_h1_marker) @keyword)
(atx_heading (atx_h2_marker) @keyword)
(atx_heading (atx_h3_marker) @keyword)
(atx_heading (atx_h4_marker) @keyword)
(atx_heading (atx_h5_marker) @keyword)
(atx_heading (atx_h6_marker) @keyword)

; Code
(fenced_code_block) @string
(indented_code_block) @string
(fenced_code_block_delimiter) @punctuation.delimiter

; Lists
(list_marker_minus) @punctuation.delimiter
(list_marker_plus) @punctuation.delimiter
(list_marker_star) @punctuation.delimiter
(list_marker_dot) @punctuation.delimiter

; Block quotes
(block_quote_marker) @punctuation.delimiter

; Thematic breaks
(thematic_break) @punctuation.delimiter

; Links
(link_destination) @constant
(link_label) @string.special
