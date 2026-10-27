# Lua

Build, run and test Lua 5.4 with Lua built from source through the
[`lua` module](https://registry.bazel.build/modules/lua) on the Bazel Central
Registry. Nothing is installed on the host and no host `lua` runs. rules_dx
registers the Lua toolchain, so there is no toolchain setup in
`MODULE.bazel`.

## Start

Load the rules from `@rules_dx//lua/rules:defs.bzl`.

```python
load("@rules_dx//lua/rules:defs.bzl", "lua_binary", "lua_library", "lua_test")

lua_library(
    name = "catalog",
    srcs = [
        "catalog/init.lua",
        "catalog/parse.lua",
    ],
    strip_import_prefix = "/app",
)

lua_test(
    name = "catalog_test",
    main = "catalog_test.lua",
    deps = [":catalog"],
)

lua_binary(
    name = "cli",
    main = "cli.lua",
    data = ["units.tsv"],
    deps = [":catalog"],
)
```

`bazel run //app:cli -- arg1 arg2` runs `cli.lua` with the arguments in
`arg` and in `...`.

## Rules

| Rule | What it makes |
| --- | --- |
| `lua_library` | Lua modules other Lua targets can require |
| `lua_binary` | A Lua program that `bazel run` starts through the shared launcher |
| `lua_test` | A Lua test; it passes when `main` returns or calls `os.exit(0)` |

| Attribute | Rules | Meaning |
| --- | --- | --- |
| `srcs` | all | Lua modules of this target. Each file becomes one module named after its path |
| `deps` | all | `lua_library` targets whose modules this target requires |
| `data` | all | Files the Lua code reads at run time; find them with `require('runfiles').rlocation` |
| `strip_import_prefix` | all | Directory removed from module paths. `/x` starts at the repository root, `x` at the package |
| `import_prefix` | all | Module prefix added after `strip_import_prefix`, for example `vendor.inspect` |
| `main` | `lua_binary`, `lua_test` | The `.lua` chunk that runs first |
| `env` | `lua_binary`, `lua_test` | Environment variables set when the target runs or is tested; `$(location)` is expanded |

`LuaInfo` carries the modules of a target and its deps. Load it from
`@rules_dx//lua/rules:defs.bzl`.

## Modules and data files

- A module is named after its path from the repository root, after
  `strip_import_prefix` and `import_prefix`: `a/b.lua` is `a.b` and
  `a/init.lua` is `a`.
- `require` looks only at `package.preload` and at the modules of the target
  and its `deps`. A module that is not in `deps` fails with
  `no module '<name>' in the deps of this target`. Two targets that provide
  the same module name fail the build and name both.
- `package.path` and `package.cpath` are empty. `LUA_PATH`, `LUA_CPATH` and
  `LUA_INIT` are ignored, and C modules are never loaded.
- Read data files through the built-in `runfiles` module:

  ```lua
  local path = require("runfiles").rlocation("_main/app/units.tsv")
  ```

  Pass the key with `env = {"UNITS": "$(rlocationpath units.tsv)"}` so it
  stays right when the repository is used as a dependency.
- An uncaught error prints a traceback and exits with code 1.
  `os.exit(n)` exits with `n`.
- `bazel build` checks the syntax of every source with `luac`, so a syntax
  error fails the build.

## Packages

There is no LuaRocks support. Pin a pure-Lua package with `http_archive` and
a `build_file` that declares a `lua_library`:

```python
http_archive(
    name = "inspect_lua",
    build_file = "//third_party/lua:inspect.BUILD.bazel",
    sha256 = "<sha256>",
    strip_prefix = "inspect.lua-3.1.3",
    urls = ["https://github.com/kikito/inspect.lua/archive/refs/tags/v3.1.3.tar.gz"],
)
```

```python
load("@rules_dx//lua/rules:defs.bzl", "lua_library")

lua_library(
    name = "inspect",
    srcs = ["inspect.lua"],
    visibility = ["//visibility:public"],
)
```

## Quality

Lua sources form the `lua` class, so `dx check` sees them. StyLua formats them
when a `lua` family selects it.

```python
quality_family(
    name = "lua_family",
    family_id = "lua",
    format = ["stylua"],
)
```

StyLua never reads a `stylua.toml` or `.editorconfig` next to the sources. Bind
a `stylua_config` from `@rules_dx//quality:native_config.bzl` to set indent
width or quote style. No Lua linter or type check runs yet, so the Lua rules add
the `no-lint` and `no-typecheck` tags to every target.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_x86_64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |
| `windows_arm64` | Not qualified |
