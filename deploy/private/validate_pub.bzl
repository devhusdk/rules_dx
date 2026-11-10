"""Validators for the pub deploy rule."""

load(":validate.bzl", "LOWER_ALNUM", "VERSION_CHARS", "charset_error", "https_url_error")

_VALID_NAME_CHARS = LOWER_ALNUM + "_"

_VALID_VERSION_CHARS = VERSION_CHARS

PUB_DEFAULT_REGISTRY_URL = "https://pub.dev"

def pub_name_error(package_name):
    """Validates one pub package name value."""
    return charset_error(
        "pub_deploy",
        "package name",
        package_name,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'pub_demo')",
        "only [a-z0-9_]",
    )

def pub_version_error(version):
    """Validates one pub package version value."""
    return charset_error(
        "pub_deploy",
        "version",
        version,
        _VALID_VERSION_CHARS,
        "a non-empty version (for example '0.0.0')",
        "only [A-Za-z0-9._-+]",
    )

def pub_registry_url_error(registry_url):
    """Validates one pub server URL value."""
    return https_url_error(
        "pub_deploy",
        "registry_url",
        registry_url,
        "a non-empty https URL (for example '" + PUB_DEFAULT_REGISTRY_URL + "')",
        "an https URL so publishes never go over plaintext",
    )
