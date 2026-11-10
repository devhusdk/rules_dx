"""Validators for the archive deploy rule."""

CONSUMER_OSES = ["linux", "macos", "windows"]

CONSUMER_LINKAGES = ["static", "dynamic"]

CONSUMER_ARCHES = ["x86_64", "aarch64"]

CONSUMER_FORMATS = ["tar.gz", "zip"]
def consumer_identity_error(arch, os, linkage, min_runtime, format = "tar.gz"):
    """Validates one consumer archive identity, returning an error or empty."""
    if arch not in CONSUMER_ARCHES:
        return ("consumer_archive: unsupported arch '" + str(arch) +
                "': want one of " + ", ".join(CONSUMER_ARCHES))
    if os not in CONSUMER_OSES:
        return ("consumer_archive: unsupported os '" + str(os) +
                "': want one of " + ", ".join(CONSUMER_OSES))
    if linkage not in CONSUMER_LINKAGES:
        return ("consumer_archive: unsupported linkage '" + str(linkage) +
                "': want one of " + ", ".join(CONSUMER_LINKAGES))
    if type(min_runtime) != "string" or min_runtime == "":
        return "consumer_archive: min_runtime must be a non-empty requirement"
    if format not in CONSUMER_FORMATS:
        return ("consumer_archive: unsupported format '" + str(format) +
                "': want one of " + ", ".join(CONSUMER_FORMATS))
    return ""
