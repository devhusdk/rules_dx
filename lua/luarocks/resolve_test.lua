local rockspec = require("rockspec")

local INSPECT = [[
package = "inspect"
version = "3.1.3-0"
source = {
   url = "https://github.com/kikito/inspect.lua/archive/v3.1.3.tar.gz",
   dir = "inspect.lua-3.1.3",
}
dependencies = { "lua >= 5.1" }
build = { type = "builtin", modules = { inspect = "inspect.lua" } }
]]

local PENLIGHT = [[
local package_name = "penlight"
local package_version = "1.13.1"
local rockspec_revision = "1"
local github_account_name = "lunarmodules"
local github_repo_name = "Penlight"

package = package_name
version = package_version .. "-" .. rockspec_revision
source = {
   url = "git+https://github.com/" .. github_account_name .. "/" .. github_repo_name .. ".git",
   tag = "1.13.1",
}
dependencies = { "lua >= 5.1", "luafilesystem >= 1.6, < 2" }
build = {
   type = "builtin",
   modules = { ["pl.List"] = "lua/pl/List.lua", ["pl.utils"] = "lua/pl/utils.lua" },
}
]]

local function expect(text, label, golden)
  local resolved, problem = rockspec.resolve(text, label)
  assert(resolved, problem)
  local actual = rockspec.encode(resolved)
  assert(actual == golden, actual)
end

expect(
  INSPECT,
  "inspect-3.1.3-0.rockspec",
  '{"deps":[],"modules":{"inspect":"inspect.lua"},"package":"inspect",'
    .. '"source":{"strip_prefix":"inspect.lua-3.1.3","urls":["https://github.com/kikito/inspect.lua/archive/v3.1.3.tar.gz"]},'
    .. '"version":"3.1.3-0"}'
)

expect(
  PENLIGHT,
  "penlight-1.13.1-1.rockspec",
  '{"deps":["luafilesystem"],"modules":{"pl.List":"lua/pl/List.lua","pl.utils":"lua/pl/utils.lua"},"package":"penlight",'
    .. '"source":{"strip_prefix":"Penlight-1.13.1","urls":["https://github.com/lunarmodules/Penlight/archive/refs/tags/1.13.1.tar.gz"]},'
    .. '"version":"1.13.1-1"}'
)

local function refuses(text, needle)
  local resolved, problem = rockspec.resolve(text, "x.rockspec")
  assert(resolved == nil and problem:find(needle, 1, true), tostring(problem))
end

refuses(INSPECT:gsub('type = "builtin"', 'type = "make"'), "inspect: build.type make")
refuses(INSPECT:gsub('inspect = "inspect.lua"', 'inspect = { sources = "inspect.c" }'), "inspect: module inspect is a C module")
refuses(INSPECT:gsub("https://github.com/kikito/inspect.lua/archive/v3.1.3.tar.gz", "git+https://gitlab.com/a/b.git"), "inspect: source git+https://gitlab.com/a/b.git")
refuses(PENLIGHT:gsub('tag = "1.13.1",', ""), "penlight: source git+https://github.com/lunarmodules/Penlight.git has no tag")
refuses("this is not lua", "x.rockspec")
