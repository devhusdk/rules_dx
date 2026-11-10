"""Validators for the helm deploy rule."""

load(":validate.bzl", "VERSION_CHARS", "only_chars")

def helm_version_error(version):
    """Validates one chart release version value."""
    if type(version) != "string" or version == "":
        return ("helm_chart_deploy: invalid version '" + str(version) +
                "': want the Chart.yaml version (for example '1.2.3')")
    if not only_chars(version, VERSION_CHARS):
        return ("helm_chart_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._+-]")
    return ""
