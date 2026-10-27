"""Python foundation pins plus uv projects."""

load(":versions.bzl", _ASPECT_RULES_PY_VERSION = "FLOOR_ASPECT_RULES_PY", _PYTHON_VERSION = "PYTHON_VERSION", _RULES_PYTHON_VERSION = "FLOOR_RULES_PYTHON")

RULES_PYTHON_VERSION = _RULES_PYTHON_VERSION
ASPECT_RULES_PY_VERSION = _ASPECT_RULES_PY_VERSION
PYTHON_VERSION = _PYTHON_VERSION

UV_HUB = "dx_pypi"
UV_PROJECTS = [
    "//examples/python-service:pyproject.toml",
    "//python/tests/fixtures/hello:pyproject.toml",
    "//quality/tools/python:pyproject.toml",
    "//tools/ci/tooling:pyproject.toml",
]
UV_LOCKS = [
    "//examples/python-service:uv.lock",
    "//python/tests/fixtures/hello:uv.lock",
    "//quality/tools/python:uv.lock",
    "//tools/ci/tooling:uv.lock",
]

UV_REPIN_DIRS = [
    "examples/python-service",
    "python/tests/fixtures/hello",
    "quality/tools/python",
    "tools/ci/tooling",
]
