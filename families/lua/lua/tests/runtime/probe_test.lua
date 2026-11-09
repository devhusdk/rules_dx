local lib = require("lib")
local pkg = require("pkg")
local runfiles = require("runfiles")

local function check(ok, message)
  if not ok then
    error(message, 2)
  end
end

check(lib.greet("x") == "hello, x", "lib.greet")
check(pkg.shout("x") == "HELLO, X", "pkg.shout through init.lua")
check(package.loaded["pkg"] == pkg, "pkg is cached")

check(arg[1] == "--flag" and arg[2] == "value", "args arrive in arg")
check(select("#", ...) == 2 and select(1, ...) == "--flag", "args arrive in ...")
check(type(arg[0]) == "string" and arg[0]:sub(-#"probe_test.lua") == "probe_test.lua", "arg[0] is the main chunk")

check(os.getenv("LUA_PATH") ~= nil, "the test sets LUA_PATH")
check(os.getenv("LUA_INIT") ~= nil, "the test sets LUA_INIT")
check(package.path == "", "LUA_PATH is ignored: " .. package.path)
check(package.cpath == "", "LUA_CPATH is ignored: " .. package.cpath)
check(rawget(_G, "lua_init_ran") == nil, "LUA_INIT never runs")
check(#package.searchers == 2, "only the preload and module map searchers remain")

local path = runfiles.rlocation(os.getenv("DATA_FILE"))
check(path ~= nil, "data.txt resolves")
local file = assert(io.open(path, "r"))
local text = file:read("a")
file:close()
check(text == "lua runtime data\n", "data.txt content")

local ok, err = pcall(require, "not_declared")
check(not ok and tostring(err):find("no module 'not_declared' in the deps", 1, true), "undeclared modules fail")

print("probe_test passed")
