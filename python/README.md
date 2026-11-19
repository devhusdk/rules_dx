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

## Dependencies

Add the package to `pyproject.toml`, then run `dx generate --apply`. dx writes
`uv.lock` if it is missing, and adds the `pypi` hub to `MODULE.bazel`. A hub
is a Bazel repository that holds one target per locked package:

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

### Writing targets

`python_test` runs pytest by default, so pytest must be in your project
dependencies. To run a `main` script instead, set `runner = "plain"`.
`dx generate` sets `runner = "plain"` for a test file that imports
`unittest`, not `pytest`, and defines no bare `test` functions. Every other
test gets `@pypi//pytest`.

`python_bench` is a `python_test` for `dx bench`. List `pytest-benchmark` in
your project dependencies and `@pypi//pytest_benchmark` in `deps`. Each
benchmark becomes one row of `dx-bench.json`, in seconds.

## Quality

| Check | Default tools |
| --- | --- |
| Format | `ruff` |
| Lint | `ruff`, `typos` |
| Typecheck | `ty` |

Configure `ruff` in `ruff.toml` and bind it with `ruff_config`. To also check
docstrings with `pydoclint`, add it in `dx.toml`:

```toml
[families.python]
lint = ["+pydoclint"]
```

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

The extension builds with the hermetic C++ toolchain, and the hermetic
interpreter imports it. Hermetic means Bazel fetches a pinned copy, so
nothing comes from the host. These compiler and platform pairs build it:

| Compiler | Platform |
| --- | --- |
| LLVM | linux_x86_64, linux_arm64 |
| GCC | linux_x86_64, linux_arm64 |
| clang-cl | windows_x86_64 |
| cl.exe | windows_x86_64 |

Any other pair fails at analysis, the step where Bazel reads BUILD files
before it runs anything. On Windows the extension links the dynamic CRT, so
an app that ships it must also ship `msvcp140` and `vcruntime140*` from
`@msvc_runtime`.

## Platforms

See the [support matrix](../docs/support-matrix.md).
