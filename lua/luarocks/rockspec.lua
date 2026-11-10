local M = {}

local ARCHIVE_SUFFIXES = { ".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".zip" }

local function escape(text)
  return (
    text:gsub('[%c"\\]', function(c)
      local named = { ['"'] = '\\"', ["\\"] = "\\\\", ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t" }
      return named[c] or string.format("\\u%04x", c:byte())
    end)
  )
end

local function sorted_keys(map)
  local keys = {}
  for key in pairs(map) do
    keys[#keys + 1] = key
  end
  table.sort(keys)
  return keys
end

function M.encode(value)
  local kind = type(value)
  if kind == "string" then
    return '"' .. escape(value) .. '"'
  elseif kind ~= "table" then
    return tostring(value)
  end
  local parts = {}
  if #value > 0 or next(value) == nil then
    for _, item in ipairs(value) do
      parts[#parts + 1] = M.encode(item)
    end
    return "[" .. table.concat(parts, ",") .. "]"
  end
  for _, key in ipairs(sorted_keys(value)) do
    parts[#parts + 1] = '"' .. escape(key) .. '":' .. M.encode(value[key])
  end
  return "{" .. table.concat(parts, ",") .. "}"
end

local function archive_dir(url)
  local name = url:gsub("[?#].*$", ""):match("([^/]+)$") or ""
  for _, suffix in ipairs(ARCHIVE_SUFFIXES) do
    if name:sub(-#suffix) == suffix then
      return name:sub(1, #name - #suffix)
    end
  end
  return name
end

local function github_repo(url)
  local owner, repo = url:match("^git%+https://github%.com/([^/]+)/([^/]+)$")
  if not owner then
    owner, repo = url:match("^git://github%.com/([^/]+)/([^/]+)$")
  end
  if not owner then
    owner, repo = url:match("^git%+ssh://git@github%.com/([^/]+)/([^/]+)$")
  end
  if owner then
    return owner, (repo:gsub("%.git$", ""))
  end
end

local function source_of(rock, source)
  if type(source) ~= "table" or type(source.url) ~= "string" then
    return nil, rock .. ": source.url is missing"
  end
  local url = source.url
  local owner, repo = github_repo(url)
  if owner then
    if type(source.tag) ~= "string" then
      return nil, rock .. ": source " .. url .. " has no tag; only GitHub tags are supported"
    end
    local dir = source.dir or (repo .. "-" .. source.tag:gsub("^v", ""))
    return {
      urls = { "https://github.com/" .. owner .. "/" .. repo .. "/archive/refs/tags/" .. source.tag .. ".tar.gz" },
      strip_prefix = dir,
    }
  end
  if not (url:match("^https://") or url:match("^http://")) or url:match("%.git$") then
    return nil, rock .. ": source " .. url .. " is not an archive URL or a GitHub tag"
  end
  return { urls = { url }, strip_prefix = source.dir or archive_dir(url) }
end

local function dependencies_of(list)
  local deps = {}
  for _, entry in ipairs(type(list) == "table" and list or {}) do
    local name = type(entry) == "string" and entry:match("^%s*([%w%._%-]+)")
    if name and name ~= "lua" then
      deps[#deps + 1] = name
    end
  end
  table.sort(deps)
  return deps
end

local function modules_of(rock, build)
  local build_type = type(build) == "table" and build.type or "builtin"
  if build_type ~= "builtin" then
    return nil, rock .. ": build.type " .. tostring(build_type) .. " is not supported; only builtin"
  end
  local modules = {}
  for module, path in pairs(type(build) == "table" and build.modules or {}) do
    if type(path) ~= "string" then
      return nil, rock .. ": module " .. tostring(module) .. " is a C module; only Lua sources are supported"
    end
    modules[module] = path
  end
  if next(modules) == nil then
    return nil, rock .. ": build.modules is empty"
  end
  return modules
end

-- Returns the sidecar fields of one rockspec text, or nil and a message naming the rock.
function M.resolve(text, label)
  local env = {}
  local chunk, problem = load(text, label, "t", env)
  if not chunk then
    return nil, label .. ": " .. problem
  end
  local ok, failure = pcall(chunk)
  if not ok then
    return nil, label .. ": " .. tostring(failure)
  end
  local rock = env.package
  if type(rock) ~= "string" or type(env.version) ~= "string" then
    return nil, label .. ": package or version is missing"
  end
  local source, source_problem = source_of(rock, env.source)
  if not source then
    return nil, source_problem
  end
  local modules, module_problem = modules_of(rock, env.build)
  if not modules then
    return nil, module_problem
  end
  return {
    package = rock,
    version = env.version,
    source = source,
    modules = modules,
    deps = dependencies_of(env.dependencies),
  }
end

return M
