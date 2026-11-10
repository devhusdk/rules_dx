"""Validators for the pypi deploy rule."""

load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_NAME_CHARS = NAME_CHARS

PYPI_DEFAULT_REGISTRY_URL = "https://upload.pypi.org/legacy/"

def pypi_name_error(dist_name):
    """Validates one distribution name value."""
    if type(dist_name) != "string" or dist_name == "":
        return ("pypi_deploy: invalid distribution name '" + str(dist_name) +
                "': want a non-empty name (for example 'pypi_demo')")
    if not only_chars(dist_name, _VALID_NAME_CHARS):
        return ("pypi_deploy: invalid distribution name '" + dist_name +
                "': want only [A-Za-z0-9._-]")
    return ""

def pypi_registry_url_error(registry_url):
    """Validates one PyPI repository URL value."""
    if type(registry_url) != "string" or registry_url == "":
        return ("pypi_deploy: invalid registry_url '" + str(registry_url) +
                "': want a non-empty https URL (for example '" +
                PYPI_DEFAULT_REGISTRY_URL + "')")
    if not registry_url.startswith("https://"):
        return ("pypi_deploy: invalid registry_url '" + registry_url +
                "': want an https URL so uploads never go over plaintext")
    return ""

def pypi_wheel_error(filename):
    """Validates one wheel filename value."""
    if type(filename) != "string" or filename == "":
        return ("pypi_deploy: invalid wheel '" + str(filename) +
                "': want a non-empty .whl filename")
    if not filename.endswith(".whl"):
        return ("pypi_deploy: invalid wheel '" + filename +
                "': want a filename ending in .whl")
    return ""
