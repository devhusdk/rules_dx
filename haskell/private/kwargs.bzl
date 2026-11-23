load("//haskell/private/toolchain:pins.bzl", "CELLS")
load("//core:platform_support.bzl", "cells_support", "compatible_with", "lang_cells")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_as_errors")

HASKELL_CELLS = lang_cells("haskell")

PLATFORM_SUPPORT = cells_support("//haskell/private:defs.bzl", "HASKELL_COMPATIBLE_WITH", CELLS, HASKELL_CELLS, "//haskell/private/toolchain", unsupported = "haskell")

HASKELL_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

STANDARD_GHCOPTS = ["-Wall"]

STRICT_GHCOPTS = [
    "-Wall",
    "-Wcompat",
    "-Wredundant-constraints",
    "-Widentities",
    "-Wredundant-bang-patterns",
    "-Wunticked-promoted-constructors",
    "-Woperator-whitespace",
    "-Wincomplete-uni-patterns",
    "-Wincomplete-record-updates",
    "-Wx-partial",
    "-Wpartial-fields",
    "-Wmissing-deriving-strategies",
    "-Wunused-packages",
    "-Wmissing-home-modules",
]

STRICT_LIBRARY_GHCOPTS = STRICT_GHCOPTS + ["-Wmissing-export-lists"]

DEFAULT_GHCOPTS = select({
    Label("//config:config_tier_strict"): STRICT_GHCOPTS,
    "//conditions:default": STANDARD_GHCOPTS,
})

DEFAULT_LIBRARY_GHCOPTS = select({
    Label("//config:config_tier_strict"): STRICT_LIBRARY_GHCOPTS,
    "//conditions:default": STANDARD_GHCOPTS,
})

_WARNINGS_AS_ERRORS_GHCOPTS = ["-Werror"]

def haskell_upstream_kwargs(kwargs, library = False):
    """Returns the rules_haskell kwargs with the tier warnings, the user ghcopts and the warnings-as-errors flag."""
    upstream, warning_flags = dx_warnings_as_errors(kwargs, _WARNINGS_AS_ERRORS_GHCOPTS)
    defaults = DEFAULT_LIBRARY_GHCOPTS if library else DEFAULT_GHCOPTS
    upstream["ghcopts"] = defaults + (upstream.get("ghcopts") or []) + warning_flags
    return upstream

def haskell_kwargs(kwargs):
    """Returns kwargs with the Haskell platform guard appended."""
    return dx_language_kwargs(kwargs, [], HASKELL_COMPATIBLE_WITH)
