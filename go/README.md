# Go

This directory helps you build, run and test Go code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//go:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Go page](../docs/languages/go.md) shows
setup, every rule and the tools dx runs.

Run the Go tests from a rules_dx checkout:

```sh
tools/dx test //go/...
```

## Dependencies

`go_library` requires `importpath`. `dx generate` fills it in as the `go.mod`
module path plus the directory.

Add the module to `go.mod` and wire it in `MODULE.bazel`:

```starlark
go = use_extension("@rules_dx//go:extensions.bzl", "go")
go.from_file(go_mod = "//:go.mod")
```

Then run `dx generate --apply`. An import under a module that `go.mod`
requires directly resolves to `@<repo>//<package>`. A module required only as
`// indirect` fails with a hint to require it directly.

## Quality

| Check | Default tool | Opt-in tools |
| --- | --- | --- |
| Format | `gofumpt` | |
| Typecheck | `gotype` | |
| Lint | none | `errcheck`, `govet`, `staticcheck` |

## Toolchain

### cgo

Ordinary cgo, meaning C sources and C++ through `cdeps` over a C ABI, runs with
LLVM (the default) and with GCC on Linux x86_64 and arm64. Under GCC, libstdc++
links statically. Race mode has its own test target (`gcc_cgo_race_test`) so a
race failure never hides ordinary cgo. Windows and MSVC are not supported yet.

cgo needs a GNU-style C++ driver and ABI. With `cgo = True`, an MSVC or clang-cl
toolchain fails at analysis, because rules_go would otherwise build the package
without cgo.

## Platforms

See the [support matrix](../docs/support-matrix.md).
