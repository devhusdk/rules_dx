"""Rewrites the sha256 pins of fpm.lock from the package archives."""

load("@rules_py//py:defs.bzl", "py_binary")

_SCRIPT = Label("//fortran/private/tools:refresh.py")

def fpm_lock(name, lock = "fpm.lock", **kwargs):
    """Re-hashes the archives of fpm.lock."""
    package = native.package_name()
    prefix = package + "/" if package else ""
    py_binary(
        name = name,
        srcs = [_SCRIPT],
        main = _SCRIPT,
        args = ["--lock=" + prefix + lock],
        **kwargs
    )
