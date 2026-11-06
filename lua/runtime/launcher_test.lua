local helper = require("helper")
local runfiles = require("runfiles")

assert(helper.answer == 42, "the module map resolves helper")
assert(arg[1] == "one" and select("#", ...) == 1, "arguments reach the chunk")
assert(runfiles.rlocation("_main/lua/runtime/launcher_test_helper.lua") ~= nil, "runfiles resolves")
assert(runfiles.rlocation("../escape") == nil, "an invalid rlocation path resolves to nil")

if package.config:sub(1, 1) == "\\" then
  return { answer = 42 }
end

local tmp = assert(os.getenv("TEST_TMPDIR"), "TEST_TMPDIR")
local launcher = assert(runfiles.rlocation("_main/lua/runtime/launcher_test"), "launcher path")
local good = tmp .. "/good.lua"
local missing = tmp .. "/missing.lua"

local function write(path, text)
  local file = assert(io.open(path, "wb"))
  file:write(text)
  file:close()
end

local scripts = {
  good = 'print("ran", ...)',
  syntax = "this is not lua",
  error_string = 'error("boom")',
  error_table = "error({})",
  error_tostring = 'error(setmetatable({}, {__tostring = function() return "custom" end}))',
  modules = [[
local ok, err = pcall(require, "nope")
print(err)
ok, err = pcall(require, "gone")
print(err)
ok, err = pcall(require, "broken")
print(err)
]],
}
for name, text in pairs(scripts) do
  write(tmp .. "/" .. name .. ".lua", text)
end

local function run(name, entry, options)
  options = options or {}
  local link = tmp .. "/" .. name
  os.remove(link)
  assert(os.execute("ln -s '" .. launcher .. "' '" .. link .. "'"), "symlink " .. name)
  if entry then
    write(link:gsub("%.[eE][xX][eE]$", "") .. ".luaentry", entry)
  end
  local command = options.command or "'" .. link .. "'"
  local pipe = assert(io.popen(command .. " 2>&1"))
  local output = pipe:read("a")
  local _, _, code = pipe:close()
  return output, code
end

local function expect(output, code, want_code, want_text)
  assert(code == want_code, "exit " .. tostring(code) .. ": " .. output)
  assert(output:find(want_text, 1, true), "missing '" .. want_text .. "' in: " .. output)
end

local output, code = run("none", nil)
expect(output, code, 1, "cannot open entry file")

output, code = run("bad_line", good .. "\n\nbroken\n")
expect(output, code, 1, "is not <module>")

output, code = run("short", good .. "\n")
expect(output, code, 1, "missing the main rlocation")

output, code = run("no_main", "\n\n")
expect(output, code, 1, "missing the main rlocation")

output, code = run("lost", missing .. "\n\n")
expect(output, code, 1, "is not in the runfiles")

output, code = run("syntax_main", tmp .. "/syntax.lua\n\n")
expect(output, code, 1, "syntax.lua")

output, code = run("crlf.EXE", good .. "\r\n\r\ngood\t" .. good .. "\r\n\r\n")
expect(output, code, 0, "ran")

output, code = run("string_error", tmp .. "/error_string.lua\n\n")
expect(output, code, 1, "boom")

output, code = run("table_error", tmp .. "/error_table.lua\n\n")
expect(output, code, 1, "error object is a table value")

output, code = run("tostring_error", tmp .. "/error_tostring.lua\n\n")
expect(output, code, 1, "custom")

output, code = run(
  "map",
  tmp .. "/modules.lua\n\ngone\t" .. missing .. "\nbroken\t" .. tmp .. "/syntax.lua\n"
)
expect(output, code, 0, "no module 'nope' in the deps of this target")
expect(output, code, 0, "module 'gone' maps to " .. missing .. ", which is not in the runfiles")
expect(output, code, 0, "error loading module 'broken'")

output, code = run("coverage_ok", good .. "\n\ndx_coverage\t" .. good .. "\n")
expect(output, code, 0, "ran")

output, code = run("coverage_bad", good .. "\n\ndx_coverage\t" .. tmp .. "/error_string.lua\n")
expect(output, code, 1, "boom")

output, code = run("empty_argv0", good .. "\n\n", { command = "bash -c 'exec -a \"\" \"$0\"' '" .. tmp .. "/empty_argv0'" })
expect(output, code, 1, "argv[0] is empty")

output, code = run("no_runfiles", good .. "\n\n", { command = "env -u RUNFILES_DIR -u RUNFILES_MANIFEST_FILE '" .. tmp .. "/no_runfiles'" })
expect(output, code, 1, "cannot open runfiles")

output, code = run("a", good .. "\n\n", { command = "cd '" .. tmp .. "' && ./a" })
expect(output, code, 0, "ran")

return { answer = 42 }
