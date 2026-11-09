"""Validators for the opam deploy rule."""

load(":validate.bzl", "ALNUM", "only_chars")

def opam_name_error(package):
    """Validates one opam package name."""
    if type(package) != "string" or package == "":
        return ("opam_deploy: invalid package '" + str(package) +
                "': want a non-empty name (for example 'demo')")
    if not only_chars(package, ALNUM + "_+-"):
        return ("opam_deploy: invalid package '" + package +
                "': want only [A-Za-z0-9_+-]")
    return ""

def opam_version_error(version):
    """Validates one opam package version."""
    if type(version) != "string" or version == "":
        return ("opam_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '1.2.0')")
    if not only_chars(version, ALNUM + "._+~-"):
        return ("opam_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._+~-]")
    return ""

def opam_file_error(filename):
    """Validates the opam file name."""
    if filename != "opam":
        return ("opam_deploy: invalid opam_file '" + str(filename) +
                "': want a file named opam")
    return ""
