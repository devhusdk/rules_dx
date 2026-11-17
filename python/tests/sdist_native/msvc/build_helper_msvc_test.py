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

    check_clang_cl(helper)


def check_clang_cl(helper) -> None:
    assert helper._is_clang_cl("external/llvm/bin/clang-cl.exe")
    assert not helper._is_clang_cl("external/llvm/bin/clang.exe")
    cwd = os.getcwd()
    with tempfile.TemporaryDirectory() as tmp:
        os.chdir(tmp)
        try:
            for directory in ("llvm/bin", "sdk/ucrt", "libcxx", "sdk/lib", "resource/lib/windows"):
                os.makedirs(directory)
            for tool in ("clang-cl.exe", "lld-link.exe", "llvm-lib.exe"):
                open(os.path.join("llvm/bin", tool), "w").close()
            env = {
                "CC": "llvm/bin/clang-cl.exe",
                "AR": os.path.abspath("llvm/bin/llvm-lib.exe"),
                "PATH": "/usr/bin",
                "CFLAGS": "--target=x86_64-pc-windows-msvc /imsvcsdk/ucrt /MD -fPIC",
                "CXXFLAGS": "/clang:-isystem /clang:libcxx /std:c++17",
                "LDSHAREDFLAGS": "/clang:-Xlinker /clang:/LIBPATH:sdk/lib /clang:-fuse-ld=lld -resource-dir=" + os.path.abspath("resource"),
            }
            helper._clang_cl_env(env, os.path.join(tmp, "short"))
            bin_dir = os.path.join(tmp, "short", "clang_cl_bin")
            assert env["PATH"] == bin_dir + os.pathsep + "/usr/bin", env["PATH"]
            for tool in ("cl.exe", "link.exe", "lib.exe"):
                assert os.path.isfile(os.path.join(bin_dir, tool)), tool
            abs_path = os.path.abspath
            expected_cl = '--target=x86_64-pc-windows-msvc "/imsvc{}" /MD /clang:-isystem "/clang:{}"'.format(abs_path("sdk/ucrt"), abs_path("libcxx"))
            assert env["CL"] == expected_cl, env["CL"]
            expected_link = '"/LIBPATH:{}" /LIBPATH:{}'.format(abs_path("sdk/lib"), abs_path("resource/lib/windows"))
            assert env["_LINK_"] == expected_link, env["_LINK_"]
        finally:
            os.chdir(cwd)


if __name__ == "__main__":
    main()
