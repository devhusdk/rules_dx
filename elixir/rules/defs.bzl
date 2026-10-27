"""Thin Elixir wrappers over rules_elixir."""

load("@rules_elixir//:elixir_app.bzl", _elixir_app = "elixir_app")
load("@rules_elixir//:ex_unit_test.bzl", _ex_unit_test = "ex_unit_test")
load("@rules_erlang//:erlang_app_info.bzl", _ErlangAppInfo = "ErlangAppInfo")
load("//elixir/toolchain:pins.bzl", _ELIXIR_CELLS = "ELIXIR_CELLS")
load("//erlang/rules:defs.bzl", _escript_archive = "escript_archive")
load("//erlang/toolchain:cells.bzl", "otp_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

# False while elixir tests write empty LCOV under `bazel coverage`; the public
# test then carries dx-no-coverage-collector and `dx coverage` fails loud (#2308).
ELIXIR_COVERAGE_COLLECTOR = False

ErlangAppInfo = _ErlangAppInfo

ELIXIR_CELLS = _ELIXIR_CELLS

ELIXIR_COMPATIBLE_WITH = otp_compatible_with(ELIXIR_CELLS)

ELIXIR_SOURCE_GLOBS = ["lib/**/*.ex"]

DEFAULT_ELIXIRC_OPTS = ["--warnings-as-errors"]

escript_archive = _escript_archive

def escript_headers(module):
    """Returns escript_archive headers that start one Elixir module in a UTF-8 VM."""
    return ["shebang", '{emu_args, "+fnu -kernel standard_io_encoding unicode -escript main Elixir.' + module + '"}']

_SPECS = [("elixir", ["ex", "exs"])]

_EXTS = [".ex", ".exs"]

_elixir_app_forward = dx_library_forward_rule(
    provides = [_ErlangAppInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_ErlangAppInfo, "ErlangAppInfo")],
    quality_specs = _SPECS,
    what = "elixir_app",
    allow_files = _EXTS,
    upstream_providers = [[_ErlangAppInfo]],
    runtime = "besteffort",
)

_ex_unit_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _SPECS,
    what = "ex_unit_test",
    allow_files = _EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "besteffort",
)

def app_sources(kwargs, glob = None):
    """Returns the source files one elixir_app owns for its attributes."""
    if kwargs.get("srcs") != None:
        return kwargs["srcs"]
    glob = glob if glob != None else native.glob
    return glob(ELIXIR_SOURCE_GLOBS, allow_empty = True)

def app_mod_entry(module):
    """Returns the .app mod entry that starts one Elixir application module."""
    return "{mod, {'Elixir." + module + "', []}}"

def app_kwargs(kwargs, srcs):
    """Returns the upstream elixir_app kwargs with dx defaults filled in."""
    upstream = dict(kwargs)
    upstream["srcs"] = srcs
    if upstream.get("elixirc_opts") == None:
        upstream["elixirc_opts"] = DEFAULT_ELIXIRC_OPTS
    module = upstream.pop("app_module", None)
    if module != None:
        extra = upstream.get("app_extra_keys")
        upstream["app_extra_keys"] = app_mod_entry(module) + (",\n" + extra if extra else "")
    return upstream

def elixir_app(name = "erlang_app", visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_elixir elixir_app macro."""
    tags = kwargs.pop("tags", None)
    owned = app_sources(kwargs)
    _elixir_app(name = name + "_upstream", testonly = testonly, tags = dx_upstream_tags(tags), **app_kwargs(kwargs, owned))
    forward_kwargs = {"testonly": True} if testonly else {}
    if tags != None:
        forward_kwargs["tags"] = tags
    if kwargs.get("target_compatible_with") != None:
        forward_kwargs["target_compatible_with"] = kwargs["target_compatible_with"]
    _elixir_app_forward(
        name = name,
        upstream = ":" + name + "_upstream",
        srcs = owned,
        visibility = visibility,
        **forward_kwargs
    )

def helper_first(srcs):
    """Returns test srcs with test_helper.exs files required first."""
    if type(srcs) != "list":
        return srcs
    helpers = [src for src in srcs if src.split("/")[-1].split(":")[-1] == "test_helper.exs"]
    return helpers + [src for src in srcs if src not in helpers]

def ex_unit_test(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over the rules_elixir ex_unit_test rule."""
    dx_wrap_test(name, _ex_unit_test, _ex_unit_forward_test, helper_first(srcs), visibility = visibility, coverage_collector = ELIXIR_COVERAGE_COLLECTOR, **kwargs)
