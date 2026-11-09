load("//d/toolchain:pins.bzl", "LDC_CELLS")
load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_language_kwargs")

D_CELLS = sorted(LDC_CELLS.keys())

D_COMPATIBLE_WITH = cells_compatible_with(LDC_CELLS, "//d/toolchain", D_CELLS, unsupported = True)

def d_kwargs(kwargs):
    """Returns kwargs with the D platform guard appended."""
    return dx_language_kwargs(kwargs, [], D_COMPATIBLE_WITH)
