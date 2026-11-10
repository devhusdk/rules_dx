"""Rewrites zig.digests.json from the archives named in build.zig.zon."""

load("@rules_py//py:defs.bzl", "py_binary")

_SCRIPT = Label("//zig/private/tools:digests.py")

def zig_digests(name, zon = "build.zig.zon", digests = "zig.digests.json", **kwargs):
    """Re-hashes the archive dependencies of build.zig.zon into zig.digests.json."""
    package = native.package_name()
    prefix = package + "/" if package else ""
    py_binary(
        name = name,
        srcs = [_SCRIPT],
        main = _SCRIPT,
        args = [
            "--zon=" + prefix + zon,
            "--digests=" + prefix + digests,
        ],
        **kwargs
    )
