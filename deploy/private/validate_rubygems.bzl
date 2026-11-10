"""Validators for the rubygems deploy rule."""

load(":validate.bzl", "ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = ALNUM + "_.-"

_VALID_VERSION_CHARS = VERSION_CHARS

def rubygems_name_error(gem_name):
    """Validates one gem name value."""
    if type(gem_name) != "string" or gem_name == "":
        return ("rubygems_deploy: invalid gem name '" + str(gem_name) +
                "': want a non-empty name (for example 'rubygems_demo')")
    if not only_chars(gem_name, _VALID_NAME_CHARS):
        return ("rubygems_deploy: invalid gem name '" + gem_name +
                "': want only [A-Za-z0-9_.-]")
    return ""

def rubygems_version_error(version):
    """Validates one gem version value."""
    if type(version) != "string" or version == "":
        return ("rubygems_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("rubygems_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""
