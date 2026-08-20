-- Toggle line comments on the current line or selection.
--
-- The whole range is rewritten in one `buffer_replace_range` call rather than
-- line by line: each call is its own transaction, so a per-line loop would
-- cost one undo step per line to get back to where the file started.

-- Either `line` (a prefix) or `block` (a prefix/suffix pair wrapped around
-- each line individually, for languages that have no line comment at all).
local BY_EXTENSION = {
    -- //
    rs = { line = "//" }, c = { line = "//" }, h = { line = "//" },
    cpp = { line = "//" }, hpp = { line = "//" }, cc = { line = "//" },
    js = { line = "//" }, jsx = { line = "//" }, mjs = { line = "//" },
    ts = { line = "//" }, tsx = { line = "//" },
    go = { line = "//" }, java = { line = "//" }, kt = { line = "//" },
    swift = { line = "//" }, scala = { line = "//" }, cs = { line = "//" },
    php = { line = "//" }, dart = { line = "//" }, zig = { line = "//" },
    proto = { line = "//" }, scss = { line = "//" }, less = { line = "//" },

    -- #
    py = { line = "#" }, rb = { line = "#" }, sh = { line = "#" },
    bash = { line = "#" }, zsh = { line = "#" }, fish = { line = "#" },
    pl = { line = "#" }, r = { line = "#" }, jl = { line = "#" },
    yaml = { line = "#" }, yml = { line = "#" }, toml = { line = "#" },
    ini = { line = "#" }, cfg = { line = "#" }, conf = { line = "#" },
    tf = { line = "#" }, nix = { line = "#" }, ex = { line = "#" },
    exs = { line = "#" }, cmake = { line = "#" }, mk = { line = "#" },
    gitignore = { line = "#" }, dockerignore = { line = "#" },

    -- --
    lua = { line = "--" }, sql = { line = "--" }, hs = { line = "--" },
    elm = { line = "--" }, adb = { line = "--" }, ads = { line = "--" },

    -- ;
    lisp = { line = ";" }, clj = { line = ";" }, cljs = { line = ";" },
    el = { line = ";" }, scm = { line = ";" }, asm = { line = ";" },

    -- block-only
    html = { block = { "<!--", "-->" } },
    htm = { block = { "<!--", "-->" } },
    xml = { block = { "<!--", "-->" } },
    svg = { block = { "<!--", "-->" } },
    md = { block = { "<!--", "-->" } },
    css = { block = { "/*", "*/" } },
}

-- Files whose whole name, not extension, decides the syntax.
local BY_FILENAME = {
    ["Makefile"] = { line = "#" },
    ["makefile"] = { line = "#" },
    ["GNUmakefile"] = { line = "#" },
    ["Dockerfile"] = { line = "#" },
    ["Vagrantfile"] = { line = "#" },
    ["Gemfile"] = { line = "#" },
    ["Rakefile"] = { line = "#" },
    ["CMakeLists.txt"] = { line = "#" },
    [".gitignore"] = { line = "#" },
    [".env"] = { line = "#" },
}

local function syntax_for(filename)
    if not filename then
        return nil
    end
    local by_name = BY_FILENAME[filename]
    if by_name then
        return by_name
    end
    local ext = filename:match("%.([%w_]+)$")
    if not ext then
        return nil
    end
    return BY_EXTENSION[ext:lower()]
end

