"""Public Lua module extensions."""

load("//lua/private/lock:hub.bzl", _luarocks = "luarocks")
load("//lua/private/toolchain:extensions.bzl", _lua = "lua")

lua = _lua
luarocks = _luarocks
