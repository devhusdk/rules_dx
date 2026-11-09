"""The host cell of a repository rule: one `<os>_<cpu>` spelling for every toolchain."""

_OSES = {"linux": "linux", "mac os x": "macos"}

_ARCHES = {"aarch64": "arm64", "amd64": "x86_64", "arm64": "arm64", "x86_64": "x86_64"}

def host_cell_of(os_name, arch):
    """Returns `linux|macos|windows` + `_` + `x86_64|arm64` from Bazel's os name and arch."""
    name = os_name.lower()
    name = "windows" if name.startswith("windows") else _OSES.get(name, name)
    return name + "_" + _ARCHES.get(arch, arch)

def host_cell(rctx):
    """Returns the host cell of a repository_ctx or module_ctx."""
    return host_cell_of(rctx.os.name, rctx.os.arch)
