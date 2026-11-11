local package_name = "mid"
package = package_name
version = "2.1-1"
source = { url = "git+https://github.com/example/mid", tag = "v" .. "2.1" }
dependencies = { "lua >= 5.1", "leaf >= 1.0, < 2" }
build = {
  modules = {
    mid = "src/mid.lua",
    ["mid.util"] = "src/mid/util.lua",
  },
}
