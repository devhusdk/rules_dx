"""platform_select: a select() keyed by platform names, OS names and groups."""

load(":names.bzl", "GROUPS", "OS_NAMES", "PLATFORMS")

visibility(["//platforms/..."])

_DEFAULT = "default"

def _key_error(key):
    if key == _DEFAULT or key in PLATFORMS or key in OS_NAMES or key in GROUPS:
        return ""
    return "platform_select: unknown key {}: use a platform name, one of {}, a group ({}) or default".format(
        key,
        ", ".join(OS_NAMES),
        ", ".join(sorted(GROUPS)),
    )

def resolve(branches):
    """Returns {platform name: value} for each name a key covers; the most specific key wins."""
    for key in branches:
        error = _key_error(key)
        if error:
            fail(error)
    resolved = {}
    for name in sorted(PLATFORMS):
        os = PLATFORMS[name].os
        if name in branches:
            resolved[name] = branches[name]
        elif os in OS_NAMES and os in branches:
            resolved[name] = branches[os]
        else:
            groups = [group for group in sorted(GROUPS) if group in branches and os in GROUPS[group]]
            if len(groups) > 1:
                fail("platform_select: keys {} all cover {}; add a {} or {} key".format(" and ".join(groups), name, name, os))
            if groups:
                resolved[name] = branches[groups[0]]
    return resolved

def config_label(name):
    """Returns the hub config_setting that matches one platform name."""
    return Label("@dx_platforms//:{}_config".format(name))

def platform_select(branches):
    """Returns a select() over platform names, OS names, groups and default; the most specific key wins."""
    resolved = resolve(branches)
    arms = {config_label(name): value for name, value in resolved.items()}
    if _DEFAULT in branches:
        arms["//conditions:default"] = branches[_DEFAULT]
    return select(arms)
