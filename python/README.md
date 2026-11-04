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
| Lint | `ruff`, `pydoclint` |
| Typecheck | `ty` |

Configure `ruff` in `ruff.toml` and bind it with `ruff_config`.

## Platforms

See the [support matrix](../docs/support-matrix.md).