-- Splits a chunk of buffer text into lines, keeping each line's own CR so a
-- CRLF file is rewritten with the endings it came in with.
local function split_lines(text)
    local lines = {}
    local start = 1
    while true do
        local nl = text:find("\n", start, true)
        local raw = nl and text:sub(start, nl - 1) or text:sub(start)
        local cr = ""
        if raw:sub(-1) == "\r" then
            cr = "\r"
            raw = raw:sub(1, -2)
        end
        lines[#lines + 1] = { text = raw, cr = cr }
        if not nl then
            return lines
        end
        start = nl + 1
    end
end

local function is_blank(s)
    return s:match("^%s*$") ~= nil
end

local function indent_width(s)
    return #(s:match("^[ \t]*"))
end

local function is_commented(line, syntax)
    local body = line:match("^%s*(.-)%s*$")
    if syntax.line then
        return body:sub(1, #syntax.line) == syntax.line
    end
    local open, close = syntax.block[1], syntax.block[2]
    return body:sub(1, #open) == open and body:sub(-#close) == close
end

local function add_comment(line, syntax, column)
    local head, tail = line:sub(1, column), line:sub(column + 1)
    if syntax.line then
        return head .. syntax.line .. " " .. tail
    end
    return head .. syntax.block[1] .. " " .. tail .. " " .. syntax.block[2]
end

local function strip_comment(line, syntax)
    local indent = line:match("^[ \t]*")
    local body = line:sub(#indent + 1)
    if syntax.line then
        local rest = body:sub(#syntax.line + 1)
        -- Drop the single space `add_comment` inserted, and nothing more: the
        -- rest of the leading run belongs to the commented-out code's indent.
        if rest:sub(1, 1) == " " then
            rest = rest:sub(2)
        end
        return indent .. rest
    end
    -- `is_commented` matched the closer against a trimmed line, so the trailing
    -- whitespace has to come off here too -- and go back on afterwards, since
    -- toggling a comment is not licence to reformat the line.
    local open, close = syntax.block[1], syntax.block[2]
    local trail = body:match("[ \t]*$")
    local trimmed = body:sub(1, #body - #trail)
    local rest = trimmed:sub(#open + 1, #trimmed - #close)
    return indent .. (rest:match("^ ?(.-) ?$") or rest) .. trail
end

plugin.register_command("toggle", "Toggle comment on line or selection", function()
    local filename = editor.get_filename()
    local syntax = syntax_for(filename)
    if not syntax then
        editor.set_status("[comment] No comment syntax known for " .. (filename or "this buffer"))
        return
    end

    local sel = editor.get_selection()
    local cursor = editor.get_cursor()
    if not cursor then
        return
    end

    local first, last = cursor.line, cursor.line
    if sel then
        first, last = sel.start.line, sel["end"].line
        -- A selection ending in column 1 stops *before* that line; taking it
        -- would comment a line the user never highlighted a character of.
        if last > first and sel["end"].col == 1 then
            last = last - 1
        end
    end

    -- Column 1 to past the end of the last line: `pos_to_byte` clamps a column
    -- to the line's end, so a large number means "to end of line" exactly.
    local END_COL = 1073741824
    local text = editor.buffer_get_range(first, 1, last, END_COL)
    if not text then
        return
    end

    local lines = split_lines(text)

    -- Uncomment only when every line that has content is already commented;
    -- one bare line among them means the intent was to comment the block.
    local uncomment, any_content = true, false
    for _, entry in ipairs(lines) do
        if not is_blank(entry.text) then
            any_content = true
            if not is_commented(entry.text, syntax) then
                uncomment = false
                break
            end
        end
    end
    if not any_content then
        editor.set_status("[comment] Nothing to comment")
        return
    end

    -- Comment markers line up on the shallowest line, so relative indentation
    -- inside the block survives the round trip.
    local column = math.huge
    if not uncomment then
        for _, entry in ipairs(lines) do
            if not is_blank(entry.text) then
                column = math.min(column, indent_width(entry.text))
            end
        end
    end

    local cursor_shift = 0
    local out = {}
    for i, entry in ipairs(lines) do
        local rewritten = entry.text
        if not is_blank(entry.text) then
            if uncomment then
                rewritten = strip_comment(entry.text, syntax)
            else
                rewritten = add_comment(entry.text, syntax, column)
            end
        end
        if first + i - 1 == cursor.line then
            cursor_shift = #rewritten - #entry.text
        end
        out[#out + 1] = rewritten .. entry.cr
    end

    editor.buffer_replace_range(first, 1, last, END_COL, table.concat(out, "\n"))

    -- `buffer_replace_range` leaves the caret at the end of what it wrote.
    -- Put it back where it was typing, moved by however much its own line grew.
    local col = math.max(1, cursor.col + cursor_shift)
    if sel then
        editor.set_selection(first, 1, last, END_COL)
    else
        editor.set_cursor(cursor.line, col)
    end

    local count = last - first + 1
    editor.set_status(string.format(
        "[comment] %s %d line%s",
        uncomment and "Uncommented" or "Commented",
        count,
        count == 1 and "" or "s"
    ))
end)
