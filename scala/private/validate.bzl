load("//core:wrapper.bzl", "dx_append_flags", "dx_warnings_as_errors")

def scala_scalacopts_with_warnings_as_errors(kwargs):
    """Returns kwargs with -Xfatal-warnings on scalacopts unless turned off."""
    out, warning_flags = dx_warnings_as_errors(kwargs, ["-Xfatal-warnings"])
    return dx_append_flags(out, "scalacopts", warning_flags)
