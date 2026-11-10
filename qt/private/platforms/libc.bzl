"""glibc versions a Qt Linux cell accepts."""

load("@llvm//constraints/libc:libc_versions.bzl", "GLIBC_VERSIONS")

def _key(version):
    return [int(part) for part in version.split(".")]

def glibc_at_least(floor):
    """Returns the llvm glibc versions at or above floor."""
    return [version for version in GLIBC_VERSIONS if _key(version) >= _key(floor)]
