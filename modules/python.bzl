"""Python foundation pins plus uv projects."""

load(":versions.bzl", _PYTHON_VERSION = "PYTHON_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION", _RULES_PY_VERSION = "RULES_PY_VERSION")

RULES_PYTHON_VERSION = _RULES_PYTHON_VERSION
RULES_PY_VERSION = _RULES_PY_VERSION
PYTHON_VERSION = _PYTHON_VERSION

UV_HUB = "dx_pypi"
UV_PROJECTS = [
    "//python/tests/fixtures/hello:pyproject.toml",
    "//python/tests/fixtures/python-service:pyproject.toml",
    "//quality/tools/python:pyproject.toml",
]
UV_LOCKS = [
    "//python/tests/fixtures/hello:uv.lock",
    "//python/tests/fixtures/python-service:uv.lock",
    "//quality/tools/python:uv.lock",
]

UV_REPIN_DIRS = [
    "python/tests/fixtures/hello",
    "python/tests/fixtures/python-service",
    "quality/tools/python",
]
