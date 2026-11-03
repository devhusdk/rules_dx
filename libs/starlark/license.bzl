"""Root-only license gate shared by restricted downloads."""

def license_tag(doc):
    """Returns the license tag class."""
    return tag_class(
        doc = doc,
        attrs = {
            "accept": attr.string_list(doc = "License names to accept.", mandatory = True),
        },
    )

def root_accepts(modules):
    """Returns the license names accepted by root modules."""
    names = []
    for module in modules:
        if not module.is_root:
            continue
        for tag in module.tags.license:
            names.extend(tag.accept)
    return names

def license_errors(prefix, known, names):
    """Returns one error per license name that is not the known one."""
    return [prefix + ": unknown license '" + name + "'; the only one is '" + known + "'" for name in names if name != known]

def unaccepted(prefix, what, url, extension, name):
    """Returns the error shown when the license is not accepted."""
    return prefix + ": the " + what + " (" + url + ") is not accepted; add " + extension + ".license(accept = [\"" + name + "\"]) to the root MODULE.bazel"

def accepted(mctx, prefix, known):
    """Returns whether the root module accepted the known license."""
    names = root_accepts(mctx.modules)
    errors = license_errors(prefix, known, names)
    if errors:
        fail("; ".join(errors))
    return known in names
