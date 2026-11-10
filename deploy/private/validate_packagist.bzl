"""Validators for the packagist deploy rule."""

load(":validate.bzl", "ALNUM", "only_chars")

_VALID_PART_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789_.-"

def packagist_name_error(package):
    """Validates one Packagist package name value, vendor/name in lowercase."""
    if type(package) != "string" or package == "":
        return ("packagist_deploy: invalid package '" + str(package) +
                "': want vendor/name (for example 'acme/demo')")
    parts = package.split("/")
    if len(parts) != 2 or parts[0] == "" or parts[1] == "":
        return ("packagist_deploy: invalid package '" + package +
                "': want vendor/name (for example 'acme/demo')")
    for part in parts:
        if not only_chars(part, _VALID_PART_CHARS):
            return ("packagist_deploy: invalid package '" + package +
                    "': want only [a-z0-9_.-] in each part")
    return ""

def packagist_repository_error(url):
    """Validates the repository URL registered on Packagist."""
    if type(url) != "string" or not url.startswith("https://") or len(url) == len("https://"):
        return ("packagist_deploy: invalid repository_url '" + str(url) +
                "': want an https:// URL")
    if not only_chars(url, ALNUM + ":/._-~"):
        return ("packagist_deploy: invalid repository_url '" + url +
                "': want only [A-Za-z0-9:/._~-]")
    return ""

def packagist_composer_error(filename):
    """Validates the composer file name."""
    if filename != "composer.json":
        return ("packagist_deploy: invalid composer_json '" + str(filename) +
                "': want a file named composer.json")
    return ""
