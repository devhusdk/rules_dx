# Zig

Build, run, test and format Zig with the pinned `rules_zig` 0.14.0 and Zig
0.16.0 toolchain. Nothing needs to be installed.

```python
load("@rules_dx//zig/rules:defs.bzl", "zig_binary", "zig_library", "zig_test")

zig_library(
    name = "greet",
    main = "greet.zig",
)

zig_binary(
    name = "hello",
    main = "main.zig",
    deps = [":greet"],
)

zig_test(
    name = "greet_test",
    main = "greet.zig",
)
```

- `main` is the module root. `srcs` lists the other `.zig` files.
- `zig_library` imports by its target name. Set `import_name` to change it.
- A `cc_library` in `deps` is imported as `@import("c")`.
- `extra_srcs` carries files read with `@embedFile`.
- Other attributes pass through to `rules_zig`.

Start a project with `dx init --apply demo` and add sources. See
`examples/zig-app` for a multi-package app.

## Formatting

`dx format` runs `zig fmt` when a `zig` family selects `zig_fmt`. No flag is
needed. The Zig SDK is fetched only for Zig targets:

```sh
dx format --apply
```

## Linting

`dx lint` runs `zig ast-check` when a `zig` family selects `zig_ast_check`. No
flag is needed. It runs once per file and reports syntax and AST-level errors:

```sh
dx lint
```

## Generating BUILD files

`dx generate` writes one `zig_library` per directory. The root is
`<directory>.zig`, `root.zig`, `lib.zig` or the only `.zig` file. Other `.zig`
files go to `srcs`. `*_test.zig` and `build.zig` are skipped. The root package is named
after `.name` in `build.zig.zon`, else the repo name, else `lib`. `@import("name")` adds the library whose import
name is `name`. `@import` of `std`, `builtin`, `root`, `bazel_builtin` and
`c` adds nothing. `.zon` imports and `@embedFile` files in the same directory
go to `extra_srcs`. A file import or `@embedFile` that points outside the
directory, is missing, or names a `*_test.zig` or `build.zig` file fails
generation. A directory with `pub fn main` fails generation: write its
`zig_binary` by hand.

## Platforms

| Host | Zig SDK | Qualified |
| --- | --- | --- |
| Linux x86_64 | yes | yes |
| Linux arm64 | yes | no |
| macOS x86_64, arm64 | yes | no |
| Windows x86_64 | yes | no |
| Windows arm64 | yes | no |

## Coverage

Zig tests have no coverage collector yet. `dx coverage` fails with `no_coverage_collector` when one runs. Tag a test `no-coverage` to skip it under coverage.
