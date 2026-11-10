load("//elixir/private/toolchain:pins.bzl", _ELIXIR_CELLS = "ELIXIR_CELLS")
load("//erlang/private/toolchain:cells.bzl", "otp_compatible_with")
load("//core:wrapper.bzl", "dx_language_kwargs", "dx_warnings_as_errors")

ELIXIR_CELLS = _ELIXIR_CELLS

ELIXIR_COMPATIBLE_WITH = otp_compatible_with(ELIXIR_CELLS)

_WARNINGS_AS_ERRORS_ELIXIRC_OPTS = ["--warnings-as-errors"]

def elixir_app_mod_entry(module):
    """Returns the .app mod entry that starts one Elixir application module."""
    return "{mod, {'Elixir." + module + "', []}}"

def elixir_app_kwargs(kwargs, srcs):
    """Returns the upstream elixir_app kwargs with dx defaults filled in."""
    upstream, warning_flags = dx_warnings_as_errors(kwargs, _WARNINGS_AS_ERRORS_ELIXIRC_OPTS)
    upstream["srcs"] = srcs
    upstream["elixirc_opts"] = list(upstream.get("elixirc_opts") or []) + warning_flags
    module = upstream.pop("app_module", None)
    if module != None:
        extra = upstream.get("app_extra_keys")
        upstream["app_extra_keys"] = elixir_app_mod_entry(module) + (",\n" + extra if extra else "")
    return upstream

def elixir_kwargs(kwargs):
    """Returns kwargs with the Elixir platform guard appended."""
    return dx_language_kwargs(kwargs, [], ELIXIR_COMPATIBLE_WITH)

COVER_LOAD = "if path = System.get_env(~s(DX_ELIXIR_COVER_SCRIPT)), do: Code.require_file(path, System.fetch_env!(~s(TEST_SRCDIR)))"

def elixir_cover_opts(opts):
    """Returns the elixir options that load the coverage script when a coverage run names one."""
    load_script = select({
        "@platforms//os:windows": ["-e", '"' + COVER_LOAD + '"'],
        "//conditions:default": ["-e", COVER_LOAD],
    })
    return load_script + (opts or [])
