# Python

This directory helps you build, run and test Python code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//python:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Python page](../docs/languages/python.md) shows
setup, every rule and the tools dx runs.

Run the Python tests from a rules_dx checkout:

```sh
tools/dx test //python/...
```

## Writing targets

`python_test` runs pytest by default. Set `runner = "plain"` to run a `main`
script instead. `dx generate` sets `runner = "plain"` for a test file that
imports `unittest`, not `pytest`, and defines no bare `test` functions. Every
other test gets `@pypi//pytest`, so pytest must be in your project
dependencies.

`python_bench` is a `python_test` for `dx bench`. List `pytest-benchmark` in
your project dependencies and `@pypi//pytest_benchmark` in `deps`. Each
benchmark becomes one row, in seconds.

## Dependencies

Add the package to `pyproject.toml`, then run `dx generate --apply`. dx writes a missing `uv.lock` and the `pypi` hub in `MODULE.bazel`:

```starlark
uv = use_extension("@rules_py//uv:extensions.bzl", "uv")
uv.declare_hub(hub_name = "pypi")
uv.project(
    hub_name = "pypi",
    lock = "//:uv.lock",
    pyproject = "//:pyproject.toml",
)
use_repo(uv, "pypi")
```

An import whose top-level module matches a package in `uv.lock` resolves to
`@pypi//<name>`. To use another hub name, add
`# gazelle:dx_external python <path> <repo>` to a `BUILD` file.

## Quality

| Check | Default tool |
| --- | --- |
| Format | `ruff` |
| Lint | `ruff`, `pydoclint` |
| Typecheck | `ty` |

Configure `ruff` in `ruff.toml` and bind it with `ruff_config`.

## Packaging

### Native extensions

Build C and C++ extension modules with `py_extension`:

```starlark
load("@rules_python//python/cc:py_extension.bzl", "py_extension")

py_extension(
    name = "fast",
    srcs = ["fast.cc"],
)
```

The extension is built with the repo's hermetic C++ toolchain and imported by the hermetic interpreter. These compiler and platform pairs build it:

| Compiler | Platform |
| --- | --- |
| LLVM | linux_x86_64, linux_arm64 |
| GCC | linux_x86_64, linux_arm64 |
| clang-cl | windows_x86_64 |
| cl.exe | windows_x86_64 |

Windows links the dynamic CRT. An app that ships an extension must also ship `msvcp140` and `vcruntime140*` from `@msvc_runtime`. Invalid compiler and platform combinations fail at analysis.

## Platforms

See the [support matrix](../docs/support-matrix.md).
