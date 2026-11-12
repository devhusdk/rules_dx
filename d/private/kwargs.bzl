load("//d/private/toolchain:pins.bzl", "LDC_CELLS")
load("//core:platform_support.bzl", "cells_support", "compatible_with", "lang_cells")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_flags")

D_CELLS = sorted(LDC_CELLS.keys())

PLATFORM_SUPPORT = cells_support("//d/private:defs.bzl", "D_COMPATIBLE_WITH", LDC_CELLS, lang_cells("d"), "//d/private/toolchain")

D_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

def d_kwargs(kwargs):
    """Returns kwargs with the D platform guard appended."""
    return dx_language_kwargs(dx_warnings_flags(kwargs, "dopts", ["-w"]), [], D_COMPATIBLE_WITH)
