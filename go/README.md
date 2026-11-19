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

Every `go_library` needs an `importpath`. `dx generate` fills it in: the
`go.mod` module path plus the directory.

To use a third-party module, add it to `go.mod` and load `go.mod` in your
`MODULE.bazel`:

```starlark
go = use_extension("@rules_dx//go:extensions.bzl", "go")
go.from_file(go_mod = "//:go.mod")
```

Then run `dx generate --apply`. An import from a module that `go.mod` requires
directly resolves to `@<repo>//<package>`. A module required only as
`// indirect` fails, with a hint to require it directly.

## Quality

| Check | Default tools |
| --- | --- |
| Format | `gofumpt` |
| Lint | `errcheck`, `govet`, `staticcheck`, `typos` |
| Typecheck | `gotype` |

To change the tools, set `[families.go]` in `dx.toml`.

## Toolchain

### cgo

cgo builds C sources, and C++ through `cdeps` behind a C ABI. It works with
LLVM, the default, and with GCC on Linux x86_64 and arm64. Under GCC,
libstdc++ links statically. cgo does not work on Windows or with MSVC.

cgo needs a GNU-style C++ driver. With `cgo = True`, an MSVC or clang-cl
toolchain fails at analysis, the step where Bazel reads BUILD files before it
runs anything. So a package never builds without cgo by mistake.

## Platforms

See the [support matrix](../docs/support-matrix.md).
