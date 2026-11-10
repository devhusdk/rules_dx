"""Validators for the hex deploy rule."""

load(":validate.bzl", "ALNUM", "VERSION_CHARS", "charset_error")

_VALID_NAME_CHARS = ALNUM + "_"

HEX_TOOLS = ["mix", "rebar3"]

def hex_name_error(package_name):
    """Validates one Hex package name value."""
    return charset_error(
        "hex_deploy",
        "package name",
        package_name,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'hex_demo')",
        "only [A-Za-z0-9_]",
    )

def hex_version_error(version):
    """Validates one Hex package version value."""
    return charset_error(
        "hex_deploy",
        "version",
        version,
        VERSION_CHARS,
        "a non-empty version (for example '0.0.0')",
        "only [A-Za-z0-9._-+]",
    )

def hex_tool_error(tool):
    """Validates one Hex build tool value."""
    if tool not in HEX_TOOLS:
        return ("hex_deploy: invalid tool '" + str(tool) +
                "': want one of " + ", ".join(HEX_TOOLS))
    return ""
