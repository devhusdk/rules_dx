"""Validators for the hackage deploy rule."""

load(":validate.bzl", "ALNUM", "only_chars")

_VALID_NAME_CHARS = ALNUM + "-"

_VALID_VERSION_CHARS = "0123456789."

def hackage_name_error(package):
    """Validates one Hackage package name value."""
    if type(package) != "string" or package == "":
        return ("hackage_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'hackage_demo')")
    if not only_chars(package, _VALID_NAME_CHARS):
        return ("hackage_deploy: invalid package name '" + package +
                "': want only [A-Za-z0-9-]")
    return ""

def hackage_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("hackage_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.1.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("hackage_deploy: invalid version '" + version +
                "': want only [0-9.]")
    return ""

def hackage_sdist_error(filename):
    """Validates one sdist filename value."""
    if type(filename) != "string" or filename == "":
        return ("hackage_deploy: invalid sdist '" + str(filename) +
                "': want a non-empty .tar.gz filename")
    if not filename.endswith(".tar.gz"):
        return ("hackage_deploy: invalid sdist '" + filename +
                "': want a filename ending in .tar.gz")
    return ""
