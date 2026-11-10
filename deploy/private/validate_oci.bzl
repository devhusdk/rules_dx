"""Validators for the oci deploy rule."""

load(":validate.bzl", "LOWER_ALNUM", "NAME_CHARS", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

_OCI_TAG_MAX = 128

_VALID_REGISTRY_CHARS = NAME_CHARS + "/:"

_VALID_REPOSITORY_CHARS = LOWER_ALNUM + "._-/"

OCI_DEFAULT_REGISTRY = "ghcr.io"

def oci_tag_error(tag):
    """Validates one image tag value."""
    if type(tag) != "string" or tag == "":
        return ("oci_deploy: invalid image_tag '" + str(tag) +
                "': want a non-empty tag (for example 'latest')")
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("oci_deploy: invalid image_tag '" + tag +
                "': want only [A-Za-z0-9._-]")
    if tag[0] in ".-":
        return ("oci_deploy: invalid image_tag '" + tag +
                "': want a tag starting with [A-Za-z0-9_]")
    if len(tag) > _OCI_TAG_MAX:
        return ("oci_deploy: invalid image_tag '" + tag +
                "': want at most " + str(_OCI_TAG_MAX) + " characters")
    return ""

def oci_registry_error(registry):
    """Validates one registry value."""
    if type(registry) != "string" or registry == "":
        return ("oci_deploy: invalid registry '" + str(registry) +
                "': want a non-empty registry (for example '" +
                OCI_DEFAULT_REGISTRY + "')")
    if not only_chars(registry, _VALID_REGISTRY_CHARS):
        return ("oci_deploy: invalid registry '" + registry +
                "': want only [A-Za-z0-9._-/:]")
    if registry.startswith("/") or registry.endswith("/"):
        return ("oci_deploy: invalid registry '" + registry +
                "': want no leading or trailing slash")
    if "://" in registry:
        return ("oci_deploy: invalid registry '" + registry +
                "': want a host plus optional path, never a URL scheme")
    if "/" in registry:
        path = registry.split("/", 1)[1]
        if "//" in path:
            return ("oci_deploy: invalid registry '" + registry +
                    "': want no empty path segments")
        if path != path.lower():
            return ("oci_deploy: invalid registry '" + registry +
                    "': want a lowercase path after the host")
        for segment in path.split("/"):
            error = _oci_repository_segment_error(segment)
            if error != "":
                return "oci_deploy: invalid registry '" + registry + "': " + error
    return ""

def _oci_repository_segment_error(segment):
    """Returns "" or a short reason a path segment breaks the OCI grammar."""
    if segment[0] not in LOWER_ALNUM or segment[-1] not in LOWER_ALNUM:
        return "want each path segment to start and end with [a-z0-9]"
    run = ""
    for c in segment.elems():
        if c in LOWER_ALNUM:
            if run not in ["", ".", "_", "__"] and run.strip("-") != "":
                return "want separators to be '.', '_', '__' or dashes only"
            run = ""
        else:
            run += c
    return ""

def oci_repository_error(repository, from_name = False):
    """Validates one repository value."""
    error = _oci_repository_error(repository)
    if error != "" and from_name:
        error += ("; repository defaults to the target name, " +
                  "set repository = '...' explicitly")
    return error

def _oci_repository_error(repository):
    if type(repository) != "string" or repository == "":
        return ("oci_deploy: invalid repository '" + str(repository) +
                "': want a non-empty repository (for example 'oci_demo')")
    if repository != repository.lower():
        return ("oci_deploy: invalid repository '" + repository +
                "': want lowercase (for example '" + repository.lower() + "')")
    if not only_chars(repository, _VALID_REPOSITORY_CHARS):
        return ("oci_deploy: invalid repository '" + repository +
                "': want only [a-z0-9._-/]")
    if repository.startswith("/") or repository.endswith("/"):
        return ("oci_deploy: invalid repository '" + repository +
                "': want no leading or trailing slash")
    if "//" in repository:
        return ("oci_deploy: invalid repository '" + repository +
                "': want no empty path segments")
    for segment in repository.split("/"):
        reason = _oci_repository_segment_error(segment)
        if reason != "":
            return ("oci_deploy: invalid repository '" + repository +
                    "': " + reason)
    return ""

OCI_PLATFORMS = ["linux/amd64", "linux/arm64"]

def oci_platform_error(platform):
    """Validates one image platform value."""
    if platform not in OCI_PLATFORMS:
        return ("oci_deploy: invalid platform '" + str(platform) +
                "': want one of " + ", ".join(OCI_PLATFORMS))
    return ""

def oci_platform(id):
    """Returns the OCI platform of a platform identity, or empty when it has none."""
    if id.os != "linux":
        return ""
    return "linux/" + {"aarch64": "arm64", "x86_64": "amd64"}.get(id.cpu, id.cpu)

def oci_image_error(filename, is_directory = False):
    """Validates one image archive filename or layout directory name."""
    if type(filename) != "string" or filename == "":
        return ("oci_deploy: invalid image_tar '" + str(filename) +
                "': want a non-empty .tar filename or OCI layout directory")
    if not is_directory and not (filename.endswith(".tar") or filename.endswith(".tar.gz")):
        return ("oci_deploy: invalid image_tar '" + filename +
                "': want a filename ending in .tar or .tar.gz")
    return ""
