"""Validators for the pub deploy rule."""

load(":validate.bzl", "LOWER_ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = LOWER_ALNUM + "_"

_VALID_VERSION_CHARS = VERSION_CHARS

PUB_DEFAULT_REGISTRY_URL = "https://pub.dev"

def pub_name_error(package_name):
    """Validates one pub package name value."""
    if type(package_name) != "string" or package_name == "":
        return ("pub_deploy: invalid package name '" + str(package_name) +
                "': want a non-empty name (for example 'pub_demo')")
    if not only_chars(package_name, _VALID_NAME_CHARS):
        return ("pub_deploy: invalid package name '" + package_name +
                "': want only [a-z0-9_]")
    return ""

def pub_version_error(version):
    """Validates one pub package version value."""
    if type(version) != "string" or version == "":
        return ("pub_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("pub_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def pub_registry_url_error(registry_url):
    """Validates one pub server URL value."""
    if type(registry_url) != "string" or registry_url == "":
        return ("pub_deploy: invalid registry_url '" + str(registry_url) +
                "': want a non-empty https URL (for example '" +
                PUB_DEFAULT_REGISTRY_URL + "')")
    if not registry_url.startswith("https://"):
        return ("pub_deploy: invalid registry_url '" + registry_url +
                "': want an https URL so publishes never go over plaintext")
    return ""
