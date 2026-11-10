"""Validators for the conan deploy rule."""

load(":validate.bzl", "LOWER_ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = LOWER_ALNUM + "_.-"

_VALID_REMOTE_CHARS = LOWER_ALNUM + "_-"

def conan_name_error(package):
    """Validates one Conan package name value."""
    if type(package) != "string" or package == "":
        return ("conan_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'conan_demo')")
    if not only_chars(package, _VALID_NAME_CHARS):
        return ("conan_deploy: invalid package name '" + package +
                "': want only [a-z0-9_.-]")
    return ""

def conan_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("conan_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.1.0')")
    if not only_chars(version, VERSION_CHARS):
        return ("conan_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def conan_remote_error(remote):
    """Validates one Conan remote name value."""
    if type(remote) != "string" or remote == "":
        return ("conan_deploy: invalid remote '" + str(remote) +
                "': want a non-empty remote name (for example 'center')")
    if not only_chars(remote, _VALID_REMOTE_CHARS):
        return ("conan_deploy: invalid remote '" + remote +
                "': want only [a-z0-9_-]")
    return ""

def conan_remote_url_error(url):
    """Validates one Conan remote URL value."""
    if type(url) != "string" or not url.startswith("https://"):
        return ("conan_deploy: invalid remote_url '" + str(url) +
                "': want an https:// URL")
    if " " in url:
        return ("conan_deploy: invalid remote_url '" + url +
                "': want no spaces")
    return ""
