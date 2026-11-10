"""Validators for the luarocks deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

def luarocks_name_error(package):
    """Validates one rock package name value."""
    if type(package) != "string" or package == "":
        return ("luarocks_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'luarocks_demo')")
    if not only_chars(package, NAME_CHARS):
        return ("luarocks_deploy: invalid package name '" + package +
                "': want only [A-Za-z0-9._-]")
    return ""

def luarocks_version_error(version):
    """Validates one rock version value."""
    if type(version) != "string" or version == "":
        return ("luarocks_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.1.0' or '0.1.0-1')")
    if not only_chars(version, VERSION_CHARS):
        return ("luarocks_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def luarocks_rockspec_error(filename):
    """Validates one rockspec filename value."""
    if type(filename) != "string" or filename == "":
        return ("luarocks_deploy: invalid rockspec '" + str(filename) +
                "': want a non-empty .rockspec filename")
    if not filename.endswith(".rockspec"):
        return ("luarocks_deploy: invalid rockspec '" + filename +
                "': want a filename ending in .rockspec")
    return ""
