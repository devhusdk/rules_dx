"""Python runtime ownership gate for the coherent setup keep review."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//modules:python.bzl", _MOD_ASPECT_PY = "ASPECT_RULES_PY_VERSION", _MOD_PY_HUB = "UV_HUB", _MOD_PY_LOCKS = "UV_LOCKS", _MOD_PY_PROJECTS = "UV_PROJECTS", _MOD_PY_REPIN = "UV_REPIN_DIRS", _MOD_PY_VERSION = "PYTHON_VERSION", _MOD_RULES_PY = "RULES_PYTHON_VERSION")
load("//modules:versions.bzl", _ASPECT_PY = "FLOOR_ASPECT_RULES_PY", _PY = "PYTHON_VERSION", _RULES_PY = "FLOOR_RULES_PYTHON")

_UV_PROJECTS_BLOCK = """UV_PROJECTS = [
    "//examples/python-service:pyproject.toml",
    "//python/tests/fixtures/hello:pyproject.toml",
    "//quality/tools/python:pyproject.toml",
    "//tools/ci/tooling:pyproject.toml",
]"""

_UV_LOCKS_BLOCK = """UV_LOCKS = [
    "//examples/python-service:uv.lock",
    "//python/tests/fixtures/hello:uv.lock",
    "//quality/tools/python:uv.lock",
    "//tools/ci/tooling:uv.lock",
]"""

def python_runtime_unit_tests(name):
    """Instantiates the Python runtime ownership keep checks."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "aspect backend stays pinned",
                _MOD_ASPECT_PY,
                "2.0.0-alpha.10",
            ),
            expect_equal(
                "rules_python stays pinned beside aspect",
                _MOD_RULES_PY,
                "1.9.0",
            ),
            expect_equal(
                "managed interpreter stays 3.12",
                _MOD_PY_VERSION,
                "3.12",
            ),
            expect_equal(
                "python module follows canonical aspect pin",
                _MOD_ASPECT_PY,
                _ASPECT_PY,
            ),
            expect_equal(
                "python module follows canonical rules pin",
                _MOD_RULES_PY,
                _RULES_PY,
            ),
            expect_equal(
                "python module follows canonical interpreter",
                _MOD_PY_VERSION,
                _PY,
            ),
            expect_equal(
                "wheel hub stays dx_pypi",
                _MOD_PY_HUB,
                "dx_pypi",
            ),
            expect_equal(
                "wheel projects stay hello, ledger and tools",
                _MOD_PY_PROJECTS,
                [
                    "//examples/python-service:pyproject.toml",
                    "//python/tests/fixtures/hello:pyproject.toml",
                    "//quality/tools/python:pyproject.toml",
                    "//tools/ci/tooling:pyproject.toml",
                ],
            ),
            expect_equal(
                "wheel locks stay hello, ledger and tools",
                _MOD_PY_LOCKS,
                [
                    "//examples/python-service:uv.lock",
                    "//python/tests/fixtures/hello:uv.lock",
                    "//quality/tools/python:uv.lock",
                    "//tools/ci/tooling:uv.lock",
                ],
            ),
            expect_equal(
                "repin dirs stay hello, ledger and tools",
                _MOD_PY_REPIN,
                [
                    "examples/python-service",
                    "python/tests/fixtures/hello",
                    "quality/tools/python",
                    "tools/ci/tooling",
                ],
            ),
        ],
        file_blocks = {
            "//modules:python.bzl": [_UV_PROJECTS_BLOCK, _UV_LOCKS_BLOCK],
        },
        file_checks = {
            "//:MODULE.bazel": "//patches/aspect_rules_py:windows_venv.patch\n" +
                               "python_version = \"3.12\"",
            "//python/tests/fixtures/hello:pyproject.toml": "requires-python = \">=3.12\"",
            "//quality/tools/python:pyproject.toml": "requires-python = \">=3.12\"",
        },
    )
