-- Example plugin for termcode
-- Demonstrates command registration, hooks, and the editor API.

-- Command: wrap the current selection in double quotes
plugin.register_command("wrap-quotes", "Wrap selection in quotes", function()
    local sel = editor.get_selection()
    if not sel then
        editor.set_status("[example] No selection to wrap")
        return
    end

    local text = editor.buffer_get_range(sel.start.line, sel.start.col, sel["end"].line, sel["end"].col)
    if text then
        local quoted = '"' .. text .. '"'
        editor.buffer_replace_range(sel.start.line, sel.start.col, sel["end"].line, sel["end"].col, quoted)
        editor.set_status("[example] Wrapped selection in quotes")
    end
end)

-- Command: insert current date (YYYY-MM-DD) at cursor position
plugin.register_command("insert-date", "Insert current date at cursor", function()
    local date_str = os.date("%Y-%m-%d")
    editor.insert_text(date_str)
    editor.set_status("[example] Inserted date: " .. date_str)
end)

-- Hook: log when a file is saved
plugin.on("on_save", function(ctx)
    if ctx.filename then
        log.info("File saved: " .. ctx.filename)
    end
end)

-- Hook: log when the editor is ready
plugin.on("on_ready", function(ctx)
    log.info("Example plugin loaded and ready")
end)
