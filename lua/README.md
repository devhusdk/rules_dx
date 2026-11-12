# Lua

Build, run and test Lua 5.5 with Lua built from source through the
[`lua` module](https://registry.bazel.build/modules/lua) on the Bazel Central
Registry. Nothing is installed on the host and no host `lua` runs. rules_dx
registers the Lua toolchain, so there is no toolchain setup in
`MODULE.bazel`.

## Dependencies

### Modules and data files

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

### Packages

The `luarocks` module extension turns a `luarocks.lock.json` into a hub with one
`lua_library` per rock. Only pure-Lua rocks with the `builtin` build type are
supported.

```python
luarocks = use_extension("@rules_dx//lua:extensions.bzl", "luarocks")
luarocks.lock(
    name = "rocks",
    lock = "//third_party/lua:luarocks.lock.json",
)
use_repo(luarocks, "rocks")
```

Depend on `@rocks//inspect`. `bazel run @rules_dx//lua/private/tools/lock -- --dir third_party/lua`
resolves the `dependencies` of the `*.rockspec` in that directory from
`https://luarocks.org/manifest-5.5.json` and writes `luarocks.lock.json` with each
rock's modules and archive sha256. Unchanged rocks are kept. `dx update` runs it.

`dx security` and `dx license` read `luarocks.lock.json`. `dx security` reports the set as uncovered because OSV has no LuaRocks ecosystem; accept it with `--accept uncovered:<set>`.

Only `build.type = "builtin"` rocks with Lua modules are accepted. A `git+` source
must be a GitHub URL with a `tag`. A module file must end in the module's path, such
as `src/a/b.lua` for `a.b`.

Alternatively, pin a pure-Lua package with `http_archive` and
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
load("@rules_dx//lua:defs.bzl", "lua_library")

lua_library(
    name = "inspect",
    srcs = ["inspect.lua"],
    visibility = ["//visibility:public"],
)
```

## Generate

`dx generate` writes `lua_library` and `lua_test` rules. Each package gets one
`lua_library` over its `.lua` files and one `lua_test` per `*_test.lua`, with
`main` set to that file. Imports are the string literals of plain `require`
calls, found with a Lua parser.

- A module is named by its path, as `lua_library` names it: `catalog/init.lua`
  is `catalog`. A file with a dot in its stem has no module and is left out.
- `lua_binary` targets stay hand-written. Sources a hand-written `lua_binary`
  lists in `main` or `srcs` are left out of the generated library.
- Standard library modules and `runfiles` need no dep.
- A third-party module is resolved from the lock's `provides`; no directive.
- `require` inside `pcall` or `xpcall` is optional and is skipped. So are
  dynamic names, an aliased `require` (`local r = require`) and text in
  comments or strings.
- `strip_import_prefix` and `import_prefix` are never generated. A
  hand-written library that sets them is indexed under the names they give.

## Quality

Lua sources form the `lua` class, so `dx check` sees them. StyLua formats them
when a `lua` family selects it.

```toml
[families.lua]
format = ["stylua"]
```

See the [config reference](../docs/cli/commands/config.md#dxtoml).

StyLua never reads a `stylua.toml` or `.editorconfig` next to the sources. Bind
a `stylua_config` from `@rules_dx//core/quality:native_config.bzl` to set indent
width or quote style.

### Lint with selene

`dx check` lints Lua with `selene` 0.32.0, built from source with the Lua 5.2
to 5.5 parsers. Add `lint = ["selene"]` to the `lua` family. The default config
uses `std = "lua53+lua54"`; bind a `selene_config` from
`@rules_dx//core/quality:native_config.bzl` to change lints or the standard library.
selene has no autofix.

### Type check with LuaLS

`dx check` type checks Lua sources with `lua-language-server --check`, pinned to
3.19.2. Select `luals` in a `lua` family;
`--@rules_dx//config:tool_luals` overrides the pinned binary. Modules of `deps` are staged as a library and are never
reported; only the sources of the target are. Bind a `luals_config` from
`@rules_dx//core/quality:native_config.bzl` (a plain `.luarc.json`, never `.jsonc`) to
set globals or diagnostics. The runtime version, the library, `libraryFiles` and
the output paths are always set by dx. Linux x86_64 only.

## Coverage

Lua tests write LCOV under `bazel coverage` and `dx coverage` through luacov.
rules_dx brings pinned luacov 0.17.0 and luacov-reporter-lcov 0.2 and adds them to
a `lua_test` only under coverage, so `bazel test` never fetches them. Records
name each file by its path from the repository root.

Tag a test `no-coverage` to skip it under coverage.

## API docs

`dx docs` documents a `lua_library` with `lua_doc_shard(name, lib, package)` from
`@rules_dx//tools/docs/lua:defs.bzl`. It runs the pinned LDoc 1.5.0 on the
pinned Lua over the library's own sources and writes a DocIr shard. LDoc needs
the C module LuaFileSystem, so rules_dx builds LuaFileSystem 1.9.0 from a
sha256-pinned source with the hermetic C toolchain and links it into the Lua
that runs LDoc. Penlight 1.15.0 is pinned the same way. Document modules with
`@module` and functions with LDoc tags; local functions are left out.

## Platforms

See the [support matrix](../docs/support-matrix.md).
