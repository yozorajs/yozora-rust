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
    return #vim.diagnostic.get(source) == 1
  end)
  local diagnostic = vim.diagnostic.get(source)[1]
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
    return #vim.diagnostic.get(reopened) == 1
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
