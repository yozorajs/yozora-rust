local binary, workspace = arg[1], arg[2]
local client ---@type vim.lsp.Client|nil
local checks = {} ---@type string[]
local sent_changes = {} ---@type table<string, integer>
local settled_requests = {} ---@type table<integer, integer>
local client_errors = {} ---@type string[]
local exited = {} ---@type table<integer, integer>

---@param label                         string
---@param predicate                     fun(): boolean
---@return nil
local function wait_for(label, predicate)
  assert(vim.wait(8000, predicate, 10), "timeout: " .. label)
end

---@param actual                        any
---@param expected                      any
---@param label                         string
---@return nil
local function equal(actual, expected, label)
  assert(
    vim.deep_equal(actual, expected),
    label .. ": expected " .. vim.inspect(expected) .. ", got " .. vim.inspect(actual)
  )
end

---@param label                         string
---@return nil
local function passed(label)
  checks[#checks + 1] = label
  print("PASS " .. label)
end

---@param buffer                        integer
---@param code                          string
---@return vim.Diagnostic[]
local function diagnostics_with_code(buffer, code)
  local result = {}
  for _, diagnostic in ipairs(vim.diagnostic.get(buffer)) do
    if diagnostic.code == code then
      result[#result + 1] = diagnostic
    end
  end
  return result
end

---@param name                          string
---@param lines                         ?string[]
---@return integer
local function open_buffer(name, lines)
  local path = vim.fs.joinpath(workspace, name)
  vim.fn.mkdir(vim.fs.dirname(path), "p")
  if lines then
    vim.fn.writefile(lines, path)
  end
  vim.cmd("silent edit " .. vim.fn.fnameescape(path))
  local buffer = vim.api.nvim_get_current_buf()
  wait_for("attach " .. name, function()
    local attached = vim.lsp.get_clients({ bufnr = buffer, name = "yozora" })[1]
    return attached ~= nil and attached.initialized
  end)
  local attached = vim.lsp.get_clients({ bufnr = buffer, name = "yozora" })[1]
  if client then
    equal(attached.id, client.id, "reuse workspace client")
  else
    client = attached
  end
  return buffer
end

---@param buffer                        integer
---@param row                           integer
---@param needle                        string
---@param offset                        ?integer
---@return lsp.TextDocumentPositionParams
local function at(buffer, row, needle, offset)
  vim.api.nvim_set_current_buf(buffer)
  local line = vim.api.nvim_buf_get_lines(buffer, row - 1, row, true)[1]
  local first = assert(line:find(needle, 1, true), needle .. " missing from line " .. row)
  vim.api.nvim_win_set_cursor(0, { row, first - 1 + (offset or 0) })
  return vim.lsp.util.make_position_params(0, assert(client).offset_encoding)
end

---@param method                        string
---@param params                        table<string, any>
---@param buffer                        integer
---@return any
local function request(method, params, buffer)
  local response, err = assert(client):request_sync(method, params, 8000, buffer)
  assert(response, method .. ": " .. tostring(err))
  assert(not response.err, method .. ": " .. vim.inspect(response.err))
  return response.result
end

---@param buffer                        integer
---@param needle                        string
---@param offset                        integer
---@param label                         string
---@param expected                      string
---@return nil
local function accept_completion(buffer, needle, offset, label, expected)
  local result = request("textDocument/completion", at(buffer, 1, needle, offset), buffer)
  local selected
  for _, item in ipairs(result.items) do
    if item.label == label then
      selected = item
      break
    end
  end
  assert(selected, "completion missing: " .. label .. ": " .. vim.inspect(result))
  vim.lsp.util.apply_text_edits({ selected.textEdit }, buffer, assert(client).offset_encoding)
  equal(vim.api.nvim_buf_get_lines(buffer, 0, 1, true)[1], expected, "native completion text edit")
end

---@return nil
local function run()
  assert(vim.fn.has("nvim-0.11") == 1, "Neovim 0.11 or later is required")
  assert(vim.fn.executable(binary) == 1, "language server executable is missing")
  vim.o.hidden = true
  vim.fn.mkdir(vim.fs.joinpath(workspace, ".git"), "p")
  workspace = assert(vim.uv.fs_realpath(workspace))
  vim.filetype.add({ extension = { yozora = "markdown" } })
  vim.api.nvim_create_autocmd("LspNotify", {
    callback = function(event)
      if event.data.method == "textDocument/didChange" then
        for _, change in ipairs(event.data.params.contentChanges) do
          if change.range then
            local uri = event.data.params.textDocument.uri
            sent_changes[uri] = (sent_changes[uri] or 0) + 1
          end
        end
      end
    end,
  })
  vim.api.nvim_create_autocmd("LspRequest", {
    callback = function(event)
      if event.data.request.type == "complete" then
        local id = event.data.request_id
        settled_requests[id] = (settled_requests[id] or 0) + 1
      end
    end,
  })
  vim.lsp.config("yozora", {
    cmd = { binary, "--stdio" },
    filetypes = { "markdown", "yozora" },
    root_markers = { ".git" },
    on_error = function(code, err)
      client_errors[#client_errors + 1] = tostring(code) .. ": " .. tostring(err)
    end,
    on_exit = function(code, _, client_id)
      exited[client_id] = code
    end,
  })
  vim.lsp.enable("yozora")

  local source_lines = {
    "# Source 😀", "", "😀 [shown][old]", "![alt][old]", "",
    "[old]: ../guide.md#intro", "[OLD]: ../guide.md#missing",
  }
  local source = open_buffer("docs/source.yozora", source_lines)
  equal(vim.bo[source].filetype, "markdown", "Yozora filetype detection")
  equal(assert(client).config.root_dir, workspace, "workspace root")
  equal(client.offset_encoding, "utf-16", "position encoding")
  passed("README configuration attaches Yozora files with a workspace root")

  wait_for("duplicate diagnostic", function()
    return #diagnostics_with_code(source, "duplicate-link-definition") == 1
  end)
  local diagnostic = diagnostics_with_code(source, "duplicate-link-definition")[1]
  equal(diagnostic.code, "duplicate-link-definition", "native diagnostic code")
  equal(diagnostic.lnum, 6, "native diagnostic line")
  local source_params = { textDocument = { uri = vim.uri_from_bufnr(source) } }
  local symbols = request("textDocument/documentSymbol", source_params, source)
  equal(symbols[1].name, "Source 😀", "Unicode outline")
  local folds = request("textDocument/foldingRange", source_params, source)
  assert(#folds > 0, "heading folding range missing")
  local hover = request("textDocument/hover", at(source, 3, "shown"), source)
  assert(hover.contents.value:find("guide", 1, true), "hover destination missing")
  passed("native diagnostics, Unicode outlines, folding and hover")

  local guide = open_buffer("guide.md", { "# Disk title" })
  vim.api.nvim_buf_set_lines(guide, 0, -1, true, { "# Other", "", "# Intro" })
  request("textDocument/documentSymbol", { textDocument = { uri = vim.uri_from_bufnr(guide) } }, guide)
  local completion = open_buffer("docs/completion.md", { "😀 [go](../gu)" })
  accept_completion(completion, "gu", 2, "guide.md", "😀 [go](../guide.md)")
  vim.api.nvim_buf_set_lines(completion, 0, -1, true, { "😀 [go](../guide.md#in)" })
  accept_completion(completion, "#in", 3, "intro", "😀 [go](../guide.md#intro)")
  at(completion, 1, "go")
  vim.lsp.buf.definition()
  wait_for("native definition jump", function()
    return vim.api.nvim_get_current_buf() == guide
  end)
  equal(vim.api.nvim_win_get_cursor(0), { 3, 2 }, "unsaved heading location")
  passed("path and anchor completion plus cross-directory navigation to unsaved text")

  equal(client.server_capabilities.workspaceSymbolProvider, true, "workspace symbols capability")
  local intro = request("workspace/symbol", { query = "INTRO" }, source)
  equal(#intro, 1, "workspace query uses unsaved headings")
  equal(intro[1].location.uri, vim.uri_from_bufnr(guide), "workspace target buffer URI")
  equal(intro[1].location.range.start.line, 2, "workspace target buffer line")
  equal(#request("workspace/symbol", { query = "Disk title" }, source), 0, "disk heading is overlaid")
  local closed_path = vim.fs.joinpath(workspace, "docs/closed 中文.md")
  vim.fn.writefile({ "# Workspace 😀 Before" }, closed_path)
  vim.api.nvim_set_current_buf(source)
  local workspace_items
  vim.lsp.buf.workspace_symbol("workspace", {
    on_list = function(options)
      workspace_items = options.items
    end,
  })
  wait_for("native workspace symbols", function()
    return workspace_items ~= nil
  end)
  equal(#workspace_items, 1, "native workspace list")
  equal(workspace_items[1].filename, closed_path, "native workspace filename")
  equal(workspace_items[1].lnum, 1, "native workspace line")
  equal(workspace_items[1].col, 3, "native workspace column")
  vim.fn.writefile({ "# Workspace 😀 Edited" }, closed_path)
  local edited = request("workspace/symbol", { query = "Edited" }, source)
  equal(#edited, 1, "closed file edit refresh")
  equal(edited[1].name, "Workspace 😀 Edited", "updated closed heading")
  local moved_path = vim.fs.joinpath(workspace, "docs/moved 中文.md")
  assert(vim.uv.fs_rename(closed_path, moved_path))
  local moved = request("workspace/symbol", { query = "Edited" }, source)
  equal(#moved, 1, "renamed file is indexed once")
  equal(vim.uri_to_fname(moved[1].location.uri), moved_path, "renamed file path")
  assert(vim.uv.fs_unlink(moved_path))
  equal(#request("workspace/symbol", { query = "Edited" }, source), 0, "deleted file leaves the index")
  passed("native workspace symbols search unopened files and refresh edits, renames and deletions")

  local topic = open_buffer("refactor/topic.md", {
    "# Native Intro", "", "[self](#native-intro)", "[peer](peer.md#peer)",
  })
  vim.fn.writefile({ "# Peer" }, vim.fs.joinpath(workspace, "refactor/peer.md"))
  vim.fn.writefile({ "😀 [topic](topic.md#native-intro)" }, vim.fs.joinpath(workspace, "refactor/referrer.md"))
  at(topic, 1, "Native Intro")
  vim.lsp.buf.rename("Native 标题 🦀")
  wait_for("native heading rename", function()
    return vim.api.nvim_buf_get_lines(topic, 0, 1, true)[1] == "# Native 标题 🦀"
  end)
  -- Flush this buffer's debounced didChange before querying its new anchor
  -- from another buffer, whose requests only flush that buffer's changes.
  local renamed_symbols = request("textDocument/documentSymbol", {
    textDocument = { uri = vim.uri_from_bufnr(topic) },
  }, topic)
  equal(renamed_symbols[1].name, "Native 标题 🦀", "renamed heading is synchronized")
  local referrer = open_buffer("refactor/referrer.md", nil)
  local fragment = vim.api.nvim_buf_get_lines(referrer, 0, 1, true)[1]:match("#(.-)%)")
  equal(vim.uri_decode(assert(fragment)), "native-标题-🦀", "closed referrer heading edit")
  local heading_target = request("textDocument/definition", at(referrer, 1, "topic"), referrer)
  assert(heading_target, "renamed heading destination is missing")
  equal(heading_target.uri, vim.uri_from_bufnr(topic), "renamed heading target URI")
  equal(heading_target.range.start.line, 0, "renamed heading target line")
  passed("native heading rename updates unsaved text and a previously unopened referrer")

  local old_topic_path = vim.api.nvim_buf_get_name(topic)
  local new_topic_path = vim.fs.joinpath(workspace, "refactor/moved/topic 中文.md")
  local rename_files = {
    files = { { oldUri = vim.uri_from_bufnr(topic), newUri = vim.uri_from_fname(new_topic_path) } },
  }
  assert(client.server_capabilities.workspace.fileOperations.willRename, "file operation capability missing")
  local file_edits = request("workspace/willRenameFiles", rename_files, topic)
  vim.lsp.util.apply_workspace_edit(file_edits, client.offset_encoding)
  vim.lsp.util.rename(old_topic_path, new_topic_path)
  assert(client:notify("workspace/didRenameFiles", rename_files))
  equal(vim.api.nvim_buf_get_name(topic), new_topic_path, "native file and buffer rename")
  local moved_symbols = request("textDocument/documentSymbol", {
    textDocument = { uri = vim.uri_from_bufnr(topic) },
  }, topic)
  equal(moved_symbols[1].name, "Native 标题 🦀", "renamed buffer keeps unsaved heading")
  equal(vim.api.nvim_buf_get_lines(topic, 3, 4, true)[1], "[peer](../peer.md#peer)", "rebased outgoing link")
  local moved_target = request("textDocument/definition", at(referrer, 1, "topic"), referrer)
  equal(moved_target.uri, vim.uri_from_bufnr(topic), "incoming link follows moved file")
  equal(moved_target.range.start.line, 0, "moved heading keeps its anchor")
  passed("native file operations apply incoming and outgoing edits before moving the file")

  local labels = open_buffer("docs/labels.md", { "😀 [shown][ol]", "", "[old]: /one", "[older]: /two" })
  accept_completion(labels, "ol", 2, "old", "😀 [shown][old]")
  local definition = request("textDocument/definition", at(labels, 1, "old"), labels)
  equal(definition.range.start.line, 2, "accepted label binding")
  passed("native UTF-16 completion edits preserve text before the label")

  at(source, 3, "old")
  vim.lsp.buf.rename("新名字")
  wait_for("native rename edit", function()
    return vim.api.nvim_buf_get_lines(source, 5, 6, true)[1] == "[新名字]: ../guide.md#intro"
  end)
  equal(vim.api.nvim_buf_get_lines(source, 2, 4, true), { "😀 [shown][新名字]", "![alt][新名字]" }, "native Unicode rename")
  local references = at(source, 3, "新名字")
  references.context = { includeDeclaration = true }
  equal(#request("textDocument/references", references, source), 3, "renamed references")
  vim.api.nvim_buf_set_lines(source, 6, 7, true, {})
  wait_for("diagnostic retraction", function()
    return #vim.diagnostic.get(source) == 0
  end)
  passed("native rename applies versioned edits and diagnostics follow incremental sync")

  local source_changes = sent_changes[vim.uri_from_bufnr(source)] or 0
  vim.api.nvim_buf_set_lines(guide, 2, 3, true, { "# Changed" })
  request("textDocument/documentSymbol", { textDocument = { uri = vim.uri_from_bufnr(guide) } }, guide)
  wait_for("anchor diagnostic after target edit", function()
    return #diagnostics_with_code(source, "missing-anchor") == 1
  end)
  vim.api.nvim_buf_set_lines(guide, 2, 3, true, { "# Intro" })
  request("textDocument/documentSymbol", { textDocument = { uri = vim.uri_from_bufnr(guide) } }, guide)
  wait_for("anchor diagnostic after target recovery", function()
    return #vim.diagnostic.get(source) == 0
  end)
  equal(sent_changes[vim.uri_from_bufnr(source)] or 0, source_changes, "referrer diagnostics need no source edit")

  local watched = open_buffer("links/watch.md", { "😀 [go](target.md#intro)" })
  wait_for("missing file diagnostic", function()
    return #diagnostics_with_code(watched, "missing-file") == 1
  end)
  local watched_target = vim.fs.joinpath(workspace, "links/target.md")
  for _, change in ipairs({
    { kind = 1, text = "# Other", code = "missing-anchor" },
    { kind = 2, text = "# Intro" },
    { kind = 3, code = "missing-file" },
  }) do
    if change.text then
      vim.fn.writefile({ change.text }, watched_target)
    else
      assert(vim.uv.fs_unlink(watched_target))
    end
    assert(client:notify("workspace/didChangeWatchedFiles", {
      changes = { { uri = vim.uri_from_fname(watched_target), type = change.kind } },
    }))
    wait_for("watched file diagnostics " .. change.kind, function()
      if change.code then
        return #diagnostics_with_code(watched, change.code) == 1
      end
      return #vim.diagnostic.get(watched) == 0
    end)
  end
  equal(sent_changes[vim.uri_from_bufnr(watched)] or 0, 0, "watched target diagnostics need no source edit")
  passed("native link diagnostics follow unsaved target edits and watched file creation, edits and deletion")

  local count = 2000
  local bulk_lines = {}
  for index = 1, count do
    bulk_lines[index] = "😀 [shown][old]"
  end
  bulk_lines[count + 1] = ""
  bulk_lines[count + 2] = "[old]: /url"
  local bulk = open_buffer("bulk.md", bulk_lines)
  at(bulk, 1, "old")
  vim.lsp.buf.rename("renamed")
  wait_for("bulk native rename", function()
    return vim.api.nvim_buf_get_lines(bulk, count + 1, count + 2, true)[1] == "[renamed]: /url"
  end)
  for _, line in ipairs(vim.api.nvim_buf_get_lines(bulk, 0, count, true)) do
    equal(line, "😀 [shown][renamed]", "bulk reference edit")
  end
  local bulk_references = at(bulk, 1, "renamed")
  bulk_references.context = { includeDeclaration = true }
  equal(#request("textDocument/references", bulk_references, bulk), count + 1, "bulk rename round trip")
  assert((sent_changes[vim.uri_from_bufnr(bulk)] or 0) > 0, "Neovim did not emit incremental edits")
  passed("2001 rename edits round-trip through Neovim change tracking")

  local depth = 4000
  local deep = open_buffer("deep.yozora", { string.rep("![", depth) .. "x" .. string.rep("](/url)", depth) })
  local deep_params = { textDocument = { uri = vim.uri_from_bufnr(deep) } }
  equal(#request("textDocument/documentSymbol", deep_params, deep), 0, "deep images")
  for index = 1, 20 do
    vim.api.nvim_buf_set_lines(deep, 0, -1, true, { "# 标题 😀 " .. index })
  end
  local latest = request("textDocument/documentSymbol", deep_params, deep)
  equal(latest[1].name, "标题 😀 20", "latest edit version")
  passed("deep documents and rapid Unicode edits keep the latest analysis")

  local callbacks = 0
  local ok, request_id = client:request("textDocument/documentSymbol", source_params, function(err)
    assert(err == nil or err.code == -32800, vim.inspect(err))
    callbacks = callbacks + 1
  end, source)
  assert(ok and request_id, "native request was rejected")
  assert(client:cancel_request(request_id), "native cancellation was rejected")
  -- Neovim consumes cancellation errors before invoking request handlers.
  wait_for("cancelled request response", function()
    return client.requests[request_id] == nil
  end)
  equal(request("textDocument/documentSymbol", source_params, source)[1].name, "Source 😀", "query after cancellation")
  equal(settled_requests[request_id], 1, "exactly one native request completion")
  assert(callbacks <= 1, "duplicate request callback")
  passed("native cancellation settles once and later queries succeed")

  vim.api.nvim_buf_delete(source, { force = true })
  wait_for("buffer detach", function()
    return client.attached_buffers[source] == nil
  end)
  local reopened = open_buffer("docs/source.yozora", nil)
  local prepared = request("textDocument/prepareRename", at(reopened, 3, "old"), reopened)
  equal(prepared.placeholder, "old", "reopened buffer uses disk text")
  wait_for("reopened diagnostic", function()
    return #diagnostics_with_code(reopened, "duplicate-link-definition") == 1
  end)
  passed("close and reopen discard unsaved server state")

  client:stop(false)
  wait_for("graceful shutdown", function()
    return exited[client.id] ~= nil
  end)
  equal(exited[client.id], 0, "language server exit code")
  equal(client_errors, {}, "native client errors")
  passed("native client shuts down the server cleanly")
  print(vim.json.encode({ neovim = vim.version(), checks = checks }))
end

local ok, err = xpcall(run, debug.traceback)
if not ok then
  for _, active in ipairs(vim.lsp.get_clients()) do
    active:stop(true)
  end
  io.stderr:write(err .. "\n")
  vim.cmd("cquit 1")
end
vim.cmd("qa!")
