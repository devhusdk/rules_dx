"""Validators for the hex deploy rule."""

load(":validate.bzl", "ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = ALNUM + "_"

HEX_TOOLS = ["mix", "rebar3"]

def hex_name_error(package_name):
    """Validates one Hex package name value."""
    if type(package_name) != "string" or package_name == "":
        return ("hex_deploy: invalid package name '" + str(package_name) +
                "': want a non-empty name (for example 'hex_demo')")
    if not only_chars(package_name, _VALID_NAME_CHARS):
        return ("hex_deploy: invalid package name '" + package_name +
                "': want only [A-Za-z0-9_]")
    return ""

def hex_version_error(version):
    """Validates one Hex package version value."""
    if type(version) != "string" or version == "":
        return ("hex_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, VERSION_CHARS):
        return ("hex_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def hex_tool_error(tool):
    """Validates one Hex build tool value."""
    if tool not in HEX_TOOLS:
        return ("hex_deploy: invalid tool '" + str(tool) +
                "': want one of " + ", ".join(HEX_TOOLS))
    return ""
