"""Validators for the nuget deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "charset_error", "https_url_error", "suffix_error")

_VALID_ID_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

NUGET_DEFAULT_REGISTRY_URL = "https://api.nuget.org/v3/index.json"

def nuget_id_error(package_id):
    """Validates one package id value."""
    return charset_error(
        "nuget_deploy",
        "package id",
        package_id,
        _VALID_ID_CHARS,
        "a non-empty id (for example 'nuget_demo')",
        "only [A-Za-z0-9._-]",
    )

def nuget_version_error(version):
    """Validates one package version value."""
    return charset_error(
        "nuget_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.0.0')",
        "only [A-Za-z0-9._-+]",
    )

def nuget_nupkg_error(filename):
    """Validates one nupkg filename value."""
    return suffix_error(
        "nuget_deploy",
        "nupkg",
        filename,
        ".nupkg",
        "a non-empty .nupkg filename",
        "a filename ending in .nupkg",
    )

def nuget_registry_url_error(registry_url):
    """Validates one NuGet registry URL value."""
    return https_url_error(
        "nuget_deploy",
        "registry_url",
        registry_url,
        "a non-empty https URL (for example '" + NUGET_DEFAULT_REGISTRY_URL + "')",
        "an https URL so pushes never go over plaintext",
    )
