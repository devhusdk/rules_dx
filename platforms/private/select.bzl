"""platform_select: a select() keyed by platform names, OS names and groups."""

load(":names.bzl", "GROUPS", "OS_CONSTRAINTS", "OS_NAMES", "PLATFORMS")

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
    """Returns struct(names, oses): the values of the full-name keys, and the value each OS takes from an OS or group key."""
    for key in branches:
        error = _key_error(key)
        if error:
            fail(error)
    names = {name: branches[name] for name in sorted(PLATFORMS) if name in branches}
    oses = {}
    for os in sorted(OS_CONSTRAINTS):
        if os in OS_NAMES and os in branches:
            oses[os] = branches[os]
            continue
        groups = [group for group in sorted(GROUPS) if group in branches and os in GROUPS[group]]
        if len(groups) > 1:
            fail("platform_select: keys {} all cover {}; add a {} key".format(" and ".join(groups), os, os))
        if groups:
            oses[os] = branches[groups[0]]

    for os in list(oses):
        if all([name in names for name in PLATFORMS if PLATFORMS[name].os == os]):
            oses.pop(os)
    return struct(names = names, oses = oses)

def config_label(name):
    """Returns the hub config_setting that matches one platform name."""
    return Label("@dx_platforms//:{}_config".format(name))

def os_label(os):
    """Returns the config_setting that matches every platform of one OS."""
    return Label("//platforms:os_" + os)

def platform_select(branches):
    """Returns a select() over platform names, OS names, groups and default; the most specific key wins."""
    resolved = resolve(branches)
    arms = {config_label(name): value for name, value in resolved.names.items()}
    arms.update({os_label(os): value for os, value in resolved.oses.items()})
    if _DEFAULT in branches:
        arms["//conditions:default"] = branches[_DEFAULT]
    return select(arms)
