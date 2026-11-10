"""Keyword helpers for the Erlang wrappers."""

load("//core:platform_support.bzl", "cells_support", "compatible_with")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_as_errors", "dx_without_warnings_attr")
load("//erlang/private/toolchain:pins.bzl", "CELLS", "OTP_UNPORTED_CELLS")

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

_WERROR = ["-Werror"]

_PREBUILT_ATTRS = ("beam_files", "hdrs", "srcs", "priv", "license_files")

def _without_werror(opts):
    return [opt for opt in opts if opt not in _WERROR]

def erlang_default_erlc_opts(defaults):
    """Returns the default erlc options without -Werror, dropping +deterministic in debug builds."""
    opts = _without_werror(defaults)
    return select({
        Label("@rules_erlang//:debug_build"): [opt for opt in opts if opt != "+deterministic"],
        "//conditions:default": opts,
    })

def erlang_erlc_kwargs(kwargs, defaults):
    """Returns erlang_kwargs with -Werror on erlc_opts following warnings_as_errors; apps with listed sources keep theirs."""
    if any([kwargs.get(attr) != None for attr in _PREBUILT_ATTRS]):
        return erlang_kwargs(kwargs)
    rest, flag = dx_warnings_as_errors(kwargs, _WERROR)
    opts = rest.get("erlc_opts")
    if opts == None:
        opts = erlang_default_erlc_opts(defaults)
    elif type(opts) == "list":
        opts = _without_werror(opts)
    rest["erlc_opts"] = opts + flag
    return erlang_kwargs(rest)

def erlang_bytecode_kwargs(kwargs):
    """Returns the erlang_bytecode kwargs without erlc_opts, the erlc_opts target it named, and the -Werror value."""
    rest, flag = dx_warnings_as_errors(kwargs, _WERROR)
    base = rest.pop("erlc_opts", None)
    return erlang_kwargs(rest), base, flag

def nif_library_name(name):
    """Returns the name of the shared library target one NIF links."""
    return "_lib" + name + ".so"

def release_priv_path(package_path, short_path):
    """Returns where one priv file lands below priv in a release: its path below the app's priv directory, else its file name."""
    rel = short_path.removeprefix(package_path + "/") if package_path else short_path
    if rel.startswith("priv/"):
        return rel.removeprefix("priv/")
    return short_path.rpartition("/")[2]
