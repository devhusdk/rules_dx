load("//libs/starlark:packaging.bzl", "only_chars")

_LETTERS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

_DIGITS = "0123456789"

def msix_named_version_errors(attr, version):
    """Returns why the value of a version attribute is not four numbers up to 65535."""
    parts = version.split(".")
    if len(parts) != 4:
        return [attr + " '" + version + "' must have four parts, as in 1.2.3.0"]
    for part in parts:
        if part == "" or not only_chars(part, _DIGITS) or len(part) > 5 or int(part) > 65535:
            return [attr + " '" + version + "' parts must be numbers from 0 to 65535"]
    return []

def msix_version_errors(version):
    """Returns why a package version is not four numbers up to 65535."""
    return msix_named_version_errors("version", version)

def msix_identity_errors(name):
    """Returns why a package identity name is invalid."""
    if len(name) < 3 or len(name) > 50 or not only_chars(name, _LETTERS + _DIGITS + ".-"):
        return ["identity_name '" + name + "' must be 3 to 50 letters, digits, dots or dashes"]
    return []

def msix_application_errors(application_id):
    """Returns why an application id is invalid."""
    parts = application_id.split(".")
    for part in parts:
        if part == "" or part[0] not in _LETTERS.elems() or not only_chars(part, _LETTERS + _DIGITS):
            return ["application_id '" + application_id + "' must be dot-separated words that start with a letter"]
    if len(application_id) > 64:
        return ["application_id '" + application_id + "' must be at most 64 characters"]
    return []

def msix_publisher_errors(publisher):
    """Returns why a publisher is not a distinguished name."""
    if not publisher.startswith("CN=") or len(publisher) > 8192:
        return ["publisher '" + publisher + "' must be a distinguished name that starts with CN="]
    return []

def msix_alias_errors(alias):
    """Returns why an execution alias is invalid."""
    if alias == "":
        return []
    if not alias.endswith(".exe") or len(alias) < 5 or not only_chars(alias, _LETTERS + _DIGITS + ".-_"):
        return ["alias '" + alias + "' must be a file name that ends with .exe"]
    return []
