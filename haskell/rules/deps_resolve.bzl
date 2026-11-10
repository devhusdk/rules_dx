"""Resolve a cabal project into cabal.project.freeze and a Hackage sidecar."""

load("@rules_py//py:defs.bzl", "py_binary", "py_test")
load("//haskell/rules:defs.bzl", "HASKELL_COMPATIBLE_WITH")
load("//libs/starlark:wrapper.bzl", "dx_language_kwargs")

_SCRIPT = Label("//haskell/tools:deps_resolve.py")

_CABAL = Label("//haskell/toolchain:cabal")

_GHC = Label("//haskell/toolchain:ghc_exe")

_GHC_FILES = Label("//haskell/toolchain:ghc_files")

def _input_args(inputs):
    return ["--input=$(rlocationpath {})".format(src) for src in inputs]

def haskell_deps_resolve(name, srcs, target = "all", stack_lock = "", **kwargs):
    """Writes cabal.project.freeze and haskell_deps.json next to the cabal project in this package."""
    package = native.package_name()
    py_binary(
        name = name,
        srcs = [_SCRIPT],
        main = _SCRIPT,
        args = [
            "write",
            "--package=" + package,
            "--target=" + target,
            "--cabal=$(rlocationpath {})".format(_CABAL),
            "--ghc=$(rlocationpath {})".format(_GHC),
        ] + (["--stack-lock=" + stack_lock] if stack_lock else []) + _input_args(srcs),
        data = srcs + [_CABAL, _GHC, _GHC_FILES],
        **dx_language_kwargs(kwargs, [], HASKELL_COMPATIBLE_WITH)
    )

def haskell_deps_resolve_test(name, srcs, freeze = "cabal.project.freeze", sidecar = "haskell_deps.json", **kwargs):
    """Fails when the freeze file or the sidecar is stale against the cabal project."""
    package = native.package_name()
    py_test(
        name = name,
        srcs = [_SCRIPT],
        main = _SCRIPT,
        args = [
            "check",
            "--package=" + package,
            "--label=//{}:{}".format(package, name.removesuffix("_test")),
            "--freeze=$(rlocationpath {})".format(freeze),
            "--sidecar=$(rlocationpath {})".format(sidecar),
        ] + _input_args(srcs),
        data = srcs + [freeze, sidecar],
        **kwargs
    )
