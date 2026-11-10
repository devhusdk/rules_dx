"""Python foundation pins plus uv projects."""

load(":versions.bzl", _PYTHON_VERSION = "PYTHON_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION", _RULES_PY_VERSION = "RULES_PY_VERSION")

RULES_PYTHON_VERSION = _RULES_PYTHON_VERSION
RULES_PY_VERSION = _RULES_PY_VERSION
PYTHON_VERSION = _PYTHON_VERSION

UV_HUB = "dx_pypi"
UV_PROJECTS = [
    "//quality/tools/python:pyproject.toml",
]
UV_LOCKS = [
    "//quality/tools/python:uv.lock",
]

UV_REPIN_DIRS = [
    "quality/tools/python",
]
