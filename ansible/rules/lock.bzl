"""Updates an Ansible collections lock from Galaxy."""

load("@rules_py//py:defs.bzl", "py_binary")

_SCRIPT = Label("//ansible/tools:lock.py")

def ansible_lock(name, lock = "lock.json", **kwargs):
    """Adds a collection and its transitive dependencies to lock.json.

    Run it with `bazel run //pkg:name -- namespace.collection [version]`.
    It needs network access to Galaxy.

    Args:
        name: Target name.
        lock: The lock file in this package.
        **kwargs: Passed to the py_binary.
    """
    package = native.package_name()
    py_binary(
        name = name,
        srcs = [_SCRIPT],
        main = _SCRIPT,
        args = ["--lock=" + (package + "/" if package else "") + lock],
        **kwargs
    )
