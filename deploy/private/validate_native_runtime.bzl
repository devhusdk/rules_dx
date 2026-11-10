"""Validators for the native_runtime deploy rule."""

REDISTRIBUTIONS = ["app_local", "os_supplied", "build_only"]
def native_runtime_error(version, source, redistribution, file_count, has_notice, provides_count = 0):
    """Returns the reason a native runtime declaration is invalid, or empty."""
    if version == "":
        return "native_runtime: version must not be empty"
    if source == "":
        return "native_runtime: source must not be empty"
    if redistribution not in REDISTRIBUTIONS:
        return ("native_runtime: unsupported redistribution '" + str(redistribution) +
                "': want one of " + ", ".join(REDISTRIBUTIONS))
    if redistribution == "app_local" and file_count == 0:
        return "native_runtime: app_local needs at least one file in srcs"
    if redistribution == "app_local" and not has_notice:
        return "native_runtime: app_local needs a notice"
    if redistribution == "os_supplied" and file_count != 0:
        return "native_runtime: os_supplied must not ship files"
    if provides_count != 0 and redistribution != "os_supplied":
        return "native_runtime: only os_supplied may set provides"
    return ""
