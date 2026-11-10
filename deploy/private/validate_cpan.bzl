"""Validators for the cpan deploy rule."""

load(":validate.bzl", "ALNUM", "only_chars")

_VALID_NAME_CHARS = ALNUM + "-"

_VALID_VERSION_CHARS = "0123456789._"

def cpan_name_error(package):
    """Validates one CPAN distribution name value."""
    if type(package) != "string" or package == "":
        return ("cpan_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'Cpan-Demo')")
    if not only_chars(package, _VALID_NAME_CHARS):
        return ("cpan_deploy: invalid package name '" + package +
                "': want only [A-Za-z0-9-]")
    return ""

def cpan_version_error(version):
    """Validates one distribution version value."""
    if type(version) != "string" or version == "":
        return ("cpan_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.01')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("cpan_deploy: invalid version '" + version +
                "': want only [0-9._]")
    return ""

def cpan_dist_error(filename):
    """Validates one dist filename value."""
    if type(filename) != "string" or filename == "":
        return ("cpan_deploy: invalid dist '" + str(filename) +
                "': want a non-empty .tar.gz filename")
    if not filename.endswith(".tar.gz"):
        return ("cpan_deploy: invalid dist '" + filename +
                "': want a filename ending in .tar.gz")
    return ""
