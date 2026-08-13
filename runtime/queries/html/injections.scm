; <script>...</script> body is JavaScript
((script_element
  (raw_text) @injection.content)
 (#set! injection.language "javascript"))

; <style>...</style> body is CSS
((style_element
  (raw_text) @injection.content)
 (#set! injection.language "css"))
