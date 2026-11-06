package = "mixed"
version = "1.0-1"
source = { url = "git+https://example.invalid/mixed.git" }
dependencies = {
  "lua >= 5.1",
  "luasocket >= 3.0",
  "lua-cjson == 2.1.0",
  "penlight",
}
build = {
  type = "builtin",
  modules = { mixed = "src/mixed.lua" },
}
