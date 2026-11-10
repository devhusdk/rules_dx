"""Validators for the npm deploy rule."""

load(":validate.bzl", "NAME_CHARS", "charset_error")

_VALID_TAG_CHARS = NAME_CHARS

_VALID_PACKAGE_CHARS = _VALID_TAG_CHARS + "@/"

def npm_tag_error(tag):
    """Validates one npm dist-tag value."""
    return charset_error(
        "npm_deploy",
        "dist_tag",
        tag,
        _VALID_TAG_CHARS,
        "a non-empty tag (for example 'latest')",
        "only [A-Za-z0-9._-]",
    )

def npm_package_error(package):
    """Validates one npm package name value."""
    return charset_error(
        "npm_deploy",
        "package",
        package,
        _VALID_PACKAGE_CHARS,
        "a non-empty package name (for example 'npm-demo')",
        "only [A-Za-z0-9@/_.-]",
    )

def npm_registry_url_error(registry_url):
    """Validates one npm registry URL value."""
    if type(registry_url) != "string" or not registry_url.startswith("https://"):
        return ("npm_deploy: invalid registry_url '" + str(registry_url) +
                "': want an https:// URL " +
                "(for example 'https://registry.npmjs.org')")
    return ""

def npm_access_error(package, access):
    """Validates one npm access value against the package scope."""
    if access not in ["public", "restricted"]:
        return ("npm_deploy: invalid access '" + str(access) +
                "': want 'public' or 'restricted'")
    scoped = package.startswith("@")
    if scoped and (package.count("/") != 1 or package.startswith("@/") or package.endswith("/")):
        return ("npm_deploy: invalid package '" + package +
                "': a scoped name is @scope/name")
    if not scoped and "/" in package:
        return ("npm_deploy: invalid package '" + package +
                "': only scoped names contain '/'")
    if access == "restricted" and not scoped:
        return ("npm_deploy: access 'restricted' needs a scoped package, " +
                "got unscoped '" + package + "'")
    return ""

def npm_provenance_error(access, registry_url, provenance):
    """Validates one npm provenance policy against access and registry_url."""
    if provenance not in ["required", "none"]:
        return ("npm_deploy: invalid provenance '" + str(provenance) +
                "': want 'required' or 'none'")
    if provenance == "required" and access != "public":
        return ("npm_deploy: provenance 'required' is not supported with access '" +
                access + "'; use access 'public' or provenance 'none'")
    if provenance == "required" and registry_url.removeprefix("https://").split("/")[0] != "registry.npmjs.org":
        return ("npm_deploy: provenance 'required' is not supported by registry '" +
                registry_url + "'; use https://registry.npmjs.org or provenance 'none'")
    return ""
