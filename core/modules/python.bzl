"""Python foundation pins plus uv projects."""

load(":lock_registry.bzl", "LOCK_REGISTRY")
load(":versions.bzl", _PYTHON_VERSION = "PYTHON_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION", _RULES_PY_VERSION = "RULES_PY_VERSION")

RULES_PYTHON_VERSION = _RULES_PYTHON_VERSION
RULES_PY_VERSION = _RULES_PY_VERSION
PYTHON_VERSION = _PYTHON_VERSION

UV_HUB = "dx_pypi"
UV_PROJECTS = [
    "//core/quality/tools/python:" + LOCK_REGISTRY["uv"]["manifest"],
]
UV_LOCKS = [
    "//core/quality/tools/python:" + LOCK_REGISTRY["uv"]["lock"],
]

UV_REPIN_DIRS = [
    "core/quality/tools/python",
]
