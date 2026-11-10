load("//fortran/private/toolchain:pins.bzl", "FORTRAN_CELLS")
load("//core:platform_support.bzl", "cells_support", "compatible_with")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_flags")

PLATFORM_SUPPORT = cells_support("//fortran:defs.bzl", "FORTRAN_COMPATIBLE_WITH", FORTRAN_CELLS, sorted(FORTRAN_CELLS.keys()), "//fortran/private/toolchain", "constraints", unsupported = False)

FORTRAN_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

SKIP_TAGS = []

LIBRARY_ATTRS = ["copts", "defines", "deps", "hdrs", "includes", "srcs"]

SHARED_ATTRS = ["tags", "target_compatible_with", "testonly"]

def fortran_with_defaults(kwargs):
    """Returns kwargs with the qualified Fortran cell appended to target_compatible_with and the quality skip tags."""
    out = dx_language_kwargs(dx_warnings_flags(kwargs, "copts", ["-Werror"]), SKIP_TAGS, FORTRAN_COMPATIBLE_WITH)
    out["copts"] = (out.get("copts") or []) + select({
        Label("//fortran/private:kcov_build"): ["-g"],
        "//conditions:default": [],
    })
    return out

def fortran_library_kwargs(kwargs):
    """Returns the attributes of an executable that the Fortran library compiles with."""
    return {key: value for key, value in kwargs.items() if key in LIBRARY_ATTRS or key in SHARED_ATTRS}

def fortran_link_kwargs(kwargs):
    """Returns the attributes of an executable that the C++ link rule takes."""
    return {key: value for key, value in kwargs.items() if key not in LIBRARY_ATTRS}
