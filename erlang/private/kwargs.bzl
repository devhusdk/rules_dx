load("//erlang/private/toolchain:pins.bzl", "CELLS", "OTP_UNPORTED_CELLS")
load("//core:platform_support.bzl", "cells_support", "compatible_with")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_without_warnings_attr")

ERLANG_CELLS = sorted([cell for cell in CELLS.keys() if cell not in OTP_UNPORTED_CELLS])

PLATFORM_SUPPORT = cells_support("//erlang:defs.bzl", "ERLANG_COMPATIBLE_WITH", CELLS, ERLANG_CELLS, "//erlang/private/toolchain")

ERLANG_COMPATIBLE_WITH = compatible_with(PLATFORM_SUPPORT)

def erlang_app_forward_kwargs(testonly, nifs, tags, kwargs):
    """Returns the kwargs the public erlang_app forwarder keeps."""
    out = {"testonly": True} if testonly else {}
    if nifs:
        out["nifs"] = nifs
    if tags != None:
        out["tags"] = tags
    if kwargs.get("target_compatible_with", None) != None:
        out["target_compatible_with"] = kwargs["target_compatible_with"]
    return out

def erlang_kwargs(kwargs):
    """Returns kwargs with the Erlang platform guard appended."""
    return dx_language_kwargs(dx_without_warnings_attr(kwargs), [], ERLANG_COMPATIBLE_WITH)
