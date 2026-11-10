"""Public Lua API."""

load(
    "//lua/private:defs.bzl",
    _lua_bench = "lua_bench",
    _lua_binary = "lua_binary",
    _lua_library = "lua_library",
    _lua_test = "lua_test",
)

lua_library = _lua_library
lua_binary = _lua_binary
lua_test = _lua_test
lua_bench = _lua_bench
