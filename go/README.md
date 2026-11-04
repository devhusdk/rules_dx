# Go

Build, test, format and typecheck Go with `rules_go`.

```python
load("@rules_dx//go/rules:defs.bzl", "go_binary", "go_library", "go_test")

go_library(
    name = "greet",
    srcs = ["greet.go"],
    importpath = "example.com/demo/greet",
)

go_binary(
    name = "hello",
    srcs = ["main.go"],
    deps = [":greet"],
)

go_test(
    name = "greet_test",
    srcs = ["greet_test.go"],
    embed = [":greet"],
)
```

`go_library` requires `importpath`. `dx generate` fills it in as the `go.mod`
module path plus the directory.

## Dependencies

Add the module to `go.mod` and wire it in `MODULE.bazel`:

```starlark
go_deps = use_extension("@gazelle//:extensions.bzl", "go_deps")
go_deps.from_file(go_mod = "//:go.mod")
```

Then run `dx generate --apply`. An import under a module that `go.mod`
requires directly resolves to `@<repo>//<package>`. A module required only as
`// indirect` fails with a hint to require it directly.

## Quality tools

| Check | Default tool | Opt-in tools |
| --- | --- | --- |
| Format | `gofumpt` | |
| Typecheck | `gotype` | |
| Lint | none | `errcheck`, `govet`, `staticcheck` |

## cgo

cgo needs a GNU-style C++ driver and ABI. With `cgo = True`, an MSVC or clang-cl
toolchain fails at analysis, because rules_go would otherwise build the package
without cgo.

## Platforms

See the [support matrix](../docs/support-matrix.md).
