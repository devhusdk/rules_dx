"""Python runtime ownership gate for the coherent setup keep review."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//modules:python.bzl", _MOD_ASPECT_PY = "ASPECT_RULES_PY_VERSION", _MOD_PY_HUB = "UV_HUB", _MOD_PY_LOCKS = "UV_LOCKS", _MOD_PY_PROJECTS = "UV_PROJECTS", _MOD_PY_REPIN = "UV_REPIN_DIRS", _MOD_PY_VERSION = "PYTHON_VERSION", _MOD_RULES_PY = "RULES_PYTHON_VERSION")
load("//modules:versions.bzl", _ASPECT_PY = "ASPECT_RULES_PY_VERSION", _PY = "PYTHON_VERSION", _RULES_PY = "RULES_PYTHON_VERSION")

def python_runtime_unit_tests(name):
    """Instantiates the Python runtime ownership keep checks."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "aspect backend stays pinned",
                _MOD_ASPECT_PY,
                "2.0.0-alpha.6",
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
                "wheel hub stays pypi",
                _MOD_PY_HUB,
                "pypi",
            ),
            expect_equal(
                "wheel projects stay hello plus tools",
                _MOD_PY_PROJECTS,
                [
                    "//python/tests/fixtures/hello:pyproject.toml",
                    "//quality/tools/python:pyproject.toml",
                ],
            ),
            expect_equal(
                "wheel locks stay hello plus tools",
                _MOD_PY_LOCKS,
                [
                    "//python/tests/fixtures/hello:uv.lock",
                    "//quality/tools/python:uv.lock",
                ],
            ),
            expect_equal(
                "repin dirs stay hello plus tools",
                _MOD_PY_REPIN,
                [
                    "python/tests/fixtures/hello",
                    "quality/tools/python",
                ],
            ),
        ],
        file_checks = {
            "//:MODULE.bazel": "//patches:aspect_rules_py_site_packages.patch\n" +
                               "//patches:aspect_rules_py_venv_interpreter.patch\n" +
                               "python_version = \"3.12\"",
            "//python/tests/fixtures/hello:pyproject.toml": "requires-python = \">=3.12\"",
            "//quality/tools/python:pyproject.toml": "requires-python = \">=3.12\"",
        },
    )
