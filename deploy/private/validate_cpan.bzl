"""Validators for the cpan deploy rule."""

load(":validate.bzl", "ALNUM", "charset_error", "suffix_error")

_VALID_NAME_CHARS = ALNUM + "-"

_VALID_VERSION_CHARS = "0123456789._"

def cpan_name_error(package):
    """Validates one CPAN distribution name value."""
    return charset_error(
        "cpan_deploy",
        "package name",
        package,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'Cpan-Demo')",
        "only [A-Za-z0-9-]",
    )

def cpan_version_error(version):
    """Validates one distribution version value."""
    return charset_error(
        "cpan_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.01')",
        "only [0-9._]",
    )

def cpan_dist_error(filename):
    """Validates one dist filename value."""
    return suffix_error(
        "cpan_deploy",
        "dist",
        filename,
        ".tar.gz",
        "a non-empty .tar.gz filename",
        "a filename ending in .tar.gz",
    )
