--- Filter for `ldoc --filter dx_ldoc_json.write`.
-- Writes the documented modules of one LDoc run as JSON for the DocIr extractor.
-- @module dx_ldoc_json

local M = {}

local ESCAPES = {
  ['"'] = '\\"',
  ["\\"] = "\\\\",
  ["\b"] = "\\b",
  ["\f"] = "\\f",
  ["\n"] = "\\n",
  ["\r"] = "\\r",
  ["\t"] = "\\t",
}

local function quote(text)
  local escaped = tostring(text):gsub('[%c"\\]', function(c)
    return ESCAPES[c] or string.format("\\u%04x", c:byte())
  end)
  return '"' .. escaped .. '"'
end

local function encode(value)
  local kind = type(value)
  if kind == "string" then
    return quote(value)
  elseif kind == "number" then
    return string.format("%d", value)
  elseif kind == "boolean" then
    return value and "true" or "false"
  elseif kind == "table" then
    if #value > 0 or next(value) == nil then
      local parts = {}
      for i, item in ipairs(value) do
        parts[i] = encode(item)
      end
      return "[" .. table.concat(parts, ",") .. "]"
    end
    local keys = {}
    for key in pairs(value) do
      keys[#keys + 1] = key
    end
    table.sort(keys)
    local parts = {}
    for _, key in ipairs(keys) do
      parts[#parts + 1] = quote(key) .. ":" .. encode(value[key])
    end
    return "{" .. table.concat(parts, ",") .. "}"
  end
  error("cannot encode a " .. kind)
end

--- Reads the srcs manifest: execpath TAB repo-relative path TAB 1 when generated.
local function read_sources(path)
  local sources = {}
  for line in io.lines(path) do
    local exec, rel, generated = line:match("^([^\t]+)\t([^\t]+)\t([01])$")
    if not exec then
      error("bad srcs manifest line: " .. line)
    end
    sources[exec] = { file = rel, generated = generated == "1" }
  end
  return sources
end

local function text(value)
  local trimmed = (value or ""):gsub("^%s+", ""):gsub("%s+$", "")
  return trimmed
end

local function modifier_type(modifiers, key)
  local entry = modifiers and modifiers[key]
  return entry and entry.type or ""
end

local function item_json(item)
  local params = {}
  for index, name in ipairs(item.params or {}) do
    params[#params + 1] = {
      name = name,
      type = modifier_type(item.modifiers and item.modifiers.param, index),
      doc = text(item.params.map and item.params.map[name]),
    }
  end
  local returns = {}
  for index, doc in ipairs(item.ret or {}) do
    returns[#returns + 1] = {
      type = modifier_type(item.modifiers and item.modifiers["return"], index),
      doc = text(doc),
    }
  end
  return {
    args = item.args or "",
    description = text(item.description),
    kind = item.type,
    line = item.lineno or 1,
    name = item.name,
    params = params,
    returns = returns,
    summary = text(item.summary),
    usage = item.usage or {},
  }
end

--- Receives the module list of LDoc and writes the JSON named by DX_LDOC_OUT.
function M.write(modules)
  local sources = read_sources(assert(os.getenv("DX_LDOC_SRCS"), "DX_LDOC_SRCS is not set"))
  local out = assert(os.getenv("DX_LDOC_OUT"), "DX_LDOC_OUT is not set")
  local cwd = require("lfs").currentdir()
  local result = {}
  for module in modules:iter() do
    local file = module.file
    if file:sub(1, #cwd + 1) == cwd .. "/" then
      file = file:sub(#cwd + 2)
    end
    local source = sources[file]
    if not source then
      error("LDoc read " .. file .. ", which is not a listed source")
    end
    local items = {}
    for item in module.items:iter() do
      if item.name then
        items[#items + 1] = item_json(item)
      end
    end
    result[#result + 1] = {
      description = text(module.description),
      file = source.file,
      generated = source.generated,
      items = items,
      kind = module.type,
      line = module.lineno or 1,
      name = module.name,
      summary = text(module.summary),
    }
  end
  local handle = assert(io.open(out, "wb"))
  handle:write(encode({
    format = 1,
    ldoc = os.getenv("DX_LDOC_VERSION") or "",
    lua = _VERSION,
    modules = result,
  }))
  handle:close()
end

return M
