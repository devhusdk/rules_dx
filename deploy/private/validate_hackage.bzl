"""Validators for the hackage deploy rule."""

load(":validate.bzl", "ALNUM", "charset_error", "suffix_error")

_VALID_NAME_CHARS = ALNUM + "-"

_VALID_VERSION_CHARS = "0123456789."

def hackage_name_error(package):
    """Validates one Hackage package name value."""
    return charset_error(
        "hackage_deploy",
        "package name",
        package,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'hackage_demo')",
        "only [A-Za-z0-9-]",
    )

def hackage_version_error(version):
    """Validates one package version value."""
    return charset_error(
        "hackage_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.1.0')",
        "only [0-9.]",
    )

def hackage_sdist_error(filename):
    """Validates one sdist filename value."""
    return suffix_error(
        "hackage_deploy",
        "sdist",
        filename,
        ".tar.gz",
        "a non-empty .tar.gz filename",
        "a filename ending in .tar.gz",
    )
