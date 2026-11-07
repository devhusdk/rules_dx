"""Empty replacement for the toolchains.bzl that winsdk_configure generates."""

def register_local_rc_exe_toolchains():
    """Registers nothing; .rc sources resolve to bazel_tools' empty_rc_toolchain."""
    pass
