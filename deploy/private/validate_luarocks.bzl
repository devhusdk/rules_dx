"""Validators for the luarocks deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "charset_error", "suffix_error")

def luarocks_name_error(package):
    """Validates one rock package name value."""
    return charset_error(
        "luarocks_deploy",
        "package name",
        package,
        NAME_CHARS,
        "a non-empty name (for example 'luarocks_demo')",
        "only [A-Za-z0-9._-]",
    )

def luarocks_version_error(version):
    """Validates one rock version value."""
    return charset_error(
        "luarocks_deploy",
        "version",
        version,
        VERSION_CHARS,
        "a non-empty version (for example '0.1.0' or '0.1.0-1')",
        "only [A-Za-z0-9._-+]",
    )

def luarocks_rockspec_error(filename):
    """Validates one rockspec filename value."""
    return suffix_error(
        "luarocks_deploy",
        "rockspec",
        filename,
        ".rockspec",
        "a non-empty .rockspec filename",
        "a filename ending in .rockspec",
    )
