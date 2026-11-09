load("//haskell/toolchain:pins.bzl", "CELLS", "qualified_cells")
load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_language_kwargs", "dx_warnings_as_errors")

HASKELL_CELLS = qualified_cells()

HASKELL_COMPATIBLE_WITH = cells_compatible_with(CELLS, "//haskell/toolchain", HASKELL_CELLS, unsupported = True)

STANDARD_GHCOPTS = ["-Wall"]

STRICT_GHCOPTS = ["-Wall", "-Wcompat", "-Wredundant-constraints", "-Wpartial-fields", "-Widentities"]

DEFAULT_GHCOPTS = select({
    Label("//config:config_profile_strict"): STRICT_GHCOPTS,
    "//conditions:default": STANDARD_GHCOPTS,
})

_WARNINGS_AS_ERRORS_GHCOPTS = ["-Werror"]

def haskell_upstream_kwargs(kwargs):
    """Returns the rules_haskell kwargs with ghcopts plus the warnings-as-errors flag."""
    upstream, warning_flags = dx_warnings_as_errors(kwargs, _WARNINGS_AS_ERRORS_GHCOPTS)
    ghcopts = upstream.get("ghcopts")
    upstream["ghcopts"] = (DEFAULT_GHCOPTS if ghcopts == None else ghcopts) + warning_flags
    return upstream

def haskell_kwargs(kwargs):
    """Returns kwargs with the Haskell platform guard appended."""
    return dx_language_kwargs(kwargs, [], HASKELL_COMPATIBLE_WITH)
