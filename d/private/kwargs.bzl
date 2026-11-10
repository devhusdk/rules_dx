load("//d/private/toolchain:pins.bzl", "LDC_CELLS")
load("//core:cells.bzl", "cells_compatible_with")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_flags")

D_CELLS = sorted(LDC_CELLS.keys())

D_COMPATIBLE_WITH = cells_compatible_with(LDC_CELLS, "//d/private/toolchain", D_CELLS, unsupported = True)

def d_kwargs(kwargs):
    """Returns kwargs with the D platform guard appended."""
    return dx_language_kwargs(dx_warnings_flags(kwargs, "dopts", ["-w"]), [], D_COMPATIBLE_WITH)
