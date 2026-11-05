# Python

Build, test, format, lint and typecheck Python with `rules_python`.

```python
load("@rules_dx//python/rules:defs.bzl", "python_binary", "python_library", "python_test")

python_library(
    name = "greet",
    srcs = ["greet.py"],
)

python_binary(
    name = "hello",
    srcs = ["main.py"],
    deps = [":greet"],
)

python_test(
    name = "greet_test",
    srcs = ["greet_test.py"],
    deps = [":greet"],
)
```

`python_test` runs pytest by default. Set `runner = "plain"` to run a `main`
script instead.

`python_benchmark` is a `python_test` for `dx bench`. List `pytest-benchmark`
in the project dependencies and `@pypi//pytest_benchmark` in `deps`. Each
benchmark becomes one row, in seconds.

## Dependencies

Declare a `uv` hub named `pypi` in `MODULE.bazel`:

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

Run `uv lock`, then `dx generate --apply`. An import whose top-level module
matches a package in `uv.lock` resolves to `@pypi//<name>`. To use another
hub name, add `# gazelle:dx_external python <path> <repo>` to a `BUILD` file.

## Quality tools

| Check | Default tool |
| --- | --- |
| Format | `ruff` |
| Lint | `ruff`, `pydoclint`, `vulture` |
| Typecheck | `ty` |

Configure `ruff` in `ruff.toml` and bind it with `ruff_config`.

## Native extensions

Build C and C++ extension modules with `py_extension`:

```starlark
load("@rules_python//python/cc:py_extension.bzl", "py_extension")

py_extension(
    name = "fast",
    srcs = ["fast.cc"],
)
```

The extension is built with the repo's hermetic C++ toolchain and imported by the hermetic interpreter. `native_ext_test` loads one on every cell below.

| Compiler | Platform | Status |
| --- | --- | --- |
| LLVM | linux_x86_64, linux_arm64 | qualified |
| GCC | linux_x86_64, linux_arm64 | qualified |
| clang-cl | windows_x86_64 | qualified |
| cl.exe | windows_x86_64 | pending #6489 |
| any | windows_arm64 | pending #7119 |

Windows links the dynamic CRT. An app that ships an extension must also ship `msvcp140` and `vcruntime140*` from `@msvc_runtime`. Invalid compiler and platform combinations fail at analysis.

## Platforms

See the [support matrix](../docs/support-matrix.md).
