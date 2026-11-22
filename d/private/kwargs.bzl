"""Platform guard and strict compile flags for the D macros."""

load("//core:platform_support.bzl", "cells_support", "compatible_with", "lang_cells")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_flags")
load("//d/private/toolchain:pins.bzl", "LDC_CELLS")

D_CELLS = sorted(LDC_CELLS.keys())

PLATFORM_SUPPORT = cells_support("//d/private:defs.bzl", "D_COMPATIBLE_WITH", LDC_CELLS, lang_cells("d"), "//d/private/toolchain", unsupported = "d")

D_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

# ldc2 takes each flag with one or two dashes.
_UNCHECKED_DOPTS = ["-release", "--release", "-boundscheck=off", "--boundscheck=off"]

def d_unchecked_dopts(dopts):
    """Returns the dopts that turn off bounds, assert or contract checks."""

    # A select() hides its branches from the macro, so only a plain list is checked.
    if type(dopts) != "list":
        return []
    return [flag for flag in dopts if flag in _UNCHECKED_DOPTS]

def d_kwargs(kwargs):
    """Returns kwargs with strict warnings and the D platform guard appended."""
    out = dict(kwargs)
    unchecked = d_unchecked_dopts(out.get("dopts"))
    if unchecked and not out.pop("unchecked_dopts", False):
        fail("dopts " + ", ".join(unchecked) + " turns off bounds and contract checks; set unchecked_dopts = True on the target to allow it")
    out.pop("unchecked_dopts", None)
    return dx_language_kwargs(dx_warnings_flags(out, "dopts", ["-w", "-de"]), [], D_COMPATIBLE_WITH)
