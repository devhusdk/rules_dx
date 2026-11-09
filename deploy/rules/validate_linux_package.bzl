"""Validators for the linux_package deploy rule."""

load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

LINUX_PACKAGE_FORMATS = ["deb", "rpm", "apk"]

LINUX_PACKAGE_ARCHES = {"aarch64": "arm64", "x86_64": "amd64"}

def linux_package_arch_error(os, cpu):
    """Validates that the platform is a Linux platform the packages support."""
    if os != "linux":
        return "linux_package_deploy: unsupported os '" + os + "': want linux"
    if cpu not in LINUX_PACKAGE_ARCHES:
        return ("linux_package_deploy: unsupported cpu '" + cpu +
                "': want one of " + ", ".join(sorted(LINUX_PACKAGE_ARCHES)))
    return ""

def linux_package_name_error(package):
    """Validates one package name value."""
    if type(package) != "string" or package == "":
        return ("linux_package_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'demo')")
    if not only_chars(package, NAME_CHARS):
        return ("linux_package_deploy: invalid package name '" + package +
                "': want only [A-Za-z0-9._-]")
    return ""

def linux_package_version_error(version):
    """Validates one package version value."""
    if (type(version) != "string" or version == "" or not version[0].isdigit() or
        not only_chars(version, VERSION_CHARS)):
        return ("linux_package_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '1.2.3')")
    return ""

def linux_package_formats_error(formats):
    """Validates the list of package formats."""
    if type(formats) != "list" or len(formats) == 0:
        return "linux_package_deploy: want at least one format of " + str(LINUX_PACKAGE_FORMATS)
    seen = []
    for fmt in formats:
        if fmt not in LINUX_PACKAGE_FORMATS:
            return ("linux_package_deploy: invalid format '" + str(fmt) +
                    "': want one of " + str(LINUX_PACKAGE_FORMATS))
        if fmt in seen:
            return "linux_package_deploy: duplicate format '" + fmt + "'"
        seen.append(fmt)
    return ""

def linux_package_dst_error(dst):
    """Validates one install path inside the package."""
    if type(dst) != "string" or not dst.startswith("/") or dst.endswith("/"):
        return ("linux_package_deploy: invalid install path '" + str(dst) +
                "': want an absolute file path (for example '/usr/bin/demo')")
    return ""
