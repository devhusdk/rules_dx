load("//fortran/toolchain:pins.bzl", "FORTRAN_CELLS")
load("//libs/starlark:cells.bzl", "cells_compatible_with")

FORTRAN_COMPATIBLE_WITH = cells_compatible_with(FORTRAN_CELLS, "//fortran/toolchain", sorted(FORTRAN_CELLS.keys()))

SKIP_TAGS = []

LIBRARY_ATTRS = ["copts", "defines", "deps", "hdrs", "includes", "srcs"]

SHARED_ATTRS = ["tags", "target_compatible_with", "testonly"]

def fortran_with_defaults(kwargs):
    """Returns kwargs with the qualified Fortran cell and the quality skip tags as defaults."""
    out = dict(kwargs)
    if out.get("target_compatible_with") == None:
        out["target_compatible_with"] = FORTRAN_COMPATIBLE_WITH
    out["copts"] = (out.get("copts") or []) + select({
        Label("//fortran/rules:kcov_build"): ["-g"],
        "//conditions:default": [],
    })
    tags = list(out.get("tags") or [])
    out["tags"] = tags + [tag for tag in SKIP_TAGS if tag not in tags]
    return out

def fortran_library_kwargs(kwargs):
    """Returns the attributes of an executable that the Fortran library compiles with."""
    return {key: value for key, value in kwargs.items() if key in LIBRARY_ATTRS or key in SHARED_ATTRS}

def fortran_link_kwargs(kwargs):
    """Returns the attributes of an executable that the C++ link rule takes."""
    return {key: value for key, value in kwargs.items() if key not in LIBRARY_ATTRS}
