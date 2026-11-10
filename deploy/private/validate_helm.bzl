"""Validators for the helm deploy rule."""

load(":validate.bzl", "VERSION_CHARS", "charset_error")

def helm_version_error(version):
    """Validates one chart release version value."""
    return charset_error(
        "helm_chart_deploy",
        "version",
        version,
        VERSION_CHARS,
        "the Chart.yaml version (for example '1.2.3')",
        "only [A-Za-z0-9._+-]",
    )
