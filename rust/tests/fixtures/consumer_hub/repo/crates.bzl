"""Dependency labels this consumer hub publishes."""

def crate_deps(deps, package_name = None):
    """Returns the labels of the named crates in this hub."""
    if package_name == None:
        fail("crate_deps needs package_name")
    return ["@consumer_hub//:" + dep.replace("-", "_") for dep in deps]
