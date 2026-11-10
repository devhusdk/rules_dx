load("//fortran/private/toolchain:pins.bzl", "FORTRAN_CELLS")
load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_language_kwargs")

FORTRAN_COMPATIBLE_WITH = cells_compatible_with(FORTRAN_CELLS, "//fortran/private/toolchain", sorted(FORTRAN_CELLS.keys()))

SKIP_TAGS = []

LIBRARY_ATTRS = ["copts", "defines", "deps", "hdrs", "includes", "srcs"]

SHARED_ATTRS = ["tags", "target_compatible_with", "testonly"]

def fortran_with_defaults(kwargs):
    """Returns kwargs with the qualified Fortran cell appended to target_compatible_with and the quality skip tags."""
    out = dx_language_kwargs(kwargs, SKIP_TAGS, FORTRAN_COMPATIBLE_WITH)
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
