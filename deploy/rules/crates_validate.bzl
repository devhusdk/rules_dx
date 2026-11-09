load(":validate.bzl", "ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = ALNUM + "_-"

_VALID_VERSION_CHARS = VERSION_CHARS

def crates_name_error(crate_name):
    """Validates one crate name value."""
    if type(crate_name) != "string" or crate_name == "":
        return ("crates_deploy: invalid crate name '" + str(crate_name) +
                "': want a non-empty name (for example 'crates_demo')")
    if not only_chars(crate_name, _VALID_NAME_CHARS):
        return ("crates_deploy: invalid crate name '" + crate_name +
                "': want only [A-Za-z0-9_-]")
    return ""

def crates_version_error(version):
    """Validates one crate version value."""
    if type(version) != "string" or version == "":
        return ("crates_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("crates_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""
