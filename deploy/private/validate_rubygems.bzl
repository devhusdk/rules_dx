"""Validators for the rubygems deploy rule."""

load(":validate.bzl", "ALNUM", "VERSION_CHARS", "charset_error")

_VALID_NAME_CHARS = ALNUM + "_.-"

_VALID_VERSION_CHARS = VERSION_CHARS

def rubygems_name_error(gem_name):
    """Validates one gem name value."""
    return charset_error(
        "rubygems_deploy",
        "gem name",
        gem_name,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'rubygems_demo')",
        "only [A-Za-z0-9_.-]",
    )

def rubygems_version_error(version):
    """Validates one gem version value."""
    return charset_error(
        "rubygems_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.0.0')",
        "only [A-Za-z0-9._-+]",
    )
