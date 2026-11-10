"""Validators for the pypi deploy rule."""

load(":validate.bzl", "NAME_CHARS", "charset_error", "https_url_error", "suffix_error")

_VALID_NAME_CHARS = NAME_CHARS

PYPI_DEFAULT_REGISTRY_URL = "https://upload.pypi.org/legacy/"

def pypi_name_error(dist_name):
    """Validates one distribution name value."""
    return charset_error(
        "pypi_deploy",
        "distribution name",
        dist_name,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'pypi_demo')",
        "only [A-Za-z0-9._-]",
    )

def pypi_registry_url_error(registry_url):
    """Validates one PyPI repository URL value."""
    return https_url_error(
        "pypi_deploy",
        "registry_url",
        registry_url,
        "a non-empty https URL (for example '" + PYPI_DEFAULT_REGISTRY_URL + "')",
        "an https URL so uploads never go over plaintext",
    )

def pypi_wheel_error(filename):
    """Validates one wheel filename value."""
    return suffix_error(
        "pypi_deploy",
        "wheel",
        filename,
        ".whl",
        "a non-empty .whl filename",
        "a filename ending in .whl",
    )
