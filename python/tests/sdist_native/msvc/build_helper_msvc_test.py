"""The sdist build helper drives cl.exe through the environment, not through GNU-style compiler variables."""

import importlib.util
import os
import tempfile
from unittest import mock

MARKER = "__EXECROOT__"


def load_helper():
    spec = importlib.util.spec_from_file_location("build_helper", os.path.abspath(os.environ["BUILD_HELPER"]))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> None:
    helper = load_helper()
    ambient = {
        "CC": "gcc",
        "CXX": "g++",
        "CPP": "cpp",
        "AR": "ar",
        "LDSHARED": "gcc -shared",
        "LDCXXSHARED": "g++ -shared",
        "INCLUDE": MARKER + "/sdk/include",
    }
    with tempfile.TemporaryDirectory() as tmp, mock.patch.dict(os.environ, ambient):
        env = helper._compiler_env(tmp, MARKER, msvc=True)
    for key in ambient:
        if key != "INCLUDE":
            assert key not in env, key + " leaked into the cl.exe build"
    assert env["DISTUTILS_USE_SDK"] == "1"
    assert env["MSSdk"] == "1"
    assert env["INCLUDE"] == os.getcwd() + "/sdk/include", env["INCLUDE"]

    cwd = os.getcwd()
    root = helper._msvc_worktree_root("bazel-out/a/bin/one.whl")
    assert root.startswith(cwd), root
    assert len(root) <= len(cwd) + 20, root
    assert root == helper._msvc_worktree_root("bazel-out/a/bin/one.whl")
    assert root != helper._msvc_worktree_root("bazel-out/a/bin/two.whl")


if __name__ == "__main__":
    main()
