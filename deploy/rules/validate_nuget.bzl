"""Validators for the nuget deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

_VALID_ID_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

NUGET_DEFAULT_REGISTRY_URL = "https://api.nuget.org/v3/index.json"

def nuget_id_error(package_id):
    """Validates one package id value."""
    if type(package_id) != "string" or package_id == "":
        return ("nuget_deploy: invalid package id '" + str(package_id) +
                "': want a non-empty id (for example 'nuget_demo')")
    if not only_chars(package_id, _VALID_ID_CHARS):
        return ("nuget_deploy: invalid package id '" + package_id +
                "': want only [A-Za-z0-9._-]")
    return ""

def nuget_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("nuget_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("nuget_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def nuget_nupkg_error(filename):
    """Validates one nupkg filename value."""
    if type(filename) != "string" or filename == "":
        return ("nuget_deploy: invalid nupkg '" + str(filename) +
                "': want a non-empty .nupkg filename")
    if not filename.endswith(".nupkg"):
        return ("nuget_deploy: invalid nupkg '" + filename +
                "': want a filename ending in .nupkg")
    return ""

def nuget_registry_url_error(registry_url):
    """Validates one NuGet registry URL value."""
    if type(registry_url) != "string" or registry_url == "":
        return ("nuget_deploy: invalid registry_url '" + str(registry_url) +
                "': want a non-empty https URL (for example '" +
                NUGET_DEFAULT_REGISTRY_URL + "')")
    if not registry_url.startswith("https://"):
        return ("nuget_deploy: invalid registry_url '" + registry_url +
                "': want an https URL so pushes never go over plaintext")
    return ""
