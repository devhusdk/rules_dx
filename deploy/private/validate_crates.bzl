"""Validators for the crates deploy rule."""

load(":validate.bzl", "ALNUM", "VERSION_CHARS", "charset_error")

_VALID_NAME_CHARS = ALNUM + "_-"

_VALID_VERSION_CHARS = VERSION_CHARS

def crates_name_error(crate_name):
    """Validates one crate name value."""
    return charset_error(
        "crates_deploy",
        "crate name",
        crate_name,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'crates_demo')",
        "only [A-Za-z0-9_-]",
    )

def crates_version_error(version):
    """Validates one crate version value."""
    return charset_error(
        "crates_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.0.0')",
        "only [A-Za-z0-9._-+]",
    )
