"""Python foundation pins plus uv projects."""

load(":versions.bzl", _ASPECT_RULES_PY_VERSION = "ASPECT_RULES_PY_VERSION", _PYTHON_VERSION = "PYTHON_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION")

RULES_PYTHON_VERSION = _RULES_PYTHON_VERSION
ASPECT_RULES_PY_VERSION = _ASPECT_RULES_PY_VERSION
PYTHON_VERSION = _PYTHON_VERSION

UV_HUB = "pypi"
UV_PROJECTS = [
    "//python/tests/fixtures/hello:pyproject.toml",
    "//quality/tools/python:pyproject.toml",
]
UV_LOCKS = [
    "//python/tests/fixtures/hello:uv.lock",
    "//quality/tools/python:uv.lock",
]

UV_REPIN_DIRS = [
    "python/tests/fixtures/hello",
    "quality/tools/python",
]
