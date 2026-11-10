"""Python runtime ownership gate for the coherent setup keep review."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core/modules:python.bzl", _MOD_ASPECT_PY = "RULES_PY_VERSION", _MOD_PY_HUB = "UV_HUB", _MOD_PY_LOCKS = "UV_LOCKS", _MOD_PY_PROJECTS = "UV_PROJECTS", _MOD_PY_REPIN = "UV_REPIN_DIRS", _MOD_PY_VERSION = "PYTHON_VERSION", _MOD_RULES_PY = "RULES_PYTHON_VERSION")
load("//core/modules:versions.bzl", _ASPECT_PY = "RULES_PY_VERSION", _PY = "PYTHON_VERSION", _RULES_PY = "RULES_PYTHON_VERSION")

_UV_PROJECTS_BLOCK = """UV_PROJECTS = [
    "//core/quality/tools/python:pyproject.toml",
]"""

_UV_LOCKS_BLOCK = """UV_LOCKS = [
    "//core/quality/tools/python:uv.lock",
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
                "2.0.0-alpha.11",
            ),
            expect_equal(
                "rules_python stays pinned beside aspect",
                _MOD_RULES_PY,
                "2.4.6",
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
                "wheel projects stay the quality tools",
                _MOD_PY_PROJECTS,
                [
                    "//core/quality/tools/python:pyproject.toml",
                ],
            ),
            expect_equal(
                "wheel locks stay the quality tools",
                _MOD_PY_LOCKS,
                [
                    "//core/quality/tools/python:uv.lock",
                ],
            ),
            expect_equal(
                "repin dirs stay the quality tools",
                _MOD_PY_REPIN,
                [
                    "core/quality/tools/python",
                ],
            ),
        ],
        file_blocks = {
            "//core/modules:python.bzl": [_UV_PROJECTS_BLOCK, _UV_LOCKS_BLOCK],
        },
        file_checks = {
            "//:MODULE.bazel": "python_version = \"3.12\"",
            "//python/tests/fixtures/hello:pyproject.toml": "requires-python = \">=3.12\"",
            "//core/quality/tools/python:pyproject.toml": "requires-python = \">=3.12\"",
        },
    )
