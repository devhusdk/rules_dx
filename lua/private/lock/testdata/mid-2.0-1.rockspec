package = "mid"
version = "2.0-1"
source = { url = "git+https://github.com/example/mid", tag = "v2.0" }
dependencies = { "lua >= 5.1", "leaf >= 1.0" }
build = { type = "builtin", modules = { mid = "mid.lua" } }
