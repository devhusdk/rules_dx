# Lua

This directory helps you build, run and test Lua code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//lua:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Lua page](../docs/languages/lua.md) shows
setup, every rule and the tools dx runs.

Run the Lua tests from a rules_dx checkout:

```sh
tools/dx test //lua/...
```

## Dependencies

### Modules and data files

A module is named after its path from the repository root, after
`strip_import_prefix` and `import_prefix`: `a/b.lua` is `a.b`, and
`a/init.lua` is `a`.

`require` looks only at `package.preload` and at the modules of the target
and its `deps`:

- A module that is not in `deps` fails with
  `no module '<name>' in the deps of this target`.
- Two targets that provide the same module name fail the build and name
  both.
- `package.path` and `package.cpath` are empty. `LUA_PATH`, `LUA_CPATH` and
  `LUA_INIT` are ignored, and C modules never load.

Runfiles are the files Bazel places beside a program when it runs, such as
its `data`. Read them through the built-in `runfiles` module:

```lua
local path = require("runfiles").rlocation("_main/app/units.tsv")
```

Pass the key with `env = {"UNITS": "$(rlocationpath units.tsv)"}` so it stays
right when another repository uses yours as a dependency.

Errors and syntax:

- An uncaught error prints a traceback and exits with code 1. `os.exit(n)`
  exits with `n`.
- `bazel build` checks the syntax of every source with `luac`, so a syntax
  error fails the build.

### LuaRocks packages

Put a `*.rockspec` with your `dependencies` at the root. Then:

1. Run `dx generate --apply`. It writes `luarocks.lock.json` beside the
   rockspec if there is none, and adds the `luarocks.lock` stanza for the
   `@rocks` hub to `MODULE.bazel`. A hub is one Bazel repository that holds
   a target for every locked package.
2. Depend on a rock as `@rocks//<rock>`, for example `@rocks//inspect`.
3. Run `dx update --apply` to refresh the lock. It resolves the rockspec
   `dependencies` from `https://luarocks.org/manifest-5.5.json` and records
   each rock's modules and archive sha256. Rocks that did not change are
   kept.

The hub has one `lua_library` per rock. What the lock accepts:

- Only `build.type = "builtin"` rocks with pure-Lua modules.
- A `git+` source must be a GitHub URL with a `tag`.
- A module file must end in the module's path, such as `src/a/b.lua` for
  `a.b`.

`dx security` and `dx license` read `luarocks.lock.json`. OSV has no
LuaRocks ecosystem, so `dx security` reports the set as uncovered. Accept
that with `--accept uncovered:<set>`.

### A package by hand

You can also pin a pure-Lua package with `http_archive` and a `build_file`
that declares a `lua_library`:

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

`dx generate` writes `lua_library` and `lua_test` rules. Each package gets
one `lua_library` over its `.lua` files and one `lua_test` per
`*_test.lua`, with `main` set to that file. Imports are the string literals
of plain `require` calls, found with a Lua parser.

- A module is named by its path, as `lua_library` names it:
  `catalog/init.lua` is `catalog`. A file with a dot in its stem has no
  module and is left out.
- `lua_binary` targets stay hand-written. Sources that a hand-written
  `lua_binary` lists in `main` or `srcs` are left out of the generated
  library.
- Standard library modules and `runfiles` need no dep.
- A third-party module resolves through the lock. You need no directive.
- `require` inside `pcall` or `xpcall` is optional and is skipped. So are
  dynamic names, an aliased `require` (`local r = require`) and text in
  comments or strings.
- `strip_import_prefix` and `import_prefix` are never generated. A
  hand-written library that sets them is indexed under the names they give.

## Quality

Lua sources form the `lua` class, so `dx check` sees them. Select the tools
in a `lua` family in `dx.toml`:

```toml
[families.lua]
format = ["stylua"]
lint = ["selene"]
typecheck = ["luals"]
```

The [config reference](../docs/cli/commands/config.md#dxtoml) covers
`dx.toml`. Bind each tool's config with the rule of the same name from
`@rules_dx//core/quality:native_config.bzl`.

### Format with StyLua

StyLua never reads a `stylua.toml` or `.editorconfig` next to the sources.
Bind a `stylua_config` to set indent width or quote style.

### Lint with selene

rules_dx builds `selene` 0.32.0 from source with the Lua 5.2 to 5.5 parsers.
The default config uses `std = "lua53+lua54"`. Bind a `selene_config` to
change lints or the standard library. selene has no autofix.

### Type check with LuaLS

`luals` runs `lua-language-server --check`, pinned to 3.19.2. It runs on
Linux x86_64 only.

- Modules of `deps` are staged as a library and never reported. Only the
  target's own sources are.
- Bind a `luals_config`, a plain `.luarc.json` (never `.jsonc`), to set
  globals or diagnostics.
- dx always sets the runtime version, the library, `libraryFiles` and the
  output paths.
- `--@rules_dx//config:tool_luals` takes the label of another
  `lua-language-server`.

## Coverage

Lua tests write LCOV under `bazel coverage` and `dx coverage` through
luacov. rules_dx brings luacov 0.17.0 and luacov-reporter-lcov 0.2 and adds
them to a `lua_test` only under coverage, so `bazel test` never fetches
them. Each record names a file by its path from the repository root.

Tag a test `no-coverage` to skip it.

## API docs

`dx docs` documents a `lua_library` with `lua_doc_shard(name, lib, package)`
from `@rules_dx//tools/docs/lua:defs.bzl`. It runs the pinned LDoc 1.5.0 on
the pinned Lua over the library's own sources and writes a DocIr shard.
Nothing needs to be installed.

Document modules with `@module` and functions with LDoc tags. Local
functions are left out.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms.
