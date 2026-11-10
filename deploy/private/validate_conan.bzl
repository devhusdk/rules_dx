"""Validators for the conan deploy rule."""

load(":validate.bzl", "LOWER_ALNUM", "VERSION_CHARS", "charset_error")

_VALID_NAME_CHARS = LOWER_ALNUM + "_.-"

_VALID_REMOTE_CHARS = LOWER_ALNUM + "_-"

def conan_name_error(package):
    """Validates one Conan package name value."""
    return charset_error(
        "conan_deploy",
        "package name",
        package,
        _VALID_NAME_CHARS,
        "a non-empty name (for example 'conan_demo')",
        "only [a-z0-9_.-]",
    )

def conan_version_error(version):
    """Validates one package version value."""
    return charset_error(
        "conan_deploy",
        "version",
        version,
        VERSION_CHARS,
        "a non-empty version (for example '0.1.0')",
        "only [A-Za-z0-9._-+]",
    )

def conan_remote_error(remote):
    """Validates one Conan remote name value."""
    return charset_error(
        "conan_deploy",
        "remote",
        remote,
        _VALID_REMOTE_CHARS,
        "a non-empty remote name (for example 'center')",
        "only [a-z0-9_-]",
    )

def conan_remote_url_error(url):
    """Validates one Conan remote URL value."""
    if type(url) != "string" or not url.startswith("https://"):
        return ("conan_deploy: invalid remote_url '" + str(url) +
                "': want an https:// URL")
    if " " in url:
        return ("conan_deploy: invalid remote_url '" + url +
                "': want no spaces")
    return ""
