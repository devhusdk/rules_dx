# Lua app example

A Lua unit-stock CLI split into four packages. Every target runs on the Lua
5.4 that rules_dx builds from source, with nothing installed on the host.

| Package | Role |
| --- | --- |
| `catalog` | `catalog` and `catalog.parse`, which read `units.tsv` and reject malformed rows |
| `report` | `report`, the unit table, and a `--dump` view through the pinned `inspect.lua` |
| `cli` | `units` command line and an exit code test |
| `diagnostics` | Syntax error, runtime error, missing dependency and StyLua checks |

```sh
bazel build //examples/lua-app/...
bazel test //examples/lua-app/...
bazel run //examples/lua-app/cli:units -- report
bazel run //examples/lua-app/cli:units -- get cable
```

The report prints one line per unit and a total. `get NAME` prints one unit.
Exit codes: `0` ok, `1` the data file cannot be read, `2` usage, `4` unknown
unit. The data file reaches the program through the `UNITS` environment
variable, set with `$(rlocationpath ...)` on the `lua_binary`.

The `--//examples/lua-app/cli:order=name` flag sorts the report by unit name. It
switches `UNITS_ORDER` through `select()`, and `exit_codes_test` follows it.

`inspect.lua` 3.1.3 (MIT) is pinned with `http_archive` and a sha256 in
`MODULE.bazel`. Its `inspect.BUILD.bazel` declares one `lua_library`. There is
no LuaRocks support.

| Platform | Status |
| --- | --- |
| linux_x86_64 | Qualified |
| linux_arm64, macos_arm64, windows_x86_64, windows_arm64 | Not qualified |
