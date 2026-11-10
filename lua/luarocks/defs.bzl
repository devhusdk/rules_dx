"""Updates luarocks.lock and luarocks.digests.json from luarocks.org."""

load("@rules_py//py:defs.bzl", "py_binary")

_SCRIPT = Label("//lua/luarocks:refresh.py")
_RESOLVER = Label("//lua/luarocks:resolve")

def luarocks_lock(name, lock = "luarocks.lock", digests = "luarocks.digests.json", **kwargs):
    """Moves rocks to their newest release and rewrites the digests file."""
    package = native.package_name()
    prefix = package + "/" if package else ""
    py_binary(
        name = name,
        srcs = [_SCRIPT],
        main = _SCRIPT,
        data = [_RESOLVER],
        args = [
            "--lock=" + prefix + lock,
            "--digests=" + prefix + digests,
            "--resolver=$(rootpath %s)" % _RESOLVER,
        ],
        **kwargs
    )
